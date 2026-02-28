// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Core FFB effect types for force feedback synthesis
//!
//! Provides first-class, RT-safe effect primitives:
//! - **Constant force** — sustained control loading feel
//! - **Spring** — return-to-center centering force
//! - **Damper** — resistance proportional to velocity
//! - **Friction** — constant resistance opposing motion
//! - **Periodic** — sine, square, triangle, sawtooth waveforms
//! - **Ramp** — linear transition between two force levels (trim changes)
//! - **Composite** — combination of multiple effects with individual gains
//!
//! All computations are zero-allocation and safe for the 250 Hz RT hot path.
//! Force output is normalised to −1.0…+1.0 before final scaling.

use std::f32::consts::PI;

// ─── Waveform ────────────────────────────────────────────────────────────────

/// Waveform shape for periodic effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Waveform {
    Sine,
    Square,
    Triangle,
    Sawtooth,
}

// ─── Force scaling ───────────────────────────────────────────────────────────

/// User-configurable force scaling applied after effect computation.
#[derive(Debug, Clone, Copy)]
pub struct ForceScaling {
    /// Global gain applied to all effects (0.0–1.0).
    pub global_gain: f32,
    /// Per-axis gains (index 0 = pitch, 1 = roll). Clamped to 0.0–1.0.
    pub axis_gains: [f32; 2],
}

impl Default for ForceScaling {
    fn default() -> Self {
        Self {
            global_gain: 1.0,
            axis_gains: [1.0, 1.0],
        }
    }
}

impl ForceScaling {
    /// Apply scaling to a raw force value for the given axis.
    pub fn apply(&self, raw_force: f32, axis: usize) -> f32 {
        let axis_gain = if axis < self.axis_gains.len() {
            self.axis_gains[axis]
        } else {
            1.0
        };
        raw_force * self.global_gain.clamp(0.0, 1.0) * axis_gain.clamp(0.0, 1.0)
    }
}

// ─── Watchdog ────────────────────────────────────────────────────────────────

/// Watchdog that zeroes effects when no update arrives within the timeout.
#[derive(Debug, Clone, Copy)]
pub struct EffectWatchdog {
    /// Number of ticks since the last external update call.
    ticks_since_update: u32,
    /// Maximum allowed ticks between updates before effects are killed.
    timeout_ticks: u32,
    /// Whether the watchdog has tripped.
    tripped: bool,
}

impl EffectWatchdog {
    /// Create a watchdog with the given timeout in ticks (e.g. 250 for 1 s at 250 Hz).
    pub fn new(timeout_ticks: u32) -> Self {
        Self {
            ticks_since_update: 0,
            timeout_ticks,
            tripped: false,
        }
    }

    /// Call once per tick. Returns `true` when the watchdog has tripped.
    pub fn tick(&mut self) -> bool {
        self.ticks_since_update = self.ticks_since_update.saturating_add(1);
        if self.ticks_since_update >= self.timeout_ticks {
            self.tripped = true;
        }
        self.tripped
    }

    /// Reset the watchdog (call when an external update arrives).
    pub fn feed(&mut self) {
        self.ticks_since_update = 0;
        self.tripped = false;
    }

    /// Returns `true` if the watchdog has tripped.
    pub fn is_tripped(&self) -> bool {
        self.tripped
    }
}

// ─── Individual effect types ─────────────────────────────────────────────────

/// Constant force — sustained load in one direction.
#[derive(Debug, Clone, Copy)]
pub struct ConstantForceParams {
    /// Force magnitude, −1.0 to +1.0.
    pub magnitude: f32,
}

/// Spring centering effect — force proportional to displacement from center.
#[derive(Debug, Clone, Copy)]
pub struct SpringParams {
    /// Spring stiffness coefficient, 0.0 to 1.0.
    pub coefficient: f32,
    /// Center position, −1.0 to +1.0.
    pub center: f32,
    /// Dead-band half-width around center (0.0 to 1.0).
    pub deadband: f32,
    /// Saturation — maximum output magnitude (0.0 to 1.0).
    pub saturation: f32,
}

impl Default for SpringParams {
    fn default() -> Self {
        Self {
            coefficient: 0.8,
            center: 0.0,
            deadband: 0.0,
            saturation: 1.0,
        }
    }
}

