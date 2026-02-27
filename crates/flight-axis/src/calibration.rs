// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Per-axis calibration maps raw hardware range to normalized output.
//!
//! Raw values are u16 (0..65535 typical), normalized output is f32 in [-1.0, 1.0].
//! Calibration stores min, max, and center to handle non-centered axes.

use std::collections::HashMap;

/// Per-axis calibration maps raw hardware range to normalized output.
///
/// Raw values are u16 (0..65535 typical), normalized output is f32 in [-1.0, 1.0].
/// Calibration stores min, max, and center to handle non-centered axes.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisCalibration {
    /// Minimum raw hardware value.
    pub raw_min: u16,
    /// Maximum raw hardware value.
    pub raw_max: u16,
    /// Center (rest) raw hardware value.
    pub raw_center: u16,
    /// Deadband in raw units around center; values within this window map to 0.0.
    pub deadband_raw: u16,
}

impl AxisCalibration {
    /// Creates a new `AxisCalibration` with no deadband.
    pub fn new(raw_min: u16, raw_max: u16, raw_center: u16) -> Self {
        Self {
            raw_min,
            raw_max,
            raw_center,
            deadband_raw: 0,
        }
    }

    /// Builder: sets the deadband in raw units around center.
    #[must_use]
    pub fn with_deadband(mut self, deadband_raw: u16) -> Self {
        self.deadband_raw = deadband_raw;
        self
    }

    /// Full 16-bit range: min=0, max=65535, center=32767.
    pub fn default_full_range() -> Self {
        Self::new(0, 65535, 32767)
    }

    /// Returns the minimum raw value.
    #[inline]
    pub fn raw_min(&self) -> u16 {
        self.raw_min
    }

    /// Returns the maximum raw value.
    #[inline]
    pub fn raw_max(&self) -> u16 {
        self.raw_max
    }

    /// Returns the center raw value.
    #[inline]
    pub fn raw_center(&self) -> u16 {
        self.raw_center
    }

    /// Normalizes a raw hardware value to `[-1.0, 1.0]`.
    ///
    /// - Degenerate range (min == max) → `0.0`
    /// - Within deadband of center → `0.0`
    /// - Below min → `-1.0`; above max → `1.0`
    /// - Above center: linear interpolation into `[0.0, 1.0]`
    /// - Below center: linear interpolation into `[-1.0, 0.0]`
    #[inline]
    pub fn normalize(&self, raw: u16) -> f32 {
        if self.raw_min == self.raw_max {
            return 0.0;
        }

        // Hard clamp outside the calibrated range.
        if raw > self.raw_max {
            return 1.0;
        }
        if raw < self.raw_min {
            return -1.0;
        }

        let center = self.raw_center as i32;
        let val = raw as i32;
        let half_db = self.deadband_raw as i32;

        // Deadband check around center.
        if (val - center).abs() <= half_db {
            return 0.0;
        }

        if val > center {
            let span = self.raw_max as i32 - center;
            if span <= 0 {
                return 1.0;
            }
            let dist = val - center - half_db;
            let usable = span - half_db;
            if usable <= 0 {
                return 1.0;
            }
            (dist as f32 / usable as f32).clamp(0.0, 1.0)
        } else {
            let span = center - self.raw_min as i32;
            if span <= 0 {
                return -1.0;
            }
            let dist = center - val - half_db;
            let usable = span - half_db;
            if usable <= 0 {
                return -1.0;
            }
            -(dist as f32 / usable as f32).clamp(0.0, 1.0)
        }
    }
}

/// Manages calibration for a collection of named axes.
///
/// Not intended for use on the RT hot path due to `HashMap` heap allocation.
#[derive(Debug, Clone, Default)]
pub struct CalibrationBank {
    calibrations: HashMap<String, AxisCalibration>,
}

impl CalibrationBank {
    /// Creates a new, empty `CalibrationBank`.
    pub fn new() -> Self {
        Self {
            calibrations: HashMap::new(),
        }
    }

