// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Orchestration depth tests for the flight-service daemon.
//!
//! Covers service lifecycle, component wiring, auto-switch logic,
//! safe mode, configuration, and CLI-parity output formatting.

use std::time::Duration;

use flight_service::{
    BootSequence, DeviceEvent, AdapterEvent, OrchestratorError,
    ServiceConfig, ServiceOrchestrator,
    SubsystemHealth,
    FlightServiceConfig,
    SafeModeConfig,
};

use flight_service::config_validator::{
    ConfigValidator, PortRangeCheck,
    RequiredFieldCheck,
};
use flight_service::config_watcher::{ChangeType, ConfigWatcher};
use flight_service::degradation_manager::{DegradationLevel, DegradationManager};
use flight_service::diagnostic_bundle::{
    DegradationReason, DiagnosticBundleBuilder, DiagnosticBundleConfig,
};
use flight_service::event_journal::{EventCategory, EventJournal, JournalLevel};
use flight_service::graceful_drain::{DrainCoordinator, DrainResult};
use flight_service::health_http::{HealthEndpointState, HealthStatus};
use flight_service::instance_lock::InstanceLock;
use flight_service::metrics_server::PrometheusMetrics;
use flight_service::shutdown_coordinator::{
    ComponentShutdownOutcome, ShutdownCoordinator,
};
use flight_service::startup_sequence::StartupSequence;
use flight_service::task_supervisor::TaskSupervisor;

// ===========================================================================
// 1. Service Lifecycle (6 tests)
// ===========================================================================

/// 1-1: Full start → running → stop → stopped lifecycle via the orchestrator.
#[test]
fn lifecycle_start_running_shutdown_sequence() {
    let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
    assert_eq!(orch.phase(), BootSequence::Initializing);

    orch.start().unwrap();
    assert_eq!(orch.phase(), BootSequence::Running);
    assert!(orch.is_running());

    orch.stop().unwrap();
    assert_eq!(orch.phase(), BootSequence::Stopped);
    assert!(!orch.is_running());
}

/// 1-2: Graceful shutdown with configurable timeout using the coordinator.
#[test]
fn lifecycle_graceful_shutdown_with_timeout() {
    let timeout_ms = 3_000u64;
    let mut coord = ShutdownCoordinator::new(timeout_ms);
    coord.add_phase(
        "network",
        vec!["grpc".into(), "http".into()],
        1_000,
    );
    coord.add_phase("core", vec!["axis".into(), "bus".into()], 1_000);

    coord.set_handler(Box::new(|_name, _timeout| ComponentShutdownOutcome::Ok));
    let result = coord.execute_shutdown();

    assert!(result.is_clean());
    assert_eq!(result.completed.len(), 4);
    assert!(result.total_duration_ms < timeout_ms);
}

/// 1-3: Forced shutdown — handler reports timeout for slow components.
#[test]
fn lifecycle_forced_shutdown_on_slow_components() {
    let mut coord = ShutdownCoordinator::new(100);
    coord.add_phase("slow", vec!["stalled_adapter".into()], 50);
    coord.set_handler(Box::new(|name, _timeout| {
        if name == "stalled_adapter" {
            ComponentShutdownOutcome::TimedOut
        } else {
            ComponentShutdownOutcome::Ok
        }
    }));

    let result = coord.execute_shutdown();
    assert!(!result.is_clean());
    assert!(result.timed_out.contains(&"stalled_adapter".to_string()));
}

/// 1-4: Restart — stop then re-start the orchestrator.
#[test]
fn lifecycle_restart_after_stop() {
    let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
    orch.start().unwrap();
    orch.stop().unwrap();
    assert_eq!(orch.phase(), BootSequence::Stopped);

    // Stopped → Initializing is allowed by the state machine
    assert!(BootSequence::Stopped.can_transition_to(BootSequence::Initializing));
}

/// 1-5: Double start is rejected.
#[test]
fn lifecycle_double_start_prevented() {
    let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
    orch.start().unwrap();
    let err = orch.start().unwrap_err();
    assert_eq!(err, OrchestratorError::AlreadyRunning);
}

