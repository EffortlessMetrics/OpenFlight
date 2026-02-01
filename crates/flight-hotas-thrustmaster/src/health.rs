// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Health monitoring and watchdog integration for T.Flight HOTAS devices.

use std::time::{Duration, Instant};

use flight_hid_support::device_support::TFlightModel;
use flight_hid_support::ghost_filter::GhostFilterStats;

/// Health status for a T.Flight HOTAS device.
#[derive(Debug, Clone)]
pub struct TFlightHealthStatus {
    /// Device type
    pub device_type: TFlightModel,
    /// Whether the device is currently connected
    pub connected: bool,
    /// Ghost input rate (0.0 to 1.0)
    pub ghost_rate: f64,
    /// Consecutive communication failures
    pub consecutive_failures: u32,
    /// Last successful communication timestamp
    pub last_success: Option<Instant>,
    /// Ghost filter statistics
    pub ghost_stats: GhostFilterStats,
    /// Whether using legacy PID
    pub is_legacy_pid: bool,
}

impl TFlightHealthStatus {
    /// Check if the device is healthy.
    pub fn is_healthy(&self) -> bool {
        self.connected && self.consecutive_failures < 3 && self.ghost_rate < 0.1
    }

    /// Check if ghost input rate is concerning.
    pub fn has_ghost_issues(&self) -> bool {
        self.ghost_rate > 0.05
    }
}

/// Health monitor for T.Flight HOTAS devices with watchdog integration.
#[derive(Debug)]
pub struct TFlightHealthMonitor {
    device_type: TFlightModel,
    consecutive_failures: u32,
    last_success: Option<Instant>,
    last_health_check: Option<Instant>,
    health_check_interval: Duration,
    failure_threshold: u32,
    is_legacy_pid: bool,
}

impl TFlightHealthMonitor {
    /// Failure count that triggers watchdog alert.
    pub const DEFAULT_FAILURE_THRESHOLD: u32 = 3;

    /// Minimum interval between health checks.
    pub const DEFAULT_HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(1);

    /// Create a new health monitor for the specified device.
    pub fn new(device_type: TFlightModel) -> Self {
        Self {
            device_type,
            consecutive_failures: 0,
            last_success: None,
            last_health_check: None,
            health_check_interval: Self::DEFAULT_HEALTH_CHECK_INTERVAL,
            failure_threshold: Self::DEFAULT_FAILURE_THRESHOLD,
            is_legacy_pid: false,
        }
    }

    /// Mark that this device was detected via legacy PID.
    pub fn with_legacy_pid(mut self, is_legacy: bool) -> Self {
        self.is_legacy_pid = is_legacy;
        self
    }

    /// Record a successful operation.
    pub fn record_success(&mut self) {
        self.consecutive_failures = 0;
        self.last_success = Some(Instant::now());
    }

    /// Record a failed operation.
    ///
    /// Returns `true` if the failure threshold has been exceeded.
    pub fn record_failure(&mut self) -> bool {
        self.consecutive_failures += 1;
        self.consecutive_failures >= self.failure_threshold
    }

    /// Check if enough time has passed for another health check.
    pub fn should_check_health(&self) -> bool {
        match self.last_health_check {
            None => true,
            Some(last) => last.elapsed() >= self.health_check_interval,
        }
    }

    /// Mark that a health check was performed.
    pub fn mark_health_checked(&mut self) {
        self.last_health_check = Some(Instant::now());
    }

    /// Get the current health status.
    pub fn status(
        &self,
        connected: bool,
        ghost_rate: f64,
        ghost_stats: GhostFilterStats,
    ) -> TFlightHealthStatus {
        TFlightHealthStatus {
            device_type: self.device_type,
            connected,
            ghost_rate,
            consecutive_failures: self.consecutive_failures,
            last_success: self.last_success,
            ghost_stats,
            is_legacy_pid: self.is_legacy_pid,
        }
    }

    /// Check if the device should be considered failed.
    pub fn is_failed(&self) -> bool {
        self.consecutive_failures >= self.failure_threshold
    }

    /// Reset the monitor state.
    pub fn reset(&mut self) {
        self.consecutive_failures = 0;
        self.last_success = None;
        self.last_health_check = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_monitor_success() {
        let mut monitor = TFlightHealthMonitor::new(TFlightModel::Hotas4);

        monitor.record_success();
        assert!(!monitor.is_failed());
        assert!(monitor.last_success.is_some());
    }

    #[test]
    fn test_health_monitor_failure_threshold() {
        let mut monitor = TFlightHealthMonitor::new(TFlightModel::Hotas4);

        assert!(!monitor.record_failure());
        assert!(!monitor.record_failure());
        assert!(monitor.record_failure()); // Third failure triggers
        assert!(monitor.is_failed());
    }

    #[test]
    fn test_health_monitor_reset_on_success() {
        let mut monitor = TFlightHealthMonitor::new(TFlightModel::Hotas4);

        monitor.record_failure();
        monitor.record_failure();
        monitor.record_success();

        assert!(!monitor.is_failed());
        assert_eq!(monitor.consecutive_failures, 0);
    }

    #[test]
    fn test_health_status_healthy() {
        let status = TFlightHealthStatus {
            device_type: TFlightModel::Hotas4,
            connected: true,
            ghost_rate: 0.01,
            consecutive_failures: 0,
            last_success: Some(Instant::now()),
            ghost_stats: GhostFilterStats::default(),
            is_legacy_pid: false,
        };

        assert!(status.is_healthy());
        assert!(!status.has_ghost_issues());
    }

    #[test]
    fn test_health_status_ghost_issues() {
        let status = TFlightHealthStatus {
            device_type: TFlightModel::Hotas4,
            connected: true,
            ghost_rate: 0.08,
            consecutive_failures: 0,
            last_success: Some(Instant::now()),
            ghost_stats: GhostFilterStats::default(),
            is_legacy_pid: false,
        };

        assert!(status.is_healthy()); // Still healthy overall
        assert!(status.has_ghost_issues()); // But has ghost input concerns
    }

    #[test]
    fn test_legacy_pid_tracking() {
        let monitor = TFlightHealthMonitor::new(TFlightModel::Hotas4).with_legacy_pid(true);

        let status = monitor.status(true, 0.0, GhostFilterStats::default());
        assert!(status.is_legacy_pid);
    }
}
