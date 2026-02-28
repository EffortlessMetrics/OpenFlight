// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Diagnostic bundle collection for support and debugging.
//!
//! Collects system information, device state, adapter state, profile data,
//! performance metrics, and recent events into a structured bundle that
//! can be exported as JSON or a human-readable summary.

use serde::{Deserialize, Serialize};
use std::path::Path;

// ── System information ───────────────────────────────────────────────────

/// System-level information for diagnostic context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub os_name: String,
    pub os_version: String,
    pub cpu_name: String,
    pub cpu_cores: usize,
    pub memory_total_mb: u64,
    pub rust_version: String,
    pub openflight_version: String,
    pub build_timestamp: String,
}

impl SystemInfo {
    /// Collect basic system information from the current environment.
    pub fn collect() -> Self {
        Self {
            os_name: std::env::consts::OS.to_string(),
            os_version: String::new(),
            cpu_name: String::new(),
            cpu_cores: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1),
            memory_total_mb: 0,
            rust_version: "1.92+".to_string(),
            openflight_version: env!("CARGO_PKG_VERSION").to_string(),
            build_timestamp: String::new(),
        }
    }
}

// ── Device report ────────────────────────────────────────────────────────

/// Report on an attached device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceReport {
    pub name: String,
    pub vid_pid: String,
    pub connected: bool,
    pub state: String,
    pub errors: Vec<String>,
}

// ── Adapter report ───────────────────────────────────────────────────────

/// Report on a simulator adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterReport {
    pub name: String,
    pub sim_type: String,
    pub connected: bool,
    pub state: String,
    pub last_error: Option<String>,
}

// ── Profile report ───────────────────────────────────────────────────────

/// Report on the active profile chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileReport {
    pub active_profile: Option<String>,
    pub loaded_profiles: Vec<String>,
    pub recent_changes: Vec<String>,
}

// ── Performance report ───────────────────────────────────────────────────

/// A single tick sample for performance tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickSample {
    pub timestamp_us: u64,
    pub duration_us: u64,
    pub budget_pct: f64,
}

/// Performance metrics for the RT spine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReport {
    pub tick_rate_hz: f64,
    pub jitter_p99_us: f64,
    pub budget_usage_pct: f64,
    pub overrun_count: u64,
    pub last_30s: Vec<TickSample>,
}

impl PerformanceReport {
    /// Compute the mean tick duration from samples.
    pub fn mean_tick_duration_us(&self) -> f64 {
        if self.last_30s.is_empty() {
            return 0.0;
        }
        let total: u64 = self.last_30s.iter().map(|s| s.duration_us).sum();
        total as f64 / self.last_30s.len() as f64
    }

    /// Compute jitter p99 from samples (sorted percentile).
    pub fn compute_jitter_p99(samples: &[TickSample], target_us: u64) -> f64 {
        if samples.is_empty() {
            return 0.0;
        }
        let mut deltas: Vec<f64> = samples
            .iter()
            .map(|s| (s.duration_us as f64 - target_us as f64).abs())
            .collect();
        deltas.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let idx = ((deltas.len() as f64) * 0.99).ceil() as usize;
        let idx = idx.min(deltas.len()) - 1;
        deltas[idx]
    }

    /// Count samples that exceeded the budget.
    pub fn count_overruns(samples: &[TickSample], budget_us: u64) -> u64 {
        samples.iter().filter(|s| s.duration_us > budget_us).count() as u64
    }

    /// Compute mean budget usage percentage.
    pub fn mean_budget_usage(samples: &[TickSample]) -> f64 {
        if samples.is_empty() {
            return 0.0;
        }
        let total: f64 = samples.iter().map(|s| s.budget_pct).sum();
        total / samples.len() as f64
    }
}

// ── Flight event (local representation) ──────────────────────────────────

/// A recent event captured for the diagnostic bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlightEvent {
    pub timestamp_ns: u64,
    pub level: String,
    pub component: String,
    pub message: String,
}

// ── Diagnostic bundle ────────────────────────────────────────────────────