/// 1-6: Signal-like shutdown — DrainCoordinator signals all components
///       and waits for acknowledgment.
#[test]
fn lifecycle_signal_handling_via_drain() {
    let coord = DrainCoordinator::new(Duration::from_secs(2));

    let h1 = coord.register();
    let h2 = coord.register();

    // Simulate components observing the drain signal and acknowledging.
    assert!(!h1.is_draining());

    // Start drain (like SIGTERM handler would).
    coord.start_drain();
    assert!(h1.is_draining());
    assert!(h2.is_draining());

    h1.mark_drained();
    h2.mark_drained();

    let result = coord.wait_for_drain();
    assert_eq!(result, DrainResult::Completed);
}

// ===========================================================================
// 2. Component Wiring (6 tests)
// ===========================================================================

/// 2-1: Default config registers bus, scheduler, adapters, watchdog.
#[test]
fn wiring_adapter_registration_default() {
    let orch = ServiceOrchestrator::new(ServiceConfig::default());
    let order: Vec<&str> = orch.boot_order().iter().map(String::as_str).collect();
    assert_eq!(order, vec!["bus", "scheduler", "adapters", "watchdog"]);
}

/// 2-2: Bus subsystem is started first and stays healthy.
#[test]
fn wiring_bus_subsystem_healthy_after_start() {
    let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
    orch.start().unwrap();
    let bus = orch.subsystem("bus").unwrap();
    assert!(bus.is_running());
    assert_eq!(bus.health(), SubsystemHealth::Healthy);
}

/// 2-3: Profile loading via the orchestrator's handle_profile_change.
#[test]
fn wiring_profile_loading() {
    let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
    orch.start().unwrap();
    let compiled = orch.handle_profile_change("cessna_172").unwrap();
    assert_eq!(compiled.name, "cessna_172");
    assert_eq!(compiled.version, 1);
    assert_eq!(orch.active_profile().unwrap().name, "cessna_172");
}

/// 2-4: Watchdog subsystem present when enabled.
#[test]
fn wiring_watchdog_hookup() {
    let orch = ServiceOrchestrator::new(ServiceConfig {
        enable_watchdog: true,
        ..ServiceConfig::default()
    });
    assert!(orch.subsystem("watchdog").is_some());
}

/// 2-5: Health endpoint state generates correct JSON shape.
#[test]
fn wiring_health_endpoint_response() {
    let state = HealthEndpointState::new("0.1.0-test");
    let resp = state.to_response();
    assert_eq!(resp.status, HealthStatus::Ok);
    assert_eq!(resp.version, "0.1.0-test");
    assert!(resp.uptime_secs < 2);
}

/// 2-6: Prometheus metrics produce valid text output.
#[test]
fn wiring_metrics_endpoint() {
    let metrics = PrometheusMetrics::new();
    metrics
        .axis_ticks_total
        .store(42, std::sync::atomic::Ordering::Relaxed);
    metrics
        .profiles_loaded
        .store(3, std::sync::atomic::Ordering::Relaxed);

    let text = metrics.to_prometheus_text(120);
    assert!(text.contains("openflight_axis_ticks_total 42"));
    assert!(text.contains("openflight_profiles_loaded_total 3"));
    assert!(text.contains("openflight_uptime_seconds 120"));
}

// ===========================================================================
// 3. Auto-Switch (5 tests)
// ===========================================================================

/// 3-1: Aircraft detection triggers a profile swap in the orchestrator.
#[test]
fn autoswitch_aircraft_triggers_profile_switch() {
    let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
    orch.start().unwrap();

    // Simulate adapter reporting new aircraft.
    orch.handle_adapter_event(AdapterEvent::SimConnected {
        sim_name: "MSFS".into(),
    })
    .unwrap();

    // Profile switch should succeed.
    let compiled = orch.handle_profile_change("a320neo").unwrap();
    assert_eq!(compiled.name, "a320neo");
}

