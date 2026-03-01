// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the `flight-hotas-winwing` crate.
//!
//! These integration tests exercise cross-module interactions, boundary
//! conditions, multi-device scenarios, and protocol round-trip invariants
//! that go beyond the unit tests in each source module.

use flight_hotas_winwing::*;

// ═══════════════════════════════════════════════════════════════════════════════
// § 1 — USB identifier consistency
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn all_known_pids_use_winwing_vid() {
    assert_eq!(WINWING_VID, 0x4098);
    assert_eq!(WINWING_VID, input::WINWING_VENDOR_ID);
}

#[test]
fn known_pids_list_is_non_empty_and_unique() {
    assert!(!WINWING_PIDS.is_empty());
    let mut sorted = WINWING_PIDS.to_vec();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), WINWING_PIDS.len(), "duplicate PIDs detected");
}

#[test]
fn pid_constants_match_across_modules() {
    // The dedicated modules re-export the same PID values as `input`.
    assert_eq!(ORION2_F18_STICK_PID, orion2_stick::ORION2_STICK_PID);
    assert_eq!(ORION2_THROTTLE_PID, orion2_throttle::ORION2_THROTTLE_PID);
    assert_eq!(TFRP_RUDDER_PID, tfrp::TFRP_RUDDER_PID);
    assert_eq!(F16EX_STICK_PID, f16ex_stick::F16EX_STICK_PID);
    assert_eq!(SUPER_TAURUS_PID, super_taurus::SUPER_TAURUS_PID);
    assert_eq!(UFC_PANEL_PID, ufc_panel::UFC_PANEL_PID);
    assert_eq!(SKYWALKER_RUDDER_PID, skywalker_rudder::SKYWALKER_RUDDER_PID);
}