/// Complete diagnostic bundle for support and debugging.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticBundle {
    pub system_info: SystemInfo,
    pub device_info: Vec<DeviceReport>,
    pub adapter_info: Vec<AdapterReport>,
    pub profile_info: ProfileReport,
    pub recent_events: Vec<FlightEvent>,
    pub performance: PerformanceReport,
    pub log_excerpt: String,
}

impl DiagnosticBundle {
    /// Serialize the bundle to a pretty-printed JSON string.
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }
}

// ── Bundle collector ─────────────────────────────────────────────────────

/// Builder that assembles a [`DiagnosticBundle`] from subsystem data.
pub struct BundleCollector {
    system_info: Option<SystemInfo>,
    devices: Vec<DeviceReport>,
    adapters: Vec<AdapterReport>,
    profile: Option<ProfileReport>,
    events: Vec<FlightEvent>,
    performance: Option<PerformanceReport>,
    log_excerpt: String,
}

impl BundleCollector {
    pub fn new() -> Self {
        Self {
            system_info: None,
            devices: Vec::new(),
            adapters: Vec::new(),
            profile: None,
            events: Vec::new(),
            performance: None,
            log_excerpt: String::new(),
        }
    }

    pub fn set_system_info(&mut self, info: SystemInfo) -> &mut Self {
        self.system_info = Some(info);
        self
    }

    pub fn add_device(&mut self, report: DeviceReport) -> &mut Self {
        self.devices.push(report);
        self
    }

    pub fn add_adapter(&mut self, report: AdapterReport) -> &mut Self {
        self.adapters.push(report);
        self
    }

    pub fn set_profile(&mut self, report: ProfileReport) -> &mut Self {
        self.profile = Some(report);
        self
    }

    pub fn add_event(&mut self, event: FlightEvent) -> &mut Self {
        self.events.push(event);
        self
    }

    pub fn set_events(&mut self, events: Vec<FlightEvent>) -> &mut Self {
        self.events = events;
        self
    }

    pub fn set_performance(&mut self, report: PerformanceReport) -> &mut Self {
        self.performance = Some(report);
        self
    }

    pub fn set_log_excerpt(&mut self, excerpt: String) -> &mut Self {
        self.log_excerpt = excerpt;
        self
    }

    /// Assemble and return the diagnostic bundle.
    pub fn collect(&self) -> DiagnosticBundle {
        DiagnosticBundle {
            system_info: self.system_info.clone().unwrap_or_else(SystemInfo::collect),
            device_info: self.devices.clone(),
            adapter_info: self.adapters.clone(),
            profile_info: self.profile.clone().unwrap_or_else(|| ProfileReport {
                active_profile: None,
                loaded_profiles: Vec::new(),
                recent_changes: Vec::new(),
            }),
            recent_events: self.events.clone(),
            performance: self
                .performance
                .clone()
                .unwrap_or_else(|| PerformanceReport {
                    tick_rate_hz: 0.0,
                    jitter_p99_us: 0.0,
                    budget_usage_pct: 0.0,
                    overrun_count: 0,
                    last_30s: Vec::new(),
                }),
            log_excerpt: self.log_excerpt.clone(),
        }
    }

    /// Export the bundle as JSON to a file.
    pub fn export_json(&self, path: &Path) -> std::io::Result<()> {
        let bundle = self.collect();
        let json = bundle.to_json().map_err(std::io::Error::other)?;
        std::fs::write(path, json)
    }

    /// Export the bundle as a compressed archive.
    ///
    /// Writes a JSON bundle to the given path. Full zip compression
    /// can be layered on top by callers with access to a zip library.
    pub fn export_zip(&self, path: &Path) -> std::io::Result<()> {
        // Write JSON content — callers can wrap in actual zip if needed.
        self.export_json(path)
    }

