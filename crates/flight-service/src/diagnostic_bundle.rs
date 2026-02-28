//! Diagnostic bundle generation for support and debugging.
//!
//! Generates a text archive containing logs, configuration, metrics snapshot,
//! and system information for troubleshooting.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Reason the service degraded into safe mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DegradationReason {
    /// A force-feedback subsystem fault was detected.
    FfbFault(String),
    /// One or more HID devices failed enumeration.
    HidEnumerationFailure(String),
    /// A simulator adapter lost its connection.
    AdapterDisconnect(String),
    /// A critical configuration error prevented normal startup.
    ConfigError(String),
    /// A plugin crashed or violated its resource budget.
    PluginFault(String),
    /// The RT scheduler could not obtain required privileges.
    SchedulerFailure(String),
    /// Catch-all for unexpected failures.
    Unknown(String),
}

impl std::fmt::Display for DegradationReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FfbFault(msg) => write!(f, "FFB fault: {msg}"),
            Self::HidEnumerationFailure(msg) => write!(f, "HID enumeration failure: {msg}"),
            Self::AdapterDisconnect(msg) => write!(f, "Adapter disconnect: {msg}"),
            Self::ConfigError(msg) => write!(f, "Config error: {msg}"),
            Self::PluginFault(msg) => write!(f, "Plugin fault: {msg}"),
            Self::SchedulerFailure(msg) => write!(f, "Scheduler failure: {msg}"),
            Self::Unknown(msg) => write!(f, "Unknown: {msg}"),
        }
    }
}

/// Information about the system for diagnostic bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub os: String,
    pub os_version: String,
    pub arch: String,
    pub rust_version: String,
    pub openflight_version: String,
    pub uptime_secs: u64,
}

impl SystemInfo {
    pub fn collect() -> Self {
        Self {
            os: std::env::consts::OS.to_string(),
            os_version: String::new(),
            arch: std::env::consts::ARCH.to_string(),
            openflight_version: env!("CARGO_PKG_VERSION").to_string(),
            rust_version: "1.92+".to_string(),
            uptime_secs: 0,
        }
    }

    pub fn to_text(&self) -> String {
        format!(
            "OS: {} {}\nArch: {}\nOpenFlight: {}\nRust MSRV: {}\nUptime: {}s\n",
            self.os,
            self.os_version,
            self.arch,
            self.openflight_version,
            self.rust_version,
            self.uptime_secs
        )
    }
}

/// Configuration for diagnostic bundle generation.
#[derive(Debug, Clone)]
pub struct DiagnosticBundleConfig {
    /// Output path for the bundle (defaults to temp dir).
    pub output_path: Option<PathBuf>,
    /// Include config files in bundle.
    pub include_config: bool,
    /// Maximum log lines to include (0 = unlimited).
    pub max_log_lines: usize,
}

impl Default for DiagnosticBundleConfig {
    fn default() -> Self {
        Self {
            output_path: None,
            include_config: true,
            max_log_lines: 10000,
        }
    }
}

/// Diagnostic bundle entry (content to be included in the bundle).
#[derive(Debug, Clone)]
pub struct BundleEntry {
    /// Path within the bundle (e.g., "logs/service.log").
    pub name: String,
    /// Content of the entry.
    pub content: Vec<u8>,
}

impl BundleEntry {
    pub fn new(name: impl Into<String>, content: impl Into<Vec<u8>>) -> Self {
        Self {
            name: name.into(),
            content: content.into(),
        }
    }

    pub fn from_text(name: impl Into<String>, text: impl AsRef<str>) -> Self {
        Self::new(name, text.as_ref().as_bytes().to_vec())
    }
}

/// Generates diagnostic bundle entries.
pub struct DiagnosticBundleBuilder {
    config: DiagnosticBundleConfig,
    entries: Vec<BundleEntry>,
}

impl DiagnosticBundleBuilder {
    pub fn new(config: DiagnosticBundleConfig) -> Self {
        Self {
            config,
            entries: Vec::new(),
        }
    }

