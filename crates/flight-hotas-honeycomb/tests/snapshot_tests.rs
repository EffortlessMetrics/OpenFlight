// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Snapshot tests for Honeycomb Aeronautical device parser outputs.
//!
//! These tests pin the exact `Debug` representation of parsed HID reports at
//! known input values.  Any change to the struct layout, normalisation formula,
//! or enum variant naming will surface as a diff before it reaches users.

use flight_hotas_honeycomb::{parse_alpha_report, parse_bravo_report};

// ── report builders ───────────────────────────────────────────────────────────

/// Build an 11-byte Alpha Yoke report.
///
/// Layout: report_id=0x01, roll u16-LE, pitch u16-LE, 5 button bytes, hat nibble byte.
fn alpha_report(roll: u16, pitch: u16, buttons: u64, hat: u8) -> [u8; 11] {
    let mut r = [0u8; 11];
    r[0] = 0x01;
    r[1..3].copy_from_slice(&roll.to_le_bytes());
    r[3..5].copy_from_slice(&pitch.to_le_bytes());
    r[5] = (buttons & 0xFF) as u8;
    r[6] = ((buttons >> 8) & 0xFF) as u8;
    r[7] = ((buttons >> 16) & 0xFF) as u8;
    r[8] = ((buttons >> 24) & 0xFF) as u8;
    r[9] = ((buttons >> 32) & 0xFF) as u8;
    r[10] = hat & 0x0F;
    r
}

/// Build a 23-byte Bravo Throttle report.
///
/// Layout: report_id=0x01, 7×u16-LE throttle/flap/spoiler axes, 8 button bytes.
fn bravo_report(throttles: [u16; 7], buttons: u64) -> [u8; 23] {
    let mut r = [0u8; 23];
    r[0] = 0x01;
    for (i, &t) in throttles.iter().enumerate() {
        let off = 1 + i * 2;
        r[off..off + 2].copy_from_slice(&t.to_le_bytes());
    }
    r[15..23].copy_from_slice(&buttons.to_le_bytes());
    r
}

// ── Alpha Yoke snapshots ──────────────────────────────────────────────────────

/// Pin the parsed state of the Alpha Yoke at the neutral / centred position.
///
/// Both axes at 12-bit centre (2048 → 0.0), no buttons pressed, hat centred (raw 15).
#[test]
fn snapshot_alpha_yoke_neutral() {
    let report = alpha_report(2048, 2048, 0, 15);
    let state = parse_alpha_report(&report).expect("valid report");
    insta::assert_debug_snapshot!("alpha_yoke_neutral", state);
}

/// Pin the parsed state of the Alpha Yoke at full right roll.
///
/// Roll at maximum (4095 → ~+1.0), pitch at centre, no buttons pressed, hat centred.
#[test]
fn snapshot_alpha_yoke_full_right_roll() {
    let report = alpha_report(4095, 2048, 0, 15);
    let state = parse_alpha_report(&report).expect("valid report");
    insta::assert_debug_snapshot!("alpha_yoke_full_right_roll", state);
}

// ── Bravo Throttle snapshots ──────────────────────────────────────────────────

/// Pin the parsed state of the Bravo Throttle Quadrant at idle / all axes zero.
///
/// All seven levers at zero (→ 0.0), no buttons pressed.
#[test]
fn snapshot_bravo_throttle_idle() {
    let report = bravo_report([0; 7], 0);
    let state = parse_bravo_report(&report).expect("valid report");
    insta::assert_debug_snapshot!("bravo_throttle_idle", state);
}

/// Pin the parsed state of the Bravo Throttle Quadrant at full travel.
///
/// All seven levers at maximum (4095 → 1.0), gear-up button pressed (bit 30).
#[test]
fn snapshot_bravo_throttle_full_with_gear_up() {
    let gear_up: u64 = 1 << 30;
    let report = bravo_report([4095; 7], gear_up);
    let state = parse_bravo_report(&report).expect("valid report");
    insta::assert_debug_snapshot!("bravo_throttle_full_with_gear_up", state);
}
