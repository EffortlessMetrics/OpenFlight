// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Unified service configuration with validation and hot-reload plumbing.
//!
//! [`UnifiedServiceConfig`] aggregates all service-level settings (adapter
//! enables, bus tuning, profile paths, axis engine params) into a single
//! serialisable struct. It feeds into both [`ServiceOrchestrator`] and
//! [`FlightService`] so that every subsystem boots from one source of truth.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::config_watcher::ConfigWatcher;

// ---------------------------------------------------------------------------
// Top-level config
// ---------------------------------------------------------------------------

/// Unified service configuration covering all subsystems.
///
/// Loaded once at startup and optionally reloaded via [`ConfigReloader`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedServiceConfig {
    /// Per-simulator adapter settings.
    pub adapters: AdapterSettings,
    /// Bus tuning parameters.
    pub bus: BusSettings,
    /// Profile search paths (highest priority first).
    pub profile_paths: Vec<PathBuf>,
    /// Axis engine settings.
    pub axis: AxisSettings,
    /// Watchdog / safety settings.
    pub watchdog: WatchdogSettings,
    /// Metrics export settings.
    pub metrics: MetricsSettings,
}

/// Per-simulator adapter settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterSettings {
    pub enable_msfs: bool,
    pub enable_xplane: bool,
    pub enable_dcs: bool,
    pub enable_ac7: bool,
    pub enable_wingman: bool,
    /// Health-check interval per adapter.
    #[serde(with = "humantime_serde")]
    pub health_interval: Duration,
    /// Maximum consecutive errors before marking an adapter as failed.
    pub max_consecutive_errors: u32,
}

/// Bus tuning parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusSettings {
    /// Maximum publish rate in Hz.
    pub max_publish_rate_hz: f32,
    /// Subscriber buffer size.
    pub subscriber_buffer_size: usize,
    /// Whether to drop oldest messages when the buffer is full.
    pub drop_on_full: bool,
}

/// Axis engine settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxisSettings {
    pub enable_rt_checks: bool,
    pub max_frame_time_us: u32,
    pub enable_counters: bool,
    pub enable_conflict_detection: bool,
}

/// Watchdog settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchdogSettings {
    pub enabled: bool,
    /// Shutdown timeout.
    #[serde(with = "humantime_serde")]
    pub shutdown_timeout: Duration,
}

/// Metrics export settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSettings {
    pub enabled: bool,
    pub listen_port: u16,
}

// ---------------------------------------------------------------------------
// serde helper for Duration via humantime strings ("5s", "200ms", …)
// ---------------------------------------------------------------------------

mod humantime_serde {
    use serde::{self, Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_millis() as u64)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let ms = u64::deserialize(deserializer)?;
        Ok(Duration::from_millis(ms))
    }
}

// ---------------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------------

impl Default for UnifiedServiceConfig {
    fn default() -> Self {
        Self {
            adapters: AdapterSettings::default(),
            bus: BusSettings::default(),
            profile_paths: vec![PathBuf::from("profiles")],
            axis: AxisSettings::default(),
            watchdog: WatchdogSettings::default(),
            metrics: MetricsSettings::default(),
        }
    }
}

impl Default for AdapterSettings {
    fn default() -> Self {
        Self {
            enable_msfs: true,
            enable_xplane: true,
            enable_dcs: true,
            enable_ac7: true,
            enable_wingman: true,
            health_interval: Duration::from_secs(5),
            max_consecutive_errors: 5,
        }
    }
}

impl Default for BusSettings {
    fn default() -> Self {
        Self {
            max_publish_rate_hz: 60.0,
            subscriber_buffer_size: 100,
            drop_on_full: true,
        }
    }
}

impl Default for AxisSettings {
    fn default() -> Self {
        Self {
            enable_rt_checks: false,
            max_frame_time_us: 5_000,
            enable_counters: true,
            enable_conflict_detection: false,
        }
    }
}

impl Default for WatchdogSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            shutdown_timeout: Duration::from_secs(5),
        }
    }
}

impl Default for MetricsSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            listen_port: 9898,
        }
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// A single validation error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigValidationError {
    pub field: String,
    pub message: String,
}

impl std::fmt::Display for ConfigValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

