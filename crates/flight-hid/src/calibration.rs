// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID axis calibration — raw device integer → normalised `f32` mapping.
//!
//! HID devices report axis values as unsigned integers whose range depends on
//! the hardware (10-bit → 1023, 12-bit → 4095, 16-bit → 65535, …).
//! [`AxisCalibration`] maps those raw counts to a normalised floating-point
//! value in a configurable output range with an optional centre deadzone and
//! polarity reversal.
//!
//! # Example
//!
//! ```rust
//! use flight_hid::calibration::AxisCalibration;
//!
//! // 16-bit stick axis: centre at 32767, symmetric output [-1.0, 1.0], 3% deadzone.
//! let cal = AxisCalibration {
//!     raw_min:    0,
//!     raw_max:    65535,
//!     raw_center: 32767,
//!     deadzone:   0.03,
//!     output_min: -1.0,
//!     output_max:  1.0,
//!     reversed:   false,
//! };
//!
//! assert!((cal.normalize(32767) - 0.0_f32).abs() < 1e-4, "centre → 0");
//! assert!((cal.normalize(65535) - 1.0_f32).abs() < 1e-4, "max → 1");
//! assert!((cal.normalize(0)     - (-1.0_f32)).abs() < 1e-4, "min → -1");
//! ```

use std::collections::HashMap;
use std::time::Instant;

use crate::device_id::DeviceId;

/// Calibration parameters for a single HID axis channel.
///
/// Converts a raw hardware axis integer into a normalised `f32` value in
/// `[output_min, output_max]`.
///
/// # Parameter constraints
///
/// * `raw_min < raw_max` — non-degenerate raw range.  If `raw_min == raw_max`
///   the midpoint of the output range is returned for every input.
/// * `raw_min ≤ raw_center ≤ raw_max` — the centre is clamped into the raw
///   range before use, so out-of-range values are accepted but unusual.
/// * `output_min < output_max` — non-degenerate output range.
/// * `0.0 ≤ deadzone < 1.0` — clamped to `[0, 0.9999]` internally.
#[derive(Debug, Clone)]
pub struct AxisCalibration {
    /// Minimum raw value the device can report.
    pub raw_min: u32,
    /// Maximum raw value the device can report.
    pub raw_max: u32,
    /// Raw value corresponding to the neutral / centre position.
    ///
    /// For self-centring stick axes this is the mid-point of the hardware
    /// range; for throttles without a centre detent it is usually equal to
    /// `raw_min`.
    pub raw_center: u32,
    /// Deadzone radius around `raw_center`, as a fraction of the output
    /// half-range `[0.0, 1.0)`.
    ///
    /// Any raw input that falls within this radius of `raw_center` is mapped
    /// to the centre output value (i.e. zero for symmetric stick axes).
    /// Outside the deadzone the remaining range is rescaled to fill
    /// `[output_min, output_max]`.
    pub deadzone: f32,
    /// Minimum value of the normalised output range (e.g. `-1.0` for sticks,
    /// `0.0` for throttles).
    pub output_min: f32,
    /// Maximum value of the normalised output range (e.g. `1.0`).
    pub output_max: f32,
    /// When `true` the polarity is inverted: `raw_min` maps to `output_max`
    /// and `raw_max` maps to `output_min`.
    pub reversed: bool,
}

impl AxisCalibration {
    /// Normalise a raw hardware axis value into `[output_min, output_max]`.
    ///
    /// Steps:
    ///
    /// 1. Clamp `raw` to `[raw_min, raw_max]`.
    /// 2. Map linearly: `raw_min → output_min`, `raw_max → output_max`.
    /// 3. If `reversed`, flip the output around the output midpoint.
    /// 4. Apply the centre deadzone: values within `deadzone` of the centre
    ///    output are pinned to the centre; outside values are rescaled to
    ///    preserve the full output range.
    /// 5. Final clamp to `[output_min, output_max]` to absorb f32 rounding.
    ///
    /// The returned value is always finite and within `[output_min, output_max]`.
    pub fn normalize(&self, raw: u32) -> f32 {
        let raw_min_f = self.raw_min as f32;
        let raw_max_f = self.raw_max as f32;
        let raw_range = raw_max_f - raw_min_f;

        // Degenerate range → return midpoint.
        if raw_range <= 0.0 {
            return (self.output_min + self.output_max) * 0.5;
        }

        // Step 1: clamp raw to hardware range.
        let clamped = (raw as f32).clamp(raw_min_f, raw_max_f);

        // Step 2: linear map [raw_min, raw_max] → [output_min, output_max].
        let unit = (clamped - raw_min_f) / raw_range;
        let out_range = self.output_max - self.output_min;
        let mut out = self.output_min + unit * out_range;

        // Step 3: polarity reversal.
        if self.reversed {
            out = self.output_min + self.output_max - out;
        }

        // Step 4: centre deadzone.
        //
        // Compute the output value that raw_center maps to (honouring `reversed`),
        // then apply a symmetric deadzone around that centre.
        let dz = self.deadzone.clamp(0.0, 0.9999);
        if dz > 0.0 {
            let raw_ctr = (self.raw_center as f32).clamp(raw_min_f, raw_max_f);
            let ctr_unit = (raw_ctr - raw_min_f) / raw_range;
            let mut center_out = self.output_min + ctr_unit * out_range;
            if self.reversed {
                center_out = self.output_min + self.output_max - center_out;
            }

            // Normalise `out` relative to `center_out` into [-1, 1] space.
            let half = out_range * 0.5;
            if half > 0.0 {
                let norm = (out - center_out) / half;
                if norm.abs() < dz {
                    // Inside deadzone → snap to centre.
                    out = center_out;
                } else {
                    // Outside deadzone → rescale so the edge still reaches ±1.
                    let sign = norm.signum();
                    let rescaled = sign * (norm.abs() - dz) / (1.0 - dz);
                    out = center_out + rescaled * half;
                }
            }
        }

        // Step 5: clamp to absorb f32 rounding.
        out.clamp(self.output_min, self.output_max)
    }
}

