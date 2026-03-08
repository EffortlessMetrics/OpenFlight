// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Plugin manifest validation and discovery (ADR-003).
//!
//! Validates plugin manifests against tier-specific constraints and provides
//! a discovery registry for tracking available plugins by name and version.

use std::collections::HashMap;

use crate::plugin::PluginTier;

/// Metadata describing a plugin, loaded from its manifest file.
#[derive(Debug, Clone)]
pub struct PluginManifest {
    /// Unique plugin name (reverse-domain style recommended).
    pub name: String,
    /// Semantic version string.
    pub version: String,
    /// Human-readable author.
    pub author: String,
    /// Short description.
    pub description: String,
    /// ADR-003 tier.
    pub tier: PluginTier,
    /// Capabilities the plugin requests from the host.
    pub capabilities_requested: Vec<String>,
}

/// Validate a manifest, returning a list of problems (empty = valid).
pub fn validate_manifest(manifest: &PluginManifest) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if manifest.name.is_empty() {
        errors.push("name must not be empty".into());
    }
    if manifest.version.is_empty() {
        errors.push("version must not be empty".into());
    }
    if manifest.author.is_empty() {
        errors.push("author must not be empty".into());
    }

    // Tier-specific capability checks (ADR-003).
    match manifest.tier {
        PluginTier::Wasm => {
            // WASM plugins may not request privileged capabilities.
            let forbidden = ["raw_hardware", "file_system", "network_bind"];
            for cap in &manifest.capabilities_requested {
                if forbidden.contains(&cap.as_str()) {
                    errors.push(format!(
                        "WASM tier plugin cannot request capability '{cap}'"
                    ));
                }
            }
        }
        PluginTier::Native => {
            // Native fast-path may not request network or file_system.
            let forbidden = ["network_bind", "file_system"];
            for cap in &manifest.capabilities_requested {
                if forbidden.contains(&cap.as_str()) {
                    errors.push(format!(
                        "Native fast-path plugin cannot request capability '{cap}'"
                    ));
                }
            }
        }
        PluginTier::Service => {
            // Service tier has access to all capabilities — no restrictions.
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Key used to de-duplicate plugins in the discovery registry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RegistryKey {
    name: String,
    version: String,
}

/// Discovery registry that tracks available plugin manifests.
#[derive(Debug, Default)]
pub struct PluginDiscoveryRegistry {
    manifests: HashMap<RegistryKey, PluginManifest>,
}

impl PluginDiscoveryRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a manifest. Returns `false` if the name+version is already present.
    pub fn register(&mut self, manifest: PluginManifest) -> bool {
        let key = RegistryKey {
            name: manifest.name.clone(),
            version: manifest.version.clone(),
        };
        if self.manifests.contains_key(&key) {
            return false;
        }
        self.manifests.insert(key, manifest);
        true
    }

    /// Remove a plugin by name and version. Returns the manifest if found.
    pub fn unregister(&mut self, name: &str, version: &str) -> Option<PluginManifest> {
        let key = RegistryKey {
            name: name.into(),
            version: version.into(),
        };
        self.manifests.remove(&key)
    }

    /// Look up a manifest by name and version.
    #[must_use]
    pub fn get(&self, name: &str, version: &str) -> Option<&PluginManifest> {
        let key = RegistryKey {
            name: name.into(),
            version: version.into(),
        };
        self.manifests.get(&key)
    }

    /// List all registered manifests.
    #[must_use]
    pub fn list(&self) -> Vec<&PluginManifest> {
        self.manifests.values().collect()
    }

    /// Number of registered manifests.
    #[must_use]
    pub fn len(&self) -> usize {
        self.manifests.len()
    }

    /// Whether the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.manifests.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wasm_manifest() -> PluginManifest {
        PluginManifest {
            name: "com.example.test".into(),
            version: "1.0.0".into(),
            author: "Test Author".into(),
            description: "A test plugin".into(),
            tier: PluginTier::Wasm,
            capabilities_requested: vec!["read_telemetry".into()],
        }
    }

    fn native_manifest() -> PluginManifest {
        PluginManifest {
            name: "com.example.native".into(),
            version: "2.0.0".into(),
            author: "Native Author".into(),
            description: "A native fast-path plugin".into(),
            tier: PluginTier::Native,
            capabilities_requested: vec!["read_telemetry".into(), "raw_hardware".into()],
        }
    }

    fn service_manifest() -> PluginManifest {
        PluginManifest {
            name: "com.example.service".into(),
            version: "3.0.0".into(),
            author: "Service Author".into(),
            description: "A service plugin".into(),
            tier: PluginTier::Service,
            capabilities_requested: vec![
                "read_telemetry".into(),
                "network_bind".into(),
                "file_system".into(),
            ],
        }
    }

    // ── Manifest validation ───────────────────────────────────────────

    #[test]
    fn valid_wasm_manifest() {
        assert!(validate_manifest(&wasm_manifest()).is_ok());
    }

    #[test]
    fn valid_native_manifest() {
        assert!(validate_manifest(&native_manifest()).is_ok());
    }

    #[test]
    fn valid_service_manifest() {
        assert!(validate_manifest(&service_manifest()).is_ok());
    }

    #[test]
    fn empty_name_rejected() {
        let mut m = wasm_manifest();
        m.name = String::new();
        let errs = validate_manifest(&m).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("name")));
    }

    #[test]
    fn empty_version_rejected() {
        let mut m = wasm_manifest();
        m.version = String::new();
        let errs = validate_manifest(&m).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("version")));
    }

    #[test]
    fn empty_author_rejected() {
        let mut m = wasm_manifest();
        m.author = String::new();
        let errs = validate_manifest(&m).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("author")));
    }

    #[test]
    fn multiple_validation_errors() {
        let mut m = wasm_manifest();
        m.name = String::new();
        m.version = String::new();
        m.author = String::new();
        let errs = validate_manifest(&m).unwrap_err();
        assert_eq!(errs.len(), 3);
    }

    #[test]
    fn wasm_tier_rejects_raw_hardware() {
        let mut m = wasm_manifest();
        m.capabilities_requested = vec!["raw_hardware".into()];
        let errs = validate_manifest(&m).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("raw_hardware")));
    }

    #[test]
    fn wasm_tier_rejects_file_system() {
        let mut m = wasm_manifest();
        m.capabilities_requested = vec!["file_system".into()];
        let errs = validate_manifest(&m).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("file_system")));
    }

    #[test]
    fn wasm_tier_rejects_network_bind() {
        let mut m = wasm_manifest();
        m.capabilities_requested = vec!["network_bind".into()];
        let errs = validate_manifest(&m).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("network_bind")));
    }

    #[test]
    fn native_tier_rejects_network_bind() {
        let mut m = native_manifest();
        m.capabilities_requested = vec!["network_bind".into()];
        let errs = validate_manifest(&m).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("network_bind")));
    }

    #[test]
    fn native_tier_allows_raw_hardware() {
        let mut m = native_manifest();
        m.capabilities_requested = vec!["raw_hardware".into()];
        assert!(validate_manifest(&m).is_ok());
    }

    #[test]
    fn service_tier_allows_all_capabilities() {
        let m = service_manifest();
        assert!(validate_manifest(&m).is_ok());
    }

    // ── Discovery registry ────────────────────────────────────────────

    #[test]
    fn registry_register_and_get() {
        let mut reg = PluginDiscoveryRegistry::new();
        assert!(reg.register(wasm_manifest()));
        assert_eq!(reg.len(), 1);
        let m = reg.get("com.example.test", "1.0.0").unwrap();
        assert_eq!(m.author, "Test Author");
    }

    #[test]
    fn registry_reject_duplicate() {
        let mut reg = PluginDiscoveryRegistry::new();
        assert!(reg.register(wasm_manifest()));
        assert!(!reg.register(wasm_manifest()));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn registry_different_versions_ok() {
        let mut reg = PluginDiscoveryRegistry::new();
        let mut m2 = wasm_manifest();
        m2.version = "2.0.0".into();
        assert!(reg.register(wasm_manifest()));
        assert!(reg.register(m2));
        assert_eq!(reg.len(), 2);
    }

    #[test]
    fn registry_unregister() {
        let mut reg = PluginDiscoveryRegistry::new();
        reg.register(wasm_manifest());
        let removed = reg.unregister("com.example.test", "1.0.0");
        assert!(removed.is_some());
        assert!(reg.is_empty());
    }

    #[test]
    fn registry_unregister_nonexistent() {
        let mut reg = PluginDiscoveryRegistry::new();
        assert!(reg.unregister("nope", "0.0.0").is_none());
    }

    #[test]
    fn registry_list() {
        let mut reg = PluginDiscoveryRegistry::new();
        reg.register(wasm_manifest());
        reg.register(native_manifest());
        reg.register(service_manifest());
        let list = reg.list();
        assert_eq!(list.len(), 3);
    }

    #[test]
    fn registry_is_empty() {
        let reg = PluginDiscoveryRegistry::new();
        assert!(reg.is_empty());
    }
}
