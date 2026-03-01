// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Adapter Manager
//!
//! Manages the lifecycle of simulator adapters: start, stop, health
//! monitoring, and status reporting to the event bus. Works with the
//! [`ServiceOrchestrator`] to keep per-adapter state in sync.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::config::AdapterSettings;
use crate::orchestrator::{
    AdapterEvent, AdapterLifecycleState, OrchestratorError, ServiceOrchestrator,
};

// ---------------------------------------------------------------------------
// Adapter health
// ---------------------------------------------------------------------------

/// Health status for a single adapter.
#[derive(Debug, Clone)]
pub struct AdapterHealth {
    /// Current state of the adapter.
    pub state: AdapterLifecycleState,
    /// Number of consecutive errors since last healthy tick.
    pub consecutive_errors: u32,
    /// Total errors since creation.
    pub total_errors: u64,
    /// Total successful health checks.
    pub healthy_checks: u64,
    /// When the adapter was last checked.
    pub last_check: Option<Instant>,
    /// When the adapter was started.
    pub started_at: Option<Instant>,
}

impl Default for AdapterHealth {
    fn default() -> Self {
        Self {
            state: AdapterLifecycleState::Stopped,
            consecutive_errors: 0,
            total_errors: 0,
            healthy_checks: 0,
            last_check: None,
            started_at: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Adapter Manager
// ---------------------------------------------------------------------------

/// Manages running sim adapters: start/stop, health monitoring, status reporting.
pub struct AdapterManager {
    settings: AdapterSettings,
    /// Per-adapter health tracking.
    health: HashMap<String, AdapterHealth>,
    /// Adapter event log for recent events (bounded ring).
    event_log: Vec<AdapterEventRecord>,
    /// Maximum event log size.
    max_log_size: usize,
}

/// Recorded adapter event with timestamp.
#[derive(Debug, Clone)]
pub struct AdapterEventRecord {
    pub adapter: String,
    pub event: AdapterEvent,
    pub timestamp: Instant,
}

/// Snapshot of all adapter statuses.
#[derive(Debug, Clone)]
pub struct AdapterManagerStatus {
    pub adapters: HashMap<String, AdapterHealth>,
    pub total_events: usize,
    pub running_count: usize,
    pub error_count: usize,
}

impl AdapterManager {
    /// Create a new adapter manager from settings.
    pub fn new(settings: AdapterSettings) -> Self {
        let mut health = HashMap::new();

        // Initialize health entries for configured adapters.
        let entries = [
            ("msfs", settings.enable_msfs),
            ("xplane", settings.enable_xplane),
            ("dcs", settings.enable_dcs),
            ("ac7", settings.enable_ac7),
            ("wingman", settings.enable_wingman),
        ];

        for (name, enabled) in entries {
            health.insert(
                name.to_string(),
                AdapterHealth {
                    state: if enabled {
                        AdapterLifecycleState::Stopped
                    } else {
                        AdapterLifecycleState::Disabled
                    },
                    ..AdapterHealth::default()
                },
            );
        }

        Self {
            settings,
            health,
            event_log: Vec::new(),
            max_log_size: 1000,
        }
    }

    /// Start an adapter by name and update the orchestrator.
    ///
    /// If the adapter is already running in the orchestrator, the manager
    /// syncs its local state without returning an error.
    pub fn start_adapter(
        &mut self,
        name: &str,
        orchestrator: &mut ServiceOrchestrator,
    ) -> Result<(), OrchestratorError> {
        let key = Self::normalize_adapter_name(name);
        match orchestrator.start_adapter(&key) {
            Ok(()) => {}
            Err(OrchestratorError::SubsystemAlreadyRunning(_)) => {
                // Already running — sync local state.
            }
            Err(e) => return Err(e),
        }

        if let Some(health) = self.health.get_mut(&key) {
            health.state = AdapterLifecycleState::Running;
            health.consecutive_errors = 0;
            health.started_at = Some(Instant::now());
        }

        self.log_event(
            &key,
            AdapterEvent::SimConnected {
                sim_name: key.clone(),
            },
        );

        Ok(())
    }

    /// Stop an adapter by name and update the orchestrator.
    ///
    /// If the adapter is already stopped, the manager syncs its local state.
    pub fn stop_adapter(
        &mut self,
        name: &str,
        orchestrator: &mut ServiceOrchestrator,
    ) -> Result<(), OrchestratorError> {
        let key = Self::normalize_adapter_name(name);
        match orchestrator.stop_adapter(&key) {
            Ok(()) => {}
            Err(OrchestratorError::SubsystemNotRunning(_)) => {
                // Already stopped — sync local state.
            }
            Err(e) => return Err(e),
        }

        if let Some(health) = self.health.get_mut(&key) {
            health.state = AdapterLifecycleState::Stopped;
            health.started_at = None;
        }

        self.log_event(
            &key,
            AdapterEvent::SimDisconnected {
                sim_name: key.clone(),
            },
        );

        Ok(())
    }

    /// Record a health check result for an adapter.
    ///
    /// Returns `true` if the adapter should be marked as failed (consecutive
    /// errors exceeded threshold).
    pub fn record_health_check(&mut self, name: &str, healthy: bool) -> bool {
        let threshold = self.settings.max_consecutive_errors;
        let key = Self::normalize_adapter_name(name);

        if let Some(health) = self.health.get_mut(&key) {
            health.last_check = Some(Instant::now());

            if healthy {
                health.consecutive_errors = 0;
                health.healthy_checks += 1;
                false
            } else {
                health.consecutive_errors += 1;
                health.total_errors += 1;

                if health.consecutive_errors >= threshold {
                    health.state =
                        AdapterLifecycleState::Error("max consecutive errors exceeded".into());
                    true
                } else {
                    false
                }
            }
        } else {
            false
        }
    }

    /// Record an adapter error and propagate to the orchestrator.
    pub fn record_error(
        &mut self,
        name: &str,
        error: &str,
        orchestrator: &mut ServiceOrchestrator,
    ) {
        let key = Self::normalize_adapter_name(name);
        orchestrator.record_adapter_error(&key, error);

        if let Some(health) = self.health.get_mut(&key) {
            health.consecutive_errors += 1;
            health.total_errors += 1;
            if health.consecutive_errors >= self.settings.max_consecutive_errors {
                health.state = AdapterLifecycleState::Error(error.to_string());
            }
        }
    }

    /// Attempt to restart a failed adapter.
    pub fn restart_adapter(
        &mut self,
        name: &str,
        orchestrator: &mut ServiceOrchestrator,
    ) -> Result<(), OrchestratorError> {
        // Stop first (ignore NotRunning errors).
        let _ = self.stop_adapter(name, orchestrator);
        self.start_adapter(name, orchestrator)
    }

    /// Get health for a specific adapter.
    pub fn adapter_health(&self, name: &str) -> Option<&AdapterHealth> {
        let key = Self::normalize_adapter_name(name);
        self.health.get(&key)
    }

    /// Get a status snapshot of all adapters.
    pub fn status(&self) -> AdapterManagerStatus {
        let running_count = self
            .health
            .values()
            .filter(|h| h.state == AdapterLifecycleState::Running)
            .count();
        let error_count = self
            .health
            .values()
            .filter(|h| matches!(h.state, AdapterLifecycleState::Error(_)))
            .count();

        AdapterManagerStatus {
            adapters: self.health.clone(),
            total_events: self.event_log.len(),
            running_count,
            error_count,
        }
    }

    /// List adapters that are currently running.
    pub fn running_adapters(&self) -> Vec<String> {
        self.health
            .iter()
            .filter(|(_, h)| h.state == AdapterLifecycleState::Running)
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// List adapters that are in an error state.
    pub fn errored_adapters(&self) -> Vec<String> {
        self.health
            .iter()
            .filter(|(_, h)| matches!(h.state, AdapterLifecycleState::Error(_)))
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Start all enabled adapters.
    pub fn start_all(
        &mut self,
        orchestrator: &mut ServiceOrchestrator,
    ) -> HashMap<String, Result<(), OrchestratorError>> {
        let enabled: Vec<String> = self
            .health
            .iter()
            .filter(|(_, h)| h.state != AdapterLifecycleState::Disabled)
            .map(|(name, _)| name.clone())
            .collect();

        let mut results = HashMap::new();
        for name in enabled {
            let result = self.start_adapter(&name, orchestrator);
            results.insert(name, result);
        }
        results
    }

    /// Stop all running adapters.
    pub fn stop_all(
        &mut self,
        orchestrator: &mut ServiceOrchestrator,
    ) -> HashMap<String, Result<(), OrchestratorError>> {
        let to_stop: Vec<String> = self
            .health
            .iter()
            .filter(|(_, h)| {
                h.state == AdapterLifecycleState::Running
                    || matches!(h.state, AdapterLifecycleState::Error(_))
            })
            .map(|(name, _)| name.clone())
            .collect();
        let mut results = HashMap::new();
        for name in to_stop {
            let result = self.stop_adapter(&name, orchestrator);
            results.insert(name, result);
        }
        results
    }

    /// The health check interval from settings.
    pub fn health_interval(&self) -> Duration {
        self.settings.health_interval
    }

    /// Return the recent event log.
    pub fn event_log(&self) -> &[AdapterEventRecord] {
        &self.event_log
    }

    /// Update settings (e.g. after config reload).
    pub fn update_settings(&mut self, settings: AdapterSettings) {
        self.settings = settings;
    }

    // -- internal helpers ----------------------------------------------------

    /// Canonical form for adapter names: lowercase with hyphens and spaces stripped.
    fn normalize_adapter_name(name: &str) -> String {
        name.to_lowercase().replace(['-', ' '], "")
    }

    fn log_event(&mut self, adapter: &str, event: AdapterEvent) {
        if self.event_log.len() >= self.max_log_size {
            self.event_log.remove(0);
        }
        self.event_log.push(AdapterEventRecord {
            adapter: adapter.to_string(),
            event,
            timestamp: Instant::now(),
        });
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::{ServiceConfig, ServiceOrchestrator};

    fn test_settings() -> AdapterSettings {
        AdapterSettings {
            enable_msfs: true,
            enable_xplane: true,
            enable_dcs: false,
            enable_ac7: false,
            enable_wingman: false,
            health_interval: Duration::from_secs(5),
            max_consecutive_errors: 3,
        }
    }

    fn test_orchestrator() -> ServiceOrchestrator {
        let cfg = ServiceConfig {
            enable_adapters: true,
            enable_watchdog: false,
            adapter_flags: crate::orchestrator::AdapterFlags {
                msfs: true,
                xplane: true,
                dcs: false,
                ac7: false,
                wingman: false,
            },
            ..ServiceConfig::default()
        };
        let mut orch = ServiceOrchestrator::new(cfg);
        orch.start().expect("start should succeed");
        orch
    }

    #[test]
    fn new_creates_entries_for_all_adapters() {
        let mgr = AdapterManager::new(test_settings());
        assert!(mgr.adapter_health("msfs").is_some());
        assert!(mgr.adapter_health("xplane").is_some());
        assert!(mgr.adapter_health("dcs").is_some());

        // Enabled adapters start as Stopped.
        assert_eq!(
            mgr.adapter_health("msfs").unwrap().state,
            AdapterLifecycleState::Stopped
        );
        // Disabled adapters start as Disabled.
        assert_eq!(
            mgr.adapter_health("dcs").unwrap().state,
            AdapterLifecycleState::Disabled
        );
    }

    #[test]
    fn start_adapter_updates_state() {
        let mut mgr = AdapterManager::new(test_settings());
        let mut orch = test_orchestrator();

        mgr.start_adapter("msfs", &mut orch).unwrap();

        let health = mgr.adapter_health("msfs").unwrap();
        assert_eq!(health.state, AdapterLifecycleState::Running);
        assert!(health.started_at.is_some());
    }

    #[test]
    fn stop_adapter_updates_state() {
        let mut mgr = AdapterManager::new(test_settings());
        let mut orch = test_orchestrator();

        mgr.start_adapter("msfs", &mut orch).unwrap();
        mgr.stop_adapter("msfs", &mut orch).unwrap();

        let health = mgr.adapter_health("msfs").unwrap();
        assert_eq!(health.state, AdapterLifecycleState::Stopped);
        assert!(health.started_at.is_none());
    }

    #[test]
    fn health_check_threshold_triggers_error() {
        let mut mgr = AdapterManager::new(test_settings());

        // Report 2 failures → not yet exceeded (threshold = 3)
        assert!(!mgr.record_health_check("msfs", false));
        assert!(!mgr.record_health_check("msfs", false));

        // 3rd failure → exceeded
        assert!(mgr.record_health_check("msfs", false));
        assert!(matches!(
            mgr.adapter_health("msfs").unwrap().state,
            AdapterLifecycleState::Error(_)
        ));
    }

    #[test]
    fn healthy_check_resets_consecutive_errors() {
        let mut mgr = AdapterManager::new(test_settings());

        mgr.record_health_check("msfs", false);
        mgr.record_health_check("msfs", false);
        // One healthy check resets the counter.
        mgr.record_health_check("msfs", true);
        assert_eq!(mgr.adapter_health("msfs").unwrap().consecutive_errors, 0);
        assert_eq!(mgr.adapter_health("msfs").unwrap().healthy_checks, 1);
    }

    #[test]
    fn start_all_starts_enabled_adapters() {
        let mut mgr = AdapterManager::new(test_settings());
        let mut orch = test_orchestrator();

        let results = mgr.start_all(&mut orch);
        // msfs and xplane are enabled, dcs/ac7/wingman are disabled.
        assert!(results["msfs"].is_ok());
        assert!(results["xplane"].is_ok());

        assert_eq!(mgr.running_adapters().len(), 2);
    }

    #[test]
    fn stop_all_stops_running_adapters() {
        let mut mgr = AdapterManager::new(test_settings());
        let mut orch = test_orchestrator();

        mgr.start_all(&mut orch);
        let results = mgr.stop_all(&mut orch);
        assert!(results["msfs"].is_ok());
        assert!(results["xplane"].is_ok());

        assert_eq!(mgr.running_adapters().len(), 0);
    }

    #[test]
    fn status_snapshot_counts() {
        let mut mgr = AdapterManager::new(test_settings());
        let mut orch = test_orchestrator();

        mgr.start_adapter("msfs", &mut orch).unwrap();
        let status = mgr.status();
        assert_eq!(status.running_count, 1);
        assert_eq!(status.error_count, 0);
    }

    #[test]
    fn event_log_records_events() {
        let mut mgr = AdapterManager::new(test_settings());
        let mut orch = test_orchestrator();

        mgr.start_adapter("msfs", &mut orch).unwrap();
        mgr.stop_adapter("msfs", &mut orch).unwrap();

        assert_eq!(mgr.event_log().len(), 2);
    }

    #[test]
    fn record_error_updates_health_and_orchestrator() {
        let mut mgr = AdapterManager::new(test_settings());
        let mut orch = test_orchestrator();

        mgr.record_error("msfs", "connection timeout", &mut orch);
        assert_eq!(mgr.adapter_health("msfs").unwrap().total_errors, 1);
        assert_eq!(mgr.adapter_health("msfs").unwrap().consecutive_errors, 1);
    }

    #[test]
    fn restart_adapter_works() {
        let mut mgr = AdapterManager::new(test_settings());
        let mut orch = test_orchestrator();

        mgr.start_adapter("msfs", &mut orch).unwrap();
        mgr.restart_adapter("msfs", &mut orch).unwrap();

        assert_eq!(
            mgr.adapter_health("msfs").unwrap().state,
            AdapterLifecycleState::Running
        );
    }

    #[test]
    fn errored_adapters_lists_errors() {
        let mut mgr = AdapterManager::new(test_settings());
        // Push past threshold.
        for _ in 0..3 {
            mgr.record_health_check("msfs", false);
        }
        let errored = mgr.errored_adapters();
        assert!(errored.contains(&"msfs".to_string()));
    }

    #[test]
    fn update_settings_takes_effect() {
        let mut mgr = AdapterManager::new(test_settings());
        assert_eq!(mgr.health_interval(), Duration::from_secs(5));

        let mut new_settings = test_settings();
        new_settings.health_interval = Duration::from_secs(10);
        mgr.update_settings(new_settings);

        assert_eq!(mgr.health_interval(), Duration::from_secs(10));
    }
}
