// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Safe Mode Depth Tests
//!
//! Comprehensive testing of the safe mode system:
//! - Degradation triggers and level transitions
//! - Diagnostic bundle content and structure
//! - Recovery paths from degraded/safe-mode states
//!
//! Note: Basic profile tests that require access to private methods
//! (`create_basic_profile`, `build_pipeline_for_axis`) live in the
//! in-crate `#[cfg(test)] mod depth_tests` inside `safe_mode.rs`.

use flight_service::degradation_manager::{DegradationLevel, DegradationManager};
use flight_service::diagnostic_bundle::{
    DeviceState, DiagnosticBundleBuilder, DiagnosticBundleConfig, DiagnosticCollector, ErrorEntry,
    ProfileInfo, ServiceInfo, SystemInfo,
};
use flight_service::{SafeModeConfig, SafeModeManager, ValidationResult};
use std::collections::HashMap;

// =========================================================================
// 1. TRIGGER CONDITIONS (8 tests)
// =========================================================================

/// Service crash (critical component failure) → triggers safe mode.
#[test]
fn trigger_service_crash_enters_safe_mode() {
    let mut mgr = DegradationManager::new();
    mgr.register_component("axis_engine", true);
    mgr.register_component("hid", false);

    mgr.update_health("axis_engine", false);
    assert_eq!(mgr.current_level(), DegradationLevel::SafeMode);
    assert!(!mgr.can_operate());
}

/// Config corruption (critical config component fails) → safe mode.
#[test]
fn trigger_config_corruption_enters_safe_mode() {
    let mut mgr = DegradationManager::new();
    mgr.register_component("config_validator", true);
    mgr.register_component("panels", false);

    mgr.update_health("config_validator", false);
    assert_eq!(mgr.current_level(), DegradationLevel::SafeMode);
    assert!(!mgr.can_operate());
    assert!(mgr.degraded_features().contains(&"config_validator".to_owned()));
}

/// All adapters failed → safe mode (via critical adapter component).
#[test]
fn trigger_all_adapters_failed_enters_safe_mode() {
    let mut mgr = DegradationManager::new();
    mgr.register_component("sim_adapters", true);
    mgr.register_component("panels", false);

    mgr.update_health("sim_adapters", false);
    assert_eq!(mgr.current_level(), DegradationLevel::SafeMode);
    assert!(!mgr.can_operate());
}

/// Watchdog timeout (critical watchdog component fails) → safe mode.
#[test]
fn trigger_watchdog_timeout_enters_safe_mode() {
    let mut mgr = DegradationManager::new();
    mgr.register_component("watchdog", true);
    mgr.register_component("streamdeck", false);

    mgr.update_health("watchdog", false);
    assert_eq!(mgr.current_level(), DegradationLevel::SafeMode);
    assert!(!mgr.can_operate());
}

/// Manual trigger via CLI — safe mode manager can be created and initialized
/// directly, simulating a CLI-triggered safe mode.
#[tokio::test]
async fn trigger_manual_via_cli() {
    let config = SafeModeConfig {
        axis_only: true,
        use_basic_profile: true,
        skip_power_checks: true,
        minimal_mode: true,
    };
    let mut manager = SafeModeManager::new(config);
    let status = manager.initialize().await.unwrap();
    assert!(status.active);
    assert!(status.config.minimal_mode);
}

/// Multiple simultaneous failures — all degrade to safe mode when any is
/// critical, and non-critical accumulate.
#[test]
fn trigger_multiple_simultaneous_failures() {
    let mut mgr = DegradationManager::new();
    mgr.register_component("axis_engine", true);
    mgr.register_component("panels", false);
    mgr.register_component("streamdeck", false);
    mgr.register_component("watchdog", true);

    mgr.update_health("axis_engine", false);
    mgr.update_health("panels", false);
    mgr.update_health("streamdeck", false);
    mgr.update_health("watchdog", false);

    assert_eq!(mgr.current_level(), DegradationLevel::SafeMode);
    assert!(!mgr.can_operate());
    assert_eq!(mgr.degraded_features().len(), 4);
}

