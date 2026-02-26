// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests: VIRPIL VPC WarBRD / WarBRD-D HID parsing → bus pipeline.
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
    VIRPIL_AXIS_MAX, VPC_WARBRD_MIN_REPORT_BYTES, WarBrdVariant, parse_warbrd_report,
};

// ── helpers ───────────────────────────────────────────────────────────────────

fn make_warbrd_report(axes: [u16; 5], buttons: [u8; 4]) -> Vec<u8> {
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

// ── WarBRD tests ──────────────────────────────────────────────────────────────

/// Smoke-test: a valid 15-byte WarBRD report parses without error and all axes
/// are in [0.0, 1.0].
#[test]
fn virpil_warbrd_parses_valid_report() {
    let half = VIRPIL_AXIS_MAX / 2;
    let report = make_warbrd_report([half, half, half, half, half], [0u8; 4]);
    assert_eq!(report.len(), VPC_WARBRD_MIN_REPORT_BYTES);

    let state =
        parse_warbrd_report(&report, WarBrdVariant::Original).expect("should parse a valid report");

    for (name, val) in [
        ("x", state.inner.axes.x),
        ("y", state.inner.axes.y),
        ("z", state.inner.axes.z),
        ("sz", state.inner.axes.sz),
        ("sl", state.inner.axes.sl),
    ] {
        assert!(
            (0.0..=1.0).contains(&val),
            "{name} axis out of [0, 1]: {val}"
        );
    }
}

/// End-to-end: WarBRD Original X/Y axes survive publish → subscribe.
///
/// VIRPIL axes are 0.0–1.0 with 0.5 as centre. The bus roll/pitch range is
/// −1.0…1.0, so we apply the standard centring transform: `bus = 2·virpil − 1`.
#[test]
fn virpil_warbrd_original_through_bus() {
    // X at 75% (right of centre), Y at 25% (forward of centre)
    let x_raw = VIRPIL_AXIS_MAX * 3 / 4;
    let y_raw = VIRPIL_AXIS_MAX / 4;
    let report = make_warbrd_report([x_raw, y_raw, 0, 0, 0], [0u8; 4]);
    let state =
        parse_warbrd_report(&report, WarBrdVariant::Original).expect("parse must succeed");

    let roll = (state.inner.axes.x * 2.0 - 1.0).clamp(-1.0, 1.0);
    let pitch = (state.inner.axes.y * 2.0 - 1.0).clamp(-1.0, 1.0);

    let mut snapshot = BusSnapshot::new(SimId::Unknown, AircraftId::new("virpil-warbrd-orig"));
    snapshot.control_inputs = ControlInputs {
        roll,
        pitch,
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
    assert_eq!(state.variant, WarBrdVariant::Original);
}

/// End-to-end: WarBRD-D variant tag is preserved through the bus roundtrip.
#[test]
fn virpil_warbrd_d_variant_through_bus() {
    let center = VIRPIL_AXIS_MAX / 2;
    let report = make_warbrd_report([center, center, 0, 0, 0], [0u8; 4]);
    let state = parse_warbrd_report(&report, WarBrdVariant::D).expect("parse must succeed");

    assert_eq!(state.variant, WarBrdVariant::D);

    let roll = state.inner.axes.x * 2.0 - 1.0;
    let pitch = state.inner.axes.y * 2.0 - 1.0;

    let mut snapshot = BusSnapshot::new(SimId::Unknown, AircraftId::new("virpil-warbrd-d"));
    snapshot.control_inputs.roll = roll;
    snapshot.control_inputs.pitch = pitch;

    let received = publish_and_receive(snapshot);

    assert!(
        received.control_inputs.roll.abs() < 0.01,
        "centred WarBRD-D roll should be ≈0, got {}",
        received.control_inputs.roll
    );
    assert!(
        received.control_inputs.pitch.abs() < 0.01,
        "centred WarBRD-D pitch should be ≈0, got {}",
        received.control_inputs.pitch
    );
}

/// Short WarBRD reports are rejected by the parser before reaching the bus.
#[test]
fn virpil_warbrd_too_short_rejected() {
    let short = vec![0x01u8; VPC_WARBRD_MIN_REPORT_BYTES - 1];
    assert!(
        parse_warbrd_report(&short, WarBrdVariant::Original).is_err(),
        "report shorter than minimum must fail"
    );
}
