// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Axis normalization and device support for the Logitech G Flight Yoke System.
//!
//! # Confirmed device identifier
//!
//! VID 0x046D (Logitech), PID 0xC259 — confirmed via linux-hardware.org.
//!
//! # Input report layout
//!
//! NOTE: UNVERIFIED — the byte layout below is community-inferred from USB HID
//! captures and public HID descriptor analysis. It has not been confirmed on
//! real hardware. Treat all field offsets as approximate until verified with
//! `lsusb -d 046d:c259 -v` or equivalent hardware capture.
//!
//! The yoke produces 8-byte HID input reports (no report ID prefix). Fields
//! are packed in LSB-first bit order:
//!
//! | Bit range | Field       | Type  | Range    | Notes                              |
//! |-----------|-------------|-------|----------|------------------------------------|
//! | 0-11      | X           | u12   | 0..4095  | Roll/horizontal; center ~2047      |
//! | 12-23     | Y           | u12   | 0..4095  | Pitch/vertical; center ~2047       |
//! | 24-31     | Rz          | u8    | 0..255   | Prop pitch; unipolar               |
//! | 32-39     | Slider      | u8    | 0..255   | Mixture lever; unipolar            |
//! | 40-47     | Slider2     | u8    | 0..255   | Carb heat lever; unipolar          |
//! | 48-59     | Buttons     | u12   | bitmask  | Buttons 1-12 packed LSB-first      |
//! | 60-63     | Hat         | u4    | 0-15     | 0=N, 1=NE, ... 7=NW, 8..15=center |
//!
//! # Axes
//!
//! | Axis    | HID Usage | Physical    | Normalized   |
//! |---------|-----------|-------------|--------------|
//! | X       | 0x30      | Roll        | -1.0..=1.0   |
//! | Y       | 0x31      | Pitch       | -1.0..=1.0   |
//! | Rz      | 0x35      | Prop pitch  | 0.0..=1.0    |
//! | Slider  | 0x36      | Mixture     | 0.0..=1.0    |
//! | Slider2 | 0x37      | Carb heat   | 0.0..=1.0    |
//!
//! The throttle (Z axis, HID usage 0x32) is provided by the bundled
//! G Flight Throttle Quadrant (PID 0xC25A), which enumerates as a separate
//! USB HID device.
//!
//! # Buttons
//!
//! Up to 12 momentary pushbuttons, mapped to bits in a 12-bit button field.
//! Button indexing is 1-based; call [`GFlightYokeButtons::button`].

use thiserror::Error;

/// Hat switch positions for the G Flight Yoke 8-way hat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GFlightYokeHat {
    #[default]
    Center,
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
    NorthWest,
}

impl GFlightYokeHat {
    fn from_nibble(nibble: u8) -> Self {
        match nibble & 0x0F {
            0 => Self::North,
            1 => Self::NorthEast,
            2 => Self::East,
            3 => Self::SouthEast,
            4 => Self::South,
            5 => Self::SouthWest,
            6 => Self::West,
            7 => Self::NorthWest,
            _ => Self::Center, // 8-15
        }
    }
}

/// Parsed axis values from the G Flight Yoke, normalized.
#[derive(Debug, Clone, Default)]
pub struct GFlightYokeAxes {
    /// Roll axis (X / yoke horizontal). −1.0 = full left, 1.0 = full right.
    pub x: f32,
    /// Pitch axis (Y / yoke fore-aft). −1.0 = full forward/up, 1.0 = full back/down.
    pub y: f32,
    /// Prop pitch axis (Rz). 0.0 = minimum, 1.0 = maximum.
    pub rz: f32,
    /// Mixture lever (Slider). 0.0 = minimum, 1.0 = maximum.
    pub slider: f32,
    /// Carb heat lever (Slider2). 0.0 = minimum, 1.0 = maximum.
    pub slider2: f32,
}

