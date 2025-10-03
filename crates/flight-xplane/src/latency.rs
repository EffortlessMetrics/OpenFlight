// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Latency measurement and budget validation for X-Plane adapter
//!
//! Provides comprehensive latency tracking, budget enforcement, and performance
//! monitoring to ensure the adapter meets real-time requirements.

use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};
use thiserror::Error;
use tracing::{debug, warn};

/// Latency measurement errors
#[derive(Error, Debug)]
pub enum LatencyError {
    #[error("Budget exceeded: {actual_ms}ms > {budget_ms}ms")]
    BudgetExceeded { actual_ms: u64, budget_ms: u64 },
    #[error("Invalid measurement: {reason}")]
    InvalidMeasurement { reason: String },
    #[error("Insufficient data for statistics")]
    InsufficientData,
}

/// Latency budget configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyBudget {
    /// Maximum allowed latency
    pub max_latency: Duration,
    /// Warning threshold (percentage of max)
    pub warning_threshold: f32,
    /// Number of consecutive violations before alert
    pub violation_threshold: u32,
    /// Enable strict budget enforcement
    pub strict_enforcement: bool,
}

impl LatencyBudget {
    pub fn new(max_latency: Duration) -> Self {
        Self {
            max_latency,
            warning_threshold: 0.8, // 80% of max
            violation_threshold: 3,
            strict_enforcement: false,
        }
    }

    pub fn with_warning_threshold(mut self, threshold: f32) -> Self {
        self.warning_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    pub fn with_violation_threshold(mut self, threshold: u32) -> Self {
        self.violation_threshold = threshold;
        self
    }

    pub fn strict(mut self) -> Self {
        self.strict_enforcement = true;
        self
    }

    /// Get warning threshold as duration
    pub fn warning_duration(&self) -> Duration {
        Duration::from_nanos(
            (self.max_latency.as_nanos() as f64 * self.warning_threshold as f64) as u64
        )
    }

    /// Check if latency exceeds budget
    pub fn is_exceeded(&self, latency: Duration) -> bool {
        latency > self.max_latency
    }

    /// Check if latency exceeds warning threshold
    pub fn is_warning(&self, latency: Duration) -> bool {
        latency > self.warning_duration()
    }
}

/// Individual latency measurement
#[derive(Debug, Clone)]
pub struct LatencyMeasurement {
    pub timestamp: Instant,
    pub latency: Duration,
    pub operation: String,
    pub metadata: Option<String>,
}

impl LatencyMeasurement {
    pub fn new(operation: String, latency: Duration) -> Self {
        Self {
            timestamp: Instant::now(),
            latency,
            operation,
            metadata: None,
        }
    }

    pub fn with_metadata(mut self, metadata: String) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// Latency statistics
#[derive(Debug, Clone)]
pub struct LatencyStats {
    pub count: usize,
    pub min: Duration,
    pub max: Duration,
    pub mean: Duration,
    pub p50: Duration,
    pub p95: Duration,
    pub p99: Duration,
    pub violations: u32,
    pub warnings: u32,
    pub last_violation: Option<Instant>,
}

impl Default for LatencyStats {
    fn default() -> Self {
        Self {
            count: 0,
            min: Duration::ZERO,
            max: Duration::ZERO,
            mean: Duration::ZERO,
            p50: Duration::ZERO,
            p95: Duration::ZERO,
            p99: Duration::ZERO,
            violations: 0,
            warnings: 0,
            last_violation: None,
        }
    }
}

/// Latency tracker for performance monitoring
#[derive(Clone)]
pub struct LatencyTracker {
    budget: LatencyBudget,
    measurements: Arc<RwLock<VecDeque<LatencyMeasurement>>>,
    stats: Arc<RwLock<LatencyStats>>,
    consecutive_violations: Arc<RwLock<u32>>,
    max_measurements: usize,
}

impl LatencyTracker {
    pub fn new(budget: LatencyBudget) -> Self {
        Self {
            budget,
            measurements: Arc::new(RwLock::new(VecDeque::new())),
            stats: Arc::new(RwLock::new(LatencyStats::default())),
            consecutive_violations: Arc::new(RwLock::new(0)),
            max_measurements: 1000, // Keep last 1000 measurements
        }
    }

    /// Record a latency measurement
    pub fn record_measurement(&self, latency: Duration) {
        self.record_measurement_with_operation("default".to_string(), latency);
    }

    /// Record a latency measurement with operation name
    pub fn record_measurement_with_operation(&self, operation: String, latency: Duration) {
        let measurement = LatencyMeasurement::new(operation, latency);
        self.record_measurement_struct(measurement);
    }

