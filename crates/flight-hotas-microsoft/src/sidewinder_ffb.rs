// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! HID input parsing for the Microsoft SideWinder Force Feedback Pro and FFB 2.
//!
//! # Confirmed device identifiers
//!
//! - **FFB Pro**: VID 0x045E (Microsoft), PID 0x001B — linux-hardware.org and
//!   Linux kernel `hid-microsoft.c` (USB_DEVICE_ID_MICROSOFT_SIDEWINDER_FFB).
//! - **FFB 2**: VID 0x045E (Microsoft), PID 0x001C — Linux kernel
//!   `hid-microsoft.c` (USB_DEVICE_ID_MICROSOFT_SIDEWINDER_FFB2).
//!
//! Both devices share an identical 7-byte HID input report layout.
//!
//! # Input report layout (report ID byte stripped by caller)
//!
//! Fields are packed LSB-first in little-endian bit order, matching the device's
//! USB HID descriptor (DirectInput-era 1998 Sidewinder format):
//!
//! | Bit range | Field    | Type | Raw range | Notes                      |
//! |-----------|----------|------|-----------|----------------------------|
//! | 0–9       | X        | u10  | 0..1023   | Roll; center ≈ 512         |
//! | 10–19     | Y        | u10  | 0..1023   | Pitch; center ≈ 512        |
//! | 20–27     | Rz       | u8   | 0..255    | Twist; center ≈ 128        |
//! | 28–35     | Throttle | u8   | 0..255    | Slider; 0 = top/fwd        |
//! | 36–39     | Hat      | u4   | 0..8      | 0=N … 7=NW; 8+=center      |
//! | 40–48     | Buttons  | u9   | bitmask   | Buttons 1–9, bit 0 = btn 1 |
//! | 49–55     | Padding  | —    | —         | Unused; always 0           |
//!
//! ## Byte-level extraction
//!
//! ```text
//! Byte 0 : X[7:0]
//! Byte 1 : Y[5:0] | X[9:8]           (bits 1:0 = X[9:8], bits 7:2 = Y[5:0])
//! Byte 2 : Rz[3:0] | Y[9:6]          (bits 3:0 = Y[9:6], bits 7:4 = Rz[3:0])
//! Byte 3 : Throttle[3:0] | Rz[7:4]   (bits 3:0 = Rz[7:4], bits 7:4 = Throttle[3:0])
//! Byte 4 : Hat[3:0] | Throttle[7:4]  (bits 3:0 = Throttle[7:4], bits 7:4 = Hat[3:0])
//! Byte 5 : Buttons[7:0]              (bit 0 = button 1)
//! Byte 6 : Padding[6:1] | Buttons[8] (bit 0 = button 9; bits 7:1 = padding)
//! ```

use thiserror::Error;

/// Hat switch positions for the SideWinder Force Feedback stick hat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SidewinderFfbHat {
    /// Hat released / centered (raw nibble ≥ 8).
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

impl SidewinderFfbHat {
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
            _ => Self::Center, // 8–15
        }
    }
}

/// Normalised axis values from a SideWinder FFB Pro / FFB 2 report.
#[derive(Debug, Clone, Default)]
pub struct SidewinderFfbAxes {
    /// Roll axis (X / stick horizontal). −1.0 = full left, 1.0 = full right.
    pub x: f32,
    /// Pitch axis (Y / stick vertical). −1.0 = full forward/up, 1.0 = full back/down.
    pub y: f32,
    /// Twist axis (Rz). −1.0 = full left twist, 1.0 = full right twist; center = 0.0.
    pub rz: f32,
    /// Throttle slider. 0.0 = slider fully forward/top, 1.0 = slider fully aft/bottom.
    ///
    /// **Note:** physical top position is raw 0. Invert in your profile if you want
    /// top = full throttle.
    pub throttle: f32,
}

/// Button and hat state from a SideWinder FFB Pro / FFB 2 report.
#[derive(Debug, Clone, Default)]
pub struct SidewinderFfbButtons {
    /// Button bitmask; bit 0 = button 1, bit 8 = button 9. Upper 7 bits unused.
    pub buttons: u16,
    /// Hat switch position.
    pub hat: SidewinderFfbHat,
}

impl SidewinderFfbButtons {
    /// Returns `true` if the specified button (1-indexed, 1–9) is pressed.
    pub fn button(&self, n: u8) -> bool {
        matches!(n, 1..=9) && (self.buttons >> (n - 1)) & 1 != 0
    }
}

