// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Heartbeat monitoring for tracked components.
//!
//! Each monitored component (axis engine, FFB engine, adapters) records
//! heartbeats. The [`HeartbeatMonitor`] detects stale heartbeats and
//! reports per-component health.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// Health state derived from heartbeat freshness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HeartbeatHealth {
    /// Heartbeat received within the expected interval.
    Alive,
    /// Heartbeat is late but within tolerance.
    Late,
    /// No heartbeat received within the deadline — component is stale.
    Stale,
    /// Component has never sent a heartbeat.
    Unknown,
}

impl std::fmt::Display for HeartbeatHealth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Alive => write!(f, "Alive"),
            Self::Late => write!(f, "Late"),
            Self::Stale => write!(f, "Stale"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Configuration for a single heartbeat source.
#[derive(Debug, Clone)]
pub struct HeartbeatConfig {
    /// Expected interval between heartbeats.
    pub expected_interval: Duration,
    /// How many missed intervals before the component is considered late.
    pub late_threshold: u32,
    /// How many missed intervals before the component is considered stale.
    pub stale_threshold: u32,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            expected_interval: Duration::from_millis(4), // 250 Hz
            late_threshold: 2,
            stale_threshold: 10,
        }
    }
}

/// Per-component heartbeat tracking state.
#[derive(Debug)]
struct HeartbeatEntry {
    config: HeartbeatConfig,
    last_heartbeat: Option<Instant>,
    health: HeartbeatHealth,
    total_heartbeats: u64,
    total_misses: u64,
    consecutive_misses: u32,
}

/// Summary of a single component's heartbeat state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatStatus {
    pub component: String,
    pub health: HeartbeatHealth,
    pub total_heartbeats: u64,
    pub total_misses: u64,
    pub consecutive_misses: u32,
}

/// Aggregate heartbeat summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatSummary {
    pub alive_count: usize,
    pub late_count: usize,
    pub stale_count: usize,
    pub unknown_count: usize,
    pub components: Vec<HeartbeatStatus>,
}

/// Tracks heartbeats from multiple monitored components.
pub struct HeartbeatMonitor {
    entries: HashMap<String, HeartbeatEntry>,
}

impl HeartbeatMonitor {
    /// Create a new empty monitor.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Register a component for heartbeat tracking.
    pub fn register(&mut self, name: &str, config: HeartbeatConfig) {
        self.entries.insert(
            name.to_string(),
            HeartbeatEntry {
                config,
                last_heartbeat: None,
                health: HeartbeatHealth::Unknown,
                total_heartbeats: 0,
                total_misses: 0,
                consecutive_misses: 0,
            },
        );
    }

    /// Record a heartbeat from a component.
    pub fn record_heartbeat(&mut self, name: &str, now: Instant) {
        if let Some(entry) = self.entries.get_mut(name) {
            entry.last_heartbeat = Some(now);
            entry.total_heartbeats += 1;
            entry.consecutive_misses = 0;
            entry.health = HeartbeatHealth::Alive;
        }
    }

    /// Check all components for staleness at the given instant.
    pub fn check_all(&mut self, now: Instant) {
        for entry in self.entries.values_mut() {
            update_health(entry, now);
        }
    }

    /// Check a single component and return its health.
    pub fn check(&mut self, name: &str, now: Instant) -> HeartbeatHealth {
        if let Some(entry) = self.entries.get_mut(name) {
            update_health(entry, now);
            entry.health
        } else {
            HeartbeatHealth::Unknown
        }
    }

    /// Get the current health of a component without updating.
    pub fn health(&self, name: &str) -> HeartbeatHealth {
        self.entries
            .get(name)
            .map(|e| e.health)
            .unwrap_or(HeartbeatHealth::Unknown)
    }

    /// Get the consecutive miss count for a component.
    pub fn consecutive_misses(&self, name: &str) -> u32 {
        self.entries
            .get(name)
            .map(|e| e.consecutive_misses)
            .unwrap_or(0)
    }

    /// Returns true if all registered components are alive.
    pub fn all_alive(&self) -> bool {
        !self.entries.is_empty()
            && self
                .entries
                .values()
                .all(|e| e.health == HeartbeatHealth::Alive)
    }

