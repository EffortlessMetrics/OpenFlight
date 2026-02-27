// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests for VPforce Rhino HID report parsing (flight-hotas-vpforce).
//!
//! These tests exercise the public `parse_rhino_report` API along with device
//! detection helpers re-exported from `flight-hid-support`.

use flight_hotas_vpforce::rhino::{RHINO_MIN_REPORT_BYTES, RhinoParseError, parse_rhino_report};
use flight_hotas_vpforce::{
    VPFORCE_RHINO_PID_V2, VPFORCE_RHINO_PID_V3, VPFORCE_VENDOR_ID, VpforceModel, is_vpforce_device,
    vpforce_model,
};

// ──────────────────────────────────────────────────────────────────────────────
// Report builder helpers
// ──────────────────────────────────────────────────────────────────────────────

fn centred() -> [u8; RHINO_MIN_REPORT_BYTES] {
    let mut r = [0u8; RHINO_MIN_REPORT_BYTES];
    r[0] = 0x01;
    r[17] = 0xFF; // hat centred
    r
}

fn make_full(axes: [i16; 6], buttons: u32, hat: u8) -> [u8; RHINO_MIN_REPORT_BYTES] {
    let mut r = [0u8; RHINO_MIN_REPORT_BYTES];
    r[0] = 0x01;
    for (i, &ax) in axes.iter().enumerate() {
        let off = 1 + i * 2;
        r[off..off + 2].copy_from_slice(&ax.to_le_bytes());
    }
    r[13..17].copy_from_slice(&buttons.to_le_bytes());
    r[17] = hat;
    r
}

// ──────────────────────────────────────────────────────────────────────────────
// Axis parsing
// ──────────────────────────────────────────────────────────────────────────────

/// All axes are zero (0.0 / 0.5 for throttle) for a centred report.
#[test]
fn centred_report_all_axes_zero() {
    let s = parse_rhino_report(&centred()).unwrap();
    assert!(s.axes.roll.abs() < 1e-4, "roll={}", s.axes.roll);
    assert!(s.axes.pitch.abs() < 1e-4, "pitch={}", s.axes.pitch);
    assert!(
        (s.axes.throttle - 0.5).abs() < 1e-3,
        "throttle={}",
        s.axes.throttle
    );
    assert!(s.axes.twist.abs() < 1e-4, "twist={}", s.axes.twist);
    assert!(s.axes.rocker.abs() < 1e-4, "rocker={}", s.axes.rocker);
}

/// Full positive X deflection → roll = 1.0.
#[test]
fn full_x_deflection_gives_roll_one() {
    let r = make_full([i16::MAX, 0, 0, 0, 0, 0], 0, 0xFF);
    let s = parse_rhino_report(&r).unwrap();
    assert!((s.axes.roll - 1.0).abs() < 1e-4, "roll={}", s.axes.roll);
}

/// Full negative Y deflection → pitch = −1.0 (i16::MIN clamps to −1.0).
#[test]
fn full_negative_y_deflection_gives_pitch_minus_one() {
    let r = make_full([0, i16::MIN, 0, 0, 0, 0], 0, 0xFF);
    let s = parse_rhino_report(&r).unwrap();
    assert!((s.axes.pitch + 1.0).abs() < 1e-3, "pitch={}", s.axes.pitch);
}

/// Throttle (Z axis) is remapped from [−32768..32767] to [0.0..1.0].
#[test]
fn throttle_min_maps_to_zero_max_maps_to_one() {
    let r_min = make_full([0, 0, i16::MIN, 0, 0, 0], 0, 0xFF);
    let s_min = parse_rhino_report(&r_min).unwrap();
    assert!(
        s_min.axes.throttle < 0.01,
        "min throttle={}",
        s_min.axes.throttle
    );

    let r_max = make_full([0, 0, i16::MAX, 0, 0, 0], 0, 0xFF);
    let s_max = parse_rhino_report(&r_max).unwrap();
    assert!(
        s_max.axes.throttle > 0.99,
        "max throttle={}",
        s_max.axes.throttle
    );
}

/// The Ry (auxiliary) axis is present in the axes struct.
#[test]
fn ry_aux_axis_is_parsed() {
    // Ry is bytes 9–10 (axis index 4 in the 6-element array)
    let r = make_full([0, 0, 0, 0, i16::MAX, 0], 0, 0xFF);
    let s = parse_rhino_report(&r).unwrap();
    assert!((s.axes.ry - 1.0).abs() < 1e-4, "ry={}", s.axes.ry);
}

