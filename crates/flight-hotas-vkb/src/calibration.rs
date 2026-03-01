// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Axis calibration data structures for VKB devices.
//!
//! VKB devices report axes as raw 16-bit unsigned integers. This module provides
//! calibration structures that capture the physical range of each axis as observed
//! during a user calibration pass, and normalisation functions that map raw values
//! to the `[0.0, 1.0]` or `[−1.0, 1.0]` range using those calibration points.
//!
//! # Usage
//!
//! ```rust
//! use flight_hotas_vkb::calibration::{AxisCalibration, CalibratedNormMode};
//!
//! let cal = AxisCalibration {
//!     raw_min: 100,
//!     raw_center: 32768,
//!     raw_max: 65400,
//!     deadzone: 0.03,
//!     mode: CalibratedNormMode::Signed,
//! };
//!
//! let normalised = cal.normalize(32768); // ≈ 0.0
//! let full_right = cal.normalize(65400); // ≈ 1.0
//! ```

/// Normalisation mode for calibrated axis values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibratedNormMode {
    /// Bidirectional axis: maps `raw_min → −1.0`, `raw_center → 0.0`, `raw_max → 1.0`.
    /// Used for joystick roll, pitch, yaw, rudder.
    Signed,
    /// Unidirectional axis: maps `raw_min → 0.0`, `raw_max → 1.0`.
    /// Centre value is ignored. Used for throttle, brakes, sliders.
    Unsigned,
}

/// Calibration data for one analogue axis.
///
/// Captured during a user calibration pass. The raw values represent the
/// physical range of the sensor as observed on the user's hardware.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxisCalibration {
    /// Minimum raw value observed during calibration.
    pub raw_min: u16,
    /// Centre (resting) raw value observed during calibration.
    /// Only meaningful for [`CalibratedNormMode::Signed`] axes.
    pub raw_center: u16,
    /// Maximum raw value observed during calibration.
    pub raw_max: u16,
    /// Deadzone as a fraction of the normalised range (`0.0..=0.5`).
    /// Values within ±deadzone of the centre are snapped to 0.0 (signed)
    /// or the deadzone is applied at the low end (unsigned).
    pub deadzone: f32,
    /// Normalisation mode.
    pub mode: CalibratedNormMode,
}

impl AxisCalibration {
    /// Create a default calibration for a full-range 16-bit axis.
    pub fn default_16bit(mode: CalibratedNormMode) -> Self {
        Self {
            raw_min: 0,
            raw_center: 0x8000,
            raw_max: 0xFFFF,
            deadzone: 0.0,
            mode,
        }
    }

    /// Normalise a raw axis value using this calibration.
    ///
    /// Returns a value in `[−1.0, 1.0]` for [`CalibratedNormMode::Signed`]
    /// or `[0.0, 1.0]` for [`CalibratedNormMode::Unsigned`].
    pub fn normalize(&self, raw: u16) -> f32 {
        match self.mode {
            CalibratedNormMode::Signed => self.normalize_signed(raw),
            CalibratedNormMode::Unsigned => self.normalize_unsigned(raw),
        }
    }

    fn normalize_unsigned(&self, raw: u16) -> f32 {
        let range = self.raw_max.saturating_sub(self.raw_min);
        if range == 0 {
            return 0.0;
        }
        let clamped = raw.clamp(self.raw_min, self.raw_max);
        let norm = (clamped - self.raw_min) as f32 / range as f32;

        if norm < self.deadzone {
            0.0
        } else {
            // Rescale above deadzone to fill [0.0, 1.0]
            ((norm - self.deadzone) / (1.0 - self.deadzone)).clamp(0.0, 1.0)
        }
    }

    fn normalize_signed(&self, raw: u16) -> f32 {
        let clamped = raw.clamp(self.raw_min, self.raw_max);

        let norm = if clamped <= self.raw_center {
            let range = self.raw_center.saturating_sub(self.raw_min);
            if range == 0 {
                0.0
            } else {
                -((self.raw_center - clamped) as f32 / range as f32)
            }
        } else {
            let range = self.raw_max.saturating_sub(self.raw_center);
            if range == 0 {
                0.0
            } else {
                (clamped - self.raw_center) as f32 / range as f32
            }
        };

        if norm.abs() < self.deadzone {
            0.0
        } else {
            // Rescale outside deadzone to fill [−1.0, 1.0]
            let sign = norm.signum();
            let magnitude = (norm.abs() - self.deadzone) / (1.0 - self.deadzone);
            (sign * magnitude).clamp(-1.0, 1.0)
        }
    }
}

/// Calibration set for all axes of a VKB device.
///
/// Stores per-axis calibration data indexed by report offset or axis index.
/// The actual axis count depends on the device family.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceCalibration {
    /// Device name (for display/logging).
    pub device_name: String,
    /// Per-axis calibration entries, ordered by report axis index.
    pub axes: Vec<AxisCalibration>,
}