    /// Produce a human-readable summary suitable for CLI output.
    pub fn summary(&self) -> String {
        let bundle = self.collect();
        let mut out = String::new();

        out.push_str("=== OpenFlight Diagnostic Summary ===\n\n");

        // System
        out.push_str(&format!(
            "System: {} {} ({})\n",
            bundle.system_info.os_name, bundle.system_info.os_version, bundle.system_info.cpu_name,
        ));
        out.push_str(&format!(
            "  CPU cores: {}  Memory: {} MB\n",
            bundle.system_info.cpu_cores, bundle.system_info.memory_total_mb,
        ));
        out.push_str(&format!(
            "  OpenFlight: {}  Rust: {}\n\n",
            bundle.system_info.openflight_version, bundle.system_info.rust_version,
        ));

        // Devices
        out.push_str(&format!("Devices: {}\n", bundle.device_info.len()));
        for d in &bundle.device_info {
            let status = if d.connected {
                "connected"
            } else {
                "disconnected"
            };
            out.push_str(&format!("  - {} [{}] {}\n", d.name, d.vid_pid, status));
            for e in &d.errors {
                out.push_str(&format!("    error: {e}\n"));
            }
        }
        out.push('\n');

        // Adapters
        out.push_str(&format!("Adapters: {}\n", bundle.adapter_info.len()));
        for a in &bundle.adapter_info {
            let status = if a.connected {
                "connected"
            } else {
                "disconnected"
            };
            out.push_str(&format!("  - {} ({}) {}\n", a.name, a.sim_type, status));
            if let Some(ref e) = a.last_error {
                out.push_str(&format!("    last error: {e}\n"));
            }
        }
        out.push('\n');

        // Profile
        if let Some(ref name) = bundle.profile_info.active_profile {
            out.push_str(&format!("Active profile: {name}\n"));
        } else {
            out.push_str("Active profile: none\n");
        }
        out.push('\n');

        // Performance
        out.push_str(&format!(
            "Performance: {:.0} Hz, jitter p99 = {:.0} µs, budget = {:.1}%, overruns = {}\n",
            bundle.performance.tick_rate_hz,
            bundle.performance.jitter_p99_us,
            bundle.performance.budget_usage_pct,
            bundle.performance.overrun_count,
        ));
        out.push('\n');

        // Events
        out.push_str(&format!("Recent events: {}\n", bundle.recent_events.len()));
        for e in bundle.recent_events.iter().rev().take(5) {
            out.push_str(&format!("  [{}] {}: {}\n", e.level, e.component, e.message));
        }

        // Log excerpt
        let log_lines = bundle.log_excerpt.lines().count();
        if log_lines > 0 {
            out.push_str(&format!("\nLog excerpt: {log_lines} lines\n"));
        }

        out
    }
}

