// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the Thrustmaster T.Flight Rudder Pedals (TFRP).
//!
//! # Device identifiers
//!
//! - TFRP: VID 0x044F, PID 0xB678 (confirmed via the-sz.com USB ID DB).
//! - T-Rudder: VID 0x044F, PID 0xB679 (confirmed via the-sz.com USB ID DB).
//!
//! # Input report layout (6 bytes)
//!
//! ```text
//! bytes 0-1 : Rz  (combined rudder)   u16 LE, 0..=65535, centre ~32767
//! bytes 2-3 : Z   (right pedal)       u16 LE, 0..=65535
//! bytes 4-5 : Rx  (left pedal)        u16 LE, 0..=65535
//! ```

use crate::{Calibration, PedalVendor, PedalsAxes, PedalsInputState};
use thiserror::Error;

/// Minimum byte count for a TFRP report.
pub const TFRP_MIN_REPORT_BYTES: usize = 6;

/// Parse error for TFRP pedals.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TfrpParseError {
    #[error("TFRP report too short: got {0} bytes (need ≥6)")]
    TooShort(usize),
}

/// Parse one raw HID report from Thrustmaster TFRP pedals into the unified model.
pub fn parse_tfrp_report(data: &[u8]) -> Result<PedalsInputState, TfrpParseError> {
    parse_tfrp_report_calibrated(data, &Calibration::identity())
}

/// Parse with per-axis calibration overrides.
pub fn parse_tfrp_report_calibrated(
    data: &[u8],
    cal: &Calibration,
) -> Result<PedalsInputState, TfrpParseError> {
    if data.len() < TFRP_MIN_REPORT_BYTES {
        return Err(TfrpParseError::TooShort(data.len()));
    }

    let rz = u16::from_le_bytes([data[0], data[1]]);
    let z = u16::from_le_bytes([data[2], data[3]]);
    let rx = u16::from_le_bytes([data[4], data[5]]);

    Ok(PedalsInputState {
        vendor: PedalVendor::ThrustmasterTfrp,
        axes: PedalsAxes {
            rudder: cal.rudder.normalize(rz),
            right_toe_brake: cal.right_toe.normalize(z),
            left_toe_brake: cal.left_toe.normalize(rx),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn make_report(rz: u16, z: u16, rx: u16) -> Vec<u8> {
        let mut data = Vec::with_capacity(6);
        data.extend_from_slice(&rz.to_le_bytes());
        data.extend_from_slice(&z.to_le_bytes());
        data.extend_from_slice(&rx.to_le_bytes());
        data
    }

    #[test]
    fn too_short_is_error() {
        assert!(parse_tfrp_report(&[0u8; 5]).is_err());
    }

    #[test]
    fn all_zero_is_zero() {
        let state = parse_tfrp_report(&make_report(0, 0, 0)).unwrap();
        assert_eq!(state.axes.rudder, 0.0);
        assert_eq!(state.axes.left_toe_brake, 0.0);
        assert_eq!(state.axes.right_toe_brake, 0.0);
    }

    #[test]
    fn max_is_one() {
        let state = parse_tfrp_report(&make_report(65535, 65535, 65535)).unwrap();
        assert!((state.axes.rudder - 1.0).abs() < 1e-4);
        assert!((state.axes.left_toe_brake - 1.0).abs() < 1e-4);
        assert!((state.axes.right_toe_brake - 1.0).abs() < 1e-4);
    }

    #[test]
    fn center_is_half() {
        let state = parse_tfrp_report(&make_report(32767, 32767, 32767)).unwrap();
        assert!((state.axes.rudder - 0.5).abs() < 0.01);
    }

    #[test]
    fn vendor_is_tfrp() {
        let state = parse_tfrp_report(&make_report(0, 0, 0)).unwrap();
        assert_eq!(state.vendor, PedalVendor::ThrustmasterTfrp);
    }

    #[test]
    fn extra_bytes_are_ignored() {
        let mut report = make_report(65535, 0, 0);
        report.extend_from_slice(&[0xFF; 10]);
        let state = parse_tfrp_report(&report).unwrap();
        assert!((state.axes.rudder - 1.0).abs() < 1e-4);
        assert_eq!(state.axes.right_toe_brake, 0.0);
    }

    proptest! {
        #[test]
        fn axes_always_in_range(rz in 0u16..=u16::MAX, z in 0u16..=u16::MAX, rx in 0u16..=u16::MAX) {
            let report = make_report(rz, z, rx);
            let state = parse_tfrp_report(&report).unwrap();
            assert!((0.0..=1.0).contains(&state.axes.rudder));
            assert!((0.0..=1.0).contains(&state.axes.left_toe_brake));
            assert!((0.0..=1.0).contains(&state.axes.right_toe_brake));
        }

        #[test]
        fn random_report_no_panic(data in proptest::collection::vec(0u8..=255u8, 6..12)) {
            let _ = parse_tfrp_report(&data);
        }
    }
}
