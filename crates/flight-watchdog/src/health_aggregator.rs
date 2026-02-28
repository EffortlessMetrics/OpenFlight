// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Health aggregation and state-transition history.
//!
//! [`HealthAggregator`] performs periodic health checks on registered
//! subsystems, aggregates their state into a single dashboard-ready status,
//! and maintains a history of state transitions.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

// ── Subsystem health state ──────────────────────────────────────────────────

/// The health state of a single subsystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SubsystemHealth {
    /// Operating normally.
    Healthy,
    /// Functional with warnings.
    Warning,
    /// Partially functional.
    Degraded,
    /// Non-functional.
    Failed,
    /// Not yet checked.
    Unknown,
}

impl std::fmt::Display for SubsystemHealth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubsystemHealth::Healthy => write!(f, "Healthy"),
            SubsystemHealth::Warning => write!(f, "Warning"),
            SubsystemHealth::Degraded => write!(f, "Degraded"),
            SubsystemHealth::Failed => write!(f, "Failed"),
            SubsystemHealth::Unknown => write!(f, "Unknown"),
        }
    }
}

// ── Configuration ───────────────────────────────────────────────────────────

/// Configuration for a subsystem health check.
#[derive(Debug, Clone)]
pub struct SubsystemCheckConfig {
    /// Human-readable subsystem name.
    pub name: String,
    /// How often to run the health check.
    pub interval: Duration,
    /// How long a subsystem can go without a report before it is considered Unknown.
    pub staleness_timeout: Duration,
    /// Consecutive failures before marking as Failed.
    pub failure_threshold: u32,
    /// Consecutive warnings before marking as Degraded.
    pub warning_threshold: u32,
}

impl SubsystemCheckConfig {
    /// Create a config with sensible defaults.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            interval: Duration::from_secs(5),
            staleness_timeout: Duration::from_secs(30),
            failure_threshold: 3,
            warning_threshold: 5,
        }
    }

    /// Builder: set check interval.
    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }

    /// Builder: set staleness timeout.
    pub fn with_staleness_timeout(mut self, timeout: Duration) -> Self {
        self.staleness_timeout = timeout;
        self
    }

    /// Builder: set failure threshold.
    pub fn with_failure_threshold(mut self, threshold: u32) -> Self {
        self.failure_threshold = threshold;
        self
    }
}

// ── State transition record ─────────────────────────────────────────────────

/// Record of a subsystem health state change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthTransition {
    /// Subsystem that changed state.
    pub subsystem: String,
    /// Previous health state.
    pub from: SubsystemHealth,
    /// New health state.
    pub to: SubsystemHealth,
    /// Reason for the transition.
    pub reason: String,
    /// Monotonic timestamp (duration since aggregator creation).
    pub elapsed: Duration,
}

// ── Subsystem entry ─────────────────────────────────────────────────────────

#[derive(Debug)]
struct SubsystemEntry {
    config: SubsystemCheckConfig,
    state: SubsystemHealth,
    last_report: Option<Instant>,
    last_check_due: Instant,
    consecutive_warnings: u32,
    consecutive_failures: u32,
}

// ── Aggregate status (dashboard-ready) ──────────────────────────────────────

/// Dashboard-ready aggregated health status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateHealth {
    /// Overall system health (worst of all subsystems).
    pub overall: SubsystemHealth,
    /// Per-subsystem health state.
    pub subsystems: HashMap<String, SubsystemHealth>,
    /// Number of healthy subsystems.
    pub healthy_count: usize,
    /// Number of subsystems with warnings.
    pub warning_count: usize,
    /// Number of degraded subsystems.
    pub degraded_count: usize,
    /// Number of failed subsystems.
    pub failed_count: usize,
    /// Number of subsystems whose state is unknown.
    pub unknown_count: usize,
}

// ── Health aggregator ───────────────────────────────────────────────────────

/// Aggregates health from multiple subsystems and tracks transitions.
pub struct HealthAggregator {
    subsystems: HashMap<String, SubsystemEntry>,
    transitions: Vec<HealthTransition>,
    max_transitions: usize,
    start_time: Instant,
}

impl HealthAggregator {
    /// Create a new aggregator.
    pub fn new() -> Self {
        Self {
            subsystems: HashMap::new(),
            transitions: Vec::new(),
            max_transitions: 1000,
            start_time: Instant::now(),
        }
    }