// ── Device Calibration Wizard ────────────────────────────────────────────

/// Minimum center-detection samples per axis.
const WIZARD_MIN_CENTER_SAMPLES: usize = 50;

/// Minimum range-detection samples per axis.
const WIZARD_MIN_RANGE_SAMPLES: usize = 100;

/// Minimum verification samples per axis.
const WIZARD_MIN_VERIFY_SAMPLES: usize = 20;

/// Low percentile for outlier rejection.
const PERCENTILE_LOW: f64 = 1.0;

/// High percentile for outlier rejection.
const PERCENTILE_HIGH: f64 = 99.0;

/// Minimum acceptable quality score.
const MIN_QUALITY_THRESHOLD: f32 = 20.0;

/// Identifier for a device axis channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AxisId(pub u8);

impl std::fmt::Display for AxisId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Axis({})", self.0)
    }
}

/// State of the calibration wizard state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalibrationState {
    /// Wizard is idle, not started.
    Idle,
    /// Detecting center position — user should release all controls.
    CenterDetection,
    /// Detecting range for a specific axis — user should move it to extremes.
    RangeDetection(AxisId),
    /// Verifying calibration quality.
    Verification,
    /// Calibration completed successfully.
    Complete,
    /// Calibration failed with an error message.
    Failed(String),
}

/// Per-axis calibration data produced by the wizard.
#[derive(Debug, Clone)]
pub struct AxisCalibrationData {
    /// Detected center (neutral) raw value.
    pub center_value: u32,
    /// Minimum raw value (1st percentile, outlier-rejected).
    pub min_value: u32,
    /// Maximum raw value (99th percentile, outlier-rejected).
    pub max_value: u32,
    /// Recommended deadzone as a fraction of the half-range `[0.0, 1.0)`.
    pub deadzone_recommendation: f32,
    /// Noise floor: standard deviation of center samples in raw units.
    pub noise_floor: f32,
}

/// Result of a completed device calibration.
#[derive(Debug, Clone)]
pub struct CalibrationResult {
    /// Per-axis calibration data.
    pub axes: HashMap<AxisId, AxisCalibrationData>,
    /// How symmetric the per-axis ranges are (0.0–1.0, 1.0 = perfectly symmetric).
    pub symmetry_score: f32,
    /// Overall calibration quality score (0–100).
    pub quality_score: f32,
    /// Timestamp when calibration completed.
    pub created_at: Instant,
}

impl CalibrationResult {
    /// Convert calibration results into [`AxisCalibration`] configs suitable
    /// for use in a device profile.
    pub fn to_profile_config(&self) -> Vec<(AxisId, AxisCalibration)> {
        self.axes
            .iter()
            .map(|(&axis_id, data)| {
                let cal = AxisCalibration {
                    raw_min: data.min_value,
                    raw_max: data.max_value,
                    raw_center: data.center_value,
                    deadzone: data.deadzone_recommendation,
                    output_min: -1.0,
                    output_max: 1.0,
                    reversed: false,
                };
                (axis_id, cal)
            })
            .collect()
    }
}

/// Device calibration wizard — guides the user through a multi-axis
/// calibration process with center detection, range mapping, and quality
/// verification.
///
/// # State machine
///
/// ```text
/// Idle ─start()─▶ CenterDetection ─advance()─▶ RangeDetection(axis₀)
///   ─advance()─▶ … ─▶ RangeDetection(axisₙ) ─advance()─▶ Verification
///   ─advance()─▶ Complete | Failed
/// ```
///
/// Call [`abort()`](CalibrationWizard::abort) from any active state to
/// return to `Idle`.
pub struct CalibrationWizard {
    device_id: DeviceId,
    axes: Vec<AxisId>,
    state: CalibrationState,
    center_samples: HashMap<AxisId, Vec<u32>>,
    range_samples: HashMap<AxisId, Vec<u32>>,
    verify_samples: HashMap<AxisId, Vec<u32>>,
    current_range_axis_idx: usize,
    computed_centers: HashMap<AxisId, u32>,
    result: Option<CalibrationResult>,
}

impl CalibrationWizard {
    /// Create a new calibration wizard for the given device and axes.
    pub fn new(device_id: DeviceId, axes: Vec<AxisId>) -> Self {
        let mut center_samples = HashMap::new();
        let mut range_samples = HashMap::new();
        let mut verify_samples = HashMap::new();
        for &axis in &axes {
            center_samples.insert(axis, Vec::new());
            range_samples.insert(axis, Vec::new());
            verify_samples.insert(axis, Vec::new());
        }
        Self {
            device_id,
            axes,
            state: CalibrationState::Idle,
            center_samples,
            range_samples,
            verify_samples,
            current_range_axis_idx: 0,
            computed_centers: HashMap::new(),
            result: None,
        }
    }

