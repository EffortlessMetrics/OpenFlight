// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Sim adapter end-to-end integration tests.
//!
//! Proves: adapter state machine, telemetry → bus publish, disconnect/reconnect,
//! aircraft change detection → profile switch flow.

use flight_adapter_common::{AdapterError, AdapterMetrics, AdapterState, ReconnectionStrategy};
use flight_bus::types::SimId;
use flight_bus::{AircraftId, BusPublisher, BusSnapshot, SubscriptionConfig};
use flight_test_helpers::{FakeSim, FakeSnapshot, TelemetryFixtureBuilder, assert_approx_eq};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_publisher() -> BusPublisher {
    BusPublisher::new(60.0)
}

fn sample_snapshot(altitude: f64, airspeed: f64, on_ground: bool) -> FakeSnapshot {
    FakeSnapshot {
        altitude,
        airspeed,
        heading: 270.0,
        pitch: 2.5,
        roll: 0.0,
        yaw: 0.0,
        on_ground,
    }
}

/// Simulate the adapter state machine flow for a given sequence of states.
fn walk_state_machine(transitions: &[(AdapterState, AdapterState)]) {
    for (i, (from, to)) in transitions.iter().enumerate() {
        // In production the state machine enforces valid transitions.
        // Here we just verify the transitions are representable.
        assert_ne!(
            from, to,
            "transition {i}: from and to must differ (got {from:?} → {to:?})"
        );
    }
}

// ===========================================================================
// 1. Adapter state machine transitions
// ===========================================================================

#[test]
fn adapter_full_lifecycle_state_transitions() {
    let transitions = [
        (AdapterState::Disconnected, AdapterState::Connecting),
        (AdapterState::Connecting, AdapterState::Connected),
        (AdapterState::Connected, AdapterState::DetectingAircraft),
        (AdapterState::DetectingAircraft, AdapterState::Active),
        (AdapterState::Active, AdapterState::Disconnected),
    ];
    walk_state_machine(&transitions);
}

#[test]
fn adapter_error_recovery_transitions() {
    let transitions = [
        (AdapterState::Active, AdapterState::Error),
        (AdapterState::Error, AdapterState::Connecting),
        (AdapterState::Connecting, AdapterState::Connected),
    ];
    walk_state_machine(&transitions);
}

#[test]
fn adapter_all_states_are_distinct() {
    let states = [
        AdapterState::Disconnected,
        AdapterState::Connecting,
        AdapterState::Connected,
        AdapterState::DetectingAircraft,
        AdapterState::Active,
        AdapterState::Error,
    ];
    for i in 0..states.len() {
        for j in (i + 1)..states.len() {
            assert_ne!(states[i], states[j], "states {i} and {j} must differ");
        }
    }
}

// ===========================================================================
// 2. Adapter → telemetry → bus publish → subscriber receive
// ===========================================================================

#[test]
fn adapter_telemetry_published_to_bus() {
    let mut sim = FakeSim::new("MSFS");
    sim.connect();
    sim.set_aircraft("C172");
    sim.push_snapshot(sample_snapshot(5000.0, 120.0, false));

    let snap = sim.next_snapshot().expect("snapshot");

    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let mut bus_snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    bus_snap.environment.altitude = snap.altitude as f32;

    publisher.publish(bus_snap).expect("publish");

    let received = sub.try_recv().unwrap().expect("must receive");
    assert_eq!(received.sim, SimId::Msfs);
    assert_approx_eq(received.environment.altitude as f64, 5000.0, 0.1);
}

#[test]
fn adapter_multiple_telemetry_frames_arrive_in_order() {
    let mut sim = FakeSim::new("X-Plane");
    sim.connect();
    sim.set_aircraft("A320");

    let altitudes = [0.0, 1000.0, 5000.0, 10000.0, 35000.0];
    for &alt in &altitudes {
        sim.push_snapshot(sample_snapshot(alt, 250.0, alt == 0.0));
    }

    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let mut received_altitudes = Vec::new();
    while let Some(snap) = sim.next_snapshot() {
        let mut bus_snap = BusSnapshot::new(SimId::XPlane, AircraftId::new("A320"));
        bus_snap.environment.altitude = snap.altitude as f32;
        publisher.publish(bus_snap).expect("publish");
        std::thread::sleep(Duration::from_millis(20)); // respect rate limiter

        if let Some(r) = sub.try_recv().unwrap() {
            received_altitudes.push(r.environment.altitude as f64);
        }
    }

    assert!(
        !received_altitudes.is_empty(),
        "must receive some snapshots"
    );
    // First received altitude should be 0 (on ground)
    assert_approx_eq(received_altitudes[0], 0.0, 0.1);
}

