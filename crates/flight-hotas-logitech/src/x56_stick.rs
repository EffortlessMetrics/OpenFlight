// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the X56 Rhino stick (Mad Catz / Saitek / Logitech).
//!
//! # Device identifiers
//!
//! - Mad Catz era: VID 0x0738, PID 0x2221
//! - Saitek era: VID 0x06A3, PID 0x2215 (X55 Stick, shared layout)
//!
//! # Input report layout (UNVERIFIED — community reverse-engineered)
//!
//! The X56 stick produces a 13-byte HID input report (no report ID prefix).
//! Fields are packed in LSB-first bit order:
//!
//! | Bit range  | Field   | Type | Range   | Notes                            |
//! |------------|---------|------|---------|----------------------------------|
//! | 0-11       | X       | u12  | 0..4095 | Roll; center ~2047               |
//! | 12-23      | Y       | u12  | 0..4095 | Pitch; center ~2047              |
//! | 24-35      | Rz      | u12  | 0..4095 | Twist/yaw; center ~2047          |
//! | 36-43      | Rx      | u8   | 0..255  | Mini-stick X; center ~127        |
//! | 44-51      | Ry      | u8   | 0..255  | Mini-stick Y; center ~127        |
//! | 52-75      | Buttons | u24  | bitmask | 24 buttons, LSB-first            |
//! | 76-79      | Hat1    | u4   | 0-15    | Main hat; 0=N, …, 7=NW          |
//! | 80-83      | Hat2    | u4   | 0-15    | Secondary hat                    |
//! | 84-95      | Padding | —    | —       | Unused bits                      |

use thiserror::Error;

/// Minimum HID input report length for the X56 stick.
pub const X56_STICK_MIN_REPORT_BYTES: usize = 13;

/// Hat switch positions for the X56 8-way hat switches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum X56Hat {
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

