// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the X56 Rhino throttle (Mad Catz / Saitek / Logitech).
//!
//! # Device identifiers
//!
//! - Mad Catz era: VID 0x0738, PID 0xA221
//! - Saitek era: VID 0x06A3, PID 0xA215 (X55 Throttle, shared layout)
//!
//! # Input report layout (UNVERIFIED — community reverse-engineered)
//!
//! The X56 throttle produces a 14-byte HID input report (no report ID prefix).
//! Fields are packed in LSB-first bit order:
//!
//! | Bit range  | Field       | Type | Range   | Notes                         |
//! |------------|-------------|------|---------|-------------------------------|
//! | 0-9        | ThrottleL   | u10  | 0..1023 | Left throttle; unipolar       |
//! | 10-19      | ThrottleR   | u10  | 0..1023 | Right throttle; unipolar      |
//! | 20-27      | RotaryL     | u8   | 0..255  | Left rotary encoder; unipolar |
//! | 28-35      | RotaryR     | u8   | 0..255  | Right rotary encoder; unipolar|
//! | 36-43      | SliderL     | u8   | 0..255  | Left slider; unipolar         |
//! | 44-51      | SliderR     | u8   | 0..255  | Right slider; unipolar        |
//! | 52-79      | Buttons     | u28  | bitmask | 28 buttons, LSB-first         |
//! | 80-83      | Hat1        | u4   | 0-15    | Primary hat                   |
//! | 84-87      | Hat2        | u4   | 0-15    | Secondary hat                 |
//! | 88-111     | Padding     | —    | —       | Unused bits                   |

use thiserror::Error;

use crate::x56_stick::X56Hat;

/// Minimum HID input report length for the X56 throttle.
pub const X56_THROTTLE_MIN_REPORT_BYTES: usize = 14;

/// Parsed axis values from the X56 throttle, normalized.
#[derive(Debug, Clone, Default)]
pub struct X56ThrottleAxes {
    /// Left throttle lever. 0.0 = idle, 1.0 = full forward.
    pub throttle_left: f32,
    /// Right throttle lever. 0.0 = idle, 1.0 = full forward.
    pub throttle_right: f32,
    /// Left rotary encoder. 0.0 = minimum, 1.0 = maximum.
    pub rotary_left: f32,
    /// Right rotary encoder. 0.0 = minimum, 1.0 = maximum.
    pub rotary_right: f32,
    /// Left slider. 0.0 = minimum, 1.0 = maximum.
    pub slider_left: f32,
    /// Right slider. 0.0 = minimum, 1.0 = maximum.
    pub slider_right: f32,
}

/// Parsed button/hat state from the X56 throttle.
#[derive(Debug, Clone, Default)]
pub struct X56ThrottleButtons {
    /// Button bitmask; bit 0 = button 1, bit 27 = button 28.
    pub buttons: u32,
    /// Primary hat switch position.
    pub hat1: X56Hat,
    /// Secondary hat switch position.
    pub hat2: X56Hat,
}

impl X56ThrottleButtons {
    /// Returns `true` if the specified button (1-indexed, 1–28) is pressed.
    pub fn button(&self, n: u8) -> bool {
        matches!(n, 1..=28) && (self.buttons >> (n - 1)) & 1 != 0
    }
}

/// Full parsed input state from an X56 throttle HID report.
#[derive(Debug, Clone, Default)]
pub struct X56ThrottleInputState {
    pub axes: X56ThrottleAxes,
    pub buttons: X56ThrottleButtons,
}

/// Errors returned by X56 throttle report parsing.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum X56ThrottleParseError {
    #[error("X56 throttle report too short: expected at least {expected} bytes, got {actual}")]
    TooShort { expected: usize, actual: usize },
}

/// Normalize a 10-bit unipolar axis (0..1023) to 0.0..=1.0.
#[inline]
fn normalize_10bit_unipolar(raw: u16) -> f32 {
    (raw as f32 / 1023.0).clamp(0.0, 1.0)
}

/// Normalize an 8-bit unipolar axis (0..255) to 0.0..=1.0.
#[inline]
fn normalize_8bit_unipolar(raw: u8) -> f32 {
    (raw as f32 / 255.0).clamp(0.0, 1.0)
}

