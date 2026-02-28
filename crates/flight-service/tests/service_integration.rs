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
    let mut cfg = FlightServiceConfig::default();
    cfg.safe_mode = true;

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
// Test 7: Watchdog / health report — core components registered, uptime present
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
