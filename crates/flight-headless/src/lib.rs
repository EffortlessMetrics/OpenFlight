//! Headless operation mode for OpenFlight.
//!
//! Provides a mode for running OpenFlight without a display, suitable for:
//! - Server/rack-mount setups
//! - CI and automated testing
//! - Replay and batch processing

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

/// Configuration for headless mode.
#[derive(Debug, Clone)]
pub struct HeadlessConfig {
    /// Whether headless mode is enabled.
    pub enabled: bool,
    /// Maximum runtime before auto-shutdown (None = run forever).
    pub max_duration: Option<Duration>,
    /// Tick rate in Hz for batch processing.
    pub tick_rate_hz: f64,
    /// Whether to exit with error on first failure.
    pub fail_fast: bool,
    /// Output format for batch results.
    pub output_format: OutputFormat,
}

impl Default for HeadlessConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_duration: None,
            tick_rate_hz: 250.0,
            fail_fast: false,
            output_format: OutputFormat::Text,
        }
    }
}

/// Output format for headless mode results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
    Csv,
}

/// Headless mode session result.
#[derive(Debug, Clone)]
pub struct HeadlessResult {
    pub ticks: u64,
    pub errors: u64,
    pub duration_ms: u64,
    pub exit_code: i32,
}

impl HeadlessResult {
    pub fn success(ticks: u64, duration_ms: u64) -> Self {
        Self {
            ticks,
            errors: 0,
            duration_ms,
            exit_code: 0,
        }
    }

    pub fn with_errors(ticks: u64, errors: u64, duration_ms: u64) -> Self {
        Self {
            ticks,
            errors,
            duration_ms,
            exit_code: if errors > 0 { 1 } else { 0 },
        }
    }

    pub fn is_success(&self) -> bool {
        self.exit_code == 0
    }

    pub fn to_text(&self) -> String {
        format!(
            "Headless run: {} ticks, {} errors, {}ms",
            self.ticks, self.errors, self.duration_ms
        )
    }

    pub fn to_json(&self) -> String {
        format!(
            r#"{{"ticks":{},"errors":{},"duration_ms":{},"exit_code":{}}}"#,
            self.ticks, self.errors, self.duration_ms, self.exit_code
        )
    }

    pub fn to_csv(&self) -> String {
        format!(
            "{},{},{},{}",
            self.ticks, self.errors, self.duration_ms, self.exit_code
        )
    }
}

/// Headless mode runner.
///
/// Runs the service in headless mode for a specified duration or tick count.
pub struct HeadlessRunner {
    config: HeadlessConfig,
    running: Arc<AtomicBool>,
    tick_count: Arc<AtomicU64>,
    error_count: Arc<AtomicU64>,
}

impl HeadlessRunner {
    pub fn new(config: HeadlessConfig) -> Self {
        Self {
            config,
            running: Arc::new(AtomicBool::new(false)),
            tick_count: Arc::new(AtomicU64::new(0)),
            error_count: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    pub fn tick_count(&self) -> u64 {
        self.tick_count.load(Ordering::Relaxed)
    }

    pub fn error_count(&self) -> u64 {
        self.error_count.load(Ordering::Relaxed)
    }

    pub fn config(&self) -> &HeadlessConfig {
        &self.config
    }

    pub fn record_tick(&self) {
        self.tick_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_error(&self) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn start(&self) {
        self.running.store(true, Ordering::Relaxed);
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }

    pub fn build_result(&self, duration_ms: u64) -> HeadlessResult {
        HeadlessResult::with_errors(self.tick_count(), self.error_count(), duration_ms)
    }

    pub fn format_result(&self, result: &HeadlessResult) -> String {
        match self.config.output_format {
            OutputFormat::Json => result.to_json(),
            OutputFormat::Csv => result.to_csv(),
            OutputFormat::Text => result.to_text(),
        }
    }
}

#[cfg(test)]
mod depth_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = HeadlessConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.tick_rate_hz, 250.0);
        assert!(!config.fail_fast);
    }

    #[test]
    fn test_result_success() {
        let r = HeadlessResult::success(1000, 4000);
        assert!(r.is_success());
        assert_eq!(r.ticks, 1000);
        assert_eq!(r.errors, 0);
    }

    #[test]
    fn test_result_with_errors() {
        let r = HeadlessResult::with_errors(1000, 5, 4000);
        assert!(!r.is_success());
        assert_eq!(r.exit_code, 1);
    }

    #[test]
    fn test_result_to_json() {
        let r = HeadlessResult::success(100, 400);
        let j = r.to_json();
        assert!(j.contains("\"ticks\":100"));
        assert!(j.contains("\"errors\":0"));
    }

    #[test]
    fn test_result_to_csv() {
        let r = HeadlessResult::success(100, 400);
        let csv = r.to_csv();
        assert!(csv.contains("100,0,400,0"));
    }

    #[test]
    fn test_runner_not_running_initially() {
        let runner = HeadlessRunner::new(Default::default());
        assert!(!runner.is_running());
    }

    #[test]
    fn test_runner_start_stop() {
        let runner = HeadlessRunner::new(Default::default());
        runner.start();
        assert!(runner.is_running());
        runner.stop();
        assert!(!runner.is_running());
    }

    #[test]
    fn test_runner_record_ticks() {
        let runner = HeadlessRunner::new(Default::default());
        runner.record_tick();
        runner.record_tick();
        assert_eq!(runner.tick_count(), 2);
    }

    #[test]
    fn test_runner_record_errors() {
        let runner = HeadlessRunner::new(Default::default());
        runner.record_error();
        assert_eq!(runner.error_count(), 1);
    }

    #[test]
    fn test_runner_format_result_json() {
        let config = HeadlessConfig {
            output_format: OutputFormat::Json,
            ..Default::default()
        };
        let runner = HeadlessRunner::new(config);
        let r = HeadlessResult::success(50, 200);
        let out = runner.format_result(&r);
        assert!(out.starts_with('{'));
    }

    #[test]
    fn test_runner_format_result_csv() {
        let config = HeadlessConfig {
            output_format: OutputFormat::Csv,
            ..Default::default()
        };
        let runner = HeadlessRunner::new(config);
        let r = HeadlessResult::success(50, 200);
        let out = runner.format_result(&r);
        assert!(out.contains(','));
        assert!(!out.starts_with('{'));
    }

    #[test]
    fn test_output_format_default() {
        let f = OutputFormat::default();
        assert_eq!(f, OutputFormat::Text);
    }
}
