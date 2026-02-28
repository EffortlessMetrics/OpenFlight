// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! X65F F-22 Raptor HOTAS report parser.
//!
//! VID: 0x06A3 (Saitek)  PID: 0x0B6A
//!
//! # Protocol Status
//!
//! Input path uses standard HID — supported by the generic HID driver.
//! The exact report layout is UNVERIFIED — see `docs/reference/hotas-claims.md`.
//!
//! # Hypothesised Report Layout (≥ 16 bytes)
//!
//! | Bytes | Contents                              |
//! |-------|---------------------------------------|
//! | 0–1   | X axis — 11-bit, little-endian        |
//! | 2–3   | Y axis — 11-bit                       |
//! | 4–5   | Rz / twist — 10-bit                   |
//! | 6     | Throttle — 8-bit, unipolar 0–255      |
//! | 7–11  | Buttons 0–39 (5-byte bitmask)         |
//! | 12    | HAT switches — low nibble             |
//!
//! The X65F has approximately 36 physical buttons plus mode switches, requiring 5
//! button bytes.  The button bitmask spans bytes 7–11; `secondary` holds the high
//! byte (byte 11) for easy access to extended buttons.

use crate::input::HotasInputState;

/// Saitek vendor ID (VID 0x06A3).
pub const SAITEK_VID: u16 = 0x06A3;

/// X65F USB product ID.
///
/// Source: Linux kernel hid-ids.h (USB_DEVICE_ID_SAITEK_X65 = 0x0B6A).
/// Confidence: **Likely** — needs lsusb verification from real hardware.
pub const X65F_PID: u16 = 0x0B6A;

/// Minimum HID report length accepted by the X65F parser.
pub const MIN_REPORT_LEN: usize = 16;

/// Parse a raw HID report from the X65F.
///
/// Normalises axes to −1.0..=1.0 (bipolar) or 0.0..=1.0 (throttle).
/// Buttons are returned as a raw bitmask — callers should apply ghost filtering.
///
/// Returns `None` if `report` is shorter than [`MIN_REPORT_LEN`].
pub fn parse_x65f_report(report: &[u8]) -> Option<HotasInputState> {
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

    // Primary buttons: bytes 7-10 (buttons 0-31)
    state.buttons.primary = u32::from_le_bytes([report[7], report[8], report[9], report[10]]);
    // Extended buttons: byte 11 stored in secondary low byte (buttons 32-39)
    state.buttons.secondary = report[11] as u32;

    state.buttons.hats = report[12] & 0x0F;

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
        assert!(parse_x65f_report(&zeros(MIN_REPORT_LEN - 1)).is_none());
    }

    #[test]
    fn minimum_length_report_parses_ok() {
        assert!(parse_x65f_report(&zeros(MIN_REPORT_LEN)).is_some());
    }

    #[test]
    fn zero_report_gives_zero_throttle() {
        let state = parse_x65f_report(&zeros(MIN_REPORT_LEN)).unwrap();
        assert!(
            (state.axes.throttle - 0.0).abs() < 0.001,
            "throttle={}",
            state.axes.throttle
        );
    }

    #[test]
    fn max_throttle_gives_near_one() {
        let mut report = zeros(MIN_REPORT_LEN);
        report[6] = 255;
        let state = parse_x65f_report(&report).unwrap();
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
        let state = parse_x65f_report(&report).unwrap();
        assert!(
            state.axes.stick_x.abs() < 0.01,
            "stick_x={}",
            state.axes.stick_x
        );
    }

    #[test]
    fn primary_buttons_parsed_from_bytes_7_to_10() {
        let mut report = zeros(MIN_REPORT_LEN);
        report[7] = 0x01;
        report[8] = 0x02;
        report[9] = 0x03;
        report[10] = 0x04;
        let state = parse_x65f_report(&report).unwrap();
        assert_eq!(state.buttons.primary, 0x0403_0201);
    }

    #[test]
    fn extended_buttons_in_secondary() {
        let mut report = zeros(MIN_REPORT_LEN);
        report[11] = 0xBE; // extended buttons byte
        let state = parse_x65f_report(&report).unwrap();
        assert_eq!(state.buttons.secondary, 0xBE);
    }

    #[test]
    fn hat_decoded_from_low_nibble_of_byte_12() {
        let mut report = zeros(MIN_REPORT_LEN);
        report[12] = 0xA5; // low nibble = 5
        let state = parse_x65f_report(&report).unwrap();
        assert_eq!(state.buttons.hats, 5);
    }
}
