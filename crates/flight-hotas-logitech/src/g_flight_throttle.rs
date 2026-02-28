// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Axis normalization and device support for the Logitech G Flight Throttle Quadrant.
//!
//! # Confirmed device identifier
//!
//! VID 0x046D (Logitech), PID 0xC25A — confirmed via linux-hardware.org.
//! The throttle quadrant enumerates as a separate USB HID device from the
//! bundled G Flight Yoke System (PID 0xC259).
//!
//! # Input report layout
//!
//! NOTE: UNVERIFIED — the byte layout below is community-inferred from USB HID
//! captures and public HID descriptor analysis. It has not been confirmed on
//! real hardware. Treat all field offsets as approximate until verified with
//! `lsusb -d 046d:c25a -v` or equivalent hardware capture.
//!
//! The throttle quadrant produces 6-byte HID input reports (no report ID prefix).
//! Fields are packed in LSB-first bit order:
//!
//! | Bit range | Field    | Type  | Range    | Notes                           |
//! |-----------|----------|-------|----------|---------------------------------|
//! | 0-11      | Left     | u12   | 0..4095  | Left lever (Z); unipolar        |
//! | 12-23     | Center   | u12   | 0..4095  | Center lever (Rz); unipolar     |
//! | 24-35     | Right    | u12   | 0..4095  | Right lever (Slider); unipolar  |
//! | 36-41     | Buttons  | u6    | bitmask  | Buttons 1-6 packed LSB-first    |
//! | 42-47     | Padding  | —     | —        | Unused bits, always 0           |
//!
//! # Axes
//!
//! | Axis   | HID Usage | Physical       | Normalized   |
//! |--------|-----------|----------------|--------------|
//! | Left   | 0x32      | Left lever     | 0.0..=1.0    |
//! | Center | 0x35      | Center lever   | 0.0..=1.0    |
//! | Right  | 0x36      | Right lever    | 0.0..=1.0    |
//!
//! # Buttons
//!
//! 6 momentary pushbuttons on the quadrant face plate, mapped to bits in a
//! 6-bit button field. Button indexing is 1-based; call
//! [`GFlightThrottleButtons::button`].

use thiserror::Error;

/// Parsed axis values from the G Flight Throttle Quadrant, normalized.
#[derive(Debug, Clone, Default)]
pub struct GFlightThrottleAxes {
    /// Left lever (Z axis). 0.0 = minimum, 1.0 = maximum.
    pub left: f32,
    /// Center lever (Rz axis). 0.0 = minimum, 1.0 = maximum.
    pub center: f32,
    /// Right lever (Slider axis). 0.0 = minimum, 1.0 = maximum.
    pub right: f32,
}

/// Parsed buttons from the G Flight Throttle Quadrant.
#[derive(Debug, Clone, Default)]
pub struct GFlightThrottleButtons {
    /// Button bitmask; bit 0 = button 1, bit 5 = button 6. Upper 10 bits unused.
    pub buttons: u8,
}

impl GFlightThrottleButtons {
    /// Returns `true` if the specified button (1-indexed, 1-6) is pressed.
    pub fn button(&self, n: u8) -> bool {
        matches!(n, 1..=6) && (self.buttons >> (n - 1)) & 1 != 0
    }
}

/// Full parsed input state from a G Flight Throttle Quadrant HID report.
#[derive(Debug, Clone, Default)]
pub struct GFlightThrottleInputState {
    pub axes: GFlightThrottleAxes,
    pub buttons: GFlightThrottleButtons,
}

/// Errors returned by G Flight Throttle Quadrant report parsing.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GFlightThrottleParseError {
    #[error(
        "G Flight Throttle Quadrant report too short: expected at least {expected} bytes, got {actual}"
    )]
    TooShort { expected: usize, actual: usize },
}

/// Minimum HID input report length for the G Flight Throttle Quadrant.
pub const G_FLIGHT_THROTTLE_MIN_REPORT_BYTES: usize = 6;

/// Normalize a 12-bit unipolar axis (0..4095) to 0.0..=1.0.
#[inline]
fn normalize_12bit_unipolar(raw: u16) -> f32 {
    (raw as f32 / 4095.0).clamp(0.0, 1.0)
}