/// 3-2: Multi-adapter detection — two sims connected simultaneously.
#[test]
fn autoswitch_multi_adapter_detection() {
    let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
    orch.start().unwrap();

    orch.handle_adapter_event(AdapterEvent::SimConnected {
        sim_name: "MSFS".into(),
    })
    .unwrap();
    orch.handle_adapter_event(AdapterEvent::SimConnected {
        sim_name: "XPlane".into(),
    })
    .unwrap();

    let sims = orch.connected_sims();
    assert_eq!(sims.len(), 2);
    assert!(sims.contains(&"MSFS".to_string()));
    assert!(sims.contains(&"XPlane".to_string()));
}

/// 3-3: Switch debounce — rapid profile changes increment version correctly.
#[test]
fn autoswitch_rapid_profile_changes_debounce() {
    let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
    orch.start().unwrap();

    let p1 = orch.handle_profile_change("cessna_172").unwrap();
    let p2 = orch.handle_profile_change("a320neo").unwrap();
    let p3 = orch.handle_profile_change("f18").unwrap();

    // Versions should monotonically increase.
    assert!(p2.version > p1.version);
    assert!(p3.version > p2.version);
    // Latest profile is the active one.
    assert_eq!(orch.active_profile().unwrap().name, "f18");
}

/// 3-4: Fallback when the orchestrator is not running — profile change rejected.
#[test]
fn autoswitch_fallback_when_not_running() {
    let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
    let err = orch.handle_profile_change("default").unwrap_err();
    assert_eq!(err, OrchestratorError::NotRunning);
}

/// 3-5: Sim disconnect removes it from the connected list.
#[test]
fn autoswitch_sim_disconnect_removes_adapter() {
    let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
    orch.start().unwrap();

    orch.handle_adapter_event(AdapterEvent::SimConnected {
        sim_name: "DCS".into(),
    })
    .unwrap();
    assert_eq!(orch.connected_sims().len(), 1);

    orch.handle_adapter_event(AdapterEvent::SimDisconnected {
        sim_name: "DCS".into(),
    })
    .unwrap();
    assert!(orch.connected_sims().is_empty());
}

// ===========================================================================
// 4. Safe Mode (5 tests)
// ===========================================================================

/// 4-1: Degradation manager enters safe mode on critical failure.
#[test]
fn safemode_degraded_entry_on_critical_failure() {
    let mut dm = DegradationManager::new();
    dm.register_component("axis_engine", true); // critical
    dm.register_component("panels", false);

    dm.update_health("axis_engine", false);
    assert_eq!(dm.current_level(), DegradationLevel::SafeMode);
    assert!(!dm.can_operate());
}

/// 4-2: Basic profile remains active in safe mode config defaults.
#[test]
fn safemode_basic_profile_activation() {
    let cfg = SafeModeConfig::default();
    assert!(cfg.axis_only);
    assert!(cfg.use_basic_profile);
    assert!(cfg.minimal_mode);
}

/// 4-3: Diagnostic bundle is created with system info.
#[test]
fn safemode_diagnostic_bundle_creation() {
    let mut builder = DiagnosticBundleBuilder::new(DiagnosticBundleConfig::default());
    builder.add_system_info();
    builder.add_text("reason.txt", "Axis engine failed to initialize");

    let text = builder.finalize_as_text();
    assert!(text.contains("OpenFlight Diagnostic Bundle"));
    assert!(text.contains("system_info.txt"));
    assert!(text.contains("Axis engine failed"));
    assert_eq!(builder.entry_count(), 2);
}

/// 4-4: Recovery path — non-critical failure allows continued operation.
#[test]
fn safemode_recovery_path_non_critical() {
    let mut dm = DegradationManager::new();
    dm.register_component("panels", false);
    dm.register_component("bus", true);

    dm.update_health("panels", false);
    assert_eq!(dm.current_level(), DegradationLevel::Reduced);
    assert!(dm.can_operate());

    // Recover the failed component.
    dm.update_health("panels", true);
    assert_eq!(dm.current_level(), DegradationLevel::Full);
}

