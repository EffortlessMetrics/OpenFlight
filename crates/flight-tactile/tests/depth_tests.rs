// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the tactile / haptic feedback engine
//!
//! Covers: motor control, effect zones, effect patterns, safety,
//! configuration, and integration with the telemetry pipeline.

use flight_bus::{
    AircraftId, BusSnapshot, EngineData, HeloData, Percentage, SimId, ValidatedAngle,
    ValidatedSpeed,
};
use flight_tactile::{
    ChannelId, ChannelMapping, ChannelRouter, EffectEvent, EffectIntensity, EffectProcessor,
    EffectType, FrequencyBand, TactileBridge, TactileConfig, TactileEffect, TactileEngine,
    TactileManager, TactileMixer, TactilePresets, TexturePattern, MAX_EFFECTS,
};
use parking_lot::RwLock;
use std::sync::Arc;

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

// ═══════════════════════════════════════════════════════════════════════
//  1 · MOTOR CONTROL  (6 tests)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn motor_pulse_single_tick() {
    let mut engine = TactileEngine::new();
    engine.add_effect(TactileEffect::Rumble {
        frequency_hz: 100.0,
        amplitude: 0.8,
        duration_ticks: 1,
    });
    let sample = engine.tick();
    // One-tick rumble — sine at t=0 is ~0, but the effect fires and expires.
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

    // Full gain → ~1.0 at t=0
    let out_full = mixer.tick();
    assert!((out_full.combined - 1.0).abs() < 0.02);

    // Now add another impact with half slot-gain
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
    // Two impacts: first decayed + second at 0.5 gain
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

    // Disabled → process_telemetry is a no-op
    mgr.set_enabled(false);
    assert!(!mgr.is_enabled());
    let snap = make_snapshot();
    assert!(mgr.process_telemetry(&snap).is_ok());

    // Re-enable
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

    // All slots occupied — one more must fail
    assert!(engine
        .add_effect(TactileEffect::Impact {
            magnitude: 0.5,
            decay_rate: 5.0,
        })
        .is_none());

    // Ticking with all slots still clamps output
    let v = engine.tick();
    assert!((-1.0..=1.0).contains(&v));
}

// ═══════════════════════════════════════════════════════════════════════
//  2 · EFFECT ZONES  (6 tests)
// ═══════════════════════════════════════════════════════════════════════

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

    // Only fire touchdown
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
    // Router uses max-of, so result should be 0.6
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

    // Reduce gain to 50%
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

// ═══════════════════════════════════════════════════════════════════════
//  3 · EFFECT PATTERNS  (6 tests)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn pattern_landing_gear_bump() {
    let effects = TactilePresets::landing_gear_down();
    let mut engine = TactileEngine::new();
    for e in effects {
        engine.add_effect(e);
    }
    assert_eq!(engine.active_count(), 2);

    // Impact should produce a strong first sample
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
    assert!(amp_fast > amp_slow, "faster speed → stronger texture");
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
    assert!(amp_severe > amp_mild, "deeper stall → stronger buffet");
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
            assert!(a2 > a1, "higher RPM → stronger vibration");
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
    // Simulate overspeed as a high-frequency texture effect
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

// ═══════════════════════════════════════════════════════════════════════
//  4 · SAFETY  (5 tests)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn safety_maximum_intensity_clamped() {
    let mut engine = TactileEngine::new();
    // Fill all slots with maximum-magnitude impacts
    for _ in 0..MAX_EFFECTS {
        engine.add_effect(TactileEffect::Impact {
            magnitude: 1.0,
            decay_rate: 0.01, // very slow decay → all near 1.0 at t=0
        });
    }
    let sample = engine.tick();
    assert!(
        (-1.0..=1.0).contains(&sample),
        "output must never exceed ±1.0"
    );
}

