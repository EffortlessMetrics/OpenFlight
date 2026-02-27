// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Health-check registry for watchdog-monitored components.

use std::time::{Duration, Instant};

/// Result of a health check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    /// Component is operating normally.
    Healthy,
    /// Component is functional but in a degraded state.
    Degraded(String),
    /// Component is non-functional.
    Unhealthy(String),
}

/// A registered health check for a single component.
pub struct HealthCheck {
    /// Human-readable component name.
    pub name: String,
    /// Maximum interval between reports before the check is considered timed out.
    pub timeout: Duration,
    /// When the last report was received.
    pub last_check: Option<Instant>,
    /// Most recent reported status.
    pub last_status: HealthStatus,
    /// Number of consecutive non-healthy reports.
    pub consecutive_failures: u32,
    /// How many consecutive failures trigger an alert.
    pub max_failures_before_alert: u32,
}

/// Aggregate counts returned by [`HealthCheckManager::summary`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthSummary {
    pub healthy: usize,
    pub degraded: usize,
    pub unhealthy: usize,
    pub timed_out: usize,
}

/// Manages multiple [`HealthCheck`] instances.
pub struct HealthCheckManager {
    checks: Vec<HealthCheck>,
}

impl HealthCheckManager {
    /// Create an empty manager.
    pub fn new() -> Self {
        Self { checks: Vec::new() }
    }

    /// Register a new component for health monitoring.
    pub fn register(&mut self, name: &str, timeout: Duration, max_failures: u32) {
        self.checks.push(HealthCheck {
            name: name.to_string(),
            timeout,
            last_check: None,
            last_status: HealthStatus::Healthy,
            consecutive_failures: 0,
            max_failures_before_alert: max_failures,
        });
    }

    /// Report that a component is healthy, resetting its failure counter.
    pub fn report_healthy(&mut self, name: &str) {
        if let Some(check) = self.find_mut(name) {
            check.last_status = HealthStatus::Healthy;
            check.last_check = Some(Instant::now());
            check.consecutive_failures = 0;
        }
    }

    /// Report that a component is in a degraded state.
    pub fn report_degraded(&mut self, name: &str, reason: &str) {
        if let Some(check) = self.find_mut(name) {
            check.last_status = HealthStatus::Degraded(reason.to_string());
            check.last_check = Some(Instant::now());
            check.consecutive_failures += 1;
        }
    }

    /// Report that a component is unhealthy.
    pub fn report_unhealthy(&mut self, name: &str, reason: &str) {
        if let Some(check) = self.find_mut(name) {
            check.last_status = HealthStatus::Unhealthy(reason.to_string());
            check.last_check = Some(Instant::now());
            check.consecutive_failures += 1;
        }
    }

    /// Query the last reported status of a component.
    pub fn check_status(&self, name: &str) -> Option<&HealthStatus> {
        self.find(name).map(|c| &c.last_status)
    }

    /// Returns `true` only if every registered check is [`HealthStatus::Healthy`]
    /// and none have timed out.
    pub fn is_all_healthy(&self) -> bool {
        self.checks
            .iter()
            .all(|c| c.last_status == HealthStatus::Healthy)
            && self.timed_out_checks().is_empty()
    }

    /// Return references to all checks that are currently unhealthy.
    pub fn unhealthy_checks(&self) -> Vec<&HealthCheck> {
        self.checks
            .iter()
            .filter(|c| matches!(c.last_status, HealthStatus::Unhealthy(_)))
            .collect()
    }

    /// Return references to checks that have not reported within their timeout.
    pub fn timed_out_checks(&self) -> Vec<&HealthCheck> {
        let now = Instant::now();
        self.checks
            .iter()
            .filter(|c| match c.last_check {
                Some(t) => now.duration_since(t) > c.timeout,
                // Never reported — not timed out yet (no baseline).
                None => false,
            })
            .collect()
    }

    /// Build an aggregate [`HealthSummary`].
    pub fn summary(&self) -> HealthSummary {
        let mut s = HealthSummary {
            healthy: 0,
            degraded: 0,
            unhealthy: 0,
            timed_out: self.timed_out_checks().len(),
        };
        for c in &self.checks {
            match c.last_status {
                HealthStatus::Healthy => s.healthy += 1,
                HealthStatus::Degraded(_) => s.degraded += 1,
                HealthStatus::Unhealthy(_) => s.unhealthy += 1,
            }
        }
        s
    }

    // ── helpers ──────────────────────────────────────────────

