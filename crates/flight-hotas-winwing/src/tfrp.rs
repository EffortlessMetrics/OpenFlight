// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID report parser for the WinWing TFRP Rudder Pedals (PID 0xBE64).
//!
//! The TFRP is a three-axis rudder pedal unit with a central rudder axis and
//! independent left and right toe brakes.  There are no buttons.
//!
//! # Report layout (8 bytes, report ID `0x03`)
//!
//! | Offset | Size | Field |
//! |--------|------|-------|
//! | 0      | 1    | Report ID (`0x03`) |
//! | 1      | 2    | Rudder axis (i16 LE, −32768..32767 → −1.0..1.0) |
//! | 3      | 2    | Left toe brake (u16 LE, 0..65535 → 0.0..1.0) |
//! | 5      | 2    | Right toe brake (u16 LE, 0..65535 → 0.0..1.0) |
//! | 7      | 1    | Reserved / padding |

use thiserror::Error;

/// USB Product ID for the WinWing TFRP Rudder Pedals.
pub const TFRP_RUDDER_PID: u16 = 0xBE64;

/// Minimum bytes required in a valid TFRP HID report.
pub const MIN_REPORT_BYTES: usize = 8;

const REPORT_ID: u8 = 0x03;

// ── Axis snapshot ─────────────────────────────────────────────────────────────

/// Axis snapshot for the WinWing TFRP Rudder Pedals.
#[derive(Debug, Clone, PartialEq)]
pub struct TfrpAxes {
    /// Rudder axis — \[−1.0, 1.0\] (negative = left yaw, positive = right yaw).
    pub rudder: f32,
    /// Left toe brake — \[0.0, 1.0\] (0.0 = released, 1.0 = fully depressed).
    pub brake_left: f32,
    /// Right toe brake — \[0.0, 1.0\] (0.0 = released, 1.0 = fully depressed).
    pub brake_right: f32,
}

// ── Input state ───────────────────────────────────────────────────────────────

/// Parsed state from a single TFRP Rudder Pedals HID report.
#[derive(Debug, Clone)]
pub struct TfrpInputState {
    pub axes: TfrpAxes,
}

// ── Parser ────────────────────────────────────────────────────────────────────

/// Parse a raw HID report from the WinWing TFRP Rudder Pedals.
///
/// # Errors
///
/// Returns [`TfrpParseError::TooShort`] if `data` is shorter than
/// [`MIN_REPORT_BYTES`], or [`TfrpParseError::UnknownReportId`] if
/// `data[0]` is not `0x03`.
pub fn parse_tfrp_report(data: &[u8]) -> Result<TfrpInputState, TfrpParseError> {
    if data.len() < MIN_REPORT_BYTES {
        return Err(TfrpParseError::TooShort {
            expected: MIN_REPORT_BYTES,
            got: data.len(),
        });
    }
    if data[0] != REPORT_ID {
        return Err(TfrpParseError::UnknownReportId { id: data[0] });
    }

    let rudder = norm_i16(i16::from_le_bytes([data[1], data[2]]));
    let brake_left = read_u16(data, 3) as f32 / 65535.0;
    let brake_right = read_u16(data, 5) as f32 / 65535.0;

    Ok(TfrpInputState {
        axes: TfrpAxes {
            rudder,
            brake_left,
            brake_right,
        },
    })
}

// ── Error type ────────────────────────────────────────────────────────────────

