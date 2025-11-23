// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Output comparison with floating-point tolerance

use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::replay_config::ToleranceConfig;

/// Output comparison errors
#[derive(Error, Debug)]
pub enum ComparisonError {
    #[error(
        "Axis output mismatch: expected {expected}, got {actual}, diff {diff} > epsilon {epsilon}"
    )]
    AxisMismatch {
        expected: f32,
        actual: f32,
        diff: f32,
        epsilon: f32,
    },
    #[error(
        "FFB output mismatch: expected {expected} Nm, got {actual} Nm, diff {diff} > epsilon {epsilon}"
    )]
    FfbMismatch {
        expected: f32,
        actual: f32,
        diff: f32,
        epsilon: f32,
    },
    #[error("Timing drift exceeded: {drift_ns} ns/s > limit {limit_ns} ns/s")]
    TimingDrift { drift_ns: u64, limit_ns: u64 },
    #[error("Timing jitter exceeded: {jitter_ns} ns > limit {limit_ns} ns")]
    TimingJitter { jitter_ns: u64, limit_ns: u64 },
    #[error("Missing device output: {device_id}")]
    MissingDevice { device_id: String },
}

/// Configuration for output comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonConfig {
    /// Tolerance settings
    pub tolerance: ToleranceConfig,
    /// Whether to collect detailed statistics
    pub collect_stats: bool,
    /// Whether to fail fast on first mismatch
    pub fail_fast: bool,
}

/// Result of output comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonResult {
    /// Whether comparison passed
    pub passed: bool,
    /// Number of comparisons performed
    pub total_comparisons: u64,
    /// Number of mismatches found
    pub mismatches: u64,
    /// Axis comparison statistics
    pub axis_stats: ComparisonStats,
    /// FFB comparison statistics
    pub ffb_stats: ComparisonStats,
    /// Timing statistics
    pub timing_stats: TimingStats,
    /// Detailed error messages (if any)
    pub errors: Vec<String>,
}

/// Statistics for a specific comparison type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonStats {
    /// Number of comparisons
    pub count: u64,
    /// Maximum absolute difference observed
    pub max_diff: f32,
    /// Average absolute difference
    pub avg_diff: f32,
    /// Root mean square difference
    pub rms_diff: f32,
    /// Number of values within tolerance
    pub within_tolerance: u64,
}

/// Timing-specific statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingStats {
    /// Number of timing measurements
    pub count: u64,
    /// Maximum timing drift (ns/s)
    pub max_drift_ns_per_s: u64,
    /// Average timing drift (ns/s)
    pub avg_drift_ns_per_s: u64,
    /// Maximum jitter observed (ns)
    pub max_jitter_ns: u64,
    /// Average jitter (ns)
    pub avg_jitter_ns: u64,
    /// Number of measurements within tolerance
    pub within_tolerance: u64,
}

/// Output comparator for replay validation
pub struct OutputComparator {
    config: ComparisonConfig,
    axis_diffs: Vec<f32>,
    ffb_diffs: Vec<f32>,
    timing_measurements: Vec<TimingMeasurement>,
    errors: Vec<String>,
}

/// Single timing measurement
#[derive(Debug, Clone)]
struct TimingMeasurement {
    expected_ns: u64,
    actual_ns: u64,
    drift_ns_per_s: u64,
    jitter_ns: u64,
}

