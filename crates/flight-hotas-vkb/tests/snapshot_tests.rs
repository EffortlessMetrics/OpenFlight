// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Snapshot tests for VKB device parser outputs.
//!
//! These tests pin the exact `Debug` representation of parsed HID reports at
//! known input values.  Any change to the struct layout, normalisation formula,
//! or enum variant naming will surface as a diff before it reaches users.

use flight_hotas_vkb::{
    GladiatorInputHandler, StecsInputHandler, StecsMtVariant, VkbGladiatorVariant, VkbStecsVariant,
    parse_stecs_mt_report,
};

// ── report builders ───────────────────────────────────────────────────────────

/// Build a 21-byte Gladiator NXT EVO report.
///
/// Layout: 6×u16-LE axes (roll, pitch, yaw, mini_x, mini_y, throttle),
/// 2×u32-LE button words, 1 hat byte.
fn gladiator_report(axes: [u16; 6], btn_lo: u32, btn_hi: u32, hat_byte: u8) -> Vec<u8> {
    let mut report = vec![0u8; 21];
    for (i, &v) in axes.iter().enumerate() {
        let bytes = v.to_le_bytes();
        report[i * 2] = bytes[0];
        report[i * 2 + 1] = bytes[1];
    }
    report[12..16].copy_from_slice(&btn_lo.to_le_bytes());
    report[16..20].copy_from_slice(&btn_hi.to_le_bytes());
    report[20] = hat_byte;
    report
}

/// Build a 17-byte STECS Modern Throttle report (including the report_id byte).
fn stecs_mt_report(
    throttle: u16,
    mini_left: u16,
    mini_right: u16,
    rotary: u16,
    w0: u32,
    w1: u32,
) -> Vec<u8> {
    let mut data = vec![0x01u8]; // report_id
    data.extend_from_slice(&throttle.to_le_bytes());
    data.extend_from_slice(&mini_left.to_le_bytes());
    data.extend_from_slice(&mini_right.to_le_bytes());
    data.extend_from_slice(&rotary.to_le_bytes());
    data.extend_from_slice(&w0.to_le_bytes());
    data.extend_from_slice(&w1.to_le_bytes());
    data
}

// ── Gladiator NXT EVO snapshots ───────────────────────────────────────────────

/// Pin the parsed state of the Gladiator NXT EVO Right at the neutral position.
///
/// All bidirectional axes at centre (0x8000 → ~0.0), throttle wheel at zero,
/// no buttons pressed, both hats centred (0xFF).
#[test]
fn snapshot_gladiator_nxt_evo_right_neutral() {
    let report = gladiator_report([0x8000, 0x8000, 0x8000, 0x8000, 0x8000, 0x0000], 0, 0, 0xFF);
    let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoRight);
    let state = handler.parse_report(&report).expect("valid report");
    insta::assert_debug_snapshot!("gladiator_nxt_evo_right_neutral", state);
}

/// Pin the parsed state of the Gladiator NXT EVO Left at full throttle wheel.
///
/// All stick axes at centre, throttle wheel at maximum (0xFFFF → 1.0),
/// no buttons, hats centred.
#[test]
fn snapshot_gladiator_nxt_evo_left_full_throttle() {
    let report = gladiator_report([0x8000, 0x8000, 0x8000, 0x8000, 0x8000, 0xFFFF], 0, 0, 0xFF);
    let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoLeft);
    let state = handler.parse_report(&report).expect("valid report");
    insta::assert_debug_snapshot!("gladiator_nxt_evo_left_full_throttle", state);
}

// ── STECS Space interface snapshots ──────────────────────────────────────────

/// Pin the parsed state of the STECS Right Mini interface in the neutral position.
///
/// 4-byte buttons-only report (no axes block), all buttons unpressed.
#[test]
fn snapshot_stecs_right_mini_neutral_buttons_only() {
    let handler = StecsInputHandler::new(VkbStecsVariant::RightSpaceThrottleGripMini);
    let report = [0x00u8, 0x00, 0x00, 0x00];
    let state = handler
        .parse_interface_report(&report)
        .expect("valid report");
    insta::assert_debug_snapshot!("stecs_right_mini_neutral_buttons_only", state);
}

/// Pin the parsed state of the STECS Standard interface with full axes and some buttons.
///
/// 14-byte full report: all axes at zero (0x0000 → 0.0), buttons 1 and 8 pressed.
#[test]
fn snapshot_stecs_standard_full_report_with_axes() {
    let handler = StecsInputHandler::new(VkbStecsVariant::RightSpaceThrottleGripStandard);
    // 5×u16 axes all zero, then u32 buttons = 0x81 (bits 0 and 7)
    let mut report = vec![0u8; 14];
    report[10] = 0x81; // button 1 and 8 set (bit 0 and bit 7)
    let state = handler
        .parse_interface_report(&report)
        .expect("valid report");
    insta::assert_debug_snapshot!("stecs_standard_full_report_with_axes", state);
}

// ── STECS Modern Throttle snapshots ──────────────────────────────────────────

/// Pin the parsed state of the STECS Modern Throttle Mini at idle / all-zero.
///
/// All four axes at zero (0x0000 → 0.0), no buttons pressed.
#[test]
fn snapshot_stecs_mt_mini_idle() {
    let report = stecs_mt_report(0, 0, 0, 0, 0, 0);
    let state = parse_stecs_mt_report(&report, StecsMtVariant::Mini).expect("valid report");
    insta::assert_debug_snapshot!("stecs_mt_mini_idle", state);
}

/// Pin the parsed state of the STECS Modern Throttle Max at full travel.
///
/// All axes at maximum (0xFFFF → 1.0), no buttons pressed.
#[test]
fn snapshot_stecs_mt_max_full() {
    let report = stecs_mt_report(u16::MAX, u16::MAX, u16::MAX, u16::MAX, 0, 0);
    let state = parse_stecs_mt_report(&report, StecsMtVariant::Max).expect("valid report");
    insta::assert_debug_snapshot!("stecs_mt_max_full", state);
}
