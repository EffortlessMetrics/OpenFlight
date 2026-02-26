// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests: WinWing Orion 2 Throttle HID parsing → bus pipeline.
//!
//! Each test parses a raw HID byte slice using the dedicated Orion 2 Throttle
//! parser, maps the resulting state to a [`BusSnapshot`], publishes it through
//! a [`BusPublisher`], receives it via a [`Subscriber`], and asserts that the
//! round-tripped values match the originally parsed axes.

use flight_bus::{
    BusPublisher, SubscriptionConfig,
    snapshot::{BusSnapshot, ControlInputs},
    types::{AircraftId, SimId},
};
use flight_hotas_winwing::{ORION2_THROTTLE_REPORT_BYTES, parse_orion2_throttle_report};

// ── helpers ───────────────────────────────────────────────────────────────────

fn make_orion2_throttle_report(tl: u16, tr: u16) -> [u8; ORION2_THROTTLE_REPORT_BYTES] {
    let mut r = [0u8; ORION2_THROTTLE_REPORT_BYTES];
    r[0] = 0x01; // report ID
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

// ── Orion 2 Throttle tests ────────────────────────────────────────────────────

/// Smoke-test: a valid 24-byte Orion 2 Throttle report parses without error
/// and all throttle axes are in [0.0, 1.0].
#[test]
fn winwing_orion2_throttle_parses_valid_report() {
    let half: u16 = 32767;
    let report = make_orion2_throttle_report(half, half);
    assert_eq!(report.len(), ORION2_THROTTLE_REPORT_BYTES);

    let state = parse_orion2_throttle_report(&report).expect("should parse a valid report");

    assert!(
        (0.0..=1.0).contains(&state.axes.throttle_left),
        "throttle_left out of [0, 1]: {}",
        state.axes.throttle_left
    );
    assert!(
        (0.0..=1.0).contains(&state.axes.throttle_right),
        "throttle_right out of [0, 1]: {}",
        state.axes.throttle_right
    );
    assert!(
        (0.0..=1.0).contains(&state.axes.throttle_combined),
        "throttle_combined out of [0, 1]: {}",
        state.axes.throttle_combined
    );
}

/// End-to-end: Orion 2 Throttle left and right throttle axes survive
/// publish → subscribe with no precision loss beyond floating-point representation.
#[test]
fn winwing_orion2_throttle_through_bus() {
    // Left at ~25%, right at ~75%
    let report = make_orion2_throttle_report(16384, 49151);
    let state = parse_orion2_throttle_report(&report).expect("parse must succeed");

    let mut snapshot = BusSnapshot::new(SimId::Unknown, AircraftId::new("winwing-orion2-throttle"));
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

/// Short Orion 2 Throttle reports are rejected by the parser before reaching the bus.
#[test]
fn winwing_orion2_throttle_too_short_rejected() {
    let short = vec![0x01u8; ORION2_THROTTLE_REPORT_BYTES - 1];
    assert!(
        parse_orion2_throttle_report(&short).is_err(),
        "report shorter than minimum must fail"
    );
}
