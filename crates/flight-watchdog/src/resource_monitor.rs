// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Resource monitoring with trait-based data sources.
//!
//! Defines thresholds for memory usage, thread count, and open handles.
//! The actual OS calls are abstracted behind [`ResourceSource`] so the
//! monitoring logic is pure and testable.

use serde::{Deserialize, Serialize};

/// Snapshot of process resource usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSnapshot {
    /// Memory usage in bytes.
    pub memory_bytes: u64,
    /// Number of active threads.
    pub thread_count: u32,
    /// Number of open handles / file descriptors.
    pub open_handles: u32,
}

/// Trait abstracting the source of resource data.
///
/// Implement this for platform-specific monitoring. For tests, use
/// [`StaticResourceSource`].
pub trait ResourceSource {
    /// Take a point-in-time snapshot of process resources.
    fn snapshot(&self) -> ResourceSnapshot;
}

/// A static resource source for testing.
pub struct StaticResourceSource {
    pub snapshot: ResourceSnapshot,
}

impl ResourceSource for StaticResourceSource {
    fn snapshot(&self) -> ResourceSnapshot {
        self.snapshot.clone()
    }
}

/// Thresholds for resource monitoring.
#[derive(Debug, Clone)]
pub struct ResourceThresholds {
    /// Memory warning threshold in bytes.
    pub memory_warn_bytes: u64,
    /// Memory critical threshold in bytes.
    pub memory_critical_bytes: u64,
    /// Thread count warning threshold.
    pub thread_warn_count: u32,
    /// Thread count critical threshold.
    pub thread_critical_count: u32,
    /// Open handles warning threshold.
    pub handle_warn_count: u32,
    /// Open handles critical threshold.
    pub handle_critical_count: u32,
}

impl Default for ResourceThresholds {
    fn default() -> Self {
        Self {
            memory_warn_bytes: 512 * 1024 * 1024,      // 512 MB
            memory_critical_bytes: 1024 * 1024 * 1024, // 1 GB
            thread_warn_count: 100,
            thread_critical_count: 500,
            handle_warn_count: 500,
            handle_critical_count: 2000,
        }
    }
}

/// Severity of a resource alert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ResourceSeverity {
    /// All resources within normal bounds.
    Normal,
    /// One or more resources approaching limits.
    Warning,
    /// One or more resources at critical levels.
    Critical,
}

/// A single resource alert detail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAlert {
    pub resource: String,
    pub severity: ResourceSeverity,
    pub message: String,
}

/// Result of evaluating resource usage against thresholds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceEvaluation {
    pub overall_severity: ResourceSeverity,
    pub alerts: Vec<ResourceAlert>,
    pub snapshot: ResourceSnapshot,
}

/// Evaluates resource snapshots against configured thresholds.
pub struct ResourceMonitor {
    thresholds: ResourceThresholds,
}

impl ResourceMonitor {
    /// Create a new resource monitor with the given thresholds.
    pub fn new(thresholds: ResourceThresholds) -> Self {
        Self { thresholds }
    }

    /// Evaluate a resource snapshot against thresholds.
    pub fn evaluate(&self, snapshot: &ResourceSnapshot) -> ResourceEvaluation {
        let mut alerts = Vec::new();
        let mut worst = ResourceSeverity::Normal;

        // Memory checks
        if snapshot.memory_bytes >= self.thresholds.memory_critical_bytes {
            alerts.push(ResourceAlert {
                resource: "memory".into(),
                severity: ResourceSeverity::Critical,
                message: format!(
                    "Memory at {} MB (critical: {} MB)",
                    snapshot.memory_bytes / (1024 * 1024),
                    self.thresholds.memory_critical_bytes / (1024 * 1024),
                ),
            });
            worst = ResourceSeverity::Critical;
        } else if snapshot.memory_bytes >= self.thresholds.memory_warn_bytes {
            alerts.push(ResourceAlert {
                resource: "memory".into(),
                severity: ResourceSeverity::Warning,
                message: format!(
                    "Memory at {} MB (warn: {} MB)",
                    snapshot.memory_bytes / (1024 * 1024),
                    self.thresholds.memory_warn_bytes / (1024 * 1024),
                ),
            });
            if worst < ResourceSeverity::Warning {
                worst = ResourceSeverity::Warning;
            }
        }

        // Thread count checks
        if snapshot.thread_count >= self.thresholds.thread_critical_count {
            alerts.push(ResourceAlert {
                resource: "threads".into(),
                severity: ResourceSeverity::Critical,
                message: format!(
                    "Thread count {} (critical: {})",
                    snapshot.thread_count, self.thresholds.thread_critical_count,
                ),
            });
            worst = ResourceSeverity::Critical;
        } else if snapshot.thread_count >= self.thresholds.thread_warn_count {
            alerts.push(ResourceAlert {
                resource: "threads".into(),
                severity: ResourceSeverity::Warning,
                message: format!(
                    "Thread count {} (warn: {})",
                    snapshot.thread_count, self.thresholds.thread_warn_count,
                ),
            });
            if worst < ResourceSeverity::Warning {
                worst = ResourceSeverity::Warning;
            }
        }

        // Handle checks
        if snapshot.open_handles >= self.thresholds.handle_critical_count {
            alerts.push(ResourceAlert {
                resource: "handles".into(),
                severity: ResourceSeverity::Critical,
                message: format!(
                    "Open handles {} (critical: {})",
                    snapshot.open_handles, self.thresholds.handle_critical_count,
                ),
            });
            worst = ResourceSeverity::Critical;
        } else if snapshot.open_handles >= self.thresholds.handle_warn_count {
            alerts.push(ResourceAlert {
                resource: "handles".into(),
                severity: ResourceSeverity::Warning,
                message: format!(
                    "Open handles {} (warn: {})",
                    snapshot.open_handles, self.thresholds.handle_warn_count,
                ),
            });
            if worst < ResourceSeverity::Warning {
                worst = ResourceSeverity::Warning;
            }
        }

        ResourceEvaluation {
            overall_severity: worst,
            snapshot: snapshot.clone(),
            alerts,
        }
    }

