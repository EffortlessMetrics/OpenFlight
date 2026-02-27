// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Ramped effect transitions for force feedback (REQ-702).
//!
//! Provides linear magnitude ramping between values over a configurable
//! number of ticks. Zero-allocation and RT-safe — no heap allocations
//! on the hot path.

/// Configuration for effect ramping behaviour.
#[derive(Debug, Clone, Copy)]
pub struct RampConfig {
    /// Number of ticks over which to ramp. 0 = immediate transition.
    pub duration_ticks: u32,
}

impl Default for RampConfig {
    fn default() -> Self {
        Self { duration_ticks: 0 }
    }
}

/// Linear magnitude ramp between two values over a fixed number of ticks.
///
/// All operations are zero-allocation and safe for the RT hot-path.
#[derive(Debug, Clone, Copy)]
pub struct EffectRamp {
    start_magnitude: f32,
    target_magnitude: f32,
    current_tick: u32,
    duration: u32,
}

impl EffectRamp {
    /// Create a new ramp from `start` to `target` over `duration_ticks`.
    ///
    /// If `duration_ticks` is 0 the ramp completes immediately.
    pub fn new(start: f32, target: f32, duration_ticks: u32) -> Self {
        Self {
            start_magnitude: start,
            target_magnitude: target,
            current_tick: 0,
            duration: duration_ticks,
        }
    }

    /// Advance by one tick and return the current ramped magnitude.
    pub fn tick(&mut self) -> f32 {
        if self.duration == 0 {
            return self.target_magnitude;
        }

        if self.current_tick >= self.duration {
            return self.target_magnitude;
        }

        let t = self.current_tick as f32 / self.duration as f32;
        let value =
            self.start_magnitude + (self.target_magnitude - self.start_magnitude) * t;

        self.current_tick += 1;
        value
    }

    /// Returns `true` when the ramp has reached (or passed) its target.
    pub fn is_complete(&self) -> bool {
        self.duration == 0 || self.current_tick >= self.duration
    }

    /// Interrupt the current ramp and start a new one toward `new_target`
    /// from the current interpolated position.
    pub fn interrupt(&mut self, new_target: f32, new_duration: u32) {
        let current_value = if self.duration == 0 {
            self.target_magnitude
        } else {
            let t = self.current_tick.min(self.duration) as f32 / self.duration as f32;
            self.start_magnitude + (self.target_magnitude - self.start_magnitude) * t
        };

        self.start_magnitude = current_value;
        self.target_magnitude = new_target;
        self.current_tick = 0;
        self.duration = new_duration;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_immediate_ramp() {
        let mut ramp = EffectRamp::new(0.0, 1.0, 0);
        assert!(ramp.is_complete());
        let val = ramp.tick();
        assert!((val - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_linear_ramp_up() {
        let mut ramp = EffectRamp::new(0.0, 1.0, 10);
        // Tick to midpoint (tick 5 reads t=5/10=0.5)
        for _ in 0..5 {
            ramp.tick();
        }
        let mid = ramp.tick(); // tick 5 → t=5/10=0.5
        assert!(
            (mid - 0.5).abs() < 0.05,
            "midpoint should be ~0.5, got {mid}"
        );
    }

    #[test]
    fn test_linear_ramp_down() {
        let mut ramp = EffectRamp::new(1.0, 0.0, 10);
        for _ in 0..5 {
            ramp.tick();
        }
        let mid = ramp.tick();
        assert!(
            (mid - 0.5).abs() < 0.05,
            "midpoint should be ~0.5, got {mid}"
        );
    }

    #[test]
    fn test_ramp_complete() {
        let mut ramp = EffectRamp::new(0.0, 1.0, 5);
        assert!(!ramp.is_complete());
        for _ in 0..5 {
            ramp.tick();
        }
        assert!(ramp.is_complete());
    }

    #[test]
    fn test_ramp_interrupt() {
        let mut ramp = EffectRamp::new(0.0, 1.0, 10);
        // Advance 5 ticks → value read at tick 5 is 0.5
        for _ in 0..5 {
            ramp.tick();
        }
        // Interrupt: new ramp from current (~0.5) to 0.0 over 10 ticks
        ramp.interrupt(0.0, 10);
        assert!(!ramp.is_complete());
        let first = ramp.tick(); // t=0/10 → start_magnitude ≈ 0.5
        assert!(
            (first - 0.5).abs() < 0.05,
            "interrupt should start from current value ~0.5, got {first}"
        );
    }

    #[test]
    fn test_ramp_past_complete() {
        let mut ramp = EffectRamp::new(0.0, 1.0, 3);
        for _ in 0..10 {
            ramp.tick();
        }
        let val = ramp.tick();
        assert!(
            (val - 1.0).abs() < f32::EPSILON,
            "past-complete should stay at target, got {val}"
        );
        assert!(ramp.is_complete());
    }
}
