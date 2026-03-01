// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Per-axis calibration for Thrustmaster HID devices.
//!
//! Thrustmaster sticks and throttles report raw integer axis values that
//! may vary between individual units due to potentiometer tolerances,
//! gimbal alignment, and wear. This module provides:
//!
//! - [`AxisCalibration`]: per-axis min/max/center calibration data
//! - [`CalibrationProfile`]: a named collection of axis calibrations
//! - [`apply_calibration`]: normalize a raw u16 to −1.0 … 1.0 using a
//!   calibration entry
//! - [`detect_center_drift`]: compute the drift of a set of center samples
//!   from the nominal midpoint
//! - [`auto_calibrate`]: derive an [`AxisCalibration`] from observed
//!   (min, max) sample pairs
//!
//! # Example
//!
//! ```
//! use flight_hotas_thrustmaster::calibration::{AxisCalibration, apply_calibration};
//!
//! let cal = AxisCalibration { min: 100, max: 65400, center: 32750 };
//! let value = apply_calibration(32750, &cal);
//! assert!(value.abs() < 0.01, "center should map to ~0.0");
//! ```

/// Per-axis calibration data.
///
/// `min` and `max` are the observed hardware extremes; `center` is the
/// resting / spring-return position. [`apply_calibration`] maps values
/// in `[min, center]` to `[-1.0, 0.0]` and `[center, max]` to `[0.0, 1.0]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AxisCalibration {
    /// Lowest raw value observed at the physical stop.
    pub min: u16,
    /// Highest raw value observed at the physical stop.
    pub max: u16,
    /// Raw value at the resting / center position.
    pub center: u16,
}

impl AxisCalibration {
    /// Default calibration assuming full u16 range with center at midpoint.
    pub const fn default_u16() -> Self {
        Self {
            min: 0,
            max: 65535,
            center: 32768,
        }
    }

    /// Default calibration for 14-bit axes (T.16000M HALL sensors).
    pub const fn default_14bit() -> Self {
        Self {
            min: 0,
            max: 16383,
            center: 8192,
        }
    }
}

/// A named set of axis calibrations for a single device.
#[derive(Debug, Clone)]
pub struct CalibrationProfile {
    /// Human-readable profile name (e.g. `"My Warthog #1"`).
    pub name: String,
    /// Per-axis calibration entries keyed by axis ID.
    pub axes: Vec<(String, AxisCalibration)>,
}

impl CalibrationProfile {
    /// Create a new empty calibration profile.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            axes: Vec::new(),
        }
    }

    /// Add a calibration entry for the given axis.
    pub fn add_axis(&mut self, axis_id: impl Into<String>, cal: AxisCalibration) {
        self.axes.push((axis_id.into(), cal));
    }

    /// Look up the calibration for a given axis ID.
    pub fn get(&self, axis_id: &str) -> Option<&AxisCalibration> {
        self.axes
            .iter()
            .find(|(id, _)| id == axis_id)
            .map(|(_, c)| c)
    }
}

/// Normalize a raw u16 axis value to −1.0 … 1.0 using the given calibration.
///
/// Values below `cal.center` map linearly to `[-1.0, 0.0]`.
/// Values above `cal.center` map linearly to `[0.0, 1.0]`.
/// The result is clamped to `[-1.0, 1.0]`.
pub fn apply_calibration(raw: u16, cal: &AxisCalibration) -> f64 {
    let raw = raw.clamp(cal.min, cal.max);
    let result = if raw <= cal.center {
        let span = (cal.center - cal.min) as f64;
        if span == 0.0 {
            0.0
        } else {
            (raw as f64 - cal.center as f64) / span
        }
    } else {
        let span = (cal.max - cal.center) as f64;
        if span == 0.0 {
            0.0
        } else {
            (raw as f64 - cal.center as f64) / span
        }
    };
    result.clamp(-1.0, 1.0)
}

/// Detect center drift from a set of resting-position samples.
///
/// Returns the signed offset of the sample mean from `nominal_center`
/// as a fraction of the full u16 range (0.0 = no drift, positive = drifted
/// upward).
///
/// Returns `0.0` if the sample slice is empty.
pub fn detect_center_drift(samples: &[u16], nominal_center: u16) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f64 = samples.iter().map(|&s| s as f64).sum();
    let mean = sum / samples.len() as f64;
    (mean - nominal_center as f64) / 65535.0
}

