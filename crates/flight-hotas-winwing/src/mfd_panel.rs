// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID report parser for the WinWing MFD (Multi-Function Display) Panel (PID 0xBEE8).
//!
//! The MFD panel provides 20 bezel buttons arranged around an LCD screen
//! mount (5 per side: top, bottom, left, right).  Each button has an
//! individually-addressable backlight LED.  There are no axes or encoders.
//!
//! # ASSUMED report layout (5 bytes, report ID `0x09`)
//!
//! *This layout was derived by analogy with other WinWing button panels.
//! It has **not** been verified against actual hardware.*
//!
//! | Offset | Size | Field |
//! |--------|------|-------|
//! | 0      | 1    | Report ID (`0x09`) — ASSUMED |
//! | 1      | 3    | Button bitmask (u24 LE, bits 0–19 = buttons 1–20) — ASSUMED |
//! | 4      | 1    | Brightness rocker (0=no-change, 1=increase, 2=decrease) — ASSUMED |

use thiserror::Error;

/// USB Product ID for the WinWing MFD Panel.
pub const MFD_PANEL_PID: u16 = 0xBEE8;

/// Minimum bytes required in a valid MFD panel HID report.
pub const MIN_REPORT_BYTES: usize = 5;

/// Number of bezel buttons per MFD panel.
pub const BUTTON_COUNT: u8 = 20;

/// Number of bezel buttons per side.
pub const BUTTONS_PER_SIDE: u8 = 5;

const REPORT_ID: u8 = 0x09;

// ── Button snapshot ───────────────────────────────────────────────────────────

/// Bezel side on an MFD panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MfdSide {
    Top,
    Right,
    Bottom,
    Left,
}

/// Button state for the WinWing MFD Panel (20 bezel buttons).
///
/// Buttons 1–5 = top bezel, 6–10 = right bezel, 11–15 = bottom bezel,
/// 16–20 = left bezel (ASSUMED layout).
#[derive(Debug, Clone, Default)]
pub struct MfdButtons {
    /// 32-bit packed bitmask; bit `n−1` set → button `n` pressed.
    pub mask: u32,
    /// Brightness rocker: 0 = no change, 1 = increase, 2 = decrease.
    pub brightness_rocker: u8,
}

impl MfdButtons {
    /// Returns `true` if button `n` (1-indexed, 1–20) is pressed.
    pub fn is_pressed(&self, n: u8) -> bool {
        (1u8..=BUTTON_COUNT).contains(&n) && (self.mask >> (n - 1)) & 1 == 1
    }

    /// Returns `true` if a button on the given side is pressed.
    ///
    /// `index` is 1-indexed within the side (1–5).
    pub fn is_side_pressed(&self, side: MfdSide, index: u8) -> bool {
        if !(1..=BUTTONS_PER_SIDE).contains(&index) {
            return false;
        }
        let base = match side {
            MfdSide::Top => 0,
            MfdSide::Right => 5,
            MfdSide::Bottom => 10,
            MfdSide::Left => 15,
        };
        self.is_pressed(base + index)
    }

    /// Returns the [`MfdSide`] for a given button number (1–20), or `None` if out of range.
    pub fn button_side(n: u8) -> Option<MfdSide> {
        match n {
            1..=5 => Some(MfdSide::Top),
            6..=10 => Some(MfdSide::Right),
            11..=15 => Some(MfdSide::Bottom),
            16..=20 => Some(MfdSide::Left),
            _ => None,
        }
    }
}

// ── Input state ───────────────────────────────────────────────────────────────

/// Parsed state from a single MFD panel HID report.
#[derive(Debug, Clone)]
pub struct MfdPanelInputState {
    pub buttons: MfdButtons,
}

// ── Parser ────────────────────────────────────────────────────────────────────

