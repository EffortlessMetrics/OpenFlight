// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Performance gate implementation for CI testing
//!
//! Provides automated performance testing that fails builds
//! when timing regressions are detected.

use std::time::{Duration, Instant};
use flight_scheduler::{Scheduler, SchedulerConfig};
use flight_scheduler::metrics::TimingValidator;
use crate::loopback::{LoopbackHid, HidReport};

/// Performance gate configuration
#[derive(Debug, Clone)]
pub struct PerfGateConfig {
    /// Target frequency for timing test
    pub frequency_hz: u32,
    /// Test duration
    pub duration: Duration,
    /// Maximum allowed jitter p99 (nanoseconds)
    pub max_jitter_p99_ns: i64,
    /// Maximum allowed HID write latency p99 (microseconds)
    pub max_hid_latency_p99_us: u64,
    /// Maximum allowed miss rate (fraction)
    pub max_miss_rate: f64,
    /// Number of HID write samples for latency test
    pub hid_samples: usize,
}

impl Default for PerfGateConfig {
    fn default() -> Self {
        Self {
            frequency_hz: 250,
            duration: Duration::from_secs(60), // 1 minute test
            max_jitter_p99_ns: 500_000,        // 0.5ms
            max_hid_latency_p99_us: 300,       // 300μs
            max_miss_rate: 0.001,              // 0.1%
            hid_samples: 1000,
        }
    }
}

/// Performance test results
#[derive(Debug, Clone)]
pub struct PerfResult {
    /// Whether all tests passed
    pub passed: bool,
    /// Timing test results
    pub timing_result: TimingTestResult,
    /// HID latency test results
    pub hid_result: HidLatencyResult,
    /// Overall test duration
    pub total_duration: Duration,
}

/// Timing test results
#[derive(Debug, Clone)]
pub struct TimingTestResult {
    /// Whether timing test passed
    pub passed: bool,
    /// Total ticks processed
    pub total_ticks: u64,
    /// Number of missed ticks
    pub missed_ticks: u64,
    /// Miss rate (fraction)
    pub miss_rate: f64,
    /// Jitter p99 (nanoseconds)
    pub jitter_p99_ns: i64,
    /// Test duration
    pub duration: Duration,
}

/// HID latency test results
#[derive(Debug, Clone)]
pub struct HidLatencyResult {
    /// Whether HID test passed
    pub passed: bool,
    /// Number of samples
    pub samples: usize,
    /// Average latency (microseconds)
    pub avg_latency_us: f64,
    /// p99 latency (microseconds)
    pub p99_latency_us: u64,
    /// Maximum latency (microseconds)
    pub max_latency_us: u64,
}

/// Performance gate runner
pub struct PerfGate {
    config: PerfGateConfig,
}

impl PerfGate {
    /// Create new performance gate
    pub fn new(config: PerfGateConfig) -> Self {
        Self { config }
    }

    /// Run complete performance gate test
    pub fn run(&mut self) -> PerfResult {
        let start_time = Instant::now();
        
        println!("Running performance gate tests...");
        println!("  Target frequency: {}Hz", self.config.frequency_hz);
        println!("  Test duration: {:?}", self.config.duration);
        
        // Run timing discipline test
        let timing_result = self.run_timing_test();
        
        // Run HID latency test
        let hid_result = self.run_hid_latency_test();
        
        let total_duration = start_time.elapsed();
        let passed = timing_result.passed && hid_result.passed;
        
        // Print results
        self.print_results(&timing_result, &hid_result, passed);
        
        PerfResult {
            passed,
            timing_result,
            hid_result,
            total_duration,
        }
    }

    fn run_timing_test(&self) -> TimingTestResult {
        println!("\n=== Timing Discipline Test ===");
        
        let scheduler_config = SchedulerConfig {
            frequency_hz: self.config.frequency_hz,
            busy_spin_us: 65,
            pll_gain: 0.001,
            measure_jitter: true,
        };
        
        let mut scheduler = Scheduler::new(scheduler_config);
        let mut validator = TimingValidator::new(self.config.frequency_hz, self.config.duration);
        
        let start = Instant::now();
        let mut tick_count = 0u64;
        
        // Run scheduler for specified duration
        loop {
            let result = scheduler.wait_for_tick();
            tick_count += 1;
            
            if !validator.record_and_check(result.timestamp) {
                break;
            }
            
            // Progress reporting every 10 seconds
            if tick_count % (self.config.frequency_hz as u64 * 10) == 0 {
                let elapsed = start.elapsed().as_secs();
                let target = self.config.duration.as_secs();
                println!("  Progress: {}s / {}s", elapsed, target);
            }
        }
        
        let final_stats = scheduler.get_stats();
        let validation_result = validator.finalize();
        
        let jitter_p99 = validation_result.jitter_stats.p99_ns;
        let miss_rate = final_stats.miss_rate;
        
        let passed = jitter_p99.abs() <= self.config.max_jitter_p99_ns 
                    && miss_rate <= self.config.max_miss_rate;
        
        TimingTestResult {
            passed,
            total_ticks: final_stats.total_ticks,
            missed_ticks: final_stats.missed_ticks,
            miss_rate,
            jitter_p99_ns: jitter_p99,
            duration: validation_result.duration,
        }
    }

