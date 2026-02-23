// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Performance regression detection and alerting
//!
//! Provides automated detection of performance regressions by comparing
//! current metrics against historical baselines and configurable thresholds.

use crate::counters::CounterSnapshot;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};

/// Regression detection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressionDetector {
    /// Historical baseline snapshots
    baselines: VecDeque<CounterSnapshot>,
    /// Maximum number of baselines to keep
    max_baselines: usize,
    /// Thresholds for regression detection
    thresholds: Thresholds,
    /// Minimum sample count for valid comparison
    min_samples: usize,
}

/// Performance thresholds for regression detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thresholds {
    /// Jitter p99 threshold (nanoseconds)
    pub jitter_p99_ns: Threshold,
    /// HID write average latency threshold (nanoseconds)
    pub hid_avg_ns: Threshold,
    /// Deadline miss rate threshold (0.0 to 1.0)
    pub miss_rate: Threshold,
    /// Writer drop count threshold
    pub writer_drops: Threshold,
}

/// Individual threshold configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Threshold {
    /// Absolute maximum value (hard limit)
    pub absolute_max: f64,
    /// Relative increase threshold (e.g., 0.20 = 20% increase)
    pub relative_increase: f64,
    /// Number of consecutive violations before alerting
    pub consecutive_violations: usize,
}

/// Regression detection result
#[derive(Debug, Clone)]
pub struct RegressionResult {
    /// Whether a regression was detected
    pub regression_detected: bool,
    /// List of alerts generated
    pub alerts: Vec<Alert>,
    /// Current snapshot
    pub current: CounterSnapshot,
    /// Baseline used for comparison
    pub baseline: Option<CounterSnapshot>,
    /// Comparison statistics
    pub comparison: Option<ComparisonStats>,
}

/// Performance alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    /// Alert severity level
    pub severity: AlertSeverity,
    /// Metric that triggered the alert
    pub metric: String,
    /// Human-readable description
    pub description: String,
    /// Current value
    pub current_value: f64,
    /// Baseline value (if available)
    pub baseline_value: Option<f64>,
    /// Threshold that was exceeded
    pub threshold: f64,
    /// Timestamp when alert was generated
    pub timestamp: u64,
}

/// Alert severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
    /// Performance degradation detected
    Warning,
    /// Significant regression detected
    Critical,
    /// Quality gate failure
    Fatal,
}

/// Comparison statistics between current and baseline
#[derive(Debug, Clone)]
pub struct ComparisonStats {
    /// Jitter comparison
    pub jitter: MetricComparison,
    /// HID latency comparison
    pub hid_latency: MetricComparison,
    /// Miss rate comparison
    pub miss_rate: MetricComparison,
    /// Writer drops comparison
    pub writer_drops: MetricComparison,
}

/// Individual metric comparison
#[derive(Debug, Clone)]
pub struct MetricComparison {
    /// Current value
    pub current: f64,
    /// Baseline value
    pub baseline: f64,
    /// Absolute change
    pub absolute_change: f64,
    /// Relative change (percentage)
    pub relative_change: f64,
    /// Whether this metric shows regression
    pub is_regression: bool,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            // QG-AX-Jitter: p99 ≤ 0.5ms
            jitter_p99_ns: Threshold {
                absolute_max: 500_000.0, // 0.5ms
                relative_increase: 0.20, // 20% increase
                consecutive_violations: 3,
            },

            // QG-HID-Latency: p99 ≤ 300μs
            hid_avg_ns: Threshold {
                absolute_max: 300_000.0, // 300μs
                relative_increase: 0.20, // 20% increase
                consecutive_violations: 3,
            },

            // Deadline miss rate should be very low
            miss_rate: Threshold {
                absolute_max: 0.01,      // 1%
                relative_increase: 0.50, // 50% increase
                consecutive_violations: 2,
            },

            // Writer drops should be rare
            writer_drops: Threshold {
                absolute_max: 100.0,    // 100 drops per session
                relative_increase: 2.0, // 200% increase (very sensitive)
                consecutive_violations: 1,
            },
        }
    }
}

impl Default for RegressionDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl RegressionDetector {
    /// Create new regression detector with default thresholds
    pub fn new() -> Self {
        Self::with_config(50, Thresholds::default(), 1000)
    }

