// SPDX-License-Identifier: MIT OR Apache-2.0

//! Snapshot tests for flight-service health reports, diagnostic bundles,
//! error taxonomy, and safe mode status.

use flight_service::diagnostic_bundle::DegradationReason;
use flight_service::error_taxonomy::{ErrorCategory, ErrorCode, ErrorTaxonomy};
use flight_service::health::{
    HealthCategory, HealthCheck, HealthCheckReport, HealthEvent, HealthSeverity, OverallStatus,
};
use flight_service::safe_mode::{SafeModeConfig, SafeModeDiagnostic, ValidationResult};
use flight_service::power::{PowerCheck, PowerCheckStatus, PowerStatus, RemediationStep};
use std::collections::HashMap;

// ── Health report snapshots ─────────────────────────────────────────────────

#[test]
fn snapshot_health_check_report_healthy() {
    let report = HealthCheckReport {
        status: OverallStatus::Healthy,
        checks: vec![
            HealthCheck {
                name: "axis-engine".into(),
                status: OverallStatus::Healthy,
                message: "250Hz tick rate stable, p99 jitter 0.12ms".into(),
                latency_ms: 0.45,
            },
            HealthCheck {
                name: "hid-manager".into(),
                status: OverallStatus::Healthy,
                message: "3 devices connected, no errors".into(),
                latency_ms: 1.2,
            },
            HealthCheck {
                name: "ffb-engine".into(),
                status: OverallStatus::Healthy,
                message: "All envelopes within limits".into(),
                latency_ms: 0.8,
            },
        ],
        recommendations: vec![],
    };
    let json = report.to_json().unwrap();
    insta::assert_snapshot!("health_check_report_healthy", json);
}

#[test]
fn snapshot_health_check_report_degraded() {
    let report = HealthCheckReport {
        status: OverallStatus::Degraded,
        checks: vec![
            HealthCheck {
                name: "axis-engine".into(),
                status: OverallStatus::Healthy,
                message: "Operating normally".into(),
                latency_ms: 0.3,
            },
            HealthCheck {
                name: "hid-manager".into(),
                status: OverallStatus::Degraded,
                message: "Device 044F:B10A not responding, retrying".into(),
                latency_ms: 305.0,
            },
        ],
        recommendations: vec![
            "Check USB connection for Thrustmaster T.Flight HOTAS 4".into(),
            "Run 'flightctl diag' for a full diagnostic bundle".into(),
        ],
    };
    let json = report.to_json().unwrap();
    insta::assert_snapshot!("health_check_report_degraded", json);
}

// ── Health event snapshot ───────────────────────────────────────────────────

#[test]
fn snapshot_health_event_yaml() {
    let event = HealthEvent {
        id: "evt_00000001".into(),
        timestamp: 1700000000,
        component: "ffb-engine".into(),
        severity: HealthSeverity::Warning,
        category: HealthCategory::Safety,
        message: "Torque approaching envelope limit on channel 0".into(),
        error_code: Some(ErrorCode {
            code: "TORQUE_LIMIT_EXCEEDED".into(),
            category: ErrorCategory::Safety,
            description: "Force feedback torque exceeded safety limits".into(),
            kb_url: Some("https://docs.flight-hub.dev/kb/TORQUE_LIMIT_EXCEEDED".into()),
        }),
        metadata: {
            let mut m = HashMap::new();
            m.insert("channel".into(), "0".into());
            m.insert("current_nm".into(), "2.8".into());
            m.insert("limit_nm".into(), "3.0".into());
            m
        },
    };
    insta::with_settings!({sort_maps => true}, {
        insta::assert_yaml_snapshot!("health_event_warning", event);
    });
}

// ── Error taxonomy snapshots ────────────────────────────────────────────────

