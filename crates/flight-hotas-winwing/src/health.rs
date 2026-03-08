// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Health monitoring for WinWing devices.

use std::time::{Duration, Instant};

/// WinWing product variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WinWingDevice {
    Orion2Throttle,
    Orion2Stick,
    TfrpRudder,
    F16ExStick,
    SuperTaurus,
    SuperLibra,
    UfcPanel,
    F16Icp,
    MfdPanel,
    CombatReadyPanel,
    TakeOffPanel,
    SkywalkerRudder,
}

impl std::fmt::Display for WinWingDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Orion2Throttle => f.write_str("WinWing Orion 2 Throttle"),
            Self::Orion2Stick => f.write_str("WinWing Orion 2 Stick"),
            Self::TfrpRudder => f.write_str("WinWing TFRP Rudder Pedals"),
            Self::F16ExStick => f.write_str("WinWing F-16EX Stick"),
            Self::SuperTaurus => f.write_str("WinWing Super Taurus F-15EX Throttle"),
            Self::SuperLibra => f.write_str("WinWing Super Libra Joystick Base"),
            Self::UfcPanel => f.write_str("WinWing UFC1 + HUD1 Panel"),
            Self::F16Icp => f.write_str("WinWing F-16 ICP"),
            Self::MfdPanel => f.write_str("WinWing MFD Panel"),
            Self::CombatReadyPanel => f.write_str("WinWing F/A-18 Combat Ready Panel"),
            Self::TakeOffPanel => f.write_str("WinWing F/A-18 Take Off Panel"),
            Self::SkywalkerRudder => f.write_str("WinWing Skywalker Metal Rudder Pedals"),
        }
    }
}

/// Health snapshot for a WinWing device.
#[derive(Debug, Clone)]
pub struct WinWingHealthStatus {
    pub device: WinWingDevice,
    pub connected: bool,
    pub consecutive_failures: u32,
    pub last_success: Option<Instant>,
}

impl WinWingHealthStatus {
    pub fn is_healthy(&self) -> bool {
        self.connected && self.consecutive_failures < 3
    }
}

/// Health monitor for a single WinWing device.
#[derive(Debug)]
pub struct WinWingHealthMonitor {
    device: WinWingDevice,
    consecutive_failures: u32,
    last_success: Option<Instant>,
    failure_threshold: u32,
}

impl WinWingHealthMonitor {
    pub const DEFAULT_FAILURE_THRESHOLD: u32 = 3;

    pub fn new(device: WinWingDevice) -> Self {
        Self {
            device,
            consecutive_failures: 0,
            last_success: None,
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

    pub fn is_offline(&self) -> bool {
        self.consecutive_failures >= self.failure_threshold
    }

    pub fn status(&self) -> WinWingHealthStatus {
        WinWingHealthStatus {
            device: self.device,
            connected: !self.is_offline(),
            consecutive_failures: self.consecutive_failures,
            last_success: self.last_success,
        }
    }

    pub fn time_since_last_success(&self) -> Option<Duration> {
        self.last_success.map(|t| t.elapsed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_monitor_connected() {
        let m = WinWingHealthMonitor::new(WinWingDevice::Orion2Throttle);
        assert!(!m.is_offline());
        assert!(m.status().is_healthy());
    }

    #[test]
    fn test_three_failures_marks_offline() {
        let mut m = WinWingHealthMonitor::new(WinWingDevice::TfrpRudder);
        m.record_failure();
        m.record_failure();
        assert!(!m.is_offline());
        m.record_failure();
        assert!(m.is_offline());
    }

    #[test]
    fn test_success_resets_failure_count() {
        let mut m = WinWingHealthMonitor::new(WinWingDevice::Orion2Stick);
        m.record_failure();
        m.record_failure();
        m.record_success();
        assert!(!m.is_offline());
    }

    #[test]
    fn test_device_display() {
        assert_eq!(
            WinWingDevice::Orion2Throttle.to_string(),
            "WinWing Orion 2 Throttle"
        );
        assert_eq!(
            WinWingDevice::TfrpRudder.to_string(),
            "WinWing TFRP Rudder Pedals"
        );
    }
}
