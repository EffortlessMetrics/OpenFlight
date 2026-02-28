// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the Brunner CLS-E Force Feedback Yoke.
//!
//! # Device identifier
//!
//! VID 0x25BB (Brunner Elektronik AG), PID 0x0063 (PRT.5105 [Yoke]).
//! VID confirmed from linux-usb.org; PID confirmed from the-sz.com USB ID database.
//!
//! # Input report layout (9 bytes minimum)
//!
//! ```text
//! byte  0     : report_id (0x01)
//! bytes 1–2   : roll  / X axis (i16 LE, bipolar: −32768…+32767)
//! bytes 3–4   : pitch / Y axis (i16 LE, bipolar: −32768…+32767)
//! bytes 5–8   : button bytes (32 buttons, LSB-first across 4 bytes)
//! ```
//!
//! Axis values are normalised to −1.0…+1.0 using `value / 32767.0` (clamped).
//! The most-negative i16 value (−32768) is clamped to −1.0 to avoid overflow.
//!
//! **Note:** Report layout inferred from Brunner SDK and USB registry data.
//! HIL validation has not been performed. Byte order requires confirmation
//! from real hardware (see quirk `HIL_NOT_VALIDATED` in `cls-e.yaml`).

use thiserror::Error;

/// Minimum byte count for a CLS-E HID input report.
pub const CLS_E_MIN_REPORT_BYTES: usize = 9;

const AXIS_SCALE: f32 = 32767.0;
const CLS_E_BUTTON_BYTES: usize = 4;

/// Parse error for the Brunner CLS-E yoke.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ClsEParseError {
    #[error("CLS-E report too short: got {0} bytes (need ≥{CLS_E_MIN_REPORT_BYTES})")]
    TooShort(usize),
}

/// Normalised axes from the Brunner CLS-E yoke.
///
/// Both values are in the range `[−1.0, +1.0]`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ClsEAxes {
    /// Roll / aileron axis (X). −1.0 = full left, 0.0 = centre, +1.0 = full right.
    pub roll: f32,
    /// Pitch / elevator axis (Y). −1.0 = full forward, 0.0 = centre, +1.0 = full back.
    pub pitch: f32,
}

/// Button state from the Brunner CLS-E yoke (up to 32 buttons).
///
/// Buttons 1–32 are mapped LSB-first across four bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ClsEButtons {
    /// Raw button bytes covering buttons 1–32.
    pub raw: [u8; CLS_E_BUTTON_BYTES],
}

impl ClsEButtons {
    /// Return `true` if button `n` (1-indexed, 1..=32) is pressed.
    pub fn is_pressed(&self, n: u8) -> bool {
        if !(1..=32).contains(&n) {
            return false;
        }
        let idx = (n - 1) as usize;
        let byte = idx / 8;
        let bit = idx % 8;
        (self.raw[byte] >> bit) & 1 == 1
    }

    /// Return a `Vec` of pressed button numbers (1-indexed, 1..=32).
    pub fn pressed(&self) -> Vec<u8> {
        (1u8..=32).filter(|&n| self.is_pressed(n)).collect()
    }
}

/// Full parsed input state from one Brunner CLS-E HID input report.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ClsEInputState {
    pub axes: ClsEAxes,
    pub buttons: ClsEButtons,
}

