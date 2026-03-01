// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for VPforce Rhino HID report parsing — covers boundary values,
//! individual axis extremes, hat-switch states, rocker axis, and malformed data.

use flight_ffb_vpforce::input::{
    RHINO_REPORT_LEN, RhinoButtons, RhinoParseError, parse_report,
};

fn make_report_full(
    roll: i16,
    pitch: i16,
    throttle: i16,
    rocker: i16,
    twist: i16,
    button_mask: u32,
    hat: u8,
) -> [u8; RHINO_REPORT_LEN] {
    let mut r = [0u8; RHINO_REPORT_LEN];
    r[0] = 0x01;
    r[1..3].copy_from_slice(&roll.to_le_bytes());
    r[3..5].copy_from_slice(&pitch.to_le_bytes());
    r[5..7].copy_from_slice(&throttle.to_le_bytes());
    r[7..9].copy_from_slice(&rocker.to_le_bytes());
    // bytes 9-10: Ry (unused)
    r[11..13].copy_from_slice(&twist.to_le_bytes());
    r[13..17].copy_from_slice(&button_mask.to_le_bytes());
    r[17] = hat;
    r
}

fn centred() -> [u8; RHINO_REPORT_LEN] {
    make_report_full(0, 0, 0, 0, 0, 0, 0xFF)
}

// ── Axis boundary tests ─────────────────────────────────────────────────

#[test]
fn negative_full_deflection_roll() {
    let r = make_report_full(i16::MIN, 0, 0, 0, 0, 0, 0);
    let s = parse_report(&r).unwrap();
    assert!(s.axes.roll <= -0.99, "roll should be ≈−1.0, got {}", s.axes.roll);
    assert!(s.axes.roll >= -1.0);
}

#[test]
fn negative_full_deflection_pitch() {
    let r = make_report_full(0, i16::MIN, 0, 0, 0, 0, 0);
    let s = parse_report(&r).unwrap();
    assert!(s.axes.pitch <= -0.99);
    assert!(s.axes.pitch >= -1.0);
}

#[test]
fn positive_full_deflection_roll() {
    let r = make_report_full(i16::MAX, 0, 0, 0, 0, 0, 0);
    let s = parse_report(&r).unwrap();
    assert!((s.axes.roll - 1.0).abs() < 1e-4);
}

#[test]
fn twist_full_positive_and_negative() {
    let r_pos = make_report_full(0, 0, 0, 0, i16::MAX, 0, 0);
    let s_pos = parse_report(&r_pos).unwrap();
    assert!((s_pos.axes.twist - 1.0).abs() < 1e-4);

    let r_neg = make_report_full(0, 0, 0, 0, i16::MIN, 0, 0);
    let s_neg = parse_report(&r_neg).unwrap();
    assert!(s_neg.axes.twist <= -0.99);
}

#[test]
fn rocker_full_positive_and_negative() {
    let r_pos = make_report_full(0, 0, 0, i16::MAX, 0, 0, 0);
    let s_pos = parse_report(&r_pos).unwrap();
    assert!((s_pos.axes.rocker - 1.0).abs() < 1e-4);

    let r_neg = make_report_full(0, 0, 0, i16::MIN, 0, 0, 0);
    let s_neg = parse_report(&r_neg).unwrap();
    assert!(s_neg.axes.rocker <= -0.99);
}

#[test]
fn small_positive_axis_value() {
    let r = make_report_full(1, 0, 0, 0, 0, 0, 0);
    let s = parse_report(&r).unwrap();
    assert!(s.axes.roll > 0.0, "single LSB should be slightly positive");
    assert!(s.axes.roll < 0.001);
}

#[test]
fn small_negative_axis_value() {
    let r = make_report_full(-1, 0, 0, 0, 0, 0, 0);
    let s = parse_report(&r).unwrap();
    assert!(s.axes.roll < 0.0);
    assert!(s.axes.roll > -0.001);
}

#[test]
fn throttle_at_midpoint_is_half() {
    let r = centred();
    let s = parse_report(&r).unwrap();
    assert!(
        (s.axes.throttle - 0.5).abs() < 1e-3,
        "zero raw → 0.5 normalised, got {}",
        s.axes.throttle
    );
}

