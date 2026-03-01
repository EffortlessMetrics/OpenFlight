// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the `flight-hotas-vpforce` crate.
//!
//! Covers HID input parsing, axis normalisation, button/hat handling,
//! error paths, device detection, and cross-module invariants.

use flight_hotas_vpforce::rhino::{
    RHINO_MIN_REPORT_BYTES, RhinoAxes, RhinoButtons, RhinoParseError,
    parse_rhino_report,
};
use flight_hotas_vpforce::{
    VPFORCE_RHINO_PID_V2, VPFORCE_RHINO_PID_V3, VPFORCE_VENDOR_ID, VpforceModel,
    is_vpforce_device, vpforce_model,
};

// ──────────────────────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────────────────────

fn centred() -> [u8; RHINO_MIN_REPORT_BYTES] {
    let mut r = [0u8; RHINO_MIN_REPORT_BYTES];
    r[0] = 0x01;
    r[17] = 0xFF;
    r
}

fn with_axes(axes: [i16; 6]) -> [u8; RHINO_MIN_REPORT_BYTES] {
    let mut r = centred();
    for (i, &ax) in axes.iter().enumerate() {
        let off = 1 + i * 2;
        r[off..off + 2].copy_from_slice(&ax.to_le_bytes());
    }
    r
}

fn with_buttons(mask: u32, hat: u8) -> [u8; RHINO_MIN_REPORT_BYTES] {
    let mut r = centred();
    r[13..17].copy_from_slice(&mask.to_le_bytes());
    r[17] = hat;
    r
}

// ──────────────────────────────────────────────────────────────────────────────
// §1 — Axis Parsing: Full Range
// ──────────────────────────────────────────────────────────────────────────────

/// Roll full positive → 1.0.
#[test]
fn roll_full_positive() {
    let s = parse_rhino_report(&with_axes([i16::MAX, 0, 0, 0, 0, 0])).unwrap();
    assert!((s.axes.roll - 1.0).abs() < 1e-4);
}

/// Roll full negative → −1.0.
#[test]
fn roll_full_negative() {
    let s = parse_rhino_report(&with_axes([i16::MIN, 0, 0, 0, 0, 0])).unwrap();
    assert!((s.axes.roll + 1.0).abs() < 1e-3);
}

/// Pitch full positive → 1.0.
#[test]
fn pitch_full_positive() {
    let s = parse_rhino_report(&with_axes([0, i16::MAX, 0, 0, 0, 0])).unwrap();
    assert!((s.axes.pitch - 1.0).abs() < 1e-4);
}

/// Throttle minimum → 0.0.
#[test]
fn throttle_minimum() {
    let s = parse_rhino_report(&with_axes([0, 0, i16::MIN, 0, 0, 0])).unwrap();
    assert!(s.axes.throttle < 0.01, "throttle={}", s.axes.throttle);
}

/// Throttle maximum → 1.0.
#[test]
fn throttle_maximum() {
    let s = parse_rhino_report(&with_axes([0, 0, i16::MAX, 0, 0, 0])).unwrap();
    assert!(s.axes.throttle > 0.99, "throttle={}", s.axes.throttle);
}

/// Throttle centred → 0.5.
#[test]
fn throttle_centred() {
    let s = parse_rhino_report(&with_axes([0, 0, 0, 0, 0, 0])).unwrap();
    assert!((s.axes.throttle - 0.5).abs() < 1e-3);
}

/// Rocker full positive → 1.0.
#[test]
fn rocker_full_positive() {
    let s = parse_rhino_report(&with_axes([0, 0, 0, i16::MAX, 0, 0])).unwrap();
    assert!((s.axes.rocker - 1.0).abs() < 1e-4);
}

/// Twist full negative → −1.0.
#[test]
fn twist_full_negative() {
    let s = parse_rhino_report(&with_axes([0, 0, 0, 0, 0, i16::MIN])).unwrap();
    assert!((s.axes.twist + 1.0).abs() < 1e-3);
}

/// Ry auxiliary axis is parsed.
#[test]
fn ry_axis_parsed() {
    let s = parse_rhino_report(&with_axes([0, 0, 0, 0, i16::MAX, 0])).unwrap();
    assert!((s.axes.ry - 1.0).abs() < 1e-4);
}

// ──────────────────────────────────────────────────────────────────────────────
// §2 — Axis Parsing: Linearity & Independence
// ──────────────────────────────────────────────────────────────────────────────

