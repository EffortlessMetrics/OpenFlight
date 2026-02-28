// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Flight-specific tactile presets
//!
//! Ready-made effect configurations for common flight events such as
//! landing gear extension, runway roll, engine vibration, stall buffet,
//! weapon fire, and turbulence.

use crate::engine::{TactileEffect, TexturePattern};

/// Collection of flight-specific tactile presets.
///
/// Each method returns one or more [`TactileEffect`] values that can be
/// directly queued into a [`TactileEngine`](crate::engine::TactileEngine) or
/// [`TactileMixer`](crate::mixer::TactileMixer).
pub struct TactilePresets;

impl TactilePresets {
    /// Landing gear extension / retraction: impact hit followed by rumble.
    ///
    /// Returns two effects that should both be queued.
    pub fn landing_gear_down() -> [TactileEffect; 2] {
        [
            TactileEffect::Impact {
                magnitude: 0.8,
                decay_rate: 5.0,
            },
            TactileEffect::Rumble {
                frequency_hz: 30.0,
                amplitude: 0.4,
                duration_ticks: 75, // 300 ms at 250 Hz
            },
        ]
    }

    /// Runway roll texture scaled by ground speed.
    ///
    /// Frequency and amplitude increase with `ground_speed_knots` (≥ 0).
    pub fn runway_roll(ground_speed_knots: f64) -> TactileEffect {
        let speed = ground_speed_knots.max(0.0);
        let freq = 10.0 + speed * 0.5;
        let amp = (speed / 100.0).min(0.6);
        TactileEffect::Texture {
            frequency_hz: freq,
            amplitude: amp,
            pattern: TexturePattern::Sawtooth,
        }
    }

    /// Engine vibration proportional to RPM.
    ///
    /// Typical GA piston range: 600–2 700 RPM.
    pub fn engine_vibration(rpm: f64) -> TactileEffect {
        let rpm = rpm.max(0.0);
        let amp = (rpm / 2700.0).min(0.3);
        TactileEffect::Engine {
            rpm,
            amplitude: amp,
        }
    }

    /// Stall buffet with increasing frequency beyond the critical AoA.
    ///
    /// `aoa_excess_degrees` is how many degrees past the stall angle.
    pub fn stall_buffet(aoa_excess_degrees: f64) -> TactileEffect {
        let excess = aoa_excess_degrees.max(0.0);
        let freq = 5.0 + excess * 2.0;
        let amp = (excess / 10.0).min(0.8);
        TactileEffect::Texture {
            frequency_hz: freq,
            amplitude: amp,
            pattern: TexturePattern::Triangle,
        }
    }

    /// Sharp impact for weapon fire / cannon recoil.
    pub fn weapon_fire() -> TactileEffect {
        TactileEffect::Impact {
            magnitude: 1.0,
            decay_rate: 15.0,
        }
    }