#[test]
fn throttle_minimum_and_maximum() {
    let r_min = make_report_full(0, 0, i16::MIN, 0, 0, 0, 0);
    let s_min = parse_report(&r_min).unwrap();
    assert!(s_min.axes.throttle < 0.01, "min throttle should be ≈0.0");

    let r_max = make_report_full(0, 0, i16::MAX, 0, 0, 0, 0);
    let s_max = parse_report(&r_max).unwrap();
    assert!(s_max.axes.throttle > 0.99, "max throttle should be ≈1.0");
}

#[test]
fn all_axes_at_positive_extreme() {
    let r = make_report_full(i16::MAX, i16::MAX, i16::MAX, i16::MAX, i16::MAX, 0, 0);
    let s = parse_report(&r).unwrap();
    assert!((s.axes.roll - 1.0).abs() < 1e-4);
    assert!((s.axes.pitch - 1.0).abs() < 1e-4);
    assert!(s.axes.throttle > 0.99);
    assert!((s.axes.rocker - 1.0).abs() < 1e-4);
    assert!((s.axes.twist - 1.0).abs() < 1e-4);
}

#[test]
fn all_axes_at_negative_extreme() {
    let r = make_report_full(i16::MIN, i16::MIN, i16::MIN, i16::MIN, i16::MIN, 0, 0);
    let s = parse_report(&r).unwrap();
    assert!(s.axes.roll >= -1.0 && s.axes.roll <= -0.99);
    assert!(s.axes.pitch >= -1.0 && s.axes.pitch <= -0.99);
    assert!(s.axes.throttle < 0.01);
    assert!(s.axes.rocker >= -1.0 && s.axes.rocker <= -0.99);
    assert!(s.axes.twist >= -1.0 && s.axes.twist <= -0.99);
}

// ── Hat switch tests ────────────────────────────────────────────────────

#[test]
fn hat_switch_all_eight_directions() {
    for hat_val in 0..=7 {
        let r = make_report_full(0, 0, 0, 0, 0, 0, hat_val);
        let s = parse_report(&r).unwrap();
        assert_eq!(s.buttons.hat, hat_val, "hat direction {} not preserved", hat_val);
    }
}

#[test]
fn hat_switch_centred_is_0xff() {
    let r = make_report_full(0, 0, 0, 0, 0, 0, 0xFF);
    let s = parse_report(&r).unwrap();
    assert_eq!(s.buttons.hat, 0xFF);
}

// ── Button tests ────────────────────────────────────────────────────────

#[test]
fn each_button_individually() {
    for n in 1u8..=32 {
        let mask = 1u32 << (n - 1);
        let r = make_report_full(0, 0, 0, 0, 0, mask, 0);
        let s = parse_report(&r).unwrap();
        for btn in 1u8..=32 {
            if btn == n {
                assert!(s.buttons.is_pressed(btn), "button {} should be pressed", btn);
            } else {
                assert!(!s.buttons.is_pressed(btn), "button {} should NOT be pressed when only {} is set", btn, n);
            }
        }
    }
}

#[test]
fn button_out_of_range_returns_false() {
    let b = RhinoButtons { mask: u32::MAX, hat: 0 };
    assert!(!b.is_pressed(0), "button 0 is out of range");
    assert!(!b.is_pressed(33), "button 33 is out of range for 32-bit mask");
    assert!(!b.is_pressed(255), "button 255 is out of range");
}

#[test]
fn no_buttons_pressed() {
    let r = make_report_full(0, 0, 0, 0, 0, 0, 0);
    let s = parse_report(&r).unwrap();
    for n in 1u8..=32 {
        assert!(!s.buttons.is_pressed(n));
    }
}

#[test]
fn all_buttons_pressed() {
    let r = make_report_full(0, 0, 0, 0, 0, u32::MAX, 0);
    let s = parse_report(&r).unwrap();
    for n in 1u8..=32 {
        assert!(s.buttons.is_pressed(n));
    }
}

