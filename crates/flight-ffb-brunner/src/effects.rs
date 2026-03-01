// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Force-feedback effect engine for Brunner CLS-E / CLS-P devices.
//!
//! Implements the core FFB effect types supported by the Brunner control
//! loading system:
//!
//! - **Spring** — position-proportional restoring force (centering)
//! - **Damper** — velocity-proportional resistance
//! - **Friction** — coulomb friction (direction-dependent resistance)
//! - **Constant force** — static directional force vector
//! - **Periodic** — sine, square, triangle, or sawtooth oscillation
//!
//! All effect computations produce a normalised force output in the range
//! \[-1.0, 1.0\]. The safety envelope (see [`crate::safety`]) clamps the
//! final output before it reaches the hardware.

use serde::{Deserialize, Serialize};

/// Periodic waveform shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PeriodicWaveform {
    Sine,
    Square,
    Triangle,
    Sawtooth,
}

/// Spring effect parameters.
///
/// Produces a restoring force proportional to displacement from `center`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SpringParams {
    /// Spring coefficient (stiffness). Range: 0.0..=1.0.
    pub coefficient: f32,
    /// Center position (normalised -1.0..1.0). Default 0.0.
    pub center: f32,
    /// Dead band half-width around center. Default 0.0.
    pub dead_band: f32,
    /// Saturation force (max output magnitude). Default 1.0.
    pub saturation: f32,
}

impl Default for SpringParams {
    fn default() -> Self {
        Self {
            coefficient: 0.5,
            center: 0.0,
            dead_band: 0.0,
            saturation: 1.0,
        }
    }
}

/// Damper effect parameters.
///
/// Produces a force opposing the velocity of movement.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DamperParams {
    /// Damping coefficient. Range: 0.0..=1.0.
    pub coefficient: f32,
}

impl Default for DamperParams {
    fn default() -> Self {
        Self { coefficient: 0.3 }
    }
}

/// Friction effect parameters (coulomb friction model).
///
/// Produces a constant opposing force when the axis is in motion.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FrictionParams {
    /// Friction magnitude. Range: 0.0..=1.0.
    pub coefficient: f32,
}

impl Default for FrictionParams {
    fn default() -> Self {
        Self { coefficient: 0.15 }
    }
}

/// Constant force parameters.
///
/// Produces a static force in a given direction (e.g. trim, crosswind).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ConstantForceParams {
    /// Force magnitude. Range: -1.0..=1.0.
    pub magnitude: f32,
}

impl Default for ConstantForceParams {
    fn default() -> Self {
        Self { magnitude: 0.0 }
    }
}

/// Periodic effect parameters.
///
/// Produces an oscillating force for vibration effects (engine, turbulence, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PeriodicParams {
    /// Waveform shape.
    pub waveform: PeriodicWaveform,
    /// Oscillation frequency in Hz. Clamped to 1..=200.
    pub frequency_hz: f32,
    /// Amplitude (peak magnitude). Range: 0.0..=1.0.
    pub amplitude: f32,
    /// Phase offset in radians.
    pub phase: f32,
}

impl Default for PeriodicParams {
    fn default() -> Self {
        Self {
            waveform: PeriodicWaveform::Sine,
            frequency_hz: 10.0,
            amplitude: 0.2,
            phase: 0.0,
        }
    }
}

/// A single FFB effect.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum BrunnerEffect {
    Spring(SpringParams),
    Damper(DamperParams),
    Friction(FrictionParams),
    ConstantForce(ConstantForceParams),
    Periodic(PeriodicParams),
}

/// Composite of multiple effects applied to a single axis.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EffectComposite {
    effects: Vec<BrunnerEffect>,
}

impl EffectComposite {
    /// Create an empty composite.
    pub fn new() -> Self {
        Self {
            effects: Vec::new(),
        }
    }

    /// Add an effect to the composite.
    pub fn add(&mut self, effect: BrunnerEffect) {
        self.effects.push(effect);
    }

    /// Remove all effects.
    pub fn clear(&mut self) {
        self.effects.clear();
    }

    /// Number of active effects.
    pub fn len(&self) -> usize {
        self.effects.len()
    }

    /// Returns `true` if no effects are active.
    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    /// Iterate over the contained effects.
    pub fn effects(&self) -> &[BrunnerEffect] {
        &self.effects
    }

