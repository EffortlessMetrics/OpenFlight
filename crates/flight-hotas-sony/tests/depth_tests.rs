// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the `flight-hotas-sony` crate covering HID parsing,
//! device identification, normalization edge cases, and error handling.

use flight_hotas_sony::{
    DUALSHOCK_3_PID, DUALSHOCK_4_V1_PID, DUALSHOCK_4_V2_PID, DUALSENSE_EDGE_PID,
    DUALSENSE_MIN_REPORT_BYTES, DUALSENSE_PID, DS4_MIN_REPORT_BYTES, SONY_VENDOR_ID,
    is_dualsense, is_dualshock, parse_ds4_report, parse_dualsense_report,
};

// ── Helpers ──────────────────────────────────────────────────────────────────

fn build_ds4(
    left_x: u8,
    left_y: u8,
    right_x: u8,
    right_y: u8,
    l2: u8,
    r2: u8,
    buttons_lo: u8,
    buttons_hi: u8,
    buttons_ps: u8,
) -> [u8; 10] {
    [
        0x01, left_x, left_y, right_x, right_y, l2, r2, buttons_lo, buttons_hi, buttons_ps,
    ]
}

fn build_dualsense(
    left_x: u8,
    left_y: u8,
    right_x: u8,
    right_y: u8,
    l2: u8,
    r2: u8,
    buttons_0: u8,
    buttons_1: u8,
    buttons_2: u8,
    hat: u8,
) -> [u8; 11] {
    [
        0x01, left_x, left_y, right_x, right_y, l2, r2, buttons_0, buttons_1, buttons_2, hat,
    ]
}

// ── Device identification ────────────────────────────────────────────────────

#[test]
fn dualshock3_identified() {
    assert!(is_dualshock(SONY_VENDOR_ID, DUALSHOCK_3_PID));
}

#[test]
fn dualshock4_v1_identified() {
    assert!(is_dualshock(SONY_VENDOR_ID, DUALSHOCK_4_V1_PID));
}

#[test]
fn dualshock4_v2_identified() {
    assert!(is_dualshock(SONY_VENDOR_ID, DUALSHOCK_4_V2_PID));
}

#[test]
fn dualsense_identified() {
    assert!(is_dualsense(SONY_VENDOR_ID, DUALSENSE_PID));
}

#[test]
fn dualsense_edge_identified() {
    assert!(is_dualsense(SONY_VENDOR_ID, DUALSENSE_EDGE_PID));
}

#[test]
fn dualshock_rejects_wrong_vid() {
    assert!(!is_dualshock(0x1234, DUALSHOCK_4_V1_PID));
}

#[test]
fn dualsense_rejects_wrong_vid() {
    assert!(!is_dualsense(0x1234, DUALSENSE_PID));
}

#[test]
fn dualshock_does_not_match_dualsense_pid() {
    assert!(!is_dualshock(SONY_VENDOR_ID, DUALSENSE_PID));
    assert!(!is_dualshock(SONY_VENDOR_ID, DUALSENSE_EDGE_PID));
}

#[test]
fn dualsense_does_not_match_dualshock_pid() {
    assert!(!is_dualsense(SONY_VENDOR_ID, DUALSHOCK_3_PID));
    assert!(!is_dualsense(SONY_VENDOR_ID, DUALSHOCK_4_V1_PID));
    assert!(!is_dualsense(SONY_VENDOR_ID, DUALSHOCK_4_V2_PID));
}

#[test]
fn unknown_pid_not_matched() {
    assert!(!is_dualshock(SONY_VENDOR_ID, 0xFFFF));
    assert!(!is_dualsense(SONY_VENDOR_ID, 0xFFFF));
}

// ── DualShock 4 — boundary axis values ───────────────────────────────────────

#[test]
fn ds4_full_right_stick_x() {
    let data = build_ds4(127, 127, 255, 127, 0, 0, 0x08, 0, 0);
    let s = parse_ds4_report(&data).unwrap();
    assert!(s.right_x > 0.99, "right_x at 255: {}", s.right_x);
}

#[test]
fn ds4_full_left_right_stick_x() {
    let data = build_ds4(127, 127, 0, 127, 0, 0, 0x08, 0, 0);
    let s = parse_ds4_report(&data).unwrap();
    assert!(s.right_x < -0.99, "right_x at 0: {}", s.right_x);
}

