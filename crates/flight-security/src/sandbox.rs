// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Plugin sandbox policy engine (REQ-932).
//!
//! Declares fine-grained capabilities that plugins may request and enforces
//! the principle of least privilege: everything not explicitly granted is
//! denied.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Individual capability a plugin may request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SandboxCapability {
    ReadAxes,
    WriteAxes,
    ReadTelemetry,
    AccessHID,
    NetworkAccess,
}

/// A sandbox policy that declares which capabilities a plugin is granted.
///
/// Deny-by-default: only capabilities present in `granted` are allowed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxPolicy {
    /// Human-readable plugin identifier.
    pub plugin_name: String,
    /// Capabilities explicitly granted to the plugin.
    granted: HashSet<SandboxCapability>,
}

impl SandboxPolicy {
    /// Create a policy that grants nothing.
    pub fn deny_all(name: impl Into<String>) -> Self {
        Self {
            plugin_name: name.into(),
            granted: HashSet::new(),
        }
    }

    /// Create a policy with a pre-defined set of capabilities.
    pub fn with_capabilities(
        name: impl Into<String>,
        caps: impl IntoIterator<Item = SandboxCapability>,
    ) -> Self {
        Self {
            plugin_name: name.into(),
            granted: caps.into_iter().collect(),
        }
    }

    /// Grant an additional capability.
    pub fn grant(&mut self, cap: SandboxCapability) {
        self.granted.insert(cap);
    }

    /// Revoke a capability.
    pub fn revoke(&mut self, cap: SandboxCapability) {
        self.granted.remove(&cap);
    }

    /// Check if a specific capability is granted.
    pub fn is_granted(&self, cap: SandboxCapability) -> bool {
        self.granted.contains(&cap)
    }

    /// Number of granted capabilities.
    pub fn granted_count(&self) -> usize {
        self.granted.len()
    }

    /// Iterate over granted capabilities.
    pub fn granted_capabilities(&self) -> impl Iterator<Item = &SandboxCapability> {
        self.granted.iter()
    }
}

/// Stateless capability checker.
pub struct CapabilityCheck;