    /// Compute the combined force output at the given position and velocity.
    ///
    /// `time_s` is the current simulation time (used for periodic effects).
    /// Returns a normalised force in \[-1.0, 1.0\].
    pub fn compute(&self, position: f32, velocity: f32, time_s: f64) -> f32 {
        let mut total = 0.0f32;
        for effect in &self.effects {
            total += compute_effect_force(effect, position, velocity, time_s);
        }
        total.clamp(-1.0, 1.0)
    }
}

/// Compute the force output for a single effect.
///
/// - `position`: current axis position, normalised -1.0..1.0
/// - `velocity`: current axis velocity (per-tick delta, positive = increasing)
/// - `time_s`: current simulation time in seconds (for periodic effects)
///
/// Returns a force value in \[-1.0, 1.0\].
pub fn compute_effect_force(
    effect: &BrunnerEffect,
    position: f32,
    velocity: f32,
    time_s: f64,
) -> f32 {
    match effect {
        BrunnerEffect::Spring(p) => compute_spring(p, position),
        BrunnerEffect::Damper(p) => compute_damper(p, velocity),
        BrunnerEffect::Friction(p) => compute_friction(p, velocity),
        BrunnerEffect::ConstantForce(p) => p.magnitude.clamp(-1.0, 1.0),
        BrunnerEffect::Periodic(p) => compute_periodic(p, time_s),
    }
}

fn compute_spring(p: &SpringParams, position: f32) -> f32 {
    let coeff = p.coefficient.clamp(0.0, 1.0);
    let sat = p.saturation.clamp(0.0, 1.0);
    let dead = p.dead_band.clamp(0.0, 1.0);

    let displacement = position - p.center.clamp(-1.0, 1.0);

    // Apply dead band
    let effective = if displacement.abs() <= dead {
        0.0
    } else if displacement > 0.0 {
        displacement - dead
    } else {
        displacement + dead
    };

    // Spring force = -coefficient * displacement (restoring)
    let force = -coeff * effective;
    force.clamp(-sat, sat)
}

fn compute_damper(p: &DamperParams, velocity: f32) -> f32 {
    let coeff = p.coefficient.clamp(0.0, 1.0);
    // Damper opposes velocity
    let force = -coeff * velocity;
    force.clamp(-1.0, 1.0)
}

fn compute_friction(p: &FrictionParams, velocity: f32) -> f32 {
    let coeff = p.coefficient.clamp(0.0, 1.0);
    // Coulomb friction: constant opposing force when in motion
    if velocity.abs() < 1e-6 {
        0.0
    } else if velocity > 0.0 {
        -coeff
    } else {
        coeff
    }
}