impl DeviceCalibration {
    /// Create a default calibration for a device with the given number of axes.
    pub fn default_for_axes(device_name: &str, axis_modes: &[CalibratedNormMode]) -> Self {
        Self {
            device_name: device_name.to_string(),
            axes: axis_modes
                .iter()
                .map(|&mode| AxisCalibration::default_16bit(mode))
                .collect(),
        }
    }

    /// Normalise a raw axis value for the given axis index.
    ///
    /// Returns `None` if the axis index is out of range.
    pub fn normalize(&self, axis_index: usize, raw: u16) -> Option<f32> {
        self.axes.get(axis_index).map(|cal| cal.normalize(raw))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsigned_default_endpoints() {
        let cal = AxisCalibration::default_16bit(CalibratedNormMode::Unsigned);
        assert_eq!(cal.normalize(0), 0.0);
        assert!((cal.normalize(0xFFFF) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn unsigned_midpoint() {
        let cal = AxisCalibration::default_16bit(CalibratedNormMode::Unsigned);
        assert!((cal.normalize(0x8000) - 0.5).abs() < 0.01);
    }

    #[test]
    fn signed_default_endpoints() {
        let cal = AxisCalibration::default_16bit(CalibratedNormMode::Signed);
        assert!((cal.normalize(0) - (-1.0)).abs() < 0.01);
        assert!(cal.normalize(0x8000).abs() < 0.01);
        assert!((cal.normalize(0xFFFF) - 1.0).abs() < 0.01);
    }

    #[test]
    fn signed_with_deadzone() {
        let cal = AxisCalibration {
            raw_min: 0,
            raw_center: 0x8000,
            raw_max: 0xFFFF,
            deadzone: 0.1,
            mode: CalibratedNormMode::Signed,
        };
        // Centre is in deadzone
        assert_eq!(cal.normalize(0x8000), 0.0);
        // Small deflection within deadzone
        let small = 0x8000 + 0x0800; // ~3% of range
        assert_eq!(cal.normalize(small), 0.0);
    }

    #[test]
    fn unsigned_with_deadzone() {
        let cal = AxisCalibration {
            raw_min: 0,
            raw_center: 0x8000,
            raw_max: 0xFFFF,
            deadzone: 0.1,
            mode: CalibratedNormMode::Unsigned,
        };
        // Small values in deadzone
        assert_eq!(cal.normalize(0), 0.0);
        assert_eq!(cal.normalize(100), 0.0);
        // Full range still reaches 1.0
        assert!((cal.normalize(0xFFFF) - 1.0).abs() < 0.01);
    }

    #[test]
    fn custom_range_unsigned() {
        let cal = AxisCalibration {
            raw_min: 1000,
            raw_center: 32000,
            raw_max: 64000,
            deadzone: 0.0,
            mode: CalibratedNormMode::Unsigned,
        };
        assert_eq!(cal.normalize(1000), 0.0);
        assert!((cal.normalize(64000) - 1.0).abs() < 0.01);
        assert!((cal.normalize(32500) - 0.5).abs() < 0.01);
    }

    #[test]
    fn custom_range_signed() {
        let cal = AxisCalibration {
            raw_min: 500,
            raw_center: 32000,
            raw_max: 63500,
            deadzone: 0.0,
            mode: CalibratedNormMode::Signed,
        };
        assert!((cal.normalize(500) - (-1.0)).abs() < 0.01);
        assert!(cal.normalize(32000).abs() < 0.01);
        assert!((cal.normalize(63500) - 1.0).abs() < 0.01);
    }

    #[test]
    fn values_clamp_to_calibrated_range() {
        let cal = AxisCalibration {
            raw_min: 1000,
            raw_center: 32000,
            raw_max: 64000,
            deadzone: 0.0,
            mode: CalibratedNormMode::Unsigned,
        };
        // Below min clamps to 0
        assert_eq!(cal.normalize(0), 0.0);
        // Above max clamps to 1
        assert!((cal.normalize(0xFFFF) - 1.0).abs() < 0.01);
    }

    #[test]
    fn zero_range_returns_zero() {
        let cal = AxisCalibration {
            raw_min: 100,
            raw_center: 100,
            raw_max: 100,
            deadzone: 0.0,
            mode: CalibratedNormMode::Unsigned,
        };
        assert_eq!(cal.normalize(100), 0.0);
    }

    #[test]
    fn device_calibration_normalize() {
        let modes = [
            CalibratedNormMode::Unsigned,
            CalibratedNormMode::Unsigned,
            CalibratedNormMode::Signed,
        ];
        let cal = DeviceCalibration::default_for_axes("T-Rudder", &modes);
        assert_eq!(cal.axes.len(), 3);
        assert_eq!(cal.normalize(0, 0), Some(0.0));
        assert!(cal.normalize(3, 0).is_none()); // out of range
    }

    #[test]
    fn device_calibration_name() {
        let cal = DeviceCalibration::default_for_axes("Test Device", &[]);
        assert_eq!(cal.device_name, "Test Device");
    }
}