/// Damper effect — force proportional to velocity, opposing motion.
#[derive(Debug, Clone, Copy)]
pub struct DamperParams {
    /// Damping coefficient, 0.0 to 1.0.
    pub coefficient: f32,
}

/// Friction effect — constant force opposing direction of motion.
#[derive(Debug, Clone, Copy)]
pub struct FrictionParams {
    /// Static friction coefficient, 0.0 to 1.0.
    pub coefficient: f32,
}

/// Periodic effect parameters.
#[derive(Debug, Clone, Copy)]
pub struct PeriodicParams {
    /// Waveform shape.
    pub waveform: Waveform,
    /// Frequency in Hz.
    pub frequency_hz: f32,
    /// Amplitude, 0.0 to 1.0.
    pub amplitude: f32,
    /// Phase offset in degrees (0–360).
    pub phase_deg: f32,
    /// DC offset, −1.0 to +1.0.
    pub offset: f32,
}

/// Ramp effect — linear transition between two force levels.
#[derive(Debug, Clone, Copy)]
pub struct RampParams {
    /// Start magnitude, −1.0 to +1.0.
    pub start: f32,
    /// End magnitude, −1.0 to +1.0.
    pub end: f32,
    /// Duration in ticks.
    pub duration_ticks: u32,
}

// ─── Unified effect enum ─────────────────────────────────────────────────────

/// A single FFB effect. Enum-based to stay zero-allocation on the hot path.
#[derive(Debug, Clone, Copy)]
pub enum FfbEffect {
    ConstantForce(ConstantForceParams),
    Spring(SpringParams),
    Damper(DamperParams),
    Friction(FrictionParams),
    Periodic(PeriodicParams),
    Ramp(RampParams),
}

/// Input state fed to effect computation.
#[derive(Debug, Clone, Copy)]
pub struct EffectInput {
    /// Axis position, −1.0 to +1.0.
    pub position: f32,
    /// Axis velocity (position units per second). Sign indicates direction.
    pub velocity: f32,
    /// Elapsed time in seconds since effect started (for periodic / ramp).
    pub elapsed_s: f32,
    /// Current tick index (for ramp).
    pub tick: u32,
}

impl FfbEffect {
    /// Compute the force output for a single effect.
    ///
    /// Returns a value in −1.0…+1.0 (clamped).
    pub fn compute(&self, input: &EffectInput) -> f32 {
        let raw = match self {
            FfbEffect::ConstantForce(p) => p.magnitude,

            FfbEffect::Spring(p) => {
                let displacement = input.position - p.center;
                let abs_disp = displacement.abs();
                if abs_disp <= p.deadband {
                    0.0
                } else {
                    let effective = abs_disp - p.deadband;
                    let force = -p.coefficient * effective * displacement.signum();
                    force.clamp(-p.saturation, p.saturation)
                }
            }

            FfbEffect::Damper(p) => {
                let force = -p.coefficient * input.velocity;
                force.clamp(-1.0, 1.0)
            }

            FfbEffect::Friction(p) => {
                if input.velocity.abs() < 1e-6 {
                    0.0
                } else {
                    let force = -p.coefficient * input.velocity.signum();
                    force.clamp(-1.0, 1.0)
                }
            }

            FfbEffect::Periodic(p) => {
                let phase_rad = p.phase_deg * PI / 180.0;
                let t = 2.0 * PI * p.frequency_hz * input.elapsed_s + phase_rad;

                let wave = match p.waveform {
                    Waveform::Sine => t.sin(),
                    Waveform::Square => {
                        if t.sin() >= 0.0 {
                            1.0
                        } else {
                            -1.0
                        }
                    }
                    Waveform::Triangle => 2.0 * (t / (2.0 * PI)).fract().abs() * 2.0 - 1.0,
                    Waveform::Sawtooth => 2.0 * (t / (2.0 * PI)).fract() - 1.0,
                };

                wave * p.amplitude + p.offset
            }

            FfbEffect::Ramp(p) => {
                if p.duration_ticks == 0 {
                    return p.end.clamp(-1.0, 1.0);
                }
                let progress = (input.tick as f32 / p.duration_ticks as f32).clamp(0.0, 1.0);
                p.start + (p.end - p.start) * progress
            }
        };

        raw.clamp(-1.0, 1.0)
    }
}