impl OutputComparator {
    /// Create a new output comparator
    pub fn new(config: ComparisonConfig) -> Self {
        Self {
            config,
            axis_diffs: Vec::new(),
            ffb_diffs: Vec::new(),
            timing_measurements: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Compare axis outputs between expected and actual
    pub fn compare_axis_outputs(
        &mut self,
        expected: &HashMap<String, f32>,
        actual: &HashMap<String, f32>,
    ) -> Result<(), ComparisonError> {
        for (device_id, &expected_value) in expected {
            let actual_value =
                actual
                    .get(device_id)
                    .ok_or_else(|| ComparisonError::MissingDevice {
                        device_id: device_id.clone(),
                    })?;

            let diff = (expected_value - actual_value).abs();

            if self.config.collect_stats {
                self.axis_diffs.push(diff);
            }

            if diff > self.config.tolerance.axis_epsilon {
                let error = ComparisonError::AxisMismatch {
                    expected: expected_value,
                    actual: *actual_value,
                    diff,
                    epsilon: self.config.tolerance.axis_epsilon,
                };

                if self.config.collect_stats {
                    self.errors.push(error.to_string());
                }

                if self.config.fail_fast {
                    return Err(error);
                }
            }
        }

        Ok(())
    }

    /// Compare FFB outputs between expected and actual
    pub fn compare_ffb_outputs(
        &mut self,
        expected: &HashMap<String, f32>,
        actual: &HashMap<String, f32>,
    ) -> Result<(), ComparisonError> {
        for (device_id, &expected_value) in expected {
            let actual_value =
                actual
                    .get(device_id)
                    .ok_or_else(|| ComparisonError::MissingDevice {
                        device_id: device_id.clone(),
                    })?;

            let diff = (expected_value - actual_value).abs();

            if self.config.collect_stats {
                self.ffb_diffs.push(diff);
            }

            if diff > self.config.tolerance.ffb_epsilon {
                let error = ComparisonError::FfbMismatch {
                    expected: expected_value,
                    actual: *actual_value,
                    diff,
                    epsilon: self.config.tolerance.ffb_epsilon,
                };

                if self.config.collect_stats {
                    self.errors.push(error.to_string());
                }

                if self.config.fail_fast {
                    return Err(error);
                }
            }
        }

        Ok(())
    }

    /// Compare timing between expected and actual
    pub fn compare_timing(
        &mut self,
        expected_ns: u64,
        actual_ns: u64,
        duration_s: f64,
    ) -> Result<(), ComparisonError> {
        let time_diff = if actual_ns > expected_ns {
            actual_ns - expected_ns
        } else {
            expected_ns - actual_ns
        };

        // Calculate drift rate (ns per second)
        let drift_ns_per_s = if duration_s > 0.0 {
            (time_diff as f64 / duration_s) as u64
        } else {
            0
        };

        // Jitter is the absolute time difference
        let jitter_ns = time_diff;

        let measurement = TimingMeasurement {
            expected_ns,
            actual_ns,
            drift_ns_per_s,
            jitter_ns,
        };

        if self.config.collect_stats {
            self.timing_measurements.push(measurement);
        }

        // Check drift tolerance
        if drift_ns_per_s > self.config.tolerance.timing_drift_ns_per_s {
            let error = ComparisonError::TimingDrift {
                drift_ns: drift_ns_per_s,
                limit_ns: self.config.tolerance.timing_drift_ns_per_s,
            };

            if self.config.collect_stats {
                self.errors.push(error.to_string());
            }

            if self.config.fail_fast {
                return Err(error);
            }
        }

        // Check jitter tolerance
        if jitter_ns > self.config.tolerance.max_timing_jitter_ns {
            let error = ComparisonError::TimingJitter {
                jitter_ns,
                limit_ns: self.config.tolerance.max_timing_jitter_ns,
            };

            if self.config.collect_stats {
                self.errors.push(error.to_string());
            }

            if self.config.fail_fast {
                return Err(error);
            }
        }

        Ok(())
    }

    /// Finalize comparison and get results
    pub fn finalize(self) -> ComparisonResult {
        let axis_stats = self.calculate_axis_stats();
        let ffb_stats = self.calculate_ffb_stats();
        let timing_stats = self.calculate_timing_stats();

        let total_comparisons = axis_stats.count + ffb_stats.count + timing_stats.count;
        let mismatches = (axis_stats.count - axis_stats.within_tolerance)
            + (ffb_stats.count - ffb_stats.within_tolerance)
            + (timing_stats.count - timing_stats.within_tolerance);

        ComparisonResult {
            passed: mismatches == 0,
            total_comparisons,
            mismatches,
            axis_stats,
            ffb_stats,
            timing_stats,
            errors: self.errors,
        }
    }

    /// Calculate statistics for axis comparisons
    fn calculate_axis_stats(&self) -> ComparisonStats {
        if self.axis_diffs.is_empty() {
            return ComparisonStats::default();
        }

        let count = self.axis_diffs.len() as u64;
        let max_diff = self.axis_diffs.iter().fold(0.0f32, |a, &b| a.max(b));
        let sum_diff: f32 = self.axis_diffs.iter().sum();
        let avg_diff = sum_diff / count as f32;

        let sum_squared: f32 = self.axis_diffs.iter().map(|&x| x * x).sum();
        let rms_diff = (sum_squared / count as f32).sqrt();

        let within_tolerance = self
            .axis_diffs
            .iter()
            .filter(|&&diff| diff <= self.config.tolerance.axis_epsilon)
            .count() as u64;

        ComparisonStats {
            count,
            max_diff,
            avg_diff,
            rms_diff,
            within_tolerance,
        }
    }

    /// Calculate statistics for FFB comparisons
    fn calculate_ffb_stats(&self) -> ComparisonStats {
        if self.ffb_diffs.is_empty() {
            return ComparisonStats::default();
        }

        let count = self.ffb_diffs.len() as u64;
        let max_diff = self.ffb_diffs.iter().fold(0.0f32, |a, &b| a.max(b));
        let sum_diff: f32 = self.ffb_diffs.iter().sum();
        let avg_diff = sum_diff / count as f32;

        let sum_squared: f32 = self.ffb_diffs.iter().map(|&x| x * x).sum();
        let rms_diff = (sum_squared / count as f32).sqrt();

        let within_tolerance = self
            .ffb_diffs
            .iter()
            .filter(|&&diff| diff <= self.config.tolerance.ffb_epsilon)
            .count() as u64;

        ComparisonStats {
            count,
            max_diff,
            avg_diff,
            rms_diff,
            within_tolerance,
        }
    }

    /// Calculate statistics for timing comparisons
    fn calculate_timing_stats(&self) -> TimingStats {
        if self.timing_measurements.is_empty() {
            return TimingStats::default();
        }

        let count = self.timing_measurements.len() as u64;

        let max_drift_ns_per_s = self
            .timing_measurements
            .iter()
            .map(|m| m.drift_ns_per_s)
            .max()
            .unwrap_or(0);

        let avg_drift_ns_per_s = self
            .timing_measurements
            .iter()
            .map(|m| m.drift_ns_per_s)
            .sum::<u64>()
            / count;

        let max_jitter_ns = self
            .timing_measurements
            .iter()
            .map(|m| m.jitter_ns)
            .max()
            .unwrap_or(0);

        let avg_jitter_ns = self
            .timing_measurements
            .iter()
            .map(|m| m.jitter_ns)
            .sum::<u64>()
            / count;

        let within_tolerance = self
            .timing_measurements
            .iter()
            .filter(|m| {
                m.drift_ns_per_s <= self.config.tolerance.timing_drift_ns_per_s
                    && m.jitter_ns <= self.config.tolerance.max_timing_jitter_ns
            })
            .count() as u64;

        TimingStats {
            count,
            max_drift_ns_per_s,
            avg_drift_ns_per_s,
            max_jitter_ns,
            avg_jitter_ns,
            within_tolerance,
        }
    }
}

impl Default for ComparisonConfig {
    fn default() -> Self {
        Self {
            tolerance: ToleranceConfig::default(),
            collect_stats: true,
            fail_fast: false,
        }
    }
}

impl Default for ComparisonStats {
    fn default() -> Self {
        Self {
            count: 0,
            max_diff: 0.0,
            avg_diff: 0.0,
            rms_diff: 0.0,
            within_tolerance: 0,
        }
    }
}

impl Default for TimingStats {
    fn default() -> Self {
        Self {
            count: 0,
            max_drift_ns_per_s: 0,
            avg_drift_ns_per_s: 0,
            max_jitter_ns: 0,
            avg_jitter_ns: 0,
            within_tolerance: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_axis_comparison_within_tolerance() {
        let config = ComparisonConfig::default();
        let mut comparator = OutputComparator::new(config);

        let mut expected = HashMap::new();
        expected.insert("device1".to_string(), 0.5);

        let mut actual = HashMap::new();
        actual.insert("device1".to_string(), 0.5000005); // Within 1e-6 tolerance

        assert!(comparator.compare_axis_outputs(&expected, &actual).is_ok());

        let result = comparator.finalize();
        assert!(result.passed);
        assert_eq!(result.axis_stats.within_tolerance, 1);
    }

    #[test]
    fn test_axis_comparison_exceeds_tolerance() {
        let config = ComparisonConfig {
            tolerance: ToleranceConfig::default(),
            collect_stats: true,
            fail_fast: true,
        };
        let mut comparator = OutputComparator::new(config);

        let mut expected = HashMap::new();
        expected.insert("device1".to_string(), 0.5);

        let mut actual = HashMap::new();
        actual.insert("device1".to_string(), 0.6); // Exceeds 1e-6 tolerance

        assert!(comparator.compare_axis_outputs(&expected, &actual).is_err());
    }

    #[test]
    fn test_ffb_comparison_within_tolerance() {
        let config = ComparisonConfig::default();
        let mut comparator = OutputComparator::new(config);

        let mut expected = HashMap::new();
        expected.insert("device1".to_string(), 5.0);

        let mut actual = HashMap::new();
        actual.insert("device1".to_string(), 5.00005); // Within 1e-4 tolerance

        assert!(comparator.compare_ffb_outputs(&expected, &actual).is_ok());

        let result = comparator.finalize();
        assert!(result.passed);
        assert_eq!(result.ffb_stats.within_tolerance, 1);
    }

    #[test]
    fn test_timing_comparison_within_tolerance() {
        let config = ComparisonConfig::default();
        let mut comparator = OutputComparator::new(config);

        // 50μs difference over 1 second = 50,000 ns/s drift (within 100,000 ns/s limit)
        assert!(
            comparator
                .compare_timing(1000000000, 1000050000, 1.0)
                .is_ok()
        );

        let result = comparator.finalize();
        assert!(result.passed);
        assert_eq!(result.timing_stats.within_tolerance, 1);
    }

    #[test]
    fn test_timing_comparison_exceeds_drift_tolerance() {
        let config = ComparisonConfig {
            tolerance: ToleranceConfig::default(),
            collect_stats: true,
            fail_fast: true,
        };
        let mut comparator = OutputComparator::new(config);

        // 200μs difference over 1 second = 200,000 ns/s drift (exceeds 100,000 ns/s limit)
        assert!(
            comparator
                .compare_timing(1000000000, 1000200000, 1.0)
                .is_err()
        );
    }

    #[test]
    fn test_missing_device_error() {
        let config = ComparisonConfig::default();
        let mut comparator = OutputComparator::new(config);

        let mut expected = HashMap::new();
        expected.insert("device1".to_string(), 0.5);

        let actual = HashMap::new(); // Missing device1

        let result = comparator.compare_axis_outputs(&expected, &actual);
        assert!(result.is_err());

        if let Err(ComparisonError::MissingDevice { device_id }) = result {
            assert_eq!(device_id, "device1");
        } else {
            panic!("Expected MissingDevice error");
        }
    }

    #[test]
    fn test_statistics_calculation() {
        let config = ComparisonConfig::default();
        let mut comparator = OutputComparator::new(config);

        // Add multiple comparisons to test statistics
        let mut expected = HashMap::new();
        let mut actual = HashMap::new();

        for i in 0..10 {
            expected.insert(format!("device{}", i), i as f32);
            actual.insert(format!("device{}", i), i as f32 + 0.000001); // Small difference
            comparator.compare_axis_outputs(&expected, &actual).unwrap();
            expected.clear();
            actual.clear();
        }

        let result = comparator.finalize();
        assert!(result.passed);
        assert_eq!(result.axis_stats.count, 10);
        assert!(result.axis_stats.avg_diff > 0.0);
        assert!(result.axis_stats.max_diff >= result.axis_stats.avg_diff);
    }
}
