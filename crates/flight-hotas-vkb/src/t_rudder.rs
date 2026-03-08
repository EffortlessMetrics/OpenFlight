// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! HID input parsing for VKB T-Rudder Mk.IV/V pedals.
//!
//! # Report layout (ASSUMED — not captured from hardware)
//!
//! ```text
//! bytes 0–1 : left toe brake    (u16 LE, 0..65535 → 0.0..=1.0)
//! bytes 2–3 : right toe brake   (u16 LE, 0..65535 → 0.0..=1.0)
//! bytes 4–5 : rudder axis       (u16 LE, 0..65535 → −1.0..=1.0)
//! ```
//!
//! Minimum report payload length: 6 bytes.
//! Reports shorter than 6 bytes return an error; extra bytes are silently ignored.

use thiserror::Error;

/// Minimum byte count for a T-Rudder HID report payload (excluding optional report ID).
pub const T_RUDDER_MIN_PAYLOAD_BYTES: usize = 6;

/// T-Rudder variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TRudderVariant {
    /// T-Rudder Mk.IV.
    Mk4,
    /// T-Rudder Mk.V.
    Mk5,
}

impl TRudderVariant {
    /// Human-readable product name.
    pub fn product_name(self) -> &'static str {
        match self {
            Self::Mk4 => "VKB T-Rudder Mk.IV",
            Self::Mk5 => "VKB T-Rudder Mk.V",
        }
    }
}

/// Parsed axes from one T-Rudder HID report.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TRudderAxes {
    /// Left toe brake, `0.0..=1.0`.
    pub left_toe_brake: f32,
    /// Right toe brake, `0.0..=1.0`.
    pub right_toe_brake: f32,
    /// Rudder axis, `−1.0..=1.0`.
    pub rudder: f32,
}

/// Parsed state from one T-Rudder HID report.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TRudderInputState {
    /// Device variant.
    pub variant: TRudderVariant,
    /// Normalised axes.
    pub axes: TRudderAxes,
}

/// Parse errors for T-Rudder reports.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TRudderParseError {
    /// Report payload is shorter than the minimum required size.
    #[error("T-Rudder report too short: expected at least {expected} bytes, got {actual}")]
    ReportTooShort { expected: usize, actual: usize },
}

/// Parser for VKB T-Rudder Mk.IV/V HID reports.
#[derive(Debug, Clone, Copy)]
pub struct TRudderInputHandler {
    variant: TRudderVariant,
    has_report_id: bool,
}

impl TRudderInputHandler {
    /// Create a parser for the given T-Rudder variant.
    pub fn new(variant: TRudderVariant) -> Self {
        Self {
            variant,
            has_report_id: false,
        }
    }

    /// Enable stripping a 1-byte HID report ID prefix.
    pub fn with_report_id(mut self, enabled: bool) -> Self {
        self.has_report_id = enabled;
        self
    }

    /// Return the associated variant.
    pub fn variant(&self) -> TRudderVariant {
        self.variant
    }

    /// Parse one T-Rudder HID report.
    pub fn parse_report(&self, report: &[u8]) -> Result<TRudderInputState, TRudderParseError> {
        let payload = if self.has_report_id {
            report.get(1..).unwrap_or(&[])
        } else {
            report
        };

        if payload.len() < T_RUDDER_MIN_PAYLOAD_BYTES {
            return Err(TRudderParseError::ReportTooShort {
                expected: T_RUDDER_MIN_PAYLOAD_BYTES,
                actual: payload.len(),
            });
        }

        let axes = TRudderAxes {
            left_toe_brake: normalize_u16(le_u16(payload, 0)),
            right_toe_brake: normalize_u16(le_u16(payload, 2)),
            rudder: normalize_signed(le_u16(payload, 4)),
        };

        Ok(TRudderInputState {
            variant: self.variant,
            axes,
        })
    }
}

fn le_u16(bytes: &[u8], offset: usize) -> u16 {
    let low = bytes.get(offset).copied().unwrap_or(0);
    let high = bytes.get(offset + 1).copied().unwrap_or(0);
    u16::from_le_bytes([low, high])
}

fn normalize_u16(raw: u16) -> f32 {
    (raw as f32 / u16::MAX as f32).clamp(0.0, 1.0)
}

fn normalize_signed(raw: u16) -> f32 {
    ((raw as f32 / 32767.5) - 1.0).clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_report(left: u16, right: u16, rudder: u16) -> [u8; 6] {
        let mut r = [0u8; 6];
        r[0..2].copy_from_slice(&left.to_le_bytes());
        r[2..4].copy_from_slice(&right.to_le_bytes());
        r[4..6].copy_from_slice(&rudder.to_le_bytes());
        r
    }

    #[test]
    fn report_too_short_returns_error() {
        let handler = TRudderInputHandler::new(TRudderVariant::Mk4);
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
    fn idle_position_all_zero() {
        let handler = TRudderInputHandler::new(TRudderVariant::Mk4);
        let report = make_report(0, 0, 0x8000);
        let state = handler.parse_report(&report).unwrap();
        assert_eq!(state.axes.left_toe_brake, 0.0);
        assert_eq!(state.axes.right_toe_brake, 0.0);
        assert!(state.axes.rudder.abs() < 0.01);
    }

    #[test]
    fn full_brakes_and_rudder_left() {
        let handler = TRudderInputHandler::new(TRudderVariant::Mk5);
        let report = make_report(0xFFFF, 0xFFFF, 0x0000);
        let state = handler.parse_report(&report).unwrap();
        assert!((state.axes.left_toe_brake - 1.0).abs() < 0.001);
        assert!((state.axes.right_toe_brake - 1.0).abs() < 0.001);
        assert!((state.axes.rudder - (-1.0)).abs() < 0.01);
    }

    #[test]
    fn rudder_full_right() {
        let handler = TRudderInputHandler::new(TRudderVariant::Mk4);
        let report = make_report(0, 0, 0xFFFF);
        let state = handler.parse_report(&report).unwrap();
        assert!((state.axes.rudder - 1.0).abs() < 0.01);
    }

    #[test]
    fn with_report_id_strips_prefix() {
        let handler = TRudderInputHandler::new(TRudderVariant::Mk5).with_report_id(true);
        let mut report = vec![0x01u8];
        report.extend_from_slice(&make_report(0xFFFF, 0, 0x8000));
        let state = handler.parse_report(&report).unwrap();
        assert!((state.axes.left_toe_brake - 1.0).abs() < 0.001);
        assert_eq!(state.axes.right_toe_brake, 0.0);
    }

    #[test]
    fn variant_preserved() {
        let mk4 = TRudderInputHandler::new(TRudderVariant::Mk4);
        let mk5 = TRudderInputHandler::new(TRudderVariant::Mk5);
        let report = make_report(0, 0, 0x8000);
        assert_eq!(
            mk4.parse_report(&report).unwrap().variant,
            TRudderVariant::Mk4
        );
        assert_eq!(
            mk5.parse_report(&report).unwrap().variant,
            TRudderVariant::Mk5
        );
    }

    #[test]
    fn variant_names() {
        assert_eq!(TRudderVariant::Mk4.product_name(), "VKB T-Rudder Mk.IV");
        assert_eq!(TRudderVariant::Mk5.product_name(), "VKB T-Rudder Mk.V");
    }

    #[test]
    fn longer_report_does_not_error() {
        let handler = TRudderInputHandler::new(TRudderVariant::Mk4);
        let mut report = make_report(0x1234, 0x5678, 0xABCD).to_vec();
        report.extend_from_slice(&[0xFFu8; 4]);
        assert!(handler.parse_report(&report).is_ok());
    }
}
