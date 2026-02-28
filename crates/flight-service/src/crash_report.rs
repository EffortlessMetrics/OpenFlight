// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Crash report generation and prior-crash detection (REQ-656).
//!
//! Produces structured JSON crash reports on panic and allows the service
//! to detect whether a previous run ended with a crash.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Name of the crash report file written into the crash directory.
const CRASH_FILE_NAME: &str = "last_crash.json";

/// A structured crash report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashReport {
    /// Service version at the time of the crash.
    pub version: String,
    /// Hash of the active configuration (for reproducibility).
    pub config_hash: String,
    /// Captured backtrace (may be empty if unavailable).
    pub backtrace: String,
    /// Unix timestamp in milliseconds when the crash occurred.
    pub timestamp: u64,
    /// Description of the last known state before the crash.
    pub last_state: String,
}

impl CrashReport {
    /// Build a crash report from a panic payload and contextual info.
    pub fn from_panic(
        version: impl Into<String>,
        config_hash: impl Into<String>,
        last_state: impl Into<String>,
        panic_message: impl Into<String>,
    ) -> Self {
        let bt = std::backtrace::Backtrace::force_capture();
        let _ = panic_message.into(); // consumed for context; backtrace is primary
        Self {
            version: version.into(),
            config_hash: config_hash.into(),
            backtrace: bt.to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            last_state: last_state.into(),
        }
    }

    /// Create a report with an explicit backtrace string (useful for tests).
    pub fn new(
        version: impl Into<String>,
        config_hash: impl Into<String>,
        backtrace: impl Into<String>,
        last_state: impl Into<String>,
    ) -> Self {
        Self {
            version: version.into(),
            config_hash: config_hash.into(),
            backtrace: backtrace.into(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            last_state: last_state.into(),
        }
    }

    /// Write this crash report to `crash_dir/last_crash.json`.
    pub fn write_to_dir(&self, crash_dir: &Path) -> std::io::Result<()> {
        fs::create_dir_all(crash_dir)?;
        let path = crash_dir.join(CRASH_FILE_NAME);
        let json =
            serde_json::to_string_pretty(self).map_err(|e| std::io::Error::other(e.to_string()))?;
        fs::write(path, json)
    }
}

/// Install a panic hook that writes a crash report to `crash_dir`.
///
/// This should be called early in the service startup sequence.
pub fn install_crash_hook(
    crash_dir: PathBuf,
    version: String,
    config_hash: String,
    last_state: String,
) {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic".to_string()
        };

        let report = CrashReport::from_panic(&version, &config_hash, &last_state, &msg);

        if let Err(e) = report.write_to_dir(&crash_dir) {
            eprintln!("Failed to write crash report: {e}");
        }

        prev(info);
    }));
}

/// Check whether a prior crash report exists in `crash_dir`.
///
/// Returns `Some(report)` if `last_crash.json` is present and parseable.
pub fn check_prior_crash(crash_dir: &Path) -> Option<CrashReport> {
    let path = crash_dir.join(CRASH_FILE_NAME);
    let content = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_generation() {
        let report = CrashReport::new("1.2.3", "abc123", "bt here", "processing axis");
        assert_eq!(report.version, "1.2.3");
        assert_eq!(report.config_hash, "abc123");
        assert_eq!(report.backtrace, "bt here");
        assert_eq!(report.last_state, "processing axis");
        assert!(report.timestamp > 0);
    }

    #[test]
    fn report_write_and_parse() {
        let dir = tempfile::tempdir().unwrap();
        let crash_dir = dir.path().join("crashes");

        let report = CrashReport::new("0.1.0", "deadbeef", "trace", "idle");
        report.write_to_dir(&crash_dir).unwrap();

        let path = crash_dir.join(CRASH_FILE_NAME);
        assert!(path.exists());

        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: CrashReport = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed.version, "0.1.0");
        assert_eq!(parsed.config_hash, "deadbeef");
    }

    #[test]
    fn prior_crash_detection() {
        let dir = tempfile::tempdir().unwrap();
        let crash_dir = dir.path().join("crashes");

        // No crash file yet.
        assert!(check_prior_crash(&crash_dir).is_none());

        // Write a crash report.
        let report = CrashReport::new("2.0.0", "hash", "backtrace", "running");
        report.write_to_dir(&crash_dir).unwrap();

        let prior = check_prior_crash(&crash_dir).expect("should find prior crash");
        assert_eq!(prior.version, "2.0.0");
        assert_eq!(prior.last_state, "running");
    }

    #[test]
    fn prior_crash_returns_none_for_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let crash_dir = dir.path();
        std::fs::write(crash_dir.join(CRASH_FILE_NAME), "not json").unwrap();
        assert!(check_prior_crash(crash_dir).is_none());
    }
}
