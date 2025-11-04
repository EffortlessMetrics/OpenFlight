// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Jitter measurement and timing statistics
//!
//! Provides comprehensive timing analysis for the real-time scheduler
//! including jitter calculation and percentile tracking.

use std::time::Instant;
use std::collections::VecDeque;
use parking_lot::Mutex;

/// Jitter measurement configuration
const JITTER_WINDOW_SIZE: usize = 15000; // 1 minute at 250Hz
const WARMUP_TICKS: usize = 1250; // 5 seconds at 250Hz

/// Jitter statistics
#[derive(Debug, Clone)]
pub struct JitterStats {
    /// Mean jitter in nanoseconds
    pub mean_ns: f64,
    /// Standard deviation in nanoseconds
    pub std_dev_ns: f64,
    /// 50th percentile (median) jitter in nanoseconds
    pub p50_ns: i64,
    /// 99th percentile jitter in nanoseconds
    pub p99_ns: i64,
    /// Maximum jitter observed in nanoseconds
    pub max_ns: i64,
    /// Number of samples in statistics
    pub sample_count: usize,
}

/// Complete timing statistics
#[derive(Debug, Clone)]
pub struct TimingStats {
    /// Total number of ticks processed
    pub total_ticks: u64,
    /// Number of missed ticks (>1.5x period late)
    pub missed_ticks: u64,
    /// Miss rate as fraction (0.0 to 1.0)
    pub miss_rate: f64,
    /// Jitter statistics (if enabled)
    pub jitter_stats: Option<JitterStats>,
}

/// Jitter measurement system
pub struct JitterMetrics {
    #[allow(dead_code)]
    frequency_hz: u32,
    expected_period_ns: u64,
    last_tick: Option<Instant>,
    intervals: Mutex<VecDeque<i64>>,
    tick_count: usize,
}

impl JitterMetrics {
    /// Create new jitter metrics
    pub fn new(frequency_hz: u32) -> Self {
        let expected_period_ns = 1_000_000_000 / frequency_hz as u64;
        
        Self {
            frequency_hz,
            expected_period_ns,
            last_tick: None,
            intervals: Mutex::new(VecDeque::with_capacity(JITTER_WINDOW_SIZE)),
            tick_count: 0,
        }
    }

    /// Record a tick for jitter measurement
    pub fn record_tick(&mut self, timestamp: Instant, _error_ns: i64) {
        self.tick_count += 1;
        
        if let Some(last) = self.last_tick {
            let interval = timestamp.duration_since(last).as_nanos() as i64;
            let jitter = interval - self.expected_period_ns as i64;
            
            // Skip warmup period
            if self.tick_count > WARMUP_TICKS {
                let mut intervals = self.intervals.lock();
                
                if intervals.len() >= JITTER_WINDOW_SIZE {
                    intervals.pop_front();
                }
                intervals.push_back(jitter);
            }
        }
        
        self.last_tick = Some(timestamp);
    }

    /// Get current jitter statistics
    pub fn get_stats(&self) -> JitterStats {
        let intervals = self.intervals.lock();
        
        if intervals.is_empty() {
            return JitterStats {
                mean_ns: 0.0,
                std_dev_ns: 0.0,
                p50_ns: 0,
                p99_ns: 0,
                max_ns: 0,
                sample_count: 0,
            };
        }

        let mut sorted: Vec<i64> = intervals.iter().cloned().collect();
        sorted.sort_unstable();
        
        let count = sorted.len();
        let sum: i64 = sorted.iter().sum();
        let mean = sum as f64 / count as f64;
        
        let variance = sorted.iter()
            .map(|&x| {
                let diff = x as f64 - mean;
                diff * diff
            })
            .sum::<f64>() / count as f64;
        
        let std_dev = variance.sqrt();
        
        let p50_idx = count / 2;
        let p99_idx = (count * 99) / 100;
        
        let p50 = sorted[p50_idx];
        let p99 = sorted[p99_idx.min(count - 1)];
        let max_jitter = *sorted.last().unwrap();

        JitterStats {
            mean_ns: mean,
            std_dev_ns: std_dev,
            p50_ns: p50,
            p99_ns: p99,
            max_ns: max_jitter,
            sample_count: count,
        }
    }

