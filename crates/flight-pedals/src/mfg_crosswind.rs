// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the MFG Crosswind V3 rudder pedals.
//!
//! # Device identifiers
//!
//! - VID 0x1551 (MFG / Motion Fantasy Games).
//! - PID 0x0003 — community estimate following V1=0x0001, V2=0x0002 pattern.
//!
//! # Input report layout (7 bytes)
//!
//! The Crosswind reports via the standard HID joystick descriptor.
//! Axes are 12-bit effective, packed into u16 LE fields:
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
//! The rudder axis is bipolar (centred at ~2048).  Toe brakes are
//! unipolar (0 = released, 4095 = fully depressed).

use crate::{Calibration, PedalVendor, PedalsAxes, PedalsInputState, calibration::AxisCalibration};
use thiserror::Error;

/// Minimum byte count for an MFG Crosswind report.
pub const MFG_CROSSWIND_MIN_REPORT_BYTES: usize = 7;

/// 12-bit maximum value.
const MAX_12BIT: u16 = 4095;

/// Parse error for MFG Crosswind pedals.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MfgCrosswdParseError {
    #[error("MFG Crosswind report too short: got {0} bytes (need ≥7)")]
    TooShort(usize),
    #[error("MFG Crosswind unexpected report ID: {0:#04X} (expected 0x01)")]
    InvalidReportId(u8),
}

/// Default calibration for 12-bit Crosswind axes.
fn crosswind_default_cal() -> Calibration {
    Calibration {
        rudder: AxisCalibration::new(0, MAX_12BIT),
        left_toe: AxisCalibration::new(0, MAX_12BIT),
        right_toe: AxisCalibration::new(0, MAX_12BIT),
    }
}

/// Parse one raw HID report from MFG Crosswind V3 pedals.
pub fn parse_mfg_crosswind_report(data: &[u8]) -> Result<PedalsInputState, MfgCrosswdParseError> {
    parse_mfg_crosswind_report_calibrated(data, &crosswind_default_cal())
}

/// Parse with per-axis calibration overrides.
pub fn parse_mfg_crosswind_report_calibrated(
    data: &[u8],
    cal: &Calibration,
) -> Result<PedalsInputState, MfgCrosswdParseError> {
    if data.len() < MFG_CROSSWIND_MIN_REPORT_BYTES {
        return Err(MfgCrosswdParseError::TooShort(data.len()));
    }
    if data[0] != 0x01 {
        return Err(MfgCrosswdParseError::InvalidReportId(data[0]));
    }

    let rudder_raw = u16::from_le_bytes([data[1], data[2]]) & 0x0FFF;
    let left_raw = u16::from_le_bytes([data[3], data[4]]) & 0x0FFF;
    let right_raw = u16::from_le_bytes([data[5], data[6]]) & 0x0FFF;

    Ok(PedalsInputState {
        vendor: PedalVendor::MfgCrosswind,
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
        assert!(parse_mfg_crosswind_report(&[0x01; 6]).is_err());
    }

    #[test]
    fn bad_report_id() {
        let mut r = make_report(0, 0, 0);
        r[0] = 0x02;
        assert!(matches!(
            parse_mfg_crosswind_report(&r),
            Err(MfgCrosswdParseError::InvalidReportId(0x02))
        ));
    }

    #[test]
    fn all_zero_is_zero() {
        let state = parse_mfg_crosswind_report(&make_report(0, 0, 0)).unwrap();
        assert_eq!(state.axes.rudder, 0.0);
        assert_eq!(state.axes.left_toe_brake, 0.0);
        assert_eq!(state.axes.right_toe_brake, 0.0);
    }

    #[test]
    fn max_12bit() {
        let state = parse_mfg_crosswind_report(&make_report(4095, 4095, 4095)).unwrap();
        assert!((state.axes.rudder - 1.0).abs() < 1e-4);
        assert!((state.axes.left_toe_brake - 1.0).abs() < 1e-4);
        assert!((state.axes.right_toe_brake - 1.0).abs() < 1e-4);
    }

    #[test]
    fn masks_upper_bits() {
        // 0xFFFF should be masked to 0x0FFF = 4095
        let state = parse_mfg_crosswind_report(&make_report(0xFFFF, 0, 0)).unwrap();
        assert!((state.axes.rudder - 1.0).abs() < 1e-4);
    }

    #[test]
    fn vendor_is_mfg() {
        let state = parse_mfg_crosswind_report(&make_report(0, 0, 0)).unwrap();
        assert_eq!(state.vendor, PedalVendor::MfgCrosswind);
    }

    proptest! {
        #[test]
        fn axes_always_in_range(
            rud in 0u16..=4095u16,
            left in 0u16..=4095u16,
            right in 0u16..=4095u16,
        ) {
            let report = make_report(rud, left, right);
            let state = parse_mfg_crosswind_report(&report).unwrap();
            assert!((0.0..=1.0).contains(&state.axes.rudder));
            assert!((0.0..=1.0).contains(&state.axes.left_toe_brake));
            assert!((0.0..=1.0).contains(&state.axes.right_toe_brake));
        }

        #[test]
        fn random_report_no_panic(data in proptest::collection::vec(0u8..=255u8, 7..16)) {
            let _ = parse_mfg_crosswind_report(&data);
        }
    }
}
