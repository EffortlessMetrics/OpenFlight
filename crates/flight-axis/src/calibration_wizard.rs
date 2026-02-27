// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Guided axis calibration wizard with a fixed-step state machine.
//!
//! Walks the user through center detection, min/max sweep, deadzone
//! measurement, and linearity verification — all with zero heap
//! allocations (RT-safe).

/// Maximum number of samples stored in the ring buffer.
const RING_CAP: usize = 1024;

/// Samples required for center detection.
const CENTER_SAMPLES: usize = 100;

/// Samples required for deadzone/noise measurement.
const DEADZONE_SAMPLES: usize = 200;

/// Maximum allowed range (fraction of 1.0) during center detection
/// before we declare "excessive noise".
const CENTER_NOISE_THRESHOLD: f64 = 0.05;

/// Fraction of center value the min/max must exceed on each side.
const SWEEP_THRESHOLD: f64 = 0.40;

// ── Public types ──────────────────────────────────────────────────────────

/// Calibration state machine for guided axis calibration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibrationStep {
    /// Wizard has not been started.
    NotStarted,
    /// "Release all controls to center."
    CenterDetection,
    /// "Move control to full extents."
    MinMaxSweep,
    /// "Release control — measuring noise."
    DeadzoneTest,
    /// "Move control slowly for linearity check."
    Verification,
    /// Calibration finished successfully.
    Complete,
    /// Calibration failed with an error.
    Failed(CalibrationError),
}

/// Errors that can occur during calibration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibrationError {
    /// Axis didn't reach enough range during sweep.
    InsufficientRange,
    /// Too much noise at center position.
    ExcessiveNoise,
    /// Linearity (monotonicity) check failed.
    NonMonotonic,
    /// A step took longer than the configured timeout.
    Timeout,
}

/// A single raw calibration sample (stack-allocated).
#[derive(Debug, Clone, Copy)]
pub struct CalibrationSample {
    /// Raw axis value in arbitrary units (typically 0.0–1.0).
    pub raw_value: f64,
    /// Timestamp in microseconds.
    pub timestamp_us: u64,
}

/// Result produced by a successful calibration run.
#[derive(Debug, Clone, Copy)]
pub struct CalibrationResult {
    /// Detected center (rest) value.
    pub center: f64,
    /// Minimum value seen during sweep.
    pub min: f64,
    /// Maximum value seen during sweep.
    pub max: f64,
    /// Recommended deadzone radius (2× noise floor).
    pub deadzone_recommended: f64,
    /// Measured RMS-like noise at rest.
    pub noise_floor: f64,
    /// Linearity / monotonicity score (0.0 = terrible, 1.0 = perfect).
    pub linearity_score: f64,
}

/// Fixed-size, RT-safe calibration wizard.
///
/// All storage is inline — no heap allocations ever occur.
pub struct CalibrationWizard {
    step: CalibrationStep,
    samples: [CalibrationSample; RING_CAP],
    sample_count: usize,
    sample_head: usize,
    center_value: f64,
    min_seen: f64,
    max_seen: f64,
    noise_sum: f64,
    noise_samples: usize,
    step_start_us: u64,
    step_timeout_us: u64,
    linearity_score: f64,
}

impl CalibrationWizard {
    /// Creates a new wizard with the given per-step timeout (microseconds).
    pub fn new(step_timeout_us: u64) -> Self {
        let zero_sample = CalibrationSample {
            raw_value: 0.0,
            timestamp_us: 0,
        };
        Self {
            step: CalibrationStep::NotStarted,
            samples: [zero_sample; RING_CAP],
            sample_count: 0,
            sample_head: 0,
            center_value: 0.0,
            min_seen: f64::MAX,
            max_seen: f64::MIN,
            noise_sum: 0.0,
            noise_samples: 0,
            step_start_us: 0,
            step_timeout_us,
            linearity_score: 0.0,
        }
    }

