// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Full service lifecycle end-to-end integration tests.
//!
//! Proves: start service → discover device → detect sim → load profile →
//! process axis → output to sim, all with testkit fakes.

use flight_axis::{AxisEngine, AxisFrame, PipelineBuilder};
use flight_bus::types::SimId;
use flight_bus::{AircraftId, BusPublisher, BusSnapshot, SubscriptionConfig};
use flight_core::profile::{AxisConfig, CapabilityMode, PROFILE_SCHEMA_VERSION, Profile};
use flight_service::{
    AircraftAutoSwitchService, AircraftAutoSwitchServiceConfig, CapabilityService, FlightService,
    FlightServiceConfig, ServiceState,
};
use flight_test_helpers::{DeterministicClock, FakeDevice, FakeInput, FakeSim, assert_in_range};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn tick() {
    std::thread::sleep(Duration::from_millis(25));
}

fn test_profile() -> Profile {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.05),
            expo: Some(0.2),
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes,
        pof_overrides: None,
    }
}

// ===========================================================================
// 1. Full lifecycle: boot → discover → detect → load → process → output
// ===========================================================================

#[tokio::test]
async fn e2e_full_lifecycle_boot_to_axis_output() {
    // 1. Start service
    let mut service = FlightService::new(FlightServiceConfig::default());
    tokio::time::timeout(Duration::from_secs(10), service.start())
        .await
        .expect("start timeout")
        .expect("start ok");
    assert_eq!(service.get_state().await, ServiceState::Running);

    // 2. Discover fake device
    let mut device = FakeDevice::new("HOTAS Warthog", 0x044F, 0xB10A, 4, 12);
    device.connect();
    assert!(device.next_input().is_none(), "no input yet");

    // 3. Detect sim via bus
    let mut bus = BusPublisher::new(60.0);
    let auto_switch =
        AircraftAutoSwitchService::new(AircraftAutoSwitchServiceConfig::default());
    auto_switch.start(&mut bus).await.expect("auto-switch start");

    let snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    bus.publish(snap).expect("publish");
    tokio::time::sleep(Duration::from_millis(300)).await;

    let metrics = auto_switch.get_metrics().await;
    assert!(metrics.aircraft_switch_count >= 1);

    // 4. Load profile
    let profile = test_profile();
    service
        .apply_profile(&profile)
        .await
        .expect("apply profile");

    // 5. Process axis input through compiled engine
    let engine = AxisEngine::new_for_axis("pitch".to_string());
    let pipeline = PipelineBuilder::new()
        .deadzone(0.05)
        .curve(0.2)
        .unwrap()
        .compile()
        .expect("compile");
    engine.update_pipeline(pipeline);

    device.enqueue_input(FakeInput {
        axes: vec![0.7, 0.0, 0.0, 0.0],
        buttons: vec![false; 12],
        delay_ms: 4,
    });

    let input = device.next_input().unwrap();
    let mut frame = AxisFrame::new(input.axes[0] as f32, 4_000_000);
    engine.process(&mut frame).expect("process");

    assert!(frame.out.is_finite());
    assert_in_range(frame.out as f64, -1.0, 1.0);
    assert!(frame.out > 0.0, "positive input → positive output");

    // 6. Output to sim
    let mut sim = FakeSim::new("MSFS 2020");
    sim.connect();
    sim.send_command(&format!("SET_PITCH:{:.4}", frame.out));
    assert_eq!(sim.received_commands().len(), 1);

    // Cleanup
    auto_switch.stop().await.expect("auto-switch stop");
    tokio::time::timeout(Duration::from_secs(10), service.shutdown())
        .await
        .expect("shutdown timeout")
        .expect("shutdown ok");
    assert_eq!(service.get_state().await, ServiceState::Stopped);
}

// ===========================================================================
// 2. Multiple ticks through pipeline at 250Hz
// ===========================================================================

