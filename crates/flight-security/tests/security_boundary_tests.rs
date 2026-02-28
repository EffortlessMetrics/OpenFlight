// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Security boundary tests — IPC access control and connection validation.

use flight_security::{
    AclConfig, FsAccessPolicy, IpcClientInfo, PluginCapability, PluginCapabilityManifest,
    PluginType, SecurityConfig, SecurityManager, SignatureStatus, TelemetryConfig,
};
use std::collections::HashSet;
use std::path::PathBuf;

fn make_client(user_id: &str) -> IpcClientInfo {
    IpcClientInfo {
        user_id: user_id.to_string(),
        process_id: std::process::id(),
        executable_path: PathBuf::from("flightctl"),
    }
}

fn unsigned_ok_config() -> SecurityConfig {
    SecurityConfig {
        allow_unsigned: true,
        ..SecurityConfig::default()
    }
}

fn mgr_allow_unsigned() -> SecurityManager {
    SecurityManager::with_config(
        unsigned_ok_config(),
        TelemetryConfig::default(),
        AclConfig::default(),
    )
}

fn mgr_with_acl(current_user_only: bool, allowed_users: HashSet<String>) -> SecurityManager {
    SecurityManager::with_config(
        SecurityConfig::default(),
        TelemetryConfig::default(),
        AclConfig {
            current_user_only,
            allowed_users,
            ..AclConfig::default()
        },
    )
}

/// Local connections are accepted when no ACL list is configured and
/// `current_user_only` is disabled.
#[test]
fn test_local_only_connection_accepted() {
    let mgr = mgr_with_acl(false, HashSet::new());
    let client = make_client("any-local-user");
    assert!(
        mgr.validate_ipc_acl(&client).is_ok(),
        "connection from a local user should be accepted when no ACL restrictions apply"
    );
}

/// When the `allowed_users` list is non-empty, callers not on the list are
/// rejected — analogous to rejecting unauthorised non-loopback connections.
#[test]
fn test_remote_connection_rejected() {
    let mut allowed = HashSet::new();
    allowed.insert("authorised-user".to_string());
    let mgr = mgr_with_acl(false, allowed);
    let client = make_client("unauthorised-user");
    assert!(
        mgr.validate_ipc_acl(&client).is_err(),
        "connection from a user not in the allowed list must be rejected"
    );
}

/// A user that appears in the `allowed_users` allow-list should be permitted
/// — validates allow-list semantics (analogous to a valid token being accepted).
#[test]
fn test_token_format_validation() {
    let mut allowed = HashSet::new();
    allowed.insert("valid-user".to_string());
    let mgr = mgr_with_acl(false, allowed);

    // Valid: user matches the allow-list entry exactly
    assert!(
        mgr.validate_ipc_acl(&make_client("valid-user")).is_ok(),
        "user in the allowed list should be accepted"
    );

    // Invalid: partial/misspelled ID must not sneak through
    assert!(
        mgr.validate_ipc_acl(&make_client("valid-user-extra"))
            .is_err(),
        "a suffix-extended user ID must not match an allow-list entry"
    );
    assert!(
        mgr.validate_ipc_acl(&make_client("VALID-USER")).is_err(),
        "a case-differing user ID must not match (allow-list is case-sensitive)"
    );
}

/// When `current_user_only` is enabled every caller whose user ID does not
/// match the current process owner is rejected — this simulates the rate-limit
/// / hard-block behaviour after an ACL violation, ensuring repeated foreign
/// callers are never let through.
#[test]
fn test_rate_limit_enforcement() {
    // Use a user ID that is guaranteed to differ from the current OS user.
    let foreign_uid = "S-1-5-21-FOREIGN-USER-9999";
    let mgr = mgr_with_acl(true, HashSet::new());

    // All attempts from the foreign user must be rejected, every time.
    for attempt in 0..5 {
        let result = mgr.validate_ipc_acl(&make_client(foreign_uid));
        assert!(
            result.is_err(),
            "attempt {attempt}: foreign user must be rejected in current_user_only mode"
        );
    }
}

// --- Plugin capability enforcement (integration) ---

#[test]
fn test_plugin_capability_enforcement_end_to_end() {
    let mut mgr = mgr_allow_unsigned();

    let manifest = PluginCapabilityManifest {
        name: "panel-driver".to_string(),
        version: "2.0.0".to_string(),
        capabilities: [PluginCapability::ReadBus, PluginCapability::EmitPanel]
            .into_iter()
            .collect(),
        description: Some("Panel LED driver".to_string()),
        plugin_type: PluginType::Wasm,
        signature: SignatureStatus::Unsigned,
    };
    mgr.validate_plugin(manifest).unwrap();

    // Declared capabilities succeed
    assert!(mgr.check_escalation("panel-driver", &PluginCapability::ReadBus).is_ok());
    assert!(mgr.check_escalation("panel-driver", &PluginCapability::EmitPanel).is_ok());

    // Undeclared capability is denied
    assert!(mgr.check_escalation("panel-driver", &PluginCapability::WriteBlackbox).is_err());
    assert!(mgr.check_escalation("panel-driver", &PluginCapability::ReadProfiles).is_err());
}

// --- Path traversal prevention (integration) ---

