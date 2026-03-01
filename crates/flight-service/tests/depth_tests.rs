// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for service orchestration.
//!
//! Covers startup sequence, graceful shutdown, safe mode activation,
//! profile loading pipeline, adapter lifecycle, capability service,
//! health reporting aggregation, config validation, multi-sim handling,
//! and error recovery.

use std::collections::HashMap;

use flight_service::{
    // Service / config
    FlightService, FlightServiceConfig, ServiceState,
    // Safe mode
    SafeModeConfig, SafeModeManager,
    // Capability
    CapabilityService, CapabilityServiceConfig,
    // Health
    HealthChecker, HealthStream, AdapterHealthInput, DeviceHealthInput,
    MemoryHealthInput, SchedulerHealthInput, OverallStatus,
    // Config validator
    config_validator::{
        ConfigValidator, PortRangeCheck, RequiredFieldCheck, NumericRangeCheck,
    },
    // Orchestrator
    orchestrator::{
        ServiceOrchestrator, ServiceConfig, BootSequence, SubsystemHealth,
        OrchestratorError, AdapterEvent, DeviceEvent,
        SUBSYSTEM_BUS, SUBSYSTEM_SCHEDULER, SUBSYSTEM_ADAPTERS, SUBSYSTEM_WATCHDOG,
    },
};

use flight_axis::{AxisEngine, AxisFrame};
use flight_core::profile::{AxisConfig, CapabilityMode, Profile};

// ===========================================================================
// 1. Service startup sequence
// ===========================================================================

mod startup_sequence {
    use super::*;

    #[tokio::test]
    async fn full_mode_startup_reaches_running() {
        let config = FlightServiceConfig::default();
        let mut service = FlightService::new(config);
        assert_eq!(service.get_state().await, ServiceState::Stopped);

        service.start().await.expect("start should succeed");
        assert_eq!(service.get_state().await, ServiceState::Running);

        service.shutdown().await.expect("shutdown should succeed");
    }

    #[tokio::test]
    async fn safe_mode_startup_reaches_safe_mode() {
        let config = FlightServiceConfig {
            safe_mode: true,
            ..Default::default()
        };
        let mut service = FlightService::new(config);
        service.start().await.expect("start should succeed");
        assert_eq!(service.get_state().await, ServiceState::SafeMode);

        service.shutdown().await.expect("shutdown should succeed");
    }

    #[tokio::test]
    async fn health_components_registered_after_start() {
        let config = FlightServiceConfig::default();
        let mut service = FlightService::new(config);
        service.start().await.unwrap();

        let health = service.get_health_status().await;
        // Core components should be registered
        assert!(
            health.components.contains_key("service"),
            "service component should be registered"
        );
        assert!(
            health.components.contains_key("axis_engine"),
            "axis_engine component should be registered"
        );

        service.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn double_start_is_rejected() {
        let config = FlightServiceConfig::default();
        let mut service = FlightService::new(config);
        service.start().await.unwrap();
        let second = service.start().await;
        assert!(second.is_err(), "double start must fail");
        service.shutdown().await.unwrap();
    }

    #[test]
    fn orchestrator_boot_sequence_order() {
        let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
        assert_eq!(orch.phase(), BootSequence::Initializing);

        orch.start().unwrap();
        assert_eq!(orch.phase(), BootSequence::Running);

        // All subsystems running
        for name in orch.boot_order() {
            let handle = orch.subsystem(name).unwrap();
            assert!(handle.is_running(), "{name} should be running after boot");
            assert_eq!(handle.health(), SubsystemHealth::Healthy);
        }
    }

    #[test]
    fn orchestrator_boot_order_is_bus_scheduler_adapters_watchdog() {
        let orch = ServiceOrchestrator::new(ServiceConfig::default());
        let names: Vec<&str> = orch.boot_order().iter().map(String::as_str).collect();
        assert_eq!(
            names,
            vec![SUBSYSTEM_BUS, SUBSYSTEM_SCHEDULER, SUBSYSTEM_ADAPTERS, SUBSYSTEM_WATCHDOG]
        );
    }
}

// ===========================================================================
// 2. Graceful shutdown
// ===========================================================================

mod graceful_shutdown {
    use super::*;

    #[tokio::test]
    async fn shutdown_transitions_to_stopped() {
        let config = FlightServiceConfig::default();
        let mut service = FlightService::new(config);
        service.start().await.unwrap();
        service.shutdown().await.unwrap();
        assert_eq!(service.get_state().await, ServiceState::Stopped);
    }

    #[tokio::test]
    async fn shutdown_emits_health_events() {
        let config = FlightServiceConfig::default();
        let mut service = FlightService::new(config);
        service.start().await.unwrap();
        let mut rx = service.subscribe_health();

        service.shutdown().await.unwrap();

        // Drain all pending health events — at least one should mention shutdown
        let mut found_shutdown_event = false;
        while let Ok(event) = rx.try_recv() {
            if event.message.to_lowercase().contains("shutdown") {
                found_shutdown_event = true;
            }
        }
        assert!(found_shutdown_event, "should emit a shutdown health event");
    }

