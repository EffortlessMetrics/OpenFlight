// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Snapshot tests for VIRPIL VPC device parser outputs.
//!
//! These tests pin the exact `Debug` representation of parsed HID reports at
//! known input values.  Any change to the struct layout, normalization formula,
//! or enum variant naming will surface as a diff before it reaches users.

use flight_hotas_virpil::{
    WarBrdVariant, parse_alpha_report, parse_mongoost_stick_report, parse_warbrd_report,
};

// ── report builder ────────────────────────────────────────────────────────────

/// Build a 15-byte VIRPIL report (report_id=0x01, 5×u16-LE axes, 4 button bytes).
fn virpil_report(axes: [u16; 5], buttons: [u8; 4]) -> Vec<u8> {
    let mut data = vec![0x01u8];
    for ax in &axes {
        data.extend_from_slice(&ax.to_le_bytes());
    }
    data.extend_from_slice(&buttons);
    data
}

// ── Constellation Alpha snapshots ─────────────────────────────────────────────

/// Pin the parsed state of the VPC Constellation Alpha at the centered position.
///
/// All axes at midpoint (8192 / 16384 = 0.5), no buttons pressed, hat centered.
#[test]
fn test_alpha_stick_center_snapshot() {
    // hat nibble 0xF in high bits of button byte 3 → Center
    let report = virpil_report([8192; 5], [0x00, 0x00, 0x00, 0xF0]);
    let state = parse_alpha_report(&report).expect("valid report");
    insta::assert_debug_snapshot!("alpha_stick_center", state);
}

/// Pin the parsed state of the VPC Constellation Alpha at full-right deflection.
///
/// X axis at maximum (16384 → 1.0), all other axes centered, no buttons pressed.
#[test]
fn test_alpha_stick_full_right_snapshot() {
    let axes = [16384, 8192, 8192, 8192, 8192];
    let report = virpil_report(axes, [0x00, 0x00, 0x00, 0xF0]);
    let state = parse_alpha_report(&report).expect("valid report");
    insta::assert_debug_snapshot!("alpha_stick_full_right", state);
}

// ── MongoosT-50CM3 snapshots ──────────────────────────────────────────────────

/// Pin the parsed state of the VPC MongoosT-50CM3 at the centered position.
///
/// All axes at midpoint (8192 / 16384 = 0.5), no buttons pressed, hat centered.
#[test]
fn test_mongoost_stick_center_snapshot() {
    let report = virpil_report([8192; 5], [0x00, 0x00, 0x00, 0xF0]);
    let state = parse_mongoost_stick_report(&report).expect("valid report");
    insta::assert_debug_snapshot!("mongoost_stick_center", state);
}

// ── WarBRD snapshots ──────────────────────────────────────────────────────────

/// Pin the parsed state of the VPC WarBRD base at the centered position.
///
/// Uses the `Original` variant; all axes at midpoint, no buttons pressed,
/// hat centered.
#[test]
fn test_warbrd_center_snapshot() {
    let report = virpil_report([8192; 5], [0x00, 0x00, 0x00, 0xF0]);
    let state = parse_warbrd_report(&report, WarBrdVariant::Original).expect("valid report");
    insta::assert_debug_snapshot!("warbrd_center", state);
}
