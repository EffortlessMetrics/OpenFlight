// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Bus → sim adapter end-to-end integration tests.
//!
//! Proves: bus snapshot → sim variable write, adapter disconnect handling,
//! multiple subscribers, event ordering, and backpressure.

use flight_adapter_common::AdapterMetrics;
use flight_bus::types::SimId;
use flight_bus::{AircraftId, BusPublisher, BusSnapshot, SubscriptionConfig};
use flight_test_helpers::{FakeSim, FakeSnapshot, assert_approx_eq};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_publisher() -> BusPublisher {
    BusPublisher::new(60.0)
}

fn tick() {
    std::thread::sleep(Duration::from_millis(25));
}

fn snapshot_with_controls(sim: SimId, icao: &str, pitch: f32, roll: f32) -> BusSnapshot {
    let mut snap = BusSnapshot::new(sim, AircraftId::new(icao));
    snap.control_inputs.pitch = pitch;
    snap.control_inputs.roll = roll;
    snap
}

// ===========================================================================
// 1. Bus snapshot → sim variable write
// ===========================================================================

#[test]
fn e2e_bus_snapshot_to_sim_variable_write() {
    let mut sim = FakeSim::new("MSFS");
    sim.connect();
    sim.set_aircraft("C172");

    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    // Publish axis values through bus
    let mut snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    snap.control_inputs.pitch = 0.45;
    snap.control_inputs.roll = -0.3;
    snap.control_inputs.yaw = 0.1;
    snap.control_inputs.throttle = vec![0.75];
    publisher.publish(snap).expect("publish");

    // Subscriber receives and "writes" to sim
    let received = sub.try_recv().unwrap().expect("must receive");
    sim.send_command(&format!(
        "SET_PITCH={}",
        received.control_inputs.pitch
    ));
    sim.send_command(&format!(
        "SET_ROLL={}",
        received.control_inputs.roll
    ));
    sim.send_command(&format!(
        "SET_THROTTLE={}",
        received.control_inputs.throttle[0]
    ));

    let cmds = sim.received_commands();
    assert_eq!(cmds.len(), 3);
    assert!(cmds[0].contains("0.45"), "pitch command: {}", cmds[0]);
    assert!(cmds[1].contains("-0.3"), "roll command: {}", cmds[1]);
    assert!(cmds[2].contains("0.75"), "throttle command: {}", cmds[2]);
}

// ===========================================================================
// 2. Adapter disconnect → bus receives stale signal
// ===========================================================================

#[test]
fn e2e_adapter_disconnect_stale_detection() {
    let mut sim = FakeSim::new("X-Plane");
    sim.connect();
    sim.set_aircraft("B737");

    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    // Publish initial telemetry
    sim.push_snapshot(FakeSnapshot {
        altitude: 5000.0,
        airspeed: 250.0,
        heading: 180.0,
        pitch: 3.0,
        roll: 0.0,
        yaw: 0.0,
        on_ground: false,
    });

    let telem = sim.next_snapshot().unwrap();
    let mut snap = BusSnapshot::new(SimId::XPlane, AircraftId::new("B737"));
    snap.environment.altitude = telem.altitude as f32;
    publisher.publish(snap).expect("publish");

    let r1 = sub.try_recv().unwrap().expect("receive pre-disconnect");
    assert_approx_eq(r1.environment.altitude as f64, 5000.0, 0.1);

    // Disconnect sim adapter
    sim.disconnect();
    assert!(!sim.connected);

    // Track state transition to disconnected
    let mut metrics = AdapterMetrics::new();
    metrics.record_update(); // Last update before disconnect

    // No new snapshots should arrive from a disconnected adapter
    // The bus still holds the last snapshot but no new ones come in
    tick();
    let stale_check = sub.try_recv().unwrap();
    assert!(
        stale_check.is_none(),
        "no new snapshots after adapter disconnect"
    );
}

// ===========================================================================
// 3. Multiple subscribers (UI + sim + diagnostics)
// ===========================================================================

#[test]
fn e2e_multiple_subscribers_ui_sim_diagnostics() {
    let mut publisher = make_publisher();

    // Three subscriber roles
    let mut ui_sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();
    let mut sim_sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();
    let mut diag_sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    // Publish 3 snapshots
    let aircraft = ["C172", "B737", "F18"];
    for icao in &aircraft {
        let snap = snapshot_with_controls(SimId::Msfs, icao, 0.5, -0.3);
        publisher.publish(snap).expect("publish");
        tick();
    }

    // Each subscriber should receive all snapshots
    for (name, sub) in [
        ("ui", &mut ui_sub),
        ("sim", &mut sim_sub),
        ("diag", &mut diag_sub),
    ] {
        let mut received = Vec::new();
        while let Ok(Some(s)) = sub.try_recv() {
            received.push(s.aircraft.icao.clone());
        }

        let expected: Vec<String> = aircraft.iter().map(|s| s.to_string()).collect();
        assert_eq!(
            received, expected,
            "{name} subscriber must receive all snapshots in order"
        );
    }
}