#[test]
fn safety_thermal_protection_sustained_load() {
    // Simulate sustained full-power output across many ticks — mixer must clamp
    let mut mixer = TactileMixer::new();
    mixer.set_master_gain(2.0); // max allowed

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
        assert!((-1.0..=1.0).contains(&out.low));
        assert!((-1.0..=1.0).contains(&out.mid));
        assert!((-1.0..=1.0).contains(&out.high));
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

    // Post-expiry output must be zero
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
fn safety_fadeout_on_disconnect() {
    // Simulates disconnect by dropping the bridge → TactileManager::stop clears state
    let mut mgr = TactileManager::new();
    mgr.initialize(TactileConfig::default()).unwrap();
    mgr.set_enabled(true);

    // Stop is the disconnect path
    mgr.stop().unwrap();

    // After stop, process_telemetry is a no-op (bridge is None)
    let snap = make_snapshot();
    assert!(mgr.process_telemetry(&snap).is_ok());
}

// ═══════════════════════════════════════════════════════════════════════
//  5 · CONFIGURATION  (5 tests)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn config_effect_mapping_from_profile() {
    let mut mapping = ChannelMapping::new();
    mapping.set_mapping(EffectType::Touchdown, ChannelId::new(7));
    assert_eq!(
        mapping.get_channel(EffectType::Touchdown),
        Some(ChannelId::new(7))
    );
}

#[test]
fn config_sensitivity_adjustment() {
    let mut mixer = TactileMixer::new();
    mixer.set_master_gain(0.25);
    mixer.add_effect(TactileEffect::Impact {
        magnitude: 1.0,
        decay_rate: 5.0,
    });
    let out = mixer.tick();
    assert!(
        (out.combined - 0.25).abs() < 0.02,
        "master gain acts as sensitivity"
    );
}

#[test]
fn config_per_effect_enable_disable() {
    let mut config = TactileConfig::default();

    // Disable touchdown
    config.effect_enabled.insert(EffectType::Touchdown, false);
    assert!(!config.effect_enabled[&EffectType::Touchdown]);

    // Stall buffet stays enabled
    assert!(config.effect_enabled[&EffectType::StallBuffet]);
}

#[test]
fn config_global_enable_disable() {
    let mut mgr = TactileManager::new();
    mgr.initialize(TactileConfig::default()).unwrap();

    mgr.set_enabled(false);
    assert!(!mgr.is_enabled());

    mgr.set_enabled(true);
    assert!(mgr.is_enabled());
}

