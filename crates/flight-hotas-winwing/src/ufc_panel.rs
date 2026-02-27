// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID report parser for the WinWing UFC1 + HUD1 Panel (PID 0xBEDE).
//!
//! The UFC (Universal Flight Controller) + HUD panel is a combined
//! button/LED panel with **no axes**.  The UFC section provides keypad
//! and communication management buttons; the HUD section provides
//! display-related control buttons.
//!
//! # ASSUMED report layout (6 bytes, report ID `0x06`)
//!
//! *This layout was derived by analogy with other WinWing button panels and
//! community reverse-engineering notes.  It has **not** been verified against
//! actual hardware.  Treat every byte offset and bitmask position as ASSUMED
//! until confirmed with a hardware capture.*
//!
//! | Offset | Size | Field |
//! |--------|------|-------|
//! | 0      | 1    | Report ID (`0x06`) — ASSUMED |
//! | 1      | 3    | UFC button bitmask (u24 LE, bits 0–23 = UFC buttons 1–24) — ASSUMED |
//! | 4      | 2    | HUD button bitmask (u16 LE, bits 0–11 = HUD buttons 1–12) — ASSUMED |

use thiserror::Error;

/// USB Product ID for the WinWing UFC1 + HUD1 Panel.
pub const UFC_PANEL_PID: u16 = 0xBEDE;

/// Minimum bytes required in a valid UFC panel HID report.
// ASSUMED: 1 report-ID byte + 5 button bytes = 6 bytes total
pub const MIN_REPORT_BYTES: usize = 6;

/// Number of buttons on the UFC section (keypad + function buttons).
// ASSUMED: 24 UFC buttons — digits 0–9, CLR, ENTER, and 12 function/mode keys
pub const UFC_BUTTON_COUNT: u8 = 24;

/// Number of buttons on the HUD section.
// ASSUMED: 12 HUD buttons
pub const HUD_BUTTON_COUNT: u8 = 12;

/// Total mapped button count (UFC + HUD).
pub const TOTAL_BUTTON_COUNT: u8 = UFC_BUTTON_COUNT + HUD_BUTTON_COUNT; // 36

// ASSUMED report ID (analogy with other WinWing panel products)
const REPORT_ID: u8 = 0x06;

// ── Button snapshot ───────────────────────────────────────────────────────────

/// Button state for the WinWing UFC1 + HUD1 Panel (36 buttons total).
///
/// Buttons are packed into a 40-bit integer (`mask: u64`).  The lower 24 bits
/// correspond to the UFC section (buttons 1–24) and bits 24–35 correspond to
/// the HUD section (buttons 25–36).
///
/// # Button layout (ASSUMED)
///
/// | Button range | Section | Physical controls |
/// |---|---|---|
/// | 1–10  | UFC | Keypad digits 0–9 |
/// | 11    | UFC | CLR key |
/// | 12    | UFC | ENTER key |
/// | 13–24 | UFC | COM1/COM2/NAV1/NAV2 and function keys |
/// | 25–36 | HUD | HUD brightness, mode, and view buttons |
#[derive(Debug, Clone, Default)]
pub struct UfcButtons {
    /// 40-bit packed bitmask; bit `n−1` set → button `n` pressed.
    ///
    /// Bits 0–23: UFC section (buttons 1–24).
    /// Bits 24–35: HUD section (buttons 25–36).
    /// ASSUMED layout — verify with hardware capture.
    pub mask: u64,
}

impl UfcButtons {
    /// Returns `true` if button `n` (1-indexed, 1–36) is pressed.
    pub fn is_pressed(&self, n: u8) -> bool {
        (1u8..=TOTAL_BUTTON_COUNT).contains(&n) && (self.mask >> (n - 1)) & 1 == 1
    }

    /// Returns `true` if UFC button `n` (1-indexed, 1–24) is pressed.
    pub fn is_ufc_pressed(&self, n: u8) -> bool {
        (1u8..=UFC_BUTTON_COUNT).contains(&n) && (self.mask >> (n - 1)) & 1 == 1
    }