#[test]
fn ds4_right_stick_y_extremes() {
    let up = build_ds4(127, 127, 127, 0, 0, 0, 0x08, 0, 0);
    let down = build_ds4(127, 127, 127, 255, 0, 0, 0x08, 0, 0);
    let s_up = parse_ds4_report(&up).unwrap();
    let s_down = parse_ds4_report(&down).unwrap();
    assert!(s_up.right_y < -0.99, "right_y up: {}", s_up.right_y);
    assert!(s_down.right_y > 0.99, "right_y down: {}", s_down.right_y);
}

#[test]
fn ds4_r2_half_pressed() {
    let data = build_ds4(127, 127, 127, 127, 0, 128, 0x08, 0, 0);
    let s = parse_ds4_report(&data).unwrap();
    assert!(
        (0.49..=0.52).contains(&s.r2),
        "r2 half pressed: {}",
        s.r2
    );
}

// ── DualShock 4 — button bitmask extraction ──────────────────────────────────

#[test]
fn ds4_ps_button_in_buttons() {
    // PS button is typically bit 0 of byte 9 → bit 16 of buttons mask
    let data = build_ds4(127, 127, 127, 127, 0, 0, 0x08, 0, 0x01);
    let s = parse_ds4_report(&data).unwrap();
    assert_ne!(s.buttons & (1 << 16), 0, "PS button should be in bit 16");
}

#[test]
fn ds4_shoulder_buttons() {
    // L1=bit 0, R1=bit 1 of byte 8 → bits 8..9 of buttons
    let data = build_ds4(127, 127, 127, 127, 0, 0, 0x08, 0x03, 0);
    let s = parse_ds4_report(&data).unwrap();
    assert_ne!(s.buttons & (1 << 8), 0, "L1 bit");
    assert_ne!(s.buttons & (1 << 9), 0, "R1 bit");
}

#[test]
fn ds4_all_buttons_set() {
    let data = build_ds4(127, 127, 127, 127, 0, 0, 0xFF, 0xFF, 0xFF);
    let s = parse_ds4_report(&data).unwrap();
    // All 24 bits of the 3-byte button field should be set
    assert_eq!(s.buttons & 0x00FF_FFFF, 0x00FF_FFFF);
}

// ── DualShock 4 — dpad with high nibble button bits ──────────────────────────

#[test]
fn ds4_dpad_masked_from_high_nibble() {
    // byte 7 = 0xF8 → dpad = low nibble = 8 (released), high nibble = buttons
    let data = build_ds4(127, 127, 127, 127, 0, 0, 0xF8, 0, 0);
    let s = parse_ds4_report(&data).unwrap();
    assert_eq!(s.dpad, 8, "dpad should be 8 (released)");
    // buttons should still contain the full byte 7 including high nibble
    assert_ne!(s.buttons & 0xF0, 0, "high nibble button bits present");
}

// ── DualShock 4 — error handling ─────────────────────────────────────────────

#[test]
fn ds4_exactly_min_length_parses() {
    let data = build_ds4(127, 127, 127, 127, 0, 0, 0x08, 0, 0);
    assert_eq!(data.len(), DS4_MIN_REPORT_BYTES);
    assert!(parse_ds4_report(&data).is_ok());
}

#[test]
fn ds4_extra_trailing_bytes_ok() {
    let mut data = vec![0x01, 127, 127, 127, 127, 0, 0, 0x08, 0, 0];
    data.extend_from_slice(&[0xAA; 54]); // 64-byte report
    let s = parse_ds4_report(&data).unwrap();
    assert!(s.left_x.abs() < 0.01);
}

#[test]
fn ds4_single_byte_error() {
    let err = parse_ds4_report(&[0x01]).unwrap_err();
    assert_eq!(
        err,
        flight_hotas_sony::SonyError::TooShort {
            expected: 10,
            actual: 1
        }
    );
}

// ── DualSense — boundary axis values ─────────────────────────────────────────

#[test]
fn dualsense_full_right_stick() {
    let data = build_dualsense(127, 127, 255, 127, 0, 0, 0, 0, 0, 8);
    let s = parse_dualsense_report(&data).unwrap();
    assert!(s.right_x > 0.99, "right_x: {}", s.right_x);
}

#[test]
fn dualsense_stick_y_down() {
    let data = build_dualsense(127, 255, 127, 255, 0, 0, 0, 0, 0, 8);
    let s = parse_dualsense_report(&data).unwrap();
    assert!(s.left_y > 0.99, "left_y down: {}", s.left_y);
    assert!(s.right_y > 0.99, "right_y down: {}", s.right_y);
}

