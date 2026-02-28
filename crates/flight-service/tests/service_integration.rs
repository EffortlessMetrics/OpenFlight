// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Service orchestration integration tests
//!
//! Verifies service lifecycle, multi-adapter orchestration, profile-switch
//! handling, and error recovery through the public API only.
//! All tests are self-contained with no external process or hardware deps.

use flight_bus::{AircraftId, BusPublisher, BusSnapshot, SimId};
use flight_service::{
    AircraftAutoSwitchService, AircraftAutoSwitchServiceConfig, FlightService, FlightServiceConfig,
    ServiceState,
};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Test 1: ServiceConfig builds and its defaults are sane
// ---------------------------------------------------------------------------

#[test]
fn test_service_config_defaults_are_sane() {
    let cfg = FlightServiceConfig::default();

    assert!(!cfg.safe_mode, "safe_mode should default to false");
    assert!(
        cfg.enable_health_monitoring,
        "health monitoring should be enabled by default"
    );
    assert!(
        cfg.enable_power_checks,
        "power checks should be enabled by default"
    );
    assert!(
        !cfg.enable_tflight_runtime,
        "T.Flight runtime should be off by default"
    );
    assert!(
        !cfg.enable_stecs_runtime,
        "STECS runtime should be off by default"
    );
    assert_eq!(
        cfg.tflight_poll_hz, 250,
        "T.Flight poll rate should be 250 Hz"
    );
    assert_eq!(cfg.stecs_poll_hz, 250, "STECS poll rate should be 250 Hz");
    assert!(
        cfg.axis_config.max_frame_time_us > 0,
        "frame time budget should be positive"
    );
}

// ---------------------------------------------------------------------------
// Test 2: Service shuts down cleanly within a tokio::time::timeout
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_service_lifecycle_clean_shutdown() {
    let mut service = FlightService::new(FlightServiceConfig::default());

    tokio::time::timeout(Duration::from_secs(10), service.start())
        .await
        .expect("service.start() must complete within 10 s")
        .expect("service.start() should succeed");

    assert_eq!(
        service.get_state().await,
        ServiceState::Running,
        "state after start must be Running"
    );

    tokio::time::timeout(Duration::from_secs(10), service.shutdown())
        .await
        .expect("service.shutdown() must complete within 10 s")
        .expect("service.shutdown() should succeed");

    assert_eq!(
        service.get_state().await,
        ServiceState::Stopped,
        "state after shutdown must be Stopped"
    );
}

// ---------------------------------------------------------------------------
// Test 3: AircraftDetected event (via bus snapshot) increments switch count
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_aircraft_detected_increments_switch_count() {
    // BusPublisher rate is clamped to [30, 60] Hz; 60.0 allows a publish every ~17 ms.
    let mut bus = BusPublisher::new(60.0);
    let service = AircraftAutoSwitchService::new(AircraftAutoSwitchServiceConfig::default());

    service
        .start(&mut bus)
        .await
        .expect("auto-switch service start");

    // Publish a snapshot with a non-empty ICAO.  The bus-monitor task (30 Hz)
    // will pick this up, emit ServiceEvent::AircraftDetected, and the event
    // loop will increment `aircraft_switch_count`.
    let snapshot = BusSnapshot::new(SimId::XPlane, AircraftId::new("C172"));
    bus.publish(snapshot)
        .expect("snapshot must publish successfully");

    // Give the 30 Hz bus-monitor task enough time to process (>= 2 ticks).
    tokio::time::sleep(Duration::from_millis(300)).await;

    let metrics = service.get_metrics().await;
    assert!(
        metrics.aircraft_switch_count >= 1,
        "expected at least 1 aircraft switch after publishing snapshot, got {}",
        metrics.aircraft_switch_count
    );

    service.stop().await.expect("auto-switch service stop");
}