    /// Begin the calibration process (transitions to `CenterDetection`).
    pub fn start(&mut self) {
        self.clear_samples();
        self.step = CalibrationStep::CenterDetection;
    }

    /// Feed a new raw sample into the wizard.
    ///
    /// Samples are ignored when the wizard is `NotStarted`, `Complete`, or
    /// `Failed`.
    pub fn feed_sample(&mut self, sample: CalibrationSample) {
        match self.step {
            CalibrationStep::NotStarted
            | CalibrationStep::Complete
            | CalibrationStep::Failed(_) => return,
            _ => {}
        }

        // Record step start time from the first sample of each step.
        if self.sample_count == 0 {
            self.step_start_us = sample.timestamp_us;
        }

        // Write into the ring buffer.
        self.samples[self.sample_head] = sample;
        self.sample_head = (self.sample_head + 1) % RING_CAP;
        if self.sample_count < RING_CAP {
            self.sample_count += 1;
        }

        // Per-step accumulation.
        match self.step {
            CalibrationStep::MinMaxSweep => {
                if sample.raw_value < self.min_seen {
                    self.min_seen = sample.raw_value;
                }
                if sample.raw_value > self.max_seen {
                    self.max_seen = sample.raw_value;
                }
            }
            CalibrationStep::DeadzoneTest => {
                let diff = sample.raw_value - self.center_value;
                self.noise_sum += diff * diff;
                self.noise_samples += 1;
            }
            _ => {}
        }
    }

    /// Check whether the current step is complete and, if so, transition to
    /// the next one.  Returns the (possibly updated) current step.
    pub fn advance(&mut self) -> CalibrationStep {
        match self.step {
            CalibrationStep::CenterDetection => self.advance_center(),
            CalibrationStep::MinMaxSweep => self.advance_sweep(),
            CalibrationStep::DeadzoneTest => self.advance_deadzone(),
            CalibrationStep::Verification => self.advance_verification(),
            _ => {}
        }
        self.step
    }

    /// Returns the current step.
    #[inline]
    pub fn current_step(&self) -> CalibrationStep {
        self.step
    }

    /// Returns a [`CalibrationResult`] if calibration is `Complete`.
    pub fn result(&self) -> Option<CalibrationResult> {
        if self.step != CalibrationStep::Complete {
            return None;
        }
        Some(CalibrationResult {
            center: self.center_value,
            min: self.min_seen,
            max: self.max_seen,
            deadzone_recommended: self.noise_floor() * 2.0,
            noise_floor: self.noise_floor(),
            linearity_score: self.linearity_score,
        })
    }

    /// Reset the wizard back to `NotStarted`.
    pub fn reset(&mut self) {
        self.step = CalibrationStep::NotStarted;
        self.clear_samples();
        self.center_value = 0.0;
        self.min_seen = f64::MAX;
        self.max_seen = f64::MIN;
        self.noise_sum = 0.0;
        self.noise_samples = 0;
        self.linearity_score = 0.0;
    }

    /// Overall progress as a percentage (0–100).
    pub fn progress_percent(&self) -> f32 {
        match self.step {
            CalibrationStep::NotStarted => 0.0,
            CalibrationStep::CenterDetection => {
                let pct = self.sample_count as f32 / CENTER_SAMPLES as f32;
                pct.min(1.0) * 20.0
            }
            CalibrationStep::MinMaxSweep => 20.0 + self.sweep_progress() * 30.0,
            CalibrationStep::DeadzoneTest => {
                let pct = self.sample_count as f32 / DEADZONE_SAMPLES as f32;
                50.0 + pct.min(1.0) * 25.0
            }
            CalibrationStep::Verification => 75.0 + self.sample_count.min(50) as f32 / 50.0 * 25.0,
            CalibrationStep::Complete => 100.0,
            CalibrationStep::Failed(_) => 0.0,
        }
    }

    /// Total number of samples collected in the current step.
    #[inline]
    pub fn samples_collected(&self) -> usize {
        self.sample_count
    }

    // ── Private helpers ──────────────────────────────────────────────────

