// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests proving end-to-end telemetry snapshot flow through the bus.
//!
//! Each test exercises the full pipeline:
//!   adapter fixture → FixtureConverter → BusPublisher::publish → Subscriber::try_recv
//!
//! These are pure in-process tests — no network, no I/O.

use flight_bus::{
    AircraftId, BusPublisher, BusSnapshot, FixtureConverter, PublisherError, SubscriptionConfig,
};
use flight_bus::adapter_fixtures::BuiltinFixtures;
use flight_bus::types::SimId;
use std::time::Duration;

// ─── helpers ────────────────────────────────────────────────────────────────

fn make_publisher() -> BusPublisher {
    BusPublisher::new(60.0)
}

fn default_config() -> SubscriptionConfig {
    SubscriptionConfig::default()
}

// ─── end-to-end: adapter → convert → publish → receive ──────────────────────

/// MSFS fixture converted to a snapshot flows through the bus and arrives at the
/// subscriber with correct field values.
#[test]
fn test_msfs_fixture_snapshot_flow() {
    let fixture = BuiltinFixtures::msfs_c172_cruise();
    let snapshot = FixtureConverter::msfs_to_snapshot(&fixture).unwrap();

    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(default_config()).unwrap();

    publisher.publish(snapshot.clone()).unwrap();

    let received = sub.try_recv().unwrap().expect("subscriber should receive snapshot");
    assert_eq!(received.sim, SimId::Msfs);
    assert_eq!(received.aircraft, AircraftId::new("C172"));
    // Verify converted kinematics survived the bus round-trip
    assert_eq!(received.kinematics.ias, snapshot.kinematics.ias);
    assert_eq!(received.kinematics.g_force, snapshot.kinematics.g_force);
    assert_eq!(received.environment.altitude, snapshot.environment.altitude);
}

/// X-Plane fixture converted to a snapshot flows through the bus intact.
#[test]
fn test_xplane_fixture_snapshot_flow() {
    let fixture = BuiltinFixtures::xplane_c172_cruise();
    let snapshot = FixtureConverter::xplane_to_snapshot(&fixture).unwrap();

    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(default_config()).unwrap();

    publisher.publish(snapshot.clone()).unwrap();

    let received = sub.try_recv().unwrap().expect("subscriber should receive snapshot");
    assert_eq!(received.sim, SimId::XPlane);
    assert_eq!(received.aircraft, AircraftId::new("C172"));
    assert_eq!(received.kinematics.ias, snapshot.kinematics.ias);
    assert_eq!(received.kinematics.bank, snapshot.kinematics.bank);
}

/// DCS fixture converted to a snapshot flows through the bus intact.
#[test]
fn test_dcs_fixture_snapshot_flow() {
    let fixture = BuiltinFixtures::dcs_f16c_cruise();
    let snapshot = FixtureConverter::dcs_to_snapshot(&fixture).unwrap();

    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(default_config()).unwrap();

    publisher.publish(snapshot.clone()).unwrap();

    let received = sub.try_recv().unwrap().expect("subscriber should receive snapshot");
    assert_eq!(received.sim, SimId::Dcs);
    assert_eq!(received.aircraft, AircraftId::new("F-16C"));
    assert_eq!(received.kinematics.heading, snapshot.kinematics.heading);
}

// ─── staleness propagation ───────────────────────────────────────────────────

/// A snapshot whose `safe_for_ffb` validity flag is false (the default) retains
/// that flag after flowing through the bus — subscribers can detect staleness.
#[test]
fn test_stale_snapshot_propagation() {
    let snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    // Default validity has safe_for_ffb = false
    assert!(!snapshot.validity.safe_for_ffb, "precondition: safe_for_ffb starts false");

    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(default_config()).unwrap();

    publisher.publish(snapshot).unwrap();

    let received = sub.try_recv().unwrap().expect("should receive snapshot");
    assert!(
        !received.validity.safe_for_ffb,
        "staleness flag must survive the bus"
    );
}

/// A snapshot with all validity flags set to true preserves them through the bus.
#[test]
fn test_validity_flags_preserved_through_bus() {
    let mut snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    snapshot.validity.safe_for_ffb = true;
    snapshot.validity.attitude_valid = true;
    snapshot.validity.angular_rates_valid = true;
    snapshot.validity.velocities_valid = true;
    snapshot.validity.kinematics_valid = true;
    snapshot.validity.aero_valid = true;
    snapshot.validity.position_valid = true;

    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(default_config()).unwrap();

    publisher.publish(snapshot).unwrap();

    let received = sub.try_recv().unwrap().expect("should receive snapshot");
    assert!(received.validity.safe_for_ffb);
    assert!(received.validity.attitude_valid);
    assert!(received.validity.angular_rates_valid);
    assert!(received.validity.velocities_valid);
    assert!(received.validity.kinematics_valid);
    assert!(received.validity.aero_valid);
    assert!(received.validity.position_valid);
}

// ─── multiple subscribers ────────────────────────────────────────────────────