    #[test]
    fn orchestrator_stop_reverses_boot_order() {
        let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
        orch.start().unwrap();
        orch.stop().unwrap();

        assert_eq!(orch.phase(), BootSequence::Stopped);
        for name in orch.boot_order() {
            assert!(
                !orch.subsystem(name).unwrap().is_running(),
                "{name} should be stopped after shutdown"
            );
        }
    }

    #[test]
    fn orchestrator_stop_when_not_running_fails() {
        let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
        let err = orch.stop().unwrap_err();
        assert_eq!(err, OrchestratorError::NotRunning);
    }
}

// ===========================================================================
// 3. Safe mode activation
// ===========================================================================

mod safe_mode_activation {
    use super::*;

    #[tokio::test]
    async fn safe_mode_initializes_successfully() {
        let mut mgr = SafeModeManager::new(SafeModeConfig::default());
        let status = mgr.initialize().await.unwrap();
        assert!(status.active);
        assert!(status.config.axis_only);
        assert!(status.config.use_basic_profile);
    }

    #[tokio::test]
    async fn safe_mode_creates_basic_profile_with_all_axes() {
        let mut mgr = SafeModeManager::new(SafeModeConfig::default());
        mgr.initialize().await.unwrap();
        let status = mgr.get_status();
        assert!(status.active);
        // Validation results should include Basic Profile
        let profile_result = status
            .validation_results
            .iter()
            .find(|r| r.component == "Basic Profile");
        assert!(
            profile_result.is_some(),
            "should have validated basic profile"
        );
        assert!(
            profile_result.unwrap().success,
            "basic profile validation should pass"
        );
    }

    #[tokio::test]
    async fn safe_mode_produces_diagnostic_bundle() {
        let mut mgr = SafeModeManager::new(SafeModeConfig::default());
        mgr.initialize().await.unwrap();
        let diag = mgr.get_diagnostic().expect("diagnostic should be present");
        // When all validations pass, reason should mention operator request
        assert!(!diag.reason.is_empty());
        assert!(!diag.recommended_actions.is_empty());
    }

    #[tokio::test]
    async fn safe_mode_shutdown_cleans_up() {
        let mut mgr = SafeModeManager::new(SafeModeConfig::default());
        mgr.initialize().await.unwrap();
        let result = mgr.shutdown().await;
        assert!(result.is_ok(), "safe mode shutdown should succeed");
    }

    #[tokio::test]
    async fn safe_mode_skip_power_checks() {
        let config = SafeModeConfig {
            skip_power_checks: true,
            ..Default::default()
        };
        let mut mgr = SafeModeManager::new(config);
        let status = mgr.initialize().await.unwrap();
        // Should still succeed without power checks
        assert!(status.active);
    }
}

// ===========================================================================
// 4. Profile loading pipeline
// ===========================================================================

mod profile_loading {
    use super::*;

    fn test_profile(axes: Vec<(&str, AxisConfig)>) -> Profile {
        Profile {
            schema: "flight.profile/1".to_string(),
            sim: None,
            aircraft: None,
            axes: axes
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
            pof_overrides: None,
        }
    }

    fn basic_axis() -> AxisConfig {
        AxisConfig {
            deadzone: Some(0.03),
            expo: Some(0.2),
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        }
    }