/// Parse a raw HID report from the WinWing MFD Panel.
///
/// # Errors
///
/// Returns [`MfdPanelParseError::TooShort`] if `data` is shorter than
/// [`MIN_REPORT_BYTES`], or [`MfdPanelParseError::UnknownReportId`] if
/// `data[0]` is not `0x09`.
pub fn parse_mfd_panel_report(data: &[u8]) -> Result<MfdPanelInputState, MfdPanelParseError> {
    if data.len() < MIN_REPORT_BYTES {
        return Err(MfdPanelParseError::TooShort {
            expected: MIN_REPORT_BYTES,
            got: data.len(),
        });
    }
    if data[0] != REPORT_ID {
        return Err(MfdPanelParseError::UnknownReportId { id: data[0] });
    }

    // 3 button bytes packed little-endian into a u32.
    let mask = (data[1] as u32) | ((data[2] as u32) << 8) | ((data[3] as u32) << 16);
    let brightness_rocker = data[4].min(2);

    Ok(MfdPanelInputState {
        buttons: MfdButtons {
            mask,
            brightness_rocker,
        },
    })
}

// ── Error type ────────────────────────────────────────────────────────────────

/// Errors returned by [`parse_mfd_panel_report`].
#[derive(Debug, Error, PartialEq)]
pub enum MfdPanelParseError {
    #[error("report too short: expected ≥{expected} bytes, got {got}")]
    TooShort { expected: usize, got: usize },
    #[error("unknown report ID: 0x{id:02X}")]
    UnknownReportId { id: u8 },
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_report(b0: u8, b1: u8, b2: u8, rocker: u8) -> [u8; MIN_REPORT_BYTES] {
        [REPORT_ID, b0, b1, b2, rocker]
    }

    fn all_zero() -> [u8; MIN_REPORT_BYTES] {
        make_report(0, 0, 0, 0)
    }

    #[test]
    fn test_no_buttons_pressed() {
        let s = parse_mfd_panel_report(&all_zero()).unwrap();
        for n in 1..=BUTTON_COUNT {
            assert!(!s.buttons.is_pressed(n), "button {n} should not be pressed");
        }
    }

    #[test]
    fn test_button_1_pressed() {
        let r = make_report(0x01, 0, 0, 0);
        let s = parse_mfd_panel_report(&r).unwrap();
        assert!(s.buttons.is_pressed(1));
        assert!(!s.buttons.is_pressed(2));
    }

    #[test]
    fn test_button_8_pressed() {
        let r = make_report(0x80, 0, 0, 0);
        let s = parse_mfd_panel_report(&r).unwrap();
        assert!(s.buttons.is_pressed(8));
    }

    #[test]
    fn test_button_16_pressed() {
        let r = make_report(0, 0x80, 0, 0);
        let s = parse_mfd_panel_report(&r).unwrap();
        assert!(s.buttons.is_pressed(16));
    }

    #[test]
    fn test_button_20_pressed() {
        // Button 20 → bit 19 → byte 2 bit 3
        let r = make_report(0, 0, 0x08, 0);
        let s = parse_mfd_panel_report(&r).unwrap();
        assert!(s.buttons.is_pressed(20));
    }

    #[test]
    fn test_all_buttons_pressed() {
        // Bits 0–19 all set: 0x0F_FFFF spread across 3 bytes
        let r = make_report(0xFF, 0xFF, 0x0F, 0);
        let s = parse_mfd_panel_report(&r).unwrap();
        for n in 1..=BUTTON_COUNT {
            assert!(s.buttons.is_pressed(n), "button {n} should be pressed");
        }
        assert!(!s.buttons.is_pressed(21));
    }

    #[test]
    fn test_multiple_buttons() {
        let r = make_report(0x05, 0, 0x01, 0);
        let s = parse_mfd_panel_report(&r).unwrap();
        assert!(s.buttons.is_pressed(1));
        assert!(s.buttons.is_pressed(3));
        assert!(s.buttons.is_pressed(17));
    }

    #[test]
    fn test_out_of_range_buttons() {
        let s = parse_mfd_panel_report(&all_zero()).unwrap();
        assert!(!s.buttons.is_pressed(0));
        assert!(!s.buttons.is_pressed(BUTTON_COUNT + 1));
    }

    // ── Side-based access ──────────────────────────────────────────────────

    #[test]
    fn test_top_side_button() {
        let r = make_report(0x01, 0, 0, 0); // button 1 = top side, index 1
        let s = parse_mfd_panel_report(&r).unwrap();
        assert!(s.buttons.is_side_pressed(MfdSide::Top, 1));
        assert!(!s.buttons.is_side_pressed(MfdSide::Top, 2));
    }

