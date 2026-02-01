// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Health monitoring and watchdog integration for HOTAS devices.

use std::time::{Duration, Instant};

use flight_hid_support::ghost_filter::GhostFilterStats;
use flight_hid_support::saitek_hotas::SaitekHotasType;

/// Health status for a HOTAS device.
#[derive(Debug, Clone)]
pub struct HotasHealthStatus {
    /// Device type
    pub device_type: SaitekHotasType,
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
}

impl HotasHealthStatus {
    /// Check if the device is healthy.
    pub fn is_healthy(&self) -> bool {
        self.connected && self.consecutive_failures < 3 && self.ghost_rate < 0.1
    }

    /// Check if ghost input rate is concerning.
    pub fn has_ghost_issues(&self) -> bool {
        self.ghost_rate > 0.05
    }
}

/// Health monitor for HOTAS devices with watchdog integration.
#[derive(Debug)]
pub struct HotasHealthMonitor {
    device_type: SaitekHotasType,
    consecutive_failures: u32,
    last_success: Option<Instant>,
    last_health_check: Option<Instant>,
    health_check_interval: Duration,
    failure_threshold: u32,
}

impl HotasHealthMonitor {
    /// Failure count that triggers watchdog alert.
    pub const DEFAULT_FAILURE_THRESHOLD: u32 = 3;

    /// Minimum interval between health checks.
    pub const DEFAULT_HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(1);

    /// Create a new health monitor for the specified device.
    pub fn new(device_type: SaitekHotasType) -> Self {
        Self {
            device_type,
            consecutive_failures: 0,
            last_success: None,
            last_health_check: None,
            health_check_interval: Self::DEFAULT_HEALTH_CHECK_INTERVAL,
            failure_threshold: Self::DEFAULT_FAILURE_THRESHOLD,
        }
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
    ) -> HotasHealthStatus {
        HotasHealthStatus {
            device_type: self.device_type,
            connected,
            ghost_rate,
            consecutive_failures: self.consecutive_failures,
            last_success: self.last_success,
            ghost_stats,
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
        let mut monitor = HotasHealthMonitor::new(SaitekHotasType::X52Pro);

        monitor.record_success();
        assert!(!monitor.is_failed());
        assert!(monitor.last_success.is_some());
    }

    #[test]
    fn test_health_monitor_failure_threshold() {
        let mut monitor = HotasHealthMonitor::new(SaitekHotasType::X52Pro);

        assert!(!monitor.record_failure());
        assert!(!monitor.record_failure());
        assert!(monitor.record_failure()); // Third failure triggers
        assert!(monitor.is_failed());
    }

    #[test]
    fn test_health_monitor_reset_on_success() {
        let mut monitor = HotasHealthMonitor::new(SaitekHotasType::X52Pro);

        monitor.record_failure();
        monitor.record_failure();
        monitor.record_success();

        assert!(!monitor.is_failed());
        assert_eq!(monitor.consecutive_failures, 0);
    }

    #[test]
    fn test_health_status_healthy() {
        let status = HotasHealthStatus {
            device_type: SaitekHotasType::X52Pro,
            connected: true,
            ghost_rate: 0.01,
            consecutive_failures: 0,
            last_success: Some(Instant::now()),
            ghost_stats: GhostFilterStats::default(),
        };

        assert!(status.is_healthy());
        assert!(!status.has_ghost_issues());
    }

    #[test]
    fn test_health_status_ghost_issues() {
        let status = HotasHealthStatus {
            device_type: SaitekHotasType::X56Stick,
            connected: true,
            ghost_rate: 0.08,
            consecutive_failures: 0,
            last_success: Some(Instant::now()),
            ghost_stats: GhostFilterStats::default(),
        };

        assert!(status.is_healthy()); // Still healthy overall
        assert!(status.has_ghost_issues()); // But has ghost input concerns
    }
}
