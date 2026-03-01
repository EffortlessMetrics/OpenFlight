// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Plugin lifecycle management with state machine and health tracking (ADR-003).
//!
//! Provides [`PluginManager`] which owns plugin instances and enforces the
//! lifecycle state machine:
//!
//! ```text
//! Discovered ─→ Loading ─→ Active ─⇄─ Suspended
//!                  │           │            │
//!                  └──→ Failed ←────────────┘
//!                          │
//!                       Unloading
//! ```
//!
//! Each plugin tracks its error count against a configurable budget. When the
//! budget is exceeded the plugin is automatically suspended.

use std::collections::HashMap;
use std::time::Instant;

use crate::plugin_manifest::PluginManifest;

/// Lifecycle states for a managed plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleState {
    Discovered,
    Loading,
    Active,
    Suspended,
    Failed(String),
    Unloading,
}

impl std::fmt::Display for LifecycleState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Discovered => f.write_str("Discovered"),
            Self::Loading => f.write_str("Loading"),
            Self::Active => f.write_str("Active"),
            Self::Suspended => f.write_str("Suspended"),
            Self::Failed(msg) => write!(f, "Failed({msg})"),
            Self::Unloading => f.write_str("Unloading"),
        }
    }
}

/// Runtime metadata for a managed plugin instance.
#[derive(Debug)]
pub struct PluginInstance {
    pub manifest: PluginManifest,
    pub state: LifecycleState,
    pub load_time: Option<Instant>,
    pub error_count: u32,
}

/// Errors from lifecycle operations.
#[derive(Debug)]
pub enum LifecycleError {
    NotFound(String),
    InvalidTransition { from: String, to: String },
    LoadFailed(String),
}

impl std::fmt::Display for LifecycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(name) => write!(f, "plugin not found: {name}"),
            Self::InvalidTransition { from, to } => {
                write!(f, "invalid state transition: {from} → {to}")
            }
            Self::LoadFailed(msg) => write!(f, "load failed: {msg}"),
        }
    }
}

impl std::error::Error for LifecycleError {}

/// Default number of errors before auto-suspend.
const DEFAULT_ERROR_BUDGET: u32 = 5;