// ──────────────────────────────────────────────────────────────────────────────
// Button handling
// ──────────────────────────────────────────────────────────────────────────────

/// Parsing the same button mask twice gives identical results (idempotent).
#[test]
fn button_bitmask_decode_is_idempotent() {
    let mask: u32 = 0b1010_1010_1010_1010;
    let r = make_full([0i16; 6], mask, 0xFF);
    let s1 = parse_rhino_report(&r).unwrap();
    let s2 = parse_rhino_report(&r).unwrap();
    assert_eq!(s1.buttons.mask, s2.buttons.mask);
    for n in 1u8..=32 {
        assert_eq!(s1.buttons.is_pressed(n), s2.buttons.is_pressed(n));
    }
}

/// `pressed()` returns exactly the buttons whose bits are set in the mask.
#[test]
fn pressed_buttons_helper_matches_mask() {
    let mask: u32 = (1 << 0) | (1 << 4) | (1 << 31); // buttons 1, 5, 32
    let r = make_full([0i16; 6], mask, 0xFF);
    let s = parse_rhino_report(&r).unwrap();
    let pressed = s.buttons.pressed();
    assert!(pressed.contains(&1), "button 1 should be in pressed()");
    assert!(pressed.contains(&5), "button 5 should be in pressed()");
    assert!(pressed.contains(&32), "button 32 should be in pressed()");
    assert_eq!(pressed.len(), 3, "exactly 3 buttons should be pressed");
}

/// Hat switch value is preserved exactly.
#[test]
fn hat_switch_value_preserved() {
    for &hat in &[0u8, 1, 2, 3, 4, 5, 6, 7, 0xFF] {
        let r = make_full([0i16; 6], 0, hat);
        let s = parse_rhino_report(&r).unwrap();
        assert_eq!(s.buttons.hat, hat, "hat={hat}");
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Error handling
// ──────────────────────────────────────────────────────────────────────────────

/// Reports shorter than RHINO_MIN_REPORT_BYTES always return TooShort.
#[test]
fn short_report_gives_too_short_error() {
    let err = parse_rhino_report(&[0x01; 19]).unwrap_err();
    assert!(
        matches!(err, RhinoParseError::TooShort { .. }),
        "expected TooShort, got {err:?}"
    );
}

/// An empty slice returns TooShort.
#[test]
fn empty_slice_gives_too_short_error() {
    assert!(matches!(
        parse_rhino_report(&[]),
        Err(RhinoParseError::TooShort { .. })
    ));
}

/// Report ID ≠ 0x01 returns UnknownReportId.
#[test]
fn wrong_report_id_gives_error() {
    let mut r = [0u8; RHINO_MIN_REPORT_BYTES];
    r[0] = 0x02;
    assert!(matches!(
        parse_rhino_report(&r),
        Err(RhinoParseError::UnknownReportId { id: 0x02 })
    ));
}

// ──────────────────────────────────────────────────────────────────────────────
// Device detection
// ──────────────────────────────────────────────────────────────────────────────

/// `is_vpforce_device` accepts both Rhino revisions and rejects foreign VID/PIDs.
#[test]
fn device_detection_accepts_known_vid_pid() {
    assert!(is_vpforce_device(VPFORCE_VENDOR_ID, VPFORCE_RHINO_PID_V2));
    assert!(is_vpforce_device(VPFORCE_VENDOR_ID, VPFORCE_RHINO_PID_V3));
    assert!(!is_vpforce_device(0x1234, VPFORCE_RHINO_PID_V2));
    assert!(!is_vpforce_device(VPFORCE_VENDOR_ID, 0xFFFF));
}

/// `vpforce_model` correctly identifies v2 vs v3 and returns None for unknowns.
#[test]
fn vpforce_model_identifies_v2_and_v3() {
    assert_eq!(
        vpforce_model(VPFORCE_RHINO_PID_V2),
        Some(VpforceModel::RhinoV2)
    );
    assert_eq!(
        vpforce_model(VPFORCE_RHINO_PID_V3),
        Some(VpforceModel::RhinoV3)
    );
    assert_eq!(vpforce_model(0xFFFF), None);
}

/// Model name strings are non-empty and mention "Rhino".
#[test]
fn vpforce_model_names_mention_rhino() {
    assert!(VpforceModel::RhinoV2.name().contains("Rhino"));
    assert!(VpforceModel::RhinoV3.name().contains("Rhino"));
}