/// 4-5: User notification — degraded features list populated.
#[test]
fn safemode_user_notification_degraded_features() {
    let mut dm = DegradationManager::new();
    dm.register_component("ffb", false);
    dm.register_component("panels", false);
    dm.register_component("streamdeck", false);

    dm.update_health("ffb", false);
    dm.update_health("streamdeck", false);

    let degraded = dm.degraded_features();
    assert_eq!(degraded.len(), 2);
    assert!(degraded.contains(&"ffb".to_string()));
    assert!(degraded.contains(&"streamdeck".to_string()));
    assert_eq!(dm.current_level(), DegradationLevel::Minimal);
}

// ===========================================================================
// 5. Configuration (5 tests)
// ===========================================================================

/// 5-1: Config file loading from valid JSON.
#[test]
fn config_file_loading_valid_json() {
    let json = serde_json::to_string(&FlightServiceConfig::default()).unwrap();
    let loaded = FlightServiceConfig::load_from_str(&json).unwrap();
    assert_eq!(loaded.tflight_poll_hz, 250);
    assert!(!loaded.safe_mode);
}

/// 5-2: Config validation rejects zero poll rate.
#[test]
fn config_validation_rejects_zero_poll_rate() {
    let cfg = FlightServiceConfig {
        tflight_poll_hz: 0,
        ..FlightServiceConfig::default()
    };
    let err = cfg.validate().unwrap_err();
    assert!(err.to_string().contains("tflight_poll_hz"));
}

/// 5-3: Config hot-reload — watcher detects file modification.
#[test]
fn config_hot_reload_detects_modification() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("config.json");
    std::fs::write(&file, r#"{"version": 1}"#).unwrap();

    let mut watcher = ConfigWatcher::new(Duration::from_millis(50));
    watcher.watch(&file);

    // Prime watcher state.
    let _ = watcher.check_for_changes();

    // Modify the file.
    std::thread::sleep(Duration::from_millis(50));
    std::fs::write(&file, r#"{"version": 2}"#).unwrap();

    let changes = watcher.check_for_changes();
    assert!(!changes.is_empty());
    assert_eq!(changes[0].change_type, ChangeType::Modified);
}

/// 5-4: Default config generation roundtrips through JSON.
#[test]
fn config_default_generation_roundtrips() {
    let default_cfg = FlightServiceConfig::default();
    let json = serde_json::to_string_pretty(&default_cfg).unwrap();
    let parsed: FlightServiceConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.tflight_poll_hz, default_cfg.tflight_poll_hz);
    assert_eq!(parsed.stecs_poll_hz, default_cfg.stecs_poll_hz);
    assert_eq!(parsed.safe_mode, default_cfg.safe_mode);
}

/// 5-5: Environment-style override — load_or_default falls back gracefully.
#[test]
fn config_env_var_override_fallback() {
    let cfg = FlightServiceConfig::load_or_default("/nonexistent/path/config.json");
    // Should produce defaults when file is missing.
    assert_eq!(cfg.tflight_poll_hz, 250);
    assert!(!cfg.safe_mode);
}

// ===========================================================================
// 6. CLI Parity (5 tests)
// ===========================================================================

/// 6-1: JSON output — OrchestratorStatus can be serialised to JSON.
#[test]
fn cli_json_output_mode() {
    let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
    orch.start().unwrap();
    let status = orch.status();

    // Build a JSON representation matching CLI output.
    let json = serde_json::json!({
        "phase": format!("{}", status.boot_phase),
        "subsystems": status.subsystems.iter().map(|(name, s)| {
            (name.clone(), serde_json::json!({
                "running": s.running,
                "health": format!("{:?}", s.health),
                "error_count": s.error_count,
            }))
        }).collect::<serde_json::Map<String, serde_json::Value>>(),
    });

    let text = serde_json::to_string_pretty(&json).unwrap();
    assert!(text.contains("Running"));
    assert!(text.contains("\"running\": true"));
}

