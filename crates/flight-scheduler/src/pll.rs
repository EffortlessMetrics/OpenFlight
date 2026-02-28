// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Phase-Locked Loop for scheduler timing correction (ADR-005).
//!
//! Provides two implementations:
//! - [`Pll`] — the original integral-only controller used by [`crate::Scheduler`]
//! - [`PhaseLockLoop`] — enhanced PI controller with lock detection and
//!   hysteresis, suitable for advanced jitter-correction scenarios
//!
//! Both are **zero-allocation** on the hot path per ADR-004.

// ---------------------------------------------------------------------------
// Original Pll (integral-only, used by Scheduler)
// ---------------------------------------------------------------------------

/// Phase-Locked Loop for timing correction
#[derive(Debug, Clone)]
pub struct Pll {
    /// PLL gain (typically 0.001 for 0.1%/s correction)
    gain: f64,
    /// Nominal period in nanoseconds
    nominal_period_ns: f64,
    /// Current corrected period
    corrected_period_ns: f64,
    /// Accumulated phase error
    phase_error: f64,
}

impl Pll {
    /// Create new PLL
    pub fn new(gain: f64, nominal_period_ns: f64) -> Self {
        Self {
            gain,
            nominal_period_ns,
            corrected_period_ns: nominal_period_ns,
            phase_error: 0.0,
        }
    }

    /// Update PLL with timing error and return corrected period
    pub fn update(&mut self, error_ns: f64) -> f64 {
        // Accumulate phase error
        self.phase_error += error_ns;

        // Apply proportional correction to period
        let correction = -self.gain * self.phase_error;
        self.corrected_period_ns = self.nominal_period_ns + correction;

        // Clamp correction to reasonable bounds (±1%)
        let max_correction = self.nominal_period_ns * 0.01;
        self.corrected_period_ns = self
            .corrected_period_ns
            .max(self.nominal_period_ns - max_correction)
            .min(self.nominal_period_ns + max_correction);

        self.corrected_period_ns
    }

    /// Get current phase error
    pub fn phase_error(&self) -> f64 {
        self.phase_error
    }

    /// Get current period correction
    pub fn period_correction(&self) -> f64 {
        self.corrected_period_ns - self.nominal_period_ns
    }

    /// Reset PLL state
    pub fn reset(&mut self) {
        self.phase_error = 0.0;
        self.corrected_period_ns = self.nominal_period_ns;
    }
}

// ---------------------------------------------------------------------------
// PhaseLockLoop — enhanced PI controller with lock detection
// ---------------------------------------------------------------------------

/// Enhanced phase-locked loop with proportional-integral control and lock
/// detection (ADR-005).
///
/// Compared to [`Pll`], this struct adds:
/// - A proportional gain (`kp`) for fast transient response
/// - Integral anti-windup (clamped accumulator)
/// - Lock / unlock detection with configurable thresholds and hysteresis
/// - Frequency and lock-state queries
///
/// # Zero-allocation guarantee
///
/// All fields are stack-allocated scalars. No heap allocation occurs on the
/// hot path ([`tick`](Self::tick)).
#[derive(Debug, Clone)]
pub struct PhaseLockLoop {
    // PI gains
    kp: f64,
    ki: f64,

    // NCO state
    nominal_period_ns: f64,
    corrected_period_ns: f64,

    // PI accumulator
    integral: f64,
    current_error_ns: f64,

    // Lock detection
    lock_threshold_ns: f64,
    unlock_threshold_ns: f64,
    is_locked: bool,
    lock_counter: u32,
    unlock_counter: u32,
    lock_hysteresis: u32,

    tick_count: u64,
}

impl PhaseLockLoop {
    /// Create a new PLL with the given PI gains and nominal period.
    ///
    /// Recommended defaults for 250 Hz:
    /// - `kp = 0.1` (fast proportional response)
    /// - `ki = 0.001` (slow integral — 0.1 %/s per ADR-005)
    /// - `nominal_period_ns = 4_000_000.0`
    pub fn new(kp: f64, ki: f64, nominal_period_ns: f64) -> Self {
        Self {
            kp,
            ki,
            nominal_period_ns,
            corrected_period_ns: nominal_period_ns,
            integral: 0.0,
            current_error_ns: 0.0,
            lock_threshold_ns: 50_000.0,    // 50 µs
            unlock_threshold_ns: 200_000.0, // 200 µs
            is_locked: false,
            lock_counter: 0,
            unlock_counter: 0,
            lock_hysteresis: 10,
            tick_count: 0,
        }
    }