fn compute_periodic(p: &PeriodicParams, time_s: f64) -> f32 {
    let freq = p.frequency_hz.clamp(1.0, 200.0) as f64;
    let amp = p.amplitude.clamp(0.0, 1.0);
    let phase = p.phase as f64;

    let t = time_s * freq * std::f64::consts::TAU + phase;

    let wave = match p.waveform {
        PeriodicWaveform::Sine => t.sin() as f32,
        PeriodicWaveform::Square => {
            if t.sin() >= 0.0 {
                1.0
            } else {
                -1.0
            }
        }
        PeriodicWaveform::Triangle => (2.0 / std::f32::consts::PI) * (t as f32).sin().asin(),
        PeriodicWaveform::Sawtooth => {
            let phase_norm = (t / std::f64::consts::TAU).fract() as f32;
            2.0 * phase_norm - 1.0
        }
    };

    (wave * amp).clamp(-1.0, 1.0)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Spring effect ─────────────────────────────────────────────────────────

    #[test]
    fn spring_zero_at_center() {
        let p = SpringParams::default();
        let force = compute_spring(&p, 0.0);
        assert!(
            (force).abs() < 1e-6,
            "force at center should be 0, got {force}"
        );
    }

    #[test]
    fn spring_positive_displacement_gives_negative_force() {
        let p = SpringParams {
            coefficient: 0.5,
            center: 0.0,
            dead_band: 0.0,
            saturation: 1.0,
        };
        let force = compute_spring(&p, 0.5);
        assert!(
            force < 0.0,
            "positive displacement should give restoring (negative) force"
        );
        assert!(
            (force - (-0.25)).abs() < 1e-6,
            "expected -0.25, got {force}"
        );
    }

    #[test]
    fn spring_negative_displacement_gives_positive_force() {
        let p = SpringParams {
            coefficient: 0.5,
            center: 0.0,
            dead_band: 0.0,
            saturation: 1.0,
        };
        let force = compute_spring(&p, -0.5);
        assert!(
            force > 0.0,
            "negative displacement should give positive force"
        );
        assert!((force - 0.25).abs() < 1e-6, "expected 0.25, got {force}");
    }

    #[test]
    fn spring_off_center() {
        let p = SpringParams {
            coefficient: 1.0,
            center: 0.3,
            dead_band: 0.0,
            saturation: 1.0,
        };
        // At position 0.3 (center), force should be ~0
        let force = compute_spring(&p, 0.3);
        assert!((force).abs() < 1e-6);
    }

    #[test]
    fn spring_dead_band_suppresses_small_displacement() {
        let p = SpringParams {
            coefficient: 1.0,
            center: 0.0,
            dead_band: 0.1,
            saturation: 1.0,
        };
        // Position within dead band
        let force = compute_spring(&p, 0.05);
        assert!(
            (force).abs() < 1e-6,
            "within dead band force should be 0, got {force}"
        );
    }

    #[test]
    fn spring_dead_band_force_outside() {
        let p = SpringParams {
            coefficient: 1.0,
            center: 0.0,
            dead_band: 0.1,
            saturation: 1.0,
        };
        // Position 0.3 = displacement 0.3, effective = 0.3 - 0.1 = 0.2
        let force = compute_spring(&p, 0.3);
        assert!((force - (-0.2)).abs() < 1e-6, "expected -0.2, got {force}");
    }

    #[test]
    fn spring_saturation_clamp() {
        let p = SpringParams {
            coefficient: 1.0,
            center: 0.0,
            dead_band: 0.0,
            saturation: 0.3,
        };
        // Full deflection should be clamped to saturation
        let force = compute_spring(&p, 1.0);
        assert!(
            (force - (-0.3)).abs() < 1e-6,
            "should saturate at -0.3, got {force}"
        );
    }

    #[test]
    fn spring_coefficient_clamped_to_unit() {
        let p = SpringParams {
            coefficient: 2.0, // over range
            center: 0.0,
            dead_band: 0.0,
            saturation: 1.0,
        };
        let force = compute_spring(&p, 0.5);
        // coefficient clamped to 1.0, so force = -1.0 * 0.5 = -0.5
        assert!((force - (-0.5)).abs() < 1e-6);
    }

    // ── Damper effect ─────────────────────────────────────────────────────────

    #[test]
    fn damper_zero_velocity_gives_zero_force() {
        let p = DamperParams::default();
        let force = compute_damper(&p, 0.0);
        assert!((force).abs() < 1e-6);
    }

    #[test]
    fn damper_positive_velocity_gives_negative_force() {
        let p = DamperParams { coefficient: 0.5 };
        let force = compute_damper(&p, 0.8);
        assert!(force < 0.0);
        assert!((force - (-0.4)).abs() < 1e-6);
    }

    #[test]
    fn damper_negative_velocity_gives_positive_force() {
        let p = DamperParams { coefficient: 0.5 };
        let force = compute_damper(&p, -0.8);
        assert!(force > 0.0);
        assert!((force - 0.4).abs() < 1e-6);
    }

    #[test]
    fn damper_force_clamped() {
        let p = DamperParams { coefficient: 1.0 };
        // Velocity > 1.0 would produce force > 1.0 without clamp
        let force = compute_damper(&p, 2.0);
        assert!(
            (force - (-1.0)).abs() < 1e-6,
            "should clamp to -1.0, got {force}"
        );
    }

    // ── Friction effect ───────────────────────────────────────────────────────

    #[test]
    fn friction_zero_velocity_gives_zero_force() {
        let p = FrictionParams { coefficient: 0.5 };
        let force = compute_friction(&p, 0.0);
        assert!((force).abs() < 1e-6);
    }

    #[test]
    fn friction_positive_velocity_gives_negative_force() {
        let p = FrictionParams { coefficient: 0.3 };
        let force = compute_friction(&p, 0.5);
        assert!((force - (-0.3)).abs() < 1e-6);
    }

    #[test]
    fn friction_negative_velocity_gives_positive_force() {
        let p = FrictionParams { coefficient: 0.3 };
        let force = compute_friction(&p, -0.5);
        assert!((force - 0.3).abs() < 1e-6);
    }

    // ── Constant force ────────────────────────────────────────────────────────

    #[test]
    fn constant_force_passthrough() {
        let effect = BrunnerEffect::ConstantForce(ConstantForceParams { magnitude: 0.7 });
        let force = compute_effect_force(&effect, 0.0, 0.0, 0.0);
        assert!((force - 0.7).abs() < 1e-6);
    }

    #[test]
    fn constant_force_clamped() {
        let effect = BrunnerEffect::ConstantForce(ConstantForceParams { magnitude: 1.5 });
        let force = compute_effect_force(&effect, 0.0, 0.0, 0.0);
        assert!((force - 1.0).abs() < 1e-6);
    }

    // ── Periodic effects ──────────────────────────────────────────────────────

    #[test]
    fn sine_at_quarter_period_is_positive() {
        let p = PeriodicParams {
            waveform: PeriodicWaveform::Sine,
            frequency_hz: 1.0,
            amplitude: 1.0,
            phase: 0.0,
        };
        // At t=0.25s (quarter period of 1Hz), sin(2π*0.25) = sin(π/2) = 1.0
        let force = compute_periodic(&p, 0.25);
        assert!((force - 1.0).abs() < 1e-3, "expected ~1.0, got {force}");
    }

    #[test]
    fn sine_at_zero_is_zero() {
        let p = PeriodicParams {
            waveform: PeriodicWaveform::Sine,
            frequency_hz: 1.0,
            amplitude: 1.0,
            phase: 0.0,
        };
        let force = compute_periodic(&p, 0.0);
        assert!((force).abs() < 1e-3, "expected ~0.0, got {force}");
    }

    #[test]
    fn square_wave_positive_half() {
        let p = PeriodicParams {
            waveform: PeriodicWaveform::Square,
            frequency_hz: 1.0,
            amplitude: 0.5,
            phase: 0.0,
        };
        // In first half of period (t=0.1), sin > 0 → +0.5
        let force = compute_periodic(&p, 0.1);
        assert!((force - 0.5).abs() < 1e-6, "expected 0.5, got {force}");
    }

    #[test]
    fn square_wave_negative_half() {
        let p = PeriodicParams {
            waveform: PeriodicWaveform::Square,
            frequency_hz: 1.0,
            amplitude: 0.5,
            phase: 0.0,
        };
        // In second half of period (t=0.6), sin < 0 → -0.5
        let force = compute_periodic(&p, 0.6);
        assert!((force - (-0.5)).abs() < 1e-6, "expected -0.5, got {force}");
    }

    #[test]
    fn periodic_amplitude_clamped() {
        let p = PeriodicParams {
            waveform: PeriodicWaveform::Sine,
            frequency_hz: 1.0,
            amplitude: 2.0, // over range
            phase: 0.0,
        };
        // amplitude clamped to 1.0
        let force = compute_periodic(&p, 0.25);
        assert!(force <= 1.0 && force >= -1.0);
    }

    #[test]
    fn periodic_frequency_clamped_low() {
        let p = PeriodicParams {
            waveform: PeriodicWaveform::Sine,
            frequency_hz: 0.0, // clamped to 1.0
            amplitude: 1.0,
            phase: 0.0,
        };
        // Should behave as 1 Hz
        let force_quarter = compute_periodic(&p, 0.25);
        assert!((force_quarter - 1.0).abs() < 1e-3);
    }

    // ── Effect composite ──────────────────────────────────────────────────────

    #[test]
    fn composite_empty_gives_zero() {
        let composite = EffectComposite::new();
        let force = composite.compute(0.5, 0.1, 0.0);
        assert!((force).abs() < 1e-6);
    }

    #[test]
    fn composite_single_effect() {
        let mut composite = EffectComposite::new();
        composite.add(BrunnerEffect::ConstantForce(ConstantForceParams {
            magnitude: 0.5,
        }));
        let force = composite.compute(0.0, 0.0, 0.0);
        assert!((force - 0.5).abs() < 1e-6);
    }

    #[test]
    fn composite_multiple_effects_sum() {
        let mut composite = EffectComposite::new();
        composite.add(BrunnerEffect::ConstantForce(ConstantForceParams {
            magnitude: 0.3,
        }));
        composite.add(BrunnerEffect::ConstantForce(ConstantForceParams {
            magnitude: 0.2,
        }));
        let force = composite.compute(0.0, 0.0, 0.0);
        assert!((force - 0.5).abs() < 1e-6);
    }

    #[test]
    fn composite_clamped_to_unit_range() {
        let mut composite = EffectComposite::new();
        composite.add(BrunnerEffect::ConstantForce(ConstantForceParams {
            magnitude: 0.8,
        }));
        composite.add(BrunnerEffect::ConstantForce(ConstantForceParams {
            magnitude: 0.8,
        }));
        let force = composite.compute(0.0, 0.0, 0.0);
        assert!(
            (force - 1.0).abs() < 1e-6,
            "should clamp to 1.0, got {force}"
        );
    }

    #[test]
    fn composite_clear() {
        let mut composite = EffectComposite::new();
        composite.add(BrunnerEffect::ConstantForce(ConstantForceParams {
            magnitude: 0.5,
        }));
        assert_eq!(composite.len(), 1);
        composite.clear();
        assert!(composite.is_empty());
    }

    #[test]
    fn composite_spring_plus_damper() {
        let mut composite = EffectComposite::new();
        composite.add(BrunnerEffect::Spring(SpringParams {
            coefficient: 0.5,
            center: 0.0,
            dead_band: 0.0,
            saturation: 1.0,
        }));
        composite.add(BrunnerEffect::Damper(DamperParams { coefficient: 0.3 }));
        // Position 0.4, velocity 0.2
        // Spring: -0.5 * 0.4 = -0.2
        // Damper: -0.3 * 0.2 = -0.06
        // Total: -0.26
        let force = composite.compute(0.4, 0.2, 0.0);
        assert!(
            (force - (-0.26)).abs() < 1e-4,
            "expected -0.26, got {force}"
        );
    }

    // ── Property tests ────────────────────────────────────────────────────────

    mod prop {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn spring_force_always_in_range(
                pos in -1.0f32..=1.0,
                coeff in 0.0f32..=1.0,
                center in -1.0f32..=1.0,
            ) {
                let p = SpringParams { coefficient: coeff, center, dead_band: 0.0, saturation: 1.0 };
                let force = compute_spring(&p, pos);
                prop_assert!((-1.0..=1.0).contains(&force),
                    "spring force {} out of range for pos={}, coeff={}, center={}",
                    force, pos, coeff, center);
            }

            #[test]
            fn damper_force_always_in_range(
                vel in -5.0f32..=5.0,
                coeff in 0.0f32..=1.0,
            ) {
                let p = DamperParams { coefficient: coeff };
                let force = compute_damper(&p, vel);
                prop_assert!((-1.0..=1.0).contains(&force),
                    "damper force {} out of range", force);
            }

            #[test]
            fn friction_force_always_in_range(
                vel in -5.0f32..=5.0,
                coeff in 0.0f32..=1.0,
            ) {
                let p = FrictionParams { coefficient: coeff };
                let force = compute_friction(&p, vel);
                prop_assert!((-1.0..=1.0).contains(&force),
                    "friction force {} out of range", force);
            }

            #[test]
            fn periodic_force_always_in_range(
                time in 0.0f64..100.0,
                freq in 1.0f32..=200.0,
                amp in 0.0f32..=1.0,
            ) {
                let p = PeriodicParams {
                    waveform: PeriodicWaveform::Sine,
                    frequency_hz: freq,
                    amplitude: amp,
                    phase: 0.0,
                };
                let force = compute_periodic(&p, time);
                prop_assert!((-1.0..=1.0).contains(&force),
                    "periodic force {} out of range", force);
            }

            #[test]
            fn composite_force_always_in_range(
                pos in -1.0f32..=1.0,
                vel in -2.0f32..=2.0,
                time in 0.0f64..10.0,
            ) {
                let mut composite = EffectComposite::new();
                composite.add(BrunnerEffect::Spring(SpringParams::default()));
                composite.add(BrunnerEffect::Damper(DamperParams::default()));
                composite.add(BrunnerEffect::Friction(FrictionParams::default()));
                composite.add(BrunnerEffect::Periodic(PeriodicParams::default()));
                let force = composite.compute(pos, vel, time);
                prop_assert!((-1.0..=1.0).contains(&force),
                    "composite force {} out of range", force);
            }
        }
    }
}
