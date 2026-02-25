// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Property-based tests for VPforce Rhino HID report parsing.

use flight_ffb_vpforce::input::{RHINO_REPORT_LEN, VPFORCE_VENDOR_ID, parse_report};
use proptest::prelude::*;

fn make_rhino_report(
    roll: i16,
    pitch: i16,
    throttle_raw: i16,
    rocker: i16,
    twist: i16,
    mask: u32,
) -> [u8; RHINO_REPORT_LEN] {
    let mut r = [0u8; RHINO_REPORT_LEN];
    r[0] = 0x01;
    r[1..3].copy_from_slice(&roll.to_le_bytes());
    r[3..5].copy_from_slice(&pitch.to_le_bytes());
    r[5..7].copy_from_slice(&throttle_raw.to_le_bytes());
    r[7..9].copy_from_slice(&rocker.to_le_bytes());
    // bytes 9-10: Ry (unused in current parser)
    r[11..13].copy_from_slice(&twist.to_le_bytes());
    r[13..17].copy_from_slice(&mask.to_le_bytes());
    r
}

proptest! {
    /// All parsed axis values stay within valid bounds.
    #[test]
    fn prop_axes_in_range(
        roll in i16::MIN..=i16::MAX,
        pitch in i16::MIN..=i16::MAX,
        throttle_raw in i16::MIN..=i16::MAX,
        rocker in i16::MIN..=i16::MAX,
        twist in i16::MIN..=i16::MAX,
    ) {
        let r = make_rhino_report(roll, pitch, throttle_raw, rocker, twist, 0);
        let s = parse_report(&r).unwrap();
        prop_assert!(s.axes.roll >= -1.0 && s.axes.roll <= 1.0,
            "roll out of range: {}", s.axes.roll);
        prop_assert!(s.axes.pitch >= -1.0 && s.axes.pitch <= 1.0,
            "pitch out of range: {}", s.axes.pitch);
        prop_assert!(s.axes.throttle >= 0.0 && s.axes.throttle <= 1.0,
            "throttle out of range: {}", s.axes.throttle);
        prop_assert!(s.axes.rocker >= -1.0 && s.axes.rocker <= 1.0,
            "rocker out of range: {}", s.axes.rocker);
        prop_assert!(s.axes.twist >= -1.0 && s.axes.twist <= 1.0,
            "twist out of range: {}", s.axes.twist);
    }

    /// Button mask (32-bit) round-trips exactly.
    #[test]
    fn prop_button_mask_roundtrip(mask in 0u32..=u32::MAX) {
        let r = make_rhino_report(0, 0, 0, 0, 0, mask);
        let s = parse_report(&r).unwrap();
        prop_assert_eq!(s.buttons.mask, mask);
    }

    /// Individual button queries agree with the mask for all 32 buttons.
    #[test]
    fn prop_button_query_matches_mask(mask in 0u32..=u32::MAX) {
        let r = make_rhino_report(0, 0, 0, 0, 0, mask);
        let s = parse_report(&r).unwrap();
        for n in 1u8..=32 {
            let expected = (mask >> (n - 1)) & 1 == 1;
            prop_assert_eq!(
                s.buttons.is_pressed(n),
                expected,
                "button {} mismatch: mask=0x{:08X}", n, mask
            );
        }
    }

    /// Truncated reports always error.
    #[test]
    fn prop_short_report_errors(len in 0usize..RHINO_REPORT_LEN) {
        let data = vec![0x01u8; len];
        prop_assert!(parse_report(&data).is_err());
    }

    /// Non-0x01 report IDs always error.
    #[test]
    fn prop_wrong_id_errors(id in 2u8..=0xFF) {
        let mut r = [0u8; RHINO_REPORT_LEN];
        r[0] = id;
        prop_assert!(parse_report(&r).is_err());
    }
}

#[test]
fn vendor_id_is_correct() {
    // VPforce Rhino uses STMicro VID 0x0483
    assert_eq!(VPFORCE_VENDOR_ID, 0x0483);
}

#[test]
fn centred_report_gives_neutral_axes() {
    let r = make_rhino_report(0, 0, 0, 0, 0, 0);
    let s = parse_report(&r).unwrap();
    assert!(s.axes.roll.abs() < 1e-4);
    assert!(s.axes.pitch.abs() < 1e-4);
    assert!((s.axes.throttle - 0.5).abs() < 1e-3);
    assert!(s.axes.rocker.abs() < 1e-4);
    assert!(s.axes.twist.abs() < 1e-4);
    assert_eq!(s.buttons.mask, 0);
}

#[test]
fn hat_value_preserved() {
    let mut r = [0u8; RHINO_REPORT_LEN];
    r[0] = 0x01;
    r[17] = 4; // West direction
    let s = parse_report(&r).unwrap();
    assert_eq!(s.buttons.hat, 4);
}

#[test]
fn all_buttons_on() {
    let r = make_rhino_report(0, 0, 0, 0, 0, u32::MAX);
    let s = parse_report(&r).unwrap();
    for n in 1u8..=32 {
        assert!(s.buttons.is_pressed(n), "button {} should be pressed", n);
    }
}