/// Parse a 14-byte HID input report from the X56 Rhino throttle.
///
/// # Bit layout (LSB-first within each byte, little-endian)
///
/// ```text
/// Byte  0:    ThrottleL[7:0]
/// Byte  1:    ThrottleL[9:8] in bits 1:0, ThrottleR[5:0] in bits 7:2
/// Byte  2:    ThrottleR[9:6] in bits 3:0, RotaryL[3:0] in bits 7:4
/// Byte  3:    RotaryL[7:4] in bits 3:0, RotaryR[3:0] in bits 7:4
/// Byte  4:    RotaryR[7:4] in bits 3:0, SliderL[3:0] in bits 7:4
/// Byte  5:    SliderL[7:4] in bits 3:0, SliderR[3:0] in bits 7:4
/// Byte  6:    SliderR[7:4] in bits 3:0, Buttons[3:0] in bits 7:4
/// Bytes 7-9:  Buttons continued
/// Byte 10:    Hat1[3:0] in lower nibble, Hat2[3:0] in upper nibble
/// Bytes 11-13: Padding
/// ```
///
/// Simplified (byte-aligned) layout used here:
/// ```text
/// Byte  0:    ThrottleL[7:0]
/// Byte  1:    ThrottleR[5:0] << 2 | ThrottleL[9:8]
/// Byte  2:    ThrottleR[9:6] in bits 3:0, RotaryL[3:0] in bits 7:4
/// Byte  3:    RotaryL[7:4] | RotaryR[3:0]
/// Byte  4:    RotaryR[7:4] | SliderL[3:0]
/// Byte  5:    SliderL[7:4] | SliderR[3:0]
/// Byte  6:    SliderR[7:4] | Buttons[3:0]
/// Byte  7:    Buttons[11:4]
/// Byte  8:    Buttons[19:12]
/// Byte  9:    Buttons[27:20]
/// Byte 10:    Hat1[3:0] | Hat2[3:0]
/// ```
///
/// # Errors
/// Returns [`X56ThrottleParseError::TooShort`] if `data` is shorter than
/// [`X56_THROTTLE_MIN_REPORT_BYTES`].
pub fn parse_x56_throttle(data: &[u8]) -> Result<X56ThrottleInputState, X56ThrottleParseError> {
    if data.len() < X56_THROTTLE_MIN_REPORT_BYTES {
        return Err(X56ThrottleParseError::TooShort {
            expected: X56_THROTTLE_MIN_REPORT_BYTES,
            actual: data.len(),
        });
    }

    // ThrottleL: bits 0-9 (10-bit)
    let throttle_left = (data[0] as u16) | (((data[1] & 0x03) as u16) << 8);

    // ThrottleR: bits 10-19 (10-bit)
    let throttle_right = ((data[1] >> 2) as u16) | (((data[2] & 0x0F) as u16) << 6);

    // RotaryL: bits 20-27 (8-bit)
    let rotary_left = (data[2] >> 4) | ((data[3] & 0x0F) << 4);

    // RotaryR: bits 28-35 (8-bit)
    let rotary_right = (data[3] >> 4) | ((data[4] & 0x0F) << 4);

    // SliderL: bits 36-43 (8-bit)
    let slider_left = (data[4] >> 4) | ((data[5] & 0x0F) << 4);

    // SliderR: bits 44-51 (8-bit)
    let slider_right = (data[5] >> 4) | ((data[6] & 0x0F) << 4);

    // Buttons: bits 52-79 (28 buttons)
    let btn_b0 = (data[6] >> 4) as u32; // bits 0-3
    let btn_b1 = (data[7] as u32) << 4; // bits 4-11
    let btn_b2 = (data[8] as u32) << 12; // bits 12-19
    let btn_b3 = (data[9] as u32) << 20; // bits 20-27
    let buttons = (btn_b0 | btn_b1 | btn_b2 | btn_b3) & 0x0FFF_FFFF;

    // Hats: byte 10
    let hat1_raw = data[10] & 0x0F;
    let hat2_raw = data[10] >> 4;

    Ok(X56ThrottleInputState {
        axes: X56ThrottleAxes {
            throttle_left: normalize_10bit_unipolar(throttle_left),
            throttle_right: normalize_10bit_unipolar(throttle_right),
            rotary_left: normalize_8bit_unipolar(rotary_left),
            rotary_right: normalize_8bit_unipolar(rotary_right),
            slider_left: normalize_8bit_unipolar(slider_left),
            slider_right: normalize_8bit_unipolar(slider_right),
        },
        buttons: X56ThrottleButtons {
            buttons,
            hat1: X56Hat::from_nibble(hat1_raw),
            hat2: X56Hat::from_nibble(hat2_raw),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a 14-byte X56 throttle report from logical field values.
    fn build_report(
        thr_left: u16,
        thr_right: u16,
        rot_left: u8,
        rot_right: u8,
        sld_left: u8,
        sld_right: u8,
        buttons: u32,
        hat1: u8,
        hat2: u8,
    ) -> [u8; 14] {
        let tl = thr_left & 0x3FF;
        let tr = thr_right & 0x3FF;
        let buttons = buttons & 0x0FFF_FFFF;
        let hat1 = hat1 & 0x0F;
        let hat2 = hat2 & 0x0F;

        let mut data = [0u8; 14];
        // ThrottleL: bits 0-9
        data[0] = tl as u8;
        // ThrottleL[9:8] in bits 1:0, ThrottleR[5:0] in bits 7:2
        data[1] = ((tl >> 8) as u8 & 0x03) | (((tr & 0x3F) as u8) << 2);
        // ThrottleR[9:6] in bits 3:0, RotaryL[3:0] in bits 7:4
        data[2] = ((tr >> 6) as u8 & 0x0F) | ((rot_left & 0x0F) << 4);
        // RotaryL[7:4] in bits 3:0, RotaryR[3:0] in bits 7:4
        data[3] = (rot_left >> 4) | ((rot_right & 0x0F) << 4);
        // RotaryR[7:4] in bits 3:0, SliderL[3:0] in bits 7:4
        data[4] = (rot_right >> 4) | ((sld_left & 0x0F) << 4);
        // SliderL[7:4] in bits 3:0, SliderR[3:0] in bits 7:4
        data[5] = (sld_left >> 4) | ((sld_right & 0x0F) << 4);
        // SliderR[7:4] in bits 3:0, Buttons[3:0] in bits 7:4
        data[6] = (sld_right >> 4) | (((buttons & 0x0F) as u8) << 4);
        // Buttons[11:4]
        data[7] = ((buttons >> 4) & 0xFF) as u8;
        // Buttons[19:12]
        data[8] = ((buttons >> 12) & 0xFF) as u8;
        // Buttons[27:20]
        data[9] = ((buttons >> 20) & 0xFF) as u8;
        // Hat1 in lower nibble, Hat2 in upper nibble
        data[10] = hat1 | (hat2 << 4);
        data[11] = 0;
        data[12] = 0;
        data[13] = 0;
        data
    }

    #[test]
    fn test_too_short() {
        assert!(parse_x56_throttle(&[0u8; 13]).is_err());
        assert!(parse_x56_throttle(&[]).is_err());
    }

    #[test]
    fn test_too_short_error_fields() {
        let err = parse_x56_throttle(&[0u8; 5]).unwrap_err();
        assert_eq!(
            err,
            X56ThrottleParseError::TooShort {
                expected: X56_THROTTLE_MIN_REPORT_BYTES,
                actual: 5
            }
        );
    }

    #[test]
    fn test_all_idle() {
        let data = build_report(0, 0, 0, 0, 0, 0, 0, 8, 8);
        let state = parse_x56_throttle(&data).unwrap();
        assert!(state.axes.throttle_left < 0.001);
        assert!(state.axes.throttle_right < 0.001);
        assert!(state.axes.rotary_left < 0.001);
        assert!(state.axes.rotary_right < 0.001);
        assert!(state.axes.slider_left < 0.001);
        assert!(state.axes.slider_right < 0.001);
    }

    #[test]
    fn test_all_max() {
        let data = build_report(1023, 1023, 255, 255, 255, 255, 0, 8, 8);
        let state = parse_x56_throttle(&data).unwrap();
        assert!(state.axes.throttle_left > 0.999);
        assert!(state.axes.throttle_right > 0.999);
        assert!(state.axes.rotary_left > 0.999);
        assert!(state.axes.rotary_right > 0.999);
        assert!(state.axes.slider_left > 0.999);
        assert!(state.axes.slider_right > 0.999);
    }

    #[test]
    fn test_left_throttle_half() {
        let data = build_report(512, 0, 0, 0, 0, 0, 0, 8, 8);
        let state = parse_x56_throttle(&data).unwrap();
        let expected = 512.0f32 / 1023.0;
        assert!(
            (state.axes.throttle_left - expected).abs() < 1e-3,
            "left throttle half: {}",
            state.axes.throttle_left
        );
    }

    #[test]
    fn test_throttles_independent() {
        let data = build_report(1023, 0, 0, 0, 0, 0, 0, 8, 8);
        let state = parse_x56_throttle(&data).unwrap();
        assert!(state.axes.throttle_left > 0.999);
        assert!(state.axes.throttle_right < 0.001);
    }

    #[test]
    fn test_hat1_north() {
        let data = build_report(0, 0, 0, 0, 0, 0, 0, 0, 8);
        let state = parse_x56_throttle(&data).unwrap();
        assert_eq!(state.buttons.hat1, X56Hat::North);
    }

    #[test]
    fn test_hat2_west() {
        let data = build_report(0, 0, 0, 0, 0, 0, 0, 8, 6);
        let state = parse_x56_throttle(&data).unwrap();
        assert_eq!(state.buttons.hat2, X56Hat::West);
    }

    #[test]
    fn test_buttons_individual() {
        for b in 1u8..=28 {
            let mask = 1u32 << (b - 1);
            let data = build_report(0, 0, 0, 0, 0, 0, mask, 8, 8);
            let state = parse_x56_throttle(&data).unwrap();
            assert!(state.buttons.button(b), "button {} should be pressed", b);
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
        let data = build_report(0, 0, 0, 0, 0, 0, 0x0FFF_FFFF, 8, 8);
        let state = parse_x56_throttle(&data).unwrap();
        for b in 1u8..=28 {
            assert!(state.buttons.button(b), "button {} should be pressed", b);
        }
    }

    #[test]
    fn test_out_of_range_button_returns_false() {
        let data = build_report(0, 0, 0, 0, 0, 0, 0x0FFF_FFFF, 8, 8);
        let state = parse_x56_throttle(&data).unwrap();
        assert!(!state.buttons.button(0));
        assert!(!state.buttons.button(29));
    }

    #[test]
    fn test_longer_report_accepted() {
        let data = build_report(512, 512, 128, 128, 128, 128, 0, 8, 8);
        let mut longer = data.to_vec();
        longer.extend_from_slice(&[0xFF, 0xFF]);
        assert!(parse_x56_throttle(&longer).is_ok());
    }

    #[cfg(test)]
    mod proptest_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn throttle_left_always_unipolar(tl in 0u16..=1023) {
                let data = build_report(tl, 0, 0, 0, 0, 0, 0, 8, 8);
                let state = parse_x56_throttle(&data).unwrap();
                prop_assert!((0.0f32..=1.0).contains(&state.axes.throttle_left));
            }

            #[test]
            fn throttle_right_always_unipolar(tr in 0u16..=1023) {
                let data = build_report(0, tr, 0, 0, 0, 0, 0, 8, 8);
                let state = parse_x56_throttle(&data).unwrap();
                prop_assert!((0.0f32..=1.0).contains(&state.axes.throttle_right));
            }

            #[test]
            fn rotary_left_always_unipolar(rl in 0u8..=255) {
                let data = build_report(0, 0, rl, 0, 0, 0, 0, 8, 8);
                let state = parse_x56_throttle(&data).unwrap();
                prop_assert!((0.0f32..=1.0).contains(&state.axes.rotary_left));
            }

            #[test]
            fn buttons_roundtrip(buttons in 0u32..=0x0FFF_FFFFu32) {
                let data = build_report(0, 0, 0, 0, 0, 0, buttons, 8, 8);
                let state = parse_x56_throttle(&data).unwrap();
                prop_assert_eq!(state.buttons.buttons, buttons);
            }

            #[test]
            fn any_14byte_report_parses(data in proptest::collection::vec(any::<u8>(), 14..20usize)) {
                let result = parse_x56_throttle(&data);
                prop_assert!(result.is_ok());
            }

            #[test]
            fn arbitrary_bytes_all_axes_in_range(
                data in proptest::collection::vec(any::<u8>(), 14..20usize),
            ) {
                let state = parse_x56_throttle(&data).unwrap();
                prop_assert!((0.0f32..=1.0).contains(&state.axes.throttle_left));
                prop_assert!((0.0f32..=1.0).contains(&state.axes.throttle_right));
                prop_assert!((0.0f32..=1.0).contains(&state.axes.rotary_left));
                prop_assert!((0.0f32..=1.0).contains(&state.axes.rotary_right));
                prop_assert!((0.0f32..=1.0).contains(&state.axes.slider_left));
                prop_assert!((0.0f32..=1.0).contains(&state.axes.slider_right));
            }

            #[test]
            fn out_of_range_buttons_always_false(
                data in proptest::collection::vec(any::<u8>(), 14..20usize),
            ) {
                let state = parse_x56_throttle(&data).unwrap();
                prop_assert!(!state.buttons.button(0));
                for b in 29u8..=32 {
                    prop_assert!(!state.buttons.button(b));
                }
                prop_assert_eq!(
                    state.buttons.buttons & 0xF000_0000,
                    0,
                    "upper 4 bits of button word must be 0"
                );
            }
        }
    }
}