/// Parsed buttons from the G Flight Yoke.
#[derive(Debug, Clone, Default)]
pub struct GFlightYokeButtons {
    /// Button bitmask; bit 0 = button 1, bit 11 = button 12. Upper 4 bits unused.
    pub buttons: u16,
    /// Hat switch position.
    pub hat: GFlightYokeHat,
}

impl GFlightYokeButtons {
    /// Returns `true` if the specified button (1-indexed, 1-12) is pressed.
    pub fn button(&self, n: u8) -> bool {
        matches!(n, 1..=12) && (self.buttons >> (n - 1)) & 1 != 0
    }
}

/// Full parsed input state from a G Flight Yoke HID report.
#[derive(Debug, Clone, Default)]
pub struct GFlightYokeInputState {
    pub axes: GFlightYokeAxes,
    pub buttons: GFlightYokeButtons,
}

/// Errors returned by G Flight Yoke report parsing.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GFlightYokeParseError {
    #[error("G Flight Yoke report too short: expected at least {expected} bytes, got {actual}")]
    TooShort { expected: usize, actual: usize },
}

/// Minimum HID input report length for the G Flight Yoke.
pub const G_FLIGHT_YOKE_MIN_REPORT_BYTES: usize = 8;

/// 12-bit axis raw maximum for G Flight Yoke (0..4095).
pub const G_FLIGHT_YOKE_AXIS_MAX: u16 = 4095;

/// Centre value for a 12-bit bipolar axis.
pub const G_FLIGHT_YOKE_AXIS_CENTER: f32 = 2047.5;

/// Normalize a 12-bit centered axis (0..4095) to −1.0..=1.0.
#[inline]
fn normalize_12bit_bipolar(raw: u16) -> f32 {
    ((raw as f32 - 2047.5) / 2047.5).clamp(-1.0, 1.0)
}

/// Normalize an 8-bit unipolar axis (0..255) to 0.0..=1.0.
#[inline]
fn normalize_8bit_unipolar(raw: u8) -> f32 {
    (raw as f32 / 255.0).clamp(0.0, 1.0)
}

