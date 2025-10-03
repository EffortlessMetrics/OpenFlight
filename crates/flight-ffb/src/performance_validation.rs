// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Performance validation for FFB mode negotiation
//!
//! This module provides comprehensive performance testing to ensure that
//! FFB mode negotiation and trim operations do not introduce jitter or
//! latency regressions in the axis processing pipeline.

use std::time::{Duration, Instant};
use std::collections::VecDeque;
use crate::{FfbEngine, FfbConfig, FfbMode, DeviceCapabilities, ModeNegotiator};

/// Performance metrics for validation
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    /// Average processing time in microseconds
    pub avg_processing_time_us: f32,
    /// p99 processing time in microseconds
    pub p99_processing_time_us: f32,
    /// Maximum processing time in microseconds
    pub max_processing_time_us: f32,
    /// Jitter (standard deviation) in microseconds
    pub jitter_us: f32,
    /// Number of samples
    pub sample_count: usize,
    /// Missed deadlines (>5ms processing time)
    pub missed_deadlines: usize,
}

/// Performance validation configuration
#[derive(Debug, Clone)]
pub struct PerformanceConfig {
    /// Test duration
    pub test_duration: Duration,
    /// Target processing frequency (250Hz for axis processing)
    pub target_frequency_hz: u32,
    /// Maximum allowed p99 latency in microseconds
    pub max_p99_latency_us: f32,
    /// Maximum allowed jitter in microseconds
    pub max_jitter_us: f32,
    /// Maximum allowed missed deadlines
    pub max_missed_deadlines: usize,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            test_duration: Duration::from_secs(10),
            target_frequency_hz: 250,
            max_p99_latency_us: 5000.0, // 5ms p99 latency limit
            max_jitter_us: 500.0,       // 0.5ms jitter limit
            max_missed_deadlines: 0,    // No missed deadlines allowed
        }
    }
}

/// Performance validation result
#[derive(Debug, Clone)]
pub struct PerformanceResult {
    /// Test name
    pub name: String,
    /// Whether test passed all criteria
    pub passed: bool,
    /// Measured metrics
    pub metrics: PerformanceMetrics,
    /// Failure reasons if any
    pub failures: Vec<String>,
}

/// Performance validator for FFB operations
pub struct PerformanceValidator {
    config: PerformanceConfig,
}

impl PerformanceValidator {
    /// Create new performance validator
    pub fn new(config: PerformanceConfig) -> Self {
        Self { config }
    }

    /// Run comprehensive performance validation
    pub fn run_comprehensive_validation(&self) -> Vec<PerformanceResult> {
        let mut results = Vec::new();

        // Test baseline FFB engine performance
        results.push(self.test_baseline_ffb_performance());

        // Test mode negotiation performance impact
        results.push(self.test_mode_negotiation_performance());

        // Test trim operation performance
        results.push(self.test_trim_operation_performance());

        // Test concurrent operations performance
        results.push(self.test_concurrent_operations_performance());

        // Test different FFB modes performance
        results.push(self.test_raw_torque_mode_performance());
        results.push(self.test_directinput_mode_performance());
        results.push(self.test_telemetry_synth_mode_performance());

        results
    }

    /// Test baseline FFB engine performance without mode negotiation
    fn test_baseline_ffb_performance(&self) -> PerformanceResult {
        let config = FfbConfig {
            max_torque_nm: 15.0,
            fault_timeout_ms: 50,
            interlock_required: true,
            mode: FfbMode::DirectInput, // Fixed mode to avoid negotiation
            device_path: None,
        };

        let mut engine = FfbEngine::new(config).expect("Failed to create FFB engine");
        
        let metrics = self.measure_engine_performance(&mut engine, "Baseline FFB");
        
        let mut failures = Vec::new();
        self.validate_metrics(&metrics, &mut failures);

        PerformanceResult {
            name: "Baseline FFB Performance".to_string(),
            passed: failures.is_empty(),
            metrics,
            failures,
        }
    }

