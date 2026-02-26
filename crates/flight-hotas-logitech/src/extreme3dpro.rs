// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the Logitech Extreme 3D Pro joystick.
//!
//! # Confirmed device identifier
//!
//! VID 0x046D (Logitech), PID 0xC215 — confirmed via linux-hardware.org (221 probes).
//!
//! # Input report layout (community-documented, widely verified)
//!
//! The Extreme 3D Pro produces a 7-byte HID input report (no report ID). Fields
//! are tightly packed in LSB-first bit order as specified in the HID descriptor:
//!
//! | Bit range | Field      | Type  | Range   | Notes                              |
//! |-----------|------------|-------|---------|-----------------------------------|
//! | 0-9       | X          | u10   | 0..1023 | Roll/horizontal; center ~511       |
//! | 10-19     | Y          | u10   | 0..1023 | Pitch/vertical; center ~511        |
//! | 20-27     | Twist (Rz) | u8    | 0..255  | Twist handle; center ~127          |
//! | 28-34     | Throttle   | u7    | 0..127  | Side slider; see quirk note below  |
//! | 35-46     | Buttons    | u12   | bitmask | Buttons 1-12 packed LSB-first      |
//! | 47-50     | Hat        | u4    | 0-15    | 0=N, 1=NE, ... 7=NW, 8..15=center |
//! | 51-55     | Padding    | —     | —       | Unused bits, always 0              |
//!
//! ## Throttle direction
//!
//! The physical slider has no spring return. When the lever is pushed fully
//! forward (top), the raw value is **0** (idle). When pulled fully back
//! (bottom), the raw value is **127** (maximum physical travel). OpenFlight
//! normalizes this as-is: 0→0.0, 127→1.0. Users who prefer 0=back/off and
//! 1=forward/full should invert the axis in their profile.

use thiserror::Error;

/// Hat switch positions for the Extreme 3D Pro top-mounted 8-way hat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Extreme3DProHat {
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

impl Extreme3DProHat {
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

/// Parsed axis values from the Extreme 3D Pro, normalized.
#[derive(Debug, Clone, Default)]
pub struct Extreme3DProAxes {
    /// Roll axis (X / stick horizontal). −1.0 = full left, 1.0 = full right.
    pub x: f32,
    /// Pitch axis (Y / stick vertical). −1.0 = full forward/up, 1.0 = full back/down.
    pub y: f32,
    /// Twist axis (Rz). −1.0 = full left twist, 1.0 = full right twist; center = 0.0.
    pub twist: f32,
    /// Throttle lever. 0.0 = lever top/forward, 1.0 = lever bottom/back.
    ///
    /// **Note:** physical orientation is inverted from conventional throttle — top is idle.
    /// Invert in your profile if you want top = full throttle.
    pub throttle: f32,
}

/// Parsed buttons from the Extreme 3D Pro.
#[derive(Debug, Clone, Default)]
pub struct Extreme3DProButtons {
    /// Button bitmask; bit 0 = button 1, bit 11 = button 12. Upper 4 bits unused.
    pub buttons: u16,
    /// Hat switch position.
    pub hat: Extreme3DProHat,
}

impl Extreme3DProButtons {
    /// Returns `true` if the specified button (1-indexed, 1-12) is pressed.
    pub fn button(&self, n: u8) -> bool {
        matches!(n, 1..=12) && (self.buttons >> (n - 1)) & 1 != 0
    }
}

/// Full parsed input state from an Extreme 3D Pro HID report.
#[derive(Debug, Clone, Default)]
pub struct Extreme3DProInputState {
    pub axes: Extreme3DProAxes,
    pub buttons: Extreme3DProButtons,
}

/// Errors returned by Extreme 3D Pro report parsing.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Extreme3DProParseError {
    #[error("Extreme 3D Pro report too short: expected at least {expected} bytes, got {actual}")]
    TooShort { expected: usize, actual: usize },
}

/// Minimum HID input report length for the Extreme 3D Pro.
pub const EXTREME_3D_PRO_MIN_REPORT_BYTES: usize = 7;

/// Normalize a 10-bit centered axis (0..1023) to −1.0..=1.0.
#[inline]
fn normalize_10bit_bipolar(raw: u16) -> f32 {
    (raw as f32 - 511.5) / 511.5
}

/// Normalize an 8-bit centered axis (0..255) to −1.0..=1.0.
#[inline]
fn normalize_8bit_bipolar(raw: u8) -> f32 {
    (raw as f32 - 127.5) / 127.5
}