    /// Returns names of stale components.
    pub fn stale_components(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|(_, e)| e.health == HeartbeatHealth::Stale)
            .map(|(k, _)| k.clone())
            .collect()
    }

    /// Build a full summary of all tracked components.
    pub fn summary(&self) -> HeartbeatSummary {
        let mut alive = 0;
        let mut late = 0;
        let mut stale = 0;
        let mut unknown = 0;
        let mut components = Vec::new();

        for (name, entry) in &self.entries {
            match entry.health {
                HeartbeatHealth::Alive => alive += 1,
                HeartbeatHealth::Late => late += 1,
                HeartbeatHealth::Stale => stale += 1,
                HeartbeatHealth::Unknown => unknown += 1,
            }
            components.push(HeartbeatStatus {
                component: name.clone(),
                health: entry.health,
                total_heartbeats: entry.total_heartbeats,
                total_misses: entry.total_misses,
                consecutive_misses: entry.consecutive_misses,
            });
        }

        // Sort for deterministic output.
        components.sort_by(|a, b| a.component.cmp(&b.component));

        HeartbeatSummary {
            alive_count: alive,
            late_count: late,
            stale_count: stale,
            unknown_count: unknown,
            components,
        }
    }

    /// Number of registered components.
    pub fn component_count(&self) -> usize {
        self.entries.len()
    }
}

impl Default for HeartbeatMonitor {
    fn default() -> Self {
        Self::new()
    }
}

