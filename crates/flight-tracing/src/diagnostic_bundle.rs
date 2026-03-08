// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Diagnostic snapshot collection for debugging and support.
//!
//! [`DiagnosticBundle`] captures a point-in-time snapshot of system state
//! including device information, active profiles, adapter status, health
//! metrics, recent errors, axis values, and timing statistics.
//!
//! [`DiagnosticCollector`] aggregates data sources and produces a bundle
//! on demand via [`DiagnosticCollector::collect`].

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

// ── Data model ────────────────────────────────────────────────────────────

/// Device information snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub connected: bool,
}

/// Active profile information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileInfo {
    pub name: String,
    pub layer: String,
    pub active: bool,
}

/// Simulator adapter status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterStatus {
    pub name: String,
    pub connected: bool,
    pub version: Option<String>,
    pub last_error: Option<String>,
}

/// System health indicator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthInfo {
    pub component: String,
    pub healthy: bool,
    pub message: String,
}

/// Recent error entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorEntry {
    pub timestamp_ns: u64,
    pub code: u32,
    pub message: String,
    pub component: String,
}

/// Axis value snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxisValue {
    pub name: String,
    pub raw: f64,
    pub processed: f64,
}

/// Timing statistics snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingStats {
    pub tick_count: u64,
    pub avg_tick_ns: u64,
    pub p99_tick_ns: u64,
    pub deadline_misses: u64,
    pub jitter_p99_ns: u64,
}

/// A point-in-time snapshot of the entire system state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticBundle {
    pub timestamp_ns: u64,
    pub version: String,
    pub devices: Vec<DeviceInfo>,
    pub profiles: Vec<ProfileInfo>,
    pub adapters: Vec<AdapterStatus>,
    pub health: Vec<HealthInfo>,
    pub recent_errors: Vec<ErrorEntry>,
    pub axes: Vec<AxisValue>,
    pub timing: TimingStats,
}

impl DiagnosticBundle {
    /// Serialize the bundle to a pretty-printed JSON string.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
    }

    /// Produce a human-readable one-page summary.
    pub fn summary(&self) -> String {
        let mut out = String::with_capacity(1024);

        out.push_str("=== OpenFlight Diagnostic Summary ===\n");
        out.push_str(&format!("Version: {}\n", self.version));
        out.push_str(&format!(
            "Timestamp: {}\n\n",
            format_timestamp(self.timestamp_ns)
        ));

        // Devices
        let connected = self.devices.iter().filter(|d| d.connected).count();
        out.push_str(&format!(
            "Devices: {} connected / {} total\n",
            connected,
            self.devices.len()
        ));
        for d in &self.devices {
            let status = if d.connected { "✓" } else { "✗" };
            out.push_str(&format!(
                "  [{status}] {} ({:04x}:{:04x})\n",
                d.name, d.vendor_id, d.product_id
            ));
        }

        // Profiles
        out.push('\n');
        let active = self.profiles.iter().filter(|p| p.active).count();
        out.push_str(&format!(
            "Profiles: {} active / {} total\n",
            active,
            self.profiles.len()
        ));
        for p in &self.profiles {
            let status = if p.active { "active" } else { "inactive" };
            out.push_str(&format!("  {} [{}] ({})\n", p.name, p.layer, status));
        }

        // Adapters
        out.push('\n');
        out.push_str("Adapters:\n");
        for a in &self.adapters {
            let status = if a.connected {
                "connected"
            } else {
                "disconnected"
            };
            out.push_str(&format!("  {} — {status}", a.name));
            if let Some(ref v) = a.version {
                out.push_str(&format!(" (v{v})"));
            }
            if let Some(ref e) = a.last_error {
                out.push_str(&format!(" [error: {e}]"));
            }
            out.push('\n');
        }

        // Health
        out.push('\n');
        let unhealthy: Vec<_> = self.health.iter().filter(|h| !h.healthy).collect();
        if unhealthy.is_empty() {
            out.push_str("Health: ALL OK\n");
        } else {
            out.push_str(&format!("Health: {} issue(s)\n", unhealthy.len()));
            for h in &unhealthy {
                out.push_str(&format!("  ⚠ {} — {}\n", h.component, h.message));
            }
        }

        // Timing
        out.push('\n');
        out.push_str("Timing:\n");
        out.push_str(&format!("  Ticks: {}\n", self.timing.tick_count));
        out.push_str(&format!(
            "  Avg tick: {:.1} µs\n",
            self.timing.avg_tick_ns as f64 / 1_000.0
        ));
        out.push_str(&format!(
            "  P99 tick: {:.1} µs\n",
            self.timing.p99_tick_ns as f64 / 1_000.0
        ));
        out.push_str(&format!(
            "  Deadline misses: {}\n",
            self.timing.deadline_misses
        ));
        out.push_str(&format!(
            "  Jitter P99: {:.1} µs\n",
            self.timing.jitter_p99_ns as f64 / 1_000.0
        ));

        // Recent errors
        if !self.recent_errors.is_empty() {
            out.push('\n');
            out.push_str(&format!("Recent errors ({}):\n", self.recent_errors.len()));
            for e in self.recent_errors.iter().rev().take(5) {
                out.push_str(&format!(
                    "  [0x{:04x}] {} — {}\n",
                    e.code, e.component, e.message
                ));
            }
        }

        out
    }
}

