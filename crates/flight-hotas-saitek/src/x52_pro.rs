// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! X52 Pro HOTAS report parser.
//!
//! VID: 0x06A3 (Saitek)  PID: 0x0762
//!
//! # Protocol Status
//!
//! Input path uses standard HID — supported by the generic HID driver.
//! Axis resolution is reported as 11-bit (UNVERIFIED — see `docs/reference/hotas-claims.md`).
//!
//! # Hypothesised Report Layout (21 bytes)
//!
//! | Bytes | Contents                            |
//! |-------|-------------------------------------|
//! | 0–1   | X axis — 11-bit, little-endian      |
//! | 2–3   | Y axis — 11-bit                     |
//! | 4–5   | Rz / twist — 10-bit                 |
//! | 6     | Throttle — 8-bit, unipolar 0–255    |
//! | 7–10  | Buttons 0–31 (bitmask)              |
//! | 11    | HAT switch — low nibble             |

use crate::input::HotasInputState;

/// Saitek vendor ID (VID 0x06A3).
pub const SAITEK_VID: u16 = 0x06A3;

/// X52 Pro USB product ID.
///
/// Confirmed: VID 0x06A3 (Saitek), PID 0x0762 — USB-IF registry, libx52, hid-saitek.c.
pub const X52_PRO_PID: u16 = 0x0762;

/// Minimum HID report length accepted by the X52 Pro parser.
pub const MIN_REPORT_LEN: usize = 14;

/// Parse a raw HID report from the X52 Pro.
///
/// Normalises axes to −1.0..=1.0 (bipolar) or 0.0..=1.0 (throttle).
/// Buttons are returned as a raw bitmask — callers should apply ghost filtering.
///
/// Returns `None` if `report` is shorter than [`MIN_REPORT_LEN`].
pub fn parse_x52_pro_report(report: &[u8]) -> Option<HotasInputState> {
    if report.len() < MIN_REPORT_LEN {
        return None;
    }

    let mut state = HotasInputState::default();

    // 11-bit X axis: bits 10-0 packed across bytes 0-1
    let x_raw = u16::from_le_bytes([report[0], report[1] & 0x07]) & 0x7FF;
    // 11-bit Y axis: bytes 2-3
    let y_raw = u16::from_le_bytes([report[2], report[3] & 0x07]) & 0x7FF;
    // 10-bit twist axis: bytes 4-5
    let rz_raw = u16::from_le_bytes([report[4], report[5] & 0x03]) & 0x3FF;

    state.axes.stick_x = normalize_11bit(x_raw);
    state.axes.stick_y = normalize_11bit(y_raw);
    state.axes.stick_twist = normalize_10bit(rz_raw);
    state.axes.throttle = report[6] as f32 / 255.0;

    state.buttons.primary = u32::from_le_bytes([report[7], report[8], report[9], report[10]]);

    if report.len() > 11 {
        state.buttons.hats = report[11] & 0x0F;
    }

    Some(state)
}

/// Normalise an 11-bit unsigned value to −1.0..=1.0.
#[inline]
fn normalize_11bit(raw: u16) -> f32 {
    ((raw as f32 / 1023.5) - 1.0).clamp(-1.0, 1.0)
}

/// Normalise a 10-bit unsigned value to −1.0..=1.0.
#[inline]
fn normalize_10bit(raw: u16) -> f32 {
    ((raw as f32 / 511.5) - 1.0).clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zeros(len: usize) -> Vec<u8> {
        vec![0u8; len]
    }

    #[test]
    fn report_too_short_returns_none() {
        assert!(parse_x52_pro_report(&zeros(MIN_REPORT_LEN - 1)).is_none());
    }

    #[test]
    fn minimum_length_report_parses_ok() {
        assert!(parse_x52_pro_report(&zeros(MIN_REPORT_LEN)).is_some());
    }

    #[test]
    fn zero_report_gives_negative_one_for_bipolar_axes() {
        let state = parse_x52_pro_report(&zeros(MIN_REPORT_LEN)).unwrap();
        assert!(
            (state.axes.stick_x - (-1.0)).abs() < 0.01,
            "stick_x={}",
            state.axes.stick_x
        );
        assert!(
            (state.axes.stick_y - (-1.0)).abs() < 0.01,
            "stick_y={}",
            state.axes.stick_y
        );
        assert!(
            (state.axes.stick_twist - (-1.0)).abs() < 0.01,
            "twist={}",
            state.axes.stick_twist
        );
    }

    #[test]
    fn zero_report_gives_zero_throttle() {
        let state = parse_x52_pro_report(&zeros(MIN_REPORT_LEN)).unwrap();
        assert!(
            (state.axes.throttle - 0.0).abs() < 0.001,
            "throttle={}",
            state.axes.throttle
        );
    }

    #[test]
    fn max_throttle_byte_gives_near_one() {
        let mut report = zeros(MIN_REPORT_LEN);
        report[6] = 255;
        let state = parse_x52_pro_report(&report).unwrap();
        assert!(
            state.axes.throttle > 0.99,
            "throttle={}",
            state.axes.throttle
        );
    }

    #[test]
    fn mid_x_axis_is_near_zero() {
        let mut report = zeros(MIN_REPORT_LEN);
        // 11-bit midpoint ≈ 1024 (0x0400): byte[0]=0x00, byte[1] bit2=1 → 0x04
        report[0] = 0x00;
        report[1] = 0x04;
        let state = parse_x52_pro_report(&report).unwrap();
        assert!(
            state.axes.stick_x.abs() < 0.01,
            "stick_x={}",
            state.axes.stick_x
        );
    }

    #[test]
    fn max_x_axis_is_near_positive_one() {
        let mut report = zeros(MIN_REPORT_LEN);
        // 11-bit max = 0x7FF: byte[0]=0xFF, byte[1] & 0x07 = 0x07
        report[0] = 0xFF;
        report[1] = 0x07;
        let state = parse_x52_pro_report(&report).unwrap();
        assert!(state.axes.stick_x > 0.99, "stick_x={}", state.axes.stick_x);
    }

    #[test]
    fn buttons_parsed_from_bytes_7_to_10() {
        let mut report = zeros(MIN_REPORT_LEN);
        report[7] = 0xAB;
        report[8] = 0xCD;
        report[9] = 0x00;
        report[10] = 0x00;
        let state = parse_x52_pro_report(&report).unwrap();
        assert_eq!(state.buttons.primary, 0x0000_CDAB);
    }

    #[test]
    fn hat_decoded_from_low_nibble_of_byte_11() {
        let mut report = zeros(MIN_REPORT_LEN + 1);
        report[11] = 0xF3; // low nibble = 3
        let state = parse_x52_pro_report(&report).unwrap();
        assert_eq!(state.buttons.hats, 3);
    }

    #[test]
    fn hat_is_zero_when_report_at_minimum_length() {
        // MIN_REPORT_LEN = 14, byte[11] is within range but not beyond it
        let state = parse_x52_pro_report(&zeros(MIN_REPORT_LEN)).unwrap();
        assert_eq!(state.buttons.hats, 0);
    }
}