#[test]
fn winwing_pids_contains_all_device_pids() {
    let expected = [
        ORION2_THROTTLE_PID,
        ORION2_F18_STICK_PID,
        TFRP_RUDDER_PID,
        F16EX_STICK_PID,
        SUPER_TAURUS_PID,
        UFC_PANEL_PID,
        SKYWALKER_RUDDER_PID,
    ];
    for pid in &expected {
        assert!(
            WINWING_PIDS.contains(pid),
            "WINWING_PIDS missing PID 0x{pid:04X}"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// § 2 — Cross-device report length sanity
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn report_lengths_are_positive() {
    const {
        assert!(THROTTLE_REPORT_LEN > 0);
        assert!(STICK_REPORT_LEN > 0);
        assert!(RUDDER_REPORT_LEN > 0);
        assert!(F16EX_REPORT_LEN > 0);
        assert!(SUPER_TAURUS_REPORT_LEN > 0);
        assert!(UFC_PANEL_REPORT_LEN > 0);
        assert!(SKYWALKER_RUDDER_REPORT_LEN > 0);
        assert!(ORION2_STICK_REPORT_LEN > 0);
        assert!(ORION2_THROTTLE_REPORT_BYTES > 0);
        assert!(TFRP_REPORT_BYTES > 0);
    }
}

#[test]
fn dedicated_report_len_matches_generic() {
    // Dedicated modules and generic input module should agree on sizes.
    assert_eq!(THROTTLE_REPORT_LEN, ORION2_THROTTLE_REPORT_BYTES);
    assert_eq!(STICK_REPORT_LEN, ORION2_STICK_REPORT_LEN);
    assert_eq!(RUDDER_REPORT_LEN, TFRP_REPORT_BYTES);
}

// ═══════════════════════════════════════════════════════════════════════════════
// § 3 — Orion 2 Throttle (generic input + dedicated module)
// ═══════════════════════════════════════════════════════════════════════════════

fn make_throttle_report(tl: u16, tr: u16, friction: u16, mx: u16, my: u16) -> Vec<u8> {
    let mut r = vec![0u8; THROTTLE_REPORT_LEN];
    r[0] = 0x01;
    r[1..3].copy_from_slice(&tl.to_le_bytes());
    r[3..5].copy_from_slice(&tr.to_le_bytes());
    r[5..7].copy_from_slice(&friction.to_le_bytes());
    r[7..9].copy_from_slice(&mx.to_le_bytes());
    r[9..11].copy_from_slice(&my.to_le_bytes());
    r
}

#[test]
fn throttle_generic_and_dedicated_agree_on_axes() {
    let r = make_throttle_report(30000, 45000, 10000, 32768, 32768);
    let generic = parse_throttle_report(&r).unwrap();
    let dedicated = parse_orion2_throttle_report(&r).unwrap();
    let eps = 1e-4;
    assert!((generic.axes.throttle_left - dedicated.axes.throttle_left).abs() < eps);
    assert!((generic.axes.throttle_right - dedicated.axes.throttle_right).abs() < eps);
    assert!((generic.axes.throttle_combined - dedicated.axes.throttle_combined).abs() < eps);
    assert!((generic.axes.friction - dedicated.axes.friction).abs() < eps);
}

#[test]
fn throttle_combined_equals_average_of_left_right() {
    for (tl, tr) in [(0, 0), (0xFFFF, 0), (0, 0xFFFF), (20000, 40000)] {
        let r = make_throttle_report(tl, tr, 0, 32768, 32768);
        let s = parse_orion2_throttle_report(&r).unwrap();
        let expected = (s.axes.throttle_left + s.axes.throttle_right) * 0.5;
        assert!(
            (s.axes.throttle_combined - expected).abs() < 1e-5,
            "combined mismatch for tl={tl}, tr={tr}"
        );
    }
}

#[test]
fn throttle_mouse_stick_extreme_left_and_right() {
    let left = make_throttle_report(0, 0, 0, 0, 32768);
    let right = make_throttle_report(0, 0, 0, 0xFFFF, 32768);
    let sl = parse_orion2_throttle_report(&left).unwrap();
    let sr = parse_orion2_throttle_report(&right).unwrap();
    assert!(sl.axes.mouse_x < -0.9, "full left mouse_x should be < -0.9");
    assert!(sr.axes.mouse_x > 0.9, "full right mouse_x should be > 0.9");
}

#[test]
fn throttle_all_50_buttons_individually_pressed() {
    for btn in 1u8..=ORION2_THROTTLE_BUTTON_COUNT {
        let mut r = make_throttle_report(0, 0, 0, 32768, 32768);
        let bit = u64::from(btn - 1);
        r[11..19].copy_from_slice(&(1u64 << bit).to_le_bytes());
        let s = parse_orion2_throttle_report(&r).unwrap();
        assert!(
            s.buttons.is_pressed(btn),
            "button {btn} should be pressed"
        );
        // All other buttons should be unpressed.
        for other in 1u8..=ORION2_THROTTLE_BUTTON_COUNT {
            if other != btn {
                assert!(!s.buttons.is_pressed(other), "button {other} should NOT be pressed when only {btn} is set");
            }
        }
    }
}

#[test]
fn throttle_encoder_positive_and_negative_deltas() {
    let mut r = make_throttle_report(0, 0, 0, 32768, 32768);
    r[19] = 5u8;           // encoder 0: +5
    r[20] = (-7i8) as u8;  // encoder 1: -7
    r[21] = 0;             // encoder 2: 0
    r[22] = 127u8;         // encoder 3: +127 (max)
    r[23] = (-128i8) as u8; // encoder 4: -128 (min)
    let s = parse_orion2_throttle_report(&r).unwrap();
    assert_eq!(s.buttons.encoders[0], 5);
    assert_eq!(s.buttons.encoders[1], -7);
    assert_eq!(s.buttons.encoders[2], 0);
    assert_eq!(s.buttons.encoders[3], 127);
    assert_eq!(s.buttons.encoders[4], -128);
}

// ═══════════════════════════════════════════════════════════════════════════════
// § 4 — Orion 2 Stick (generic input + dedicated module)
// ═══════════════════════════════════════════════════════════════════════════════

fn make_stick_report(roll: i16, pitch: i16, hat_a: u8, hat_b: u8) -> Vec<u8> {
    let mut r = vec![0u8; STICK_REPORT_LEN];
    r[0] = 0x02;
    r[1..3].copy_from_slice(&roll.to_le_bytes());
    r[3..5].copy_from_slice(&pitch.to_le_bytes());
    r[9] = hat_a;
    r[10] = hat_b;
    r
}

#[test]
fn stick_generic_and_dedicated_agree() {
    let r = make_stick_report(10000, -15000, 0x0F, 0x0F);
    let generic = parse_stick_report(&r).unwrap();
    let dedicated = parse_orion2_stick_report(&r).unwrap();
    let eps = 1e-4;
    assert!((generic.axes.roll - dedicated.axes.roll).abs() < eps);
    assert!((generic.axes.pitch - dedicated.axes.pitch).abs() < eps);
}

#[test]
fn stick_all_8_hat_directions() {
    let directions = [0u8, 1, 2, 3, 4, 5, 6, 7]; // N, NE, E, SE, S, SW, W, NW
    for &dir in &directions {
        let r = make_stick_report(0, 0, dir, 0x0F);
        let s = parse_orion2_stick_report(&r).unwrap();
        assert_eq!(s.buttons.hat_a, dir);
        assert!(!s.buttons.hat_a_neutral());
        assert!(s.buttons.hat_b_neutral());
    }
}

#[test]
fn stick_both_hats_active_simultaneously() {
    let r = make_stick_report(0, 0, 0x00, 0x04); // hat_a=N, hat_b=S
    let s = parse_orion2_stick_report(&r).unwrap();
    assert!(!s.buttons.hat_a_neutral());
    assert!(!s.buttons.hat_b_neutral());
    assert_eq!(s.buttons.hat_a, 0x00);
    assert_eq!(s.buttons.hat_b, 0x04);
}

#[test]
fn stick_full_deflection_all_corners() {
    let corners: [(i16, i16); 4] = [
        (32767, 32767),   // full right + full forward
        (32767, -32767),  // full right + full aft
        (-32767, 32767),  // full left + full forward
        (-32767, -32767), // full left + full aft
    ];
    for (roll, pitch) in corners {
        let r = make_stick_report(roll, pitch, 0x0F, 0x0F);
        let s = parse_orion2_stick_report(&r).unwrap();
        assert!(s.axes.roll.abs() > 0.99);
        assert!(s.axes.pitch.abs() > 0.99);
    }
}

#[test]
fn stick_all_20_buttons() {
    for btn in 1u8..=ORION2_STICK_BUTTON_COUNT {
        let mut r = make_stick_report(0, 0, 0x0F, 0x0F);
        let mask = 1u32 << (btn - 1);
        r[5..9].copy_from_slice(&mask.to_le_bytes());
        let s = parse_orion2_stick_report(&r).unwrap();
        assert!(s.buttons.is_pressed(btn));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// § 5 — TFRP Rudder Pedals (generic input + dedicated module)
// ═══════════════════════════════════════════════════════════════════════════════

fn make_tfrp_report(rudder: i16, bl: u16, br: u16) -> Vec<u8> {
    let mut r = vec![0u8; TFRP_REPORT_BYTES];
    r[0] = 0x03;
    r[1..3].copy_from_slice(&rudder.to_le_bytes());
    r[3..5].copy_from_slice(&bl.to_le_bytes());
    r[5..7].copy_from_slice(&br.to_le_bytes());
    r
}

#[test]
fn tfrp_generic_and_dedicated_agree() {
    let r = make_tfrp_report(5000, 20000, 40000);
    let generic = parse_rudder_report(&r).unwrap();
    let dedicated = parse_tfrp_report(&r).unwrap();
    let eps = 1e-4;
    assert!((generic.rudder - dedicated.axes.rudder).abs() < eps);
    assert!((generic.brake_left - dedicated.axes.brake_left).abs() < eps);
    assert!((generic.brake_right - dedicated.axes.brake_right).abs() < eps);
}

#[test]
fn tfrp_asymmetric_braking() {
    let r = make_tfrp_report(0, 0xFFFF, 0);
    let s = parse_tfrp_report(&r).unwrap();
    assert!((s.axes.brake_left - 1.0).abs() < 1e-4);
    assert!(s.axes.brake_right < 1e-4);

    let r2 = make_tfrp_report(0, 0, 0xFFFF);
    let s2 = parse_tfrp_report(&r2).unwrap();
    assert!(s2.axes.brake_left < 1e-4);
    assert!((s2.axes.brake_right - 1.0).abs() < 1e-4);
}

#[test]
fn tfrp_rudder_clamps_at_i16_min() {
    let r = make_tfrp_report(i16::MIN, 0, 0);
    let s = parse_tfrp_report(&r).unwrap();
    assert!(s.axes.rudder >= -1.0, "rudder should be clamped to >= -1.0");
    assert!(s.axes.rudder <= -0.99, "i16::MIN should normalize near -1.0");
}

// ═══════════════════════════════════════════════════════════════════════════════
// § 6 — F-16EX Grip
// ═══════════════════════════════════════════════════════════════════════════════

fn make_f16ex_report(roll: i16, pitch: i16, hat: u8) -> Vec<u8> {
    let mut r = vec![0u8; F16EX_REPORT_LEN];
    r[0] = 0x04;
    r[1..3].copy_from_slice(&roll.to_le_bytes());
    r[3..5].copy_from_slice(&pitch.to_le_bytes());
    r[9] = hat;
    r
}

#[test]
fn f16ex_hat_all_positions() {
    for dir in 0u8..=7 {
        let r = make_f16ex_report(0, 0, dir);
        let s = parse_f16ex_stick_report(&r).unwrap();
        assert_eq!(s.buttons.hat, dir);
        assert!(!s.buttons.hat_neutral());
    }
    // Neutral
    let r = make_f16ex_report(0, 0, 0x0F);
    let s = parse_f16ex_stick_report(&r).unwrap();
    assert!(s.buttons.hat_neutral());
}

#[test]
fn f16ex_simultaneous_buttons_and_hat() {
    let mut r = make_f16ex_report(0, 0, 0x02); // hat = East
    r[5..9].copy_from_slice(&0b0000_0000_0000_0000_0000_0000_0000_0011u32.to_le_bytes()); // buttons 1+2
    let s = parse_f16ex_stick_report(&r).unwrap();
    assert!(s.buttons.is_pressed(1));
    assert!(s.buttons.is_pressed(2));
    assert!(!s.buttons.hat_neutral());
    assert_eq!(s.buttons.hat, 0x02);
}

#[test]
fn f16ex_axis_symmetry() {
    let pos = make_f16ex_report(16000, 16000, 0x0F);
    let neg = make_f16ex_report(-16000, -16000, 0x0F);
    let sp = parse_f16ex_stick_report(&pos).unwrap();
    let sn = parse_f16ex_stick_report(&neg).unwrap();
    // Roll and pitch should be symmetric (opposite sign, same magnitude).
    assert!((sp.axes.roll + sn.axes.roll).abs() < 1e-4);
    assert!((sp.axes.pitch + sn.axes.pitch).abs() < 1e-4);
}

// ═══════════════════════════════════════════════════════════════════════════════
// § 7 — SuperTaurus Dual Throttle
// ═══════════════════════════════════════════════════════════════════════════════

fn make_super_taurus_report(tl: u16, tr: u16, trim: i16) -> Vec<u8> {
    let mut r = vec![0u8; SUPER_TAURUS_REPORT_LEN];
    r[0] = 0x05;
    r[1..3].copy_from_slice(&tl.to_le_bytes());
    r[3..5].copy_from_slice(&tr.to_le_bytes());
    r[5..7].copy_from_slice(&trim.to_le_bytes());
    r
}

#[test]
fn super_taurus_combined_throttle() {
    let r = make_super_taurus_report(0xFFFF, 0, 0);
    let s = parse_super_taurus_report(&r).unwrap();
    assert!((s.axes.throttle_combined - 0.5).abs() < 1e-3);
}

#[test]
fn super_taurus_trim_full_range() {
    let pos = make_super_taurus_report(0, 0, 32767);
    let neg = make_super_taurus_report(0, 0, -32767);
    let sp = parse_super_taurus_report(&pos).unwrap();
    let sn = parse_super_taurus_report(&neg).unwrap();
    assert!((sp.axes.trim - 1.0).abs() < 1e-4);
    assert!((sn.axes.trim + 1.0).abs() < 1e-4);
}

#[test]
fn super_taurus_all_32_buttons() {
    for btn in 1u8..=SUPER_TAURUS_BUTTON_COUNT {
        let mut r = make_super_taurus_report(0, 0, 0);
        let mask = 1u32 << (btn - 1);
        r[7..11].copy_from_slice(&mask.to_le_bytes());
        let s = parse_super_taurus_report(&r).unwrap();
        assert!(s.buttons.is_pressed(btn), "button {btn} should be pressed");
    }
}

#[test]
fn super_taurus_encoder_deltas() {
    let mut r = make_super_taurus_report(0, 0, 0);
    r[11] = 3u8;           // encoder 0: +3
    r[12] = (-5i8) as u8;  // encoder 1: -5
    let s = parse_super_taurus_report(&r).unwrap();
    assert_eq!(s.buttons.encoders[0], 3);
    assert_eq!(s.buttons.encoders[1], -5);
}

// ═══════════════════════════════════════════════════════════════════════════════
// § 8 — Skywalker Metal Rudder Pedals
// ═══════════════════════════════════════════════════════════════════════════════

fn make_skywalker_report(rudder: i16, bl: u16, br: u16) -> Vec<u8> {
    let mut r = vec![0u8; SKYWALKER_RUDDER_REPORT_LEN];
    r[0] = 0x07;
    r[1..3].copy_from_slice(&rudder.to_le_bytes());
    r[3..5].copy_from_slice(&bl.to_le_bytes());
    r[5..7].copy_from_slice(&br.to_le_bytes());
    r
}

#[test]
fn skywalker_diff_brake_positive_when_right_only() {
    let s = parse_skywalker_rudder_report(&make_skywalker_report(0, 0, 0xFFFF)).unwrap();
    assert!(s.axes.diff_brake > 0.99);
}

#[test]
fn skywalker_diff_brake_negative_when_left_only() {
    let s = parse_skywalker_rudder_report(&make_skywalker_report(0, 0xFFFF, 0)).unwrap();
    assert!(s.axes.diff_brake < -0.99);
}

#[test]
fn skywalker_diff_brake_zero_when_equal() {
    let s = parse_skywalker_rudder_report(&make_skywalker_report(0, 30000, 30000)).unwrap();
    assert!(s.axes.diff_brake.abs() < 1e-4);
}

#[test]
fn skywalker_rudder_full_range() {
    let left = parse_skywalker_rudder_report(&make_skywalker_report(i16::MIN, 0, 0)).unwrap();
    let right = parse_skywalker_rudder_report(&make_skywalker_report(i16::MAX, 0, 0)).unwrap();
    assert!(left.axes.rudder < -0.99);
    assert!(right.axes.rudder > 0.99);
}

// ═══════════════════════════════════════════════════════════════════════════════
// § 9 — UFC Panel
// ═══════════════════════════════════════════════════════════════════════════════

fn make_ufc_report(b0: u8, b1: u8, b2: u8, b3: u8, b4: u8) -> Vec<u8> {
    vec![0x06, b0, b1, b2, b3, b4]
}

#[test]
fn ufc_panel_all_ufc_buttons() {
    for btn in 1u8..=UFC_BUTTON_COUNT {
        let byte_idx = ((btn - 1) / 8) as usize;
        let bit = (btn - 1) % 8;
        let mut bytes = [0u8; 5];
        bytes[byte_idx] = 1 << bit;
        let r = make_ufc_report(bytes[0], bytes[1], bytes[2], bytes[3], bytes[4]);
        let s = parse_ufc_panel_report(&r).unwrap();
        assert!(s.buttons.is_ufc_pressed(btn), "UFC button {btn} should be pressed");
        assert!(s.buttons.is_pressed(btn), "overall button {btn} should be pressed");
    }
}

#[test]
fn ufc_panel_all_hud_buttons() {
    for hud_btn in 1u8..=UFC_HUD_BUTTON_COUNT {
        let overall = UFC_BUTTON_COUNT + hud_btn;
        let byte_idx = ((overall - 1) / 8) as usize;
        let bit = (overall - 1) % 8;
        let mut bytes = [0u8; 5];
        bytes[byte_idx] = 1 << bit;
        let r = make_ufc_report(bytes[0], bytes[1], bytes[2], bytes[3], bytes[4]);
        let s = parse_ufc_panel_report(&r).unwrap();
        assert!(s.buttons.is_hud_pressed(hud_btn), "HUD button {hud_btn} should be pressed");
        assert!(s.buttons.is_pressed(overall), "overall button {overall} should be pressed");
    }
}

#[test]
fn ufc_panel_simultaneous_ufc_and_hud() {
    // Button 1 (UFC) and button 25 (HUD 1) at the same time.
    let r = make_ufc_report(0b0000_0001, 0, 0, 0b0000_0001, 0);
    let s = parse_ufc_panel_report(&r).unwrap();
    assert!(s.buttons.is_ufc_pressed(1));
    assert!(s.buttons.is_hud_pressed(1));
    assert!(!s.buttons.is_ufc_pressed(2));
}

// ═══════════════════════════════════════════════════════════════════════════════
// § 10 — Orion Joystick (simple parser)
// ═══════════════════════════════════════════════════════════════════════════════

fn make_orion_joystick_report(x: u16, y: u16, twist: u16, hat: u8) -> Vec<u8> {
    let mut r = vec![0u8; ORION_JOYSTICK_MIN_REPORT_BYTES];
    r[0] = 0x00; // report ID
    r[1..3].copy_from_slice(&x.to_le_bytes());
    r[3..5].copy_from_slice(&y.to_le_bytes());
    r[5..7].copy_from_slice(&twist.to_le_bytes());
    r[11] = hat;
    r
}

#[test]
fn orion_joystick_raw_axes() {
    let r = make_orion_joystick_report(1000, 2000, 3000, 8);
    let s = parse_orion_joystick(&r).unwrap();
    assert_eq!(s.x, 1000);
    assert_eq!(s.y, 2000);
    assert_eq!(s.twist, 3000);
    assert_eq!(s.hat, 8); // center
}

#[test]
fn orion_joystick_hat_clamps_invalid_to_center() {
    let r = make_orion_joystick_report(0, 0, 0, 0xFF);
    let s = parse_orion_joystick(&r).unwrap();
    assert_eq!(s.hat, 8, "invalid hat values should clamp to center (8)");
}

#[test]
fn orion_joystick_too_short() {
    let err = parse_orion_joystick(&[0u8; 5]).unwrap_err();
    assert!(matches!(err, WinWingError::ReportTooShort { .. }));
}

// ═══════════════════════════════════════════════════════════════════════════════
// § 11 — Simple Orion 2 Throttle parser
// ═══════════════════════════════════════════════════════════════════════════════

fn make_simple_throttle_report(main: u16, secondary: u16, a3: u16, a4: u16) -> Vec<u8> {
    let mut r = vec![0u8; ORION2_THROTTLE_MIN_REPORT_BYTES];
    r[0] = 0x00; // report ID
    r[1..3].copy_from_slice(&main.to_le_bytes());
    r[3..5].copy_from_slice(&secondary.to_le_bytes());
    r[5..7].copy_from_slice(&a3.to_le_bytes());
    r[7..9].copy_from_slice(&a4.to_le_bytes());
    r
}

#[test]
fn simple_throttle_parser_axes() {
    let r = make_simple_throttle_report(10000, 20000, 30000, 40000);
    let s = parse_orion2_throttle(&r).unwrap();
    assert_eq!(s.throttle_main, 10000);
    assert_eq!(s.throttle_secondary, 20000);
    assert_eq!(s.axis3, 30000);
    assert_eq!(s.axis4, 40000);
}

#[test]
fn simple_throttle_too_short() {
    let err = parse_orion2_throttle(&[0u8; 5]).unwrap_err();
    assert!(matches!(err, WinWingError::ReportTooShort { .. }));
}

// ═══════════════════════════════════════════════════════════════════════════════
// § 12 — Normalization utility functions
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn normalize_axis_16bit_center() {
    let val = normalize_axis_16bit(32768);
    assert!(val.abs() < 0.01, "center should be ~0.0, got {val}");
}

#[test]
fn normalize_axis_16bit_extremes() {
    let min = normalize_axis_16bit(0);
    let max = normalize_axis_16bit(65535);
    assert!((min + 1.0).abs() < 0.01, "min should be ~-1.0, got {min}");
    assert!((max - 1.0).abs() < 0.01, "max should be ~1.0, got {max}");
}

#[test]
fn normalize_throttle_16bit_range() {
    assert!(normalize_throttle_16bit(0).abs() < 1e-6);
    assert!((normalize_throttle_16bit(65535) - 1.0).abs() < 1e-4);
    assert!((normalize_throttle_16bit(32768) - 0.5).abs() < 0.01);
}

// ═══════════════════════════════════════════════════════════════════════════════
// § 13 — Protocol: frame building and round-trip
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn protocol_frame_roundtrip_all_categories() {
    let categories = [
        CommandCategory::Display,
        CommandCategory::Backlight,
        CommandCategory::Detent,
        CommandCategory::DeviceInfo,
    ];
    for cat in categories {
        let payload = [0x42, 0x43];
        let frame = FeatureReportFrame::new(cat, 0x01, &payload).unwrap();
        let parsed = parse_feature_report(frame.as_bytes()).unwrap();
        assert_eq!(parsed.category, cat);
        assert_eq!(parsed.sub_command, 0x01);
        assert_eq!(parsed.payload, &payload);
    }
}

#[test]
fn protocol_display_text_round_trip() {
    let frame = build_display_text_command(0x01, 0x02, "FL350").unwrap();
    let parsed = parse_feature_report(frame.as_bytes()).unwrap();
    assert_eq!(parsed.category, CommandCategory::Display);
    assert_eq!(parsed.payload[0], 0x01);
    assert_eq!(parsed.payload[1], 0x02);
    assert_eq!(&parsed.payload[2..], b"FL350");
}

#[test]
fn protocol_display_text_truncation_at_16() {
    let long = "12345678901234567890"; // 20 chars
    let frame = build_display_text_command(0x01, 0x00, long).unwrap();
    let parsed = parse_feature_report(frame.as_bytes()).unwrap();
    assert_eq!(parsed.payload.len(), 2 + 16);
}

#[test]
fn protocol_backlight_rgb_preserves_colors() {
    let frame = build_backlight_single_rgb_command(0x01, 0, 255, 128, 64).unwrap();
    let parsed = parse_feature_report(frame.as_bytes()).unwrap();
    assert_eq!(parsed.payload, &[0x01, 0, 255, 128, 64]);
}

#[test]
fn protocol_detent_set_preserves_position() {
    let frame = build_detent_set_command(1, 1, 50000).unwrap();
    let parsed = parse_feature_report(frame.as_bytes()).unwrap();
    let pos = u16::from_le_bytes([parsed.payload[2], parsed.payload[3]]);
    assert_eq!(pos, 50000);
}

#[test]
fn protocol_detent_response_idle_and_afterburner() {
    let idle_pos = 100u16.to_le_bytes();
    let ab_pos = 62000u16.to_le_bytes();
    let payload = [
        0, 0, idle_pos[0], idle_pos[1], 0,
        0, 1, ab_pos[0], ab_pos[1], 0,
    ];
    let report = parse_detent_response(&payload).unwrap();
    assert_eq!(report.positions.len(), 2);
    assert_eq!(report.positions[0].name, DetentName::Idle);
    assert_eq!(report.positions[0].raw_position, 100);
    assert_eq!(report.positions[1].name, DetentName::Afterburner);
    assert_eq!(report.positions[1].raw_position, 62000);
}

#[test]
fn protocol_detent_response_custom_detent() {
    let pos = 32000u16.to_le_bytes();
    let payload = [1, 5, pos[0], pos[1], 0];
    let report = parse_detent_response(&payload).unwrap();
    assert_eq!(report.positions[0].name, DetentName::Custom(5));
    assert_eq!(report.positions[0].lever, 1);
}

// ═══════════════════════════════════════════════════════════════════════════════
// § 14 — Protocol: error handling
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn protocol_empty_frame_rejected() {
    let err = parse_feature_report(&[]).unwrap_err();
    assert!(matches!(err, ProtocolError::FrameTooShort { .. }));
}

#[test]
fn protocol_invalid_report_id_rejected() {
    let frame = FeatureReportFrame::new(CommandCategory::Display, 0x01, &[]).unwrap();
    let mut bytes = frame.as_bytes().to_vec();
    bytes[0] = 0xAA;
    let err = parse_feature_report(&bytes).unwrap_err();
    assert!(matches!(err, ProtocolError::InvalidReportId { .. }));
}

#[test]
fn protocol_corrupted_checksum_rejected() {
    let frame = FeatureReportFrame::new(CommandCategory::Display, 0x01, &[1, 2, 3]).unwrap();
    let mut bytes = frame.as_bytes().to_vec();
    let last = bytes.len() - 1;
    bytes[last] ^= 0xFF;
    let err = parse_feature_report(&bytes).unwrap_err();
    assert!(matches!(err, ProtocolError::ChecksumMismatch { .. }));
}

#[test]
fn protocol_detent_bad_payload_length() {
    let err = parse_detent_response(&[1, 2, 3]).unwrap_err();
    assert!(matches!(err, ProtocolError::InvalidDetentPayload { .. }));
}

// ═══════════════════════════════════════════════════════════════════════════════
// § 15 — Error type coverage — every parser rejects empty/short/bad-ID
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn all_parsers_reject_empty_input() {
    let empty: &[u8] = &[];
    assert!(parse_throttle_report(empty).is_err());
    assert!(parse_stick_report(empty).is_err());
    assert!(parse_rudder_report(empty).is_err());
    assert!(parse_orion2_throttle_report(empty).is_err());
    assert!(parse_orion2_stick_report(empty).is_err());
    assert!(parse_tfrp_report(empty).is_err());
    assert!(parse_f16ex_stick_report(empty).is_err());
    assert!(parse_super_taurus_report(empty).is_err());
    assert!(parse_ufc_panel_report(empty).is_err());
    assert!(parse_skywalker_rudder_report(empty).is_err());
    assert!(parse_orion_joystick(empty).is_err());
    assert!(parse_orion2_throttle(empty).is_err());
}

#[test]
fn all_parsers_reject_wrong_report_id() {
    // Build correctly-sized reports with 0xFF as the report ID.
    let bad_throttle = {
        let mut r = vec![0u8; THROTTLE_REPORT_LEN];
        r[0] = 0xFF;
        r
    };
    assert!(parse_throttle_report(&bad_throttle).is_err());
    assert!(parse_orion2_throttle_report(&bad_throttle).is_err());

    let bad_stick = {
        let mut r = vec![0u8; STICK_REPORT_LEN];
        r[0] = 0xFF;
        r
    };
    assert!(parse_stick_report(&bad_stick).is_err());
    assert!(parse_orion2_stick_report(&bad_stick).is_err());

    let bad_rudder = {
        let mut r = vec![0u8; RUDDER_REPORT_LEN];
        r[0] = 0xFF;
        r
    };
    assert!(parse_rudder_report(&bad_rudder).is_err());
    assert!(parse_tfrp_report(&bad_rudder).is_err());

    let bad_f16 = {
        let mut r = vec![0u8; F16EX_REPORT_LEN];
        r[0] = 0xFF;
        r
    };
    assert!(parse_f16ex_stick_report(&bad_f16).is_err());

    let bad_st = {
        let mut r = vec![0u8; SUPER_TAURUS_REPORT_LEN];
        r[0] = 0xFF;
        r
    };
    assert!(parse_super_taurus_report(&bad_st).is_err());

    let bad_ufc = {
        let mut r = vec![0u8; UFC_PANEL_REPORT_LEN];
        r[0] = 0xFF;
        r
    };
    assert!(parse_ufc_panel_report(&bad_ufc).is_err());

    let bad_sky = {
        let mut r = vec![0u8; SKYWALKER_RUDDER_REPORT_LEN];
        r[0] = 0xFF;
        r
    };
    assert!(parse_skywalker_rudder_report(&bad_sky).is_err());
}

// ═══════════════════════════════════════════════════════════════════════════════
// § 16 — Health monitor cross-device
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn health_monitor_success_resets_after_two_failures() {
    let mut m = WinWingHealthMonitor::new(WinWingDevice::Orion2Throttle);
    m.record_failure();
    m.record_failure();
    assert!(!m.is_offline());
    m.record_success();
    assert!(!m.is_offline());
    assert_eq!(m.status().consecutive_failures, 0);
}

#[test]
fn health_monitor_all_device_variants_display() {
    let devices = [
        WinWingDevice::Orion2Throttle,
        WinWingDevice::Orion2Stick,
        WinWingDevice::TfrpRudder,
    ];
    for dev in devices {
        let name = dev.to_string();
        assert!(!name.is_empty());
        assert!(name.contains("WinWing"));
    }
}

#[test]
fn health_status_healthy_thresholds() {
    let mut m = WinWingHealthMonitor::new(WinWingDevice::TfrpRudder);
    assert!(m.status().is_healthy());
    m.record_failure();
    m.record_failure();
    // 2 failures: still healthy (threshold is 3)
    assert!(m.status().is_healthy());
    m.record_failure();
    // 3 failures: no longer healthy
    assert!(!m.status().is_healthy());
}

// ═══════════════════════════════════════════════════════════════════════════════
// § 17 — Presets coverage
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn preset_configs_have_valid_deadzones() {
    for cfg in &orion2_throttle_config() {
        assert!(cfg.deadzone >= 0.0 && cfg.deadzone <= 1.0, "bad deadzone for {}", cfg.name);
    }
    for cfg in &orion2_stick_config() {
        assert!(cfg.deadzone >= 0.0 && cfg.deadzone <= 1.0, "bad deadzone for {}", cfg.name);
    }
    for cfg in &tfrp_rudder_config() {
        assert!(cfg.deadzone >= 0.0 && cfg.deadzone <= 1.0, "bad deadzone for {}", cfg.name);
    }
}

#[test]
fn preset_filter_alpha_within_range() {
    let all_configs: Vec<_> = orion2_throttle_config()
        .iter()
        .chain(orion2_stick_config().iter())
        .chain(tfrp_rudder_config().iter())
        .filter_map(|c| c.filter_alpha)
        .collect();
    for alpha in all_configs {
        assert!(alpha > 0.0 && alpha <= 1.0, "filter_alpha {alpha} out of (0, 1]");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// § 18 — Profile lookup and catalogue
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn profile_by_pid_returns_correct_names() {
    let throttle = profile_by_pid(0xBE62).unwrap();
    assert!(throttle.name.contains("Throttle"));
    let base = profile_by_pid(0xBE63).unwrap();
    assert!(base.name.contains("Orion 2"));
    let f16 = profile_by_pid(0xBEA8).unwrap();
    assert!(f16.name.contains("F-16EX"));
}

#[test]
fn profile_by_pid_unknown_returns_none() {
    assert!(profile_by_pid(0x0000).is_none());
    assert!(profile_by_pid(0xFFFF).is_none());
}

#[test]
fn all_profiles_have_consistent_button_groups() {
    for p in all_profiles() {
        let sum: u8 = p.button_groups.iter().map(|g| g.count).sum();
        assert_eq!(
            sum, p.button_count,
            "{}: groups sum {sum} != button_count {}",
            p.name, p.button_count
        );
    }
}

#[test]
fn all_profiles_use_winwing_vid() {
    for p in all_profiles() {
        assert_eq!(p.vid, 0x4098, "{} VID mismatch", p.name);
    }
}

#[test]
fn all_profiles_pid_count() {
    // Some profiles may share a PID (e.g. F-18 grip and Orion 2 base are
    // the same physical device), so we just verify the catalogue is non-empty.
    let profiles = all_profiles();
    assert!(profiles.len() >= 9, "expected at least 9 profiles");
    for p in &profiles {
        assert_ne!(p.pid, 0, "{} has zero PID", p.name);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// § 19 — WinWingError Display formatting
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn winwing_error_report_too_short_display() {
    let err = WinWingError::ReportTooShort { need: 14, got: 5 };
    let msg = err.to_string();
    assert!(msg.contains("14") && msg.contains("5"));
}

#[test]
fn winwing_error_unknown_report_id_display() {
    let err = WinWingError::UnknownReportId(0xAB);
    let msg = err.to_string();
    assert!(msg.contains("0xAB") || msg.contains("0xab"));
}

// ═══════════════════════════════════════════════════════════════════════════════
// § 20 — Oversized reports are accepted (extra trailing bytes ignored)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn parsers_accept_oversized_reports() {
    // All parsers should accept reports that are longer than the minimum.
    let throttle = {
        let mut r = vec![0u8; THROTTLE_REPORT_LEN + 10];
        r[0] = 0x01;
        r
    };
    assert!(parse_throttle_report(&throttle).is_ok());

    let stick = {
        let mut r = vec![0u8; STICK_REPORT_LEN + 10];
        r[0] = 0x02;
        r[9] = 0x0F;
        r[10] = 0x0F;
        r
    };
    assert!(parse_stick_report(&stick).is_ok());
    assert!(parse_orion2_stick_report(&stick).is_ok());

    let rudder = {
        let mut r = vec![0u8; RUDDER_REPORT_LEN + 10];
        r[0] = 0x03;
        r
    };
    assert!(parse_rudder_report(&rudder).is_ok());
    assert!(parse_tfrp_report(&rudder).is_ok());

    let f16 = {
        let mut r = vec![0u8; F16EX_REPORT_LEN + 10];
        r[0] = 0x04;
        r[9] = 0x0F;
        r
    };
    assert!(parse_f16ex_stick_report(&f16).is_ok());

    let st = {
        let mut r = vec![0u8; SUPER_TAURUS_REPORT_LEN + 10];
        r[0] = 0x05;
        r
    };
    assert!(parse_super_taurus_report(&st).is_ok());

    let ufc = {
        let mut r = vec![0u8; UFC_PANEL_REPORT_LEN + 10];
        r[0] = 0x06;
        r
    };
    assert!(parse_ufc_panel_report(&ufc).is_ok());

    let sky = {
        let mut r = vec![0u8; SKYWALKER_RUDDER_REPORT_LEN + 10];
        r[0] = 0x07;
        r
    };
    assert!(parse_skywalker_rudder_report(&sky).is_ok());
}
