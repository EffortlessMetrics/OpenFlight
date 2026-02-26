// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests: VIRPIL device HID parsing → bus pipeline.
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
use flight_hotas_virpil::{
    VIRPIL_AXIS_MAX, VPC_CM3_THROTTLE_MIN_REPORT_BYTES, VPC_MONGOOST_STICK_MIN_REPORT_BYTES,
    parse_cm3_throttle_report, parse_mongoost_stick_report,
};

// ── helpers ──────────────────────────────────────────────────────────────────

fn make_cm3_report(axes: [u16; 6], buttons: [u8; 10]) -> Vec<u8> {
    let mut data = vec![0x01u8]; // report_id
    for ax in &axes {
        data.extend_from_slice(&ax.to_le_bytes());
    }
    data.extend_from_slice(&buttons);
    data
}

fn make_mongoost_report(axes: [u16; 5], buttons: [u8; 4]) -> Vec<u8> {
    let mut data = vec![0x01u8]; // report_id
    for ax in &axes {
        data.extend_from_slice(&ax.to_le_bytes());
    }
    data.extend_from_slice(&buttons);
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

// ── CM3 Throttle tests ────────────────────────────────────────────────────────

/// Smoke-test: a 23-byte zeroed CM3 report with a mid-point left throttle parses
/// without error and all axes stay in [0.0, 1.0].
#[test]
fn virpil_cm3_throttle_parses_valid_report() {
    let half = VIRPIL_AXIS_MAX / 2;
    let report = make_cm3_report([half, half, half, half, half, half], [0u8; 10]);
    assert_eq!(report.len(), VPC_CM3_THROTTLE_MIN_REPORT_BYTES);

    let state = parse_cm3_throttle_report(&report).expect("should parse a valid report");

    assert!(
        (0.0..=1.0).contains(&state.axes.left_throttle),
        "left_throttle out of range: {}",
        state.axes.left_throttle
    );
    assert!(
        (0.0..=1.0).contains(&state.axes.right_throttle),
        "right_throttle out of range: {}",
        state.axes.right_throttle
    );
    assert!(
        (0.0..=1.0).contains(&state.axes.flaps),
        "flaps out of range: {}",
        state.axes.flaps
    );
    assert!(
        (0.0..=1.0).contains(&state.axes.scx),
        "scx out of range: {}",
        state.axes.scx
    );
    assert!(
        (0.0..=1.0).contains(&state.axes.scy),
        "scy out of range: {}",
        state.axes.scy
    );
    assert!(
        (0.0..=1.0).contains(&state.axes.slider),
        "slider out of range: {}",
        state.axes.slider
    );
}

/// End-to-end: CM3 throttle axes survive publish → subscribe with no precision
/// loss beyond floating-point representation.
#[test]
fn virpil_cm3_throttle_axes_through_bus_pipeline() {
    let left_raw = VIRPIL_AXIS_MAX / 4; // ~25 %
    let right_raw = VIRPIL_AXIS_MAX * 3 / 4; // ~75 %
    let report = make_cm3_report([left_raw, right_raw, 0, 0, 0, 0], [0u8; 10]);
    let state = parse_cm3_throttle_report(&report).expect("parse must succeed");

    // Map CM3 throttle axes (0.0..1.0) directly into BusSnapshot.control_inputs.throttle.
    let mut snapshot = BusSnapshot::new(SimId::Unknown, AircraftId::new("virpil-cm3"));
    snapshot.control_inputs = ControlInputs {
        pitch: 0.0,
        roll: 0.0,
        yaw: 0.0,
        throttle: vec![state.axes.left_throttle, state.axes.right_throttle],
    };

    let received = publish_and_receive(snapshot);

    assert_eq!(received.control_inputs.throttle.len(), 2);
    assert!(
        (received.control_inputs.throttle[0] - state.axes.left_throttle).abs() < f32::EPSILON,
        "left_throttle mismatch: {} vs {}",
        received.control_inputs.throttle[0],
        state.axes.left_throttle
    );
    assert!(
        (received.control_inputs.throttle[1] - state.axes.right_throttle).abs() < f32::EPSILON,
        "right_throttle mismatch: {} vs {}",
        received.control_inputs.throttle[1],
        state.axes.right_throttle
    );
}

/// Extreme raw values (0 and VIRPIL_AXIS_MAX) produce exactly 0.0 and ≈1.0 and
/// are still accepted by the bus validator.
#[test]
fn virpil_cm3_axes_clamp_at_max() {
    let report = make_cm3_report([VIRPIL_AXIS_MAX, 0, VIRPIL_AXIS_MAX, 0, 0, 0], [0u8; 10]);
    let state = parse_cm3_throttle_report(&report).expect("parse must succeed");

    assert!(
        (state.axes.left_throttle - 1.0).abs() < 1e-4,
        "left_throttle at max raw should be ≈1.0, got {}",
        state.axes.left_throttle
    );
    assert_eq!(
        state.axes.right_throttle, 0.0,
        "right_throttle at zero raw should be 0.0"
    );

    // Overflow: raw > VIRPIL_AXIS_MAX should be clamped to 1.0
    let overflow_report = make_cm3_report(
        [u16::MAX, u16::MAX, u16::MAX, u16::MAX, u16::MAX, u16::MAX],
        [0u8; 10],
    );
    let overflow_state =
        parse_cm3_throttle_report(&overflow_report).expect("parse must succeed for overflow");
    assert_eq!(
        overflow_state.axes.left_throttle, 1.0,
        "overflow should clamp to 1.0"
    );

    // The clamped state must still pass bus validation.
    let mut snapshot = BusSnapshot::new(SimId::Unknown, AircraftId::new("virpil-clamp"));
    snapshot.control_inputs.throttle = vec![state.axes.left_throttle, state.axes.right_throttle];
    let received = publish_and_receive(snapshot);
    assert!((received.control_inputs.throttle[0] - 1.0).abs() < 1e-4);
    assert_eq!(received.control_inputs.throttle[1], 0.0);
}

/// Short reports are rejected by the parser before reaching the bus.
#[test]
fn virpil_cm3_throttle_too_short_report_rejected() {
    let short = vec![0x01u8; VPC_CM3_THROTTLE_MIN_REPORT_BYTES - 1];
    assert!(
        parse_cm3_throttle_report(&short).is_err(),
        "report shorter than minimum must fail"
    );
}

// ── MongoosT Stick tests ──────────────────────────────────────────────────────

/// Smoke-test: a minimum-length MongoosT report parses without error and all
/// axes stay in [0.0, 1.0].
#[test]
fn virpil_mongoost_stick_parses_valid_report() {
    let center = VIRPIL_AXIS_MAX / 2;
    let report = make_mongoost_report([center, center, center, center, center], [0u8; 4]);
    assert_eq!(report.len(), VPC_MONGOOST_STICK_MIN_REPORT_BYTES);

    let state = parse_mongoost_stick_report(&report).expect("should parse a valid report");

    for (name, val) in [
        ("x", state.axes.x),
        ("y", state.axes.y),
        ("z", state.axes.z),
        ("sz", state.axes.sz),
        ("sl", state.axes.sl),
    ] {
        assert!(
            (0.0..=1.0).contains(&val),
            "{name} axis out of [0, 1]: {val}"
        );
    }
}

/// End-to-end: MongoosT stick X/Y axes survive publish → subscribe.
///
/// VIRPIL stick axes are 0.0 (full left/forward) … 1.0 (full right/back)
/// with 0.5 as centre.  The bus `control_inputs.roll/pitch` range is −1.0…1.0,
/// so we apply the standard centring transform: `bus = 2·virpil − 1`.
#[test]
fn virpil_mongoost_stick_axes_through_bus_pipeline() {
    // X at 75 % (right of centre), Y at 25 % (forward of centre)
    let x_raw = VIRPIL_AXIS_MAX * 3 / 4;
    let y_raw = VIRPIL_AXIS_MAX / 4;
    let report = make_mongoost_report([x_raw, y_raw, 0, 0, 0], [0u8; 4]);
    let state = parse_mongoost_stick_report(&report).expect("parse must succeed");

    // Centre transform: virpil 0.0-1.0 → bus -1.0-1.0
    let roll = (state.axes.x * 2.0 - 1.0).clamp(-1.0, 1.0);
    let pitch = (state.axes.y * 2.0 - 1.0).clamp(-1.0, 1.0);

    let mut snapshot = BusSnapshot::new(SimId::Unknown, AircraftId::new("virpil-mongoost"));
    snapshot.control_inputs = ControlInputs {
        pitch,
        roll,
        yaw: 0.0,
        throttle: vec![],
    };

    let received = publish_and_receive(snapshot);

    assert!(
        (received.control_inputs.roll - roll).abs() < f32::EPSILON,
        "roll mismatch: {} vs {}",
        received.control_inputs.roll,
        roll
    );
    assert!(
        (received.control_inputs.pitch - pitch).abs() < f32::EPSILON,
        "pitch mismatch: {} vs {}",
        received.control_inputs.pitch,
        pitch
    );
}

/// Centred MongoosT report maps to roll=0.0 / pitch=0.0 through the bus.
#[test]
fn virpil_mongoost_stick_centred_maps_to_zero_in_bus() {
    let center = VIRPIL_AXIS_MAX / 2;
    let report = make_mongoost_report([center, center, 0, 0, 0], [0u8; 4]);
    let state = parse_mongoost_stick_report(&report).expect("parse must succeed");

    let roll = state.axes.x * 2.0 - 1.0;
    let pitch = state.axes.y * 2.0 - 1.0;

    let mut snapshot = BusSnapshot::new(SimId::Unknown, AircraftId::new("virpil-center"));
    snapshot.control_inputs.roll = roll;
    snapshot.control_inputs.pitch = pitch;

    let received = publish_and_receive(snapshot);

    assert!(
        received.control_inputs.roll.abs() < 0.01,
        "centered stick should produce roll ≈ 0, got {}",
        received.control_inputs.roll
    );
    assert!(
        received.control_inputs.pitch.abs() < 0.01,
        "centered stick should produce pitch ≈ 0, got {}",
        received.control_inputs.pitch
    );
}

/// Short MongoosT reports are rejected before reaching the bus.
#[test]
fn virpil_mongoost_too_short_report_rejected() {
    let short = vec![0x01u8; VPC_MONGOOST_STICK_MIN_REPORT_BYTES - 1];
    assert!(
        parse_mongoost_stick_report(&short).is_err(),
        "report shorter than minimum must fail"
    );
}
