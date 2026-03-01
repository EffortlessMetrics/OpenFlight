// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the tactile/haptic feedback system.
//!
//! Covers motor drivers, zone routing, effect patterns, timing,
//! safety limits, and integration with the FFB/sim event pipeline.

use std::time::Duration;

use flight_bus::{AircraftId, BusSnapshot, SimId};
use flight_tactile::engine::{MAX_EFFECTS, TICK_RATE_HZ, TactileEffect, TactileEngine, TexturePattern};
use flight_tactile::effects::{EffectEvent, EffectIntensity, EffectProcessor, EffectType};
use flight_tactile::mixer::TactileMixer;
use flight_tactile::channel::{ChannelId, ChannelMapping, ChannelRouter};
use flight_tactile::presets::TactilePresets;
use flight_tactile::{TactileConfig, TactileManager};

// ── Helpers ──────────────────────────────────────────────────────────

fn test_snapshot() -> BusSnapshot {
    BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"))
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
// 1. Motor driver tests (8)
// ═══════════════════════════════════════════════════════════════════════

mod motor_driver {
    use super::*;

    #[test]
    fn set_intensity_via_slot_gain() {
        let mut mixer = TactileMixer::new();
        let slot = mixer
            .add_effect(TactileEffect::Impact { magnitude: 1.0, decay_rate: 5.0 })
            .unwrap();

        // 75 % intensity
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

        // At 250 Hz tick rate and 100 Hz signal, expect 2.5 samples per cycle.
        // After one full cycle (~2–3 ticks) the sine should cross zero again.
        let samples = collect_samples(&mut engine, 5);
        let zero_crossings = samples.windows(2).filter(|w| w[0].signum() != w[1].signum()).count();
        assert!(zero_crossings >= 1, "100 Hz rumble should produce zero crossings in 5 ticks");
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
        // Low band motor (< 40 Hz)
        mixer.add_effect(TactileEffect::Impact { magnitude: 0.5, decay_rate: 5.0 });
        // Mid band motor (80 Hz)
        mixer.add_effect(TactileEffect::Rumble {
            frequency_hz: 80.0, amplitude: 0.5, duration_ticks: 100,
        });
        // High band motor (200 Hz)
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
            let gain = step as f64 * 0.2; // 0.2, 0.4, 0.6, 0.8, 1.0
            mixer.set_slot_gain(slot, gain);
            let out = mixer.tick();
            // Each step should be >= previous (decay is slow at 0.1)
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

        let expected_freq = 1800.0 / 60.0; // 30 Hz
        let effect = TactileEffect::Engine { rpm: 1800.0, amplitude: 0.5 };
        assert!((effect.frequency_hz() - expected_freq).abs() < 0.01);

        let samples = collect_samples(&mut engine, 50);
        let max = samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        assert!(max > 0.1, "engine effect should produce output");
        assert!(max <= 0.5 + 1e-9, "engine output must not exceed amplitude");
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 2. Zone routing tests (8)
// ═══════════════════════════════════════════════════════════════════════

mod zone_routing {
    use super::*;

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
    fn priority_based_routing_takes_max() {
        let mapping = ChannelMapping::new();
        let mut router = ChannelRouter::new(mapping);

        // Two effects on different channels: each keeps its own intensity.
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
        // Route both Touchdown and GearWarning to channel 0
        mapping.set_mapping(EffectType::Touchdown, ChannelId::new(0));
        mapping.set_mapping(EffectType::GearWarning, ChannelId::new(0));

        let mut router = ChannelRouter::new(mapping);

        let e1 = EffectEvent::new(EffectType::Touchdown, EffectIntensity::new(0.6).unwrap());
        let e2 = EffectEvent::new(EffectType::GearWarning, EffectIntensity::new(0.8).unwrap());

        let outputs = router.process_events(vec![e1, e2]);
        let ch0 = outputs.iter().find(|o| o.channel_id == ChannelId::new(0)).unwrap();
        // Router uses max(): should be 0.8
        assert!((ch0.intensity.value() - 0.8).abs() < 0.01);
    }

    #[test]
    fn dead_zone_unmapped_effect_produces_zero() {
        let mut mapping = ChannelMapping::new();
        // Remove Touchdown mapping by overwriting with a fresh map that doesn't include it
        mapping.mappings.clear();
        mapping.set_mapping(EffectType::GroundRoll, ChannelId::new(1));

        let mut router = ChannelRouter::new(mapping);
        let e = EffectEvent::new(EffectType::Touchdown, EffectIntensity::new(1.0).unwrap());
        let outputs = router.process_events(vec![e]);

        // Channel 0 should not have received the touchdown effect
        let ch0 = outputs.iter().find(|o| o.channel_id == ChannelId::new(0));
        match ch0 {
            Some(o) => assert_eq!(o.intensity.value(), 0.0, "unmapped effect should yield zero"),
            None => {} // absent is also fine
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

        // First: disabled → zero
        let e = EffectEvent::new(EffectType::Touchdown, EffectIntensity::new(1.0).unwrap());
        let outputs = router.process_events(vec![e]);
        let ch0 = outputs.iter().find(|o| o.channel_id == ChannelId::new(0)).unwrap();
        assert_eq!(ch0.intensity.value(), 0.0);

        // Re-enable via mapping update
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
}

// ═══════════════════════════════════════════════════════════════════════
// 3. Effect patterns tests (8)
// ═══════════════════════════════════════════════════════════════════════

mod effect_patterns {
    use super::*;

    #[test]
    fn vibration_sine_wave_symmetry() {
        let mut engine = TactileEngine::new();
        engine.add_effect(TactileEffect::Rumble {
            frequency_hz: 50.0,
            amplitude: 1.0,
            duration_ticks: 250,
        });

        // Collect one full period at 250 Hz / 50 Hz = 5 ticks
        let samples = collect_samples(&mut engine, 5);
        let positive: f64 = samples.iter().filter(|&&s| s > 0.0).sum();
        let negative: f64 = samples.iter().filter(|&&s| s < 0.0).sum::<f64>().abs();

        // Sine is symmetric; positive and negative halves should be roughly equal
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
        // Very slow decay ~ effectively constant for short duration
        engine.add_effect(TactileEffect::Impact { magnitude: 0.5, decay_rate: 0.001 });

        let samples = collect_samples(&mut engine, 10);
        let variance: f64 = samples.iter().map(|s| (s - 0.5).powi(2)).sum::<f64>() / 10.0;
        assert!(variance < 0.01, "near-constant effect should have low variance");
    }

    #[test]
    fn sine_wave_engine_produces_periodic_output() {
        let mut engine = TactileEngine::new();
        engine.add_effect(TactileEffect::Engine { rpm: 3000.0, amplitude: 0.8 });

        // freq = 3000/60 = 50 Hz, period in ticks = 250/50 = 5
        let samples = collect_samples(&mut engine, 10);
        // Samples at tick 0 and tick 5 should be similar (both near zero for sine)
        assert!(
            (samples[4] - samples[9]).abs() < 0.2,
            "engine should repeat with period ≈ 5 ticks"
        );
    }

    #[test]
    fn ramp_effect_via_texture_sawtooth() {
        let mut engine = TactileEngine::new();
        engine.add_effect(TactileEffect::Texture {
            frequency_hz: TICK_RATE_HZ / 10.0, // 25 Hz → 10 ticks per cycle
            amplitude: 1.0,
            pattern: TexturePattern::Sawtooth,
        });

        // Over one cycle the sawtooth rises linearly
        let samples = collect_samples(&mut engine, 10);
        let rising = samples.windows(2).filter(|w| w[1] >= w[0]).count();
        // Most pairs should be rising in the first cycle of a sawtooth
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

        let samples = collect_samples(&mut engine, 10); // one full cycle at 25 Hz / 250 Hz = 10 ticks
        let positive_count = samples.iter().filter(|&&s| s > 0.0).count();
        let negative_count = samples.iter().filter(|&&s| s < 0.0).count();
        // 50% duty cycle: roughly half positive, half negative
        assert!(positive_count >= 3 && negative_count >= 3, "square should be ~50% duty cycle");
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
        // Combined should be the sum of all contributions
        assert!(out.combined.abs() > 0.0, "composed effects should produce output");
        assert!(out.low.abs() > 0.0, "impact contributes to low band");
        assert_eq!(mixer.active_count(), 3);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 4. Timing tests (6)
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

        engine.tick(); // tick 10
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

        // Simulate fade-in over 5 steps
        let mut outputs = Vec::new();
        for step in 0..5 {
            let gain = (step + 1) as f64 * 0.2;
            mixer.set_slot_gain(slot, gain);
            outputs.push(mixer.tick().combined.abs());
        }

        // Each step should produce roughly increasing output
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

        // Simulate fade-out
        let mut outputs = Vec::new();
        for step in (0..5).rev() {
            let gain = step as f64 * 0.2;
            mixer.set_slot_gain(slot, gain);
            outputs.push(mixer.tick().combined.abs());
        }

        // Last output should be near zero
        assert!(outputs.last().unwrap().abs() < 0.05, "fade-out should end near zero");
    }

    #[test]
    fn delay_before_start_via_slot_reuse() {
        let mut engine = TactileEngine::new();
        // First: fill a slot with a very short effect to create a "delay"
        engine.add_effect(TactileEffect::Rumble {
            frequency_hz: 0.0,
            amplitude: 0.0,
            duration_ticks: 5, // 5-tick delay
        });
        assert_eq!(engine.active_count(), 1);

        for _ in 0..5 {
            let v = engine.tick();
            assert!(v.abs() < 0.01, "during delay, output should be ~zero");
        }

        // Now the slot is free; add the real effect
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

        // Remaining duration should be <= 50ms
        let remaining = event.remaining_duration().unwrap();
        assert!(remaining <= Duration::from_millis(50));

        std::thread::sleep(Duration::from_millis(60));
        assert!(event.is_expired(), "event should expire after wall-clock duration");
        assert!(event.remaining_duration().is_none(), "no remaining duration after expiry");
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 5. Safety tests (5)
// ═══════════════════════════════════════════════════════════════════════

mod safety {
    use super::*;

    #[test]
    fn max_intensity_output_clamped() {
        let mut engine = TactileEngine::new();
        // Fill all slots with high-magnitude impacts
        for _ in 0..MAX_EFFECTS {
            engine.add_effect(TactileEffect::Impact { magnitude: 1.0, decay_rate: 0.01 });
        }

        for _ in 0..100 {
            let v = engine.tick();
            assert!(
                (-1.0..=1.0).contains(&v),
                "output must always be clamped to [-1,1], got {v}"
            );
        }
    }

    #[test]
    fn thermal_protection_master_gain_ceiling() {
        let mut mixer = TactileMixer::new();
        // Try to set excessive gain — should be clamped to 2.0
        mixer.set_master_gain(100.0);
        assert_eq!(mixer.master_gain(), 2.0, "master gain ceiling is 2.0");

        mixer.set_master_gain(-5.0);
        assert_eq!(mixer.master_gain(), 0.0, "master gain floor is 0.0");

        // Even with max gain (2.0) and max effects, output stays in [-1, 1]
        mixer.set_master_gain(2.0);
        for _ in 0..MAX_EFFECTS {
            mixer.add_effect(TactileEffect::Impact { magnitude: 1.0, decay_rate: 0.01 });
        }
        let out = mixer.tick();
        assert!((-1.0..=1.0).contains(&out.combined), "mixer output must be clamped");
    }

    #[test]
    fn frequency_bounds_validation() {
        // Impact has a fixed 20 Hz
        assert_eq!(TactileEffect::Impact { magnitude: 1.0, decay_rate: 5.0 }.frequency_hz(), 20.0);

        // Engine at 0 RPM → 0 Hz
        assert_eq!(TactileEffect::Engine { rpm: 0.0, amplitude: 0.5 }.frequency_hz(), 0.0);

        // Very high RPM → high frequency; system should still operate
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

        // One more should fail
        assert!(
            engine.add_effect(TactileEffect::Impact { magnitude: 1.0, decay_rate: 5.0 }).is_none(),
            "exceeding MAX_EFFECTS must return None"
        );
        assert_eq!(engine.active_count(), MAX_EFFECTS);
    }

    #[test]
    fn effect_intensity_bounds_rejected() {
        assert!(EffectIntensity::new(-0.1).is_err(), "negative intensity rejected");
        assert!(EffectIntensity::new(1.1).is_err(), "over-1.0 intensity rejected");
        assert!(EffectIntensity::new(0.0).is_ok(), "zero is valid");
        assert!(EffectIntensity::new(1.0).is_ok(), "1.0 is valid");
        assert_eq!(EffectIntensity::zero().value(), 0.0);
        assert_eq!(EffectIntensity::max().value(), 1.0);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 6. Integration tests (5)
// ═══════════════════════════════════════════════════════════════════════

mod integration {
    use super::*;

    #[test]
    fn ffb_event_to_tactile_effect_touchdown() {
        let mut processor = EffectProcessor::new();
        let mut snapshot = test_snapshot();

        // Setup: in the air
        snapshot.environment.altitude = 200.0;
        snapshot.kinematics.vertical_speed = -300.0;
        processor.process(&snapshot); // update internal state

        // Touchdown: on ground, high descent rate
        snapshot.environment.altitude = 5.0;
        let events = processor.process(&snapshot);

        let td_events: Vec<_> = events.iter().filter(|e| e.effect_type == EffectType::Touchdown).collect();
        assert!(!td_events.is_empty(), "touchdown should trigger tactile effect");
        assert!(td_events[0].intensity.value() > 0.0);
        assert!(td_events[0].duration.is_some(), "touchdown effect should have duration");
    }

    #[test]
    fn sim_event_stall_buffet_trigger() {
        let mut processor = EffectProcessor::new();
        let mut snapshot = test_snapshot();

        // Stall conditions: high AoA + sufficient airspeed
        snapshot.kinematics.aoa = flight_bus::ValidatedAngle::new_degrees(22.0).unwrap();
        snapshot.kinematics.ias = flight_bus::ValidatedSpeed::new_knots(60.0).unwrap();

        let events = processor.process(&snapshot);
        let stall: Vec<_> = events.iter().filter(|e| e.effect_type == EffectType::StallBuffet).collect();
        assert_eq!(stall.len(), 1, "stall buffet should trigger");
        // Intensity should scale with AoA excess (22 - 18 = 4 deg → 4/10 = 0.4)
        assert!((stall[0].intensity.value() - 0.4).abs() < 0.1);
    }

    #[test]
    fn calibration_workflow_test_effect() {
        let mapping = ChannelMapping::new();
        let mut router = ChannelRouter::new(mapping);

        // Test each effect type at known intensity
        for effect_type in [
            EffectType::Touchdown,
            EffectType::GroundRoll,
            EffectType::StallBuffet,
            EffectType::EngineVibration,
            EffectType::GearWarning,
            EffectType::RotorVibration,
        ] {
            let result = router.test_effect(effect_type, 0.5);
            assert!(result.is_ok(), "test_effect should succeed for {:?}", effect_type);
            let outputs = result.unwrap();
            assert!(!outputs.is_empty(), "test_effect should produce outputs");
        }

        // After testing, no lingering active effects
        assert!(router.get_active_effects().is_empty(), "test should not leave active effects");
    }

    #[test]
    fn profile_based_configuration() {
        let config = TactileConfig::default();

        // Verify all effect types are enabled by default
        assert!(config.effect_enabled.get(&EffectType::Touchdown).copied().unwrap());
        assert!(config.effect_enabled.get(&EffectType::GroundRoll).copied().unwrap());
        assert!(config.effect_enabled.get(&EffectType::StallBuffet).copied().unwrap());
        assert!(config.effect_enabled.get(&EffectType::EngineVibration).copied().unwrap());
        assert!(config.effect_enabled.get(&EffectType::GearWarning).copied().unwrap());
        assert!(config.effect_enabled.get(&EffectType::RotorVibration).copied().unwrap());

        // Manager can be initialized and configured
        let mut manager = TactileManager::new();
        assert!(manager.initialize(config.clone()).is_ok());
        assert!(manager.get_config().update_rate_hz == 60.0);

        // Update config
        let mut new_config = config;
        new_config.update_rate_hz = 30.0;
        assert!(manager.update_config(new_config).is_ok());
        assert_eq!(manager.get_config().update_rate_hz, 30.0);
    }

    #[test]
    fn preset_to_engine_roundtrip() {
        let mut mixer = TactileMixer::new();

        // Queue all preset types through the mixer
        for effect in TactilePresets::landing_gear_down() {
            mixer.add_effect(effect);
        }
        mixer.add_effect(TactilePresets::runway_roll(50.0));
        mixer.add_effect(TactilePresets::engine_vibration(2000.0));
        mixer.add_effect(TactilePresets::stall_buffet(5.0));
        mixer.add_effect(TactilePresets::weapon_fire());
        mixer.add_effect(TactilePresets::turbulence(0.6));

        assert_eq!(mixer.active_count(), 7);

        // All presets should produce audible output through the mixer
        let samples = collect_mixer_samples(&mut mixer, 50);
        let any_nonzero = samples.iter().any(|s| s.abs() > 0.001);
        assert!(any_nonzero, "presets through mixer should produce output");

        // All outputs must be clamped
        for (i, &s) in samples.iter().enumerate() {
            assert!((-1.0..=1.0).contains(&s), "sample {i} out of range: {s}");
        }
    }
}
