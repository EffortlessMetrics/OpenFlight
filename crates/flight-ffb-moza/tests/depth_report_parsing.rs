// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for Moza AB9 HID report parsing — covers boundary values,
//! individual axis extremes, hat-switch states, and malformed data.

use flight_ffb_moza::input::{
    AB9_REPORT_LEN, Ab9Buttons, MozaParseError, parse_ab9_report,
};

fn make_report_full(
    roll: i16,
    pitch: i16,
    throttle: i16,
    twist: i16,
    mask: u16,
    hat: u8,
) -> [u8; AB9_REPORT_LEN] {
    let mut r = [0u8; AB9_REPORT_LEN];
    r[0] = 0x01;
    r[1..3].copy_from_slice(&roll.to_le_bytes());
    r[3..5].copy_from_slice(&pitch.to_le_bytes());
    r[5..7].copy_from_slice(&throttle.to_le_bytes());
    r[7..9].copy_from_slice(&twist.to_le_bytes());
    r[9..11].copy_from_slice(&mask.to_le_bytes());
    r[11] = hat;
    r
}

fn centred() -> [u8; AB9_REPORT_LEN] {
    make_report_full(0, 0, 0, 0, 0, 0xFF)
}

// ── Axis boundary tests ─────────────────────────────────────────────────

#[test]
fn negative_full_deflection_roll() {
    let r = make_report_full(i16::MIN, 0, 0, 0, 0, 0);
    let s = parse_ab9_report(&r).unwrap();
    assert!(s.axes.roll <= -0.99, "roll should be ≈ −1.0, got {}", s.axes.roll);
    assert!(s.axes.roll >= -1.0, "roll should be clamped to −1.0, got {}", s.axes.roll);
}

#[test]
fn negative_full_deflection_pitch() {
    let r = make_report_full(0, i16::MIN, 0, 0, 0, 0);
    let s = parse_ab9_report(&r).unwrap();
    assert!(s.axes.pitch <= -0.99);
    assert!(s.axes.pitch >= -1.0);
}

#[test]
fn twist_full_positive_and_negative() {
    let r_pos = make_report_full(0, 0, 0, i16::MAX, 0, 0);
    let s_pos = parse_ab9_report(&r_pos).unwrap();
    assert!((s_pos.axes.twist - 1.0).abs() < 1e-4);

    let r_neg = make_report_full(0, 0, 0, i16::MIN, 0, 0);
    let s_neg = parse_ab9_report(&r_neg).unwrap();
    assert!(s_neg.axes.twist <= -0.99);
}

#[test]
fn small_positive_axis_value() {
    let r = make_report_full(1, 0, 0, 0, 0, 0);
    let s = parse_ab9_report(&r).unwrap();
    assert!(s.axes.roll > 0.0, "single LSB should be slightly positive");
    assert!(s.axes.roll < 0.001, "single LSB should be very small");
}

#[test]
fn small_negative_axis_value() {
    let r = make_report_full(-1, 0, 0, 0, 0, 0);
    let s = parse_ab9_report(&r).unwrap();
    assert!(s.axes.roll < 0.0, "negative LSB should be slightly negative");
    assert!(s.axes.roll > -0.001);
}

#[test]
fn throttle_at_midpoint_is_half() {
    let r = make_report_full(0, 0, 0, 0, 0, 0);
    let s = parse_ab9_report(&r).unwrap();
    assert!(
        (s.axes.throttle - 0.5).abs() < 1e-3,
        "zero raw → 0.5 normalised, got {}",
        s.axes.throttle
    );
}

#[test]
fn throttle_minimum_and_maximum() {
    let r_min = make_report_full(0, 0, i16::MIN, 0, 0, 0);
    let s_min = parse_ab9_report(&r_min).unwrap();
    assert!(s_min.axes.throttle < 0.01, "min throttle should be ≈0.0");

    let r_max = make_report_full(0, 0, i16::MAX, 0, 0, 0);
    let s_max = parse_ab9_report(&r_max).unwrap();
    assert!(s_max.axes.throttle > 0.99, "max throttle should be ≈1.0");
}

#[test]
fn all_axes_at_positive_extreme() {
    let r = make_report_full(i16::MAX, i16::MAX, i16::MAX, i16::MAX, 0, 0);
    let s = parse_ab9_report(&r).unwrap();
    assert!((s.axes.roll - 1.0).abs() < 1e-4);
    assert!((s.axes.pitch - 1.0).abs() < 1e-4);
    assert!(s.axes.throttle > 0.99);
    assert!((s.axes.twist - 1.0).abs() < 1e-4);
}

#[test]
fn all_axes_at_negative_extreme() {
    let r = make_report_full(i16::MIN, i16::MIN, i16::MIN, i16::MIN, 0, 0);
    let s = parse_ab9_report(&r).unwrap();
    assert!(s.axes.roll >= -1.0 && s.axes.roll <= -0.99);
    assert!(s.axes.pitch >= -1.0 && s.axes.pitch <= -0.99);
    assert!(s.axes.throttle < 0.01);
    assert!(s.axes.twist >= -1.0 && s.axes.twist <= -0.99);
}

