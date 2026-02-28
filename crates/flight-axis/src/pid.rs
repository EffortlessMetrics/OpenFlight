// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! PID controller for stable axis output.
//!
//! Suitable for heading hold, altitude hold, and similar stability augmentation.
//!
//! # Formula
//!
//! ```text
//! error  = setpoint - process_variable
//! P      = Kp * error
//! I     += Ki * error * dt  (clamped to ±i_limit)
//! D      = Kd * (error - prev_error) / dt  (EMA-filtered)
//! output = clamp(P + I + D_filtered, -1.0, 1.0)
//! ```

use std::collections::HashMap;

/// Configuration for a [`PidController`].
#[derive(Debug, Clone, PartialEq)]
pub struct PidConfig {
    /// Proportional gain.
    pub kp: f32,
    /// Integral gain.
    pub ki: f32,
    /// Derivative gain.
    pub kd: f32,
    /// Integral windup limit — integral is clamped to `[-i_limit, i_limit]`.
    pub i_limit: f32,
    /// EMA alpha for derivative smoothing. `1.0` = no filtering, `0.0` = frozen.
    pub d_filter_alpha: f32,
}

impl Default for PidConfig {
    fn default() -> Self {
        Self {
            kp: 1.0,
            ki: 0.0,
            kd: 0.0,
            i_limit: 0.5,
            d_filter_alpha: 0.5,
        }
    }
}

/// PID controller for stable axis output.
///
/// Suitable for heading hold, altitude hold, and similar stability augmentation.
#[derive(Debug, Clone)]
pub struct PidController {
    /// Controller gains and limits.
    pub config: PidConfig,
    /// Current integral accumulator.
    integral: f32,
    /// Previous error, used for derivative calculation.
    prev_error: f32,
    /// EMA-filtered derivative term.
    d_filtered: f32,
    /// `false` until the first [`PidController::update`] call.
    initialized: bool,
}

impl PidController {
    /// Creates a new `PidController` with the given configuration.
    pub fn new(config: PidConfig) -> Self {
        Self {
            config,
            integral: 0.0,
            prev_error: 0.0,
            d_filtered: 0.0,
            initialized: false,
        }
    }

    /// Runs one PID update step.
    ///
    /// - `setpoint` — desired value
    /// - `process_variable` — current measured value
    /// - `dt` — elapsed time in seconds since last call (must be > 0)
    ///
    /// On the first call the derivative term is zero (no previous error is available).
    /// Returns the clamped controller output in `[-1.0, 1.0]`.
    #[inline]
    pub fn update(&mut self, setpoint: f32, process_variable: f32, dt: f32) -> f32 {
        let error = setpoint - process_variable;

        // Proportional
        let p = self.config.kp * error;

        // Integral with anti-windup clamp
        self.integral = (self.integral + self.config.ki * error * dt)
            .clamp(-self.config.i_limit, self.config.i_limit);

        // Derivative (zero on first tick to avoid derivative kick)
        let d_raw = if self.initialized {
            self.config.kd * (error - self.prev_error) / dt
        } else {
            0.0
        };

        // EMA filter on derivative
        let alpha = self.config.d_filter_alpha;
        self.d_filtered = alpha * d_raw + (1.0 - alpha) * self.d_filtered;

        self.prev_error = error;
        self.initialized = true;

        (p + self.integral + self.d_filtered).clamp(-1.0, 1.0)
    }

    /// Resets all internal state (integral, prev_error, filtered derivative).
    #[inline]
    pub fn reset(&mut self) {
        self.integral = 0.0;
        self.prev_error = 0.0;
        self.d_filtered = 0.0;
        self.initialized = false;
    }

    /// Returns the current integral accumulator value.
    #[inline]
    pub fn integral(&self) -> f32 {
        self.integral
    }

    /// Sets the integral accumulator directly, e.g. for bump-less transfer.
    #[inline]
    pub fn set_integral(&mut self, value: f32) {
        self.integral = value.clamp(-self.config.i_limit, self.config.i_limit);
    }
}

/// Manages a collection of named [`PidController`]s.
///
/// Not intended for use on the RT hot path due to `HashMap` heap allocation.
#[derive(Debug, Clone, Default)]
pub struct PidBank {
    pids: HashMap<String, PidController>,
}

impl PidBank {
    /// Creates a new, empty `PidBank`.
    pub fn new() -> Self {
        Self {
            pids: HashMap::new(),
        }
    }

    /// Inserts a named PID controller, replacing any existing entry.
    pub fn insert(&mut self, name: &str, pid: PidController) {
        self.pids.insert(name.to_string(), pid);
    }

