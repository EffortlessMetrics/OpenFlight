// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Snapshot tests for Honeycomb Aeronautical device parser outputs.
//!
//! These tests pin the exact `Debug` representation of parsed HID reports at
//! known input values.  Any change to the struct layout, normalisation formula,
//! or enum variant naming will surface as a diff before it reaches users.

use flight_hotas_honeycomb::bravo_leds::BravoLedState;
use flight_hotas_honeycomb::button_delta::ButtonDelta;
use flight_hotas_honeycomb::{parse_alpha_report, parse_bravo_report, serialize_led_report};

// ── report builders ───────────────────────────────────────────────────────────

/// Build an 11-byte Alpha Yoke report.
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

#[test]
fn snapshot_alpha_yoke_neutral() {
    let report = alpha_report(2048, 2048, 0, 15);
    let state = parse_alpha_report(&report).expect("valid report");
    insta::assert_debug_snapshot!("alpha_yoke_neutral", state);
}

#[test]
fn snapshot_alpha_yoke_full_right_roll() {
    let report = alpha_report(4095, 2048, 0, 15);
    let state = parse_alpha_report(&report).expect("valid report");
    insta::assert_debug_snapshot!("alpha_yoke_full_right_roll", state);
}

#[test]
fn snapshot_alpha_yoke_full_left_roll_with_buttons() {
    let buttons: u64 = (1 << 0) | (1 << 24) | (1 << 25); // PTT + magneto both
    let report = alpha_report(0, 2048, buttons, 0); // hat N
    let state = parse_alpha_report(&report).expect("valid report");
    insta::assert_debug_snapshot!("alpha_yoke_full_left_with_magneto_both", state);
}

#[test]
fn snapshot_alpha_yoke_all_buttons() {
    let all_36: u64 = (1u64 << 36) - 1;
    let report = alpha_report(2048, 2048, all_36, 3);
    let state = parse_alpha_report(&report).expect("valid report");
    insta::assert_debug_snapshot!("alpha_yoke_all_36_buttons_hat_se", state);
}

// ── Bravo Throttle snapshots ──────────────────────────────────────────────────

#[test]
fn snapshot_bravo_throttle_idle() {
    let report = bravo_report([0; 7], 0);
    let state = parse_bravo_report(&report).expect("valid report");
    insta::assert_debug_snapshot!("bravo_throttle_idle", state);
}

#[test]
fn snapshot_bravo_throttle_full_with_gear_up() {
    let gear_up: u64 = 1 << 30;
    let report = bravo_report([4095; 7], gear_up);
    let state = parse_bravo_report(&report).expect("valid report");
    insta::assert_debug_snapshot!("bravo_throttle_full_with_gear_up", state);
}

#[test]
fn snapshot_bravo_throttle_ap_panel_active() {
    let ap_mask: u64 = 0xFF; // all 8 AP buttons (bits 0-7)
    let report = bravo_report([2048, 2048, 0, 0, 0, 0, 0], ap_mask);
    let state = parse_bravo_report(&report).expect("valid report");
    insta::assert_debug_snapshot!("bravo_throttle_ap_panel_active", state);
}

// ── LED snapshots ─────────────────────────────────────────────────────────────

#[test]
fn snapshot_led_all_off() {
    let leds = BravoLedState::all_off();
    let report = serialize_led_report(&leds);
    insta::assert_debug_snapshot!("led_all_off", report);
}

#[test]
fn snapshot_led_all_on() {
    let leds = BravoLedState::all_on();
    let report = serialize_led_report(&leds);
    insta::assert_debug_snapshot!("led_all_on", report);
}

#[test]
fn snapshot_led_gear_down_green() {
    let mut leds = BravoLedState::all_off();
    leds.set_all_gear(true);
    let report = serialize_led_report(&leds);
    insta::assert_debug_snapshot!("led_gear_down_green", report);
}

// ── Button delta snapshots ────────────────────────────────────────────────────

#[test]
fn snapshot_button_delta_gear_transition() {
    let prev: u64 = 1 << 30; // gear up
    let curr: u64 = 1 << 31; // gear down
    let delta = ButtonDelta::compute(prev, curr);
    insta::assert_debug_snapshot!("button_delta_gear_transition", delta);
}
