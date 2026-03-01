// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID report parser for the WinWing Super Libra joystick base (PID 0xBD70).
//!
//! The Super Libra is a high-end centre-mount joystick base with Hall-effect
//! sensors on X/Y axes.  Grips attach separately and share the same USB
//! composite device.  The base provides 2 axes (roll/pitch), buttons from the
//! attached grip, and a single 8-way HAT switch.
//!
//! PID 0xBD70 is a community estimate.  It has **not** been confirmed from
//! hardware probes.
//!
//! # ASSUMED report layout (12 bytes, report ID `0x0A`)
//!
//! *Derived by analogy with the Orion 2 stick (PID 0xBE63).*
//!
//! | Offset | Size | Field |
//! |--------|------|-------|
//! | 0      | 1    | Report ID (`0x0A`) — ASSUMED |
//! | 1      | 2    | Roll axis (i16 LE, −32768..32767 → −1.0..1.0) — ASSUMED |
//! | 3      | 2    | Pitch axis (i16 LE, −32768..32767 → −1.0..1.0) — ASSUMED |
//! | 5      | 4    | Button bitmask (u32 LE, bits 0–23 = buttons 1–24) — ASSUMED |
//! | 9      | 1    | HAT (8-way: 0=N..7=NW, 0x0F=neutral) — ASSUMED |
//! | 10     | 2    | Reserved — ASSUMED |

use thiserror::Error;

/// USB Product ID for the WinWing Super Libra joystick base.
///
/// Community estimate — not confirmed from hardware probes.
pub const SUPER_LIBRA_PID: u16 = 0xBD70;

/// Minimum bytes required in a valid Super Libra HID report.
pub const MIN_REPORT_BYTES: usize = 12;

/// Number of mapped buttons on the Super Libra (including grip buttons).
pub const BUTTON_COUNT: u8 = 24;

// ASSUMED report ID
const REPORT_ID: u8 = 0x0A;

/// HAT neutral position value.
const HAT_NEUTRAL: u8 = 0x0F;

// ── Axis snapshot ─────────────────────────────────────────────────────────────

/// Axis snapshot for the WinWing Super Libra joystick base.
#[derive(Debug, Clone, PartialEq)]
pub struct SuperLibraAxes {
    /// Roll (left/right deflection) — \[−1.0, 1.0\].
    pub roll: f32,
    /// Pitch (forward/aft deflection) — \[−1.0, 1.0\].
    pub pitch: f32,
}

// ── Button snapshot ───────────────────────────────────────────────────────────

/// Button state for the WinWing Super Libra (24 buttons + 1 HAT).
#[derive(Debug, Clone, Default)]
pub struct SuperLibraButtons {
    /// Bitmask for up to 24 buttons; bit `n−1` set → button `n` pressed.
    pub mask: u32,
    /// 8-way HAT position. `0`=N, `1`=NE … `7`=NW, `0x0F`=neutral.
    pub hat: u8,
}

impl SuperLibraButtons {
    /// Returns `true` if button `n` (1-indexed, 1–24) is pressed.
    pub fn is_pressed(&self, n: u8) -> bool {
        (1u8..=BUTTON_COUNT).contains(&n) && (self.mask >> (n - 1)) & 1 == 1
    }

    /// Returns `true` when the HAT is in its neutral (released) position.
    pub fn hat_neutral(&self) -> bool {
        self.hat == HAT_NEUTRAL
    }
}

// ── Input state ───────────────────────────────────────────────────────────────

/// Parsed state from a single Super Libra HID report.
#[derive(Debug, Clone)]
pub struct SuperLibraInputState {
    pub axes: SuperLibraAxes,
    pub buttons: SuperLibraButtons,
}

// ── Parser ────────────────────────────────────────────────────────────────────

/// Parse a raw HID report from the WinWing Super Libra joystick base.
///
/// # Errors
///
/// Returns [`SuperLibraParseError::TooShort`] if `data` is shorter than
/// [`MIN_REPORT_BYTES`], or [`SuperLibraParseError::UnknownReportId`] if
/// `data[0]` is not `0x0A`.
///
/// # Note
///
/// The report format has **not** been verified against real hardware.
pub fn parse_super_libra_report(data: &[u8]) -> Result<SuperLibraInputState, SuperLibraParseError> {
    if data.len() < MIN_REPORT_BYTES {
        return Err(SuperLibraParseError::TooShort {
            expected: MIN_REPORT_BYTES,
            got: data.len(),
        });
    }
    if data[0] != REPORT_ID {
        return Err(SuperLibraParseError::UnknownReportId { id: data[0] });
    }

    let roll = norm_i16(i16::from_le_bytes([data[1], data[2]]));
    let pitch = norm_i16(i16::from_le_bytes([data[3], data[4]]));
    let mask = u32::from_le_bytes([data[5], data[6], data[7], data[8]]);
    let hat = data[9];

    Ok(SuperLibraInputState {
        axes: SuperLibraAxes { roll, pitch },
        buttons: SuperLibraButtons { mask, hat },
    })
}