    /// Returns `true` if HUD button `n` (1-indexed, 1–12) is pressed.
    pub fn is_hud_pressed(&self, n: u8) -> bool {
        (1u8..=HUD_BUTTON_COUNT).contains(&n)
            && (self.mask >> (u64::from(UFC_BUTTON_COUNT) + u64::from(n) - 1)) & 1 == 1
    }
}

// ── Input state ───────────────────────────────────────────────────────────────

/// Parsed state from a single UFC1 + HUD1 HID report.
///
/// This panel has no axes; only button data is present.
#[derive(Debug, Clone)]
pub struct UfcPanelInputState {
    /// Button state across both UFC and HUD sections.
    pub buttons: UfcButtons,
}

// ── Parser ────────────────────────────────────────────────────────────────────

/// Parse a raw HID report from the WinWing UFC1 + HUD1 Panel.
///
/// # Errors
///
/// Returns [`UfcPanelParseError::TooShort`] if `data` is shorter than
/// [`MIN_REPORT_BYTES`], or [`UfcPanelParseError::UnknownReportId`] if
/// `data[0]` is not `0x06`.
///
/// # Note
///
/// The report format has **not** been verified against real hardware.
/// All byte offsets and bitmask positions are ASSUMED by analogy.
pub fn parse_ufc_panel_report(data: &[u8]) -> Result<UfcPanelInputState, UfcPanelParseError> {
    if data.len() < MIN_REPORT_BYTES {
        return Err(UfcPanelParseError::TooShort {
            expected: MIN_REPORT_BYTES,
            got: data.len(),
        });
    }
    if data[0] != REPORT_ID {
        return Err(UfcPanelParseError::UnknownReportId { id: data[0] });
    }

    // ASSUMED: 5 button bytes at offsets 1–5, packed little-endian into a u64.
    // Bits 0–7   → data[1], bits 8–15  → data[2], bits 16–23 → data[3] (UFC section)
    // Bits 24–31 → data[4], bits 32–39 → data[5]              (HUD section)
    let mask = (data[1] as u64)
        | ((data[2] as u64) << 8)
        | ((data[3] as u64) << 16)
        | ((data[4] as u64) << 24)
        | ((data[5] as u64) << 32);

    Ok(UfcPanelInputState {
        buttons: UfcButtons { mask },
    })
}

// ── Error type ────────────────────────────────────────────────────────────────

/// Errors returned by [`parse_ufc_panel_report`].
#[derive(Debug, Error, PartialEq)]
pub enum UfcPanelParseError {
    #[error("report too short: expected ≥{expected} bytes, got {got}")]
    TooShort { expected: usize, got: usize },
    #[error("unknown report ID: 0x{id:02X}")]
    UnknownReportId { id: u8 },
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal valid report from five button bytes (b0..b4).
    fn make_report(b0: u8, b1: u8, b2: u8, b3: u8, b4: u8) -> [u8; MIN_REPORT_BYTES] {
        [REPORT_ID, b0, b1, b2, b3, b4]
    }

    fn all_zero() -> [u8; MIN_REPORT_BYTES] {
        make_report(0, 0, 0, 0, 0)
    }

    #[test]
    fn test_no_buttons_pressed() {
        let s = parse_ufc_panel_report(&all_zero()).unwrap();
        for n in 1..=TOTAL_BUTTON_COUNT {
            assert!(!s.buttons.is_pressed(n), "button {n} should not be pressed");
        }
    }

    #[test]
    fn test_ufc_button_1_pressed() {
        // Button 1 → bit 0 → data[1] LSB
        let r = make_report(0b0000_0001, 0, 0, 0, 0);
        let s = parse_ufc_panel_report(&r).unwrap();
        assert!(s.buttons.is_pressed(1));
        assert!(s.buttons.is_ufc_pressed(1));
        assert!(!s.buttons.is_pressed(2));
    }

    #[test]
    fn test_ufc_button_8_pressed() {
        // Button 8 → bit 7 → data[1] MSB
        let r = make_report(0b1000_0000, 0, 0, 0, 0);
        let s = parse_ufc_panel_report(&r).unwrap();
        assert!(s.buttons.is_pressed(8));
        assert!(s.buttons.is_ufc_pressed(8));
    }