/// Full parsed input state from a SideWinder FFB Pro or FFB 2 HID report.
#[derive(Debug, Clone, Default)]
pub struct SidewinderFfbInputState {
    pub axes: SidewinderFfbAxes,
    pub buttons: SidewinderFfbButtons,
}

/// Errors returned by SideWinder FFB report parsing.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SidewinderFfbParseError {
    #[error("SideWinder FFB report too short: expected at least {expected} bytes, got {actual}")]
    TooShort { expected: usize, actual: usize },
}

/// Minimum HID input report length for the SideWinder FFB Pro / FFB 2 (report ID stripped).
pub const SIDEWINDER_FFB_MIN_REPORT_BYTES: usize = 7;

/// Normalise a 10-bit centered axis (0..1023) to −1.0..=1.0.
#[inline]
fn normalize_10bit_bipolar(raw: u16) -> f32 {
    (raw as f32 - 511.5) / 511.5
}

/// Normalise an 8-bit centered axis (0..255) to −1.0..=1.0.
#[inline]
fn normalize_8bit_bipolar(raw: u8) -> f32 {
    (raw as f32 - 127.5) / 127.5
}

/// Normalise an 8-bit unipolar axis (0..255) to 0.0..=1.0.
#[inline]
fn normalize_8bit_unipolar(raw: u8) -> f32 {
    raw as f32 / 255.0
}

/// Internal bit-extraction common to both FFB variants.
fn parse_ffb_report(data: &[u8]) -> Result<SidewinderFfbInputState, SidewinderFfbParseError> {
    if data.len() < SIDEWINDER_FFB_MIN_REPORT_BYTES {
        return Err(SidewinderFfbParseError::TooShort {
            expected: SIDEWINDER_FFB_MIN_REPORT_BYTES,
            actual: data.len(),
        });
    }

    // X: bits 0–9
    let x = (data[0] as u16) | ((data[1] as u16 & 0x03) << 8);

    // Y: bits 10–19
    let y = ((data[1] as u16) >> 2) | ((data[2] as u16 & 0x0F) << 6);

    // Rz: bits 20–27
    let rz: u8 = (data[2] >> 4) | ((data[3] & 0x0F) << 4);

    // Throttle: bits 28–35
    let throttle: u8 = (data[3] >> 4) | ((data[4] & 0x0F) << 4);

    // Hat: bits 36–39 (high nibble of byte 4)
    let hat_raw: u8 = data[4] >> 4;

    // Buttons: bits 40–48 — byte 5 holds buttons 1–8, byte 6 bit 0 holds button 9
    let buttons: u16 = (data[5] as u16) | (((data[6] & 0x01) as u16) << 8);

    Ok(SidewinderFfbInputState {
        axes: SidewinderFfbAxes {
            x: normalize_10bit_bipolar(x),
            y: normalize_10bit_bipolar(y),
            rz: normalize_8bit_bipolar(rz),
            throttle: normalize_8bit_unipolar(throttle),
        },
        buttons: SidewinderFfbButtons {
            buttons,
            hat: SidewinderFfbHat::from_nibble(hat_raw),
        },
    })
}

/// Parse a 7-byte HID input report from a Microsoft SideWinder Force Feedback Pro.
///
/// The report must not include the report ID prefix byte. Strip it before calling
/// (the OS / HID runtime prepends it; `data[0]` must be the first axis byte).
///
/// # Errors
/// Returns [`SidewinderFfbParseError::TooShort`] if `data` is shorter than
/// [`SIDEWINDER_FFB_MIN_REPORT_BYTES`].
pub fn parse_sidewinder_ffb_pro(
    data: &[u8],
) -> Result<SidewinderFfbInputState, SidewinderFfbParseError> {
    parse_ffb_report(data)
}