    /// Inserts or replaces calibration for the named axis.
    pub fn insert(&mut self, axis: &str, cal: AxisCalibration) {
        self.calibrations.insert(axis.to_string(), cal);
    }

    /// Returns a reference to the `AxisCalibration` for the given axis, if present.
    pub fn get(&self, axis: &str) -> Option<&AxisCalibration> {
        self.calibrations.get(axis)
    }

    /// Normalizes a raw value for the named axis.
    ///
    /// Falls back to `AxisCalibration::default_full_range()` for unknown axes.
    #[inline]
    pub fn normalize(&self, axis: &str, raw: u16) -> f32 {
        match self.calibrations.get(axis) {
            Some(cal) => cal.normalize(raw),
            None => AxisCalibration::default_full_range().normalize(raw),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // ── AxisCalibration unit tests ────────────────────────────────────────────

    #[test]
    fn test_normalize_center_is_zero() {
        let cal = AxisCalibration::default_full_range();
        let out = cal.normalize(32767);
        assert!(out.abs() < 1e-4, "center should be ~0.0, got {out}");
    }

    #[test]
    fn test_normalize_max_is_one() {
        let cal = AxisCalibration::default_full_range();
        assert_eq!(cal.normalize(65535), 1.0);
    }

    #[test]
    fn test_normalize_min_is_neg_one() {
        let cal = AxisCalibration::default_full_range();
        assert_eq!(cal.normalize(0), -1.0);
    }

    #[test]
    fn test_normalize_midpoint_above_center() {
        // mid between 32767 and 65535 ≈ 49151
        let cal = AxisCalibration::default_full_range();
        let out = cal.normalize(49151);
        assert!((out - 0.5).abs() < 0.01, "expected ~0.5, got {out}");
    }

    #[test]
    fn test_normalize_midpoint_below_center() {
        // mid between 0 and 32767 ≈ 16383
        let cal = AxisCalibration::default_full_range();
        let out = cal.normalize(16383);
        assert!((out - (-0.5)).abs() < 0.01, "expected ~-0.5, got {out}");
    }

    #[test]
    fn test_normalize_clamped_above_max() {
        let cal = AxisCalibration::new(100, 1000, 550);
        // raw > max after construction — simulate by calling with a value above max
        // (u16 can't exceed 65535; use a cal with max < 65535 and pass max+1 via direct value)
        // We pass raw_max directly to verify clamp returns 1.0, then confirm raw > max returns 1.0
        assert_eq!(cal.normalize(cal.raw_max), 1.0);
        // The only way to get raw > max with a u16 is if max < 65535; verify value just above
        // Since normalize takes u16 and raw_max is 1000 < 65535, passing 1001 should give 1.0
        assert_eq!(cal.normalize(1001), 1.0);
    }

    #[test]
    fn test_normalize_clamped_below_min() {
        let cal = AxisCalibration::new(100, 1000, 550);
        assert_eq!(cal.normalize(cal.raw_min), -1.0);
        // raw < min (100), e.g. 50
        assert_eq!(cal.normalize(50), -1.0);
    }

    #[test]
    fn test_normalize_deadband_near_center() {
        let cal = AxisCalibration::new(0, 65535, 32767).with_deadband(500);
        // raw = center ± 400, within deadband of 500
        assert_eq!(cal.normalize(32767), 0.0);
        assert_eq!(cal.normalize(32767 + 400), 0.0);
        assert_eq!(cal.normalize(32767 - 400), 0.0);
    }

    #[test]
    fn test_normalize_above_deadband() {
        let cal = AxisCalibration::new(0, 65535, 32767).with_deadband(500);
        // raw just outside the deadband (center + 501)
        let out = cal.normalize(32767 + 501);
        assert!(out > 0.0, "expected positive, got {out}");
        assert!(out <= 1.0, "expected ≤1.0, got {out}");
    }

    #[test]
    fn test_normalize_min_equals_max() {
        let cal = AxisCalibration::new(1000, 1000, 1000);
        assert_eq!(cal.normalize(1000), 0.0);
        assert_eq!(cal.normalize(500), 0.0);
        assert_eq!(cal.normalize(1500), 0.0);
    }

    #[test]
    fn test_normalize_virpil_14bit() {
        // 14-bit device: 0..16383, center 8191
        let cal = AxisCalibration::new(0, 16383, 8191);
        let at_center = cal.normalize(8191);
        assert!(
            at_center.abs() < 1e-3,
            "center should be ~0, got {at_center}"
        );
        let at_max = cal.normalize(16383);
        assert_eq!(at_max, 1.0);
        let at_min = cal.normalize(0);
        assert_eq!(at_min, -1.0);
        // midpoint above center ~ 12287
        let mid_high = cal.normalize(12287);
        assert!(
            (mid_high - 0.5).abs() < 0.01,
            "expected ~0.5, got {mid_high}"
        );
    }

    #[test]
    fn test_normalize_8bit() {
        // 8-bit: 0..255, center 127
        let cal = AxisCalibration::new(0, 255, 127);
        let at_center = cal.normalize(127);
        assert!(
            at_center.abs() < 1e-3,
            "center should be ~0, got {at_center}"
        );
        assert_eq!(cal.normalize(255), 1.0);
        assert_eq!(cal.normalize(0), -1.0);
    }

    // ── CalibrationBank tests ─────────────────────────────────────────────────

    #[test]
    fn test_calbank_unknown_axis_uses_default() {
        let bank = CalibrationBank::new();
        // full range: 65535 → 1.0
        assert_eq!(bank.normalize("nonexistent", 65535), 1.0);
        // full range: 0 → -1.0
        assert_eq!(bank.normalize("nonexistent", 0), -1.0);
    }

    #[test]
    fn test_calbank_insert_and_get() {
        let mut bank = CalibrationBank::new();
        assert!(bank.get("pitch").is_none());
        let cal = AxisCalibration::new(0, 1000, 500);
        bank.insert("pitch", cal.clone());
        assert!(bank.get("pitch").is_some());
        assert_eq!(*bank.get("pitch").unwrap(), cal);
        assert_eq!(bank.normalize("pitch", 1000), 1.0);
    }

    // ── Proptests ────────────────────────────────────────────────────────────

    proptest! {
        /// normalize output must always be within [-1.0, 1.0].
        #[test]
        fn proptest_normalize_bounded(
            raw in 0u16..=u16::MAX,
            raw_min in 0u16..=32767u16,
            raw_max in 32768u16..=u16::MAX,
            raw_center in 0u16..=u16::MAX,
        ) {
            let cal = AxisCalibration::new(raw_min, raw_max, raw_center);
            let out = cal.normalize(raw);
            prop_assert!(
                out >= -1.0 && out <= 1.0,
                "out={out} for raw={raw} cal={cal:?}"
            );
        }

        /// raw == center (no deadband) should map to ~0.0.
        #[test]
        fn proptest_center_is_zero(
            raw_min in 0u16..=32767u16,
            raw_max in 32768u16..=u16::MAX,
        ) {
            // Choose a center strictly inside [raw_min, raw_max]
            let center = ((raw_min as u32 + raw_max as u32) / 2) as u16;
            let cal = AxisCalibration::new(raw_min, raw_max, center);
            let out = cal.normalize(center);
            prop_assert!(
                out.abs() < 1e-3,
                "center={center} should map to ~0.0, got {out}, cal={cal:?}"
            );
        }

        /// raw > max → 1.0; raw < min → -1.0 (when min > 0 and max < u16::MAX).
        #[test]
        fn proptest_clamp_extremes(
            raw_min in 1u16..=1000u16,
            raw_max in 60000u16..=64534u16,
        ) {
            let center = ((raw_min as u32 + raw_max as u32) / 2) as u16;
            let cal = AxisCalibration::new(raw_min, raw_max, center);
            // raw above max
            let above = raw_max + 1;
            prop_assert_eq!(cal.normalize(above), 1.0, "raw={} > max={}", above, raw_max);
            // raw below min
            let below = raw_min - 1;
            prop_assert_eq!(cal.normalize(below), -1.0, "raw={} < min={}", below, raw_min);
        }
    }
}