#[test]
fn snapshot_error_taxonomy_hardware_codes() {
    let taxonomy = ErrorTaxonomy::new();
    let mut hardware_codes: Vec<String> = taxonomy
        .get_errors_by_category(ErrorCategory::Hardware)
        .iter()
        .map(|c| format!("{}: {}", c.code, c.description))
        .collect();
    hardware_codes.sort();
    let output = hardware_codes.join("\n");
    insta::assert_snapshot!("error_taxonomy_hardware_codes", output);
}

#[test]
fn snapshot_error_taxonomy_safety_codes() {
    let taxonomy = ErrorTaxonomy::new();
    let mut safety_codes: Vec<String> = taxonomy
        .get_errors_by_category(ErrorCategory::Safety)
        .iter()
        .map(|c| format!("{}: {}", c.code, c.description))
        .collect();
    safety_codes.sort();
    let output = safety_codes.join("\n");
    insta::assert_snapshot!("error_taxonomy_safety_codes", output);
}

// ── Safe mode snapshots ─────────────────────────────────────────────────────

#[test]
fn snapshot_safe_mode_config_default_yaml() {
    let config = SafeModeConfig::default();
    insta::assert_yaml_snapshot!("safe_mode_config_default", config);
}

#[test]
fn snapshot_safe_mode_diagnostic_yaml() {
    let diag = SafeModeDiagnostic {
        reason: "FFB subsystem fault detected during startup".into(),
        failed_components: vec!["ffb-engine".into(), "hid-manager".into()],
        recommended_actions: vec![
            "Disconnect and reconnect force feedback device".into(),
            "Check device firmware version".into(),
            "Run 'flightctl diag' and share the output with support".into(),
        ],
        validation_snapshot: vec![
            ValidationResult {
                component: "axis-engine".into(),
                success: true,
                message: "Axis processing pipeline initialized".into(),
                execution_time_ms: 12,
            },
            ValidationResult {
                component: "ffb-engine".into(),
                success: false,
                message: "FFB device handshake timeout after 5000ms".into(),
                execution_time_ms: 5001,
            },
        ],
    };
    insta::assert_yaml_snapshot!("safe_mode_diagnostic", diag);
}

// ── Power status snapshot ───────────────────────────────────────────────────

#[test]
fn snapshot_power_status_degraded_yaml() {
    let status = PowerStatus {
        overall_status: PowerCheckStatus::Degraded,
        checks: vec![
            PowerCheck {
                name: "Power Plan".into(),
                status: PowerCheckStatus::Degraded,
                description: "Windows power plan is not set to High Performance".into(),
                current_value: Some("Balanced".into()),
                expected_value: Some("High Performance".into()),
            },
            PowerCheck {
                name: "USB Selective Suspend".into(),
                status: PowerCheckStatus::Optimal,
                description: "USB selective suspend is disabled".into(),
                current_value: Some("Disabled".into()),
                expected_value: Some("Disabled".into()),
            },
        ],
        remediation_steps: vec![RemediationStep {
            description: "Switch to High Performance power plan".into(),
            action: "powercfg /setactive 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c".into(),
            requires_admin: true,
            priority: 1,
        }],
    };
    insta::assert_yaml_snapshot!("power_status_degraded", status);
}

// ── Degradation reason display ──────────────────────────────────────────────

#[test]
fn snapshot_degradation_reason_display() {
    let reasons = vec![
        DegradationReason::FfbFault("envelope limit exceeded on channel 0".into()),
        DegradationReason::HidEnumerationFailure("044F:B10A not found on any USB bus".into()),
        DegradationReason::AdapterDisconnect("MSFS SimConnect connection lost".into()),
        DegradationReason::ConfigError("profile merge conflict on pitch axis".into()),
        DegradationReason::PluginFault("plugin 'nav-helper' exceeded 100μs budget".into()),
        DegradationReason::SchedulerFailure("MMCSS task priority elevation failed".into()),
        DegradationReason::Unknown("unclassified startup error".into()),
    ];
    let mut output = String::new();
    for r in &reasons {
        output.push_str(&format!("{}\n", r));
    }
    insta::assert_snapshot!("degradation_reason_display", output);
}
