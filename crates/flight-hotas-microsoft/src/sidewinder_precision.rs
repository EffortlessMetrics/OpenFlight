// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! HID input parsing for the Microsoft SideWinder Precision 2 joystick.
//!
//! # Confirmed device identifier
//!
//! VID 0x045E (Microsoft), PID 0x002B — linux-hardware.org (multiple probes,
//! USB string "Microsoft SideWinder Precision 2").
//!
//! The Precision 2 is a non-FFB budget joystick from ~2000. It uses the same
//! 7-byte HID report layout as the SideWinder Force Feedback Pro, minus the
//! force feedback output channel.
//!
//! # Input report layout (report ID byte stripped by caller)
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

use thiserror::Error;

/// Hat switch positions for the SideWinder Precision 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SidewinderP2Hat {
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

impl SidewinderP2Hat {
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
            _ => Self::Center,
        }
    }
}

/// Normalised axis values from a SideWinder Precision 2 report.
#[derive(Debug, Clone, Default)]
pub struct SidewinderP2Axes {
    /// Roll axis (X). −1.0 = full left, 1.0 = full right.
    pub x: f32,
    /// Pitch axis (Y). −1.0 = full forward/up, 1.0 = full back/down.
    pub y: f32,
    /// Twist axis (Rz). −1.0 = full left twist, 1.0 = full right twist.
    pub rz: f32,
    /// Throttle slider. 0.0 = slider top/forward, 1.0 = slider aft/bottom.
    pub throttle: f32,
}

/// Button and hat state from a SideWinder Precision 2 report.
#[derive(Debug, Clone, Default)]
pub struct SidewinderP2Buttons {
    /// Button bitmask; bit 0 = button 1, bit 8 = button 9. Upper 7 bits unused.
    pub buttons: u16,
    /// Hat switch position.
    pub hat: SidewinderP2Hat,
}

impl SidewinderP2Buttons {
    /// Returns `true` if the specified button (1-indexed, 1–9) is pressed.
    pub fn button(&self, n: u8) -> bool {
        matches!(n, 1..=9) && (self.buttons >> (n - 1)) & 1 != 0
    }
}

/// Full parsed input state from a SideWinder Precision 2 HID report.
#[derive(Debug, Clone, Default)]
pub struct SidewinderP2InputState {
    pub axes: SidewinderP2Axes,
    pub buttons: SidewinderP2Buttons,
}

/// Errors returned by SideWinder Precision 2 report parsing.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SidewinderP2ParseError {
    #[error(
        "SideWinder Precision 2 report too short: expected at least {expected} bytes, got {actual}"
    )]
    TooShort { expected: usize, actual: usize },
}

/// Minimum HID input report length for the SideWinder Precision 2 (report ID stripped).
pub const SIDEWINDER_P2_MIN_REPORT_BYTES: usize = 7;

#[inline]
fn normalize_10bit_bipolar(raw: u16) -> f32 {
    (raw as f32 - 511.5) / 511.5
}

#[inline]
fn normalize_8bit_bipolar(raw: u8) -> f32 {
    (raw as f32 - 127.5) / 127.5
}

#[inline]
fn normalize_8bit_unipolar(raw: u8) -> f32 {
    raw as f32 / 255.0
}