// ─── Composite effect ────────────────────────────────────────────────────────

/// A slot inside the composite effect stack. Stack size is fixed to avoid heap.
const MAX_COMPOSITE_EFFECTS: usize = 8;

/// Composite effect — combines up to [`MAX_COMPOSITE_EFFECTS`] effects with
/// individual gain values. All storage is inline (no heap).
#[derive(Debug, Clone, Copy)]
pub struct CompositeEffect {
    effects: [Option<(FfbEffect, f32)>; MAX_COMPOSITE_EFFECTS],
    count: usize,
}

impl Default for CompositeEffect {
    fn default() -> Self {
        Self::new()
    }
}

impl CompositeEffect {
    /// Create an empty composite.
    pub fn new() -> Self {
        Self {
            effects: [None; MAX_COMPOSITE_EFFECTS],
            count: 0,
        }
    }

    /// Add an effect with a gain multiplier. Returns `false` if full.
    pub fn add(&mut self, effect: FfbEffect, gain: f32) -> bool {
        if self.count >= MAX_COMPOSITE_EFFECTS {
            return false;
        }
        self.effects[self.count] = Some((effect, gain));
        self.count += 1;
        true
    }

    /// Remove all effects.
    pub fn clear(&mut self) {
        for slot in &mut self.effects {
            *slot = None;
        }
        self.count = 0;
    }

    /// Number of active effects.
    pub fn len(&self) -> usize {
        self.count
    }