fn update_health(entry: &mut HeartbeatEntry, now: Instant) {
    let Some(last) = entry.last_heartbeat else {
        // Never received a heartbeat — stay Unknown.
        return;
    };

    let elapsed = now.saturating_duration_since(last);
    let interval_ns = entry.config.expected_interval.as_nanos().max(1);
    let missed = (elapsed.as_nanos() / interval_ns).min(u128::from(u32::MAX)) as u32;

    if missed >= entry.config.stale_threshold {
        let delta = missed.saturating_sub(entry.consecutive_misses);
        entry.total_misses += u64::from(delta);
        entry.consecutive_misses = missed;
        entry.health = HeartbeatHealth::Stale;
    } else if missed >= entry.config.late_threshold {
        let delta = missed.saturating_sub(entry.consecutive_misses);
        entry.total_misses += u64::from(delta);
        entry.consecutive_misses = missed;
        entry.health = HeartbeatHealth::Late;
    } else {
        entry.consecutive_misses = 0;
        entry.health = HeartbeatHealth::Alive;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(interval_ms: u64, late: u32, stale: u32) -> HeartbeatConfig {
        HeartbeatConfig {
            expected_interval: Duration::from_millis(interval_ms),
            late_threshold: late,
            stale_threshold: stale,
        }
    }

    #[test]
    fn new_component_is_unknown() {
        let mut mon = HeartbeatMonitor::new();
        mon.register("axis", HeartbeatConfig::default());
        assert_eq!(mon.health("axis"), HeartbeatHealth::Unknown);
    }

    #[test]
    fn heartbeat_makes_alive() {
        let mut mon = HeartbeatMonitor::new();
        mon.register("axis", make_config(10, 2, 5));
        let now = Instant::now();
        mon.record_heartbeat("axis", now);
        mon.check("axis", now);
        assert_eq!(mon.health("axis"), HeartbeatHealth::Alive);
    }

    #[test]
    fn late_detection() {
        let mut mon = HeartbeatMonitor::new();
        mon.register("ffb", make_config(10, 2, 5));
        let t0 = Instant::now();
        mon.record_heartbeat("ffb", t0);

        // 25ms later = 2.5 intervals ≥ late_threshold=2
        let t1 = t0 + Duration::from_millis(25);
        let h = mon.check("ffb", t1);
        assert_eq!(h, HeartbeatHealth::Late);
    }

    #[test]
    fn stale_detection() {
        let mut mon = HeartbeatMonitor::new();
        mon.register("adapter", make_config(10, 2, 5));
        let t0 = Instant::now();
        mon.record_heartbeat("adapter", t0);

        // 60ms later = 6 intervals ≥ stale_threshold=5
        let t1 = t0 + Duration::from_millis(60);
        let h = mon.check("adapter", t1);
        assert_eq!(h, HeartbeatHealth::Stale);
    }

    #[test]
    fn recovery_from_stale() {
        let mut mon = HeartbeatMonitor::new();
        mon.register("axis", make_config(10, 2, 5));
        let t0 = Instant::now();
        mon.record_heartbeat("axis", t0);

        // Go stale.
        let t1 = t0 + Duration::from_millis(60);
        mon.check("axis", t1);
        assert_eq!(mon.health("axis"), HeartbeatHealth::Stale);

        // Recover with a new heartbeat.
        mon.record_heartbeat("axis", t1);
        mon.check("axis", t1);
        assert_eq!(mon.health("axis"), HeartbeatHealth::Alive);
    }

    #[test]
    fn multi_component_isolation() {
        let mut mon = HeartbeatMonitor::new();
        mon.register("axis", make_config(10, 2, 5));
        mon.register("ffb", make_config(10, 2, 5));

        let t0 = Instant::now();
        mon.record_heartbeat("axis", t0);
        mon.record_heartbeat("ffb", t0);

        // Only ffb goes stale.
        let t1 = t0 + Duration::from_millis(60);
        mon.record_heartbeat("axis", t1);
        mon.check_all(t1);

        assert_eq!(mon.health("axis"), HeartbeatHealth::Alive);
        assert_eq!(mon.health("ffb"), HeartbeatHealth::Stale);
    }

    #[test]
    fn all_alive_when_all_healthy() {
        let mut mon = HeartbeatMonitor::new();
        mon.register("a", make_config(10, 2, 5));
        mon.register("b", make_config(10, 2, 5));
        let now = Instant::now();
        mon.record_heartbeat("a", now);
        mon.record_heartbeat("b", now);
        mon.check_all(now);
        assert!(mon.all_alive());
    }

    #[test]
    fn all_alive_false_when_one_stale() {
        let mut mon = HeartbeatMonitor::new();
        mon.register("a", make_config(10, 2, 5));
        mon.register("b", make_config(10, 2, 5));
        let t0 = Instant::now();
        mon.record_heartbeat("a", t0);
        mon.record_heartbeat("b", t0);

        let t1 = t0 + Duration::from_millis(60);
        mon.record_heartbeat("a", t1);
        mon.check_all(t1);
        assert!(!mon.all_alive());
    }

    #[test]
    fn stale_components_lists_only_stale() {
        let mut mon = HeartbeatMonitor::new();
        mon.register("a", make_config(10, 2, 5));
        mon.register("b", make_config(10, 2, 5));
        let t0 = Instant::now();
        mon.record_heartbeat("a", t0);
        mon.record_heartbeat("b", t0);

        let t1 = t0 + Duration::from_millis(60);
        mon.record_heartbeat("a", t1);
        mon.check_all(t1);

        let stale = mon.stale_components();
        assert_eq!(stale, vec!["b"]);
    }

    #[test]
    fn summary_counts_are_correct() {
        let mut mon = HeartbeatMonitor::new();
        mon.register("alive1", make_config(10, 2, 5));
        mon.register("alive2", make_config(10, 2, 5));
        mon.register("stale1", make_config(10, 2, 5));
        mon.register("unknown1", make_config(10, 2, 5));

        let t0 = Instant::now();
        mon.record_heartbeat("alive1", t0);
        mon.record_heartbeat("alive2", t0);
        mon.record_heartbeat("stale1", t0);

        let t1 = t0 + Duration::from_millis(60);
        mon.record_heartbeat("alive1", t1);
        mon.record_heartbeat("alive2", t1);
        mon.check_all(t1);

        let s = mon.summary();
        assert_eq!(s.alive_count, 2);
        assert_eq!(s.stale_count, 1);
        assert_eq!(s.unknown_count, 1);
    }

    #[test]
    fn unregistered_component_is_unknown() {
        let mon = HeartbeatMonitor::new();
        assert_eq!(mon.health("ghost"), HeartbeatHealth::Unknown);
    }

    #[test]
    fn consecutive_misses_tracked() {
        let mut mon = HeartbeatMonitor::new();
        mon.register("x", make_config(10, 2, 5));
        let t0 = Instant::now();
        mon.record_heartbeat("x", t0);

        let t1 = t0 + Duration::from_millis(35);
        mon.check("x", t1);
        assert!(mon.consecutive_misses("x") >= 2);
    }

    #[test]
    fn component_count_correct() {
        let mut mon = HeartbeatMonitor::new();
        assert_eq!(mon.component_count(), 0);
        mon.register("a", HeartbeatConfig::default());
        mon.register("b", HeartbeatConfig::default());
        assert_eq!(mon.component_count(), 2);
    }

    #[test]
    fn heartbeat_within_interval_stays_alive() {
        let mut mon = HeartbeatMonitor::new();
        mon.register("x", make_config(100, 2, 5));
        let t0 = Instant::now();
        mon.record_heartbeat("x", t0);

        // 50ms later = 0.5 intervals < late_threshold=2
        let t1 = t0 + Duration::from_millis(50);
        mon.check("x", t1);
        assert_eq!(mon.health("x"), HeartbeatHealth::Alive);
    }
}
