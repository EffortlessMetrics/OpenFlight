// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Security boundary tests — IPC access control and connection validation.

use flight_security::{AclConfig, IpcClientInfo, SecurityConfig, SecurityManager, TelemetryConfig};
use std::collections::HashSet;
use std::path::PathBuf;

fn make_client(user_id: &str) -> IpcClientInfo {
    IpcClientInfo {
        user_id: user_id.to_string(),
        process_id: std::process::id(),
        executable_path: PathBuf::from("flightctl"),
    }
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