    /// Register a subsystem for health tracking.
    pub fn register(&mut self, config: SubsystemCheckConfig) {
        let now = Instant::now();
        self.subsystems.insert(
            config.name.clone(),
            SubsystemEntry {
                config,
                state: SubsystemHealth::Unknown,
                last_report: None,
                last_check_due: now,
                consecutive_warnings: 0,
                consecutive_failures: 0,
            },
        );
    }

    /// Report a healthy check result for a subsystem.
    pub fn report_healthy(&mut self, subsystem: &str) {
        if let Some(entry) = self.subsystems.get_mut(subsystem) {
            entry.last_report = Some(Instant::now());
            entry.consecutive_warnings = 0;
            entry.consecutive_failures = 0;
            let old = entry.state;
            entry.state = SubsystemHealth::Healthy;
            if old != SubsystemHealth::Healthy {
                self.record_transition(subsystem, old, SubsystemHealth::Healthy, "check passed");
            }
        }
    }

    /// Report a warning for a subsystem.
    pub fn report_warning(&mut self, subsystem: &str, reason: &str) {
        if let Some(entry) = self.subsystems.get_mut(subsystem) {
            entry.last_report = Some(Instant::now());
            entry.consecutive_warnings += 1;
            entry.consecutive_failures = 0;
            let old = entry.state;

            let new_state = if entry.consecutive_warnings >= entry.config.warning_threshold {
                SubsystemHealth::Degraded
            } else {
                SubsystemHealth::Warning
            };

            entry.state = new_state;
            if old != new_state {
                self.record_transition(subsystem, old, new_state, reason);
            }
        }
    }

    /// Report a failure for a subsystem.
    pub fn report_failure(&mut self, subsystem: &str, reason: &str) {
        if let Some(entry) = self.subsystems.get_mut(subsystem) {
            entry.last_report = Some(Instant::now());
            entry.consecutive_failures += 1;
            entry.consecutive_warnings = 0;
            let old = entry.state;

            let new_state = if entry.consecutive_failures >= entry.config.failure_threshold {
                SubsystemHealth::Failed
            } else {
                SubsystemHealth::Degraded
            };

            entry.state = new_state;
            if old != new_state {
                self.record_transition(subsystem, old, new_state, reason);
            }
        }
    }

    /// Check for stale subsystems (those that haven't reported within their timeout).
    pub fn check_staleness(&mut self) {
        let names: Vec<String> = self.subsystems.keys().cloned().collect();
        for name in names {
            let (old, is_stale) = {
                let entry = &self.subsystems[&name];
                let is_stale = entry
                    .last_report
                    .map(|t| t.elapsed() > entry.config.staleness_timeout)
                    .unwrap_or(false);
                (entry.state, is_stale)
            };
            if is_stale && old != SubsystemHealth::Unknown {
                if let Some(entry) = self.subsystems.get_mut(&name) {
                    entry.state = SubsystemHealth::Unknown;
                }
                self.record_transition(&name, old, SubsystemHealth::Unknown, "staleness timeout");
            }
        }
    }

