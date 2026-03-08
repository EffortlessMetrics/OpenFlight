// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! FFB effect library — trait-based force effect primitives for synthesis depth
//!
//! Provides a [`SynthEffect`] trait and five implementations:
//! - [`ConstantForce`] — steady force in one direction
//! - [`SpringForce`] — position-proportional restoring force
//! - [`DamperForce`] — velocity-proportional resistance
//! - [`FrictionForce`] — constant resistance to movement
//! - [`PeriodicForce`] — sine/square/triangle/sawtooth waveforms
//!
//! All computations are zero-allocation, deterministic, and NaN/Inf-safe.
//! Force output is normalised to −1.0…+1.0.

use std::f64::consts::PI;

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Sanitise an input value: replace NaN/Inf with 0.0.
#[inline]
fn sanitize(v: f64) -> f64 {
    if v.is_finite() { v } else { 0.0 }
}

// ─── Trait ───────────────────────────────────────────────────────────────────

/// Common interface for all synthesis-depth force effects.
///
/// Implementations must be pure, deterministic, and zero-allocation.
pub trait SynthEffect {
    /// Compute force output given axis state.
    ///
    /// * `position` — axis position (−1.0 to +1.0)
    /// * `velocity` — axis velocity (units/s, sign = direction)
    /// * `dt_s` — timestep in seconds
    ///
    /// Returns force in −1.0…+1.0 (clamped).
    fn compute(&mut self, position: f64, velocity: f64, dt_s: f64) -> f64;
}

// ─── Waveform shape ──────────────────────────────────────────────────────────

/// Waveform shape for periodic effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SynthWaveform {
    Sine,
    Square,
    Triangle,
    Sawtooth,
}

// ─── ConstantForce ───────────────────────────────────────────────────────────

/// Steady force in one direction.
#[derive(Debug, Clone, Copy)]
pub struct ConstantForce {
    /// Force magnitude, −1.0 to +1.0.
    pub magnitude: f64,
}

impl ConstantForce {
    pub fn new(magnitude: f64) -> Self {
        Self {
            magnitude: magnitude.clamp(-1.0, 1.0),
        }
    }
}

impl SynthEffect for ConstantForce {
    #[inline]
    fn compute(&mut self, position: f64, velocity: f64, _dt_s: f64) -> f64 {
        let _ = (position, velocity);
        if !self.magnitude.is_finite() {
            return 0.0;
        }
        self.magnitude.clamp(-1.0, 1.0)
    }
}

// ─── SpringForce ─────────────────────────────────────────────────────────────

/// Position-proportional restoring force with configurable center, gain,
/// deadband, and saturation.
#[derive(Debug, Clone, Copy)]
pub struct SpringForce {
    /// Center position, −1.0 to +1.0.
    pub center: f64,
    /// Gain (stiffness), 0.0 to 1.0.
    pub gain: f64,
    /// Dead-band half-width around center.
    pub deadband: f64,
    /// Maximum output magnitude.
    pub saturation: f64,
}

impl SpringForce {
    pub fn new(center: f64, gain: f64, deadband: f64, saturation: f64) -> Self {
        Self {
            center: center.clamp(-1.0, 1.0),
            gain: gain.clamp(0.0, 1.0),
            deadband: deadband.clamp(0.0, 1.0),
            saturation: saturation.clamp(0.0, 1.0),
        }
    }
}

impl SynthEffect for SpringForce {
    #[inline]
    fn compute(&mut self, position: f64, _velocity: f64, _dt_s: f64) -> f64 {
        if !self.center.is_finite()
            || !self.gain.is_finite()
            || !self.deadband.is_finite()
            || !self.saturation.is_finite()
        {
            return 0.0;
        }
        let pos = sanitize(position);
        let displacement = pos - self.center;
        let abs_disp = displacement.abs();
        if abs_disp <= self.deadband {
            return 0.0;
        }
        let effective = abs_disp - self.deadband;
        let force = -self.gain * effective * displacement.signum();
        force.clamp(-self.saturation, self.saturation)
    }
}

// ─── DamperForce ─────────────────────────────────────────────────────────────

