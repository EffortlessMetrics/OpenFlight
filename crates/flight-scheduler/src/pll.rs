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
        self.corrected_period_ns = self.corrected_period_ns.clamp(
            self.nominal_period_ns - max_delta,
            self.nominal_period_ns + max_delta,
        );

        // --- Lock detection ---
        self.update_lock_state(phase_error_ns);

        self.corrected_period_ns
    }

    /// Tick the PLL and return a [`PllTickResult`] with full diagnostics.
    ///
    /// Convenience wrapper around [`tick`](Self::tick) that bundles the
    /// corrected period, input phase error, and net correction into a single
    /// struct.
    #[inline]
    pub fn tick_with_result(&mut self, phase_error_ns: f64) -> PllTickResult {
        let corrected = self.tick(phase_error_ns);
        PllTickResult {
            corrected_period_ns: corrected,
            phase_error_ns,
            correction_ns: self.nominal_period_ns - corrected,
        }
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

    /// Create a PLL with default PI gains for the given target frequency.
    ///
    /// Uses recommended gains for flight-sim RT scheduling:
    /// - `kp = 0.1` (fast proportional response)
    /// - `ki = 0.001` (slow integral — 0.1 %/s per ADR-005)
    pub fn from_hz(target_hz: u32) -> Self {
        let period_ns = 1_000_000_000.0 / target_hz as f64;
        Self::new(0.1, 0.001, period_ns)
    }

    /// Update the PLL with a measured inter-tick period and return a
    /// [`PllCorrection`].
    ///
    /// This is a convenience wrapper around [`tick`](Self::tick) for callers
    /// that measure the actual wall-clock period between ticks rather than
    /// computing the phase error themselves.
    ///
    /// # Zero-allocation guarantee
    ///
    /// No heap allocation occurs.
    #[inline]
    pub fn update(&mut self, actual_period_ns: u64) -> PllCorrection {
        let phase_error = actual_period_ns as f64 - self.nominal_period_ns;
        let corrected = self.tick(phase_error);
        let sleep_adjust = corrected - self.nominal_period_ns;
        PllCorrection {
            sleep_adjust_ns: sleep_adjust as i64,
            phase_error_ns: phase_error as i64,
            frequency_ratio: if actual_period_ns > 0 {
                self.nominal_period_ns / actual_period_ns as f64
            } else {
                1.0
            },
        }
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
// PllTickResult
// ---------------------------------------------------------------------------

/// Result of a single PLL tick, carrying the corrected period and
/// diagnostics.
///
/// Returned by [`PhaseLockLoop::tick_with_result`].
#[derive(Debug, Clone, Copy)]
pub struct PllTickResult {
    /// Corrected period for scheduling the next tick (nanoseconds).
    pub corrected_period_ns: f64,
    /// Phase error that was fed into this tick (nanoseconds; positive = late).
    pub phase_error_ns: f64,
    /// Net correction: `nominal − corrected` (positive means period was
    /// shortened to catch up).
    pub correction_ns: f64,
}

// ---------------------------------------------------------------------------
// PllCorrection — period-based update result
// ---------------------------------------------------------------------------

/// Result of a period-based PLL update via [`PhaseLockLoop::update`].
///
/// Unlike [`PllTickResult`] (which works with raw phase error), this struct
/// is returned when feeding measured inter-tick periods directly.
///
/// All fields are scalars — the struct is [`Copy`].
#[derive(Debug, Clone, Copy)]
pub struct PllCorrection {
    /// Adjustment to apply to the next sleep duration (nanoseconds).
    /// Positive = sleep longer, negative = sleep shorter.
    pub sleep_adjust_ns: i64,
    /// Phase error: measured period minus target period (nanoseconds).
    /// Positive = tick arrived late, negative = tick arrived early.
    pub phase_error_ns: i64,
    /// Ratio of target frequency to actual frequency.
    /// Values near 1.0 indicate good lock; >1.0 means running slow.
    pub frequency_ratio: f64,
}

// ---------------------------------------------------------------------------
// JitterStats — zero-allocation running statistics (ADR-004)
// ---------------------------------------------------------------------------

/// Zero-allocation running jitter statistics.
///
/// Accumulates min, max, sum, sum-of-squares, and count using only
/// stack-allocated scalars. All methods are **O(1)** and suitable for the
/// RT hot path per ADR-004.
///
/// [`p99_estimate`](Self::p99_estimate) uses the normal-approximation
/// formula `mean + 2.326 × σ`. For exact histogram-based percentiles see
/// [`crate::timer::TimerStats`].
#[derive(Debug, Clone, Copy)]
pub struct JitterStats {
    min_ns: i64,
    max_ns: i64,
    sum_ns: i64,
    sum_squares_ns: u128,
    count: u64,
}

impl JitterStats {
    /// Create empty statistics.
    pub const fn new() -> Self {
        Self {
            min_ns: i64::MAX,
            max_ns: i64::MIN,
            sum_ns: 0,
            sum_squares_ns: 0,
            count: 0,
        }
    }

    /// Record a single jitter sample (**hot path — zero allocation**).
    #[inline]
    pub fn record(&mut self, jitter_ns: i64) {
        self.count += 1;
        self.sum_ns += jitter_ns;
        let abs = jitter_ns.unsigned_abs() as u128;
        self.sum_squares_ns = self.sum_squares_ns.wrapping_add(abs * abs);
        if jitter_ns < self.min_ns {
            self.min_ns = jitter_ns;
        }
        if jitter_ns > self.max_ns {
            self.max_ns = jitter_ns;
        }
    }

    /// Mean jitter in nanoseconds (signed).
    pub fn mean(&self) -> f64 {
        if self.count == 0 {
            return 0.0;
        }
        self.sum_ns as f64 / self.count as f64
    }

    /// Population standard deviation in nanoseconds.
    pub fn stddev(&self) -> f64 {
        if self.count < 2 {
            return 0.0;
        }
        let mean = self.mean();
        let mean_of_squares = self.sum_squares_ns as f64 / self.count as f64;
        let variance = mean_of_squares - mean * mean;
        // Guard against negative variance from floating-point rounding.
        if variance < 0.0 {
            return 0.0;
        }
        variance.sqrt()
    }

    /// Approximate p99 using the normal-distribution formula
    /// `mean + 2.326 × σ`.
    pub fn p99_estimate(&self) -> f64 {
        self.mean() + 2.326 * self.stddev()
    }

    /// Minimum jitter observed (nanoseconds). Returns 0 if empty.
    pub fn min_ns(&self) -> i64 {
        if self.count == 0 { 0 } else { self.min_ns }
    }

    /// Maximum jitter observed (nanoseconds). Returns 0 if empty.
    pub fn max_ns(&self) -> i64 {
        if self.count == 0 { 0 } else { self.max_ns }
    }

    /// Number of samples recorded.
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Running sum of jitter values (nanoseconds, signed).
    pub fn sum_ns(&self) -> i64 {
        self.sum_ns
    }

    /// Reset all statistics to initial state.
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for JitterStats {
    fn default() -> Self {
        Self::new()
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
        let mut pll =
            PhaseLockLoop::new(0.1, 0.001, 4_000_000.0).with_lock_detection(50_000.0, 200_000.0, 5);

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
        let mut pll =
            PhaseLockLoop::new(0.1, 0.001, 4_000_000.0).with_lock_detection(50_000.0, 200_000.0, 5);

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
        let mut pll =
            PhaseLockLoop::new(0.1, 0.001, nominal).with_lock_detection(50_000.0, 200_000.0, 10);

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

    // -- PllTickResult tests --------------------------------------------

    #[test]
    fn pll_tick_with_result_returns_diagnostics() {
        let nominal = 4_000_000.0;
        let mut pll = PhaseLockLoop::new(0.1, 0.001, nominal);
        let result = pll.tick_with_result(1_000.0);
        assert!(result.corrected_period_ns < nominal);
        assert!((result.phase_error_ns - 1_000.0).abs() < f64::EPSILON);
        assert!(
            result.correction_ns > 0.0,
            "positive error should produce positive correction"
        );
    }

    #[test]
    fn pll_tick_with_result_negative_error() {
        let nominal = 4_000_000.0;
        let mut pll = PhaseLockLoop::new(0.1, 0.001, nominal);
        let result = pll.tick_with_result(-1_000.0);
        assert!(result.corrected_period_ns > nominal);
        assert!(
            result.correction_ns < 0.0,
            "negative error should produce negative correction"
        );
    }

    #[test]
    fn pll_tick_with_result_zero_error() {
        let nominal = 4_000_000.0;
        let mut pll = PhaseLockLoop::new(0.1, 0.001, nominal);
        let result = pll.tick_with_result(0.0);
        assert!((result.corrected_period_ns - nominal).abs() < 1e-6);
        assert!(result.correction_ns.abs() < 1e-6);
    }

    #[test]
    fn pll_tick_with_result_convergence() {
        let nominal = 4_000_000.0;
        let mut pll = PhaseLockLoop::new(0.1, 0.001, nominal);
        let mut error = 100_000.0; // 100µs initial error

        for _ in 0..500 {
            let result = pll.tick_with_result(error);
            error -= result.correction_ns;
        }

        assert!(
            error.abs() < 10_000.0,
            "PLL should converge via tick_with_result: residual {error} ns"
        );
    }

    // -- JitterStats tests ----------------------------------------------

    #[test]
    fn jitter_stats_empty_returns_zeros() {
        let stats = JitterStats::new();
        assert_eq!(stats.count(), 0);
        assert_eq!(stats.min_ns(), 0);
        assert_eq!(stats.max_ns(), 0);
        assert!((stats.mean() - 0.0).abs() < f64::EPSILON);
        assert!((stats.stddev() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn jitter_stats_single_value() {
        let mut stats = JitterStats::new();
        stats.record(5_000);
        assert_eq!(stats.count(), 1);
        assert_eq!(stats.min_ns(), 5_000);
        assert_eq!(stats.max_ns(), 5_000);
        assert!((stats.mean() - 5_000.0).abs() < f64::EPSILON);
        assert!((stats.stddev() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn jitter_stats_known_sequence() {
        let mut stats = JitterStats::new();
        stats.record(100);
        stats.record(200);
        stats.record(300);
        assert_eq!(stats.count(), 3);
        assert_eq!(stats.min_ns(), 100);
        assert_eq!(stats.max_ns(), 300);
        assert!((stats.mean() - 200.0).abs() < 1e-6);
        assert_eq!(stats.sum_ns(), 600);
    }

    #[test]
    fn jitter_stats_negative_values() {
        let mut stats = JitterStats::new();
        stats.record(-300);
        stats.record(500);
        stats.record(-100);
        assert_eq!(stats.min_ns(), -300);
        assert_eq!(stats.max_ns(), 500);
        let mean = stats.mean();
        // mean = (-300 + 500 + -100) / 3 = 100/3 ≈ 33.33
        assert!((mean - 100.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn jitter_stats_stddev_known_data() {
        let mut stats = JitterStats::new();
        // Values: 2, 4, 4, 4, 5, 5, 7, 9
        // Mean = 40/8 = 5.0
        // Variance = sum((x-5)²)/8 = (9+1+1+1+0+0+4+16)/8 = 32/8 = 4.0
        // stddev = 2.0
        for &v in &[2i64, 4, 4, 4, 5, 5, 7, 9] {
            stats.record(v);
        }
        assert!((stats.mean() - 5.0).abs() < 1e-6);
        assert!(
            (stats.stddev() - 2.0).abs() < 1e-6,
            "stddev should be 2.0, got {}",
            stats.stddev()
        );
    }

    #[test]
    fn jitter_stats_p99_estimate() {
        let mut stats = JitterStats::new();
        // Generate 1000 samples centered at 0 with known range
        for i in 0..1000i64 {
            stats.record(i - 500); // range [-500, 499]
        }
        let p99 = stats.p99_estimate();
        // p99 should be greater than mean and within reasonable bounds
        assert!(p99 > stats.mean());
        assert!(p99 < 1000.0, "p99 should be bounded, got {p99}");
    }

    #[test]
    fn jitter_stats_reset_clears_all() {
        let mut stats = JitterStats::new();
        for i in 0..100 {
            stats.record(i * 1000);
        }
        assert!(stats.count() > 0);
        stats.reset();
        assert_eq!(stats.count(), 0);
        assert_eq!(stats.min_ns(), 0);
        assert_eq!(stats.max_ns(), 0);
        assert!((stats.mean() - 0.0).abs() < f64::EPSILON);
    }

    // -- PllCorrection / from_hz / update tests -------------------------

    #[test]
    fn pll_from_hz_creates_correct_period() {
        let pll = PhaseLockLoop::from_hz(250);
        let expected = 1_000_000_000.0 / 250.0;
        assert!(
            (pll.nominal_period_ns() - expected).abs() < 1e-6,
            "expected {expected}, got {}",
            pll.nominal_period_ns()
        );
    }

    #[test]
    fn pll_from_hz_100() {
        let pll = PhaseLockLoop::from_hz(100);
        let expected = 10_000_000.0;
        assert!((pll.nominal_period_ns() - expected).abs() < 1e-6);
    }

    #[test]
    fn pll_update_returns_correction_opposing_late_arrival() {
        let mut pll = PhaseLockLoop::from_hz(250);
        let target = 4_000_000u64;
        let correction = pll.update(target + 1000); // 1 µs late
        assert!(
            correction.phase_error_ns > 0,
            "should report positive phase error"
        );
        assert!(
            correction.sleep_adjust_ns < 0,
            "should shorten sleep for late tick"
        );
    }

    #[test]
    fn pll_update_returns_correction_opposing_early_arrival() {
        let mut pll = PhaseLockLoop::from_hz(250);
        let target = 4_000_000u64;
        let correction = pll.update(target - 1000); // 1 µs early
        assert!(
            correction.phase_error_ns < 0,
            "should report negative phase error"
        );
        assert!(
            correction.sleep_adjust_ns > 0,
            "should lengthen sleep for early tick"
        );
    }

    #[test]
    fn pll_update_exact_period_gives_near_zero_correction() {
        let mut pll = PhaseLockLoop::from_hz(250);
        let target = 4_000_000u64;
        let correction = pll.update(target);
        assert!(
            correction.phase_error_ns.abs() <= 1,
            "exact period should give zero error, got {}",
            correction.phase_error_ns
        );
        assert!(
            correction.sleep_adjust_ns.abs() < 100,
            "correction should be tiny, got {}",
            correction.sleep_adjust_ns
        );
        assert!(
            (correction.frequency_ratio - 1.0).abs() < 0.001,
            "frequency ratio should be ~1.0, got {}",
            correction.frequency_ratio
        );
    }

    #[test]
    fn pll_update_convergence_from_offset() {
        let mut pll = PhaseLockLoop::from_hz(250);
        let target = 4_000_000u64;
        let mut simulated_period = target + 100_000; // start 100 µs slow

        for _ in 0..500 {
            let correction = pll.update(simulated_period);
            simulated_period = (simulated_period as i64 + correction.sleep_adjust_ns).max(1) as u64;
        }

        let final_error = (simulated_period as i64 - target as i64).unsigned_abs();
        assert!(
            final_error < 10_000,
            "should converge, residual: {final_error} ns"
        );
    }

    #[test]
    fn pll_update_handles_large_drift() {
        let mut pll = PhaseLockLoop::from_hz(250);
        let target = 4_000_000u64;
        // Period 10 % off — PLL should clamp correction
        let correction = pll.update(target + target / 10);
        let max_adjust = (target as f64 * 0.01) as i64;
        assert!(
            correction.sleep_adjust_ns.abs() <= max_adjust + 1,
            "correction {} should be clamped to ±{}",
            correction.sleep_adjust_ns,
            max_adjust
        );
    }

    #[test]
    fn pll_update_frequency_ratio_near_one_for_small_errors() {
        let mut pll = PhaseLockLoop::from_hz(250);
        let target = 4_000_000u64;
        let correction = pll.update(target + 100); // tiny error
        assert!(
            (correction.frequency_ratio - 1.0).abs() < 0.01,
            "frequency ratio should be near 1.0, got {}",
            correction.frequency_ratio
        );
    }

    #[test]
    fn pll_correction_is_copy() {
        let mut pll = PhaseLockLoop::from_hz(250);
        let c = pll.update(4_000_000);
        let c2 = c; // Copy
        let _ = c; // original still usable
        assert_eq!(c2.phase_error_ns, c.phase_error_ns);
    }

    #[test]
    fn pll_update_steady_state_accuracy() {
        let mut pll = PhaseLockLoop::from_hz(250);
        let target = 4_000_000u64;
        // Feed exact period many times
        for _ in 0..1000 {
            pll.update(target);
        }
        let correction = pll.update(target);
        assert!(
            correction.sleep_adjust_ns.abs() < 10,
            "steady-state correction should be near zero, got {}",
            correction.sleep_adjust_ns
        );
    }
}