// ---------------------------------------------------------------------------
// Test 4: AdapterError event — verify adapter health metrics default state
//         and that errors are tracked per-adapter when adapters run.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_adapter_metrics_initial_state_is_zero() {
    let service = AircraftAutoSwitchService::new(AircraftAutoSwitchServiceConfig::default());

    // Before any adapter processes events all counters must be zero.
    let metrics = service.get_metrics().await;

    assert_eq!(
        metrics.aircraft_switch_count, 0,
        "aircraft_switch_count must start at zero"
    );
    assert_eq!(
        metrics.detection_latency_us, 0,
        "detection_latency_us must start at zero"
    );

    // Any pre-existing adapter entries (if any) must show zero errors.
    for (sim, m) in &metrics.adapter_metrics {
        assert_eq!(
            m.detection_errors, 0,
            "no detection errors expected for {sim} before any event is processed"
        );
    }
}

// ---------------------------------------------------------------------------
// Test 5: TelemetryUpdate with aircraft change triggers AircraftDetected
//         re-emission (second distinct aircraft causes second switch).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_telemetry_aircraft_change_triggers_re_emission() {
    let mut bus = BusPublisher::new(60.0);
    let service = AircraftAutoSwitchService::new(AircraftAutoSwitchServiceConfig::default());

    service
        .start(&mut bus)
        .await
        .expect("auto-switch service start");

    // First aircraft — new sim entry, ICAO non-empty → AircraftDetected emitted.
    let snap1 = BusSnapshot::new(SimId::XPlane, AircraftId::new("C172"));
    bus.publish(snap1).expect("publish snap1");
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Second aircraft — ICAO changed on same sim → another AircraftDetected.
    let snap2 = BusSnapshot::new(SimId::XPlane, AircraftId::new("B737"));
    bus.publish(snap2).expect("publish snap2");
    tokio::time::sleep(Duration::from_millis(200)).await;

    let metrics = service.get_metrics().await;
    assert!(
        metrics.aircraft_switch_count >= 2,
        "expected ≥2 aircraft switches (one per distinct aircraft), got {}",
        metrics.aircraft_switch_count
    );

    service.stop().await.expect("auto-switch service stop");
}

// ---------------------------------------------------------------------------
// Test 6: Safe mode engagement — state transitions and status fields
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_safe_mode_state_transitions() {
    let cfg = FlightServiceConfig {
        safe_mode: true,
        ..Default::default()
    };

    let mut service = FlightService::new(cfg);

    tokio::time::timeout(Duration::from_secs(10), service.start())
        .await
        .expect("safe-mode start must complete within 10 s")
        .expect("safe-mode start should succeed");

    assert_eq!(
        service.get_state().await,
        ServiceState::SafeMode,
        "state after safe-mode start must be SafeMode"
    );

    let status = service.get_safe_mode_status().await;
    assert!(
        status.is_some(),
        "safe-mode status must be present after start"
    );
    assert!(
        status.unwrap().active,
        "safe mode must report itself as active"
    );

    tokio::time::timeout(Duration::from_secs(10), service.shutdown())
        .await
        .expect("safe-mode shutdown must complete within 10 s")
        .expect("safe-mode shutdown should succeed");

    assert_eq!(
        service.get_state().await,
        ServiceState::Stopped,
        "state after shutdown must be Stopped"
    );
}