    /// Start the calibration process (transitions to `CenterDetection`).
    pub fn start(&mut self) {
        if self.axes.is_empty() {
            self.state = CalibrationState::Failed("no axes configured".into());
            return;
        }
        self.reset_samples();
        self.state = CalibrationState::CenterDetection;
    }

    /// Feed a raw axis sample into the wizard.
    ///
    /// Samples are silently ignored in `Idle`, `Complete`, and `Failed` states,
    /// or if `axis_id` is not in the configured axis list.
    pub fn record_sample(&mut self, axis_id: AxisId, raw_value: u32) {
        match &self.state {
            CalibrationState::Idle | CalibrationState::Complete | CalibrationState::Failed(_) => {
                return;
            }
            _ => {}
        }
        if !self.axes.contains(&axis_id) {
            return;
        }
        match &self.state {
            CalibrationState::CenterDetection => {
                if let Some(samples) = self.center_samples.get_mut(&axis_id) {
                    samples.push(raw_value);
                }
            }
            CalibrationState::RangeDetection(target) => {
                if axis_id == *target
                    && let Some(samples) = self.range_samples.get_mut(&axis_id)
                {
                    samples.push(raw_value);
                }
            }
            CalibrationState::Verification => {
                if let Some(samples) = self.verify_samples.get_mut(&axis_id) {
                    samples.push(raw_value);
                }
            }
            _ => {}
        }
    }

    /// Advance to the next calibration step if the current step has collected
    /// enough data. Does nothing if the wizard is in a terminal state.
    pub fn advance(&mut self) {
        match &self.state {
            CalibrationState::CenterDetection => self.advance_center(),
            CalibrationState::RangeDetection(_) => self.advance_range(),
            CalibrationState::Verification => self.advance_verification(),
            _ => {}
        }
    }

    /// Abort the calibration and return to `Idle`.
    pub fn abort(&mut self) {
        self.state = CalibrationState::Idle;
        self.reset_samples();
        self.result = None;
    }

    /// Get the final calibration result, if calibration completed.
    pub fn result(&self) -> Option<CalibrationResult> {
        self.result.clone()
    }

    /// Get the current wizard state.
    pub fn current_step(&self) -> CalibrationState {
        self.state.clone()
    }

    /// Overall progress as a fraction in `[0.0, 1.0]`.
    pub fn progress(&self) -> f32 {
        match &self.state {
            CalibrationState::Idle => 0.0,
            CalibrationState::CenterDetection => {
                let min_count = self
                    .axes
                    .iter()
                    .filter_map(|a| self.center_samples.get(a))
                    .map(Vec::len)
                    .min()
                    .unwrap_or(0);
                let frac = (min_count as f32 / WIZARD_MIN_CENTER_SAMPLES as f32).min(1.0);
                frac * 0.2
            }
            CalibrationState::RangeDetection(current_axis) => {
                let completed = self.current_range_axis_idx as f32;
                let total = self.axes.len() as f32;
                let current_count = self.range_samples.get(current_axis).map_or(0, Vec::len);
                let axis_frac = (current_count as f32 / WIZARD_MIN_RANGE_SAMPLES as f32).min(1.0);
                let range_frac = (completed + axis_frac) / total;
                0.2 + range_frac * 0.5
            }
            CalibrationState::Verification => {
                let min_count = self
                    .axes
                    .iter()
                    .filter_map(|a| self.verify_samples.get(a))
                    .map(Vec::len)
                    .min()
                    .unwrap_or(0);
                let frac = (min_count as f32 / WIZARD_MIN_VERIFY_SAMPLES as f32).min(1.0);
                0.7 + frac * 0.3
            }
            CalibrationState::Complete => 1.0,
            CalibrationState::Failed(_) => 0.0,
        }
    }

    /// Reference to the device being calibrated.
    pub fn device_id(&self) -> &DeviceId {
        &self.device_id
    }

    // ── Private helpers ──────────────────────────────────────────────────

    fn reset_samples(&mut self) {
        for samples in self.center_samples.values_mut() {
            samples.clear();
        }
        for samples in self.range_samples.values_mut() {
            samples.clear();
        }
        for samples in self.verify_samples.values_mut() {
            samples.clear();
        }
        self.computed_centers.clear();
        self.current_range_axis_idx = 0;
    }

    fn advance_center(&mut self) {
        let all_ready = self.axes.iter().all(|a| {
            self.center_samples
                .get(a)
                .is_some_and(|s| s.len() >= WIZARD_MIN_CENTER_SAMPLES)
        });
        if !all_ready {
            return;
        }
        // Compute statistical median for each axis.
        let axes_snapshot: Vec<AxisId> = self.axes.clone();
        for &axis in &axes_snapshot {
            if let Some(samples) = self.center_samples.get_mut(&axis) {
                let center = compute_median(samples);
                self.computed_centers.insert(axis, center);
            }
        }
        self.current_range_axis_idx = 0;
        self.state = CalibrationState::RangeDetection(self.axes[0]);
    }

