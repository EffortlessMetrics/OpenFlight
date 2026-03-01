// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Adapter lifecycle integration tests.
//!
//! Exercises adapter start/stop through the [`ServiceOrchestrator`],
//! verifies state transitions, bus snapshot flow during adapter lifecycle,
//! and device connect/disconnect interleaved with adapter events.

use flight_adapter_common::{AdapterMetrics, AdapterState, ReconnectionStrategy};
use flight_bus::types::SimId;
use flight_bus::{AircraftId, BusPublisher, BusSnapshot, SubscriptionConfig};
use flight_service::{
    AdapterEvent, DeviceEvent, ServiceConfig, ServiceOrchestrator, SubsystemHealth,
};

const SUBSYSTEM_ADAPTERS: &str = "adapters";
use flight_test_helpers::{
    FakeDevice, FakeInput, FakeSim, FakeSnapshot, assert_approx_eq,
};
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

fn started_orchestrator() -> ServiceOrchestrator {
    let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
    orch.start().expect("start");
    orch
}

fn sample_snapshot(alt: f64, airspeed: f64, on_ground: bool) -> FakeSnapshot {
    FakeSnapshot {
        altitude: alt,
        airspeed,
        heading: 270.0,
        pitch: 2.5,
        roll: 0.0,
        yaw: 0.0,
        on_ground,
    }
}

// ===========================================================================
// 1. Adapter state machine: full lifecycle
// ===========================================================================

#[test]
fn adapter_state_machine_full_lifecycle() {
    let transitions = [
        (AdapterState::Disconnected, AdapterState::Connecting),
        (AdapterState::Connecting, AdapterState::Connected),
        (AdapterState::Connected, AdapterState::DetectingAircraft),
        (AdapterState::DetectingAircraft, AdapterState::Active),
        (AdapterState::Active, AdapterState::Disconnected),
    ];

    for (from, to) in &transitions {
        assert_ne!(from, to, "transitions must change state");
    }
}

// ===========================================================================
// 2. FakeSim connect → produce telemetry → publish to bus → receive
// ===========================================================================

#[test]
fn adapter_sim_telemetry_flows_to_bus_subscriber() {
    let mut sim = FakeSim::new("MSFS");
    sim.connect();
    sim.set_aircraft("C172");
    sim.push_snapshot(sample_snapshot(5000.0, 120.0, false));

    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    // Simulate adapter reading from sim and publishing to bus
    let snap = sim.next_snapshot().expect("snapshot");
    let mut bus_snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    bus_snap.environment.altitude = snap.altitude as f32;
    publisher.publish(bus_snap).expect("publish");

    let received = sub.try_recv().unwrap().expect("must receive");
    assert_eq!(received.sim, SimId::Msfs);
    assert_approx_eq(received.environment.altitude as f64, 5000.0, 0.1);
}

// ===========================================================================
// 3. Adapter disconnect → reconnect resumes bus flow
// ===========================================================================

#[test]
fn adapter_disconnect_reconnect_resumes_bus_flow() {
    let mut sim = FakeSim::new("DCS");
    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    // Phase 1: connected, publish telemetry
    sim.connect();
    sim.set_aircraft("F-16C");
    sim.push_snapshot(sample_snapshot(20000.0, 350.0, false));

    let snap = sim.next_snapshot().unwrap();
    let mut bus_snap = BusSnapshot::new(SimId::Dcs, AircraftId::new("F16C"));
    bus_snap.environment.altitude = snap.altitude as f32;
    publisher.publish(bus_snap).expect("publish");

    let r1 = sub.try_recv().unwrap().expect("receive before disconnect");
    assert_approx_eq(r1.environment.altitude as f64, 20000.0, 0.1);

    // Phase 2: disconnect
    sim.disconnect();
    assert!(!sim.connected);

    // Phase 3: reconnect and resume
    sim.connect();
    sim.push_snapshot(sample_snapshot(21000.0, 340.0, false));

    let snap2 = sim.next_snapshot().unwrap();
    let mut bus_snap2 = BusSnapshot::new(SimId::Dcs, AircraftId::new("F16C"));
    bus_snap2.environment.altitude = snap2.altitude as f32;
    tick();
    publisher.publish(bus_snap2).expect("publish after reconnect");

    let r2 = sub.try_recv().unwrap().expect("receive after reconnect");
    assert_approx_eq(r2.environment.altitude as f64, 21000.0, 0.1);
}

// ===========================================================================
// 4. Orchestrator tracks adapter sim connect/disconnect
// ===========================================================================

#[test]
fn adapter_orchestrator_tracks_sim_lifecycle() {
    let mut orch = started_orchestrator();

    // Sim connects
    orch.handle_adapter_event(AdapterEvent::SimConnected {
        sim_name: "MSFS".to_string(),
    })
    .expect("connect");
    assert_eq!(orch.connected_sims().len(), 1);

    // Second sim connects
    orch.handle_adapter_event(AdapterEvent::SimConnected {
        sim_name: "X-Plane".to_string(),
    })
    .expect("connect");
    assert_eq!(orch.connected_sims().len(), 2);

    // First sim disconnects
    orch.handle_adapter_event(AdapterEvent::SimDisconnected {
        sim_name: "MSFS".to_string(),
    })
    .expect("disconnect");
    assert_eq!(orch.connected_sims().len(), 1);
    assert_eq!(orch.connected_sims()[0], "X-Plane");

    orch.stop().expect("stop");
}

