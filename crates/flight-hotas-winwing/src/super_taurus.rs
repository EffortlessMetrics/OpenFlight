// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID report parser for the WinWing SuperTaurus Dual Throttle (PID 0xBD64).
//!
//! Report layout (13 bytes, report ID `0x05`):
//!
//! | Offset | Size | Field |
//! |--------|------|-------|
//! | 0      | 1    | Report ID (`0x05`) |
//! | 1      | 2    | Left throttle (u16 LE, 0..65535 → 0.0..1.0) |
//! | 3      | 2    | Right throttle (u16 LE, 0..65535 → 0.0..1.0) |
//! | 5      | 2    | Trim wheel (i16 LE, −32768..32767 → −1.0..1.0) |
//! | 7      | 4    | Button bitmask (u32 LE, bits 0–31 used for buttons 1–32) |
//! | 11     | 1    | Encoder 0 delta (i8, positive = CW) |
//! | 12     | 1    | Encoder 1 delta (i8, positive = CW) |

use thiserror::Error;

/// USB Product ID for the WinWing SuperTaurus Dual Throttle.
pub const SUPER_TAURUS_PID: u16 = 0xBD64;

/// Minimum bytes required in a valid SuperTaurus HID report.
pub const MIN_REPORT_BYTES: usize = 13;

/// Number of mapped buttons on the SuperTaurus throttle.
pub const BUTTON_COUNT: u8 = 32;

const REPORT_ID: u8 = 0x05;

// ── Axis snapshot ─────────────────────────────────────────────────────────────

/// Axis snapshot for the WinWing SuperTaurus Dual Throttle.
#[derive(Debug, Clone, PartialEq)]
pub struct SuperTaurusAxes {
    /// Left throttle lever — \[0.0, 1.0\].
    pub throttle_left: f32,
    /// Right throttle lever — \[0.0, 1.0\].
    pub throttle_right: f32,
    /// Combined throttle (average of left and right) — \[0.0, 1.0\].
    pub throttle_combined: f32,
    /// Trim wheel — \[−1.0, 1.0\].
    pub trim: f32,
}

// ── Button snapshot ───────────────────────────────────────────────────────────

/// Button state for the WinWing SuperTaurus Dual Throttle (32 buttons + 2 encoders).
#[derive(Debug, Clone, Default)]
pub struct SuperTaurusButtons {
    /// Bitmask for up to 32 buttons; bit `n−1` set → button `n` pressed.
    pub mask: u32,
    /// Encoder detent deltas (positive = CW, negative = CCW), 2 encoders.
    pub encoders: [i8; 2],
}

impl SuperTaurusButtons {
    /// Returns `true` if button `n` (1-indexed, 1–32) is pressed.
    pub fn is_pressed(&self, n: u8) -> bool {
        (1u8..=BUTTON_COUNT).contains(&n) && (self.mask >> (n - 1)) & 1 == 1
    }
}

// ── Input state ───────────────────────────────────────────────────────────────

/// Parsed state from a single SuperTaurus HID report.
#[derive(Debug, Clone)]
pub struct SuperTaurusInputState {
    pub axes: SuperTaurusAxes,
    pub buttons: SuperTaurusButtons,
}

// ── Parser ────────────────────────────────────────────────────────────────────

/// Parse a raw HID report from the WinWing SuperTaurus Dual Throttle.
///
/// # Errors
///
/// Returns [`SuperTaurusParseError::TooShort`] if `data` is shorter than
/// [`MIN_REPORT_BYTES`], or [`SuperTaurusParseError::UnknownReportId`] if
/// `data[0]` is not `0x05`.
pub fn parse_super_taurus_report(
    data: &[u8],
) -> Result<SuperTaurusInputState, SuperTaurusParseError> {
    if data.len() < MIN_REPORT_BYTES {
        return Err(SuperTaurusParseError::TooShort {
            expected: MIN_REPORT_BYTES,
            got: data.len(),
        });
    }
    if data[0] != REPORT_ID {
        return Err(SuperTaurusParseError::UnknownReportId { id: data[0] });
    }

    let tl = read_u16(data, 1) as f32 / 65535.0;
    let tr = read_u16(data, 3) as f32 / 65535.0;
    let trim = norm_i16(i16::from_le_bytes([data[5], data[6]]));
    let mask = u32::from_le_bytes([data[7], data[8], data[9], data[10]]);
    let encoders = [data[11] as i8, data[12] as i8];

    Ok(SuperTaurusInputState {
        axes: SuperTaurusAxes {
            throttle_left: tl,
            throttle_right: tr,
            throttle_combined: (tl + tr) * 0.5,
            trim,
        },
        buttons: SuperTaurusButtons { mask, encoders },
    })
}