    #[test]
    fn test_ufc_button_16_pressed() {
        // Button 16 → bit 15 → data[2] MSB
        let r = make_report(0, 0b1000_0000, 0, 0, 0);
        let s = parse_ufc_panel_report(&r).unwrap();
        assert!(s.buttons.is_pressed(16));
        assert!(s.buttons.is_ufc_pressed(16));
    }

    #[test]
    fn test_ufc_button_24_pressed() {
        // Button 24 → bit 23 → data[3] MSB (last UFC button)
        let r = make_report(0, 0, 0b1000_0000, 0, 0);
        let s = parse_ufc_panel_report(&r).unwrap();
        assert!(s.buttons.is_pressed(24));
        assert!(s.buttons.is_ufc_pressed(24));
    }

    #[test]
    fn test_hud_button_1_pressed() {
        // Button 25 → bit 24 → data[4] LSB (first HUD button)
        let r = make_report(0, 0, 0, 0b0000_0001, 0);
        let s = parse_ufc_panel_report(&r).unwrap();
        assert!(s.buttons.is_pressed(25));
        assert!(s.buttons.is_hud_pressed(1));
        assert!(!s.buttons.is_ufc_pressed(1));
    }

    #[test]
    fn test_hud_button_12_pressed() {
        // Button 36 → bit 35 → data[5] bit 3
        let r = make_report(0, 0, 0, 0, 0b0000_1000);
        let s = parse_ufc_panel_report(&r).unwrap();
        assert!(s.buttons.is_pressed(36));
        assert!(s.buttons.is_hud_pressed(12));
    }

    #[test]
    fn test_multiple_buttons_simultaneously() {
        // Buttons 1, 3, 25 pressed at once.
        // Button 1 → bit 0 of data[1]; Button 3 → bit 2 of data[1]; Button 25 → bit 0 of data[4]
        let r = make_report(0b0000_0101, 0, 0, 0b0000_0001, 0);
        let s = parse_ufc_panel_report(&r).unwrap();
        assert!(s.buttons.is_pressed(1));
        assert!(!s.buttons.is_pressed(2));
        assert!(s.buttons.is_pressed(3));
        assert!(s.buttons.is_pressed(25));
    }

    #[test]
    fn test_all_buttons_pressed() {
        // Bits 0–23 → data[1..=3] all 0xFF; bits 24–35 → data[4]=0xFF, data[5]=0x0F
        let r = make_report(0xFF, 0xFF, 0xFF, 0xFF, 0x0F);
        let s = parse_ufc_panel_report(&r).unwrap();
        for n in 1..=TOTAL_BUTTON_COUNT {
            assert!(s.buttons.is_pressed(n), "button {n} should be pressed");
        }
        // Bit 36 (button 37) must not exist
        assert!(!s.buttons.is_pressed(37));
    }

    #[test]
    fn test_out_of_range_buttons() {
        let s = parse_ufc_panel_report(&all_zero()).unwrap();
        assert!(!s.buttons.is_pressed(0));
        assert!(!s.buttons.is_pressed(TOTAL_BUTTON_COUNT + 1));
        assert!(!s.buttons.is_ufc_pressed(0));
        assert!(!s.buttons.is_ufc_pressed(UFC_BUTTON_COUNT + 1));
        assert!(!s.buttons.is_hud_pressed(0));
        assert!(!s.buttons.is_hud_pressed(HUD_BUTTON_COUNT + 1));
    }

    #[test]
    fn test_report_too_short() {
        let err = parse_ufc_panel_report(&[0u8; 3]).unwrap_err();
        assert_eq!(
            err,
            UfcPanelParseError::TooShort {
                expected: MIN_REPORT_BYTES,
                got: 3
            }
        );
    }

    #[test]
    fn test_wrong_report_id() {
        let mut r = all_zero();
        r[0] = 0xFF;
        let err = parse_ufc_panel_report(&r).unwrap_err();
        assert_eq!(err, UfcPanelParseError::UnknownReportId { id: 0xFF });
    }

    #[test]
    fn test_empty_report() {
        let err = parse_ufc_panel_report(&[]).unwrap_err();
        assert_eq!(
            err,
            UfcPanelParseError::TooShort {
                expected: MIN_REPORT_BYTES,
                got: 0
            }
        );
    }
}