    fn advance_range(&mut self) {
        let current_axis = self.axes[self.current_range_axis_idx];
        let ready = self
            .range_samples
            .get(&current_axis)
            .is_some_and(|s| s.len() >= WIZARD_MIN_RANGE_SAMPLES);
        if !ready {
            return;
        }
        self.current_range_axis_idx += 1;
        if self.current_range_axis_idx < self.axes.len() {
            self.state = CalibrationState::RangeDetection(self.axes[self.current_range_axis_idx]);
        } else {
            self.state = CalibrationState::Verification;
        }
    }

    fn advance_verification(&mut self) {
        let all_ready = self.axes.iter().all(|a| {
            self.verify_samples
                .get(a)
                .is_some_and(|s| s.len() >= WIZARD_MIN_VERIFY_SAMPLES)
        });
        if !all_ready {
            return;
        }

        let mut axes_data = HashMap::new();
        let mut total_symmetry = 0.0_f32;
        let axes_snapshot: Vec<AxisId> = self.axes.clone();

        for &axis in &axes_snapshot {
            match self.compute_axis_result(axis) {
                Ok((data, sym)) => {
                    total_symmetry += sym;
                    axes_data.insert(axis, data);
                }
                Err(msg) => {
                    self.state = CalibrationState::Failed(msg);
                    return;
                }
            }
        }

        let axis_count = axes_snapshot.len() as f32;
        let symmetry_score = if axis_count > 0.0 {
            total_symmetry / axis_count
        } else {
            0.0
        };
        let quality_score = compute_quality_score(&axes_data, &axes_snapshot, symmetry_score);

        let cal_result = CalibrationResult {
            axes: axes_data,
            symmetry_score,
            quality_score,
            created_at: Instant::now(),
        };

        if quality_score < MIN_QUALITY_THRESHOLD {
            self.result = Some(cal_result);
            self.state = CalibrationState::Failed(format!(
                "quality score {quality_score:.1} below threshold {MIN_QUALITY_THRESHOLD}"
            ));
        } else {
            self.result = Some(cal_result);
            self.state = CalibrationState::Complete;
        }
    }

    /// Compute per-axis calibration data and symmetry score.
    fn compute_axis_result(&mut self, axis: AxisId) -> Result<(AxisCalibrationData, f32), String> {
        let center = *self
            .computed_centers
            .get(&axis)
            .ok_or_else(|| format!("missing center for {axis}"))?;

        let range_samples = self
            .range_samples
            .get_mut(&axis)
            .ok_or_else(|| format!("no range samples for {axis}"))?;
        if range_samples.is_empty() {
            return Err(format!("no range samples for {axis}"));
        }

        let min_value = compute_percentile(range_samples, PERCENTILE_LOW);
        let max_value = compute_percentile(range_samples, PERCENTILE_HIGH);
        if min_value >= max_value {
            return Err(format!(
                "degenerate range for {axis}: min={min_value} max={max_value}"
            ));
        }

        let center_samples = self
            .center_samples
            .get(&axis)
            .ok_or_else(|| format!("missing center samples for {axis}"))?;

        let noise_floor = compute_std_dev(center_samples, center);
        let half_range = (max_value - min_value) as f32 / 2.0;
        let deadzone = if half_range > 0.0 {
            (noise_floor * 3.0 / half_range).min(0.5)
        } else {
            0.0
        };

        // Symmetry: ratio of shorter side to longer side around center.
        let low_range = center.saturating_sub(min_value) as f32;
        let high_range = max_value.saturating_sub(center) as f32;
        let symmetry = if low_range > 0.0 || high_range > 0.0 {
            low_range.min(high_range) / low_range.max(high_range)
        } else {
            0.0
        };

        Ok((
            AxisCalibrationData {
                center_value: center,
                min_value,
                max_value,
                deadzone_recommendation: deadzone,
                noise_floor,
            },
            symmetry,
        ))
    }
}

// ── Calibration Algorithms ───────────────────────────────────────────────

/// Compute the statistical median (sorts in-place).
fn compute_median(samples: &mut [u32]) -> u32 {
    samples.sort_unstable();
    let n = samples.len();
    if n == 0 {
        return 0;
    }
    if n.is_multiple_of(2) {
        let a = samples[n / 2 - 1] as u64;
        let b = samples[n / 2] as u64;
        ((a + b) / 2) as u32
    } else {
        samples[n / 2]
    }
}

/// Compute a percentile value with outlier rejection (sorts in-place).
fn compute_percentile(samples: &mut [u32], percentile: f64) -> u32 {
    samples.sort_unstable();
    let n = samples.len();
    if n == 0 {
        return 0;
    }
    let idx = ((percentile / 100.0) * (n - 1) as f64).round() as usize;
    samples[idx.min(n - 1)]
}

/// Standard deviation of samples relative to a known center value.
fn compute_std_dev(samples: &[u32], center: u32) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let n = samples.len() as f64;
    let sum_sq: f64 = samples
        .iter()
        .map(|&s| {
            let diff = f64::from(s) - f64::from(center);
            diff * diff
        })
        .sum();
    (sum_sq / n).sqrt() as f32
}

