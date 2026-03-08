// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the Saitek Pro Flight Rudder Pedals.
//!
//! # Device identifiers
//!
//! - Saitek: VID 0x06A3, PID 0x0763
//! - Logitech (rebrand): VID 0x046D, PID 0xC264
//!
//! # Input report layout (UNVERIFIED — community reverse-engineered)
//!
//! The rudder pedals produce a 5-byte HID input report (no report ID prefix).
//! All three axes are 10-bit, packed LSB-first:
//!
//! | Bit range | Field       | Type | Range   | Notes                            |
//! |-----------|-------------|------|---------|----------------------------------|
//! | 0-9       | Rudder      | u10  | 0..1023 | Bipolar; center ~511             |
//! | 10-19     | Left brake  | u10  | 0..1023 | Unipolar; 0=released, 1023=full  |
//! | 20-29     | Right brake | u10  | 0..1023 | Unipolar; 0=released, 1023=full  |
//! | 30-39     | Padding     | —    | —       | Unused bits                      |
//!
//! # Axes
//!
//! | Axis        | Physical    | Normalized   |
//! |-------------|-------------|--------------|
//! | Rudder      | Yaw pedals  | -1.0..=1.0   |
//! | Left brake  | Left toe    | 0.0..=1.0    |
//! | Right brake | Right toe   | 0.0..=1.0    |

use thiserror::Error;

/// Minimum HID input report length for the rudder pedals.
pub const RUDDER_PEDALS_MIN_REPORT_BYTES: usize = 5;

/// Parsed axis values from the rudder pedals, normalized.
#[derive(Debug, Clone, Default)]
pub struct RudderPedalsAxes {
    /// Rudder axis (yaw). −1.0 = full left, 1.0 = full right.
    pub rudder: f32,
    /// Left toe brake. 0.0 = released, 1.0 = fully depressed.
    pub left_brake: f32,
    /// Right toe brake. 0.0 = released, 1.0 = fully depressed.
    pub right_brake: f32,
}

/// Full parsed input state from a rudder pedals HID report.
#[derive(Debug, Clone, Default)]
pub struct RudderPedalsInputState {
    pub axes: RudderPedalsAxes,
}

/// Errors returned by rudder pedals report parsing.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RudderPedalsParseError {
    #[error("Rudder pedals report too short: expected at least {expected} bytes, got {actual}")]
    TooShort { expected: usize, actual: usize },
}

/// Normalize a 10-bit bipolar axis (0..1023) to −1.0..=1.0.
#[inline]
fn normalize_10bit_bipolar(raw: u16) -> f32 {
    ((raw as f32 - 511.5) / 511.5).clamp(-1.0, 1.0)
}

/// Normalize a 10-bit unipolar axis (0..1023) to 0.0..=1.0.
#[inline]
fn normalize_10bit_unipolar(raw: u16) -> f32 {
    (raw as f32 / 1023.0).clamp(0.0, 1.0)
}

