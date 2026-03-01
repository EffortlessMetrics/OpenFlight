// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Snapshot tests for Logitech device parser outputs.
//!
//! These tests pin the exact `Debug` representation of parsed HID reports at
//! known input values.  Any change to the struct layout, normalization formula,
//! or enum variant naming will surface as a diff before it reaches users.

use flight_hotas_logitech::{
    parse_g_flight_yoke, parse_g27, parse_g29, parse_g940_joystick, parse_g940_throttle,
    parse_rudder_pedals, parse_x56_stick, parse_x56_throttle,
};

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

// ── G27 report builder ────────────────────────────────────────────────────────

/// Build an 8-byte G27 report from logical field values.
///
/// `wheel` is 16-bit big-endian; center ≈ 32768.
/// `buttons` covers 20 bits (bits 0-19); `dpad`: 0=N … 7=NW, 8-15=center.
fn g27_report(
    wheel: u16,
    accelerator: u8,
    brake: u8,
    clutch: u8,
    buttons: u32,
    dpad: u8,
) -> [u8; 8] {
    let buttons = buttons & 0x000F_FFFF;
    let dpad = dpad & 0x0F;
    let [hi, lo] = wheel.to_be_bytes();
    let mut d = [0u8; 8];
    d[0] = hi;
    d[1] = lo;
    d[2] = accelerator;
    d[3] = brake;
    d[4] = clutch;
    d[5] = buttons as u8;
    d[6] = (buttons >> 8) as u8;
    d[7] = ((buttons >> 16) as u8 & 0x0F) | (dpad << 4);
    d
}

// ── G29 report builder ────────────────────────────────────────────────────────

/// Build an 8-byte G29/G920 report from logical field values.
///
/// `wheel` is 16-bit little-endian; center ≈ 32768.
/// `dpad`: lower nibble, 0=N … 7=NW, 8-15=center.
fn g29_report(
    wheel: u16,
    dpad: u8,
    accelerator: u8,
    brake: u8,
    clutch: u8,
    buttons: u16,
) -> [u8; 8] {
    let [lo, hi] = wheel.to_le_bytes();
    let mut d = [0u8; 8];
    d[0] = lo;
    d[1] = hi;
    d[2] = dpad & 0x0F;
    d[3] = accelerator;
    d[4] = brake;
    d[5] = clutch;
    d[6] = buttons as u8;
    d[7] = (buttons >> 8) as u8;
    d
}

// ── G27 snapshots ─────────────────────────────────────────────────────────────

/// Pin the parsed G27 state at the default (idle) position.
///
/// Wheel centered (32768), all pedals released, no buttons, dpad centered.
#[test]
fn test_g27_default_report_snapshot() {
    let report = g27_report(32768, 0, 0, 0, 0, 8);
    let state = parse_g27(&report).expect("valid report");
    insta::assert_debug_snapshot!("g27_default_report", state);
}

/// Pin the parsed G27 state at full-left steer, brake fully depressed.
///
/// Wheel at minimum (0 = full left), brake at maximum, dpad centered.
#[test]
fn test_g27_full_left_steer_snapshot() {
    let report = g27_report(0, 0, 0, 0, 0, 8);
    let state = parse_g27(&report).expect("valid report");
    insta::assert_debug_snapshot!("g27_full_left_steer", state);
}

/// Pin the parsed G27 state with brake pedal fully depressed.
///
/// Wheel centered, brake at maximum (255), dpad centered.
#[test]
fn test_g27_brake_pedal_snapshot() {
    let report = g27_report(32768, 0, 255, 0, 0, 8);
    let state = parse_g27(&report).expect("valid report");
    insta::assert_debug_snapshot!("g27_brake_pedal", state);
}

// ── G29 snapshots ─────────────────────────────────────────────────────────────

/// Pin the parsed G29 state at the default (idle) position.
///
/// Wheel centered (32768), all pedals released, no buttons, dpad centered.
#[test]
fn test_g29_default_report_snapshot() {
    let report = g29_report(32768, 8, 0, 0, 0, 0);
    let state = parse_g29(&report).expect("valid report");
    insta::assert_debug_snapshot!("g29_default_report", state);
}

/// Pin the parsed G29 state at full-right steer.
///
/// Wheel at maximum (65535 = full right), all pedals released, dpad centered.
#[test]
fn test_g29_full_right_steer_snapshot() {
    let report = g29_report(65535, 8, 0, 0, 0, 0);
    let state = parse_g29(&report).expect("valid report");
    insta::assert_debug_snapshot!("g29_full_right_steer", state);
}

// ── X56 stick report builder ──────────────────────────────────────────────────

/// Build a 13-byte X56 stick report from logical field values.
fn x56_stick_report(
    x: u16,
    y: u16,
    rz: u16,
    rx: u8,
    ry: u8,
    buttons: u32,
    hat1: u8,
    hat2: u8,
) -> [u8; 13] {
    let x = x & 0xFFF;
    let y = y & 0xFFF;
    let rz = rz & 0xFFF;
    let buttons = buttons & 0x00FF_FFFF;
    let hat1 = hat1 & 0x0F;
    let hat2 = hat2 & 0x0F;

    let mut d = [0u8; 13];
    d[0] = x as u8;
    d[1] = ((x >> 8) as u8 & 0x0F) | (((y & 0x0F) as u8) << 4);
    d[2] = (y >> 4) as u8;
    d[3] = rz as u8;
    d[4] = ((rz >> 8) as u8) & 0x0F;
    d[5] = rx;
    d[6] = ry;
    d[7] = buttons as u8;
    d[8] = (buttons >> 8) as u8;
    d[9] = (buttons >> 16) as u8;
    d[10] = hat1 | (hat2 << 4);
    d[11] = 0;
    d[12] = 0;
    d
}

