// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests for the WinWing Orion 2 throttle and Orion joystick parsers.

use flight_hotas_winwing::{
    WinWingError, normalize_axis_16bit, normalize_throttle_16bit, parse_orion_joystick,
    parse_orion2_throttle,
};

// ── helpers ───────────────────────────────────────────────────────────────────

/// Build a minimal 14-byte Orion 2 throttle report.
///
/// Layout: report_id + 4×u16-LE axes + 5 button bytes.
fn make_throttle_report(
    throttle_main: u16,
    throttle_secondary: u16,
    axis3: u16,
    axis4: u16,
    buttons: [u8; 5],
) -> [u8; 14] {
    let mut r = [0u8; 14];
    r[0] = 0x01;
    r[1..3].copy_from_slice(&throttle_main.to_le_bytes());
    r[3..5].copy_from_slice(&throttle_secondary.to_le_bytes());
    r[5..7].copy_from_slice(&axis3.to_le_bytes());
    r[7..9].copy_from_slice(&axis4.to_le_bytes());
    r[9..14].copy_from_slice(&buttons);
    r
}

/// Build a minimal 12-byte Orion joystick report.
///
/// Layout: report_id + 3×u16-LE axes + 4 button bytes + hat byte.
fn make_joystick_report(x: u16, y: u16, twist: u16, buttons: [u8; 4], hat: u8) -> [u8; 12] {
    let mut r = [0u8; 12];
    r[0] = 0x01;
    r[1..3].copy_from_slice(&x.to_le_bytes());
    r[3..5].copy_from_slice(&y.to_le_bytes());
    r[5..7].copy_from_slice(&twist.to_le_bytes());
    r[7..11].copy_from_slice(&buttons);
    r[11] = hat;
    r
}

// ── Orion 2 throttle ──────────────────────────────────────────────────────────

#[test]
fn test_parse_orion2_throttle_full_deflection() {
    let report = make_throttle_report(0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, [0u8; 5]);
    let state = parse_orion2_throttle(&report).unwrap();
    assert_eq!(state.throttle_main, 0xFFFF);
    assert_eq!(state.throttle_secondary, 0xFFFF);
    assert_eq!(state.axis3, 0xFFFF);
    assert_eq!(state.axis4, 0xFFFF);
    assert_eq!(state.buttons, 0);
}

#[test]
fn test_parse_orion2_throttle_zero() {
    let report = make_throttle_report(0, 0, 0, 0, [0u8; 5]);
    let state = parse_orion2_throttle(&report).unwrap();
    assert_eq!(state.throttle_main, 0);
    assert_eq!(state.throttle_secondary, 0);
    assert_eq!(state.axis3, 0);
    assert_eq!(state.axis4, 0);
    assert_eq!(state.buttons, 0);
}

#[test]
fn test_parse_orion2_throttle_midpoint() {
    let mid = 0x8000u16;
    let report = make_throttle_report(mid, mid, mid, mid, [0u8; 5]);
    let state = parse_orion2_throttle(&report).unwrap();
    assert_eq!(state.throttle_main, mid);
    assert_eq!(state.throttle_secondary, mid);
    assert_eq!(state.axis3, mid);
    assert_eq!(state.axis4, mid);
}

#[test]
fn test_parse_orion2_throttle_buttons() {
    // Set buttons 1 (bit 0) and 9 (bit 8 → second button byte bit 0)
    let buttons = [0x01u8, 0x01, 0x00, 0x00, 0x00];
    let report = make_throttle_report(0, 0, 0, 0, buttons);
    let state = parse_orion2_throttle(&report).unwrap();
    assert_ne!(state.buttons, 0);
    assert!(state.buttons & 1 != 0, "button 1 (bit 0) should be set");
    assert!(
        state.buttons & (1 << 8) != 0,
        "button 9 (bit 8) should be set"
    );
}

#[test]
fn test_parse_orion2_throttle_too_short() {
    let err = parse_orion2_throttle(&[0u8; 10]).unwrap_err();
    assert_eq!(err, WinWingError::ReportTooShort { need: 14, got: 10 });
}

// ── Orion joystick ────────────────────────────────────────────────────────────

#[test]
fn test_parse_orion_joystick_center() {
    let center = 0x8000u16;
    let report = make_joystick_report(center, center, center, [0u8; 4], 8);
    let state = parse_orion_joystick(&report).unwrap();
    assert_eq!(state.x, center);
    assert_eq!(state.y, center);
    assert_eq!(state.twist, center);
    assert_eq!(state.buttons, 0);
    assert_eq!(state.hat, 8); // center
}

#[test]
fn test_parse_orion_joystick_full_deflection() {
    let report = make_joystick_report(0xFFFF, 0xFFFF, 0xFFFF, [0xFFu8; 4], 8);
    let state = parse_orion_joystick(&report).unwrap();
    assert_eq!(state.x, 0xFFFF);
    assert_eq!(state.y, 0xFFFF);
    assert_eq!(state.twist, 0xFFFF);
    assert_eq!(state.buttons, 0xFFFF_FFFF);
    assert_eq!(state.hat, 8);
}

#[test]
fn test_parse_orion_joystick_hat_north() {
    let report = make_joystick_report(0, 0, 0, [0u8; 4], 0); // 0 = North
    let state = parse_orion_joystick(&report).unwrap();
    assert_eq!(state.hat, 0);
}

#[test]
fn test_parse_orion_joystick_hat_center() {
    let report = make_joystick_report(0, 0, 0, [0u8; 4], 8); // 8 = center
    let state = parse_orion_joystick(&report).unwrap();
    assert_eq!(state.hat, 8);
}

#[test]
fn test_parse_orion_joystick_too_short() {
    let err = parse_orion_joystick(&[0u8; 8]).unwrap_err();
    assert_eq!(err, WinWingError::ReportTooShort { need: 12, got: 8 });
}

// ── Normalization ─────────────────────────────────────────────────────────────

#[test]
fn test_normalize_axis_16bit_center() {
    // 32767 → (32767 / 32767.5) - 1.0 ≈ 0.0
    let v = normalize_axis_16bit(32767);
    assert!(v.abs() < 1e-4, "center should map to ~0.0, got {v}");
}

#[test]
fn test_normalize_throttle_16bit_max() {
    let v = normalize_throttle_16bit(65535);
    assert!((v - 1.0).abs() < 1e-6, "max should map to 1.0, got {v}");
}
