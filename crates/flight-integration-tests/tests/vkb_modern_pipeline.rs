// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests: VKB S-TECS Modern Throttle HID parsing → bus pipeline.
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
use flight_hotas_vkb::{StecsMtVariant, VKC_STECS_MT_MIN_REPORT_BYTES, parse_stecs_mt_report};

// ── helpers ───────────────────────────────────────────────────────────────────

fn make_stecs_mt_report(throttle: u16, mini_left: u16, mini_right: u16, rotary: u16) -> Vec<u8> {
    let mut data = vec![0x01u8]; // report_id
    data.extend_from_slice(&throttle.to_le_bytes());
    data.extend_from_slice(&mini_left.to_le_bytes());
    data.extend_from_slice(&mini_right.to_le_bytes());
    data.extend_from_slice(&rotary.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes()); // buttons word0
    data.extend_from_slice(&0u32.to_le_bytes()); // buttons word1
    data
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

// ── VKB S-TECS Modern Throttle tests ─────────────────────────────────────────

/// Smoke-test: a valid 17-byte S-TECS Modern Throttle report parses without
/// error and the throttle axis is in [0.0, 1.0].
#[test]
fn vkb_stecs_modern_throttle_parses_valid_report() {
    let half: u16 = 32767;
    let report = make_stecs_mt_report(half, 0, 0, 0);
    assert_eq!(report.len(), VKC_STECS_MT_MIN_REPORT_BYTES);

    let state =
        parse_stecs_mt_report(&report, StecsMtVariant::Mini).expect("should parse a valid report");

    assert!(
        (0.0..=1.0).contains(&state.axes.throttle),
        "throttle out of range: {}",
        state.axes.throttle
    );
    assert!(
        (0.0..=1.0).contains(&state.axes.mini_left),
        "mini_left out of range: {}",
        state.axes.mini_left
    );
    assert!(
        (0.0..=1.0).contains(&state.axes.mini_right),
        "mini_right out of range: {}",
        state.axes.mini_right
    );
    assert!(
        (0.0..=1.0).contains(&state.axes.rotary),
        "rotary out of range: {}",
        state.axes.rotary
    );
}

/// End-to-end: STECS Modern Throttle main lever survives publish → subscribe.
#[test]
fn vkb_stecs_modern_throttle_through_bus() {
    // Main throttle at ~75 %
    let report = make_stecs_mt_report(49151, 0, 0, 0);
    let state = parse_stecs_mt_report(&report, StecsMtVariant::Mini).expect("parse must succeed");

    let mut snapshot = BusSnapshot::new(SimId::Unknown, AircraftId::new("vkb-stecs-mt"));
    snapshot.control_inputs = ControlInputs {
        pitch: 0.0,
        roll: 0.0,
        yaw: 0.0,
        throttle: vec![state.axes.throttle],
    };

    let received = publish_and_receive(snapshot);

    assert_eq!(received.control_inputs.throttle.len(), 1);
    assert!(
        (received.control_inputs.throttle[0] - state.axes.throttle).abs() < f32::EPSILON,
        "throttle mismatch: {} vs {}",
        received.control_inputs.throttle[0],
        state.axes.throttle
    );
    assert!(
        state.axes.throttle > 0.5,
        "throttle at ~75% should be > 0.5, got {}",
        state.axes.throttle
    );
}