impl X56Hat {
    /// Decode hat position from a 4-bit nibble (public for cross-module use).
    pub fn from_nibble(nibble: u8) -> Self {
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

/// Parsed axis values from the X56 stick, normalized.
#[derive(Debug, Clone, Default)]
pub struct X56StickAxes {
    /// Roll axis (X). −1.0 = full left, 1.0 = full right.
    pub x: f32,
    /// Pitch axis (Y). −1.0 = full forward, 1.0 = full back.
    pub y: f32,
    /// Twist axis (Rz). −1.0 = full left twist, 1.0 = full right twist.
    pub rz: f32,
    /// Mini-stick X (Rx). −1.0 = full left, 1.0 = full right.
    pub rx: f32,
    /// Mini-stick Y (Ry). −1.0 = full forward, 1.0 = full back.
    pub ry: f32,
}

/// Parsed button/hat state from the X56 stick.
#[derive(Debug, Clone, Default)]
pub struct X56StickButtons {
    /// Button bitmask; bit 0 = button 1, bit 23 = button 24.
    pub buttons: u32,
    /// Primary hat switch position.
    pub hat1: X56Hat,
    /// Secondary hat switch position.
    pub hat2: X56Hat,
}

impl X56StickButtons {
    /// Returns `true` if the specified button (1-indexed, 1–24) is pressed.
    pub fn button(&self, n: u8) -> bool {
        matches!(n, 1..=24) && (self.buttons >> (n - 1)) & 1 != 0
    }
}

/// Full parsed input state from an X56 stick HID report.
#[derive(Debug, Clone, Default)]
pub struct X56StickInputState {
    pub axes: X56StickAxes,
    pub buttons: X56StickButtons,
}

/// Errors returned by X56 stick report parsing.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum X56StickParseError {
    #[error("X56 stick report too short: expected at least {expected} bytes, got {actual}")]
    TooShort { expected: usize, actual: usize },
}

/// Normalize a 12-bit bipolar axis (0..4095) to −1.0..=1.0.
#[inline]
fn normalize_12bit_bipolar(raw: u16) -> f32 {
    ((raw as f32 - 2047.5) / 2047.5).clamp(-1.0, 1.0)
}

/// Normalize an 8-bit bipolar axis (0..255) to −1.0..=1.0.
#[inline]
fn normalize_8bit_bipolar(raw: u8) -> f32 {
    ((raw as f32 - 127.5) / 127.5).clamp(-1.0, 1.0)
}

/// Parse a 13-byte HID input report from the X56 Rhino stick.
///
/// # Bit layout (LSB-first within each byte, little-endian)
///
/// ```text
/// Bytes 0-2:  X[11:0] (lo) and Y[11:0] (hi)
/// Bytes 3-5:  Rz[11:0] (lo) — upper nibble of byte 5 starts Rx
/// Byte  4-5:  (shared with Rz)
/// Byte  5[7:4] + Byte 6[3:0]: Rx[7:0]
/// Byte  6[7:4] + Byte 7[3:0]: Ry[7:0]
/// ```
///
/// Actually using simplified layout:
/// ```text
/// Byte  0:    X[7:0]
/// Byte  1:    X[11:8] in lower nibble, Y[3:0] in upper nibble
/// Byte  2:    Y[11:4]
/// Byte  3:    Rz[7:0]
/// Byte  4:    Rz[11:8] in lower nibble, upper nibble unused for Rz
/// Byte  5:    Rx[7:0]
/// Byte  6:    Ry[7:0]
/// Byte  7:    Buttons[7:0]
/// Byte  8:    Buttons[15:8]
/// Byte  9:    Buttons[23:16]
/// Byte 10:    Hat1[3:0] in lower nibble, Hat2[3:0] in upper nibble
/// Bytes 11-12: Padding
/// ```
///
/// # Errors
/// Returns [`X56StickParseError::TooShort`] if `data` is shorter than
/// [`X56_STICK_MIN_REPORT_BYTES`].
pub fn parse_x56_stick(data: &[u8]) -> Result<X56StickInputState, X56StickParseError> {
    if data.len() < X56_STICK_MIN_REPORT_BYTES {
        return Err(X56StickParseError::TooShort {
            expected: X56_STICK_MIN_REPORT_BYTES,
            actual: data.len(),
        });
    }

    // X: bits 0-11 (12-bit LE)
    let x = (data[0] as u16) | (((data[1] & 0x0F) as u16) << 8);

    // Y: bits 12-23 (12-bit LE)
    let y = ((data[1] >> 4) as u16) | ((data[2] as u16) << 4);

    // Rz: bytes 3-4 (12-bit LE)
    let rz = (data[3] as u16) | (((data[4] & 0x0F) as u16) << 8);

    // Rx: byte 5 (8-bit bipolar)
    let rx = data[5];

    // Ry: byte 6 (8-bit bipolar)
    let ry = data[6];

    // Buttons: bytes 7-9 (24 buttons)
    let buttons = (data[7] as u32) | ((data[8] as u32) << 8) | ((data[9] as u32) << 16);

    // Hats: byte 10
    let hat1_raw = data[10] & 0x0F;
    let hat2_raw = data[10] >> 4;

    Ok(X56StickInputState {
        axes: X56StickAxes {
            x: normalize_12bit_bipolar(x),
            y: normalize_12bit_bipolar(y),
            rz: normalize_12bit_bipolar(rz),
            rx: normalize_8bit_bipolar(rx),
            ry: normalize_8bit_bipolar(ry),
        },
        buttons: X56StickButtons {
            buttons,
            hat1: X56Hat::from_nibble(hat1_raw),
            hat2: X56Hat::from_nibble(hat2_raw),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a 13-byte X56 stick report from logical field values.
    fn build_report(
        x: u16,
        y: u16,
        rz: u16,
        rx: u8,
        ry: u8,
        buttons: u32,
        hat1: u8,
        hat2: u8,
    ) -> [u8; 13] {
        let x = x & 0xFFF;
        let y = y & 0xFFF;
        let rz = rz & 0xFFF;
        let buttons = buttons & 0x00FF_FFFF;
        let hat1 = hat1 & 0x0F;
        let hat2 = hat2 & 0x0F;

        let mut data = [0u8; 13];
        data[0] = x as u8;
        data[1] = ((x >> 8) as u8 & 0x0F) | (((y & 0x0F) as u8) << 4);
        data[2] = (y >> 4) as u8;
        data[3] = rz as u8;
        data[4] = ((rz >> 8) as u8) & 0x0F;
        data[5] = rx;
        data[6] = ry;
        data[7] = buttons as u8;
        data[8] = (buttons >> 8) as u8;
        data[9] = (buttons >> 16) as u8;
        data[10] = hat1 | (hat2 << 4);
        data[11] = 0;
        data[12] = 0;
        data
    }

    #[test]
    fn test_too_short() {
        assert!(parse_x56_stick(&[0u8; 12]).is_err());
        assert!(parse_x56_stick(&[]).is_err());
    }

    #[test]
    fn test_too_short_error_fields() {
        let err = parse_x56_stick(&[0u8; 5]).unwrap_err();
        assert_eq!(
            err,
            X56StickParseError::TooShort {
                expected: X56_STICK_MIN_REPORT_BYTES,
                actual: 5
            }
        );
    }

    #[test]
    fn test_centered() {
        let data = build_report(2048, 2048, 2048, 128, 128, 0, 8, 8);
        let state = parse_x56_stick(&data).unwrap();
        assert!(state.axes.x.abs() < 0.01, "x near 0: {}", state.axes.x);
        assert!(state.axes.y.abs() < 0.01, "y near 0: {}", state.axes.y);
        assert!(state.axes.rz.abs() < 0.01, "rz near 0: {}", state.axes.rz);
        assert!(state.axes.rx.abs() < 0.05, "rx near 0: {}", state.axes.rx);
        assert!(state.axes.ry.abs() < 0.05, "ry near 0: {}", state.axes.ry);
        assert_eq!(state.buttons.hat1, X56Hat::Center);
        assert_eq!(state.buttons.hat2, X56Hat::Center);
    }

    #[test]
    fn test_x_full_right() {
        let data = build_report(4095, 2048, 2048, 128, 128, 0, 8, 8);
        let state = parse_x56_stick(&data).unwrap();
        assert!(state.axes.x > 0.999, "x should be ~1.0: {}", state.axes.x);
    }

    #[test]
    fn test_x_full_left() {
        let data = build_report(0, 2048, 2048, 128, 128, 0, 8, 8);
        let state = parse_x56_stick(&data).unwrap();
        assert!(state.axes.x < -0.999, "x should be ~-1.0: {}", state.axes.x);
    }

    #[test]
    fn test_y_full_forward() {
        let data = build_report(2048, 0, 2048, 128, 128, 0, 8, 8);
        let state = parse_x56_stick(&data).unwrap();
        assert!(state.axes.y < -0.999, "y should be ~-1.0: {}", state.axes.y);
    }

    #[test]
    fn test_y_full_back() {
        let data = build_report(2048, 4095, 2048, 128, 128, 0, 8, 8);
        let state = parse_x56_stick(&data).unwrap();
        assert!(state.axes.y > 0.999, "y should be ~1.0: {}", state.axes.y);
    }

    #[test]
    fn test_rz_full_right() {
        let data = build_report(2048, 2048, 4095, 128, 128, 0, 8, 8);
        let state = parse_x56_stick(&data).unwrap();
        assert!(
            state.axes.rz > 0.999,
            "rz should be ~1.0: {}",
            state.axes.rz
        );
    }

    #[test]
    fn test_rz_full_left() {
        let data = build_report(2048, 2048, 0, 128, 128, 0, 8, 8);
        let state = parse_x56_stick(&data).unwrap();
        assert!(
            state.axes.rz < -0.999,
            "rz should be ~-1.0: {}",
            state.axes.rz
        );
    }

    #[test]
    fn test_rx_full_right() {
        let data = build_report(2048, 2048, 2048, 255, 128, 0, 8, 8);
        let state = parse_x56_stick(&data).unwrap();
        assert!(state.axes.rx > 0.99, "rx should be ~1.0: {}", state.axes.rx);
    }

    #[test]
    fn test_ry_full_back() {
        let data = build_report(2048, 2048, 2048, 128, 255, 0, 8, 8);
        let state = parse_x56_stick(&data).unwrap();
        assert!(state.axes.ry > 0.99, "ry should be ~1.0: {}", state.axes.ry);
    }

    #[test]
    fn test_hat1_north() {
        let data = build_report(2048, 2048, 2048, 128, 128, 0, 0, 8);
        let state = parse_x56_stick(&data).unwrap();
        assert_eq!(state.buttons.hat1, X56Hat::North);
    }

    #[test]
    fn test_hat1_south() {
        let data = build_report(2048, 2048, 2048, 128, 128, 0, 4, 8);
        let state = parse_x56_stick(&data).unwrap();
        assert_eq!(state.buttons.hat1, X56Hat::South);
    }

    #[test]
    fn test_hat2_east() {
        let data = build_report(2048, 2048, 2048, 128, 128, 0, 8, 2);
        let state = parse_x56_stick(&data).unwrap();
        assert_eq!(state.buttons.hat2, X56Hat::East);
    }

    #[test]
    fn test_buttons_individual() {
        for b in 1u8..=24 {
            let mask = 1u32 << (b - 1);
            let data = build_report(2048, 2048, 2048, 128, 128, mask, 8, 8);
            let state = parse_x56_stick(&data).unwrap();
            assert!(state.buttons.button(b), "button {} should be pressed", b);
            // Spot check neighbors
            if b > 1 {
                assert!(
                    !state.buttons.button(b - 1),
                    "button {} should NOT be pressed when {} is",
                    b - 1,
                    b
                );
            }
        }
    }

    #[test]
    fn test_all_buttons_pressed() {
        let data = build_report(2048, 2048, 2048, 128, 128, 0x00FF_FFFF, 8, 8);
        let state = parse_x56_stick(&data).unwrap();
        for b in 1u8..=24 {
            assert!(state.buttons.button(b), "button {} should be pressed", b);
        }
    }

    #[test]
    fn test_out_of_range_button_returns_false() {
        let data = build_report(2048, 2048, 2048, 128, 128, 0x00FF_FFFF, 8, 8);
        let state = parse_x56_stick(&data).unwrap();
        assert!(!state.buttons.button(0));
        assert!(!state.buttons.button(25));
    }

    #[test]
    fn test_longer_report_accepted() {
        let data = build_report(2048, 2048, 2048, 128, 128, 0, 8, 8);
        let mut longer = data.to_vec();
        longer.extend_from_slice(&[0xFF, 0xFF]);
        assert!(parse_x56_stick(&longer).is_ok());
    }

    #[cfg(test)]
    mod proptest_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn x_axis_always_in_range(x in 0u16..=4095) {
                let data = build_report(x, 2048, 2048, 128, 128, 0, 8, 8);
                let state = parse_x56_stick(&data).unwrap();
                prop_assert!((-1.0f32..=1.0).contains(&state.axes.x));
            }

            #[test]
            fn y_axis_always_in_range(y in 0u16..=4095) {
                let data = build_report(2048, y, 2048, 128, 128, 0, 8, 8);
                let state = parse_x56_stick(&data).unwrap();
                prop_assert!((-1.0f32..=1.0).contains(&state.axes.y));
            }

            #[test]
            fn rz_axis_always_in_range(rz in 0u16..=4095) {
                let data = build_report(2048, 2048, rz, 128, 128, 0, 8, 8);
                let state = parse_x56_stick(&data).unwrap();
                prop_assert!((-1.0f32..=1.0).contains(&state.axes.rz));
            }

            #[test]
            fn rx_axis_always_in_range(rx in 0u8..=255) {
                let data = build_report(2048, 2048, 2048, rx, 128, 0, 8, 8);
                let state = parse_x56_stick(&data).unwrap();
                prop_assert!((-1.0f32..=1.0).contains(&state.axes.rx));
            }

            #[test]
            fn ry_axis_always_in_range(ry in 0u8..=255) {
                let data = build_report(2048, 2048, 2048, 128, ry, 0, 8, 8);
                let state = parse_x56_stick(&data).unwrap();
                prop_assert!((-1.0f32..=1.0).contains(&state.axes.ry));
            }

            #[test]
            fn buttons_roundtrip(buttons in 0u32..=0x00FF_FFFFu32) {
                let data = build_report(2048, 2048, 2048, 128, 128, buttons, 8, 8);
                let state = parse_x56_stick(&data).unwrap();
                prop_assert_eq!(state.buttons.buttons, buttons);
            }

            #[test]
            fn any_13byte_report_parses(data in proptest::collection::vec(any::<u8>(), 13..20usize)) {
                let result = parse_x56_stick(&data);
                prop_assert!(result.is_ok());
            }

            #[test]
            fn arbitrary_bytes_all_axes_in_range(
                data in proptest::collection::vec(any::<u8>(), 13..20usize),
            ) {
                let state = parse_x56_stick(&data).unwrap();
                prop_assert!((-1.0f32..=1.0).contains(&state.axes.x));
                prop_assert!((-1.0f32..=1.0).contains(&state.axes.y));
                prop_assert!((-1.0f32..=1.0).contains(&state.axes.rz));
                prop_assert!((-1.0f32..=1.0).contains(&state.axes.rx));
                prop_assert!((-1.0f32..=1.0).contains(&state.axes.ry));
            }

            #[test]
            fn out_of_range_buttons_always_false(
                data in proptest::collection::vec(any::<u8>(), 13..20usize),
            ) {
                let state = parse_x56_stick(&data).unwrap();
                prop_assert!(!state.buttons.button(0));
                for b in 25u8..=32 {
                    prop_assert!(!state.buttons.button(b));
                }
                prop_assert_eq!(
                    state.buttons.buttons & 0xFF00_0000,
                    0,
                    "upper 8 bits of button word must be 0"
                );
            }
        }
    }
}
