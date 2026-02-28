// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the Honeycomb Charlie Rudder Pedals.
//!
//! # Report layout (estimated)
//!
//! The exact HID descriptor for the Charlie Rudder Pedals is not publicly
//! documented. This layout is inferred from the HID joystick specification
//! and the compat YAML device description. **Hardware validation required
//! before production use.**
//!
//! ```text
//! Byte 0:       Report ID = 0x01
//! Bytes 1–2:    Rudder         — u16 LE, 0–4095, centre = 2048 (bipolar)
//! Bytes 3–4:    Left toe brake — u16 LE, 0–4095 (unipolar, 0 = released)
//! Bytes 5–6:    Right toe brake — u16 LE, 0–4095 (unipolar, 0 = released)
//! ```
//!
//! Axis resolution: 12-bit (0–4095), stored in 16-bit LE fields.
//! Charlie VID: 0x294B  PID: 0x1902 (community-inferred, unverified)

/// Expected minimum report length in bytes.
pub const CHARLIE_REPORT_LEN: usize = 7;

/// Axis values for the Charlie Rudder Pedals, normalised to standard ranges.
#[derive(Debug, Clone, PartialEq)]
pub struct CharlieAxes {
    /// Rudder axis — \[−1.0, +1.0\]; left = negative, right = positive.
    pub rudder: f32,
    /// Left toe brake — \[0.0, 1.0\]; 0.0 = released, 1.0 = fully depressed.
    pub left_brake: f32,
    /// Right toe brake — \[0.0, 1.0\]; 0.0 = released, 1.0 = fully depressed.
    pub right_brake: f32,
}

/// Parsed state from a single Charlie Rudder Pedals HID input report.
#[derive(Debug, Clone)]
pub struct CharlieInputState {
    pub axes: CharlieAxes,
}

/// Parse a raw HID input report from the Charlie Rudder Pedals.
///
/// # Errors
///
/// Returns [`CharlieParseError`] if the report is too short or has an
/// unexpected report ID byte.
///
/// # Layout assumption
///
/// See module documentation for the assumed report layout. This parser has not
/// been validated against real hardware.
pub fn parse_charlie_report(data: &[u8]) -> Result<CharlieInputState, CharlieParseError> {
    if data.len() < CHARLIE_REPORT_LEN {
        return Err(CharlieParseError::TooShort {
            expected: CHARLIE_REPORT_LEN,
            got: data.len(),
        });
    }
    if data[0] != 0x01 {
        return Err(CharlieParseError::UnknownReportId { id: data[0] });
    }

    let rudder_raw = u16::from_le_bytes([data[1], data[2]]);
    let left_brake_raw = u16::from_le_bytes([data[3], data[4]]);
    let right_brake_raw = u16::from_le_bytes([data[5], data[6]]);

    let rudder = norm_12bit_centered(rudder_raw);
    let left_brake = norm_12bit_unipolar(left_brake_raw);
    let right_brake = norm_12bit_unipolar(right_brake_raw);

    Ok(CharlieInputState {
        axes: CharlieAxes {
            rudder,
            left_brake,
            right_brake,
        },
    })
}

/// Errors returned by [`parse_charlie_report`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CharlieParseError {
    #[error("report too short: expected ≥{expected} bytes, got {got}")]
    TooShort { expected: usize, got: usize },
    #[error("unknown report ID: 0x{id:02X}")]
    UnknownReportId { id: u8 },
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Normalise a 12-bit unsigned value centred at 2048 to \[−1.0, +1.0\].
fn norm_12bit_centered(raw: u16) -> f32 {
    let raw = raw.min(4095);
    ((raw as f32 - 2048.0) / 2048.0).clamp(-1.0, 1.0)
}

