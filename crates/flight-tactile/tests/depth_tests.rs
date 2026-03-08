// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the tactile / haptic feedback engine
//!
//! Covers: motor control, effect zones, effect patterns, safety,
//! configuration, and integration with the telemetry pipeline.

use std::sync::Arc;
use std::time::Duration;
use parking_lot::RwLock;

use flight_bus::{
    AircraftId, BusSnapshot, EngineData, HeloData, Percentage, SimId, ValidatedAngle,
    ValidatedSpeed,
};
use flight_tactile::engine::{MAX_EFFECTS, TICK_RATE_HZ, TactileEffect, TactileEngine, TexturePattern};
use flight_tactile::effects::{EffectEvent, EffectIntensity, EffectProcessor, EffectType};
use flight_tactile::mixer::{TactileMixer, FrequencyBand};
use flight_tactile::channel::{ChannelId, ChannelMapping, ChannelRouter};
use flight_tactile::presets::TactilePresets;
use flight_tactile::{TactileConfig, TactileManager, TactileBridge};

// ── Helpers ──────────────────────────────────────────────────────────────

fn make_snapshot() -> BusSnapshot {
    BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"))
}

fn make_engine(running: bool, rpm_pct: f32) -> EngineData {
    EngineData {
        index: 0,
        running,
        rpm: Percentage::new(rpm_pct).unwrap(),
        manifold_pressure: None,
        egt: None,
        cht: None,
        fuel_flow: None,
        oil_pressure: None,
        oil_temperature: None,
    }
}

fn make_helo(nr: f32, torque: f32) -> HeloData {
    HeloData {
        nr: Percentage::new(nr).unwrap(),
        np: Percentage::new(100.0).unwrap(),
        torque: Percentage::new(torque).unwrap(),
        collective: Percentage::new(50.0).unwrap(),
        pedals: 0.0,
    }
}

/// Advance the engine N ticks and collect all samples.
fn collect_samples(engine: &mut TactileEngine, n: usize) -> Vec<f64> {
    (0..n).map(|_| engine.tick()).collect()
}

/// Advance the mixer N ticks and collect combined output.
fn collect_mixer_samples(mixer: &mut TactileMixer, n: usize) -> Vec<f64> {
    (0..n).map(|_| mixer.tick().combined).collect()
}

// ═══════════════════════════════════════════════════════════════════════
// 1. Motor driver tests
// ═══════════════════════════════════════════════════════════════════════

mod motor_driver {
    use super::*;

    #[test]
    fn motor_pulse_single_tick() {
        let mut engine = TactileEngine::new();
        engine.add_effect(TactileEffect::Rumble {
            frequency_hz: 100.0,
            amplitude: 0.8,
            duration_ticks: 1,
        });
        let sample = engine.tick();
        assert!((-1.0..=1.0).contains(&sample));
        assert_eq!(engine.active_count(), 0, "single-tick rumble must expire");
    }

    #[test]
    fn motor_continuous_vibration() {
        let mut engine = TactileEngine::new();
        engine.add_effect(TactileEffect::Engine {
            rpm: 1800.0,
            amplitude: 0.5,
        });

        let mut any_nonzero = false;
        for _ in 0..500 {
            let v = engine.tick();
            assert!((-1.0..=1.0).contains(&v));
            if v.abs() > 0.01 {
                any_nonzero = true;
            }
        }
        assert!(any_nonzero, "engine vibration must produce output");
        assert_eq!(engine.active_count(), 1, "engine effect never self-expires");
    }

    #[test]
    fn motor_variable_intensity() {
        let mut mixer = TactileMixer::new();
        let _slot = mixer
            .add_effect(TactileEffect::Impact {
                magnitude: 1.0,
                decay_rate: 5.0,
            })
            .unwrap();

        let out_full = mixer.tick();
        assert!((out_full.combined - 1.0).abs() < 0.02);

        let _slot2 = mixer
            .add_effect_with_gain(
                TactileEffect::Impact {
                    magnitude: 1.0,
                    decay_rate: 5.0,
                },
                0.5,
            )
            .unwrap();
        let out = mixer.tick();
        assert!(out.combined > 0.1, "should have non-zero output");
    }

    #[test]
    fn motor_duty_cycle_bounded_duration() {
        let mut engine = TactileEngine::new();
        let ticks = 50u32;
        engine.add_effect(TactileEffect::Rumble {
            frequency_hz: 40.0,
            amplitude: 0.6,
            duration_ticks: ticks,
        });

        for _ in 0..ticks {
            engine.tick();
        }
        assert_eq!(engine.active_count(), 0, "rumble must stop after duration");
    }

    #[test]
    fn motor_enable_disable_via_manager() {
        let mut mgr = TactileManager::new();
        mgr.initialize(TactileConfig::default()).unwrap();

        mgr.set_enabled(false);
        assert!(!mgr.is_enabled());
        let snap = make_snapshot();
        assert!(mgr.process_telemetry(&snap).is_ok());

        mgr.set_enabled(true);
        assert!(mgr.is_enabled());
    }

    #[test]
    fn motor_multiple_simultaneous_effects() {
        let mut engine = TactileEngine::new();
        for i in 0..MAX_EFFECTS {
            assert!(
                engine
                    .add_effect(TactileEffect::Rumble {
                        frequency_hz: 20.0 + i as f64 * 5.0,
                        amplitude: 0.05,
                        duration_ticks: 100,
                    })
                    .is_some()
            );
        }
        assert_eq!(engine.active_count(), MAX_EFFECTS);

        assert!(engine
            .add_effect(TactileEffect::Impact {
                magnitude: 0.5,
                decay_rate: 5.0,
            })
            .is_none());

        let v = engine.tick();
        assert!((-1.0..=1.0).contains(&v));
    }

    #[test]
    fn set_intensity_via_slot_gain() {
        let mut mixer = TactileMixer::new();
        let slot = mixer
            .add_effect(TactileEffect::Impact { magnitude: 1.0, decay_rate: 5.0 })
            .unwrap();

        mixer.set_slot_gain(slot, 0.75);
        let out = mixer.tick();
        assert!((out.combined - 0.75).abs() < 0.02, "intensity should scale with slot gain");
    }

