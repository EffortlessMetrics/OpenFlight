// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

use flight_hotas_logitech_wheel::{
    WheelError, normalize_pedal, normalize_wheel, parse_g27, parse_g29,
};

// ── G29 helpers ──────────────────────────────────────────────────────────────

/// Build a valid 12-byte G29 report.
fn g29_report(wheel: u16, gas: u16, brake: u16, clutch: u16, buttons: u16, hat: u8) -> [u8; 12] {
    let [wl, wh] = wheel.to_le_bytes();
    let [gl, gh] = gas.to_le_bytes();
    let [bl, bh] = brake.to_le_bytes();
    let [cl, ch] = clutch.to_le_bytes();
    let [btl, bth] = buttons.to_le_bytes();
    [0x01, wl, wh, gl, gh, bl, bh, cl, ch, btl, bth, hat]
}

// ── G27 helpers ──────────────────────────────────────────────────────────────

/// Build a valid 11-byte G27 report.
fn g27_report(wheel: u16, gas: u16, brake: u16, clutch: u16, buttons: u16) -> [u8; 11] {
    let [wl, wh] = wheel.to_le_bytes();
    let [gl, gh] = gas.to_le_bytes();
    let [bl, bh] = brake.to_le_bytes();
    let [cl, ch] = clutch.to_le_bytes();
    let [btl, bth] = buttons.to_le_bytes();
    [0x01, wl, wh, gl, gh, bl, bh, cl, ch, btl, bth]
}

// ── G29 tests ─────────────────────────────────────────────────────────────────

#[test]
fn test_g29_parse_center_position() {
    let report = g29_report(32768, 0, 0, 0, 0, 8);
    let state = parse_g29(&report).unwrap();
    assert_eq!(state.wheel, 32768);
    assert_eq!(state.hat, 8);
    let norm = normalize_wheel(state.wheel);
    assert!(norm.abs() < 0.01, "wheel near center: {norm}");
}

#[test]
fn test_g29_parse_full_left() {
    let report = g29_report(0, 0, 0, 0, 0, 8);
    let state = parse_g29(&report).unwrap();
    assert_eq!(state.wheel, 0);
    let norm = normalize_wheel(state.wheel);
    assert!(norm < -0.999, "wheel full left: {norm}");
}

#[test]
fn test_g29_parse_full_right() {
    let report = g29_report(65535, 0, 0, 0, 0, 8);
    let state = parse_g29(&report).unwrap();
    assert_eq!(state.wheel, 65535);
    let norm = normalize_wheel(state.wheel);
    assert!(norm > 0.999, "wheel full right: {norm}");
}

#[test]
fn test_g29_gas_full() {
    let report = g29_report(32768, 65535, 0, 0, 0, 8);
    let state = parse_g29(&report).unwrap();
    assert_eq!(state.gas, 65535);
    let norm = normalize_pedal(state.gas);
    assert!((norm - 1.0).abs() < 0.001, "gas full: {norm}");
}

#[test]
fn test_g29_brake_full() {
    let report = g29_report(32768, 0, 65535, 0, 0, 8);
    let state = parse_g29(&report).unwrap();
    assert_eq!(state.brake, 65535);
    let norm = normalize_pedal(state.brake);
    assert!((norm - 1.0).abs() < 0.001, "brake full: {norm}");
}

#[test]
fn test_g29_buttons_set() {
    let report = g29_report(32768, 0, 0, 0, 0b1010_1010_0101_0101, 8);
    let state = parse_g29(&report).unwrap();
    assert_eq!(state.buttons, 0b1010_1010_0101_0101);
}

#[test]
fn test_g29_too_short() {
    assert_eq!(
        parse_g29(&[0x01; 11]).unwrap_err(),
        WheelError::TooShort { need: 12, got: 11 }
    );
    assert_eq!(
        parse_g29(&[]).unwrap_err(),
        WheelError::TooShort { need: 12, got: 0 }
    );
}

#[test]
fn test_g29_invalid_report_id() {
    let mut report = g29_report(32768, 0, 0, 0, 0, 8);
    report[0] = 0x02;
    assert_eq!(
        parse_g29(&report).unwrap_err(),
        WheelError::InvalidReportId(0x02)
    );
}

// ── G27 tests ─────────────────────────────────────────────────────────────────

#[test]
fn test_g27_parse_center() {
    let report = g27_report(32768, 0, 0, 0, 0);
    let state = parse_g27(&report).unwrap();
    assert_eq!(state.wheel, 32768);
    let norm = normalize_wheel(state.wheel);
    assert!(norm.abs() < 0.01, "G27 wheel near center: {norm}");
}

#[test]
fn test_g27_parse_buttons() {
    let report = g27_report(32768, 0, 0, 0, 0xBEEF);
    let state = parse_g27(&report).unwrap();
    assert_eq!(state.buttons, 0xBEEF_u32);
}

#[test]
fn test_g27_too_short() {
    assert_eq!(
        parse_g27(&[0x01; 10]).unwrap_err(),
        WheelError::TooShort { need: 11, got: 10 }
    );
    assert_eq!(
        parse_g27(&[]).unwrap_err(),
        WheelError::TooShort { need: 11, got: 0 }
    );
}

// ── Normalizer tests ──────────────────────────────────────────────────────────

#[test]
fn test_normalize_wheel_center() {
    let norm = normalize_wheel(32768);
    assert!(norm.abs() < 0.01, "center wheel should be ~0.0: {norm}");
}

#[test]
fn test_normalize_pedal_max() {
    let norm = normalize_pedal(65535);
    assert!(
        (norm - 1.0).abs() < 0.001,
        "max pedal should be ~1.0: {norm}"
    );
}