    /// Test performance impact of mode negotiation
    fn test_mode_negotiation_performance(&self) -> PerformanceResult {
        let config = FfbConfig {
            max_torque_nm: 15.0,
            fault_timeout_ms: 50,
            interlock_required: true,
            mode: FfbMode::Auto, // Auto mode triggers negotiation
            device_path: None,
        };

        let mut engine = FfbEngine::new(config).expect("Failed to create FFB engine");
        
        // Set device capabilities to trigger negotiation
        let capabilities = DeviceCapabilities {
            supports_pid: true,
            supports_raw_torque: true,
            max_torque_nm: 15.0,
            min_period_us: 1000,
            has_health_stream: true,
            supports_interlock: true,
        };

        // Measure negotiation overhead
        let negotiation_start = Instant::now();
        engine.set_device_capabilities(capabilities).expect("Failed to set capabilities");
        let negotiation_time = negotiation_start.elapsed();

        let metrics = self.measure_engine_performance(&mut engine, "Mode Negotiation");
        
        let mut failures = Vec::new();
        self.validate_metrics(&metrics, &mut failures);

        // Check negotiation time doesn't exceed reasonable bounds
        if negotiation_time > Duration::from_millis(10) {
            failures.push(format!(
                "Mode negotiation took too long: {:.2}ms > 10ms",
                negotiation_time.as_secs_f32() * 1000.0
            ));
        }

        PerformanceResult {
            name: "Mode Negotiation Performance".to_string(),
            passed: failures.is_empty(),
            metrics,
            failures,
        }
    }

    /// Test trim operation performance impact
    fn test_trim_operation_performance(&self) -> PerformanceResult {
        let config = FfbConfig {
            max_torque_nm: 15.0,
            fault_timeout_ms: 50,
            interlock_required: true,
            mode: FfbMode::DirectInput,
            device_path: None,
        };

        let mut engine = FfbEngine::new(config).expect("Failed to create FFB engine");
        
        // Start a trim operation during measurement
        let trim_controller = engine.get_trim_controller_mut();
        let change = crate::SetpointChange {
            target_nm: 5.0,
            limits: crate::TrimLimits::default(),
        };
        trim_controller.apply_setpoint_change(change).expect("Failed to apply trim change");

        let metrics = self.measure_engine_performance(&mut engine, "Trim Operation");
        
        let mut failures = Vec::new();
        self.validate_metrics(&metrics, &mut failures);

        PerformanceResult {
            name: "Trim Operation Performance".to_string(),
            passed: failures.is_empty(),
            metrics,
            failures,
        }
    }

    /// Test concurrent operations performance
    fn test_concurrent_operations_performance(&self) -> PerformanceResult {
        let config = FfbConfig {
            max_torque_nm: 15.0,
            fault_timeout_ms: 50,
            interlock_required: true,
            mode: FfbMode::Auto,
            device_path: None,
        };

        let mut engine = FfbEngine::new(config).expect("Failed to create FFB engine");
        
        // Set up device capabilities
        let capabilities = DeviceCapabilities {
            supports_pid: true,
            supports_raw_torque: true,
            max_torque_nm: 15.0,
            min_period_us: 1000,
            has_health_stream: true,
            supports_interlock: true,
        };
        engine.set_device_capabilities(capabilities).expect("Failed to set capabilities");

        // Start trim operation
        let trim_controller = engine.get_trim_controller_mut();
        let change = crate::SetpointChange {
            target_nm: 8.0,
            limits: crate::TrimLimits::default(),
        };
        trim_controller.apply_setpoint_change(change).expect("Failed to apply trim change");

        let metrics = self.measure_engine_performance_with_concurrent_ops(&mut engine);
        
        let mut failures = Vec::new();
        self.validate_metrics(&metrics, &mut failures);

        PerformanceResult {
            name: "Concurrent Operations Performance".to_string(),
            passed: failures.is_empty(),
            metrics,
            failures,
        }
    }

    /// Test raw torque mode performance
    fn test_raw_torque_mode_performance(&self) -> PerformanceResult {
        let config = FfbConfig {
            max_torque_nm: 15.0,
            fault_timeout_ms: 50,
            interlock_required: true,
            mode: FfbMode::RawTorque,
            device_path: None,
        };

        let mut engine = FfbEngine::new(config).expect("Failed to create FFB engine");
        
        let capabilities = DeviceCapabilities {
            supports_pid: true,
            supports_raw_torque: true,
            max_torque_nm: 15.0,
            min_period_us: 1000, // 1kHz for raw torque
            has_health_stream: true,
            supports_interlock: true,
        };
        engine.set_device_capabilities(capabilities).expect("Failed to set capabilities");

        let metrics = self.measure_engine_performance(&mut engine, "Raw Torque Mode");
        
        let mut failures = Vec::new();
        self.validate_metrics(&metrics, &mut failures);

        PerformanceResult {
            name: "Raw Torque Mode Performance".to_string(),
            passed: failures.is_empty(),
            metrics,
            failures,
        }
    }