/// Quarter-deflection ≈ 0.25 (linearity check).
#[test]
fn axis_linearity_quarter() {
    let quarter = (32767.0 * 0.25) as i16;
    let s = parse_rhino_report(&with_axes([quarter, 0, 0, 0, 0, 0])).unwrap();
    assert!((s.axes.roll - 0.25).abs() < 0.01, "roll={}", s.axes.roll);
}

/// Three-quarter deflection ≈ 0.75.
#[test]
fn axis_linearity_three_quarter() {
    let tq = (32767.0 * 0.75) as i16;
    let s = parse_rhino_report(&with_axes([tq, 0, 0, 0, 0, 0])).unwrap();
    assert!((s.axes.roll - 0.75).abs() < 0.01, "roll={}", s.axes.roll);
}

/// Setting one axis doesn't affect others.
#[test]
fn axes_independent_roll_only() {
    let s = parse_rhino_report(&with_axes([i16::MAX, 0, 0, 0, 0, 0])).unwrap();
    assert!(s.axes.pitch.abs() < 1e-4);
    assert!(s.axes.rocker.abs() < 1e-4);
    assert!(s.axes.twist.abs() < 1e-4);
    assert!(s.axes.ry.abs() < 1e-4);
}

/// Setting all axes to max doesn't cross-contaminate.
#[test]
fn all_axes_max_simultaneously() {
    let s = parse_rhino_report(&with_axes([i16::MAX; 6])).unwrap();
    assert!((s.axes.roll - 1.0).abs() < 1e-4);
    assert!((s.axes.pitch - 1.0).abs() < 1e-4);
    assert!(s.axes.throttle > 0.99);
    assert!((s.axes.rocker - 1.0).abs() < 1e-4);
    assert!((s.axes.twist - 1.0).abs() < 1e-4);
    assert!((s.axes.ry - 1.0).abs() < 1e-4);
}

/// Positive and negative deflections are symmetric.
#[test]
fn axis_symmetry() {
    let val = 20000i16;
    let s_pos = parse_rhino_report(&with_axes([val, 0, 0, 0, 0, 0])).unwrap();
    let s_neg = parse_rhino_report(&with_axes([-val, 0, 0, 0, 0, 0])).unwrap();
    assert!((s_pos.axes.roll + s_neg.axes.roll).abs() < 1e-3);
}

// ──────────────────────────────────────────────────────────────────────────────
// §3 — Button Handling
// ──────────────────────────────────────────────────────────────────────────────

/// Individual button isolation (only the target bit is set).
#[test]
fn button_isolation_each_bit() {
    for n in 1u8..=32 {
        let mask = 1u32 << (n - 1);
        let s = parse_rhino_report(&with_buttons(mask, 0xFF)).unwrap();
        assert!(s.buttons.is_pressed(n), "button {n} should be pressed");
        // Adjacent buttons should NOT be pressed
        if n > 1 {
            assert!(!s.buttons.is_pressed(n - 1), "button {} false positive", n - 1);
        }
        if n < 32 {
            assert!(!s.buttons.is_pressed(n + 1), "button {} false positive", n + 1);
        }
    }
}

/// pressed() returns all 32 when mask is u32::MAX.
#[test]
fn pressed_all_32_buttons() {
    let s = parse_rhino_report(&with_buttons(u32::MAX, 0xFF)).unwrap();
    let pressed = s.buttons.pressed();
    assert_eq!(pressed.len(), 32);
    for n in 1u8..=32 {
        assert!(pressed.contains(&n));
    }
}

/// pressed() returns empty vec when no buttons pressed.
#[test]
fn pressed_empty_when_none() {
    let s = parse_rhino_report(&with_buttons(0, 0xFF)).unwrap();
    assert!(s.buttons.pressed().is_empty());
}

/// Button 0 and 33+ are always false regardless of mask.
#[test]
fn out_of_range_buttons_always_false() {
    let s = parse_rhino_report(&with_buttons(u32::MAX, 0xFF)).unwrap();
    assert!(!s.buttons.is_pressed(0));
    assert!(!s.buttons.is_pressed(33));
    assert!(!s.buttons.is_pressed(255));
}

