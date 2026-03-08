// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Per-axis calibration for pedal devices.
//!
//! Worn potentiometers or manufacturing variance can shift the effective
//! min/max of a pedal axis.  [`Calibration`] holds per-axis overrides
//! that remap raw values into the full `0.0–1.0` range.

/// Calibration bounds for a single axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxisCalibration {
    /// Observed minimum raw value (mapped to 0.0 after calibration).
    pub raw_min: u16,
    /// Observed maximum raw value (mapped to 1.0 after calibration).
    pub raw_max: u16,
}

impl Default for AxisCalibration {
    fn default() -> Self {
        Self {
            raw_min: 0,
            raw_max: u16::MAX,
        }
    }
}

impl AxisCalibration {
    /// Create a new calibration with the given raw bounds.
    pub fn new(raw_min: u16, raw_max: u16) -> Self {
        Self { raw_min, raw_max }
    }

    /// Normalise a raw u16 value using these calibration bounds.
    ///
    /// Returns a value clamped to `0.0..=1.0`.
    pub fn normalize(&self, raw: u16) -> f32 {
        let span = self.raw_max.saturating_sub(self.raw_min);
        if span == 0 {
            return 0.5;
        }
        let clamped = raw.clamp(self.raw_min, self.raw_max);
        ((clamped - self.raw_min) as f32 / span as f32).clamp(0.0, 1.0)
    }
}

/// Full calibration for a three-axis pedal device.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Calibration {
    /// Rudder axis calibration.
    pub rudder: AxisCalibration,
    /// Left toe brake calibration.
    pub left_toe: AxisCalibration,
    /// Right toe brake calibration.
    pub right_toe: AxisCalibration,
}

impl Calibration {
    /// Identity calibration (full u16 range, no remapping).
    pub fn identity() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_normalizes_full_range() {
        let cal = AxisCalibration::default();
        assert_eq!(cal.normalize(0), 0.0);
        assert!((cal.normalize(u16::MAX) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn custom_range_normalizes() {
        let cal = AxisCalibration::new(100, 900);
        assert_eq!(cal.normalize(100), 0.0);
        assert!((cal.normalize(900) - 1.0).abs() < 1e-4);
        assert!((cal.normalize(500) - 0.5).abs() < 0.01);
    }

    #[test]
    fn values_outside_range_are_clamped() {
        let cal = AxisCalibration::new(100, 900);
        assert_eq!(cal.normalize(0), 0.0);
        assert!((cal.normalize(1000) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn zero_span_returns_midpoint() {
        let cal = AxisCalibration::new(500, 500);
        assert_eq!(cal.normalize(500), 0.5);
    }
}