    /// Test DirectInput mode performance
    fn test_directinput_mode_performance(&self) -> PerformanceResult {
        let config = FfbConfig {
            max_torque_nm: 15.0,
            fault_timeout_ms: 50,
            interlock_required: true,
            mode: FfbMode::DirectInput,
            device_path: None,
        };

        let mut engine = FfbEngine::new(config).expect("Failed to create FFB engine");
        
        let capabilities = DeviceCapabilities {
            supports_pid: true,
            supports_raw_torque: false,
            max_torque_nm: 10.0,
            min_period_us: 0,
            has_health_stream: true,
            supports_interlock: false,
        };
        engine.set_device_capabilities(capabilities).expect("Failed to set capabilities");

        let metrics = self.measure_engine_performance(&mut engine, "DirectInput Mode");
        
        let mut failures = Vec::new();
        self.validate_metrics(&metrics, &mut failures);

        PerformanceResult {
            name: "DirectInput Mode Performance".to_string(),
            passed: failures.is_empty(),
            metrics,
            failures,
        }
    }

    /// Test telemetry synthesis mode performance
    fn test_telemetry_synth_mode_performance(&self) -> PerformanceResult {
        let config = FfbConfig {
            max_torque_nm: 15.0,
            fault_timeout_ms: 50,
            interlock_required: true,
            mode: FfbMode::TelemetrySynth,
            device_path: None,
        };

        let mut engine = FfbEngine::new(config).expect("Failed to create FFB engine");
        
        let capabilities = DeviceCapabilities {
            supports_pid: false,
            supports_raw_torque: false,
            max_torque_nm: 5.0,
            min_period_us: 0,
            has_health_stream: false,
            supports_interlock: false,
        };
        engine.set_device_capabilities(capabilities).expect("Failed to set capabilities");

        let metrics = self.measure_engine_performance(&mut engine, "Telemetry Synthesis Mode");
        
        let mut failures = Vec::new();
        self.validate_metrics(&metrics, &mut failures);

        PerformanceResult {
            name: "Telemetry Synthesis Mode Performance".to_string(),
            passed: failures.is_empty(),
            metrics,
            failures,
        }
    }

    /// Measure engine performance over the configured test duration
    fn measure_engine_performance(&self, engine: &mut FfbEngine, _test_name: &str) -> PerformanceMetrics {
        let mut processing_times = Vec::new();
        let target_interval = Duration::from_nanos(1_000_000_000 / self.config.target_frequency_hz as u64);
        let start_time = Instant::now();
        let mut missed_deadlines = 0;

        while start_time.elapsed() < self.config.test_duration {
            let iteration_start = Instant::now();
            
            // Simulate axis processing work
            let _ = engine.update();
            engine.update_heartbeat();
            
            // Record axis frame (simulated)
            let _ = engine.record_axis_frame(
                "test_device".to_string(),
                0.5, // raw input
                0.6, // processed output
                2.0, // torque
            );
            
            let processing_time = iteration_start.elapsed();
            let processing_time_us = processing_time.as_secs_f32() * 1_000_000.0;
            processing_times.push(processing_time_us);
            
            // Check for missed deadlines (>5ms processing time)
            if processing_time > Duration::from_millis(5) {
                missed_deadlines += 1;
            }
            
            // Sleep to maintain target frequency
            let elapsed = iteration_start.elapsed();
            if elapsed < target_interval {
                std::thread::sleep(target_interval - elapsed);
            }
        }

        self.calculate_metrics(processing_times, missed_deadlines)
    }