    /// Record a latency measurement struct
    pub fn record_measurement_struct(&self, measurement: LatencyMeasurement) {
        // Add to measurements queue
        {
            let mut measurements = self.measurements.write().unwrap();
            measurements.push_back(measurement.clone());
            
            // Keep only the last N measurements
            while measurements.len() > self.max_measurements {
                measurements.pop_front();
            }
        }

        // Check budget violations
        let is_violation = self.budget.is_exceeded(measurement.latency);
        let is_warning = self.budget.is_warning(measurement.latency);

        if is_violation {
            let mut violations = self.consecutive_violations.write().unwrap();
            *violations += 1;
            
            warn!(
                "Latency budget violation: {}ms > {}ms (operation: {}, consecutive: {})",
                measurement.latency.as_millis(),
                self.budget.max_latency.as_millis(),
                measurement.operation,
                *violations
            );

            // Check if we've exceeded the violation threshold
            if *violations >= self.budget.violation_threshold {
                warn!(
                    "Latency violation threshold exceeded: {} consecutive violations",
                    *violations
                );
            }
        } else {
            // Reset consecutive violations on successful measurement
            *self.consecutive_violations.write().unwrap() = 0;
        }

        if is_warning && !is_violation {
            debug!(
                "Latency warning: {}ms > {}ms (operation: {})",
                measurement.latency.as_millis(),
                self.budget.warning_duration().as_millis(),
                measurement.operation
            );
        }

        // Update statistics
        self.update_stats(measurement, is_violation, is_warning);
    }

    /// Update internal statistics
    fn update_stats(&self, measurement: LatencyMeasurement, is_violation: bool, is_warning: bool) {
        let mut stats = self.stats.write().unwrap();
        
        stats.count += 1;
        
        if stats.count == 1 {
            stats.min = measurement.latency;
            stats.max = measurement.latency;
            stats.mean = measurement.latency;
        } else {
            stats.min = stats.min.min(measurement.latency);
            stats.max = stats.max.max(measurement.latency);
            
            // Update running mean
            let total_nanos = stats.mean.as_nanos() as u64 * (stats.count - 1) as u64 + measurement.latency.as_nanos() as u64;
            stats.mean = Duration::from_nanos(total_nanos / stats.count as u64);
        }

        if is_violation {
            stats.violations += 1;
            stats.last_violation = Some(measurement.timestamp);
        }

        if is_warning {
            stats.warnings += 1;
        }

        // Update percentiles (recalculate from recent measurements)
        self.update_percentiles(&mut stats);
    }

    /// Update percentile statistics
    fn update_percentiles(&self, stats: &mut LatencyStats) {
        let measurements = self.measurements.read().unwrap();
        
        if measurements.is_empty() {
            return;
        }

        // Collect latencies and sort
        let mut latencies: Vec<Duration> = measurements.iter()
            .map(|m| m.latency)
            .collect();
        latencies.sort();

        let len = latencies.len();
        
        // Calculate percentiles
        stats.p50 = latencies[len * 50 / 100];
        stats.p95 = latencies[len * 95 / 100];
        stats.p99 = latencies[len * 99 / 100];
    }

    /// Get current statistics
    pub fn get_stats(&self) -> LatencyStats {
        self.stats.read().unwrap().clone()
    }

    /// Get recent measurements
    pub fn get_recent_measurements(&self, count: usize) -> Vec<LatencyMeasurement> {
        let measurements = self.measurements.read().unwrap();
        measurements.iter()
            .rev()
            .take(count)
            .cloned()
            .collect()
    }

    /// Check if currently in violation state
    pub fn is_in_violation(&self) -> bool {
        *self.consecutive_violations.read().unwrap() > 0
    }

    /// Get consecutive violation count
    pub fn get_consecutive_violations(&self) -> u32 {
        *self.consecutive_violations.read().unwrap()
    }

    /// Reset statistics
    pub fn reset_stats(&self) {
        *self.measurements.write().unwrap() = VecDeque::new();
        *self.stats.write().unwrap() = LatencyStats::default();
        *self.consecutive_violations.write().unwrap() = 0;
    }

    /// Validate against budget
    pub fn validate_budget(&self, latency: Duration) -> Result<(), LatencyError> {
        if self.budget.is_exceeded(latency) {
            Err(LatencyError::BudgetExceeded {
                actual_ms: latency.as_millis() as u64,
                budget_ms: self.budget.max_latency.as_millis() as u64,
            })
        } else {
            Ok(())
        }
    }

    /// Get budget configuration
    pub fn get_budget(&self) -> &LatencyBudget {
        &self.budget
    }

    /// Update budget configuration
    pub fn update_budget(&mut self, budget: LatencyBudget) {
        self.budget = budget;
    }