/// Normalize a 7-bit unipolar axis (0..127) to 0.0..=1.0.
#[inline]
fn normalize_7bit_unipolar(raw: u8) -> f32 {
    raw as f32 / 127.0
}

/// Parse a 7-byte HID input report from the Logitech Extreme 3D Pro.
///
/// The report must not include a report ID prefix. If the OS/driver prepends
/// one, strip it before calling this function.
///
/// # Bit layout (LSB-first within each byte, little-endian)
///
/// ```text
/// Byte 0:  X[7:0]
/// Byte 1:  Y[5:0] | X[9:8]          (bits 1:0 = X[9:8], bits 7:2 = Y[5:0])
/// Byte 2:  Twist[3:0] | Y[9:6]      (bits 3:0 = Y[9:6], bits 7:4 = Twist[3:0])
/// Byte 3:  Throttle[3:0] | Twist[7:4] (bits 3:0 = Twist[7:4], bits 6:4 = Throttle[2:0],
///                                       bit 7 = Throttle[3])
/// Byte 4:  Btn[4:0] | Throttle[6:4] (bits 2:0 = Throttle[6:4], bits 7:3 = Btn[4:0])
/// Byte 5:  Hat[0] | Btn[11:5]       (bits 6:0 = Btn[11:5], bit 7 = Hat[0])
/// Byte 6:  Pad[4:0] | Hat[3:1]      (bits 2:0 = Hat[3:1], bits 7:3 = padding)
/// ```
///
/// # Errors
/// Returns [`Extreme3DProParseError::TooShort`] if `data` is shorter than
/// [`EXTREME_3D_PRO_MIN_REPORT_BYTES`].
pub fn parse_extreme_3d_pro(data: &[u8]) -> Result<Extreme3DProInputState, Extreme3DProParseError> {
    if data.len() < EXTREME_3D_PRO_MIN_REPORT_BYTES {
        return Err(Extreme3DProParseError::TooShort {
            expected: EXTREME_3D_PRO_MIN_REPORT_BYTES,
            actual: data.len(),
        });
    }

    // X: bits 0-9
    let x = (data[0] as u16) | ((data[1] as u16 & 0x03) << 8);

    // Y: bits 10-19
    let y = ((data[1] as u16) >> 2) | ((data[2] as u16 & 0x0F) << 6);

    // Twist: bits 20-27
    let twist = ((data[2] >> 4) | ((data[3] & 0x0F) << 4)) as u8;

    // Throttle: bits 28-34 (7 bits)
    let throttle_raw = ((data[3] >> 4) | ((data[4] & 0x07) << 4)) as u8;

    // Buttons: bits 35-46 (12 buttons)
    let btn_low = (data[4] >> 3) as u16; // 5 bits from byte 4
    let btn_high = (data[5] & 0x7F) as u16; // 7 bits from byte 5
    let buttons = btn_low | (btn_high << 5);

    // Hat: bits 47-50 (4 bits)
    let hat_raw = ((data[5] >> 7) | ((data[6] & 0x07) << 1)) as u8;

    Ok(Extreme3DProInputState {
        axes: Extreme3DProAxes {
            x: normalize_10bit_bipolar(x),
            y: normalize_10bit_bipolar(y),
            twist: normalize_8bit_bipolar(twist),
            throttle: normalize_7bit_unipolar(throttle_raw),
        },
        buttons: Extreme3DProButtons {
            buttons,
            hat: Extreme3DProHat::from_nibble(hat_raw),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a 7-byte Extreme 3D Pro report from logical field values.
    fn build_report(x: u16, y: u16, twist: u8, throttle: u8, buttons: u16, hat: u8) -> [u8; 7] {
        let x = x & 0x3FF;
        let y = y & 0x3FF;
        let twist = twist;
        let throttle = throttle & 0x7F;
        let buttons = buttons & 0x0FFF;
        let hat = hat & 0x0F;

        let mut data = [0u8; 7];
        // X: bits 0-9
        data[0] = x as u8;
        // Y starts at bit 10; byte 1 carries X[9:8] in bits 0-1 and Y[5:0] in bits 7:2
        data[1] = ((x >> 8) as u8) | ((y as u8 & 0x3F) << 2);
        // Y[9:6] in bits 3:0 of byte 2, Twist[3:0] in bits 7:4
        data[2] = ((y >> 6) as u8 & 0x0F) | ((twist & 0x0F) << 4);
        // Twist[7:4] in bits 3:0 of byte 3, Throttle[3:0] in bits 7:4
        data[3] = (twist >> 4) | ((throttle & 0x0F) << 4);
        // Throttle[6:4] in bits 2:0 of byte 4, Buttons[4:0] in bits 7:3
        data[4] = (throttle >> 4) | (((buttons & 0x1F) as u8) << 3);
        // Buttons[11:5] in bits 6:0, Hat[0] in bit 7
        data[5] = ((buttons >> 5) as u8 & 0x7F) | ((hat & 0x01) << 7);
        // Hat[3:1] in bits 2:0, padding in bits 7:3
        data[6] = (hat >> 1) & 0x07;
        data
    }

    #[test]
    fn test_too_short() {
        assert!(parse_extreme_3d_pro(&[0u8; 6]).is_err());
        assert!(parse_extreme_3d_pro(&[]).is_err());
    }

    #[test]
    fn test_centered() {
        let data = build_report(512, 512, 128, 0, 0, 8); // hat center
        let state = parse_extreme_3d_pro(&data).unwrap();
        assert!(state.axes.x.abs() < 0.01, "x near 0: {}", state.axes.x);
        assert!(state.axes.y.abs() < 0.01, "y near 0: {}", state.axes.y);
        assert!(
            state.axes.twist.abs() < 0.01,
            "twist near 0: {}",
            state.axes.twist
        );
        assert_eq!(state.buttons.hat, Extreme3DProHat::Center);
    }

    #[test]
    fn test_full_right() {
        let data = build_report(1023, 512, 128, 0, 0, 8);
        let state = parse_extreme_3d_pro(&data).unwrap();
        assert!(state.axes.x > 0.99, "x should be ~1.0: {}", state.axes.x);
    }

    #[test]
    fn test_full_left() {
        let data = build_report(0, 512, 128, 0, 0, 8);
        let state = parse_extreme_3d_pro(&data).unwrap();
        assert!(state.axes.x < -0.99, "x should be ~-1.0: {}", state.axes.x);
    }

    #[test]
    fn test_full_forward() {
        let data = build_report(512, 0, 128, 0, 0, 8);
        let state = parse_extreme_3d_pro(&data).unwrap();
        assert!(state.axes.y < -0.99, "y should be ~-1.0: {}", state.axes.y);
    }

    #[test]
    fn test_throttle_max() {
        let data = build_report(512, 512, 128, 127, 0, 8);
        let state = parse_extreme_3d_pro(&data).unwrap();
        assert!(
            state.axes.throttle > 0.999,
            "throttle should be ~1.0: {}",
            state.axes.throttle
        );
    }

    #[test]
    fn test_throttle_min() {
        let data = build_report(512, 512, 128, 0, 0, 8);
        let state = parse_extreme_3d_pro(&data).unwrap();
        assert!(
            state.axes.throttle < 0.001,
            "throttle should be ~0.0: {}",
            state.axes.throttle
        );
    }

    #[test]
    fn test_hat_north() {
        let data = build_report(512, 512, 128, 0, 0, 0);
        let state = parse_extreme_3d_pro(&data).unwrap();
        assert_eq!(state.buttons.hat, Extreme3DProHat::North);
    }

    #[test]
    fn test_hat_east() {
        let data = build_report(512, 512, 128, 0, 0, 2);
        let state = parse_extreme_3d_pro(&data).unwrap();
        assert_eq!(state.buttons.hat, Extreme3DProHat::East);
    }

    #[test]
    fn test_hat_south() {
        let data = build_report(512, 512, 128, 0, 0, 4);
        let state = parse_extreme_3d_pro(&data).unwrap();
        assert_eq!(state.buttons.hat, Extreme3DProHat::South);
    }

    #[test]
    fn test_buttons_individual() {
        for b in 1u8..=12 {
            let mask = 1u16 << (b - 1);
            let data = build_report(512, 512, 128, 0, mask, 8);
            let state = parse_extreme_3d_pro(&data).unwrap();
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
        let data = build_report(512, 512, 128, 0, 0x0FFF, 8);
        let state = parse_extreme_3d_pro(&data).unwrap();
        for b in 1u8..=12 {
            assert!(state.buttons.button(b), "button {} should be pressed", b);
        }
    }

    #[test]
    fn test_twist_full_right() {
        let data = build_report(512, 512, 255, 0, 0, 8);
        let state = parse_extreme_3d_pro(&data).unwrap();
        assert!(
            state.axes.twist > 0.99,
            "twist full right: {}",
            state.axes.twist
        );
    }

    #[test]
    fn test_twist_full_left() {
        let data = build_report(512, 512, 0, 0, 0, 8);
        let state = parse_extreme_3d_pro(&data).unwrap();
        assert!(
            state.axes.twist < -0.99,
            "twist full left: {}",
            state.axes.twist
        );
    }

    #[test]
    fn test_axes_roundtrip_sample_values() {
        // Ensure axes normalize correctly at a range of known raw values
        for x_raw in [0u16, 256, 511, 512, 767, 1023] {
            let data = build_report(x_raw, 512, 128, 0, 0, 8);
            let state = parse_extreme_3d_pro(&data).unwrap();
            assert!(
                (-1.0..=1.0).contains(&state.axes.x),
                "x out of range at raw {}: {}",
                x_raw,
                state.axes.x
            );
        }
    }

    #[cfg(test)]
    mod proptest_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn x_axis_always_in_range(x in 0u16..=1023) {
                let data = build_report(x, 512, 128, 0, 0, 8);
                let state = parse_extreme_3d_pro(&data).unwrap();
                prop_assert!((-1.0f32..=1.0).contains(&state.axes.x));
            }

            #[test]
            fn y_axis_always_in_range(y in 0u16..=1023) {
                let data = build_report(512, y, 128, 0, 0, 8);
                let state = parse_extreme_3d_pro(&data).unwrap();
                prop_assert!((-1.0f32..=1.0).contains(&state.axes.y));
            }

            #[test]
            fn twist_always_in_range(twist in 0u8..=255) {
                let data = build_report(512, 512, twist, 0, 0, 8);
                let state = parse_extreme_3d_pro(&data).unwrap();
                prop_assert!((-1.0f32..=1.0).contains(&state.axes.twist));
            }

            #[test]
            fn throttle_always_unipolar(throttle in 0u8..=127) {
                let data = build_report(512, 512, 128, throttle, 0, 8);
                let state = parse_extreme_3d_pro(&data).unwrap();
                prop_assert!((0.0f32..=1.0).contains(&state.axes.throttle));
            }

            #[test]
            fn buttons_roundtrip(buttons in 0u16..=0x0FFFu16) {
                let data = build_report(512, 512, 128, 0, buttons, 8);
                let state = parse_extreme_3d_pro(&data).unwrap();
                prop_assert_eq!(state.buttons.buttons, buttons);
            }

            #[test]
            fn any_7byte_report_parses(data in proptest::collection::vec(any::<u8>(), 7..20usize)) {
                let result = parse_extreme_3d_pro(&data);
                prop_assert!(result.is_ok());
            }

            /// Verify that *arbitrary* byte patterns (not just well-formed reports from
            /// `build_report`) always produce axis values within the normalised output
            /// ranges.  This exercises the bit-extraction arithmetic under adversarial
            /// bit patterns that the structured tests cannot reach.
            #[test]
            fn arbitrary_bytes_all_axes_in_range(
                data in proptest::collection::vec(any::<u8>(), 7..20usize),
            ) {
                let state = parse_extreme_3d_pro(&data).unwrap();
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
                    (-1.0f32..=1.0).contains(&state.axes.twist),
                    "twist out of range: {}",
                    state.axes.twist
                );
                prop_assert!(
                    (0.0f32..=1.0).contains(&state.axes.throttle),
                    "throttle out of range: {}",
                    state.axes.throttle
                );
            }

            /// Verify that button numbers outside the valid 1-12 range always return
            /// `false`, regardless of the raw report bytes.
            #[test]
            fn out_of_range_buttons_always_false(
                data in proptest::collection::vec(any::<u8>(), 7..20usize),
            ) {
                let state = parse_extreme_3d_pro(&data).unwrap();
                // button(0) is out-of-range; must never be reported as pressed
                prop_assert!(!state.buttons.button(0));
                // buttons 13-20 are all out-of-range
                for b in 13u8..=20 {
                    prop_assert!(
                        !state.buttons.button(b),
                        "button {} out of range should be false",
                        b
                    );
                }
                // The upper 4 bits of the 16-bit button word must always be 0
                prop_assert_eq!(
                    state.buttons.buttons & 0xF000,
                    0,
                    "upper bits of button word must be 0"
                );
            }
        }
    }
}
