// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Health monitoring for Moza bases.

use std::time::{Duration, Instant};

/// Health snapshot for a Moza base.
#[derive(Debug, Clone)]
pub struct MozaHealthStatus {
    pub connected: bool,
    pub consecutive_failures: u32,
    pub last_success: Option<Instant>,
    pub torque_fault: bool,
}

impl MozaHealthStatus {
    pub fn is_healthy(&self) -> bool {
        self.connected && self.consecutive_failures < 3 && !self.torque_fault
    }
}

/// Health monitor for a Moza FFB base.
#[derive(Debug)]
pub struct MozaHealthMonitor {
    consecutive_failures: u32,
    last_success: Option<Instant>,
    torque_fault: bool,
    failure_threshold: u32,
}

impl MozaHealthMonitor {
    pub const DEFAULT_FAILURE_THRESHOLD: u32 = 3;

    pub fn new() -> Self {
        Self {
            consecutive_failures: 0,
            last_success: None,
            torque_fault: false,
            failure_threshold: Self::DEFAULT_FAILURE_THRESHOLD,
        }
    }

    pub fn record_success(&mut self) {
        self.consecutive_failures = 0;
        self.last_success = Some(Instant::now());
    }

    pub fn record_failure(&mut self) {
        self.consecutive_failures += 1;
    }

    /// Flag a torque fault (e.g. over-temperature, overcurrent).
    pub fn set_torque_fault(&mut self, fault: bool) {
        self.torque_fault = fault;
    }

    pub fn is_offline(&self) -> bool {
        self.consecutive_failures >= self.failure_threshold
    }

    pub fn status(&self) -> MozaHealthStatus {
        MozaHealthStatus {
            connected: !self.is_offline(),
            consecutive_failures: self.consecutive_failures,
            last_success: self.last_success,
            torque_fault: self.torque_fault,
        }
    }

    pub fn time_since_last_success(&self) -> Option<Duration> {
        self.last_success.map(|t| t.elapsed())
    }
}

impl Default for MozaHealthMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_is_healthy() {
        let m = MozaHealthMonitor::new();
        assert!(m.status().is_healthy());
    }

    #[test]
    fn test_three_failures_offline() {
        let mut m = MozaHealthMonitor::new();
        m.record_failure();
        m.record_failure();
        assert!(!m.is_offline());
        m.record_failure();
        assert!(m.is_offline());
    }

    #[test]
    fn test_torque_fault_makes_unhealthy() {
        let mut m = MozaHealthMonitor::new();
        m.set_torque_fault(true);
        assert!(!m.status().is_healthy());
    }

    #[test]
    fn test_clear_torque_fault_restores_health() {
        let mut m = MozaHealthMonitor::new();
        m.set_torque_fault(true);
        m.set_torque_fault(false);
        assert!(m.status().is_healthy());
    }

    #[test]
    fn test_success_resets_failures() {
        let mut m = MozaHealthMonitor::new();
        m.record_failure();
        m.record_failure();
        m.record_success();
        assert!(!m.is_offline());
    }
}