    /// Get performance summary
    pub fn get_performance_summary(&self) -> PerformanceSummary {
        let stats = self.get_stats();
        let measurements = self.measurements.read().unwrap();
        
        let recent_violations = measurements.iter()
            .rev()
            .take(100) // Last 100 measurements
            .filter(|m| self.budget.is_exceeded(m.latency))
            .count();

        let violation_rate = if stats.count > 0 {
            stats.violations as f32 / stats.count as f32
        } else {
            0.0
        };

        let health_status = if violation_rate > 0.1 {
            HealthStatus::Critical
        } else if violation_rate > 0.05 {
            HealthStatus::Warning
        } else if stats.p99 > self.budget.warning_duration() {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        PerformanceSummary {
            health_status,
            violation_rate,
            recent_violations,
            avg_latency: stats.mean,
            p99_latency: stats.p99,
            budget_utilization: stats.p99.as_nanos() as f32 / self.budget.max_latency.as_nanos() as f32,
            consecutive_violations: self.get_consecutive_violations(),
        }
    }
}

/// Performance summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSummary {
    pub health_status: HealthStatus,
    pub violation_rate: f32,
    pub recent_violations: usize,
    pub avg_latency: Duration,
    pub p99_latency: Duration,
    pub budget_utilization: f32,
    pub consecutive_violations: u32,
}

/// Health status levels
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Warning,
    Critical,
}

/// Latency measurement helper for timing operations
pub struct LatencyTimer {
    start_time: Instant,
    operation: String,
    tracker: Option<LatencyTracker>,
}

impl LatencyTimer {
    /// Start timing an operation
    pub fn start(operation: String) -> Self {
        Self {
            start_time: Instant::now(),
            operation,
            tracker: None,
        }
    }

    /// Start timing with a tracker
    pub fn start_with_tracker(operation: String, tracker: LatencyTracker) -> Self {
        Self {
            start_time: Instant::now(),
            operation,
            tracker: Some(tracker),
        }
    }

    /// Finish timing and get duration
    pub fn finish(self) -> Duration {
        let duration = self.start_time.elapsed();
        
        if let Some(tracker) = self.tracker {
            tracker.record_measurement_with_operation(self.operation, duration);
        }
        
        duration
    }