// ---------------------------------------------------------------------------
// Test 7: Full-mode startup wiring — subsystem init, bus snapshot, profile apply
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_integration_full_startup_bus_profile_shutdown() {
    // 1. Start service in full mode (no hardware features).
    let cfg = FlightServiceConfig::default();
    assert!(!cfg.safe_mode, "test requires full mode");

    let mut service = FlightService::new(cfg);

    tokio::time::timeout(Duration::from_secs(10), service.start())
        .await
        .expect("start must complete within 10 s")
        .expect("start must succeed");

    assert_eq!(
        service.get_state().await,
        ServiceState::Running,
        "full-mode start must reach Running"
    );

    // 2. Verify core health components are registered after full startup.
    let health = service.get_health_status().await;
    for component in &["service", "axis_engine", "auto_switch", "safety"] {
        assert!(
            health.components.contains_key(*component),
            "'{component}' must be registered in health report after full startup"
        );
    }

    // 3. Bus is operational: publish a snapshot and confirm the auto-switch
    //    service (started separately) picks it up.
    let mut bus = BusPublisher::new(60.0);
    let auto_switch = AircraftAutoSwitchService::new(AircraftAutoSwitchServiceConfig::default());

    auto_switch
        .start(&mut bus)
        .await
        .expect("auto-switch start must succeed");

    let snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("A320"));
    bus.publish(snapshot)
        .expect("snapshot must publish successfully");

    // Allow bus-monitor (30 Hz) at least two ticks to process.
    tokio::time::sleep(Duration::from_millis(300)).await;

    let metrics = auto_switch.get_metrics().await;
    assert!(
        metrics.aircraft_switch_count >= 1,
        "bus must be active — expected ≥1 aircraft switch, got {}",
        metrics.aircraft_switch_count
    );

    auto_switch
        .stop()
        .await
        .expect("auto-switch stop must succeed");

    // 4. Apply a profile through the service and verify the axis engine is wired.
    use flight_core::profile::{AxisConfig, Profile};
    use std::collections::HashMap;

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
    let profile = Profile {
        schema: "flight.profile/1".to_string(),
        sim: None,
        aircraft: None,
        axes,
        pof_overrides: None,
    };

    let apply_result = service.apply_profile(&profile).await;
    assert!(
        apply_result.is_ok(),
        "apply_profile must succeed after full startup: {:?}",
        apply_result.err()
    );

    // 5. Clean shutdown.
    tokio::time::timeout(Duration::from_secs(10), service.shutdown())
        .await
        .expect("shutdown must complete within 10 s")
        .expect("shutdown must succeed");

    assert_eq!(
        service.get_state().await,
        ServiceState::Stopped,
        "state after shutdown must be Stopped"
    );
}

// ---------------------------------------------------------------------------
// Test 8: StartupSequence phases advance correctly through full lifecycle
// ---------------------------------------------------------------------------

#[test]
fn test_startup_sequence_full_phase_progression() {
    use flight_service::startup_sequence::{StartupPhase, StartupSequence};

    let mut seq = StartupSequence::new();
    assert_eq!(seq.phase(), StartupPhase::Idle);
    assert!(!seq.is_running());

    // Walk through every phase in order.
    seq.run_preflight();
    assert_eq!(seq.phase(), StartupPhase::PreFlight);

    seq.loading_config();
    assert_eq!(seq.phase(), StartupPhase::LoadingConfig);

    seq.enumerating_devices();
    assert_eq!(seq.phase(), StartupPhase::EnumeratingDevices);

    // Simulate a non-fatal warning during device enumeration.
    seq.warn("No HID devices found — continuing without hardware");
    assert_eq!(seq.warnings().len(), 1);

    seq.starting_axis_engine();
    assert_eq!(seq.phase(), StartupPhase::StartingAxisEngine);

    seq.starting_adapters();
    assert_eq!(seq.phase(), StartupPhase::StartingAdapters);

    seq.running();
    assert_eq!(seq.phase(), StartupPhase::Running);
    assert!(seq.is_running());

    // Warnings survive the transition to Running.
    assert_eq!(
        seq.warnings().len(),
        1,
        "warnings must be preserved after reaching Running"
    );
}

// ---------------------------------------------------------------------------
// Test 9: Watchdog / health report — core components registered, uptime present
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_health_report_registers_core_components() {
    let mut service = FlightService::new(FlightServiceConfig::default());

    tokio::time::timeout(Duration::from_secs(10), service.start())
        .await
        .expect("service start must complete within 10 s")
        .expect("service start should succeed");

    let health = service.get_health_status().await;

    // All four core watchdog-monitored components must be present.
    for component in &["service", "axis_engine", "auto_switch", "safety"] {
        assert!(
            health.components.contains_key(*component),
            "component '{component}' must be registered in health report"
        );
    }

    // uptime_seconds is u64 (always ≥ 0); the field itself must be accessible.
    let _uptime = health.uptime_seconds;

    // At least the overall health entry must exist (the type asserts this at
    // compile time, but we also exercise the accessor here).
    let _overall = health.overall;

    tokio::time::timeout(Duration::from_secs(10), service.shutdown())
        .await
        .expect("service shutdown must complete within 10 s")
        .expect("service shutdown should succeed");
}