    fn clear_samples(&mut self) {
        self.sample_count = 0;
        self.sample_head = 0;
        self.step_start_us = 0;
    }

    fn last_timestamp(&self) -> u64 {
        if self.sample_count == 0 {
            return 0;
        }
        let idx = if self.sample_head == 0 {
            RING_CAP - 1
        } else {
            self.sample_head - 1
        };
        self.samples[idx].timestamp_us
    }

    fn is_timed_out(&self) -> bool {
        if self.step_timeout_us == 0 || self.sample_count == 0 {
            return false;
        }
        let elapsed = self.last_timestamp().saturating_sub(self.step_start_us);
        elapsed >= self.step_timeout_us
    }

    fn transition(&mut self, next: CalibrationStep) {
        self.step = next;
        self.clear_samples();
    }

    fn fail(&mut self, err: CalibrationError) {
        self.step = CalibrationStep::Failed(err);
    }

    fn noise_floor(&self) -> f64 {
        if self.noise_samples < 2 {
            return 0.0;
        }
        (self.noise_sum / self.noise_samples as f64).sqrt()
    }

    /// Compute what fraction of the sweep criteria is met (0.0–1.0).
    fn sweep_progress(&self) -> f32 {
        if self.center_value.abs() < f64::EPSILON {
            return 0.0;
        }
        let range = self.max_seen - self.min_seen;
        let required = self.center_value.abs() * SWEEP_THRESHOLD * 2.0;
        if required <= 0.0 {
            return 0.0;
        }
        (range as f32 / required as f32).min(1.0)
    }

    // ── Step-specific advance logic ──────────────────────────────────────

    fn advance_center(&mut self) {
        if self.is_timed_out() {
            self.fail(CalibrationError::Timeout);
            return;
        }
        if self.sample_count < CENTER_SAMPLES {
            return;
        }

        // Compute average of collected samples.
        let mut sum = 0.0_f64;
        let mut min_v = f64::MAX;
        let mut max_v = f64::MIN;
        let start = if self.sample_count <= RING_CAP {
            0
        } else {
            self.sample_head
        };
        let count = self.sample_count.min(RING_CAP);
        for i in 0..count {
            let idx = (start + i) % RING_CAP;
            let v = self.samples[idx].raw_value;
            sum += v;
            if v < min_v {
                min_v = v;
            }
            if v > max_v {
                max_v = v;
            }
        }

        let avg = sum / count as f64;
        let range = max_v - min_v;

        // If the range during "hold center" exceeds the noise threshold
        // relative to the average, the input is too noisy.
        if avg.abs() > f64::EPSILON && range / avg.abs() > CENTER_NOISE_THRESHOLD {
            self.fail(CalibrationError::ExcessiveNoise);
            return;
        }
        // Also handle zero-centered values: use absolute range check.
        if avg.abs() <= f64::EPSILON && range > CENTER_NOISE_THRESHOLD {
            self.fail(CalibrationError::ExcessiveNoise);
            return;
        }

        self.center_value = avg;
        self.min_seen = avg;
        self.max_seen = avg;
        self.transition(CalibrationStep::MinMaxSweep);
    }

    fn advance_sweep(&mut self) {
        if self.is_timed_out() {
            // Check if we at least got *some* range before declaring failure.
            let low_ok = self.min_seen < self.center_value - self.center_value.abs() * SWEEP_THRESHOLD;
            let high_ok = self.max_seen > self.center_value + self.center_value.abs() * SWEEP_THRESHOLD;
            if !(low_ok && high_ok) {
                self.fail(CalibrationError::InsufficientRange);
                return;
            }
        }

        let low_ok = self.min_seen < self.center_value - self.center_value.abs() * SWEEP_THRESHOLD;
        let high_ok = self.max_seen > self.center_value + self.center_value.abs() * SWEEP_THRESHOLD;

        if low_ok && high_ok {
            self.transition(CalibrationStep::DeadzoneTest);
        }
    }

