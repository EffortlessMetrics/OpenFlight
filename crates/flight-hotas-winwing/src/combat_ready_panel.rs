// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID report parser for the WinWing F/A-18 Combat Ready Panel (PID 0xBE05).
//!
//! The Combat Ready Panel is a dedicated button panel replicating the F/A-18C
//! master arm, stores management, and weapons release controls.  It has
//! individually backlit push-buttons but **no axes** or encoders.
//!
//! PID 0xBE05 is confirmed via linux-hardware.org (1 probe, USB string
//! "F18 COMBAT READY PANEL", vendor "Winwing").
//!
//! # ASSUMED report layout (6 bytes, report ID `0x08`)
//!
//! *This layout is estimated by analogy with other WinWing button panels.
//! It has **not** been verified against actual hardware.*
//!
//! | Offset | Size | Field |
//! |--------|------|-------|
//! | 0      | 1    | Report ID (`0x08`) — ASSUMED |
//! | 1      | 4    | Button bitmask (u32 LE, bits 0–29 = buttons 1–30) — ASSUMED |
//! | 5      | 1    | Reserved / padding — ASSUMED |

use thiserror::Error;

/// USB Product ID for the WinWing F/A-18 Combat Ready Panel.
///
/// Confirmed via linux-hardware.org (USB string "F18 COMBAT READY PANEL").
pub const COMBAT_READY_PANEL_PID: u16 = 0xBE05;

/// Minimum bytes required in a valid Combat Ready Panel HID report.
pub const MIN_REPORT_BYTES: usize = 6;

/// Number of mapped buttons on the Combat Ready Panel.
pub const BUTTON_COUNT: u8 = 30;

// ASSUMED report ID
const REPORT_ID: u8 = 0x08;

// ── Button snapshot ───────────────────────────────────────────────────────────

/// Button state for the WinWing Combat Ready Panel (30 backlit buttons).
#[derive(Debug, Clone, Default)]
pub struct CombatReadyButtons {
    /// Bitmask for up to 30 buttons; bit `n−1` set → button `n` pressed.
    pub mask: u32,
}

impl CombatReadyButtons {
    /// Returns `true` if button `n` (1-indexed, 1–30) is pressed.
    pub fn is_pressed(&self, n: u8) -> bool {
        (1u8..=BUTTON_COUNT).contains(&n) && (self.mask >> (n - 1)) & 1 == 1
    }
}

// ── Input state ───────────────────────────────────────────────────────────────

/// Parsed state from a single Combat Ready Panel HID report.
///
/// This panel has no axes; only button data is present.
#[derive(Debug, Clone)]
pub struct CombatReadyPanelInputState {
    pub buttons: CombatReadyButtons,
}

// ── Parser ────────────────────────────────────────────────────────────────────

/// Parse a raw HID report from the WinWing Combat Ready Panel.
///
/// # Errors
///
/// Returns [`CombatReadyParseError::TooShort`] if `data` is shorter than
/// [`MIN_REPORT_BYTES`], or [`CombatReadyParseError::UnknownReportId`] if
/// `data[0]` is not the expected report ID.
///
/// # Note
///
/// The report format has **not** been verified against real hardware.
pub fn parse_combat_ready_panel_report(
    data: &[u8],
) -> Result<CombatReadyPanelInputState, CombatReadyParseError> {
    if data.len() < MIN_REPORT_BYTES {
        return Err(CombatReadyParseError::TooShort {
            expected: MIN_REPORT_BYTES,
            got: data.len(),
        });
    }
    if data[0] != REPORT_ID {
        return Err(CombatReadyParseError::UnknownReportId { id: data[0] });
    }

    let mask = u32::from_le_bytes([data[1], data[2], data[3], data[4]]);

    Ok(CombatReadyPanelInputState {
        buttons: CombatReadyButtons { mask },
    })
}

// ── Error type ────────────────────────────────────────────────────────────────

/// Errors returned by [`parse_combat_ready_panel_report`].
#[derive(Debug, Error, PartialEq)]
pub enum CombatReadyParseError {
    #[error("report too short: expected ≥{expected} bytes, got {got}")]
    TooShort { expected: usize, got: usize },
    #[error("unknown report ID: 0x{id:02X}")]
    UnknownReportId { id: u8 },
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_report(buttons: u32) -> [u8; MIN_REPORT_BYTES] {
        let mut r = [0u8; MIN_REPORT_BYTES];
        r[0] = REPORT_ID;
        r[1..5].copy_from_slice(&buttons.to_le_bytes());
        r
    }

    #[test]
    fn test_no_buttons_pressed() {
        let s = parse_combat_ready_panel_report(&make_report(0)).unwrap();
        for n in 1..=BUTTON_COUNT {
            assert!(!s.buttons.is_pressed(n), "button {n} should not be pressed");
        }
    }

    #[test]
    fn test_button_1_pressed() {
        let s = parse_combat_ready_panel_report(&make_report(0x01)).unwrap();
        assert!(s.buttons.is_pressed(1));
        assert!(!s.buttons.is_pressed(2));
    }

    #[test]
    fn test_button_30_pressed() {
        let s = parse_combat_ready_panel_report(&make_report(1 << 29)).unwrap();
        assert!(s.buttons.is_pressed(30));
        assert!(!s.buttons.is_pressed(29));
    }

    #[test]
    fn test_multiple_buttons() {
        let s = parse_combat_ready_panel_report(&make_report(0b0000_0101)).unwrap();
        assert!(s.buttons.is_pressed(1));
        assert!(!s.buttons.is_pressed(2));
        assert!(s.buttons.is_pressed(3));
    }

    #[test]
    fn test_out_of_range_buttons() {
        let s = parse_combat_ready_panel_report(&make_report(0xFFFF_FFFF)).unwrap();
        assert!(!s.buttons.is_pressed(0));
        assert!(!s.buttons.is_pressed(31));
    }

    #[test]
    fn test_report_too_short() {
        let err = parse_combat_ready_panel_report(&[0u8; 3]).unwrap_err();
        assert_eq!(
            err,
            CombatReadyParseError::TooShort {
                expected: MIN_REPORT_BYTES,
                got: 3
            }
        );
    }

    #[test]
    fn test_wrong_report_id() {
        let mut r = make_report(0);
        r[0] = 0xFF;
        let err = parse_combat_ready_panel_report(&r).unwrap_err();
        assert_eq!(err, CombatReadyParseError::UnknownReportId { id: 0xFF });
    }

    #[test]
    fn test_empty_report() {
        let err = parse_combat_ready_panel_report(&[]).unwrap_err();
        assert_eq!(
            err,
            CombatReadyParseError::TooShort {
                expected: MIN_REPORT_BYTES,
                got: 0
            }
        );
    }

    #[test]
    fn test_padding_byte_ignored() {
        let mut r = make_report(0);
        r[5] = 0xAB;
        let s = parse_combat_ready_panel_report(&r).unwrap();
        assert!(!s.buttons.is_pressed(1));
    }
}