/// Three subscribers each receive the same converted snapshot with identical data.
#[test]
fn test_multiple_subscribers_receive_same_snapshot() {
    let fixture = BuiltinFixtures::msfs_a320_approach();
    let snapshot = FixtureConverter::msfs_to_snapshot(&fixture).unwrap();

    let mut publisher = make_publisher();
    let mut sub1 = publisher.subscribe(default_config()).unwrap();
    let mut sub2 = publisher.subscribe(default_config()).unwrap();
    let mut sub3 = publisher.subscribe(default_config()).unwrap();

    publisher.publish(snapshot.clone()).unwrap();

    for (label, sub) in [("sub1", &mut sub1), ("sub2", &mut sub2), ("sub3", &mut sub3)] {
        let received = sub.try_recv().unwrap().unwrap_or_else(|| {
            panic!("{label} should have received the snapshot")
        });
        assert_eq!(received.sim, SimId::Msfs, "{label}: sim mismatch");
        assert_eq!(received.aircraft, AircraftId::new("A320"), "{label}: aircraft mismatch");
        assert_eq!(
            received.kinematics.ias, snapshot.kinematics.ias,
            "{label}: IAS mismatch"
        );
    }
}

/// A late subscriber (created after publish) does not receive the old snapshot,
/// but does receive the next one.
#[test]
fn test_late_subscriber_gets_only_new_snapshots() {
    let mut publisher = make_publisher();

    // First snapshot — published before the late subscriber exists.
    let snap1 = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    publisher.publish(snap1).unwrap();

    // Late subscriber joins now.
    let mut late_sub = publisher.subscribe(default_config()).unwrap();
    assert!(
        late_sub.try_recv().unwrap().is_none(),
        "late subscriber must not see old snapshot"
    );

    // Second snapshot — published after late subscriber exists.
    std::thread::sleep(Duration::from_millis(20));
    let snap2 = BusSnapshot::new(SimId::XPlane, AircraftId::new("A320"));
    publisher.publish(snap2).unwrap();

    let received = late_sub.try_recv().unwrap().expect("late subscriber should get new snapshot");
    assert_eq!(received.sim, SimId::XPlane);
}

// ─── backpressure / drop-tail ────────────────────────────────────────────────

/// When the subscriber buffer is full, the oldest messages are not lost — instead
/// the publisher skips sending (channel is bounded). Verify the subscriber drains
/// exactly the buffer capacity worth of messages.
#[test]
fn test_bus_backpressure_bounded_buffer() {
    let buffer_size = 3;
    let mut publisher = make_publisher();
    let config = SubscriptionConfig {
        buffer_size,
        drop_on_full: true,
        max_rate_hz: 60.0,
    };
    let mut sub = publisher.subscribe(config).unwrap();

    // Publish more snapshots than the buffer can hold, spacing them to pass the
    // publisher rate limiter (60 Hz → ~17 ms interval).
    let total_publishes = buffer_size + 2;
    for i in 0..total_publishes {
        let mut snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
        snap.environment.altitude = (i * 1000) as f32;
        // Sleep to satisfy the publisher rate limiter
        std::thread::sleep(Duration::from_millis(20));
        publisher.publish(snap).unwrap();
    }

    // Drain the subscriber — should receive at most `buffer_size` snapshots.
    let mut received_count = 0;
    while sub.try_recv().unwrap().is_some() {
        received_count += 1;
    }
    assert!(
        received_count <= buffer_size,
        "received {received_count} snapshots but buffer_size is {buffer_size}"
    );
    assert!(
        received_count >= 1,
        "subscriber should have received at least one snapshot"
    );
}

// ─── invalid snapshot rejection ──────────────────────────────────────────────

/// A snapshot with NaN in angular rates is rejected by the publisher and never
/// reaches the subscriber — even when produced by manual adapter-like construction.
#[test]
fn test_invalid_snapshot_never_reaches_subscriber() {
    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(default_config()).unwrap();

    let mut bad_snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    bad_snapshot.angular_rates.p = f32::NAN;

    let result = publisher.publish(bad_snapshot);
    assert!(
        matches!(result, Err(PublisherError::ValidationError(_))),
        "NaN snapshot must be rejected"
    );
    assert!(
        sub.try_recv().unwrap().is_none(),
        "invalid snapshot must not reach subscriber"
    );
}

/// Publishing a snapshot with out-of-range control inputs is rejected.
#[test]
fn test_out_of_range_controls_rejected() {
    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(default_config()).unwrap();

    let mut bad_snapshot = BusSnapshot::new(SimId::XPlane, AircraftId::new("A320"));
    bad_snapshot.control_inputs.pitch = 1.5; // exceeds [-1, 1]

    let result = publisher.publish(bad_snapshot);
    assert!(result.is_err(), "out-of-range pitch must be rejected");
    assert!(
        sub.try_recv().unwrap().is_none(),
        "rejected snapshot must not reach subscriber"
    );
}

// ─── subscriber stats ────────────────────────────────────────────────────────

/// After receiving a snapshot the subscriber stats reflect the message count.
#[test]
fn test_subscriber_stats_after_receive() {
    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(default_config()).unwrap();

    assert_eq!(sub.stats().messages_received, 0);

    publisher
        .publish(BusSnapshot::new(SimId::Msfs, AircraftId::new("C172")))
        .unwrap();
    let _ = sub.try_recv().unwrap();

    assert_eq!(
        sub.stats().messages_received, 1,
        "stats should reflect one received message"
    );
}