impl UnifiedServiceConfig {
    /// Validate that all fields are within acceptable ranges.
    pub fn validate(&self) -> Result<(), Vec<ConfigValidationError>> {
        let mut errors = Vec::new();

        if self.bus.max_publish_rate_hz <= 0.0 || self.bus.max_publish_rate_hz > 1000.0 {
            errors.push(ConfigValidationError {
                field: "bus.max_publish_rate_hz".into(),
                message: "must be in (0, 1000]".into(),
            });
        }

        if self.bus.subscriber_buffer_size == 0 {
            errors.push(ConfigValidationError {
                field: "bus.subscriber_buffer_size".into(),
                message: "must be > 0".into(),
            });
        }

        if self.axis.max_frame_time_us == 0 {
            errors.push(ConfigValidationError {
                field: "axis.max_frame_time_us".into(),
                message: "must be > 0".into(),
            });
        }

        if self.metrics.listen_port == 0 {
            errors.push(ConfigValidationError {
                field: "metrics.listen_port".into(),
                message: "must be > 0".into(),
            });
        }

        if self.adapters.max_consecutive_errors == 0 {
            errors.push(ConfigValidationError {
                field: "adapters.max_consecutive_errors".into(),
                message: "must be > 0".into(),
            });
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Load from a JSON file, validate, and return.
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self, String> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| format!("failed to read config: {e}"))?;
        let config: Self =
            serde_json::from_str(&content).map_err(|e| format!("failed to parse config: {e}"))?;
        config
            .validate()
            .map_err(|errs| format!("config validation failed: {:?}", errs))?;
        Ok(config)
    }

    /// Load from file or fall back to defaults.
    pub fn load_or_default(path: impl AsRef<Path>) -> Self {
        Self::load_from_file(path).unwrap_or_default()
    }

    /// Convert adapter settings to orchestrator [`AdapterFlags`].
    pub fn to_adapter_flags(&self) -> crate::orchestrator::AdapterFlags {
        crate::orchestrator::AdapterFlags {
            msfs: self.adapters.enable_msfs,
            xplane: self.adapters.enable_xplane,
            dcs: self.adapters.enable_dcs,
            ac7: self.adapters.enable_ac7,
            wingman: self.adapters.enable_wingman,
        }
    }

    /// Convert to an orchestrator [`ServiceConfig`].
    pub fn to_orchestrator_config(
        &self,
        config_path: Option<PathBuf>,
    ) -> crate::orchestrator::ServiceConfig {
        crate::orchestrator::ServiceConfig {
            shutdown_timeout_ms: self.watchdog.shutdown_timeout.as_millis() as u64,
            enable_watchdog: self.watchdog.enabled,
            enable_adapters: true,
            adapter_flags: self.to_adapter_flags(),
            config_path,
        }
    }

    /// Return the list of enabled adapter names.
    pub fn enabled_adapters(&self) -> Vec<&'static str> {
        let mut names = Vec::new();
        if self.adapters.enable_msfs {
            names.push("msfs");
        }
        if self.adapters.enable_xplane {
            names.push("xplane");
        }
        if self.adapters.enable_dcs {
            names.push("dcs");
        }
        if self.adapters.enable_ac7 {
            names.push("ac7");
        }
        if self.adapters.enable_wingman {
            names.push("wingman");
        }
        names
    }
}

// ---------------------------------------------------------------------------
// Config hot-reload plumbing
// ---------------------------------------------------------------------------

/// Watches a config file and provides reload notifications.
pub struct ConfigReloader {
    watcher: ConfigWatcher,
    config_path: PathBuf,
    current: UnifiedServiceConfig,
    reload_count: u64,
}

impl ConfigReloader {
    /// Create a new reloader watching `path`.
    pub fn new(path: impl Into<PathBuf>, config: UnifiedServiceConfig) -> Self {
        let path = path.into();
        let mut watcher = ConfigWatcher::new(Duration::from_secs(2));
        watcher.watch(&path);
        // Prime so the initial state is recorded.
        let _ = watcher.check_for_changes();
        Self {
            watcher,
            config_path: path,
            current: config,
            reload_count: 0,
        }
    }

    /// Poll for changes and reload if the file was modified.
    /// Returns `Some(new_config)` when a valid reload occurred.
    pub fn poll(&mut self) -> Option<UnifiedServiceConfig> {
        let changes = self.watcher.check_for_changes();
        if changes.is_empty() {
            return None;
        }

        match UnifiedServiceConfig::load_from_file(&self.config_path) {
            Ok(new_config) => {
                self.current = new_config.clone();
                self.reload_count += 1;
                Some(new_config)
            }
            Err(_) => None,
        }
    }

    /// Current config snapshot.
    pub fn current(&self) -> &UnifiedServiceConfig {
        &self.current
    }

