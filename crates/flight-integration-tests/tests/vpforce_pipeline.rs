// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests: VPforce Rhino HID parsing → bus pipeline.
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
use flight_hotas_vpforce::{RHINO_MIN_REPORT_BYTES, parse_rhino_report};

// ── helpers ───────────────────────────────────────────────────────────────────

/// Build a valid Rhino HID report from 6 signed axis values, a button mask,
/// and a HAT value.
///
/// Axis order (matching the physical layout): [roll, pitch, z(throttle), rocker, ry, twist]
fn make_rhino_report(axes: [i16; 6], buttons: u32, hat: u8) -> [u8; RHINO_MIN_REPORT_BYTES] {
    let mut r = [0u8; RHINO_MIN_REPORT_BYTES];
    r[0] = 0x01; // report ID
    for (i, &ax) in axes.iter().enumerate() {
        let offset = 1 + i * 2;
        let le = ax.to_le_bytes();
        r[offset] = le[0];
        r[offset + 1] = le[1];
    }
    let btn = buttons.to_le_bytes();
    r[13..17].copy_from_slice(&btn);
    r[17] = hat;
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

// ── VPforce Rhino tests ───────────────────────────────────────────────────────

/// Smoke-test: a valid 20-byte centred Rhino report parses without error and
/// all 6 axes are within their expected ranges.
#[test]
fn vpforce_rhino_parses_valid_report() {
    let report = make_rhino_report([0i16; 6], 0, 0xFF);
    assert_eq!(report.len(), RHINO_MIN_REPORT_BYTES);

    let state = parse_rhino_report(&report).expect("should parse a valid report");

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
    assert!(
        (0.0..=1.0).contains(&state.axes.throttle),
        "throttle out of range: {}",
        state.axes.throttle
    );
    assert!(
        (-1.0..=1.0).contains(&state.axes.rocker),
        "rocker out of range: {}",
        state.axes.rocker
    );
    assert!(
        (-1.0..=1.0).contains(&state.axes.twist),
        "twist out of range: {}",
        state.axes.twist
    );
    assert!(
        (-1.0..=1.0).contains(&state.axes.ry),
        "ry out of range: {}",
        state.axes.ry
    );
}

/// End-to-end: Rhino roll and pitch axes survive publish → subscribe.
///
/// Rhino X/Y axes are already centred (−1.0…1.0) so they map directly to
/// bus `control_inputs.roll` and `pitch`.
#[test]
fn vpforce_rhino_through_bus_pipeline() {
    // ~50 % right roll, ~25 % forward pitch
    let report = make_rhino_report([16383, -8192, 0, 0, 0, 0], 0, 0xFF);
    let state = parse_rhino_report(&report).expect("parse must succeed");

    let mut snapshot = BusSnapshot::new(SimId::Unknown, AircraftId::new("vpforce-rhino"));
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

/// Full-deflection roll (i16::MAX) produces roll > 0.99 through the bus.
#[test]
fn vpforce_rhino_full_deflection_through_bus() {
    let report = make_rhino_report([i16::MAX, 0, 0, 0, 0, 0], 0, 0xFF);
    let state = parse_rhino_report(&report).expect("parse must succeed");

    assert!(
        state.axes.roll > 0.99,
        "full right roll should be > 0.99, got {}",
        state.axes.roll
    );

    let mut snapshot = BusSnapshot::new(SimId::Unknown, AircraftId::new("vpforce-rhino-full"));
    snapshot.control_inputs.roll = state.axes.roll;

    let received = publish_and_receive(snapshot);

    assert!(
        received.control_inputs.roll > 0.99,
        "full right roll should survive bus roundtrip as > 0.99, got {}",
        received.control_inputs.roll
    );
}

/// Short Rhino reports are rejected by the parser before reaching the bus.
#[test]
fn vpforce_rhino_too_short_report_rejected() {
    let short = vec![0x01u8; RHINO_MIN_REPORT_BYTES - 1];
    assert!(
        parse_rhino_report(&short).is_err(),
        "report shorter than minimum must fail"
    );
}
