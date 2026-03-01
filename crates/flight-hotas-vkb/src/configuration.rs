// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! VKB firmware-level device configuration reading.
//!
//! VKB devices expose configuration data (sensitivity, curves, dead zones) via
//! HID feature reports.  This module parses those reports into structured config
//! types.
//!
//! # Report layout (ASSUMED)
//!
//! VKB firmware stores per-axis configuration in a feature report with report ID
//! `0x0A`.  The exact layout is inferred from VKBDevCfg behaviour and has **not**
//! been confirmed from a hardware capture.
//!
//! ```text
//! byte  0    : report_id (0x0A)
//! byte  1    : axis_count (number of configured axes, max 6)
//! bytes 2–7  : sensitivity per axis (u8, 0–100 → 0.0–1.0)
//! bytes 8–13 : dead zone per axis (u8, 0–100 → 0.0–1.0)
//! bytes 14–19: curve type per axis (u8, see CurveType)
//! byte  20   : profile_id (0–3, which on-device profile slot)
//! ```

use thiserror::Error;

/// Maximum number of configurable axes in VKB firmware.
pub const VKB_MAX_CONFIG_AXES: usize = 6;

/// Minimum byte length for a VKB config feature report (including report ID).
pub const VKB_CONFIG_MIN_REPORT_BYTES: usize = 21;

/// Report ID used by VKB firmware for configuration feature reports.
///
/// **ASSUMED** — not captured from hardware.
pub const VKB_CONFIG_REPORT_ID: u8 = 0x0A;

/// Response curve type for a VKB axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurveType {
    /// Linear response (1:1 mapping).
    Linear,
    /// S-curve (gentle centre, steep edges).
    SCurve,
    /// J-curve (shallow start, steep finish).
    JCurve,
    /// Custom user-defined curve stored in firmware.
    Custom,
}

impl CurveType {
    /// Parse a curve type from a firmware byte value.
    fn from_byte(b: u8) -> Self {
        match b {
            0 => Self::Linear,
            1 => Self::SCurve,
            2 => Self::JCurve,
            _ => Self::Custom,
        }
    }

    /// Encode this curve type as a firmware byte value.
    pub fn to_byte(self) -> u8 {
        match self {
            Self::Linear => 0,
            Self::SCurve => 1,
            Self::JCurve => 2,
            Self::Custom => 3,
        }
    }
}

/// VKB firmware-level device configuration.
///
/// Represents per-axis sensitivity, dead zone, and response curve settings
/// as stored in the device firmware.
#[derive(Debug, Clone, PartialEq)]
pub struct VkbConfig {
    /// Number of axes configured (1–6).
    pub axis_count: u8,
    /// Per-axis sensitivity (0.0–1.0).  Index corresponds to axis order.
    pub sensitivity: [f32; VKB_MAX_CONFIG_AXES],
    /// Per-axis dead zone (0.0–1.0).  Index corresponds to axis order.
    pub deadzone: [f32; VKB_MAX_CONFIG_AXES],
    /// Per-axis response curve type.
    pub curve_type: [CurveType; VKB_MAX_CONFIG_AXES],
    /// On-device profile slot (0–3).
    pub profile_slot: u8,
}

impl Default for VkbConfig {
    fn default() -> Self {
        Self {
            axis_count: VKB_MAX_CONFIG_AXES as u8,
            sensitivity: [1.0; VKB_MAX_CONFIG_AXES],
            deadzone: [0.0; VKB_MAX_CONFIG_AXES],
            curve_type: [CurveType::Linear; VKB_MAX_CONFIG_AXES],
            profile_slot: 0,
        }
    }
}

/// A named configuration profile stored on the device.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigProfile {
    /// Human-readable profile name (user-assigned or default).
    pub name: String,
    /// The configuration data for this profile.
    pub config: VkbConfig,
}