    /// How many successful reloads have been performed.
    pub fn reload_count(&self) -> u64 {
        self.reload_count
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_validates() {
        let cfg = UnifiedServiceConfig::default();
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn validation_catches_zero_publish_rate() {
        let mut cfg = UnifiedServiceConfig::default();
        cfg.bus.max_publish_rate_hz = 0.0;
        let errs = cfg.validate().unwrap_err();
        assert!(errs.iter().any(|e| e.field == "bus.max_publish_rate_hz"));
    }

    #[test]
    fn validation_catches_zero_buffer() {
        let mut cfg = UnifiedServiceConfig::default();
        cfg.bus.subscriber_buffer_size = 0;
        let errs = cfg.validate().unwrap_err();
        assert!(errs.iter().any(|e| e.field == "bus.subscriber_buffer_size"));
    }

    #[test]
    fn validation_catches_zero_frame_time() {
        let mut cfg = UnifiedServiceConfig::default();
        cfg.axis.max_frame_time_us = 0;
        let errs = cfg.validate().unwrap_err();
        assert!(errs.iter().any(|e| e.field == "axis.max_frame_time_us"));
    }

    #[test]
    fn validation_catches_zero_port() {
        let mut cfg = UnifiedServiceConfig::default();
        cfg.metrics.listen_port = 0;
        let errs = cfg.validate().unwrap_err();
        assert!(errs.iter().any(|e| e.field == "metrics.listen_port"));
    }

    #[test]
    fn validation_catches_zero_max_errors() {
        let mut cfg = UnifiedServiceConfig::default();
        cfg.adapters.max_consecutive_errors = 0;
        let errs = cfg.validate().unwrap_err();
        assert!(
            errs.iter()
                .any(|e| e.field == "adapters.max_consecutive_errors")
        );
    }

    #[test]
    fn multiple_errors_collected() {
        let mut cfg = UnifiedServiceConfig::default();
        cfg.bus.max_publish_rate_hz = -1.0;
        cfg.axis.max_frame_time_us = 0;
        cfg.metrics.listen_port = 0;
        let errs = cfg.validate().unwrap_err();
        assert!(errs.len() >= 3, "expected ≥3 errors, got {}", errs.len());
    }

    #[test]
    fn enabled_adapters_lists_only_enabled() {
        let mut cfg = UnifiedServiceConfig::default();
        cfg.adapters.enable_msfs = false;
        cfg.adapters.enable_wingman = false;
        let names = cfg.enabled_adapters();
        assert_eq!(names, vec!["xplane", "dcs", "ac7"]);
    }

    #[test]
    fn to_adapter_flags_maps_correctly() {
        let mut cfg = UnifiedServiceConfig::default();
        cfg.adapters.enable_msfs = false;
        cfg.adapters.enable_dcs = false;
        let flags = cfg.to_adapter_flags();
        assert!(!flags.msfs);
        assert!(flags.xplane);
        assert!(!flags.dcs);
        assert!(flags.ac7);
        assert!(flags.wingman);
    }

    #[test]
    fn to_orchestrator_config_propagates_settings() {
        let cfg = UnifiedServiceConfig::default();
        let orch_cfg = cfg.to_orchestrator_config(Some(PathBuf::from("test.json")));
        assert!(orch_cfg.enable_watchdog);
        assert!(orch_cfg.enable_adapters);
        assert_eq!(
            orch_cfg.shutdown_timeout_ms,
            cfg.watchdog.shutdown_timeout.as_millis() as u64
        );
        assert_eq!(orch_cfg.config_path, Some(PathBuf::from("test.json")));
    }

    #[test]
    fn serde_roundtrip() {
        let cfg = UnifiedServiceConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: UnifiedServiceConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg.adapters.enable_msfs, parsed.adapters.enable_msfs);
        assert_eq!(cfg.bus.max_publish_rate_hz, parsed.bus.max_publish_rate_hz);
        assert_eq!(cfg.axis.max_frame_time_us, parsed.axis.max_frame_time_us);
    }

    #[test]
    fn load_or_default_returns_default_on_missing_file() {
        let cfg = UnifiedServiceConfig::load_or_default("nonexistent.json");
        // Should match defaults
        assert!(cfg.adapters.enable_msfs);
        assert_eq!(cfg.bus.max_publish_rate_hz, 60.0);
    }

    #[test]
    fn load_from_file_with_valid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let cfg = UnifiedServiceConfig::default();
        std::fs::write(&path, serde_json::to_string_pretty(&cfg).unwrap()).unwrap();
        let loaded = UnifiedServiceConfig::load_from_file(&path).unwrap();
        assert_eq!(loaded.bus.max_publish_rate_hz, 60.0);
    }

    #[test]
    fn load_from_file_rejects_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.json");
        std::fs::write(&path, "not json").unwrap();
        assert!(UnifiedServiceConfig::load_from_file(&path).is_err());
    }

    #[test]
    fn config_reloader_tracks_count() {
        let cfg = UnifiedServiceConfig::default();
        let reloader = ConfigReloader::new("test.json", cfg);
        assert_eq!(reloader.reload_count(), 0);
    }

    #[test]
    fn config_validation_error_display() {
        let err = ConfigValidationError {
            field: "bus.rate".into(),
            message: "too high".into(),
        };
        assert_eq!(format!("{err}"), "bus.rate: too high");
    }
}
