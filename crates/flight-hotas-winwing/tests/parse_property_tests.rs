// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Property-based and unit tests for WinWing HOTAS HID report parsing.

use flight_hotas_winwing::input::{
    RUDDER_REPORT_LEN, STICK_REPORT_LEN, THROTTLE_REPORT_LEN, WINWING_VENDOR_ID,
    parse_rudder_report, parse_stick_report, parse_throttle_report,
};
use proptest::prelude::*;

// ─── Throttle ────────────────────────────────────────────────────────────────

fn make_throttle_report(
    tl: u16,
    tr: u16,
    friction: u16,
    mx: u16,
    my: u16,
) -> [u8; THROTTLE_REPORT_LEN] {
    let mut r = [0u8; THROTTLE_REPORT_LEN];
    r[0] = 0x01;
    r[1..3].copy_from_slice(&tl.to_le_bytes());
    r[3..5].copy_from_slice(&tr.to_le_bytes());
    r[5..7].copy_from_slice(&friction.to_le_bytes());
    r[7..9].copy_from_slice(&mx.to_le_bytes());
    r[9..11].copy_from_slice(&my.to_le_bytes());
    r
}

proptest! {
    /// Throttle axes always in [0,1] for unsigned inputs.
    #[test]
    fn prop_throttle_axes_in_range(
        tl in 0u16..=u16::MAX,
        tr in 0u16..=u16::MAX,
        friction in 0u16..=u16::MAX,
    ) {
        let r = make_throttle_report(tl, tr, friction, 32768, 32768);
        let s = parse_throttle_report(&r).unwrap();
        prop_assert!(s.axes.throttle_left >= 0.0 && s.axes.throttle_left <= 1.0);
        prop_assert!(s.axes.throttle_right >= 0.0 && s.axes.throttle_right <= 1.0);
        prop_assert!(s.axes.friction >= 0.0 && s.axes.friction <= 1.0);
        // combined is average of left + right
        let expected = (s.axes.throttle_left + s.axes.throttle_right) * 0.5;
        prop_assert!((s.axes.throttle_combined - expected).abs() < 1e-5);
    }

    /// Throttle button mask (64-bit) round-trips exactly.
    #[test]
    fn prop_throttle_button_mask(mask_lo in 0u32..=u32::MAX, mask_hi in 0u32..=u32::MAX) {
        let mask = (mask_hi as u64) << 32 | mask_lo as u64;
        let mut r = make_throttle_report(0, 0, 0, 0, 0);
        r[11..19].copy_from_slice(&mask.to_le_bytes());
        let s = parse_throttle_report(&r).unwrap();
        prop_assert_eq!(s.buttons.mask, mask);
    }

    /// Short throttle reports always error.
    #[test]
    fn prop_throttle_short_errors(len in 0usize..THROTTLE_REPORT_LEN) {
        let data = vec![0x01u8; len];
        prop_assert!(parse_throttle_report(&data).is_err());
    }
}

// ─── Stick ────────────────────────────────────────────────────────────────────

fn make_stick_report(roll: i16, pitch: i16, mask: u32) -> [u8; STICK_REPORT_LEN] {
    let mut r = [0u8; STICK_REPORT_LEN];
    r[0] = 0x02;
    r[1..3].copy_from_slice(&roll.to_le_bytes());
    r[3..5].copy_from_slice(&pitch.to_le_bytes());
    r[5..9].copy_from_slice(&mask.to_le_bytes());
    r
}