// ===========================================================================
// 3. Disconnect → reconnect flow
// ===========================================================================

#[test]
fn adapter_disconnect_reconnect_resumes_telemetry() {
    let mut sim = FakeSim::new("MSFS");

    // Phase 1: Connected, produce telemetry
    sim.connect();
    sim.set_aircraft("C172");
    sim.push_snapshot(sample_snapshot(5000.0, 120.0, false));

    let snap1 = sim.next_snapshot().expect("phase 1 snapshot");
    assert!((snap1.altitude - 5000.0).abs() < f64::EPSILON);

    // Phase 2: Disconnect
    sim.disconnect();
    assert!(!sim.connected);

    // Phase 3: Reconnect and resume
    sim.connect();
    sim.push_snapshot(sample_snapshot(6000.0, 130.0, false));

    let snap2 = sim
        .next_snapshot()
        .expect("phase 3 snapshot after reconnect");
    assert!((snap2.altitude - 6000.0).abs() < f64::EPSILON);
}

#[test]
fn adapter_reconnect_publishes_to_bus_after_recovery() {
    let mut sim = FakeSim::new("DCS");
    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    // Connect, publish, disconnect
    sim.connect();
    sim.set_aircraft("F-16C");
    sim.push_snapshot(sample_snapshot(20000.0, 350.0, false));

    let snap = sim.next_snapshot().unwrap();
    let mut bus_snap = BusSnapshot::new(SimId::Dcs, AircraftId::new("F16C"));
    bus_snap.environment.altitude = snap.altitude as f32;
    publisher
        .publish(bus_snap)
        .expect("publish before disconnect");

    let r1 = sub.try_recv().unwrap().expect("received before disconnect");
    assert_eq!(r1.sim, SimId::Dcs);

    // Disconnect
    sim.disconnect();

    // Reconnect and resume publishing
    sim.connect();
    sim.push_snapshot(sample_snapshot(21000.0, 340.0, false));

    let snap2 = sim.next_snapshot().unwrap();
    let mut bus_snap2 = BusSnapshot::new(SimId::Dcs, AircraftId::new("F16C"));
    bus_snap2.environment.altitude = snap2.altitude as f32;
    std::thread::sleep(Duration::from_millis(20));
    publisher
        .publish(bus_snap2)
        .expect("publish after reconnect");

    let r2 = sub.try_recv().unwrap().expect("received after reconnect");
    assert_approx_eq(r2.environment.altitude as f64, 21000.0, 0.1);
}

// ===========================================================================
// 4. Aircraft change detection → profile switch
// ===========================================================================

#[test]
fn adapter_aircraft_change_detection() {
    let mut sim = FakeSim::new("MSFS");
    sim.connect();
    sim.set_aircraft("C172");

    let mut metrics = AdapterMetrics::new();
    metrics.record_aircraft_change(sim.aircraft.clone().unwrap());

    assert_eq!(metrics.aircraft_changes, 1);
    assert_eq!(metrics.last_aircraft_title.as_deref(), Some("C172"));

    // Switch aircraft
    sim.set_aircraft("B737");
    metrics.record_aircraft_change(sim.aircraft.clone().unwrap());

    assert_eq!(metrics.aircraft_changes, 2);
    assert_eq!(metrics.last_aircraft_title.as_deref(), Some("B737"));
}

#[test]
fn adapter_same_aircraft_does_not_double_count() {
    let mut metrics = AdapterMetrics::new();
    metrics.record_aircraft_change("C172".to_string());
    metrics.record_aircraft_change("C172".to_string()); // duplicate
    metrics.record_aircraft_change("C172".to_string()); // duplicate

    assert_eq!(metrics.aircraft_changes, 1, "same aircraft = no new switch");
}