#[test]
fn config_defaults_all_effects_enabled() {
    let config = TactileConfig::default();
    for effect_type in [
        EffectType::Touchdown,
        EffectType::GroundRoll,
        EffectType::StallBuffet,
        EffectType::EngineVibration,
        EffectType::GearWarning,
        EffectType::RotorVibration,
    ] {
        assert!(
            *config.effect_enabled.get(&effect_type).unwrap(),
            "{effect_type:?} should be enabled by default"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  6 · INTEGRATION  (5 tests)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn integration_telemetry_to_touchdown_effect() {
    let mut processor = EffectProcessor::new();

    // Start airborne
    let mut snap = make_snapshot();
    snap.environment.altitude = 200.0;
    snap.kinematics.vertical_speed = -400.0;
    processor.process(&snap); // sets state: airborne

    // Transition to ground with descent rate
    snap.environment.altitude = 10.0;
    snap.kinematics.vertical_speed = -300.0;
    let events = processor.process(&snap);

    let touchdown = events
        .iter()
        .find(|e| e.effect_type == EffectType::Touchdown);
    assert!(touchdown.is_some(), "touchdown must fire on air→ground");
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
    // Apply config filter (same logic as bridge)
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
    // Route different effects to different "devices" (channels)
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

    let ch0 = outputs
        .iter()
        .find(|o| o.channel_id == ChannelId::new(0))
        .unwrap();
    let ch2 = outputs
        .iter()
        .find(|o| o.channel_id == ChannelId::new(2))
        .unwrap();
    let ch4 = outputs
        .iter()
        .find(|o| o.channel_id == ChannelId::new(4))
        .unwrap();

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

// ═══════════════════════════════════════════════════════════════════════
//  BONUS · ADDITIONAL DEPTH COVERAGE
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mixer_band_separation_low_mid_high() {
    let mut mixer = TactileMixer::new();

    // Low band: impact at 20 Hz
    mixer.add_effect(TactileEffect::Impact {
        magnitude: 0.5,
        decay_rate: 5.0,
    });
    // Mid band: rumble at 80 Hz
    mixer.add_effect(TactileEffect::Rumble {
        frequency_hz: 80.0,
        amplitude: 0.5,
        duration_ticks: 100,
    });
    // High band: texture at 200 Hz
    mixer.add_effect(TactileEffect::Texture {
        frequency_hz: 200.0,
        amplitude: 0.5,
        pattern: TexturePattern::Square,
    });

    let out = mixer.tick();
    // Impact at t=0 → low should be ≈0.5
    assert!(out.low.abs() > 0.1, "low band must carry impact");
}

#[test]
fn effect_slot_reuse_after_expiry() {
    let mut engine = TactileEngine::new();

    // Fill all slots with short rumbles
    for _ in 0..MAX_EFFECTS {
        engine.add_effect(TactileEffect::Rumble {
            frequency_hz: 50.0,
            amplitude: 0.1,
            duration_ticks: 5,
        });
    }
    assert_eq!(engine.active_count(), MAX_EFFECTS);

    // Expire them
    for _ in 0..5 {
        engine.tick();
    }
    assert_eq!(engine.active_count(), 0);

    // Slots should now be reusable
    let slot = engine.add_effect(TactileEffect::Impact {
        magnitude: 0.5,
        decay_rate: 5.0,
    });
    assert!(slot.is_some(), "slots must be reusable after expiry");
    assert_eq!(slot.unwrap(), 0, "should reuse first slot");
}

#[test]
fn mixer_zero_master_gain_silences_output() {
    let mut mixer = TactileMixer::new();
    mixer.set_master_gain(0.0);
    mixer.add_effect(TactileEffect::Impact {
        magnitude: 1.0,
        decay_rate: 5.0,
    });
    let out = mixer.tick();
    assert_eq!(out.combined, 0.0, "zero master gain must silence");
}

#[test]
fn mixer_band_gain_zeroes_specific_band() {
    let mut mixer = TactileMixer::new();
    mixer.set_band_gain(FrequencyBand::Low, 0.0);

    // Impact is low band (20 Hz)
    mixer.add_effect(TactileEffect::Impact {
        magnitude: 1.0,
        decay_rate: 5.0,
    });
    let out = mixer.tick();
    assert_eq!(out.low, 0.0, "zeroed low band must produce no low output");
    assert_eq!(out.combined, 0.0, "only low-band content, so combined = 0");
}

#[test]
fn effect_intensity_boundary_values() {
    assert!(EffectIntensity::new(0.0).is_ok());
    assert!(EffectIntensity::new(1.0).is_ok());
    assert!(EffectIntensity::new(0.5).is_ok());
    assert!(EffectIntensity::new(-0.001).is_err());
    assert!(EffectIntensity::new(1.001).is_err());
    assert!(EffectIntensity::new(f32::NAN).is_err());
}

#[test]
fn bridge_rejects_invalid_update_rate() {
    let enabled = Arc::new(RwLock::new(true));

    let cfg_zero = TactileConfig {
        update_rate_hz: 0.0,
        ..TactileConfig::default()
    };
    assert!(TactileBridge::new(cfg_zero, enabled.clone()).is_err());

    let cfg_neg = TactileConfig {
        update_rate_hz: -1.0,
        ..TactileConfig::default()
    };
    assert!(TactileBridge::new(cfg_neg, enabled.clone()).is_err());

    let cfg_high = TactileConfig {
        update_rate_hz: 2000.0,
        ..TactileConfig::default()
    };
    assert!(TactileBridge::new(cfg_high, enabled).is_err());
}

#[test]
fn bridge_rejects_zero_queue_size() {
    let enabled = Arc::new(RwLock::new(true));
    let cfg = TactileConfig {
        max_queue_size: 0,
        ..TactileConfig::default()
    };
    assert!(TactileBridge::new(cfg, enabled).is_err());
}

#[test]
fn gear_warning_fires_when_gear_up_low_altitude() {
    let mut processor = EffectProcessor::new();
    let mut snap = make_snapshot();
    snap.environment.altitude = 500.0;
    snap.kinematics.ias = ValidatedSpeed::new_knots(120.0).unwrap();
    // Default gear state is all-down; set to up
    snap.config.gear.nose = flight_bus::GearPosition::Up;
    snap.config.gear.left = flight_bus::GearPosition::Up;
    snap.config.gear.right = flight_bus::GearPosition::Up;

    let events = processor.process(&snap);
    let gear_warn = events
        .iter()
        .find(|e| e.effect_type == EffectType::GearWarning);
    assert!(gear_warn.is_some(), "gear warning should fire when gear up at low altitude");
}

#[test]
fn no_engine_vibration_when_engines_off() {
    let mut processor = EffectProcessor::new();
    let mut snap = make_snapshot();
    snap.engines = vec![make_engine(false, 0.0)];

    let events = processor.process(&snap);
    let engine_ev = events
        .iter()
        .find(|e| e.effect_type == EffectType::EngineVibration);
    assert!(engine_ev.is_none(), "stopped engine must not vibrate");
}

#[test]
fn rotor_vibration_requires_nr_above_threshold() {
    let mut processor = EffectProcessor::new();
    let mut snap = make_snapshot();

    // Below threshold
    snap.helo = Some(make_helo(80.0, 50.0));
    let events = processor.process(&snap);
    assert!(
        !events
            .iter()
            .any(|e| e.effect_type == EffectType::RotorVibration),
        "low Nr should not produce rotor vibration"
    );

    // Above threshold
    snap.helo = Some(make_helo(95.0, 60.0));
    let events = processor.process(&snap);
    assert!(
        events
            .iter()
            .any(|e| e.effect_type == EffectType::RotorVibration),
        "high Nr should produce rotor vibration"
    );
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

#[test]
fn tick_count_increments_correctly() {
    let mut engine = TactileEngine::new();
    assert_eq!(engine.tick_count(), 0);
    for i in 1..=100 {
        engine.tick();
        assert_eq!(engine.tick_count(), i);
    }
}

#[test]
fn manager_config_roundtrip() {
    let mgr = TactileManager::new();
    let config = TactileConfig::default();
    mgr.update_config(config.clone()).unwrap();
    let retrieved = mgr.get_config();
    assert_eq!(retrieved.update_rate_hz, config.update_rate_hz);
    assert_eq!(retrieved.max_queue_size, config.max_queue_size);
}

// ═══════════════════════════════════════════════════════════════════════
//  7 · ADDITIONAL DEPTH: EDGE CASES & PROPERTY TESTS
// ═══════════════════════════════════════════════════════════════════════

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
    engine.remove_effect(MAX_EFFECTS + 10); // out of bounds
    assert_eq!(engine.active_count(), 1, "out-of-bounds remove must be no-op");
}

#[test]
fn effect_processor_custom_stall_threshold() {
    let mut processor = EffectProcessor::new();
    processor.set_stall_threshold(25.0); // raise threshold

    let mut snap = make_snapshot();
    snap.kinematics.aoa = ValidatedAngle::new_degrees(22.0).unwrap();
    snap.kinematics.ias = ValidatedSpeed::new_knots(55.0).unwrap();

    let events = processor.process(&snap);
    assert!(
        !events.iter().any(|e| e.effect_type == EffectType::StallBuffet),
        "22° AoA should not trigger stall at 25° threshold"
    );
}

#[test]
fn effect_processor_custom_ground_roll_threshold() {
    let mut processor = EffectProcessor::new();
    processor.set_ground_roll_threshold(50.0); // raise threshold

    let mut snap = make_snapshot();
    snap.environment.altitude = 5.0;
    snap.kinematics.ground_speed = ValidatedSpeed::new_knots(30.0).unwrap();

    let events = processor.process(&snap);
    assert!(
        !events.iter().any(|e| e.effect_type == EffectType::GroundRoll),
        "30 kt should not trigger ground roll at 50 kt threshold"
    );
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
    // Default has 8 channels (0..8)
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
fn mixer_slot_gain_out_of_bounds_returns_zero() {
    let mixer = TactileMixer::new();
    assert_eq!(mixer.slot_gain(MAX_EFFECTS + 5), 0.0);
}

#[test]
fn preset_turbulence_clamped_at_extremes() {
    // Negative intensity should clamp to 0
    match TactilePresets::turbulence(-1.0) {
        TactileEffect::Texture { amplitude, .. } => {
            assert_eq!(amplitude, 0.0, "negative intensity → zero amplitude");
        }
        _ => panic!("expected Texture"),
    }
    // Very high intensity should clamp to max
    match TactilePresets::turbulence(5.0) {
        TactileEffect::Texture { amplitude, .. } => {
            assert!(amplitude <= 0.7 + 1e-9, "clamped intensity → max amplitude");
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

    // After 10 ticks, should have decayed significantly
    for _ in 0..9 {
        engine.tick();
    }
    let tenth = engine.tick();
    assert!(tenth < 0.6, "weapon fire must decay fast (decay_rate=15)");
}

#[test]
fn effect_event_remaining_duration_decreases() {
    let event = EffectEvent::with_duration(
        EffectType::Touchdown,
        EffectIntensity::new(1.0).unwrap(),
        std::time::Duration::from_secs(1),
    );
    let remaining = event.remaining_duration().unwrap();
    assert!(remaining <= std::time::Duration::from_secs(1));
    assert!(remaining > std::time::Duration::from_millis(900));
}

#[test]
fn effect_event_no_duration_never_expires() {
    let event = EffectEvent::new(EffectType::EngineVibration, EffectIntensity::new(0.5).unwrap());
    assert!(!event.is_expired(), "no-duration event must never expire");
    assert!(event.remaining_duration().is_none());
}

#[test]
fn mixer_tick_count_tracks_engine() {
    let mut mixer = TactileMixer::new();
    for _ in 0..42 {
        mixer.tick();
    }
    assert_eq!(mixer.tick_count(), 42);
    assert_eq!(mixer.engine().tick_count(), 42);
}

#[test]
fn manager_stats_none_before_init() {
    let mgr = TactileManager::new();
    assert!(mgr.get_stats().is_none(), "stats require bridge init");
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

    // First call: airborne
    snap.environment.altitude = 200.0;
    snap.kinematics.vertical_speed = -300.0;
    processor.process(&snap);

    // Gentle touchdown
    snap.environment.altitude = 10.0;
    snap.kinematics.vertical_speed = -250.0;
    let gentle = processor.process(&snap);
    let gentle_int = gentle
        .iter()
        .find(|e| e.effect_type == EffectType::Touchdown)
        .unwrap()
        .intensity
        .value();

    // Reset: airborne again
    snap.environment.altitude = 200.0;
    snap.kinematics.vertical_speed = -300.0;
    processor.process(&snap);

    // Hard touchdown
    snap.environment.altitude = 10.0;
    snap.kinematics.vertical_speed = -500.0;
    let hard = processor.process(&snap);
    let hard_int = hard
        .iter()
        .find(|e| e.effect_type == EffectType::Touchdown)
        .unwrap()
        .intensity
        .value();

    assert!(
        hard_int > gentle_int,
        "harder landing must produce stronger effect"
    );
}

// ── proptest ────────────────────────────────────────────────────────────

mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn engine_output_always_clamped(
            freq in 1.0f64..500.0,
            amp in 0.0f64..1.0,
            ticks in 1u32..200,
        ) {
            let mut engine = TactileEngine::new();
            engine.add_effect(TactileEffect::Rumble {
                frequency_hz: freq,
                amplitude: amp,
                duration_ticks: ticks,
            });
            for _ in 0..ticks {
                let v = engine.tick();
                prop_assert!((-1.0..=1.0).contains(&v));
            }
        }

        #[test]
        fn effect_intensity_rejects_out_of_range(val in -10.0f32..10.0) {
            let result = EffectIntensity::new(val);
            if (0.0..=1.0).contains(&val) {
                prop_assert!(result.is_ok());
            } else {
                prop_assert!(result.is_err());
            }
        }

        #[test]
        fn mixer_combined_always_clamped(
            mag in 0.0f64..1.0,
            decay in 0.1f64..20.0,
            gain in 0.0f64..2.0,
        ) {
            let mut mixer = TactileMixer::new();
            mixer.set_master_gain(gain);
            mixer.add_effect(TactileEffect::Impact {
                magnitude: mag,
                decay_rate: decay,
            });
            let out = mixer.tick();
            prop_assert!((-1.0..=1.0).contains(&out.combined));
            prop_assert!((-1.0..=1.0).contains(&out.low));
        }
    }
}