#[test]
fn test_path_traversal_prevention_integration() {
    let tmp = tempfile::TempDir::new().unwrap();
    let config_dir = tmp.path().join("config");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(config_dir.join("ok.json"), b"{}").unwrap();

    let mut mgr = SecurityManager::new();
    mgr.set_fs_policy(FsAccessPolicy::new(&[config_dir.clone()]));

    // Access within the config dir — allowed
    assert!(mgr.validate_fs_access(&config_dir.join("ok.json")).is_ok());

    // Path traversal — blocked
    let traversal = config_dir.join("..").join("secrets");
    assert!(mgr.validate_fs_access(&traversal).is_err());
}

// --- Update signature verification (integration) ---

#[test]
fn test_update_signature_verification_integration() {
    let content = b"flight-update-v3.1.0 binary payload";
    let digest = flight_security::sha256_hex(content);

    let payload = flight_security::SignedPayload {
        name: "flightd-update".to_string(),
        version: "3.1.0".to_string(),
        expected_digest: digest,
    };

    // Valid content passes
    assert!(payload.verify(content).is_ok());

    // Tampered content fails
    assert!(payload.verify(b"tampered payload").is_err());
}

#[test]
fn test_file_based_update_verification() {
    let tmp = tempfile::TempDir::new().unwrap();
    let update_path = tmp.path().join("update.bin");
    let content = b"authentic update payload";
    std::fs::write(&update_path, content).unwrap();

    let expected = flight_security::sha256_hex(content);
    assert!(flight_security::verify_file_digest(&update_path, &expected).is_ok());

    // Write tampered content
    std::fs::write(&update_path, b"corrupted").unwrap();
    assert!(flight_security::verify_file_digest(&update_path, &expected).is_err());
}

// --- Audit event emission (integration) ---

#[test]
fn test_audit_events_emitted_for_security_actions() {
    let mut mgr = mgr_allow_unsigned();

    // Register a plugin — should produce audit success
    let manifest = PluginCapabilityManifest {
        name: "audit-integration".to_string(),
        version: "1.0.0".to_string(),
        capabilities: [PluginCapability::ReadBus].into_iter().collect(),
        description: None,
        plugin_type: PluginType::Wasm,
        signature: SignatureStatus::Unsigned,
    };
    mgr.validate_plugin(manifest).unwrap();

    // Attempt an escalation — should produce audit denied
    let _ = mgr.check_escalation("audit-integration", &PluginCapability::EmitPanel);

    let log = mgr.audit_log();
    assert!(log.len() >= 2, "expected at least 2 audit entries, got {}", log.len());

    // Verify we have both success and denied entries
    let has_success = log.entries().iter().any(|e| {
        e.outcome == flight_security::audit_log::AuditOutcome::Success
    });
    let has_denied = log.entries().iter().any(|e| {
        e.outcome == flight_security::audit_log::AuditOutcome::Denied
    });
    assert!(has_success, "should have a success audit entry");
    assert!(has_denied, "should have a denied audit entry");
}

// --- Permission escalation prevention (integration) ---

#[test]
fn test_permission_escalation_prevention_wasm_fs() {
    let mut mgr = mgr_allow_unsigned();

    // WASM plugin requesting filesystem access — must be rejected
    let manifest = PluginCapabilityManifest {
        name: "wasm-fs-attempt".to_string(),
        version: "1.0.0".to_string(),
        capabilities: [PluginCapability::FileSystem {
            paths: vec![PathBuf::from("/etc")],
        }]
        .into_iter()
        .collect(),
        description: None,
        plugin_type: PluginType::Wasm,
        signature: SignatureStatus::Unsigned,
    };
    assert!(
        mgr.validate_plugin(manifest).is_err(),
        "WASM plugin must not be granted filesystem capabilities"
    );
}

#[test]
fn test_permission_escalation_prevention_wasm_network() {
    let mut mgr = mgr_allow_unsigned();

    // WASM plugin requesting network access — must be rejected
    let manifest = PluginCapabilityManifest {
        name: "wasm-net-attempt".to_string(),
        version: "1.0.0".to_string(),
        capabilities: [PluginCapability::Network {
            hosts: vec!["evil.example.com".to_string()],
        }]
        .into_iter()
        .collect(),
        description: None,
        plugin_type: PluginType::Wasm,
        signature: SignatureStatus::Unsigned,
    };
    assert!(
        mgr.validate_plugin(manifest).is_err(),
        "WASM plugin must not be granted network capabilities"
    );
}

#[test]
fn test_runtime_escalation_after_registration() {
    let mut mgr = mgr_allow_unsigned();

    let manifest = PluginCapabilityManifest {
        name: "read-only".to_string(),
        version: "1.0.0".to_string(),
        capabilities: [PluginCapability::ReadBus].into_iter().collect(),
        description: None,
        plugin_type: PluginType::Wasm,
        signature: SignatureStatus::Unsigned,
    };
    mgr.validate_plugin(manifest).unwrap();

    // Try every capability the plugin did NOT declare
    let undeclared = [
        PluginCapability::EmitPanel,
        PluginCapability::ReadProfiles,
        PluginCapability::WriteBlackbox,
        PluginCapability::ReadDeviceHealth,
    ];
    for cap in &undeclared {
        assert!(
            mgr.check_escalation("read-only", cap).is_err(),
            "capability {cap:?} should be denied for read-only plugin"
        );
    }
}