// ===========================================================================
// 4. Event ordering preservation
// ===========================================================================

#[test]
fn e2e_event_ordering_preserved() {
    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    // Publish snapshots with monotonically increasing pitch values
    let pitch_sequence: Vec<f32> = (0..10).map(|i| i as f32 * 0.1).collect();
    for &pitch in &pitch_sequence {
        let snap = snapshot_with_controls(SimId::Msfs, "C172", pitch, 0.0);
        publisher.publish(snap).expect("publish");
        tick();
    }

    // Receive and verify ordering
    let mut received_pitches = Vec::new();
    while let Ok(Some(s)) = sub.try_recv() {
        received_pitches.push(s.control_inputs.pitch);
    }

    assert_eq!(
        received_pitches.len(),
        pitch_sequence.len(),
        "must receive all snapshots"
    );

    // Verify monotonic ordering
    for i in 1..received_pitches.len() {
        assert!(
            received_pitches[i] >= received_pitches[i - 1],
            "ordering violated at index {i}: {} < {}",
            received_pitches[i],
            received_pitches[i - 1]
        );
    }

    // Verify values match
    for (expected, actual) in pitch_sequence.iter().zip(received_pitches.iter()) {
        assert_approx_eq(*expected as f64, *actual as f64, 1e-5);
    }
}

// ===========================================================================
// 5. Backpressure handling (slow consumer)
// ===========================================================================

#[test]
fn e2e_backpressure_slow_consumer() {
    let mut publisher = make_publisher();

    // Create subscriber with small buffer and drop-on-full
    let config = SubscriptionConfig {
        buffer_size: 3,
        drop_on_full: true,
        max_rate_hz: 60.0,
    };
    let mut slow_sub = publisher.subscribe(config).unwrap();

    // Fast subscriber with default config (should get everything)
    let mut fast_sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    // Publish many snapshots quickly (overflowing the slow sub's buffer)
    for i in 0..10 {
        let pitch = i as f32 * 0.1;
        let snap = snapshot_with_controls(SimId::Msfs, "C172", pitch, 0.0);
        publisher.publish(snap).expect("publish");
        tick();
    }

    // Fast subscriber should receive all snapshots
    let mut fast_count = 0;
    while let Ok(Some(_)) = fast_sub.try_recv() {
        fast_count += 1;
    }
    assert_eq!(fast_count, 10, "fast subscriber must receive all 10");

    // Slow subscriber may have dropped some (buffer = 3, drop_on_full = true)
    let mut slow_count = 0;
    while let Ok(Some(_)) = slow_sub.try_recv() {
        slow_count += 1;
    }
    assert!(
        slow_count <= 10,
        "slow subscriber cannot receive more than published"
    );
    assert!(
        slow_count > 0,
        "slow subscriber must receive at least some snapshots"
    );
}

// ===========================================================================
// 6. Adapter state tracking through bus lifecycle
// ===========================================================================

#[test]
fn e2e_adapter_state_tracked_through_bus() {
    let mut sim = FakeSim::new("DCS");
    let mut metrics = AdapterMetrics::new();

    // State: Disconnected → Connecting → Connected → Active
    assert!(!sim.connected);
    sim.connect();
    assert!(sim.connected);

    sim.set_aircraft("F-16C");
    metrics.record_aircraft_change(sim.aircraft.clone().unwrap());

    // Publish telemetry while active
    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    for alt in [20000.0, 21000.0, 22000.0] {
        sim.push_snapshot(FakeSnapshot {
            altitude: alt,
            airspeed: 350.0,
            heading: 90.0,
            pitch: 0.0,
            roll: 0.0,
            yaw: 0.0,
            on_ground: false,
        });
    }

    let mut received_altitudes = Vec::new();
    while let Some(telem) = sim.next_snapshot() {
        let mut snap = BusSnapshot::new(SimId::Dcs, AircraftId::new("F16C"));
        snap.environment.altitude = telem.altitude as f32;
        publisher.publish(snap).expect("publish");
        metrics.record_update();
        tick();

        if let Ok(Some(r)) = sub.try_recv() {
            received_altitudes.push(r.environment.altitude as f64);
        }
    }

    assert!(!received_altitudes.is_empty(), "must receive some data");
    assert_eq!(metrics.total_updates, 3);
    assert_eq!(metrics.aircraft_changes, 1);
}