/// Parse an 8-byte HID input report from the Logitech G Flight Yoke System.
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
/// Byte 0:         X[7:0]
/// Byte 1[3:0]:    X[11:8]
/// Byte 1[7:4]:    Y[3:0]
/// Byte 2:         Y[11:4]
/// Byte 3:         Rz[7:0]      (prop pitch, 8-bit unipolar)
/// Byte 4:         Slider[7:0]  (mixture, 8-bit unipolar)
/// Byte 5:         Slider2[7:0] (carb heat, 8-bit unipolar)
/// Byte 6:         Buttons[7:0]
/// Byte 7[3:0]:    Buttons[11:8]
/// Byte 7[7:4]:    Hat[3:0]
/// ```
///
/// # Errors
/// Returns [`GFlightYokeParseError::TooShort`] if `data` is shorter than
/// [`G_FLIGHT_YOKE_MIN_REPORT_BYTES`].
pub fn parse_g_flight_yoke(data: &[u8]) -> Result<GFlightYokeInputState, GFlightYokeParseError> {
    if data.len() < G_FLIGHT_YOKE_MIN_REPORT_BYTES {
        return Err(GFlightYokeParseError::TooShort {
            expected: G_FLIGHT_YOKE_MIN_REPORT_BYTES,
            actual: data.len(),
        });
    }

    // X: bits 0-11 (12-bit LE)
    let x = (data[0] as u16) | (((data[1] & 0x0F) as u16) << 8);

    // Y: bits 12-23 (12-bit LE)
    let y = ((data[1] >> 4) as u16) | ((data[2] as u16) << 4);

    // Rz: byte 3 (8-bit unipolar)
    let rz = data[3];

    // Slider: byte 4 (8-bit unipolar)
    let slider = data[4];

    // Slider2: byte 5 (8-bit unipolar)
    let slider2 = data[5];

    // Buttons: bits 48-59 (12 buttons)
    let buttons = (data[6] as u16) | (((data[7] & 0x0F) as u16) << 8);

    // Hat: bits 60-63 (4-bit nibble)
    let hat_raw = data[7] >> 4;

    Ok(GFlightYokeInputState {
        axes: GFlightYokeAxes {
            x: normalize_12bit_bipolar(x),
            y: normalize_12bit_bipolar(y),
            rz: normalize_8bit_unipolar(rz),
            slider: normalize_8bit_unipolar(slider),
            slider2: normalize_8bit_unipolar(slider2),
        },
        buttons: GFlightYokeButtons {
            buttons,
            hat: GFlightYokeHat::from_nibble(hat_raw),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build an 8-byte G Flight Yoke report from logical field values.
    fn build_report(
        x: u16,
        y: u16,
        rz: u8,
        slider: u8,
        slider2: u8,
        buttons: u16,
        hat: u8,
    ) -> [u8; 8] {
        let x = x & 0xFFF;
        let y = y & 0xFFF;
        let buttons = buttons & 0x0FFF;
        let hat = hat & 0x0F;

        let mut data = [0u8; 8];
        // X: bits 0-11
        data[0] = (x & 0xFF) as u8;
        // X[11:8] in lower nibble, Y[3:0] in upper nibble
        data[1] = ((x >> 8) & 0x0F) as u8 | (((y & 0x0F) as u8) << 4);
        // Y[11:4] in all 8 bits
        data[2] = ((y >> 4) & 0xFF) as u8;
        data[3] = rz;
        data[4] = slider;
        data[5] = slider2;
        // Buttons[7:0]
        data[6] = (buttons & 0xFF) as u8;
        // Buttons[11:8] in lower nibble, Hat in upper nibble
        data[7] = ((buttons >> 8) & 0x0F) as u8 | (hat << 4);
        data
    }

    #[test]
    fn test_too_short() {
        assert!(parse_g_flight_yoke(&[0u8; 7]).is_err());
        assert!(parse_g_flight_yoke(&[]).is_err());
    }

    #[test]
    fn test_too_short_error_fields() {
        let err = parse_g_flight_yoke(&[0u8; 3]).unwrap_err();
        assert_eq!(
            err,
            GFlightYokeParseError::TooShort {
                expected: G_FLIGHT_YOKE_MIN_REPORT_BYTES,
                actual: 3
            }
        );
    }

    #[test]
    fn test_centered() {
        let data = build_report(2048, 2048, 128, 128, 128, 0, 8); // hat center
        let state = parse_g_flight_yoke(&data).unwrap();
        assert!(state.axes.x.abs() < 0.01, "x near 0: {}", state.axes.x);
        assert!(state.axes.y.abs() < 0.01, "y near 0: {}", state.axes.y);
        assert_eq!(state.buttons.hat, GFlightYokeHat::Center);
    }

    #[test]
    fn test_x_full_right() {
        let data = build_report(4095, 2048, 128, 0, 0, 0, 8);
        let state = parse_g_flight_yoke(&data).unwrap();
        assert!(state.axes.x > 0.99, "x should be ~1.0: {}", state.axes.x);
    }

    #[test]
    fn test_x_full_left() {
        let data = build_report(0, 2048, 128, 0, 0, 0, 8);
        let state = parse_g_flight_yoke(&data).unwrap();
        assert!(state.axes.x < -0.99, "x should be ~-1.0: {}", state.axes.x);
    }

    #[test]
    fn test_y_full_forward() {
        let data = build_report(2048, 0, 128, 0, 0, 0, 8);
        let state = parse_g_flight_yoke(&data).unwrap();
        assert!(state.axes.y < -0.99, "y should be ~-1.0: {}", state.axes.y);
    }

    #[test]
    fn test_y_full_back() {
        let data = build_report(2048, 4095, 128, 0, 0, 0, 8);
        let state = parse_g_flight_yoke(&data).unwrap();
        assert!(state.axes.y > 0.99, "y should be ~1.0: {}", state.axes.y);
    }

    #[test]
    fn test_rz_max() {
        let data = build_report(2048, 2048, 255, 0, 0, 0, 8);
        let state = parse_g_flight_yoke(&data).unwrap();
        assert!(
            state.axes.rz > 0.999,
            "rz should be ~1.0: {}",
            state.axes.rz
        );
    }

    #[test]
    fn test_rz_min() {
        let data = build_report(2048, 2048, 0, 0, 0, 0, 8);
        let state = parse_g_flight_yoke(&data).unwrap();
        assert!(
            state.axes.rz < 0.001,
            "rz should be ~0.0: {}",
            state.axes.rz
        );
    }

    #[test]
    fn test_slider_max() {
        let data = build_report(2048, 2048, 0, 255, 0, 0, 8);
        let state = parse_g_flight_yoke(&data).unwrap();
        assert!(
            state.axes.slider > 0.999,
            "slider should be ~1.0: {}",
            state.axes.slider
        );
    }

    #[test]
    fn test_slider2_max() {
        let data = build_report(2048, 2048, 0, 0, 255, 0, 8);
        let state = parse_g_flight_yoke(&data).unwrap();
        assert!(
            state.axes.slider2 > 0.999,
            "slider2 should be ~1.0: {}",
            state.axes.slider2
        );
    }

    #[test]
    fn test_hat_north() {
        let data = build_report(2048, 2048, 128, 0, 0, 0, 0);
        let state = parse_g_flight_yoke(&data).unwrap();
        assert_eq!(state.buttons.hat, GFlightYokeHat::North);
    }

    #[test]
    fn test_hat_east() {
        let data = build_report(2048, 2048, 128, 0, 0, 0, 2);
        let state = parse_g_flight_yoke(&data).unwrap();
        assert_eq!(state.buttons.hat, GFlightYokeHat::East);
    }

    #[test]
    fn test_hat_south() {
        let data = build_report(2048, 2048, 128, 0, 0, 0, 4);
        let state = parse_g_flight_yoke(&data).unwrap();
        assert_eq!(state.buttons.hat, GFlightYokeHat::South);
    }

    #[test]
    fn test_hat_northwest() {
        let data = build_report(2048, 2048, 128, 0, 0, 0, 7);
        let state = parse_g_flight_yoke(&data).unwrap();
        assert_eq!(state.buttons.hat, GFlightYokeHat::NorthWest);
    }

    #[test]
    fn test_buttons_individual() {
        for b in 1u8..=12 {
            let mask = 1u16 << (b - 1);
            let data = build_report(2048, 2048, 128, 0, 0, mask, 8);
            let state = parse_g_flight_yoke(&data).unwrap();
            assert!(state.buttons.button(b), "button {} should be pressed", b);
            for other in 1u8..=12 {
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
        let data = build_report(2048, 2048, 128, 0, 0, 0x0FFF, 8);
        let state = parse_g_flight_yoke(&data).unwrap();
        for b in 1u8..=12 {
            assert!(state.buttons.button(b), "button {} should be pressed", b);
        }
    }

    #[test]
    fn test_out_of_range_button_returns_false() {
        let data = build_report(2048, 2048, 128, 0, 0, 0x0FFF, 8);
        let state = parse_g_flight_yoke(&data).unwrap();
        assert!(!state.buttons.button(0));
        assert!(!state.buttons.button(13));
    }

    #[test]
    fn test_axes_in_range_sample_values() {
        for x_raw in [0u16, 1024, 2047, 2048, 3071, 4095] {
            let data = build_report(x_raw, 2048, 128, 0, 0, 0, 8);
            let state = parse_g_flight_yoke(&data).unwrap();
            assert!(
                (-1.0..=1.0).contains(&state.axes.x),
                "x out of range at raw {}: {}",
                x_raw,
                state.axes.x
            );
        }
    }

    #[test]
    fn test_longer_report_accepted() {
        // Reports longer than minimum must not be rejected.
        let data = build_report(2048, 2048, 128, 0, 0, 0, 8);
        let mut longer = data.to_vec();
        longer.extend_from_slice(&[0xFF, 0xFF]);
        assert!(parse_g_flight_yoke(&longer).is_ok());
    }

    #[cfg(test)]
    mod proptest_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn x_axis_always_in_range(x in 0u16..=4095) {
                let data = build_report(x, 2048, 128, 0, 0, 0, 8);
                let state = parse_g_flight_yoke(&data).unwrap();
                prop_assert!((-1.0f32..=1.0).contains(&state.axes.x));
            }

            #[test]
            fn y_axis_always_in_range(y in 0u16..=4095) {
                let data = build_report(2048, y, 128, 0, 0, 0, 8);
                let state = parse_g_flight_yoke(&data).unwrap();
                prop_assert!((-1.0f32..=1.0).contains(&state.axes.y));
            }

            #[test]
            fn rz_always_unipolar(rz in 0u8..=255) {
                let data = build_report(2048, 2048, rz, 0, 0, 0, 8);
                let state = parse_g_flight_yoke(&data).unwrap();
                prop_assert!((0.0f32..=1.0).contains(&state.axes.rz));
            }

            #[test]
            fn slider_always_unipolar(slider in 0u8..=255) {
                let data = build_report(2048, 2048, 0, slider, 0, 0, 8);
                let state = parse_g_flight_yoke(&data).unwrap();
                prop_assert!((0.0f32..=1.0).contains(&state.axes.slider));
            }

            #[test]
            fn slider2_always_unipolar(slider2 in 0u8..=255) {
                let data = build_report(2048, 2048, 0, 0, slider2, 0, 8);
                let state = parse_g_flight_yoke(&data).unwrap();
                prop_assert!((0.0f32..=1.0).contains(&state.axes.slider2));
            }

            #[test]
            fn buttons_roundtrip(buttons in 0u16..=0x0FFFu16) {
                let data = build_report(2048, 2048, 128, 0, 0, buttons, 8);
                let state = parse_g_flight_yoke(&data).unwrap();
                prop_assert_eq!(state.buttons.buttons, buttons);
            }

            #[test]
            fn any_8byte_report_parses(data in proptest::collection::vec(any::<u8>(), 8..20usize)) {
                let result = parse_g_flight_yoke(&data);
                prop_assert!(result.is_ok());
            }

            #[test]
            fn arbitrary_bytes_all_axes_in_range(
                data in proptest::collection::vec(any::<u8>(), 8..20usize),
            ) {
                let state = parse_g_flight_yoke(&data).unwrap();
                prop_assert!(
                    (-1.0f32..=1.0).contains(&state.axes.x),
                    "x out of range: {}",
                    state.axes.x
                );
                prop_assert!(
                    (-1.0f32..=1.0).contains(&state.axes.y),
                    "y out of range: {}",
                    state.axes.y
                );
                prop_assert!(
                    (0.0f32..=1.0).contains(&state.axes.rz),
                    "rz out of range: {}",
                    state.axes.rz
                );
                prop_assert!(
                    (0.0f32..=1.0).contains(&state.axes.slider),
                    "slider out of range: {}",
                    state.axes.slider
                );
                prop_assert!(
                    (0.0f32..=1.0).contains(&state.axes.slider2),
                    "slider2 out of range: {}",
                    state.axes.slider2
                );
            }

            #[test]
            fn out_of_range_buttons_always_false(
                data in proptest::collection::vec(any::<u8>(), 8..20usize),
            ) {
                let state = parse_g_flight_yoke(&data).unwrap();
                prop_assert!(!state.buttons.button(0));
                for b in 13u8..=20 {
                    prop_assert!(
                        !state.buttons.button(b),
                        "button {} out of range should be false",
                        b
                    );
                }
                prop_assert_eq!(
                    state.buttons.buttons & 0xF000,
                    0,
                    "upper bits of button word must be 0"
                );
            }
        }
    }
}
