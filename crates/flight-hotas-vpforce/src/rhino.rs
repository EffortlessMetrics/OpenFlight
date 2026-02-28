// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the VPforce Rhino FFB joystick base.
//!
//! # Report layout (20 bytes)
//!
//! ```text
//! byte  0         : report_id (always 0x01)
//! bytes  1– 2     : X  (roll),            i16 LE  →  normalised [−1.0, 1.0]
//! bytes  3– 4     : Y  (pitch),           i16 LE  →  normalised [−1.0, 1.0]
//! bytes  5– 6     : Z  (throttle slider), i16 LE  →  remapped   [ 0.0, 1.0]
//! bytes  7– 8     : Rx (rocker),          i16 LE  →  normalised [−1.0, 1.0]
//! bytes  9–10     : Ry (unused),          i16 LE  →  normalised [−1.0, 1.0]
//! bytes 11–12     : Rz (twist),           i16 LE  →  normalised [−1.0, 1.0]
//! bytes 13–16     : button bitmask, u32 LE (bit 0 = button 1)
//! byte  17        : POV hat (0=N,1=NE,2=E,3=SE,4=S,5=SW,6=W,7=NW; 0xFF=centred)
//! bytes 18–19     : reserved
//! ```
//!
//! Source: reverse-engineered from the VPforce Rhino firmware and validated by
//! the property tests in `flight-ffb-vpforce::input`. See also
//! `compat/devices/vpforce/rhino.yaml`.

use thiserror::Error;

/// Minimum byte count for a valid Rhino HID input report (including report ID).
pub const RHINO_MIN_REPORT_BYTES: usize = 20;

/// Normalised axis snapshot from one Rhino HID input report.
///
/// All signed axes are in `[−1.0, 1.0]`; the throttle slider is remapped to
/// `[0.0, 1.0]` (fully aft = 0.0, fully forward = 1.0).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RhinoAxes {
    /// Roll (stick left/right, HID X).  −1.0 = full left, 0.0 = centre, 1.0 = full right.
    pub roll: f32,
    /// Pitch (stick fore/aft, HID Y).   −1.0 = full forward, 1.0 = full back.
    pub pitch: f32,
    /// Throttle slider (HID Z).  0.0 = minimum, 1.0 = maximum.
    pub throttle: f32,
    /// Rocker / toe-brake axis (HID Rx).  −1.0 = left, 1.0 = right.
    pub rocker: f32,
    /// Twist (HID Rz).  −1.0 = full left twist, 1.0 = full right twist.
    pub twist: f32,
    /// Auxiliary axis (HID Ry, not wired in default Rhino firmware).
    pub ry: f32,
}

/// Button and hat state from one Rhino HID input report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RhinoButtons {
    /// 32-bit bitmask of pressed buttons.  Bit 0 corresponds to button 1.
    pub mask: u32,
    /// POV hat switch value.
    ///
    /// `0`=North, `1`=NE, `2`=East, `3`=SE, `4`=South, `5`=SW, `6`=West,
    /// `7`=NW, `0xFF`=centred.  Any other value should be treated as centred.
    pub hat: u8,
}

impl RhinoButtons {
    /// Returns `true` if button `n` (1-based, 1..=32) is currently pressed.
    pub fn is_pressed(&self, n: u8) -> bool {
        (1u8..=32).contains(&n) && (self.mask >> (n - 1)) & 1 == 1
    }

    /// Returns a `Vec` of all pressed button numbers (1-indexed, 1..=32).
    pub fn pressed(&self) -> Vec<u8> {
        (1u8..=32).filter(|&n| self.is_pressed(n)).collect()
    }
}

/// Fully parsed input state from one VPforce Rhino HID report.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RhinoInputState {
    pub axes: RhinoAxes,
    pub buttons: RhinoButtons,
}