// ── Error type ────────────────────────────────────────────────────────────────

/// Errors returned by [`parse_super_taurus_report`].
#[derive(Debug, Error, PartialEq)]
pub enum SuperTaurusParseError {
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
    (v as f32 / 32768.0).clamp(-1.0, 1.0)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Build a minimal valid report with the given throttle values.
    fn make_report(tl: u16, tr: u16) -> [u8; MIN_REPORT_BYTES] {
        let mut r = [0u8; MIN_REPORT_BYTES];
        r[0] = REPORT_ID;
        r[1..3].copy_from_slice(&tl.to_le_bytes());
        r[3..5].copy_from_slice(&tr.to_le_bytes());
        r
    }

    #[test]
    fn test_throttle_min_position() {
        let s = parse_super_taurus_report(&make_report(0, 0)).unwrap();
        assert!(s.axes.throttle_left < 0.001);
        assert!(s.axes.throttle_right < 0.001);
        assert!(s.axes.throttle_combined < 0.001);
    }

    #[test]
    fn test_throttle_max_position() {
        let s = parse_super_taurus_report(&make_report(0xFFFF, 0xFFFF)).unwrap();
        assert!((s.axes.throttle_left - 1.0).abs() < 1e-4);
        assert!((s.axes.throttle_right - 1.0).abs() < 1e-4);
        assert!((s.axes.throttle_combined - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_throttle_combined_is_average() {
        let s = parse_super_taurus_report(&make_report(0xFFFF, 0)).unwrap();
        assert!((s.axes.throttle_combined - 0.5).abs() < 1e-3);
    }

    #[test]
    fn test_trim_centred() {
        let r = make_report(0, 0); // trim bytes [5..7] = 0 → 0.0
        let s = parse_super_taurus_report(&r).unwrap();
        assert!(s.axes.trim.abs() < 1e-4);
    }

    #[test]
    fn test_trim_full_positive() {
        let mut r = make_report(0, 0);
        r[5..7].copy_from_slice(&32767i16.to_le_bytes());
        let s = parse_super_taurus_report(&r).unwrap();
        assert!((s.axes.trim - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_button_detection() {
        let mut r = make_report(0, 0);
        r[7] = 0b0000_0101; // buttons 1 and 3
        let s = parse_super_taurus_report(&r).unwrap();
        assert!(s.buttons.is_pressed(1));
        assert!(!s.buttons.is_pressed(2));
        assert!(s.buttons.is_pressed(3));
    }

    #[test]
    fn test_button_boundary() {
        let mut r = make_report(0, 0);
        // Set bit 31 (button 32).
        r[7..11].copy_from_slice(&(1u32 << 31).to_le_bytes());
        let s = parse_super_taurus_report(&r).unwrap();
        assert!(s.buttons.is_pressed(32));
        assert!(!s.buttons.is_pressed(0));
    }

    #[test]
    fn test_encoder_delta() {
        let mut r = make_report(0, 0);
        r[11] = 1u8; // encoder 0: +1 CW
        r[12] = (-2i8) as u8; // encoder 1: -2 CCW
        let s = parse_super_taurus_report(&r).unwrap();
        assert_eq!(s.buttons.encoders[0], 1);
        assert_eq!(s.buttons.encoders[1], -2);
    }

    #[test]
    fn test_report_too_short() {
        let err = parse_super_taurus_report(&[0u8; 5]).unwrap_err();
        assert_eq!(
            err,
            SuperTaurusParseError::TooShort {
                expected: MIN_REPORT_BYTES,
                got: 5
            }
        );
    }

    #[test]
    fn test_wrong_report_id() {
        let mut r = make_report(0, 0);
        r[0] = 0xFF;
        let err = parse_super_taurus_report(&r).unwrap_err();
        assert_eq!(err, SuperTaurusParseError::UnknownReportId { id: 0xFF });
    }

    proptest! {
        #[test]
        fn prop_left_throttle_in_range(raw: u16) {
            let s = parse_super_taurus_report(&make_report(raw, 0)).unwrap();
            prop_assert!(
                (0.0..=1.0).contains(&s.axes.throttle_left),
                "left throttle out of range: {}",
                s.axes.throttle_left
            );
        }

        #[test]
        fn prop_right_throttle_in_range(raw: u16) {
            let s = parse_super_taurus_report(&make_report(0, raw)).unwrap();
            prop_assert!(
                (0.0..=1.0).contains(&s.axes.throttle_right),
                "right throttle out of range: {}",
                s.axes.throttle_right
            );
        }
    }
}