    /// Create regression detector with custom configuration
    pub fn with_config(max_baselines: usize, thresholds: Thresholds, min_samples: usize) -> Self {
        Self {
            baselines: VecDeque::with_capacity(max_baselines),
            max_baselines,
            thresholds,
            min_samples,
        }
    }

    /// Add a baseline snapshot
    pub fn add_baseline(&mut self, snapshot: CounterSnapshot) {
        if self.baselines.len() >= self.max_baselines {
            self.baselines.pop_front();
        }
        self.baselines.push_back(snapshot);
    }

    /// Check for regressions against current baselines
    pub fn check_regression(&self, current: CounterSnapshot) -> RegressionResult {
        let baseline = self.get_best_baseline(&current);

        let comparison = baseline
            .as_ref()
            .map(|b| self.compare_snapshots(&current, b));

        let alerts = self.generate_alerts(&current, baseline.as_ref(), comparison.as_ref());

        let regression_detected = alerts
            .iter()
            .any(|a| matches!(a.severity, AlertSeverity::Critical | AlertSeverity::Fatal));

        RegressionResult {
            regression_detected,
            alerts,
            current,
            baseline,
            comparison,
        }
    }

    /// Get the best baseline for comparison
    fn get_best_baseline(&self, current: &CounterSnapshot) -> Option<CounterSnapshot> {
        if self.baselines.is_empty() {
            return None;
        }

        // Find baseline with similar session duration (within 50%)
        let target_duration = current.session_duration_ms;
        let tolerance = target_duration / 2;

        let similar_baseline = self
            .baselines
            .iter()
            .rfind(|b| {
                let duration_diff = b.session_duration_ms.abs_diff(target_duration);
                duration_diff <= tolerance && b.jitter.sample_count >= self.min_samples
            }); // Use most recent similar baseline

        // Fall back to most recent baseline if no similar one found
        similar_baseline
            .cloned()
            .or_else(|| self.baselines.back().cloned())
    }

    /// Compare two snapshots and generate statistics
    fn compare_snapshots(
        &self,
        current: &CounterSnapshot,
        baseline: &CounterSnapshot,
    ) -> ComparisonStats {
        ComparisonStats {
            jitter: self.compare_metric(
                current.jitter.p99_ns as f64,
                baseline.jitter.p99_ns as f64,
                &self.thresholds.jitter_p99_ns,
            ),
            hid_latency: self.compare_metric(
                current.hid.avg_time_ns as f64,
                baseline.hid.avg_time_ns as f64,
                &self.thresholds.hid_avg_ns,
            ),
            miss_rate: self.compare_metric(
                current.miss_rate,
                baseline.miss_rate,
                &self.thresholds.miss_rate,
            ),
            writer_drops: self.compare_metric(
                current.writer_drops as f64,
                baseline.writer_drops as f64,
                &self.thresholds.writer_drops,
            ),
        }
    }

    /// Compare individual metric values
    fn compare_metric(
        &self,
        current: f64,
        baseline: f64,
        threshold: &Threshold,
    ) -> MetricComparison {
        let absolute_change = current - baseline;
        let relative_change = if baseline != 0.0 {
            absolute_change / baseline
        } else if current > 0.0 {
            1.0 // 100% increase from zero
        } else {
            0.0
        };

        let is_regression = current > threshold.absolute_max
            || (baseline > 0.0 && relative_change > threshold.relative_increase);

        MetricComparison {
            current,
            baseline,
            absolute_change,
            relative_change,
            is_regression,
        }
    }