    #[test]
    fn frequency_control_rumble() {
        let mut engine = TactileEngine::new();
        engine.add_effect(TactileEffect::Rumble {
            frequency_hz: 100.0,
            amplitude: 1.0,
            duration_ticks: 250,
        });

        let duration_ticks = TICK_RATE_HZ as usize;
        let samples = collect_samples(&mut engine, duration_ticks);

        let mut zero_crossings = 0usize;
        let mut prev_sign: Option<f64> = None;
        const EPS: f64 = 1.0e-6;
        for s in samples {
            if s.abs() < EPS {
                continue;
            }
            let sign = s.signum();
            if let Some(ps) = prev_sign {
                if sign != ps {
                    zero_crossings += 1;
                }
            }
            prev_sign = Some(sign);
        }

        let duration_sec = duration_ticks as f64 / TICK_RATE_HZ as f64;
        let expected_zero_crossings = 2.0 * 100.0 * duration_sec;

        let lower_bound = expected_zero_crossings * 0.8;
        let upper_bound = expected_zero_crossings * 1.2;
        assert!(
            (zero_crossings as f64) >= lower_bound && (zero_crossings as f64) <= upper_bound,
            "100 Hz rumble should produce ~{expected_zero_crossings:.1} zero crossings in {duration_sec:.3} s (observed {zero_crossings})"
        );
    }

    #[test]
    fn waveform_sawtooth_covers_full_range() {
        let mut engine = TactileEngine::new();
        engine.add_effect(TactileEffect::Texture {
            frequency_hz: 25.0,
            amplitude: 1.0,
            pattern: TexturePattern::Sawtooth,
        });

        let samples = collect_samples(&mut engine, 20);
        let min = samples.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        assert!(max > 0.5, "sawtooth max should be above 0.5");
        assert!(min < -0.5, "sawtooth min should be below -0.5");
    }

    #[test]
    fn multiple_motors_via_band_separation() {
        let mut mixer = TactileMixer::new();
        mixer.add_effect(TactileEffect::Impact { magnitude: 0.5, decay_rate: 5.0 });
        mixer.add_effect(TactileEffect::Rumble {
            frequency_hz: 80.0, amplitude: 0.5, duration_ticks: 100,
        });
        mixer.add_effect(TactileEffect::Texture {
            frequency_hz: 200.0, amplitude: 0.5, pattern: TexturePattern::Square,
        });

        let out = mixer.tick();
        assert!(out.low.abs() > 0.0, "low motor should fire");
        assert_eq!(mixer.active_count(), 3, "all three motors should be active");
    }

    #[test]
    fn ramp_up_via_increasing_gain() {
        let mut mixer = TactileMixer::new();
        let slot = mixer
            .add_effect(TactileEffect::Impact { magnitude: 1.0, decay_rate: 0.1 })
            .unwrap();

        let mut prev = 0.0_f64;
        for step in 1..=5 {
            let gain = step as f64 * 0.2;
            mixer.set_slot_gain(slot, gain);
            let out = mixer.tick();
            assert!(out.combined.abs() >= prev * 0.5, "ramp up step {step}");
            prev = out.combined.abs();
        }
    }

    #[test]
    fn ramp_down_via_decreasing_gain() {
        let mut mixer = TactileMixer::new();
        let slot = mixer
            .add_effect(TactileEffect::Impact { magnitude: 1.0, decay_rate: 0.01 })
            .unwrap();

        mixer.set_slot_gain(slot, 1.0);
        let first = mixer.tick().combined.abs();

        mixer.set_slot_gain(slot, 0.5);
        let second = mixer.tick().combined.abs();

        mixer.set_slot_gain(slot, 0.1);
        let third = mixer.tick().combined.abs();

        assert!(first > second, "ramp down: first > second");
        assert!(second > third, "ramp down: second > third");
    }

    #[test]
    fn emergency_stop_clears_all() {
        let mut engine = TactileEngine::new();
        for _ in 0..MAX_EFFECTS {
            engine.add_effect(TactileEffect::Rumble {
                frequency_hz: 50.0, amplitude: 1.0, duration_ticks: 10000,
            });
        }
        assert_eq!(engine.active_count(), MAX_EFFECTS);

        engine.clear();
        assert_eq!(engine.active_count(), 0);
        assert_eq!(engine.tick(), 0.0, "after emergency stop output must be zero");
    }

    #[test]
    fn engine_effect_tracks_rpm() {
        let mut engine = TactileEngine::new();
        engine.add_effect(TactileEffect::Engine { rpm: 1800.0, amplitude: 0.5 });

        let expected_freq = 1800.0 / 60.0;
        let effect = TactileEffect::Engine { rpm: 1800.0, amplitude: 0.5 };
        assert!((effect.frequency_hz() - expected_freq).abs() < 0.01);

        let samples = collect_samples(&mut engine, 50);
        let max = samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        assert!(max > 0.1, "engine effect should produce output");
        assert!(max <= 0.5 + 1e-9, "engine output must not exceed amplitude");
    }

    #[test]
    fn engine_zero_amplitude_produces_silence() {
        let mut engine = TactileEngine::new();
        engine.add_effect(TactileEffect::Rumble {
            frequency_hz: 100.0,
            amplitude: 0.0,
            duration_ticks: 50,
        });
        for _ in 0..50 {
            assert_eq!(engine.tick(), 0.0, "zero amplitude must produce silence");
        }
    }

    #[test]
    fn engine_remove_effect_by_slot() {
        let mut engine = TactileEngine::new();
        let slot = engine
            .add_effect(TactileEffect::Engine {
                rpm: 2400.0,
                amplitude: 0.5,
            })
            .unwrap();
        assert_eq!(engine.active_count(), 1);

        engine.remove_effect(slot);
        assert_eq!(engine.active_count(), 0);
        assert_eq!(engine.tick(), 0.0, "removed effect must not produce output");
    }

    #[test]
    fn engine_remove_out_of_bounds_is_noop() {
        let mut engine = TactileEngine::new();
        engine.add_effect(TactileEffect::Impact {
            magnitude: 0.5,
            decay_rate: 5.0,
        });
        engine.remove_effect(MAX_EFFECTS + 10);
        assert_eq!(engine.active_count(), 1, "out-of-bounds remove must be no-op");
    }

