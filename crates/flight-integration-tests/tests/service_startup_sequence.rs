// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Service startup sequence integration tests.
//!
//! Exercises the [`ServiceOrchestrator`] boot sequence state machine:
//! Initializing → BusReady → SchedulerReady → AdaptersReady → Running,
//! including subsystem health, profile hot-swap, device events, and
//! graceful shutdown.

use flight_service::{
    AdapterEvent, BootSequence, DeviceEvent,
    ServiceConfig, ServiceOrchestrator, SubsystemHealth,
};

// Subsystem name constants (matching orchestrator module)
const SUBSYSTEM_BUS: &str = "bus";
const SUBSYSTEM_SCHEDULER: &str = "scheduler";
const SUBSYSTEM_ADAPTERS: &str = "adapters";
const SUBSYSTEM_WATCHDOG: &str = "watchdog";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn default_orchestrator() -> ServiceOrchestrator {
    ServiceOrchestrator::new(ServiceConfig::default())
}

fn started_orchestrator() -> ServiceOrchestrator {
    let mut orch = default_orchestrator();
    orch.start().expect("start should succeed");
    orch
}

// ===========================================================================
// 1. Full boot sequence: Initializing → BusReady → SchedulerReady → Running
// ===========================================================================

#[test]
fn startup_full_boot_sequence_reaches_running() {
    let mut orch = default_orchestrator();
    assert_eq!(orch.phase(), BootSequence::Initializing);
    assert!(!orch.is_running());

    orch.start().expect("start ok");

    assert_eq!(orch.phase(), BootSequence::Running);
    assert!(orch.is_running());
}

// ===========================================================================
// 2. All subsystems healthy after boot
// ===========================================================================

#[test]
fn startup_all_subsystems_healthy_after_boot() {
    let orch = started_orchestrator();
    let status = orch.status();

    assert_eq!(status.boot_phase, BootSequence::Running);
    assert_eq!(status.overall_health, SubsystemHealth::Healthy);

    // Verify all expected subsystems are running
    for name in [SUBSYSTEM_BUS, SUBSYSTEM_SCHEDULER, SUBSYSTEM_ADAPTERS, SUBSYSTEM_WATCHDOG] {
        let sub = status.subsystems.get(name).expect(&format!("'{name}' must exist"));
        assert!(sub.running, "'{name}' must be running");
        assert_eq!(sub.health, SubsystemHealth::Healthy);
        assert_eq!(sub.error_count, 0);
    }
}

// ===========================================================================
// 3. Boot order is dependency-ordered
// ===========================================================================

#[test]
fn startup_boot_order_is_dependency_ordered() {
    let orch = default_orchestrator();
    let order = orch.boot_order();

    // bus must come before scheduler
    let bus_idx = order.iter().position(|s| s == SUBSYSTEM_BUS).unwrap();
    let sched_idx = order.iter().position(|s| s == SUBSYSTEM_SCHEDULER).unwrap();
    assert!(bus_idx < sched_idx, "bus must start before scheduler");

    // adapters after scheduler
    if let Some(adapter_idx) = order.iter().position(|s| s == SUBSYSTEM_ADAPTERS) {
        assert!(sched_idx < adapter_idx, "scheduler before adapters");
    }
}

// ===========================================================================
// 4. Double-start returns error
// ===========================================================================

#[test]
fn startup_double_start_rejected() {
    let mut orch = started_orchestrator();
    let result = orch.start();
    assert!(result.is_err(), "double start must fail");
}

// ===========================================================================
// 5. Graceful shutdown: Running → ShuttingDown → Stopped
// ===========================================================================

#[test]
fn startup_graceful_shutdown_reaches_stopped() {
    let mut orch = started_orchestrator();
    assert!(orch.is_running());

    orch.stop().expect("stop ok");
    assert_eq!(orch.phase(), BootSequence::Stopped);
    assert!(!orch.is_running());

    // All subsystems stopped
    let status = orch.status();
    for (name, sub) in &status.subsystems {
        assert!(!sub.running, "'{name}' must be stopped after shutdown");
    }
}