/// Manages the full lifecycle of plugins including health tracking.
#[derive(Debug)]
pub struct PluginManager {
    plugins: HashMap<String, PluginInstance>,
    error_budget: u32,
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginManager {
    /// Create a manager with the default error budget.
    #[must_use]
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            error_budget: DEFAULT_ERROR_BUDGET,
        }
    }

    /// Create a manager with a custom error budget.
    #[must_use]
    pub fn with_error_budget(budget: u32) -> Self {
        Self {
            plugins: HashMap::new(),
            error_budget: budget,
        }
    }

    /// Discover a plugin from its manifest (→ Discovered state).
    pub fn discover(&mut self, manifest: PluginManifest) -> Result<(), LifecycleError> {
        if self.plugins.contains_key(&manifest.name) {
            return Err(LifecycleError::InvalidTransition {
                from: "already registered".into(),
                to: "Discovered".into(),
            });
        }
        let name = manifest.name.clone();
        self.plugins.insert(
            name,
            PluginInstance {
                manifest,
                state: LifecycleState::Discovered,
                load_time: None,
                error_count: 0,
            },
        );
        Ok(())
    }

    /// Load a discovered plugin (Discovered → Loading → Active).
    pub fn load(&mut self, name: &str) -> Result<(), LifecycleError> {
        let instance = self
            .plugins
            .get_mut(name)
            .ok_or_else(|| LifecycleError::NotFound(name.into()))?;

        if instance.state != LifecycleState::Discovered {
            return Err(LifecycleError::InvalidTransition {
                from: instance.state.to_string(),
                to: "Loading".into(),
            });
        }

        instance.state = LifecycleState::Loading;
        instance.load_time = Some(Instant::now());
        // Transition straight to Active (actual WASM/native loading is handled
        // by the tier-specific backend; this state machine tracks the logical
        // lifecycle only).
        instance.state = LifecycleState::Active;
        Ok(())
    }

    /// Suspend an active plugin (Active → Suspended).
    pub fn suspend(&mut self, name: &str) -> Result<(), LifecycleError> {
        let instance = self
            .plugins
            .get_mut(name)
            .ok_or_else(|| LifecycleError::NotFound(name.into()))?;

        if instance.state != LifecycleState::Active {
            return Err(LifecycleError::InvalidTransition {
                from: instance.state.to_string(),
                to: "Suspended".into(),
            });
        }
        instance.state = LifecycleState::Suspended;
        Ok(())
    }

    /// Resume a suspended plugin (Suspended → Active).
    pub fn resume(&mut self, name: &str) -> Result<(), LifecycleError> {
        let instance = self
            .plugins
            .get_mut(name)
            .ok_or_else(|| LifecycleError::NotFound(name.into()))?;

        if instance.state != LifecycleState::Suspended {
            return Err(LifecycleError::InvalidTransition {
                from: instance.state.to_string(),
                to: "Active".into(),
            });
        }
        instance.state = LifecycleState::Active;
        instance.error_count = 0; // reset on resume
        Ok(())
    }

    /// Unload a plugin from any non-Unloading state.
    pub fn unload(&mut self, name: &str) -> Result<(), LifecycleError> {
        let instance = self
            .plugins
            .get_mut(name)
            .ok_or_else(|| LifecycleError::NotFound(name.into()))?;

        if matches!(instance.state, LifecycleState::Unloading) {
            return Err(LifecycleError::InvalidTransition {
                from: "Unloading".into(),
                to: "Unloading".into(),
            });
        }
        instance.state = LifecycleState::Unloading;
        Ok(())
    }

    /// Record an error against a plugin. If the error budget is exceeded the
    /// plugin is automatically suspended.
    pub fn record_error(&mut self, name: &str, message: &str) -> Result<(), LifecycleError> {
        let budget = self.error_budget;
        let instance = self
            .plugins
            .get_mut(name)
            .ok_or_else(|| LifecycleError::NotFound(name.into()))?;

        instance.error_count += 1;
        if instance.error_count >= budget && instance.state == LifecycleState::Active {
            instance.state = LifecycleState::Suspended;
            tracing::warn!(
                plugin = name,
                errors = instance.error_count,
                budget,
                "plugin auto-suspended: error budget exceeded"
            );
        } else {
            tracing::debug!(
                plugin = name,
                error = message,
                count = instance.error_count,
                "plugin error recorded"
            );
        }
        Ok(())
    }

    /// Mark a plugin as failed.
    pub fn fail(&mut self, name: &str, reason: &str) -> Result<(), LifecycleError> {
        let instance = self
            .plugins
            .get_mut(name)
            .ok_or_else(|| LifecycleError::NotFound(name.into()))?;
        instance.state = LifecycleState::Failed(reason.into());
        Ok(())
    }

    /// Get the current state of a plugin.
    #[must_use]
    pub fn state(&self, name: &str) -> Option<&LifecycleState> {
        self.plugins.get(name).map(|i| &i.state)
    }

    /// Get a reference to a plugin instance.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&PluginInstance> {
        self.plugins.get(name)
    }

    /// List all plugin names.
    #[must_use]
    pub fn list_names(&self) -> Vec<&str> {
        self.plugins.keys().map(String::as_str).collect()
    }

    /// Number of managed plugins.
    #[must_use]
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    /// Whether the manager has no plugins.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_manifest(name: &str) -> PluginManifest {
        PluginManifest {
            name: name.into(),
            version: "1.0.0".into(),
            author: "Test".into(),
            description: "test".into(),
            tier: crate::plugin::PluginTier::Wasm,
            capabilities_requested: vec![],
        }
    }

    // ── Discovery ─────────────────────────────────────────────────────

    #[test]
    fn discover_adds_plugin() {
        let mut mgr = PluginManager::new();
        mgr.discover(test_manifest("p1")).unwrap();
        assert_eq!(mgr.len(), 1);
        assert_eq!(
            mgr.state("p1"),
            Some(&LifecycleState::Discovered)
        );
    }

    #[test]
    fn discover_duplicate_rejected() {
        let mut mgr = PluginManager::new();
        mgr.discover(test_manifest("p1")).unwrap();
        assert!(mgr.discover(test_manifest("p1")).is_err());
    }

    // ── Loading ───────────────────────────────────────────────────────

    #[test]
    fn load_transitions_to_active() {
        let mut mgr = PluginManager::new();
        mgr.discover(test_manifest("p1")).unwrap();
        mgr.load("p1").unwrap();
        assert_eq!(mgr.state("p1"), Some(&LifecycleState::Active));
        assert!(mgr.get("p1").unwrap().load_time.is_some());
    }

    #[test]
    fn load_from_active_rejected() {
        let mut mgr = PluginManager::new();
        mgr.discover(test_manifest("p1")).unwrap();
        mgr.load("p1").unwrap();
        assert!(mgr.load("p1").is_err());
    }

    #[test]
    fn load_nonexistent_rejected() {
        let mut mgr = PluginManager::new();
        assert!(matches!(
            mgr.load("nope"),
            Err(LifecycleError::NotFound(_))
        ));
    }

    // ── Suspend / Resume ──────────────────────────────────────────────

    #[test]
    fn suspend_active_plugin() {
        let mut mgr = PluginManager::new();
        mgr.discover(test_manifest("p1")).unwrap();
        mgr.load("p1").unwrap();
        mgr.suspend("p1").unwrap();
        assert_eq!(
            mgr.state("p1"),
            Some(&LifecycleState::Suspended)
        );
    }

    #[test]
    fn suspend_non_active_rejected() {
        let mut mgr = PluginManager::new();
        mgr.discover(test_manifest("p1")).unwrap();
        assert!(mgr.suspend("p1").is_err());
    }

    #[test]
    fn resume_suspended_plugin() {
        let mut mgr = PluginManager::new();
        mgr.discover(test_manifest("p1")).unwrap();
        mgr.load("p1").unwrap();
        mgr.suspend("p1").unwrap();
        mgr.resume("p1").unwrap();
        assert_eq!(mgr.state("p1"), Some(&LifecycleState::Active));
    }

    #[test]
    fn resume_non_suspended_rejected() {
        let mut mgr = PluginManager::new();
        mgr.discover(test_manifest("p1")).unwrap();
        mgr.load("p1").unwrap();
        assert!(mgr.resume("p1").is_err());
    }

    #[test]
    fn resume_resets_error_count() {
        let mut mgr = PluginManager::with_error_budget(10);
        mgr.discover(test_manifest("p1")).unwrap();
        mgr.load("p1").unwrap();
        mgr.record_error("p1", "oops").unwrap();
        mgr.record_error("p1", "oops").unwrap();
        assert_eq!(mgr.get("p1").unwrap().error_count, 2);
        mgr.suspend("p1").unwrap();
        mgr.resume("p1").unwrap();
        assert_eq!(mgr.get("p1").unwrap().error_count, 0);
    }

    // ── Unload ────────────────────────────────────────────────────────

    #[test]
    fn unload_from_active() {
        let mut mgr = PluginManager::new();
        mgr.discover(test_manifest("p1")).unwrap();
        mgr.load("p1").unwrap();
        mgr.unload("p1").unwrap();
        assert_eq!(
            mgr.state("p1"),
            Some(&LifecycleState::Unloading)
        );
    }

    #[test]
    fn unload_from_suspended() {
        let mut mgr = PluginManager::new();
        mgr.discover(test_manifest("p1")).unwrap();
        mgr.load("p1").unwrap();
        mgr.suspend("p1").unwrap();
        mgr.unload("p1").unwrap();
        assert_eq!(
            mgr.state("p1"),
            Some(&LifecycleState::Unloading)
        );
    }

    #[test]
    fn unload_from_discovered() {
        let mut mgr = PluginManager::new();
        mgr.discover(test_manifest("p1")).unwrap();
        mgr.unload("p1").unwrap();
        assert_eq!(
            mgr.state("p1"),
            Some(&LifecycleState::Unloading)
        );
    }

    #[test]
    fn unload_already_unloading_rejected() {
        let mut mgr = PluginManager::new();
        mgr.discover(test_manifest("p1")).unwrap();
        mgr.unload("p1").unwrap();
        assert!(mgr.unload("p1").is_err());
    }

    #[test]
    fn unload_nonexistent_rejected() {
        let mut mgr = PluginManager::new();
        assert!(matches!(
            mgr.unload("nope"),
            Err(LifecycleError::NotFound(_))
        ));
    }

    // ── Error budget ──────────────────────────────────────────────────

    #[test]
    fn errors_within_budget_keep_active() {
        let mut mgr = PluginManager::with_error_budget(3);
        mgr.discover(test_manifest("p1")).unwrap();
        mgr.load("p1").unwrap();
        mgr.record_error("p1", "e1").unwrap();
        mgr.record_error("p1", "e2").unwrap();
        assert_eq!(mgr.state("p1"), Some(&LifecycleState::Active));
        assert_eq!(mgr.get("p1").unwrap().error_count, 2);
    }

    #[test]
    fn exceeding_budget_auto_suspends() {
        let mut mgr = PluginManager::with_error_budget(3);
        mgr.discover(test_manifest("p1")).unwrap();
        mgr.load("p1").unwrap();
        mgr.record_error("p1", "e1").unwrap();
        mgr.record_error("p1", "e2").unwrap();
        mgr.record_error("p1", "e3").unwrap(); // hits budget
        assert_eq!(
            mgr.state("p1"),
            Some(&LifecycleState::Suspended)
        );
    }

    #[test]
    fn error_budget_one_suspends_immediately() {
        let mut mgr = PluginManager::with_error_budget(1);
        mgr.discover(test_manifest("p1")).unwrap();
        mgr.load("p1").unwrap();
        mgr.record_error("p1", "boom").unwrap();
        assert_eq!(
            mgr.state("p1"),
            Some(&LifecycleState::Suspended)
        );
    }

    #[test]
    fn record_error_nonexistent_rejected() {
        let mut mgr = PluginManager::new();
        assert!(mgr.record_error("nope", "err").is_err());
    }

    // ── Fail ──────────────────────────────────────────────────────────

    #[test]
    fn fail_sets_failed_state() {
        let mut mgr = PluginManager::new();
        mgr.discover(test_manifest("p1")).unwrap();
        mgr.load("p1").unwrap();
        mgr.fail("p1", "fatal error").unwrap();
        assert!(matches!(
            mgr.state("p1"),
            Some(LifecycleState::Failed(_))
        ));
    }

    // ── Listing ───────────────────────────────────────────────────────

    #[test]
    fn list_names_returns_all() {
        let mut mgr = PluginManager::new();
        mgr.discover(test_manifest("a")).unwrap();
        mgr.discover(test_manifest("b")).unwrap();
        let mut names = mgr.list_names();
        names.sort();
        assert_eq!(names, vec!["a", "b"]);
    }

    #[test]
    fn empty_manager() {
        let mgr = PluginManager::new();
        assert!(mgr.is_empty());
        assert_eq!(mgr.len(), 0);
    }
}