#[test]
fn dualsense_triggers_quarter_pressed() {
    let data = build_dualsense(127, 127, 127, 127, 64, 64, 0, 0, 0, 8);
    let s = parse_dualsense_report(&data).unwrap();
    assert!(
        (0.24..=0.26).contains(&s.l2),
        "l2 quarter: {}",
        s.l2
    );
    assert!(
        (0.24..=0.26).contains(&s.r2),
        "r2 quarter: {}",
        s.r2
    );
}

// ── DualSense — button bitmask ───────────────────────────────────────────────

#[test]
fn dualsense_buttons_from_three_bytes() {
    let data = build_dualsense(127, 127, 127, 127, 0, 0, 0xAB, 0xCD, 0xEF, 8);
    let s = parse_dualsense_report(&data).unwrap();
    assert_eq!(s.buttons, 0xAB | (0xCD << 8) | (0xEF << 16));
}

#[test]
fn dualsense_all_buttons_set() {
    let data = build_dualsense(127, 127, 127, 127, 0, 0, 0xFF, 0xFF, 0xFF, 8);
    let s = parse_dualsense_report(&data).unwrap();
    assert_eq!(s.buttons & 0x00FF_FFFF, 0x00FF_FFFF);
}

#[test]
fn dualsense_no_buttons() {
    let data = build_dualsense(127, 127, 127, 127, 0, 0, 0, 0, 0, 8);
    let s = parse_dualsense_report(&data).unwrap();
    assert_eq!(s.buttons, 0);
}

// ── DualSense — dpad ─────────────────────────────────────────────────────────

#[test]
fn dualsense_dpad_masked_to_low_nibble() {
    // hat byte = 0xF4 → dpad = low nibble = 4 (South)
    let data = build_dualsense(127, 127, 127, 127, 0, 0, 0, 0, 0, 0xF4);
    let s = parse_dualsense_report(&data).unwrap();
    assert_eq!(s.dpad, 4, "dpad south");
}

#[test]
fn dualsense_dpad_all_directions() {
    for dir in 0u8..=8 {
        let data = build_dualsense(127, 127, 127, 127, 0, 0, 0, 0, 0, dir);
        let s = parse_dualsense_report(&data).unwrap();
        assert_eq!(s.dpad, dir);
    }
}

// ── DualSense — touchpad defaults ────────────────────────────────────────────

#[test]
fn dualsense_touchpad_defaults_to_zero() {
    let data = build_dualsense(127, 127, 127, 127, 0, 0, 0, 0, 0, 8);
    let s = parse_dualsense_report(&data).unwrap();
    assert_eq!(s.touchpad_x, 0.0);
    assert_eq!(s.touchpad_y, 0.0);
}

// ── DualSense — error handling ───────────────────────────────────────────────

#[test]
fn dualsense_exactly_min_length_parses() {
    let data = build_dualsense(127, 127, 127, 127, 0, 0, 0, 0, 0, 8);
    assert_eq!(data.len(), DUALSENSE_MIN_REPORT_BYTES);
    assert!(parse_dualsense_report(&data).is_ok());
}

#[test]
fn dualsense_extra_trailing_bytes_ok() {
    let mut data = vec![0x01, 127, 127, 127, 127, 0, 0, 0, 0, 0, 8];
    data.extend_from_slice(&[0xBB; 53]); // 64-byte extended report
    let s = parse_dualsense_report(&data).unwrap();
    assert!(s.left_x.abs() < 0.01);
}

#[test]
fn dualsense_single_byte_error() {
    let err = parse_dualsense_report(&[0x01]).unwrap_err();
    assert_eq!(
        err,
        flight_hotas_sony::SonyError::TooShort {
            expected: 11,
            actual: 1,
        }
    );
}

// ── Cross-report consistency ─────────────────────────────────────────────────

#[test]
fn ds4_and_dualsense_center_agree() {
    let ds4 = parse_ds4_report(&build_ds4(127, 127, 127, 127, 0, 0, 0x08, 0, 0)).unwrap();
    let ds = parse_dualsense_report(&build_dualsense(127, 127, 127, 127, 0, 0, 0, 0, 0, 8)).unwrap();
    assert!(
        (ds4.left_x - ds.left_x).abs() < 0.001,
        "DS4 left_x {} vs DS left_x {}",
        ds4.left_x,
        ds.left_x
    );
    assert!(
        (ds4.left_y - ds.left_y).abs() < 0.001,
        "DS4 left_y {} vs DS left_y {}",
        ds4.left_y,
        ds.left_y
    );
}

