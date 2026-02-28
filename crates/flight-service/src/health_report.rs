// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Comprehensive Health Report
//!
//! Aggregates per-component health, system metrics, and overall status
//! into a single serialisable report suitable for diagnostics and IPC.

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Top-level health status classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Critical,
    Unknown,
}

impl HealthStatus {
    /// Returns a numeric severity (higher is worse).
    fn severity(self) -> u8 {
        match self {
            Self::Healthy => 0,
            Self::Unknown => 1,
            Self::Degraded => 2,
            Self::Critical => 3,
        }
    }

    /// Return the worse of two statuses.
    #[must_use]
    pub fn worse(self, other: Self) -> Self {
        if self.severity() >= other.severity() {
            self
        } else {
            other
        }
    }
}

/// Health information for a single component.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub name: String,
    pub status: HealthStatus,
    pub latency_us: Option<u64>,
    pub error_count: u64,
    pub last_error: Option<String>,
}

/// System-level resource metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealth {
    pub cpu_usage_percent: f64,
    pub memory_used_bytes: u64,
    pub thread_count: u32,
    pub open_handles: u32,
}

/// A point-in-time health snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    pub status: HealthStatus,
    pub uptime_secs: u64,
    pub components: Vec<ComponentHealth>,
    pub system: SystemHealth,
    pub timestamp_epoch_ms: u64,
}

impl HealthReport {
    /// Derive the worst status across all components.
    #[must_use]
    pub fn worst_status(&self) -> HealthStatus {
        self.components
            .iter()
            .fold(HealthStatus::Healthy, |acc, c| acc.worse(c.status))
    }

    /// Return components whose status is not `Healthy`.
    #[must_use]
    pub fn failing_components(&self) -> Vec<&ComponentHealth> {
        self.components
            .iter()
            .filter(|c| c.status != HealthStatus::Healthy)
            .collect()
    }

    /// Serialise the report as a JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

/// Builder for constructing a [`HealthReport`].
pub struct HealthReportBuilder {
    uptime_secs: u64,
    components: Vec<ComponentHealth>,
    system: Option<SystemHealth>,
}

impl HealthReportBuilder {
    #[must_use]
    pub fn new(uptime_secs: u64) -> Self {
        Self {
            uptime_secs,
            components: Vec::new(),
            system: None,
        }
    }

    pub fn add_component(&mut self, component: ComponentHealth) -> &mut Self {
        self.components.push(component);
        self
    }

    pub fn set_system(&mut self, system: SystemHealth) -> &mut Self {
        self.system = Some(system);
        self
    }