// ── Hat switch tests ────────────────────────────────────────────────────

#[test]
fn hat_switch_all_directions() {
    for hat_val in 0..=7 {
        let r = make_report_full(0, 0, 0, 0, 0, hat_val);
        let s = parse_ab9_report(&r).unwrap();
        assert_eq!(s.buttons.hat, hat_val, "hat direction {} not preserved", hat_val);
    }
}

#[test]
fn hat_switch_centred_is_0xff() {
    let r = make_report_full(0, 0, 0, 0, 0, 0xFF);
    let s = parse_ab9_report(&r).unwrap();
    assert_eq!(s.buttons.hat, 0xFF);
}

// ── Button tests ────────────────────────────────────────────────────────

#[test]
fn each_button_individually() {
    for n in 1u8..=16 {
        let mask = 1u16 << (n - 1);
        let r = make_report_full(0, 0, 0, 0, mask, 0);
        let s = parse_ab9_report(&r).unwrap();
        for btn in 1u8..=16 {
            if btn == n {
                assert!(s.buttons.is_pressed(btn), "button {} should be pressed", btn);
            } else {
                assert!(!s.buttons.is_pressed(btn), "button {} should NOT be pressed", btn);
            }
        }
    }
}

#[test]
fn button_out_of_range_returns_false() {
    let b = Ab9Buttons { mask: 0xFFFF, hat: 0 };
    assert!(!b.is_pressed(0), "button 0 is out of range");
    assert!(!b.is_pressed(17), "button 17 is out of range for 16-bit mask");
    assert!(!b.is_pressed(255), "button 255 is out of range");
}

#[test]
fn no_buttons_pressed() {
    let r = make_report_full(0, 0, 0, 0, 0, 0);
    let s = parse_ab9_report(&r).unwrap();
    for n in 1u8..=16 {
        assert!(!s.buttons.is_pressed(n));
    }
}

#[test]
fn all_buttons_pressed() {
    let r = make_report_full(0, 0, 0, 0, 0xFFFF, 0);
    let s = parse_ab9_report(&r).unwrap();
    for n in 1u8..=16 {
        assert!(s.buttons.is_pressed(n));
    }
}

// ── Report length boundary tests ────────────────────────────────────────

#[test]
fn exact_length_report_parses() {
    let r = centred();
    assert_eq!(r.len(), AB9_REPORT_LEN);
    assert!(parse_ab9_report(&r).is_ok());
}

#[test]
fn longer_report_parses_ignoring_extra_bytes() {
    let mut extended = vec![0u8; AB9_REPORT_LEN + 16];
    extended[0] = 0x01;
    extended[AB9_REPORT_LEN] = 0xDE; // extra bytes
    let s = parse_ab9_report(&extended).unwrap();
    assert!(s.axes.roll.abs() < 1e-4);
}

#[test]
fn one_byte_short_fails() {
    let data = vec![0x01u8; AB9_REPORT_LEN - 1];
    let err = parse_ab9_report(&data).unwrap_err();
    assert!(matches!(err, MozaParseError::TooShort { expected: 16, got: 15 }));
}

#[test]
fn empty_report_fails() {
    let err = parse_ab9_report(&[]).unwrap_err();
    assert!(matches!(err, MozaParseError::TooShort { expected: 16, got: 0 }));
}

// ── Report ID tests ────────────────────────────────────────────────────

#[test]
fn report_id_zero_is_rejected() {
    let mut r = centred();
    r[0] = 0x00;
    assert!(matches!(
        parse_ab9_report(&r),
        Err(MozaParseError::UnknownReportId { id: 0x00 })
    ));
}

#[test]
fn report_id_0xff_is_rejected() {
    let mut r = centred();
    r[0] = 0xFF;
    assert!(matches!(
        parse_ab9_report(&r),
        Err(MozaParseError::UnknownReportId { id: 0xFF })
    ));
}

// ── Error display tests ────────────────────────────────────────────────

#[test]
fn error_display_too_short() {
    let err = MozaParseError::TooShort { expected: 16, got: 4 };
    let msg = err.to_string();
    assert!(msg.contains("16"), "should mention expected length");
    assert!(msg.contains("4"), "should mention actual length");
}

#[test]
fn error_display_unknown_id() {
    let err = MozaParseError::UnknownReportId { id: 0xAB };
    let msg = err.to_string();
    assert!(msg.contains("AB"), "should contain hex ID");
}

// ── All-0xFF data test ──────────────────────────────────────────────────

#[test]
fn all_0xff_data_except_report_id() {
    let mut r = [0xFFu8; AB9_REPORT_LEN];
    r[0] = 0x01;
    let s = parse_ab9_report(&r).unwrap();
    // 0xFFFF as i16 = -1 → roll ≈ -0.00003 (very close to 0)
    assert!(s.axes.roll.abs() < 0.01);
    // All buttons on (mask = 0xFFFF)
    assert_eq!(s.buttons.mask, 0xFFFF);
    assert_eq!(s.buttons.hat, 0xFF);
}
