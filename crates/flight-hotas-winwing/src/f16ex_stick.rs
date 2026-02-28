// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID report parser for the WinWing F-16EX Grip (PID 0xBEA8).
//!
//! Report layout (10 bytes, report ID `0x04`):
//!
//! | Offset | Size | Field |
//! |--------|------|-------|
//! | 0      | 1    | Report ID (`0x04`) |
//! | 1      | 2    | Roll axis (i16 LE, −32768..32767 → −1.0..1.0) |
//! | 3      | 2    | Pitch axis (i16 LE, −32768..32767 → −1.0..1.0) |
//! | 5      | 4    | Button bitmask (u32 LE, bits 0–19 used for buttons 1–20) |
//! | 9      | 1    | HAT (8-way: 0=N, 1=NE, 2=E, 3=SE, 4=S, 5=SW, 6=W, 7=NW, 0x0F=neutral) |

use thiserror::Error;

/// USB Product ID for the WinWing F-16EX Grip.
pub const F16EX_STICK_PID: u16 = 0xBEA8;

/// Minimum bytes required in a valid F-16EX HID report.
pub const MIN_REPORT_BYTES: usize = 10;

/// Number of mapped buttons on the F-16EX grip.
pub const BUTTON_COUNT: u8 = 20;

const REPORT_ID: u8 = 0x04;

// ── Axis snapshot ─────────────────────────────────────────────────────────────

/// Axis snapshot for the WinWing F-16EX Grip.
#[derive(Debug, Clone, PartialEq)]
pub struct F16ExAxes {
    /// Roll (left/right deflection) — \[−1.0, 1.0\].
    pub roll: f32,
    /// Pitch (forward/aft deflection) — \[−1.0, 1.0\].
    pub pitch: f32,
}

// ── Button snapshot ───────────────────────────────────────────────────────────

/// Button state for the WinWing F-16EX Grip (20 buttons + 1 HAT).
#[derive(Debug, Clone, Default)]
pub struct F16ExButtons {
    /// Bitmask for up to 20 buttons; bit `n−1` set → button `n` pressed.
    pub mask: u32,
    /// 8-way HAT position. `0`=N, `1`=NE … `7`=NW, `0x0F`=neutral.
    pub hat: u8,
}

impl F16ExButtons {
    /// Returns `true` if button `n` (1-indexed, 1–20) is pressed.
    pub fn is_pressed(&self, n: u8) -> bool {
        (1u8..=BUTTON_COUNT).contains(&n) && (self.mask >> (n - 1)) & 1 == 1
    }

    /// Returns `true` when the HAT is in its neutral (released) position.
    pub fn hat_neutral(&self) -> bool {
        self.hat == 0x0F
    }
}

// ── Input state ───────────────────────────────────────────────────────────────

/// Parsed state from a single F-16EX HID report.
#[derive(Debug, Clone)]
pub struct F16ExInputState {
    pub axes: F16ExAxes,
    pub buttons: F16ExButtons,
}

// ── Parser ────────────────────────────────────────────────────────────────────

/// Parse a raw HID report from the WinWing F-16EX Grip.
///
/// # Errors
///
/// Returns [`F16ExParseError::TooShort`] if `data` is shorter than
/// [`MIN_REPORT_BYTES`], or [`F16ExParseError::UnknownReportId`] if
/// `data[0]` is not `0x04`.
pub fn parse_f16ex_stick_report(data: &[u8]) -> Result<F16ExInputState, F16ExParseError> {
    if data.len() < MIN_REPORT_BYTES {
        return Err(F16ExParseError::TooShort {
            expected: MIN_REPORT_BYTES,
            got: data.len(),
        });
    }
    if data[0] != REPORT_ID {
        return Err(F16ExParseError::UnknownReportId { id: data[0] });
    }

    let roll = norm_i16(i16::from_le_bytes([data[1], data[2]]));
    let pitch = norm_i16(i16::from_le_bytes([data[3], data[4]]));
    let mask = u32::from_le_bytes([data[5], data[6], data[7], data[8]]);
    let hat = data[9];

    Ok(F16ExInputState {
        axes: F16ExAxes { roll, pitch },
        buttons: F16ExButtons { mask, hat },
    })
}

// ── Error type ────────────────────────────────────────────────────────────────

