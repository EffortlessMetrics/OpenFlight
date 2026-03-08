// SPDX-License-Identifier: MIT OR Apache-2.0

//! Snapshot tests for flight-security audit events, verification results,
//! capability manifests, and security configuration.

use flight_security::verification::{
    AuditEvent, AuditEventType, RecommendationPriority, SecurityCategory, SecurityCheck,
    SecurityRecommendation, SecuritySeverity, SecurityVerificationResult, VerificationConfig,
    VerificationStatus,
};
use flight_security::{
    PluginCapability, PluginCapabilityManifest, PluginType, SecurityConfig, SignatureStatus,
    TelemetryConfig, TelemetryDataType,
};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

// ── Security verification result snapshots ──────────────────────────────────

#[test]
fn snapshot_verification_result_all_pass() {
    let result = SecurityVerificationResult {
        overall_status: VerificationStatus::Pass,
        checks: vec![
            SecurityCheck {
                name: "IPC local-only binding".into(),
                category: SecurityCategory::IpcSecurity,
                status: VerificationStatus::Pass,
                description: "IPC server binds only to localhost".into(),
                details: Some("Bound to 127.0.0.1:50051".into()),
                severity: SecuritySeverity::Critical,
                remediation: None,
            },
            SecurityCheck {
                name: "Plugin signature verification".into(),
                category: SecurityCategory::SignatureValidation,
                status: VerificationStatus::Pass,
                description: "All loaded plugins have valid signatures".into(),
                details: None,
                severity: SecuritySeverity::High,
                remediation: None,
            },
            SecurityCheck {
                name: "Telemetry consent".into(),
                category: SecurityCategory::TelemetryPrivacy,
                status: VerificationStatus::Pass,
                description: "Telemetry collection disabled (no consent)".into(),
                details: None,
                severity: SecuritySeverity::Medium,
                remediation: None,
            },
        ],
        audit_events: vec![],
        recommendations: vec![],
        timestamp: 1700000000,
        duration_ms: 42,
    };
    insta::assert_yaml_snapshot!("verification_result_all_pass", result);
}

#[test]
fn snapshot_verification_result_with_warnings() {
    let result = SecurityVerificationResult {
        overall_status: VerificationStatus::Warning,
        checks: vec![
            SecurityCheck {
                name: "File system access policy".into(),
                category: SecurityCategory::FileSystemAccess,
                status: VerificationStatus::Warning,
                description: "Plugin 'nav-helper' has broad file system access".into(),
                details: Some("Allowed paths: /home/user/*, /tmp/*".into()),
                severity: SecuritySeverity::Medium,
                remediation: Some(
                    "Restrict file system access to specific directories".into(),
                ),
            },
        ],
        audit_events: vec![AuditEvent {
            timestamp: 1700000000,
            event_type: AuditEventType::FileAccess,
            component: "nav-helper".into(),
            description: "Plugin accessed /home/user/.config/openflight/profiles".into(),
            metadata: HashMap::new(),
            severity: SecuritySeverity::Low,
        }],
        recommendations: vec![SecurityRecommendation {
            title: "Restrict plugin file system access".into(),
            description: "The nav-helper plugin has broad file system access permissions".into(),
            priority: RecommendationPriority::Medium,
            action_required: false,
            remediation_steps: vec![
                "Edit plugin manifest to specify exact paths".into(),
                "Remove wildcard path patterns".into(),
            ],
        }],
        timestamp: 1700000000,
        duration_ms: 55,
    };
    insta::assert_yaml_snapshot!("verification_result_with_warnings", result);
}

// ── Audit event snapshots ───────────────────────────────────────────────────

#[test]
fn snapshot_audit_event_types() {
    let events = vec![
        AuditEvent {
            timestamp: 1700000000,
            event_type: AuditEventType::IpcConnection,
            component: "grpc-server".into(),
            description: "Client connected from PID 12345".into(),
            metadata: {
                let mut m = HashMap::new();
                m.insert("pid".into(), "12345".into());
                m
            },
            severity: SecuritySeverity::Info,
        },
        AuditEvent {
            timestamp: 1700000001,
            event_type: AuditEventType::PluginLoad,
            component: "plugin-registry".into(),
            description: "Loaded WASM plugin 'traffic-overlay' v1.2.0".into(),
            metadata: HashMap::new(),
            severity: SecuritySeverity::Low,
        },
        AuditEvent {
            timestamp: 1700000002,
            event_type: AuditEventType::SecurityViolation,
            component: "capability-enforcer".into(),
            description: "Plugin 'rogue-plugin' attempted unauthorized network access".into(),
            metadata: {
                let mut m = HashMap::new();
                m.insert("target_host".into(), "example.com:443".into());
                m
            },
            severity: SecuritySeverity::Critical,
        },
    ];
    insta::with_settings!({sort_maps => true}, {
        insta::assert_yaml_snapshot!("audit_event_types", events);
    });
}

