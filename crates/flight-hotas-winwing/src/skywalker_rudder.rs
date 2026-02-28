// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID report parser for the WinWing Skywalker Metal Rudder Pedals (PID 0xBEF0).
//!
//! The Skywalker pedals are premium all-metal rudder pedals with independent
//! toe brakes and a rudder twist axis.  A computed differential toe-brake axis
//! is derived from the left and right brake values.
//!
//! # ASSUMED report layout (8 bytes, report ID `0x07`)
//!
//! *This layout was derived by analogy with the TFRP rudder pedals (PID
//! 0xBE64) and community notes.  It has **not** been verified against actual
//! hardware.  Treat every byte offset as ASSUMED until confirmed with a
//! hardware capture.*
//!
//! | Offset | Size | Field |
//! |--------|------|-------|
//! | 0      | 1    | Report ID (`0x07`) — ASSUMED |
//! | 1      | 2    | Rudder axis (i16 LE, −32768..32767 → −1.0..1.0) — ASSUMED |
//! | 3      | 2    | Left toe brake (u16 LE, 0..65535 → 0.0..1.0) — ASSUMED |
//! | 5      | 2    | Right toe brake (u16 LE, 0..65535 → 0.0..1.0) — ASSUMED |
//! | 7      | 1    | Reserved / padding — ASSUMED |

use thiserror::Error;

/// USB Product ID for the WinWing Skywalker Metal Rudder Pedals.
pub const SKYWALKER_RUDDER_PID: u16 = 0xBEF0;

/// Minimum bytes required in a valid Skywalker rudder HID report.
// ASSUMED: 1 report-ID + 2 rudder + 2 left-brake + 2 right-brake + 1 padding = 8 bytes
pub const MIN_REPORT_BYTES: usize = 8;

// ASSUMED report ID (follows TFRP's 0x03 with room for other devices)
const REPORT_ID: u8 = 0x07;

// ── Axis snapshot ─────────────────────────────────────────────────────────────

/// Axis snapshot for the WinWing Skywalker Metal Rudder Pedals.
#[derive(Debug, Clone, PartialEq)]
pub struct SkywalkerAxes {
    /// Rudder axis — \[−1.0, 1.0\] (negative = left yaw, positive = right yaw).
    pub rudder: f32,
    /// Left toe brake — \[0.0, 1.0\] (0.0 = released, 1.0 = fully depressed).
    pub brake_left: f32,
    /// Right toe brake — \[0.0, 1.0\] (0.0 = released, 1.0 = fully depressed).
    pub brake_right: f32,
    /// Differential toe brake — \[−1.0, 1.0\].
    ///
    /// Computed as `brake_right − brake_left`.  Positive values indicate more
    /// right-brake pressure; negative values indicate more left-brake pressure.
    /// Useful as a single axis for differential braking in simulators.
    pub diff_brake: f32,
}

// ── Input state ───────────────────────────────────────────────────────────────

/// Parsed state from a single Skywalker rudder HID report.
#[derive(Debug, Clone)]
pub struct SkywalkerRudderInputState {
    pub axes: SkywalkerAxes,
}

// ── Parser ────────────────────────────────────────────────────────────────────

/// Parse a raw HID report from the WinWing Skywalker Metal Rudder Pedals.
///
/// # Errors
///
/// Returns [`SkywalkerParseError::TooShort`] if `data` is shorter than
/// [`MIN_REPORT_BYTES`], or [`SkywalkerParseError::UnknownReportId`] if
/// `data[0]` is not `0x07`.
///
/// # Note
///
/// The report format has **not** been verified against real hardware.
/// All byte offsets are ASSUMED by analogy with the TFRP pedals.
pub fn parse_skywalker_rudder_report(
    data: &[u8],
) -> Result<SkywalkerRudderInputState, SkywalkerParseError> {
    if data.len() < MIN_REPORT_BYTES {
        return Err(SkywalkerParseError::TooShort {
            expected: MIN_REPORT_BYTES,
            got: data.len(),
        });
    }
    if data[0] != REPORT_ID {
        return Err(SkywalkerParseError::UnknownReportId { id: data[0] });
    }

    // ASSUMED layout (mirrored from TFRP with the next available report ID)
    let rudder = norm_i16(i16::from_le_bytes([data[1], data[2]]));
    let brake_left = read_u16(data, 3) as f32 / 65535.0;
    let brake_right = read_u16(data, 5) as f32 / 65535.0;
    // Differential brake: right minus left, clamped to [-1.0, 1.0].
    let diff_brake = (brake_right - brake_left).clamp(-1.0, 1.0);

    Ok(SkywalkerRudderInputState {
        axes: SkywalkerAxes {
            rudder,
            brake_left,
            brake_right,
            diff_brake,
        },
    })
}

// ── Error type ────────────────────────────────────────────────────────────────

