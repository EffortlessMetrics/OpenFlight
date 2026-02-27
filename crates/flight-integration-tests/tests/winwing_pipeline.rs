// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests: WinWing device HID parsing → bus pipeline.
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
use flight_hotas_winwing::{
    F16EX_REPORT_LEN, SUPER_TAURUS_REPORT_LEN, parse_f16ex_stick_report, parse_super_taurus_report,
};

// ── helpers ───────────────────────────────────────────────────────────────────

fn make_f16ex_report(roll: i16, pitch: i16) -> [u8; F16EX_REPORT_LEN] {
    let mut r = [0u8; F16EX_REPORT_LEN];
    r[0] = 0x04; // report ID
    r[1..3].copy_from_slice(&roll.to_le_bytes());
    r[3..5].copy_from_slice(&pitch.to_le_bytes());
    r[9] = 0x0F; // HAT neutral
    r
}

fn make_super_taurus_report(tl: u16, tr: u16) -> [u8; SUPER_TAURUS_REPORT_LEN] {
    let mut r = [0u8; SUPER_TAURUS_REPORT_LEN];
    r[0] = 0x05; // report ID
    r[1..3].copy_from_slice(&tl.to_le_bytes());
    r[3..5].copy_from_slice(&tr.to_le_bytes());
    r
}

/// Publish a snapshot through a fresh bus and return the first received snapshot.
fn publish_and_receive(snapshot: BusSnapshot) -> BusSnapshot {
    let mut publisher = BusPublisher::new(60.0);
    let mut subscriber = publisher.subscribe(SubscriptionConfig::default()).unwrap();
    publisher.publish(snapshot).expect("publish must succeed");
    subscriber
        .try_recv()
        .expect("channel must not error")
        .expect("snapshot must be present after publish")
}

// ── F-16EX Stick tests ────────────────────────────────────────────────────────

/// Smoke-test: a valid 10-byte F-16EX report parses without error and both
/// axes are in [−1.0, 1.0].
#[test]
fn winwing_f16ex_stick_parses_valid_report() {
    let report = make_f16ex_report(0, 0);
    assert_eq!(report.len(), F16EX_REPORT_LEN);

    let state = parse_f16ex_stick_report(&report).expect("should parse a valid report");

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
}

/// End-to-end: F-16EX stick roll/pitch axes survive publish → subscribe.
///
/// The F-16EX axes are already centred (−1.0…1.0) so they map directly to
/// bus `control_inputs.roll` and `pitch`.
#[test]
fn winwing_f16ex_stick_through_bus_pipeline() {
    // ~75 % right roll, ~25 % forward pitch
    let report = make_f16ex_report(24575, -8192);
    let state = parse_f16ex_stick_report(&report).expect("parse must succeed");

    let mut snapshot = BusSnapshot::new(SimId::Unknown, AircraftId::new("winwing-f16ex"));
    snapshot.control_inputs = ControlInputs {
        roll: state.axes.roll,
        pitch: state.axes.pitch,
        yaw: 0.0,
        throttle: vec![],
    };

    let received = publish_and_receive(snapshot);

    assert!(
        (received.control_inputs.roll - state.axes.roll).abs() < f32::EPSILON,
        "roll mismatch: {} vs {}",
        received.control_inputs.roll,
        state.axes.roll
    );
    assert!(
        (received.control_inputs.pitch - state.axes.pitch).abs() < f32::EPSILON,
        "pitch mismatch: {} vs {}",
        received.control_inputs.pitch,
        state.axes.pitch
    );
}

/// Short F-16EX reports are rejected by the parser before reaching the bus.
#[test]
fn winwing_f16ex_too_short_report_rejected() {
    let short = vec![0x04u8; F16EX_REPORT_LEN - 1];
    assert!(
        parse_f16ex_stick_report(&short).is_err(),
        "report shorter than minimum must fail"
    );
}

// ── SuperTaurus Throttle tests ────────────────────────────────────────────────

/// Smoke-test: a valid 13-byte SuperTaurus report parses without error and all
/// throttle axes are in [0.0, 1.0].
#[test]
fn winwing_super_taurus_parses_valid_report() {
    let half: u16 = 32767;
    let report = make_super_taurus_report(half, half);
    assert_eq!(report.len(), SUPER_TAURUS_REPORT_LEN);

    let state = parse_super_taurus_report(&report).expect("should parse a valid report");

    assert!(
        (0.0..=1.0).contains(&state.axes.throttle_left),
        "throttle_left out of range: {}",
        state.axes.throttle_left
    );
    assert!(
        (0.0..=1.0).contains(&state.axes.throttle_right),
        "throttle_right out of range: {}",
        state.axes.throttle_right
    );
    assert!(
        (0.0..=1.0).contains(&state.axes.throttle_combined),
        "throttle_combined out of range: {}",
        state.axes.throttle_combined
    );
}

/// End-to-end: SuperTaurus left and right throttle axes survive publish → subscribe.
#[test]
fn winwing_super_taurus_dual_throttle_through_bus() {
    // Left at ~25 %, right at ~75 %
    let report = make_super_taurus_report(16384, 49151);
    let state = parse_super_taurus_report(&report).expect("parse must succeed");

    let mut snapshot = BusSnapshot::new(SimId::Unknown, AircraftId::new("winwing-supertaurus"));
    snapshot.control_inputs = ControlInputs {
        pitch: 0.0,
        roll: 0.0,
        yaw: 0.0,
        throttle: vec![state.axes.throttle_left, state.axes.throttle_right],
    };

    let received = publish_and_receive(snapshot);

    assert_eq!(received.control_inputs.throttle.len(), 2);
    assert!(
        (received.control_inputs.throttle[0] - state.axes.throttle_left).abs() < f32::EPSILON,
        "throttle_left mismatch: {} vs {}",
        received.control_inputs.throttle[0],
        state.axes.throttle_left
    );
    assert!(
        (received.control_inputs.throttle[1] - state.axes.throttle_right).abs() < f32::EPSILON,
        "throttle_right mismatch: {} vs {}",
        received.control_inputs.throttle[1],
        state.axes.throttle_right
    );
    assert!(
        state.axes.throttle_right > state.axes.throttle_left,
        "right throttle ({}) should be higher than left ({})",
        state.axes.throttle_right,
        state.axes.throttle_left
    );
}
