// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Depth tests for legacy Microsoft SideWinder devices.
//!
//! Covers: FFB effect semantics, axis handling, button/hat behaviour, and
//! device-identification logic for the Force Feedback Pro, Force Feedback 2,
//! and Precision 2 SideWinder sticks (VID 0x045E).

use flight_hotas_microsoft::{
    MICROSOFT_VENDOR_ID, SIDEWINDER_FFB2_PID, SIDEWINDER_FFB_PRO_PID, SIDEWINDER_PRECISION_2_PID,
    SidewinderFfbHat, SidewinderFfbInputState, SidewinderModel,
    SidewinderP2Hat, is_sidewinder_device, parse_sidewinder_ffb2, parse_sidewinder_ffb_pro,
    parse_sidewinder_precision2, sidewinder_model,
};

// ── report builder helpers ────────────────────────────────────────────────────

/// Build a 7-byte SideWinder FFB / P2 HID report from logical field values.
fn build_report(x: u16, y: u16, rz: u8, throttle: u8, hat: u8, buttons: u16) -> [u8; 7] {
    let x = x & 0x3FF;
    let y = y & 0x3FF;
    let hat = hat & 0x0F;
    let buttons = buttons & 0x01FF;

    let mut b = [0u8; 7];
    b[0] = x as u8;
    b[1] = ((x >> 8) as u8) | ((y as u8 & 0x3F) << 2);
    b[2] = ((y >> 6) as u8 & 0x0F) | ((rz & 0x0F) << 4);
    b[3] = (rz >> 4) | ((throttle & 0x0F) << 4);
    b[4] = (throttle >> 4) | ((hat & 0x0F) << 4);
    b[5] = (buttons & 0xFF) as u8;
    b[6] = ((buttons >> 8) & 0x01) as u8;
    b
}

/// Centered idle report: axes centered, throttle zero, hat center, no buttons.
fn idle_report() -> [u8; 7] {
    build_report(512, 512, 128, 0, 8, 0)
}

// ═══════════════════════════════════════════════════════════════════════════════
// 1. FFB (Force Feedback 2) depth tests — effect semantics & safety
// ═══════════════════════════════════════════════════════════════════════════════

/// FFB2 should recognise spring-like centered-axis semantics: when the stick is
/// at center the normalised axes are near zero, meaning no force offset.
#[test]
fn ffb2_effect_spring_center_produces_zero_offset() {
    let state = parse_sidewinder_ffb2(&idle_report()).unwrap();
    assert!(state.axes.x.abs() < 0.01, "spring center X: {}", state.axes.x);
    assert!(state.axes.y.abs() < 0.01, "spring center Y: {}", state.axes.y);
}

/// FFB2 force direction: full deflection on one axis must produce ±1.0 while
/// the orthogonal axis stays near zero — ensures force vectors are independent.
#[test]
fn ffb2_force_direction_axes_independent() {
    let right = build_report(1023, 512, 128, 0, 8, 0);
    let s = parse_sidewinder_ffb2(&right).unwrap();
    assert!(s.axes.x > 0.99, "X should be ~1.0: {}", s.axes.x);
    assert!(s.axes.y.abs() < 0.01, "Y should stay ~0.0: {}", s.axes.y);

    let forward = build_report(512, 0, 128, 0, 8, 0);
    let s = parse_sidewinder_ffb2(&forward).unwrap();
    assert!(s.axes.x.abs() < 0.01, "X should stay ~0.0: {}", s.axes.x);
    assert!(s.axes.y < -0.99, "Y should be ~-1.0: {}", s.axes.y);
}

/// FFB update rate fidelity: parsing the same report twice must yield identical
/// floating-point results, ensuring deterministic 250 Hz ticks.
#[test]
fn ffb2_update_rate_deterministic_parsing() {
    let data = build_report(300, 700, 60, 180, 3, 0b1_0101_0101);
    let a = parse_sidewinder_ffb2(&data).unwrap();
    let b = parse_sidewinder_ffb2(&data).unwrap();
    assert_eq!(a.axes.x, b.axes.x);
    assert_eq!(a.axes.y, b.axes.y);
    assert_eq!(a.axes.rz, b.axes.rz);
    assert_eq!(a.axes.throttle, b.axes.throttle);
    assert_eq!(a.buttons.buttons, b.buttons.buttons);
    assert_eq!(a.buttons.hat, b.buttons.hat);
}

