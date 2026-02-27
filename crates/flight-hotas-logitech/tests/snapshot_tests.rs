// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Snapshot tests for Logitech device parser outputs.
//!
//! These tests pin the exact `Debug` representation of parsed HID reports at
//! known input values.  Any change to the struct layout, normalization formula,
//! or enum variant naming will surface as a diff before it reaches users.

use flight_hotas_logitech::{parse_g_flight_yoke, parse_g940_joystick, parse_g940_throttle};

// ── G940 report builders ──────────────────────────────────────────────────────

/// Build an 11-byte G940 joystick report from logical field values.
///
/// Two consecutive 12-bit values share 3 bytes (LSB-first bit order).
/// `hat`: 0=N, 1=NE, …, 7=NW, 8-15=Center.
fn g940_joystick_report(x: u16, y: u16, z: u16, rz: u16, buttons: u32, hat: u8) -> [u8; 11] {
    let x = x & 0x0FFF;
    let y = y & 0x0FFF;
    let z = z & 0x0FFF;
    let rz = rz & 0x0FFF;
    let buttons = buttons & 0x000F_FFFF;
    let hat = hat & 0x0F;
    let mut d = [0u8; 11];
    d[0] = x as u8;
    d[1] = ((x >> 8) as u8 & 0x0F) | (((y & 0x0F) as u8) << 4);
    d[2] = (y >> 4) as u8;
    d[3] = z as u8;
    d[4] = ((z >> 8) as u8 & 0x0F) | (((rz & 0x0F) as u8) << 4);
    d[5] = (rz >> 4) as u8;
    d[6] = buttons as u8;
    d[7] = (buttons >> 8) as u8;
    d[8] = ((buttons >> 16) as u8 & 0x0F) | (hat << 4);
    d[9] = 8 | (8 << 4); // secondary hats centered
    d
}

/// Build a 5-byte G940 throttle report from logical field values.
fn g940_throttle_report(left: u16, right: u16, buttons: u16) -> [u8; 5] {
    let left = left & 0x0FFF;
    let right = right & 0x0FFF;
    let buttons = buttons & 0x07FF;
    let mut d = [0u8; 5];
    d[0] = left as u8;
    d[1] = ((left >> 8) as u8 & 0x0F) | (((right & 0x0F) as u8) << 4);
    d[2] = (right >> 4) as u8;
    d[3] = buttons as u8;
    d[4] = (buttons >> 8) as u8 & 0x07;
    d
}

// ── G Flight Yoke report builder ──────────────────────────────────────────────

/// Build an 8-byte G Flight Yoke report from logical field values.
///
/// X/Y are 12-bit packed LSB-first; Rz/slider/slider2 are 8-bit unipolar.
/// `hat`: 0=N, 1=NE, …, 7=NW, 8-15=Center.
fn yoke_report(x: u16, y: u16, rz: u8, slider: u8, slider2: u8, buttons: u16, hat: u8) -> [u8; 8] {
    let x = x & 0x0FFF;
    let y = y & 0x0FFF;
    let buttons = buttons & 0x0FFF;
    let hat = hat & 0x0F;
    let mut d = [0u8; 8];
    d[0] = x as u8;
    d[1] = ((x >> 8) as u8 & 0x0F) | (((y & 0x0F) as u8) << 4);
    d[2] = (y >> 4) as u8;
    d[3] = rz;
    d[4] = slider;
    d[5] = slider2;
    d[6] = buttons as u8;
    d[7] = ((buttons >> 8) as u8 & 0x0F) | (hat << 4);
    d
}

// ── G940 joystick snapshots ───────────────────────────────────────────────────

/// Pin the parsed state of the G940 joystick at the centered position.
///
/// Bipolar axes at midpoint (2048), Z throttle at zero, no buttons, hat centered.
#[test]
fn snapshot_g940_joystick_center() {
    let report = g940_joystick_report(2048, 2048, 0, 2048, 0, 8);
    let state = parse_g940_joystick(&report).expect("valid report");
    insta::assert_debug_snapshot!("g940_joystick_center", state);
}

/// Pin the parsed state of the G940 joystick at full deflection.
///
/// X at max, Y at min, Z at max, RZ at max, all 20 buttons pressed, hat north.
#[test]
fn snapshot_g940_joystick_full_deflection() {
    let report = g940_joystick_report(4095, 0, 4095, 4095, 0x000F_FFFF, 0);
    let state = parse_g940_joystick(&report).expect("valid report");
    insta::assert_debug_snapshot!("g940_joystick_full_deflection", state);
}

// ── G940 throttle snapshots ───────────────────────────────────────────────────

/// Pin the parsed state of the G940 throttle at idle.
///
/// Both levers at zero, no buttons pressed.
#[test]
fn snapshot_g940_throttle_idle() {
    let report = g940_throttle_report(0, 0, 0);
    let state = parse_g940_throttle(&report).expect("valid report");
    insta::assert_debug_snapshot!("g940_throttle_idle", state);
}

/// Pin the parsed state of the G940 throttle at full.
///
/// Both levers at maximum, all 11 buttons pressed.
#[test]
fn snapshot_g940_throttle_full() {
    let report = g940_throttle_report(4095, 4095, 0x07FF);
    let state = parse_g940_throttle(&report).expect("valid report");
    insta::assert_debug_snapshot!("g940_throttle_full", state);
}

// ── G Flight Yoke snapshots ───────────────────────────────────────────────────

/// Pin the parsed state of the G Flight Yoke at the centered position.
///
/// Bipolar axes at midpoint (2047), unipolar sliders at zero, no buttons, hat centered.
#[test]
fn snapshot_g_flight_yoke_center() {
    let report = yoke_report(2047, 2047, 0, 0, 0, 0, 8);
    let state = parse_g_flight_yoke(&report).expect("valid report");
    insta::assert_debug_snapshot!("g_flight_yoke_center", state);
}

/// Pin the parsed state of the G Flight Yoke at full deflection.
///
/// X at max, Y at min, all sliders at max, all 12 buttons pressed, hat north.
#[test]
fn snapshot_g_flight_yoke_full_deflection() {
    let report = yoke_report(4095, 0, 255, 255, 255, 0x0FFF, 0);
    let state = parse_g_flight_yoke(&report).expect("valid report");
    insta::assert_debug_snapshot!("g_flight_yoke_full_deflection", state);
}