/// Errors returned when parsing a VKB configuration report.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ConfigParseError {
    /// Report is shorter than the minimum required length.
    #[error("config report too short: got {actual} bytes, need >= {expected}")]
    TooShort {
        /// Expected minimum byte count.
        expected: usize,
        /// Actual byte count received.
        actual: usize,
    },
    /// Report ID does not match the expected config report ID.
    #[error("unexpected report ID: expected 0x{expected:02X}, got 0x{actual:02X}")]
    WrongReportId {
        /// Expected report ID.
        expected: u8,
        /// Actual report ID.
        actual: u8,
    },
    /// Axis count in the report exceeds the supported maximum.
    #[error("axis count {count} exceeds maximum {max}")]
    AxisCountOverflow {
        /// Reported axis count.
        count: u8,
        /// Maximum supported.
        max: u8,
    },
}

/// Parse a VKB configuration feature report into a [`VkbConfig`].
///
/// The report must be at least [`VKB_CONFIG_MIN_REPORT_BYTES`] bytes long and
/// start with [`VKB_CONFIG_REPORT_ID`].
///
/// **Note:** The report layout is **ASSUMED** from VKB firmware family
/// conventions.  Verify against a hardware USB capture before shipping.
pub fn read_config_from_report(data: &[u8]) -> Result<VkbConfig, ConfigParseError> {
    if data.len() < VKB_CONFIG_MIN_REPORT_BYTES {
        return Err(ConfigParseError::TooShort {
            expected: VKB_CONFIG_MIN_REPORT_BYTES,
            actual: data.len(),
        });
    }

    if data[0] != VKB_CONFIG_REPORT_ID {
        return Err(ConfigParseError::WrongReportId {
            expected: VKB_CONFIG_REPORT_ID,
            actual: data[0],
        });
    }

    let axis_count = data[1];
    if axis_count > VKB_MAX_CONFIG_AXES as u8 {
        return Err(ConfigParseError::AxisCountOverflow {
            count: axis_count,
            max: VKB_MAX_CONFIG_AXES as u8,
        });
    }

    let mut sensitivity = [0.0f32; VKB_MAX_CONFIG_AXES];
    let mut deadzone = [0.0f32; VKB_MAX_CONFIG_AXES];
    let mut curve_type = [CurveType::Linear; VKB_MAX_CONFIG_AXES];

    for i in 0..VKB_MAX_CONFIG_AXES {
        sensitivity[i] = data[2 + i] as f32 / 100.0;
        deadzone[i] = data[8 + i] as f32 / 100.0;
        curve_type[i] = CurveType::from_byte(data[14 + i]);
    }

    let profile_slot = data[20];

    Ok(VkbConfig {
        axis_count,
        sensitivity,
        deadzone,
        curve_type,
        profile_slot,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config_report(
        axis_count: u8,
        sensitivity: [u8; 6],
        deadzone: [u8; 6],
        curves: [u8; 6],
        profile_slot: u8,
    ) -> Vec<u8> {
        let mut data = vec![VKB_CONFIG_REPORT_ID, axis_count];
        data.extend_from_slice(&sensitivity);
        data.extend_from_slice(&deadzone);
        data.extend_from_slice(&curves);
        data.push(profile_slot);
        data
    }

    #[test]
    fn parse_valid_config_report() {
        let report = make_config_report(
            6,
            [100, 80, 60, 50, 50, 50],
            [5, 5, 10, 0, 0, 0],
            [0, 1, 2, 3, 0, 0],
            1,
        );
        let config = read_config_from_report(&report).unwrap();
        assert_eq!(config.axis_count, 6);
        assert!((config.sensitivity[0] - 1.0).abs() < 1e-4);
        assert!((config.sensitivity[1] - 0.8).abs() < 1e-4);
        assert!((config.sensitivity[2] - 0.6).abs() < 1e-4);
        assert!((config.deadzone[0] - 0.05).abs() < 1e-4);
        assert!((config.deadzone[2] - 0.10).abs() < 1e-4);
        assert_eq!(config.curve_type[0], CurveType::Linear);
        assert_eq!(config.curve_type[1], CurveType::SCurve);
        assert_eq!(config.curve_type[2], CurveType::JCurve);
        assert_eq!(config.curve_type[3], CurveType::Custom);
        assert_eq!(config.profile_slot, 1);
    }

    #[test]
    fn parse_report_too_short() {
        let report = vec![VKB_CONFIG_REPORT_ID; 10];
        let err = read_config_from_report(&report);
        assert!(matches!(
            err,
            Err(ConfigParseError::TooShort {
                expected: 21,
                actual: 10
            })
        ));
    }

    #[test]
    fn parse_wrong_report_id() {
        let mut report = make_config_report(6, [50; 6], [0; 6], [0; 6], 0);
        report[0] = 0xFF;
        let err = read_config_from_report(&report);
        assert!(matches!(
            err,
            Err(ConfigParseError::WrongReportId {
                expected: 0x0A,
                actual: 0xFF
            })
        ));
    }

    #[test]
    fn parse_axis_count_overflow() {
        let report = make_config_report(7, [50; 6], [0; 6], [0; 6], 0);
        let err = read_config_from_report(&report);
        assert!(matches!(
            err,
            Err(ConfigParseError::AxisCountOverflow { count: 7, max: 6 })
        ));
    }

    #[test]
    fn default_config_is_neutral() {
        let config = VkbConfig::default();
        assert_eq!(config.axis_count, 6);
        for i in 0..VKB_MAX_CONFIG_AXES {
            assert_eq!(config.sensitivity[i], 1.0);
            assert_eq!(config.deadzone[i], 0.0);
            assert_eq!(config.curve_type[i], CurveType::Linear);
        }
        assert_eq!(config.profile_slot, 0);
    }

    #[test]
    fn curve_type_round_trip() {
        for &(byte, expected) in &[
            (0, CurveType::Linear),
            (1, CurveType::SCurve),
            (2, CurveType::JCurve),
            (3, CurveType::Custom),
            (255, CurveType::Custom),
        ] {
            let ct = CurveType::from_byte(byte);
            assert_eq!(ct, expected);
        }
        assert_eq!(CurveType::Linear.to_byte(), 0);
        assert_eq!(CurveType::SCurve.to_byte(), 1);
        assert_eq!(CurveType::JCurve.to_byte(), 2);
        assert_eq!(CurveType::Custom.to_byte(), 3);
    }

    #[test]
    fn config_profile_stores_name_and_config() {
        let profile = ConfigProfile {
            name: "Combat".to_string(),
            config: VkbConfig::default(),
        };
        assert_eq!(profile.name, "Combat");
        assert_eq!(profile.config.axis_count, 6);
    }

    #[test]
    fn parse_all_zeros_sensitivity_and_deadzone() {
        let report = make_config_report(3, [0; 6], [0; 6], [0; 6], 0);
        let config = read_config_from_report(&report).unwrap();
        assert_eq!(config.axis_count, 3);
        for i in 0..VKB_MAX_CONFIG_AXES {
            assert_eq!(config.sensitivity[i], 0.0);
            assert_eq!(config.deadzone[i], 0.0);
        }
    }

    #[test]
    fn parse_max_sensitivity_and_deadzone() {
        let report = make_config_report(6, [100; 6], [100; 6], [0; 6], 3);
        let config = read_config_from_report(&report).unwrap();
        for i in 0..VKB_MAX_CONFIG_AXES {
            assert!((config.sensitivity[i] - 1.0).abs() < 1e-4);
            assert!((config.deadzone[i] - 1.0).abs() < 1e-4);
        }
        assert_eq!(config.profile_slot, 3);
    }

    #[test]
    fn empty_report_returns_too_short() {
        let err = read_config_from_report(&[]);
        assert!(matches!(err, Err(ConfigParseError::TooShort { .. })));
    }

    #[test]
    fn longer_report_accepted() {
        let mut report = make_config_report(6, [50; 6], [10; 6], [1; 6], 0);
        report.extend_from_slice(&[0xFF; 8]); // extra bytes
        let config = read_config_from_report(&report).unwrap();
        assert_eq!(config.axis_count, 6);
    }
}
