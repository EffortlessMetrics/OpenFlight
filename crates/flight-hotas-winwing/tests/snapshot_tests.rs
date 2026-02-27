// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Snapshot tests for WinWing device parser outputs.
//!
//! These tests pin the exact `Debug` representation of parsed HID reports at
//! known input values.  Any change to the struct layout, normalization formula,
//! or enum variant naming will surface as a diff before it reaches users.

use flight_hotas_winwing::{parse_orion2_stick_report, parse_orion2_throttle_report};

// ── report builders ───────────────────────────────────────────────────────────

/// Build a 12-byte Orion 2 Stick report (report ID `0x02`).
fn orion2_stick_report(roll: i16, pitch: i16, buttons: u32, hat_a: u8, hat_b: u8) -> [u8; 12] {
    let mut r = [0u8; 12];
    r[0] = 0x02;
    r[1..3].copy_from_slice(&roll.to_le_bytes());
    r[3..5].copy_from_slice(&pitch.to_le_bytes());
    r[5..9].copy_from_slice(&buttons.to_le_bytes());
    r[9] = hat_a;
    r[10] = hat_b;
    r
}

/// Build a 24-byte Orion 2 Throttle report (report ID `0x01`).
///
/// `mouse_x` / `mouse_y` are raw u16 values; pass `32768` for center (→ 0.0).
fn orion2_throttle_report(
    tl: u16,
    tr: u16,
    friction: u16,
    mouse_x: u16,
    mouse_y: u16,
    buttons: u64,
    encoders: [i8; 5],
) -> [u8; 24] {
    let mut r = [0u8; 24];
    r[0] = 0x01;
    r[1..3].copy_from_slice(&tl.to_le_bytes());
    r[3..5].copy_from_slice(&tr.to_le_bytes());
    r[5..7].copy_from_slice(&friction.to_le_bytes());
    r[7..9].copy_from_slice(&mouse_x.to_le_bytes());
    r[9..11].copy_from_slice(&mouse_y.to_le_bytes());
    r[11..19].copy_from_slice(&buttons.to_le_bytes());
    for (i, &e) in encoders.iter().enumerate() {
        r[19 + i] = e as u8;
    }
    r
}

// ── Orion 2 Stick snapshots ───────────────────────────────────────────────────

/// Pin the parsed state of the WinWing Orion 2 Stick at the centered position.
///
/// Roll and pitch both at zero (center, → 0.0), no buttons pressed,
/// both HATs in neutral position.
#[test]
fn test_orion2_stick_center_snapshot() {
    let report = orion2_stick_report(0, 0, 0, 0x0F, 0x0F);
    let state = parse_orion2_stick_report(&report).expect("valid report");
    insta::assert_debug_snapshot!("orion2_stick_center", state);
}

// ── Orion 2 Throttle snapshots ────────────────────────────────────────────────

/// Pin the parsed state of the WinWing Orion 2 Throttle at idle.
///
/// Both throttle levers at zero (→ 0.0), friction at zero, mouse/slew stick
/// centered (raw 32768 → 0.0), no buttons pressed, all encoders at zero.
#[test]
fn test_orion2_throttle_idle_snapshot() {
    let report = orion2_throttle_report(0, 0, 0, 32768, 32768, 0, [0; 5]);
    let state = parse_orion2_throttle_report(&report).expect("valid report");
    insta::assert_debug_snapshot!("orion2_throttle_idle", state);
}