    /// Measure performance with concurrent operations
    fn measure_engine_performance_with_concurrent_ops(&self, engine: &mut FfbEngine) -> PerformanceMetrics {
        let mut processing_times = Vec::new();
        let target_interval = Duration::from_nanos(1_000_000_000 / self.config.target_frequency_hz as u64);
        let start_time = Instant::now();
        let mut missed_deadlines = 0;
        let mut iteration_count = 0;

        while start_time.elapsed() < self.config.test_duration {
            let iteration_start = Instant::now();
            
            // Simulate axis processing work
            let _ = engine.update();
            engine.update_heartbeat();
            
            // Periodically trigger additional operations
            if iteration_count % 50 == 0 {
                // Simulate mode re-negotiation
                if let Some(capabilities) = engine.device_capabilities().cloned() {
                    let negotiator = ModeNegotiator::new();
                    let _selection = negotiator.negotiate_mode(&capabilities);
                }
            }
            
            if iteration_count % 25 == 0 {
                // Simulate trim adjustments
                let trim_controller = engine.get_trim_controller_mut();
                let change = crate::SetpointChange {
                    target_nm: (iteration_count as f32 % 10.0) - 5.0, // Vary between -5 and 5
                    limits: crate::TrimLimits::default(),
                };
                let _ = trim_controller.apply_setpoint_change(change);
            }
            
            // Record axis frame
            let _ = engine.record_axis_frame(
                "test_device".to_string(),
                (iteration_count as f32 * 0.01) % 2.0 - 1.0, // Sine-like input
                (iteration_count as f32 * 0.01) % 2.0 - 1.0, // Processed output
                2.0 + (iteration_count as f32 * 0.1) % 3.0,   // Varying torque
            );
            
            let processing_time = iteration_start.elapsed();
            let processing_time_us = processing_time.as_secs_f32() * 1_000_000.0;
            processing_times.push(processing_time_us);
            
            // Check for missed deadlines
            if processing_time > Duration::from_millis(5) {
                missed_deadlines += 1;
            }
            
            // Sleep to maintain target frequency
            let elapsed = iteration_start.elapsed();
            if elapsed < target_interval {
                std::thread::sleep(target_interval - elapsed);
            }
            
            iteration_count += 1;
        }

        self.calculate_metrics(processing_times, missed_deadlines)
    }

    /// Calculate performance metrics from processing times
    fn calculate_metrics(&self, mut processing_times: Vec<f32>, missed_deadlines: usize) -> PerformanceMetrics {
        if processing_times.is_empty() {
            return PerformanceMetrics {
                avg_processing_time_us: 0.0,
                p99_processing_time_us: 0.0,
                max_processing_time_us: 0.0,
                jitter_us: 0.0,
                sample_count: 0,
                missed_deadlines,
            };
        }

        processing_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let sample_count = processing_times.len();
        let avg_processing_time_us = processing_times.iter().sum::<f32>() / sample_count as f32;
        let max_processing_time_us = processing_times[sample_count - 1];
        
        // Calculate p99
        let p99_index = ((sample_count as f32 * 0.99) as usize).min(sample_count - 1);
        let p99_processing_time_us = processing_times[p99_index];
        
        // Calculate jitter (standard deviation)
        let variance = processing_times.iter()
            .map(|&x| (x - avg_processing_time_us).powi(2))
            .sum::<f32>() / sample_count as f32;
        let jitter_us = variance.sqrt();

        PerformanceMetrics {
            avg_processing_time_us,
            p99_processing_time_us,
            max_processing_time_us,
            jitter_us,
            sample_count,
            missed_deadlines,
        }
    }

    /// Validate metrics against performance criteria
    fn validate_metrics(&self, metrics: &PerformanceMetrics, failures: &mut Vec<String>) {
        if metrics.p99_processing_time_us > self.config.max_p99_latency_us {
            failures.push(format!(
                "p99 latency exceeded: {:.2}μs > {:.2}μs",
                metrics.p99_processing_time_us, self.config.max_p99_latency_us
            ));
        }

        if metrics.jitter_us > self.config.max_jitter_us {
            failures.push(format!(
                "Jitter exceeded: {:.2}μs > {:.2}μs",
                metrics.jitter_us, self.config.max_jitter_us
            ));
        }

        if metrics.missed_deadlines > self.config.max_missed_deadlines {
            failures.push(format!(
                "Missed deadlines: {} > {}",
                metrics.missed_deadlines, self.config.max_missed_deadlines
            ));
        }

        // Additional sanity checks
        if metrics.sample_count == 0 {
            failures.push("No samples collected during test".to_string());
        }

        if metrics.avg_processing_time_us > 1000.0 { // 1ms average seems excessive
            failures.push(format!(
                "Average processing time too high: {:.2}μs",
                metrics.avg_processing_time_us
            ));
        }
    }

