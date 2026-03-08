// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the Saitek/Logitech Pro Flight Rudder Pedals.
//!
//! # Device identifiers
//!
//! - VID 0x06A3 (Saitek / Logitech PLC).
//! - PID 0x0763 — confirmed via linux-hardware.org (51 probes).
//!
//! # Input report layout (7 bytes)
//!
//! ```text
//! byte  0      : report_id (0x00)
//! bytes 1-2    : rudder      u16 LE, 0..=1023 (10-bit)
//! bytes 3-4    : left_toe    u16 LE, 0..=1023 (10-bit)
//! bytes 5-6    : right_toe   u16 LE, 0..=1023 (10-bit)
//! ```
//!
//! ## Quirk: 10-bit resolution
//!
//! The Pro Flight pedals use 10-bit ADCs.  Raw values range 0..=1023.
//!
//! ## Quirk: RUDDER_CENTRED
//!
//! Rudder axis is bipolar.  Toe brakes are unipolar.
//!
//! ## Quirk: NO_FORCE_TRIM
//!
//! Spring tension is set via a physical knob on the unit; no software
//! force-trim is available.

use crate::{Calibration, PedalVendor, PedalsAxes, PedalsInputState, calibration::AxisCalibration};
use thiserror::Error;

/// Minimum byte count for a Saitek Pro Flight pedals report.
pub const SAITEK_PEDALS_MIN_REPORT_BYTES: usize = 7;

/// 10-bit maximum value.
const MAX_10BIT: u16 = 1023;

/// Parse error for Saitek Pro Flight pedals.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SaitekPedalsParseError {
    #[error("Saitek pedals report too short: got {0} bytes (need ≥7)")]
    TooShort(usize),
    #[error("Saitek pedals unexpected report ID: {0:#04X} (expected 0x00)")]
    InvalidReportId(u8),
}

fn saitek_default_cal() -> Calibration {
    Calibration {
        rudder: AxisCalibration::new(0, MAX_10BIT),
        left_toe: AxisCalibration::new(0, MAX_10BIT),
        right_toe: AxisCalibration::new(0, MAX_10BIT),
    }
}

/// Parse one raw HID report from Saitek Pro Flight rudder pedals.
pub fn parse_saitek_pedals_report(data: &[u8]) -> Result<PedalsInputState, SaitekPedalsParseError> {
    parse_saitek_pedals_report_calibrated(data, &saitek_default_cal())
}

/// Parse with per-axis calibration overrides.
pub fn parse_saitek_pedals_report_calibrated(
    data: &[u8],
    cal: &Calibration,
) -> Result<PedalsInputState, SaitekPedalsParseError> {
    if data.len() < SAITEK_PEDALS_MIN_REPORT_BYTES {
        return Err(SaitekPedalsParseError::TooShort(data.len()));
    }
    if data[0] != 0x00 {
        return Err(SaitekPedalsParseError::InvalidReportId(data[0]));
    }

    let rudder_raw = u16::from_le_bytes([data[1], data[2]]) & 0x03FF;
    let left_raw = u16::from_le_bytes([data[3], data[4]]) & 0x03FF;
    let right_raw = u16::from_le_bytes([data[5], data[6]]) & 0x03FF;

    Ok(PedalsInputState {
        vendor: PedalVendor::SaitekProFlight,
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
        r[0] = 0x00;
        r[1..3].copy_from_slice(&rudder.to_le_bytes());
        r[3..5].copy_from_slice(&left.to_le_bytes());
        r[5..7].copy_from_slice(&right.to_le_bytes());
        r
    }

    #[test]
    fn too_short_is_error() {
        assert!(parse_saitek_pedals_report(&[0x00; 6]).is_err());
    }

    #[test]
    fn bad_report_id() {
        let mut r = make_report(0, 0, 0);
        r[0] = 0x01;
        assert!(matches!(
            parse_saitek_pedals_report(&r),
            Err(SaitekPedalsParseError::InvalidReportId(0x01))
        ));
    }

    #[test]
    fn all_zero() {
        let state = parse_saitek_pedals_report(&make_report(0, 0, 0)).unwrap();
        assert_eq!(state.axes.rudder, 0.0);
        assert_eq!(state.axes.left_toe_brake, 0.0);
        assert_eq!(state.axes.right_toe_brake, 0.0);
    }

    #[test]
    fn max_10bit() {
        let state = parse_saitek_pedals_report(&make_report(1023, 1023, 1023)).unwrap();
        assert!((state.axes.rudder - 1.0).abs() < 1e-4);
        assert!((state.axes.left_toe_brake - 1.0).abs() < 1e-4);
        assert!((state.axes.right_toe_brake - 1.0).abs() < 1e-4);
    }

    #[test]
    fn masks_upper_bits() {
        // 0xFFFF masked to 0x03FF = 1023
        let state = parse_saitek_pedals_report(&make_report(0xFFFF, 0, 0)).unwrap();
        assert!((state.axes.rudder - 1.0).abs() < 1e-4);
    }

    #[test]
    fn center_10bit() {
        let state = parse_saitek_pedals_report(&make_report(512, 0, 0)).unwrap();
        assert!((state.axes.rudder - 0.5).abs() < 0.01);
    }

    #[test]
    fn vendor_is_saitek() {
        let state = parse_saitek_pedals_report(&make_report(0, 0, 0)).unwrap();
        assert_eq!(state.vendor, PedalVendor::SaitekProFlight);
    }

    proptest! {
        #[test]
        fn axes_always_in_range(
            rud in 0u16..=1023u16,
            left in 0u16..=1023u16,
            right in 0u16..=1023u16,
        ) {
            let report = make_report(rud, left, right);
            let state = parse_saitek_pedals_report(&report).unwrap();
            assert!((0.0..=1.0).contains(&state.axes.rudder));
            assert!((0.0..=1.0).contains(&state.axes.left_toe_brake));
            assert!((0.0..=1.0).contains(&state.axes.right_toe_brake));
        }

        #[test]
        fn random_report_no_panic(data in proptest::collection::vec(0u8..=255u8, 7..16)) {
            let _ = parse_saitek_pedals_report(&data);
        }
    }
}