/// Error type returned by [`parse_rhino_report`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RhinoParseError {
    #[error("Rhino report too short: expected ≥{expected} bytes, got {got}")]
    TooShort { expected: usize, got: usize },
    #[error("unknown report ID: 0x{id:02X} (expected 0x01)")]
    UnknownReportId { id: u8 },
}

/// Parse a raw HID input report from the VPforce Rhino.
///
/// # Errors
///
/// Returns [`RhinoParseError::TooShort`] if `data` has fewer than
/// [`RHINO_MIN_REPORT_BYTES`] bytes, or [`RhinoParseError::UnknownReportId`]
/// if byte 0 is not `0x01`.
pub fn parse_rhino_report(data: &[u8]) -> Result<RhinoInputState, RhinoParseError> {
    if data.len() < RHINO_MIN_REPORT_BYTES {
        return Err(RhinoParseError::TooShort {
            expected: RHINO_MIN_REPORT_BYTES,
            got: data.len(),
        });
    }
    if data[0] != 0x01 {
        return Err(RhinoParseError::UnknownReportId { id: data[0] });
    }

    let roll = norm_signed(read_i16(data, 1));
    let pitch = norm_signed(read_i16(data, 3));
    // Z throttle slider: remap [−1.0, 1.0] → [0.0, 1.0]
    let throttle = (norm_signed(read_i16(data, 5)) + 1.0) * 0.5;
    let rocker = norm_signed(read_i16(data, 7));
    let ry = norm_signed(read_i16(data, 9));
    let twist = norm_signed(read_i16(data, 11));

    let mask = u32::from_le_bytes([data[13], data[14], data[15], data[16]]);
    let hat = data[17];

    Ok(RhinoInputState {
        axes: RhinoAxes {
            roll,
            pitch,
            throttle,
            rocker,
            twist,
            ry,
        },
        buttons: RhinoButtons { mask, hat },
    })
}

#[inline]
fn read_i16(data: &[u8], offset: usize) -> i16 {
    i16::from_le_bytes([data[offset], data[offset + 1]])
}

