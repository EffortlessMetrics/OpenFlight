// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for Moza AB9 torque command encoding — covers mid-range values,
//! combined axes, byte layout, symmetry, and FfbMode variants.

use flight_ffb_moza::effects::{FfbMode, TORQUE_REPORT_ID, TORQUE_REPORT_LEN, TorqueCommand};

fn decode_x(report: &[u8; TORQUE_REPORT_LEN]) -> i16 {
    i16::from_le_bytes([report[1], report[2]])
}

fn decode_y(report: &[u8; TORQUE_REPORT_LEN]) -> i16 {
    i16::from_le_bytes([report[3], report[4]])
}

// ── Mid-range torque values ─────────────────────────────────────────────

#[test]
fn half_positive_x_torque() {
    let cmd = TorqueCommand { x: 0.5, y: 0.0 };
    let r = cmd.to_report();
    let x = decode_x(&r);
    // 0.5 * 32767 = 16383.5 → truncated to 16383
    assert_eq!(x, 16383);
}

#[test]
fn quarter_negative_y_torque() {
    let cmd = TorqueCommand { x: 0.0, y: -0.25 };
    let r = cmd.to_report();
    let y = decode_y(&r);
    // -0.25 * 32767 = -8191.75 → truncated to -8191
    assert_eq!(y, -8191);
}

#[test]
fn three_quarter_positive_torque() {
    let cmd = TorqueCommand { x: 0.75, y: 0.75 };
    let r = cmd.to_report();
    let x = decode_x(&r);
    let y = decode_y(&r);
    // 0.75 * 32767 = 24575.25 → 24575
    assert_eq!(x, 24575);
    assert_eq!(y, 24575);
}

// ── Combined axes ───────────────────────────────────────────────────────

#[test]
fn both_axes_at_full_positive() {
    let cmd = TorqueCommand { x: 1.0, y: 1.0 };
    let r = cmd.to_report();
    assert_eq!(decode_x(&r), 32767);
    assert_eq!(decode_y(&r), 32767);
}

#[test]
fn both_axes_at_full_negative() {
    let cmd = TorqueCommand { x: -1.0, y: -1.0 };
    let r = cmd.to_report();
    assert_eq!(decode_x(&r), -32767);
    assert_eq!(decode_y(&r), -32767);
}

#[test]
fn opposing_axes() {
    let cmd = TorqueCommand { x: 1.0, y: -1.0 };
    let r = cmd.to_report();
    assert_eq!(decode_x(&r), 32767);
    assert_eq!(decode_y(&r), -32767);
}

// ── Byte layout verification ────────────────────────────────────────────

#[test]
fn report_byte_layout() {
    let cmd = TorqueCommand { x: 0.5, y: -0.5 };
    let r = cmd.to_report();
    assert_eq!(r.len(), TORQUE_REPORT_LEN);
    assert_eq!(r[0], TORQUE_REPORT_ID, "byte 0 must be report ID");
    // byte 5 is padding
    assert_eq!(r[5], 0, "padding byte should be zero");
}

#[test]
fn report_length_constant() {
    assert_eq!(TORQUE_REPORT_LEN, 6, "report must be exactly 6 bytes");
}

#[test]
fn report_id_constant() {
    assert_eq!(TORQUE_REPORT_ID, 0x20);
}

// ── Torque symmetry ────────────────────────────────────────────────────

#[test]
fn x_torque_symmetry() {
    let pos = TorqueCommand { x: 0.6, y: 0.0 }.to_report();
    let neg = TorqueCommand { x: -0.6, y: 0.0 }.to_report();
    let x_pos = decode_x(&pos);
    let x_neg = decode_x(&neg);
    // Due to f32→i16 truncation, symmetry may differ by 1 LSB
    assert!((x_pos + x_neg).abs() <= 1, "x axis should be symmetric");
}

#[test]
fn y_torque_symmetry() {
    let pos = TorqueCommand { x: 0.0, y: 0.3 }.to_report();
    let neg = TorqueCommand { x: 0.0, y: -0.3 }.to_report();
    let y_pos = decode_y(&pos);
    let y_neg = decode_y(&neg);
    assert!((y_pos + y_neg).abs() <= 1, "y axis should be symmetric");
}

// ── Clamping edge cases ─────────────────────────────────────────────────

#[test]
fn large_positive_clamped_to_max() {
    let cmd = TorqueCommand { x: 100.0, y: 50.0 };
    let r = cmd.to_report();
    assert_eq!(decode_x(&r), 32767);
    assert_eq!(decode_y(&r), 32767);
}

#[test]
fn large_negative_clamped_to_min() {
    let cmd = TorqueCommand { x: -100.0, y: -50.0 };
    let r = cmd.to_report();
    assert_eq!(decode_x(&r), -32767);
    assert_eq!(decode_y(&r), -32767);
}

#[test]
fn exactly_one_produces_max() {
    let cmd = TorqueCommand { x: 1.0, y: -1.0 };
    let r = cmd.to_report();
    assert_eq!(decode_x(&r), 32767);
    assert_eq!(decode_y(&r), -32767);
}

// ── FfbMode coverage ────────────────────────────────────────────────────

#[test]
fn ffb_mode_display_all_variants() {
    assert_eq!(FfbMode::Passive.to_string(), "Passive");
    assert_eq!(FfbMode::Spring.to_string(), "Spring");
    assert_eq!(FfbMode::Damper.to_string(), "Damper");
    assert_eq!(FfbMode::Direct.to_string(), "Direct");
}

#[test]
fn ffb_mode_equality() {
    assert_eq!(FfbMode::Passive, FfbMode::Passive);
    assert_ne!(FfbMode::Passive, FfbMode::Direct);
    assert_ne!(FfbMode::Spring, FfbMode::Damper);
}

#[test]
fn ffb_mode_clone() {
    let mode = FfbMode::Direct;
    let cloned = mode.clone();
    assert_eq!(mode, cloned);
}

// ── TorqueCommand traits ────────────────────────────────────────────────

#[test]
fn torque_command_debug() {
    let cmd = TorqueCommand { x: 0.1, y: 0.2 };
    let dbg = format!("{cmd:?}");
    assert!(dbg.contains("TorqueCommand"));
    assert!(dbg.contains("0.1"));
}

#[test]
fn torque_command_clone() {
    let cmd = TorqueCommand { x: 0.5, y: -0.5 };
    let cloned = cmd.clone();
    assert_eq!(cmd, cloned);
}

#[test]
fn torque_command_partial_eq() {
    let a = TorqueCommand { x: 0.5, y: 0.5 };
    let b = TorqueCommand { x: 0.5, y: 0.5 };
    let c = TorqueCommand { x: 0.5, y: 0.6 };
    assert_eq!(a, b);
    assert_ne!(a, c);
}

// ── ZERO constant ───────────────────────────────────────────────────────

#[test]
fn zero_constant_values() {
    assert_eq!(TorqueCommand::ZERO.x, 0.0);
    assert_eq!(TorqueCommand::ZERO.y, 0.0);
}

#[test]
fn zero_constant_is_safe() {
    assert!(TorqueCommand::ZERO.is_safe());
}