    /// Return which subsystems are due for a health check.
    pub fn due_checks(&self) -> Vec<String> {
        let now = Instant::now();
        self.subsystems
            .iter()
            .filter(|(_, entry)| now.duration_since(entry.last_check_due) >= entry.config.interval)
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Mark a subsystem's check as having been performed (resets the interval timer).
    pub fn mark_checked(&mut self, subsystem: &str) {
        if let Some(entry) = self.subsystems.get_mut(subsystem) {
            entry.last_check_due = Instant::now();
        }
    }

    /// Get the current health state of a subsystem.
    pub fn subsystem_health(&self, name: &str) -> SubsystemHealth {
        self.subsystems
            .get(name)
            .map(|e| e.state)
            .unwrap_or(SubsystemHealth::Unknown)
    }

    /// Build a dashboard-ready aggregate health report.
    pub fn aggregate(&self) -> AggregateHealth {
        let mut subsystems = HashMap::new();
        let mut healthy = 0usize;
        let mut warning = 0usize;
        let mut degraded = 0usize;
        let mut failed = 0usize;
        let mut unknown = 0usize;
        let mut worst = SubsystemHealth::Healthy;

        for (name, entry) in &self.subsystems {
            subsystems.insert(name.clone(), entry.state);
            match entry.state {
                SubsystemHealth::Healthy => healthy += 1,
                SubsystemHealth::Warning => {
                    warning += 1;
                    if worst == SubsystemHealth::Healthy {
                        worst = SubsystemHealth::Warning;
                    }
                }
                SubsystemHealth::Degraded => {
                    degraded += 1;
                    if worst == SubsystemHealth::Healthy || worst == SubsystemHealth::Warning {
                        worst = SubsystemHealth::Degraded;
                    }
                }
                SubsystemHealth::Failed => {
                    failed += 1;
                    worst = SubsystemHealth::Failed;
                }
                SubsystemHealth::Unknown => {
                    unknown += 1;
                    if worst == SubsystemHealth::Healthy {
                        worst = SubsystemHealth::Unknown;
                    }
                }
            }
        }

        AggregateHealth {
            overall: worst,
            subsystems,
            healthy_count: healthy,
            warning_count: warning,
            degraded_count: degraded,
            failed_count: failed,
            unknown_count: unknown,
        }
    }

    /// Return the history of health state transitions.
    pub fn transitions(&self) -> &[HealthTransition] {
        &self.transitions
    }

    /// Return transitions for a specific subsystem.
    pub fn transitions_for(&self, subsystem: &str) -> Vec<&HealthTransition> {
        self.transitions
            .iter()
            .filter(|t| t.subsystem == subsystem)
            .collect()
    }

    fn record_transition(
        &mut self,
        subsystem: &str,
        from: SubsystemHealth,
        to: SubsystemHealth,
        reason: &str,
    ) {
        if self.transitions.len() >= self.max_transitions {
            self.transitions.remove(0);
        }
        self.transitions.push(HealthTransition {
            subsystem: subsystem.to_string(),
            from,
            to,
            reason: reason.to_string(),
            elapsed: self.start_time.elapsed(),
        });
    }
}

impl Default for HealthAggregator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(name: &str) -> SubsystemCheckConfig {
        SubsystemCheckConfig::new(name)
            .with_interval(Duration::from_millis(100))
            .with_staleness_timeout(Duration::from_millis(500))
            .with_failure_threshold(3)
    }

    #[test]
    fn new_subsystem_starts_unknown() {
        let mut agg = HealthAggregator::new();
        agg.register(make_config("axis"));
        assert_eq!(agg.subsystem_health("axis"), SubsystemHealth::Unknown);
    }

    #[test]
    fn report_healthy_transitions_to_healthy() {
        let mut agg = HealthAggregator::new();
        agg.register(make_config("axis"));
        agg.report_healthy("axis");
        assert_eq!(agg.subsystem_health("axis"), SubsystemHealth::Healthy);
    }

    #[test]
    fn report_warning_transitions_to_warning() {
        let mut agg = HealthAggregator::new();
        agg.register(make_config("ffb"));
        agg.report_healthy("ffb");
        agg.report_warning("ffb", "high latency");
        assert_eq!(agg.subsystem_health("ffb"), SubsystemHealth::Warning);
    }

    #[test]
    fn consecutive_warnings_escalate_to_degraded() {
        let mut agg = HealthAggregator::new();
        let mut cfg = make_config("hid");
        cfg.warning_threshold = 3;
        agg.register(cfg);
        agg.report_healthy("hid");
        for _ in 0..3 {
            agg.report_warning("hid", "slow");
        }
        assert_eq!(agg.subsystem_health("hid"), SubsystemHealth::Degraded);
    }

    #[test]
    fn report_failure_transitions_to_degraded() {
        let mut agg = HealthAggregator::new();
        agg.register(make_config("bus"));
        agg.report_healthy("bus");
        agg.report_failure("bus", "error");
        assert_eq!(agg.subsystem_health("bus"), SubsystemHealth::Degraded);
    }

    #[test]
    fn consecutive_failures_escalate_to_failed() {
        let mut agg = HealthAggregator::new();
        agg.register(make_config("panel"));
        agg.report_healthy("panel");
        for _ in 0..3 {
            agg.report_failure("panel", "disconnected");
        }
        assert_eq!(agg.subsystem_health("panel"), SubsystemHealth::Failed);
    }

