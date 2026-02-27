// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests for flight-service orchestration and auto-switch.
//!
//! Covers:
//! - Auto-switch service idle lifecycle (start/stop, no hardware needed)
//! - Empty-ICAO snapshots are ignored
//! - Safe mode diagnostic bundle and basic-profile axis validation
//! - Capability service clamp counter and mode limits
//!
//! All tests are self-contained; no MSFS, X-Plane, or real hardware required.

use flight_axis::{AxisEngine, AxisFrame};
use flight_bus::{AircraftId, BusPublisher, BusSnapshot, SimId};
use flight_core::profile::CapabilityMode;
use flight_service::{
    AircraftAutoSwitchService, AircraftAutoSwitchServiceConfig, CapabilityService, FlightService,
    FlightServiceConfig, SafeModeConfig, SafeModeManager,
};
use std::sync::Arc;
use std::time::Duration;

// ============================================================================
// Auto-switch lifecycle
// ============================================================================

/// A freshly-constructed service (never started) must report zero for all
/// numeric metrics — no background tasks have had a chance to modify them.
#[tokio::test]
async fn test_auto_switch_metrics_are_zero_before_any_start() {
    let service = AircraftAutoSwitchService::new(AircraftAutoSwitchServiceConfig::default());
    let metrics = service.get_metrics().await;

    assert_eq!(
        metrics.aircraft_switch_count, 0,
        "aircraft_switch_count must start at zero"
    );
    assert_eq!(
        metrics.detection_latency_ms, 0,
        "detection_latency_ms must start at zero"
    );
    assert_eq!(
        metrics.last_detection_time_ms, 0,
        "last_detection_time_ms must start at zero"
    );
}

/// Start the service, publish no events, then stop — switch count must stay 0.
#[tokio::test]
async fn test_auto_switch_idle_lifecycle_start_stop() {
    let mut bus = BusPublisher::new(60.0);
    let service = AircraftAutoSwitchService::new(AircraftAutoSwitchServiceConfig::default());

    service
        .start(&mut bus)
        .await
        .expect("auto-switch start must succeed");

    // No snapshots published — switch count must remain 0.
    let metrics = service.get_metrics().await;
    assert_eq!(
        metrics.aircraft_switch_count, 0,
        "switch count must stay 0 when no snapshots are published"
    );

    service.stop().await.expect("auto-switch stop must succeed");
}

/// A snapshot whose aircraft ICAO field is empty must NOT trigger a switch.
/// The event loop only emits AircraftDetected when the ICAO is non-empty.
#[tokio::test]
async fn test_auto_switch_empty_icao_snapshot_is_ignored() {
    let mut bus = BusPublisher::new(60.0);
    let service = AircraftAutoSwitchService::new(AircraftAutoSwitchServiceConfig::default());

    service
        .start(&mut bus)
        .await
        .expect("auto-switch start must succeed");

    // Publish a snapshot with an empty ICAO.
    let empty_snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new(""));
    bus.publish(empty_snapshot)
        .expect("publish must not fail");

    // Allow at least two bus-monitor ticks (30 Hz → ~33 ms each) to pass.
    tokio::time::sleep(Duration::from_millis(200)).await;

    let metrics = service.get_metrics().await;
    assert_eq!(
        metrics.aircraft_switch_count, 0,
        "empty ICAO must not increment aircraft_switch_count"
    );

    service.stop().await.expect("auto-switch stop must succeed");
}

// ============================================================================
// Safe mode — diagnostic bundle and basic-profile defaults
// ============================================================================

/// initialize() must produce at least one validation result (the diagnostic
/// bundle is non-empty), and the status must be marked active.
#[tokio::test]
async fn test_safe_mode_diagnostic_bundle_is_non_empty() {
    let config = SafeModeConfig {
        axis_only: true,
        use_basic_profile: true,
        skip_power_checks: true, // skip slow OS power queries in CI
        minimal_mode: true,
    };

    let mut manager = SafeModeManager::new(config);
    let status = manager
        .initialize()
        .await
        .expect("SafeModeManager::initialize must succeed");

    assert!(status.active, "safe mode must report itself as active");
    assert!(
        !status.validation_results.is_empty(),
        "at least one validation result must be produced"
    );
}