    #[test]
    fn test_right_side_button() {
        // Button 6 = right side, index 1 → bit 5
        let r = make_report(0x20, 0, 0, 0);
        let s = parse_mfd_panel_report(&r).unwrap();
        assert!(s.buttons.is_side_pressed(MfdSide::Right, 1));
    }

    #[test]
    fn test_bottom_side_button() {
        // Button 11 = bottom side, index 1 → bit 10 → byte 1 bit 2
        let r = make_report(0, 0x04, 0, 0);
        let s = parse_mfd_panel_report(&r).unwrap();
        assert!(s.buttons.is_side_pressed(MfdSide::Bottom, 1));
    }

    #[test]
    fn test_left_side_button() {
        // Button 16 = left side, index 1 → bit 15 → byte 1 bit 7
        let r = make_report(0, 0x80, 0, 0);
        let s = parse_mfd_panel_report(&r).unwrap();
        assert!(s.buttons.is_side_pressed(MfdSide::Left, 1));
    }

    #[test]
    fn test_side_button_out_of_range() {
        let s = parse_mfd_panel_report(&all_zero()).unwrap();
        assert!(!s.buttons.is_side_pressed(MfdSide::Top, 0));
        assert!(!s.buttons.is_side_pressed(MfdSide::Top, 6));
    }

    #[test]
    fn test_button_side_mapping() {
        assert_eq!(MfdButtons::button_side(1), Some(MfdSide::Top));
        assert_eq!(MfdButtons::button_side(5), Some(MfdSide::Top));
        assert_eq!(MfdButtons::button_side(6), Some(MfdSide::Right));
        assert_eq!(MfdButtons::button_side(10), Some(MfdSide::Right));
        assert_eq!(MfdButtons::button_side(11), Some(MfdSide::Bottom));
        assert_eq!(MfdButtons::button_side(15), Some(MfdSide::Bottom));
        assert_eq!(MfdButtons::button_side(16), Some(MfdSide::Left));
        assert_eq!(MfdButtons::button_side(20), Some(MfdSide::Left));
        assert_eq!(MfdButtons::button_side(0), None);
        assert_eq!(MfdButtons::button_side(21), None);
    }

    // ── Brightness rocker ──────────────────────────────────────────────────

    #[test]
    fn test_brightness_rocker_no_change() {
        let s = parse_mfd_panel_report(&all_zero()).unwrap();
        assert_eq!(s.buttons.brightness_rocker, 0);
    }

    #[test]
    fn test_brightness_rocker_increase() {
        let r = make_report(0, 0, 0, 1);
        let s = parse_mfd_panel_report(&r).unwrap();
        assert_eq!(s.buttons.brightness_rocker, 1);
    }

    #[test]
    fn test_brightness_rocker_decrease() {
        let r = make_report(0, 0, 0, 2);
        let s = parse_mfd_panel_report(&r).unwrap();
        assert_eq!(s.buttons.brightness_rocker, 2);
    }

    #[test]
    fn test_brightness_rocker_clamped() {
        let r = make_report(0, 0, 0, 255);
        let s = parse_mfd_panel_report(&r).unwrap();
        assert_eq!(s.buttons.brightness_rocker, 2);
    }

    // ── Error cases ────────────────────────────────────────────────────────

    #[test]
    fn test_report_too_short() {
        let err = parse_mfd_panel_report(&[0u8; 2]).unwrap_err();
        assert_eq!(
            err,
            MfdPanelParseError::TooShort {
                expected: MIN_REPORT_BYTES,
                got: 2
            }
        );
    }

    #[test]
    fn test_wrong_report_id() {
        let mut r = all_zero();
        r[0] = 0xFF;
        let err = parse_mfd_panel_report(&r).unwrap_err();
        assert_eq!(err, MfdPanelParseError::UnknownReportId { id: 0xFF });
    }

    #[test]
    fn test_empty_report() {
        let err = parse_mfd_panel_report(&[]).unwrap_err();
        assert_eq!(
            err,
            MfdPanelParseError::TooShort {
                expected: MIN_REPORT_BYTES,
                got: 0
            }
        );
    }
}
