// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Diagnostic bundle collection.
//!
//! Collects component states, recent health events, configuration snapshot,
//! and resource data into a single serializable bundle for support/debugging.

use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::heartbeat::{HeartbeatHealth, HeartbeatSummary};
use crate::resource_monitor::ResourceEvaluation;

/// A single component's state in the diagnostic bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentDiagnostic {
    pub name: String,
    pub health: HeartbeatHealth,
    pub consecutive_misses: u32,
    pub total_heartbeats: u64,
}

/// A recent event record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticEvent {
    pub source: String,
    pub message: String,
    pub elapsed: Duration,
}

/// Configuration snapshot included in the bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSnapshot {
    pub entries: HashMap<String, String>,
}

impl ConfigSnapshot {
    /// Create an empty config snapshot.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Add a configuration entry.
    pub fn add(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.entries.insert(key.into(), value.into());
    }
}

impl Default for ConfigSnapshot {
    fn default() -> Self {
        Self::new()
    }
}

/// A full diagnostic bundle for support/debugging.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticBundle {
    /// Bundle generation timestamp as duration since some epoch.
    pub generated_at: Duration,
    /// Component health states.
    pub components: Vec<ComponentDiagnostic>,
    /// Heartbeat summary.
    pub heartbeat_summary: HeartbeatSummary,
    /// Resource evaluation (if available).
    pub resource_evaluation: Option<ResourceEvaluation>,
    /// Recent events.
    pub recent_events: Vec<DiagnosticEvent>,
    /// Configuration snapshot.
    pub config_snapshot: ConfigSnapshot,
    /// Bundle version for forward compatibility.
    pub bundle_version: u32,
}

/// Builder for constructing diagnostic bundles.
pub struct DiagnosticBundleBuilder {
    generated_at: Duration,
    components: Vec<ComponentDiagnostic>,
    heartbeat_summary: Option<HeartbeatSummary>,
    resource_evaluation: Option<ResourceEvaluation>,
    recent_events: Vec<DiagnosticEvent>,
    config_snapshot: ConfigSnapshot,
}

impl DiagnosticBundleBuilder {
    /// Create a new builder.
    pub fn new(generated_at: Duration) -> Self {
        Self {
            generated_at,
            components: Vec::new(),
            heartbeat_summary: None,
            resource_evaluation: None,
            recent_events: Vec::new(),
            config_snapshot: ConfigSnapshot::new(),
        }
    }

    /// Add a component diagnostic.
    pub fn add_component(
        &mut self,
        name: &str,
        health: HeartbeatHealth,
        consecutive_misses: u32,
        total_heartbeats: u64,
    ) -> &mut Self {
        self.components.push(ComponentDiagnostic {
            name: name.to_string(),
            health,
            consecutive_misses,
            total_heartbeats,
        });
        self
    }

    /// Set the heartbeat summary.
    pub fn heartbeat_summary(&mut self, summary: HeartbeatSummary) -> &mut Self {
        self.heartbeat_summary = Some(summary);
        self
    }

    /// Set the resource evaluation.
    pub fn resource_evaluation(&mut self, eval: ResourceEvaluation) -> &mut Self {
        self.resource_evaluation = Some(eval);
        self
    }

    /// Add a recent event.
    pub fn add_event(&mut self, source: &str, message: &str, elapsed: Duration) -> &mut Self {
        self.recent_events.push(DiagnosticEvent {
            source: source.to_string(),
            message: message.to_string(),
            elapsed,
        });
        self
    }

    /// Set the configuration snapshot.
    pub fn config_snapshot(&mut self, config: ConfigSnapshot) -> &mut Self {
        self.config_snapshot = config;
        self
    }

    /// Build the diagnostic bundle.
    pub fn build(self) -> DiagnosticBundle {
        let heartbeat_summary = self.heartbeat_summary.unwrap_or(HeartbeatSummary {
            alive_count: 0,
            late_count: 0,
            stale_count: 0,
            unknown_count: 0,
            components: Vec::new(),
        });

        DiagnosticBundle {
            generated_at: self.generated_at,
            components: self.components,
            heartbeat_summary,
            resource_evaluation: self.resource_evaluation,
            recent_events: self.recent_events,
            config_snapshot: self.config_snapshot,
            bundle_version: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource_monitor::{ResourceSeverity, ResourceSnapshot};

    #[test]
    fn empty_bundle_serializes() {
        let bundle = DiagnosticBundleBuilder::new(Duration::from_secs(42)).build();
        let json = serde_json::to_string(&bundle).expect("should serialize");
        assert!(json.contains("\"bundle_version\":1"));
        assert!(json.contains("\"generated_at\""));
    }

    #[test]
    fn bundle_with_components() {
        let mut builder = DiagnosticBundleBuilder::new(Duration::from_secs(1));
        builder.add_component("axis", HeartbeatHealth::Alive, 0, 1000);
        builder.add_component("ffb", HeartbeatHealth::Stale, 5, 500);
        let bundle = builder.build();

        assert_eq!(bundle.components.len(), 2);
        assert_eq!(bundle.components[0].name, "axis");
        assert_eq!(bundle.components[1].health, HeartbeatHealth::Stale);
    }

    #[test]
    fn bundle_with_events() {
        let mut builder = DiagnosticBundleBuilder::new(Duration::from_secs(10));
        builder.add_event("monitor", "heartbeat missed", Duration::from_secs(5));
        builder.add_event("adapter", "disconnected", Duration::from_secs(8));
        let bundle = builder.build();

        assert_eq!(bundle.recent_events.len(), 2);
        assert_eq!(bundle.recent_events[0].source, "monitor");
    }

    #[test]
    fn bundle_with_config_snapshot() {
        let mut config = ConfigSnapshot::new();
        config.add("tick_interval_ms", "4");
        config.add("warn_threshold", "1");

        let mut builder = DiagnosticBundleBuilder::new(Duration::from_secs(1));
        builder.config_snapshot(config);
        let bundle = builder.build();

        assert_eq!(bundle.config_snapshot.entries.len(), 2);
        assert_eq!(
            bundle.config_snapshot.entries.get("tick_interval_ms"),
            Some(&"4".to_string())
        );
    }

    #[test]
    fn bundle_round_trip_json() {
        let mut builder = DiagnosticBundleBuilder::new(Duration::from_secs(99));
        builder.add_component("axis", HeartbeatHealth::Alive, 0, 42);
        builder.add_event("test", "event", Duration::from_millis(100));

        let bundle = builder.build();
        let json = serde_json::to_string(&bundle).unwrap();
        let deserialized: DiagnosticBundle = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.bundle_version, 1);
        assert_eq!(deserialized.components.len(), 1);
        assert_eq!(deserialized.recent_events.len(), 1);
    }

    #[test]
    fn bundle_with_resource_evaluation() {
        let eval = ResourceEvaluation {
            overall_severity: ResourceSeverity::Warning,
            alerts: vec![],
            snapshot: ResourceSnapshot {
                memory_bytes: 1024,
                thread_count: 4,
                open_handles: 10,
            },
        };

        let mut builder = DiagnosticBundleBuilder::new(Duration::from_secs(1));
        builder.resource_evaluation(eval);
        let bundle = builder.build();

        assert!(bundle.resource_evaluation.is_some());
        assert_eq!(
            bundle.resource_evaluation.unwrap().overall_severity,
            ResourceSeverity::Warning
        );
    }
}
