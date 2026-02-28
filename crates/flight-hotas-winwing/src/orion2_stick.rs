// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID report parser for the WinWing Orion 2 F/A-18C Stick (PID 0xBE63).
//!
//! Report layout (12 bytes, report ID `0x02`):
//!
//! | Offset | Size | Field |
//! |--------|------|-------|
//! | 0      | 1    | Report ID (`0x02`) |
//! | 1      | 2    | Roll axis (i16 LE, −32768..32767 → −1.0..1.0) |
//! | 3      | 2    | Pitch axis (i16 LE, −32768..32767 → −1.0..1.0) |
//! | 5      | 4    | Button bitmask (u32 LE, bits 0–19 used for buttons 1–20) |
//! | 9      | 1    | HAT A (8-way: 0=N, 1=NE, 2=E, 3=SE, 4=S, 5=SW, 6=W, 7=NW, 0x0F=neutral) |
//! | 10     | 1    | HAT B (same encoding as HAT A) |
//! | 11     | 1    | Reserved |

use thiserror::Error;

/// USB Product ID for the WinWing Orion 2 F/A-18C Stick.
///
/// Note: This is the same as `ORION2_F18_STICK_PID` in the generic `input` module.
/// This dedicated module provides strongly-typed structs and an ergonomic API.
pub const ORION2_STICK_PID: u16 = 0xBE63;

/// Minimum bytes required in a valid Orion 2 Stick HID report.
pub const MIN_REPORT_BYTES: usize = 12;

/// Number of mapped buttons on the Orion 2 F/A-18C Stick.
pub const BUTTON_COUNT: u8 = 20;

const REPORT_ID: u8 = 0x02;

/// HAT neutral position value.
const HAT_NEUTRAL: u8 = 0x0F;

// ── Axis snapshot ─────────────────────────────────────────────────────────────

/// Axis snapshot for the WinWing Orion 2 F/A-18C Stick.
#[derive(Debug, Clone, PartialEq)]
pub struct Orion2StickAxes {
    /// Roll (left/right deflection) — \[−1.0, 1.0\].
    pub roll: f32,
    /// Pitch (forward/aft deflection) — \[−1.0, 1.0\].
    pub pitch: f32,
}

// ── Button snapshot ───────────────────────────────────────────────────────────

/// Button and HAT state for the WinWing Orion 2 F/A-18C Stick (20 buttons + 2 HATs).
#[derive(Debug, Clone, Default)]
pub struct Orion2StickButtons {
    /// Bitmask for up to 20 buttons; bit `n−1` set → button `n` pressed.
    pub mask: u32,
    /// First 8-way HAT position. `0`=N, `1`=NE … `7`=NW, `0x0F`=neutral.
    pub hat_a: u8,
    /// Second 8-way HAT position. `0`=N, `1`=NE … `7`=NW, `0x0F`=neutral.
    pub hat_b: u8,
}

impl Orion2StickButtons {
    /// Returns `true` if button `n` (1-indexed, 1–20) is pressed.
    pub fn is_pressed(&self, n: u8) -> bool {
        (1u8..=BUTTON_COUNT).contains(&n) && (self.mask >> (n - 1)) & 1 == 1
    }

    /// Returns `true` when HAT A is in its neutral (released) position.
    pub fn hat_a_neutral(&self) -> bool {
        self.hat_a == HAT_NEUTRAL
    }

    /// Returns `true` when HAT B is in its neutral (released) position.
    pub fn hat_b_neutral(&self) -> bool {
        self.hat_b == HAT_NEUTRAL
    }
}

// ── Input state ───────────────────────────────────────────────────────────────

/// Parsed state from a single Orion 2 F/A-18C Stick HID report.
#[derive(Debug, Clone)]
pub struct Orion2StickInputState {
    pub axes: Orion2StickAxes,
    pub buttons: Orion2StickButtons,
}

// ── Parser ────────────────────────────────────────────────────────────────────

/// Parse a raw HID report from the WinWing Orion 2 F/A-18C Stick.
///
/// # Errors
///
/// Returns [`Orion2StickParseError::TooShort`] if `data` is shorter than
/// [`MIN_REPORT_BYTES`], or [`Orion2StickParseError::UnknownReportId`] if
/// `data[0]` is not `0x02`.
pub fn parse_orion2_stick_report(
    data: &[u8],
) -> Result<Orion2StickInputState, Orion2StickParseError> {
    if data.len() < MIN_REPORT_BYTES {
        return Err(Orion2StickParseError::TooShort {
            expected: MIN_REPORT_BYTES,
            got: data.len(),
        });
    }
    if data[0] != REPORT_ID {
        return Err(Orion2StickParseError::UnknownReportId { id: data[0] });
    }

    let roll = norm_i16(i16::from_le_bytes([data[1], data[2]]));
    let pitch = norm_i16(i16::from_le_bytes([data[3], data[4]]));
    let mask = u32::from_le_bytes([data[5], data[6], data[7], data[8]]);
    let hat_a = data[9];
    let hat_b = data[10];

    Ok(Orion2StickInputState {
        axes: Orion2StickAxes { roll, pitch },
        buttons: Orion2StickButtons { mask, hat_a, hat_b },
    })
}

// ── Error type ────────────────────────────────────────────────────────────────

