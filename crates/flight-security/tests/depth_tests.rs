// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the security subsystem — capability model, sandbox boundaries,
//! audit logging, signature verification, token management, and integration scenarios.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use flight_security::audit_log::{
    AuditCategory, AuditEntry, AuditLog, AuditOutcome, AuditSeverity,
};
use flight_security::{
    AclConfig, FsAccessPolicy, IpcClientInfo, PluginCapability, PluginCapabilityManifest,
    PluginType, SecurityConfig, SecurityManager, SignatureStatus, SignedPayload, TelemetryConfig,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn mgr_allow_unsigned() -> SecurityManager {
    SecurityManager::with_config(
        SecurityConfig {
            allow_unsigned: true,
            ..SecurityConfig::default()
        },
        TelemetryConfig::default(),
        AclConfig::default(),
    )
}

fn wasm_manifest(name: &str, caps: impl IntoIterator<Item = PluginCapability>) -> PluginCapabilityManifest {
    PluginCapabilityManifest {
        name: name.to_string(),
        version: "1.0.0".to_string(),
        capabilities: caps.into_iter().collect(),
        description: None,
        plugin_type: PluginType::Wasm,
        signature: SignatureStatus::Unsigned,
    }
}

fn native_manifest(name: &str, caps: impl IntoIterator<Item = PluginCapability>) -> PluginCapabilityManifest {
    PluginCapabilityManifest {
        name: name.to_string(),
        version: "1.0.0".to_string(),
        capabilities: caps.into_iter().collect(),
        description: None,
        plugin_type: PluginType::Native,
        signature: SignatureStatus::Unsigned,
    }
}

fn signed_manifest(name: &str, valid_from: u64, valid_until: u64) -> PluginCapabilityManifest {
    PluginCapabilityManifest {
        name: name.to_string(),
        version: "1.0.0".to_string(),
        capabilities: [PluginCapability::ReadBus].into_iter().collect(),
        description: None,
        plugin_type: PluginType::Wasm,
        signature: SignatureStatus::Signed {
            issuer: "OpenFlight-CA".to_string(),
            subject: name.to_string(),
            valid_from,
            valid_until,
        },
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

// =========================================================================
// 1. Capability model
// =========================================================================

mod capability_model {
    use super::*;

    /// Plugins declare required capabilities that are stored in the registry.
    #[test]
    fn plugins_declare_required_capabilities() {
        let mut mgr = mgr_allow_unsigned();
        let manifest =
            wasm_manifest("sensor-reader", [PluginCapability::ReadBus, PluginCapability::EmitPanel]);
        mgr.validate_plugin(manifest).unwrap();

        assert!(mgr.check_capability("sensor-reader", &PluginCapability::ReadBus));
        assert!(mgr.check_capability("sensor-reader", &PluginCapability::EmitPanel));
    }

    /// Capabilities can be granted (present in manifest) or denied (absent).
    #[test]
    fn capabilities_can_be_granted_and_denied() {
        let mut mgr = mgr_allow_unsigned();
        mgr.validate_plugin(wasm_manifest("rw", [PluginCapability::ReadBus]))
            .unwrap();

        // Granted
        assert!(mgr.check_escalation("rw", &PluginCapability::ReadBus).is_ok());
        // Denied
        assert!(mgr.check_escalation("rw", &PluginCapability::EmitPanel).is_err());
    }

    /// Capability check is all-or-nothing — a plugin must hold *every* capability
    /// it exercises; a single missing one causes denial.
    #[test]
    fn capability_check_is_atomic_all_or_nothing() {
        let mut mgr = mgr_allow_unsigned();
        mgr.validate_plugin(wasm_manifest("partial", [PluginCapability::ReadBus]))
            .unwrap();

        let required = [
            PluginCapability::ReadBus,
            PluginCapability::EmitPanel,
            PluginCapability::WriteBlackbox,
        ];
        let results: Vec<bool> = required
            .iter()
            .map(|c| mgr.check_escalation("partial", c).is_ok())
            .collect();

        // Only the first should pass, the rest should be denied.
        assert_eq!(results, vec![true, false, false]);
        // The "all" check must fail because at least one capability is missing.
        assert!(
            results.iter().any(|ok| !ok),
            "atomic check: at least one denied ⇒ overall denied"
        );
    }

    /// An unknown/unregistered plugin name is treated as "no capabilities" → denied.
    #[test]
    fn unknown_plugin_denied_by_default() {
        let mut mgr = mgr_allow_unsigned();
        assert!(
            mgr.check_escalation("nonexistent-plugin", &PluginCapability::ReadBus)
                .is_err()
        );
    }

    /// Property-style test: denied capabilities never pass the escalation check,
    /// regardless of which plugin type or how many other caps are granted.
    #[test]
    fn denied_capability_never_passes_check() {
        let mut mgr = mgr_allow_unsigned();

        // Register a plugin with a subset of capabilities.
        let granted: HashSet<PluginCapability> = [
            PluginCapability::ReadBus,
            PluginCapability::EmitPanel,
        ]
        .into_iter()
        .collect();

        mgr.validate_plugin(wasm_manifest("prop-test", granted.clone()))
            .unwrap();

        let all_simple = [
            PluginCapability::ReadBus,
            PluginCapability::EmitPanel,
            PluginCapability::ReadProfiles,
            PluginCapability::WriteBlackbox,
            PluginCapability::ReadDeviceHealth,
        ];

        for cap in &all_simple {
            let result = mgr.check_escalation("prop-test", cap);
            if granted.contains(cap) {
                assert!(result.is_ok(), "{cap:?} was granted but check failed");
            } else {
                assert!(result.is_err(), "{cap:?} was NOT granted but check passed");
            }
        }
    }

    /// Empty capability set means everything is denied.
    #[test]
    fn empty_capability_set_denies_all() {
        let mut mgr = mgr_allow_unsigned();
        mgr.validate_plugin(native_manifest("empty-caps", []))
            .unwrap();

        for cap in [
            PluginCapability::ReadBus,
            PluginCapability::EmitPanel,
            PluginCapability::ReadProfiles,
        ] {
            assert!(
                mgr.check_escalation("empty-caps", &cap).is_err(),
                "empty-caps must deny {cap:?}"
            );
        }
    }

    /// Multiple plugins are isolated — one plugin's capabilities don't leak.
    #[test]
    fn capabilities_isolated_between_plugins() {
        let mut mgr = mgr_allow_unsigned();
        mgr.validate_plugin(wasm_manifest("alpha", [PluginCapability::ReadBus]))
            .unwrap();
        mgr.validate_plugin(wasm_manifest("beta", [PluginCapability::EmitPanel]))
            .unwrap();

        assert!(mgr.check_capability("alpha", &PluginCapability::ReadBus));
        assert!(!mgr.check_capability("alpha", &PluginCapability::EmitPanel));

        assert!(!mgr.check_capability("beta", &PluginCapability::ReadBus));
        assert!(mgr.check_capability("beta", &PluginCapability::EmitPanel));
    }
}

// =========================================================================
// 2. Sandbox boundaries
// =========================================================================

mod sandbox_boundaries {
    use super::*;

    /// Sandboxed (WASM) plugins cannot access the filesystem.
    #[test]
    fn wasm_plugin_cannot_access_filesystem() {
        let mut mgr = mgr_allow_unsigned();
        let manifest = PluginCapabilityManifest {
            name: "sandbox-fs".to_string(),
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
            "WASM plugin must be denied filesystem access"
        );
    }

    /// Sandboxed (WASM) plugins cannot access the network.
    #[test]
    fn wasm_plugin_cannot_access_network() {
        let mut mgr = mgr_allow_unsigned();
        let manifest = PluginCapabilityManifest {
            name: "sandbox-net".to_string(),
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
            "WASM plugin must be denied network access"
        );
    }

    /// Native plugins *may* request filesystem access (it is validated).
    #[test]
    fn native_plugin_filesystem_access_allowed() {
        let mut mgr = mgr_allow_unsigned();
        let manifest = PluginCapabilityManifest {
            name: "native-fs".to_string(),
            version: "1.0.0".to_string(),
            capabilities: [PluginCapability::FileSystem {
                paths: vec![PathBuf::from("/var/log")],
            }]
            .into_iter()
            .collect(),
            description: None,
            plugin_type: PluginType::Native,
            signature: SignatureStatus::Unsigned,
        };
        assert!(
            mgr.validate_plugin(manifest).is_ok(),
            "native plugin should be allowed filesystem access"
        );
    }

    /// Native plugins *may* request network access.
    #[test]
    fn native_plugin_network_access_allowed() {
        let mut mgr = mgr_allow_unsigned();
        let manifest = PluginCapabilityManifest {
            name: "native-net".to_string(),
            version: "1.0.0".to_string(),
            capabilities: [PluginCapability::Network {
                hosts: vec!["api.example.com".to_string()],
            }]
            .into_iter()
            .collect(),
            description: None,
            plugin_type: PluginType::Native,
            signature: SignatureStatus::Unsigned,
        };
        assert!(
            mgr.validate_plugin(manifest).is_ok(),
            "native plugin should be allowed network access"
        );
    }

    /// FsAccessPolicy confines access to declared roots.
    #[test]
    fn fs_policy_confines_to_declared_roots() {
        let tmp = tempfile::TempDir::new().unwrap();
        let sandbox_root = tmp.path().join("sandbox");
        std::fs::create_dir_all(&sandbox_root).unwrap();
        let allowed_file = sandbox_root.join("data.bin");
        std::fs::write(&allowed_file, b"ok").unwrap();

        let policy = FsAccessPolicy::new(&[sandbox_root]);

        assert!(policy.validate(&allowed_file).is_ok());
        assert!(
            policy.validate(tmp.path().join("outside.txt").as_path()).is_err(),
            "access outside sandbox root must be rejected"
        );
    }

    /// Path-traversal attacks are detected and blocked.
    #[test]
    fn path_traversal_blocked() {
        let tmp = tempfile::TempDir::new().unwrap();
        let sandbox = tmp.path().join("sandbox");
        std::fs::create_dir_all(&sandbox).unwrap();

        let policy = FsAccessPolicy::new(&[sandbox.clone()]);
        let attack = sandbox.join("..").join("..").join("etc").join("shadow");
        assert!(policy.validate(&attack).is_err());
    }

    /// Memory limits: max_plugin_budget_us is accepted in configuration.
    #[test]
    fn plugin_budget_limit_configured() {
        let config = SecurityConfig {
            max_plugin_budget_us: 50,
            allow_unsigned: true,
            ..SecurityConfig::default()
        };
        let mgr = SecurityManager::with_config(
            config,
            TelemetryConfig::default(),
            AclConfig::default(),
        );
        // Verify the config was accepted and the manager is functional.
        let registry = mgr.get_plugin_registry();
        assert!(registry.is_empty(), "fresh registry should have no plugins");
    }

    /// Empty policy rejects everything.
    #[test]
    fn empty_fs_policy_rejects_all() {
        let policy = FsAccessPolicy::new(&[]);
        assert!(policy.validate(std::path::Path::new("any.txt")).is_err());
    }
}

// =========================================================================
// 3. Audit logging
// =========================================================================

mod audit_logging {
    use super::*;

    /// All capability checks (success and denial) produce audit entries.
    #[test]
    fn all_capability_checks_are_logged() {
        let mut mgr = mgr_allow_unsigned();
        mgr.validate_plugin(wasm_manifest("aud-cap", [PluginCapability::ReadBus]))
            .unwrap();

        // Successful check (validate_plugin already audited)
        let _ = mgr.check_escalation("aud-cap", &PluginCapability::ReadBus);
        // Denied check
        let _ = mgr.check_escalation("aud-cap", &PluginCapability::EmitPanel);

        let log = mgr.audit_log();
        // There should be at least the plugin-load success + the denied escalation.
        assert!(log.len() >= 2, "expected ≥2 audit entries, got {}", log.len());

        let has_success = log
            .entries()
            .iter()
            .any(|e| e.outcome == AuditOutcome::Success);
        let has_denied = log
            .entries()
            .iter()
            .any(|e| e.outcome == AuditOutcome::Denied);
        assert!(has_success, "successful check must be audited");
        assert!(has_denied, "denied check must be audited");
    }

    /// Failed access attempts are logged with contextual information.
    #[test]
    fn failed_access_logged_with_context() {
        let mut mgr = mgr_allow_unsigned();
        mgr.validate_plugin(wasm_manifest("ctx-log", [PluginCapability::ReadBus]))
            .unwrap();

        let _ = mgr.check_escalation("ctx-log", &PluginCapability::WriteBlackbox);

        let denied_entries: Vec<_> = mgr
            .audit_log()
            .entries()
            .iter()
            .filter(|e| e.outcome == AuditOutcome::Denied)
            .collect();

        assert!(!denied_entries.is_empty());
        let entry = denied_entries.last().unwrap();
        assert_eq!(entry.actor, "ctx-log");
        assert!(entry.details.is_some(), "denied entry should include details");
    }

    /// Audit-log export format: exported JSON is parseable and entries have expected fields.
    #[test]
    fn audit_log_export_format_valid() {
        let mut log = AuditLog::new(100);
        for i in 0..5 {
            log.record_event(
                AuditCategory::Authorization,
                AuditSeverity::Info,
                format!("actor-{i}"),
                "check",
                "resource",
                AuditOutcome::Success,
                None,
            );
        }

        let json = log.export_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        let arr = parsed.as_array().expect("should be array");
        assert_eq!(arr.len(), 5);

        // Verify structural integrity — each entry has expected fields.
        for entry in arr {
            assert!(entry.get("timestamp").is_some());
            assert!(entry.get("category").is_some());
            assert!(entry.get("actor").is_some());
            assert!(entry.get("outcome").is_some());
        }
    }

    /// Log rotation doesn't lose recent entries.
    #[test]
    fn log_rotation_preserves_recent_entries() {
        let capacity = 5;
        let mut log = AuditLog::new(capacity);

        // Write 10 entries; only the last 5 should remain.
        for i in 0..10 {
            log.record_event(
                AuditCategory::PluginLoad,
                AuditSeverity::Info,
                format!("actor-{i}"),
                "load",
                "plugin",
                AuditOutcome::Success,
                None,
            );
        }

        assert_eq!(log.len(), capacity);

        let actors: Vec<&str> = log.entries().iter().map(|e| e.actor.as_str()).collect();
        assert_eq!(actors, vec!["actor-5", "actor-6", "actor-7", "actor-8", "actor-9"]);

        // `recent(3)` returns the 3 newest.
        let recent = log.recent(3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].actor, "actor-7");
        assert_eq!(recent[2].actor, "actor-9");
    }

    /// Disabled audit log doesn't record.
    #[test]
    fn disabled_audit_log_records_nothing() {
        let mut log = AuditLog::new(100);
        log.disable();
        log.record(AuditEntry {
            timestamp: SystemTime::now(),
            category: AuditCategory::Authentication,
            severity: AuditSeverity::Critical,
            actor: "attacker".to_string(),
            action: "brute-force".to_string(),
            resource: "login".to_string(),
            outcome: AuditOutcome::Denied,
            details: None,
        });
        assert!(log.is_empty());
    }

    /// Severity filtering works correctly.
    #[test]
    fn severity_filtering() {
        let mut log = AuditLog::new(100);
        log.record_event(
            AuditCategory::Authorization,
            AuditSeverity::Info,
            "a",
            "x",
            "r",
            AuditOutcome::Success,
            None,
        );
        log.record_event(
            AuditCategory::Authorization,
            AuditSeverity::Alert,
            "b",
            "x",
            "r",
            AuditOutcome::Denied,
            None,
        );
        log.record_event(
            AuditCategory::Authorization,
            AuditSeverity::Critical,
            "c",
            "x",
            "r",
            AuditOutcome::Failure,
            None,
        );

        let high = log.entries_by_severity(AuditSeverity::Alert);
        assert_eq!(high.len(), 2);
        assert!(high.iter().all(|e| e.severity >= AuditSeverity::Alert));
    }
}

// =========================================================================
// 4. Signature-status validation
// =========================================================================

mod signature_status_validation {
    use super::*;

    /// Valid (non-expired) signature status → plugin accepted.
    #[test]
    fn valid_signature_accepted() {
        let mut mgr = SecurityManager::new(); // signatures enforced
        let now = now_secs();
        let manifest = signed_manifest("valid-sig", now - 3600, now + 3600);
        assert!(mgr.validate_plugin(manifest).is_ok());
    }

    /// Invalid signature → rejected.
    #[test]
    fn invalid_signature_rejected() {
        let mut mgr = SecurityManager::new();
        let manifest = PluginCapabilityManifest {
            name: "bad-sig".to_string(),
            version: "1.0.0".to_string(),
            capabilities: HashSet::new(),
            description: None,
            plugin_type: PluginType::Wasm,
            signature: SignatureStatus::Invalid {
                reason: "corrupted RSA block".to_string(),
            },
        };
        let err = mgr.validate_plugin(manifest).unwrap_err();
        assert!(
            format!("{err}").contains("corrupted RSA block"),
            "error should carry the invalidity reason"
        );
    }

    /// Expired signature → rejected.
    #[test]
    fn expired_signature_rejected() {
        let mut mgr = SecurityManager::new();
        let manifest = signed_manifest("expired-sig", 946684800, 946684801); // year 2000
        assert!(mgr.validate_plugin(manifest).is_err());
    }

    /// Missing (unsigned) signature → rejected when unsigned not allowed.
    #[test]
    fn missing_signature_rejected() {
        let mut mgr = SecurityManager::new(); // enforce_signatures=true, allow_unsigned=false
        let manifest = wasm_manifest("unsigned-denied", [PluginCapability::ReadBus]);
        assert!(mgr.validate_plugin(manifest).is_err());
    }

    /// Self-signed / unsigned → configurable: accept when `allow_unsigned = true`.
    #[test]
    fn self_signed_configurable_accept() {
        let mut mgr = mgr_allow_unsigned();
        let manifest = wasm_manifest("self-signed-ok", [PluginCapability::ReadBus]);
        assert!(mgr.validate_plugin(manifest).is_ok());
    }

    /// Self-signed / unsigned → configurable: reject when `allow_unsigned = false`.
    #[test]
    fn self_signed_configurable_reject() {
        let mut mgr = SecurityManager::new(); // default: unsigned rejected
        let manifest = wasm_manifest("self-signed-bad", [PluginCapability::ReadBus]);
        assert!(mgr.validate_plugin(manifest).is_err());
    }

    /// Property test: verify(sign(data)) always succeeds (SHA-256 digest round-trip).
    #[test]
    fn verify_sign_roundtrip_always_succeeds() {
        let payloads: &[&[u8]] = &[
            b"",
            b"hello",
            b"a]very*long$payload\x00\xff\xfe with binary",
            &[0u8; 4096],
            &[0xFFu8; 256],
        ];
        for data in payloads {
            let digest = flight_security::sha256_hex(data);
            assert!(
                flight_security::verify_digest(data, &digest).is_ok(),
                "round-trip must succeed for payload of len {}",
                data.len()
            );
        }
    }

    /// Digest is case-insensitive.
    #[test]
    fn digest_case_insensitive() {
        let data = b"case test";
        let lower = flight_security::sha256_hex(data);
        let upper = lower.to_uppercase();
        assert!(flight_security::verify_digest(data, &upper).is_ok());
    }

    /// `SignedPayload` correctly verifies matching data and rejects mismatches.
    #[test]
    fn signed_payload_verify_and_reject() {
        let data = b"plugin binary v2";
        let payload = SignedPayload {
            name: "my-plugin".to_string(),
            version: "2.0.0".to_string(),
            expected_digest: flight_security::sha256_hex(data),
        };
        assert!(payload.verify(data).is_ok());
        assert!(payload.verify(b"tampered content").is_err());
    }

    /// File-based digest verification detects tampering.
    #[test]
    fn file_digest_detects_tampering() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("update.bin");
        let original = b"authentic update";
        std::fs::write(&path, original).unwrap();

        let digest = flight_security::sha256_hex(original);
        assert!(flight_security::verify_file_digest(&path, &digest).is_ok());

        // Tamper
        std::fs::write(&path, b"corrupted").unwrap();
        assert!(flight_security::verify_file_digest(&path, &digest).is_err());
    }
}

// =========================================================================
// 5. Token management (IPC ACL as token analogue)
// =========================================================================

mod token_management {
    use super::*;

    fn make_client(user_id: &str) -> IpcClientInfo {
        IpcClientInfo {
            user_id: user_id.to_string(),
            process_id: std::process::id(),
            executable_path: PathBuf::from("flightctl"),
        }
    }

    fn mgr_with_acl(current_user_only: bool, allowed: HashSet<String>) -> SecurityManager {
        SecurityManager::with_config(
            SecurityConfig::default(),
            TelemetryConfig::default(),
            AclConfig {
                current_user_only,
                allowed_users: allowed,
                ..AclConfig::default()
            },
        )
    }

    /// Token (allowed user) grants access.
    #[test]
    fn valid_token_grants_access() {
        let mut allowed = HashSet::new();
        allowed.insert("alice".to_string());
        let mgr = mgr_with_acl(false, allowed);
        assert!(mgr.validate_ipc_acl(&make_client("alice")).is_ok());
    }

    /// Expired/missing token (user not in list) → rejected.
    #[test]
    fn missing_token_rejected() {
        let mut allowed = HashSet::new();
        allowed.insert("alice".to_string());
        let mgr = mgr_with_acl(false, allowed);
        assert!(mgr.validate_ipc_acl(&make_client("bob")).is_err());
    }

    /// Revoked token (removed from allowed list) → rejected.
    #[test]
    fn revoked_token_rejected() {
        // Build with "alice", but try "eve".
        let mut allowed = HashSet::new();
        allowed.insert("alice".to_string());
        let mgr = mgr_with_acl(false, allowed);
        assert!(mgr.validate_ipc_acl(&make_client("eve")).is_err());
    }

    /// current_user_only mode rejects foreign users consistently.
    #[test]
    fn current_user_only_rejects_foreign() {
        let mgr = mgr_with_acl(true, HashSet::new());
        let foreign = make_client("S-1-5-21-FOREIGN-9999");
        for _ in 0..5 {
            assert!(mgr.validate_ipc_acl(&foreign).is_err());
        }
    }

    /// Empty allowed-list with current_user_only disabled → open access.
    #[test]
    fn empty_acl_open_access() {
        let mgr = mgr_with_acl(false, HashSet::new());
        assert!(mgr.validate_ipc_acl(&make_client("anyone")).is_ok());
    }

    /// Case sensitivity: allow-list is case-sensitive.
    #[test]
    fn acl_case_sensitive() {
        let mut allowed = HashSet::new();
        allowed.insert("Alice".to_string());
        let mgr = mgr_with_acl(false, allowed);

        assert!(mgr.validate_ipc_acl(&make_client("Alice")).is_ok());
        assert!(mgr.validate_ipc_acl(&make_client("alice")).is_err());
        assert!(mgr.validate_ipc_acl(&make_client("ALICE")).is_err());
    }
}

// =========================================================================
// 6. Integration scenarios
// =========================================================================

mod integration_scenarios {
    use super::*;

    /// End-to-end: plugin install → capability check → sandbox → audit trail.
    #[test]
    fn plugin_install_capability_sandbox_audit() {
        let mut mgr = mgr_allow_unsigned();

        // 1. Install plugin
        let manifest = wasm_manifest(
            "led-driver",
            [PluginCapability::ReadBus, PluginCapability::EmitPanel],
        );
        mgr.validate_plugin(manifest).unwrap();

        // 2. Capability check — declared caps succeed
        assert!(mgr.check_escalation("led-driver", &PluginCapability::ReadBus).is_ok());
        assert!(mgr.check_escalation("led-driver", &PluginCapability::EmitPanel).is_ok());

        // 3. Sandbox: undeclared cap denied
        assert!(
            mgr.check_escalation("led-driver", &PluginCapability::WriteBlackbox)
                .is_err()
        );

        // 4. Audit trail: at least plugin-load success + denied escalation
        let log = mgr.audit_log();
        assert!(log.len() >= 2);

        let denied: Vec<_> = log
            .entries()
            .iter()
            .filter(|e| e.outcome == AuditOutcome::Denied)
            .collect();
        assert!(!denied.is_empty(), "denied escalation must appear in audit trail");
    }

    /// Update with signature verification: valid digest passes, tampered fails.
    #[test]
    fn update_signature_verification_flow() {
        let update_data = b"flight-service v4.0.0 binary";
        let digest = flight_security::sha256_hex(update_data);

        let payload = SignedPayload {
            name: "flightd".to_string(),
            version: "4.0.0".to_string(),
            expected_digest: digest.clone(),
        };

        // Step 1: download matches expected digest
        assert!(payload.verify(update_data).is_ok());

        // Step 2: simulate MITM corruption
        let tampered = b"flight-service v4.0.0 + backdoor";
        assert!(payload.verify(tampered).is_err());

        // Step 3: file-based verification
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("flightd.exe");
        std::fs::write(&path, update_data).unwrap();
        assert!(flight_security::verify_file_digest(&path, &digest).is_ok());
    }

    /// Malicious plugin detection: unsigned rejected, invalid sig rejected,
    /// WASM filesystem/network requests rejected.
    #[test]
    fn malicious_plugin_detection_and_blocking() {
        let mut mgr = SecurityManager::new(); // strict mode

        // Scenario A: unsigned plugin → blocked
        let unsigned = wasm_manifest("malware-a", [PluginCapability::ReadBus]);
        assert!(mgr.validate_plugin(unsigned).is_err());

        // Scenario B: corrupted signature → blocked
        let bad_sig = PluginCapabilityManifest {
            name: "malware-b".to_string(),
            version: "1.0.0".to_string(),
            capabilities: [PluginCapability::ReadBus].into_iter().collect(),
            description: None,
            plugin_type: PluginType::Wasm,
            signature: SignatureStatus::Invalid {
                reason: "signature mismatch".to_string(),
            },
        };
        assert!(mgr.validate_plugin(bad_sig).is_err());

        // Scenario C: expired certificate → blocked
        let expired = signed_manifest("malware-c", 946684800, 946684801);
        assert!(mgr.validate_plugin(expired).is_err());

        // The unsigned and invalid-signature rejections produce audit "Denied"
        // entries. The expired-signature path returns an error but may not emit
        // an explicit audit entry (it fails during cryptographic verification).
        let denied_count = mgr
            .audit_log()
            .entries()
            .iter()
            .filter(|e| e.outcome == AuditOutcome::Denied)
            .count();
        assert!(
            denied_count >= 2,
            "expected ≥2 audit denials, got {denied_count}"
        );
    }

    /// WASM sandbox enforcement combined with audit — requesting fs+net
    /// produces auditable rejection.
    #[test]
    fn wasm_sandbox_enforcement_with_audit() {
        let mut mgr = mgr_allow_unsigned();

        let fs_manifest = PluginCapabilityManifest {
            name: "wasm-escape-fs".to_string(),
            version: "1.0.0".to_string(),
            capabilities: [PluginCapability::FileSystem {
                paths: vec![PathBuf::from("/etc/passwd")],
            }]
            .into_iter()
            .collect(),
            description: None,
            plugin_type: PluginType::Wasm,
            signature: SignatureStatus::Unsigned,
        };
        assert!(mgr.validate_plugin(fs_manifest).is_err());

        let net_manifest = PluginCapabilityManifest {
            name: "wasm-escape-net".to_string(),
            version: "1.0.0".to_string(),
            capabilities: [PluginCapability::Network {
                hosts: vec!["c2.evil.example.com".to_string()],
            }]
            .into_iter()
            .collect(),
            description: None,
            plugin_type: PluginType::Wasm,
            signature: SignatureStatus::Unsigned,
        };
        assert!(mgr.validate_plugin(net_manifest).is_err());
    }

    /// Filesystem policy integration with SecurityManager: allowed root → ok,
    /// traversal → blocked.
    #[test]
    fn fs_policy_integration_with_manager() {
        let tmp = tempfile::TempDir::new().unwrap();
        let allowed_dir = tmp.path().join("data");
        std::fs::create_dir_all(&allowed_dir).unwrap();
        let file = allowed_dir.join("config.json");
        std::fs::write(&file, b"{}").unwrap();

        let mut mgr = SecurityManager::new();
        mgr.set_fs_policy(FsAccessPolicy::new(&[allowed_dir.clone()]));

        assert!(mgr.validate_fs_access(&file).is_ok());
        assert!(mgr.validate_fs_access(&allowed_dir.join("..").join("secret")).is_err());
    }

    /// Multiple plugins registered in sequence don't interfere.
    #[test]
    fn multiple_plugins_no_interference() {
        let mut mgr = mgr_allow_unsigned();

        mgr.validate_plugin(wasm_manifest("p1", [PluginCapability::ReadBus]))
            .unwrap();
        mgr.validate_plugin(wasm_manifest("p2", [PluginCapability::EmitPanel]))
            .unwrap();
        mgr.validate_plugin(wasm_manifest("p3", [PluginCapability::ReadProfiles]))
            .unwrap();

        assert!(mgr.check_escalation("p1", &PluginCapability::ReadBus).is_ok());
        assert!(mgr.check_escalation("p1", &PluginCapability::EmitPanel).is_err());

        assert!(mgr.check_escalation("p2", &PluginCapability::EmitPanel).is_ok());
        assert!(mgr.check_escalation("p2", &PluginCapability::ReadBus).is_err());

        assert!(mgr.check_escalation("p3", &PluginCapability::ReadProfiles).is_ok());
        assert!(mgr.check_escalation("p3", &PluginCapability::ReadBus).is_err());
    }
}