    fn advance_deadzone(&mut self) {
        if self.is_timed_out() {
            self.fail(CalibrationError::Timeout);
            return;
        }
        if self.sample_count >= DEADZONE_SAMPLES {
            self.transition(CalibrationStep::Verification);
        }
    }

    fn advance_verification(&mut self) {
        if self.is_timed_out() {
            self.fail(CalibrationError::Timeout);
            return;
        }
        if self.sample_count < 10 {
            return;
        }

        // Check monotonicity: count adjacent pairs that go in the same
        // direction as their overall trend.
        let count = self.sample_count.min(RING_CAP);
        let start = if self.sample_count <= RING_CAP {
            0
        } else {
            self.sample_head
        };

        let mut monotonic_pairs: usize = 0;
        let total_pairs = count - 1;

        for i in 0..total_pairs {
            let idx_a = (start + i) % RING_CAP;
            let idx_b = (start + i + 1) % RING_CAP;
            let a = self.samples[idx_a].raw_value;
            let b = self.samples[idx_b].raw_value;
            // We accept both ascending and equal (plateau is fine).
            if b >= a {
                monotonic_pairs += 1;
            }
        }

        let score = if total_pairs > 0 {
            monotonic_pairs as f64 / total_pairs as f64
        } else {
            1.0
        };

        if score < 0.5 {
            self.fail(CalibrationError::NonMonotonic);
            return;
        }

        self.linearity_score = score;
        self.step = CalibrationStep::Complete;
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(raw: f64, ts: u64) -> CalibrationSample {
        CalibrationSample {
            raw_value: raw,
            timestamp_us: ts,
        }
    }

    // 1. Initial state is NotStarted
    #[test]
    fn initial_state_is_not_started() {
        let wiz = CalibrationWizard::new(10_000_000);
        assert_eq!(wiz.current_step(), CalibrationStep::NotStarted);
        assert_eq!(wiz.samples_collected(), 0);
        assert!(wiz.result().is_none());
    }

    // 2. Start transitions to CenterDetection
    #[test]
    fn start_transitions_to_center_detection() {
        let mut wiz = CalibrationWizard::new(10_000_000);
        wiz.start();
        assert_eq!(wiz.current_step(), CalibrationStep::CenterDetection);
    }

    // 3. Center detection with stable samples succeeds
    #[test]
    fn center_detection_stable_succeeds() {
        let mut wiz = CalibrationWizard::new(10_000_000);
        wiz.start();

        for i in 0..CENTER_SAMPLES {
            wiz.feed_sample(sample(0.5, i as u64 * 1000));
        }

        let step = wiz.advance();
        assert_eq!(step, CalibrationStep::MinMaxSweep);
        assert!((wiz.center_value - 0.5).abs() < 1e-6);
    }

    // 4. Center detection with noisy samples fails
    #[test]
    fn center_detection_noisy_fails() {
        let mut wiz = CalibrationWizard::new(10_000_000);
        wiz.start();

        for i in 0..CENTER_SAMPLES {
            // Oscillate wildly around 0.5
            let v = if i % 2 == 0 { 0.3 } else { 0.7 };
            wiz.feed_sample(sample(v, i as u64 * 1000));
        }

        let step = wiz.advance();
        assert_eq!(step, CalibrationStep::Failed(CalibrationError::ExcessiveNoise));
    }

    // 5. Min/max sweep detects full range
    #[test]
    fn sweep_detects_full_range() {
        let mut wiz = CalibrationWizard::new(10_000_000);
        wiz.start();

        // Stable center at 0.5
        for i in 0..CENTER_SAMPLES {
            wiz.feed_sample(sample(0.5, i as u64 * 1000));
        }
        wiz.advance();
        assert_eq!(wiz.current_step(), CalibrationStep::MinMaxSweep);

        // Sweep from low to high
        let base_ts = CENTER_SAMPLES as u64 * 1000;
        for i in 0..50 {
            let v = 0.5 - (i as f64 / 50.0) * 0.5; // 0.5 → 0.0
            wiz.feed_sample(sample(v, base_ts + i as u64 * 1000));
        }
        for i in 0..50 {
            let v = 0.0 + (i as f64 / 50.0) * 1.0; // 0.0 → 1.0
            wiz.feed_sample(sample(v, base_ts + 50_000 + i as u64 * 1000));
        }

        let step = wiz.advance();
        assert_eq!(step, CalibrationStep::DeadzoneTest);
    }

    // 6. Min/max sweep with insufficient range fails (timeout)
    #[test]
    fn sweep_insufficient_range_fails() {
        let mut wiz = CalibrationWizard::new(1_000_000); // 1s timeout
        wiz.start();

        // Stable center at 0.5
        for i in 0..CENTER_SAMPLES {
            wiz.feed_sample(sample(0.5, i as u64 * 1000));
        }
        wiz.advance();

        // Tiny range that doesn't meet the 40% threshold
        let base_ts = CENTER_SAMPLES as u64 * 1000;
        for i in 0..100 {
            wiz.feed_sample(sample(0.48 + 0.04 * (i as f64 / 100.0), base_ts + i as u64 * 20_000));
        }

        let step = wiz.advance();
        assert_eq!(step, CalibrationStep::Failed(CalibrationError::InsufficientRange));
    }

    // 7. Deadzone test recommends appropriate value
    #[test]
    fn deadzone_recommends_value() {
        let mut wiz = CalibrationWizard::new(10_000_000);
        wiz.start();

        // Center
        for i in 0..CENTER_SAMPLES {
            wiz.feed_sample(sample(0.5, i as u64 * 1000));
        }
        wiz.advance();

        // Sweep
        let base = CENTER_SAMPLES as u64 * 1000;
        wiz.feed_sample(sample(0.0, base));
        wiz.feed_sample(sample(1.0, base + 1000));
        wiz.advance();
        assert_eq!(wiz.current_step(), CalibrationStep::DeadzoneTest);

        // Deadzone: samples near center with small noise
        let base2 = base + 2000;
        for i in 0..DEADZONE_SAMPLES {
            let noise = if i % 2 == 0 { 0.001 } else { -0.001 };
            wiz.feed_sample(sample(0.5 + noise, base2 + i as u64 * 1000));
        }
        wiz.advance();
        assert_eq!(wiz.current_step(), CalibrationStep::Verification);

        // The noise floor should be ~0.001, so recommended deadzone ~0.002
        let nf = wiz.noise_floor();
        assert!(nf > 0.0005, "noise floor too small: {nf}");
        assert!(nf < 0.01, "noise floor too large: {nf}");
    }

    // 8. Verification passes with linear data
    #[test]
    fn verification_passes_linear() {
        let mut wiz = drive_to_verification();

        // Monotonically increasing sweep
        for i in 0..50 {
            wiz.feed_sample(sample(i as f64 / 50.0, 500_000 + i as u64 * 1000));
        }

        let step = wiz.advance();
        assert_eq!(step, CalibrationStep::Complete);
    }

    // 9. Complete produces valid CalibrationResult
    #[test]
    fn complete_produces_result() {
        let mut wiz = drive_to_verification();

        for i in 0..50 {
            wiz.feed_sample(sample(i as f64 / 50.0, 500_000 + i as u64 * 1000));
        }
        wiz.advance();

        let res = wiz.result().expect("should have result when Complete");
        assert!((res.center - 0.5).abs() < 0.01);
        assert!(res.min < 0.1);
        assert!(res.max > 0.9);
        assert!(res.linearity_score > 0.8);
        assert!(res.deadzone_recommended >= 0.0);
    }

    // 10. Reset returns to NotStarted
    #[test]
    fn reset_returns_to_not_started() {
        let mut wiz = drive_to_verification();
        wiz.reset();
        assert_eq!(wiz.current_step(), CalibrationStep::NotStarted);
        assert_eq!(wiz.samples_collected(), 0);
        assert!(wiz.result().is_none());
    }

    // 11. Progress increases through steps
    #[test]
    fn progress_increases_through_steps() {
        let mut wiz = CalibrationWizard::new(10_000_000);
        let p0 = wiz.progress_percent();
        assert!((p0 - 0.0).abs() < f32::EPSILON);

        wiz.start();
        for i in 0..CENTER_SAMPLES {
            wiz.feed_sample(sample(0.5, i as u64 * 1000));
        }
        let p1 = wiz.progress_percent();
        assert!(p1 > p0, "progress should increase during center: {p1}");

        wiz.advance();
        let base = CENTER_SAMPLES as u64 * 1000;
        wiz.feed_sample(sample(0.0, base));
        wiz.feed_sample(sample(1.0, base + 1000));
        wiz.advance();

        let p2 = wiz.progress_percent();
        assert!(p2 > p1, "progress should increase after sweep: {p2} vs {p1}");

        let base2 = base + 2000;
        for i in 0..DEADZONE_SAMPLES {
            wiz.feed_sample(sample(0.5, base2 + i as u64 * 1000));
        }
        wiz.advance();

        let p3 = wiz.progress_percent();
        assert!(p3 > p2, "progress should increase after deadzone: {p3} vs {p2}");

        for i in 0..50 {
            wiz.feed_sample(sample(i as f64 / 50.0, 600_000 + i as u64 * 1000));
        }
        wiz.advance();
        let p4 = wiz.progress_percent();
        assert!((p4 - 100.0).abs() < f32::EPSILON, "complete should be 100%: {p4}");
    }

    // 12. Timeout handling
    #[test]
    fn timeout_fails_center_detection() {
        let mut wiz = CalibrationWizard::new(100_000); // 100ms timeout
        wiz.start();

        // Feed samples spread over more than 100ms
        for i in 0..10 {
            wiz.feed_sample(sample(0.5, i as u64 * 50_000));
        }

        let step = wiz.advance();
        assert_eq!(step, CalibrationStep::Failed(CalibrationError::Timeout));
    }

    // 13. Verification fails with non-monotonic data
    #[test]
    fn verification_fails_non_monotonic() {
        let mut wiz = drive_to_verification();

        // Feed chaotic data
        for i in 0..50 {
            let v = if i % 2 == 0 { 1.0 } else { 0.0 };
            wiz.feed_sample(sample(v, 500_000 + i as u64 * 1000));
        }

        let step = wiz.advance();
        assert_eq!(step, CalibrationStep::Failed(CalibrationError::NonMonotonic));
    }

    // 14. Samples ignored in terminal states
    #[test]
    fn samples_ignored_in_terminal_states() {
        let mut wiz = CalibrationWizard::new(10_000_000);
        wiz.feed_sample(sample(0.5, 1000));
        assert_eq!(wiz.samples_collected(), 0);
    }

    // ── Test helper ──────────────────────────────────────────────────────

    /// Drives a wizard through center → sweep → deadzone → verification.
    fn drive_to_verification() -> CalibrationWizard {
        let mut wiz = CalibrationWizard::new(10_000_000);
        wiz.start();

        // Center detection
        for i in 0..CENTER_SAMPLES {
            wiz.feed_sample(sample(0.5, i as u64 * 1000));
        }
        wiz.advance();

        // Min/max sweep
        let base = CENTER_SAMPLES as u64 * 1000;
        wiz.feed_sample(sample(0.0, base));
        wiz.feed_sample(sample(1.0, base + 1000));
        wiz.advance();

        // Deadzone
        let base2 = base + 2000;
        for i in 0..DEADZONE_SAMPLES {
            let noise = if i % 2 == 0 { 0.001 } else { -0.001 };
            wiz.feed_sample(sample(0.5 + noise, base2 + i as u64 * 1000));
        }
        wiz.advance();

        assert_eq!(wiz.current_step(), CalibrationStep::Verification);
        wiz
    }
}