/// Errors returned by [`parse_orion2_stick_report`].
#[derive(Debug, Error, PartialEq)]
pub enum Orion2StickParseError {
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

    /// Build a minimal valid report with the given axis values; both HATs default to neutral.
    fn make_report(roll: i16, pitch: i16) -> [u8; MIN_REPORT_BYTES] {
        let mut r = [0u8; MIN_REPORT_BYTES];
        r[0] = REPORT_ID;
        r[1..3].copy_from_slice(&roll.to_le_bytes());
        r[3..5].copy_from_slice(&pitch.to_le_bytes());
        r[9] = HAT_NEUTRAL;
        r[10] = HAT_NEUTRAL;
        r
    }

    #[test]
    fn test_centred() {
        let s = parse_orion2_stick_report(&make_report(0, 0)).unwrap();
        assert!(s.axes.roll.abs() < 1e-4);
        assert!(s.axes.pitch.abs() < 1e-4);
    }

    #[test]
    fn test_full_right_roll() {
        let s = parse_orion2_stick_report(&make_report(32767, 0)).unwrap();
        assert!((s.axes.roll - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_full_left_roll() {
        // i16::MIN / 32767 is just below -1.0; allow small overshoot.
        let s = parse_orion2_stick_report(&make_report(i16::MIN, 0)).unwrap();
        assert!(s.axes.roll < 0.0);
    }

    #[test]
    fn test_full_forward_pitch() {
        let s = parse_orion2_stick_report(&make_report(0, 32767)).unwrap();
        assert!((s.axes.pitch - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_full_aft_pitch() {
        let s = parse_orion2_stick_report(&make_report(0, i16::MIN)).unwrap();
        assert!(s.axes.pitch < 0.0);
    }

    #[test]
    fn test_button_detection() {
        let mut r = make_report(0, 0);
        r[5] = 0b0000_0101; // buttons 1 and 3
        let s = parse_orion2_stick_report(&r).unwrap();
        assert!(s.buttons.is_pressed(1));
        assert!(!s.buttons.is_pressed(2));
        assert!(s.buttons.is_pressed(3));
    }

    #[test]
    fn test_button_boundary() {
        let mut r = make_report(0, 0);
        // Set bit 19 (button 20).
        r[5..9].copy_from_slice(&(1u32 << 19).to_le_bytes());
        let s = parse_orion2_stick_report(&r).unwrap();
        assert!(s.buttons.is_pressed(20));
        assert!(!s.buttons.is_pressed(21));
    }

    #[test]
    fn test_button_out_of_range_returns_false() {
        let r = make_report(0, 0);
        let s = parse_orion2_stick_report(&r).unwrap();
        assert!(!s.buttons.is_pressed(0));
        assert!(!s.buttons.is_pressed(21));
    }

    #[test]
    fn test_hat_a_neutral() {
        let r = make_report(0, 0);
        let s = parse_orion2_stick_report(&r).unwrap();
        assert!(s.buttons.hat_a_neutral());
        assert_eq!(s.buttons.hat_a, HAT_NEUTRAL);
    }

    #[test]
    fn test_hat_b_neutral() {
        let r = make_report(0, 0);
        let s = parse_orion2_stick_report(&r).unwrap();
        assert!(s.buttons.hat_b_neutral());
        assert_eq!(s.buttons.hat_b, HAT_NEUTRAL);
    }

    #[test]
    fn test_hat_a_north() {
        let mut r = make_report(0, 0);
        r[9] = 0x00;
        let s = parse_orion2_stick_report(&r).unwrap();
        assert!(!s.buttons.hat_a_neutral());
        assert_eq!(s.buttons.hat_a, 0x00);
    }

    #[test]
    fn test_hat_b_southeast() {
        let mut r = make_report(0, 0);
        r[10] = 0x03; // SE
        let s = parse_orion2_stick_report(&r).unwrap();
        assert!(!s.buttons.hat_b_neutral());
        assert_eq!(s.buttons.hat_b, 0x03);
    }

    #[test]
    fn test_report_too_short() {
        let err = parse_orion2_stick_report(&[0u8; 5]).unwrap_err();
        assert_eq!(
            err,
            Orion2StickParseError::TooShort {
                expected: MIN_REPORT_BYTES,
                got: 5
            }
        );
    }

    #[test]
    fn test_wrong_report_id() {
        let mut r = make_report(0, 0);
        r[0] = 0xFF;
        let err = parse_orion2_stick_report(&r).unwrap_err();
        assert_eq!(err, Orion2StickParseError::UnknownReportId { id: 0xFF });
    }

    proptest! {
        #[test]
        fn prop_roll_always_in_range(raw: i16) {
            let s = parse_orion2_stick_report(&make_report(raw, 0)).unwrap();
            prop_assert!(
                (-1.001..=1.001).contains(&s.axes.roll),
                "roll out of range: {}",
                s.axes.roll
            );
        }

        #[test]
        fn prop_pitch_always_in_range(raw: i16) {
            let s = parse_orion2_stick_report(&make_report(0, raw)).unwrap();
            prop_assert!(
                (-1.001..=1.001).contains(&s.axes.pitch),
                "pitch out of range: {}",
                s.axes.pitch
            );
        }
    }
}
