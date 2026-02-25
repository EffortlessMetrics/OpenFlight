// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Health monitoring for VKB STECS devices.

use std::time::{Duration, Instant};

use flight_hid_support::device_support::VkbStecsVariant;

/// Health status for one STECS physical device.
#[derive(Debug, Clone)]
pub struct StecsHealthStatus {
    /// Device variant.
    pub variant: VkbStecsVariant,
    /// Whether at least one interface is currently connected.
    pub connected: bool,
    /// Number of interfaces detected for this physical device.
    pub interface_count: u8,
    /// Number of virtual controllers that delivered reports this cycle.
    pub active_virtual_controller_count: u8,
    /// Consecutive read/parse failures.
    pub consecutive_failures: u32,
    /// Last successful ingest timestamp.
    pub last_success: Option<Instant>,
}

impl StecsHealthStatus {
    /// Check if the device is currently healthy.
    pub fn is_healthy(&self) -> bool {
        self.connected && self.consecutive_failures < 3
    }
}

/// Health monitor for STECS ingest path.
#[derive(Debug)]
pub struct StecsHealthMonitor {
    variant: VkbStecsVariant,
    consecutive_failures: u32,
    last_success: Option<Instant>,
    last_health_check: Option<Instant>,
    health_check_interval: Duration,
    failure_threshold: u32,
}

impl StecsHealthMonitor {
    /// Failure count that triggers degraded state.
    pub const DEFAULT_FAILURE_THRESHOLD: u32 = 3;
    /// Minimum interval between health checks.
    pub const DEFAULT_HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(1);

    /// Create monitor for one STECS variant.
    pub fn new(variant: VkbStecsVariant) -> Self {
        Self {
            variant,
            consecutive_failures: 0,
            last_success: None,
            last_health_check: None,
            health_check_interval: Self::DEFAULT_HEALTH_CHECK_INTERVAL,
            failure_threshold: Self::DEFAULT_FAILURE_THRESHOLD,
        }
    }

    /// Record successful operation.
    pub fn record_success(&mut self) {
        self.consecutive_failures = 0;
        self.last_success = Some(Instant::now());
    }

    /// Record failed operation.
    ///
    /// Returns `true` when failure threshold is exceeded.
    pub fn record_failure(&mut self) -> bool {
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        self.consecutive_failures >= self.failure_threshold
    }

    /// Check if enough time passed for periodic health work.
    pub fn should_check_health(&self) -> bool {
        match self.last_health_check {
            None => true,
            Some(last) => last.elapsed() >= self.health_check_interval,
        }
    }

    /// Mark a health check as completed.
    pub fn mark_health_checked(&mut self) {
        self.last_health_check = Some(Instant::now());
    }

    /// Build current health status.
    pub fn status(
        &self,
        connected: bool,
        interface_count: u8,
        active_virtual_controller_count: u8,
    ) -> StecsHealthStatus {
        StecsHealthStatus {
            variant: self.variant,
            connected,
            interface_count,
            active_virtual_controller_count,
            consecutive_failures: self.consecutive_failures,
            last_success: self.last_success,
        }
    }

    /// Whether failure threshold is currently exceeded.
    pub fn is_failed(&self) -> bool {
        self.consecutive_failures >= self.failure_threshold
    }

    /// Reset monitor to initial state.
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
    fn test_health_monitor_success_and_failure_threshold() {
        let mut monitor = StecsHealthMonitor::new(VkbStecsVariant::RightSpaceThrottleGripMini);
        assert!(!monitor.record_failure());
        assert!(!monitor.record_failure());
        assert!(monitor.record_failure());
        assert!(monitor.is_failed());

        monitor.record_success();
        assert!(!monitor.is_failed());
    }

    #[test]
    fn test_health_status_is_healthy() {
        let monitor = StecsHealthMonitor::new(VkbStecsVariant::LeftSpaceThrottleGripStandard);
        let status = monitor.status(true, 3, 2);
        assert!(status.is_healthy());
    }

    #[test]
    fn test_health_status_unhealthy_when_not_connected() {
        let monitor = StecsHealthMonitor::new(VkbStecsVariant::LeftSpaceThrottleGripMini);
        let status = monitor.status(false, 0, 0);
        assert!(!status.is_healthy());
    }

    #[test]
    fn test_should_check_health_initially_true() {
        let monitor = StecsHealthMonitor::new(VkbStecsVariant::RightSpaceThrottleGripMini);
        assert!(monitor.should_check_health(), "should be true initially");
    }

    #[test]
    fn test_mark_health_checked_suppresses_immediate_recheck() {
        let mut monitor = StecsHealthMonitor::new(VkbStecsVariant::RightSpaceThrottleGripMini);
        monitor.mark_health_checked();
        // Just after marking, the interval hasn't passed yet
        assert!(
            !monitor.should_check_health(),
            "should not recheck immediately after mark"
        );
    }

    #[test]
    fn test_reset_clears_all_state() {
        let mut monitor = StecsHealthMonitor::new(VkbStecsVariant::RightSpaceThrottleGripMiniPlus);
        monitor.record_failure();
        monitor.record_failure();
        monitor.record_success();
        monitor.mark_health_checked();

        monitor.reset();
        assert!(!monitor.is_failed(), "is_failed should be false after reset");
        assert!(
            monitor.should_check_health(),
            "should_check_health should be true after reset"
        );
    }
}