    /// Turbulence as a random-feeling texture.
    ///
    /// `intensity` is clamped to 0.0 (calm) – 1.0 (severe).
    pub fn turbulence(intensity: f64) -> TactileEffect {
        let intensity = intensity.clamp(0.0, 1.0);
        let freq = 3.0 + intensity * 8.0;
        let amp = intensity * 0.7;
        TactileEffect::Texture {
            frequency_hz: freq,
            amplitude: amp,
            pattern: TexturePattern::Triangle,
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{TactileEngine, TexturePattern};

    // ── landing gear ────────────────────────────────────────────────

    #[test]
    fn test_landing_gear_down() {
        let effects = TactilePresets::landing_gear_down();
        assert_eq!(effects.len(), 2);

        match effects[0] {
            TactileEffect::Impact {
                magnitude,
                decay_rate,
            } => {
                assert!(magnitude > 0.0 && magnitude <= 1.0);
                assert!(decay_rate > 0.0);
            }
            _ => panic!("first effect must be Impact"),
        }

        match effects[1] {
            TactileEffect::Rumble {
                frequency_hz,
                amplitude,
                duration_ticks,
            } => {
                assert!(frequency_hz > 0.0);
                assert!(amplitude > 0.0 && amplitude <= 1.0);
                assert!(duration_ticks > 0);
            }
            _ => panic!("second effect must be Rumble"),
        }
    }

    // ── runway roll ─────────────────────────────────────────────────

    #[test]
    fn test_runway_roll_various_speeds() {
        for &speed in &[0.0, 10.0, 50.0, 100.0, 200.0] {
            match TactilePresets::runway_roll(speed) {
                TactileEffect::Texture {
                    frequency_hz,
                    amplitude,
                    pattern,
                } => {
                    assert!(frequency_hz > 0.0, "freq > 0 at speed {speed}");
                    assert!(
                        (0.0..=1.0).contains(&amplitude),
                        "amp in range at speed {speed}"
                    );
                    assert_eq!(pattern, TexturePattern::Sawtooth);
                }
                _ => panic!("expected Texture"),
            }
        }
    }

    #[test]
    fn test_runway_roll_negative_speed() {
        match TactilePresets::runway_roll(-10.0) {
            TactileEffect::Texture { amplitude, .. } => assert_eq!(amplitude, 0.0),
            _ => panic!("expected Texture"),
        }
    }

    // ── engine vibration ────────────────────────────────────────────

    #[test]
    fn test_engine_vibration_range() {
        for &rpm in &[0.0, 600.0, 1200.0, 2400.0, 2700.0, 5000.0] {
            match TactilePresets::engine_vibration(rpm) {
                TactileEffect::Engine { rpm: r, amplitude } => {
                    assert!(r >= 0.0);
                    assert!((0.0..=0.3 + 1e-9).contains(&amplitude));
                }
                _ => panic!("expected Engine"),
            }
        }
    }

    // ── stall buffet ────────────────────────────────────────────────

    #[test]
    fn test_stall_buffet_range() {
        for &excess in &[0.0, 2.0, 5.0, 10.0, 20.0] {
            match TactilePresets::stall_buffet(excess) {
                TactileEffect::Texture {
                    frequency_hz,
                    amplitude,
                    pattern,
                } => {
                    assert!(frequency_hz >= 5.0);
                    assert!((0.0..=0.8 + 1e-9).contains(&amplitude));
                    assert_eq!(pattern, TexturePattern::Triangle);
                }
                _ => panic!("expected Texture"),
            }
        }
    }

    // ── weapon fire ─────────────────────────────────────────────────

    #[test]
    fn test_weapon_fire() {
        match TactilePresets::weapon_fire() {
            TactileEffect::Impact {
                magnitude,
                decay_rate,
            } => {
                assert_eq!(magnitude, 1.0);
                assert!(decay_rate > 10.0, "weapon fire needs fast decay");
            }
            _ => panic!("expected Impact"),
        }
    }

    // ── turbulence ──────────────────────────────────────────────────

    #[test]
    fn test_turbulence_range() {
        for &intensity in &[0.0, 0.25, 0.5, 0.75, 1.0] {
            match TactilePresets::turbulence(intensity) {
                TactileEffect::Texture {
                    frequency_hz,
                    amplitude,
                    pattern,
                } => {
                    assert!(frequency_hz >= 3.0);
                    assert!((0.0..=0.7 + 1e-9).contains(&amplitude));
                    assert_eq!(pattern, TexturePattern::Triangle);
                }
                _ => panic!("expected Texture"),
            }
        }
    }

    #[test]
    fn test_turbulence_clamping() {
        match TactilePresets::turbulence(2.0) {
            TactileEffect::Texture { amplitude, .. } => {
                assert!(amplitude <= 0.7 + 1e-9, "intensity should be clamped");
            }
            _ => panic!("expected Texture"),
        }
    }

    // ── integration: presets through engine ──────────────────────────

    #[test]
    fn test_presets_playback_through_engine() {
        let mut engine = TactileEngine::new();

        // Landing gear
        for effect in TactilePresets::landing_gear_down() {
            engine.add_effect(effect);
        }
        assert_eq!(engine.active_count(), 2);

        for _ in 0..10 {
            let v = engine.tick();
            assert!((-1.0..=1.0).contains(&v));
        }

        // Weapon fire
        engine.add_effect(TactilePresets::weapon_fire());
        let v = engine.tick();
        assert!((-1.0..=1.0).contains(&v));

        // Turbulence
        engine.add_effect(TactilePresets::turbulence(0.5));
        for _ in 0..50 {
            let v = engine.tick();
            assert!((-1.0..=1.0).contains(&v));
        }
    }

    #[test]
    fn test_all_presets_produce_output() {
        let mut engine = TactileEngine::new();

        engine.add_effect(TactilePresets::runway_roll(60.0));
        engine.add_effect(TactilePresets::engine_vibration(2000.0));
        engine.add_effect(TactilePresets::stall_buffet(5.0));
        engine.add_effect(TactilePresets::weapon_fire());
        engine.add_effect(TactilePresets::turbulence(0.8));

        let mut any_nonzero = false;
        for _ in 0..50 {
            let v = engine.tick();
            if v.abs() > 0.001 {
                any_nonzero = true;
            }
        }
        assert!(any_nonzero, "presets should produce audible output");
    }
}