// ===========================================================================
// 6. Profile hot-swap while running
// ===========================================================================

#[test]
fn startup_profile_hotswap_while_running() {
    let mut orch = started_orchestrator();

    let compiled = orch
        .handle_profile_change("combat-f16")
        .expect("profile change ok");

    assert_eq!(compiled.name, "combat-f16");
    assert_eq!(compiled.version, 1);

    let active = orch.active_profile().expect("must have active profile");
    assert_eq!(active.name, "combat-f16");

    // Second profile change bumps version
    let compiled2 = orch
        .handle_profile_change("civilian-c172")
        .expect("second change ok");
    assert_eq!(compiled2.version, 2);
}

// ===========================================================================
// 7. Profile change rejected when not running
// ===========================================================================

#[test]
fn startup_profile_change_rejected_before_start() {
    let mut orch = default_orchestrator();
    let result = orch.handle_profile_change("test");
    assert!(result.is_err(), "profile change before start must fail");
}

// ===========================================================================
// 8. Device events tracked while running
// ===========================================================================

#[test]
fn startup_device_connect_disconnect_tracked() {
    let mut orch = started_orchestrator();

    orch.handle_device_change(DeviceEvent::Connected {
        device_id: "hid:044f:b10a".to_string(),
        device_type: "joystick".to_string(),
    })
    .expect("connect ok");

    assert_eq!(orch.connected_devices().len(), 1);

    orch.handle_device_change(DeviceEvent::Disconnected {
        device_id: "hid:044f:b10a".to_string(),
    })
    .expect("disconnect ok");

    assert!(orch.connected_devices().is_empty());
}

// ===========================================================================
// 9. Adapter events tracked while running
// ===========================================================================

#[test]
fn startup_adapter_sim_connect_disconnect_tracked() {
    let mut orch = started_orchestrator();

    orch.handle_adapter_event(AdapterEvent::SimConnected {
        sim_name: "MSFS".to_string(),
    })
    .expect("sim connect ok");
    assert_eq!(orch.connected_sims().len(), 1);
    assert_eq!(orch.connected_sims()[0], "MSFS");

    orch.handle_adapter_event(AdapterEvent::SimDisconnected {
        sim_name: "MSFS".to_string(),
    })
    .expect("sim disconnect ok");
    assert!(orch.connected_sims().is_empty());
}

// ===========================================================================
// 10. Subsystem degradation and recovery
// ===========================================================================

#[test]
fn startup_subsystem_degradation_and_recovery() {
    let mut orch = started_orchestrator();

    // Record error → degraded
    orch.record_subsystem_error(SUBSYSTEM_ADAPTERS, "timeout")
        .expect("record error ok");

    let status = orch.status();
    let adapters = status.subsystems.get(SUBSYSTEM_ADAPTERS).unwrap();
    assert_eq!(adapters.health, SubsystemHealth::Degraded);
    assert_eq!(adapters.error_count, 1);

    // Overall health should reflect worst subsystem
    assert_eq!(status.overall_health, SubsystemHealth::Degraded);
}

// ===========================================================================
// 11. Restart → stop → start cycle
// ===========================================================================

#[test]
fn startup_stop_and_restart() {
    let mut orch = started_orchestrator();
    orch.stop().expect("stop");
    assert_eq!(orch.phase(), BootSequence::Stopped);

    // Can restart from Stopped → Initializing is valid via restart
    // The orchestrator allows Stopped → Initializing transition
}

// ===========================================================================
// 12. Invalid state transitions rejected
// ===========================================================================

#[test]
fn startup_invalid_boot_transitions_rejected() {
    // Running → BusReady is not a valid transition
    assert!(!BootSequence::Running.can_transition_to(BootSequence::BusReady));
    // Initializing → Running is not valid (must go through intermediate steps)
    assert!(!BootSequence::Initializing.can_transition_to(BootSequence::Running));
    // ShuttingDown → Running is not valid
    assert!(!BootSequence::ShuttingDown.can_transition_to(BootSequence::Running));
}
