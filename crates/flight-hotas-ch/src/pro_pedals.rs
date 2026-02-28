// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! HID input parsing for the CH Products Pro Pedals.
//!
//! # Device identifier
//!
//! VID 0x068E (CH Products), PID 0x00F2 (from Linux kernel hid-ids.h).
//!
//! # Report format (minimum 7 bytes)
//!
//! ```text
//! byte  0      : report_id (0x01)
//! bytes 1–2    : rudder LE u16
//! bytes 3–4    : left_toe LE u16
//! bytes 5–6    : right_toe LE u16
//! ```

use crate::ChError;

/// PID for the CH Products Pro Pedals (confirmed from Linux kernel hid-ids.h).
pub const PRO_PEDALS_PID: u16 = 0x00F2;

/// Minimum byte count for a Pro Pedals HID report.
pub const PRO_PEDALS_MIN_REPORT_BYTES: usize = 7;

/// Parsed input state from one CH Pro Pedals HID report.
#[derive(Debug, Clone, PartialEq)]
pub struct ProPedalsState {
    /// Main rudder axis, 0–65535.
    pub rudder: u16,
    /// Left toe brake, 0–65535.
    pub left_toe: u16,
    /// Right toe brake, 0–65535.
    pub right_toe: u16,
}

/// Parse one raw HID report from the CH Pro Pedals.
///
/// Returns [`ChError::TooShort`] if `report.len() < 7`, or
/// [`ChError::InvalidReportId`] if `report[0] != 0x01`.
pub fn parse_pro_pedals(report: &[u8]) -> Result<ProPedalsState, ChError> {
    if report.len() < PRO_PEDALS_MIN_REPORT_BYTES {
        return Err(ChError::TooShort {
            need: PRO_PEDALS_MIN_REPORT_BYTES,
            got: report.len(),
        });
    }
    if report[0] != 0x01 {
        return Err(ChError::InvalidReportId(report[0]));
    }

    Ok(ProPedalsState {
        rudder: u16::from_le_bytes([report[1], report[2]]),
        left_toe: u16::from_le_bytes([report[3], report[4]]),
        right_toe: u16::from_le_bytes([report[5], report[6]]),
    })
}

/// Normalise a pedal axis value to `[-1.0, 1.0]`.
///
/// Maps `0` → `-1.0`, `32768` ≈ `0.0`, `65535` → `1.0`.
/// For the rudder axis this maps left-full to right-full.
/// For toe brakes the caller can map to `[0.0, 1.0]` via `(v + 1.0) / 2.0`.
pub fn normalize_pedal(raw: u16) -> f32 {
    (raw as f32 / 65535.0 * 2.0 - 1.0).clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn make_report(rudder: u16, left: u16, right: u16) -> [u8; 7] {
        let mut r = [0u8; 7];
        r[0] = 0x01;
        r[1..3].copy_from_slice(&rudder.to_le_bytes());
        r[3..5].copy_from_slice(&left.to_le_bytes());
        r[5..7].copy_from_slice(&right.to_le_bytes());
        r
    }

    #[test]
    fn too_short_returns_error() {
        assert!(parse_pro_pedals(&[0x01; 6]).is_err());
    }

    #[test]
    fn all_zero_parses_ok() {
        let r = make_report(0, 0, 0);
        let s = parse_pro_pedals(&r).unwrap();
        assert_eq!((s.rudder, s.left_toe, s.right_toe), (0, 0, 0));
    }

    #[test]
    fn max_rudder_parses_correctly() {
        let r = make_report(65535, 0, 0);
        let s = parse_pro_pedals(&r).unwrap();
        assert_eq!(s.rudder, 65535);
    }

    #[test]
    fn normalize_pedal_bounds() {
        assert!((normalize_pedal(0) + 1.0).abs() < 1e-4);
        assert!((normalize_pedal(65535) - 1.0).abs() < 1e-4);
    }

    proptest! {
        #[test]
        fn pedal_always_in_range(raw in 0u16..=u16::MAX) {
            let n = normalize_pedal(raw);
            prop_assert!((-1.0..=1.0).contains(&n));
        }

        #[test]
        fn random_report_does_not_panic(data in proptest::collection::vec(0u8..=255u8, 7..32)) {
            let _ = parse_pro_pedals(&data);
        }
    }
}