#[inline]
fn norm_signed(v: i16) -> f32 {
    (v as f32 / 32767.0_f32).clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn make_report(axes: [i16; 6], buttons: u32, hat: u8) -> [u8; RHINO_MIN_REPORT_BYTES] {
        let mut r = [0u8; RHINO_MIN_REPORT_BYTES];
        r[0] = 0x01;
        for (i, &ax) in axes.iter().enumerate() {
            let offset = 1 + i * 2;
            let le = ax.to_le_bytes();
            r[offset] = le[0];
            r[offset + 1] = le[1];
        }
        let btn = buttons.to_le_bytes();
        r[13] = btn[0];
        r[14] = btn[1];
        r[15] = btn[2];
        r[16] = btn[3];
        r[17] = hat;
        r
    }

    #[test]
    fn too_short_returns_error() {
        assert!(parse_rhino_report(&[0x01; 19]).is_err());
    }

    #[test]
    fn empty_slice_returns_error() {
        assert!(parse_rhino_report(&[]).is_err());
    }

    #[test]
    fn wrong_report_id_returns_error() {
        let mut r = [0u8; RHINO_MIN_REPORT_BYTES];
        r[0] = 0x02;
        assert!(matches!(
            parse_rhino_report(&r),
            Err(RhinoParseError::UnknownReportId { id: 0x02 })
        ));
    }

    #[test]
    fn centred_report_gives_zero_axes() {
        let r = make_report([0i16; 6], 0, 0xFF);
        let state = parse_rhino_report(&r).unwrap();
        assert!(state.axes.roll.abs() < 1e-4);
        assert!(state.axes.pitch.abs() < 1e-4);
        assert!((state.axes.throttle - 0.5).abs() < 1e-3);
        assert!(state.axes.twist.abs() < 1e-4);
        assert!(state.axes.rocker.abs() < 1e-4);
    }

    #[test]
    fn full_positive_roll_gives_one() {
        let r = make_report([i16::MAX, 0, 0, 0, 0, 0], 0, 0xFF);
        let state = parse_rhino_report(&r).unwrap();
        assert!((state.axes.roll - 1.0).abs() < 1e-4);
    }

    #[test]
    fn full_negative_roll_gives_minus_one() {
        let r = make_report([i16::MIN, 0, 0, 0, 0, 0], 0, 0xFF);
        let state = parse_rhino_report(&r).unwrap();
        assert!((state.axes.roll + 1.0).abs() < 1e-3);
    }

    #[test]
    fn throttle_min_gives_zero() {
        let r = make_report([0, 0, i16::MIN, 0, 0, 0], 0, 0xFF);
        let state = parse_rhino_report(&r).unwrap();
        assert!(state.axes.throttle < 0.01);
    }

    #[test]
    fn throttle_max_gives_one() {
        let r = make_report([0, 0, i16::MAX, 0, 0, 0], 0, 0xFF);
        let state = parse_rhino_report(&r).unwrap();
        assert!(state.axes.throttle > 0.99);
    }

    #[test]
    fn button_1_pressed() {
        let r = make_report([0i16; 6], 0b0000_0001, 0xFF);
        let state = parse_rhino_report(&r).unwrap();
        assert!(state.buttons.is_pressed(1));
        assert!(!state.buttons.is_pressed(2));
    }

    #[test]
    fn button_32_pressed() {
        let r = make_report([0i16; 6], 1u32 << 31, 0xFF);
        let state = parse_rhino_report(&r).unwrap();
        assert!(state.buttons.is_pressed(32));
        assert!(!state.buttons.is_pressed(31));
    }

    #[test]
    fn button_0_and_33_always_false() {
        let r = make_report([0i16; 6], u32::MAX, 0xFF);
        let state = parse_rhino_report(&r).unwrap();
        assert!(!state.buttons.is_pressed(0));
        assert!(!state.buttons.is_pressed(33));
    }

    #[test]
    fn hat_parsed_correctly() {
        let r = make_report([0i16; 6], 0, 0x02); // East
        let state = parse_rhino_report(&r).unwrap();
        assert_eq!(state.buttons.hat, 0x02);
    }

    #[test]
    fn longer_report_does_not_error() {
        let mut v = make_report([0i16; 6], 0, 0xFF).to_vec();
        v.extend_from_slice(&[0u8; 8]);
        assert!(parse_rhino_report(&v).is_ok());
    }

    #[test]
    fn error_message_contains_byte_count() {
        let err = parse_rhino_report(&[0x01; 5]).unwrap_err();
        assert!(err.to_string().contains('5'));
    }

    proptest! {
        #[test]
        fn axes_always_in_range(
            roll in i16::MIN..=i16::MAX,
            pitch in i16::MIN..=i16::MAX,
            z in i16::MIN..=i16::MAX,
            rx in i16::MIN..=i16::MAX,
            ry in i16::MIN..=i16::MAX,
            rz in i16::MIN..=i16::MAX,
        ) {
            let r = make_report([roll, pitch, z, rx, ry, rz], 0, 0xFF);
            let state = parse_rhino_report(&r).unwrap();
            prop_assert!((-1.0..=1.0).contains(&state.axes.roll));
            prop_assert!((-1.0..=1.0).contains(&state.axes.pitch));
            prop_assert!((0.0..=1.0).contains(&state.axes.throttle));
            prop_assert!((-1.0..=1.0).contains(&state.axes.rocker));
            prop_assert!((-1.0..=1.0).contains(&state.axes.twist));
            prop_assert!((-1.0..=1.0).contains(&state.axes.ry));
        }

        #[test]
        fn random_report_does_not_panic(data in proptest::collection::vec(0u8..=255u8, 20..40)) {
            let _ = parse_rhino_report(&data);
        }
    }
}
