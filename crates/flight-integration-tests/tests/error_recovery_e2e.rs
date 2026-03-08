// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Error recovery end-to-end integration tests.
//!
//! Proves: device disconnect, sim disconnect, profile load failure, service
//! restart recovery, and watchdog trigger/recovery all work correctly.

use flight_axis::pipeline::{AxisPipeline, ClampStage, CurveStage, DeadzoneStage};
use flight_axis::{AxisEngine, AxisFrame, InputValidator, PipelineBuilder};
use flight_bus::types::SimId;
use flight_bus::{AircraftId, BusPublisher, BusSnapshot, SubscriptionConfig};
use flight_core::profile::{AxisConfig, PROFILE_SCHEMA_VERSION, Profile};
use flight_service::{FlightService, FlightServiceConfig, ServiceState};
use flight_test_helpers::{
    FakeDevice, FakeInput, FakeSim, assert_approx_eq, assert_in_range,
};
use flight_watchdog::supervisor::{
    DeadManStatus, DeadManSwitch, DeadManSwitchConfig, HardwareWatchdog, WatchdogTimerConfig,
    WatchdogTimerStatus,
};
use flight_watchdog::{
    ComponentType, QuarantineStatus, WatchdogAction, WatchdogConfig, WatchdogEventType,
    WatchdogSystem,
};
use std::collections::HashMap;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn standard_pipeline() -> AxisPipeline {
    let mut pipeline = AxisPipeline::new();
    pipeline.add_stage(Box::new(DeadzoneStage {
        inner: 0.05,
        outer: 1.0,
    }));
    pipeline.add_stage(Box::new(CurveStage { expo: 0.3 }));
    pipeline.add_stage(Box::new(ClampStage {
        min: -1.0,
        max: 1.0,
    }));
    pipeline
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
// 1. Device disconnect mid-flight
// ===========================================================================

#[test]
fn e2e_device_disconnect_mid_flight_pipeline_holds_last_value() {
    let mut device = FakeDevice::new("HOTAS", 0x044F, 0xB10A, 4, 12);
    device.connect();

    let pipeline = standard_pipeline();

    // Process a valid input
    device.enqueue_input(FakeInput {
        axes: vec![0.6, 0.0, 0.0, 0.0],
        buttons: vec![false; 12],
        delay_ms: 4,
    });

    let input = device.next_input().unwrap();
    let last_valid_output = pipeline.process(input.axes[0], 0.004);
    assert!(last_valid_output > 0.0);

    // Device disconnects
    device.disconnect();
    assert!(device.next_input().is_none(), "no input after disconnect");

    // Pipeline still holds last output; validator uses last known good value
    let mut validator = InputValidator::new();
    validator.update(last_valid_output as f32);

    // Simulate NaN from disconnected device
    let sanitised = validator.update(f32::NAN);
    assert_approx_eq(
        sanitised as f64,
        last_valid_output,
        1e-5,
    );

    // Reconnect
    device.connect();
    device.enqueue_input(FakeInput {
        axes: vec![0.3, 0.0, 0.0, 0.0],
        buttons: vec![false; 12],
        delay_ms: 4,
    });

    let new_input = device.next_input().unwrap();
    let new_output = pipeline.process(new_input.axes[0], 0.004);
    assert!(new_output.is_finite());
    assert_in_range(new_output, -1.0, 1.0);
}

// ===========================================================================
// 2. Sim disconnect mid-flight
// ===========================================================================

#[test]
fn e2e_sim_disconnect_mid_flight_bus_continues() {
    let mut sim = FakeSim::new("MSFS 2020");
    sim.connect();

    let mut publisher = BusPublisher::new(60.0);
    let mut subscriber = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    // Publish while sim is connected
    let snap1 = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    publisher.publish(snap1).unwrap();
    std::thread::sleep(Duration::from_millis(25));

    let received1 = subscriber.try_recv().unwrap();
    assert!(received1.is_some(), "snapshot arrives while sim connected");

    // Sim disconnects
    sim.disconnect();

    // Bus publisher still works (sim disconnect is external)
    let snap2 = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    publisher.publish(snap2).unwrap();
    std::thread::sleep(Duration::from_millis(25));

    let received2 = subscriber.try_recv().unwrap();
    assert!(
        received2.is_some(),
        "bus continues publishing after sim disconnect"
    );

    // Sim reconnects and receives new data
    sim.connect();
    sim.send_command("RECONNECTED");
    assert_eq!(sim.received_commands().len(), 1);
}

// ===========================================================================
// 3. Profile load failure
// ===========================================================================

#[test]
fn e2e_invalid_profile_rejected_pipeline_unchanged() {
    let engine = AxisEngine::new_for_axis("pitch".to_string());

    // Load valid pipeline
    let pipeline_v1 = PipelineBuilder::new()
        .deadzone(0.05)
        .curve(0.2)
        .unwrap()
        .compile()
        .expect("v1 compile");
    engine.update_pipeline(pipeline_v1);

    let mut frame1 = AxisFrame::new(0.5, 1_000_000);
    engine.process(&mut frame1).expect("process v1");
    let output_v1 = frame1.out;

    // Try to apply invalid profile
    let invalid_profile = Profile {
        schema: "invalid_version".to_string(),
        sim: None,
        aircraft: None,
        axes: HashMap::new(),
        pof_overrides: None,
    };
    let validation = invalid_profile.validate();
    assert!(validation.is_err(), "invalid schema must fail validation");

    // Original pipeline still works
    let mut frame2 = AxisFrame::new(0.5, 2_000_000);
    engine.process(&mut frame2).expect("process still works");
    let output_after = frame2.out;

    assert_approx_eq(
        output_v1 as f64,
        output_after as f64,
        1e-6,
    );
}

// ===========================================================================
// 4. Service restart recovery
// ===========================================================================

#[tokio::test]
async fn e2e_service_restart_recovers_state() {
    // First boot
    let mut service = FlightService::new(FlightServiceConfig::default());
    tokio::time::timeout(Duration::from_secs(10), service.start())
        .await
        .expect("start1")
        .expect("start1 ok");
    assert_eq!(service.get_state().await, ServiceState::Running);

    // Apply profile before shutdown
    let profile = test_profile();
    service
        .apply_profile(&profile)
        .await
        .expect("apply profile");

    // Shutdown
    tokio::time::timeout(Duration::from_secs(10), service.shutdown())
        .await
        .expect("shutdown1")
        .expect("shutdown1 ok");
    assert_eq!(service.get_state().await, ServiceState::Stopped);

    // Second boot — service recovers
    let mut service2 = FlightService::new(FlightServiceConfig::default());
    tokio::time::timeout(Duration::from_secs(10), service2.start())
        .await
        .expect("start2")
        .expect("start2 ok");
    assert_eq!(service2.get_state().await, ServiceState::Running);

    // Health check passes after restart
    let health = service2.get_health_status().await;
    assert!(health.components.contains_key("service"));

    // Can reapply profile
    service2
        .apply_profile(&profile)
        .await
        .expect("reapply profile");

    tokio::time::timeout(Duration::from_secs(10), service2.shutdown())
        .await
        .expect("shutdown2")
        .expect("shutdown2 ok");
}

// ===========================================================================
// 5. Watchdog trigger and recovery
// ===========================================================================

#[test]
fn e2e_watchdog_trigger_quarantines_and_recovers() {
    let mut watchdog = WatchdogSystem::new();

    let component = ComponentType::UsbEndpoint("joystick_ep0".to_string());
    let config = WatchdogConfig {
        max_execution_time: Duration::from_micros(100),
        usb_timeout: Duration::from_millis(50),
        max_consecutive_failures: 3,
        failure_rate_window: Duration::from_secs(60),
        max_failures_per_window: 10,
        enable_nan_guards: true,
        is_critical: false,
    };

    watchdog.register_component(component.clone(), config);

    // Initially active
    assert_eq!(
        watchdog.get_quarantine_status(&component),
        Some(&QuarantineStatus::Active)
    );

    // Simulate 3 consecutive USB errors → quarantine
    for i in 0..3 {
        let event = watchdog.record_usb_error("joystick_ep0", &format!("error {i}"));
        if i < 2 {
            assert_eq!(event.action_taken, WatchdogAction::ResetUsbEndpoint);
        }
    }

    assert!(
        watchdog.is_quarantined(&component),
        "3 failures must quarantine component"
    );

    // Attempt recovery
    let recovered = watchdog.attempt_recovery(&component);
    assert!(recovered, "recovery attempt must succeed");
    assert!(
        matches!(
            watchdog.get_quarantine_status(&component),
            Some(QuarantineStatus::Recovering { .. })
        ),
        "component must be in Recovering state"
    );
}

#[test]
fn e2e_watchdog_nan_guard_detects_bad_values() {
    let mut watchdog = WatchdogSystem::new();

    let component = ComponentType::AxisNode("pitch".to_string());
    let config = WatchdogConfig {
        enable_nan_guards: true,
        is_critical: true,
        ..WatchdogConfig::default()
    };

    watchdog.register_component(component.clone(), config);

    // NaN detection
    let nan_event = watchdog.check_nan_guard(f32::NAN, "pitch_output", component.clone());
    assert!(nan_event.is_some(), "NaN must trigger guard");

    let event = nan_event.unwrap();
    assert_eq!(event.event_type, WatchdogEventType::NanDetected);
    assert_eq!(
        event.action_taken,
        WatchdogAction::EmergencyStop,
        "critical component NaN → emergency stop"
    );

    // Inf detection
    let inf_event =
        watchdog.check_nan_guard(f32::INFINITY, "pitch_output", component.clone());
    assert!(inf_event.is_some(), "Inf must trigger guard");
}

#[test]
fn e2e_hardware_watchdog_pet_and_expire() {
    let config = WatchdogTimerConfig {
        timeout: Duration::from_millis(50),
        max_timeouts: 3,
    };

    let mut watchdog = HardwareWatchdog::new(config);

    // Pet immediately → OK
    watchdog.pet();
    assert_eq!(watchdog.check(), WatchdogTimerStatus::Ok);

    // Wait beyond timeout
    std::thread::sleep(Duration::from_millis(60));

    let status = watchdog.check();
    assert!(
        matches!(status, WatchdogTimerStatus::Warning { .. }),
        "first timeout → warning"
    );

    // Pet resets consecutive count
    watchdog.pet();
    assert_eq!(watchdog.check(), WatchdogTimerStatus::Ok);
    assert!(watchdog.total_timeouts() >= 1);
}

#[test]
fn e2e_dead_man_switch_detects_stalled_engine() {
    let config = DeadManSwitchConfig {
        expected_interval: Duration::from_millis(10),
        missed_intervals_threshold: 3,
    };

    let mut dms = DeadManSwitch::new(config);

    // Tick normally → alive
    dms.tick();
    assert_eq!(dms.check(), DeadManStatus::Alive);

    // Wait long enough to miss several intervals
    std::thread::sleep(Duration::from_millis(50));

    let status = dms.check();
    assert!(
        matches!(status, DeadManStatus::Triggered { .. }),
        "stalled engine must trigger dead-man switch"
    );

    assert!(dms.total_triggers() >= 1);

    // Reset and tick again → alive
    dms.reset();
    dms.tick();
    assert_eq!(dms.check(), DeadManStatus::Alive);
}

#[test]
fn e2e_plugin_overrun_quarantines_after_threshold() {
    let mut watchdog = WatchdogSystem::new();

    let component = ComponentType::NativePlugin("bad_plugin".to_string());
    let config = WatchdogConfig {
        max_execution_time: Duration::from_micros(100),
        max_consecutive_failures: 3,
        ..WatchdogConfig::default()
    };

    watchdog.register_component(component.clone(), config);

    // 3 overruns → quarantine
    for _ in 0..3 {
        watchdog.record_plugin_execution("bad_plugin", Duration::from_micros(200), true);
    }

    assert!(
        watchdog.is_quarantined(&component),
        "3 overruns must quarantine plugin"
    );

    let quarantined = watchdog.get_quarantined_components();
    assert_eq!(quarantined.len(), 1);
    assert_eq!(quarantined[0], component);
}