    /// Generate alerts based on current snapshot and comparison
    fn generate_alerts(
        &self,
        current: &CounterSnapshot,
        baseline: Option<&CounterSnapshot>,
        comparison: Option<&ComparisonStats>,
    ) -> Vec<Alert> {
        let mut alerts = Vec::new();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Check absolute thresholds (quality gates)
        if current.jitter.sample_count >= self.min_samples
            && current.jitter.p99_ns.abs() > self.thresholds.jitter_p99_ns.absolute_max as i64
        {
            alerts.push(Alert {
                severity: AlertSeverity::Fatal,
                metric: "jitter_p99".to_string(),
                description: format!(
                    "Jitter p99 exceeds quality gate: {:.1}μs > {:.1}μs",
                    current.jitter.p99_ns as f64 / 1_000.0,
                    self.thresholds.jitter_p99_ns.absolute_max / 1_000.0
                ),
                current_value: current.jitter.p99_ns as f64,
                baseline_value: baseline.map(|b| b.jitter.p99_ns as f64),
                threshold: self.thresholds.jitter_p99_ns.absolute_max,
                timestamp,
            });
        }

        if current.hid.avg_time_ns > self.thresholds.hid_avg_ns.absolute_max as u64 {
            alerts.push(Alert {
                severity: AlertSeverity::Fatal,
                metric: "hid_avg_latency".to_string(),
                description: format!(
                    "HID average latency exceeds quality gate: {:.1}μs > {:.1}μs",
                    current.hid.avg_time_ns as f64 / 1_000.0,
                    self.thresholds.hid_avg_ns.absolute_max / 1_000.0
                ),
                current_value: current.hid.avg_time_ns as f64,
                baseline_value: baseline.map(|b| b.hid.avg_time_ns as f64),
                threshold: self.thresholds.hid_avg_ns.absolute_max,
                timestamp,
            });
        }

        if current.miss_rate > self.thresholds.miss_rate.absolute_max {
            alerts.push(Alert {
                severity: AlertSeverity::Critical,
                metric: "miss_rate".to_string(),
                description: format!(
                    "Deadline miss rate too high: {:.2}% > {:.2}%",
                    current.miss_rate * 100.0,
                    self.thresholds.miss_rate.absolute_max * 100.0
                ),
                current_value: current.miss_rate,
                baseline_value: baseline.map(|b| b.miss_rate),
                threshold: self.thresholds.miss_rate.absolute_max,
                timestamp,
            });
        }

        // Check relative regressions
        if let Some(comp) = comparison {
            if comp.jitter.is_regression
                && comp.jitter.relative_change > self.thresholds.jitter_p99_ns.relative_increase
            {
                alerts.push(Alert {
                    severity: AlertSeverity::Critical,
                    metric: "jitter_p99_regression".to_string(),
                    description: format!(
                        "Jitter p99 regression detected: {:.1}% increase ({:.1}μs → {:.1}μs)",
                        comp.jitter.relative_change * 100.0,
                        comp.jitter.baseline / 1_000.0,
                        comp.jitter.current / 1_000.0
                    ),
                    current_value: comp.jitter.current,
                    baseline_value: Some(comp.jitter.baseline),
                    threshold: self.thresholds.jitter_p99_ns.relative_increase,
                    timestamp,
                });
            }

            if comp.hid_latency.is_regression
                && comp.hid_latency.relative_change > self.thresholds.hid_avg_ns.relative_increase
            {
                alerts.push(Alert {
                    severity: AlertSeverity::Critical,
                    metric: "hid_latency_regression".to_string(),
                    description: format!(
                        "HID latency regression detected: {:.1}% increase ({:.1}μs → {:.1}μs)",
                        comp.hid_latency.relative_change * 100.0,
                        comp.hid_latency.baseline / 1_000.0,
                        comp.hid_latency.current / 1_000.0
                    ),
                    current_value: comp.hid_latency.current,
                    baseline_value: Some(comp.hid_latency.baseline),
                    threshold: self.thresholds.hid_avg_ns.relative_increase,
                    timestamp,
                });
            }

            if comp.writer_drops.is_regression && comp.writer_drops.current > 0.0 {
                alerts.push(Alert {
                    severity: AlertSeverity::Warning,
                    metric: "writer_drops".to_string(),
                    description: format!(
                        "Writer drops detected: {} (baseline: {})",
                        comp.writer_drops.current as u64, comp.writer_drops.baseline as u64
                    ),
                    current_value: comp.writer_drops.current,
                    baseline_value: Some(comp.writer_drops.baseline),
                    threshold: self.thresholds.writer_drops.absolute_max,
                    timestamp,
                });
            }
        }

        alerts
    }

    /// Load baselines from JSON file
    pub fn load_baselines(
        &mut self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let baselines: Vec<CounterSnapshot> = serde_json::from_str(&content)?;

        self.baselines.clear();
        for baseline in baselines {
            self.add_baseline(baseline);
        }

        Ok(())
    }