/// Errors returned by [`parse_tfrp_report`].
#[derive(Debug, Error, PartialEq)]
pub enum TfrpParseError {
    #[error("report too short: expected ≥{expected} bytes, got {got}")]
    TooShort { expected: usize, got: usize },
    #[error("unknown report ID: 0x{id:02X}")]
    UnknownReportId { id: u8 },
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn read_u16(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

/// Normalise a signed 16-bit integer to the range \[−1.0, 1.0\].
fn norm_i16(v: i16) -> f32 {
    v as f32 / 32767.0
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Build a minimal valid report with the given axis raw values.
    fn make_report(rudder: i16, bl: u16, br: u16) -> [u8; MIN_REPORT_BYTES] {
        let mut r = [0u8; MIN_REPORT_BYTES];
        r[0] = REPORT_ID;
        r[1..3].copy_from_slice(&rudder.to_le_bytes());
        r[3..5].copy_from_slice(&bl.to_le_bytes());
        r[5..7].copy_from_slice(&br.to_le_bytes());
        // r[7] is the reserved/padding byte — left as 0
        r
    }

    #[test]
    fn test_all_axes_centred() {
        let s = parse_tfrp_report(&make_report(0, 0, 0)).unwrap();
        assert!(s.axes.rudder.abs() < 1e-4, "rudder should be ~0");
        assert!(s.axes.brake_left < 1e-4, "left brake should be ~0");
        assert!(s.axes.brake_right < 1e-4, "right brake should be ~0");
    }

    #[test]
    fn test_full_right_rudder() {
        let s = parse_tfrp_report(&make_report(32767, 0, 0)).unwrap();
        assert!((s.axes.rudder - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_full_left_rudder() {
        // i16::MIN / 32767 is just below -1.0; allow small overshoot.
        let s = parse_tfrp_report(&make_report(i16::MIN, 0, 0)).unwrap();
        assert!(s.axes.rudder < 0.0);
    }

    #[test]
    fn test_full_left_brake() {
        let s = parse_tfrp_report(&make_report(0, 0xFFFF, 0)).unwrap();
        assert!((s.axes.brake_left - 1.0).abs() < 1e-4);
        assert!(s.axes.brake_right < 1e-4);
    }

    #[test]
    fn test_full_right_brake() {
        let s = parse_tfrp_report(&make_report(0, 0, 0xFFFF)).unwrap();
        assert!(s.axes.brake_left < 1e-4);
        assert!((s.axes.brake_right - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_both_brakes_full() {
        let s = parse_tfrp_report(&make_report(0, 0xFFFF, 0xFFFF)).unwrap();
        assert!((s.axes.brake_left - 1.0).abs() < 1e-4);
        assert!((s.axes.brake_right - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_report_too_short() {
        let err = parse_tfrp_report(&[0u8; 4]).unwrap_err();
        assert_eq!(
            err,
            TfrpParseError::TooShort {
                expected: MIN_REPORT_BYTES,
                got: 4
            }
        );
    }

    #[test]
    fn test_empty_report() {
        let err = parse_tfrp_report(&[]).unwrap_err();
        assert_eq!(
            err,
            TfrpParseError::TooShort {
                expected: MIN_REPORT_BYTES,
                got: 0
            }
        );
    }

    #[test]
    fn test_wrong_report_id() {
        let mut r = make_report(0, 0, 0);
        r[0] = 0xFF;
        let err = parse_tfrp_report(&r).unwrap_err();
        assert_eq!(err, TfrpParseError::UnknownReportId { id: 0xFF });
    }

    #[test]
    fn test_padding_byte_ignored() {
        let mut r = make_report(0, 0, 0);
        r[7] = 0xAB; // padding byte should be ignored
        let s = parse_tfrp_report(&r).unwrap();
        assert!(s.axes.rudder.abs() < 1e-4);
    }

    proptest! {
        #[test]
        fn prop_rudder_always_in_range(raw: i16) {
            let s = parse_tfrp_report(&make_report(raw, 0, 0)).unwrap();
            prop_assert!(
                s.axes.rudder >= -1.001 && s.axes.rudder <= 1.001,
                "rudder out of range: {}",
                s.axes.rudder
            );
        }

        #[test]
        fn prop_brakes_always_in_range(bl: u16, br: u16) {
            let s = parse_tfrp_report(&make_report(0, bl, br)).unwrap();
            prop_assert!(
                s.axes.brake_left >= 0.0 && s.axes.brake_left <= 1.0,
                "left brake out of range: {}",
                s.axes.brake_left
            );
            prop_assert!(
                s.axes.brake_right >= 0.0 && s.axes.brake_right <= 1.0,
                "right brake out of range: {}",
                s.axes.brake_right
            );
        }
    }
}
