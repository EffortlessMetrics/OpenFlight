// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID report parser for the WinWing F-16 ICP (Integrated Control Panel) (PID 0xBEDF).
//!
//! The F-16 ICP is a dedicated panel replicating the F-16 DED/ICP layout with
//! push buttons for data entry, priority/override functions, and rotary knobs
//! for radio and navigation tuning.
//!
//! # ASSUMED report layout (8 bytes, report ID `0x08`)
//!
//! *This layout was derived by analogy with the UFC panel and community
//! reverse-engineering notes.  It has **not** been verified against actual
//! hardware.  Treat every byte offset and bitmask position as ASSUMED
//! until confirmed with a hardware capture.*
//!
//! | Offset | Size | Field |
//! |--------|------|-------|
//! | 0      | 1    | Report ID (`0x08`) — ASSUMED |
//! | 1      | 4    | ICP button bitmask (u32 LE, bits 0–25 = buttons 1–26) — ASSUMED |
//! | 5      | 1    | Encoder 0 delta (DEDUP/RET wheel, i8) — ASSUMED |
//! | 6      | 1    | Encoder 1 delta (SYM wheel, i8) — ASSUMED |
//! | 7      | 1    | Rocker switch state (0=center, 1=up, 2=down) — ASSUMED |

use thiserror::Error;

/// USB Product ID for the WinWing F-16 ICP.
pub const F16_ICP_PID: u16 = 0xBEDF;

/// Minimum bytes required in a valid F-16 ICP HID report.
pub const MIN_REPORT_BYTES: usize = 8;

/// Number of buttons on the F-16 ICP.
pub const BUTTON_COUNT: u8 = 26;

/// Number of rotary encoders on the F-16 ICP.
pub const ENCODER_COUNT: usize = 2;

const REPORT_ID: u8 = 0x08;

// ── Button snapshot ───────────────────────────────────────────────────────────

/// Button and encoder state for the WinWing F-16 ICP.
///
/// # Button layout (ASSUMED)
///
/// | Button range | Physical controls |
/// |---|---|
/// | 1–10  | Keypad digits 0–9 |
/// | 11    | ENTR key |
/// | 12    | RCL key |
/// | 13–14 | COM 1 / COM 2 override |
/// | 15–16 | IFF IDENT / CNI |
/// | 17–20 | DCS sequence (SEQ, up, down, RTN) |
/// | 21–22 | Flare / Chaff increment |
/// | 23–24 | MARK / HACK |
/// | 25–26 | Drift CO / WARN RESET |
#[derive(Debug, Clone, Default)]
pub struct IcpButtons {
    /// 32-bit packed bitmask; bit `n−1` set → button `n` pressed.
    pub mask: u32,
    /// Encoder deltas: [0] = DEDUP/RET wheel, [1] = SYM wheel.
    pub encoders: [i8; ENCODER_COUNT],
    /// Rocker switch state: 0 = center, 1 = up, 2 = down.
    pub rocker: u8,
}

impl IcpButtons {
    /// Returns `true` if button `n` (1-indexed, 1–26) is pressed.
    pub fn is_pressed(&self, n: u8) -> bool {
        (1u8..=BUTTON_COUNT).contains(&n) && (self.mask >> (n - 1)) & 1 == 1
    }
}

// ── Input state ───────────────────────────────────────────────────────────────

/// Parsed state from a single F-16 ICP HID report.
#[derive(Debug, Clone)]
pub struct F16IcpInputState {
    pub buttons: IcpButtons,
}

// ── Parser ────────────────────────────────────────────────────────────────────

/// Parse a raw HID report from the WinWing F-16 ICP.
///
/// # Errors
///
/// Returns [`F16IcpParseError::TooShort`] if `data` is shorter than
/// [`MIN_REPORT_BYTES`], or [`F16IcpParseError::UnknownReportId`] if
/// `data[0]` is not `0x08`.
///
/// # Note
///
/// The report format has **not** been verified against real hardware.
pub fn parse_f16_icp_report(data: &[u8]) -> Result<F16IcpInputState, F16IcpParseError> {
    if data.len() < MIN_REPORT_BYTES {
        return Err(F16IcpParseError::TooShort {
            expected: MIN_REPORT_BYTES,
            got: data.len(),
        });
    }
    if data[0] != REPORT_ID {
        return Err(F16IcpParseError::UnknownReportId { id: data[0] });
    }

    let mask = u32::from_le_bytes([data[1], data[2], data[3], data[4]]);
    let encoders = [data[5] as i8, data[6] as i8];
    let rocker = data[7].min(2); // clamp to valid range

    Ok(F16IcpInputState {
        buttons: IcpButtons {
            mask,
            encoders,
            rocker,
        },
    })
}

// ── Error type ────────────────────────────────────────────────────────────────

/// Errors returned by [`parse_f16_icp_report`].
#[derive(Debug, Error, PartialEq)]
pub enum F16IcpParseError {
    #[error("report too short: expected ≥{expected} bytes, got {got}")]
    TooShort { expected: usize, got: usize },
    #[error("unknown report ID: 0x{id:02X}")]
    UnknownReportId { id: u8 },
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_report(b0: u8, b1: u8, b2: u8, b3: u8, enc0: i8, enc1: i8, rocker: u8) -> [u8; MIN_REPORT_BYTES] {
        [REPORT_ID, b0, b1, b2, b3, enc0 as u8, enc1 as u8, rocker]
    }