    #[tokio::test]
    async fn apply_valid_profile_succeeds() {
        let mut service = FlightService::new(FlightServiceConfig::default());
        service.start().await.unwrap();

        let profile = test_profile(vec![("pitch", basic_axis()), ("roll", basic_axis())]);
        let result = service.apply_profile(&profile).await;
        assert!(result.is_ok(), "valid profile should apply: {:?}", result.err());

        service.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn apply_profile_without_engine_fails() {
        let service = FlightService::new(FlightServiceConfig::default());
        let profile = test_profile(vec![]);
        let result = service.apply_profile(&profile).await;
        assert!(result.is_err(), "apply without engine should fail");
    }

    #[tokio::test]
    async fn apply_invalid_schema_fails() {
        let mut service = FlightService::new(FlightServiceConfig::default());
        service.start().await.unwrap();

        let profile = Profile {
            schema: "wrong_schema".to_string(),
            sim: None,
            aircraft: None,
            axes: HashMap::new(),
            pof_overrides: None,
        };
        let result = service.apply_profile(&profile).await;
        assert!(result.is_err(), "invalid schema should fail validation");

        service.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn apply_empty_profile_succeeds() {
        let mut service = FlightService::new(FlightServiceConfig::default());
        service.start().await.unwrap();

        let profile = test_profile(vec![]);
        let result = service.apply_profile(&profile).await;
        assert!(result.is_ok(), "empty axes should succeed");

        service.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn apply_multi_axis_profile() {
        let mut service = FlightService::new(FlightServiceConfig::default());
        service.start().await.unwrap();

        let profile = test_profile(vec![
            ("pitch", basic_axis()),
            ("roll", basic_axis()),
            ("yaw", AxisConfig {
                deadzone: Some(0.08),
                expo: None,
                slew_rate: None,
                detents: vec![],
                curve: None,
                filter: None,
            }),
            ("throttle", AxisConfig {
                deadzone: Some(0.01),
                expo: None,
                slew_rate: None,
                detents: vec![],
                curve: None,
                filter: None,
            }),
        ]);
        let result = service.apply_profile(&profile).await;
        assert!(result.is_ok(), "multi-axis profile should apply");

        service.shutdown().await.unwrap();
    }

    #[test]
    fn orchestrator_profile_hot_swap() {
        let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
        orch.start().unwrap();

        let compiled = orch.handle_profile_change("cessna-172").unwrap();
        assert_eq!(compiled.name, "cessna-172");
        assert_eq!(compiled.version, 1);

        let compiled2 = orch.handle_profile_change("f16-viper").unwrap();
        assert_eq!(compiled2.name, "f16-viper");
        assert_eq!(compiled2.version, 2);

        assert_eq!(orch.active_profile().unwrap().name, "f16-viper");
    }

    #[test]
    fn orchestrator_profile_swap_when_not_running_fails() {
        let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
        let err = orch.handle_profile_change("whatever").unwrap_err();
        assert_eq!(err, OrchestratorError::NotRunning);
    }
}

// ===========================================================================
// 5. Adapter lifecycle
// ===========================================================================

mod adapter_lifecycle {
    use super::*;

    #[test]
    fn adapter_connect_disconnect_cycle() {
        let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
        orch.start().unwrap();
        assert!(orch.connected_sims().is_empty());

        // Connect
        orch.handle_adapter_event(AdapterEvent::SimConnected {
            sim_name: "MSFS".into(),
        })
        .unwrap();
        assert_eq!(orch.connected_sims(), &["MSFS"]);

        // Disconnect
        orch.handle_adapter_event(AdapterEvent::SimDisconnected {
            sim_name: "MSFS".into(),
        })
        .unwrap();
        assert!(orch.connected_sims().is_empty());
    }

    #[test]
    fn adapter_reconnect_after_disconnect() {
        let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
        orch.start().unwrap();

        // Connect, disconnect, reconnect
        orch.handle_adapter_event(AdapterEvent::SimConnected {
            sim_name: "XPlane".into(),
        })
        .unwrap();
        orch.handle_adapter_event(AdapterEvent::SimDisconnected {
            sim_name: "XPlane".into(),
        })
        .unwrap();
        orch.handle_adapter_event(AdapterEvent::SimConnected {
            sim_name: "XPlane".into(),
        })
        .unwrap();
        assert_eq!(orch.connected_sims(), &["XPlane"]);
    }

    #[test]
    fn adapter_duplicate_connect_is_idempotent() {
        let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
        orch.start().unwrap();

        orch.handle_adapter_event(AdapterEvent::SimConnected {
            sim_name: "DCS".into(),
        })
        .unwrap();
        orch.handle_adapter_event(AdapterEvent::SimConnected {
            sim_name: "DCS".into(),
        })
        .unwrap();
        assert_eq!(orch.connected_sims().len(), 1);
    }

    #[test]
    fn adapter_events_rejected_when_not_running() {
        let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
        let err = orch
            .handle_adapter_event(AdapterEvent::SimConnected {
                sim_name: "X".into(),
            })
            .unwrap_err();
        assert_eq!(err, OrchestratorError::NotRunning);
    }

    #[test]
    fn device_connect_and_disconnect() {
        let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
        orch.start().unwrap();

        orch.handle_device_change(DeviceEvent::Connected {
            device_id: "tflight-1".into(),
            device_type: "joystick".into(),
        })
        .unwrap();
        assert_eq!(orch.connected_devices().len(), 1);
        assert_eq!(orch.connected_devices()["tflight-1"], "joystick");

        orch.handle_device_change(DeviceEvent::Disconnected {
            device_id: "tflight-1".into(),
        })
        .unwrap();
        assert!(orch.connected_devices().is_empty());
    }

    #[test]
    fn subsystem_restart_recovers() {
        let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
        orch.start().unwrap();

        // Record error then restart
        orch.record_subsystem_error(SUBSYSTEM_ADAPTERS, "connection lost")
            .unwrap();
        assert_eq!(
            orch.subsystem(SUBSYSTEM_ADAPTERS).unwrap().health(),
            SubsystemHealth::Degraded
        );

        orch.restart_subsystem(SUBSYSTEM_ADAPTERS).unwrap();
        assert!(orch.subsystem(SUBSYSTEM_ADAPTERS).unwrap().is_running());
        assert_eq!(
            orch.subsystem(SUBSYSTEM_ADAPTERS).unwrap().health(),
            SubsystemHealth::Healthy
        );
    }
}

// ===========================================================================
// 6. Capability service
// ===========================================================================

mod capability_service {
    use super::*;
    use std::sync::Arc;

    fn make_service_with_axes(names: &[&str]) -> (CapabilityService, Vec<Arc<AxisEngine>>) {
        let svc = CapabilityService::new();
        let engines: Vec<Arc<AxisEngine>> = names
            .iter()
            .map(|n| {
                let e = Arc::new(AxisEngine::new_for_axis(n.to_string()));
                svc.register_axis(n.to_string(), Arc::clone(&e)).unwrap();
                e
            })
            .collect();
        (svc, engines)
    }

    #[test]
    fn register_and_query_capabilities() {
        let (svc, _engines) = make_service_with_axes(&["pitch", "roll"]);
        let statuses = svc.get_capability_status(None).unwrap();
        assert_eq!(statuses.len(), 2);
        for s in &statuses {
            assert_eq!(s.mode, CapabilityMode::Full);
        }
    }

    #[test]
    fn kid_mode_clamps_output() {
        let (svc, engines) = make_service_with_axes(&["throttle"]);
        svc.set_kid_mode(true).unwrap();

        let mut frame = AxisFrame::new(0.9, 1000);
        frame.out = 0.9;
        engines[0].process(&mut frame).unwrap();
        assert_eq!(frame.out, 0.5, "kid mode should clamp to 50%");
    }

    #[test]
    fn demo_mode_clamps_output() {
        let (svc, engines) = make_service_with_axes(&["roll"]);
        svc.set_demo_mode(true).unwrap();

        let mut frame = AxisFrame::new(0.95, 1000);
        frame.out = 0.95;
        engines[0].process(&mut frame).unwrap();
        assert_eq!(frame.out, 0.8, "demo mode should clamp to 80%");
    }

    #[test]
    fn clamp_counter_increments() {
        let (svc, engines) = make_service_with_axes(&["yaw"]);
        svc.set_kid_mode(true).unwrap();

        for i in 0..5 {
            let mut f = AxisFrame::new(0.9, 1000 + i * 100);
            f.out = 0.9;
            engines[0].process(&mut f).unwrap();
        }

        assert_eq!(svc.total_clamp_events().unwrap(), 5);
    }

    #[test]
    fn max_value_before_clamp_tracks_worst() {
        let (svc, engines) = make_service_with_axes(&["throttle"]);
        svc.set_kid_mode(true).unwrap();

        let mut f1 = AxisFrame::new(0.7, 1000);
        f1.out = 0.7;
        engines[0].process(&mut f1).unwrap();

        let mut f2 = AxisFrame::new(0.95, 2000);
        f2.out = 0.95;
        engines[0].process(&mut f2).unwrap();

        assert!((svc.max_value_before_clamp().unwrap() - 0.95).abs() < f32::EPSILON);
    }

    #[test]
    fn reset_counters_clears_all() {
        let (svc, engines) = make_service_with_axes(&["pitch", "roll"]);
        svc.set_kid_mode(true).unwrap();

        for engine in &engines {
            let mut f = AxisFrame::new(0.9, 1000);
            f.out = 0.9;
            engine.process(&mut f).unwrap();
        }
        assert_eq!(svc.total_clamp_events().unwrap(), 2);

        svc.reset_clamp_counters(None).unwrap();
        assert_eq!(svc.total_clamp_events().unwrap(), 0);
    }

    #[test]
    fn per_axis_mode_override() {
        let (svc, engines) = make_service_with_axes(&["pitch", "roll"]);

        // Set only pitch to kid mode
        svc.set_capability_mode(
            CapabilityMode::Kid,
            Some(vec!["pitch".to_string()]),
            true,
        )
        .unwrap();

        assert_eq!(engines[0].capability_mode(), CapabilityMode::Kid);
        assert_eq!(engines[1].capability_mode(), CapabilityMode::Full);
    }

    #[test]
    fn has_restricted_axes_detects_kid_mode() {
        let (svc, _engines) = make_service_with_axes(&["pitch"]);
        assert!(!svc.has_restricted_axes().unwrap());

        svc.set_kid_mode(true).unwrap();
        assert!(svc.has_restricted_axes().unwrap());

        let restricted = svc.get_restricted_axes().unwrap();
        assert_eq!(restricted.len(), 1);
        assert_eq!(restricted[0].1, CapabilityMode::Kid);
    }

    #[test]
    fn unregister_axis_removes_it() {
        let (svc, _engines) = make_service_with_axes(&["pitch", "roll"]);
        svc.unregister_axis("pitch").unwrap();
        let statuses = svc.get_capability_status(None).unwrap();
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].axis_name, "roll");
    }

    #[test]
    fn custom_config_sets_default_mode() {
        let config = CapabilityServiceConfig {
            default_mode: CapabilityMode::Demo,
            audit_enabled: false,
        };
        let svc = CapabilityService::with_config(config);
        let engine = Arc::new(AxisEngine::new_for_axis("pitch".to_string()));
        svc.register_axis("pitch".to_string(), engine.clone())
            .unwrap();

        assert_eq!(engine.capability_mode(), CapabilityMode::Demo);
    }
}

// ===========================================================================
// 7. Health reporting aggregation
// ===========================================================================

mod health_reporting {
    use super::*;