/// Parse a 6-byte HID input report from the Logitech G Flight Throttle Quadrant.
///
/// The report must not include a report ID prefix. If the OS/driver prepends
/// one, strip it before calling this function.
///
/// NOTE: UNVERIFIED — the bit layout below is community-inferred and has not
/// been confirmed on real hardware.
///
/// # Bit layout (LSB-first within each byte, little-endian)
///
/// ```text
/// Byte 0:         Left[7:0]
/// Byte 1[3:0]:    Left[11:8]
/// Byte 1[7:4]:    Center[3:0]
/// Byte 2:         Center[11:4]
/// Byte 3:         Right[7:0]
/// Byte 4[3:0]:    Right[11:8]
/// Byte 4[7:4]:    Buttons[3:0]
/// Byte 5[1:0]:    Buttons[5:4]
/// Byte 5[7:2]:    Padding
/// ```
///
/// # Errors
/// Returns [`GFlightThrottleParseError::TooShort`] if `data` is shorter than
/// [`G_FLIGHT_THROTTLE_MIN_REPORT_BYTES`].
pub fn parse_g_flight_throttle(
    data: &[u8],
) -> Result<GFlightThrottleInputState, GFlightThrottleParseError> {
    if data.len() < G_FLIGHT_THROTTLE_MIN_REPORT_BYTES {
        return Err(GFlightThrottleParseError::TooShort {
            expected: G_FLIGHT_THROTTLE_MIN_REPORT_BYTES,
            actual: data.len(),
        });
    }

    // Left lever: bits 0-11 (12-bit LE)
    let left = (data[0] as u16) | (((data[1] & 0x0F) as u16) << 8);

    // Center lever: bits 12-23 (12-bit LE)
    let center = ((data[1] >> 4) as u16) | ((data[2] as u16) << 4);

    // Right lever: bits 24-35 (12-bit LE)
    let right = (data[3] as u16) | (((data[4] & 0x0F) as u16) << 8);

    // Buttons: bits 36-41 (6 buttons)
    let btn_low = (data[4] >> 4) & 0x0F; // buttons 1-4
    let btn_high = data[5] & 0x03; // buttons 5-6
    let buttons = btn_low | (btn_high << 4);

    Ok(GFlightThrottleInputState {
        axes: GFlightThrottleAxes {
            left: normalize_12bit_unipolar(left),
            center: normalize_12bit_unipolar(center),
            right: normalize_12bit_unipolar(right),
        },
        buttons: GFlightThrottleButtons { buttons },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a 6-byte G Flight Throttle Quadrant report from logical field values.
    fn build_report(left: u16, center: u16, right: u16, buttons: u8) -> [u8; 6] {
        let left = left & 0xFFF;
        let center = center & 0xFFF;
        let right = right & 0xFFF;
        let buttons = buttons & 0x3F;

        let mut data = [0u8; 6];
        // Left[7:0]
        data[0] = (left & 0xFF) as u8;
        // Left[11:8] in lower nibble, Center[3:0] in upper nibble
        data[1] = ((left >> 8) & 0x0F) as u8 | (((center & 0x0F) as u8) << 4);
        // Center[11:4]
        data[2] = ((center >> 4) & 0xFF) as u8;
        // Right[7:0]
        data[3] = (right & 0xFF) as u8;
        // Right[11:8] in lower nibble, Buttons[3:0] in upper nibble
        data[4] = ((right >> 8) & 0x0F) as u8 | ((buttons & 0x0F) << 4);
        // Buttons[5:4] in bits 1:0, padding in bits 7:2
        data[5] = (buttons >> 4) & 0x03;
        data
    }

    #[test]
    fn test_too_short() {
        assert!(parse_g_flight_throttle(&[0u8; 5]).is_err());
        assert!(parse_g_flight_throttle(&[]).is_err());
    }

    #[test]
    fn test_too_short_error_fields() {
        let err = parse_g_flight_throttle(&[0u8; 2]).unwrap_err();
        assert_eq!(
            err,
            GFlightThrottleParseError::TooShort {
                expected: G_FLIGHT_THROTTLE_MIN_REPORT_BYTES,
                actual: 2
            }
        );
    }

    #[test]
    fn test_all_levers_min() {
        let data = build_report(0, 0, 0, 0);
        let state = parse_g_flight_throttle(&data).unwrap();
        assert!(
            state.axes.left < 0.001,
            "left should be 0.0: {}",
            state.axes.left
        );
        assert!(
            state.axes.center < 0.001,
            "center should be 0.0: {}",
            state.axes.center
        );
        assert!(
            state.axes.right < 0.001,
            "right should be 0.0: {}",
            state.axes.right
        );
    }

    #[test]
    fn test_all_levers_max() {
        let data = build_report(4095, 4095, 4095, 0);
        let state = parse_g_flight_throttle(&data).unwrap();
        assert!(
            state.axes.left > 0.999,
            "left should be 1.0: {}",
            state.axes.left
        );
        assert!(
            state.axes.center > 0.999,
            "center should be 1.0: {}",
            state.axes.center
        );
        assert!(
            state.axes.right > 0.999,
            "right should be 1.0: {}",
            state.axes.right
        );
    }

    #[test]
    fn test_left_lever_midpoint() {
        let data = build_report(2048, 0, 0, 0);
        let state = parse_g_flight_throttle(&data).unwrap();
        let expected = 2048.0f32 / 4095.0;
        assert!(
            (state.axes.left - expected).abs() < 1e-4,
            "left midpoint: {}",
            state.axes.left
        );
    }

    #[test]
    fn test_levers_independent() {
        let data = build_report(4095, 0, 2048, 0);
        let state = parse_g_flight_throttle(&data).unwrap();
        assert!(state.axes.left > 0.999, "left: {}", state.axes.left);
        assert!(state.axes.center < 0.001, "center: {}", state.axes.center);
        let expected_right = 2048.0f32 / 4095.0;
        assert!(
            (state.axes.right - expected_right).abs() < 1e-4,
            "right: {}",
            state.axes.right
        );
    }

    #[test]
    fn test_buttons_individual() {
        for b in 1u8..=6 {
            let mask = 1u8 << (b - 1);
            let data = build_report(0, 0, 0, mask);
            let state = parse_g_flight_throttle(&data).unwrap();
            assert!(state.buttons.button(b), "button {} should be pressed", b);
            for other in 1u8..=6 {
                if other != b {
                    assert!(
                        !state.buttons.button(other),
                        "button {} should NOT be pressed when {} is",
                        other,
                        b
                    );
                }
            }
        }
    }

    #[test]
    fn test_all_buttons_pressed() {
        let data = build_report(0, 0, 0, 0x3F);
        let state = parse_g_flight_throttle(&data).unwrap();
        for b in 1u8..=6 {
            assert!(state.buttons.button(b), "button {} should be pressed", b);
        }
    }

    #[test]
    fn test_out_of_range_button_returns_false() {
        let data = build_report(0, 0, 0, 0x3F);
        let state = parse_g_flight_throttle(&data).unwrap();
        assert!(!state.buttons.button(0));
        assert!(!state.buttons.button(7));
    }

    #[test]
    fn test_longer_report_accepted() {
        let data = build_report(4095, 0, 2048, 0x3F);
        let mut longer = data.to_vec();
        longer.extend_from_slice(&[0xFF, 0xFF]);
        assert!(parse_g_flight_throttle(&longer).is_ok());
    }

    #[test]
    fn test_axes_roundtrip_sample_values() {
        for raw in [0u16, 1024, 2048, 3071, 4095] {
            let data = build_report(raw, raw, raw, 0);
            let state = parse_g_flight_throttle(&data).unwrap();
            assert!(
                (0.0..=1.0).contains(&state.axes.left),
                "left out of range at raw {}: {}",
                raw,
                state.axes.left
            );
            assert!(
                (0.0..=1.0).contains(&state.axes.center),
                "center out of range at raw {}: {}",
                raw,
                state.axes.center
            );
            assert!(
                (0.0..=1.0).contains(&state.axes.right),
                "right out of range at raw {}: {}",
                raw,
                state.axes.right
            );
        }
    }

    #[cfg(test)]
    mod proptest_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn left_always_unipolar(left in 0u16..=4095) {
                let data = build_report(left, 0, 0, 0);
                let state = parse_g_flight_throttle(&data).unwrap();
                prop_assert!((0.0f32..=1.0).contains(&state.axes.left));
            }

            #[test]
            fn center_always_unipolar(center in 0u16..=4095) {
                let data = build_report(0, center, 0, 0);
                let state = parse_g_flight_throttle(&data).unwrap();
                prop_assert!((0.0f32..=1.0).contains(&state.axes.center));
            }

            #[test]
            fn right_always_unipolar(right in 0u16..=4095) {
                let data = build_report(0, 0, right, 0);
                let state = parse_g_flight_throttle(&data).unwrap();
                prop_assert!((0.0f32..=1.0).contains(&state.axes.right));
            }

            #[test]
            fn buttons_roundtrip(buttons in 0u8..=0x3Fu8) {
                let data = build_report(0, 0, 0, buttons);
                let state = parse_g_flight_throttle(&data).unwrap();
                prop_assert_eq!(state.buttons.buttons, buttons);
            }

            #[test]
            fn any_6byte_report_parses(data in proptest::collection::vec(any::<u8>(), 6..20usize)) {
                let result = parse_g_flight_throttle(&data);
                prop_assert!(result.is_ok());
            }

            #[test]
            fn arbitrary_bytes_all_axes_in_range(
                data in proptest::collection::vec(any::<u8>(), 6..20usize),
            ) {
                let state = parse_g_flight_throttle(&data).unwrap();
                prop_assert!(
                    (0.0f32..=1.0).contains(&state.axes.left),
                    "left out of range: {}",
                    state.axes.left
                );
                prop_assert!(
                    (0.0f32..=1.0).contains(&state.axes.center),
                    "center out of range: {}",
                    state.axes.center
                );
                prop_assert!(
                    (0.0f32..=1.0).contains(&state.axes.right),
                    "right out of range: {}",
                    state.axes.right
                );
            }
        }
    }
}
