// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Health monitoring for the VPforce Rhino.

use std::time::{Duration, Instant};

/// Health status snapshot.
#[derive(Debug, Clone)]
pub struct RhinoHealthStatus {
    pub connected: bool,
    pub consecutive_failures: u32,
    pub last_success: Option<Instant>,
    pub ghost_rate: f64,
}

impl RhinoHealthStatus {
    pub fn is_healthy(&self) -> bool {
        self.connected && self.consecutive_failures < 3 && self.ghost_rate < 0.1
    }
}

/// Monitor that tracks report successes/failures and ghost-input rate.
#[derive(Debug)]
pub struct RhinoHealthMonitor {
    consecutive_failures: u32,
    last_success: Option<Instant>,
    total_reports: u64,
    ghost_reports: u64,
    failure_threshold: u32,
}

impl RhinoHealthMonitor {
    pub const DEFAULT_FAILURE_THRESHOLD: u32 = 3;

    pub fn new() -> Self {
        Self {
            consecutive_failures: 0,
            last_success: None,
            total_reports: 0,
            ghost_reports: 0,
            failure_threshold: Self::DEFAULT_FAILURE_THRESHOLD,
        }
    }

    pub fn with_failure_threshold(mut self, threshold: u32) -> Self {
        self.failure_threshold = threshold;
        self
    }

    /// Record a successful report.
    pub fn record_success(&mut self, is_ghost: bool) {
        self.consecutive_failures = 0;
        self.last_success = Some(Instant::now());
        self.total_reports += 1;
        if is_ghost {
            self.ghost_reports += 1;
        }
    }

    /// Record a failed report read.
    pub fn record_failure(&mut self) {
        self.consecutive_failures += 1;
        self.total_reports += 1;
    }

    /// Current health snapshot.
    pub fn status(&self) -> RhinoHealthStatus {
        let ghost_rate = if self.total_reports == 0 {
            0.0
        } else {
            self.ghost_reports as f64 / self.total_reports as f64
        };
        RhinoHealthStatus {
            connected: self.consecutive_failures < self.failure_threshold,
            consecutive_failures: self.consecutive_failures,
            last_success: self.last_success,
            ghost_rate,
        }
    }

    /// Returns `true` if the device is considered offline.
    pub fn is_offline(&self) -> bool {
        self.consecutive_failures >= self.failure_threshold
    }

    /// Time since last successful report.
    pub fn time_since_last_success(&self) -> Option<Duration> {
        self.last_success.map(|t| t.elapsed())
    }
}

impl Default for RhinoHealthMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_monitor_is_connected() {
        let m = RhinoHealthMonitor::new();
        assert!(!m.is_offline());
    }

    #[test]
    fn test_three_failures_marks_offline() {
        let mut m = RhinoHealthMonitor::new();
        m.record_failure();
        m.record_failure();
        assert!(!m.is_offline());
        m.record_failure();
        assert!(m.is_offline());
    }

    #[test]
    fn test_success_resets_failure_count() {
        let mut m = RhinoHealthMonitor::new();
        m.record_failure();
        m.record_failure();
        m.record_success(false);
        assert_eq!(m.consecutive_failures, 0);
        assert!(!m.is_offline());
    }

    #[test]
    fn test_ghost_rate_calculation() {
        let mut m = RhinoHealthMonitor::new();
        m.record_success(false);
        m.record_success(true);
        m.record_success(true);
        m.record_success(false);
        let s = m.status();
        assert!((s.ghost_rate - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_high_ghost_rate_unhealthy() {
        let mut m = RhinoHealthMonitor::new();
        for _ in 0..20 {
            m.record_success(true);
        }
        assert!(!m.status().is_healthy());
    }
}
