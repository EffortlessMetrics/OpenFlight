// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Security and Privacy Module
//!
//! Implements security controls, plugin signing verification, capability validation,
//! and privacy-preserving telemetry collection according to SEC-01 requirements.

pub mod verification;

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

pub use verification::{
    AuditEvent, AuditEventType, RecommendationPriority, SecurityCategory, SecurityCheck,
    SecurityRecommendation, SecuritySeverity, SecurityVerificationResult, SecurityVerifier,
    VerificationConfig, VerificationError, VerificationStatus,
};

pub type Result<T> = std::result::Result<T, SecurityError>;

#[derive(Debug, Error)]
pub enum SecurityError {
    #[error("Plugin signature verification failed: {reason}")]
    SignatureVerificationFailed { reason: String },

    #[error("Capability not granted: {capability}")]
    CapabilityDenied { capability: String },

    #[error("Plugin manifest invalid: {reason}")]
    InvalidManifest { reason: String },

    #[error("Telemetry collection not authorized")]
    TelemetryNotAuthorized,

    #[error("ACL validation failed: {reason}")]
    AclValidationFailed { reason: String },

    #[error("Security policy violation: {reason}")]
    PolicyViolation { reason: String },
}

/// Plugin signature status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignatureStatus {
    /// Plugin is signed with a valid signature
    Signed {
        issuer: String,
        subject: String,
        valid_from: u64,
        valid_until: u64,
    },
    /// Plugin is unsigned
    Unsigned,
    /// Plugin signature is invalid or corrupted
    Invalid { reason: String },
}

/// Plugin capability manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCapabilityManifest {
    /// Plugin name and version
    pub name: String,
    pub version: String,

    /// Required capabilities
    pub capabilities: HashSet<PluginCapability>,

    /// Optional description
    pub description: Option<String>,

    /// Plugin type (WASM or Native)
    pub plugin_type: PluginType,

    /// Signature status
    pub signature: SignatureStatus,
}

/// Plugin types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginType {
    /// WebAssembly plugin (sandboxed)
    Wasm,
    /// Native plugin (isolated process)
    Native,
}

/// Plugin capabilities that can be requested
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginCapability {
    /// Read telemetry bus data
    ReadBus,
    /// Emit panel LED/display commands
    EmitPanel,
    /// Read profile data
    ReadProfiles,
    /// Write to blackbox (events only)
    WriteBlackbox,
    /// Access device health data
    ReadDeviceHealth,
    /// File system access (native plugins only)
    FileSystem { paths: Vec<PathBuf> },
    /// Network access (native plugins only)
    Network { hosts: Vec<String> },
}

/// Telemetry collection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    /// Whether telemetry collection is enabled
    pub enabled: bool,

    /// User consent timestamp
    pub consent_timestamp: Option<u64>,

    /// What data types are collected
    pub collected_data: HashSet<TelemetryDataType>,

    /// Retention period in days
    pub retention_days: u32,

    /// Whether to include in support bundles
    pub include_in_support: bool,
}

/// Types of telemetry data that can be collected
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TelemetryDataType {
    /// Performance metrics (jitter, latency, etc.)
    Performance,
    /// Error events and fault counts
    Errors,
    /// Feature usage statistics
    Usage,
    /// Device connection/disconnection events
    DeviceEvents,
    /// Profile application events
    ProfileEvents,
}

/// ACL (Access Control List) configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AclConfig {
    /// Allowed user/group IDs for IPC access
    pub allowed_users: HashSet<String>,

    /// Whether to restrict to current user only
    pub current_user_only: bool,

    /// Platform-specific ACL settings
    #[cfg(windows)]
    pub windows_acl: WindowsAclConfig,

    #[cfg(unix)]
    pub unix_acl: UnixAclConfig,
}

#[cfg(windows)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowsAclConfig {
    /// Security descriptor for named pipes
    pub pipe_security_descriptor: Option<String>,

    /// Whether to allow network access
    pub allow_network: bool,
}

#[cfg(unix)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnixAclConfig {
    /// Socket file permissions (octal)
    pub socket_permissions: u32,

    /// Socket file group
    pub socket_group: Option<String>,
}