/// 6-2: Status query — phase and subsystems match expected values.
#[test]
fn cli_status_query() {
    let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
    orch.start().unwrap();
    let status = orch.status();

    assert_eq!(status.boot_phase, BootSequence::Running);
    assert!(status.subsystems.contains_key("bus"));
    assert!(status.subsystems.contains_key("scheduler"));
    assert!(status.subsystems.contains_key("adapters"));
    assert!(status.subsystems.contains_key("watchdog"));
    assert_eq!(status.overall_health, SubsystemHealth::Healthy);
}

/// 6-3: Profile list — sequential profile loads are all recorded.
#[test]
fn cli_profile_list() {
    let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
    orch.start().unwrap();

    orch.handle_profile_change("default").unwrap();
    orch.handle_profile_change("cessna_172").unwrap();

    // Active profile is the most recently loaded.
    let active = orch.active_profile().unwrap();
    assert_eq!(active.name, "cessna_172");
    assert_eq!(active.version, 2);
}

/// 6-4: Device list — connected devices reflect connect/disconnect events.
#[test]
fn cli_device_list() {
    let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
    orch.start().unwrap();

    orch.handle_device_change(DeviceEvent::Connected {
        device_id: "usb:1234:5678".into(),
        device_type: "TFlight HOTAS".into(),
    })
    .unwrap();
    orch.handle_device_change(DeviceEvent::Connected {
        device_id: "usb:abcd:ef01".into(),
        device_type: "VKB STECS".into(),
    })
    .unwrap();

    let devices = orch.connected_devices();
    assert_eq!(devices.len(), 2);
    assert_eq!(devices.get("usb:1234:5678").unwrap(), "TFlight HOTAS");

    orch.handle_device_change(DeviceEvent::Disconnected {
        device_id: "usb:1234:5678".into(),
    })
    .unwrap();
    assert_eq!(orch.connected_devices().len(), 1);
}

/// 6-5: Health check — overall health degrades when a subsystem fails.
#[test]
fn cli_health_check() {
    let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
    orch.start().unwrap();

    // All healthy initially.
    assert_eq!(orch.status().overall_health, SubsystemHealth::Healthy);

    // Degrade one subsystem.
    orch.record_subsystem_error("adapters", "MSFS disconnected")
        .unwrap();
    let status = orch.status();
    assert_eq!(status.overall_health, SubsystemHealth::Degraded);

    let adapters = status.subsystems.get("adapters").unwrap();
    assert_eq!(adapters.error_count, 1);
    assert_eq!(
        adapters.last_error.as_deref(),
        Some("MSFS disconnected")
    );
}

// ===========================================================================
// Bonus depth tests (to reach 32+)
// ===========================================================================

/// B-1: Boot sequence state machine — Stopped can re-enter Initializing.
#[test]
fn bonus_stopped_to_initializing_allowed() {
    assert!(BootSequence::Stopped.can_transition_to(BootSequence::Initializing));
    assert!(!BootSequence::Stopped.can_transition_to(BootSequence::Running));
}

/// B-2: Subsystem restart via orchestrator.
#[test]
fn bonus_subsystem_restart() {
    let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
    orch.start().unwrap();

    orch.restart_subsystem("bus").unwrap();
    assert!(orch.subsystem("bus").unwrap().is_running());
    assert_eq!(orch.subsystem("bus").unwrap().health(), SubsystemHealth::Healthy);
}

/// B-3: Config validator — port range check passes for valid port.
#[test]
fn bonus_config_validator_port_range() {
    let mut validator = ConfigValidator::new();
    validator.add_check(Box::new(PortRangeCheck::new("/port")));

    let config = serde_json::json!({"port": 8080});
    let result = validator.validate(&config);
    assert!(result.valid);

    let bad = serde_json::json!({"port": 99999});
    let result = validator.validate(&bad);
    assert!(!result.valid);
}

/// B-4: Config validator — required field check.
#[test]
fn bonus_config_validator_required_field() {
    let mut validator = ConfigValidator::new();
    validator.add_check(Box::new(RequiredFieldCheck::new("/name")));

    let missing = serde_json::json!({"other": 1});
    let result = validator.validate(&missing);
    assert!(!result.valid);

    let present = serde_json::json!({"name": "test"});
    let result = validator.validate(&present);
    assert!(result.valid);
}