    /// Add system info entry.
    pub fn add_system_info(&mut self) -> &mut Self {
        let info = SystemInfo::collect();
        self.entries
            .push(BundleEntry::from_text("system_info.txt", info.to_text()));
        self
    }

    /// Add a text entry.
    pub fn add_text(&mut self, name: impl Into<String>, text: impl AsRef<str>) -> &mut Self {
        self.entries.push(BundleEntry::from_text(name, text));
        self
    }

    /// Add a file entry from disk.
    pub fn add_file(&mut self, name: impl Into<String>, path: &Path) -> &mut Self {
        if let Ok(content) = std::fs::read(path) {
            self.entries.push(BundleEntry::new(name, content));
        }
        self
    }

    /// Returns the default output path.
    pub fn default_output_path() -> PathBuf {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("openflight_diag_{}.txt", ts))
    }

    /// Finalize the bundle entries as a flat text summary.
    pub fn finalize_as_text(&self) -> String {
        let mut out = String::new();
        out.push_str("=== OpenFlight Diagnostic Bundle ===\n\n");
        for entry in &self.entries {
            out.push_str(&format!("--- {} ---\n", entry.name));
            if let Ok(text) = std::str::from_utf8(&entry.content) {
                out.push_str(text);
            } else {
                out.push_str(&format!("[binary {} bytes]\n", entry.content.len()));
            }
            out.push('\n');
        }
        out
    }

    /// Write the bundle to the configured output path.
    pub fn write(&self) -> std::io::Result<PathBuf> {
        let path = self
            .config
            .output_path
            .clone()
            .unwrap_or_else(Self::default_output_path);
        let content = self.finalize_as_text();
        std::fs::write(&path, content)?;
        Ok(path)
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn config(&self) -> &DiagnosticBundleConfig {
        &self.config
    }
}

/// State of a connected device for diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceState {
    pub name: String,
    pub vid_pid: String,
    pub connected: bool,
    pub last_seen_secs_ago: Option<u64>,
    pub error: Option<String>,
}

/// State of a simulator adapter for diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterState {
    pub sim_name: String,
    pub connected: bool,
    pub last_snapshot_secs_ago: Option<u64>,
    pub error: Option<String>,
}

/// Active profile information for diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileInfo {
    pub active_profile: Option<String>,
    pub loaded_profiles: Vec<String>,
    pub last_switch_secs_ago: Option<u64>,
}

/// Service runtime information for diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    pub state: String,
    pub safe_mode_reason: Option<String>,
    pub active_plugins: Vec<String>,
    pub metrics: HashMap<String, String>,
}

/// A recent error entry for diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorEntry {
    pub timestamp_secs_ago: u64,
    pub category: String,
    pub message: String,
}

/// Collects diagnostic information into a structured bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticBundle {
    pub system_info: SystemInfo,
    pub device_states: Vec<DeviceState>,
    pub adapter_states: Vec<AdapterState>,
    pub profile_info: ProfileInfo,
    pub service_info: ServiceInfo,
    pub recent_errors: Vec<ErrorEntry>,
    pub degradation_reason: Option<DegradationReason>,
}

impl DiagnosticBundle {
    /// Serialize the bundle to a JSON string.
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

    /// Render a human-readable text report.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        out.push_str("=== OpenFlight Diagnostic Report ===\n\n");

        if let Some(ref reason) = self.degradation_reason {
            out.push_str(&format!("Degradation reason: {reason}\n\n"));
        }

        out.push_str("## System\n");
        out.push_str(&self.system_info.to_text());
        out.push('\n');

        out.push_str("## Devices\n");
        if self.device_states.is_empty() {
            out.push_str("No devices reported.\n");
        } else {
            for d in &self.device_states {
                let status = if d.connected {
                    "connected"
                } else {
                    "disconnected"
                };
                out.push_str(&format!("- {} [{}] {}", d.name, d.vid_pid, status));
                if let Some(e) = &d.error {
                    out.push_str(&format!(" error: {e}"));
                }
                out.push('\n');
            }
        }
        out.push('\n');