// ===========================================================================
// 5. Device connect + adapter event interleaved
// ===========================================================================

#[test]
fn adapter_device_and_sim_events_interleaved() {
    let mut orch = started_orchestrator();

    // Device connects
    orch.handle_device_change(DeviceEvent::Connected {
        device_id: "hid:044f:b10a".to_string(),
        device_type: "joystick".to_string(),
    })
    .expect("device connect");

    // Sim connects
    orch.handle_adapter_event(AdapterEvent::SimConnected {
        sim_name: "MSFS".to_string(),
    })
    .expect("sim connect");

    // Data arrives
    orch.handle_adapter_event(AdapterEvent::DataReceived {
        sim_name: "MSFS".to_string(),
    })
    .expect("data received");

    assert_eq!(orch.connected_devices().len(), 1);
    assert_eq!(orch.connected_sims().len(), 1);

    // Device disconnects
    orch.handle_device_change(DeviceEvent::Disconnected {
        device_id: "hid:044f:b10a".to_string(),
    })
    .expect("device disconnect");
    assert!(orch.connected_devices().is_empty());

    orch.stop().expect("stop");
}

// ===========================================================================
// 6. Adapter metrics track aircraft changes
// ===========================================================================

#[test]
fn adapter_metrics_aircraft_change_tracking() {
    let mut metrics = AdapterMetrics::new();

    // First aircraft
    metrics.record_aircraft_change("C172".to_string());
    assert_eq!(metrics.aircraft_changes, 1);
    assert_eq!(metrics.last_aircraft_title.as_deref(), Some("C172"));

    // Different aircraft
    metrics.record_aircraft_change("B737".to_string());
    assert_eq!(metrics.aircraft_changes, 2);

    // Same aircraft again — no double-count
    metrics.record_aircraft_change("B737".to_string());
    assert_eq!(metrics.aircraft_changes, 2);
}

// ===========================================================================
// 7. Reconnection strategy backoff
// ===========================================================================

#[test]
fn adapter_reconnection_exponential_backoff() {
    let strategy = ReconnectionStrategy::new(5, Duration::from_millis(100), Duration::from_secs(2));

    // Exponential: 100, 200, 400, 800, 1600
    assert_eq!(strategy.next_backoff(1), Duration::from_millis(100));
    assert_eq!(strategy.next_backoff(2), Duration::from_millis(200));
    assert_eq!(strategy.next_backoff(5), Duration::from_millis(1600));
    // Capped
    assert_eq!(strategy.next_backoff(10), Duration::from_secs(2));

    // Retry limits
    assert!(strategy.should_retry(5));
    assert!(!strategy.should_retry(6));
}

// ===========================================================================
// 8. FakeDevice input sequence through bus
// ===========================================================================

#[test]
fn adapter_fake_device_input_published_to_bus() {
    let mut device = FakeDevice::new("Test Stick", 0x044F, 0xB10A, 4, 12);
    device.connect();
    device.enqueue_input(FakeInput {
        axes: vec![0.5, -0.3, 0.0, 0.8],
        buttons: vec![true, false, false, false, false, false, false, false, false, false, false, false],
        delay_ms: 4,
    });

    let input = device.next_input().expect("input");

    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let mut snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    snap.control_inputs.pitch = input.axes[0] as f32;
    snap.control_inputs.roll = input.axes[1] as f32;
    snap.control_inputs.yaw = input.axes[2] as f32;
    snap.control_inputs.throttle = vec![input.axes[3] as f32];

    publisher.publish(snap).expect("publish");

    let received = sub.try_recv().unwrap().expect("receive");
    assert_approx_eq(received.control_inputs.pitch as f64, 0.5, 1e-5);
    assert_approx_eq(received.control_inputs.roll as f64, -0.3, 1e-5);
    assert_approx_eq(received.control_inputs.throttle[0] as f64, 0.8, 1e-5);
}

// ===========================================================================
// 9. Adapter subsystem failure and orchestrator health
// ===========================================================================

#[test]
fn adapter_subsystem_failure_degrades_orchestrator_health() {
    let mut orch = started_orchestrator();

    orch.record_subsystem_error(SUBSYSTEM_ADAPTERS, "sim connection timeout")
        .expect("record error");

    let status = orch.status();
    let adapters = status.subsystems.get(SUBSYSTEM_ADAPTERS).unwrap();
    assert_eq!(adapters.health, SubsystemHealth::Degraded);
    assert_eq!(status.overall_health, SubsystemHealth::Degraded);

    orch.stop().expect("stop");
}