impl Default for BundleCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn sample_system_info() -> SystemInfo {
        SystemInfo {
            os_name: "windows".into(),
            os_version: "11".into(),
            cpu_name: "AMD Ryzen 9 7950X".into(),
            cpu_cores: 16,
            memory_total_mb: 32768,
            rust_version: "1.92.0".into(),
            openflight_version: "0.1.0".into(),
            build_timestamp: "2025-01-01T00:00:00Z".into(),
        }
    }

    fn sample_device() -> DeviceReport {
        DeviceReport {
            name: "T.Flight HOTAS 4".into(),
            vid_pid: "044f:b10a".into(),
            connected: true,
            state: "active".into(),
            errors: Vec::new(),
        }
    }

    fn sample_device_error() -> DeviceReport {
        DeviceReport {
            name: "Throttle".into(),
            vid_pid: "044f:b687".into(),
            connected: false,
            state: "disconnected".into(),
            errors: vec!["USB reset".into(), "timeout".into()],
        }
    }

    fn sample_adapter() -> AdapterReport {
        AdapterReport {
            name: "MSFS 2024".into(),
            sim_type: "SimConnect".into(),
            connected: true,
            state: "streaming".into(),
            last_error: None,
        }
    }

    fn sample_adapter_error() -> AdapterReport {
        AdapterReport {
            name: "X-Plane 12".into(),
            sim_type: "UDP".into(),
            connected: false,
            state: "disconnected".into(),
            last_error: Some("connection refused".into()),
        }
    }

    fn sample_profile() -> ProfileReport {
        ProfileReport {
            active_profile: Some("combat".into()),
            loaded_profiles: vec!["global".into(), "dcs".into(), "combat".into()],
            recent_changes: vec!["switched to combat at 12:00".into()],
        }
    }

    fn sample_event(level: &str, component: &str, message: &str) -> FlightEvent {
        FlightEvent {
            timestamp_ns: 1_700_000_000_000_000_000,
            level: level.into(),
            component: component.into(),
            message: message.into(),
        }
    }

    fn sample_performance() -> PerformanceReport {
        PerformanceReport {
            tick_rate_hz: 250.0,
            jitter_p99_us: 120.0,
            budget_usage_pct: 45.0,
            overrun_count: 3,
            last_30s: vec![
                TickSample {
                    timestamp_us: 0,
                    duration_us: 3800,
                    budget_pct: 95.0,
                },
                TickSample {
                    timestamp_us: 4000,
                    duration_us: 3600,
                    budget_pct: 90.0,
                },
                TickSample {
                    timestamp_us: 8000,
                    duration_us: 4200,
                    budget_pct: 105.0,
                },
            ],
        }
    }

    fn populated_collector() -> BundleCollector {
        let mut c = BundleCollector::new();
        c.set_system_info(sample_system_info())
            .add_device(sample_device())
            .add_device(sample_device_error())
            .add_adapter(sample_adapter())
            .add_adapter(sample_adapter_error())
            .set_profile(sample_profile())
            .add_event(sample_event("INFO", "axis", "tick ok"))
            .add_event(sample_event("WARN", "hid", "USB reset"))
            .add_event(sample_event("ERROR", "ffb", "motor stall"))
            .set_performance(sample_performance())
            .set_log_excerpt("line 1\nline 2\nline 3\n".into());
        c
    }

    // ── Bundle collection tests ──────────────────────────────────────────

    #[test]
    fn test_collect_with_mock_data() {
        let bundle = populated_collector().collect();
        assert_eq!(bundle.system_info.os_name, "windows");
        assert_eq!(bundle.device_info.len(), 2);
        assert_eq!(bundle.adapter_info.len(), 2);
        assert_eq!(bundle.recent_events.len(), 3);
        assert_eq!(bundle.performance.tick_rate_hz, 250.0);
        assert_eq!(bundle.log_excerpt.lines().count(), 3);
    }

    #[test]
    fn test_collect_empty_defaults() {
        let bundle = BundleCollector::new().collect();
        assert!(!bundle.system_info.os_name.is_empty());
        assert!(bundle.device_info.is_empty());
        assert!(bundle.adapter_info.is_empty());
        assert!(bundle.profile_info.active_profile.is_none());
        assert!(bundle.recent_events.is_empty());
        assert_eq!(bundle.performance.tick_rate_hz, 0.0);
        assert!(bundle.log_excerpt.is_empty());
    }

    #[test]
    fn test_system_info_collect() {
        let info = SystemInfo::collect();
        assert!(!info.os_name.is_empty());
        assert!(info.cpu_cores >= 1);
        assert!(!info.openflight_version.is_empty());
    }

    // ── JSON export tests ────────────────────────────────────────────────

    #[test]
    fn test_json_export_roundtrip() {
        let bundle = populated_collector().collect();
        let json = bundle.to_json().expect("serialization should succeed");
        let restored: DiagnosticBundle =
            serde_json::from_str(&json).expect("deserialization should succeed");
        assert_eq!(restored.system_info.os_name, "windows");
        assert_eq!(restored.device_info.len(), 2);
        assert_eq!(restored.adapter_info.len(), 2);
        assert_eq!(restored.recent_events.len(), 3);
        assert_eq!(restored.performance.tick_rate_hz, 250.0);
    }

    #[test]
    fn test_json_export_valid_structure() {
        let bundle = populated_collector().collect();
        let json = bundle.to_json().unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(v.get("system_info").is_some());
        assert!(v.get("device_info").is_some());
        assert!(v.get("adapter_info").is_some());
        assert!(v.get("profile_info").is_some());
        assert!(v.get("recent_events").is_some());
        assert!(v.get("performance").is_some());
        assert!(v.get("log_excerpt").is_some());
    }

    #[test]
    fn test_export_json_to_file() {
        let path = env::temp_dir().join("openflight_diag_test_export.json");
        let _ = std::fs::remove_file(&path);
        let collector = populated_collector();
        collector.export_json(&path).expect("export should succeed");
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(v.get("system_info").is_some());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_export_zip_to_file() {
        let path = env::temp_dir().join("openflight_diag_test_export.zip");
        let _ = std::fs::remove_file(&path);
        let collector = populated_collector();
        collector.export_zip(&path).expect("export should succeed");
        assert!(path.exists());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_empty_bundle_json_roundtrip() {
        let bundle = BundleCollector::new().collect();
        let json = bundle.to_json().unwrap();
        let restored: DiagnosticBundle = serde_json::from_str(&json).unwrap();
        assert!(restored.device_info.is_empty());
        assert!(restored.adapter_info.is_empty());
    }

    // ── Summary formatting tests ─────────────────────────────────────────

    #[test]
    fn test_summary_contains_sections() {
        let summary = populated_collector().summary();
        assert!(summary.contains("OpenFlight Diagnostic Summary"));
        assert!(summary.contains("System:"));
        assert!(summary.contains("Devices:"));
        assert!(summary.contains("Adapters:"));
        assert!(summary.contains("Active profile:"));
        assert!(summary.contains("Performance:"));
        assert!(summary.contains("Recent events:"));
    }

    #[test]
    fn test_summary_device_details() {
        let summary = populated_collector().summary();
        assert!(summary.contains("T.Flight HOTAS 4"));
        assert!(summary.contains("044f:b10a"));
        assert!(summary.contains("connected"));
        assert!(summary.contains("Throttle"));
        assert!(summary.contains("disconnected"));
        assert!(summary.contains("USB reset"));
    }

    #[test]
    fn test_summary_adapter_details() {
        let summary = populated_collector().summary();
        assert!(summary.contains("MSFS 2024"));
        assert!(summary.contains("X-Plane 12"));
        assert!(summary.contains("connection refused"));
    }

    #[test]
    fn test_summary_performance_metrics() {
        let summary = populated_collector().summary();
        assert!(summary.contains("250 Hz"));
        assert!(summary.contains("120 µs"));
        assert!(summary.contains("45.0%"));
        assert!(summary.contains("overruns = 3"));
    }

    #[test]
    fn test_summary_empty_bundle() {
        let summary = BundleCollector::new().summary();
        assert!(summary.contains("Devices: 0"));
        assert!(summary.contains("Adapters: 0"));
        assert!(summary.contains("Active profile: none"));
        assert!(summary.contains("Recent events: 0"));
    }

    #[test]
    fn test_summary_log_excerpt_line_count() {
        let mut c = BundleCollector::new();
        c.set_log_excerpt("a\nb\nc\nd\ne\n".into());
        let summary = c.summary();
        assert!(summary.contains("Log excerpt: 5 lines"));
    }

    // ── Performance report tests ─────────────────────────────────────────

    #[test]
    fn test_mean_tick_duration() {
        let report = sample_performance();
        let mean = report.mean_tick_duration_us();
        // (3800 + 3600 + 4200) / 3 ≈ 3866.67
        assert!((mean - 3866.67).abs() < 1.0);
    }

    #[test]
    fn test_mean_tick_duration_empty() {
        let report = PerformanceReport {
            tick_rate_hz: 0.0,
            jitter_p99_us: 0.0,
            budget_usage_pct: 0.0,
            overrun_count: 0,
            last_30s: Vec::new(),
        };
        assert_eq!(report.mean_tick_duration_us(), 0.0);
    }

    #[test]
    fn test_compute_jitter_p99() {
        let samples: Vec<TickSample> = (0..100)
            .map(|i| TickSample {
                timestamp_us: i * 4000,
                duration_us: 4000 + (i % 10) * 10,
                budget_pct: 100.0,
            })
            .collect();
        let jitter = PerformanceReport::compute_jitter_p99(&samples, 4000);
        // The deltas are 0,10,20,...,90 repeating. p99 should be near 90.
        assert!(jitter >= 80.0);
    }

    #[test]
    fn test_compute_jitter_p99_empty() {
        assert_eq!(PerformanceReport::compute_jitter_p99(&[], 4000), 0.0);
    }

    #[test]
    fn test_count_overruns() {
        let samples = vec![
            TickSample {
                timestamp_us: 0,
                duration_us: 3900,
                budget_pct: 97.5,
            },
            TickSample {
                timestamp_us: 4000,
                duration_us: 4100,
                budget_pct: 102.5,
            },
            TickSample {
                timestamp_us: 8000,
                duration_us: 5000,
                budget_pct: 125.0,
            },
        ];
        assert_eq!(PerformanceReport::count_overruns(&samples, 4000), 2);
    }

    #[test]
    fn test_count_overruns_none() {
        let samples = vec![TickSample {
            timestamp_us: 0,
            duration_us: 3500,
            budget_pct: 87.5,
        }];
        assert_eq!(PerformanceReport::count_overruns(&samples, 4000), 0);
    }

    #[test]
    fn test_mean_budget_usage() {
        let samples = vec![
            TickSample {
                timestamp_us: 0,
                duration_us: 0,
                budget_pct: 80.0,
            },
            TickSample {
                timestamp_us: 0,
                duration_us: 0,
                budget_pct: 100.0,
            },
            TickSample {
                timestamp_us: 0,
                duration_us: 0,
                budget_pct: 120.0,
            },
        ];
        assert!((PerformanceReport::mean_budget_usage(&samples) - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_mean_budget_usage_empty() {
        assert_eq!(PerformanceReport::mean_budget_usage(&[]), 0.0);
    }

    // ── Builder pattern tests ────────────────────────────────────────────

    #[test]
    fn test_collector_chaining() {
        let mut c = BundleCollector::new();
        c.set_system_info(sample_system_info())
            .add_device(sample_device())
            .add_adapter(sample_adapter())
            .set_profile(sample_profile())
            .set_performance(sample_performance())
            .set_log_excerpt("log".into());
        let bundle = c.collect();
        assert_eq!(bundle.device_info.len(), 1);
        assert_eq!(bundle.adapter_info.len(), 1);
    }

    #[test]
    fn test_set_events_replaces_all() {
        let mut c = BundleCollector::new();
        c.add_event(sample_event("INFO", "a", "first"));
        c.set_events(vec![
            sample_event("WARN", "b", "second"),
            sample_event("ERROR", "c", "third"),
        ]);
        let bundle = c.collect();
        assert_eq!(bundle.recent_events.len(), 2);
        assert_eq!(bundle.recent_events[0].component, "b");
    }

    // ── Serialization tests ──────────────────────────────────────────────

    #[test]
    fn test_device_report_serialization() {
        let d = sample_device();
        let json = serde_json::to_string(&d).unwrap();
        let restored: DeviceReport = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.name, d.name);
        assert_eq!(restored.vid_pid, d.vid_pid);
    }

    #[test]
    fn test_adapter_report_serialization() {
        let a = sample_adapter();
        let json = serde_json::to_string(&a).unwrap();
        let restored: AdapterReport = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.name, a.name);
        assert_eq!(restored.sim_type, a.sim_type);
    }

    #[test]
    fn test_tick_sample_serialization() {
        let s = TickSample {
            timestamp_us: 42,
            duration_us: 3800,
            budget_pct: 95.0,
        };
        let json = serde_json::to_string(&s).unwrap();
        let restored: TickSample = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.timestamp_us, 42);
        assert_eq!(restored.duration_us, 3800);
    }

    #[test]
    fn test_profile_report_with_changes() {
        let p = sample_profile();
        assert_eq!(p.active_profile, Some("combat".into()));
        assert_eq!(p.loaded_profiles.len(), 3);
        assert_eq!(p.recent_changes.len(), 1);
    }
}