/// Velocity-proportional resistance opposing motion.
#[derive(Debug, Clone, Copy)]
pub struct DamperForce {
    /// Damping gain, 0.0 to 1.0.
    pub gain: f64,
    /// Maximum output magnitude.
    pub saturation: f64,
}

impl DamperForce {
    pub fn new(gain: f64, saturation: f64) -> Self {
        Self {
            gain: gain.clamp(0.0, 1.0),
            saturation: saturation.clamp(0.0, 1.0),
        }
    }
}

impl SynthEffect for DamperForce {
    #[inline]
    fn compute(&mut self, _position: f64, velocity: f64, _dt_s: f64) -> f64 {
        if !self.gain.is_finite() || !self.saturation.is_finite() {
            return 0.0;
        }
        let vel = sanitize(velocity);
        let force = -self.gain * vel;
        force.clamp(-self.saturation, self.saturation)
    }
}

// ─── FrictionForce ───────────────────────────────────────────────────────────

/// Constant resistance opposing direction of movement.
#[derive(Debug, Clone, Copy)]
pub struct FrictionForce {
    /// Friction coefficient, 0.0 to 1.0.
    pub coefficient: f64,
    /// Maximum output magnitude.
    pub saturation: f64,
}

impl FrictionForce {
    pub fn new(coefficient: f64, saturation: f64) -> Self {
        Self {
            coefficient: coefficient.clamp(0.0, 1.0),
            saturation: saturation.clamp(0.0, 1.0),
        }
    }
}

impl SynthEffect for FrictionForce {
    #[inline]
    fn compute(&mut self, _position: f64, velocity: f64, _dt_s: f64) -> f64 {
        if !self.coefficient.is_finite() || !self.saturation.is_finite() {
            return 0.0;
        }
        let vel = sanitize(velocity);
        if vel.abs() < 1e-9 {
            return 0.0;
        }
        let force = -self.coefficient * vel.signum();
        force.clamp(-self.saturation, self.saturation)
    }
}

// ─── PeriodicForce ───────────────────────────────────────────────────────────

/// Periodic waveform force (sine, square, triangle, sawtooth).
#[derive(Debug, Clone, Copy)]
pub struct PeriodicForce {
    /// Waveform shape.
    pub waveform: SynthWaveform,
    /// Frequency in Hz.
    pub frequency: f64,
    /// Amplitude, 0.0 to 1.0.
    pub amplitude: f64,
    /// Phase offset in radians.
    pub phase: f64,
    /// DC offset, −1.0 to +1.0.
    pub offset: f64,
    /// Accumulated phase (ticks forward each compute call via dt_s).
    accumulated_phase: f64,
}

impl PeriodicForce {
    pub fn new(
        waveform: SynthWaveform,
        frequency: f64,
        amplitude: f64,
        phase: f64,
        offset: f64,
    ) -> Self {
        Self {
            waveform,
            frequency: frequency.max(0.0),
            amplitude: amplitude.clamp(0.0, 1.0),
            phase,
            offset: offset.clamp(-1.0, 1.0),
            accumulated_phase: 0.0,
        }
    }

    /// Reset the internal phase accumulator.
    pub fn reset_phase(&mut self) {
        self.accumulated_phase = 0.0;
    }
}

impl SynthEffect for PeriodicForce {
    #[inline]
    fn compute(&mut self, _position: f64, _velocity: f64, dt_s: f64) -> f64 {
        if !self.frequency.is_finite()
            || !self.amplitude.is_finite()
            || !self.phase.is_finite()
            || !self.offset.is_finite()
        {
            return 0.0;
        }
        let dt = sanitize(dt_s).max(0.0);
        let t = self.accumulated_phase + 2.0 * PI * self.frequency * dt + self.phase;

        // Advance the phase accumulator so periodic effects evolve each tick.
        self.advance(dt);

        let wave = match self.waveform {
            SynthWaveform::Sine => t.sin(),
            SynthWaveform::Square => {
                if t.sin() >= 0.0 {
                    1.0
                } else {
                    -1.0
                }
            }
            SynthWaveform::Triangle => {
                // Map to [0,1) fraction within cycle, then triangle wave.
                let frac = (t / (2.0 * PI)).fract().abs();
                if frac < 0.5 {
                    4.0 * frac - 1.0
                } else {
                    3.0 - 4.0 * frac
                }
            }
            SynthWaveform::Sawtooth => {
                let frac = (t / (2.0 * PI)).fract();
                2.0 * frac - 1.0
            }
        };

        (wave * self.amplitude + self.offset).clamp(-1.0, 1.0)
    }
}