/// Derive an [`AxisCalibration`] from observed (min, max) sample pairs.
///
/// Each element of `samples` is a `(min_seen, max_seen)` pair from a
/// single sweep. The function takes the global minimum and maximum
/// across all samples and infers the center as the midpoint.
///
/// Returns `None` if the slice is empty.
pub fn auto_calibrate(samples: &[(u16, u16)]) -> Option<AxisCalibration> {
    if samples.is_empty() {
        return None;
    }
    let global_min = samples.iter().map(|&(lo, _)| lo).min().unwrap();
    let global_max = samples.iter().map(|&(_, hi)| hi).max().unwrap();
    let center = ((global_min as u32 + global_max as u32) / 2) as u16;
    Some(AxisCalibration {
        min: global_min,
        max: global_max,
        center,
    })
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── AxisCalibration defaults ─────────────────────────────────────────

    #[test]
    fn default_u16_calibration() {
        let cal = AxisCalibration::default_u16();
        assert_eq!(cal.min, 0);
        assert_eq!(cal.max, 65535);
        assert_eq!(cal.center, 32768);
    }

    #[test]
    fn default_14bit_calibration() {
        let cal = AxisCalibration::default_14bit();
        assert_eq!(cal.min, 0);
        assert_eq!(cal.max, 16383);
        assert_eq!(cal.center, 8192);
    }

    // ── apply_calibration ────────────────────────────────────────────────

    #[test]
    fn calibration_center_maps_to_zero() {
        let cal = AxisCalibration::default_u16();
        let v = apply_calibration(32768, &cal);
        assert!(v.abs() < 0.001, "center → 0.0, got {v}");
    }

    #[test]
    fn calibration_min_maps_to_neg_one() {
        let cal = AxisCalibration::default_u16();
        let v = apply_calibration(0, &cal);
        assert!((v - (-1.0)).abs() < 0.001, "min → -1.0, got {v}");
    }

    #[test]
    fn calibration_max_maps_to_pos_one() {
        let cal = AxisCalibration::default_u16();
        let v = apply_calibration(65535, &cal);
        assert!((v - 1.0).abs() < 0.001, "max → 1.0, got {v}");
    }

    #[test]
    fn calibration_midpoint_below_center() {
        let cal = AxisCalibration::default_u16();
        let v = apply_calibration(16384, &cal);
        assert!(v < 0.0 && v > -1.0, "below center → negative, got {v}");
    }

    #[test]
    fn calibration_midpoint_above_center() {
        let cal = AxisCalibration::default_u16();
        let v = apply_calibration(49152, &cal);
        assert!(v > 0.0 && v < 1.0, "above center → positive, got {v}");
    }

    #[test]
    fn calibration_clamps_below_min() {
        let cal = AxisCalibration {
            min: 1000,
            max: 64000,
            center: 32000,
        };
        let v = apply_calibration(500, &cal);
        assert!(
            (v - (-1.0)).abs() < 0.001,
            "below min clamps to -1.0, got {v}"
        );
    }

    #[test]
    fn calibration_clamps_above_max() {
        let cal = AxisCalibration {
            min: 1000,
            max: 64000,
            center: 32000,
        };
        let v = apply_calibration(65000, &cal);
        assert!((v - 1.0).abs() < 0.001, "above max clamps to 1.0, got {v}");
    }

    #[test]
    fn calibration_asymmetric_ranges() {
        let cal = AxisCalibration {
            min: 100,
            max: 65400,
            center: 30000,
        };
        let at_center = apply_calibration(30000, &cal);
        assert!(at_center.abs() < 0.001, "center → 0.0, got {at_center}");

        let at_min = apply_calibration(100, &cal);
        assert!((at_min - (-1.0)).abs() < 0.001, "min → -1.0, got {at_min}");

        let at_max = apply_calibration(65400, &cal);
        assert!((at_max - 1.0).abs() < 0.001, "max → 1.0, got {at_max}");
    }

    #[test]
    fn calibration_zero_span_below_center() {
        let cal = AxisCalibration {
            min: 32000,
            max: 65535,
            center: 32000,
        };
        let v = apply_calibration(32000, &cal);
        assert!(v.abs() < 0.001, "degenerate lower span → 0.0, got {v}");
    }

    #[test]
    fn calibration_zero_span_above_center() {
        let cal = AxisCalibration {
            min: 0,
            max: 32000,
            center: 32000,
        };
        let v = apply_calibration(32000, &cal);
        assert!(v.abs() < 0.001, "degenerate upper span → 0.0, got {v}");
    }

    #[test]
    fn calibration_14bit_center() {
        let cal = AxisCalibration::default_14bit();
        let v = apply_calibration(8192, &cal);
        assert!(v.abs() < 0.001, "14-bit center → 0.0, got {v}");
    }

    #[test]
    fn calibration_14bit_extremes() {
        let cal = AxisCalibration::default_14bit();
        let lo = apply_calibration(0, &cal);
        let hi = apply_calibration(16383, &cal);
        assert!((lo - (-1.0)).abs() < 0.001, "14-bit min → -1.0, got {lo}");
        assert!((hi - 1.0).abs() < 0.001, "14-bit max → 1.0, got {hi}");
    }

    // ── detect_center_drift ──────────────────────────────────────────────

    #[test]
    fn no_drift_at_nominal_center() {
        let samples = vec![32768, 32768, 32768, 32768];
        let drift = detect_center_drift(&samples, 32768);
        assert!(drift.abs() < 1e-6, "no drift expected, got {drift}");
    }

    #[test]
    fn positive_drift_detected() {
        let samples = vec![33000, 33000, 33000];
        let drift = detect_center_drift(&samples, 32768);
        assert!(drift > 0.0, "expected positive drift, got {drift}");
    }

    #[test]
    fn negative_drift_detected() {
        let samples = vec![32000, 32000, 32000];
        let drift = detect_center_drift(&samples, 32768);
        assert!(drift < 0.0, "expected negative drift, got {drift}");
    }

    #[test]
    fn drift_empty_samples_returns_zero() {
        assert_eq!(detect_center_drift(&[], 32768), 0.0);
    }

    #[test]
    fn drift_magnitude_is_reasonable() {
        // ~500 counts off center on a 65535 range ≈ 0.76%
        let samples = vec![33268; 100];
        let drift = detect_center_drift(&samples, 32768);
        assert!(
            (drift - 500.0 / 65535.0).abs() < 1e-6,
            "drift should be ~0.0076, got {drift}"
        );
    }

    // ── auto_calibrate ───────────────────────────────────────────────────

    #[test]
    fn auto_calibrate_single_sweep() {
        let samples = vec![(100, 65400)];
        let cal = auto_calibrate(&samples).unwrap();
        assert_eq!(cal.min, 100);
        assert_eq!(cal.max, 65400);
        assert_eq!(cal.center, 32750);
    }

    #[test]
    fn auto_calibrate_multiple_sweeps() {
        let samples = vec![(200, 65000), (100, 65400), (150, 65200)];
        let cal = auto_calibrate(&samples).unwrap();
        assert_eq!(cal.min, 100);
        assert_eq!(cal.max, 65400);
        assert_eq!(cal.center, 32750);
    }

    #[test]
    fn auto_calibrate_empty_returns_none() {
        assert!(auto_calibrate(&[]).is_none());
    }

    #[test]
    fn auto_calibrate_identical_sweeps() {
        let samples = vec![(1000, 64000), (1000, 64000)];
        let cal = auto_calibrate(&samples).unwrap();
        assert_eq!(cal.min, 1000);
        assert_eq!(cal.max, 64000);
        assert_eq!(cal.center, 32500);
    }

    #[test]
    fn auto_calibrate_narrow_range() {
        let samples = vec![(32000, 33000)];
        let cal = auto_calibrate(&samples).unwrap();
        assert_eq!(cal.min, 32000);
        assert_eq!(cal.max, 33000);
        assert_eq!(cal.center, 32500);
    }

    // ── CalibrationProfile ───────────────────────────────────────────────

    #[test]
    fn profile_add_and_get() {
        let mut profile = CalibrationProfile::new("test");
        profile.add_axis("x", AxisCalibration::default_u16());
        profile.add_axis("y", AxisCalibration::default_14bit());
        assert!(profile.get("x").is_some());
        assert!(profile.get("y").is_some());
        assert!(profile.get("z").is_none());
    }

    #[test]
    fn profile_name() {
        let profile = CalibrationProfile::new("My Warthog");
        assert_eq!(profile.name, "My Warthog");
    }

    // ── Round-trip: auto_calibrate → apply_calibration ───────────────────

    #[test]
    fn roundtrip_auto_calibrate_then_apply() {
        let samples = vec![(500, 65000)];
        let cal = auto_calibrate(&samples).unwrap();

        let at_min = apply_calibration(500, &cal);
        let at_max = apply_calibration(65000, &cal);
        let at_center = apply_calibration(cal.center, &cal);

        assert!((at_min - (-1.0)).abs() < 0.001, "min → -1.0, got {at_min}");
        assert!((at_max - 1.0).abs() < 0.001, "max → 1.0, got {at_max}");
        assert!(at_center.abs() < 0.001, "center → 0.0, got {at_center}");
    }
}