/// Normalise a 12-bit unsigned value to \[0.0, 1.0\].
fn norm_12bit_unipolar(raw: u16) -> f32 {
    let raw = raw.min(4095);
    (raw as f32 / 4095.0).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn charlie_report(rudder: u16, left: u16, right: u16) -> [u8; CHARLIE_REPORT_LEN] {
        let mut r = [0u8; CHARLIE_REPORT_LEN];
        r[0] = 0x01;
        r[1..3].copy_from_slice(&rudder.to_le_bytes());
        r[3..5].copy_from_slice(&left.to_le_bytes());
        r[5..7].copy_from_slice(&right.to_le_bytes());
        r
    }

    #[test]
    fn test_neutral_position() {
        let state = parse_charlie_report(&charlie_report(2048, 0, 0)).unwrap();
        assert!(state.axes.rudder.abs() < 1e-4, "rudder should be near 0");
        assert!(state.axes.left_brake < 0.001);
        assert!(state.axes.right_brake < 0.001);
    }

    #[test]
    fn test_full_left_rudder() {
        let state = parse_charlie_report(&charlie_report(0, 0, 0)).unwrap();
        assert!(
            state.axes.rudder < -0.99,
            "full left rudder should be ~-1.0"
        );
    }

    #[test]
    fn test_full_right_rudder() {
        let state = parse_charlie_report(&charlie_report(4095, 0, 0)).unwrap();
        assert!(
            state.axes.rudder > 0.99,
            "full right rudder should be ~+1.0"
        );
    }

    #[test]
    fn test_full_brakes() {
        let state = parse_charlie_report(&charlie_report(2048, 4095, 4095)).unwrap();
        assert!((state.axes.left_brake - 1.0).abs() < 1e-4);
        assert!((state.axes.right_brake - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_asymmetric_brakes() {
        let state = parse_charlie_report(&charlie_report(2048, 4095, 0)).unwrap();
        assert!((state.axes.left_brake - 1.0).abs() < 1e-4);
        assert!(state.axes.right_brake < 0.001);
    }

    #[test]
    fn test_report_too_short() {
        let err = parse_charlie_report(&[0x01, 0x00, 0x00]).unwrap_err();
        assert!(matches!(err, CharlieParseError::TooShort { .. }));
    }

    #[test]
    fn test_unknown_report_id() {
        let mut r = [0u8; CHARLIE_REPORT_LEN];
        r[0] = 0x02;
        assert!(matches!(
            parse_charlie_report(&r),
            Err(CharlieParseError::UnknownReportId { id: 0x02 })
        ));
    }

    #[test]
    fn test_empty_report() {
        let err = parse_charlie_report(&[]).unwrap_err();
        assert!(matches!(
            err,
            CharlieParseError::TooShort {
                expected: 7,
                got: 0
            }
        ));
    }

    #[cfg(test)]
    mod proptest_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn rudder_within_bounds(raw in 0u16..=4095u16) {
                let state = parse_charlie_report(&super::charlie_report(raw, 0, 0)).unwrap();
                prop_assert!((-1.001..=1.001).contains(&state.axes.rudder),
                    "rudder out of range: {}", state.axes.rudder);
            }

            #[test]
            fn left_brake_within_bounds(raw in 0u16..=4095u16) {
                let state = parse_charlie_report(&super::charlie_report(2048, raw, 0)).unwrap();
                prop_assert!((0.0..=1.0001).contains(&state.axes.left_brake),
                    "left_brake out of range: {}", state.axes.left_brake);
            }

            #[test]
            fn right_brake_within_bounds(raw in 0u16..=4095u16) {
                let state = parse_charlie_report(&super::charlie_report(2048, 0, raw)).unwrap();
                prop_assert!((0.0..=1.0001).contains(&state.axes.right_brake),
                    "right_brake out of range: {}", state.axes.right_brake);
            }

            #[test]
            fn any_valid_report_parses(
                rudder in 0u16..=4095u16,
                left in 0u16..=4095u16,
                right in 0u16..=4095u16,
            ) {
                let r = super::charlie_report(rudder, left, right);
                let result = parse_charlie_report(&r);
                prop_assert!(result.is_ok());
            }

            #[test]
            fn all_axes_finite(
                rudder in 0u16..=u16::MAX,
                left in 0u16..=u16::MAX,
                right in 0u16..=u16::MAX,
            ) {
                let state = parse_charlie_report(&super::charlie_report(rudder, left, right)).unwrap();
                prop_assert!(state.axes.rudder.is_finite(), "rudder not finite");
                prop_assert!(state.axes.left_brake.is_finite(), "left_brake not finite");
                prop_assert!(state.axes.right_brake.is_finite(), "right_brake not finite");
            }
        }
    }
}
