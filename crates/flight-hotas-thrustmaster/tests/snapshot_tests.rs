// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Snapshot tests for Thrustmaster device parser outputs.
//!
//! These tests pin the exact `Debug` representation of parsed HID reports at
//! known input values.  Any change to the struct layout, normalization formula,
//! or enum variant naming will surface as a diff before it reaches users.

use flight_hotas_thrustmaster::{parse_warthog_stick, parse_warthog_throttle};

// ── report builders ───────────────────────────────────────────────────────────

fn stick_report(x: u16, y: u16, rz: u16, btn_low: u16, btn_high: u8, hat: u8) -> [u8; 10] {
    let mut r = [0u8; 10];
    r[0..2].copy_from_slice(&x.to_le_bytes());
    r[2..4].copy_from_slice(&y.to_le_bytes());
    r[4..6].copy_from_slice(&rz.to_le_bytes());
    r[6..8].copy_from_slice(&btn_low.to_le_bytes());
    r[8] = btn_high;
    r[9] = hat;
    r
}

fn throttle_report(
    scx: u16,
    scy: u16,
    tl: u16,
    tr: u16,
    tc: u16,
    btn_low: u16,
    btn_mid: u16,
    btn_high: u8,
    toggles: u8,
    hat_dms: u8,
    hat_csl: u8,
) -> [u8; 20] {
    let mut r = [0u8; 20];
    r[0..2].copy_from_slice(&scx.to_le_bytes());
    r[2..4].copy_from_slice(&scy.to_le_bytes());
    r[4..6].copy_from_slice(&tl.to_le_bytes());
    r[6..8].copy_from_slice(&tr.to_le_bytes());
    r[8..10].copy_from_slice(&tc.to_le_bytes());
    r[10..12].copy_from_slice(&btn_low.to_le_bytes());
    r[12..14].copy_from_slice(&btn_mid.to_le_bytes());
    r[14] = btn_high;
    r[15] = toggles;
    r[16] = hat_dms;
    r[17] = hat_csl;
    r
}

// ── Warthog Stick snapshots ───────────────────────────────────────────────────

/// Pin the parsed state of the Warthog Joystick at the centered position.
///
/// All axes at midpoint (32768), no buttons pressed, hat centered.
#[test]
fn snapshot_warthog_stick_center() {
    let report = stick_report(32768, 32768, 32768, 0, 0, 0xFF);
    let state = parse_warthog_stick(&report).expect("valid report");
    insta::assert_debug_snapshot!("warthog_stick_center", state);
}

/// Pin the parsed state of the Warthog Joystick at maximum deflection.
///
/// X/RZ at full positive, Y at full negative, all buttons pressed, hat north.
#[test]
fn snapshot_warthog_stick_full_deflection() {
    // hat byte upper nibble 0x0 → North
    let report = stick_report(65535, 0, 65535, 0xFFFF, 0x07, 0x00);
    let state = parse_warthog_stick(&report).expect("valid report");
    insta::assert_debug_snapshot!("warthog_stick_full_deflection", state);
}

// ── Warthog Throttle snapshots ────────────────────────────────────────────────

/// Pin the parsed state of the Warthog Throttle at idle.
///
/// Throttle levers at zero, slew centered, no buttons pressed, hats centered.
#[test]
fn snapshot_warthog_throttle_idle() {
    let report = throttle_report(32768, 32768, 0, 0, 0, 0, 0, 0, 0, 0xFF, 0xFF);
    let state = parse_warthog_throttle(&report).expect("valid report");
    insta::assert_debug_snapshot!("warthog_throttle_idle", state);
}

/// Pin the parsed state of the Warthog Throttle at full power.
///
/// Both throttle levers at maximum, slew centered, all buttons pressed,
/// DMS hat north, CSL hat south.
#[test]
fn snapshot_warthog_throttle_full() {
    // hat_dms byte 0x00 → North; hat_csl byte 0x04 → South
    let report = throttle_report(32768, 32768, 65535, 65535, 65535, 0xFFFF, 0xFFFF, 0xFF, 0xFF, 0x00, 0x04);
    let state = parse_warthog_throttle(&report).expect("valid report");
    insta::assert_debug_snapshot!("warthog_throttle_full", state);
}
