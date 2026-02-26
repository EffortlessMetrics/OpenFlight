// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests: Thrustmaster device HID parsing → bus pipeline.
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
use flight_hotas_thrustmaster::t16000m::T16000M_MIN_REPORT_BYTES;
use flight_hotas_thrustmaster::{TFRP_MIN_REPORT_BYTES, parse_t16000m_report, parse_tfrp_report};

// ── helpers ───────────────────────────────────────────────────────────────────

fn make_tfrp_report(rz: u16, z: u16, rx: u16) -> Vec<u8> {
    let mut data = Vec::with_capacity(6);
    data.extend_from_slice(&rz.to_le_bytes());
    data.extend_from_slice(&z.to_le_bytes());
    data.extend_from_slice(&rx.to_le_bytes());
    data
}

fn make_t16000m_report(x: u16, y: u16, rz: u16, slider: u16, buttons: u16, hat: u8) -> Vec<u8> {
    let mut r = vec![0u8; 11];
    r[0..2].copy_from_slice(&x.to_le_bytes());
    r[2..4].copy_from_slice(&y.to_le_bytes());
    r[4..6].copy_from_slice(&rz.to_le_bytes());
    r[6..8].copy_from_slice(&slider.to_le_bytes());
    r[8..10].copy_from_slice(&buttons.to_le_bytes());
    r[10] = hat;
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

// ── TFRP Rudder Pedals tests ──────────────────────────────────────────────────

/// Smoke-test: a valid 6-byte TFRP report parses without error and all three
/// axes are in [0.0, 1.0].
#[test]
fn tfrp_pedals_parse_valid_report() {
    let center: u16 = 32767;
    let report = make_tfrp_report(center, center, center);
    assert_eq!(report.len(), TFRP_MIN_REPORT_BYTES);

    let state = parse_tfrp_report(&report).expect("should parse a valid report");

    assert!(
        (0.0..=1.0).contains(&state.axes.rudder),
        "rudder out of range: {}",
        state.axes.rudder
    );
    assert!(
        (0.0..=1.0).contains(&state.axes.right_pedal),
        "right_pedal out of range: {}",
        state.axes.right_pedal
    );
    assert!(
        (0.0..=1.0).contains(&state.axes.left_pedal),
        "left_pedal out of range: {}",
        state.axes.left_pedal
    );
}

/// End-to-end: TFRP rudder axis survives publish → subscribe.
///
/// The TFRP rudder (Rz) is a 0.0-1.0 unipolar value; we map it to the
/// bus `control_inputs.yaw` range (-1.0..1.0) by centring: `yaw = 2·rudder − 1`.
#[test]
fn tfrp_pedals_through_bus_pipeline() {
    // Full right rudder: Rz = 65535 → rudder = 1.0 → yaw = 1.0
    let report = make_tfrp_report(65535, 0, 0);
    let state = parse_tfrp_report(&report).expect("parse must succeed");

    assert!(
        (state.axes.rudder - 1.0).abs() < 1e-4,
        "expected rudder ≈ 1.0, got {}",
        state.axes.rudder
    );

    let yaw = (state.axes.rudder * 2.0 - 1.0).clamp(-1.0, 1.0);

    let mut snapshot = BusSnapshot::new(SimId::Unknown, AircraftId::new("tfrp"));
    snapshot.control_inputs.yaw = yaw;

    let received = publish_and_receive(snapshot);

    assert!(
        (received.control_inputs.yaw - yaw).abs() < f32::EPSILON,
        "yaw mismatch: {} vs {}",
        received.control_inputs.yaw,
        yaw
    );
}

/// Centred TFRP report (Rz ≈ 32767) maps to yaw ≈ 0.0 through the bus.
#[test]
fn tfrp_pedals_centred_maps_to_zero_yaw() {
    let center: u16 = 32767;
    let report = make_tfrp_report(center, 0, 0);
    let state = parse_tfrp_report(&report).expect("parse must succeed");

    let yaw = state.axes.rudder * 2.0 - 1.0;

    let mut snapshot = BusSnapshot::new(SimId::Unknown, AircraftId::new("tfrp-center"));
    snapshot.control_inputs.yaw = yaw;

    let received = publish_and_receive(snapshot);

    assert!(
        received.control_inputs.yaw.abs() < 0.01,
        "centred rudder should produce yaw ≈ 0, got {}",
        received.control_inputs.yaw
    );
}

/// All three pedal axes are independent — setting only right_pedal to max
/// leaves left_pedal and rudder unaffected.
#[test]
fn tfrp_pedals_axes_are_independent_through_bus() {
    let report = make_tfrp_report(0, 65535, 0);
    let state = parse_tfrp_report(&report).expect("parse must succeed");

    assert_eq!(state.axes.rudder, 0.0, "rudder should be 0");
    assert!(
        (state.axes.right_pedal - 1.0).abs() < 1e-4,
        "right_pedal should be ≈1"
    );
    assert_eq!(state.axes.left_pedal, 0.0, "left_pedal should be 0");

    // Encode independent braking as two throttle channels (left, right brake)
    let mut snapshot = BusSnapshot::new(SimId::Unknown, AircraftId::new("tfrp-brake"));
    snapshot.control_inputs.throttle = vec![state.axes.left_pedal, state.axes.right_pedal];

    let received = publish_and_receive(snapshot);

    assert_eq!(received.control_inputs.throttle[0], 0.0);
    assert!((received.control_inputs.throttle[1] - 1.0).abs() < 1e-4);
}

/// Short TFRP reports are rejected before reaching the bus.
#[test]
fn tfrp_too_short_report_rejected() {
    let short = vec![0u8; TFRP_MIN_REPORT_BYTES - 1];
    assert!(
        parse_tfrp_report(&short).is_err(),
        "report shorter than minimum must fail"
    );
}

// ── T.16000M Joystick tests ───────────────────────────────────────────────────

/// Smoke-test: a valid 11-byte T.16000M report parses without error.
/// X/Y/twist should be in [−1.0, 1.0]; throttle in [0.0, 1.0].
#[test]
fn t16000m_joystick_parses_valid_report() {
    let center: u16 = 8192;
    let report = make_t16000m_report(center, center, center, 0, 0, 0x0F);
    assert_eq!(report.len(), T16000M_MIN_REPORT_BYTES);

    let state = parse_t16000m_report(&report).expect("should parse a valid report");

    assert!(
        (-1.0..=1.0).contains(&state.axes.x),
        "x out of range: {}",
        state.axes.x
    );
    assert!(
        (-1.0..=1.0).contains(&state.axes.y),
        "y out of range: {}",
        state.axes.y
    );
    assert!(
        (-1.0..=1.0).contains(&state.axes.twist),
        "twist out of range: {}",
        state.axes.twist
    );
    assert!(
        (0.0..=1.0).contains(&state.axes.throttle),
        "throttle out of range: {}",
        state.axes.throttle
    );
}

/// End-to-end: T.16000M joystick axes survive publish → subscribe.
///
/// T.16000M X/Y/twist are already centred (−1.0…1.0) so they map directly to
/// bus `control_inputs.roll`, `pitch`, and `yaw`.
#[test]
fn t16000m_joystick_axes_through_bus_pipeline() {
    // Full right (x=16383), half-forward (y=4096), no twist, half throttle
    let report = make_t16000m_report(16383, 4096, 8192, u16::MAX / 2, 0, 0x0F);
    let state = parse_t16000m_report(&report).expect("parse must succeed");

    let mut snapshot = BusSnapshot::new(SimId::Unknown, AircraftId::new("t16000m"));
    snapshot.control_inputs = ControlInputs {
        roll: state.axes.x,
        pitch: state.axes.y,
        yaw: state.axes.twist,
        throttle: vec![state.axes.throttle],
    };

    let received = publish_and_receive(snapshot);

    assert!(
        (received.control_inputs.roll - state.axes.x).abs() < f32::EPSILON,
        "roll mismatch"
    );
    assert!(
        (received.control_inputs.pitch - state.axes.y).abs() < f32::EPSILON,
        "pitch mismatch"
    );
    assert!(
        (received.control_inputs.yaw - state.axes.twist).abs() < f32::EPSILON,
        "yaw mismatch"
    );
    assert!(
        (received.control_inputs.throttle[0] - state.axes.throttle).abs() < f32::EPSILON,
        "throttle mismatch"
    );
}

/// Centred T.16000M report (x=y=twist=8192) maps to roll=pitch=yaw ≈ 0.0
/// through the bus.
#[test]
fn t16000m_joystick_centred_maps_to_zero_in_bus() {
    let center: u16 = 8192;
    let report = make_t16000m_report(center, center, center, 0, 0, 0x0F);
    let state = parse_t16000m_report(&report).expect("parse must succeed");

    let mut snapshot = BusSnapshot::new(SimId::Unknown, AircraftId::new("t16000m-center"));
    snapshot.control_inputs.roll = state.axes.x;
    snapshot.control_inputs.pitch = state.axes.y;
    snapshot.control_inputs.yaw = state.axes.twist;

    let received = publish_and_receive(snapshot);

    assert!(
        received.control_inputs.roll.abs() < 0.01,
        "centred stick should produce roll ≈ 0, got {}",
        received.control_inputs.roll
    );
    assert!(
        received.control_inputs.pitch.abs() < 0.01,
        "centred stick should produce pitch ≈ 0, got {}",
        received.control_inputs.pitch
    );
    assert!(
        received.control_inputs.yaw.abs() < 0.01,
        "centred twist should produce yaw ≈ 0, got {}",
        received.control_inputs.yaw
    );
}

/// Full-deflection T.16000M report produces roll ≈ 1.0, throttle ≈ 1.0 through
/// the bus.
#[test]
fn t16000m_joystick_full_deflection_through_bus() {
    let report = make_t16000m_report(16383, 8192, 8192, u16::MAX, 0, 0x0F);
    let state = parse_t16000m_report(&report).expect("parse must succeed");

    assert!(
        state.axes.x > 0.99,
        "x should be ≈ 1.0 at full right, got {}",
        state.axes.x
    );
    assert!(
        state.axes.throttle > 0.99,
        "throttle should be ≈ 1.0 at max, got {}",
        state.axes.throttle
    );

    let mut snapshot = BusSnapshot::new(SimId::Unknown, AircraftId::new("t16000m-full"));
    snapshot.control_inputs.roll = state.axes.x;
    snapshot.control_inputs.throttle = vec![state.axes.throttle];

    let received = publish_and_receive(snapshot);

    assert!(received.control_inputs.roll > 0.99);
    assert!(received.control_inputs.throttle[0] > 0.99);
}

/// Short T.16000M reports are rejected before reaching the bus.
#[test]
fn t16000m_joystick_too_short_report_rejected() {
    let short = vec![0u8; T16000M_MIN_REPORT_BYTES - 1];
    assert!(
        parse_t16000m_report(&short).is_err(),
        "report shorter than minimum must fail"
    );
}
