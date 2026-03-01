// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! HID input parsing for VKB T-Rudder Mk.IV / Mk.V pedal units.
//!
//! # Supported devices
//!
//! | Model        | VID    | PID    | Source |
//! |--------------|--------|--------|--------|
//! | T-Rudder Mk.IV | 0x231D | (unconfirmed) | Community-reported |
//! | T-Rudder Mk.V  | 0x231D | 0x0132 | Community estimate |
//!
//! # Report format (ASSUMED)
//!
//! The VKB T-Rudder pedal unit has three analogue axes and zero buttons/hats.
//! All VKB-firmware devices use the same u16 LE axis encoding:
//!
//! ```text
//! byte  0–1 : left toe brake  (u16 LE, 0x0000–0xFFFF → 0.0–1.0)
//! byte  2–3 : right toe brake (u16 LE, 0x0000–0xFFFF → 0.0–1.0)
//! byte  4–5 : rudder          (u16 LE, 0x0000–0xFFFF → −1.0–1.0)
//! ```
//!
//! The optional 1-byte HID report ID (`0x01`) is stripped when
//! [`TRudderInputHandler::with_report_id`] is enabled.
//!
//! # Axis semantics
//!
//! - **Toe brakes** are unidirectional: 0x0000 = released, 0xFFFF = fully depressed.
//! - **Rudder** is bidirectional: 0x0000 = full left, 0x8000 ≈ centre, 0xFFFF = full right.
//! - Hall-effect sensors on all three axes (no mechanical wear).

use crate::protocol::{le_u16, normalize_signed, normalize_u16};

/// Minimum report payload size in bytes (excluding optional report ID).
///
/// 3 axes × 2 bytes = 6 bytes.
pub const T_RUDDER_MIN_REPORT_BYTES: usize = 6;

/// Parsed axes from one VKB T-Rudder HID report.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TRudderAxes {
    /// Left toe brake, `0.0` (released) to `1.0` (fully depressed).
    pub left_toe_brake: f32,
    /// Right toe brake, `0.0` (released) to `1.0` (fully depressed).
    pub right_toe_brake: f32,
    /// Rudder yaw axis, `−1.0` (full left) to `1.0` (full right).
    pub rudder: f32,
}

/// Parsed state from one VKB T-Rudder HID report.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TRudderInputState {
    /// Normalised axes.
    pub axes: TRudderAxes,
}

/// Parse errors for T-Rudder reports.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TRudderParseError {
    /// Report payload is shorter than the minimum required size.
    #[error("T-Rudder report too short: expected at least {expected} bytes, got {actual}")]
    ReportTooShort { expected: usize, actual: usize },
}

/// Parser for VKB T-Rudder Mk.IV / Mk.V HID reports.
///
/// ## Expected HID report layout (ASSUMED)
///
/// | Bytes | Content |
/// |-------|---------|
/// | 0–1   | Left toe brake (u16 LE, 0..65535 → 0.0..=1.0) |
/// | 2–3   | Right toe brake (u16 LE, 0..65535 → 0.0..=1.0) |
/// | 4–5   | Rudder yaw (u16 LE, 0..65535 → −1.0..=1.0) |
#[derive(Debug, Clone, Copy)]
pub struct TRudderInputHandler {
    has_report_id: bool,
}

impl TRudderInputHandler {
    /// Create a new T-Rudder parser.
    pub fn new() -> Self {
        Self {
            has_report_id: false,
        }
    }

    /// Enable stripping a 1-byte HID report ID prefix.
    pub fn with_report_id(mut self, enabled: bool) -> Self {
        self.has_report_id = enabled;
        self
    }

    /// Parse one T-Rudder HID report.
    pub fn parse_report(&self, report: &[u8]) -> Result<TRudderInputState, TRudderParseError> {
        let payload = if self.has_report_id {
            report.get(1..).unwrap_or(&[])
        } else {
            report
        };

        if payload.len() < T_RUDDER_MIN_REPORT_BYTES {
            return Err(TRudderParseError::ReportTooShort {
                expected: T_RUDDER_MIN_REPORT_BYTES,
                actual: payload.len(),
            });
        }

        let axes = TRudderAxes {
            left_toe_brake: normalize_u16(le_u16(payload, 0)),
            right_toe_brake: normalize_u16(le_u16(payload, 2)),
            rudder: normalize_signed(le_u16(payload, 4)),
        };

        Ok(TRudderInputState { axes })
    }
}