    fn run_hid_latency_test(&self) -> HidLatencyResult {
        println!("\n=== HID Latency Test ===");
        
        let loopback = LoopbackHid::with_config(1024, Duration::from_micros(10));
        
        println!("  Testing {} HID writes...", self.config.hid_samples);
        
        let latencies = loopback.test_write_latency(self.config.hid_samples);
        
        // Calculate statistics
        let mut latencies_us: Vec<u64> = latencies.iter()
            .map(|d| d.as_micros() as u64)
            .collect();
        
        latencies_us.sort_unstable();
        
        let avg_latency_us = latencies_us.iter().sum::<u64>() as f64 / latencies_us.len() as f64;
        let p99_idx = (latencies_us.len() * 99) / 100;
        let p99_latency_us = latencies_us[p99_idx.min(latencies_us.len() - 1)];
        let max_latency_us = *latencies_us.last().unwrap();
        
        let passed = p99_latency_us <= self.config.max_hid_latency_p99_us;
        
        HidLatencyResult {
            passed,
            samples: latencies_us.len(),
            avg_latency_us,
            p99_latency_us,
            max_latency_us,
        }
    }

    fn print_results(&self, timing: &TimingTestResult, hid: &HidLatencyResult, overall_passed: bool) {
        println!("\n=== Performance Gate Results ===");
        
        // Timing results
        println!("Timing Test:");
        println!("  Status: {}", if timing.passed { "PASS" } else { "FAIL" });
        println!("  Total ticks: {}", timing.total_ticks);
        println!("  Missed ticks: {}", timing.missed_ticks);
        println!("  Miss rate: {:.6}% (limit: {:.3}%)", 
                timing.miss_rate * 100.0, self.config.max_miss_rate * 100.0);
        println!("  Jitter p99: {}μs (limit: {}μs)", 
                timing.jitter_p99_ns / 1000, self.config.max_jitter_p99_ns / 1000);
        println!("  Duration: {:?}", timing.duration);
        
        // HID results
        println!("\nHID Latency Test:");
        println!("  Status: {}", if hid.passed { "PASS" } else { "FAIL" });
        println!("  Samples: {}", hid.samples);
        println!("  Average: {:.1}μs", hid.avg_latency_us);
        println!("  p99: {}μs (limit: {}μs)", hid.p99_latency_us, self.config.max_hid_latency_p99_us);
        println!("  Max: {}μs", hid.max_latency_us);
        
        // Overall result
        println!("\nOverall Result: {}", if overall_passed { "PASS" } else { "FAIL" });
        
        if !overall_passed {
            println!("\n❌ Performance gate FAILED - build should be rejected");
            std::process::exit(1);
        } else {
            println!("\n✅ Performance gate PASSED");
        }
    }
}

/// Quick performance check for CI
pub fn quick_perf_check() -> bool {
    let config = PerfGateConfig {
        duration: Duration::from_secs(10), // Quick 10-second test
        hid_samples: 100,
        ..Default::default()
    };
    
    let mut gate = PerfGate::new(config);
    let result = gate.run();
    result.passed
}

/// Full performance validation for nightly builds
pub fn full_perf_validation() -> PerfResult {
    let config = PerfGateConfig {
        duration: Duration::from_secs(600), // 10-minute test
        hid_samples: 10000,
        ..Default::default()
    };
    
    let mut gate = PerfGate::new(config);
    gate.run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perf_gate_config() {
        let config = PerfGateConfig::default();
        assert_eq!(config.frequency_hz, 250);
        assert_eq!(config.max_jitter_p99_ns, 500_000);
    }

    #[test]
    fn test_quick_perf_check() {
        // This test may be flaky on heavily loaded systems
        // In real CI, we'd run this on dedicated hardware
        let config = PerfGateConfig {
            duration: Duration::from_millis(100), // Very quick test
            hid_samples: 10,
            max_jitter_p99_ns: 5_000_000, // Lenient for test
            max_hid_latency_p99_us: 10_000, // Lenient for test
            ..Default::default()
        };
        
        let mut gate = PerfGate::new(config);
        let result = gate.run();
        
        // Should complete without crashing
        assert!(result.timing_result.total_ticks > 0);
        assert!(result.hid_result.samples > 0);
    }

    #[test]
    fn test_hid_latency_calculation() {
        let config = PerfGateConfig {
            hid_samples: 100,
            max_hid_latency_p99_us: 1000,
            ..Default::default()
        };
        
        let gate = PerfGate::new(config);
        let result = gate.run_hid_latency_test();
        
        assert_eq!(result.samples, 100);
        assert!(result.avg_latency_us > 0.0);
        assert!(result.p99_latency_us > 0);
    }
}