/// Parse a 7-byte HID input report from a Microsoft SideWinder Force Feedback 2.
///
/// The FFB 2 uses an identical HID report layout to the FFB Pro; this function
/// is provided as a distinct entry point for clarity at call sites.
///
/// # Errors
/// Returns [`SidewinderFfbParseError::TooShort`] if `data` is shorter than
/// [`SIDEWINDER_FFB_MIN_REPORT_BYTES`].
pub fn parse_sidewinder_ffb2(
    data: &[u8],
) -> Result<SidewinderFfbInputState, SidewinderFfbParseError> {
    parse_ffb_report(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a 7-byte FFB Pro / FFB 2 report from logical field values.
    fn build_report(x: u16, y: u16, rz: u8, throttle: u8, hat: u8, buttons: u16) -> [u8; 7] {
        let x = x & 0x3FF;
        let y = y & 0x3FF;
        let hat = hat & 0x0F;
        let buttons = buttons & 0x01FF;

        let mut b = [0u8; 7];
        // X: bits 0–9
        b[0] = x as u8;
        // bits 1:0 = X[9:8], bits 7:2 = Y[5:0]
        b[1] = ((x >> 8) as u8) | ((y as u8 & 0x3F) << 2);
        // bits 3:0 = Y[9:6], bits 7:4 = Rz[3:0]
        b[2] = ((y >> 6) as u8 & 0x0F) | ((rz & 0x0F) << 4);
        // bits 3:0 = Rz[7:4], bits 7:4 = Throttle[3:0]
        b[3] = (rz >> 4) | ((throttle & 0x0F) << 4);
        // bits 3:0 = Throttle[7:4], bits 7:4 = Hat[3:0]
        b[4] = (throttle >> 4) | ((hat & 0x0F) << 4);
        // Buttons 1–8
        b[5] = (buttons & 0xFF) as u8;
        // Button 9 in bit 0
        b[6] = ((buttons >> 8) & 0x01) as u8;
        b
    }

    #[test]
    fn too_short_returns_error() {
        assert!(parse_sidewinder_ffb_pro(&[0u8; 6]).is_err());
        assert!(parse_sidewinder_ffb_pro(&[]).is_err());
    }

    #[test]
    fn error_message_contains_sizes() {
        let err = parse_sidewinder_ffb_pro(&[0u8; 3]).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains('7'), "expected '7' in: {msg}");
        assert!(msg.contains('3'), "expected '3' in: {msg}");
    }

    #[test]
    fn centered_axes_near_zero() {
        let data = build_report(512, 512, 128, 0, 8, 0);
        let s = parse_sidewinder_ffb_pro(&data).unwrap();
        assert!(s.axes.x.abs() < 0.01, "x near 0: {}", s.axes.x);
        assert!(s.axes.y.abs() < 0.01, "y near 0: {}", s.axes.y);
        assert!(s.axes.rz.abs() < 0.01, "rz near 0: {}", s.axes.rz);
    }

    #[test]
    fn x_full_right() {
        let data = build_report(1023, 512, 128, 0, 8, 0);
        let s = parse_sidewinder_ffb_pro(&data).unwrap();
        assert!(s.axes.x > 0.99, "x ~1.0: {}", s.axes.x);
    }

    #[test]
    fn x_full_left() {
        let data = build_report(0, 512, 128, 0, 8, 0);
        let s = parse_sidewinder_ffb_pro(&data).unwrap();
        assert!(s.axes.x < -0.99, "x ~-1.0: {}", s.axes.x);
    }

    #[test]
    fn y_full_forward() {
        let data = build_report(512, 0, 128, 0, 8, 0);
        let s = parse_sidewinder_ffb_pro(&data).unwrap();
        assert!(s.axes.y < -0.99, "y ~-1.0: {}", s.axes.y);
    }

    #[test]
    fn throttle_max() {
        let data = build_report(512, 512, 128, 255, 8, 0);
        let s = parse_sidewinder_ffb_pro(&data).unwrap();
        assert!(
            s.axes.throttle > 0.999,
            "throttle ~1.0: {}",
            s.axes.throttle
        );
    }

    #[test]
    fn throttle_min() {
        let data = build_report(512, 512, 128, 0, 8, 0);
        let s = parse_sidewinder_ffb_pro(&data).unwrap();
        assert!(
            s.axes.throttle < 0.001,
            "throttle ~0.0: {}",
            s.axes.throttle
        );
    }

    #[test]
    fn hat_center_on_nibble_8() {
        let data = build_report(512, 512, 128, 0, 8, 0);
        let s = parse_sidewinder_ffb_pro(&data).unwrap();
        assert_eq!(s.buttons.hat, SidewinderFfbHat::Center);
    }

    #[test]
    fn hat_all_cardinal_positions() {
        let cases: &[(u8, SidewinderFfbHat)] = &[
            (0, SidewinderFfbHat::North),
            (1, SidewinderFfbHat::NorthEast),
            (2, SidewinderFfbHat::East),
            (3, SidewinderFfbHat::SouthEast),
            (4, SidewinderFfbHat::South),
            (5, SidewinderFfbHat::SouthWest),
            (6, SidewinderFfbHat::West),
            (7, SidewinderFfbHat::NorthWest),
        ];
        for &(raw, ref expected) in cases {
            let data = build_report(512, 512, 128, 0, raw, 0);
            let s = parse_sidewinder_ffb_pro(&data).unwrap();
            assert_eq!(&s.buttons.hat, expected, "hat nibble {raw}");
        }
    }

    #[test]
    fn buttons_individual_1_through_9() {
        for btn in 1u8..=9 {
            let mask = 1u16 << (btn - 1);
            let data = build_report(512, 512, 128, 0, 8, mask);
            let s = parse_sidewinder_ffb_pro(&data).unwrap();
            assert!(s.buttons.button(btn), "button {btn} should be pressed");
            for other in 1u8..=9 {
                if other != btn {
                    assert!(
                        !s.buttons.button(other),
                        "button {other} should NOT be pressed when {btn} is"
                    );
                }
            }
        }
    }

    #[test]
    fn all_buttons_pressed() {
        let data = build_report(512, 512, 128, 0, 8, 0x01FF);
        let s = parse_sidewinder_ffb_pro(&data).unwrap();
        for btn in 1u8..=9 {
            assert!(s.buttons.button(btn), "button {btn} should be pressed");
        }
        assert_eq!(s.buttons.buttons, 0x01FF);
    }

    #[test]
    fn out_of_range_button_numbers_always_false() {
        let data = build_report(512, 512, 128, 0, 8, 0x01FF);
        let s = parse_sidewinder_ffb_pro(&data).unwrap();
        assert!(!s.buttons.button(0));
        for b in 10u8..=20 {
            assert!(
                !s.buttons.button(b),
                "button {b} out of range should be false"
            );
        }
    }

    #[test]
    fn ffb2_parser_produces_same_state_as_ffb_pro() {
        let data = build_report(300, 700, 60, 180, 3, 0b101010101);
        let pro = parse_sidewinder_ffb_pro(&data).unwrap();
        let ffb2 = parse_sidewinder_ffb2(&data).unwrap();
        assert_eq!(pro.axes.x, ffb2.axes.x);
        assert_eq!(pro.axes.y, ffb2.axes.y);
        assert_eq!(pro.axes.rz, ffb2.axes.rz);
        assert_eq!(pro.axes.throttle, ffb2.axes.throttle);
        assert_eq!(pro.buttons.buttons, ffb2.buttons.buttons);
        assert_eq!(pro.buttons.hat, ffb2.buttons.hat);
    }

    #[test]
    fn extra_bytes_beyond_minimum_are_ignored() {
        let base = build_report(512, 512, 128, 0, 8, 0);
        let mut extended = base.to_vec();
        extended.extend_from_slice(&[0xFF, 0xFF, 0xFF]);
        let s = parse_sidewinder_ffb_pro(&extended).unwrap();
        // Button 9 bit must still be clean (from byte 6 bit 0 = 0)
        assert_eq!(s.buttons.buttons, 0);
    }

    #[cfg(test)]
    mod proptest_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn all_axes_always_in_range(
                x in 0u16..=1023,
                y in 0u16..=1023,
                rz in 0u8..=255,
                throttle in 0u8..=255,
            ) {
                let data = build_report(x, y, rz, throttle, 8, 0);
                let s = parse_sidewinder_ffb_pro(&data).unwrap();
                prop_assert!((-1.0f32..=1.0).contains(&s.axes.x),   "x={}", s.axes.x);
                prop_assert!((-1.0f32..=1.0).contains(&s.axes.y),   "y={}", s.axes.y);
                prop_assert!((-1.0f32..=1.0).contains(&s.axes.rz),  "rz={}", s.axes.rz);
                prop_assert!((0.0f32..=1.0).contains(&s.axes.throttle), "throttle={}", s.axes.throttle);
            }

            #[test]
            fn arbitrary_bytes_never_panic_and_axes_in_range(
                data in proptest::collection::vec(any::<u8>(), 7..20usize),
            ) {
                let s = parse_sidewinder_ffb_pro(&data).unwrap();
                prop_assert!((-1.0f32..=1.0).contains(&s.axes.x));
                prop_assert!((-1.0f32..=1.0).contains(&s.axes.y));
                prop_assert!((-1.0f32..=1.0).contains(&s.axes.rz));
                prop_assert!((0.0f32..=1.0).contains(&s.axes.throttle));
                // Upper 7 bits of the button word must always be zero
                prop_assert_eq!(s.buttons.buttons & 0xFE00, 0, "upper bits must be zero");
            }
        }
    }
}
