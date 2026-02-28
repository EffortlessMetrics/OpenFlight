// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! CH Products HID protocol layer.
//!
//! CH Products devices use standard HID joystick reports. All devices share
//! VID `0x068E` and use report ID `0x01`. The OS HID descriptor decoding
//! provides structured axis/button data, but this module defines the raw
//! report constants and encoding helpers used by the per-device parsers.
//!
//! # Common report structure
//!
//! CH Products devices follow a common pattern:
//! - Byte 0: Report ID (`0x01`)
//! - Axes: 16-bit little-endian values
//! - Buttons: packed bitmask bytes
//! - Hat: 4-bit nibble encoding (8-way or 4-way)
//!
//! # POV hat encoding
//!
//! CH devices use two hat encoding schemes:
//! - **4-way**: 0=center, 1=N, 2=E, 3=S, 4=W (Fighterstick secondary hats)
//! - **8-way**: 0=center, 1=N, 2=NE, 3=E, 4=SE, 5=S, 6=SW, 7=W, 8=NW
//!
//! # Mini-stick (analog hat)
//!
//! The Pro Throttle features a spring-return mini-stick that reports as two
//! analog axes (X/Y) rather than a digital hat. The mini-stick axes are
//! center-return and should use bipolar normalization.
//!
//! # Button encoding
//!
//! Buttons are packed as bitmasks. The Pro Throttle supports 24+ buttons
//! across multiple report bytes. Button indices are 0-based in the bitmask.

use crate::ChError;

/// HID report ID used by all CH Products devices.
pub const REPORT_ID: u8 = 0x01;

/// Maximum raw axis value (16-bit unsigned).
pub const AXIS_MAX: u16 = 65535;

/// Center value for a centered bipolar axis.
pub const AXIS_CENTER: u16 = 32768;

/// 8-way POV hat direction values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PovDirection {
    Center = 0,
    North = 1,
    NorthEast = 2,
    East = 3,
    SouthEast = 4,
    South = 5,
    SouthWest = 6,
    West = 7,
    NorthWest = 8,
}

impl PovDirection {
    /// Parse a 4-bit hat nibble into a direction.
    ///
    /// Returns `None` for values outside the valid range (0–8).
    pub fn from_nibble(nibble: u8) -> Option<Self> {
        match nibble {
            0 => Some(Self::Center),
            1 => Some(Self::North),
            2 => Some(Self::NorthEast),
            3 => Some(Self::East),
            4 => Some(Self::SouthEast),
            5 => Some(Self::South),
            6 => Some(Self::SouthWest),
            7 => Some(Self::West),
            8 => Some(Self::NorthWest),
            _ => None,
        }
    }

    /// Convert to angle in degrees (0=North, clockwise). `None` for center.
    pub fn to_degrees(self) -> Option<u16> {
        match self {
            Self::Center => None,
            Self::North => Some(0),
            Self::NorthEast => Some(45),
            Self::East => Some(90),
            Self::SouthEast => Some(135),
            Self::South => Some(180),
            Self::SouthWest => Some(225),
            Self::West => Some(270),
            Self::NorthWest => Some(315),
        }
    }

    /// Returns `true` if the hat is not in the center position.
    pub fn is_active(self) -> bool {
        self != Self::Center
    }
}

/// 4-way hat direction values (used on some Fighterstick secondary hats).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FourWayHat {
    Center = 0,
    North = 1,
    East = 2,
    South = 3,
    West = 4,
}

impl FourWayHat {
    /// Parse a 4-bit hat nibble into a 4-way direction.
    pub fn from_nibble(nibble: u8) -> Option<Self> {
        match nibble {
            0 => Some(Self::Center),
            1 => Some(Self::North),
            2 => Some(Self::East),
            3 => Some(Self::South),
            4 => Some(Self::West),
            _ => None,
        }
    }
}

/// Validate a report starts with the expected report ID.
pub fn validate_report_id(report: &[u8]) -> Result<(), ChError> {
    if report.is_empty() {
        return Err(ChError::TooShort { need: 1, got: 0 });
    }
    if report[0] != REPORT_ID {
        return Err(ChError::InvalidReportId(report[0]));
    }
    Ok(())
}

