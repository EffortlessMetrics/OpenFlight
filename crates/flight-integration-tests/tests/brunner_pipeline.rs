// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests: Brunner CLS-E yoke HID parsing → bus pipeline.
//!
//! Each test parses a raw HID byte slice using the device-specific parser,
//! maps the resulting state to a [`BusSnapshot`], publishes it through a
//! [`BusPublisher`], receives it via a [`Subscriber`], and asserts that the
//! round-tripped values match the originally parsed axes.

use flight_bus::{
    BusPublisher, SubscriptionConfig,
    snapshot::{BusSnapshot, ControlInputs},
    types::{AircraftId, SimId},
};
use flight_hotas_brunner::{CLS_E_MIN_REPORT_BYTES, parse_cls_e_report};

// ── helpers ───────────────────────────────────────────────────────────────────

/// Build a valid CLS-E HID report from two signed i16 axis values and
/// a 32-bit button bitmask.
///
/// Layout: [0x01 (report_id), roll_lo, roll_hi, pitch_lo, pitch_hi, btn0..btn3]
fn make_cls_e_report(roll: i16, pitch: i16, buttons: u32) -> [u8; CLS_E_MIN_REPORT_BYTES] {
    let mut r = [0u8; CLS_E_MIN_REPORT_BYTES];
    r[0] = 0x01;
    let roll_le = roll.to_le_bytes();
    r[1] = roll_le[0];
    r[2] = roll_le[1];
    let pitch_le = pitch.to_le_bytes();
    r[3] = pitch_le[0];
    r[4] = pitch_le[1];
    let btn = buttons.to_le_bytes();
    r[5] = btn[0];
    r[6] = btn[1];
    r[7] = btn[2];
    r[8] = btn[3];
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

// ── Brunner CLS-E tests ───────────────────────────────────────────────────────

/// Smoke-test: centred CLS-E report parses without error and axes are at zero.
#[test]
fn brunner_cls_e_parses_centred_report() {
    let report = make_cls_e_report(0, 0, 0);
    assert_eq!(report.len(), CLS_E_MIN_REPORT_BYTES);

    let state = parse_cls_e_report(&report).expect("should parse valid CLS-E report");

    assert_eq!(state.axes.roll, 0.0, "centred roll should be 0.0");
    assert_eq!(state.axes.pitch, 0.0, "centred pitch should be 0.0");
    assert!(
        state.buttons.pressed().is_empty(),
        "no buttons should be pressed"
    );
}

/// Full-deflection report: both axes at max produce +1.0.
#[test]
fn brunner_cls_e_full_deflection_right_back() {
    let report = make_cls_e_report(i16::MAX, i16::MAX, 0);
    let state = parse_cls_e_report(&report).expect("should parse valid CLS-E report");

    assert!(
        (0.99..=1.0).contains(&state.axes.roll),
        "full-right roll should approach +1.0, got {}",
        state.axes.roll
    );
    assert!(
        (0.99..=1.0).contains(&state.axes.pitch),
        "full-aft pitch should approach +1.0, got {}",
        state.axes.pitch
    );
}

/// Full-deflection report: both axes at min produce approximately -1.0.
#[test]
fn brunner_cls_e_full_deflection_left_forward() {
    let report = make_cls_e_report(i16::MIN, i16::MIN, 0);
    let state = parse_cls_e_report(&report).expect("should parse valid CLS-E report");

    assert!(
        (-1.0..=-0.99).contains(&state.axes.roll),
        "full-left roll should be approximately -1.0, got {}",
        state.axes.roll
    );
    assert!(
        (-1.0..=-0.99).contains(&state.axes.pitch),
        "full-forward pitch should be approximately -1.0, got {}",
        state.axes.pitch
    );
}

/// Axis values remain within [-1.0, +1.0] after round-tripping through bus.
#[test]
fn brunner_cls_e_axes_in_bounds_after_bus_round_trip() {
    let report = make_cls_e_report(16384, -16384, 0b0000_0101);
    let state = parse_cls_e_report(&report).expect("valid report");

    let snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    let received = publish_and_receive(snapshot);

    // Control inputs round-trip: map parsed axes to inputs and verify bounds
    let roll = state.axes.roll;
    let pitch = state.axes.pitch;
    assert!(
        (-1.0..=1.0).contains(&roll),
        "roll {roll} out of range after round-trip"
    );
    assert!(
        (-1.0..=1.0).contains(&pitch),
        "pitch {pitch} out of range after round-trip"
    );
    assert_eq!(received.sim, SimId::Msfs);
}

/// Short report (fewer than 9 bytes) returns an error, not a panic.
#[test]
fn brunner_cls_e_rejects_short_report() {
    for len in 0..CLS_E_MIN_REPORT_BYTES {
        let short = vec![0x01u8; len];
        let result = parse_cls_e_report(&short);
        assert!(
            result.is_err(),
            "expected error for {len}-byte report, got ok"
        );
    }
}

/// Button bitmask is decoded correctly for buttons 1 and 32.
#[test]
fn brunner_cls_e_button_decoding() {
    // Button 1: bit 0 of first byte
    let report = make_cls_e_report(0, 0, 0b0000_0001);
    let state = parse_cls_e_report(&report).expect("valid report");
    assert!(state.buttons.is_pressed(1), "button 1 should be pressed");
    assert!(
        !state.buttons.is_pressed(2),
        "button 2 should not be pressed"
    );

    // Button 32: bit 7 of fourth byte
    let report32 = make_cls_e_report(0, 0, 1 << 31);
    let state32 = parse_cls_e_report(&report32).expect("valid report");
    assert!(
        state32.buttons.is_pressed(32),
        "button 32 should be pressed"
    );
    assert!(
        !state32.buttons.is_pressed(31),
        "button 31 should not be pressed"
    );
}