/// The built-in basic profile must pass validation — its deadzone (≥ 0.01)
/// and expo (0.0–0.2) values must be within acceptable ranges.  This is
/// confirmed by checking that the 'Basic Profile' entry in the diagnostic
/// bundle has success == true.
#[tokio::test]
async fn test_safe_mode_basic_profile_validation_reports_success() {
    let config = SafeModeConfig {
        axis_only: true,
        use_basic_profile: true,
        skip_power_checks: true,
        minimal_mode: true,
    };

    let mut manager = SafeModeManager::new(config);
    let status = manager
        .initialize()
        .await
        .expect("SafeModeManager::initialize must succeed");

    let profile_result = status
        .validation_results
        .iter()
        .find(|r| r.component == "Basic Profile");

    assert!(
        profile_result.is_some(),
        "validation results must contain a 'Basic Profile' entry"
    );
    assert!(
        profile_result.unwrap().success,
        "basic profile validation must succeed \
         (deadzone ≥ 0.01 and expo in [0, 1] are sane defaults)"
    );
}

/// Safe mode initialization must include an 'Axis Engine' validation entry
/// when axis_only is true, confirming the engine is spun up in safe mode.
#[tokio::test]
async fn test_safe_mode_axis_engine_initializes_in_safe_mode() {
    let config = SafeModeConfig {
        axis_only: true,
        use_basic_profile: false, // isolate axis engine init from profile validation
        skip_power_checks: true,
        minimal_mode: true,
    };

    let mut manager = SafeModeManager::new(config);
    let status = manager
        .initialize()
        .await
        .expect("SafeModeManager::initialize must succeed");

    let engine_result = status
        .validation_results
        .iter()
        .find(|r| r.component == "Axis Engine");

    assert!(
        engine_result.is_some(),
        "validation results must include an 'Axis Engine' entry when axis_only = true"
    );
    assert!(
        engine_result.unwrap().success,
        "axis engine init must succeed in safe mode"
    );
}

/// FlightService started in safe mode must expose a non-None safe-mode status
/// with active == true.
#[tokio::test]
async fn test_safe_mode_service_exposes_active_status() {
    let mut cfg = FlightServiceConfig::default();
    cfg.safe_mode = true;

    let mut service = FlightService::new(cfg);

    tokio::time::timeout(Duration::from_secs(10), service.start())
        .await
        .expect("start must complete within 10 s")
        .expect("start must succeed");

    let status = service.get_safe_mode_status().await;
    assert!(
        status.is_some(),
        "get_safe_mode_status must return Some when safe mode is active"
    );
    assert!(
        status.unwrap().active,
        "SafeModeStatus.active must be true when service is in safe mode"
    );

    tokio::time::timeout(Duration::from_secs(10), service.shutdown())
        .await
        .expect("shutdown must complete within 10 s")
        .expect("shutdown must succeed");
}

// ============================================================================
// Capability service — clamp counter and mode limits
// ============================================================================

/// A freshly registered axis must have clamp_events_count == 0 and no
/// last_clamp_timestamp.
#[test]
fn test_capability_clamp_counter_starts_at_zero() {
    let service = CapabilityService::new();
    let engine = Arc::new(AxisEngine::new_for_axis("pitch".to_string()));

    service
        .register_axis("pitch".to_string(), engine)
        .expect("register_axis must succeed");

    let status_list = service
        .get_capability_status(None)
        .expect("get_capability_status must succeed");

    assert_eq!(status_list.len(), 1);
    assert_eq!(
        status_list[0].clamp_events_count, 0,
        "clamp_events_count must be 0 for a fresh axis"
    );
    assert!(
        status_list[0].last_clamp_timestamp.is_none(),
        "last_clamp_timestamp must be None before any frames are processed"
    );
}