/// Parse one raw HID input report from the Brunner CLS-E yoke.
///
/// The first byte is expected to be the report ID and is skipped.
/// Axis values are i16 little-endian and normalised to −1.0…+1.0.
pub fn parse_cls_e_report(data: &[u8]) -> Result<ClsEInputState, ClsEParseError> {
    if data.len() < CLS_E_MIN_REPORT_BYTES {
        return Err(ClsEParseError::TooShort(data.len()));
    }

    // skip report_id byte at index 0
    let roll_raw = i16::from_le_bytes([data[1], data[2]]);
    let pitch_raw = i16::from_le_bytes([data[3], data[4]]);

    let normalise = |v: i16| (v as f32 / AXIS_SCALE).clamp(-1.0, 1.0);

    let axes = ClsEAxes {
        roll: normalise(roll_raw),
        pitch: normalise(pitch_raw),
    };

    let mut raw_buttons = [0u8; CLS_E_BUTTON_BYTES];
    raw_buttons.copy_from_slice(&data[5..9]);

    Ok(ClsEInputState {
        axes,
        buttons: ClsEButtons { raw: raw_buttons },
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn make_report(roll: i16, pitch: i16, buttons: [u8; 4]) -> Vec<u8> {
        let mut data = vec![0x01u8]; // report_id
        data.extend_from_slice(&roll.to_le_bytes());
        data.extend_from_slice(&pitch.to_le_bytes());
        data.extend_from_slice(&buttons);
        data
    }

    // ── Error cases ───────────────────────────────────────────────────────────

    #[test]
    fn empty_slice_returns_error() {
        assert!(parse_cls_e_report(&[]).is_err());
    }

    #[test]
    fn too_short_by_one_returns_error() {
        let data = vec![0u8; CLS_E_MIN_REPORT_BYTES - 1];
        let err = parse_cls_e_report(&data).unwrap_err();
        assert_eq!(err, ClsEParseError::TooShort(CLS_E_MIN_REPORT_BYTES - 1));
    }

    #[test]
    fn error_message_contains_byte_count() {
        let err = parse_cls_e_report(&[0x01; 5]).unwrap_err();
        assert!(err.to_string().contains('5'));
    }

    // ── Happy-path parsing ────────────────────────────────────────────────────

    #[test]
    fn exactly_min_bytes_parses_ok() {
        let report = make_report(0, 0, [0u8; 4]);
        assert_eq!(report.len(), CLS_E_MIN_REPORT_BYTES);
        assert!(parse_cls_e_report(&report).is_ok());
    }

    #[test]
    fn longer_report_does_not_error() {
        let mut report = make_report(0, 0, [0u8; 4]);
        report.extend_from_slice(&[0u8; 10]);
        assert!(parse_cls_e_report(&report).is_ok());
    }

    #[test]
    fn zero_axes_parse_to_zero() {
        let report = make_report(0, 0, [0u8; 4]);
        let state = parse_cls_e_report(&report).unwrap();
        assert_eq!(state.axes.roll, 0.0);
        assert_eq!(state.axes.pitch, 0.0);
    }

    #[test]
    fn max_positive_axis_parses_to_one() {
        let report = make_report(i16::MAX, i16::MAX, [0u8; 4]);
        let state = parse_cls_e_report(&report).unwrap();
        assert!(
            (state.axes.roll - 1.0).abs() < 1e-4,
            "roll={}",
            state.axes.roll
        );
        assert!(
            (state.axes.pitch - 1.0).abs() < 1e-4,
            "pitch={}",
            state.axes.pitch
        );
    }

    #[test]
    fn max_negative_axis_parses_to_minus_one() {
        // i16::MIN (-32768) is clamped to -1.0 to prevent overflow
        let report = make_report(i16::MIN, i16::MIN, [0u8; 4]);
        let state = parse_cls_e_report(&report).unwrap();
        assert_eq!(state.axes.roll, -1.0);
        assert_eq!(state.axes.pitch, -1.0);
    }

    #[test]
    fn negative_32767_parses_to_minus_one() {
        let report = make_report(-32767, -32767, [0u8; 4]);
        let state = parse_cls_e_report(&report).unwrap();
        assert!((state.axes.roll - (-1.0)).abs() < 1e-4);
        assert!((state.axes.pitch - (-1.0)).abs() < 1e-4);
    }

    // ── Button extraction ─────────────────────────────────────────────────────

    #[test]
    fn no_buttons_pressed_by_default() {
        let report = make_report(0, 0, [0u8; 4]);
        let state = parse_cls_e_report(&report).unwrap();
        assert!(state.buttons.pressed().is_empty());
    }

    #[test]
    fn button_1_detected() {
        let mut buttons = [0u8; 4];
        buttons[0] = 0x01; // bit 0 of byte 0 → button 1
        let report = make_report(0, 0, buttons);
        let state = parse_cls_e_report(&report).unwrap();
        assert!(state.buttons.is_pressed(1));
        assert!(!state.buttons.is_pressed(2));
    }

    #[test]
    fn button_8_detected() {
        let mut buttons = [0u8; 4];
        buttons[0] = 0x80; // bit 7 of byte 0 → button 8
        let report = make_report(0, 0, buttons);
        let state = parse_cls_e_report(&report).unwrap();
        assert!(state.buttons.is_pressed(8));
        assert!(!state.buttons.is_pressed(7));
        assert!(!state.buttons.is_pressed(9));
    }

    #[test]
    fn button_32_detected() {
        let mut buttons = [0u8; 4];
        // button 32 = index 31 → byte 3 (31/8=3), bit 7 (31%8=7)
        buttons[3] = 0x80;
        let report = make_report(0, 0, buttons);
        let state = parse_cls_e_report(&report).unwrap();
        assert!(state.buttons.is_pressed(32));
        assert!(!state.buttons.is_pressed(31));
    }

    #[test]
    fn out_of_range_button_returns_false() {
        let report = make_report(0, 0, [0xFFu8; 4]);
        let state = parse_cls_e_report(&report).unwrap();
        assert!(!state.buttons.is_pressed(0));
        assert!(!state.buttons.is_pressed(33));
        assert!(!state.buttons.is_pressed(255));
    }

    #[test]
    fn all_buttons_pressed() {
        let report = make_report(0, 0, [0xFFu8; 4]);
        let state = parse_cls_e_report(&report).unwrap();
        let pressed = state.buttons.pressed();
        assert_eq!(pressed.len(), 32);
        for n in 1u8..=32 {
            assert!(state.buttons.is_pressed(n), "button {n} should be pressed");
        }
    }

    // ── proptest ──────────────────────────────────────────────────────────────

    proptest! {
        #[test]
        fn axes_always_in_range(roll in i16::MIN..=i16::MAX, pitch in i16::MIN..=i16::MAX) {
            let report = make_report(roll, pitch, [0u8; 4]);
            let state = parse_cls_e_report(&report).unwrap();
            prop_assert!((-1.0..=1.0).contains(&state.axes.roll),
                "roll={} out of range for raw={}", state.axes.roll, roll);
            prop_assert!((-1.0..=1.0).contains(&state.axes.pitch),
                "pitch={} out of range for raw={}", state.axes.pitch, pitch);
        }

        #[test]
        fn random_report_does_not_panic(data in proptest::collection::vec(0u8..=255u8, 9..32)) {
            // Must not panic — result may be Ok or Err
            let _ = parse_cls_e_report(&data);
        }
    }
}