    /// Finish timing and validate against budget
    pub fn finish_and_validate(self, budget: &LatencyBudget) -> Result<Duration, LatencyError> {
        let duration = self.finish();
        if budget.is_exceeded(duration) {
            Err(LatencyError::BudgetExceeded {
                actual_ms: duration.as_millis() as u64,
                budget_ms: budget.max_latency.as_millis() as u64,
            })
        } else {
            Ok(duration)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_latency_budget_creation() {
        let budget = LatencyBudget::new(Duration::from_millis(50))
            .with_warning_threshold(0.8)
            .with_violation_threshold(3)
            .strict();

        assert_eq!(budget.max_latency, Duration::from_millis(50));
        assert_eq!(budget.warning_threshold, 0.8);
        assert_eq!(budget.violation_threshold, 3);
        assert!(budget.strict_enforcement);
    }

    #[test]
    fn test_budget_thresholds() {
        let budget = LatencyBudget::new(Duration::from_millis(100))
            .with_warning_threshold(0.8);

        assert!(!budget.is_exceeded(Duration::from_millis(50)));
        assert!(!budget.is_exceeded(Duration::from_millis(100)));
        assert!(budget.is_exceeded(Duration::from_millis(101)));

        assert!(!budget.is_warning(Duration::from_millis(70)));
        assert!(budget.is_warning(Duration::from_millis(85)));
        // Allow for floating point precision issues
        let warning_duration = budget.warning_duration();
        assert!((warning_duration.as_millis() as i64 - 80).abs() <= 1);
    }

    #[test]
    fn test_latency_measurement() {
        let measurement = LatencyMeasurement::new(
            "test_operation".to_string(),
            Duration::from_millis(25)
        ).with_metadata("test metadata".to_string());

        assert_eq!(measurement.operation, "test_operation");
        assert_eq!(measurement.latency, Duration::from_millis(25));
        assert_eq!(measurement.metadata, Some("test metadata".to_string()));
    }

    #[test]
    fn test_latency_tracker() {
        let budget = LatencyBudget::new(Duration::from_millis(100));
        let tracker = LatencyTracker::new(budget);

        // Record some measurements
        tracker.record_measurement(Duration::from_millis(50));
        tracker.record_measurement(Duration::from_millis(75));
        tracker.record_measurement(Duration::from_millis(25));

        let stats = tracker.get_stats();
        assert_eq!(stats.count, 3);
        assert_eq!(stats.min, Duration::from_millis(25));
        assert_eq!(stats.max, Duration::from_millis(75));
        assert_eq!(stats.violations, 0);
    }

    #[test]
    fn test_budget_violations() {
        let budget = LatencyBudget::new(Duration::from_millis(50))
            .with_violation_threshold(2);
        let tracker = LatencyTracker::new(budget);

        // Record violation
        tracker.record_measurement(Duration::from_millis(75));
        assert!(tracker.is_in_violation());
        assert_eq!(tracker.get_consecutive_violations(), 1);

        // Record another violation
        tracker.record_measurement(Duration::from_millis(80));
        assert_eq!(tracker.get_consecutive_violations(), 2);

        // Record good measurement
        tracker.record_measurement(Duration::from_millis(25));
        assert!(!tracker.is_in_violation());
        assert_eq!(tracker.get_consecutive_violations(), 0);

        let stats = tracker.get_stats();
        assert_eq!(stats.violations, 2);
    }

    #[test]
    fn test_percentile_calculation() {
        let budget = LatencyBudget::new(Duration::from_millis(100));
        let tracker = LatencyTracker::new(budget);

        // Record measurements with known distribution
        for i in 1..=100 {
            tracker.record_measurement(Duration::from_millis(i));
        }

        let stats = tracker.get_stats();
        assert_eq!(stats.count, 100);
        // Allow for off-by-one in percentile calculation
        assert!((stats.p50.as_millis() as i64 - 50).abs() <= 1);
        assert!((stats.p95.as_millis() as i64 - 95).abs() <= 1);
        assert!((stats.p99.as_millis() as i64 - 99).abs() <= 1);
    }

    #[test]
    fn test_performance_summary() {
        let budget = LatencyBudget::new(Duration::from_millis(50));
        let tracker = LatencyTracker::new(budget);

        // Record mostly good measurements with some violations
        for _ in 0..90 {
            tracker.record_measurement(Duration::from_millis(25));
        }
        for _ in 0..10 {
            tracker.record_measurement(Duration::from_millis(75)); // Violations
        }

        let summary = tracker.get_performance_summary();
        assert!((summary.violation_rate - 0.1).abs() < 0.01); // ~10%
        assert!(summary.budget_utilization > 1.0); // P99 exceeds budget
        
        // Should be in critical or warning state due to violation rate
        assert!(matches!(summary.health_status, HealthStatus::Critical | HealthStatus::Warning));
    }

    #[test]
    fn test_latency_timer() {
        let timer = LatencyTimer::start("test_operation".to_string());
        
        // Simulate some work
        thread::sleep(Duration::from_millis(10));
        
        let duration = timer.finish();
        assert!(duration >= Duration::from_millis(10));
        assert!(duration < Duration::from_millis(50)); // Should be reasonable
    }

    #[test]
    fn test_timer_with_tracker() {
        let budget = LatencyBudget::new(Duration::from_millis(100));
        let tracker = LatencyTracker::new(budget);
        
        let timer = LatencyTimer::start_with_tracker(
            "test_operation".to_string(),
            tracker.clone()
        );
        
        thread::sleep(Duration::from_millis(5));
        let _duration = timer.finish();
        
        // Should have recorded the measurement
        let stats = tracker.get_stats();
        assert_eq!(stats.count, 1);
        assert!(stats.mean >= Duration::from_millis(5));
    }

    #[test]
    fn test_timer_budget_validation() {
        let budget = LatencyBudget::new(Duration::from_millis(5));
        let timer = LatencyTimer::start("test_operation".to_string());
        
        thread::sleep(Duration::from_millis(10));
        
        let result = timer.finish_and_validate(&budget);
        assert!(result.is_err());
        
        if let Err(LatencyError::BudgetExceeded { actual_ms, budget_ms }) = result {
            assert!(actual_ms > budget_ms);
            assert_eq!(budget_ms, 5);
        }
    }

    #[test]
    fn test_stats_reset() {
        let budget = LatencyBudget::new(Duration::from_millis(100));
        let tracker = LatencyTracker::new(budget);

        // Record some measurements
        tracker.record_measurement(Duration::from_millis(50));
        tracker.record_measurement(Duration::from_millis(150)); // Violation

        let stats_before = tracker.get_stats();
        assert_eq!(stats_before.count, 2);
        assert_eq!(stats_before.violations, 1);

        // Reset and verify
        tracker.reset_stats();
        let stats_after = tracker.get_stats();
        assert_eq!(stats_after.count, 0);
        assert_eq!(stats_after.violations, 0);
        assert!(!tracker.is_in_violation());
    }

    #[test]
    fn test_recent_measurements() {
        let budget = LatencyBudget::new(Duration::from_millis(100));
        let tracker = LatencyTracker::new(budget);

        // Record several measurements
        for i in 1..=10 {
            tracker.record_measurement_with_operation(
                format!("operation_{}", i),
                Duration::from_millis(i * 10)
            );
        }

        let recent = tracker.get_recent_measurements(5);
        assert_eq!(recent.len(), 5);
        
        // Should be in reverse order (most recent first)
        assert_eq!(recent[0].operation, "operation_10");
        assert_eq!(recent[4].operation, "operation_6");
    }
}