    /// Convenience: take a snapshot from a source and evaluate it.
    pub fn check(&self, source: &dyn ResourceSource) -> ResourceEvaluation {
        let snap = source.snapshot();
        self.evaluate(&snap)
    }
}

impl Default for ResourceMonitor {
    fn default() -> Self {
        Self::new(ResourceThresholds::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small_thresholds() -> ResourceThresholds {
        ResourceThresholds {
            memory_warn_bytes: 100,
            memory_critical_bytes: 200,
            thread_warn_count: 5,
            thread_critical_count: 10,
            handle_warn_count: 50,
            handle_critical_count: 100,
        }
    }

    #[test]
    fn normal_snapshot_no_alerts() {
        let mon = ResourceMonitor::new(small_thresholds());
        let snap = ResourceSnapshot {
            memory_bytes: 50,
            thread_count: 2,
            open_handles: 10,
        };
        let eval = mon.evaluate(&snap);
        assert_eq!(eval.overall_severity, ResourceSeverity::Normal);
        assert!(eval.alerts.is_empty());
    }

    #[test]
    fn memory_warning() {
        let mon = ResourceMonitor::new(small_thresholds());
        let snap = ResourceSnapshot {
            memory_bytes: 150,
            thread_count: 2,
            open_handles: 10,
        };
        let eval = mon.evaluate(&snap);
        assert_eq!(eval.overall_severity, ResourceSeverity::Warning);
        assert_eq!(eval.alerts.len(), 1);
        assert_eq!(eval.alerts[0].resource, "memory");
    }

    #[test]
    fn memory_critical() {
        let mon = ResourceMonitor::new(small_thresholds());
        let snap = ResourceSnapshot {
            memory_bytes: 300,
            thread_count: 2,
            open_handles: 10,
        };
        let eval = mon.evaluate(&snap);
        assert_eq!(eval.overall_severity, ResourceSeverity::Critical);
    }

    #[test]
    fn thread_warning() {
        let mon = ResourceMonitor::new(small_thresholds());
        let snap = ResourceSnapshot {
            memory_bytes: 10,
            thread_count: 7,
            open_handles: 10,
        };
        let eval = mon.evaluate(&snap);
        assert_eq!(eval.overall_severity, ResourceSeverity::Warning);
    }

    #[test]
    fn thread_critical() {
        let mon = ResourceMonitor::new(small_thresholds());
        let snap = ResourceSnapshot {
            memory_bytes: 10,
            thread_count: 15,
            open_handles: 10,
        };
        let eval = mon.evaluate(&snap);
        assert_eq!(eval.overall_severity, ResourceSeverity::Critical);
    }

    #[test]
    fn handle_warning() {
        let mon = ResourceMonitor::new(small_thresholds());
        let snap = ResourceSnapshot {
            memory_bytes: 10,
            thread_count: 2,
            open_handles: 60,
        };
        let eval = mon.evaluate(&snap);
        assert_eq!(eval.overall_severity, ResourceSeverity::Warning);
    }

    #[test]
    fn handle_critical() {
        let mon = ResourceMonitor::new(small_thresholds());
        let snap = ResourceSnapshot {
            memory_bytes: 10,
            thread_count: 2,
            open_handles: 150,
        };
        let eval = mon.evaluate(&snap);
        assert_eq!(eval.overall_severity, ResourceSeverity::Critical);
    }

    #[test]
    fn multiple_alerts_worst_wins() {
        let mon = ResourceMonitor::new(small_thresholds());
        let snap = ResourceSnapshot {
            memory_bytes: 150, // warning
            thread_count: 15,  // critical
            open_handles: 10,  // normal
        };
        let eval = mon.evaluate(&snap);
        assert_eq!(eval.overall_severity, ResourceSeverity::Critical);
        assert_eq!(eval.alerts.len(), 2);
    }

    #[test]
    fn static_source_works() {
        let mon = ResourceMonitor::new(small_thresholds());
        let source = StaticResourceSource {
            snapshot: ResourceSnapshot {
                memory_bytes: 50,
                thread_count: 2,
                open_handles: 10,
            },
        };
        let eval = mon.check(&source);
        assert_eq!(eval.overall_severity, ResourceSeverity::Normal);
    }

    #[test]
    fn default_thresholds_are_sane() {
        let t = ResourceThresholds::default();
        assert!(t.memory_warn_bytes < t.memory_critical_bytes);
        assert!(t.thread_warn_count < t.thread_critical_count);
        assert!(t.handle_warn_count < t.handle_critical_count);
    }
}