    #[test]
    fn healthy_report_resets_failure_count() {
        let mut agg = HealthAggregator::new();
        agg.register(make_config("x"));
        agg.report_failure("x", "err");
        agg.report_failure("x", "err");
        agg.report_healthy("x");
        assert_eq!(agg.subsystem_health("x"), SubsystemHealth::Healthy);
        // One more failure should not reach threshold
        agg.report_failure("x", "err");
        assert_eq!(agg.subsystem_health("x"), SubsystemHealth::Degraded);
    }

    #[test]
    fn aggregate_returns_worst_overall() {
        let mut agg = HealthAggregator::new();
        agg.register(make_config("a"));
        agg.register(make_config("b"));
        agg.register(make_config("c"));
        agg.report_healthy("a");
        agg.report_warning("b", "slow");
        agg.report_healthy("c");
        let report = agg.aggregate();
        assert_eq!(report.overall, SubsystemHealth::Warning);
        assert_eq!(report.healthy_count, 2);
        assert_eq!(report.warning_count, 1);
    }

    #[test]
    fn aggregate_all_healthy() {
        let mut agg = HealthAggregator::new();
        agg.register(make_config("a"));
        agg.register(make_config("b"));
        agg.report_healthy("a");
        agg.report_healthy("b");
        let report = agg.aggregate();
        assert_eq!(report.overall, SubsystemHealth::Healthy);
        assert_eq!(report.healthy_count, 2);
    }

    #[test]
    fn transitions_are_recorded() {
        let mut agg = HealthAggregator::new();
        agg.register(make_config("x"));
        agg.report_healthy("x"); // Unknown -> Healthy
        agg.report_failure("x", "err"); // Healthy -> Degraded
        assert_eq!(agg.transitions().len(), 2);
        assert_eq!(agg.transitions()[0].from, SubsystemHealth::Unknown);
        assert_eq!(agg.transitions()[0].to, SubsystemHealth::Healthy);
        assert_eq!(agg.transitions()[1].from, SubsystemHealth::Healthy);
        assert_eq!(agg.transitions()[1].to, SubsystemHealth::Degraded);
    }

    #[test]
    fn transitions_for_filters_by_subsystem() {
        let mut agg = HealthAggregator::new();
        agg.register(make_config("a"));
        agg.register(make_config("b"));
        agg.report_healthy("a");
        agg.report_healthy("b");
        agg.report_failure("a", "err");
        let a_transitions = agg.transitions_for("a");
        assert_eq!(a_transitions.len(), 2); // Unknown->Healthy, Healthy->Degraded
        let b_transitions = agg.transitions_for("b");
        assert_eq!(b_transitions.len(), 1); // Unknown->Healthy
    }

    #[test]
    fn staleness_marks_unknown() {
        let mut agg = HealthAggregator::new();
        let mut cfg = make_config("stale");
        cfg.staleness_timeout = Duration::from_millis(1);
        agg.register(cfg);
        agg.report_healthy("stale");
        assert_eq!(agg.subsystem_health("stale"), SubsystemHealth::Healthy);
        std::thread::sleep(Duration::from_millis(5));
        agg.check_staleness();
        assert_eq!(agg.subsystem_health("stale"), SubsystemHealth::Unknown);
    }

    #[test]
    fn unknown_subsystem_returns_unknown() {
        let agg = HealthAggregator::new();
        assert_eq!(agg.subsystem_health("ghost"), SubsystemHealth::Unknown);
    }

    #[test]
    fn due_checks_returns_names_past_interval() {
        let mut agg = HealthAggregator::new();
        let mut cfg = make_config("fast");
        cfg.interval = Duration::from_millis(1);
        agg.register(cfg);
        std::thread::sleep(Duration::from_millis(5));
        let due = agg.due_checks();
        assert!(due.contains(&"fast".to_string()));
    }

    #[test]
    fn mark_checked_resets_interval_timer() {
        let mut agg = HealthAggregator::new();
        let mut cfg = make_config("x");
        cfg.interval = Duration::from_secs(60);
        agg.register(cfg);
        agg.mark_checked("x");
        let due = agg.due_checks();
        assert!(due.is_empty());
    }

    #[test]
    fn aggregate_with_failed_subsystem() {
        let mut agg = HealthAggregator::new();
        agg.register(make_config("a"));
        agg.register(make_config("b"));
        agg.report_healthy("a");
        for _ in 0..3 {
            agg.report_failure("b", "down");
        }
        let report = agg.aggregate();
        assert_eq!(report.overall, SubsystemHealth::Failed);
        assert_eq!(report.failed_count, 1);
    }
}