impl CapabilityCheck {
    /// Returns `true` only if `policy` explicitly grants `cap`.
    pub fn check(cap: SandboxCapability, policy: &SandboxPolicy) -> bool {
        policy.is_granted(cap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Deny-by-default ---

    #[test]
    fn test_deny_all_has_no_capabilities() {
        let policy = SandboxPolicy::deny_all("test");
        assert_eq!(policy.granted_count(), 0);
    }

    #[test]
    fn test_deny_all_rejects_every_capability() {
        let policy = SandboxPolicy::deny_all("test");
        assert!(!CapabilityCheck::check(
            SandboxCapability::ReadAxes,
            &policy
        ));
        assert!(!CapabilityCheck::check(
            SandboxCapability::WriteAxes,
            &policy
        ));
        assert!(!CapabilityCheck::check(
            SandboxCapability::ReadTelemetry,
            &policy
        ));
        assert!(!CapabilityCheck::check(
            SandboxCapability::AccessHID,
            &policy
        ));
        assert!(!CapabilityCheck::check(
            SandboxCapability::NetworkAccess,
            &policy
        ));
    }

    // --- Granting and revoking ---

    #[test]
    fn test_grant_capability() {
        let mut policy = SandboxPolicy::deny_all("p");
        policy.grant(SandboxCapability::ReadAxes);
        assert!(CapabilityCheck::check(SandboxCapability::ReadAxes, &policy));
        assert_eq!(policy.granted_count(), 1);
    }

    #[test]
    fn test_revoke_capability() {
        let mut policy = SandboxPolicy::with_capabilities("p", [SandboxCapability::ReadAxes]);
        assert!(CapabilityCheck::check(SandboxCapability::ReadAxes, &policy));
        policy.revoke(SandboxCapability::ReadAxes);
        assert!(!CapabilityCheck::check(
            SandboxCapability::ReadAxes,
            &policy
        ));
    }

    #[test]
    fn test_revoke_ungrant_is_noop() {
        let mut policy = SandboxPolicy::deny_all("p");
        policy.revoke(SandboxCapability::NetworkAccess); // never granted
        assert_eq!(policy.granted_count(), 0);
    }

    // --- with_capabilities ---

    #[test]
    fn test_with_capabilities_grants_listed() {
        let policy = SandboxPolicy::with_capabilities(
            "multi",
            [
                SandboxCapability::ReadAxes,
                SandboxCapability::ReadTelemetry,
            ],
        );
        assert!(policy.is_granted(SandboxCapability::ReadAxes));
        assert!(policy.is_granted(SandboxCapability::ReadTelemetry));
        assert!(!policy.is_granted(SandboxCapability::WriteAxes));
        assert_eq!(policy.granted_count(), 2);
    }

    // --- Individual capability checks ---

    #[test]
    fn test_check_read_axes() {
        let policy = SandboxPolicy::with_capabilities("p", [SandboxCapability::ReadAxes]);
        assert!(CapabilityCheck::check(SandboxCapability::ReadAxes, &policy));
        assert!(!CapabilityCheck::check(
            SandboxCapability::WriteAxes,
            &policy
        ));
    }

    #[test]
    fn test_check_write_axes() {
        let policy = SandboxPolicy::with_capabilities("p", [SandboxCapability::WriteAxes]);
        assert!(CapabilityCheck::check(
            SandboxCapability::WriteAxes,
            &policy
        ));
        assert!(!CapabilityCheck::check(
            SandboxCapability::ReadAxes,
            &policy
        ));
    }

    #[test]
    fn test_check_read_telemetry() {
        let policy = SandboxPolicy::with_capabilities("p", [SandboxCapability::ReadTelemetry]);
        assert!(CapabilityCheck::check(
            SandboxCapability::ReadTelemetry,
            &policy
        ));
    }

    #[test]
    fn test_check_access_hid() {
        let policy = SandboxPolicy::with_capabilities("p", [SandboxCapability::AccessHID]);
        assert!(CapabilityCheck::check(
            SandboxCapability::AccessHID,
            &policy
        ));
    }

    #[test]
    fn test_check_network_access() {
        let policy = SandboxPolicy::with_capabilities("p", [SandboxCapability::NetworkAccess]);
        assert!(CapabilityCheck::check(
            SandboxCapability::NetworkAccess,
            &policy
        ));
    }

    // --- Serialization round-trip ---

    #[test]
    fn test_policy_json_round_trip() {
        let policy = SandboxPolicy::with_capabilities(
            "roundtrip",
            [
                SandboxCapability::ReadAxes,
                SandboxCapability::WriteAxes,
                SandboxCapability::NetworkAccess,
            ],
        );
        let json = serde_json::to_string(&policy).unwrap();
        let restored: SandboxPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(policy, restored);
    }

    #[test]
    fn test_empty_policy_json_round_trip() {
        let policy = SandboxPolicy::deny_all("empty");
        let json = serde_json::to_string(&policy).unwrap();
        let restored: SandboxPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(policy, restored);
    }

    #[test]
    fn test_all_capabilities_json_round_trip() {
        let policy = SandboxPolicy::with_capabilities(
            "full",
            [
                SandboxCapability::ReadAxes,
                SandboxCapability::WriteAxes,
                SandboxCapability::ReadTelemetry,
                SandboxCapability::AccessHID,
                SandboxCapability::NetworkAccess,
            ],
        );
        let json = serde_json::to_string(&policy).unwrap();
        let restored: SandboxPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(policy, restored);
    }

    // --- Property-style tests ---

    #[test]
    fn test_granting_same_capability_twice_is_idempotent() {
        let mut policy = SandboxPolicy::deny_all("dup");
        policy.grant(SandboxCapability::ReadAxes);
        policy.grant(SandboxCapability::ReadAxes);
        assert_eq!(policy.granted_count(), 1);
    }

    #[test]
    fn test_granted_capabilities_iterator() {
        let policy = SandboxPolicy::with_capabilities(
            "iter",
            [SandboxCapability::ReadAxes, SandboxCapability::AccessHID],
        );
        let caps: HashSet<_> = policy.granted_capabilities().copied().collect();
        assert!(caps.contains(&SandboxCapability::ReadAxes));
        assert!(caps.contains(&SandboxCapability::AccessHID));
        assert_eq!(caps.len(), 2);
    }

    #[test]
    fn test_plugin_name_preserved() {
        let policy = SandboxPolicy::deny_all("my-plugin-v2");
        assert_eq!(policy.plugin_name, "my-plugin-v2");
    }
}
