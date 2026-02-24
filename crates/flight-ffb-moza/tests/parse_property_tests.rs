// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Property-based tests for Moza AB9 HID report parsing.

use flight_ffb_moza::input::{AB9_REPORT_LEN, MOZA_VENDOR_ID, parse_ab9_report};
use proptest::prelude::*;

/// Build a valid AB9 report from axis values (i16 raw) + button mask.
fn make_ab9_report(roll: i16, pitch: i16, throttle_raw: i16, twist: i16, mask: u16) -> [u8; AB9_REPORT_LEN] {
    let mut r = [0u8; AB9_REPORT_LEN];
    r[0] = 0x01;
    r[1..3].copy_from_slice(&roll.to_le_bytes());
    r[3..5].copy_from_slice(&pitch.to_le_bytes());
    r[5..7].copy_from_slice(&throttle_raw.to_le_bytes());
    r[7..9].copy_from_slice(&twist.to_le_bytes());
    r[9..11].copy_from_slice(&mask.to_le_bytes());
    r
}

proptest! {
    /// Axes from a valid report are always within [-1.0, 1.0] (roll, pitch, twist)
    /// or [0.0, 1.0] (throttle).
    #[test]
    fn prop_axes_in_range(
        roll in i16::MIN..=i16::MAX,
        pitch in i16::MIN..=i16::MAX,
        throttle_raw in i16::MIN..=i16::MAX,
        twist in i16::MIN..=i16::MAX,
    ) {
        let r = make_ab9_report(roll, pitch, throttle_raw, twist, 0);
        let state = parse_ab9_report(&r).unwrap();
        prop_assert!(state.axes.roll >= -1.0 && state.axes.roll <= 1.0,
            "roll out of range: {}", state.axes.roll);
        prop_assert!(state.axes.pitch >= -1.0 && state.axes.pitch <= 1.0,
            "pitch out of range: {}", state.axes.pitch);
        // Allow ε below 0 for i16::MIN edge case (−32768/32767 ≈ −1.000030)
        prop_assert!(state.axes.throttle >= -0.001 && state.axes.throttle <= 1.001,
            "throttle out of range: {}", state.axes.throttle);
        prop_assert!(state.axes.twist >= -1.0 && state.axes.twist <= 1.0,
            "twist out of range: {}", state.axes.twist);
    }

    /// Button mask round-trips: any u16 mask is preserved exactly.
    #[test]
    fn prop_button_mask_roundtrip(mask in 0u16..=0xFFFF) {
        let r = make_ab9_report(0, 0, 0, 0, mask);
        let state = parse_ab9_report(&r).unwrap();
        prop_assert_eq!(state.buttons.mask, mask);
    }

    /// Individual button queries agree with the mask.
    #[test]
    fn prop_button_query_matches_mask(mask in 0u16..=0xFFFF) {
        let r = make_ab9_report(0, 0, 0, 0, mask);
        let state = parse_ab9_report(&r).unwrap();
        for n in 1u8..=16 {
            let expected = (mask >> (n - 1)) & 1 == 1;
            prop_assert_eq!(
                state.buttons.is_pressed(n),
                expected,
                "button {} mismatch: mask=0b{:016b}", n, mask
            );
        }
    }

    /// Reports shorter than AB9_REPORT_LEN always return TooShort error.
    #[test]
    fn prop_short_report_errors(len in 0usize..AB9_REPORT_LEN) {
        let data = vec![0x01u8; len];
        let result = parse_ab9_report(&data);
        prop_assert!(result.is_err(), "expected error for len={}", len);
    }

    /// Reports with wrong report-ID (not 0x01) return UnknownReportId.
    #[test]
    fn prop_wrong_report_id_errors(id in 2u8..=0xFF) {
        let mut r = [0u8; AB9_REPORT_LEN];
        r[0] = id;
        let result = parse_ab9_report(&r);
        prop_assert!(result.is_err(), "expected error for id=0x{:02X}", id);
    }
}

#[test]
fn vendor_id_is_correct() {
    assert_eq!(MOZA_VENDOR_ID, 0x346E);
}

#[test]
fn zero_report_gives_neutral_axes() {
    let r = make_ab9_report(0, 0, 0, 0, 0);
    let s = parse_ab9_report(&r).unwrap();
    assert!(s.axes.roll.abs() < 1e-4, "roll={}", s.axes.roll);
    assert!(s.axes.pitch.abs() < 1e-4, "pitch={}", s.axes.pitch);
    assert!((s.axes.throttle - 0.5).abs() < 1e-3, "throttle={}", s.axes.throttle);
    assert!(s.axes.twist.abs() < 1e-4, "twist={}", s.axes.twist);
    assert_eq!(s.buttons.mask, 0);
}

#[test]
fn full_positive_deflection() {
    let r = make_ab9_report(i16::MAX, i16::MAX, i16::MAX, i16::MAX, 0xFFFF);
    let s = parse_ab9_report(&r).unwrap();
    assert!((s.axes.roll - 1.0).abs() < 0.001);
    assert!((s.axes.pitch - 1.0).abs() < 0.001);
    assert!((s.axes.throttle - 1.0).abs() < 0.001);
    assert!((s.axes.twist - 1.0).abs() < 0.001);
    assert_eq!(s.buttons.mask, 0xFFFF);
}

#[test]
fn full_negative_deflection() {
    let r = make_ab9_report(i16::MIN, i16::MIN, i16::MIN, i16::MIN, 0);
    let s = parse_ab9_report(&r).unwrap();
    // i16::MIN / 32767 ≈ -1.0001 → close to -1
    assert!(s.axes.roll <= -0.99, "roll={}", s.axes.roll);
    assert!(s.axes.pitch <= -0.99, "pitch={}", s.axes.pitch);
    assert!(s.axes.throttle >= -0.001, "throttle={}", s.axes.throttle);
}