fn format_timestamp(ns: u64) -> String {
    let secs = ns / 1_000_000_000;
    let subsec = ns % 1_000_000_000;
    format!("{secs}.{subsec:09}")
}

// ── DiagnosticCollector ───────────────────────────────────────────────────

/// Collects system state snapshots from registered data sources.
///
/// Data is supplied through the builder-style setters before calling
/// [`DiagnosticCollector::collect`].
pub struct DiagnosticCollector {
    version: String,
    devices: Vec<DeviceInfo>,
    profiles: Vec<ProfileInfo>,
    adapters: Vec<AdapterStatus>,
    health: Vec<HealthInfo>,
    recent_errors: Vec<ErrorEntry>,
    axes: Vec<AxisValue>,
    timing: TimingStats,
}

impl DiagnosticCollector {
    /// Create a new collector tagged with the given application version.
    pub fn new(version: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            devices: Vec::new(),
            profiles: Vec::new(),
            adapters: Vec::new(),
            health: Vec::new(),
            recent_errors: Vec::new(),
            axes: Vec::new(),
            timing: TimingStats {
                tick_count: 0,
                avg_tick_ns: 0,
                p99_tick_ns: 0,
                deadline_misses: 0,
                jitter_p99_ns: 0,
            },
        }
    }

    pub fn set_devices(&mut self, devices: Vec<DeviceInfo>) {
        self.devices = devices;
    }

    pub fn set_profiles(&mut self, profiles: Vec<ProfileInfo>) {
        self.profiles = profiles;
    }

    pub fn set_adapters(&mut self, adapters: Vec<AdapterStatus>) {
        self.adapters = adapters;
    }

    pub fn set_health(&mut self, health: Vec<HealthInfo>) {
        self.health = health;
    }

    pub fn set_recent_errors(&mut self, errors: Vec<ErrorEntry>) {
        self.recent_errors = errors;
    }

    pub fn set_axes(&mut self, axes: Vec<AxisValue>) {
        self.axes = axes;
    }

    pub fn set_timing(&mut self, timing: TimingStats) {
        self.timing = timing;
    }

    /// Collect a snapshot of all registered data into a [`DiagnosticBundle`].
    pub fn collect(&self) -> DiagnosticBundle {
        DiagnosticBundle {
            timestamp_ns: now_ns(),
            version: self.version.clone(),
            devices: self.devices.clone(),
            profiles: self.profiles.clone(),
            adapters: self.adapters.clone(),
            health: self.health.clone(),
            recent_errors: self.recent_errors.clone(),
            axes: self.axes.clone(),
            timing: self.timing.clone(),
        }
    }
}

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_collector() -> DiagnosticCollector {
        let mut c = DiagnosticCollector::new("0.1.0-test");
        c.set_devices(vec![
            DeviceInfo {
                id: "dev-1".into(),
                name: "VKB Gladiator".into(),
                vendor_id: 0x231d,
                product_id: 0x0200,
                connected: true,
            },
            DeviceInfo {
                id: "dev-2".into(),
                name: "Saitek Throttle".into(),
                vendor_id: 0x06a3,
                product_id: 0x0764,
                connected: false,
            },
        ]);
        c.set_profiles(vec![
            ProfileInfo {
                name: "F/A-18C".into(),
                layer: "aircraft".into(),
                active: true,
            },
            ProfileInfo {
                name: "Global".into(),
                layer: "global".into(),
                active: true,
            },
        ]);
        c.set_adapters(vec![
            AdapterStatus {
                name: "MSFS".into(),
                connected: true,
                version: Some("2024.1".into()),
                last_error: None,
            },
            AdapterStatus {
                name: "X-Plane".into(),
                connected: false,
                version: None,
                last_error: Some("Connection refused".into()),
            },
        ]);
        c.set_health(vec![
            HealthInfo {
                component: "RT Spine".into(),
                healthy: true,
                message: "OK".into(),
            },
            HealthInfo {
                component: "FFB".into(),
                healthy: false,
                message: "Device timeout".into(),
            },
        ]);
        c.set_recent_errors(vec![ErrorEntry {
            timestamp_ns: 1_000_000_000,
            code: 0x0042,
            message: "FFB device timeout".into(),
            component: "ffb".into(),
        }]);
        c.set_axes(vec![AxisValue {
            name: "pitch".into(),
            raw: 0.512,
            processed: 0.498,
        }]);
        c.set_timing(TimingStats {
            tick_count: 250_000,
            avg_tick_ns: 3_800_000,
            p99_tick_ns: 4_200_000,
            deadline_misses: 3,
            jitter_p99_ns: 450_000,
        });
        c
    }

    #[test]
    fn collect_produces_bundle() {
        let c = sample_collector();
        let bundle = c.collect();

        assert_eq!(bundle.version, "0.1.0-test");
        assert!(bundle.timestamp_ns > 0);
        assert_eq!(bundle.devices.len(), 2);
        assert_eq!(bundle.profiles.len(), 2);
        assert_eq!(bundle.adapters.len(), 2);
        assert_eq!(bundle.health.len(), 2);
        assert_eq!(bundle.recent_errors.len(), 1);
        assert_eq!(bundle.axes.len(), 1);
        assert_eq!(bundle.timing.tick_count, 250_000);
    }

    #[test]
    fn json_round_trip() {
        let bundle = sample_collector().collect();
        let json = bundle.to_json();

        let recovered: DiagnosticBundle =
            serde_json::from_str(&json).expect("JSON should deserialize back");
        assert_eq!(recovered.version, bundle.version);
        assert_eq!(recovered.devices.len(), bundle.devices.len());
        assert_eq!(recovered.timing.tick_count, bundle.timing.tick_count);
        assert_eq!(
            recovered.recent_errors[0].code,
            bundle.recent_errors[0].code
        );
    }

    #[test]
    fn json_contains_expected_fields() {
        let bundle = sample_collector().collect();
        let json = bundle.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert!(parsed["version"].is_string());
        assert!(parsed["devices"].is_array());
        assert!(parsed["timing"]["tick_count"].is_number());
        assert_eq!(parsed["devices"][0]["name"], "VKB Gladiator");
    }

    #[test]
    fn summary_contains_sections() {
        let bundle = sample_collector().collect();
        let summary = bundle.summary();

        assert!(summary.contains("Diagnostic Summary"));
        assert!(summary.contains("0.1.0-test"));
        assert!(summary.contains("VKB Gladiator"));
        assert!(summary.contains("Saitek Throttle"));
        assert!(summary.contains("F/A-18C"));
        assert!(summary.contains("MSFS"));
        assert!(summary.contains("X-Plane"));
        assert!(summary.contains("Device timeout"));
        assert!(summary.contains("250000")); // tick count
        assert!(summary.contains("FFB device timeout"));
    }

    #[test]
    fn summary_healthy_system() {
        let mut c = DiagnosticCollector::new("1.0.0");
        c.set_health(vec![HealthInfo {
            component: "core".into(),
            healthy: true,
            message: "OK".into(),
        }]);
        let bundle = c.collect();
        let summary = bundle.summary();
        assert!(summary.contains("ALL OK"));
    }

    #[test]
    fn empty_collector_produces_valid_bundle() {
        let c = DiagnosticCollector::new("0.0.0");
        let bundle = c.collect();
        assert!(bundle.devices.is_empty());
        assert!(bundle.profiles.is_empty());
        assert!(bundle.adapters.is_empty());
        // JSON should still round-trip
        let json = bundle.to_json();
        let _: DiagnosticBundle = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn summary_format_no_errors() {
        let c = DiagnosticCollector::new("1.0.0");
        let bundle = c.collect();
        let summary = bundle.summary();
        // Should not contain "Recent errors" section when there are none
        assert!(!summary.contains("Recent errors"));
    }

    #[test]
    fn device_connected_count_in_summary() {
        let mut c = DiagnosticCollector::new("1.0.0");
        c.set_devices(vec![
            DeviceInfo {
                id: "a".into(),
                name: "A".into(),
                vendor_id: 0,
                product_id: 0,
                connected: true,
            },
            DeviceInfo {
                id: "b".into(),
                name: "B".into(),
                vendor_id: 0,
                product_id: 0,
                connected: true,
            },
            DeviceInfo {
                id: "c".into(),
                name: "C".into(),
                vendor_id: 0,
                product_id: 0,
                connected: false,
            },
        ]);
        let summary = c.collect().summary();
        assert!(summary.contains("2 connected / 3 total"));
    }

    #[test]
    fn timing_stats_serialization() {
        let ts = TimingStats {
            tick_count: 100,
            avg_tick_ns: 4_000_000,
            p99_tick_ns: 4_500_000,
            deadline_misses: 1,
            jitter_p99_ns: 200_000,
        };
        let json = serde_json::to_string(&ts).unwrap();
        let recovered: TimingStats = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered.tick_count, 100);
        assert_eq!(recovered.p99_tick_ns, 4_500_000);
    }
}
