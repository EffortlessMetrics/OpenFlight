// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests: VPforce Rhino HID input parsing → bus pipeline.
//!
//! Each test parses a raw HID byte slice using [`parse_rhino_report`],
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

/// Build a valid Rhino HID report from 6 signed axis values, a button bitmask,
/// and a HAT value.
///
/// Axis order: [roll, pitch, z(throttle), rocker, ry, twist]
fn make_rhino_report(axes: [i16; 6], buttons: u32, hat: u8) -> [u8; RHINO_MIN_REPORT_BYTES] {
    let mut r = [0u8; RHINO_MIN_REPORT_BYTES];
    r[0] = 0x01;
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

// ── VPforce Rhino pipeline tests ──────────────────────────────────────────────

/// Centred report (all axes = 0) produces roll=0.0, pitch=0.0, throttle=0.5.
#[test]
fn vpforce_rhino_centred_report_parses_to_zero() {
    let report = make_rhino_report([0i16; 6], 0, 0xFF);
    assert_eq!(report.len(), RHINO_MIN_REPORT_BYTES);

    let state = parse_rhino_report(&report).expect("centred report must parse");

    assert!(
        state.axes.roll.abs() < 1e-4,
        "centred roll should be ≈0.0, got {}",
        state.axes.roll
    );
    assert!(
        state.axes.pitch.abs() < 1e-4,
        "centred pitch should be ≈0.0, got {}",
        state.axes.pitch
    );
    // Z=0 maps to throttle=0.5 (midpoint of the [0,1] remap)
    assert!(
        (state.axes.throttle - 0.5).abs() < 1e-3,
        "zero Z should give throttle ≈0.5, got {}",
        state.axes.throttle
    );
    assert!(
        state.axes.twist.abs() < 1e-4,
        "centred twist should be ≈0.0, got {}",
        state.axes.twist
    );
}

/// Full-deflection report: i16::MAX roll → roll ≈ +1.0; i16::MIN pitch → pitch ≈ −1.0.
#[test]
fn vpforce_rhino_full_deflection_axes() {
    let report = make_rhino_report([i16::MAX, i16::MIN, i16::MAX, 0, 0, 0], 0, 0xFF);
    let state = parse_rhino_report(&report).expect("full-deflection report must parse");

    assert!(
        state.axes.roll > 0.99,
        "full right roll should be > 0.99, got {}",
        state.axes.roll
    );
    assert!(
        state.axes.pitch < -0.99,
        "full forward pitch should be < −0.99, got {}",
        state.axes.pitch
    );
    assert!(
        state.axes.throttle > 0.99,
        "full-forward Z should give throttle > 0.99, got {}",
        state.axes.throttle
    );
}

/// All axes produced by the parser are within their documented ranges.
///
/// Signed axes (roll, pitch, rocker, twist, ry) must be in [−1.0, 1.0].
/// Throttle (Z remapped) must be in [0.0, 1.0].
#[test]
fn vpforce_rhino_axes_within_bounds_check() {
    // Spot-check several representative raw values.
    let cases: &[[i16; 6]] = &[
        [i16::MIN, i16::MIN, i16::MIN, i16::MIN, i16::MIN, i16::MIN],
        [i16::MAX, i16::MAX, i16::MAX, i16::MAX, i16::MAX, i16::MAX],
        [16383, -16383, 16383, -16383, 16383, -16383],
        [1, -1, 1, -1, 1, -1],
        [0; 6],
    ];

    for (idx, &axes) in cases.iter().enumerate() {
        let report = make_rhino_report(axes, 0, 0xFF);
        let state = parse_rhino_report(&report)
            .unwrap_or_else(|_| panic!("case {idx}: must parse without error"));

        assert!(
            (-1.0..=1.0).contains(&state.axes.roll),
            "case {idx}: roll {} out of [−1, 1]",
            state.axes.roll
        );
        assert!(
            (-1.0..=1.0).contains(&state.axes.pitch),
            "case {idx}: pitch {} out of [−1, 1]",
            state.axes.pitch
        );
        assert!(
            (0.0..=1.0).contains(&state.axes.throttle),
            "case {idx}: throttle {} out of [0, 1]",
            state.axes.throttle
        );
        assert!(
            (-1.0..=1.0).contains(&state.axes.rocker),
            "case {idx}: rocker {} out of [−1, 1]",
            state.axes.rocker
        );
        assert!(
            (-1.0..=1.0).contains(&state.axes.twist),
            "case {idx}: twist {} out of [−1, 1]",
            state.axes.twist
        );
        assert!(
            (-1.0..=1.0).contains(&state.axes.ry),
            "case {idx}: ry {} out of [−1, 1]",
            state.axes.ry
        );
    }
}

/// Reports shorter than [`RHINO_MIN_REPORT_BYTES`] are rejected before touching the bus.
#[test]
fn vpforce_rhino_short_report_rejection() {
    for len in 0..RHINO_MIN_REPORT_BYTES {
        let short = vec![0x01u8; len];
        assert!(
            parse_rhino_report(&short).is_err(),
            "expected error for {len}-byte report, got ok"
        );
    }
}

/// End-to-end: Rhino roll and pitch survive a publish → subscribe round-trip
/// with no precision loss beyond floating-point representation.
#[test]
fn vpforce_rhino_bus_round_trip() {
    // ~62 % right roll, ~31 % forward pitch
    let report = make_rhino_report([20_000, -10_000, 0, 0, 0, 0], 0, 0xFF);
    let state = parse_rhino_report(&report).expect("parse must succeed");

    let mut snapshot = BusSnapshot::new(SimId::Unknown, AircraftId::new("vpforce-rhino-rt"));
    snapshot.control_inputs = ControlInputs {
        roll: state.axes.roll,
        pitch: state.axes.pitch,
        yaw: state.axes.twist,
        throttle: vec![state.axes.throttle],
    };

    let received = publish_and_receive(snapshot);

    assert!(
        (received.control_inputs.roll - state.axes.roll).abs() < f32::EPSILON,
        "roll mismatch after bus: {} vs {}",
        received.control_inputs.roll,
        state.axes.roll
    );
    assert!(
        (received.control_inputs.pitch - state.axes.pitch).abs() < f32::EPSILON,
        "pitch mismatch after bus: {} vs {}",
        received.control_inputs.pitch,
        state.axes.pitch
    );
    assert_eq!(
        received.control_inputs.throttle.len(),
        1,
        "throttle channel count must be preserved"
    );
    assert!(
        (received.control_inputs.throttle[0] - state.axes.throttle).abs() < f32::EPSILON,
        "throttle mismatch after bus: {} vs {}",
        received.control_inputs.throttle[0],
        state.axes.throttle
    );
}
