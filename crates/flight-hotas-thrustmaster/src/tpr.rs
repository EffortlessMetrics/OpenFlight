// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the Thrustmaster T-Pendular Rudder (TPR).
//!
//! # Confirmed device identifiers
//!
//! - T-Pendular Rudder (standard): VID 0x044F, PID 0xB68F — linux-hardware.org (7 probes).
//! - T-Pendular Rudder (Bulk):     VID 0x044F, PID 0xB68E — linux-hardware.org.
//!
//! # Input report layout (6 bytes)
//!
//! The TPR exposes the same three-axis report as the TFRP:
//!
//! ```text
//! bytes 0-1 : Rz  (combined rudder deflection)  u16 LE, 0..=65535, centre ~32767
//! bytes 2-3 : Z   (right pedal independent)      u16 LE, 0..=65535
//! bytes 4-5 : Rx  (left pedal independent)       u16 LE, 0..=65535
//! ```
//!
//! OpenFlight normalises all axes to 0.0–1.0.  For the combined rudder (Rz),
//! users who prefer −1.0..1.0 should apply a centre-subtract in their profile.
//!
//! ## Differences from TFRP
//!
//! The TPR uses a pendular (swinging) mechanism giving longer pedal travel and
//! a more realistic feel, but the USB HID report layout is identical to the
//! TFRP (`crate::tfrp`).  This module re-exports the shared parser with
//! TPR-specific type aliases and constants.

pub use crate::tfrp::{
    TfrpAxes as TprAxes, TfrpInputState as TprInputState, TfrpParseError as TprParseError,
    parse_tfrp_report as parse_tpr_report,
};

/// Minimum byte count for a TPR report (same layout as TFRP).
pub const TPR_MIN_REPORT_BYTES: usize = crate::tfrp::TFRP_MIN_REPORT_BYTES;

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tpr_report(rz: u16, z: u16, rx: u16) -> Vec<u8> {
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
        let report = make_tpr_report(65535, 65535, 65535);
        let state = parse_tpr_report(&report).unwrap();
        assert!((state.axes.rudder - 1.0).abs() < 1e-4);
        assert!((state.axes.right_pedal - 1.0).abs() < 1e-4);
        assert!((state.axes.left_pedal - 1.0).abs() < 1e-4);
    }

    #[test]
    fn all_zero_is_zero() {
        let report = make_tpr_report(0, 0, 0);
        let state = parse_tpr_report(&report).unwrap();
        assert_eq!(state.axes.rudder, 0.0);
        assert_eq!(state.axes.right_pedal, 0.0);
        assert_eq!(state.axes.left_pedal, 0.0);
    }
}