    /// Runs one update step for the named controller.
    ///
    /// Returns `None` if no controller with that name exists.
    #[inline]
    pub fn update(&mut self, name: &str, setpoint: f32, pv: f32, dt: f32) -> Option<f32> {
        self.pids
            .get_mut(name)
            .map(|pid| pid.update(setpoint, pv, dt))
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const DT: f32 = 0.004; // 250 Hz

    fn p_only(kp: f32) -> PidController {
        PidController::new(PidConfig {
            kp,
            ki: 0.0,
            kd: 0.0,
            i_limit: 0.5,
            d_filter_alpha: 0.5,
        })
    }

    // ── Unit tests ────────────────────────────────────────────────────────────

    #[test]
    fn test_pid_proportional_only() {
        let mut pid = p_only(2.0);
        let out = pid.update(0.5, 0.0, DT);
        // P = 2.0 * 0.5 = 1.0, clamped to 1.0
        assert!((out - 1.0).abs() < 1e-6, "out={out}");
    }

    #[test]
    fn test_pid_zero_error() {
        let mut pid = p_only(1.0);
        let out = pid.update(0.5, 0.5, DT);
        assert!(out.abs() < 1e-6, "out={out}");
    }

    #[test]
    fn test_pid_integral_accumulates() {
        let mut pid = PidController::new(PidConfig {
            kp: 0.0,
            ki: 1.0,
            kd: 0.0,
            i_limit: 10.0,
            d_filter_alpha: 0.5,
        });
        for _ in 0..10 {
            pid.update(1.0, 0.0, DT);
        }
        // After 10 steps: I = 1.0 * 1.0 * 0.004 * 10 = 0.04
        assert!(pid.integral() > 0.0, "integral should have grown");
        assert!(
            (pid.integral() - 0.04).abs() < 1e-5,
            "integral={}",
            pid.integral()
        );
    }

    #[test]
    fn test_pid_integral_windup_limited() {
        let mut pid = PidController::new(PidConfig {
            kp: 0.0,
            ki: 1.0,
            kd: 0.0,
            i_limit: 0.5,
            d_filter_alpha: 0.5,
        });
        for _ in 0..1000 {
            pid.update(1.0, 0.0, DT);
        }
        assert!(
            (pid.integral() - 0.5).abs() < 1e-6,
            "integral={}, expected 0.5",
            pid.integral()
        );
    }

    #[test]
    fn test_pid_derivative_on_step() {
        let mut pid = PidController::new(PidConfig {
            kp: 0.0,
            ki: 0.0,
            kd: 1.0,
            i_limit: 0.5,
            d_filter_alpha: 1.0, // no filter for predictability
        });
        // First tick: D = 0 (not initialized)
        pid.update(0.0, 0.0, DT);
        // Large step: pv jumps from 0 to 0.5 → error changes by -0.5
        let out = pid.update(0.0, 0.5, DT);
        // D = 1.0 * (−0.5 − 0.0) / 0.004 = -125, clamped to -1.0
        assert!(out < 0.0, "D-term should be negative for positive pv step");
    }

    #[test]
    fn test_pid_derivative_zero_first_tick() {
        let mut pid = PidController::new(PidConfig {
            kp: 0.0,
            ki: 0.0,
            kd: 100.0,
            i_limit: 0.5,
            d_filter_alpha: 1.0,
        });
        // On first update there is no prev_error, so D must be 0
        let out = pid.update(1.0, 0.0, DT);
        assert!(out.abs() < 1e-6, "D should be 0 on first tick, got {out}");
    }

    #[test]
    fn test_pid_reset_clears_state() {
        let mut pid = PidController::new(PidConfig {
            kp: 1.0,
            ki: 1.0,
            kd: 0.0,
            i_limit: 0.5,
            d_filter_alpha: 0.5,
        });
        for _ in 0..50 {
            pid.update(1.0, 0.0, DT);
        }
        pid.reset();
        assert!(
            pid.integral().abs() < 1e-6,
            "integral should be 0 after reset"
        );
        // After reset, next output should be only P (no carry-over integral)
        let out = pid.update(0.5, 0.0, DT);
        // P = 1.0 * 0.5 = 0.5; I grows by ki*error*dt = 1.0*0.5*0.004 = 0.002; D=0
        assert!(
            (out - 0.502).abs() < 1e-4,
            "expected ~0.502 after reset, got {out}"
        );
    }

    #[test]
    fn test_pid_output_clamped_to_1() {
        let mut pid = p_only(100.0);
        let out = pid.update(1.0, 0.0, DT);
        assert!((out - 1.0).abs() < 1e-6, "expected 1.0 clamp, got {out}");
    }

    #[test]
    fn test_pid_output_clamped_to_neg1() {
        let mut pid = p_only(100.0);
        let out = pid.update(-1.0, 0.0, DT);
        assert!((out + 1.0).abs() < 1e-6, "expected -1.0 clamp, got {out}");
    }

    #[test]
    fn test_pid_setpoint_above_pv() {
        let mut pid = p_only(1.0);
        let out = pid.update(0.8, 0.2, DT);
        assert!(out > 0.0, "positive error should give positive output");
    }

    #[test]
    fn test_pid_setpoint_below_pv() {
        let mut pid = p_only(1.0);
        let out = pid.update(0.2, 0.8, DT);
        assert!(out < 0.0, "negative error should give negative output");
    }

    #[test]
    fn test_pid_converges_step_response() {
        // P-only controller: error should reduce each step as output is applied
        let mut pid = p_only(0.5);
        let mut pv = 0.0f32;
        let setpoint = 1.0f32;
        let mut last_error = (setpoint - pv).abs();

        for _ in 0..20 {
            let out = pid.update(setpoint, pv, DT);
            // Simulate: apply output as increment to pv
            pv += out * DT * 50.0; // arbitrary plant gain
            let error = (setpoint - pv).abs();
            assert!(
                error <= last_error + 1e-4,
                "error should not grow: prev={last_error}, now={error}"
            );
            last_error = error;
        }
        assert!(last_error < 1.0, "error should have reduced from 1.0");
    }

    #[test]
    fn test_pid_bank_update_known_axis() {
        let mut bank = PidBank::new();
        bank.insert("heading", p_only(1.0));
        let out = bank.update("heading", 0.5, 0.0, DT);
        assert!(out.is_some(), "known axis should return Some");
        assert!(out.unwrap() > 0.0, "positive error → positive output");
    }

    #[test]
    fn test_pid_bank_update_unknown_axis() {
        let mut bank = PidBank::new();
        let out = bank.update("altitude", 100.0, 50.0, DT);
        assert!(out.is_none(), "unknown axis should return None");
    }

    #[test]
    fn test_pid_bank_reset_not_needed() {
        // Verify bank insert + update + controller reset via direct access
        let mut bank = PidBank::new();
        bank.insert("pitch", p_only(1.0));
        bank.update("pitch", 1.0, 0.0, DT);
        // Re-insert a fresh controller to simulate reset
        bank.insert("pitch", p_only(1.0));
        let out = bank.update("pitch", 0.5, 0.0, DT);
        assert!(out.is_some());
        // Fresh controller: only P term, no accumulated integral
        assert!(
            (out.unwrap() - 0.5).abs() < 1e-5,
            "expected ~0.5, got {:?}",
            out
        );
    }

    #[test]
    fn test_pid_set_integral_clamped() {
        let mut pid = PidController::new(PidConfig {
            kp: 0.0,
            ki: 0.0,
            kd: 0.0,
            i_limit: 0.5,
            d_filter_alpha: 0.5,
        });
        pid.set_integral(10.0);
        assert!(
            (pid.integral() - 0.5).abs() < 1e-6,
            "set_integral should clamp to i_limit"
        );
        pid.set_integral(-10.0);
        assert!(
            (pid.integral() + 0.5).abs() < 1e-6,
            "set_integral should clamp negative"
        );
    }

    // ── Proptests ─────────────────────────────────────────────────────────────

    proptest! {
        /// Output must always lie in [-1.0, 1.0] regardless of inputs.
        #[test]
        fn proptest_output_always_in_bounds(
            kp in 0.0f32..10.0,
            ki in 0.0f32..10.0,
            kd in 0.0f32..10.0,
            setpoint in -1.0f32..=1.0,
            pv in -1.0f32..=1.0,
        ) {
            let mut pid = PidController::new(PidConfig {
                kp, ki, kd,
                i_limit: 0.5,
                d_filter_alpha: 0.5,
            });
            for _ in 0..10 {
                let out = pid.update(setpoint, pv, DT);
                prop_assert!(
                    (-1.0..=1.0).contains(&out),
                    "out={out} not in [-1.0, 1.0]"
                );
            }
        }

        /// When setpoint == pv, error is zero and output should be ~0 (within float precision).
        #[test]
        fn proptest_zero_error_zero_output(
            kp in 0.0f32..10.0,
            value in -1.0f32..=1.0,
        ) {
            let mut pid = PidController::new(PidConfig {
                kp,
                ki: 0.0,
                kd: 0.0,
                i_limit: 0.5,
                d_filter_alpha: 0.5,
            });
            let out = pid.update(value, value, DT);
            prop_assert!(
                out.abs() < 1e-6,
                "zero error should give zero output, got {out}"
            );
        }

        /// Integral accumulator must never exceed i_limit.
        #[test]
        fn proptest_integral_never_exceeds_limit(
            ki in 0.0f32..10.0,
            error in -1.0f32..=1.0,
            i_limit in 0.01f32..1.0,
            steps in 1usize..200,
        ) {
            let mut pid = PidController::new(PidConfig {
                kp: 0.0,
                ki,
                kd: 0.0,
                i_limit,
                d_filter_alpha: 0.5,
            });
            for _ in 0..steps {
                pid.update(error, 0.0, DT);
            }
            prop_assert!(
                pid.integral().abs() <= i_limit + 1e-5,
                "integral={} exceeded i_limit={}",
                pid.integral(),
                i_limit
            );
        }
    }
}