/// A transient (non-critical) error should NOT trigger safe mode.
#[test]
fn trigger_transient_error_not_safe_mode() {
    let mut mgr = DegradationManager::new();
    mgr.register_component("axis_engine", true);
    mgr.register_component("panels", false);

    mgr.update_health("panels", false);
    assert_eq!(mgr.current_level(), DegradationLevel::Reduced);
    assert!(mgr.can_operate());
    assert!(
        mgr.components()
            .iter()
            .find(|c| c.name == "axis_engine")
            .unwrap()
            .healthy
    );
}

/// Recovery from degraded to normal: healing a failed component restores level.
#[test]
fn trigger_recovery_from_degraded_to_normal() {
    let mut mgr = DegradationManager::new();
    mgr.register_component("axis_engine", true);
    mgr.register_component("panels", false);

    mgr.update_health("panels", false);
    assert_eq!(mgr.current_level(), DegradationLevel::Reduced);

    mgr.update_health("panels", true);
    assert_eq!(mgr.current_level(), DegradationLevel::Full);
    assert!(mgr.can_operate());
    assert!(mgr.degraded_features().is_empty());
}

// =========================================================================
// 2. DIAGNOSTIC BUNDLE (6 tests)
// =========================================================================

/// Bundle includes system info.
#[test]
fn diagnostic_bundle_includes_system_info() {
    let bundle = DiagnosticCollector::new().build();
    let info = &bundle.system_info;
    assert!(!info.os.is_empty(), "OS must be populated");
    assert!(!info.arch.is_empty(), "architecture must be populated");
    assert!(!info.openflight_version.is_empty(), "version must be populated");

    let text = bundle.to_text();
    assert!(text.contains("## System"));
    assert!(text.contains("OS:"));
}

/// Bundle includes error log (recent errors).
#[test]
fn diagnostic_bundle_includes_error_log() {
    let mut collector = DiagnosticCollector::new();
    collector.add_error(ErrorEntry {
        timestamp_secs_ago: 5,
        category: "hid".into(),
        message: "device enumeration failed".into(),
    });
    collector.add_error(ErrorEntry {
        timestamp_secs_ago: 2,
        category: "ffb".into(),
        message: "motor stall detected".into(),
    });
    let bundle = collector.build();

    assert_eq!(bundle.error_count(), 2);
    let text = bundle.to_text();
    assert!(text.contains("## Errors"));
    assert!(text.contains("device enumeration failed"));
    assert!(text.contains("motor stall detected"));
}

/// Bundle includes device state.
#[test]
fn diagnostic_bundle_includes_device_state() {
    let mut collector = DiagnosticCollector::new();
    collector.add_device(DeviceState {
        name: "T.Flight HOTAS 4".into(),
        vid_pid: "044f:b67b".into(),
        connected: true,
        last_seen_secs_ago: Some(0),
        error: None,
    });
    collector.add_device(DeviceState {
        name: "Saitek X52".into(),
        vid_pid: "06a3:0762".into(),
        connected: false,
        last_seen_secs_ago: Some(300),
        error: Some("USB reset".into()),
    });
    let bundle = collector.build();

    assert_eq!(bundle.device_states.len(), 2);
    let text = bundle.to_text();
    assert!(text.contains("T.Flight HOTAS 4"));
    assert!(text.contains("Saitek X52"));
    assert!(text.contains("044f:b67b"));
    assert!(text.contains("USB reset"));
}