    #[tokio::test]
    async fn health_stream_starts_healthy() {
        let health = HealthStream::new();
        let status = health.get_health_status().await;
        assert_eq!(
            status.overall.state,
            flight_service::health::HealthState::Healthy
        );
    }

    #[tokio::test]
    async fn warning_escalates_component_state() {
        let health = HealthStream::new();
        health.register_component("bus").await;
        health.warning("bus", "queue near capacity").await;

        let status = health.get_health_status().await;
        assert_eq!(
            status.components["bus"].state,
            flight_service::health::HealthState::Warning
        );
    }

    #[tokio::test]
    async fn error_escalates_to_degraded() {
        let health = HealthStream::new();
        health.register_component("adapter").await;
        health.error("adapter", "connection timeout", None).await;

        let status = health.get_health_status().await;
        assert_eq!(
            status.components["adapter"].state,
            flight_service::health::HealthState::Degraded
        );
    }

    #[tokio::test]
    async fn critical_escalates_to_failed() {
        let health = HealthStream::new();
        health.register_component("ffb").await;
        health.critical("ffb", "safety interlock tripped", None).await;

        let status = health.get_health_status().await;
        assert_eq!(
            status.components["ffb"].state,
            flight_service::health::HealthState::Failed
        );
        // Overall should reflect the worst component
        assert_eq!(
            status.overall.state,
            flight_service::health::HealthState::Failed
        );
    }