impl Default for TRudderInputHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_rudder_report(left: u16, right: u16, rudder: u16) -> Vec<u8> {
        let mut buf = Vec::with_capacity(6);
        buf.extend_from_slice(&left.to_le_bytes());
        buf.extend_from_slice(&right.to_le_bytes());
        buf.extend_from_slice(&rudder.to_le_bytes());
        buf
    }

    #[test]
    fn report_too_short() {
        let handler = TRudderInputHandler::new();
        let err = handler.parse_report(&[0u8; 4]);
        assert!(matches!(
            err,
            Err(TRudderParseError::ReportTooShort {
                expected: 6,
                actual: 4
            })
        ));
    }

    #[test]
    fn all_zeroes_means_released_and_full_left() {
        let handler = TRudderInputHandler::new();
        let report = make_rudder_report(0, 0, 0);
        let state = handler.parse_report(&report).unwrap();
        assert_eq!(state.axes.left_toe_brake, 0.0);
        assert_eq!(state.axes.right_toe_brake, 0.0);
        assert!((state.axes.rudder - (-1.0)).abs() < 0.01);
    }

    #[test]
    fn full_brakes_and_centred_rudder() {
        let handler = TRudderInputHandler::new();
        let report = make_rudder_report(0xFFFF, 0xFFFF, 0x8000);
        let state = handler.parse_report(&report).unwrap();
        assert!((state.axes.left_toe_brake - 1.0).abs() < 0.001);
        assert!((state.axes.right_toe_brake - 1.0).abs() < 0.001);
        assert!(state.axes.rudder.abs() < 0.01);
    }

    #[test]
    fn full_right_rudder() {
        let handler = TRudderInputHandler::new();
        let report = make_rudder_report(0, 0, 0xFFFF);
        let state = handler.parse_report(&report).unwrap();
        assert!((state.axes.rudder - 1.0).abs() < 0.01);
    }

    #[test]
    fn half_brake_values() {
        let handler = TRudderInputHandler::new();
        let report = make_rudder_report(0x8000, 0x4000, 0x8000);
        let state = handler.parse_report(&report).unwrap();
        assert!((state.axes.left_toe_brake - 0.5).abs() < 0.01);
        assert!((state.axes.right_toe_brake - 0.25).abs() < 0.01);
    }

    #[test]
    fn with_report_id_prefix() {
        let handler = TRudderInputHandler::new().with_report_id(true);
        let mut report = vec![0x01u8];
        report.extend_from_slice(&make_rudder_report(0xFFFF, 0, 0x8000));
        let state = handler.parse_report(&report).unwrap();
        assert!((state.axes.left_toe_brake - 1.0).abs() < 0.001);
        assert_eq!(state.axes.right_toe_brake, 0.0);
        assert!(state.axes.rudder.abs() < 0.01);
    }

    #[test]
    fn report_id_strips_exactly_one_byte() {
        let handler = TRudderInputHandler::new().with_report_id(true);
        // 1 byte report ID + 5 bytes = 6 total, but payload = 5, too short
        let short = vec![0x01, 0, 0, 0, 0, 0];
        let err = handler.parse_report(&short);
        assert!(matches!(err, Err(TRudderParseError::ReportTooShort { .. })));

        // 1 byte report ID + 6 bytes = 7 total, payload = 6, OK
        let ok = vec![0x01, 0, 0, 0, 0, 0, 0];
        assert!(handler.parse_report(&ok).is_ok());
    }

    #[test]
    fn extra_trailing_bytes_ignored() {
        let handler = TRudderInputHandler::new();
        let mut report = make_rudder_report(0x8000, 0x8000, 0x8000);
        report.extend_from_slice(&[0xFF; 10]); // extra bytes
        let state = handler.parse_report(&report).unwrap();
        assert!((state.axes.left_toe_brake - 0.5).abs() < 0.01);
    }

    #[test]
    fn default_handler_has_no_report_id() {
        let handler = TRudderInputHandler::default();
        let report = make_rudder_report(0, 0, 0x8000);
        let state = handler.parse_report(&report).unwrap();
        assert!(state.axes.rudder.abs() < 0.01);
    }
}