    fn find(&self, name: &str) -> Option<&HealthCheck> {
        self.checks.iter().find(|c| c.name == name)
    }

    fn find_mut(&mut self, name: &str) -> Option<&mut HealthCheck> {
        self.checks.iter_mut().find(|c| c.name == name)
    }
}

impl Default for HealthCheckManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_check_status() {
        let mut mgr = HealthCheckManager::new();
        mgr.register("axis", Duration::from_secs(5), 3);
        assert_eq!(mgr.check_status("axis"), Some(&HealthStatus::Healthy));
    }

    #[test]
    fn report_healthy_updates_status() {
        let mut mgr = HealthCheckManager::new();
        mgr.register("axis", Duration::from_secs(5), 3);
        mgr.report_unhealthy("axis", "bad");
        mgr.report_healthy("axis");
        assert_eq!(mgr.check_status("axis"), Some(&HealthStatus::Healthy));
        assert_eq!(mgr.find("axis").unwrap().consecutive_failures, 0);
    }

    #[test]
    fn report_degraded_increments_failures() {
        let mut mgr = HealthCheckManager::new();
        mgr.register("ffb", Duration::from_secs(5), 3);
        mgr.report_degraded("ffb", "high latency");
        let check = mgr.find("ffb").unwrap();
        assert_eq!(
            check.last_status,
            HealthStatus::Degraded("high latency".into())
        );
        assert_eq!(check.consecutive_failures, 1);
    }

    #[test]
    fn report_unhealthy_increments_failures() {
        let mut mgr = HealthCheckManager::new();
        mgr.register("hid", Duration::from_secs(5), 3);
        mgr.report_unhealthy("hid", "device lost");
        let check = mgr.find("hid").unwrap();
        assert_eq!(
            check.last_status,
            HealthStatus::Unhealthy("device lost".into())
        );
        assert_eq!(check.consecutive_failures, 1);
    }

    #[test]
    fn consecutive_failures_accumulate() {
        let mut mgr = HealthCheckManager::new();
        mgr.register("panel", Duration::from_secs(5), 5);
        mgr.report_unhealthy("panel", "err1");
        mgr.report_degraded("panel", "err2");
        mgr.report_unhealthy("panel", "err3");
        assert_eq!(mgr.find("panel").unwrap().consecutive_failures, 3);
    }

    #[test]
    fn is_all_healthy_with_mixed_states() {
        let mut mgr = HealthCheckManager::new();
        mgr.register("a", Duration::from_secs(5), 3);
        mgr.register("b", Duration::from_secs(5), 3);
        assert!(mgr.is_all_healthy());
        mgr.report_degraded("b", "slow");
        assert!(!mgr.is_all_healthy());
    }

    #[test]
    fn unhealthy_checks_filter() {
        let mut mgr = HealthCheckManager::new();
        mgr.register("a", Duration::from_secs(5), 3);
        mgr.register("b", Duration::from_secs(5), 3);
        mgr.register("c", Duration::from_secs(5), 3);
        mgr.report_unhealthy("a", "dead");
        mgr.report_degraded("b", "slow");
        let unhealthy: Vec<_> = mgr
            .unhealthy_checks()
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(unhealthy, vec!["a"]);
    }

    #[test]
    fn summary_counts() {
        let mut mgr = HealthCheckManager::new();
        mgr.register("a", Duration::from_secs(5), 3);
        mgr.register("b", Duration::from_secs(5), 3);
        mgr.register("c", Duration::from_secs(5), 3);
        mgr.report_degraded("b", "slow");
        mgr.report_unhealthy("c", "dead");
        let s = mgr.summary();
        assert_eq!(s.healthy, 1);
        assert_eq!(s.degraded, 1);
        assert_eq!(s.unhealthy, 1);
    }

    #[test]
    fn unknown_component_is_none() {
        let mgr = HealthCheckManager::new();
        assert_eq!(mgr.check_status("ghost"), None);
    }

    #[test]
    fn report_healthy_resets_consecutive_failures() {
        let mut mgr = HealthCheckManager::new();
        mgr.register("x", Duration::from_secs(5), 3);
        mgr.report_unhealthy("x", "err");
        mgr.report_unhealthy("x", "err");
        assert_eq!(mgr.find("x").unwrap().consecutive_failures, 2);
        mgr.report_healthy("x");
        assert_eq!(mgr.find("x").unwrap().consecutive_failures, 0);
    }
}