#[tokio::test]
async fn e2e_full_cycle_250hz_processing() {
    let mut service = FlightService::new(FlightServiceConfig::default());
    tokio::time::timeout(Duration::from_secs(10), service.start())
        .await
        .expect("start")
        .expect("start ok");

    let engine = AxisEngine::new_for_axis("pitch".to_string());
    let pipeline = PipelineBuilder::new()
        .deadzone(0.05)
        .curve(0.3)
        .unwrap()
        .compile()
        .expect("compile");
    engine.update_pipeline(pipeline);

    let mut clock = DeterministicClock::new(0);
    let mut device = FakeDevice::new("Stick", 0x044F, 0xB10A, 4, 12);
    device.connect();

    // Process 50 ticks (200ms) of sinusoidal input
    let mut outputs = Vec::new();
    for tick_num in 0..50 {
        let t = tick_num as f64 / 250.0;
        let raw = (t * std::f64::consts::TAU * 2.0).sin() * 0.8;

        let ts_ns = clock.now_us() * 1_000;
        let mut frame = AxisFrame::new(raw as f32, ts_ns);
        engine.process(&mut frame).expect("process");
        outputs.push(frame.out);
        clock.advance_ticks(1);
    }

    assert_eq!(outputs.len(), 50);
    assert_eq!(clock.now_us(), 200_000, "50 ticks = 200ms");

    // All outputs valid
    for (i, &o) in outputs.iter().enumerate() {
        assert!(o.is_finite(), "output[{i}] not finite");
        assert_in_range(o as f64, -1.0, 1.0);
    }

    tokio::time::timeout(Duration::from_secs(10), service.shutdown())
        .await
        .expect("shutdown")
        .expect("shutdown ok");
}

// ===========================================================================
// 3. Service health reflects running subsystems
// ===========================================================================

#[tokio::test]
async fn e2e_service_health_reflects_subsystems() {
    let mut service = FlightService::new(FlightServiceConfig::default());
    tokio::time::timeout(Duration::from_secs(10), service.start())
        .await
        .expect("start")
        .expect("start ok");

    let health = service.get_health_status().await;

    // Core components must be present
    assert!(health.components.contains_key("service"));
    assert!(health.components.contains_key("axis_engine"));
    assert!(health.components.contains_key("auto_switch"));
    assert!(health.components.contains_key("safety"));

    let _ = health.uptime_seconds;

    tokio::time::timeout(Duration::from_secs(10), service.shutdown())
        .await
        .expect("shutdown")
        .expect("shutdown ok");
}

// ===========================================================================
// 4. Capability mode affects axis processing
// ===========================================================================

#[test]
fn e2e_capability_mode_affects_axis_engine() {
    let service = CapabilityService::new();
    let engine = Arc::new(AxisEngine::new_for_axis("pitch".to_string()));

    service
        .register_axis("pitch".to_string(), engine.clone())
        .expect("register");

    // Full mode
    assert_eq!(engine.capability_mode(), CapabilityMode::Full);

    // Switch to Kid mode
    service
        .set_capability_mode(CapabilityMode::Kid, None, false)
        .expect("kid mode");
    assert_eq!(engine.capability_mode(), CapabilityMode::Kid);

    // Switch to Demo mode
    service.set_demo_mode(true).expect("demo mode");
    assert_eq!(engine.capability_mode(), CapabilityMode::Demo);

    // Back to Full
    service.set_demo_mode(false).expect("full mode");
    assert_eq!(engine.capability_mode(), CapabilityMode::Full);
}

// ===========================================================================
// 5. Bus snapshot identity preserved through full cycle
// ===========================================================================

#[tokio::test]
async fn e2e_snapshot_identity_preserved_through_cycle() {
    let mut publisher = BusPublisher::new(60.0);
    let mut subscriber = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let aircraft = [
        (SimId::Msfs, "C172"),
        (SimId::XPlane, "B737"),
        (SimId::Dcs, "F18"),
    ];

    for &(sim, icao) in &aircraft {
        let snap = BusSnapshot::new(sim, AircraftId::new(icao));
        publisher.publish(snap).expect("publish");
        tick();
    }

    let mut received = Vec::new();
    while let Ok(Some(s)) = subscriber.try_recv() {
        received.push((s.sim, s.aircraft.icao.clone()));
    }

    assert_eq!(received.len(), aircraft.len());
    for (i, &(sim, icao)) in aircraft.iter().enumerate() {
        assert_eq!(received[i].0, sim, "sim mismatch at {i}");
        assert_eq!(received[i].1, icao, "aircraft mismatch at {i}");
    }
}