#[test]
fn high_buttons_16_to_32() {
    // Only upper 16 buttons set
    let mask = 0xFFFF_0000u32;
    let r = make_report_full(0, 0, 0, 0, 0, mask, 0);
    let s = parse_report(&r).unwrap();
    for n in 1u8..=16 {
        assert!(!s.buttons.is_pressed(n), "button {} should NOT be pressed", n);
    }
    for n in 17u8..=32 {
        assert!(s.buttons.is_pressed(n), "button {} should be pressed", n);
    }
}

// ── Report length boundary tests ────────────────────────────────────────

#[test]
fn exact_length_report_parses() {
    let r = centred();
    assert_eq!(r.len(), RHINO_REPORT_LEN);
    assert!(parse_report(&r).is_ok());
}

#[test]
fn longer_report_parses_ignoring_extra_bytes() {
    let mut extended = vec![0u8; RHINO_REPORT_LEN + 16];
    extended[0] = 0x01;
    extended[RHINO_REPORT_LEN] = 0xDE; // extra data
    let s = parse_report(&extended).unwrap();
    assert!(s.axes.roll.abs() < 1e-4);
}

#[test]
fn one_byte_short_fails() {
    let data = vec![0x01u8; RHINO_REPORT_LEN - 1];
    let err = parse_report(&data).unwrap_err();
    assert!(matches!(err, RhinoParseError::TooShort { expected: 20, got: 19 }));
}

#[test]
fn empty_report_fails() {
    let err = parse_report(&[]).unwrap_err();
    assert!(matches!(err, RhinoParseError::TooShort { expected: 20, got: 0 }));
}

// ── Report ID tests ────────────────────────────────────────────────────

#[test]
fn report_id_zero_is_rejected() {
    let mut r = centred();
    r[0] = 0x00;
    assert!(matches!(
        parse_report(&r),
        Err(RhinoParseError::UnknownReportId { id: 0x00 })
    ));
}

#[test]
fn report_id_0xff_is_rejected() {
    let mut r = centred();
    r[0] = 0xFF;
    assert!(matches!(
        parse_report(&r),
        Err(RhinoParseError::UnknownReportId { id: 0xFF })
    ));
}

// ── Error display tests ────────────────────────────────────────────────

#[test]
fn error_display_too_short() {
    let err = RhinoParseError::TooShort { expected: 20, got: 5 };
    let msg = err.to_string();
    assert!(msg.contains("20"), "should mention expected length");
    assert!(msg.contains("5"), "should mention actual length");
}

#[test]
fn error_display_unknown_id() {
    let err = RhinoParseError::UnknownReportId { id: 0xCD };
    let msg = err.to_string();
    assert!(msg.contains("CD"), "should contain hex ID");
}

// ── All-0xFF data test ──────────────────────────────────────────────────

#[test]
fn all_0xff_data_except_report_id() {
    let mut r = [0xFFu8; RHINO_REPORT_LEN];
    r[0] = 0x01;
    let s = parse_report(&r).unwrap();
    // 0xFFFF as i16 = -1 → normalised ≈ −0.00003
    assert!(s.axes.roll.abs() < 0.01);
    // Button mask = 0xFFFFFFFF → all pressed
    assert_eq!(s.buttons.mask, u32::MAX);
    assert_eq!(s.buttons.hat, 0xFF);
}

// ── Axis independence ───────────────────────────────────────────────────

#[test]
fn changing_roll_does_not_affect_pitch() {
    let r1 = make_report_full(0, 1000, 0, 0, 0, 0, 0);
    let r2 = make_report_full(i16::MAX, 1000, 0, 0, 0, 0, 0);
    let s1 = parse_report(&r1).unwrap();
    let s2 = parse_report(&r2).unwrap();
    assert!((s1.axes.pitch - s2.axes.pitch).abs() < 1e-6, "pitch should be independent of roll");
}

#[test]
fn changing_throttle_does_not_affect_twist() {
    let r1 = make_report_full(0, 0, 0, 0, 5000, 0, 0);
    let r2 = make_report_full(0, 0, i16::MAX, 0, 5000, 0, 0);
    let s1 = parse_report(&r1).unwrap();
    let s2 = parse_report(&r2).unwrap();
    assert!((s1.axes.twist - s2.axes.twist).abs() < 1e-6, "twist should be independent of throttle");
}