    /// Generate performance report
    pub fn generate_performance_report(&self, results: &[PerformanceResult]) -> String {
        let mut report = String::new();
        report.push_str("# FFB Performance Validation Report\n\n");

        let total_tests = results.len();
        let passed_tests = results.iter().filter(|r| r.passed).count();
        let failed_tests = total_tests - passed_tests;

        report.push_str(&format!("## Summary\n"));
        report.push_str(&format!("- Total Tests: {}\n", total_tests));
        report.push_str(&format!("- Passed: {}\n", passed_tests));
        report.push_str(&format!("- Failed: {}\n", failed_tests));
        report.push_str(&format!("- Success Rate: {:.1}%\n\n", 
            (passed_tests as f32 / total_tests as f32) * 100.0));

        report.push_str("## Performance Criteria\n");
        report.push_str(&format!("- Max p99 Latency: {:.2}μs\n", self.config.max_p99_latency_us));
        report.push_str(&format!("- Max Jitter: {:.2}μs\n", self.config.max_jitter_us));
        report.push_str(&format!("- Max Missed Deadlines: {}\n", self.config.max_missed_deadlines));
        report.push_str(&format!("- Test Duration: {:.1}s\n\n", self.config.test_duration.as_secs_f32()));

        report.push_str("## Test Results\n\n");
        
        for result in results {
            let status = if result.passed { "✅ PASS" } else { "❌ FAIL" };
            report.push_str(&format!("### {} - {}\n", status, result.name));
            
            let m = &result.metrics;
            report.push_str(&format!("- Samples: {}\n", m.sample_count));
            report.push_str(&format!("- Average: {:.2}μs\n", m.avg_processing_time_us));
            report.push_str(&format!("- p99: {:.2}μs\n", m.p99_processing_time_us));
            report.push_str(&format!("- Max: {:.2}μs\n", m.max_processing_time_us));
            report.push_str(&format!("- Jitter: {:.2}μs\n", m.jitter_us));
            report.push_str(&format!("- Missed Deadlines: {}\n", m.missed_deadlines));
            
            if !result.failures.is_empty() {
                report.push_str("- Failures:\n");
                for failure in &result.failures {
                    report.push_str(&format!("  - {}\n", failure));
                }
            }
            
            report.push_str("\n");
        }

        report
    }
}



impl Default for PerformanceValidator {
    fn default() -> Self {
        Self::new(PerformanceConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_performance_validator_creation() {
        let validator = PerformanceValidator::default();
        assert_eq!(validator.config.target_frequency_hz, 250);
        assert_eq!(validator.config.max_p99_latency_us, 5000.0);
    }

    #[test]
    fn test_baseline_performance() {
        let config = PerformanceConfig {
            test_duration: Duration::from_millis(100), // Short test
            ..Default::default()
        };
        let validator = PerformanceValidator::new(config);
        
        let result = validator.test_baseline_ffb_performance();
        
        // Baseline should pass performance criteria
        assert!(result.passed, "Baseline performance test failed: {:?}", result.failures);
        assert!(result.metrics.sample_count > 0);
    }

    #[test]
    fn test_mode_negotiation_performance() {
        let config = PerformanceConfig {
            test_duration: Duration::from_millis(100), // Short test
            ..Default::default()
        };
        let validator = PerformanceValidator::new(config);
        
        let result = validator.test_mode_negotiation_performance();
        
        // Mode negotiation should not significantly impact performance
        assert!(result.passed, "Mode negotiation performance test failed: {:?}", result.failures);
    }

    #[test]
    fn test_metrics_calculation() {
        let validator = PerformanceValidator::default();
        
        let processing_times = vec![100.0, 200.0, 150.0, 300.0, 120.0];
        let metrics = validator.calculate_metrics(processing_times, 0);
        
        assert_eq!(metrics.sample_count, 5);
        assert_eq!(metrics.avg_processing_time_us, 174.0);
        assert_eq!(metrics.max_processing_time_us, 300.0);
        assert_eq!(metrics.missed_deadlines, 0);
    }

    #[test]
    fn test_performance_report_generation() {
        let validator = PerformanceValidator::default();
        
        let mock_results = vec![
            PerformanceResult {
                name: "Test 1".to_string(),
                passed: true,
                metrics: PerformanceMetrics {
                    avg_processing_time_us: 100.0,
                    p99_processing_time_us: 200.0,
                    max_processing_time_us: 250.0,
                    jitter_us: 50.0,
                    sample_count: 1000,
                    missed_deadlines: 0,
                },
                failures: vec![],
            },
        ];
        
        let report = validator.generate_performance_report(&mock_results);
        
        assert!(report.contains("Total Tests: 1"));
        assert!(report.contains("Passed: 1"));
        assert!(report.contains("✅ PASS"));
        assert!(report.contains("Average: 100.00μs"));
    }
}