/// Extract a 16-bit little-endian axis value from a report at the given offset.
pub fn read_axis_u16(report: &[u8], offset: usize) -> Result<u16, ChError> {
    if report.len() < offset + 2 {
        return Err(ChError::TooShort {
            need: offset + 2,
            got: report.len(),
        });
    }
    Ok(u16::from_le_bytes([report[offset], report[offset + 1]]))
}

/// Normalize a bipolar axis (centered at midpoint) to `[-1.0, 1.0]`.
pub fn normalize_bipolar(raw: u16) -> f32 {
    (raw as f32 / AXIS_MAX as f32 * 2.0 - 1.0).clamp(-1.0, 1.0)
}

/// Normalize a unipolar axis to `[0.0, 1.0]`.
pub fn normalize_unipolar(raw: u16) -> f32 {
    (raw as f32 / AXIS_MAX as f32).clamp(0.0, 1.0)
}

/// Extract button bits from a byte, returning them as a u32 shifted to the given position.
pub fn extract_buttons(byte: u8, shift: u32) -> u32 {
    u32::from(byte) << shift
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pov_center_no_angle() {
        assert_eq!(PovDirection::Center.to_degrees(), None);
        assert!(!PovDirection::Center.is_active());
    }

    #[test]
    fn pov_directions_clockwise() {
        let expected = [
            (PovDirection::North, 0),
            (PovDirection::NorthEast, 45),
            (PovDirection::East, 90),
            (PovDirection::SouthEast, 135),
            (PovDirection::South, 180),
            (PovDirection::SouthWest, 225),
            (PovDirection::West, 270),
            (PovDirection::NorthWest, 315),
        ];
        for (dir, angle) in expected {
            assert_eq!(dir.to_degrees(), Some(angle), "{dir:?}");
            assert!(dir.is_active());
        }
    }

    #[test]
    fn pov_from_nibble_valid() {
        for n in 0..=8 {
            assert!(PovDirection::from_nibble(n).is_some(), "nibble {n}");
        }
    }

    #[test]
    fn pov_from_nibble_invalid() {
        for n in 9..=15 {
            assert!(PovDirection::from_nibble(n).is_none(), "nibble {n}");
        }
    }

    #[test]
    fn four_way_hat_valid_values() {
        assert_eq!(FourWayHat::from_nibble(0), Some(FourWayHat::Center));
        assert_eq!(FourWayHat::from_nibble(1), Some(FourWayHat::North));
        assert_eq!(FourWayHat::from_nibble(4), Some(FourWayHat::West));
        assert_eq!(FourWayHat::from_nibble(5), None);
    }

    #[test]
    fn validate_report_id_ok() {
        assert!(validate_report_id(&[0x01, 0x00]).is_ok());
    }

    #[test]
    fn validate_report_id_empty() {
        assert!(validate_report_id(&[]).is_err());
    }

    #[test]
    fn validate_report_id_wrong() {
        let err = validate_report_id(&[0x02]).unwrap_err();
        assert!(matches!(err, ChError::InvalidReportId(0x02)));
    }

    #[test]
    fn read_axis_u16_valid() {
        let data = [0x01, 0xFF, 0x00]; // offset 1: 0x00FF = 255
        assert_eq!(read_axis_u16(&data, 1).unwrap(), 255);
    }

    #[test]
    fn read_axis_u16_too_short() {
        assert!(read_axis_u16(&[0x01], 1).is_err());
    }

    #[test]
    fn normalize_bipolar_bounds() {
        assert!((normalize_bipolar(0) + 1.0).abs() < 1e-4);
        assert!((normalize_bipolar(65535) - 1.0).abs() < 1e-4);
        assert!(normalize_bipolar(32768).abs() < 0.001);
    }

    #[test]
    fn normalize_unipolar_bounds() {
        assert!((normalize_unipolar(0)).abs() < 1e-4);
        assert!((normalize_unipolar(65535) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn extract_buttons_shift() {
        assert_eq!(extract_buttons(0xFF, 0), 0xFF);
        assert_eq!(extract_buttons(0x0F, 8), 0x0F00);
    }
}