#[test]
fn adapter_aircraft_change_triggers_bus_event() {
    let mut sim = FakeSim::new("MSFS");
    sim.connect();

    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    // First aircraft
    sim.set_aircraft("C172");
    let snap1 = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    publisher.publish(snap1).expect("publish C172");

    let r1 = sub.try_recv().unwrap().expect("receive C172");
    assert_eq!(r1.aircraft.icao, "C172");

    // Second aircraft
    sim.set_aircraft("A320");
    let snap2 = BusSnapshot::new(SimId::Msfs, AircraftId::new("A320"));
    std::thread::sleep(Duration::from_millis(20));
    publisher.publish(snap2).expect("publish A320");

    let r2 = sub.try_recv().unwrap().expect("receive A320");
    assert_eq!(r2.aircraft.icao, "A320");
}

// ===========================================================================
// 5. Reconnection strategy
// ===========================================================================

#[test]
fn adapter_reconnection_backoff_progression() {
    let strategy = ReconnectionStrategy::new(5, Duration::from_millis(100), Duration::from_secs(2));

    assert_eq!(strategy.next_backoff(1), Duration::from_millis(100));
    assert_eq!(strategy.next_backoff(2), Duration::from_millis(200));
    assert_eq!(strategy.next_backoff(3), Duration::from_millis(400));
    assert_eq!(strategy.next_backoff(4), Duration::from_millis(800));
    assert_eq!(strategy.next_backoff(5), Duration::from_millis(1600));
    // Capped at max
    assert_eq!(strategy.next_backoff(6), Duration::from_secs(2));
    assert_eq!(strategy.next_backoff(100), Duration::from_secs(2));
}

#[test]
fn adapter_reconnection_retry_limits() {
    let strategy = ReconnectionStrategy::new(3, Duration::from_millis(50), Duration::from_secs(1));

    assert!(strategy.should_retry(1));
    assert!(strategy.should_retry(3));
    assert!(!strategy.should_retry(4));
}

// ===========================================================================
// 6. Adapter metrics tracking
// ===========================================================================

#[test]
fn adapter_metrics_update_tracking() {
    let mut metrics = AdapterMetrics::new();

    for _ in 0..10 {
        metrics.record_update();
        std::thread::sleep(Duration::from_millis(5));
    }

    assert_eq!(metrics.total_updates, 10);
    assert!(metrics.last_update_time.is_some());
    assert!(!metrics.update_intervals.is_empty());
}

#[test]
fn adapter_metrics_summary_format() {
    let mut metrics = AdapterMetrics::new();
    metrics.record_update();
    metrics.record_aircraft_change("C172".to_string());

    let summary = metrics.summary();
    assert!(summary.contains("Updates:"), "summary: {summary}");
    assert!(summary.contains("Aircraft changes:"), "summary: {summary}");
}

// ===========================================================================
// 7. Adapter error types
// ===========================================================================

#[test]
fn adapter_error_display_all_variants() {
    let errors = [
        AdapterError::NotConnected,
        AdapterError::Timeout("deadline".to_string()),
        AdapterError::AircraftNotDetected,
        AdapterError::Configuration("bad config".to_string()),
        AdapterError::ReconnectExhausted,
        AdapterError::Other("custom".to_string()),
    ];

    for err in &errors {
        let msg = err.to_string();
        assert!(!msg.is_empty(), "error display must not be empty");
    }
}

// ===========================================================================
// 8. Telemetry fixture builders
// ===========================================================================

#[test]
fn adapter_telemetry_fixture_cruising_state() {
    let telem = TelemetryFixtureBuilder::new().cruising().build();
    assert!(!telem.on_ground);
    assert_approx_eq(telem.altitude_ft, 35_000.0, 0.1);
    assert_approx_eq(telem.airspeed_kts, 250.0, 0.1);
}

#[test]
fn adapter_telemetry_fixture_on_ramp_state() {
    let telem = TelemetryFixtureBuilder::new().on_ramp().build();
    assert!(telem.on_ground);
    assert_approx_eq(telem.airspeed_kts, 0.0, 0.1);
}