#[test]
fn ds4_and_dualsense_extremes_agree() {
    let ds4 = parse_ds4_report(&build_ds4(0, 255, 255, 0, 255, 0, 0x08, 0, 0)).unwrap();
    let ds = parse_dualsense_report(&build_dualsense(0, 255, 255, 0, 255, 0, 0, 0, 0, 8)).unwrap();
    assert!((ds4.left_x - ds.left_x).abs() < 0.001);
    assert!((ds4.left_y - ds.left_y).abs() < 0.001);
    assert!((ds4.right_x - ds.right_x).abs() < 0.001);
    assert!((ds4.right_y - ds.right_y).abs() < 0.001);
    assert!((ds4.l2 - ds.l2).abs() < 0.001);
    assert!((ds4.r2 - ds.r2).abs() < 0.001);
}

// ── Constants sanity ─────────────────────────────────────────────────────────

#[test]
fn sony_vendor_id_correct() {
    assert_eq!(SONY_VENDOR_ID, 0x054C);
}

#[test]
fn report_min_lengths_are_positive() {
    assert!(DS4_MIN_REPORT_BYTES > 0);
    assert!(DUALSENSE_MIN_REPORT_BYTES > 0);
    assert!(DUALSENSE_MIN_REPORT_BYTES > DS4_MIN_REPORT_BYTES);
}

// ── Property-based tests ─────────────────────────────────────────────────────

mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn ds4_axes_never_nan(
            lx in 0u8..=255,
            ly in 0u8..=255,
            rx in 0u8..=255,
            ry in 0u8..=255,
            l2 in 0u8..=255,
            r2 in 0u8..=255,
        ) {
            let data = [0x01, lx, ly, rx, ry, l2, r2, 0x08, 0, 0];
            let s = parse_ds4_report(&data).unwrap();
            prop_assert!(!s.left_x.is_nan());
            prop_assert!(!s.left_y.is_nan());
            prop_assert!(!s.right_x.is_nan());
            prop_assert!(!s.right_y.is_nan());
            prop_assert!(!s.l2.is_nan());
            prop_assert!(!s.r2.is_nan());
        }

        #[test]
        fn ds4_trigger_monotonic(a in 0u8..=254) {
            let lo = [0x01u8, 127, 127, 127, 127, a, 0, 0x08, 0, 0];
            let hi = [0x01u8, 127, 127, 127, 127, a + 1, 0, 0x08, 0, 0];
            let s_lo = parse_ds4_report(&lo).unwrap();
            let s_hi = parse_ds4_report(&hi).unwrap();
            prop_assert!(s_hi.l2 >= s_lo.l2);
        }

        #[test]
        fn dualsense_trigger_monotonic(a in 0u8..=254) {
            let lo = [0x01u8, 127, 127, 127, 127, a, 0, 0, 0, 0, 8];
            let hi = [0x01u8, 127, 127, 127, 127, a + 1, 0, 0, 0, 0, 8];
            let s_lo = parse_dualsense_report(&lo).unwrap();
            let s_hi = parse_dualsense_report(&hi).unwrap();
            prop_assert!(s_hi.l2 >= s_lo.l2);
        }

        #[test]
        fn dualsense_dpad_always_valid(hat in 0u8..=255) {
            let data = [0x01u8, 127, 127, 127, 127, 0, 0, 0, 0, 0, hat];
            let s = parse_dualsense_report(&data).unwrap();
            prop_assert!(s.dpad <= 0x0F, "dpad masked to low nibble: {}", s.dpad);
        }

        #[test]
        fn ds4_stick_monotonic_x(a in 0u8..=254) {
            let lo = [0x01u8, a, 127, 127, 127, 0, 0, 0x08, 0, 0];
            let hi = [0x01u8, a + 1, 127, 127, 127, 0, 0, 0x08, 0, 0];
            let s_lo = parse_ds4_report(&lo).unwrap();
            let s_hi = parse_ds4_report(&hi).unwrap();
            prop_assert!(s_hi.left_x >= s_lo.left_x);
        }

        #[test]
        fn identification_mutual_exclusion(pid in 0u16..=0xFFFF) {
            // A Sony PID cannot be both DualShock and DualSense
            let ds = is_dualshock(SONY_VENDOR_ID, pid);
            let sense = is_dualsense(SONY_VENDOR_ID, pid);
            prop_assert!(!(ds && sense), "PID {pid:#06x} matched both");
        }
    }
}