    fn all_zero() -> [u8; MIN_REPORT_BYTES] {
        make_report(0, 0, 0, 0, 0, 0, 0)
    }

    #[test]
    fn test_no_buttons_pressed() {
        let s = parse_f16_icp_report(&all_zero()).unwrap();
        for n in 1..=BUTTON_COUNT {
            assert!(!s.buttons.is_pressed(n), "button {n} should not be pressed");
        }
    }

    #[test]
    fn test_button_1_pressed() {
        let r = make_report(0x01, 0, 0, 0, 0, 0, 0);
        let s = parse_f16_icp_report(&r).unwrap();
        assert!(s.buttons.is_pressed(1));
        assert!(!s.buttons.is_pressed(2));
    }

    #[test]
    fn test_button_8_pressed() {
        let r = make_report(0x80, 0, 0, 0, 0, 0, 0);
        let s = parse_f16_icp_report(&r).unwrap();
        assert!(s.buttons.is_pressed(8));
    }

    #[test]
    fn test_button_16_pressed() {
        let r = make_report(0, 0x80, 0, 0, 0, 0, 0);
        let s = parse_f16_icp_report(&r).unwrap();
        assert!(s.buttons.is_pressed(16));
    }

    #[test]
    fn test_button_26_pressed() {
        let r = make_report(0, 0, 0, 0x02, 0, 0, 0);
        let s = parse_f16_icp_report(&r).unwrap();
        assert!(s.buttons.is_pressed(26));
    }

    #[test]
    fn test_multiple_buttons() {
        let r = make_report(0x05, 0, 0, 0x01, 0, 0, 0);
        let s = parse_f16_icp_report(&r).unwrap();
        assert!(s.buttons.is_pressed(1));
        assert!(s.buttons.is_pressed(3));
        assert!(s.buttons.is_pressed(25));
    }

    #[test]
    fn test_all_buttons_pressed() {
        // Bits 0–25 set = 0x03FF_FFFF
        let mask = 0x03FF_FFFFu32.to_le_bytes();
        let r = [REPORT_ID, mask[0], mask[1], mask[2], mask[3], 0, 0, 0];
        let s = parse_f16_icp_report(&r).unwrap();
        for n in 1..=BUTTON_COUNT {
            assert!(s.buttons.is_pressed(n), "button {n} should be pressed");
        }
        assert!(!s.buttons.is_pressed(27));
    }

    #[test]
    fn test_out_of_range_buttons() {
        let s = parse_f16_icp_report(&all_zero()).unwrap();
        assert!(!s.buttons.is_pressed(0));
        assert!(!s.buttons.is_pressed(BUTTON_COUNT + 1));
    }

    #[test]
    fn test_encoder_deltas() {
        let r = make_report(0, 0, 0, 0, 3, -2, 0);
        let s = parse_f16_icp_report(&r).unwrap();
        assert_eq!(s.buttons.encoders[0], 3);
        assert_eq!(s.buttons.encoders[1], -2);
    }

    #[test]
    fn test_rocker_center() {
        let r = make_report(0, 0, 0, 0, 0, 0, 0);
        let s = parse_f16_icp_report(&r).unwrap();
        assert_eq!(s.buttons.rocker, 0);
    }

    #[test]
    fn test_rocker_up() {
        let r = make_report(0, 0, 0, 0, 0, 0, 1);
        let s = parse_f16_icp_report(&r).unwrap();
        assert_eq!(s.buttons.rocker, 1);
    }

    #[test]
    fn test_rocker_down() {
        let r = make_report(0, 0, 0, 0, 0, 0, 2);
        let s = parse_f16_icp_report(&r).unwrap();
        assert_eq!(s.buttons.rocker, 2);
    }

    #[test]
    fn test_rocker_clamped() {
        let r = make_report(0, 0, 0, 0, 0, 0, 255);
        let s = parse_f16_icp_report(&r).unwrap();
        assert_eq!(s.buttons.rocker, 2);
    }

    #[test]
    fn test_report_too_short() {
        let err = parse_f16_icp_report(&[0u8; 3]).unwrap_err();
        assert_eq!(
            err,
            F16IcpParseError::TooShort {
                expected: MIN_REPORT_BYTES,
                got: 3
            }
        );
    }

    #[test]
    fn test_wrong_report_id() {
        let mut r = all_zero();
        r[0] = 0xFF;
        let err = parse_f16_icp_report(&r).unwrap_err();
        assert_eq!(err, F16IcpParseError::UnknownReportId { id: 0xFF });
    }

    #[test]
    fn test_empty_report() {
        let err = parse_f16_icp_report(&[]).unwrap_err();
        assert_eq!(
            err,
            F16IcpParseError::TooShort {
                expected: MIN_REPORT_BYTES,
                got: 0
            }
        );
    }
}