/// Parse a 7-byte HID input report from a Microsoft SideWinder Precision 2.
///
/// The report must not include the report ID prefix byte.
///
/// # Errors
/// Returns [`SidewinderP2ParseError::TooShort`] if `data` is shorter than
/// [`SIDEWINDER_P2_MIN_REPORT_BYTES`].
pub fn parse_sidewinder_precision2(
    data: &[u8],
) -> Result<SidewinderP2InputState, SidewinderP2ParseError> {
    if data.len() < SIDEWINDER_P2_MIN_REPORT_BYTES {
        return Err(SidewinderP2ParseError::TooShort {
            expected: SIDEWINDER_P2_MIN_REPORT_BYTES,
            actual: data.len(),
        });
    }

    let x = (data[0] as u16) | ((data[1] as u16 & 0x03) << 8);
    let y = ((data[1] as u16) >> 2) | ((data[2] as u16 & 0x0F) << 6);
    let rz: u8 = (data[2] >> 4) | ((data[3] & 0x0F) << 4);
    let throttle: u8 = (data[3] >> 4) | ((data[4] & 0x0F) << 4);
    let hat_raw: u8 = data[4] >> 4;
    let buttons: u16 = (data[5] as u16) | (((data[6] & 0x01) as u16) << 8);

    Ok(SidewinderP2InputState {
        axes: SidewinderP2Axes {
            x: normalize_10bit_bipolar(x),
            y: normalize_10bit_bipolar(y),
            rz: normalize_8bit_bipolar(rz),
            throttle: normalize_8bit_unipolar(throttle),
        },
        buttons: SidewinderP2Buttons {
            buttons,
            hat: SidewinderP2Hat::from_nibble(hat_raw),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_report(x: u16, y: u16, rz: u8, throttle: u8, hat: u8, buttons: u16) -> [u8; 7] {
        let x = x & 0x3FF;
        let y = y & 0x3FF;
        let hat = hat & 0x0F;
        let buttons = buttons & 0x01FF;

        let mut b = [0u8; 7];
        b[0] = x as u8;
        b[1] = ((x >> 8) as u8) | ((y as u8 & 0x3F) << 2);
        b[2] = ((y >> 6) as u8 & 0x0F) | ((rz & 0x0F) << 4);
        b[3] = (rz >> 4) | ((throttle & 0x0F) << 4);
        b[4] = (throttle >> 4) | ((hat & 0x0F) << 4);
        b[5] = (buttons & 0xFF) as u8;
        b[6] = ((buttons >> 8) & 0x01) as u8;
        b
    }

    #[test]
    fn too_short_returns_error() {
        assert!(parse_sidewinder_precision2(&[0u8; 6]).is_err());
        assert!(parse_sidewinder_precision2(&[]).is_err());
    }

    #[test]
    fn centered_axes_near_zero() {
        let data = build_report(512, 512, 128, 0, 8, 0);
        let s = parse_sidewinder_precision2(&data).unwrap();
        assert!(s.axes.x.abs() < 0.01);
        assert!(s.axes.y.abs() < 0.01);
        assert!(s.axes.rz.abs() < 0.01);
    }

    #[test]
    fn x_full_deflection() {
        let left = build_report(0, 512, 128, 0, 8, 0);
        let right = build_report(1023, 512, 128, 0, 8, 0);
        let sl = parse_sidewinder_precision2(&left).unwrap();
        let sr = parse_sidewinder_precision2(&right).unwrap();
        assert!(sl.axes.x < -0.99);
        assert!(sr.axes.x > 0.99);
    }

    #[test]
    fn hat_center_and_north() {
        let center = build_report(512, 512, 128, 0, 8, 0);
        let north = build_report(512, 512, 128, 0, 0, 0);
        assert_eq!(
            parse_sidewinder_precision2(&center).unwrap().buttons.hat,
            SidewinderP2Hat::Center
        );
        assert_eq!(
            parse_sidewinder_precision2(&north).unwrap().buttons.hat,
            SidewinderP2Hat::North
        );
    }

    #[test]
    fn buttons_individual() {
        for btn in 1u8..=9 {
            let mask = 1u16 << (btn - 1);
            let data = build_report(512, 512, 128, 0, 8, mask);
            let s = parse_sidewinder_precision2(&data).unwrap();
            assert!(s.buttons.button(btn), "button {btn}");
        }
    }

    #[test]
    fn throttle_full_range() {
        let min = build_report(512, 512, 128, 0, 8, 0);
        let max = build_report(512, 512, 128, 255, 8, 0);
        let smin = parse_sidewinder_precision2(&min).unwrap();
        let smax = parse_sidewinder_precision2(&max).unwrap();
        assert!(smin.axes.throttle < 0.001);
        assert!(smax.axes.throttle > 0.999);
    }

    #[test]
    fn out_of_range_button_always_false() {
        let data = build_report(512, 512, 128, 0, 8, 0x01FF);
        let s = parse_sidewinder_precision2(&data).unwrap();
        assert!(!s.buttons.button(0));
        assert!(!s.buttons.button(10));
    }
}