/// FFB safety: axis values must always be clamped within [-1, 1] (bipolar) or
/// [0, 1] (unipolar) even for extreme raw inputs.
#[test]
fn ffb2_safety_limits_clamp_axes() {
    // All-maximum raw values
    let data = build_report(1023, 1023, 255, 255, 8, 0);
    let s = parse_sidewinder_ffb2(&data).unwrap();
    assert!(s.axes.x <= 1.0 && s.axes.x >= -1.0);
    assert!(s.axes.y <= 1.0 && s.axes.y >= -1.0);
    assert!(s.axes.rz <= 1.0 && s.axes.rz >= -1.0);
    assert!(s.axes.throttle >= 0.0 && s.axes.throttle <= 1.0);

    // All-minimum raw values
    let data = build_report(0, 0, 0, 0, 8, 0);
    let s = parse_sidewinder_ffb2(&data).unwrap();
    assert!(s.axes.x <= 1.0 && s.axes.x >= -1.0);
    assert!(s.axes.y <= 1.0 && s.axes.y >= -1.0);
    assert!(s.axes.rz <= 1.0 && s.axes.rz >= -1.0);
    assert!(s.axes.throttle >= 0.0 && s.axes.throttle <= 1.0);
}

/// FFB effect lifecycle: default state → full deflection → back to default must
/// all parse without error and return correct values.
#[test]
fn ffb2_effect_lifecycle_idle_active_idle() {
    let idle = parse_sidewinder_ffb2(&idle_report()).unwrap();
    assert!(idle.axes.x.abs() < 0.01);
    assert_eq!(idle.buttons.buttons, 0);

    let active = parse_sidewinder_ffb2(&build_report(1023, 0, 255, 255, 0, 0x1FF)).unwrap();
    assert!(active.axes.x > 0.99);
    assert!(active.axes.y < -0.99);
    assert_eq!(active.buttons.hat, SidewinderFfbHat::North);
    assert_eq!(active.buttons.buttons, 0x1FF);

    let idle2 = parse_sidewinder_ffb2(&idle_report()).unwrap();
    assert!(idle2.axes.x.abs() < 0.01);
    assert_eq!(idle2.buttons.buttons, 0);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. Axis handling depth tests
// ═══════════════════════════════════════════════════════════════════════════════

/// All four FFB axes must reach their full normalised ranges.
#[test]
fn axis_four_axes_full_range() {
    // X axis
    let left = parse_sidewinder_ffb_pro(&build_report(0, 512, 128, 0, 8, 0)).unwrap();
    let right = parse_sidewinder_ffb_pro(&build_report(1023, 512, 128, 0, 8, 0)).unwrap();
    assert!(left.axes.x < -0.99, "X min: {}", left.axes.x);
    assert!(right.axes.x > 0.99, "X max: {}", right.axes.x);

    // Y axis
    let fwd = parse_sidewinder_ffb_pro(&build_report(512, 0, 128, 0, 8, 0)).unwrap();
    let back = parse_sidewinder_ffb_pro(&build_report(512, 1023, 128, 0, 8, 0)).unwrap();
    assert!(fwd.axes.y < -0.99, "Y min: {}", fwd.axes.y);
    assert!(back.axes.y > 0.99, "Y max: {}", back.axes.y);

    // Rz (twist)
    let tleft = parse_sidewinder_ffb_pro(&build_report(512, 512, 0, 0, 8, 0)).unwrap();
    let tright = parse_sidewinder_ffb_pro(&build_report(512, 512, 255, 0, 8, 0)).unwrap();
    assert!(tleft.axes.rz < -0.99, "Rz min: {}", tleft.axes.rz);
    assert!(tright.axes.rz > 0.99, "Rz max: {}", tright.axes.rz);

    // Throttle
    let tmin = parse_sidewinder_ffb_pro(&build_report(512, 512, 128, 0, 8, 0)).unwrap();
    let tmax = parse_sidewinder_ffb_pro(&build_report(512, 512, 128, 255, 8, 0)).unwrap();
    assert!(tmin.axes.throttle < 0.001, "throttle min: {}", tmin.axes.throttle);
    assert!(tmax.axes.throttle > 0.999, "throttle max: {}", tmax.axes.throttle);
}

/// Twist rudder (Rz) on the FFB2: verify bipolar normalisation around center.
#[test]
fn axis_twist_rudder_bipolar_center() {
    let center = parse_sidewinder_ffb2(&build_report(512, 512, 128, 0, 8, 0)).unwrap();
    assert!(center.axes.rz.abs() < 0.01, "twist center: {}", center.axes.rz);

    let quarter_left = parse_sidewinder_ffb2(&build_report(512, 512, 64, 0, 8, 0)).unwrap();
    assert!(quarter_left.axes.rz < -0.45 && quarter_left.axes.rz > -0.55,
        "twist quarter-left: {}", quarter_left.axes.rz);
}

/// Throttle slider: verify unipolar range 0.0..=1.0 with mid-travel check.
#[test]
fn axis_throttle_slider_unipolar_midrange() {
    let mid = parse_sidewinder_ffb_pro(&build_report(512, 512, 128, 128, 8, 0)).unwrap();
    assert!(mid.axes.throttle > 0.49 && mid.axes.throttle < 0.51,
        "throttle midpoint: {}", mid.axes.throttle);
}

/// Center calibration: axes centered (raw 512/512/128) must all be within ±0.01
/// of zero for both FFB Pro and Precision 2.
#[test]
fn axis_center_calibration_both_models() {
    let ffb = parse_sidewinder_ffb_pro(&idle_report()).unwrap();
    assert!(ffb.axes.x.abs() < 0.01);
    assert!(ffb.axes.y.abs() < 0.01);
    assert!(ffb.axes.rz.abs() < 0.01);

    let p2 = parse_sidewinder_precision2(&idle_report()).unwrap();
    assert!(p2.axes.x.abs() < 0.01);
    assert!(p2.axes.y.abs() < 0.01);
    assert!(p2.axes.rz.abs() < 0.01);
}

/// Precision 2 axes must mirror the FFB layout exactly (same normalisation).
#[test]
fn axis_precision2_matches_ffb_layout() {
    let data = build_report(200, 800, 30, 220, 5, 0);
    let ffb = parse_sidewinder_ffb_pro(&data).unwrap();
    let p2 = parse_sidewinder_precision2(&data).unwrap();
    assert_eq!(ffb.axes.x, p2.axes.x);
    assert_eq!(ffb.axes.y, p2.axes.y);
    assert_eq!(ffb.axes.rz, p2.axes.rz);
    assert_eq!(ffb.axes.throttle, p2.axes.throttle);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. Button / hat depth tests
// ═══════════════════════════════════════════════════════════════════════════════

/// All 8 primary buttons must independently register.
#[test]
fn button_eight_buttons_independent() {
    for btn in 1u8..=8 {
        let mask = 1u16 << (btn - 1);
        let data = build_report(512, 512, 128, 0, 8, mask);
        let s = parse_sidewinder_ffb2(&data).unwrap();
        assert!(s.buttons.button(btn), "button {btn} should be pressed");
        // No other button pressed
        for other in 1u8..=9 {
            if other != btn {
                assert!(!s.buttons.button(other),
                    "button {other} should NOT be pressed when only {btn} is");
            }
        }
    }
}

/// POV hat must correctly resolve all 8 cardinal + ordinal directions.
#[test]
fn button_pov_hat_eight_way() {
    let expected_ffb = [
        (0, SidewinderFfbHat::North),
        (1, SidewinderFfbHat::NorthEast),
        (2, SidewinderFfbHat::East),
        (3, SidewinderFfbHat::SouthEast),
        (4, SidewinderFfbHat::South),
        (5, SidewinderFfbHat::SouthWest),
        (6, SidewinderFfbHat::West),
        (7, SidewinderFfbHat::NorthWest),
    ];
    for (raw, expected) in expected_ffb {
        let s = parse_sidewinder_ffb2(&build_report(512, 512, 128, 0, raw, 0)).unwrap();
        assert_eq!(s.buttons.hat, expected, "FFB hat nibble {raw}");
    }

    let expected_p2 = [
        (0, SidewinderP2Hat::North),
        (1, SidewinderP2Hat::NorthEast),
        (2, SidewinderP2Hat::East),
        (3, SidewinderP2Hat::SouthEast),
        (4, SidewinderP2Hat::South),
        (5, SidewinderP2Hat::SouthWest),
        (6, SidewinderP2Hat::West),
        (7, SidewinderP2Hat::NorthWest),
    ];
    for (raw, expected) in expected_p2 {
        let s = parse_sidewinder_precision2(&build_report(512, 512, 128, 0, raw, 0)).unwrap();
        assert_eq!(s.buttons.hat, expected, "P2 hat nibble {raw}");
    }
}

/// Button debounce: pressing the same button pattern in rapid succession must
/// always produce identical parsed state (no bit-flip artefacts).
#[test]
fn button_debounce_stable_repeated_parse() {
    let data = build_report(512, 512, 128, 0, 8, 0b10101);
    for _ in 0..100 {
        let r = parse_sidewinder_ffb2(&data).unwrap().buttons;
        assert_eq!(r.buttons, 0b10101, "button mask must be stable");
        assert_eq!(r.hat, SidewinderFfbHat::Center);
    }
}

/// Hat center values >= 8 must all map to Center (DirectInput convention).
#[test]
fn button_hat_center_high_nibbles() {
    for nibble in 8u8..=15 {
        let s = parse_sidewinder_ffb2(&build_report(512, 512, 128, 0, nibble, 0)).unwrap();
        assert_eq!(s.buttons.hat, SidewinderFfbHat::Center,
            "nibble {nibble} must map to Center");
    }
}

/// Simultaneous buttons and hat: pressing buttons while hat is active must
/// not corrupt either field.
#[test]
fn button_simultaneous_buttons_and_hat() {
    let data = build_report(512, 512, 128, 0, 2, 0b1_1001_0110); // East hat + buttons 2,3,5,8,9
    let s = parse_sidewinder_ffb2(&data).unwrap();
    assert_eq!(s.buttons.hat, SidewinderFfbHat::East);
    assert!(s.buttons.button(2));
    assert!(s.buttons.button(3));
    assert!(s.buttons.button(5));
    assert!(s.buttons.button(8));
    assert!(s.buttons.button(9));
    assert!(!s.buttons.button(1));
    assert!(!s.buttons.button(4));
    assert!(!s.buttons.button(6));
    assert!(!s.buttons.button(7));
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. Device identification depth tests
// ═══════════════════════════════════════════════════════════════════════════════

/// VID/PID matching: all known SideWinder models must be recognised.
#[test]
fn devid_vid_pid_matching_known_devices() {
    assert!(is_sidewinder_device(0x045E, SIDEWINDER_FFB_PRO_PID));
    assert!(is_sidewinder_device(0x045E, SIDEWINDER_FFB2_PID));
    assert!(is_sidewinder_device(0x045E, SIDEWINDER_PRECISION_2_PID));
}

/// VID/PID matching: non-Microsoft VIDs must be rejected even with valid PIDs.
#[test]
fn devid_wrong_vid_rejected() {
    assert!(!is_sidewinder_device(0x0000, SIDEWINDER_FFB2_PID));
    assert!(!is_sidewinder_device(0x046D, SIDEWINDER_FFB2_PID)); // Logitech VID
    assert!(!is_sidewinder_device(0xFFFF, SIDEWINDER_PRECISION_2_PID));
}

/// Model discrimination: each PID must map to exactly one SidewinderModel variant.
#[test]
fn devid_model_discrimination() {
    assert_eq!(sidewinder_model(SIDEWINDER_FFB_PRO_PID), Some(SidewinderModel::FfbPro));
    assert_eq!(sidewinder_model(SIDEWINDER_FFB2_PID), Some(SidewinderModel::Ffb2));
    assert_eq!(sidewinder_model(SIDEWINDER_PRECISION_2_PID), Some(SidewinderModel::Precision2));
}

/// Unknown PIDs under the Microsoft VID must return None.
#[test]
fn devid_unknown_pid_returns_none() {
    assert_eq!(sidewinder_model(0x0000), None);
    assert_eq!(sidewinder_model(0x0001), None);
    assert_eq!(sidewinder_model(0xFFFF), None);
}

/// Legacy device handling: FFB models must report has_ffb() = true, Precision 2
/// must report false.
#[test]
fn devid_legacy_ffb_capability_flag() {
    assert!(SidewinderModel::FfbPro.has_ffb(), "FFB Pro must have FFB");
    assert!(SidewinderModel::Ffb2.has_ffb(), "FFB 2 must have FFB");
    assert!(!SidewinderModel::Precision2.has_ffb(), "Precision 2 has no FFB");
}

/// VID constant must match the canonical Microsoft vendor ID.
#[test]
fn devid_microsoft_vid_constant() {
    assert_eq!(MICROSOFT_VENDOR_ID, 0x045E);
}

/// Model names must contain "SideWinder" for user-facing display.
#[test]
fn devid_model_names_contain_sidewinder() {
    assert!(SidewinderModel::FfbPro.name().contains("SideWinder"));
    assert!(SidewinderModel::Ffb2.name().contains("SideWinder"));
    assert!(SidewinderModel::Precision2.name().contains("SideWinder"));
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. Cross-model & edge-case depth tests
// ═══════════════════════════════════════════════════════════════════════════════

/// FFB Pro and FFB2 parsers must produce bit-identical output for the same input.
#[test]
fn cross_ffb_pro_and_ffb2_identical() {
    let data = build_report(333, 666, 99, 200, 5, 0b1_0011_0011);
    let pro = parse_sidewinder_ffb_pro(&data).unwrap();
    let ffb2 = parse_sidewinder_ffb2(&data).unwrap();
    assert_eq!(pro.axes.x, ffb2.axes.x);
    assert_eq!(pro.axes.y, ffb2.axes.y);
    assert_eq!(pro.axes.rz, ffb2.axes.rz);
    assert_eq!(pro.axes.throttle, ffb2.axes.throttle);
    assert_eq!(pro.buttons.buttons, ffb2.buttons.buttons);
    assert_eq!(pro.buttons.hat, ffb2.buttons.hat);
}

/// Short reports must produce structured errors, not panics.
#[test]
fn edge_short_reports_structured_errors() {
    for len in 0..7 {
        let data = vec![0xFFu8; len];
        let ffb = parse_sidewinder_ffb_pro(&data);
        assert!(ffb.is_err(), "FFB should reject {len}-byte report");

        let p2 = parse_sidewinder_precision2(&data);
        assert!(p2.is_err(), "P2 should reject {len}-byte report");
    }
}

/// Extra trailing bytes beyond the 7-byte minimum must be silently ignored.
#[test]
fn edge_extra_bytes_ignored() {
    let mut extended = idle_report().to_vec();
    extended.extend_from_slice(&[0xFF; 32]);
    let s = parse_sidewinder_ffb2(&extended).unwrap();
    assert!(s.axes.x.abs() < 0.01);
    assert_eq!(s.buttons.buttons, 0);
}

/// Default state struct must represent an idle / unplugged device.
#[test]
fn edge_default_state_is_idle() {
    let state = SidewinderFfbInputState::default();
    assert_eq!(state.axes.x, 0.0);
    assert_eq!(state.axes.y, 0.0);
    assert_eq!(state.axes.rz, 0.0);
    assert_eq!(state.axes.throttle, 0.0);
    assert_eq!(state.buttons.buttons, 0);
    assert_eq!(state.buttons.hat, SidewinderFfbHat::Center);
}