/// Errors returned by [`parse_skywalker_rudder_report`].
#[derive(Debug, Error, PartialEq)]
pub enum SkywalkerParseError {
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
    (v as f32 / 32767.0).clamp(-1.0, 1.0)
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
        let s = parse_skywalker_rudder_report(&make_report(0, 0, 0)).unwrap();
        assert!(s.axes.rudder.abs() < 1e-4, "rudder should be ~0");
        assert!(s.axes.brake_left < 1e-4, "left brake should be ~0");
        assert!(s.axes.brake_right < 1e-4, "right brake should be ~0");
        assert!(s.axes.diff_brake.abs() < 1e-4, "diff brake should be ~0");
    }

    #[test]
    fn test_full_right_rudder() {
        let s = parse_skywalker_rudder_report(&make_report(32767, 0, 0)).unwrap();
        assert!((s.axes.rudder - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_full_left_rudder() {
        // i16::MIN / 32767 is just below -1.0; allow small overshoot.
        let s = parse_skywalker_rudder_report(&make_report(i16::MIN, 0, 0)).unwrap();
        assert!(s.axes.rudder < 0.0);
    }

    #[test]
    fn test_full_left_brake() {
        let s = parse_skywalker_rudder_report(&make_report(0, 0xFFFF, 0)).unwrap();
        assert!((s.axes.brake_left - 1.0).abs() < 1e-4);
        assert!(s.axes.brake_right < 1e-4);
    }

    #[test]
    fn test_full_right_brake() {
        let s = parse_skywalker_rudder_report(&make_report(0, 0, 0xFFFF)).unwrap();
        assert!(s.axes.brake_left < 1e-4);
        assert!((s.axes.brake_right - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_both_brakes_full() {
        let s = parse_skywalker_rudder_report(&make_report(0, 0xFFFF, 0xFFFF)).unwrap();
        assert!((s.axes.brake_left - 1.0).abs() < 1e-4);
        assert!((s.axes.brake_right - 1.0).abs() < 1e-4);
        // Equal pressure → diff_brake ≈ 0
        assert!(s.axes.diff_brake.abs() < 1e-4);
    }

    #[test]
    fn test_diff_brake_positive_when_right_dominant() {
        // More right brake → positive diff_brake
        let s = parse_skywalker_rudder_report(&make_report(0, 0, 0xFFFF)).unwrap();
        assert!(s.axes.diff_brake > 0.0);
        assert!((s.axes.diff_brake - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_diff_brake_negative_when_left_dominant() {
        // More left brake → negative diff_brake
        let s = parse_skywalker_rudder_report(&make_report(0, 0xFFFF, 0)).unwrap();
        assert!(s.axes.diff_brake < 0.0);
        assert!((s.axes.diff_brake + 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_diff_brake_clamped() {
        // With valid u16 inputs diff_brake is always in [-1.0, 1.0]; verify clamp boundary.
        let s = parse_skywalker_rudder_report(&make_report(0, 0xFFFF, 0xFFFF)).unwrap();
        assert!((-1.0..=1.0).contains(&s.axes.diff_brake));
    }

    #[test]
    fn test_report_too_short() {
        let err = parse_skywalker_rudder_report(&[0u8; 4]).unwrap_err();
        assert_eq!(
            err,
            SkywalkerParseError::TooShort {
                expected: MIN_REPORT_BYTES,
                got: 4
            }
        );
    }

    #[test]
    fn test_empty_report() {
        let err = parse_skywalker_rudder_report(&[]).unwrap_err();
        assert_eq!(
            err,
            SkywalkerParseError::TooShort {
                expected: MIN_REPORT_BYTES,
                got: 0
            }
        );
    }

    #[test]
    fn test_wrong_report_id() {
        let mut r = make_report(0, 0, 0);
        r[0] = 0xFF;
        let err = parse_skywalker_rudder_report(&r).unwrap_err();
        assert_eq!(err, SkywalkerParseError::UnknownReportId { id: 0xFF });
    }

    proptest! {
        #[test]
        fn prop_rudder_always_in_range(raw: i16) {
            let s = parse_skywalker_rudder_report(&make_report(raw, 0, 0)).unwrap();
            prop_assert!(
                (-1.001..=1.001).contains(&s.axes.rudder),
                "rudder out of range: {}",
                s.axes.rudder
            );
        }

        #[test]
        fn prop_brakes_always_in_range(bl: u16, br: u16) {
            let s = parse_skywalker_rudder_report(&make_report(0, bl, br)).unwrap();
            prop_assert!(
                (0.0..=1.0).contains(&s.axes.brake_left),
                "left brake out of range: {}",
                s.axes.brake_left
            );
            prop_assert!(
                (0.0..=1.0).contains(&s.axes.brake_right),
                "right brake out of range: {}",
                s.axes.brake_right
            );
            prop_assert!(
                (-1.0..=1.0).contains(&s.axes.diff_brake),
                "diff_brake out of range: {}",
                s.axes.diff_brake
            );
        }
    }
}