        out.push_str("## Adapters\n");
        if self.adapter_states.is_empty() {
            out.push_str("No adapters reported.\n");
        } else {
            for a in &self.adapter_states {
                let status = if a.connected {
                    "connected"
                } else {
                    "disconnected"
                };
                out.push_str(&format!("- {} {status}", a.sim_name));
                if let Some(e) = &a.error {
                    out.push_str(&format!(" error: {e}"));
                }
                out.push('\n');
            }
        }
        out.push('\n');

        out.push_str("## Profile\n");
        if let Some(ref name) = self.profile_info.active_profile {
            out.push_str(&format!("Active: {name}\n"));
        } else {
            out.push_str("Active: none\n");
        }
        if !self.profile_info.loaded_profiles.is_empty() {
            out.push_str(&format!(
                "Loaded: {}\n",
                self.profile_info.loaded_profiles.join(", ")
            ));
        }
        out.push('\n');

        out.push_str("## Service\n");
        out.push_str(&format!("State: {}\n", self.service_info.state));
        if let Some(ref reason) = self.service_info.safe_mode_reason {
            out.push_str(&format!("Safe mode reason: {reason}\n"));
        }
        if !self.service_info.active_plugins.is_empty() {
            out.push_str(&format!(
                "Plugins: {}\n",
                self.service_info.active_plugins.join(", ")
            ));
        }
        out.push('\n');

        out.push_str("## Errors\n");
        if self.recent_errors.is_empty() {
            out.push_str("No recent errors.\n");
        } else {
            for e in &self.recent_errors {
                out.push_str(&format!(
                    "- [{}] {} ({}s ago)\n",
                    e.category, e.message, e.timestamp_secs_ago
                ));
            }
        }

        out.push_str(&format!("\nSeverity: {}\n", self.severity_summary()));
        out
    }

    /// Number of recent errors.
    pub fn error_count(&self) -> usize {
        self.recent_errors.len()
    }

    /// Returns `true` if any device or adapter is disconnected.
    pub fn has_connectivity_issues(&self) -> bool {
        self.device_states.iter().any(|d| !d.connected)
            || self.adapter_states.iter().any(|a| !a.connected)
    }

    /// Overall severity: "healthy", "degraded", or "critical".
    pub fn severity_summary(&self) -> String {
        if self.adapter_states.iter().any(|a| !a.connected) {
            return "critical".to_string();
        }
        if self.device_states.iter().any(|d| !d.connected) || !self.recent_errors.is_empty() {
            return "degraded".to_string();
        }
        "healthy".to_string()
    }
}

/// Builder for collecting diagnostic info.
pub struct DiagnosticCollector {
    system_info: Option<SystemInfo>,
    device_states: Vec<DeviceState>,
    adapter_states: Vec<AdapterState>,
    profile_info: Option<ProfileInfo>,
    service_info: Option<ServiceInfo>,
    recent_errors: Vec<ErrorEntry>,
    degradation_reason: Option<DegradationReason>,
}

impl DiagnosticCollector {
    pub fn new() -> Self {
        Self {
            system_info: None,
            device_states: Vec::new(),
            adapter_states: Vec::new(),
            profile_info: None,
            service_info: None,
            recent_errors: Vec::new(),
            degradation_reason: None,
        }
    }

    pub fn set_system_info(&mut self, info: SystemInfo) -> &mut Self {
        self.system_info = Some(info);
        self
    }

    pub fn add_device(&mut self, state: DeviceState) -> &mut Self {
        self.device_states.push(state);
        self
    }

    pub fn add_adapter(&mut self, state: AdapterState) -> &mut Self {
        self.adapter_states.push(state);
        self
    }

    pub fn set_profile_info(&mut self, info: ProfileInfo) -> &mut Self {
        self.profile_info = Some(info);
        self
    }

    pub fn set_service_info(&mut self, info: ServiceInfo) -> &mut Self {
        self.service_info = Some(info);
        self
    }