    #[must_use]
    pub fn build(&mut self) -> HealthReport {
        let status = self
            .components
            .iter()
            .fold(HealthStatus::Healthy, |acc, c| acc.worse(c.status));

        let system = self.system.take().unwrap_or(SystemHealth {
            cpu_usage_percent: 0.0,
            memory_used_bytes: 0,
            thread_count: 0,
            open_handles: 0,
        });

        let timestamp_epoch_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        HealthReport {
            status,
            uptime_secs: self.uptime_secs,
            components: std::mem::take(&mut self.components),
            system,
            timestamp_epoch_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn healthy_component(name: &str) -> ComponentHealth {
        ComponentHealth {
            name: name.to_string(),
            status: HealthStatus::Healthy,
            latency_us: Some(100),
            error_count: 0,
            last_error: None,
        }
    }

    fn degraded_component(name: &str) -> ComponentHealth {
        ComponentHealth {
            name: name.to_string(),
            status: HealthStatus::Degraded,
            latency_us: Some(5000),
            error_count: 3,
            last_error: Some("timeout".to_string()),
        }
    }

    fn critical_component(name: &str) -> ComponentHealth {
        ComponentHealth {
            name: name.to_string(),
            status: HealthStatus::Critical,
            latency_us: None,
            error_count: 10,
            last_error: Some("connection refused".to_string()),
        }
    }

    #[test]
    fn builder_creates_healthy_report() {
        let report = HealthReportBuilder::new(120)
            .add_component(healthy_component("axis"))
            .build();

        assert_eq!(report.status, HealthStatus::Healthy);
        assert_eq!(report.uptime_secs, 120);
        assert_eq!(report.components.len(), 1);
    }

    #[test]
    fn builder_derives_worst_status() {
        let report = HealthReportBuilder::new(60)
            .add_component(healthy_component("axis"))
            .add_component(degraded_component("hid"))
            .build();

        assert_eq!(report.status, HealthStatus::Degraded);
    }

    #[test]
    fn builder_with_critical_component() {
        let report = HealthReportBuilder::new(10)
            .add_component(healthy_component("bus"))
            .add_component(critical_component("ffb"))
            .build();

        assert_eq!(report.status, HealthStatus::Critical);
    }

    #[test]
    fn worst_status_matches_build_status() {
        let report = HealthReportBuilder::new(0)
            .add_component(healthy_component("a"))
            .add_component(degraded_component("b"))
            .add_component(critical_component("c"))
            .build();

        assert_eq!(report.worst_status(), HealthStatus::Critical);
        assert_eq!(report.status, report.worst_status());
    }

    #[test]
    fn failing_components_excludes_healthy() {
        let report = HealthReportBuilder::new(0)
            .add_component(healthy_component("ok"))
            .add_component(degraded_component("slow"))
            .add_component(critical_component("down"))
            .build();

        let failing = report.failing_components();
        assert_eq!(failing.len(), 2);
        assert!(failing.iter().any(|c| c.name == "slow"));
        assert!(failing.iter().any(|c| c.name == "down"));
    }

    #[test]
    fn failing_components_empty_when_all_healthy() {
        let report = HealthReportBuilder::new(0)
            .add_component(healthy_component("a"))
            .add_component(healthy_component("b"))
            .build();

        assert!(report.failing_components().is_empty());
    }

    #[test]
    fn to_json_roundtrip() {
        let report = HealthReportBuilder::new(42)
            .add_component(healthy_component("axis"))
            .set_system(SystemHealth {
                cpu_usage_percent: 12.5,
                memory_used_bytes: 1024 * 1024,
                thread_count: 8,
                open_handles: 32,
            })
            .build();

        let json = report.to_json().unwrap();
        let parsed: HealthReport = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.uptime_secs, 42);
        assert_eq!(parsed.components.len(), 1);
        assert_eq!(parsed.system.thread_count, 8);
    }

    #[test]
    fn empty_report_is_healthy() {
        let report = HealthReportBuilder::new(0).build();
        assert_eq!(report.status, HealthStatus::Healthy);
        assert!(report.components.is_empty());
    }

    #[test]
    fn system_defaults_when_not_set() {
        let report = HealthReportBuilder::new(0).build();
        assert_eq!(report.system.cpu_usage_percent, 0.0);
        assert_eq!(report.system.memory_used_bytes, 0);
    }

    #[test]
    fn timestamp_is_plausible() {
        let report = HealthReportBuilder::new(0).build();
        // Should be after 2024-01-01 (1_704_067_200_000 ms)
        assert!(report.timestamp_epoch_ms > 1_704_067_200_000);
    }

    #[test]
    fn health_status_worse_ordering() {
        assert_eq!(
            HealthStatus::Healthy.worse(HealthStatus::Degraded),
            HealthStatus::Degraded
        );
        assert_eq!(
            HealthStatus::Critical.worse(HealthStatus::Healthy),
            HealthStatus::Critical
        );
        assert_eq!(
            HealthStatus::Unknown.worse(HealthStatus::Degraded),
            HealthStatus::Degraded
        );
        assert_eq!(
            HealthStatus::Healthy.worse(HealthStatus::Healthy),
            HealthStatus::Healthy
        );
    }
}