/// Security policy manager
pub struct SecurityManager {
    config: SecurityConfig,
    telemetry_config: TelemetryConfig,
    acl_config: AclConfig,
    plugin_registry: HashMap<String, PluginCapabilityManifest>,
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Whether to enforce plugin signatures
    pub enforce_signatures: bool,

    /// Trusted certificate authorities for plugin signing
    pub trusted_cas: Vec<String>,

    /// Whether to allow unsigned plugins
    pub allow_unsigned: bool,

    /// Maximum plugin execution time budget (microseconds)
    pub max_plugin_budget_us: u64,

    /// Whether to enable security audit logging
    pub audit_logging: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enforce_signatures: true,
            trusted_cas: vec![],
            allow_unsigned: false,
            max_plugin_budget_us: 100, // 100μs as per requirements
            audit_logging: true,
        }
    }
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: false, // Opt-in by default
            consent_timestamp: None,
            collected_data: HashSet::new(),
            retention_days: 30,
            include_in_support: false,
        }
    }
}

impl Default for AclConfig {
    fn default() -> Self {
        Self {
            allowed_users: HashSet::new(),
            current_user_only: true,

            #[cfg(windows)]
            windows_acl: WindowsAclConfig {
                pipe_security_descriptor: None,
                allow_network: false,
            },

            #[cfg(unix)]
            unix_acl: UnixAclConfig {
                socket_permissions: 0o600, // Owner read/write only
                socket_group: None,
            },
        }
    }
}

