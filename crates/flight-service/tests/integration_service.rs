// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests for service lifecycle.
//!
//! Exercises: orchestrator boot → health check → shutdown, graceful drain,
//! profile hot-swap, and error recovery.

use flight_service::graceful_drain::{DrainCoordinator, DrainResult};
use flight_service::{
    BootSequence, DeviceEvent, OrchestratorError, ServiceConfig, ServiceOrchestrator,
    SubsystemHealth,
};
use std::time::Duration;

// ── helpers ────────────────────────────────────────────────────────────────

fn default_orchestrator() -> ServiceOrchestrator {
    ServiceOrchestrator::new(ServiceConfig::default())
}

// ── 1. Service startup → running → shutdown ──────────────────────────────

#[test]
fn integration_service_startup_and_shutdown() {
    let mut orch = default_orchestrator();
    assert_eq!(orch.phase(), BootSequence::Initializing);

    orch.start().expect("start ok");
    assert_eq!(orch.phase(), BootSequence::Running);
    assert!(orch.is_running());

    orch.stop().expect("stop ok");
    assert_eq!(orch.phase(), BootSequence::Stopped);
    assert!(!orch.is_running());
}

// ── 2. Health status reports correctly after start ────────────────────────

#[test]
fn integration_health_status_after_start() {
    let mut orch = default_orchestrator();
    orch.start().expect("start ok");

    let status = orch.status();
    assert_eq!(status.boot_phase, BootSequence::Running);
    assert_eq!(status.overall_health, SubsystemHealth::Healthy);

    // All subsystems should be running.
    for (name, sub) in &status.subsystems {
        assert!(sub.running, "subsystem {name} should be running");
        assert_eq!(sub.health, SubsystemHealth::Healthy);
    }
}

// ── 3. Graceful drain completes within timeout ───────────────────────────

#[test]
fn integration_graceful_drain_completes() {
    let coord = DrainCoordinator::new(Duration::from_secs(2));
    let h1 = coord.register();
    let h2 = coord.register();

    coord.start_drain();
    assert!(h1.is_draining());

    h1.mark_drained();
    h2.mark_drained();

    assert_eq!(coord.wait_for_drain(), DrainResult::Completed);
}

// ── 4. Graceful drain times out when component hangs ─────────────────────

#[test]
fn integration_graceful_drain_timeout() {
    let coord = DrainCoordinator::new(Duration::from_millis(50));
    let _h1 = coord.register(); // never drains
    let h2 = coord.register();

    coord.start_drain();
    h2.mark_drained();

    let result = coord.wait_for_drain();
    assert_eq!(
        result,
        DrainResult::TimedOut {
            completed: 1,
            total: 2
        }
    );
}

// ── 5. Config reload (profile hot-swap) while running ────────────────────

#[test]
fn integration_profile_hot_swap() {
    let mut orch = default_orchestrator();
    orch.start().expect("start ok");

    let compiled = orch
        .handle_profile_change("cessna_172")
        .expect("profile swap ok");
    assert_eq!(compiled.name, "cessna_172");
    assert_eq!(compiled.version, 1);

    // Second profile change increments version.
    let compiled2 = orch
        .handle_profile_change("boeing_737")
        .expect("profile swap ok");
    assert_eq!(compiled2.version, 2);

    assert_eq!(orch.active_profile().unwrap().name, "boeing_737");
}

// ── 6. Profile change rejected when not running ──────────────────────────

#[test]
fn integration_profile_change_rejected_when_stopped() {
    let mut orch = default_orchestrator();
    let result = orch.handle_profile_change("cessna_172");
    assert!(
        result.is_err(),
        "should reject profile change when not running"
    );
}

// ── 7. Double start returns error ────────────────────────────────────────

#[test]
fn integration_double_start_returns_error() {
    let mut orch = default_orchestrator();
    orch.start().expect("first start ok");

    let result = orch.start();
    assert!(
        matches!(result, Err(OrchestratorError::AlreadyRunning)),
        "double start should fail"
    );
}

// ── 8. Stop when not running returns error ───────────────────────────────

#[test]
fn integration_stop_when_not_running() {
    let mut orch = default_orchestrator();
    let result = orch.stop();
    assert!(result.is_err(), "stop when not running should fail");
}

// ── 9. Device connect/disconnect tracking ────────────────────────────────

#[test]
fn integration_device_tracking() {
    let mut orch = default_orchestrator();
    orch.start().expect("start ok");

    orch.handle_device_change(DeviceEvent::Connected {
        device_id: "hotas_1".to_string(),
        device_type: "joystick".to_string(),
    })
    .expect("connect ok");

    assert!(orch.connected_devices().contains_key("hotas_1"));

    orch.handle_device_change(DeviceEvent::Disconnected {
        device_id: "hotas_1".to_string(),
    })
    .expect("disconnect ok");

    assert!(!orch.connected_devices().contains_key("hotas_1"));
}

// ── 10. Subsystem error degrades health ──────────────────────────────────

#[test]
fn integration_subsystem_error_degrades_health() {
    let mut orch = default_orchestrator();
    orch.start().expect("start ok");

    orch.record_subsystem_error("bus", "simulated failure")
        .expect("record error ok");

    let status = orch.status();
    // Overall health should reflect degradation.
    let bus_status = &status.subsystems["bus"];
    assert_eq!(bus_status.health, SubsystemHealth::Degraded);
    assert_eq!(bus_status.error_count, 1);
}
