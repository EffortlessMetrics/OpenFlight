// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests: Thrustmaster T-Pendular Rudder (TPR) HID parsing → bus pipeline.
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
use flight_hotas_thrustmaster::{TPR_MIN_REPORT_BYTES, parse_tpr_report};

// ── helpers ───────────────────────────────────────────────────────────────────

fn make_tpr_report(rz: u16, z: u16, rx: u16) -> Vec<u8> {
    let mut data = Vec::with_capacity(6);
    data.extend_from_slice(&rz.to_le_bytes());
    data.extend_from_slice(&z.to_le_bytes());
    data.extend_from_slice(&rx.to_le_bytes());
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

// ── TPR Rudder Pedals tests ───────────────────────────────────────────────────

/// Smoke-test: a valid 6-byte TPR report parses without error and all 3 axes
/// are in [0.0, 1.0].
#[test]
fn tpr_pedals_parse_valid_report() {
    let half: u16 = 32767;
    let report = make_tpr_report(half, half, half);
    assert_eq!(report.len(), TPR_MIN_REPORT_BYTES);

    let state = parse_tpr_report(&report).expect("should parse a valid report");

    assert!(
        (0.0..=1.0).contains(&state.axes.rudder),
        "rudder out of [0, 1]: {}",
        state.axes.rudder
    );
    assert!(
        (0.0..=1.0).contains(&state.axes.right_pedal),
        "right_pedal out of [0, 1]: {}",
        state.axes.right_pedal
    );
    assert!(
        (0.0..=1.0).contains(&state.axes.left_pedal),
        "left_pedal out of [0, 1]: {}",
        state.axes.left_pedal
    );
}

/// End-to-end: TPR rudder axis survives publish → subscribe as bus yaw.
///
/// TPR rudder is 0.0–1.0 with 0.5 as centre. The bus yaw range is −1.0…1.0,
/// so we apply the centring transform: `yaw = 2·rudder − 1`.
#[test]
fn tpr_pedals_through_bus_pipeline() {
    // Rudder at ~75% right (right-of-centre)
    let report = make_tpr_report(49151, 0, 0);
    let state = parse_tpr_report(&report).expect("parse must succeed");

    let yaw = (state.axes.rudder * 2.0 - 1.0).clamp(-1.0, 1.0);

    let mut snapshot = BusSnapshot::new(SimId::Unknown, AircraftId::new("tm-tpr"));
    snapshot.control_inputs = ControlInputs {
        roll: 0.0,
        pitch: 0.0,
        yaw,
        throttle: vec![],
    };

    let received = publish_and_receive(snapshot);

    assert!(
        (received.control_inputs.yaw - yaw).abs() < f32::EPSILON,
        "yaw mismatch: {} vs {}",
        received.control_inputs.yaw,
        yaw
    );
    // Right deflection → positive yaw
    assert!(
        received.control_inputs.yaw > 0.0,
        "right rudder deflection should produce positive yaw, got {}",
        received.control_inputs.yaw
    );
}

/// Short TPR reports are rejected by the parser before reaching the bus.
#[test]
fn tpr_too_short_rejected() {
    let short = vec![0u8; TPR_MIN_REPORT_BYTES - 1];
    assert!(
        parse_tpr_report(&short).is_err(),
        "report shorter than minimum must fail"
    );
}