    #[test]
    fn tick_count_increments_correctly() {
        let mut engine = TactileEngine::new();
        assert_eq!(engine.tick_count(), 0);
        for i in 1..=100 {
            engine.tick();
            assert_eq!(engine.tick_count(), i);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 2. Zone routing tests
// ═══════════════════════════════════════════════════════════════════════

mod zone_routing {
    use super::*;

    #[test]
    fn zone_seat_channel_receives_touchdown() {
        let mut mapping = ChannelMapping::new();
        let seat_ch = ChannelId::new(0);
        mapping.set_mapping(EffectType::Touchdown, seat_ch);

        let mut router = ChannelRouter::new(mapping);
        let evt = EffectEvent::new(EffectType::Touchdown, EffectIntensity::new(0.9).unwrap());
        let outputs = router.process_events(vec![evt]);

        let seat_out = outputs.iter().find(|o| o.channel_id == seat_ch).unwrap();
        assert!(seat_out.intensity.value() > 0.5, "seat should feel touchdown");
    }

    #[test]
    fn zone_pedal_channel_receives_ground_roll() {
        let mut mapping = ChannelMapping::new();
        let pedal_ch = ChannelId::new(1);
        mapping.set_mapping(EffectType::GroundRoll, pedal_ch);

        let mut router = ChannelRouter::new(mapping);
        let evt = EffectEvent::new(EffectType::GroundRoll, EffectIntensity::new(0.4).unwrap());
        let outputs = router.process_events(vec![evt]);

        let pedal_out = outputs.iter().find(|o| o.channel_id == pedal_ch).unwrap();
        assert!(pedal_out.intensity.value() > 0.0);
    }

    #[test]
    fn zone_stick_channel_receives_stall_buffet() {
        let mut mapping = ChannelMapping::new();
        let stick_ch = ChannelId::new(2);
        mapping.set_mapping(EffectType::StallBuffet, stick_ch);

        let mut router = ChannelRouter::new(mapping);
        let evt = EffectEvent::new(EffectType::StallBuffet, EffectIntensity::new(0.7).unwrap());
        let outputs = router.process_events(vec![evt]);

        let stick_out = outputs.iter().find(|o| o.channel_id == stick_ch).unwrap();
        assert!(stick_out.intensity.value() > 0.4);
    }

    #[test]
    fn zone_isolation_no_crosstalk() {
        let mut mapping = ChannelMapping::new();
        let seat_ch = ChannelId::new(0);
        let pedal_ch = ChannelId::new(1);
        mapping.set_mapping(EffectType::Touchdown, seat_ch);
        mapping.set_mapping(EffectType::GroundRoll, pedal_ch);

        let mut router = ChannelRouter::new(mapping);

        let evt = EffectEvent::new(EffectType::Touchdown, EffectIntensity::new(1.0).unwrap());
        let outputs = router.process_events(vec![evt]);

        let pedal_out = outputs.iter().find(|o| o.channel_id == pedal_ch).unwrap();
        assert_eq!(
            pedal_out.intensity.value(),
            0.0,
            "pedal must not receive touchdown"
        );
    }

    #[test]
    fn zone_mixing_two_effects_same_channel() {
        let mut mapping = ChannelMapping::new();
        let ch = ChannelId::new(0);
        mapping.set_mapping(EffectType::Touchdown, ch);
        mapping.set_mapping(EffectType::GroundRoll, ch);

        let mut router = ChannelRouter::new(mapping);
        let evts = vec![
            EffectEvent::new(EffectType::Touchdown, EffectIntensity::new(0.6).unwrap()),
            EffectEvent::new(EffectType::GroundRoll, EffectIntensity::new(0.3).unwrap()),
        ];
        let outputs = router.process_events(evts);

        let out = outputs.iter().find(|o| o.channel_id == ch).unwrap();
        assert!(
            (out.intensity.value() - 0.6).abs() < 0.01,
            "mixing should take the maximum"
        );
    }

    #[test]
    fn zone_priority_via_gain() {
        let mut mapping = ChannelMapping::new();
        let ch = ChannelId::new(0);
        mapping.set_mapping(EffectType::Touchdown, ch);

        mapping.set_channel_gain(ch, 0.5).unwrap();

        let mut router = ChannelRouter::new(mapping);
        let evt = EffectEvent::new(EffectType::Touchdown, EffectIntensity::new(1.0).unwrap());
        let outputs = router.process_events(vec![evt]);

        let out = outputs.iter().find(|o| o.channel_id == ch).unwrap();
        assert!(
            (out.intensity.value() - 0.5).abs() < 0.01,
            "channel gain must attenuate"
        );
    }

    #[test]
    fn map_zones_to_motors_defaults() {
        let mapping = ChannelMapping::new();
        assert_eq!(mapping.get_channel(EffectType::Touchdown), Some(ChannelId::new(0)));
        assert_eq!(mapping.get_channel(EffectType::GroundRoll), Some(ChannelId::new(1)));
        assert_eq!(mapping.get_channel(EffectType::StallBuffet), Some(ChannelId::new(2)));
        assert_eq!(mapping.get_channel(EffectType::EngineVibration), Some(ChannelId::new(3)));
        assert_eq!(mapping.get_channel(EffectType::GearWarning), Some(ChannelId::new(4)));
        assert_eq!(mapping.get_channel(EffectType::RotorVibration), Some(ChannelId::new(5)));
    }

    #[test]
    fn priority_based_routing_separates_channels() {
        let mapping = ChannelMapping::new();
        let mut router = ChannelRouter::new(mapping);

        let e1 = EffectEvent::new(EffectType::Touchdown, EffectIntensity::new(0.9).unwrap());
        let e2 = EffectEvent::new(EffectType::StallBuffet, EffectIntensity::new(0.4).unwrap());

        let outputs = router.process_events(vec![e1, e2]);

        let td = outputs.iter().find(|o| o.channel_id == ChannelId::new(0)).unwrap();
        let sb = outputs.iter().find(|o| o.channel_id == ChannelId::new(2)).unwrap();
        assert!((td.intensity.value() - 0.9).abs() < 0.01);
        assert!((sb.intensity.value() - 0.4).abs() < 0.01);
    }

    #[test]
    fn zone_overlap_two_effects_same_channel() {
        let mut mapping = ChannelMapping::new();
        mapping.set_mapping(EffectType::Touchdown, ChannelId::new(0));
        mapping.set_mapping(EffectType::GearWarning, ChannelId::new(0));

        let mut router = ChannelRouter::new(mapping);

        let e1 = EffectEvent::new(EffectType::Touchdown, EffectIntensity::new(0.6).unwrap());
        let e2 = EffectEvent::new(EffectType::GearWarning, EffectIntensity::new(0.8).unwrap());

        let outputs = router.process_events(vec![e1, e2]);
        let ch0 = outputs.iter().find(|o| o.channel_id == ChannelId::new(0)).unwrap();
        assert!((ch0.intensity.value() - 0.8).abs() < 0.01);
    }

    #[test]
    fn dead_zone_unmapped_effect_produces_zero() {
        let mut mapping = ChannelMapping::new();
        mapping.mappings.clear();
        mapping.set_mapping(EffectType::GroundRoll, ChannelId::new(1));

        let mut router = ChannelRouter::new(mapping);
        let e = EffectEvent::new(EffectType::Touchdown, EffectIntensity::new(1.0).unwrap());
        let outputs = router.process_events(vec![e]);

        let ch0 = outputs.iter().find(|o| o.channel_id == ChannelId::new(0));
        match ch0 {
            Some(o) => assert_eq!(o.intensity.value(), 0.0, "unmapped effect should yield zero"),
            None => {}
        }
    }

    #[test]
    fn zone_disable_blocks_output() {
        let mut mapping = ChannelMapping::new();
        mapping.set_channel_enabled(ChannelId::new(0), false);

        let mut router = ChannelRouter::new(mapping);
        let e = EffectEvent::new(EffectType::Touchdown, EffectIntensity::new(1.0).unwrap());
        let outputs = router.process_events(vec![e]);

        let ch0 = outputs.iter().find(|o| o.channel_id == ChannelId::new(0)).unwrap();
        assert_eq!(ch0.intensity.value(), 0.0, "disabled channel must output zero");
    }

    #[test]
    fn zone_enable_restores_output() {
        let mut mapping = ChannelMapping::new();
        mapping.set_channel_enabled(ChannelId::new(0), false);
        let mut router = ChannelRouter::new(mapping);

        let e = EffectEvent::new(EffectType::Touchdown, EffectIntensity::new(1.0).unwrap());
        let outputs = router.process_events(vec![e]);
        let ch0 = outputs.iter().find(|o| o.channel_id == ChannelId::new(0)).unwrap();
        assert_eq!(ch0.intensity.value(), 0.0);

        let mut new_mapping = ChannelMapping::new();
        new_mapping.set_channel_enabled(ChannelId::new(0), true);
        router.update_mapping(new_mapping);

        let e = EffectEvent::new(EffectType::Touchdown, EffectIntensity::new(0.7).unwrap());
        let outputs = router.process_events(vec![e]);
        let ch0 = outputs.iter().find(|o| o.channel_id == ChannelId::new(0)).unwrap();
        assert!(ch0.intensity.value() > 0.5, "re-enabled channel must route again");
    }

    #[test]
    fn channel_gain_scales_output() {
        let mut mapping = ChannelMapping::new();
        mapping.set_channel_gain(ChannelId::new(0), 0.5).unwrap();

        let mut router = ChannelRouter::new(mapping);
        let e = EffectEvent::new(EffectType::Touchdown, EffectIntensity::new(1.0).unwrap());
        let outputs = router.process_events(vec![e]);

        let ch0 = outputs.iter().find(|o| o.channel_id == ChannelId::new(0)).unwrap();
        assert!((ch0.intensity.value() - 0.5).abs() < 0.01, "gain should halve intensity");
    }

    #[test]
    fn all_eight_channels_present_in_output() {
        let mapping = ChannelMapping::new();
        let mut router = ChannelRouter::new(mapping);
        let outputs = router.process_events(Vec::new());
        assert_eq!(outputs.len(), 8, "default mapping must produce 8 channel outputs");
    }

    #[test]
    fn channel_router_clear_active_effects() {
        let mapping = ChannelMapping::new();
        let mut router = ChannelRouter::new(mapping);

        let evt = EffectEvent::new(EffectType::Touchdown, EffectIntensity::new(0.8).unwrap());
        router.process_events(vec![evt]);
        assert!(!router.get_active_effects().is_empty());

        router.clear_active_effects();
        assert!(router.get_active_effects().is_empty());
    }

    #[test]
    fn channel_mapping_get_all_channels_sorted() {
        let mapping = ChannelMapping::new();
        let channels = mapping.get_all_channels();
        assert_eq!(channels.len(), 8);
        for (i, ch) in channels.iter().enumerate() {
            assert_eq!(ch.value(), i as u8, "channels must be sorted by ID");
        }
    }

    #[test]
    fn channel_gain_rejects_out_of_range() {
        let mut mapping = ChannelMapping::new();
        let ch = ChannelId::new(0);
        assert!(mapping.set_channel_gain(ch, 1.5).is_err());
        assert!(mapping.set_channel_gain(ch, -0.1).is_err());
        assert!(mapping.set_channel_gain(ch, 0.5).is_ok());
    }

    #[test]
    fn channel_disabled_produces_zero_output() {
        let mut mapping = ChannelMapping::new();
        let ch = ChannelId::new(0);
        mapping.set_mapping(EffectType::Touchdown, ch);
        mapping.set_channel_enabled(ch, false);

        let mut router = ChannelRouter::new(mapping);
        let evt = EffectEvent::new(EffectType::Touchdown, EffectIntensity::new(1.0).unwrap());
        let outputs = router.process_events(vec![evt]);

        let out = outputs.iter().find(|o| o.channel_id == ch).unwrap();
        assert_eq!(out.intensity.value(), 0.0);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 3. Effect patterns tests
// ═══════════════════════════════════════════════════════════════════════

mod effect_patterns {
    use super::*;

    #[test]
    fn pattern_landing_gear_bump() {
        let effects = TactilePresets::landing_gear_down();
        let mut engine = TactileEngine::new();
        for e in effects {
            engine.add_effect(e);
        }
        assert_eq!(engine.active_count(), 2);

        let first = engine.tick();
        assert!(first.abs() > 0.3, "gear bump should be forceful at t=0");
    }

    #[test]
    fn pattern_runway_texture_scales_with_speed() {
        let slow = TactilePresets::runway_roll(10.0);
        let fast = TactilePresets::runway_roll(80.0);

        let amp_slow = match slow {
            TactileEffect::Texture { amplitude, .. } => amplitude,
            _ => panic!("expected Texture"),
        };
        let amp_fast = match fast {
            TactileEffect::Texture { amplitude, .. } => amplitude,
            _ => panic!("expected Texture"),
        };
        assert!(amp_fast > amp_slow, "faster speed \u{2192} stronger texture");
    }

    #[test]
    fn pattern_stall_buffet_grows_with_aoa_excess() {
        let mild = TactilePresets::stall_buffet(2.0);
        let severe = TactilePresets::stall_buffet(8.0);

        let amp_mild = match mild {
            TactileEffect::Texture { amplitude, .. } => amplitude,
            _ => panic!("expected Texture"),
        };
        let amp_severe = match severe {
            TactileEffect::Texture { amplitude, .. } => amplitude,
            _ => panic!("expected Texture"),
        };
        assert!(amp_severe > amp_mild, "deeper stall \u{2192} stronger buffet");
    }

    #[test]
    fn pattern_engine_vibration_tracks_rpm() {
        let idle = TactilePresets::engine_vibration(600.0);
        let cruise = TactilePresets::engine_vibration(2400.0);

        match (idle, cruise) {
            (
                TactileEffect::Engine {
                    rpm: r1,
                    amplitude: a1,
                },
                TactileEffect::Engine {
                    rpm: r2,
                    amplitude: a2,
                },
            ) => {
                assert!(r2 > r1);
                assert!(a2 > a1, "higher RPM \u{2192} stronger vibration");
            }
            _ => panic!("expected Engine variants"),
        }
    }

    #[test]
    fn pattern_touchdown_impact_decays() {
        let mut engine = TactileEngine::new();
        engine.add_effect(TactileEffect::Impact {
            magnitude: 1.0,
            decay_rate: 8.0,
        });

        let first = engine.tick();
        let mut prev = first;
        for _ in 0..20 {
            let cur = engine.tick();
            assert!(cur <= prev + 1e-9, "impact must monotonically decay");
            prev = cur;
        }
    }

    #[test]
    fn pattern_overspeed_high_frequency_texture() {
        let effect = TactileEffect::Texture {
            frequency_hz: 200.0,
            amplitude: 0.9,
            pattern: TexturePattern::Square,
        };
        let band = TactileMixer::classify_band(effect.frequency_hz());
        assert_eq!(band, FrequencyBand::High, "overspeed should be high-band");

        let mut engine = TactileEngine::new();
        engine.add_effect(effect);
        let mut max_abs = 0.0_f64;
        for _ in 0..50 {
            max_abs = max_abs.max(engine.tick().abs());
        }
        assert!(max_abs > 0.5, "overspeed must be perceivable");
    }

    #[test]
    fn vibration_sine_wave_symmetry() {
        let mut engine = TactileEngine::new();
        engine.add_effect(TactileEffect::Rumble {
            frequency_hz: 50.0,
            amplitude: 1.0,
            duration_ticks: 250,
        });

        let samples = collect_samples(&mut engine, 5);
        let positive: f64 = samples.iter().filter(|&&s| s > 0.0).sum();
        let negative: f64 = samples.iter().filter(|&&s| s < 0.0).sum::<f64>().abs();

        assert!(
            (positive - negative).abs() < 1.0,
            "sine vibration should be roughly symmetric, pos={positive}, neg={negative}"
        );
    }

    #[test]
    fn rumble_with_short_duration_expires() {
        let mut engine = TactileEngine::new();
        engine.add_effect(TactileEffect::Rumble {
            frequency_hz: 50.0,
            amplitude: 0.5,
            duration_ticks: 3,
        });

        for _ in 0..3 {
            engine.tick();
        }
        assert_eq!(engine.active_count(), 0, "rumble should expire after exact duration");
    }

    #[test]
    fn pulse_via_impact_decay() {
        let mut engine = TactileEngine::new();
        engine.add_effect(TactileEffect::Impact { magnitude: 1.0, decay_rate: 20.0 });

        let s1 = engine.tick();
        let s2 = engine.tick();
        let s3 = engine.tick();

        assert!(s1 > s2, "impact must decay: s1 > s2");
        assert!(s2 > s3, "impact must decay: s2 > s3");
    }

    #[test]
    fn constant_effect_via_low_decay_impact() {
        let mut engine = TactileEngine::new();
        engine.add_effect(TactileEffect::Impact { magnitude: 0.5, decay_rate: 0.001 });

        let samples = collect_samples(&mut engine, 10);
        let variance: f64 = samples.iter().map(|s| (s - 0.5).powi(2)).sum::<f64>() / 10.0;
        assert!(variance < 0.01, "near-constant effect should have low variance");
    }

    #[test]
    fn sine_wave_engine_produces_periodic_output() {
        let mut engine = TactileEngine::new();
        engine.add_effect(TactileEffect::Engine { rpm: 3000.0, amplitude: 0.8 });

        let samples = collect_samples(&mut engine, 10);
        assert!(
            (samples[4] - samples[9]).abs() < 0.2,
            "engine should repeat with period \u{2248} 5 ticks"
        );
    }

    #[test]
    fn ramp_effect_via_texture_sawtooth() {
        let mut engine = TactileEngine::new();
        engine.add_effect(TactileEffect::Texture {
            frequency_hz: TICK_RATE_HZ / 10.0,
            amplitude: 1.0,
            pattern: TexturePattern::Sawtooth,
        });

        let samples = collect_samples(&mut engine, 10);
        let rising = samples.windows(2).filter(|w| w[1] >= w[0]).count();
        assert!(rising >= 5, "sawtooth should be mostly rising within a cycle");
    }

    #[test]
    fn custom_pattern_square_duty_cycle() {
        let mut engine = TactileEngine::new();
        engine.add_effect(TactileEffect::Texture {
            frequency_hz: 25.0,
            amplitude: 1.0,
            pattern: TexturePattern::Square,
        });

        let samples = collect_samples(&mut engine, 10);
        let positive_count = samples.iter().filter(|&&s| s > 0.0).count();
        let negative_count = samples.iter().filter(|&&s| s < 0.0).count();
        assert!(positive_count >= 3 && negative_count >= 3, "square should be \u{2248}50% duty cycle");
    }

    #[test]
    fn composability_multiple_effects_mix() {
        let mut mixer = TactileMixer::new();
        mixer.add_effect(TactileEffect::Impact { magnitude: 0.3, decay_rate: 5.0 });
        mixer.add_effect(TactileEffect::Rumble {
            frequency_hz: 80.0, amplitude: 0.2, duration_ticks: 100,
        });
        mixer.add_effect(TactileEffect::Texture {
            frequency_hz: 200.0, amplitude: 0.1, pattern: TexturePattern::Triangle,
        });

        let out = mixer.tick();
        assert!(out.combined.abs() > 0.0, "composed effects should produce output");
        assert!(out.low.abs() > 0.0, "impact contributes to low band");
        assert_eq!(mixer.active_count(), 3);
    }

    #[test]
    fn preset_turbulence_clamped_at_extremes() {
        match TactilePresets::turbulence(-1.0) {
            TactileEffect::Texture { amplitude, .. } => {
                assert_eq!(amplitude, 0.0, "negative intensity \u{2192} zero amplitude");
            }
            _ => panic!("expected Texture"),
        }
        match TactilePresets::turbulence(5.0) {
            TactileEffect::Texture { amplitude, .. } => {
                assert!(amplitude <= 0.7 + 1e-9, "clamped intensity \u{2192} max amplitude");
            }
            _ => panic!("expected Texture"),
        }
    }

    #[test]
    fn preset_weapon_fire_decays_quickly() {
        let mut engine = TactileEngine::new();
        engine.add_effect(TactilePresets::weapon_fire());

        let first = engine.tick();
        assert!((first - 1.0).abs() < 0.01, "weapon fire starts at magnitude 1.0");

        for _ in 0..9 {
            engine.tick();
        }
        let tenth = engine.tick();
        assert!(tenth < 0.6, "weapon fire must decay fast (decay_rate=15)");
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 4. Timing tests
// ═══════════════════════════════════════════════════════════════════════

mod timing {
    use super::*;

    #[test]
    fn effect_duration_precise_tick_count() {
        let mut engine = TactileEngine::new();
        engine.add_effect(TactileEffect::Rumble {
            frequency_hz: 50.0,
            amplitude: 1.0,
            duration_ticks: 10,
        });

        for _ in 0..9 {
            engine.tick();
        }
        assert_eq!(engine.active_count(), 1, "should still be active at tick 9");

        engine.tick();
        assert_eq!(engine.active_count(), 0, "should expire at exactly tick 10");
    }

    #[test]
    fn cadence_synchronisation_tick_counter() {
        let mut engine = TactileEngine::new();
        for _ in 0..100 {
            engine.tick();
        }
        assert_eq!(engine.tick_count(), 100, "tick counter must be precise");

        for _ in 0..150 {
            engine.tick();
        }
        assert_eq!(engine.tick_count(), 250);
    }

    #[test]
    fn fade_in_via_progressive_gain() {
        let mut mixer = TactileMixer::new();
        let slot = mixer
            .add_effect(TactileEffect::Impact { magnitude: 1.0, decay_rate: 0.001 })
            .unwrap();

        let mut outputs = Vec::new();
        for step in 0..5 {
            let gain = (step + 1) as f64 * 0.2;
            mixer.set_slot_gain(slot, gain);
            outputs.push(mixer.tick().combined.abs());
        }

        for w in outputs.windows(2) {
            assert!(w[1] >= w[0] * 0.8, "fade-in should produce non-decreasing output");
        }
    }

    #[test]
    fn fade_out_via_progressive_gain() {
        let mut mixer = TactileMixer::new();
        let slot = mixer
            .add_effect(TactileEffect::Impact { magnitude: 1.0, decay_rate: 0.001 })
            .unwrap();

        mixer.set_slot_gain(slot, 1.0);
        let _ = mixer.tick();

        let mut outputs = Vec::new();
        for step in (0..5).rev() {
            let gain = step as f64 * 0.2;
            mixer.set_slot_gain(slot, gain);
            outputs.push(mixer.tick().combined.abs());
        }

        assert!(outputs.last().unwrap().abs() < 0.05, "fade-out should end near zero");
    }

    #[test]
    fn delay_before_start_via_slot_reuse() {
        let mut engine = TactileEngine::new();
        engine.add_effect(TactileEffect::Rumble {
            frequency_hz: 0.0,
            amplitude: 0.0,
            duration_ticks: 5,
        });
        assert_eq!(engine.active_count(), 1);

        for _ in 0..5 {
            let v = engine.tick();
            assert!(v.abs() < 0.01, "during delay, output should be ~zero");
        }

        let slot = engine.add_effect(TactileEffect::Impact { magnitude: 1.0, decay_rate: 5.0 });
        assert!(slot.is_some(), "slot should be reusable after delay");
        assert!(engine.tick().abs() > 0.5, "real effect should fire after delay");
    }

    #[test]
    fn effect_event_wall_clock_expiration() {
        let event = EffectEvent::with_duration(
            EffectType::Touchdown,
            EffectIntensity::new(1.0).unwrap(),
            Duration::from_millis(50),
        );
        assert!(!event.is_expired());

        let remaining = event.remaining_duration().unwrap();
        assert!(remaining <= Duration::from_millis(50));

        let deadline = std::time::Instant::now() + Duration::from_millis(500);
        while !event.is_expired() {
            if std::time::Instant::now() >= deadline {
                panic!("event did not expire within timeout");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(event.remaining_duration().is_none(), "no remaining duration after expiry");
    }

    #[test]
    fn effect_event_remaining_duration_decreases() {
        let event = EffectEvent::with_duration(
            EffectType::Touchdown,
            EffectIntensity::new(1.0).unwrap(),
            Duration::from_secs(1),
        );
        let remaining = event.remaining_duration().unwrap();
        assert!(remaining <= Duration::from_secs(1));
        assert!(remaining > Duration::from_millis(500));
    }

    #[test]
    fn effect_event_no_duration_never_expires() {
        let event = EffectEvent::new(EffectType::EngineVibration, EffectIntensity::new(0.5).unwrap());
        assert!(!event.is_expired(), "no-duration event must never expire");
        assert!(event.remaining_duration().is_none());
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 5. Safety tests
// ═══════════════════════════════════════════════════════════════════════

mod safety {
    use super::*;

    #[test]
    fn safety_maximum_intensity_clamped() {
        let mut engine = TactileEngine::new();
        for _ in 0..MAX_EFFECTS {
            engine.add_effect(TactileEffect::Impact {
                magnitude: 1.0,
                decay_rate: 0.01,
            });
        }
        let sample = engine.tick();
        assert!(
            (-1.0..=1.0).contains(&sample),
            "output must never exceed \u{b1}1.0"
        );
    }

    #[test]
    fn safety_thermal_protection_sustained_load() {
        let mut mixer = TactileMixer::new();
        mixer.set_master_gain(2.0);

        for _ in 0..MAX_EFFECTS {
            mixer.add_effect(TactileEffect::Texture {
                frequency_hz: 50.0,
                amplitude: 1.0,
                pattern: TexturePattern::Square,
            });
        }

        for _ in 0..1_000 {
            let out = mixer.tick();
            assert!(
                (-1.0..=1.0).contains(&out.combined),
                "mixer must clamp even with max gain"
            );
        }
    }

    #[test]
    fn safety_duty_cycle_expiry() {
        let mut engine = TactileEngine::new();
        engine.add_effect(TactileEffect::Rumble {
            frequency_hz: 80.0,
            amplitude: 0.9,
            duration_ticks: 25,
        });

        for _ in 0..25 {
            engine.tick();
        }
        assert_eq!(engine.active_count(), 0, "rumble must auto-stop");

        let after = engine.tick();
        assert_eq!(after, 0.0, "no lingering output after expiry");
    }

    #[test]
    fn safety_emergency_stop() {
        let mut engine = TactileEngine::new();
        for _ in 0..8 {
            engine.add_effect(TactileEffect::Texture {
                frequency_hz: 60.0,
                amplitude: 0.7,
                pattern: TexturePattern::Triangle,
            });
        }
        assert!(engine.active_count() > 0);

        engine.clear();
        assert_eq!(engine.active_count(), 0);

        let sample = engine.tick();
        assert_eq!(sample, 0.0, "cleared engine must output zero");
    }

    #[test]
    fn thermal_protection_master_gain_ceiling() {
        let mut mixer = TactileMixer::new();
        mixer.set_master_gain(100.0);
        assert_eq!(mixer.master_gain(), 2.0, "master gain ceiling is 2.0");

        mixer.set_master_gain(-5.0);
        assert_eq!(mixer.master_gain(), 0.0, "master gain floor is 0.0");
    }

    #[test]
    fn frequency_bounds_validation() {
        assert_eq!(TactileEffect::Impact { magnitude: 1.0, decay_rate: 5.0 }.frequency_hz(), 20.0);
        assert_eq!(TactileEffect::Engine { rpm: 0.0, amplitude: 0.5 }.frequency_hz(), 0.0);

        let mut engine = TactileEngine::new();
        engine.add_effect(TactileEffect::Engine { rpm: 60000.0, amplitude: 0.5 });
        let v = engine.tick();
        assert!((-1.0..=1.0).contains(&v), "extreme frequency must still clamp");
    }

    #[test]
    fn concurrent_effect_limit() {
        let mut engine = TactileEngine::new();
        for i in 0..MAX_EFFECTS {
            assert!(
                engine.add_effect(TactileEffect::Rumble {
                    frequency_hz: 50.0, amplitude: 0.1, duration_ticks: 1000,
                }).is_some(),
                "slot {i} should succeed"
            );
        }

        assert!(
            engine.add_effect(TactileEffect::Impact { magnitude: 1.0, decay_rate: 5.0 }).is_none(),
            "exceeding MAX_EFFECTS must return None"
        );
        assert_eq!(engine.active_count(), MAX_EFFECTS);
    }

    #[test]
    fn effect_intensity_bounds_rejected() {
        assert!(EffectIntensity::new(-0.1).is_err());
        assert!(EffectIntensity::new(1.1).is_err());
        assert!(EffectIntensity::new(0.0).is_ok());
        assert!(EffectIntensity::new(1.0).is_ok());
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 6. Integration and Configuration tests
// ═══════════════════════════════════════════════════════════════════════

mod integration {
    use super::*;

    #[test]
    fn integration_telemetry_to_touchdown_effect() {
        let mut processor = EffectProcessor::new();

        let mut snap = make_snapshot();
        snap.environment.altitude = 200.0;
        snap.kinematics.vertical_speed = -400.0;
        processor.process(&snap);

        snap.environment.altitude = 10.0;
        snap.kinematics.vertical_speed = -300.0;
        let events = processor.process(&snap);

        let touchdown = events
            .iter()
            .find(|e| e.effect_type == EffectType::Touchdown);
        assert!(touchdown.is_some(), "touchdown must fire on air\u{2192}ground");
        assert!(touchdown.unwrap().intensity.value() > 0.0);
    }

    #[test]
    fn integration_bus_subscription_stall_buffet() {
        let mut processor = EffectProcessor::new();
        let mut snap = make_snapshot();
        snap.kinematics.aoa = ValidatedAngle::new_degrees(22.0).unwrap();
        snap.kinematics.ias = ValidatedSpeed::new_knots(55.0).unwrap();

        let events = processor.process(&snap);
        let stall = events
            .iter()
            .find(|e| e.effect_type == EffectType::StallBuffet);
        assert!(stall.is_some(), "stall buffet should trigger at high AoA");
    }

    #[test]
    fn integration_profile_driven_effect_filtering() {
        let mut config = TactileConfig::default();
        config.effect_enabled.insert(EffectType::GroundRoll, false);

        let mut processor = EffectProcessor::new();
        let mut snap = make_snapshot();
        snap.environment.altitude = 5.0;
        snap.kinematics.ground_speed = ValidatedSpeed::new_knots(40.0).unwrap();

        let mut events = processor.process(&snap);
        events.retain(|e| {
            config
                .effect_enabled
                .get(&e.effect_type)
                .copied()
                .unwrap_or(true)
        });

        assert!(
            !events.iter().any(|e| e.effect_type == EffectType::GroundRoll),
            "disabled effect must be filtered out"
        );
    }

    #[test]
    fn integration_multi_device_output_routing() {
        let mut mapping = ChannelMapping::new();
        mapping.set_mapping(EffectType::Touchdown, ChannelId::new(0));
        mapping.set_mapping(EffectType::StallBuffet, ChannelId::new(2));
        mapping.set_mapping(EffectType::EngineVibration, ChannelId::new(4));

        let mut router = ChannelRouter::new(mapping);
        let events = vec![
            EffectEvent::new(EffectType::Touchdown, EffectIntensity::new(0.8).unwrap()),
            EffectEvent::new(EffectType::StallBuffet, EffectIntensity::new(0.5).unwrap()),
            EffectEvent::new(
                EffectType::EngineVibration,
                EffectIntensity::new(0.3).unwrap(),
            ),
        ];

        let outputs = router.process_events(events);

        let ch0 = outputs.iter().find(|o| o.channel_id == ChannelId::new(0)).unwrap();
        let ch2 = outputs.iter().find(|o| o.channel_id == ChannelId::new(2)).unwrap();
        let ch4 = outputs.iter().find(|o| o.channel_id == ChannelId::new(4)).unwrap();

        assert!(ch0.intensity.value() > 0.5);
        assert!(ch2.intensity.value() > 0.3);
        assert!(ch4.intensity.value() > 0.1);
    }

    #[test]
    fn integration_engine_vibration_from_telemetry() {
        let mut processor = EffectProcessor::new();
        let mut snap = make_snapshot();
        snap.engines = vec![make_engine(true, 75.0)];

        let events = processor.process(&snap);
        let engine_vib = events
            .iter()
            .find(|e| e.effect_type == EffectType::EngineVibration);
        assert!(
            engine_vib.is_some(),
            "running engine should produce vibration"
        );
        assert!(engine_vib.unwrap().intensity.value() > 0.0);
    }

    #[test]
    fn ffb_event_to_tactile_effect_touchdown() {
        let mut processor = EffectProcessor::new();
        let mut snapshot = make_snapshot();

        snapshot.environment.altitude = 200.0;
        snapshot.kinematics.vertical_speed = -300.0;
        processor.process(&snapshot);

        snapshot.environment.altitude = 5.0;
        let events = processor.process(&snapshot);

        let td_events: Vec<_> = events.iter().filter(|e| e.effect_type == EffectType::Touchdown).collect();
        assert!(!td_events.is_empty());
        assert!(td_events[0].intensity.value() > 0.0);
    }

    #[test]
    fn sim_event_stall_buffet_trigger() {
        let mut processor = EffectProcessor::new();
        let mut snapshot = make_snapshot();

        snapshot.kinematics.aoa = ValidatedAngle::new_degrees(22.0).unwrap();
        snapshot.kinematics.ias = ValidatedSpeed::new_knots(60.0).unwrap();

        let events = processor.process(&snapshot);
        let stall: Vec<_> = events.iter().filter(|e| e.effect_type == EffectType::StallBuffet).collect();
        assert_eq!(stall.len(), 1);
        assert!((stall[0].intensity.value() - 0.4).abs() < 0.1);
    }

    #[test]
    fn calibration_workflow_test_effect() {
        let mapping = ChannelMapping::new();
        let mut router = ChannelRouter::new(mapping);

        for effect_type in [
            EffectType::Touchdown,
            EffectType::GroundRoll,
            EffectType::StallBuffet,
            EffectType::EngineVibration,
            EffectType::GearWarning,
            EffectType::RotorVibration,
        ] {
            let result = router.test_effect(effect_type, 0.5);
            assert!(result.is_ok());
            let outputs = result.unwrap();
            assert!(!outputs.is_empty());
        }

        assert!(router.get_active_effects().is_empty());
    }

    #[test]
    fn profile_based_configuration() {
        let config = TactileConfig::default();

        assert!(config.effect_enabled.get(&EffectType::Touchdown).copied().unwrap());
        assert!(config.effect_enabled.get(&EffectType::GroundRoll).copied().unwrap());

        let mut manager = TactileManager::new();
        assert!(manager.initialize(config.clone()).is_ok());
        
        let mut new_config = config;
        new_config.update_rate_hz = 30.0;
        assert!(manager.update_config(new_config).is_ok());
        assert_eq!(manager.get_config().update_rate_hz, 30.0);
    }

    #[test]
    fn mixer_band_separation_low_mid_high() {
        let mut mixer = TactileMixer::new();
        mixer.add_effect(TactileEffect::Impact { magnitude: 0.5, decay_rate: 5.0 });
        mixer.add_effect(TactileEffect::Rumble { frequency_hz: 80.0, amplitude: 0.5, duration_ticks: 100 });
        mixer.add_effect(TactileEffect::Texture { frequency_hz: 200.0, amplitude: 0.5, pattern: TexturePattern::Square });

        let out = mixer.tick();
        assert!(out.low.abs() > 0.1);
    }

    #[test]
    fn effect_slot_reuse_after_expiry() {
        let mut engine = TactileEngine::new();
        for _ in 0..MAX_EFFECTS {
            engine.add_effect(TactileEffect::Rumble { frequency_hz: 50.0, amplitude: 0.1, duration_ticks: 5 });
        }
        assert_eq!(engine.active_count(), MAX_EFFECTS);

        for _ in 0..5 { engine.tick(); }
        assert_eq!(engine.active_count(), 0);

        let slot = engine.add_effect(TactileEffect::Impact { magnitude: 0.5, decay_rate: 5.0 });
        assert!(slot.is_some());
    }

    #[test]
    fn mixer_zero_master_gain_silences_output() {
        let mut mixer = TactileMixer::new();
        mixer.set_master_gain(0.0);
        mixer.add_effect(TactileEffect::Impact { magnitude: 1.0, decay_rate: 5.0 });
        let out = mixer.tick();
        assert_eq!(out.combined, 0.0);
    }

    #[test]
    fn mixer_band_gain_zeroes_specific_band() {
        let mut mixer = TactileMixer::new();
        mixer.set_band_gain(FrequencyBand::Low, 0.0);
        mixer.add_effect(TactileEffect::Impact { magnitude: 1.0, decay_rate: 5.0 });
        let out = mixer.tick();
        assert_eq!(out.low, 0.0);
    }

    #[test]
    fn manager_stats_some_after_init() {
        let mut mgr = TactileManager::new();
        mgr.initialize(TactileConfig::default()).unwrap();
        assert!(mgr.get_stats().is_some());
    }

    #[test]
    fn touchdown_intensity_scales_with_descent_rate() {
        let mut processor = EffectProcessor::new();
        let mut snap = make_snapshot();

        snap.environment.altitude = 200.0;
        snap.kinematics.vertical_speed = -300.0;
        processor.process(&snap);

        snap.environment.altitude = 10.0;
        snap.kinematics.vertical_speed = -250.0;
        let gentle_int = processor.process(&snap).iter().find(|e| e.effect_type == EffectType::Touchdown).unwrap().intensity.value();

        snap.environment.altitude = 200.0;
        snap.kinematics.vertical_speed = -300.0;
        processor.process(&snap);

        snap.environment.altitude = 10.0;
        snap.kinematics.vertical_speed = -500.0;
        let hard_int = processor.process(&snap).iter().find(|e| e.effect_type == EffectType::Touchdown).unwrap().intensity.value();

        assert!(hard_int > gentle_int);
    }
}
