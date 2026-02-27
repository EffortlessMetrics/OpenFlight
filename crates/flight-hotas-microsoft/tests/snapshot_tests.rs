// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Snapshot tests for Microsoft SideWinder device parser outputs.
//!
//! These tests pin the exact `Debug` representation of parsed HID reports at
//! known input values.  Any change to the struct layout, normalization formula,
//! or enum variant naming will surface as a diff before it reaches users.

use flight_hotas_microsoft::{
    parse_sidewinder_ffb_pro, parse_sidewinder_ffb2, parse_sidewinder_precision2,
};

// ── report builder ────────────────────────────────────────────────────────────

/// Build a 7-byte SideWinder FFB Pro / FFB 2 / Precision 2 report.
///
/// Fields are packed LSB-first: X (10-bit), Y (10-bit), Rz (8-bit),
/// Throttle (8-bit), Hat (4-bit nibble), Buttons (9-bit).
fn sidewinder_report(x: u16, y: u16, rz: u8, throttle: u8, hat: u8, buttons: u16) -> [u8; 7] {
    let x = x & 0x3FF;
    let y = y & 0x3FF;
    let hat = hat & 0x0F;
    let buttons = buttons & 0x01FF;

    let mut b = [0u8; 7];
    b[0] = x as u8;
    b[1] = ((x >> 8) as u8) | ((y as u8 & 0x3F) << 2);
    b[2] = ((y >> 6) as u8 & 0x0F) | ((rz & 0x0F) << 4);
    b[3] = (rz >> 4) | ((throttle & 0x0F) << 4);
    b[4] = (throttle >> 4) | (hat << 4);
    b[5] = (buttons & 0xFF) as u8;
    b[6] = ((buttons >> 8) & 0x01) as u8;
    b
}

// ── SideWinder FFB Pro snapshots ──────────────────────────────────────────────

/// Pin the parsed FFB Pro state at the default (idle) position.
///
/// Axes centered (X/Y=512, Rz=128), throttle at zero, hat centered, no buttons.
#[test]
fn test_ffb_pro_default_report_snapshot() {
    let report = sidewinder_report(512, 512, 128, 0, 8, 0);
    let state = parse_sidewinder_ffb_pro(&report).expect("valid report");
    insta::assert_debug_snapshot!("ffb_pro_default_report", state);
}

// ── SideWinder FFB 2 snapshots ────────────────────────────────────────────────

/// Pin the parsed FFB 2 state at the default (idle) position.
///
/// Same report layout as the FFB Pro; confirms the shared parser path is pinned.
#[test]
fn test_ffb2_default_report_snapshot() {
    let report = sidewinder_report(512, 512, 128, 0, 8, 0);
    let state = parse_sidewinder_ffb2(&report).expect("valid report");
    insta::assert_debug_snapshot!("ffb2_default_report", state);
}

// ── SideWinder Precision 2 snapshots ─────────────────────────────────────────

/// Pin the parsed Precision 2 state at the default (idle) position.
///
/// Axes centered (X/Y=512, Rz=128), throttle at zero, hat centered, no buttons.
#[test]
fn test_precision2_default_report_snapshot() {
    let report = sidewinder_report(512, 512, 128, 0, 8, 0);
    let state = parse_sidewinder_precision2(&report).expect("valid report");
    insta::assert_debug_snapshot!("precision2_default_report", state);
}
