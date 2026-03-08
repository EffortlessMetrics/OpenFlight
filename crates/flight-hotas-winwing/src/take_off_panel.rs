// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID report parser for the WinWing F/A-18 Take Off Panel (PID 0xBE04).
//!
//! The Take Off Panel provides toggle switches, push-buttons, and rotary
//! encoders for pre-flight and takeoff procedures.  It has **no axes**.
//!
//! PID 0xBE04 is confirmed via linux-hardware.org (observed alongside
//! "F18 COMBAT READY PANEL" PID 0xBE05 on the same system).
//!
//! # ASSUMED report layout (8 bytes, report ID `0x09`)
//!
//! *This layout is estimated by analogy with other WinWing panel devices.
//! It has **not** been verified against actual hardware.*
//!
//! | Offset | Size | Field |
//! |--------|------|-------|
//! | 0      | 1    | Report ID (`0x09`) — ASSUMED |
//! | 1      | 4    | Button bitmask (u32 LE, bits 0–31 = buttons 1–32) — ASSUMED |
//! | 5      | 1    | Encoder 0 delta (i8, positive = CW) — ASSUMED |
//! | 6      | 1    | Encoder 1 delta (i8, positive = CW) — ASSUMED |
//! | 7      | 1    | Encoder 2 delta (i8, positive = CW) — ASSUMED |

use thiserror::Error;

/// USB Product ID for the WinWing F/A-18 Take Off Panel.
///
/// Confirmed via linux-hardware.org (observed alongside Combat Ready Panel).
pub const TAKE_OFF_PANEL_PID: u16 = 0xBE04;

/// Minimum bytes required in a valid Take Off Panel HID report.
pub const MIN_REPORT_BYTES: usize = 8;

/// Number of mapped buttons on the Take Off Panel.
pub const BUTTON_COUNT: u8 = 32;

/// Number of rotary encoders on the Take Off Panel.
pub const ENCODER_COUNT: usize = 3;

// ASSUMED report ID
const REPORT_ID: u8 = 0x09;

// ── Button snapshot ───────────────────────────────────────────────────────────

/// Button and encoder state for the WinWing Take Off Panel.
#[derive(Debug, Clone, Default)]
pub struct TakeOffPanelButtons {
    /// Bitmask for up to 32 buttons; bit `n−1` set → button `n` pressed.
    pub mask: u32,
    /// Encoder detent deltas (positive = CW, negative = CCW), 3 encoders.
    pub encoders: [i8; ENCODER_COUNT],
}

impl TakeOffPanelButtons {
    /// Returns `true` if button `n` (1-indexed, 1–32) is pressed.
    pub fn is_pressed(&self, n: u8) -> bool {
        (1u8..=BUTTON_COUNT).contains(&n) && (self.mask >> (n - 1)) & 1 == 1
    }
}

// ── Input state ───────────────────────────────────────────────────────────────

/// Parsed state from a single Take Off Panel HID report.
///
/// This panel has no axes; only button and encoder data is present.
#[derive(Debug, Clone)]
pub struct TakeOffPanelInputState {
    pub buttons: TakeOffPanelButtons,
}

// ── Parser ────────────────────────────────────────────────────────────────────

/// Parse a raw HID report from the WinWing Take Off Panel.
///
/// # Errors
///
/// Returns [`TakeOffPanelParseError::TooShort`] if `data` is shorter than
/// [`MIN_REPORT_BYTES`], or [`TakeOffPanelParseError::UnknownReportId`] if
/// `data[0]` is not the expected report ID.
///
/// # Note
///
/// The report format has **not** been verified against real hardware.
pub fn parse_take_off_panel_report(
    data: &[u8],
) -> Result<TakeOffPanelInputState, TakeOffPanelParseError> {
    if data.len() < MIN_REPORT_BYTES {
        return Err(TakeOffPanelParseError::TooShort {
            expected: MIN_REPORT_BYTES,
            got: data.len(),
        });
    }
    if data[0] != REPORT_ID {
        return Err(TakeOffPanelParseError::UnknownReportId { id: data[0] });
    }

    let mask = u32::from_le_bytes([data[1], data[2], data[3], data[4]]);
    let encoders = [data[5] as i8, data[6] as i8, data[7] as i8];

    Ok(TakeOffPanelInputState {
        buttons: TakeOffPanelButtons { mask, encoders },
    })
}

// ── Error type ────────────────────────────────────────────────────────────────

/// Errors returned by [`parse_take_off_panel_report`].
#[derive(Debug, Error, PartialEq)]
pub enum TakeOffPanelParseError {
    #[error("report too short: expected ≥{expected} bytes, got {got}")]
    TooShort { expected: usize, got: usize },
    #[error("unknown report ID: 0x{id:02X}")]
    UnknownReportId { id: u8 },
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_report(buttons: u32, enc: [i8; ENCODER_COUNT]) -> [u8; MIN_REPORT_BYTES] {
        let mut r = [0u8; MIN_REPORT_BYTES];
        r[0] = REPORT_ID;
        r[1..5].copy_from_slice(&buttons.to_le_bytes());
        r[5] = enc[0] as u8;
        r[6] = enc[1] as u8;
        r[7] = enc[2] as u8;
        r
    }

    #[test]
    fn test_no_buttons_pressed() {
        let s = parse_take_off_panel_report(&make_report(0, [0; 3])).unwrap();
        for n in 1..=BUTTON_COUNT {
            assert!(!s.buttons.is_pressed(n), "button {n} should not be pressed");
        }
    }

    #[test]
    fn test_button_1_pressed() {
        let s = parse_take_off_panel_report(&make_report(0x01, [0; 3])).unwrap();
        assert!(s.buttons.is_pressed(1));
        assert!(!s.buttons.is_pressed(2));
    }

    #[test]
    fn test_button_32_pressed() {
        let s = parse_take_off_panel_report(&make_report(1 << 31, [0; 3])).unwrap();
        assert!(s.buttons.is_pressed(32));
    }

    #[test]
    fn test_encoder_deltas() {
        let s = parse_take_off_panel_report(&make_report(0, [2, -1, 3])).unwrap();
        assert_eq!(s.buttons.encoders[0], 2);
        assert_eq!(s.buttons.encoders[1], -1);
        assert_eq!(s.buttons.encoders[2], 3);
    }

    #[test]
    fn test_out_of_range_buttons() {
        let s = parse_take_off_panel_report(&make_report(0xFFFF_FFFF, [0; 3])).unwrap();
        assert!(!s.buttons.is_pressed(0));
        assert!(!s.buttons.is_pressed(33));
    }

    #[test]
    fn test_report_too_short() {
        let err = parse_take_off_panel_report(&[0u8; 4]).unwrap_err();
        assert_eq!(
            err,
            TakeOffPanelParseError::TooShort {
                expected: MIN_REPORT_BYTES,
                got: 4
            }
        );
    }

    #[test]
    fn test_wrong_report_id() {
        let mut r = make_report(0, [0; 3]);
        r[0] = 0xFF;
        let err = parse_take_off_panel_report(&r).unwrap_err();
        assert_eq!(err, TakeOffPanelParseError::UnknownReportId { id: 0xFF });
    }

    #[test]
    fn test_empty_report() {
        let err = parse_take_off_panel_report(&[]).unwrap_err();
        assert_eq!(
            err,
            TakeOffPanelParseError::TooShort {
                expected: MIN_REPORT_BYTES,
                got: 0
            }
        );
    }
}