/// Advance the internal phase accumulator (manual override).
///
/// Normally `compute` auto-advances. Use this only for manual phase control.
impl PeriodicForce {
    #[inline]
    pub fn advance(&mut self, dt_s: f64) {
        let dt = sanitize(dt_s).max(0.0);
        self.accumulated_phase += 2.0 * PI * self.frequency * dt;
        // Keep phase bounded to avoid precision loss over long runs.
        if self.accumulated_phase > 2.0 * PI * 1_000.0 {
            self.accumulated_phase %= 2.0 * PI;
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-9;

    // ── ConstantForce ────────────────────────────────────────────────────

    #[test]
    fn constant_positive() {
        let mut e = ConstantForce::new(0.5);
        assert!((e.compute(0.0, 0.0, 0.004) - 0.5).abs() < EPS);
    }

    #[test]
    fn constant_negative() {
        let mut e = ConstantForce::new(-0.7);
        assert!((e.compute(0.0, 0.0, 0.004) - -0.7).abs() < EPS);
    }

    #[test]
    fn constant_clamps_high() {
        let mut e = ConstantForce::new(2.0);
        assert!((e.compute(0.0, 0.0, 0.004) - 1.0).abs() < EPS);
    }

    #[test]
    fn constant_clamps_low() {
        let mut e = ConstantForce::new(-5.0);
        assert!((e.compute(0.0, 0.0, 0.004) - -1.0).abs() < EPS);
    }

    #[test]
    fn constant_nan_input() {
        let mut e = ConstantForce::new(0.3);
        assert!((e.compute(f64::NAN, f64::NAN, f64::NAN) - 0.3).abs() < EPS);
    }

    #[test]
    fn constant_inf_input() {
        let mut e = ConstantForce::new(0.3);
        assert!((e.compute(f64::INFINITY, f64::NEG_INFINITY, 0.0) - 0.3).abs() < EPS);
    }

    // ── SpringForce ──────────────────────────────────────────────────────

    #[test]
    fn spring_at_center_zero() {
        let mut e = SpringForce::new(0.0, 0.8, 0.0, 1.0);
        assert!(e.compute(0.0, 0.0, 0.004).abs() < EPS);
    }

    #[test]
    fn spring_displaced_right() {
        let mut e = SpringForce::new(0.0, 1.0, 0.0, 1.0);
        let f = e.compute(0.5, 0.0, 0.004);
        assert!(f < 0.0, "should push left");
        assert!((f - -0.5).abs() < EPS);
    }

    #[test]
    fn spring_displaced_left() {
        let mut e = SpringForce::new(0.0, 1.0, 0.0, 1.0);
        let f = e.compute(-0.5, 0.0, 0.004);
        assert!(f > 0.0, "should push right");
        assert!((f - 0.5).abs() < EPS);
    }

    #[test]
    fn spring_deadband() {
        let mut e = SpringForce::new(0.0, 1.0, 0.1, 1.0);
        assert!(e.compute(0.05, 0.0, 0.004).abs() < EPS);
        assert!(e.compute(0.3, 0.0, 0.004) < 0.0);
    }

    #[test]
    fn spring_saturation() {
        let mut e = SpringForce::new(0.0, 1.0, 0.0, 0.3);
        let f = e.compute(1.0, 0.0, 0.004);
        assert!((f - -0.3).abs() < EPS);
    }

    #[test]
    fn spring_custom_center() {
        let mut e = SpringForce::new(0.5, 1.0, 0.0, 1.0);
        assert!(e.compute(0.5, 0.0, 0.004).abs() < EPS);
        assert!(e.compute(0.8, 0.0, 0.004) < 0.0);
    }

    #[test]
    fn spring_nan_position() {
        let mut e = SpringForce::new(0.0, 1.0, 0.0, 1.0);
        assert!(e.compute(f64::NAN, 0.0, 0.004).abs() < EPS);
    }

    // ── DamperForce ──────────────────────────────────────────────────────

    #[test]
    fn damper_zero_velocity() {
        let mut e = DamperForce::new(0.8, 1.0);
        assert!(e.compute(0.0, 0.0, 0.004).abs() < EPS);
    }

    #[test]
    fn damper_positive_velocity() {
        let mut e = DamperForce::new(1.0, 1.0);
        let f = e.compute(0.0, 0.5, 0.004);
        assert!(f < 0.0, "should oppose motion");
        assert!((f - -0.5).abs() < EPS);
    }

    #[test]
    fn damper_negative_velocity() {
        let mut e = DamperForce::new(1.0, 1.0);
        let f = e.compute(0.0, -0.5, 0.004);
        assert!(f > 0.0, "should oppose motion");
        assert!((f - 0.5).abs() < EPS);
    }

    #[test]
    fn damper_saturation() {
        let mut e = DamperForce::new(1.0, 0.4);
        let f = e.compute(0.0, 1.0, 0.004);
        assert!((f - -0.4).abs() < EPS);
    }

    #[test]
    fn damper_nan_velocity() {
        let mut e = DamperForce::new(1.0, 1.0);
        assert!(e.compute(0.0, f64::NAN, 0.004).abs() < EPS);
    }

    // ── FrictionForce ────────────────────────────────────────────────────

    #[test]
    fn friction_stationary() {
        let mut e = FrictionForce::new(0.5, 1.0);
        assert!(e.compute(0.0, 0.0, 0.004).abs() < EPS);
    }

    #[test]
    fn friction_moving_positive() {
        let mut e = FrictionForce::new(0.5, 1.0);
        let f = e.compute(0.0, 1.0, 0.004);
        assert!((f - -0.5).abs() < EPS);
    }

    #[test]
    fn friction_moving_negative() {
        let mut e = FrictionForce::new(0.5, 1.0);
        let f = e.compute(0.0, -1.0, 0.004);
        assert!((f - 0.5).abs() < EPS);
    }

    #[test]
    fn friction_saturation() {
        let mut e = FrictionForce::new(0.8, 0.3);
        let f = e.compute(0.0, 1.0, 0.004);
        assert!((f - -0.3).abs() < EPS);
    }

    #[test]
    fn friction_nan_velocity() {
        let mut e = FrictionForce::new(0.5, 1.0);
        assert!(e.compute(0.0, f64::NAN, 0.004).abs() < EPS);
    }

    // ── PeriodicForce ────────────────────────────────────────────────────

    #[test]
    fn periodic_sine_zero_phase() {
        let mut e = PeriodicForce::new(SynthWaveform::Sine, 1.0, 1.0, 0.0, 0.0);
        // At dt=0 with zero accumulated phase, sin(0)=0
        assert!(e.compute(0.0, 0.0, 0.0).abs() < EPS);
    }

    #[test]
    fn periodic_sine_quarter_cycle() {
        let mut e = PeriodicForce::new(SynthWaveform::Sine, 1.0, 1.0, 0.0, 0.0);
        // dt = 0.25 → phase = π/2, sin(π/2)=1.0
        let f = e.compute(0.0, 0.0, 0.25);
        assert!((f - 1.0).abs() < 1e-6);
    }

    #[test]
    fn periodic_square_positive() {
        let mut e = PeriodicForce::new(SynthWaveform::Square, 1.0, 1.0, 0.0, 0.0);
        // dt = 0.1 → phase = 0.2π, sin(0.2π)>0 → +1
        let f = e.compute(0.0, 0.0, 0.1);
        assert!((f - 1.0).abs() < EPS);
    }

    #[test]
    fn periodic_square_negative() {
        let mut e = PeriodicForce::new(SynthWaveform::Square, 1.0, 1.0, 0.0, 0.0);
        // dt = 0.75 → phase = 1.5π, sin(1.5π)<0 → -1
        let f = e.compute(0.0, 0.0, 0.75);
        assert!((f - -1.0).abs() < EPS);
    }

    #[test]
    fn periodic_with_offset() {
        let mut e = PeriodicForce::new(SynthWaveform::Sine, 1.0, 0.5, 0.0, 0.3);
        let f = e.compute(0.0, 0.0, 0.0);
        // sin(0)*0.5 + 0.3 = 0.3
        assert!((f - 0.3).abs() < 1e-6);
    }

    #[test]
    fn periodic_clamps_output() {
        // amplitude + offset > 1.0
        let mut e = PeriodicForce::new(SynthWaveform::Sine, 1.0, 1.0, 0.0, 0.5);
        // At quarter cycle: 1.0 * 1.0 + 0.5 = 1.5 → clamped to 1.0
        let f = e.compute(0.0, 0.0, 0.25);
        assert!((f - 1.0).abs() < EPS);
    }

    #[test]
    fn periodic_nan_dt() {
        let mut e = PeriodicForce::new(SynthWaveform::Sine, 1.0, 1.0, 0.0, 0.0);
        // NaN dt → sanitised to 0.0, sin(0)=0
        let f = e.compute(0.0, 0.0, f64::NAN);
        assert!(f.is_finite());
    }

    #[test]
    fn periodic_advance_accumulates() {
        let mut e = PeriodicForce::new(SynthWaveform::Sine, 1.0, 1.0, 0.0, 0.0);
        // Advance by 0.25s → accumulated = π/2
        e.advance(0.25);
        // Now compute with dt=0 should give sin(π/2)=1.0
        let f = e.compute(0.0, 0.0, 0.0);
        assert!((f - 1.0).abs() < 1e-6);
    }

    #[test]
    fn periodic_triangle_midpoints() {
        let mut e = PeriodicForce::new(SynthWaveform::Triangle, 1.0, 1.0, 0.0, 0.0);
        // At dt=0.25 → phase = π/2, frac = 0.25 → 4*0.25 - 1 = 0.0
        let f = e.compute(0.0, 0.0, 0.25);
        assert!(f.abs() < 0.01);
    }

    #[test]
    fn periodic_sawtooth_midpoint() {
        let mut e = PeriodicForce::new(SynthWaveform::Sawtooth, 1.0, 1.0, 0.0, 0.0);
        // At dt=0.5 → phase = π, frac = 0.5, 2*0.5 - 1 = 0.0
        let f = e.compute(0.0, 0.0, 0.5);
        assert!(f.abs() < 0.01);
    }

    // ── Non-finite parameter safety ──────────────────────────────────────

    #[test]
    fn spring_nonfinite_params_return_zero() {
        let mut e = SpringForce::new(0.0, 1.0, 0.0, 1.0);
        e.center = f64::NAN;
        assert!(e.compute(0.5, 0.0, 0.004).abs() < EPS);

        let mut e2 = SpringForce::new(0.0, 1.0, 0.0, 1.0);
        e2.gain = f64::INFINITY;
        assert!(e2.compute(0.5, 0.0, 0.004).abs() < EPS);
    }

    #[test]
    fn damper_nonfinite_params_return_zero() {
        let mut e = DamperForce::new(1.0, 1.0);
        e.gain = f64::NAN;
        assert!(e.compute(0.0, 0.5, 0.004).abs() < EPS);

        let mut e2 = DamperForce::new(1.0, 1.0);
        e2.saturation = f64::NEG_INFINITY;
        assert!(e2.compute(0.0, 0.5, 0.004).abs() < EPS);
    }

    #[test]
    fn friction_nonfinite_params_return_zero() {
        let mut e = FrictionForce::new(0.5, 1.0);
        e.coefficient = f64::NAN;
        assert!(e.compute(0.0, 1.0, 0.004).abs() < EPS);
    }

    #[test]
    fn periodic_nonfinite_params_return_zero() {
        let mut e = PeriodicForce::new(SynthWaveform::Sine, 1.0, 1.0, 0.0, 0.0);
        e.frequency = f64::NAN;
        assert!(e.compute(0.0, 0.0, 0.25).abs() < EPS);

        let mut e2 = PeriodicForce::new(SynthWaveform::Sine, 1.0, 1.0, 0.0, 0.0);
        e2.amplitude = f64::INFINITY;
        assert!(e2.compute(0.0, 0.0, 0.25).abs() < EPS);
    }
}
