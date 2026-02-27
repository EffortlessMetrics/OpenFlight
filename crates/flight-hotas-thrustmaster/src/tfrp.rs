// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the Thrustmaster T.Flight Rudder Pedals (TFRP) and T-Rudder.
//!
//! # Confirmed device identifiers
//!
//! - T.Flight Rudder Pedals (TFRP): VID 0x044F, PID 0xB678 — from the-sz.com USB ID DB.
//! - T-Rudder: VID 0x044F, PID 0xB679 — from the-sz.com USB ID DB.
//!
//! # Input report layout (6 bytes, community-documented)
//!
//! The TFRP/T-Rudder expose three axes and no buttons via the standard HID
//! joystick descriptor. The raw report uses u16 LE values:
//!
//! ```text
//! bytes 0-1 : Rz  (combined rudder deflection)  u16 LE, 0..=65535, center ~32767
//! bytes 2-3 : Z   (right pedal independent)      u16 LE, 0..=65535
//! bytes 4-5 : Rx  (left pedal independent)       u16 LE, 0..=65535
//! ```
//!
//! OpenFlight normalises all axes to 0.0–1.0. For the combined rudder (Rz),
//! users who prefer −1.0..1.0 should apply a centre-subtract in their profile.
//!
//! ## Source
//!
//! Report layout from community SDL2 gamecontrollerdb TFRP entries, Linux
//! `evtest` captures shared on r/hotas, and DCS World TFRP axis calibration
//! threads on the ED forums.

use thiserror::Error;

/// Minimum byte count for a TFRP / T-Rudder report.
pub const TFRP_MIN_REPORT_BYTES: usize = 6;

/// Parse error for the TFRP/T-Rudder pedals.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TfrpParseError {
    #[error("TFRP report too short: got {0} bytes (need ≥6)")]
    TooShort(usize),
}

/// Normalised axes from the T.Flight Rudder Pedals or T-Rudder.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TfrpAxes {
    /// Combined rudder deflection (both pedals linked). 0.0=full-left, 0.5=center, 1.0=full-right.
    pub rudder: f32,
    /// Right pedal position (independent, for differential braking). 0.0=released, 1.0=fully pressed.
    pub right_pedal: f32,
    /// Left pedal position (independent, for differential braking). 0.0=released, 1.0=fully pressed.
    pub left_pedal: f32,
}

/// Full parsed input state from one TFRP / T-Rudder HID report.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TfrpInputState {
    pub axes: TfrpAxes,
}

/// Parse one raw HID report from the TFRP or T-Rudder pedals.
pub fn parse_tfrp_report(data: &[u8]) -> Result<TfrpInputState, TfrpParseError> {
    if data.len() < TFRP_MIN_REPORT_BYTES {
        return Err(TfrpParseError::TooShort(data.len()));
    }

    let normalize = |v: u16| (v as f32 / 65535.0f32).clamp(0.0, 1.0);

    let rz = u16::from_le_bytes([data[0], data[1]]);
    let z = u16::from_le_bytes([data[2], data[3]]);
    let rx = u16::from_le_bytes([data[4], data[5]]);

    Ok(TfrpInputState {
        axes: TfrpAxes {
            rudder: normalize(rz),
            right_pedal: normalize(z),
            left_pedal: normalize(rx),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn make_tfrp_report(rz: u16, z: u16, rx: u16) -> Vec<u8> {
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
    fn all_zero_is_all_zero() {
        let report = make_tfrp_report(0, 0, 0);
        let state = parse_tfrp_report(&report).unwrap();
        assert_eq!(state.axes.rudder, 0.0);
        assert_eq!(state.axes.right_pedal, 0.0);
        assert_eq!(state.axes.left_pedal, 0.0);
    }

    #[test]
    fn max_is_one() {
        let report = make_tfrp_report(65535, 65535, 65535);
        let state = parse_tfrp_report(&report).unwrap();
        assert!((state.axes.rudder - 1.0).abs() < 1e-4);
        assert!((state.axes.right_pedal - 1.0).abs() < 1e-4);
        assert!((state.axes.left_pedal - 1.0).abs() < 1e-4);
    }

    #[test]
    fn center_is_half() {
        let report = make_tfrp_report(32767, 32767, 32767);
        let state = parse_tfrp_report(&report).unwrap();
        assert!((state.axes.rudder - 0.5).abs() < 0.01);
    }

    #[test]
    fn extra_bytes_are_ignored() {
        let mut report = make_tfrp_report(65535, 0, 0);
        report.extend_from_slice(&[0xFF; 10]);
        let state = parse_tfrp_report(&report).unwrap();
        assert!((state.axes.rudder - 1.0).abs() < 1e-4);
        assert_eq!(state.axes.right_pedal, 0.0);
    }

    #[test]
    fn axes_are_independent() {
        let report = make_tfrp_report(0, 65535, 0);
        let state = parse_tfrp_report(&report).unwrap();
        assert_eq!(state.axes.rudder, 0.0);
        assert!((state.axes.right_pedal - 1.0).abs() < 1e-4);
        assert_eq!(state.axes.left_pedal, 0.0);
    }

    proptest! {
        #[test]
        fn axes_always_in_range(rz in 0u16..=u16::MAX, z in 0u16..=u16::MAX, rx in 0u16..=u16::MAX) {
            let report = make_tfrp_report(rz, z, rx);
            let state = parse_tfrp_report(&report).unwrap();
            assert!(state.axes.rudder >= 0.0 && state.axes.rudder <= 1.0);
            assert!(state.axes.right_pedal >= 0.0 && state.axes.right_pedal <= 1.0);
            assert!(state.axes.left_pedal >= 0.0 && state.axes.left_pedal <= 1.0);
        }

        #[test]
        fn random_report_does_not_panic(data in proptest::collection::vec(0u8..=255u8, 6..12)) {
            let _ = parse_tfrp_report(&data);
        }
    }
}