    /// Configure lock detection parameters (builder-style).
    ///
    /// - `lock_threshold_ns` — phase error must stay below this to enter lock
    /// - `unlock_threshold_ns` — phase error must exceed this to leave lock
    /// - `hysteresis` — number of consecutive qualifying ticks before transition
    ///
    /// `unlock_threshold_ns` should be greater than `lock_threshold_ns` to
    /// prevent chattering.
    pub fn with_lock_detection(
        mut self,
        lock_threshold_ns: f64,
        unlock_threshold_ns: f64,
        hysteresis: u32,
    ) -> Self {
        self.lock_threshold_ns = lock_threshold_ns;
        self.unlock_threshold_ns = unlock_threshold_ns;
        self.lock_hysteresis = hysteresis;
        self
    }

    /// Called each iteration with the current phase error in nanoseconds
    /// (positive = late, negative = early).
    ///
    /// Returns the corrected period in nanoseconds for scheduling the next
    /// tick.
    #[inline]
    pub fn tick(&mut self, phase_error_ns: f64) -> f64 {
        self.tick_count += 1;
        self.current_error_ns = phase_error_ns;

        // --- PI controller ---
        self.integral += phase_error_ns;

        // Anti-windup: clamp the integral so it cannot drive the correction
        // beyond the ±1 % output clamp.
        let max_integral = if self.ki.abs() > f64::EPSILON {
            (self.nominal_period_ns * 0.01) / self.ki
        } else {
            f64::MAX
        };
        self.integral = self.integral.clamp(-max_integral, max_integral);

        let p_term = self.kp * phase_error_ns;
        let i_term = self.ki * self.integral;
        let correction = p_term + i_term;

        self.corrected_period_ns = self.nominal_period_ns - correction;

        // Clamp output to ±1 % of nominal period
        let max_delta = self.nominal_period_ns * 0.01;
        self.corrected_period_ns =
            self.corrected_period_ns
                .clamp(self.nominal_period_ns - max_delta, self.nominal_period_ns + max_delta);

        // --- Lock detection ---
        self.update_lock_state(phase_error_ns);

        self.corrected_period_ns
    }

    /// Current phase error in nanoseconds (from the most recent [`tick`](Self::tick)).
    pub fn phase_error(&self) -> f64 {
        self.current_error_ns
    }

    /// Effective output frequency in Hz.
    pub fn frequency(&self) -> f64 {
        if self.corrected_period_ns <= 0.0 {
            return 0.0;
        }
        1_000_000_000.0 / self.corrected_period_ns
    }

    /// `true` when the PLL considers itself locked (phase error has been
    /// below [`lock_threshold_ns`](Self::with_lock_detection) for at least
    /// `hysteresis` consecutive ticks).
    pub fn locked(&self) -> bool {
        self.is_locked
    }