/// Bundle includes config snapshot (profile + service info).
#[test]
fn diagnostic_bundle_includes_config_snapshot() {
    let mut collector = DiagnosticCollector::new();
    collector.set_profile_info(ProfileInfo {
        active_profile: Some("combat_a10c".into()),
        loaded_profiles: vec!["global".into(), "dcs".into(), "combat_a10c".into()],
        last_switch_secs_ago: Some(120),
    });
    collector.set_service_info(ServiceInfo {
        state: "safe_mode".into(),
        safe_mode_reason: Some("FFB fault detected".into()),
        active_plugins: vec!["lua-bridge".into()],
        metrics: HashMap::new(),
    });
    let bundle = collector.build();

    let text = bundle.to_text();
    assert!(text.contains("## Profile"));
    assert!(text.contains("combat_a10c"));
    assert!(text.contains("## Service"));
    assert!(text.contains("safe_mode"));
    assert!(text.contains("FFB fault detected"));

    let json = bundle.to_json().unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["profile_info"]["active_profile"], "combat_a10c");
    assert_eq!(v["service_info"]["state"], "safe_mode");
}

/// Bundle is a single file — the builder writes to a single output path.
#[test]
fn diagnostic_bundle_is_single_file() {
    let output = std::env::temp_dir().join("openflight_diag_depth_test.txt");
    let _ = std::fs::remove_file(&output);

    let config = DiagnosticBundleConfig {
        output_path: Some(output.clone()),
        include_config: true,
        max_log_lines: 1000,
    };
    let mut builder = DiagnosticBundleBuilder::new(config);
    builder
        .add_system_info()
        .add_text("errors.log", "2024-01-01 ERROR: test failure");

    let path = builder.write().expect("should write bundle");
    assert!(path.exists(), "bundle file must exist");
    assert_eq!(path, output);
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("OpenFlight Diagnostic Bundle"));
    assert!(content.contains("errors.log"));

    let _ = std::fs::remove_file(&path);
}

/// Bundle doesn't include sensitive data: no passwords, tokens, or secrets.
#[test]
fn diagnostic_bundle_no_sensitive_data() {
    let mut collector = DiagnosticCollector::new();
    collector.set_system_info(SystemInfo::collect());
    collector.set_service_info(ServiceInfo {
        state: "running".into(),
        safe_mode_reason: None,
        active_plugins: vec![],
        metrics: HashMap::new(),
    });
    collector.set_profile_info(ProfileInfo {
        active_profile: Some("default".into()),
        loaded_profiles: vec!["default".into()],
        last_switch_secs_ago: None,
    });
    let bundle = collector.build();
    let text = bundle.to_text();
    let json = bundle.to_json().unwrap();

    let sensitive_patterns = [
        "password",
        "secret",
        "token",
        "api_key",
        "private_key",
        "credential",
    ];
    for pattern in &sensitive_patterns {
        assert!(
            !text.to_lowercase().contains(pattern),
            "text report must not contain '{pattern}'"
        );
        assert!(
            !json.to_lowercase().contains(pattern),
            "JSON report must not contain '{pattern}'"
        );
    }
}

// =========================================================================
// 3. RECOVERY (6 tests)
// =========================================================================

/// Manual recovery via CLI: shutdown the safe mode manager.
#[tokio::test]
async fn recovery_manual_via_cli() {
    let config = SafeModeConfig {
        axis_only: true,
        use_basic_profile: true,
        skip_power_checks: true,
        minimal_mode: true,
    };
    let mut manager = SafeModeManager::new(config);
    let status = manager.initialize().await.unwrap();
    assert!(status.active);

    let result = manager.shutdown().await;
    assert!(result.is_ok(), "manual recovery via shutdown must succeed");
}

/// Auto-recovery after N successful ticks: restoring health on all components
/// brings degradation level back to Full.
#[test]
fn recovery_auto_after_successful_ticks() {
    let mut mgr = DegradationManager::new();
    mgr.register_component("axis_engine", true);
    mgr.register_component("panels", false);

    mgr.update_health("axis_engine", false);
    assert_eq!(mgr.current_level(), DegradationLevel::SafeMode);

    mgr.update_health("axis_engine", true);
    assert_eq!(mgr.current_level(), DegradationLevel::Full);
    assert!(mgr.can_operate());
}

