//! WASM plugin sandboxing and registry for OpenFlight (ADR-003 Tier 1).
//!
//! This crate implements the first tier of the plugin classification system:
//! sandboxed WASM plugins running at 20–120 Hz with capability-based security.
//!
//! # Architecture
//!
//! * [`PluginManifest`] — declares name, version, capabilities, and resource limits.
//! * [`capabilities`] — bitflag capability set with a checker that enforces grants.
//! * [`sandbox`] — trait-based WASM runtime abstraction with resource limiting.
//! * [`registry`] — lifecycle management (register → load → start → stop → unload).

pub mod capabilities;
pub mod registry;
pub mod sandbox;

use capabilities::{CapabilityDenied, CapabilitySet};
use serde::{Deserialize, Serialize};

// ── Core types ─────────────────────────────────────────────────────────

/// Unique identifier for a registered plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PluginId(pub uuid::Uuid);

impl PluginId {
    /// Generate a new random plugin ID.
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl Default for PluginId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for PluginId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The classification tier for a plugin (ADR-003).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginType {
    /// Sandboxed WASM, 20–120 Hz.
    Wasm,
    /// Isolated helper process with shared-memory SPSC.
    NativeFastPath,
    /// Managed thread, event-driven, full access with user consent.
    Service,
}

/// Metadata and configuration for a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Human-readable name.
    pub name: String,
    /// Semantic version string.
    pub version: String,
    /// Plugin tier.
    pub plugin_type: PluginType,
    /// Declared capabilities.
    pub capabilities: CapabilitySet,
    /// Execution frequency in Hz (WASM tier: 20–120).
    pub frequency_hz: u32,
    /// Optional memory limit override (bytes).
    pub max_memory_bytes: Option<usize>,
    /// Optional fuel limit override.
    pub max_fuel: Option<u64>,
}

/// Lifecycle state of a plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginState {
    /// WASM bytes loaded, not yet initialised.
    Loaded,
    /// Running and receiving ticks.
    Running,
    /// Temporarily paused (can resume without re-init).
    Suspended,
    /// An error occurred; plugin needs manual intervention.
    Failed,
    /// Not loaded / removed from runtime.
    Unloaded,
}

impl std::fmt::Display for PluginState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Loaded => "Loaded",
            Self::Running => "Running",
            Self::Suspended => "Suspended",
            Self::Failed => "Failed",
            Self::Unloaded => "Unloaded",
        };
        f.write_str(s)
    }
}

/// Errors that can occur during plugin operations.
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("plugin not found: {0}")]
    NotFound(PluginId),

    #[error("invalid WASM module: {0}")]
    InvalidWasm(String),

    #[error("invalid manifest: {0}")]
    InvalidManifest(String),

    #[error("plugin initialisation failed: {0}")]
    InitFailed(String),

    #[error("plugin tick failed: {0}")]
    TickFailed(String),

    #[error("plugin shutdown failed: {0}")]
    ShutdownFailed(String),

    #[error("resource exhausted: {0}")]
    ResourceExhausted(String),

    #[error(transparent)]
    CapabilityDenied(#[from] CapabilityDenied),

    #[error("invalid state transition: {from} → {to}")]
    InvalidStateTransition { from: PluginState, to: PluginState },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::Capability;

    #[test]
    fn plugin_id_uniqueness() {
        let a = PluginId::new();
        let b = PluginId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn plugin_id_display() {
        let id = PluginId::new();
        let s = id.to_string();
        assert!(!s.is_empty());
        // UUID v4 format: 8-4-4-4-12
        assert_eq!(s.len(), 36);
    }

    #[test]
    fn plugin_state_display() {
        assert_eq!(PluginState::Running.to_string(), "Running");
        assert_eq!(PluginState::Unloaded.to_string(), "Unloaded");
        assert_eq!(PluginState::Failed.to_string(), "Failed");
    }

    #[test]
    fn manifest_serialization_roundtrip() {
        let manifest = PluginManifest {
            name: "test-plugin".into(),
            version: "1.2.3".into(),
            plugin_type: PluginType::Wasm,
            capabilities: CapabilitySet::from_caps([
                Capability::ReadAxes,
                Capability::ReadTelemetry,
            ]),
            frequency_hz: 60,
            max_memory_bytes: Some(8 * 1024 * 1024),
            max_fuel: None,
        };
        let json = serde_json::to_string(&manifest).unwrap();
        let deserialized: PluginManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "test-plugin");
        assert_eq!(deserialized.version, "1.2.3");
        assert_eq!(deserialized.frequency_hz, 60);
        assert!(deserialized.capabilities.contains(Capability::ReadAxes));
        assert!(
            deserialized
                .capabilities
                .contains(Capability::ReadTelemetry)
        );
    }

    #[test]
    fn plugin_type_variants() {
        let wasm = PluginType::Wasm;
        let native = PluginType::NativeFastPath;
        let service = PluginType::Service;
        assert_ne!(wasm, native);
        assert_ne!(native, service);
    }

    #[test]
    fn error_display() {
        let id = PluginId::new();
        let err = PluginError::NotFound(id);
        assert!(err.to_string().contains("not found"));

        let err = PluginError::InvalidWasm("bad module".into());
        assert!(err.to_string().contains("bad module"));

        let err = PluginError::ResourceExhausted("memory".into());
        assert!(err.to_string().contains("memory"));

        let err = PluginError::InvalidStateTransition {
            from: PluginState::Unloaded,
            to: PluginState::Running,
        };
        assert!(err.to_string().contains("Unloaded"));
        assert!(err.to_string().contains("Running"));
    }
}
