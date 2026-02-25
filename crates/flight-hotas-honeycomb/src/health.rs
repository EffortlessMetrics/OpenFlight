// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Health monitoring for Honeycomb devices.

use std::time::{Duration, Instant};

/// Consecutive-error threshold before the device is considered unhealthy.
const ERROR_THRESHOLD: u32 = 5;

/// Grace period after a successful read before timeout is declared.
const STALE_TIMEOUT: Duration = Duration::from_secs(2);

/// Health status of a Honeycomb device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoneycombHealth {
    /// Device is responding and reports are valid.
    Healthy,
    /// Reports are arriving but parse errors are accumulating.
    Degraded,
    /// No reports received within [`STALE_TIMEOUT`].
    Stale,
    /// Device has been disconnected or too many errors have occurred.
    Disconnected,
}

/// Lightweight health monitor for a Honeycomb device.
///
/// Call [`HoneycombHealthMonitor::record_success`] on each valid report and
/// [`HoneycombHealthMonitor::record_error`] on each parse/read error.
/// Query the current status with [`HoneycombHealthMonitor::status`].
#[derive(Debug)]
pub struct HoneycombHealthMonitor {
    last_success: Option<Instant>,
    consecutive_errors: u32,
}

impl Default for HoneycombHealthMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl HoneycombHealthMonitor {
    pub fn new() -> Self {
        Self {
            last_success: None,
            consecutive_errors: 0,
        }
    }

    /// Record a successful report parse.
    pub fn record_success(&mut self) {
        self.last_success = Some(Instant::now());
        self.consecutive_errors = 0;
    }

    /// Record a failed report parse or read error.
    pub fn record_error(&mut self) {
        self.consecutive_errors = self.consecutive_errors.saturating_add(1);
    }

    /// Returns the current health status.
    pub fn status(&self) -> HoneycombHealth {
        if self.consecutive_errors >= ERROR_THRESHOLD {
            return HoneycombHealth::Disconnected;
        }

        match self.last_success {
            None => HoneycombHealth::Stale,
            Some(t) if t.elapsed() > STALE_TIMEOUT => HoneycombHealth::Stale,
            _ if self.consecutive_errors > 0 => HoneycombHealth::Degraded,
            _ => HoneycombHealth::Healthy,
        }
    }

    /// Returns `true` if the device is considered healthy.
    pub fn is_healthy(&self) -> bool {
        self.status() == HoneycombHealth::Healthy
    }

    /// Reset the monitor (e.g., on reconnect).
    pub fn reset(&mut self) {
        self.last_success = None;
        self.consecutive_errors = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state_is_stale() {
        let m = HoneycombHealthMonitor::new();
        assert_eq!(m.status(), HoneycombHealth::Stale);
        assert!(!m.is_healthy());
    }

    #[test]
    fn test_healthy_after_success() {
        let mut m = HoneycombHealthMonitor::new();
        m.record_success();
        assert_eq!(m.status(), HoneycombHealth::Healthy);
        assert!(m.is_healthy());
    }

    #[test]
    fn test_degraded_after_one_error() {
        let mut m = HoneycombHealthMonitor::new();
        m.record_success();
        m.record_error();
        assert_eq!(m.status(), HoneycombHealth::Degraded);
    }

    #[test]
    fn test_disconnected_after_threshold_errors() {
        let mut m = HoneycombHealthMonitor::new();
        m.record_success();
        for _ in 0..ERROR_THRESHOLD {
            m.record_error();
        }
        assert_eq!(m.status(), HoneycombHealth::Disconnected);
    }

    #[test]
    fn test_recovery_after_reset() {
        let mut m = HoneycombHealthMonitor::new();
        m.record_success();
        for _ in 0..ERROR_THRESHOLD {
            m.record_error();
        }
        assert_eq!(m.status(), HoneycombHealth::Disconnected);

        m.reset();
        assert_eq!(m.status(), HoneycombHealth::Stale);

        m.record_success();
        assert_eq!(m.status(), HoneycombHealth::Healthy);
    }
}