proptest! {
    /// Stick axes always in [-1, 1].
    #[test]
    fn prop_stick_axes_in_range(
        roll in i16::MIN..=i16::MAX,
        pitch in i16::MIN..=i16::MAX,
    ) {
        let r = make_stick_report(roll, pitch, 0);
        let s = parse_stick_report(&r).unwrap();
        prop_assert!(s.axes.roll >= -1.0 && s.axes.roll <= 1.0);
        prop_assert!(s.axes.pitch >= -1.0 && s.axes.pitch <= 1.0);
    }

    /// Stick buttons 1–20 match the mask bits.
    #[test]
    fn prop_stick_button_query_matches(mask in 0u32..(1u32 << 20)) {
        let r = make_stick_report(0, 0, mask);
        let s = parse_stick_report(&r).unwrap();
        for n in 1u8..=20 {
            let expected = (mask >> (n - 1)) & 1 == 1;
            prop_assert_eq!(
                s.buttons.is_pressed(n),
                expected,
                "button {} mismatch mask=0x{:05X}", n, mask
            );
        }
    }

    /// Short stick reports always error.
    #[test]
    fn prop_stick_short_errors(len in 0usize..STICK_REPORT_LEN) {
        let data = vec![0x02u8; len];
        prop_assert!(parse_stick_report(&data).is_err());
    }
}

// ─── Rudder Pedals ────────────────────────────────────────────────────────────

fn make_rudder_report(
    rudder: i16,
    brake_left: u16,
    brake_right: u16,
) -> [u8; RUDDER_REPORT_LEN] {
    let mut r = [0u8; RUDDER_REPORT_LEN];
    r[0] = 0x03;
    r[1..3].copy_from_slice(&rudder.to_le_bytes());
    r[3..5].copy_from_slice(&brake_left.to_le_bytes());
    r[5..7].copy_from_slice(&brake_right.to_le_bytes());
    r
}

proptest! {
    /// Rudder axis in [-1, 1]; brakes in [0, 1].
    #[test]
    fn prop_rudder_axes_in_range(
        rudder in i16::MIN..=i16::MAX,
        brake_l in 0u16..=u16::MAX,
        brake_r in 0u16..=u16::MAX,
    ) {
        let r = make_rudder_report(rudder, brake_l, brake_r);
        let s = parse_rudder_report(&r).unwrap();
        prop_assert!(s.rudder >= -1.0 && s.rudder <= 1.0,
            "rudder out of range: {}", s.rudder);
        prop_assert!(s.brake_left >= 0.0 && s.brake_left <= 1.0,
            "brake_left out of range: {}", s.brake_left);
        prop_assert!(s.brake_right >= 0.0 && s.brake_right <= 1.0,
            "brake_right out of range: {}", s.brake_right);
    }

    /// Short rudder reports always error.
    #[test]
    fn prop_rudder_short_errors(len in 0usize..RUDDER_REPORT_LEN) {
        let data = vec![0x03u8; len];
        prop_assert!(parse_rudder_report(&data).is_err());
    }
}

// ─── Misc ─────────────────────────────────────────────────────────────────────

#[test]
fn vendor_id_is_winwing() {
    // WinWing VID 0x4098
    assert_eq!(WINWING_VENDOR_ID, 0x4098);
}

#[test]
fn centred_throttle_has_equal_combined() {
    let r = make_throttle_report(32768, 32768, 0, 32768, 32768);
    let s = parse_throttle_report(&r).unwrap();
    assert!((s.axes.throttle_combined - s.axes.throttle_left).abs() < 1e-5);
}

#[test]
fn throttle_all_buttons_on() {
    let mut r = make_throttle_report(0, 0, 0, 0, 0);
    r[11..19].copy_from_slice(&u64::MAX.to_le_bytes());
    let s = parse_throttle_report(&r).unwrap();
    for n in 1u8..=50 {
        assert!(s.buttons.is_pressed(n), "button {} should be pressed", n);
    }
}

#[test]
fn wrong_throttle_id_errors() {
    let mut r = make_throttle_report(0, 0, 0, 0, 0);
    r[0] = 0x05;
    assert!(parse_throttle_report(&r).is_err());
}

#[test]
fn wrong_stick_id_errors() {
    let mut r = make_stick_report(0, 0, 0);
    r[0] = 0x05;
    assert!(parse_stick_report(&r).is_err());
}

#[test]
fn wrong_rudder_id_errors() {
    let mut r = make_rudder_report(0, 0, 0);
    r[0] = 0x05;
    assert!(parse_rudder_report(&r).is_err());
}