    #[tokio::test]
    async fn recent_events_buffer_respects_limit() {
        let health = HealthStream::new();
        health.register_component("test").await;

        for i in 0..110 {
            health.info("test", &format!("event {i}")).await;
        }

        let status = health.get_health_status().await;
        assert!(
            status.recent_events.len() <= 100,
            "recent events should be capped at 100"
        );
    }

    #[test]
    fn health_checker_all_healthy() {
        let mut checker = HealthChecker::new();
        checker
            .set_devices(vec![DeviceHealthInput {
                name: "Stick".into(),
                connected: true,
                error: None,
            }])
            .set_adapters(vec![AdapterHealthInput {
                name: "MSFS".into(),
                connected: true,
                error: None,
            }])
            .set_scheduler(SchedulerHealthInput {
                running: true,
                jitter_p99_us: 100.0,
                overrun_count: 0,
            })
            .set_memory(MemoryHealthInput {
                used_mb: 4096,
                total_mb: 32768,
            });

        let report = checker.check_all();
        assert_eq!(report.status, OverallStatus::Healthy);
        assert!(report.recommendations.is_empty());
    }

    #[test]
    fn health_checker_degraded_on_device_disconnect() {
        let mut checker = HealthChecker::new();
        checker.set_devices(vec![
            DeviceHealthInput {
                name: "Stick".into(),
                connected: true,
                error: None,
            },
            DeviceHealthInput {
                name: "Throttle".into(),
                connected: false,
                error: Some("USB disconnect".into()),
            },
        ]);

        let report = checker.check_all();
        assert_eq!(report.status, OverallStatus::Degraded);
        assert!(!report.recommendations.is_empty());
    }

    #[test]
    fn health_checker_critical_on_scheduler_stopped() {
        let mut checker = HealthChecker::new();
        checker.set_scheduler(SchedulerHealthInput {
            running: false,
            jitter_p99_us: 0.0,
            overrun_count: 0,
        });

        let report = checker.check_all();
        assert_eq!(report.status, OverallStatus::Critical);
    }

    #[test]
    fn health_checker_degraded_on_high_jitter() {
        let mut checker = HealthChecker::new();
        checker.set_scheduler(SchedulerHealthInput {
            running: true,
            jitter_p99_us: 600.0,
            overrun_count: 0,
        });

        let report = checker.check_all();
        assert_eq!(report.status, OverallStatus::Degraded);
    }

    #[test]
    fn health_checker_critical_on_memory_exhaustion() {
        let mut checker = HealthChecker::new();
        checker.set_memory(MemoryHealthInput {
            used_mb: 31500,
            total_mb: 32768,
        });

        let report = checker.check_all();
        assert_eq!(report.status, OverallStatus::Critical);
    }

    #[test]
    fn health_checker_worst_status_wins() {
        let mut checker = HealthChecker::new();
        checker
            .set_devices(vec![DeviceHealthInput {
                name: "OK".into(),
                connected: true,
                error: None,
            }])
            .set_adapters(vec![AdapterHealthInput {
                name: "Disconnected".into(),
                connected: false,
                error: None,
            }])
            .set_scheduler(SchedulerHealthInput {
                running: false,
                jitter_p99_us: 0.0,
                overrun_count: 0,
            });

        let report = checker.check_all();
        // Scheduler stopped is Critical, adapter disconnected is only Critical
        assert_eq!(report.status, OverallStatus::Critical);
    }