// ── Plugin capability manifest snapshots ────────────────────────────────────

#[test]
fn snapshot_plugin_manifest_wasm() {
    let manifest = PluginCapabilityManifest {
        name: "traffic-overlay".into(),
        version: "1.2.0".into(),
        capabilities: {
            let mut caps = HashSet::new();
            caps.insert(PluginCapability::ReadBus);
            caps.insert(PluginCapability::ReadDeviceHealth);
            caps
        },
        description: Some("Real-time traffic overlay for VFR navigation".into()),
        plugin_type: PluginType::Wasm,
        signature: SignatureStatus::Signed {
            issuer: "OpenFlight Plugin Authority".into(),
            subject: "traffic-overlay-dev@example.com".into(),
            valid_from: 1700000000,
            valid_until: 1731536000,
        },
    };
    let mut v: serde_json::Value = serde_json::to_value(&manifest).unwrap();
    if let Some(arr) = v.get_mut("capabilities").and_then(|c| c.as_array_mut()) {
        arr.sort_by(|a, b| a.to_string().cmp(&b.to_string()));
    }
    let json = serde_json::to_string_pretty(&v).unwrap();
    insta::assert_snapshot!("plugin_manifest_wasm", json);
}

#[test]
fn snapshot_plugin_manifest_native_unsigned() {
    let manifest = PluginCapabilityManifest {
        name: "custom-panel-driver".into(),
        version: "0.5.0".into(),
        capabilities: {
            let mut caps = HashSet::new();
            caps.insert(PluginCapability::EmitPanel);
            caps.insert(PluginCapability::WriteBlackbox);
            caps.insert(PluginCapability::FileSystem {
                paths: vec![PathBuf::from("/opt/panels/config")],
            });
            caps
        },
        description: None,
        plugin_type: PluginType::Native,
        signature: SignatureStatus::Unsigned,
    };
    let mut v: serde_json::Value = serde_json::to_value(&manifest).unwrap();
    if let Some(arr) = v.get_mut("capabilities").and_then(|c| c.as_array_mut()) {
        arr.sort_by(|a, b| a.to_string().cmp(&b.to_string()));
    }
    let json = serde_json::to_string_pretty(&v).unwrap();
    insta::assert_snapshot!("plugin_manifest_native_unsigned", json);
}

// ── Security config snapshots ───────────────────────────────────────────────

#[test]
fn snapshot_security_config_default_yaml() {
    let config = SecurityConfig::default();
    insta::assert_yaml_snapshot!("security_config_default", config);
}

#[test]
fn snapshot_telemetry_config_default_yaml() {
    let config = TelemetryConfig::default();
    insta::assert_yaml_snapshot!("telemetry_config_default", config);
}

#[test]
fn snapshot_telemetry_config_opted_in_yaml() {
    let config = TelemetryConfig {
        enabled: true,
        consent_timestamp: Some(1700000000),
        collected_data: {
            let mut d = HashSet::new();
            d.insert(TelemetryDataType::Performance);
            d.insert(TelemetryDataType::Errors);
            d.insert(TelemetryDataType::DeviceEvents);
            d
        },
        retention_days: 90,
        include_in_support: true,
    };
    let mut v: serde_json::Value = serde_json::to_value(&config).unwrap();
    if let Some(arr) = v.get_mut("collected_data").and_then(|c| c.as_array_mut()) {
        arr.sort_by(|a, b| a.to_string().cmp(&b.to_string()));
    }
    let json = serde_json::to_string_pretty(&v).unwrap();
    insta::assert_snapshot!("telemetry_config_opted_in", json);
}

// ── Verification config snapshot ────────────────────────────────────────────

#[test]
fn snapshot_verification_config_default_yaml() {
    let config = VerificationConfig::default();
    insta::assert_yaml_snapshot!("verification_config_default", config);
}

// ── Security error display ──────────────────────────────────────────────────

#[test]
fn snapshot_security_error_display() {
    let errors: Vec<String> = vec![
        flight_security::SecurityError::SignatureVerificationFailed {
            reason: "certificate expired on 2024-01-15".into(),
        }
        .to_string(),
        flight_security::SecurityError::CapabilityDenied {
            capability: "Network { hosts: [\"example.com\"] }".into(),
        }
        .to_string(),
        flight_security::SecurityError::InvalidManifest {
            reason: "missing required field 'capabilities'".into(),
        }
        .to_string(),
        flight_security::SecurityError::TelemetryNotAuthorized.to_string(),
        flight_security::SecurityError::UnauthorizedPath {
            path: PathBuf::from("/etc/shadow"),
            allowed_roots: vec![PathBuf::from("/opt/openflight"), PathBuf::from("/tmp")],
        }
        .to_string(),
        flight_security::SecurityError::PathTraversal {
            path: PathBuf::from("../../../etc/passwd"),
        }
        .to_string(),
    ];
    let output = errors.join("\n");
    insta::assert_snapshot!("security_error_display", output);
}