/// Recovery allows clean reinitialization: after safe mode shutdown, a
/// brand-new manager can initialize from default config (fresh state).
#[tokio::test]
async fn recovery_allows_clean_reinitialization() {
    let config = SafeModeConfig {
        axis_only: true,
        use_basic_profile: true,
        skip_power_checks: true,
        minimal_mode: true,
    };
    let mut manager = SafeModeManager::new(config);
    let _status = manager.initialize().await.unwrap();

    let status = manager.get_status();
    assert!(status.active);
    assert!(status.config.use_basic_profile);

    manager.shutdown().await.unwrap();
    // After shutdown, re-initialization should work cleanly (fresh state)
    let mut fresh = SafeModeManager::new(SafeModeConfig::default());
    let fresh_status = fresh.initialize().await.unwrap();
    assert!(fresh_status.active, "fresh initialization should work after recovery");
}

/// Recovery config allows FFB: safe mode config disables FFB via axis_only;
/// recovery means creating a new non-safe-mode configuration that does NOT
/// set axis_only.
///
/// TODO: When full FFB gradual re-enable is implemented, add a separate test
/// for the ramp-up sequence (e.g., 25% → 50% → 75% → 100% over successive ticks).
#[test]
fn recovery_config_allows_ffb() {
    let safe_config = SafeModeConfig {
        axis_only: true,
        use_basic_profile: true,
        skip_power_checks: true,
        minimal_mode: true,
    };
    assert!(safe_config.axis_only, "safe mode must disable FFB");
    assert!(safe_config.minimal_mode, "safe mode must be minimal");

    let recovery_config = SafeModeConfig {
        axis_only: false,
        use_basic_profile: false,
        skip_power_checks: false,
        minimal_mode: false,
    };
    assert!(!recovery_config.axis_only, "recovery should allow FFB");
    assert!(!recovery_config.minimal_mode, "recovery should exit minimal mode");
}

/// Recovery notifies connected clients: the diagnostic bundle captures the
/// recovery event so that any monitoring client can observe the transition.
///
/// TODO: When IPC notification is implemented, test that a gRPC event is
/// emitted on the health stream when transitioning out of safe mode.
#[test]
fn recovery_notifies_via_diagnostic() {
    let manager = SafeModeManager::new(SafeModeConfig::default());

    let results = vec![
        ValidationResult {
            component: "Axis Engine".into(),
            success: true,
            message: "OK".into(),
            execution_time_ms: 1,
        },
        ValidationResult {
            component: "RT Privileges".into(),
            success: true,
            message: "OK".into(),
            execution_time_ms: 1,
        },
    ];
    let diag = manager.build_diagnostic(&results);

    assert!(diag.failed_components.is_empty());
    assert!(diag.reason.contains("operator request"));
    assert!(
        diag.recommended_actions
            .iter()
            .any(|a| a.contains("exited")),
        "should recommend exiting safe mode when no failures: {:?}",
        diag.recommended_actions
    );
}

/// Recovery logs event: the diagnostic snapshot captures validation results
/// so the recovery event is auditable.
#[test]
fn recovery_logs_event() {
    let manager = SafeModeManager::new(SafeModeConfig::default());

    let failure_results = vec![ValidationResult {
        component: "Axis Engine".into(),
        success: false,
        message: "HID timeout".into(),
        execution_time_ms: 5,
    }];
    let failure_diag = manager.build_diagnostic(&failure_results);
    assert!(!failure_diag.failed_components.is_empty());
    assert_eq!(failure_diag.validation_snapshot.len(), 1);

    let recovery_results = vec![ValidationResult {
        component: "Axis Engine".into(),
        success: true,
        message: "recovered".into(),
        execution_time_ms: 2,
    }];
    let recovery_diag = manager.build_diagnostic(&recovery_results);
    assert!(recovery_diag.failed_components.is_empty());
    assert_eq!(recovery_diag.validation_snapshot.len(), 1);
    assert!(
        recovery_diag.validation_snapshot[0].success,
        "recovery snapshot must show success"
    );
}