/// Alternating button pattern (0xAAAAAAAA).
#[test]
fn alternating_button_pattern() {
    let mask = 0xAAAA_AAAAu32;
    let s = parse_rhino_report(&with_buttons(mask, 0xFF)).unwrap();
    for n in 1u8..=32 {
        let expected = (mask >> (n - 1)) & 1 == 1;
        assert_eq!(s.buttons.is_pressed(n), expected, "button {n}");
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// §4 — Hat Switch
// ──────────────────────────────────────────────────────────────────────────────

/// All 8 compass directions (0..7) are preserved.
#[test]
fn hat_all_directions() {
    for dir in 0u8..=7 {
        let s = parse_rhino_report(&with_buttons(0, dir)).unwrap();
        assert_eq!(s.buttons.hat, dir);
    }
}

/// Hat centred (0xFF) is distinct from direction 0 (North).
#[test]
fn hat_centred_distinct_from_north() {
    let s_n = parse_rhino_report(&with_buttons(0, 0)).unwrap();
    let s_c = parse_rhino_report(&with_buttons(0, 0xFF)).unwrap();
    assert_ne!(s_n.buttons.hat, s_c.buttons.hat);
}

// ──────────────────────────────────────────────────────────────────────────────
// §5 — Error Paths
// ──────────────────────────────────────────────────────────────────────────────

/// Empty slice → TooShort.
#[test]
fn error_empty_slice() {
    assert!(matches!(
        parse_rhino_report(&[]),
        Err(RhinoParseError::TooShort { expected: 20, got: 0 })
    ));
}

/// 19 bytes → TooShort.
#[test]
fn error_19_bytes() {
    assert!(matches!(
        parse_rhino_report(&[0x01; 19]),
        Err(RhinoParseError::TooShort { expected: 20, got: 19 })
    ));
}

/// Report ID 0x00 → UnknownReportId.
#[test]
fn error_report_id_zero() {
    let mut r = [0u8; RHINO_MIN_REPORT_BYTES];
    r[0] = 0x00;
    assert!(matches!(
        parse_rhino_report(&r),
        Err(RhinoParseError::UnknownReportId { id: 0x00 })
    ));
}

/// Report ID 0xFF → UnknownReportId.
#[test]
fn error_report_id_0xff() {
    let mut r = [0u8; RHINO_MIN_REPORT_BYTES];
    r[0] = 0xFF;
    assert!(matches!(
        parse_rhino_report(&r),
        Err(RhinoParseError::UnknownReportId { id: 0xFF })
    ));
}

/// Error Display strings contain useful context.
#[test]
fn error_display_contains_context() {
    let err_short = parse_rhino_report(&[0x01; 5]).unwrap_err();
    let msg = err_short.to_string();
    assert!(msg.contains("20"), "should mention expected length");
    assert!(msg.contains('5'), "should mention actual length");

    let mut r = [0u8; RHINO_MIN_REPORT_BYTES];
    r[0] = 0xAB;
    let err_id = parse_rhino_report(&r).unwrap_err();
    let msg = err_id.to_string();
    assert!(msg.contains("AB"), "should mention report ID");
}

/// Oversized report (28 bytes) parses OK — extra bytes ignored.
#[test]
fn oversized_report_ok() {
    let mut v = centred().to_vec();
    v.extend_from_slice(&[0xDE; 8]);
    let s = parse_rhino_report(&v).unwrap();
    assert!(s.axes.roll.abs() < 1e-4);
}

// ──────────────────────────────────────────────────────────────────────────────
// §6 — Device Detection
// ──────────────────────────────────────────────────────────────────────────────

/// VID must be 0x0483 (STMicro).
#[test]
fn vendor_id_is_stmicro() {
    assert_eq!(VPFORCE_VENDOR_ID, 0x0483);
}

/// PIDs are distinct for v2 and v3.
#[test]
fn pids_are_distinct() {
    assert_ne!(VPFORCE_RHINO_PID_V2, VPFORCE_RHINO_PID_V3);
}

/// is_vpforce_device accepts both known PIDs with correct VID.
#[test]
fn detection_accepts_both_pids() {
    assert!(is_vpforce_device(VPFORCE_VENDOR_ID, VPFORCE_RHINO_PID_V2));
    assert!(is_vpforce_device(VPFORCE_VENDOR_ID, VPFORCE_RHINO_PID_V3));
}

/// is_vpforce_device rejects wrong VID even with correct PID.
#[test]
fn detection_rejects_wrong_vid() {
    assert!(!is_vpforce_device(0x0000, VPFORCE_RHINO_PID_V2));
    assert!(!is_vpforce_device(0xFFFF, VPFORCE_RHINO_PID_V3));
}

/// is_vpforce_device rejects unknown PID even with correct VID.
#[test]
fn detection_rejects_unknown_pid() {
    assert!(!is_vpforce_device(VPFORCE_VENDOR_ID, 0x0000));
    assert!(!is_vpforce_device(VPFORCE_VENDOR_ID, 0xFFFF));
}

/// vpforce_model identifies v2 and v3 correctly.
#[test]
fn model_identification() {
    assert_eq!(vpforce_model(VPFORCE_RHINO_PID_V2), Some(VpforceModel::RhinoV2));
    assert_eq!(vpforce_model(VPFORCE_RHINO_PID_V3), Some(VpforceModel::RhinoV3));
    assert_eq!(vpforce_model(0x0000), None);
}

/// Model names contain "Rhino".
#[test]
fn model_names_contain_rhino() {
    assert!(VpforceModel::RhinoV2.name().contains("Rhino"));
    assert!(VpforceModel::RhinoV3.name().contains("Rhino"));
}

/// Model names distinguish v2 from v3.
#[test]
fn model_names_distinguish_versions() {
    assert_ne!(VpforceModel::RhinoV2.name(), VpforceModel::RhinoV3.name());
}

// ──────────────────────────────────────────────────────────────────────────────
// §7 — Struct Traits & Defaults
// ──────────────────────────────────────────────────────────────────────────────

/// RhinoAxes default is all zeros.
#[test]
fn axes_default_is_zero() {
    let d = RhinoAxes::default();
    assert_eq!(d.roll, 0.0);
    assert_eq!(d.pitch, 0.0);
    assert_eq!(d.throttle, 0.0);
    assert_eq!(d.rocker, 0.0);
    assert_eq!(d.twist, 0.0);
    assert_eq!(d.ry, 0.0);
}

/// RhinoButtons default has mask=0 and hat=0.
#[test]
fn buttons_default() {
    let d = RhinoButtons::default();
    assert_eq!(d.mask, 0);
    assert_eq!(d.hat, 0);
}

/// RhinoAxes implements Clone and Copy.
#[test]
fn axes_clone_copy() {
    let a = RhinoAxes { roll: 0.5, pitch: -0.3, throttle: 0.8, rocker: 0.0, twist: 0.1, ry: 0.0 };
    let b = a; // Copy
    #[allow(clippy::clone_on_copy)]
    let c = a.clone(); // Clone
    assert_eq!(a, b);
    assert_eq!(a, c);
}

/// RhinoButtons implements Clone and Copy.
#[test]
fn buttons_clone_copy() {
    let a = RhinoButtons { mask: 0xDEAD_BEEF, hat: 3 };
    let b = a;
    #[allow(clippy::clone_on_copy)]
    let c = a.clone();
    assert_eq!(a, b);
    assert_eq!(a, c);
}

/// RhinoInputState implements Clone and Copy.
#[test]
fn input_state_clone_copy() {
    let s = parse_rhino_report(&centred()).unwrap();
    let s2 = s; // Copy
    #[allow(clippy::clone_on_copy)]
    let s3 = s.clone(); // Clone
    assert_eq!(s.axes, s2.axes);
    assert_eq!(s.buttons, s3.buttons);
}

/// RhinoParseError implements Clone and PartialEq.
#[test]
fn parse_error_clone_eq() {
    let e1 = RhinoParseError::TooShort { expected: 20, got: 5 };
    let e2 = e1.clone();
    assert_eq!(e1, e2);
}

// ──────────────────────────────────────────────────────────────────────────────
// §8 — RHINO_MIN_REPORT_BYTES constant
// ──────────────────────────────────────────────────────────────────────────────

/// The minimum report size is 20 bytes.
#[test]
fn min_report_bytes_is_20() {
    assert_eq!(RHINO_MIN_REPORT_BYTES, 20);
}

/// Exactly RHINO_MIN_REPORT_BYTES succeeds.
#[test]
fn exact_min_bytes_succeeds() {
    let r = centred();
    assert_eq!(r.len(), RHINO_MIN_REPORT_BYTES);
    assert!(parse_rhino_report(&r).is_ok());
}
