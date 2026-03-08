// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the Slaw Device RX Viper rudder pedals.
//!
//! # Device identifiers
//!
//! - VID 0x0483 (STMicroelectronics — shared by Slaw Device STM32 firmware).
//! - PID 0x5746 — community estimate; confirm with `lsusb` on real hardware.
//!
//! # Input report layout (7 bytes)
//!
//! The RX Viper uses custom STM32 HID firmware.  Based on community captures
//! the layout matches the generic 12-bit three-axis pedal pattern:
//!
//! ```text
//! byte  0      : report_id (0x01)
//! bytes 1-2    : rudder      u16 LE, 0..=4095 (12-bit)
//! bytes 3-4    : left_brake  u16 LE, 0..=4095 (12-bit)
//! bytes 5-6    : right_brake u16 LE, 0..=4095 (12-bit)
//! ```
//!
//! ## Quirk: RUDDER_CENTRED
//!
//! Rudder is bipolar (centred ~2048).  Toe brakes are unipolar.
//!
//! ## Quirk: HIGH_PRECISION_POTENTIOMETERS
//!
//! Long throw + high-precision pots yield excellent linearity.  Apply a
//! small deadzone at centre for drift compensation.

use crate::{Calibration, PedalVendor, PedalsAxes, PedalsInputState, calibration::AxisCalibration};
use thiserror::Error;

/// Minimum byte count for a Slaw RX Viper report.
pub const SLAW_VIPER_MIN_REPORT_BYTES: usize = 7;

/// 12-bit maximum value.
const MAX_12BIT: u16 = 4095;

/// Parse error for Slaw RX Viper pedals.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SlawViperParseError {
    #[error("Slaw RX Viper report too short: got {0} bytes (need ≥7)")]
    TooShort(usize),
    #[error("Slaw RX Viper unexpected report ID: {0:#04X} (expected 0x01)")]
    InvalidReportId(u8),
}

fn slaw_default_cal() -> Calibration {
    Calibration {
        rudder: AxisCalibration::new(0, MAX_12BIT),
        left_toe: AxisCalibration::new(0, MAX_12BIT),
        right_toe: AxisCalibration::new(0, MAX_12BIT),
    }
}

/// Parse one raw HID report from Slaw RX Viper pedals.
pub fn parse_slaw_viper_report(data: &[u8]) -> Result<PedalsInputState, SlawViperParseError> {
    parse_slaw_viper_report_calibrated(data, &slaw_default_cal())
}

/// Parse with per-axis calibration overrides.
pub fn parse_slaw_viper_report_calibrated(
    data: &[u8],
    cal: &Calibration,
) -> Result<PedalsInputState, SlawViperParseError> {
    if data.len() < SLAW_VIPER_MIN_REPORT_BYTES {
        return Err(SlawViperParseError::TooShort(data.len()));
    }
    if data[0] != 0x01 {
        return Err(SlawViperParseError::InvalidReportId(data[0]));
    }

    let rudder_raw = u16::from_le_bytes([data[1], data[2]]) & 0x0FFF;
    let left_raw = u16::from_le_bytes([data[3], data[4]]) & 0x0FFF;
    let right_raw = u16::from_le_bytes([data[5], data[6]]) & 0x0FFF;

    Ok(PedalsInputState {
        vendor: PedalVendor::SlawRxViper,
        axes: PedalsAxes {
            rudder: cal.rudder.normalize(rudder_raw),
            left_toe_brake: cal.left_toe.normalize(left_raw),
            right_toe_brake: cal.right_toe.normalize(right_raw),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn make_report(rudder: u16, left: u16, right: u16) -> [u8; 7] {
        let mut r = [0u8; 7];
        r[0] = 0x01;
        r[1..3].copy_from_slice(&rudder.to_le_bytes());
        r[3..5].copy_from_slice(&left.to_le_bytes());
        r[5..7].copy_from_slice(&right.to_le_bytes());
        r
    }

    #[test]
    fn too_short_is_error() {
        assert!(parse_slaw_viper_report(&[0x01; 6]).is_err());
    }

    #[test]
    fn bad_report_id() {
        let mut r = make_report(0, 0, 0);
        r[0] = 0x03;
        assert!(matches!(
            parse_slaw_viper_report(&r),
            Err(SlawViperParseError::InvalidReportId(0x03))
        ));
    }

    #[test]
    fn all_zero() {
        let state = parse_slaw_viper_report(&make_report(0, 0, 0)).unwrap();
        assert_eq!(state.axes.rudder, 0.0);
        assert_eq!(state.axes.left_toe_brake, 0.0);
        assert_eq!(state.axes.right_toe_brake, 0.0);
    }

    #[test]
    fn max_12bit() {
        let state = parse_slaw_viper_report(&make_report(4095, 4095, 4095)).unwrap();
        assert!((state.axes.rudder - 1.0).abs() < 1e-4);
    }

    #[test]
    fn vendor_is_slaw() {
        let state = parse_slaw_viper_report(&make_report(0, 0, 0)).unwrap();
        assert_eq!(state.vendor, PedalVendor::SlawRxViper);
    }

    proptest! {
        #[test]
        fn axes_always_in_range(
            rud in 0u16..=4095u16,
            left in 0u16..=4095u16,
            right in 0u16..=4095u16,
        ) {
            let report = make_report(rud, left, right);
            let state = parse_slaw_viper_report(&report).unwrap();
            assert!((0.0..=1.0).contains(&state.axes.rudder));
            assert!((0.0..=1.0).contains(&state.axes.left_toe_brake));
            assert!((0.0..=1.0).contains(&state.axes.right_toe_brake));
        }

        #[test]
        fn random_report_no_panic(data in proptest::collection::vec(0u8..=255u8, 7..16)) {
            let _ = parse_slaw_viper_report(&data);
        }
    }
}
