// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID device health monitoring.
//!
//! Periodically checks device status, tracks packet rates, error rates,
//! and latency to classify each device as healthy, degraded, unresponsive,
//! or disconnected.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Error-rate threshold (1 %) above which a device is considered degraded.
const ERROR_RATE_THRESHOLD: f64 = 0.01;

/// Latency threshold (in microseconds) above which a device is degraded.
const HIGH_LATENCY_US: u64 = 5_000;

/// Health status of a single device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceHealth {
    Healthy,
    Degraded(String),
    Unresponsive,
    Disconnected,
    Unknown,
}

/// Detailed health report for a device.
#[derive(Debug, Clone)]
pub struct DeviceHealthReport {
    pub device_id: String,
    pub health: DeviceHealth,
    pub last_data_received: Option<Instant>,
    pub packet_count: u64,
    pub error_count: u64,
    pub error_rate: f64,
    pub avg_latency_us: u64,
    pub max_latency_us: u64,
    pub uptime: Duration,
}

/// Monitors health of multiple HID devices.
pub struct HealthMonitor {
    devices: HashMap<String, DeviceMetrics>,
    #[allow(dead_code)]
    check_interval: Duration,
    unresponsive_timeout: Duration,
}

struct DeviceMetrics {
    first_seen: Instant,
    last_data: Option<Instant>,
    packet_count: u64,
    error_count: u64,
    latency_sum_us: u64,
    latency_max_us: u64,
    latency_samples: u64,
}

impl DeviceMetrics {
    fn new() -> Self {
        Self {
            first_seen: Instant::now(),
            last_data: None,
            packet_count: 0,
            error_count: 0,
            latency_sum_us: 0,
            latency_max_us: 0,
            latency_samples: 0,
        }
    }

    fn error_rate(&self) -> f64 {
        let total = self.packet_count + self.error_count;
        if total == 0 {
            return 0.0;
        }
        self.error_count as f64 / total as f64
    }

    fn avg_latency_us(&self) -> u64 {
        if self.latency_samples == 0 {
            return 0;
        }
        self.latency_sum_us / self.latency_samples
    }
}

impl HealthMonitor {
    /// Create a new monitor with the given check interval and unresponsive timeout.
    pub fn new(check_interval: Duration, unresponsive_timeout: Duration) -> Self {
        Self {
            devices: HashMap::new(),
            check_interval,
            unresponsive_timeout,
        }
    }

    /// Register a device for monitoring.
    pub fn register_device(&mut self, device_id: &str) {
        self.devices
            .entry(device_id.to_owned())
            .or_insert_with(DeviceMetrics::new);
    }

    /// Unregister a device. Returns `true` if it was present.
    pub fn unregister_device(&mut self, device_id: &str) -> bool {
        self.devices.remove(device_id).is_some()
    }

    /// Record a successful packet read with its latency.
    pub fn record_packet(&mut self, device_id: &str, latency: Duration) {
        if let Some(m) = self.devices.get_mut(device_id) {
            m.packet_count += 1;
            m.last_data = Some(Instant::now());
            let us = latency.as_micros().min(u64::MAX as u128) as u64;
            m.latency_sum_us = m.latency_sum_us.saturating_add(us);
            m.latency_samples += 1;
            if us > m.latency_max_us {
                m.latency_max_us = us;
            }
        }
    }

    /// Record an error for the given device.
    pub fn record_error(&mut self, device_id: &str) {
        if let Some(m) = self.devices.get_mut(device_id) {
            m.error_count += 1;
        }
    }

    /// Compute the current health of a device.
    pub fn check_health(&self, device_id: &str) -> Option<DeviceHealth> {
        let m = self.devices.get(device_id)?;

        // Never received data
        let Some(last) = m.last_data else {
            return Some(DeviceHealth::Unknown);
        };

        // Unresponsive check
        if last.elapsed() > self.unresponsive_timeout {
            return Some(DeviceHealth::Unresponsive);
        }

        // Degraded: high error rate
        let rate = m.error_rate();
        if rate >= ERROR_RATE_THRESHOLD {
            return Some(DeviceHealth::Degraded(format!(
                "error rate {:.1}%",
                rate * 100.0
            )));
        }

        // Degraded: high latency
        let avg = m.avg_latency_us();
        if avg > HIGH_LATENCY_US {
            return Some(DeviceHealth::Degraded(format!(
                "avg latency {avg}µs"
            )));
        }

        Some(DeviceHealth::Healthy)
    }

    /// Generate a full health report for one device.
    pub fn generate_report(&self, device_id: &str) -> Option<DeviceHealthReport> {
        let m = self.devices.get(device_id)?;
        let health = self.check_health(device_id).unwrap_or(DeviceHealth::Unknown);

        Some(DeviceHealthReport {
            device_id: device_id.to_owned(),
            health,
            last_data_received: m.last_data,
            packet_count: m.packet_count,
            error_count: m.error_count,
            error_rate: m.error_rate(),
            avg_latency_us: m.avg_latency_us(),
            max_latency_us: m.latency_max_us,
            uptime: m.first_seen.elapsed(),
        })
    }

    /// Generate reports for all monitored devices.
    pub fn all_reports(&self) -> Vec<DeviceHealthReport> {
        self.devices
            .keys()
            .filter_map(|id| self.generate_report(id))
            .collect()
    }

