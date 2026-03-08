// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for VKB T-Rudder Mk.IV / Mk.V pedals.
//!
//! # Device identifiers
//!
//! - VID 0x231D (VKB / Fervian Technologies Limited).
//! - PID 0x0126 — community-reported for T-Rudder Mk.IV (linux-hardware.org, 2 probes).
//!
//! # Input report layout (7 bytes)
//!
//! The T-Rudder reports via the standard VKB HID joystick descriptor.
//! Axes are 12-bit effective, packed into u16 LE fields:
//!
//! ```text
//! byte  0      : report_id (0x01)
//! bytes 1-2    : rudder      u16 LE, 0..=4095 (12-bit)
//! bytes 3-4    : brake_left  u16 LE, 0..=4095 (12-bit)
//! bytes 5-6    : brake_right u16 LE, 0..=4095 (12-bit)
//! ```
//!
//! ## Quirk: HALL_EFFECT_SENSORS
//!
//! All three axes use contactless Hall effect sensors.  No rudder curve
//! is applied in firmware — curve shaping is left to the host.
//!
//! ## Quirk: NO_SPRING_CENTERING (brake axes)
//!
//! Brake axes have no return spring; apply a small deadzone if drift occurs.

use crate::{Calibration, PedalVendor, PedalsAxes, PedalsInputState, calibration::AxisCalibration};
use thiserror::Error;

/// Minimum byte count for a VKB T-Rudder report.
pub const VKB_TRUDDER_MIN_REPORT_BYTES: usize = 7;

/// 12-bit maximum value.
const MAX_12BIT: u16 = 4095;

/// Parse error for VKB T-Rudder pedals.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VkbTRudderParseError {
    #[error("VKB T-Rudder report too short: got {0} bytes (need ≥7)")]
    TooShort(usize),
    #[error("VKB T-Rudder unexpected report ID: {0:#04X} (expected 0x01)")]
    InvalidReportId(u8),
}

fn vkb_default_cal() -> Calibration {
    Calibration {
        rudder: AxisCalibration::new(0, MAX_12BIT),
        left_toe: AxisCalibration::new(0, MAX_12BIT),
        right_toe: AxisCalibration::new(0, MAX_12BIT),
    }
}

/// Parse one raw HID report from VKB T-Rudder pedals.
pub fn parse_vkb_trudder_report(data: &[u8]) -> Result<PedalsInputState, VkbTRudderParseError> {
    parse_vkb_trudder_report_calibrated(data, &vkb_default_cal())
}

/// Parse with per-axis calibration overrides.
pub fn parse_vkb_trudder_report_calibrated(
    data: &[u8],
    cal: &Calibration,
) -> Result<PedalsInputState, VkbTRudderParseError> {
    if data.len() < VKB_TRUDDER_MIN_REPORT_BYTES {
        return Err(VkbTRudderParseError::TooShort(data.len()));
    }
    if data[0] != 0x01 {
        return Err(VkbTRudderParseError::InvalidReportId(data[0]));
    }

    let rudder_raw = u16::from_le_bytes([data[1], data[2]]) & 0x0FFF;
    let left_raw = u16::from_le_bytes([data[3], data[4]]) & 0x0FFF;
    let right_raw = u16::from_le_bytes([data[5], data[6]]) & 0x0FFF;

    Ok(PedalsInputState {
        vendor: PedalVendor::VkbTRudder,
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
        assert!(parse_vkb_trudder_report(&[0x01; 6]).is_err());
    }

    #[test]
    fn bad_report_id() {
        let mut r = make_report(0, 0, 0);
        r[0] = 0x00;
        assert!(matches!(
            parse_vkb_trudder_report(&r),
            Err(VkbTRudderParseError::InvalidReportId(0x00))
        ));
    }

    #[test]
    fn all_zero() {
        let state = parse_vkb_trudder_report(&make_report(0, 0, 0)).unwrap();
        assert_eq!(state.axes.rudder, 0.0);
        assert_eq!(state.axes.left_toe_brake, 0.0);
        assert_eq!(state.axes.right_toe_brake, 0.0);
    }

    #[test]
    fn max_12bit() {
        let state = parse_vkb_trudder_report(&make_report(4095, 4095, 4095)).unwrap();
        assert!((state.axes.rudder - 1.0).abs() < 1e-4);
        assert!((state.axes.left_toe_brake - 1.0).abs() < 1e-4);
        assert!((state.axes.right_toe_brake - 1.0).abs() < 1e-4);
    }

    #[test]
    fn center_12bit() {
        let state = parse_vkb_trudder_report(&make_report(2048, 0, 0)).unwrap();
        assert!((state.axes.rudder - 0.5).abs() < 0.01);
    }

    #[test]
    fn vendor_is_vkb() {
        let state = parse_vkb_trudder_report(&make_report(0, 0, 0)).unwrap();
        assert_eq!(state.vendor, PedalVendor::VkbTRudder);
    }

    proptest! {
        #[test]
        fn axes_always_in_range(
            rud in 0u16..=4095u16,
            left in 0u16..=4095u16,
            right in 0u16..=4095u16,
        ) {
            let report = make_report(rud, left, right);
            let state = parse_vkb_trudder_report(&report).unwrap();
            assert!((0.0..=1.0).contains(&state.axes.rudder));
            assert!((0.0..=1.0).contains(&state.axes.left_toe_brake));
            assert!((0.0..=1.0).contains(&state.axes.right_toe_brake));
        }

        #[test]
        fn random_report_no_panic(data in proptest::collection::vec(0u8..=255u8, 7..16)) {
            let _ = parse_vkb_trudder_report(&data);
        }
    }
}