/// Parse a 5-byte HID input report from the Saitek Pro Flight Rudder Pedals.
///
/// # Bit layout (LSB-first within each byte, little-endian)
///
/// ```text
/// Byte 0:    Rudder[7:0]
/// Byte 1:    Rudder[9:8] in bits 1:0, LeftBrake[5:0] in bits 7:2
/// Byte 2:    LeftBrake[9:6] in bits 3:0, RightBrake[3:0] in bits 7:4
/// Byte 3:    RightBrake[9:4] in bits 5:0, padding in bits 7:6
/// Byte 4:    Padding
/// ```
///
/// # Errors
/// Returns [`RudderPedalsParseError::TooShort`] if `data` is shorter than
/// [`RUDDER_PEDALS_MIN_REPORT_BYTES`].
pub fn parse_rudder_pedals(data: &[u8]) -> Result<RudderPedalsInputState, RudderPedalsParseError> {
    if data.len() < RUDDER_PEDALS_MIN_REPORT_BYTES {
        return Err(RudderPedalsParseError::TooShort {
            expected: RUDDER_PEDALS_MIN_REPORT_BYTES,
            actual: data.len(),
        });
    }

    // Rudder: bits 0-9 (10-bit bipolar)
    let rudder = (data[0] as u16) | (((data[1] & 0x03) as u16) << 8);

    // Left brake: bits 10-19 (10-bit unipolar)
    let left_brake = ((data[1] >> 2) as u16) | (((data[2] & 0x0F) as u16) << 6);

    // Right brake: bits 20-29 (10-bit unipolar)
    let right_brake = ((data[2] >> 4) as u16) | (((data[3] & 0x3F) as u16) << 4);

    Ok(RudderPedalsInputState {
        axes: RudderPedalsAxes {
            rudder: normalize_10bit_bipolar(rudder),
            left_brake: normalize_10bit_unipolar(left_brake),
            right_brake: normalize_10bit_unipolar(right_brake),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a 5-byte rudder pedals report from logical field values.
    fn build_report(rudder: u16, left_brake: u16, right_brake: u16) -> [u8; 5] {
        let rudder = rudder & 0x3FF;
        let lb = left_brake & 0x3FF;
        let rb = right_brake & 0x3FF;

        let mut data = [0u8; 5];
        // Rudder[7:0]
        data[0] = rudder as u8;
        // Rudder[9:8] in bits 1:0, LeftBrake[5:0] in bits 7:2
        data[1] = ((rudder >> 8) as u8 & 0x03) | (((lb & 0x3F) as u8) << 2);
        // LeftBrake[9:6] in bits 3:0, RightBrake[3:0] in bits 7:4
        data[2] = ((lb >> 6) as u8 & 0x0F) | (((rb & 0x0F) as u8) << 4);
        // RightBrake[9:4] in bits 5:0
        data[3] = ((rb >> 4) as u8) & 0x3F;
        // Padding
        data[4] = 0;
        data
    }

    #[test]
    fn test_too_short() {
        assert!(parse_rudder_pedals(&[0u8; 4]).is_err());
        assert!(parse_rudder_pedals(&[]).is_err());
    }

    #[test]
    fn test_too_short_error_fields() {
        let err = parse_rudder_pedals(&[0u8; 2]).unwrap_err();
        assert_eq!(
            err,
            RudderPedalsParseError::TooShort {
                expected: RUDDER_PEDALS_MIN_REPORT_BYTES,
                actual: 2
            }
        );
    }

    #[test]
    fn test_centered_rudder_brakes_released() {
        let data = build_report(512, 0, 0);
        let state = parse_rudder_pedals(&data).unwrap();
        assert!(
            state.axes.rudder.abs() < 0.01,
            "rudder near 0: {}",
            state.axes.rudder
        );
        assert!(
            state.axes.left_brake < 0.001,
            "left brake released: {}",
            state.axes.left_brake
        );
        assert!(
            state.axes.right_brake < 0.001,
            "right brake released: {}",
            state.axes.right_brake
        );
    }

    #[test]
    fn test_rudder_full_left() {
        let data = build_report(0, 0, 0);
        let state = parse_rudder_pedals(&data).unwrap();
        assert!(
            state.axes.rudder < -0.99,
            "rudder full left: {}",
            state.axes.rudder
        );
    }

    #[test]
    fn test_rudder_full_right() {
        let data = build_report(1023, 0, 0);
        let state = parse_rudder_pedals(&data).unwrap();
        assert!(
            state.axes.rudder > 0.99,
            "rudder full right: {}",
            state.axes.rudder
        );
    }

    #[test]
    fn test_left_brake_full() {
        let data = build_report(512, 1023, 0);
        let state = parse_rudder_pedals(&data).unwrap();
        assert!(
            state.axes.left_brake > 0.999,
            "left brake full: {}",
            state.axes.left_brake
        );
    }

    #[test]
    fn test_right_brake_full() {
        let data = build_report(512, 0, 1023);
        let state = parse_rudder_pedals(&data).unwrap();
        assert!(
            state.axes.right_brake > 0.999,
            "right brake full: {}",
            state.axes.right_brake
        );
    }

    #[test]
    fn test_brakes_independent() {
        let data = build_report(512, 1023, 0);
        let state = parse_rudder_pedals(&data).unwrap();
        assert!(state.axes.left_brake > 0.999);
        assert!(state.axes.right_brake < 0.001);
    }

    #[test]
    fn test_both_brakes_full() {
        let data = build_report(512, 1023, 1023);
        let state = parse_rudder_pedals(&data).unwrap();
        assert!(state.axes.left_brake > 0.999);
        assert!(state.axes.right_brake > 0.999);
    }

    #[test]
    fn test_left_brake_half() {
        let data = build_report(512, 512, 0);
        let state = parse_rudder_pedals(&data).unwrap();
        let expected = 512.0f32 / 1023.0;
        assert!(
            (state.axes.left_brake - expected).abs() < 1e-3,
            "left brake half: {}",
            state.axes.left_brake
        );
    }

    #[test]
    fn test_longer_report_accepted() {
        let data = build_report(512, 256, 256);
        let mut longer = data.to_vec();
        longer.extend_from_slice(&[0xFF, 0xFF]);
        assert!(parse_rudder_pedals(&longer).is_ok());
    }

    #[test]
    fn test_axes_range_samples() {
        for raw in [0u16, 256, 511, 512, 767, 1023] {
            let data = build_report(raw, raw, raw);
            let state = parse_rudder_pedals(&data).unwrap();
            assert!(
                (-1.0..=1.0).contains(&state.axes.rudder),
                "rudder out of range at raw {}: {}",
                raw,
                state.axes.rudder
            );
            assert!(
                (0.0..=1.0).contains(&state.axes.left_brake),
                "left_brake out of range at raw {}: {}",
                raw,
                state.axes.left_brake
            );
            assert!(
                (0.0..=1.0).contains(&state.axes.right_brake),
                "right_brake out of range at raw {}: {}",
                raw,
                state.axes.right_brake
            );
        }
    }

    #[cfg(test)]
    mod proptest_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn rudder_always_in_range(rudder in 0u16..=1023) {
                let data = build_report(rudder, 0, 0);
                let state = parse_rudder_pedals(&data).unwrap();
                prop_assert!((-1.0f32..=1.0).contains(&state.axes.rudder));
            }

            #[test]
            fn left_brake_always_unipolar(lb in 0u16..=1023) {
                let data = build_report(512, lb, 0);
                let state = parse_rudder_pedals(&data).unwrap();
                prop_assert!((0.0f32..=1.0).contains(&state.axes.left_brake));
            }

            #[test]
            fn right_brake_always_unipolar(rb in 0u16..=1023) {
                let data = build_report(512, 0, rb);
                let state = parse_rudder_pedals(&data).unwrap();
                prop_assert!((0.0f32..=1.0).contains(&state.axes.right_brake));
            }

            #[test]
            fn any_5byte_report_parses(data in proptest::collection::vec(any::<u8>(), 5..10usize)) {
                let result = parse_rudder_pedals(&data);
                prop_assert!(result.is_ok());
            }

            #[test]
            fn arbitrary_bytes_all_axes_in_range(
                data in proptest::collection::vec(any::<u8>(), 5..10usize),
            ) {
                let state = parse_rudder_pedals(&data).unwrap();
                prop_assert!(
                    (-1.0f32..=1.0).contains(&state.axes.rudder),
                    "rudder out of range: {}",
                    state.axes.rudder
                );
                prop_assert!(
                    (0.0f32..=1.0).contains(&state.axes.left_brake),
                    "left_brake out of range: {}",
                    state.axes.left_brake
                );
                prop_assert!(
                    (0.0f32..=1.0).contains(&state.axes.right_brake),
                    "right_brake out of range: {}",
                    state.axes.right_brake
                );
            }

            #[test]
            fn short_report_returns_error(len in 0usize..RUDDER_PEDALS_MIN_REPORT_BYTES) {
                let data = vec![0u8; len];
                prop_assert!(parse_rudder_pedals(&data).is_err());
            }
        }
    }
}