    /// Number of ticks processed since creation or last [`reset`](Self::reset).
    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }

    /// Nominal (target) period in nanoseconds.
    pub fn nominal_period_ns(&self) -> f64 {
        self.nominal_period_ns
    }

    /// Current corrected period in nanoseconds.
    pub fn corrected_period_ns(&self) -> f64 {
        self.corrected_period_ns
    }

    /// Current integral accumulator value.
    pub fn integral(&self) -> f64 {
        self.integral
    }

    /// Reset all internal state to cold-start defaults.
    pub fn reset(&mut self) {
        self.corrected_period_ns = self.nominal_period_ns;
        self.integral = 0.0;
        self.current_error_ns = 0.0;
        self.is_locked = false;
        self.lock_counter = 0;
        self.unlock_counter = 0;
        self.tick_count = 0;
    }

    // -- internal helpers -----------------------------------------------

    #[inline]
    fn update_lock_state(&mut self, error_ns: f64) {
        let abs_error = error_ns.abs();

        if self.is_locked {
            // Currently locked — check for unlock condition
            if abs_error > self.unlock_threshold_ns {
                self.unlock_counter += 1;
                self.lock_counter = 0;
                if self.unlock_counter >= self.lock_hysteresis {
                    self.is_locked = false;
                    self.unlock_counter = 0;
                }
            } else {
                self.unlock_counter = 0;
            }
        } else {
            // Currently unlocked — check for lock condition
            if abs_error < self.lock_threshold_ns {
                self.lock_counter += 1;
                self.unlock_counter = 0;
                if self.lock_counter >= self.lock_hysteresis {
                    self.is_locked = true;
                    self.lock_counter = 0;
                }
            } else {
                self.lock_counter = 0;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Original Pll tests (unchanged) ---------------------------------

    #[test]
    fn test_pll_correction() {
        let mut pll = Pll::new(0.001, 4_000_000.0); // 250Hz = 4ms period

        // Simulate consistent late timing
        for _ in 0..100 {
            let _corrected = pll.update(1000.0); // 1μs late each time
        }

        // Should have negative correction (shorter period)
        assert!(pll.period_correction() < 0.0);

        // Should be bounded
        assert!(pll.period_correction().abs() < 40_000.0); // <1% of 4ms
    }

    #[test]
    fn test_pll_bounds() {
        let mut pll = Pll::new(0.1, 4_000_000.0); // High gain for testing

        // Large error should be clamped
        let _corrected = pll.update(1_000_000.0); // 1ms error

        // Should be clamped to ±1%
        assert!(pll.period_correction().abs() <= 40_000.0);
    }

    // -- PhaseLockLoop tests --------------------------------------------

    #[test]
    fn pll_pi_single_positive_error_shortens_period() {
        let nominal = 4_000_000.0;
        let mut pll = PhaseLockLoop::new(0.1, 0.001, nominal);
        let corrected = pll.tick(1_000.0); // 1 µs late
        assert!(
            corrected < nominal,
            "positive error should shorten period, got {corrected}"
        );
    }

    #[test]
    fn pll_pi_single_negative_error_lengthens_period() {
        let nominal = 4_000_000.0;
        let mut pll = PhaseLockLoop::new(0.1, 0.001, nominal);
        let corrected = pll.tick(-1_000.0); // 1 µs early
        assert!(
            corrected > nominal,
            "negative error should lengthen period, got {corrected}"
        );
    }

    #[test]
    fn pll_pi_zero_error_holds_nominal() {
        let nominal = 4_000_000.0;
        let mut pll = PhaseLockLoop::new(0.1, 0.001, nominal);
        for _ in 0..100 {
            let corrected = pll.tick(0.0);
            assert!(
                (corrected - nominal).abs() < 1e-6,
                "zero error should hold nominal"
            );
        }
    }

    #[test]
    fn pll_pi_convergence_from_cold_start() {
        let nominal = 4_000_000.0; // 250 Hz
        let mut pll = PhaseLockLoop::new(0.1, 0.001, nominal);

        // Simulate a system that starts 100 µs late each tick.
        // The PLL should gradually reduce the corrected period to compensate,
        // which in a closed-loop system would drive the error toward zero.
        let initial_error = 100_000.0; // 100 µs
        let mut simulated_error = initial_error;

        for _ in 0..500 {
            let corrected = pll.tick(simulated_error);
            // Simulate how the shorter period would reduce the error next tick.
            // correction = nominal - corrected (positive when period shortened).
            let period_delta = nominal - corrected;
            simulated_error -= period_delta;
        }

        // After 500 iterations the error should be substantially reduced.
        assert!(
            simulated_error.abs() < initial_error * 0.1,
            "PLL should converge: residual error {simulated_error} ns"
        );
    }

    #[test]
    fn pll_pi_lock_detection_from_cold_start() {
        let mut pll = PhaseLockLoop::new(0.1, 0.001, 4_000_000.0)
            .with_lock_detection(50_000.0, 200_000.0, 10);

        assert!(!pll.locked(), "should start unlocked");

        // Feed errors well below lock threshold for > hysteresis ticks
        for _ in 0..20 {
            pll.tick(1_000.0); // 1 µs — well below 50 µs threshold
        }

        assert!(pll.locked(), "should be locked after sustained low error");
    }

    #[test]
    fn pll_pi_lock_maintained_within_threshold() {
        let mut pll = PhaseLockLoop::new(0.1, 0.001, 4_000_000.0)
            .with_lock_detection(50_000.0, 200_000.0, 5);

        // Lock first
        for _ in 0..10 {
            pll.tick(100.0);
        }
        assert!(pll.locked());

        // Errors between lock and unlock thresholds should NOT unlock
        for _ in 0..20 {
            pll.tick(100_000.0); // 100 µs — above lock, below unlock
        }
        assert!(
            pll.locked(),
            "should stay locked when error is between thresholds"
        );
    }

    #[test]
    fn pll_pi_unlock_on_sustained_large_error() {
        let mut pll = PhaseLockLoop::new(0.1, 0.001, 4_000_000.0)
            .with_lock_detection(50_000.0, 200_000.0, 5);

        // Lock
        for _ in 0..10 {
            pll.tick(100.0);
        }
        assert!(pll.locked());

        // Sustained error above unlock threshold
        for _ in 0..10 {
            pll.tick(300_000.0); // 300 µs — above 200 µs unlock threshold
        }
        assert!(!pll.locked(), "should unlock after sustained large error");
    }

    #[test]
    fn pll_pi_phase_error_recovery_after_disturbance() {
        let nominal = 4_000_000.0;
        let mut pll = PhaseLockLoop::new(0.1, 0.001, nominal)
            .with_lock_detection(50_000.0, 200_000.0, 10);

        // Phase 1: converge and lock
        let mut error = 0.0;
        for _ in 0..100 {
            let corrected = pll.tick(error);
            error -= nominal - corrected;
        }
        // Should be locked by now
        for _ in 0..20 {
            pll.tick(error);
        }
        assert!(pll.locked(), "should be locked before disturbance");

        // Phase 2: inject disturbance (500 µs spike)
        error = 500_000.0;
        for _ in 0..5 {
            let corrected = pll.tick(error);
            error -= nominal - corrected;
        }

        // Phase 3: recovery — feed back through the loop
        for _ in 0..500 {
            let corrected = pll.tick(error);
            error -= nominal - corrected;
        }

        // Should have recovered to small error
        assert!(
            error.abs() < 10_000.0,
            "should recover from disturbance, residual: {error} ns"
        );
    }

    #[test]
    fn pll_pi_frequency_reports_correct_value() {
        let nominal = 4_000_000.0; // 250 Hz
        let mut pll = PhaseLockLoop::new(0.1, 0.001, nominal);

        // At nominal period, frequency should be ~250 Hz
        pll.tick(0.0);
        let freq = pll.frequency();
        assert!(
            (freq - 250.0).abs() < 0.1,
            "frequency should be ~250 Hz, got {freq}"
        );
    }

    #[test]
    fn pll_pi_output_clamped_to_one_percent() {
        let nominal = 4_000_000.0;
        let mut pll = PhaseLockLoop::new(10.0, 1.0, nominal); // very aggressive gains

        // Huge error
        let corrected = pll.tick(1_000_000.0);
        let delta = (corrected - nominal).abs();
        let max_delta = nominal * 0.01;

        assert!(
            delta <= max_delta + 1e-6,
            "correction {delta} ns exceeds 1% ({max_delta} ns)"
        );
    }

    #[test]
    fn pll_pi_anti_windup_limits_integral() {
        let nominal = 4_000_000.0;
        let ki = 0.001;
        let mut pll = PhaseLockLoop::new(0.0, ki, nominal); // integral-only

        // Feed large positive error for many ticks
        for _ in 0..10_000 {
            pll.tick(100_000.0);
        }

        // Integral should be bounded
        let max_integral = (nominal * 0.01) / ki;
        assert!(
            pll.integral().abs() <= max_integral + 1e-6,
            "integral {} should be bounded by {max_integral}",
            pll.integral()
        );
    }

    #[test]
    fn pll_pi_reset_returns_to_initial_state() {
        let nominal = 4_000_000.0;
        let mut pll = PhaseLockLoop::new(0.1, 0.001, nominal);

        for _ in 0..100 {
            pll.tick(5_000.0);
        }

        pll.reset();

        assert_eq!(pll.tick_count(), 0);
        assert!(!pll.locked());
        assert!((pll.phase_error() - 0.0).abs() < f64::EPSILON);
        assert!((pll.integral() - 0.0).abs() < f64::EPSILON);
        assert!((pll.corrected_period_ns() - nominal).abs() < f64::EPSILON);
    }

    #[test]
    fn pll_pi_tick_count_increments() {
        let mut pll = PhaseLockLoop::new(0.1, 0.001, 4_000_000.0);
        assert_eq!(pll.tick_count(), 0);

        for i in 1..=50 {
            pll.tick(0.0);
            assert_eq!(pll.tick_count(), i);
        }
    }
}