impl SecurityManager {
    /// Create a new security manager with default configuration
    pub fn new() -> Self {
        Self {
            config: SecurityConfig::default(),
            telemetry_config: TelemetryConfig::default(),
            acl_config: AclConfig::default(),
            plugin_registry: HashMap::new(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(
        config: SecurityConfig,
        telemetry_config: TelemetryConfig,
        acl_config: AclConfig,
    ) -> Self {
        Self {
            config,
            telemetry_config,
            acl_config,
            plugin_registry: HashMap::new(),
        }
    }

    /// Validate plugin manifest and signature
    pub fn validate_plugin(&mut self, manifest: PluginCapabilityManifest) -> Result<()> {
        // Validate signature if enforcement is enabled
        if self.config.enforce_signatures {
            match &manifest.signature {
                SignatureStatus::Signed { .. } => {
                    // In a real implementation, this would verify the signature
                    // against trusted CAs and check validity period
                    self.verify_plugin_signature(&manifest)?;
                }
                SignatureStatus::Unsigned => {
                    if !self.config.allow_unsigned {
                        return Err(SecurityError::SignatureVerificationFailed {
                            reason: "Plugin is unsigned and unsigned plugins are not allowed"
                                .to_string(),
                        });
                    }
                }
                SignatureStatus::Invalid { reason } => {
                    return Err(SecurityError::SignatureVerificationFailed {
                        reason: reason.clone(),
                    });
                }
            }
        }

        // Validate capability manifest
        self.validate_capabilities(&manifest)?;

        // Register the plugin
        self.plugin_registry.insert(manifest.name.clone(), manifest);

        Ok(())
    }

    /// Check if a plugin has a specific capability
    pub fn check_capability(&self, plugin_name: &str, capability: &PluginCapability) -> bool {
        if let Some(manifest) = self.plugin_registry.get(plugin_name) {
            manifest.capabilities.contains(capability)
        } else {
            false
        }
    }

    /// Enable telemetry collection with user consent
    pub fn enable_telemetry(&mut self, data_types: HashSet<TelemetryDataType>) -> Result<()> {
        self.telemetry_config.enabled = true;
        self.telemetry_config.consent_timestamp = Some(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        );
        self.telemetry_config.collected_data = data_types;

        Ok(())
    }

    /// Disable telemetry collection
    pub fn disable_telemetry(&mut self) {
        self.telemetry_config.enabled = false;
        self.telemetry_config.consent_timestamp = None;
        self.telemetry_config.collected_data.clear();
    }

    /// Check if telemetry collection is authorized for a specific data type
    pub fn is_telemetry_authorized(&self, data_type: &TelemetryDataType) -> bool {
        self.telemetry_config.enabled && self.telemetry_config.collected_data.contains(data_type)
    }

    /// Get redacted support bundle data
    pub fn get_redacted_support_data(&self) -> HashMap<String, serde_json::Value> {
        let mut data = HashMap::new();

        // Only include data if user has consented and enabled support inclusion
        if self.telemetry_config.enabled && self.telemetry_config.include_in_support {
            // Add performance metrics (anonymized)
            if self
                .telemetry_config
                .collected_data
                .contains(&TelemetryDataType::Performance)
            {
                data.insert(
                    "performance_summary".to_string(),
                    serde_json::json!({
                        "avg_jitter_ms": "[REDACTED]",
                        "p99_latency_us": "[REDACTED]",
                        "uptime_hours": "[REDACTED]"
                    }),
                );
            }

            // Add error counts (no personal data)
            if self
                .telemetry_config
                .collected_data
                .contains(&TelemetryDataType::Errors)
            {
                data.insert(
                    "error_summary".to_string(),
                    serde_json::json!({
                        "total_errors": "[REDACTED]",
                        "error_types": "[REDACTED]"
                    }),
                );
            }
        }

        // Always include security configuration (for support purposes)
        data.insert(
            "security_config".to_string(),
            serde_json::json!({
                "signatures_enforced": self.config.enforce_signatures,
                "unsigned_allowed": self.config.allow_unsigned,
                "plugin_count": self.plugin_registry.len(),
                "telemetry_enabled": self.telemetry_config.enabled
            }),
        );

        data
    }

    /// Validate ACL configuration for IPC
    pub fn validate_ipc_acl(&self, client_info: &IpcClientInfo) -> Result<()> {
        // Check if current user only mode is enabled
        if self.acl_config.current_user_only && client_info.user_id != get_current_user_id()? {
            return Err(SecurityError::AclValidationFailed {
                reason: "Access denied: current user only mode enabled".to_string(),
            });
        }

        // Check allowed users list
        if !self.acl_config.allowed_users.is_empty()
            && !self.acl_config.allowed_users.contains(&client_info.user_id)
        {
            return Err(SecurityError::AclValidationFailed {
                reason: "Access denied: user not in allowed list".to_string(),
            });
        }

        // Platform-specific ACL validation
        self.validate_platform_acl(client_info)?;

        Ok(())
    }

    /// Get plugin registry for UI display
    pub fn get_plugin_registry(&self) -> &HashMap<String, PluginCapabilityManifest> {
        &self.plugin_registry
    }

    /// Get telemetry configuration
    pub fn get_telemetry_config(&self) -> &TelemetryConfig {
        &self.telemetry_config
    }

    /// Get ACL configuration
    pub fn get_acl_config(&self) -> &AclConfig {
        &self.acl_config
    }

    // Private helper methods

    fn verify_plugin_signature(&self, manifest: &PluginCapabilityManifest) -> Result<()> {
        // In a real implementation, this would:
        // 1. Extract the signature from the plugin binary
        // 2. Verify against trusted CAs
        // 3. Check certificate validity period
        // 4. Validate signature matches the binary

        if let SignatureStatus::Signed { valid_until, .. } = &manifest.signature {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            if now > *valid_until {
                return Err(SecurityError::SignatureVerificationFailed {
                        reason: "Certificate has expired".to_string(),
                    }
                );
            }
        }

        Ok(())
    }

    fn validate_capabilities(&self, manifest: &PluginCapabilityManifest) -> Result<()> {
        // Validate that requested capabilities are appropriate for plugin type
        for capability in &manifest.capabilities {
            match (capability, &manifest.plugin_type) {
                // File system and network access only allowed for native plugins
                (PluginCapability::FileSystem { .. }, PluginType::Wasm) => {
                    return Err(SecurityError::InvalidManifest {
                        reason: "WASM plugins cannot request file system access".to_string(),
                    });
                }
                (PluginCapability::Network { .. }, PluginType::Wasm) => {
                    return Err(SecurityError::InvalidManifest {
                        reason: "WASM plugins cannot request network access".to_string(),
                    });
                }
                _ => {} // Other capabilities are valid for both types
            }
        }

        Ok(())
    }

    #[cfg(windows)]
    fn validate_platform_acl(&self, _client_info: &IpcClientInfo) -> Result<()> {
        // Windows-specific ACL validation
        // In a real implementation, this would check Windows security descriptors
        Ok(())
    }

    #[cfg(unix)]
    fn validate_platform_acl(&self, _client_info: &IpcClientInfo) -> Result<()> {
        // Unix-specific ACL validation
        // In a real implementation, this would check file permissions and groups
        Ok(())
    }
}

/// IPC client information for ACL validation
#[derive(Debug, Clone)]
pub struct IpcClientInfo {
    pub user_id: String,
    pub process_id: u32,
    pub executable_path: PathBuf,
}

/// Get current user ID (platform-specific)
fn get_current_user_id() -> Result<String> {
    #[cfg(windows)]
    {
        // On Windows, use SID
        Ok("S-1-5-21-000000000-000000000-000000000-1000".to_string()) // Placeholder
    }

    #[cfg(unix)]
    {
        // On Unix, use UID
        Ok(unsafe { libc::getuid() }.to_string())
    }
}

impl Default for SecurityManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_validation() {
        let mut security_manager = SecurityManager::new();

        let manifest = PluginCapabilityManifest {
            name: "test-plugin".to_string(),
            version: "1.0.0".to_string(),
            capabilities: [PluginCapability::ReadBus].into_iter().collect(),
            description: Some("Test plugin".to_string()),
            plugin_type: PluginType::Wasm,
            signature: SignatureStatus::Unsigned,
        };

        // Should fail with default config (signatures enforced, unsigned not allowed)
        assert!(security_manager.validate_plugin(manifest.clone()).is_err());

        // Allow unsigned plugins
        security_manager.config.allow_unsigned = true;
        assert!(security_manager.validate_plugin(manifest).is_ok());
    }

    #[test]
    fn test_capability_validation() {
        let mut security_manager = SecurityManager::new();
        security_manager.config.allow_unsigned = true;

        // WASM plugin requesting file system access should fail
        let invalid_manifest = PluginCapabilityManifest {
            name: "invalid-plugin".to_string(),
            version: "1.0.0".to_string(),
            capabilities: [PluginCapability::FileSystem {
                paths: vec![PathBuf::from("/tmp")],
            }]
            .into_iter()
            .collect(),
            description: None,
            plugin_type: PluginType::Wasm,
            signature: SignatureStatus::Unsigned,
        };

        assert!(security_manager.validate_plugin(invalid_manifest).is_err());

        // Native plugin requesting file system access should succeed
        let valid_manifest = PluginCapabilityManifest {
            name: "valid-plugin".to_string(),
            version: "1.0.0".to_string(),
            capabilities: [PluginCapability::FileSystem {
                paths: vec![PathBuf::from("/tmp")],
            }]
            .into_iter()
            .collect(),
            description: None,
            plugin_type: PluginType::Native,
            signature: SignatureStatus::Unsigned,
        };

        assert!(security_manager.validate_plugin(valid_manifest).is_ok());
    }

    #[test]
    fn test_telemetry_authorization() {
        let mut security_manager = SecurityManager::new();

        // Initially disabled
        assert!(!security_manager.is_telemetry_authorized(&TelemetryDataType::Performance));

        // Enable with specific data types
        let data_types = [TelemetryDataType::Performance, TelemetryDataType::Errors]
            .into_iter()
            .collect();

        security_manager.enable_telemetry(data_types).unwrap();

        assert!(security_manager.is_telemetry_authorized(&TelemetryDataType::Performance));
        assert!(security_manager.is_telemetry_authorized(&TelemetryDataType::Errors));
        assert!(!security_manager.is_telemetry_authorized(&TelemetryDataType::Usage));
    }

    #[test]
    fn test_redacted_support_data() {
        let mut security_manager = SecurityManager::new();

        // Enable telemetry and support inclusion
        security_manager.telemetry_config.enabled = true;
        security_manager.telemetry_config.include_in_support = true;
        security_manager.telemetry_config.collected_data =
            [TelemetryDataType::Performance].into_iter().collect();

        let data = security_manager.get_redacted_support_data();

        // Should include performance summary and security config
        assert!(data.contains_key("performance_summary"));
        assert!(data.contains_key("security_config"));

        // Performance data should be redacted
        let perf_data = &data["performance_summary"];
        assert_eq!(perf_data["avg_jitter_ms"], "[REDACTED]");
    }
}