// ── X56 throttle report builder ───────────────────────────────────────────────

/// Build a 14-byte X56 throttle report from logical field values.
fn x56_throttle_report(
    tl: u16,
    tr: u16,
    rot_l: u8,
    rot_r: u8,
    sld_l: u8,
    sld_r: u8,
    buttons: u32,
    hat1: u8,
    hat2: u8,
) -> [u8; 14] {
    let tl = tl & 0x3FF;
    let tr = tr & 0x3FF;
    let buttons = buttons & 0x0FFF_FFFF;
    let hat1 = hat1 & 0x0F;
    let hat2 = hat2 & 0x0F;

    let mut d = [0u8; 14];
    d[0] = tl as u8;
    d[1] = ((tl >> 8) as u8 & 0x03) | (((tr & 0x3F) as u8) << 2);
    d[2] = ((tr >> 6) as u8 & 0x0F) | ((rot_l & 0x0F) << 4);
    d[3] = (rot_l >> 4) | ((rot_r & 0x0F) << 4);
    d[4] = (rot_r >> 4) | ((sld_l & 0x0F) << 4);
    d[5] = (sld_l >> 4) | ((sld_r & 0x0F) << 4);
    d[6] = (sld_r >> 4) | (((buttons & 0x0F) as u8) << 4);
    d[7] = ((buttons >> 4) & 0xFF) as u8;
    d[8] = ((buttons >> 12) & 0xFF) as u8;
    d[9] = ((buttons >> 20) & 0xFF) as u8;
    d[10] = hat1 | (hat2 << 4);
    d[11] = 0;
    d[12] = 0;
    d[13] = 0;
    d
}

// ── Rudder pedals report builder ──────────────────────────────────────────────

/// Build a 5-byte rudder pedals report from logical field values.
fn rudder_report(rudder: u16, left_brake: u16, right_brake: u16) -> [u8; 5] {
    let rudder = rudder & 0x3FF;
    let lb = left_brake & 0x3FF;
    let rb = right_brake & 0x3FF;

    let mut d = [0u8; 5];
    d[0] = rudder as u8;
    d[1] = ((rudder >> 8) as u8 & 0x03) | (((lb & 0x3F) as u8) << 2);
    d[2] = ((lb >> 6) as u8 & 0x0F) | (((rb & 0x0F) as u8) << 4);
    d[3] = ((rb >> 4) as u8) & 0x3F;
    d[4] = 0;
    d
}

// ── X56 stick snapshots ──────────────────────────────────────────────────────

/// Pin the parsed X56 stick state at center position.
#[test]
fn snapshot_x56_stick_center() {
    let report = x56_stick_report(2048, 2048, 2048, 128, 128, 0, 8, 8);
    let state = parse_x56_stick(&report).expect("valid report");
    insta::assert_debug_snapshot!("x56_stick_center", state);
}

/// Pin the parsed X56 stick state at full deflection.
#[test]
fn snapshot_x56_stick_full_deflection() {
    let report = x56_stick_report(4095, 0, 4095, 255, 0, 0x00FF_FFFF, 0, 2);
    let state = parse_x56_stick(&report).expect("valid report");
    insta::assert_debug_snapshot!("x56_stick_full_deflection", state);
}

// ── X56 throttle snapshots ───────────────────────────────────────────────────

/// Pin the parsed X56 throttle state at idle.
#[test]
fn snapshot_x56_throttle_idle() {
    let report = x56_throttle_report(0, 0, 0, 0, 0, 0, 0, 8, 8);
    let state = parse_x56_throttle(&report).expect("valid report");
    insta::assert_debug_snapshot!("x56_throttle_idle", state);
}

/// Pin the parsed X56 throttle state at full.
#[test]
fn snapshot_x56_throttle_full() {
    let report = x56_throttle_report(1023, 1023, 255, 255, 255, 255, 0x0FFF_FFFF, 0, 0);
    let state = parse_x56_throttle(&report).expect("valid report");
    insta::assert_debug_snapshot!("x56_throttle_full", state);
}

// ── Rudder pedals snapshots ──────────────────────────────────────────────────

/// Pin the parsed rudder pedals state at center with brakes released.
#[test]
fn snapshot_rudder_pedals_center() {
    let report = rudder_report(512, 0, 0);
    let state = parse_rudder_pedals(&report).expect("valid report");
    insta::assert_debug_snapshot!("rudder_pedals_center", state);
}

/// Pin the parsed rudder pedals state with full deflection and brakes.
#[test]
fn snapshot_rudder_pedals_full() {
    let report = rudder_report(1023, 1023, 1023);
    let state = parse_rudder_pedals(&report).expect("valid report");
    insta::assert_debug_snapshot!("rudder_pedals_full", state);
}