    /// Returns `true` when no effects are loaded.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Compute the summed force of all effects (clamped to −1.0…+1.0).
    pub fn compute(&self, input: &EffectInput) -> f32 {
        let mut total = 0.0_f32;
        for (effect, gain) in self.effects[..self.count].iter().flatten() {
            total += effect.compute(input) * gain;
        }
        total.clamp(-1.0, 1.0)
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn input_at_rest() -> EffectInput {
        EffectInput {
            position: 0.0,
            velocity: 0.0,
            elapsed_s: 0.0,
            tick: 0,
        }
    }

    // ── Constant force ───────────────────────────────────────────────────

    #[test]
    fn constant_force_positive() {
        let effect = FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.6 });
        let f = effect.compute(&input_at_rest());
        assert!((f - 0.6).abs() < 1e-6);
    }

    #[test]
    fn constant_force_negative() {
        let effect = FfbEffect::ConstantForce(ConstantForceParams { magnitude: -0.4 });
        let f = effect.compute(&input_at_rest());
        assert!((f - -0.4).abs() < 1e-6);
    }

    #[test]
    fn constant_force_clamps() {
        let effect = FfbEffect::ConstantForce(ConstantForceParams { magnitude: 1.5 });
        let f = effect.compute(&input_at_rest());
        assert!((f - 1.0).abs() < 1e-6);
    }

    // ── Spring centering ─────────────────────────────────────────────────

    #[test]
    fn spring_at_center_is_zero() {
        let effect = FfbEffect::Spring(SpringParams::default());
        let f = effect.compute(&input_at_rest());
        assert!(f.abs() < 1e-6);
    }

    #[test]
    fn spring_displaced_right_pushes_left() {
        let effect = FfbEffect::Spring(SpringParams {
            coefficient: 1.0,
            center: 0.0,
            deadband: 0.0,
            saturation: 1.0,
        });
        let input = EffectInput {
            position: 0.5,
            ..input_at_rest()
        };
        let f = effect.compute(&input);
        assert!(f < 0.0, "spring should push left, got {f}");
        assert!((f - -0.5).abs() < 1e-6);
    }

    #[test]
    fn spring_displaced_left_pushes_right() {
        let effect = FfbEffect::Spring(SpringParams {
            coefficient: 1.0,
            center: 0.0,
            deadband: 0.0,
            saturation: 1.0,
        });
        let input = EffectInput {
            position: -0.5,
            ..input_at_rest()
        };
        let f = effect.compute(&input);
        assert!(f > 0.0, "spring should push right, got {f}");
        assert!((f - 0.5).abs() < 1e-6);
    }

    #[test]
    fn spring_deadband() {
        let effect = FfbEffect::Spring(SpringParams {
            coefficient: 1.0,
            center: 0.0,
            deadband: 0.1,
            saturation: 1.0,
        });
        // Inside deadband
        let input = EffectInput {
            position: 0.05,
            ..input_at_rest()
        };
        assert!(effect.compute(&input).abs() < 1e-6);

        // Outside deadband
        let input2 = EffectInput {
            position: 0.3,
            ..input_at_rest()
        };
        assert!(effect.compute(&input2) < 0.0);
    }

    #[test]
    fn spring_saturation_caps_output() {
        let effect = FfbEffect::Spring(SpringParams {
            coefficient: 1.0,
            center: 0.0,
            deadband: 0.0,
            saturation: 0.3,
        });
        let input = EffectInput {
            position: 1.0,
            ..input_at_rest()
        };
        let f = effect.compute(&input);
        assert!(
            f.abs() <= 0.3 + 1e-6,
            "should be capped at saturation, got {f}"
        );
    }

    #[test]
    fn spring_custom_center() {
        let effect = FfbEffect::Spring(SpringParams {
            coefficient: 1.0,
            center: 0.5,
            deadband: 0.0,
            saturation: 1.0,
        });
        // At the custom center → zero
        let input = EffectInput {
            position: 0.5,
            ..input_at_rest()
        };
        assert!(effect.compute(&input).abs() < 1e-6);
    }

    // ── Damper ───────────────────────────────────────────────────────────

    #[test]
    fn damper_zero_velocity_is_zero() {
        let effect = FfbEffect::Damper(DamperParams { coefficient: 0.5 });
        assert!(effect.compute(&input_at_rest()).abs() < 1e-6);
    }

    #[test]
    fn damper_opposes_positive_velocity() {
        let effect = FfbEffect::Damper(DamperParams { coefficient: 0.5 });
        let input = EffectInput {
            velocity: 0.8,
            ..input_at_rest()
        };
        let f = effect.compute(&input);
        assert!(f < 0.0, "damper should oppose positive velocity, got {f}");
        assert!((f - -0.4).abs() < 1e-6);
    }

    #[test]
    fn damper_opposes_negative_velocity() {
        let effect = FfbEffect::Damper(DamperParams { coefficient: 0.5 });
        let input = EffectInput {
            velocity: -0.8,
            ..input_at_rest()
        };
        let f = effect.compute(&input);
        assert!(f > 0.0, "damper should oppose negative velocity, got {f}");
        assert!((f - 0.4).abs() < 1e-6);
    }

    #[test]
    fn damper_proportional_to_speed() {
        let effect = FfbEffect::Damper(DamperParams { coefficient: 1.0 });
        let slow = EffectInput {
            velocity: 0.2,
            ..input_at_rest()
        };
        let fast = EffectInput {
            velocity: 0.8,
            ..input_at_rest()
        };
        assert!(
            effect.compute(&fast).abs() > effect.compute(&slow).abs(),
            "faster movement should produce stronger damping"
        );
    }

    // ── Friction ─────────────────────────────────────────────────────────

    #[test]
    fn friction_zero_velocity_is_zero() {
        let effect = FfbEffect::Friction(FrictionParams { coefficient: 0.5 });
        assert!(effect.compute(&input_at_rest()).abs() < 1e-6);
    }

    #[test]
    fn friction_constant_opposing_force() {
        let effect = FfbEffect::Friction(FrictionParams { coefficient: 0.5 });
        let slow = EffectInput {
            velocity: 0.1,
            ..input_at_rest()
        };
        let fast = EffectInput {
            velocity: 0.9,
            ..input_at_rest()
        };
        // Both should produce the same magnitude (friction is constant)
        let f_slow = effect.compute(&slow);
        let f_fast = effect.compute(&fast);
        assert!(
            (f_slow.abs() - f_fast.abs()).abs() < 1e-6,
            "friction should be constant, slow={f_slow}, fast={f_fast}"
        );
    }

    #[test]
    fn friction_opposes_motion_direction() {
        let effect = FfbEffect::Friction(FrictionParams { coefficient: 0.7 });
        let right = EffectInput {
            velocity: 0.5,
            ..input_at_rest()
        };
        let left = EffectInput {
            velocity: -0.5,
            ..input_at_rest()
        };
        assert!(effect.compute(&right) < 0.0);
        assert!(effect.compute(&left) > 0.0);
    }

    // ── Periodic effects ─────────────────────────────────────────────────

    fn periodic(waveform: Waveform) -> FfbEffect {
        FfbEffect::Periodic(PeriodicParams {
            waveform,
            frequency_hz: 1.0,
            amplitude: 1.0,
            phase_deg: 0.0,
            offset: 0.0,
        })
    }

    #[test]
    fn sine_at_quarter_period_is_one() {
        let effect = periodic(Waveform::Sine);
        let input = EffectInput {
            elapsed_s: 0.25, // quarter period at 1 Hz
            ..input_at_rest()
        };
        let f = effect.compute(&input);
        assert!((f - 1.0).abs() < 1e-4, "sine(π/2) should be ~1.0, got {f}");
    }

    #[test]
    fn sine_at_three_quarter_period_is_negative_one() {
        let effect = periodic(Waveform::Sine);
        let input = EffectInput {
            elapsed_s: 0.75,
            ..input_at_rest()
        };
        let f = effect.compute(&input);
        assert!(
            (f - -1.0).abs() < 1e-4,
            "sine(3π/2) should be ~-1.0, got {f}"
        );
    }

    #[test]
    fn square_wave_values() {
        let effect = periodic(Waveform::Square);
        // First half (positive)
        let input_pos = EffectInput {
            elapsed_s: 0.1,
            ..input_at_rest()
        };
        assert_eq!(effect.compute(&input_pos), 1.0);
        // Second half (negative)
        let input_neg = EffectInput {
            elapsed_s: 0.6,
            ..input_at_rest()
        };
        assert_eq!(effect.compute(&input_neg), -1.0);
    }

    #[test]
    fn triangle_wave_peaks() {
        let effect = periodic(Waveform::Triangle);
        // Sample at several points and verify bounded
        for i in 0..100 {
            let input = EffectInput {
                elapsed_s: i as f32 * 0.01,
                ..input_at_rest()
            };
            let f = effect.compute(&input);
            assert!(
                (-1.0..=1.0).contains(&f),
                "triangle out of bounds at t={}: {f}",
                i as f32 * 0.01
            );
        }
    }

    #[test]
    fn sawtooth_wave_ramps() {
        let effect = periodic(Waveform::Sawtooth);
        // At start of period should be near -1.0, at end near +1.0
        let early = EffectInput {
            elapsed_s: 0.01,
            ..input_at_rest()
        };
        let late = EffectInput {
            elapsed_s: 0.99,
            ..input_at_rest()
        };
        let f_early = effect.compute(&early);
        let f_late = effect.compute(&late);
        assert!(
            f_late > f_early,
            "sawtooth should ramp up: early={f_early}, late={f_late}"
        );
    }

    #[test]
    fn periodic_amplitude_scaling() {
        let effect = FfbEffect::Periodic(PeriodicParams {
            waveform: Waveform::Sine,
            frequency_hz: 1.0,
            amplitude: 0.5,
            phase_deg: 0.0,
            offset: 0.0,
        });
        let input = EffectInput {
            elapsed_s: 0.25,
            ..input_at_rest()
        };
        let f = effect.compute(&input);
        assert!((f - 0.5).abs() < 1e-4, "half-amplitude sine peak: {f}");
    }

    #[test]
    fn periodic_offset() {
        let effect = FfbEffect::Periodic(PeriodicParams {
            waveform: Waveform::Sine,
            frequency_hz: 1.0,
            amplitude: 0.5,
            phase_deg: 0.0,
            offset: 0.3,
        });
        let input = EffectInput {
            elapsed_s: 0.0, // sin(0) = 0
            ..input_at_rest()
        };
        let f = effect.compute(&input);
        assert!((f - 0.3).abs() < 1e-4, "offset at zero-crossing: {f}");
    }

    #[test]
    fn periodic_phase_shift() {
        let base = FfbEffect::Periodic(PeriodicParams {
            waveform: Waveform::Sine,
            frequency_hz: 1.0,
            amplitude: 1.0,
            phase_deg: 0.0,
            offset: 0.0,
        });
        let shifted = FfbEffect::Periodic(PeriodicParams {
            waveform: Waveform::Sine,
            frequency_hz: 1.0,
            amplitude: 1.0,
            phase_deg: 90.0,
            offset: 0.0,
        });
        let input = EffectInput {
            elapsed_s: 0.0,
            ..input_at_rest()
        };
        let f_base = base.compute(&input);
        let f_shifted = shifted.compute(&input);
        // sin(0) ≈ 0, sin(π/2) ≈ 1
        assert!(f_base.abs() < 1e-4);
        assert!((f_shifted - 1.0).abs() < 1e-4);
    }

    // ── Ramp ─────────────────────────────────────────────────────────────

    #[test]
    fn ramp_at_start() {
        let effect = FfbEffect::Ramp(RampParams {
            start: 0.0,
            end: 1.0,
            duration_ticks: 100,
        });
        let input = EffectInput {
            tick: 0,
            ..input_at_rest()
        };
        let f = effect.compute(&input);
        assert!((f - 0.0).abs() < 1e-6);
    }

    #[test]
    fn ramp_at_midpoint() {
        let effect = FfbEffect::Ramp(RampParams {
            start: 0.0,
            end: 1.0,
            duration_ticks: 100,
        });
        let input = EffectInput {
            tick: 50,
            ..input_at_rest()
        };
        let f = effect.compute(&input);
        assert!((f - 0.5).abs() < 1e-6);
    }

    #[test]
    fn ramp_at_end() {
        let effect = FfbEffect::Ramp(RampParams {
            start: 0.0,
            end: 1.0,
            duration_ticks: 100,
        });
        let input = EffectInput {
            tick: 100,
            ..input_at_rest()
        };
        let f = effect.compute(&input);
        assert!((f - 1.0).abs() < 1e-6);
    }

    #[test]
    fn ramp_past_end_clamps() {
        let effect = FfbEffect::Ramp(RampParams {
            start: 0.0,
            end: 1.0,
            duration_ticks: 100,
        });
        let input = EffectInput {
            tick: 200,
            ..input_at_rest()
        };
        let f = effect.compute(&input);
        assert!((f - 1.0).abs() < 1e-6);
    }

    #[test]
    fn ramp_zero_duration_returns_end() {
        let effect = FfbEffect::Ramp(RampParams {
            start: 0.0,
            end: 0.7,
            duration_ticks: 0,
        });
        let f = effect.compute(&input_at_rest());
        assert!((f - 0.7).abs() < 1e-6);
    }

    #[test]
    fn ramp_negative_direction() {
        let effect = FfbEffect::Ramp(RampParams {
            start: 0.8,
            end: -0.3,
            duration_ticks: 100,
        });
        let input = EffectInput {
            tick: 50,
            ..input_at_rest()
        };
        let f = effect.compute(&input);
        let expected = 0.8 + (-0.3 - 0.8) * 0.5; // 0.25
        assert!(
            (f - expected).abs() < 1e-6,
            "mid-ramp: expected {expected}, got {f}"
        );
    }

    // ── Composite effect ─────────────────────────────────────────────────

    #[test]
    fn composite_empty_is_zero() {
        let comp = CompositeEffect::new();
        assert!(comp.is_empty());
        let f = comp.compute(&input_at_rest());
        assert!(f.abs() < 1e-6);
    }

    #[test]
    fn composite_single_effect() {
        let mut comp = CompositeEffect::new();
        comp.add(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.5 }),
            1.0,
        );
        assert_eq!(comp.len(), 1);
        let f = comp.compute(&input_at_rest());
        assert!((f - 0.5).abs() < 1e-6);
    }

    #[test]
    fn composite_sums_effects() {
        let mut comp = CompositeEffect::new();
        comp.add(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.3 }),
            1.0,
        );
        comp.add(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.2 }),
            1.0,
        );
        let f = comp.compute(&input_at_rest());
        assert!((f - 0.5).abs() < 1e-6);
    }

    #[test]
    fn composite_gain_scaling() {
        let mut comp = CompositeEffect::new();
        comp.add(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 1.0 }),
            0.5,
        );
        let f = comp.compute(&input_at_rest());
        assert!((f - 0.5).abs() < 1e-6);
    }

    #[test]
    fn composite_clamps_sum() {
        let mut comp = CompositeEffect::new();
        comp.add(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.8 }),
            1.0,
        );
        comp.add(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.8 }),
            1.0,
        );
        let f = comp.compute(&input_at_rest());
        assert!((f - 1.0).abs() < 1e-6, "sum should clamp to 1.0, got {f}");
    }

    #[test]
    fn composite_max_capacity() {
        let mut comp = CompositeEffect::new();
        for _ in 0..MAX_COMPOSITE_EFFECTS {
            assert!(comp.add(
                FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.01 }),
                1.0,
            ));
        }
        assert!(!comp.add(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.01 }),
            1.0,
        ));
        assert_eq!(comp.len(), MAX_COMPOSITE_EFFECTS);
    }

    #[test]
    fn composite_clear() {
        let mut comp = CompositeEffect::new();
        comp.add(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.5 }),
            1.0,
        );
        comp.clear();
        assert!(comp.is_empty());
        assert!(comp.compute(&input_at_rest()).abs() < 1e-6);
    }

    #[test]
    fn composite_spring_plus_damper() {
        let mut comp = CompositeEffect::new();
        comp.add(
            FfbEffect::Spring(SpringParams {
                coefficient: 1.0,
                center: 0.0,
                deadband: 0.0,
                saturation: 1.0,
            }),
            1.0,
        );
        comp.add(FfbEffect::Damper(DamperParams { coefficient: 0.5 }), 1.0);

        // Moving right at position 0.4 with velocity 0.2
        let input = EffectInput {
            position: 0.4,
            velocity: 0.2,
            elapsed_s: 0.0,
            tick: 0,
        };
        let f = comp.compute(&input);
        // Spring: -1.0 * 0.4 = -0.4
        // Damper: -0.5 * 0.2 = -0.1
        // Total: -0.5
        assert!(
            (f - -0.5).abs() < 1e-6,
            "spring+damper: expected -0.5, got {f}"
        );
    }

    #[test]
    fn composite_weather_turbulence_simulation() {
        // Simulate turbulence: periodic shake + constant crosswind force
        let mut comp = CompositeEffect::new();
        comp.add(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.2 }),
            1.0, // crosswind
        );
        comp.add(
            FfbEffect::Periodic(PeriodicParams {
                waveform: Waveform::Sine,
                frequency_hz: 5.0,
                amplitude: 0.3,
                phase_deg: 0.0,
                offset: 0.0,
            }),
            1.0, // turbulence shake
        );

        let input = EffectInput {
            elapsed_s: 0.05, // t = 0.05s, sin(2π*5*0.05) = sin(π/2) = 1.0
            ..input_at_rest()
        };
        let f = comp.compute(&input);
        // Constant: 0.2, Sine: 0.3 * 1.0 = 0.3, total = 0.5
        assert!((f - 0.5).abs() < 1e-3, "turbulence composite: {f}");
    }

    // ── Force scaling ────────────────────────────────────────────────────

    #[test]
    fn force_scaling_default_is_unity() {
        let scaling = ForceScaling::default();
        assert!((scaling.apply(0.75, 0) - 0.75).abs() < 1e-6);
    }

    #[test]
    fn force_scaling_global_gain() {
        let scaling = ForceScaling {
            global_gain: 0.5,
            axis_gains: [1.0, 1.0],
        };
        assert!((scaling.apply(1.0, 0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn force_scaling_per_axis() {
        let scaling = ForceScaling {
            global_gain: 1.0,
            axis_gains: [0.8, 0.6],
        };
        assert!((scaling.apply(1.0, 0) - 0.8).abs() < 1e-6);
        assert!((scaling.apply(1.0, 1) - 0.6).abs() < 1e-6);
    }

    #[test]
    fn force_scaling_combined() {
        let scaling = ForceScaling {
            global_gain: 0.5,
            axis_gains: [0.8, 1.0],
        };
        assert!((scaling.apply(1.0, 0) - 0.4).abs() < 1e-6);
    }

    // ── Watchdog ─────────────────────────────────────────────────────────

    #[test]
    fn watchdog_does_not_trip_before_timeout() {
        let mut wd = EffectWatchdog::new(10);
        for _ in 0..9 {
            assert!(!wd.tick());
        }
        assert!(!wd.is_tripped());
    }

    #[test]
    fn watchdog_trips_at_timeout() {
        let mut wd = EffectWatchdog::new(10);
        for _ in 0..10 {
            wd.tick();
        }
        assert!(wd.is_tripped());
    }

    #[test]
    fn watchdog_feed_resets() {
        let mut wd = EffectWatchdog::new(10);
        for _ in 0..8 {
            wd.tick();
        }
        wd.feed();
        for _ in 0..8 {
            assert!(!wd.tick());
        }
        assert!(!wd.is_tripped());
    }

    #[test]
    fn watchdog_stays_tripped_until_fed() {
        let mut wd = EffectWatchdog::new(5);
        for _ in 0..10 {
            wd.tick();
        }
        assert!(wd.is_tripped());
        wd.feed();
        assert!(!wd.is_tripped());
    }

    // ── Safety limit enforcement via envelope ────────────────────────────

    #[test]
    fn all_effects_output_bounded() {
        let effects: Vec<FfbEffect> = vec![
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 5.0 }),
            FfbEffect::Spring(SpringParams {
                coefficient: 5.0,
                center: 0.0,
                deadband: 0.0,
                saturation: 5.0,
            }),
            FfbEffect::Damper(DamperParams { coefficient: 5.0 }),
            FfbEffect::Friction(FrictionParams { coefficient: 5.0 }),
            FfbEffect::Periodic(PeriodicParams {
                waveform: Waveform::Sine,
                frequency_hz: 100.0,
                amplitude: 5.0,
                phase_deg: 0.0,
                offset: 3.0,
            }),
            FfbEffect::Ramp(RampParams {
                start: -3.0,
                end: 3.0,
                duration_ticks: 10,
            }),
        ];

        for effect in &effects {
            for i in 0..100 {
                let input = EffectInput {
                    position: (i as f32 / 50.0) - 1.0,
                    velocity: (i as f32 / 25.0) - 2.0,
                    elapsed_s: i as f32 * 0.01,
                    tick: i,
                };
                let f = effect.compute(&input);
                assert!(
                    (-1.0..=1.0).contains(&f),
                    "effect {:?} out of bounds at step {i}: {f}",
                    std::mem::discriminant(effect)
                );
            }
        }
    }

    // ── Weather-to-FFB composition ───────────────────────────────────────

    #[test]
    fn wind_shear_composite_effect() {
        // Simulate wind shear: rapid ramp + periodic
        let mut comp = CompositeEffect::new();
        comp.add(
            FfbEffect::Ramp(RampParams {
                start: 0.0,
                end: 0.6,
                duration_ticks: 50,
            }),
            1.0,
        );
        comp.add(
            FfbEffect::Periodic(PeriodicParams {
                waveform: Waveform::Sine,
                frequency_hz: 8.0,
                amplitude: 0.2,
                phase_deg: 0.0,
                offset: 0.0,
            }),
            1.0,
        );

        // At midpoint (tick 25), ramp = 0.3, sine depends on elapsed_s
        let input = EffectInput {
            tick: 25,
            elapsed_s: 0.1, // 0.1s at 250Hz = 25 ticks
            ..input_at_rest()
        };
        let f = comp.compute(&input);
        // Ramp: 0.3, Sine: 0.2 * sin(2π*8*0.1) = 0.2 * sin(1.6π) ≈ 0.2 * -0.951 ≈ -0.190
        // Total ≈ 0.11
        assert!(
            (-1.0..=1.0).contains(&f),
            "wind shear composite in bounds: {f}"
        );
    }

    // ── Emergency stop ───────────────────────────────────────────────────

    #[test]
    fn emergency_stop_via_watchdog_and_scaling() {
        let mut wd = EffectWatchdog::new(5);
        let mut scaling = ForceScaling::default();

        // Simulate normal operation
        let effect = FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.8 });
        let input = input_at_rest();

        let f = scaling.apply(effect.compute(&input), 0);
        assert!((f - 0.8).abs() < 1e-6);

        // Watchdog trips → zero gain to simulate emergency stop
        for _ in 0..5 {
            wd.tick();
        }
        assert!(wd.is_tripped());

        if wd.is_tripped() {
            scaling.global_gain = 0.0;
        }
        let f = scaling.apply(effect.compute(&input), 0);
        assert!(f.abs() < 1e-6, "emergency stop should zero output, got {f}");
    }
}
