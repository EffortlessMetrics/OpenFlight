// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Service orchestration end-to-end integration tests.
//!
//! Proves: boot sequence → subsystems start → health check → profile apply
//!       → safe mode → degradation → graceful shutdown.
//! All tests are self-contained with no external process or hardware deps.

use flight_axis::AxisEngine;
use flight_bus::types::SimId;
use flight_bus::{AircraftId, BusPublisher, BusSnapshot};
use flight_core::profile::{AxisConfig, CapabilityMode, PROFILE_SCHEMA_VERSION, Profile};
use flight_service::startup_sequence::{StartupPhase, StartupSequence};
use flight_service::{
    AircraftAutoSwitchService, AircraftAutoSwitchServiceConfig, CapabilityService, FlightService,
    FlightServiceConfig, SafeModeConfig, SafeModeManager, ServiceState,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
    axes.insert(
        "roll".to_string(),
        AxisConfig {
            deadzone: Some(0.03),
            expo: Some(0.15),
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
// 1. Service boot and clean shutdown
// ===========================================================================

#[tokio::test]
async fn boot_service_starts_and_reaches_running_state() {
    let mut service = FlightService::new(FlightServiceConfig::default());

    tokio::time::timeout(Duration::from_secs(10), service.start())
        .await
        .expect("start within 10s")
        .expect("start succeeds");

    assert_eq!(service.get_state().await, ServiceState::Running);

    tokio::time::timeout(Duration::from_secs(10), service.shutdown())
        .await
        .expect("shutdown within 10s")
        .expect("shutdown succeeds");

    assert_eq!(service.get_state().await, ServiceState::Stopped);
}

#[tokio::test]
async fn boot_service_config_defaults_are_sane() {
    let cfg = FlightServiceConfig::default();
    assert!(!cfg.safe_mode);
    assert!(cfg.enable_health_monitoring);
    assert_eq!(cfg.tflight_poll_hz, 250);
    assert_eq!(cfg.stecs_poll_hz, 250);
}

// ===========================================================================
// 2. Health check after boot
// ===========================================================================

#[tokio::test]
async fn boot_health_report_contains_core_components() {
    let mut service = FlightService::new(FlightServiceConfig::default());

    tokio::time::timeout(Duration::from_secs(10), service.start())
        .await
        .expect("start within 10s")
        .expect("start succeeds");

    let health = service.get_health_status().await;

    for component in &["service", "axis_engine", "auto_switch", "safety"] {
        assert!(
            health.components.contains_key(*component),
            "'{component}' must be in health report"
        );
    }

    let _uptime = health.uptime_seconds;
    let _overall = health.overall;

    tokio::time::timeout(Duration::from_secs(10), service.shutdown())
        .await
        .expect("shutdown within 10s")
        .expect("shutdown succeeds");
}

// ===========================================================================
// 3. Startup sequence phase progression
// ===========================================================================

#[test]
fn boot_startup_sequence_full_phase_progression() {
    let mut seq = StartupSequence::new();
    assert_eq!(seq.phase(), StartupPhase::Idle);
    assert!(!seq.is_running());

    seq.run_preflight();
    assert_eq!(seq.phase(), StartupPhase::PreFlight);

    seq.loading_config();
    assert_eq!(seq.phase(), StartupPhase::LoadingConfig);

    seq.enumerating_devices();
    assert_eq!(seq.phase(), StartupPhase::EnumeratingDevices);

    seq.warn("No HID devices found");
    assert_eq!(seq.warnings().len(), 1);

    seq.starting_axis_engine();
    assert_eq!(seq.phase(), StartupPhase::StartingAxisEngine);

    seq.starting_adapters();
    assert_eq!(seq.phase(), StartupPhase::StartingAdapters);

    seq.running();
    assert_eq!(seq.phase(), StartupPhase::Running);
    assert!(seq.is_running());

    // Warnings persist
    assert_eq!(seq.warnings().len(), 1);
}

#[test]
fn boot_startup_sequence_multiple_warnings_accumulate() {
    let mut seq = StartupSequence::new();
    seq.run_preflight();

    seq.warn("Warning 1");
    seq.warn("Warning 2");
    seq.warn("Warning 3");

    assert_eq!(seq.warnings().len(), 3);
}

// ===========================================================================
// 4. Safe mode engagement
// ===========================================================================

#[tokio::test]
async fn boot_safe_mode_starts_in_safe_mode_state() {
    let mut cfg = FlightServiceConfig::default();
    cfg.safe_mode = true;

    let mut service = FlightService::new(cfg);

    tokio::time::timeout(Duration::from_secs(10), service.start())
        .await
        .expect("start within 10s")
        .expect("start succeeds");

    assert_eq!(service.get_state().await, ServiceState::SafeMode);

    let status = service.get_safe_mode_status().await;
    assert!(status.is_some());
    assert!(status.unwrap().active);

    tokio::time::timeout(Duration::from_secs(10), service.shutdown())
        .await
        .expect("shutdown within 10s")
        .expect("shutdown succeeds");

    assert_eq!(service.get_state().await, ServiceState::Stopped);
}

#[tokio::test]
async fn boot_safe_mode_diagnostic_bundle_populated() {
    let config = SafeModeConfig {
        axis_only: true,
        use_basic_profile: true,
        skip_power_checks: true,
        minimal_mode: true,
    };

    let mut manager = SafeModeManager::new(config);
    let status = manager.initialize().await.expect("init");

    assert!(status.active);
    assert!(!status.validation_results.is_empty());

    // Basic profile entry must exist and succeed
    let profile_result = status
        .validation_results
        .iter()
        .find(|r| r.component == "Basic Profile");
    assert!(profile_result.is_some());
    assert!(profile_result.unwrap().success);
}

#[tokio::test]
async fn boot_safe_mode_axis_engine_initializes() {
    let config = SafeModeConfig {
        axis_only: true,
        use_basic_profile: false,
        skip_power_checks: true,
        minimal_mode: true,
    };

    let mut manager = SafeModeManager::new(config);
    let status = manager.initialize().await.expect("init");

    let engine_result = status
        .validation_results
        .iter()
        .find(|r| r.component == "Axis Engine");
    assert!(engine_result.is_some());
    assert!(engine_result.unwrap().success);
}

// ===========================================================================
// 5. Profile apply through service
// ===========================================================================

#[tokio::test]
async fn boot_apply_profile_after_startup() {
    let mut service = FlightService::new(FlightServiceConfig::default());

    tokio::time::timeout(Duration::from_secs(10), service.start())
        .await
        .expect("start")
        .expect("start ok");

    let profile = test_profile();
    let result = service.apply_profile(&profile).await;
    assert!(result.is_ok(), "apply_profile: {:?}", result.err());

    tokio::time::timeout(Duration::from_secs(10), service.shutdown())
        .await
        .expect("shutdown")
        .expect("shutdown ok");
}

// ===========================================================================
// 6. Auto-switch service lifecycle
// ===========================================================================

#[tokio::test]
async fn boot_auto_switch_idle_lifecycle() {
    let mut bus = BusPublisher::new(60.0);
    let service = AircraftAutoSwitchService::new(AircraftAutoSwitchServiceConfig::default());

    service.start(&mut bus).await.expect("start");

    let metrics = service.get_metrics().await;
    assert_eq!(metrics.aircraft_switch_count, 0);

    service.stop().await.expect("stop");
}

#[tokio::test]
async fn boot_auto_switch_detects_aircraft_via_bus() {
    let mut bus = BusPublisher::new(60.0);
    let service = AircraftAutoSwitchService::new(AircraftAutoSwitchServiceConfig::default());

    service.start(&mut bus).await.expect("start");

    let snapshot = BusSnapshot::new(SimId::XPlane, AircraftId::new("C172"));
    bus.publish(snapshot).expect("publish");

    tokio::time::sleep(Duration::from_millis(300)).await;

    let metrics = service.get_metrics().await;
    assert!(
        metrics.aircraft_switch_count >= 1,
        "got {}",
        metrics.aircraft_switch_count
    );

    service.stop().await.expect("stop");
}

#[tokio::test]
async fn boot_auto_switch_empty_icao_ignored() {
    let mut bus = BusPublisher::new(60.0);
    let service = AircraftAutoSwitchService::new(AircraftAutoSwitchServiceConfig::default());

    service.start(&mut bus).await.expect("start");

    let empty_snap = BusSnapshot::new(SimId::Msfs, AircraftId::new(""));
    bus.publish(empty_snap).expect("publish");

    tokio::time::sleep(Duration::from_millis(200)).await;

    let metrics = service.get_metrics().await;
    assert_eq!(
        metrics.aircraft_switch_count, 0,
        "empty ICAO must not trigger switch"
    );

    service.stop().await.expect("stop");
}

// ===========================================================================
// 7. Capability service modes
// ===========================================================================

#[test]
fn boot_capability_service_clamp_starts_at_zero() {
    let service = CapabilityService::new();
    let engine = Arc::new(AxisEngine::new_for_axis("pitch".to_string()));

    service
        .register_axis("pitch".to_string(), engine)
        .expect("register");

    let status = service.get_capability_status(None).expect("status");
    assert_eq!(status.len(), 1);
    assert_eq!(status[0].clamp_events_count, 0);
}

#[test]
fn boot_capability_mode_transitions() {
    let service = CapabilityService::new();
    let engine = Arc::new(AxisEngine::new_for_axis("yaw".to_string()));

    service
        .register_axis("yaw".to_string(), engine.clone())
        .expect("register");

    // Full → Demo
    assert_eq!(engine.capability_mode(), CapabilityMode::Full);
    service.set_demo_mode(true).expect("demo");
    assert_eq!(engine.capability_mode(), CapabilityMode::Demo);

    // Demo → Kid
    service
        .set_capability_mode(CapabilityMode::Kid, None, false)
        .expect("kid");
    assert_eq!(engine.capability_mode(), CapabilityMode::Kid);

    // Kid → Full
    service.set_demo_mode(false).expect("full");
    assert_eq!(engine.capability_mode(), CapabilityMode::Full);
}

// ===========================================================================
// 8. Full integration: boot → bus → auto-switch → profile → shutdown
// ===========================================================================

#[tokio::test]
async fn boot_full_integration_lifecycle() {
    // 1. Start service
    let mut service = FlightService::new(FlightServiceConfig::default());
    tokio::time::timeout(Duration::from_secs(10), service.start())
        .await
        .expect("start timeout")
        .expect("start ok");
    assert_eq!(service.get_state().await, ServiceState::Running);

    // 2. Health check
    let health = service.get_health_status().await;
    assert!(health.components.contains_key("service"));

    // 3. Auto-switch with bus
    let mut bus = BusPublisher::new(60.0);
    let auto_switch = AircraftAutoSwitchService::new(AircraftAutoSwitchServiceConfig::default());
    auto_switch
        .start(&mut bus)
        .await
        .expect("auto-switch start");

    let snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("A320"));
    bus.publish(snap).expect("publish");
    tokio::time::sleep(Duration::from_millis(300)).await;

    let metrics = auto_switch.get_metrics().await;
    assert!(metrics.aircraft_switch_count >= 1);

    auto_switch.stop().await.expect("auto-switch stop");

    // 4. Apply profile
    let profile = test_profile();
    service
        .apply_profile(&profile)
        .await
        .expect("apply profile");

    // 5. Shutdown
    tokio::time::timeout(Duration::from_secs(10), service.shutdown())
        .await
        .expect("shutdown timeout")
        .expect("shutdown ok");
    assert_eq!(service.get_state().await, ServiceState::Stopped);
}