// ── Error type ────────────────────────────────────────────────────────────────

/// Errors returned by [`parse_super_libra_report`].
#[derive(Debug, Error, PartialEq)]
pub enum SuperLibraParseError {
    #[error("report too short: expected ≥{expected} bytes, got {got}")]
    TooShort { expected: usize, got: usize },
    #[error("unknown report ID: 0x{id:02X}")]
    UnknownReportId { id: u8 },
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Normalise a signed 16-bit integer to the range \[−1.0, 1.0\].
fn norm_i16(v: i16) -> f32 {
    (v as f32 / 32768.0).clamp(-1.0, 1.0)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn make_report(roll: i16, pitch: i16) -> [u8; MIN_REPORT_BYTES] {
        let mut r = [0u8; MIN_REPORT_BYTES];
        r[0] = REPORT_ID;
        r[1..3].copy_from_slice(&roll.to_le_bytes());
        r[3..5].copy_from_slice(&pitch.to_le_bytes());
        r[9] = HAT_NEUTRAL;
        r
    }

    #[test]
    fn test_centred() {
        let s = parse_super_libra_report(&make_report(0, 0)).unwrap();
        assert!(s.axes.roll.abs() < 1e-4);
        assert!(s.axes.pitch.abs() < 1e-4);
    }

    #[test]
    fn test_full_right_roll() {
        let s = parse_super_libra_report(&make_report(32767, 0)).unwrap();
        assert!((s.axes.roll - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_full_forward_pitch() {
        let s = parse_super_libra_report(&make_report(0, 32767)).unwrap();
        assert!((s.axes.pitch - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_button_detection() {
        let mut r = make_report(0, 0);
        r[5] = 0b0000_0101;
        let s = parse_super_libra_report(&r).unwrap();
        assert!(s.buttons.is_pressed(1));
        assert!(!s.buttons.is_pressed(2));
        assert!(s.buttons.is_pressed(3));
    }

    #[test]
    fn test_button_24_pressed() {
        let mut r = make_report(0, 0);
        r[5..9].copy_from_slice(&(1u32 << 23).to_le_bytes());
        let s = parse_super_libra_report(&r).unwrap();
        assert!(s.buttons.is_pressed(24));
        assert!(!s.buttons.is_pressed(25));
    }

    #[test]
    fn test_button_out_of_range() {
        let s = parse_super_libra_report(&make_report(0, 0)).unwrap();
        assert!(!s.buttons.is_pressed(0));
        assert!(!s.buttons.is_pressed(25));
    }

    #[test]
    fn test_hat_neutral() {
        let s = parse_super_libra_report(&make_report(0, 0)).unwrap();
        assert!(s.buttons.hat_neutral());
    }

    #[test]
    fn test_hat_north() {
        let mut r = make_report(0, 0);
        r[9] = 0x00;
        let s = parse_super_libra_report(&r).unwrap();
        assert!(!s.buttons.hat_neutral());
        assert_eq!(s.buttons.hat, 0x00);
    }

    #[test]
    fn test_report_too_short() {
        let err = parse_super_libra_report(&[0u8; 5]).unwrap_err();
        assert_eq!(
            err,
            SuperLibraParseError::TooShort {
                expected: MIN_REPORT_BYTES,
                got: 5
            }
        );
    }

    #[test]
    fn test_wrong_report_id() {
        let mut r = make_report(0, 0);
        r[0] = 0xFF;
        let err = parse_super_libra_report(&r).unwrap_err();
        assert_eq!(err, SuperLibraParseError::UnknownReportId { id: 0xFF });
    }

    proptest! {
        #[test]
        fn prop_roll_always_in_range(raw: i16) {
            let s = parse_super_libra_report(&make_report(raw, 0)).unwrap();
            prop_assert!(
                (-1.001..=1.001).contains(&s.axes.roll),
                "roll out of range: {}",
                s.axes.roll
            );
        }

        #[test]
        fn prop_pitch_always_in_range(raw: i16) {
            let s = parse_super_libra_report(&make_report(0, raw)).unwrap();
            prop_assert!(
                (-1.001..=1.001).contains(&s.axes.pitch),
                "pitch out of range: {}",
                s.axes.pitch
            );
        }
    }
}