    /// Save baselines to JSON file
    pub fn save_baselines(&self, path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        let baselines: Vec<_> = self.baselines.iter().collect();
        let content = serde_json::to_string_pretty(&baselines)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Get summary statistics of all baselines
    pub fn get_baseline_summary(&self) -> Option<BaselineSummary> {
        if self.baselines.is_empty() {
            return None;
        }

        let jitter_p99s: Vec<i64> = self.baselines.iter().map(|b| b.jitter.p99_ns).collect();

        let hid_avgs: Vec<u64> = self.baselines.iter().map(|b| b.hid.avg_time_ns).collect();

        let miss_rates: Vec<f64> = self.baselines.iter().map(|b| b.miss_rate).collect();

        Some(BaselineSummary {
            count: self.baselines.len(),
            jitter_p99_median: median_i64(&jitter_p99s),
            hid_avg_median: median_u64(&hid_avgs),
            miss_rate_median: median_f64(&miss_rates),
        })
    }
}

/// Summary of baseline performance
#[derive(Debug, Clone)]
pub struct BaselineSummary {
    pub count: usize,
    pub jitter_p99_median: i64,
    pub hid_avg_median: u64,
    pub miss_rate_median: f64,
}

/// Helper function to calculate median of i64 values
fn median_i64(values: &[i64]) -> i64 {
    if values.is_empty() {
        return 0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    sorted[sorted.len() / 2]
}

/// Helper function to calculate median of u64 values
fn median_u64(values: &[u64]) -> u64 {
    if values.is_empty() {
        return 0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    sorted[sorted.len() / 2]
}

/// Helper function to calculate median of f64 values
fn median_f64(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    sorted[sorted.len() / 2]
}

impl std::fmt::Display for Alert {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] {}: {}",
            match self.severity {
                AlertSeverity::Warning => "WARN",
                AlertSeverity::Critical => "CRIT",
                AlertSeverity::Fatal => "FATAL",
            },
            self.metric,
            self.description
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::counters::{CounterSnapshot, HidStats, JitterStats};

    fn create_test_snapshot(jitter_p99: i64, hid_avg: u64, miss_rate: f64) -> CounterSnapshot {
        CounterSnapshot {
            total_ticks: 1000,
            deadline_misses: (miss_rate * 1000.0) as u64,
            miss_rate,
            total_hid_writes: 100,
            writer_drops: 0,
            jitter: JitterStats {
                p50_ns: jitter_p99 / 2,
                p99_ns: jitter_p99,
                max_ns: jitter_p99 * 2,
                sample_count: 1000,
            },
            hid: HidStats {
                total_writes: 100,
                total_time_ns: hid_avg * 100,
                avg_time_ns: hid_avg,
                p99_time_ns: hid_avg * 2,
            },
            session_duration_ms: 4000,
            timestamp_ns: 0,
        }
    }

    #[test]
    fn test_regression_detector_creation() {
        let detector = RegressionDetector::new();
        assert_eq!(detector.baselines.len(), 0);
        assert_eq!(detector.max_baselines, 50);
    }

    #[test]
    fn test_baseline_management() {
        let mut detector = RegressionDetector::new();

        // Add some baselines
        for i in 0..5 {
            let snapshot = create_test_snapshot(1000 + i * 100, 200000 + (i as u64) * 10000, 0.001);
            detector.add_baseline(snapshot);
        }

        assert_eq!(detector.baselines.len(), 5);

        // Test capacity limit
        let detector_small = RegressionDetector::with_config(3, Thresholds::default(), 100);
        let mut detector_small = detector_small;

        for i in 0..5 {
            let snapshot = create_test_snapshot(1000, 200000, 0.001);
            detector_small.add_baseline(snapshot);
        }

        assert_eq!(detector_small.baselines.len(), 3); // Should be capped at 3
    }

    #[test]
    fn test_quality_gate_violations() {
        let detector = RegressionDetector::new();

        // Create snapshot that violates quality gates
        let bad_snapshot = create_test_snapshot(
            600_000, // 0.6ms jitter (exceeds 0.5ms gate)
            400_000, // 400μs HID latency (exceeds 300μs gate)
            0.02,    // 2% miss rate (exceeds 1% gate)
        );

        let result = detector.check_regression(bad_snapshot);

        assert!(result.regression_detected);
        assert!(result.alerts.len() >= 2); // Should have jitter and HID alerts

        // Check for fatal alerts
        let fatal_alerts: Vec<_> = result
            .alerts
            .iter()
            .filter(|a| a.severity == AlertSeverity::Fatal)
            .collect();
        assert!(!fatal_alerts.is_empty());
    }

    #[test]
    fn test_regression_detection() {
        let mut detector = RegressionDetector::new();

        // Add good baseline
        let baseline = create_test_snapshot(100_000, 200_000, 0.001); // 0.1ms, 200μs, 0.1%
        detector.add_baseline(baseline);

        // Test regression (30% increase in jitter)
        let regressed = create_test_snapshot(130_000, 200_000, 0.001);
        let result = detector.check_regression(regressed);

        assert!(result.regression_detected);
        assert!(result.comparison.is_some());

        let comp = result.comparison.unwrap();
        assert!(comp.jitter.is_regression);
        assert!((comp.jitter.relative_change - 0.30).abs() < 0.01); // ~30% increase
    }

    #[test]
    fn test_no_regression_with_good_performance() {
        let mut detector = RegressionDetector::new();

        // Add baseline
        let baseline = create_test_snapshot(100_000, 200_000, 0.001);
        detector.add_baseline(baseline);

        // Test similar performance (5% increase - within threshold)
        let similar = create_test_snapshot(105_000, 210_000, 0.001);
        let result = detector.check_regression(similar);

        assert!(!result.regression_detected);

        // Should still have comparison stats
        assert!(result.comparison.is_some());
        let comp = result.comparison.unwrap();
        assert!(!comp.jitter.is_regression);
        assert!(!comp.hid_latency.is_regression);
    }

    #[test]
    fn test_baseline_selection() {
        let mut detector = RegressionDetector::new();

        // Add baselines with different durations
        let short_baseline = CounterSnapshot {
            session_duration_ms: 2000, // 2 seconds
            ..create_test_snapshot(100_000, 200_000, 0.001)
        };

        let long_baseline = CounterSnapshot {
            session_duration_ms: 8000, // 8 seconds
            ..create_test_snapshot(100_000, 200_000, 0.001)
        };

        detector.add_baseline(short_baseline);
        detector.add_baseline(long_baseline);

        // Test with 4-second session (should prefer short_baseline as it's within tolerance)
        let current = CounterSnapshot {
            session_duration_ms: 4000,
            ..create_test_snapshot(100_000, 200_000, 0.001)
        };

        let result = detector.check_regression(current);
        assert!(result.baseline.is_some());
        // Should use short_baseline (2000ms) as it's within 50% tolerance (2000ms)
        // Long baseline (8000ms) is outside tolerance: |4000-8000| = 4000 > 2000
        assert_eq!(result.baseline.unwrap().session_duration_ms, 2000);
    }

    #[test]
    fn test_alert_formatting() {
        let alert = Alert {
            severity: AlertSeverity::Critical,
            metric: "jitter_p99".to_string(),
            description: "Jitter too high".to_string(),
            current_value: 600_000.0,
            baseline_value: Some(400_000.0),
            threshold: 500_000.0,
            timestamp: 1234567890,
        };

        let formatted = format!("{}", alert);
        assert!(formatted.contains("[CRIT]"));
        assert!(formatted.contains("jitter_p99"));
        assert!(formatted.contains("Jitter too high"));
    }

    #[test]
    fn test_baseline_persistence() {
        use tempfile::NamedTempFile;

        let mut detector = RegressionDetector::new();

        // Add some baselines
        for i in 0..3 {
            let snapshot = create_test_snapshot(1000 + i * 100, 200000, 0.001);
            detector.add_baseline(snapshot);
        }

        // Save to file
        let temp_file = NamedTempFile::new().unwrap();
        detector.save_baselines(temp_file.path()).unwrap();

        // Load into new detector
        let mut new_detector = RegressionDetector::new();
        new_detector.load_baselines(temp_file.path()).unwrap();

        assert_eq!(new_detector.baselines.len(), 3);
        assert_eq!(new_detector.baselines[0].jitter.p99_ns, 1000);
        assert_eq!(new_detector.baselines[2].jitter.p99_ns, 1200);
    }
}