    /// Return IDs of devices that are not `Healthy` or `Unknown`.
    pub fn unhealthy_devices(&self) -> Vec<String> {
        self.devices
            .keys()
            .filter(|id| {
                matches!(
                    self.check_health(id),
                    Some(DeviceHealth::Degraded(_) | DeviceHealth::Unresponsive)
                )
            })
            .cloned()
            .collect()
    }

    /// Number of monitored devices.
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor() -> HealthMonitor {
        HealthMonitor::new(Duration::from_secs(1), Duration::from_millis(500))
    }

    #[test]
    fn register_device_starts_as_unknown() {
        let mut m = monitor();
        m.register_device("dev1");
        assert_eq!(m.check_health("dev1"), Some(DeviceHealth::Unknown));
    }

    #[test]
    fn first_packet_transitions_to_healthy() {
        let mut m = monitor();
        m.register_device("dev1");
        m.record_packet("dev1", Duration::from_micros(100));
        assert_eq!(m.check_health("dev1"), Some(DeviceHealth::Healthy));
    }

    #[test]
    fn high_error_rate_transitions_to_degraded() {
        let mut m = monitor();
        m.register_device("dev1");
        // 1 packet + 1 error = 50% error rate
        m.record_packet("dev1", Duration::from_micros(100));
        m.record_error("dev1");
        match m.check_health("dev1") {
            Some(DeviceHealth::Degraded(reason)) => {
                assert!(reason.contains("error rate"), "reason: {reason}");
            }
            other => panic!("expected Degraded, got {other:?}"),
        }
    }

    #[test]
    fn no_data_for_too_long_transitions_to_unresponsive() {
        let mut m = HealthMonitor::new(Duration::from_secs(1), Duration::from_millis(10));
        m.register_device("dev1");
        m.record_packet("dev1", Duration::from_micros(100));

        // Wait past the unresponsive timeout
        std::thread::sleep(Duration::from_millis(20));

        assert_eq!(m.check_health("dev1"), Some(DeviceHealth::Unresponsive));
    }

    #[test]
    fn unregister_device_removes_it() {
        let mut m = monitor();
        m.register_device("dev1");
        assert!(m.unregister_device("dev1"));
        assert_eq!(m.check_health("dev1"), None);
        assert!(!m.unregister_device("dev1"));
    }

    #[test]
    fn report_includes_correct_packet_count() {
        let mut m = monitor();
        m.register_device("dev1");
        for _ in 0..42 {
            m.record_packet("dev1", Duration::from_micros(100));
        }
        let r = m.generate_report("dev1").unwrap();
        assert_eq!(r.packet_count, 42);
    }

    #[test]
    fn report_includes_correct_error_rate() {
        let mut m = monitor();
        m.register_device("dev1");
        // 9 packets + 1 error = 10% error rate
        for _ in 0..9 {
            m.record_packet("dev1", Duration::from_micros(100));
        }
        m.record_error("dev1");
        let r = m.generate_report("dev1").unwrap();
        assert!((r.error_rate - 0.1).abs() < 1e-9, "rate: {}", r.error_rate);
    }

    #[test]
    fn average_latency_calculated_correctly() {
        let mut m = monitor();
        m.register_device("dev1");
        m.record_packet("dev1", Duration::from_micros(100));
        m.record_packet("dev1", Duration::from_micros(300));
        let r = m.generate_report("dev1").unwrap();
        assert_eq!(r.avg_latency_us, 200);
    }

    #[test]
    fn max_latency_tracked_correctly() {
        let mut m = monitor();
        m.register_device("dev1");
        m.record_packet("dev1", Duration::from_micros(100));
        m.record_packet("dev1", Duration::from_micros(500));
        m.record_packet("dev1", Duration::from_micros(200));
        let r = m.generate_report("dev1").unwrap();
        assert_eq!(r.max_latency_us, 500);
    }

    #[test]
    fn unhealthy_devices_filter_works() {
        let mut m = HealthMonitor::new(Duration::from_secs(1), Duration::from_millis(10));
        m.register_device("good");
        m.register_device("bad");
        m.record_packet("good", Duration::from_micros(100));
        m.record_packet("bad", Duration::from_micros(100));

        std::thread::sleep(Duration::from_millis(20));

        // Refresh "good" so it stays healthy
        m.record_packet("good", Duration::from_micros(100));

        let unhealthy = m.unhealthy_devices();
        assert!(unhealthy.contains(&"bad".to_owned()));
        assert!(!unhealthy.contains(&"good".to_owned()));
    }

    #[test]
    fn multiple_devices_tracked_independently() {
        let mut m = monitor();
        m.register_device("a");
        m.register_device("b");

        m.record_packet("a", Duration::from_micros(100));
        m.record_packet("a", Duration::from_micros(100));
        m.record_packet("b", Duration::from_micros(300));

        let ra = m.generate_report("a").unwrap();
        let rb = m.generate_report("b").unwrap();

        assert_eq!(ra.packet_count, 2);
        assert_eq!(rb.packet_count, 1);
        assert_eq!(ra.avg_latency_us, 100);
        assert_eq!(rb.avg_latency_us, 300);
        assert_eq!(m.device_count(), 2);
    }
}