    /// Reset all measurements
    pub fn reset(&mut self) {
        self.intervals.lock().clear();
        self.last_tick = None;
        self.tick_count = 0;
    }

    /// Check if jitter exceeds quality gate (0.5ms p99)
    pub fn exceeds_quality_gate(&self) -> bool {
        let stats = self.get_stats();
        if stats.sample_count < 1000 {
            return false; // Not enough samples
        }
        
        const QUALITY_GATE_NS: i64 = 500_000; // 0.5ms
        stats.p99_ns.abs() > QUALITY_GATE_NS
    }
}

/// Timing validation for long-running tests
pub struct TimingValidator {
    start_time: Instant,
    target_duration: std::time::Duration,
    metrics: JitterMetrics,
}

impl TimingValidator {
    /// Create validator for long-running timing test
    pub fn new(frequency_hz: u32, target_duration: std::time::Duration) -> Self {
        Self {
            start_time: Instant::now(),
            target_duration,
            metrics: JitterMetrics::new(frequency_hz),
        }
    }

    /// Record tick and check if test should continue
    pub fn record_and_check(&mut self, timestamp: Instant) -> bool {
        self.metrics.record_tick(timestamp, 0);
        
        let elapsed = timestamp.duration_since(self.start_time);
        elapsed < self.target_duration
    }

    /// Get final validation results
    pub fn finalize(self) -> ValidationResult {
        let stats = self.metrics.get_stats();
        let elapsed = self.start_time.elapsed();
        
        ValidationResult {
            duration: elapsed,
            jitter_stats: stats,
            passed_quality_gate: !self.metrics.exceeds_quality_gate(),
        }
    }
}

/// Results of timing validation
#[derive(Debug)]
pub struct ValidationResult {
    /// Actual test duration
    pub duration: std::time::Duration,
    /// Jitter statistics
    pub jitter_stats: JitterStats,
    /// Whether the test passed quality gates
    pub passed_quality_gate: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_jitter_calculation() {
        let mut metrics = JitterMetrics::new(250);
        let start = Instant::now();
        
        // Simulate perfect timing for warmup
        for i in 0..WARMUP_TICKS + 100 {
            let tick_time = start + Duration::from_nanos(i as u64 * 4_000_000); // 4ms intervals
            metrics.record_tick(tick_time, 0);
        }
        
        let stats = metrics.get_stats();
        
        // Should have recorded samples after warmup
        assert!(stats.sample_count > 0);
        // Perfect timing should have low jitter
        assert!(stats.p99_ns.abs() < 100_000); // <100μs
    }

    #[test]
    fn test_quality_gate() {
        let mut metrics = JitterMetrics::new(250);
        let start = Instant::now();
        
        // Simulate high jitter
        for i in 0..WARMUP_TICKS + 2000 {
            let jitter = if i % 10 == 0 { 1_000_000 } else { 0 }; // 1ms jitter every 10th tick
            let tick_time = start + Duration::from_nanos(i as u64 * 4_000_000 + jitter as u64);
            metrics.record_tick(tick_time, 0);
        }
        
        // Should exceed quality gate
        assert!(metrics.exceeds_quality_gate());
    }

    #[test]
    fn test_timing_validator() {
        let mut validator = TimingValidator::new(250, Duration::from_millis(100));
        let start = Instant::now();
        
        let mut tick_count = 0;
        while validator.record_and_check(start + Duration::from_nanos(tick_count * 4_000_000)) {
            tick_count += 1;
            if tick_count > 1000 { break; } // Safety limit
        }
        
        let result = validator.finalize();
        assert!(result.duration >= Duration::from_millis(100));
    }
}