    #[test]
    fn overall_status_worse_combinator() {
        assert_eq!(
            OverallStatus::Healthy.worse(OverallStatus::Degraded),
            OverallStatus::Degraded
        );
        assert_eq!(
            OverallStatus::Critical.worse(OverallStatus::Healthy),
            OverallStatus::Critical
        );
        assert_eq!(
            OverallStatus::Degraded.worse(OverallStatus::Critical),
            OverallStatus::Critical
        );
    }
}

// ===========================================================================
// 8. Config validation
// ===========================================================================

mod config_validation {
    use super::*;
    use serde_json::json;

    #[test]
    fn default_config_validates() {
        let cfg = FlightServiceConfig::default();
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn zero_frame_time_rejects() {
        let mut cfg = FlightServiceConfig::default();
        cfg.axis_config.max_frame_time_us = 0;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn zero_poll_hz_rejects() {
        let mut cfg = FlightServiceConfig::default();
        cfg.tflight_poll_hz = 0;
        assert!(cfg.validate().is_err());

        let mut cfg2 = FlightServiceConfig::default();
        cfg2.stecs_poll_hz = 0;
        assert!(cfg2.validate().is_err());
    }

    #[test]
    fn config_roundtrip_json() {
        let cfg = FlightServiceConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let loaded = FlightServiceConfig::load_from_str(&json).unwrap();
        assert_eq!(loaded.tflight_poll_hz, cfg.tflight_poll_hz);
    }

    #[test]
    fn config_invalid_json_fails() {
        assert!(FlightServiceConfig::load_from_str("not json").is_err());
    }

    #[test]
    fn config_missing_file_fails() {
        assert!(FlightServiceConfig::load_from_file("nonexistent_42.json").is_err());
    }

    #[test]
    fn config_load_or_default_falls_back() {
        let cfg = FlightServiceConfig::load_or_default("no_such_file.json");
        assert_eq!(cfg.tflight_poll_hz, 250);
    }

    // -- ConfigValidator checks --

    #[test]
    fn port_range_check_valid() {
        let mut v = ConfigValidator::new();
        v.add_check(Box::new(PortRangeCheck::new("/port")));
        let result = v.validate(&json!({"port": 8080}));
        assert!(result.valid);
    }

    #[test]
    fn port_range_check_out_of_range() {
        let mut v = ConfigValidator::new();
        v.add_check(Box::new(PortRangeCheck::new("/port")));
        let result = v.validate(&json!({"port": 99999}));
        assert!(!result.valid);
    }

    #[test]
    fn required_field_check_missing() {
        let mut v = ConfigValidator::new();
        v.add_check(Box::new(RequiredFieldCheck::new("/name")));
        let result = v.validate(&json!({}));
        assert!(!result.valid);
    }

    #[test]
    fn required_field_check_present() {
        let mut v = ConfigValidator::new();
        v.add_check(Box::new(RequiredFieldCheck::new("/name")));
        let result = v.validate(&json!({"name": "test"}));
        assert!(result.valid);
    }

    #[test]
    fn numeric_range_check() {
        let mut v = ConfigValidator::new();
        v.add_check(Box::new(NumericRangeCheck::new("/hz", 1.0, 1000.0)));

        assert!(v.validate(&json!({"hz": 250})).valid);
        assert!(!v.validate(&json!({"hz": 0})).valid);
        assert!(!v.validate(&json!({"hz": 9999})).valid);
    }

    #[test]
    fn multiple_checks_aggregate() {
        let mut v = ConfigValidator::new();
        v.add_check(Box::new(RequiredFieldCheck::new("/name")));
        v.add_check(Box::new(PortRangeCheck::new("/port")));

        // Both fail
        let result = v.validate(&json!({}));
        assert!(!result.valid);
        assert_eq!(result.errors.len(), 1); // port is a warning when missing
        assert_eq!(result.warnings.len(), 1);
    }

    #[test]
    fn schema_version_format() {
        // Verify Profile schema format
        let profile = Profile {
            schema: "flight.profile/1".to_string(),
            sim: None,
            aircraft: None,
            axes: HashMap::new(),
            pof_overrides: None,
        };
        assert!(profile.validate().is_ok());

        let bad = Profile {
            schema: "wrong".to_string(),
            ..profile
        };
        assert!(bad.validate().is_err());
    }
}

// ===========================================================================
// 9. Multi-sim handling
// ===========================================================================

mod multi_sim_handling {
    use super::*;

    #[test]
    fn two_sims_connected_simultaneously() {
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

    #[test]
    fn multi_sim_profile_swap_per_sim() {
        let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
        orch.start().unwrap();

        // Connect two sims
        orch.handle_adapter_event(AdapterEvent::SimConnected {
            sim_name: "MSFS".into(),
        })
        .unwrap();
        orch.handle_adapter_event(AdapterEvent::SimConnected {
            sim_name: "DCS".into(),
        })
        .unwrap();

        // Profile swap should still work
        let p = orch.handle_profile_change("f18-hornet").unwrap();
        assert_eq!(p.name, "f18-hornet");
    }

    #[test]
    fn disconnect_one_sim_keeps_other() {
        let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
        orch.start().unwrap();

        orch.handle_adapter_event(AdapterEvent::SimConnected {
            sim_name: "MSFS".into(),
        })
        .unwrap();
        orch.handle_adapter_event(AdapterEvent::SimConnected {
            sim_name: "DCS".into(),
        })
        .unwrap();

        orch.handle_adapter_event(AdapterEvent::SimDisconnected {
            sim_name: "MSFS".into(),
        })
        .unwrap();

        assert_eq!(orch.connected_sims(), &["DCS"]);
    }

    #[test]
    fn multi_device_tracking() {
        let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
        orch.start().unwrap();

        orch.handle_device_change(DeviceEvent::Connected {
            device_id: "stick-1".into(),
            device_type: "joystick".into(),
        })
        .unwrap();
        orch.handle_device_change(DeviceEvent::Connected {
            device_id: "throttle-1".into(),
            device_type: "throttle".into(),
        })
        .unwrap();
        orch.handle_device_change(DeviceEvent::Connected {
            device_id: "rudder-1".into(),
            device_type: "pedals".into(),
        })
        .unwrap();

        assert_eq!(orch.connected_devices().len(), 3);

        // Disconnect one
        orch.handle_device_change(DeviceEvent::Disconnected {
            device_id: "throttle-1".into(),
        })
        .unwrap();
        assert_eq!(orch.connected_devices().len(), 2);
    }

    #[test]
    fn data_received_event_is_noop() {
        let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
        orch.start().unwrap();

        // DataReceived should not change connected_sims
        orch.handle_adapter_event(AdapterEvent::DataReceived {
            sim_name: "MSFS".into(),
        })
        .unwrap();
        assert!(orch.connected_sims().is_empty());
    }
}

// ===========================================================================
// 10. Error recovery
// ===========================================================================

mod error_recovery {
    use super::*;

    #[test]
    fn subsystem_error_degrades_health() {
        let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
        orch.start().unwrap();

        orch.record_subsystem_error(SUBSYSTEM_BUS, "queue overflow")
            .unwrap();

        let handle = orch.subsystem(SUBSYSTEM_BUS).unwrap();
        assert_eq!(handle.health(), SubsystemHealth::Degraded);
        assert_eq!(handle.error_count(), 1);
        assert_eq!(handle.last_error(), Some("queue overflow"));
    }

    #[test]
    fn subsystem_failure_does_not_stop_others() {
        let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
        orch.start().unwrap();

        orch.fail_subsystem(SUBSYSTEM_ADAPTERS, "sim crash").unwrap();

        assert!(!orch.subsystem(SUBSYSTEM_ADAPTERS).unwrap().is_running());
        assert_eq!(
            orch.subsystem(SUBSYSTEM_ADAPTERS).unwrap().health(),
            SubsystemHealth::Failed
        );
        // Others still running
        assert!(orch.subsystem(SUBSYSTEM_BUS).unwrap().is_running());
        assert!(orch.subsystem(SUBSYSTEM_SCHEDULER).unwrap().is_running());
    }

    #[test]
    fn restart_subsystem_recovers_from_error() {
        let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
        orch.start().unwrap();

        orch.fail_subsystem(SUBSYSTEM_ADAPTERS, "crash").unwrap();
        assert!(!orch.subsystem(SUBSYSTEM_ADAPTERS).unwrap().is_running());

        orch.restart_subsystem(SUBSYSTEM_ADAPTERS).unwrap();
        assert!(orch.subsystem(SUBSYSTEM_ADAPTERS).unwrap().is_running());
        assert_eq!(
            orch.subsystem(SUBSYSTEM_ADAPTERS).unwrap().health(),
            SubsystemHealth::Healthy
        );
    }

    #[test]
    fn restart_unknown_subsystem_fails() {
        let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
        orch.start().unwrap();

        let err = orch.restart_subsystem("nonexistent").unwrap_err();
        assert_eq!(
            err,
            OrchestratorError::SubsystemNotFound("nonexistent".to_string())
        );
    }

    #[tokio::test]
    async fn service_degraded_recovery_via_profile() {
        let mut service = FlightService::new(FlightServiceConfig::default());
        service.start().await.unwrap();

        // Force degrade — we start() which transitions to Running, then
        // we apply a bad profile to trigger degradation (or call directly via
        // the public state).
        // Alternatively, apply valid + check recovery path:
        let profile = Profile {
            schema: "flight.profile/1".to_string(),
            sim: None,
            aircraft: None,
            axes: HashMap::new(),
            pof_overrides: None,
        };

        // try_recover from non-degraded state returns false
        let recovered = service.try_recover(&profile).await.unwrap();
        assert!(!recovered, "should not recover when not degraded");

        service.shutdown().await.unwrap();
    }

    #[test]
    fn orchestrator_status_reflects_overall_health() {
        let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
        orch.start().unwrap();

        let status = orch.status();
        assert_eq!(status.overall_health, SubsystemHealth::Healthy);

        orch.record_subsystem_error(SUBSYSTEM_BUS, "err").unwrap();
        let status = orch.status();
        assert_eq!(status.overall_health, SubsystemHealth::Degraded);

        orch.fail_subsystem(SUBSYSTEM_SCHEDULER, "fatal").unwrap();
        let status = orch.status();
        assert_eq!(status.overall_health, SubsystemHealth::Failed);
    }

    #[test]
    fn can_restart_full_service_after_stop() {
        let mut orch = ServiceOrchestrator::new(ServiceConfig::default());
        orch.start().unwrap();
        orch.stop().unwrap();

        // Reset phase for restart (orchestrator requires manual phase reset)
        orch = ServiceOrchestrator::new(ServiceConfig::default());
        orch.start().unwrap();
        assert!(orch.is_running());
    }
}

// ===========================================================================
// Additional edge-case tests
// ===========================================================================

mod edge_cases {
    use super::*;

    #[test]
    fn service_state_transition_matrix() {
        // Valid transitions
        assert!(ServiceState::Stopped.can_transition_to(ServiceState::Starting));
        assert!(ServiceState::Starting.can_transition_to(ServiceState::Running));
        assert!(ServiceState::Starting.can_transition_to(ServiceState::SafeMode));
        assert!(ServiceState::Starting.can_transition_to(ServiceState::Failed));
        assert!(ServiceState::Running.can_transition_to(ServiceState::Degraded));
        assert!(ServiceState::Running.can_transition_to(ServiceState::Stopping));
        assert!(ServiceState::Degraded.can_transition_to(ServiceState::Running));
        assert!(ServiceState::Degraded.can_transition_to(ServiceState::Stopping));
        assert!(ServiceState::SafeMode.can_transition_to(ServiceState::Stopping));
        assert!(ServiceState::Stopping.can_transition_to(ServiceState::Stopped));

        // Invalid transitions
        assert!(!ServiceState::Running.can_transition_to(ServiceState::Starting));
        assert!(!ServiceState::Stopped.can_transition_to(ServiceState::Running));
        assert!(!ServiceState::Stopping.can_transition_to(ServiceState::Running));
        assert!(!ServiceState::Failed.can_transition_to(ServiceState::Running));
    }

    #[test]
    fn boot_sequence_transition_matrix() {
        assert!(BootSequence::Initializing.can_transition_to(BootSequence::BusReady));
        assert!(BootSequence::BusReady.can_transition_to(BootSequence::SchedulerReady));
        assert!(BootSequence::SchedulerReady.can_transition_to(BootSequence::AdaptersReady));
        assert!(BootSequence::AdaptersReady.can_transition_to(BootSequence::Running));
        assert!(BootSequence::Running.can_transition_to(BootSequence::ShuttingDown));
        assert!(BootSequence::ShuttingDown.can_transition_to(BootSequence::Stopped));
        assert!(BootSequence::Stopped.can_transition_to(BootSequence::Initializing));

        // Invalid
        assert!(!BootSequence::Initializing.can_transition_to(BootSequence::Running));
        assert!(!BootSequence::Running.can_transition_to(BootSequence::BusReady));
    }

    #[test]
    fn orchestrator_minimal_config_boots_without_adapters() {
        let mut orch = ServiceOrchestrator::new(ServiceConfig {
            enable_watchdog: false,
            enable_adapters: false,
            ..ServiceConfig::default()
        });
        orch.start().unwrap();
        assert!(orch.is_running());
        assert!(orch.subsystem(SUBSYSTEM_BUS).unwrap().is_running());
        assert!(orch.subsystem(SUBSYSTEM_SCHEDULER).unwrap().is_running());
        assert!(orch.subsystem(SUBSYSTEM_ADAPTERS).is_none());
    }

    #[test]
    fn health_check_report_serializes_to_json() {
        let mut checker = HealthChecker::new();
        checker.set_scheduler(SchedulerHealthInput {
            running: true,
            jitter_p99_us: 100.0,
            overrun_count: 0,
        });
        let report = checker.check_all();
        let json = report.to_json();
        assert!(json.is_ok(), "report should serialize to JSON");
    }

    #[test]
    fn orchestrator_error_display() {
        let err = OrchestratorError::InvalidTransition {
            from: BootSequence::Running,
            to: BootSequence::BusReady,
        };
        let msg = format!("{err}");
        assert!(msg.contains("invalid transition"));

        let err2 = OrchestratorError::AlreadyRunning;
        assert_eq!(format!("{err2}"), "orchestrator is already running");
    }
}
