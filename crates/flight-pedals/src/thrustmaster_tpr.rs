// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the Thrustmaster T-Pendular Rudder (TPR).
//!
//! # Device identifiers
//!
//! - TPR (standard): VID 0x044F, PID 0xB68F (confirmed via linux-hardware.org).
//! - TPR (bulk):     VID 0x044F, PID 0xB68E (confirmed via linux-hardware.org).
//!
//! The TPR uses the same 6-byte report layout as the TFRP.  This module
//! re-exports the shared parser with TPR-specific type aliases.

use crate::{Calibration, PedalVendor, PedalsAxes, PedalsInputState};
use thiserror::Error;

/// Minimum byte count for a TPR report (same layout as TFRP).
pub const TPR_MIN_REPORT_BYTES: usize = 6;

/// Parse error for TPR pedals.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TprParseError {
    #[error("TPR report too short: got {0} bytes (need ≥6)")]
    TooShort(usize),
}

/// Parse one raw HID report from Thrustmaster TPR pedals.
pub fn parse_tpr_report(data: &[u8]) -> Result<PedalsInputState, TprParseError> {
    parse_tpr_report_calibrated(data, &Calibration::identity())
}

/// Parse with per-axis calibration overrides.
pub fn parse_tpr_report_calibrated(
    data: &[u8],
    cal: &Calibration,
) -> Result<PedalsInputState, TprParseError> {
    if data.len() < TPR_MIN_REPORT_BYTES {
        return Err(TprParseError::TooShort(data.len()));
    }

    let rz = u16::from_le_bytes([data[0], data[1]]);
    let z = u16::from_le_bytes([data[2], data[3]]);
    let rx = u16::from_le_bytes([data[4], data[5]]);

    Ok(PedalsInputState {
        vendor: PedalVendor::ThrustmasterTpr,
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

    fn make_report(rz: u16, z: u16, rx: u16) -> Vec<u8> {
        let mut data = Vec::with_capacity(6);
        data.extend_from_slice(&rz.to_le_bytes());
        data.extend_from_slice(&z.to_le_bytes());
        data.extend_from_slice(&rx.to_le_bytes());
        data
    }

    #[test]
    fn too_short_is_error() {
        assert!(parse_tpr_report(&[0u8; 5]).is_err());
    }

    #[test]
    fn max_all_axes() {
        let state = parse_tpr_report(&make_report(65535, 65535, 65535)).unwrap();
        assert!((state.axes.rudder - 1.0).abs() < 1e-4);
        assert!((state.axes.left_toe_brake - 1.0).abs() < 1e-4);
        assert!((state.axes.right_toe_brake - 1.0).abs() < 1e-4);
    }

    #[test]
    fn vendor_is_tpr() {
        let state = parse_tpr_report(&make_report(0, 0, 0)).unwrap();
        assert_eq!(state.vendor, PedalVendor::ThrustmasterTpr);
    }

    #[test]
    fn all_zero_is_zero() {
        let state = parse_tpr_report(&make_report(0, 0, 0)).unwrap();
        assert_eq!(state.axes.rudder, 0.0);
        assert_eq!(state.axes.left_toe_brake, 0.0);
        assert_eq!(state.axes.right_toe_brake, 0.0);
    }
}