    pub fn add_error(&mut self, entry: ErrorEntry) -> &mut Self {
        self.recent_errors.push(entry);
        self
    }

    pub fn set_degradation_reason(&mut self, reason: DegradationReason) -> &mut Self {
        self.degradation_reason = Some(reason);
        self
    }

    pub fn build(self) -> DiagnosticBundle {
        DiagnosticBundle {
            system_info: self.system_info.unwrap_or_else(SystemInfo::collect),
            device_states: self.device_states,
            adapter_states: self.adapter_states,
            profile_info: self.profile_info.unwrap_or_else(|| ProfileInfo {
                active_profile: None,
                loaded_profiles: Vec::new(),
                last_switch_secs_ago: None,
            }),
            service_info: self.service_info.unwrap_or_else(|| ServiceInfo {
                state: "running".to_string(),
                safe_mode_reason: None,
                active_plugins: Vec::new(),
                metrics: HashMap::new(),
            }),
            recent_errors: self.recent_errors,
            degradation_reason: self.degradation_reason,
        }
    }
}

impl Default for DiagnosticCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_system_info_collect() {
        let info = SystemInfo::collect();
        assert!(!info.os.is_empty());
        assert!(!info.arch.is_empty());
        assert!(!info.openflight_version.is_empty());
    }

    #[test]
    fn test_system_info_to_text() {
        let info = SystemInfo::collect();
        let text = info.to_text();
        assert!(text.contains("OS:"));
        assert!(text.contains("Arch:"));
    }

    #[test]
    fn test_builder_entry_count() {
        let mut builder = DiagnosticBundleBuilder::new(Default::default());
        assert_eq!(builder.entry_count(), 0);
        builder.add_text("test.txt", "hello");
        assert_eq!(builder.entry_count(), 1);
    }

    #[test]
    fn test_builder_add_system_info() {
        let mut builder = DiagnosticBundleBuilder::new(Default::default());
        builder.add_system_info();
        assert_eq!(builder.entry_count(), 1);
    }

    #[test]
    fn test_finalize_as_text_contains_header() {
        let mut builder = DiagnosticBundleBuilder::new(Default::default());
        builder.add_text("test.txt", "content");
        let text = builder.finalize_as_text();
        assert!(text.contains("OpenFlight Diagnostic Bundle"));
        assert!(text.contains("test.txt"));
        assert!(text.contains("content"));
    }

    #[test]
    fn test_write_creates_file() {
        let output = env::temp_dir().join("openflight_diag_test.txt");
        let _ = std::fs::remove_file(&output);
        let config = DiagnosticBundleConfig {
            output_path: Some(output.clone()),
            ..Default::default()
        };
        let mut builder = DiagnosticBundleBuilder::new(config);
        builder.add_system_info();
        let path = builder.write().expect("Should write bundle");
        assert!(path.exists());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_default_config_values() {
        let config = DiagnosticBundleConfig::default();
        assert!(config.include_config);
        assert_eq!(config.max_log_lines, 10000);
        assert!(config.output_path.is_none());
    }

    #[test]
    fn test_bundle_entry_from_text() {
        let entry = BundleEntry::from_text("foo.txt", "bar");
        assert_eq!(entry.name, "foo.txt");
        assert_eq!(entry.content, b"bar");
    }

    #[test]
    fn test_empty_bundle_is_healthy() {
        let bundle = DiagnosticCollector::new().build();
        assert_eq!(bundle.severity_summary(), "healthy");
    }

    #[test]
    fn test_all_systems_ok_is_healthy() {
        let mut collector = DiagnosticCollector::new();
        collector.add_device(DeviceState {
            name: "Joystick".into(),
            vid_pid: "044f:b10a".into(),
            connected: true,
            last_seen_secs_ago: Some(0),
            error: None,
        });
        collector.add_adapter(AdapterState {
            sim_name: "MSFS".into(),
            connected: true,
            last_snapshot_secs_ago: Some(1),
            error: None,
        });
        let bundle = collector.build();
        assert_eq!(bundle.severity_summary(), "healthy");
        assert!(!bundle.has_connectivity_issues());
    }

    #[test]
    fn test_disconnected_device_is_degraded() {
        let mut collector = DiagnosticCollector::new();
        collector.add_device(DeviceState {
            name: "Throttle".into(),
            vid_pid: "044f:b687".into(),
            connected: false,
            last_seen_secs_ago: Some(120),
            error: None,
        });
        let bundle = collector.build();
        assert_eq!(bundle.severity_summary(), "degraded");
        assert!(bundle.has_connectivity_issues());
    }

    #[test]
    fn test_disconnected_sim_is_critical() {
        let mut collector = DiagnosticCollector::new();
        collector.add_adapter(AdapterState {
            sim_name: "X-Plane".into(),
            connected: false,
            last_snapshot_secs_ago: None,
            error: Some("timeout".into()),
        });
        let bundle = collector.build();
        assert_eq!(bundle.severity_summary(), "critical");
        assert!(bundle.has_connectivity_issues());
    }

    #[test]
    fn test_error_count() {
        let mut collector = DiagnosticCollector::new();
        collector.add_error(ErrorEntry {
            timestamp_secs_ago: 10,
            category: "hid".into(),
            message: "read failed".into(),
        });
        collector.add_error(ErrorEntry {
            timestamp_secs_ago: 5,
            category: "sim".into(),
            message: "connection lost".into(),
        });
        let bundle = collector.build();
        assert_eq!(bundle.error_count(), 2);
    }

    #[test]
    fn test_text_report_includes_all_sections() {
        let mut collector = DiagnosticCollector::new();
        collector.set_system_info(SystemInfo {
            os: "windows".into(),
            os_version: "11".into(),
            arch: "x86_64".into(),
            rust_version: "1.92".into(),
            openflight_version: "0.1.0".into(),
            uptime_secs: 3600,
        });
        collector.add_device(DeviceState {
            name: "Stick".into(),
            vid_pid: "1234:5678".into(),
            connected: true,
            last_seen_secs_ago: Some(0),
            error: None,
        });
        collector.add_adapter(AdapterState {
            sim_name: "DCS".into(),
            connected: true,
            last_snapshot_secs_ago: Some(2),
            error: None,
        });
        collector.set_profile_info(ProfileInfo {
            active_profile: Some("default".into()),
            loaded_profiles: vec!["default".into(), "combat".into()],
            last_switch_secs_ago: Some(60),
        });
        collector.set_service_info(ServiceInfo {
            state: "running".into(),
            safe_mode_reason: None,
            active_plugins: vec!["lua-bridge".into()],
            metrics: HashMap::new(),
        });
        let bundle = collector.build();
        let text = bundle.to_text();
        assert!(text.contains("## System"));
        assert!(text.contains("## Devices"));
        assert!(text.contains("## Adapters"));
        assert!(text.contains("## Profile"));
        assert!(text.contains("## Service"));
        assert!(text.contains("## Errors"));
        assert!(text.contains("Severity:"));
    }

    #[test]
    fn test_builder_chains_correctly() {
        let mut collector = DiagnosticCollector::new();
        collector
            .set_system_info(SystemInfo::collect())
            .add_device(DeviceState {
                name: "A".into(),
                vid_pid: "0:0".into(),
                connected: true,
                last_seen_secs_ago: None,
                error: None,
            })
            .add_adapter(AdapterState {
                sim_name: "S".into(),
                connected: true,
                last_snapshot_secs_ago: None,
                error: None,
            })
            .set_profile_info(ProfileInfo {
                active_profile: None,
                loaded_profiles: vec![],
                last_switch_secs_ago: None,
            })
            .set_service_info(ServiceInfo {
                state: "running".into(),
                safe_mode_reason: None,
                active_plugins: vec![],
                metrics: HashMap::new(),
            })
            .add_error(ErrorEntry {
                timestamp_secs_ago: 1,
                category: "test".into(),
                message: "msg".into(),
            });
        let bundle = collector.build();
        assert_eq!(bundle.error_count(), 1);
        assert_eq!(bundle.device_states.len(), 1);
    }

    #[test]
    fn test_severity_with_multiple_issues() {
        let mut collector = DiagnosticCollector::new();
        collector.add_device(DeviceState {
            name: "D1".into(),
            vid_pid: "0:0".into(),
            connected: false,
            last_seen_secs_ago: None,
            error: None,
        });
        collector.add_adapter(AdapterState {
            sim_name: "Sim".into(),
            connected: false,
            last_snapshot_secs_ago: None,
            error: None,
        });
        collector.add_error(ErrorEntry {
            timestamp_secs_ago: 1,
            category: "x".into(),
            message: "y".into(),
        });
        let bundle = collector.build();
        assert_eq!(bundle.severity_summary(), "critical");
        assert!(bundle.has_connectivity_issues());
        assert_eq!(bundle.error_count(), 1);
    }

    #[test]
    fn test_device_states_listed_correctly() {
        let mut collector = DiagnosticCollector::new();
        collector.add_device(DeviceState {
            name: "Stick".into(),
            vid_pid: "044f:b10a".into(),
            connected: true,
            last_seen_secs_ago: Some(0),
            error: None,
        });
        collector.add_device(DeviceState {
            name: "Throttle".into(),
            vid_pid: "044f:b687".into(),
            connected: false,
            last_seen_secs_ago: Some(30),
            error: Some("USB reset".into()),
        });
        let bundle = collector.build();
        assert_eq!(bundle.device_states.len(), 2);
        let text = bundle.to_text();
        assert!(text.contains("Stick"));
        assert!(text.contains("Throttle"));
        assert!(text.contains("044f:b10a"));
        assert!(text.contains("USB reset"));
    }

    #[test]
    fn test_safe_mode_info_included() {
        let mut collector = DiagnosticCollector::new();
        collector.set_service_info(ServiceInfo {
            state: "safe_mode".into(),
            safe_mode_reason: Some("FFB fault detected".into()),
            active_plugins: vec![],
            metrics: HashMap::new(),
        });
        let bundle = collector.build();
        let text = bundle.to_text();
        assert!(text.contains("safe_mode"));
        assert!(text.contains("FFB fault detected"));
    }

    #[test]
    fn test_to_json_returns_valid_json() {
        let bundle = DiagnosticCollector::new().build();
        let json = bundle.to_json().expect("serialization should succeed");
        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("output should be valid JSON");
        assert!(parsed.is_object());
        assert!(parsed.get("system_info").is_some());
        assert!(parsed.get("device_states").is_some());
        assert!(parsed.get("degradation_reason").is_some());
    }

    #[test]
    fn test_to_json_roundtrip() {
        let mut collector = DiagnosticCollector::new();
        collector
            .set_degradation_reason(DegradationReason::FfbFault("motor stall".into()))
            .add_device(DeviceState {
                name: "Stick".into(),
                vid_pid: "044f:b10a".into(),
                connected: true,
                last_seen_secs_ago: Some(0),
                error: None,
            })
            .add_error(ErrorEntry {
                timestamp_secs_ago: 5,
                category: "ffb".into(),
                message: "motor stall detected".into(),
            });
        let bundle = collector.build();
        let json = bundle.to_json().unwrap();
        let restored: DiagnosticBundle = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.device_states.len(), 1);
        assert_eq!(restored.error_count(), 1);
        assert_eq!(
            restored.degradation_reason,
            Some(DegradationReason::FfbFault("motor stall".into()))
        );
    }

    #[test]
    fn test_degradation_reason_display() {
        assert_eq!(
            DegradationReason::FfbFault("stall".into()).to_string(),
            "FFB fault: stall"
        );
        assert_eq!(
            DegradationReason::HidEnumerationFailure("timeout".into()).to_string(),
            "HID enumeration failure: timeout"
        );
        assert_eq!(
            DegradationReason::AdapterDisconnect("MSFS".into()).to_string(),
            "Adapter disconnect: MSFS"
        );
        assert_eq!(
            DegradationReason::ConfigError("bad toml".into()).to_string(),
            "Config error: bad toml"
        );
        assert_eq!(
            DegradationReason::PluginFault("oom".into()).to_string(),
            "Plugin fault: oom"
        );
        assert_eq!(
            DegradationReason::SchedulerFailure("no MMCSS".into()).to_string(),
            "Scheduler failure: no MMCSS"
        );
        assert_eq!(
            DegradationReason::Unknown("???".into()).to_string(),
            "Unknown: ???"
        );
    }

    #[test]
    fn test_degradation_reason_in_text_report() {
        let mut collector = DiagnosticCollector::new();
        collector.set_degradation_reason(DegradationReason::AdapterDisconnect(
            "X-Plane timeout".into(),
        ));
        let bundle = collector.build();
        let text = bundle.to_text();
        assert!(text.contains("Degradation reason: Adapter disconnect: X-Plane timeout"));
    }

    #[test]
    fn test_no_degradation_reason_omits_line() {
        let bundle = DiagnosticCollector::new().build();
        let text = bundle.to_text();
        assert!(!text.contains("Degradation reason:"));
    }

    #[test]
    fn test_degradation_reason_serializes_as_enum() {
        let reason = DegradationReason::SchedulerFailure("no privileges".into());
        let json = serde_json::to_string(&reason).unwrap();
        assert!(json.contains("SchedulerFailure"));
        assert!(json.contains("no privileges"));
    }

    #[test]
    fn test_collector_set_degradation_reason() {
        let mut collector = DiagnosticCollector::new();
        collector.set_degradation_reason(DegradationReason::ConfigError("missing key".into()));
        let bundle = collector.build();
        assert_eq!(
            bundle.degradation_reason,
            Some(DegradationReason::ConfigError("missing key".into()))
        );
    }

    #[test]
    fn test_bundle_without_degradation_reason_is_none() {
        let bundle = DiagnosticCollector::new().build();
        assert!(bundle.degradation_reason.is_none());
    }

    #[test]
    fn test_to_json_includes_all_fields() {
        let mut collector = DiagnosticCollector::new();
        collector
            .set_system_info(SystemInfo::collect())
            .set_degradation_reason(DegradationReason::HidEnumerationFailure(
                "device not found".into(),
            ))
            .add_device(DeviceState {
                name: "Throttle".into(),
                vid_pid: "044f:b687".into(),
                connected: false,
                last_seen_secs_ago: Some(60),
                error: Some("USB disconnect".into()),
            })
            .add_adapter(AdapterState {
                sim_name: "DCS".into(),
                connected: true,
                last_snapshot_secs_ago: Some(1),
                error: None,
            })
            .set_profile_info(ProfileInfo {
                active_profile: Some("combat".into()),
                loaded_profiles: vec!["combat".into()],
                last_switch_secs_ago: Some(30),
            })
            .set_service_info(ServiceInfo {
                state: "safe_mode".into(),
                safe_mode_reason: Some("HID failure".into()),
                active_plugins: vec![],
                metrics: HashMap::new(),
            })
            .add_error(ErrorEntry {
                timestamp_secs_ago: 2,
                category: "hid".into(),
                message: "device not found".into(),
            });
        let bundle = collector.build();
        let json = bundle.to_json().unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(
            v["degradation_reason"]["HidEnumerationFailure"]
                .as_str()
                .unwrap()
                .contains("device not found")
        );
        assert_eq!(v["device_states"][0]["name"], "Throttle");
        assert_eq!(v["adapter_states"][0]["sim_name"], "DCS");
        assert_eq!(v["profile_info"]["active_profile"], "combat");
        assert_eq!(v["service_info"]["state"], "safe_mode");
        assert_eq!(v["recent_errors"][0]["category"], "hid");
    }
}
