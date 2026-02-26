// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests: Honeycomb Alpha Yoke HID parsing → bus pipeline.
//!
//! Each test parses a raw HID byte slice using the device-specific parser,
//! maps the resulting state to a [`BusSnapshot`], publishes it through a
//! [`BusPublisher`], receives it via a [`Subscriber`], and asserts that the
//! round-tripped values match the originally parsed axes.

use flight_bus::{
    BusPublisher, SubscriptionConfig,
    snapshot::BusSnapshot,
    types::{AircraftId, SimId},
};
use flight_hotas_honeycomb::{
    HONEYCOMB_ALPHA_YOKE_PID, HONEYCOMB_VENDOR_ID,
    alpha::{ALPHA_REPORT_LEN, AlphaParseError, parse_alpha_report},
};

// ── helpers ───────────────────────────────────────────────────────────────────

/// Build a valid Alpha Yoke HID report.
///
/// Layout:
/// - byte 0: report_id (0x01)
/// - bytes 1-2: roll (u16 LE, 12-bit, 0–4095, centre=2048)
/// - bytes 3-4: pitch (u16 LE, 12-bit, 0–4095, centre=2048)
/// - bytes 5-9: button bitmask (40 bits, buttons 1–36)
/// - byte 10: hat nibble (lower 4 bits)
fn make_alpha_report(roll: u16, pitch: u16, buttons: u64, hat: u8) -> [u8; ALPHA_REPORT_LEN] {
    let mut r = [0u8; ALPHA_REPORT_LEN];
    r[0] = 0x01;
    let roll_le = roll.to_le_bytes();
    r[1] = roll_le[0];
    r[2] = roll_le[1];
    let pitch_le = pitch.to_le_bytes();
    r[3] = pitch_le[0];
    r[4] = pitch_le[1];
    r[5] = (buttons & 0xFF) as u8;
    r[6] = ((buttons >> 8) & 0xFF) as u8;
    r[7] = ((buttons >> 16) & 0xFF) as u8;
    r[8] = ((buttons >> 24) & 0xFF) as u8;
    r[9] = ((buttons >> 32) & 0xFF) as u8;
    r[10] = hat & 0x0F;
    r
}

/// Publish a snapshot through a fresh bus and return the received snapshot.
fn publish_and_receive(snapshot: BusSnapshot) -> BusSnapshot {
    let mut publisher = BusPublisher::new(60.0);
    let mut subscriber = publisher.subscribe(SubscriptionConfig::default()).unwrap();
    publisher.publish(snapshot).expect("publish must succeed");
    subscriber
        .try_recv()
        .expect("channel must not error")
        .expect("snapshot must be present after publish")
}

// ── Honeycomb Alpha Yoke tests ────────────────────────────────────────────────

/// Smoke-test: centred Alpha Yoke report parses without error.
#[test]
fn honeycomb_alpha_parses_centred_report() {
    let report = make_alpha_report(2048, 2048, 0, 15); // hat=15 → centred
    assert_eq!(report.len(), ALPHA_REPORT_LEN);

    let state = parse_alpha_report(&report).expect("should parse valid Alpha report");

    assert!(
        state.axes.roll.abs() < 0.001,
        "centred roll should be near 0.0, got {}",
        state.axes.roll
    );
    assert!(
        state.axes.pitch.abs() < 0.001,
        "centred pitch should be near 0.0, got {}",
        state.axes.pitch
    );
    assert_eq!(state.buttons.mask, 0, "no buttons should be pressed");
    assert_eq!(state.buttons.hat, 0, "hat should be centred (0)");
}

/// Full right roll produces approximately +1.0.
#[test]
fn honeycomb_alpha_full_roll_right() {
    let report = make_alpha_report(4095, 2048, 0, 15);
    let state = parse_alpha_report(&report).expect("valid Alpha report");

    assert!(
        state.axes.roll > 0.99,
        "full right roll should be ~+1.0, got {}",
        state.axes.roll
    );
    assert!(
        (-1.0..=1.0).contains(&state.axes.roll),
        "roll out of range: {}",
        state.axes.roll
    );
}

/// Full left roll produces approximately -1.0.
#[test]
fn honeycomb_alpha_full_roll_left() {
    let report = make_alpha_report(0, 2048, 0, 15);
    let state = parse_alpha_report(&report).expect("valid Alpha report");

    assert!(
        state.axes.roll < -0.99,
        "full left roll should be ~-1.0, got {}",
        state.axes.roll
    );
}

/// Short report returns an error, not a panic.
#[test]
fn honeycomb_alpha_rejects_short_report() {
    for len in 0..ALPHA_REPORT_LEN {
        let short = vec![0x01u8; len];
        let result = parse_alpha_report(&short);
        assert!(
            result.is_err(),
            "expected error for {len}-byte report, got ok"
        );
    }
}

/// Unknown report ID returns an error.
#[test]
fn honeycomb_alpha_rejects_unknown_report_id() {
    let mut report = make_alpha_report(2048, 2048, 0, 15);
    report[0] = 0xFF; // wrong report ID
    let result = parse_alpha_report(&report);
    assert!(
        matches!(result, Err(AlphaParseError::UnknownReportId { .. })),
        "expected UnknownReportId error"
    );
}

/// Button detection works for first and last button.
#[test]
fn honeycomb_alpha_button_detection() {
    // Button 1 = bit 0; Button 36 = bit 35
    let mask: u64 = (1u64 << 0) | (1u64 << 35);
    let report = make_alpha_report(2048, 2048, mask, 15);
    let state = parse_alpha_report(&report).expect("valid Alpha report");

    assert!(state.buttons.is_pressed(1), "button 1 should be pressed");
    assert!(!state.buttons.is_pressed(2), "button 2 should not be pressed");
    assert!(state.buttons.is_pressed(36), "button 36 should be pressed");
    // Out-of-range buttons always return false
    assert!(!state.buttons.is_pressed(0), "button 0 (OOB) should be false");
    assert!(!state.buttons.is_pressed(37), "button 37 (OOB) should be false");
}

/// VID/PID constants match the expected Honeycomb values.
#[test]
fn honeycomb_vid_pid_constants() {
    assert_eq!(HONEYCOMB_VENDOR_ID, 0x294B);
    assert_eq!(HONEYCOMB_ALPHA_YOKE_PID, 0x0102);
}

/// Bus round-trip: published snapshot is received with correct sim/aircraft.
#[test]
fn honeycomb_alpha_bus_round_trip() {
    let report = make_alpha_report(3000, 1500, 0b101, 0); // some buttons pressed
    let state = parse_alpha_report(&report).expect("valid Alpha report");

    // Verify axes are bounded
    assert!(
        (-1.0..=1.0).contains(&state.axes.roll),
        "roll out of range: {}",
        state.axes.roll
    );
    assert!(
        (-1.0..=1.0).contains(&state.axes.pitch),
        "pitch out of range: {}",
        state.axes.pitch
    );

    let snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("A320"));
    let received = publish_and_receive(snapshot);

    assert_eq!(received.sim, SimId::Msfs);
    assert_eq!(received.aircraft.icao, "A320");
}