/// Errors returned by [`parse_f16ex_stick_report`].
#[derive(Debug, Error, PartialEq)]
pub enum F16ExParseError {
    #[error("report too short: expected ≥{expected} bytes, got {got}")]
    TooShort { expected: usize, got: usize },
    #[error("unknown report ID: 0x{id:02X}")]
    UnknownReportId { id: u8 },
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Normalise a signed 16-bit integer to the range \[−1.0, 1.0\].
fn norm_i16(v: i16) -> f32 {
    (v as f32 / 32767.0).clamp(-1.0, 1.0)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Build a minimal valid report with the given axis values; HAT defaults to neutral.
    fn make_report(roll: i16, pitch: i16) -> [u8; MIN_REPORT_BYTES] {
        let mut r = [0u8; MIN_REPORT_BYTES];
        r[0] = REPORT_ID;
        r[1..3].copy_from_slice(&roll.to_le_bytes());
        r[3..5].copy_from_slice(&pitch.to_le_bytes());
        r[9] = 0x0F; // HAT neutral
        r
    }

    #[test]
    fn test_centred() {
        let s = parse_f16ex_stick_report(&make_report(0, 0)).unwrap();
        assert!(s.axes.roll.abs() < 1e-4);
        assert!(s.axes.pitch.abs() < 1e-4);
    }

    #[test]
    fn test_full_right_roll() {
        let s = parse_f16ex_stick_report(&make_report(32767, 0)).unwrap();
        assert!((s.axes.roll - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_full_left_roll() {
        // i16::MIN / 32767 is just below -1.0; allow small overshoot.
        let s = parse_f16ex_stick_report(&make_report(i16::MIN, 0)).unwrap();
        assert!(s.axes.roll < 0.0);
    }

    #[test]
    fn test_full_forward_pitch() {
        let s = parse_f16ex_stick_report(&make_report(0, 32767)).unwrap();
        assert!((s.axes.pitch - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_button_detection() {
        let mut r = make_report(0, 0);
        r[5] = 0b0000_0101; // buttons 1 and 3
        let s = parse_f16ex_stick_report(&r).unwrap();
        assert!(s.buttons.is_pressed(1));
        assert!(!s.buttons.is_pressed(2));
        assert!(s.buttons.is_pressed(3));
    }

    #[test]
    fn test_button_boundary() {
        let mut r = make_report(0, 0);
        // Set bit 19 (button 20).
        r[5..9].copy_from_slice(&(1u32 << 19).to_le_bytes());
        let s = parse_f16ex_stick_report(&r).unwrap();
        assert!(s.buttons.is_pressed(20));
        assert!(!s.buttons.is_pressed(21));
    }

    #[test]
    fn test_button_out_of_range_returns_false() {
        let r = make_report(0, 0);
        let s = parse_f16ex_stick_report(&r).unwrap();
        assert!(!s.buttons.is_pressed(0));
        assert!(!s.buttons.is_pressed(21));
    }

    #[test]
    fn test_hat_neutral() {
        let r = make_report(0, 0);
        let s = parse_f16ex_stick_report(&r).unwrap();
        assert!(s.buttons.hat_neutral());
        assert_eq!(s.buttons.hat, 0x0F);
    }

    #[test]
    fn test_hat_north() {
        let mut r = make_report(0, 0);
        r[9] = 0x00;
        let s = parse_f16ex_stick_report(&r).unwrap();
        assert!(!s.buttons.hat_neutral());
        assert_eq!(s.buttons.hat, 0x00);
    }

    #[test]
    fn test_report_too_short() {
        let err = parse_f16ex_stick_report(&[0u8; 5]).unwrap_err();
        assert_eq!(
            err,
            F16ExParseError::TooShort {
                expected: MIN_REPORT_BYTES,
                got: 5
            }
        );
    }

    #[test]
    fn test_wrong_report_id() {
        let mut r = make_report(0, 0);
        r[0] = 0xFF;
        let err = parse_f16ex_stick_report(&r).unwrap_err();
        assert_eq!(err, F16ExParseError::UnknownReportId { id: 0xFF });
    }

    proptest! {
        #[test]
        fn prop_roll_always_in_range(raw: i16) {
            let s = parse_f16ex_stick_report(&make_report(raw, 0)).unwrap();
            prop_assert!(
                (-1.001..=1.001).contains(&s.axes.roll),
                "roll out of range: {}",
                s.axes.roll
            );
        }

        #[test]
        fn prop_pitch_always_in_range(raw: i16) {
            let s = parse_f16ex_stick_report(&make_report(0, raw)).unwrap();
            prop_assert!(
                (-1.001..=1.001).contains(&s.axes.pitch),
                "pitch out of range: {}",
                s.axes.pitch
            );
        }
    }
}