/// Compute an overall quality score (0–100) from calibration data.
///
/// Penalties:
/// - Narrow range (< 1000 raw counts): −15 per axis
/// - High noise-to-range ratio (> 1%): up to −20 per axis
/// - Excessive deadzone (> 0.15): −10 per axis
/// - Asymmetry: up to −30
fn compute_quality_score(
    axes: &HashMap<AxisId, AxisCalibrationData>,
    axis_order: &[AxisId],
    symmetry_score: f32,
) -> f32 {
    if axes.is_empty() {
        return 0.0;
    }

    let mut score = 100.0_f32;

    for &axis_id in axis_order {
        if let Some(data) = axes.get(&axis_id) {
            let range = data.max_value.saturating_sub(data.min_value) as f32;

            // Penalise narrow ranges.
            if range < 1000.0 {
                score -= 15.0;
            }
            // Penalise high noise relative to range.
            if range > 0.0 {
                let noise_ratio = data.noise_floor / range;
                if noise_ratio > 0.01 {
                    score -= (noise_ratio * 500.0).min(20.0);
                }
            }
            // Penalise excessive deadzone.
            if data.deadzone_recommendation > 0.15 {
                score -= 10.0;
            }
        }
    }

    // Penalise asymmetry.
    score -= (1.0 - symmetry_score) * 30.0;

    score.clamp(0.0, 100.0)
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_device() -> DeviceId {
        DeviceId::from_vid_pid(0x044F, 0x0402)
    }

    fn single_axis() -> Vec<AxisId> {
        vec![AxisId(0)]
    }

    fn two_axes() -> Vec<AxisId> {
        vec![AxisId(0), AxisId(1)]
    }

    /// Helper: feed `n` center samples at `center ± jitter` for all axes.
    fn feed_center(wiz: &mut CalibrationWizard, axes: &[AxisId], center: u32, n: usize) {
        for i in 0..n {
            for &axis in axes {
                // Deterministic jitter: ±2 raw counts.
                let jitter = ((i * 7 + axis.0 as usize) % 5) as u32;
                wiz.record_sample(axis, center + jitter - 2);
            }
        }
    }

    /// Helper: feed linearly-spaced range samples for a single axis.
    fn feed_range(wiz: &mut CalibrationWizard, axis: AxisId, min: u32, max: u32, n: usize) {
        for i in 0..n {
            let value = min + ((max - min) as u64 * i as u64 / (n - 1).max(1) as u64) as u32;
            wiz.record_sample(axis, value);
        }
    }

    /// Helper: feed verification samples for all axes.
    fn feed_verify(wiz: &mut CalibrationWizard, axes: &[AxisId], center: u32, n: usize) {
        for i in 0..n {
            for &axis in axes {
                wiz.record_sample(axis, center + (i as u32 % 100));
            }
        }
    }

    /// Drive a single-axis wizard to Complete.
    fn drive_single_axis_complete() -> CalibrationWizard {
        let axes = single_axis();
        let mut wiz = CalibrationWizard::new(test_device(), axes.clone());
        wiz.start();

        // Center detection
        feed_center(&mut wiz, &axes, 32768, WIZARD_MIN_CENTER_SAMPLES);
        wiz.advance();

        // Range detection for axis 0
        feed_range(&mut wiz, AxisId(0), 0, 65535, WIZARD_MIN_RANGE_SAMPLES);
        wiz.advance();

        // Verification
        feed_verify(&mut wiz, &axes, 32768, WIZARD_MIN_VERIFY_SAMPLES);
        wiz.advance();

        wiz
    }

    // ── Full calibration cycle ───────────────────────────────────────────

    #[test]
    fn full_cycle_single_axis() {
        let wiz = drive_single_axis_complete();
        assert_eq!(wiz.current_step(), CalibrationState::Complete);

        let result = wiz.result().expect("result on Complete");
        assert!(result.quality_score > 0.0);
        assert!(result.symmetry_score > 0.0);

        let data = result.axes.get(&AxisId(0)).expect("axis 0 data");
        assert!(data.min_value < data.center_value);
        assert!(data.center_value < data.max_value);
    }

    #[test]
    fn full_cycle_two_axes() {
        let axes = two_axes();
        let mut wiz = CalibrationWizard::new(test_device(), axes.clone());
        wiz.start();
        assert_eq!(wiz.current_step(), CalibrationState::CenterDetection);

        // Center: both axes
        feed_center(&mut wiz, &axes, 32768, WIZARD_MIN_CENTER_SAMPLES);
        wiz.advance();
        assert_eq!(
            wiz.current_step(),
            CalibrationState::RangeDetection(AxisId(0))
        );

        // Range: axis 0
        feed_range(&mut wiz, AxisId(0), 0, 65535, WIZARD_MIN_RANGE_SAMPLES);
        wiz.advance();
        assert_eq!(
            wiz.current_step(),
            CalibrationState::RangeDetection(AxisId(1))
        );

        // Range: axis 1
        feed_range(&mut wiz, AxisId(1), 0, 65535, WIZARD_MIN_RANGE_SAMPLES);
        wiz.advance();
        assert_eq!(wiz.current_step(), CalibrationState::Verification);

        // Verify
        feed_verify(&mut wiz, &axes, 32768, WIZARD_MIN_VERIFY_SAMPLES);
        wiz.advance();
        assert_eq!(wiz.current_step(), CalibrationState::Complete);

        let result = wiz.result().expect("result");
        assert!(result.axes.contains_key(&AxisId(0)));
        assert!(result.axes.contains_key(&AxisId(1)));
    }

    // ── Noisy input handling ─────────────────────────────────────────────

    #[test]
    fn center_detection_with_jitter() {
        let axes = single_axis();
        let mut wiz = CalibrationWizard::new(test_device(), axes.clone());
        wiz.start();

        // Feed center samples with deterministic jitter ±3
        for i in 0..WIZARD_MIN_CENTER_SAMPLES {
            let jitter = (i % 7) as u32;
            wiz.record_sample(AxisId(0), 32768 + jitter - 3);
        }
        wiz.advance();

        // Should still detect center correctly (within a few counts).
        assert_eq!(
            wiz.current_step(),
            CalibrationState::RangeDetection(AxisId(0))
        );

        // Complete the rest of the calibration.
        feed_range(&mut wiz, AxisId(0), 0, 65535, WIZARD_MIN_RANGE_SAMPLES);
        wiz.advance();
        feed_verify(&mut wiz, &axes, 32768, WIZARD_MIN_VERIFY_SAMPLES);
        wiz.advance();

        let result = wiz.result().expect("result");
        let data = result.axes.get(&AxisId(0)).unwrap();
        // Center should be very close to 32768 despite jitter.
        assert!(
            (data.center_value as i64 - 32768).unsigned_abs() < 5,
            "center {} too far from 32768",
            data.center_value
        );
        // Noise floor should be small but non-zero.
        assert!(data.noise_floor > 0.0, "noise floor should be non-zero");
        assert!(data.noise_floor < 10.0, "noise floor should be small");
    }

    // ── Asymmetric range detection ───────────────────────────────────────

    #[test]
    fn asymmetric_range_lowers_symmetry_score() {
        let axes = single_axis();
        let mut wiz = CalibrationWizard::new(test_device(), axes.clone());
        wiz.start();

        feed_center(&mut wiz, &axes, 32768, WIZARD_MIN_CENTER_SAMPLES);
        wiz.advance();

        // Asymmetric range: center at ~32768, min 16384 (16k below), max 65535 (33k above)
        for i in 0..WIZARD_MIN_RANGE_SAMPLES {
            let value = 16384
                + ((65535u64 - 16384) * i as u64 / (WIZARD_MIN_RANGE_SAMPLES - 1) as u64) as u32;
            wiz.record_sample(AxisId(0), value);
        }
        wiz.advance();

        feed_verify(&mut wiz, &axes, 32768, WIZARD_MIN_VERIFY_SAMPLES);
        wiz.advance();

        let result = wiz.result().expect("result");
        // Symmetry should be noticeably less than 1.0 due to asymmetric range.
        assert!(
            result.symmetry_score < 0.9,
            "symmetry {} should be < 0.9 for asymmetric range",
            result.symmetry_score
        );
        // Quality score should be penalised.
        assert!(
            result.quality_score < 100.0,
            "quality should be less than 100 for asymmetric input"
        );
    }

    // ── Deadzone recommendation accuracy ─────────────────────────────────

    #[test]
    fn deadzone_scales_with_noise() {
        // Low-noise calibration
        let low_noise = run_with_noise(1);
        // High-noise calibration
        let high_noise = run_with_noise(50);

        assert!(
            high_noise.deadzone_recommendation > low_noise.deadzone_recommendation,
            "high noise ({}) should recommend larger deadzone than low noise ({})",
            high_noise.deadzone_recommendation,
            low_noise.deadzone_recommendation
        );
    }

    /// Helper: run a full calibration with specified noise amplitude.
    fn run_with_noise(noise_amp: u32) -> AxisCalibrationData {
        let axes = single_axis();
        let mut wiz = CalibrationWizard::new(test_device(), axes.clone());
        wiz.start();

        // Center with controlled noise
        for i in 0..WIZARD_MIN_CENTER_SAMPLES {
            let jitter = (i as u32 % (noise_amp * 2 + 1)) as i64 - noise_amp as i64;
            let value = (32768i64 + jitter).max(0) as u32;
            wiz.record_sample(AxisId(0), value);
        }
        wiz.advance();

        feed_range(&mut wiz, AxisId(0), 0, 65535, WIZARD_MIN_RANGE_SAMPLES);
        wiz.advance();

        feed_verify(&mut wiz, &axes, 32768, WIZARD_MIN_VERIFY_SAMPLES);
        wiz.advance();

        wiz.result()
            .expect("result")
            .axes
            .get(&AxisId(0))
            .expect("axis data")
            .clone()
    }

    #[test]
    fn deadzone_is_reasonable_fraction() {
        let wiz = drive_single_axis_complete();
        let result = wiz.result().unwrap();
        let data = result.axes.get(&AxisId(0)).unwrap();

        // Deadzone should be a small fraction of the range.
        assert!(
            data.deadzone_recommendation < 0.1,
            "deadzone {} too large for clean input",
            data.deadzone_recommendation
        );
        assert!(
            data.deadzone_recommendation >= 0.0,
            "deadzone must be non-negative"
        );
    }

    // ── State machine transitions ────────────────────────────────────────

    #[test]
    fn initial_state_is_idle() {
        let wiz = CalibrationWizard::new(test_device(), single_axis());
        assert_eq!(wiz.current_step(), CalibrationState::Idle);
        assert!(wiz.result().is_none());
    }

    #[test]
    fn start_transitions_to_center_detection() {
        let mut wiz = CalibrationWizard::new(test_device(), single_axis());
        wiz.start();
        assert_eq!(wiz.current_step(), CalibrationState::CenterDetection);
    }

    #[test]
    fn start_with_no_axes_fails() {
        let mut wiz = CalibrationWizard::new(test_device(), vec![]);
        wiz.start();
        assert!(
            matches!(wiz.current_step(), CalibrationState::Failed(_)),
            "start with no axes should fail"
        );
    }

    #[test]
    fn advance_idle_does_nothing() {
        let mut wiz = CalibrationWizard::new(test_device(), single_axis());
        wiz.advance();
        assert_eq!(wiz.current_step(), CalibrationState::Idle);
    }

    #[test]
    fn advance_without_enough_samples_stays() {
        let mut wiz = CalibrationWizard::new(test_device(), single_axis());
        wiz.start();

        // Feed fewer than required center samples.
        for i in 0..(WIZARD_MIN_CENTER_SAMPLES - 1) {
            wiz.record_sample(AxisId(0), 32768 + i as u32);
        }
        wiz.advance();
        assert_eq!(
            wiz.current_step(),
            CalibrationState::CenterDetection,
            "should stay in CenterDetection without enough samples"
        );
    }

    #[test]
    fn samples_ignored_in_idle() {
        let mut wiz = CalibrationWizard::new(test_device(), single_axis());
        wiz.record_sample(AxisId(0), 32768);
        // No crash, no state change.
        assert_eq!(wiz.current_step(), CalibrationState::Idle);
    }

    #[test]
    fn samples_ignored_after_complete() {
        let mut wiz = drive_single_axis_complete();
        assert_eq!(wiz.current_step(), CalibrationState::Complete);
        wiz.record_sample(AxisId(0), 12345);
        // Should still be Complete, no crash.
        assert_eq!(wiz.current_step(), CalibrationState::Complete);
    }

    #[test]
    fn unknown_axis_samples_ignored() {
        let mut wiz = CalibrationWizard::new(test_device(), single_axis());
        wiz.start();
        // AxisId(99) is not configured.
        wiz.record_sample(AxisId(99), 32768);
        // Should not crash or affect state.
        assert_eq!(wiz.current_step(), CalibrationState::CenterDetection);
    }

    #[test]
    fn range_only_records_current_axis() {
        let axes = two_axes();
        let mut wiz = CalibrationWizard::new(test_device(), axes.clone());
        wiz.start();

        feed_center(&mut wiz, &axes, 32768, WIZARD_MIN_CENTER_SAMPLES);
        wiz.advance();
        assert_eq!(
            wiz.current_step(),
            CalibrationState::RangeDetection(AxisId(0))
        );

        // Feeding axis 1 data during axis 0 range detection should be ignored.
        for _ in 0..WIZARD_MIN_RANGE_SAMPLES {
            wiz.record_sample(AxisId(1), 32768);
        }
        wiz.advance();
        // Should still be on axis 0 since axis 1 data is ignored.
        assert_eq!(
            wiz.current_step(),
            CalibrationState::RangeDetection(AxisId(0)),
            "feeding wrong axis should not advance"
        );
    }

    // ── Progress tracking ────────────────────────────────────────────────

    #[test]
    fn progress_starts_at_zero() {
        let wiz = CalibrationWizard::new(test_device(), single_axis());
        assert!((wiz.progress() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn progress_increases_through_steps() {
        let axes = single_axis();
        let mut wiz = CalibrationWizard::new(test_device(), axes.clone());
        let p0 = wiz.progress();

        wiz.start();
        feed_center(&mut wiz, &axes, 32768, WIZARD_MIN_CENTER_SAMPLES);
        let p1 = wiz.progress();
        assert!(p1 > p0, "progress should increase during center: {p1}");

        wiz.advance();
        let p2 = wiz.progress();
        assert!(
            p2 >= p1,
            "progress should not decrease after center→range: {p2} vs {p1}"
        );

        feed_range(&mut wiz, AxisId(0), 0, 65535, WIZARD_MIN_RANGE_SAMPLES);
        let p3 = wiz.progress();
        assert!(p3 > p2, "progress should increase during range: {p3}");

        wiz.advance();
        feed_verify(&mut wiz, &axes, 32768, WIZARD_MIN_VERIFY_SAMPLES);
        let p4 = wiz.progress();
        assert!(p4 > p3, "progress should increase during verify: {p4}");

        wiz.advance();
        let p5 = wiz.progress();
        assert!(
            (p5 - 1.0).abs() < f32::EPSILON,
            "complete should be 1.0: {p5}"
        );
    }

    #[test]
    fn progress_is_zero_on_failure() {
        let mut wiz = CalibrationWizard::new(test_device(), vec![]);
        wiz.start();
        assert!(
            (wiz.progress() - 0.0).abs() < f32::EPSILON,
            "failed state should report 0.0 progress"
        );
    }

    // ── Abort mid-calibration ────────────────────────────────────────────

    #[test]
    fn abort_returns_to_idle() {
        let axes = single_axis();
        let mut wiz = CalibrationWizard::new(test_device(), axes.clone());
        wiz.start();
        feed_center(&mut wiz, &axes, 32768, WIZARD_MIN_CENTER_SAMPLES);
        wiz.advance();
        assert_eq!(
            wiz.current_step(),
            CalibrationState::RangeDetection(AxisId(0))
        );

        wiz.abort();
        assert_eq!(wiz.current_step(), CalibrationState::Idle);
        assert!(wiz.result().is_none());
        assert!((wiz.progress() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn abort_during_center_detection() {
        let mut wiz = CalibrationWizard::new(test_device(), single_axis());
        wiz.start();
        wiz.record_sample(AxisId(0), 32768);
        wiz.abort();
        assert_eq!(wiz.current_step(), CalibrationState::Idle);
    }

    #[test]
    fn abort_then_restart_works() {
        let axes = single_axis();
        let mut wiz = CalibrationWizard::new(test_device(), axes.clone());
        wiz.start();
        feed_center(&mut wiz, &axes, 32768, WIZARD_MIN_CENTER_SAMPLES);
        wiz.abort();

        // Restart and complete.
        wiz.start();
        assert_eq!(wiz.current_step(), CalibrationState::CenterDetection);

        feed_center(&mut wiz, &axes, 32768, WIZARD_MIN_CENTER_SAMPLES);
        wiz.advance();
        feed_range(&mut wiz, AxisId(0), 0, 65535, WIZARD_MIN_RANGE_SAMPLES);
        wiz.advance();
        feed_verify(&mut wiz, &axes, 32768, WIZARD_MIN_VERIFY_SAMPLES);
        wiz.advance();
        assert_eq!(wiz.current_step(), CalibrationState::Complete);
    }

    // ── to_profile_config ────────────────────────────────────────────────

    #[test]
    fn to_profile_config_produces_valid_axis_calibration() {
        let wiz = drive_single_axis_complete();
        let result = wiz.result().unwrap();
        let configs = result.to_profile_config();

        assert_eq!(configs.len(), 1);
        let (axis_id, cal) = &configs[0];
        assert_eq!(*axis_id, AxisId(0));
        assert!(cal.raw_min < cal.raw_center);
        assert!(cal.raw_center < cal.raw_max);
        assert!(cal.deadzone >= 0.0);
        assert!(cal.deadzone < 1.0);
        assert!((cal.output_min - (-1.0)).abs() < f32::EPSILON);
        assert!((cal.output_max - 1.0).abs() < f32::EPSILON);
        assert!(!cal.reversed);
    }

    #[test]
    fn profile_config_normalizes_correctly() {
        let wiz = drive_single_axis_complete();
        let result = wiz.result().unwrap();
        let configs = result.to_profile_config();
        let (_, cal) = &configs[0];

        // Normalizing the center should produce ~0.0.
        let center_out = cal.normalize(cal.raw_center);
        assert!(
            center_out.abs() < 0.15,
            "center should normalize near 0.0, got {center_out}"
        );

        // Normalizing extremes should be near ±1.0.
        let min_out = cal.normalize(cal.raw_min);
        let max_out = cal.normalize(cal.raw_max);
        assert!(
            min_out < -0.5,
            "min should normalize below -0.5, got {min_out}"
        );
        assert!(
            max_out > 0.5,
            "max should normalize above 0.5, got {max_out}"
        );
    }

    // ── Algorithm unit tests ─────────────────────────────────────────────

    #[test]
    fn median_odd_count() {
        let mut samples = vec![5, 1, 3, 2, 4];
        assert_eq!(compute_median(&mut samples), 3);
    }

    #[test]
    fn median_even_count() {
        let mut samples = vec![1, 2, 3, 4];
        assert_eq!(compute_median(&mut samples), 2); // (2+3)/2 = 2 in integer
    }

    #[test]
    fn median_single() {
        let mut samples = vec![42];
        assert_eq!(compute_median(&mut samples), 42);
    }

    #[test]
    fn median_empty() {
        let mut samples: Vec<u32> = vec![];
        assert_eq!(compute_median(&mut samples), 0);
    }

    #[test]
    fn percentile_endpoints() {
        let mut samples: Vec<u32> = (0..100).collect();
        assert_eq!(compute_percentile(&mut samples, 0.0), 0);
        assert_eq!(compute_percentile(&mut samples, 100.0), 99);
    }

    #[test]
    fn percentile_outlier_rejection() {
        // 98 samples in [1000, 2000], plus 1 outlier at 0 and 1 at 65535.
        let mut samples: Vec<u32> = (0..98).map(|i| 1000 + i * 10).collect();
        samples.push(0);
        samples.push(65535);

        let p1 = compute_percentile(&mut samples, PERCENTILE_LOW);
        let p99 = compute_percentile(&mut samples, PERCENTILE_HIGH);

        // The 1st percentile should reject the 0 outlier.
        assert!(p1 > 0, "1st percentile should reject low outlier, got {p1}");
        // The 99th percentile should reject the 65535 outlier.
        assert!(
            p99 < 65535,
            "99th percentile should reject high outlier, got {p99}"
        );
    }

    #[test]
    fn std_dev_zero_for_constant() {
        let samples = vec![100; 50];
        let sd = compute_std_dev(&samples, 100);
        assert!((sd - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn std_dev_positive_for_spread() {
        let samples: Vec<u32> = (0..100).collect();
        let sd = compute_std_dev(&samples, 50);
        assert!(sd > 0.0, "std dev should be positive for spread data");
    }
}