/// Processing a frame whose output exceeds the kid-mode limit (50 %) must
/// increment the clamp counter to 1 and set last_clamp_timestamp.
#[test]
fn test_capability_kid_mode_clamp_increments_counter() {
    let service = CapabilityService::new();
    let engine = Arc::new(AxisEngine::new_for_axis("throttle".to_string()));

    service
        .register_axis("throttle".to_string(), engine.clone())
        .expect("register_axis must succeed");

    // Engage kid mode: max output = 50 %.
    service
        .set_kid_mode(true)
        .expect("set_kid_mode must succeed");

    // Process a frame with out = 0.9 → should be clamped to 0.5.
    let mut frame = AxisFrame::new(0.9, 1_000);
    frame.out = 0.9;
    engine.process(&mut frame).expect("process must succeed");

    let status_list = service
        .get_capability_status(None)
        .expect("get_capability_status must succeed");

    assert_eq!(
        status_list[0].clamp_events_count, 1,
        "one over-limit frame must increment clamp_events_count to 1"
    );
    assert!(
        status_list[0].last_clamp_timestamp.is_some(),
        "last_clamp_timestamp must be set after a clamped frame"
    );
}

/// Demo mode must advertise max_axis_output == 0.8 via AxisCapabilityStatus
/// and must actually clamp frame.out to 0.8 during processing.
#[test]
fn test_demo_mode_limits_and_output_clamping() {
    let service = CapabilityService::new();
    let engine = Arc::new(AxisEngine::new_for_axis("roll".to_string()));

    service
        .register_axis("roll".to_string(), engine.clone())
        .expect("register_axis must succeed");

    service
        .set_demo_mode(true)
        .expect("set_demo_mode must succeed");

    // --- Verify reported limits ---
    let status_list = service
        .get_capability_status(None)
        .expect("get_capability_status must succeed");

    assert_eq!(
        status_list[0].mode,
        CapabilityMode::Demo,
        "mode must be Demo after set_demo_mode(true)"
    );
    assert_eq!(
        status_list[0].limits.max_axis_output, 0.8,
        "demo mode max_axis_output must be 0.8"
    );

    // --- Verify actual output clamping ---
    let mut frame = AxisFrame::new(0.95, 2_000);
    frame.out = 0.95;
    engine.process(&mut frame).expect("process must succeed");

    assert_eq!(
        frame.out, 0.8,
        "frame output must be clamped to 0.8 in demo mode"
    );

    let updated = service
        .get_capability_status(None)
        .expect("get_capability_status after process must succeed");

    assert_eq!(
        updated[0].clamp_events_count, 1,
        "demo-mode clamp must be recorded"
    );
}

/// Full → Demo → Kid → Full mode transitions must be correctly reflected on
/// the registered engine and in has_restricted_axes().
#[test]
fn test_capability_mode_full_demo_kid_full_round_trip() {
    let service = CapabilityService::new();
    let engine = Arc::new(AxisEngine::new_for_axis("yaw".to_string()));

    service
        .register_axis("yaw".to_string(), engine.clone())
        .expect("register_axis must succeed");

    // Initial state: Full
    assert_eq!(engine.capability_mode(), CapabilityMode::Full);
    assert!(
        !service.has_restricted_axes().expect("has_restricted_axes"),
        "no restricted axes in Full mode"
    );

    // Full → Demo
    service
        .set_demo_mode(true)
        .expect("set_demo_mode(true) must succeed");
    assert_eq!(engine.capability_mode(), CapabilityMode::Demo);
    assert!(
        service.has_restricted_axes().expect("has_restricted_axes"),
        "Demo mode must be detected as restricted"
    );

    // Demo → Kid
    service
        .set_capability_mode(CapabilityMode::Kid, None, false)
        .expect("set_capability_mode(Kid) must succeed");
    assert_eq!(engine.capability_mode(), CapabilityMode::Kid);

    // Kid → Full
    service
        .set_demo_mode(false)
        .expect("set_demo_mode(false) must succeed");
    assert_eq!(engine.capability_mode(), CapabilityMode::Full);
    assert!(
        !service.has_restricted_axes().expect("has_restricted_axes"),
        "no restricted axes after restoring Full mode"
    );
}
