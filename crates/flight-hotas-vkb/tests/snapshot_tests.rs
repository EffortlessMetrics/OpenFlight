// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Snapshot tests for VKB device parser outputs.
//!
//! These tests pin the exact `Debug` representation of parsed HID reports at
//! known input values.  Any change to the struct layout, normalisation formula,
//! or enum variant naming will surface as a diff before it reaches users.

use flight_hotas_vkb::{
    GladiatorInputHandler, GunfighterInputHandler, GunfighterVariant, SemThqInputHandler,
    StecsInputHandler, StecsMtVariant, TRudderInputHandler, TRudderVariant, VkbGladiatorVariant,
    VkbStecsVariant, parse_stecs_mt_report,
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

// ── Gunfighter snapshots ─────────────────────────────────────────────────────

/// Pin the parsed state of the Gunfighter MCG Pro at neutral position.
#[test]
fn snapshot_gunfighter_mcg_neutral() {
    let report = gladiator_report([0x8000, 0x8000, 0x8000, 0x8000, 0x8000, 0x0000], 0, 0, 0xFF);
    let handler = GunfighterInputHandler::new(GunfighterVariant::ModernCombatPro);
    let state = handler.parse_report(&report).expect("valid report");
    insta::assert_debug_snapshot!("gunfighter_mcg_neutral", state);
}

/// Pin the parsed state of the Gunfighter with full deflection and buttons.
#[test]
fn snapshot_gunfighter_full_deflection_with_buttons() {
    let report = gladiator_report(
        [0xFFFF, 0x0000, 0xFFFF, 0x0000, 0xFFFF, 0xFFFF],
        0x8000_0001, // buttons 1 and 32
        0x0000_0004, // button 35
        0xF2,        // hat0=E(2), hat1=centred
    );
    let handler = GunfighterInputHandler::new(GunfighterVariant::SpaceGunfighter);
    let state = handler.parse_report(&report).expect("valid report");
    insta::assert_debug_snapshot!("gunfighter_full_deflection_buttons", state);
}

// ── SEM THQ snapshots ────────────────────────────────────────────────────────

/// Build a 16-byte SEM THQ report.
fn sem_thq_report(axes: [u16; 4], btn_lo: u32, btn_hi: u32) -> Vec<u8> {
    let mut report = vec![0u8; 16];
    for (i, &v) in axes.iter().enumerate() {
        let bytes = v.to_le_bytes();
        report[i * 2] = bytes[0];
        report[i * 2 + 1] = bytes[1];
    }
    report[8..12].copy_from_slice(&btn_lo.to_le_bytes());
    report[12..16].copy_from_slice(&btn_hi.to_le_bytes());
    report
}

/// Pin the SEM THQ at idle position.
#[test]
fn snapshot_sem_thq_idle() {
    let report = sem_thq_report([0, 0, 0, 0], 0, 0);
    let handler = SemThqInputHandler::new();
    let state = handler.parse_report(&report).expect("valid report");
    insta::assert_debug_snapshot!("sem_thq_idle", state);
}

/// Pin the SEM THQ at full throttle with some buttons.
#[test]
fn snapshot_sem_thq_full_throttle_buttons() {
    let report = sem_thq_report([0xFFFF, 0xFFFF, 0x8000, 0x8000], 0x0000_0005, 0);
    let handler = SemThqInputHandler::new();
    let state = handler.parse_report(&report).expect("valid report");
    insta::assert_debug_snapshot!("sem_thq_full_throttle_buttons", state);
}

// ── T-Rudder snapshots ───────────────────────────────────────────────────────

/// Build a 6-byte T-Rudder report.
fn t_rudder_report(left: u16, right: u16, rudder: u16) -> Vec<u8> {
    let mut data = Vec::with_capacity(6);
    data.extend_from_slice(&left.to_le_bytes());
    data.extend_from_slice(&right.to_le_bytes());
    data.extend_from_slice(&rudder.to_le_bytes());
    data
}

/// Pin T-Rudder Mk.IV at idle (brakes released, rudder centred).
#[test]
fn snapshot_t_rudder_mk4_idle() {
    let report = t_rudder_report(0, 0, 0x8000);
    let handler = TRudderInputHandler::new(TRudderVariant::Mk4);
    let state = handler.parse_report(&report).expect("valid report");
    insta::assert_debug_snapshot!("t_rudder_mk4_idle", state);
}

/// Pin T-Rudder Mk.V at full brakes and full left rudder.
#[test]
fn snapshot_t_rudder_mk5_full_brakes_left_rudder() {
    let report = t_rudder_report(0xFFFF, 0xFFFF, 0x0000);
    let handler = TRudderInputHandler::new(TRudderVariant::Mk5);
    let state = handler.parse_report(&report).expect("valid report");
    insta::assert_debug_snapshot!("t_rudder_mk5_full_brakes_left", state);
}