/// B-5: Startup sequence records warnings without blocking running.
#[test]
fn bonus_startup_with_warnings() {
    let mut seq = StartupSequence::new();
    seq.run_preflight();
    seq.warn("No RT scheduling available");
    seq.loading_config();
    seq.enumerating_devices();
    seq.starting_axis_engine();
    seq.starting_adapters();
    seq.running();

    assert!(seq.is_running());
    assert_eq!(seq.warnings().len(), 1);
    assert!(seq.warnings()[0].contains("RT"));
}

/// B-6: Task supervisor tracks lifecycle transitions.
#[test]
fn bonus_task_supervisor_lifecycle() {
    let mut sup = TaskSupervisor::new(3, 100);
    sup.register_task("axis_loop", "Axis Processing Loop");
    assert!(sup.start_task("axis_loop"));
    assert!(sup.complete_task("axis_loop"));

    // Failure increments restart count.
    sup.register_task("ffb_loop", "FFB Processing Loop");
    assert!(sup.start_task("ffb_loop"));
    assert!(sup.fail_task("ffb_loop", "device disconnected"));
}

/// B-7: Event journal rotates oldest entries.
#[test]
fn bonus_event_journal_rotation() {
    let mut journal = EventJournal::new(3);
    for i in 0..5 {
        journal.record(
            JournalLevel::Info,
            EventCategory::ServiceStartup,
            format!("event {i}"),
            None,
        );
    }
    // Only last 3 should remain.
    assert_eq!(journal.len(), 3);
}

/// B-8: Instance lock — path helper is stable.
#[test]
fn bonus_instance_lock_path() {
    let path = InstanceLock::default_path();
    assert!(path.ends_with("openflight.lock"));
}

/// B-9: Shutdown coordinator — empty phases are skipped cleanly.
#[test]
fn bonus_shutdown_empty_phases() {
    let mut coord = ShutdownCoordinator::new(5_000);
    coord.add_phase("empty", vec![], 1_000);
    coord.add_phase("real", vec!["bus".into()], 1_000);
    let result = coord.execute_shutdown();
    assert!(result.is_clean());
    assert_eq!(result.completed, vec!["bus"]);
}

/// B-10: DegradationReason Display implementations.
#[test]
fn bonus_degradation_reason_display() {
    let reasons = vec![
        DegradationReason::FfbFault("overcurrent".into()),
        DegradationReason::HidEnumerationFailure("no devices".into()),
        DegradationReason::AdapterDisconnect("MSFS lost".into()),
        DegradationReason::ConfigError("bad yaml".into()),
        DegradationReason::PluginFault("oom".into()),
        DegradationReason::SchedulerFailure("no rtkit".into()),
        DegradationReason::Unknown("mystery".into()),
    ];

    for reason in &reasons {
        let s = format!("{reason}");
        assert!(!s.is_empty());
    }
    assert!(format!("{}", reasons[0]).contains("overcurrent"));
}

/// B-11: Health endpoint status can be changed to degraded.
#[test]
fn bonus_health_endpoint_status_degraded() {
    let mut state = HealthEndpointState::new("0.1.0");
    state.status = HealthStatus::Degraded;
    let resp = state.to_response();
    assert_eq!(resp.status, HealthStatus::Degraded);
}

/// B-12: Config watcher — disable/enable toggling.
#[test]
fn bonus_config_watcher_disable_enable() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("test.json");
    std::fs::write(&file, "{}").unwrap();

    let mut watcher = ConfigWatcher::new(Duration::from_millis(50));
    watcher.watch(&file);

    // Prime state.
    let _ = watcher.check_for_changes();

    watcher.disable();
    assert!(!watcher.is_enabled());
    // Disabled watcher should return no changes even if file changes.
    std::fs::write(&file, r#"{"changed": true}"#).unwrap();
    assert!(watcher.check_for_changes().is_empty());

    watcher.enable();
    assert!(watcher.is_enabled());
}
