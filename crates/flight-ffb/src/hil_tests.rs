// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Hardware-in-Loop (HIL) tests for FFB mode negotiation and trim validation
//!
//! These tests validate the FFB mode selection matrix and trim behavior
//! with floating-point tolerance matching to ensure consistent behavior
//! across different hardware configurations.

use std::time::{Duration, Instant};
use crate::{
    DeviceCapabilities, FfbMode, ModeNegotiator, ModeSelectionPolicy, 
    TrimController, TrimMode, SetpointChange, TrimLimits, TrimOutput
};

/// HIL test configuration
#[derive(Debug, Clone)]
pub struct HilTestConfig {
    /// Floating-point tolerance for comparisons
    pub fp_tolerance: f32,
    /// Maximum test duration
    pub max_test_duration: Duration,
    /// Sample rate for measurements
    pub sample_rate_hz: u32,
}

impl Default for HilTestConfig {
    fn default() -> Self {
        Self {
            fp_tolerance: 1e-6,
            max_test_duration: Duration::from_secs(10),
            sample_rate_hz: 1000,
        }
    }
}

/// HIL test result
#[derive(Debug, Clone)]
pub struct HilTestResult {
    /// Test name
    pub name: String,
    /// Whether test passed
    pub passed: bool,
    /// Test duration
    pub duration: Duration,
    /// Error message if failed
    pub error: Option<String>,
    /// Measured values for analysis
    pub measurements: Vec<f32>,
}

/// HIL test suite for FFB mode negotiation
pub struct HilTestSuite {
    config: HilTestConfig,
}

impl HilTestSuite {
    /// Create new HIL test suite
    pub fn new(config: HilTestConfig) -> Self {
        Self { config }
    }

    /// Run complete mode selection matrix test
    pub fn run_mode_selection_matrix_test(&self) -> Vec<HilTestResult> {
        let negotiator = ModeNegotiator::new();
        let test_matrix = negotiator.create_selection_matrix();
        let mut results = Vec::new();

        for test_case in test_matrix {
            let start_time = Instant::now();
            let mut measurements = Vec::new();
            
            let result = match self.validate_mode_selection(&negotiator, &test_case) {
                Ok(measurement) => {
                    measurements.push(measurement);
                    HilTestResult {
                        name: format!("Mode Selection: {}", test_case.name),
                        passed: true,
                        duration: start_time.elapsed(),
                        error: None,
                        measurements,
                    }
                }
                Err(error) => {
                    HilTestResult {
                        name: format!("Mode Selection: {}", test_case.name),
                        passed: false,
                        duration: start_time.elapsed(),
                        error: Some(error),
                        measurements,
                    }
                }
            };
            
            results.push(result);
        }

        results
    }

    /// Run trim behavior validation tests
    pub fn run_trim_validation_tests(&self) -> Vec<HilTestResult> {
        let mut results = Vec::new();

        // Test FFB trim behavior
        results.push(self.test_ffb_trim_rate_limiting());
        results.push(self.test_ffb_trim_jerk_limiting());
        results.push(self.test_ffb_trim_convergence());

        // Test spring trim behavior
        results.push(self.test_spring_trim_freeze_ramp());
        results.push(self.test_spring_trim_center_mapping());

        // Test mode-specific trim limits
        results.push(self.test_mode_specific_trim_limits());

        results
    }

    /// Validate mode selection against expected results
    fn validate_mode_selection(
        &self,
        negotiator: &ModeNegotiator,
        test_case: &crate::ModeSelectionTest,
    ) -> Result<f32, String> {
        let selection = negotiator.negotiate_mode(&test_case.capabilities);

        // Validate mode selection
        if selection.mode != test_case.expected_mode {
            return Err(format!(
                "Mode mismatch: expected {:?}, got {:?}",
                test_case.expected_mode, selection.mode
            ));
        }

        // Validate high torque support
        if selection.supports_high_torque != test_case.expected_high_torque {
            return Err(format!(
                "High torque support mismatch: expected {}, got {}",
                test_case.expected_high_torque, selection.supports_high_torque
            ));
        }

        // Validate trim limits are reasonable
        if let Err(validation_error) = selection.trim_limits.validate_trim_limits() {
            return Err(format!("Invalid trim limits: {}", validation_error));
        }

        // Return update rate as measurement
        Ok(selection.update_rate_hz as f32)
    }

    /// Test FFB trim rate limiting
    fn test_ffb_trim_rate_limiting(&self) -> HilTestResult {
        let start_time = Instant::now();
        let mut measurements = Vec::new();

        let result = (|| -> Result<(), String> {
            let mut controller = TrimController::new(15.0);
            controller.set_mode(TrimMode::ForceFeedback);

            let limits = TrimLimits {
                max_rate_nm_per_s: 5.0,
                max_jerk_nm_per_s2: 20.0,
            };

            let change = SetpointChange {
                target_nm: 10.0,
                limits: limits.clone(),
            };

            controller.apply_setpoint_change(change)
                .map_err(|e| format!("Failed to apply setpoint change: {}", e))?;

            // Simulate updates and measure rate compliance
            let mut max_observed_rate = 0.0f32;
            let sample_interval = Duration::from_millis(1);
            
            for _ in 0..1000 {
                let output = controller.update();
                
                if let TrimOutput::ForceFeedback { rate_nm_per_s, .. } = output {
                    max_observed_rate = max_observed_rate.max(rate_nm_per_s.abs());
                    measurements.push(rate_nm_per_s.abs());
                    
                    // Check rate limit compliance
                    if rate_nm_per_s.abs() > limits.max_rate_nm_per_s + self.config.fp_tolerance {
                        return Err(format!(
                            "Rate limit exceeded: {} > {} Nm/s",
                            rate_nm_per_s.abs(), limits.max_rate_nm_per_s
                        ));
                    }
                }
                
                std::thread::sleep(sample_interval);
                
                if start_time.elapsed() > self.config.max_test_duration {
                    break;
                }
            }

            // Verify we actually used the available rate (within tolerance)
            if max_observed_rate < limits.max_rate_nm_per_s * 0.8 {
                return Err(format!(
                    "Rate limit underutilized: max {} < 80% of {} Nm/s",
                    max_observed_rate, limits.max_rate_nm_per_s
                ));
            }

            Ok(())
        })();

        HilTestResult {
            name: "FFB Trim Rate Limiting".to_string(),
            passed: result.is_ok(),
            duration: start_time.elapsed(),
            error: result.err(),
            measurements,
        }
    }

    /// Test FFB trim jerk limiting
    fn test_ffb_trim_jerk_limiting(&self) -> HilTestResult {
        let start_time = Instant::now();
        let mut measurements = Vec::new();

        let result = (|| -> Result<(), String> {
            let mut controller = TrimController::new(15.0);
            controller.set_mode(TrimMode::ForceFeedback);

            let limits = TrimLimits {
                max_rate_nm_per_s: 10.0,
                max_jerk_nm_per_s2: 30.0,
            };

            let change = SetpointChange {
                target_nm: 8.0,
                limits: limits.clone(),
            };

            controller.apply_setpoint_change(change)
                .map_err(|e| format!("Failed to apply setpoint change: {}", e))?;

            // Measure jerk (rate of rate change)
            let mut previous_rate = 0.0f32;
            let mut max_observed_jerk = 0.0f32;
            let dt = 0.001f32; // 1ms timestep
            
            for _ in 0..1000 {
                let output = controller.update();
                
                if let TrimOutput::ForceFeedback { rate_nm_per_s, .. } = output {
                    let jerk = (rate_nm_per_s - previous_rate).abs() / dt;
                    max_observed_jerk = max_observed_jerk.max(jerk);
                    measurements.push(jerk);
                    
                    // Check jerk limit compliance (with some tolerance for discrete sampling)
                    if jerk > limits.max_jerk_nm_per_s2 + self.config.fp_tolerance * 10.0 {
                        return Err(format!(
                            "Jerk limit exceeded: {} > {} Nm/s²",
                            jerk, limits.max_jerk_nm_per_s2
                        ));
                    }
                    
                    previous_rate = rate_nm_per_s;
                }
                
                std::thread::sleep(Duration::from_millis(1));
                
                if start_time.elapsed() > self.config.max_test_duration {
                    break;
                }
            }

            Ok(())
        })();

        HilTestResult {
            name: "FFB Trim Jerk Limiting".to_string(),
            passed: result.is_ok(),
            duration: start_time.elapsed(),
            error: result.err(),
            measurements,
        }
    }

    /// Test FFB trim convergence to target
    fn test_ffb_trim_convergence(&self) -> HilTestResult {
        let start_time = Instant::now();
        let mut measurements = Vec::new();

        let result = (|| -> Result<(), String> {
            let mut controller = TrimController::new(15.0);
            controller.set_mode(TrimMode::ForceFeedback);

            let target = 7.5f32;
            let change = SetpointChange {
                target_nm: target,
                limits: TrimLimits::default(),
            };

            controller.apply_setpoint_change(change)
                .map_err(|e| format!("Failed to apply setpoint change: {}", e))?;

            // Run until convergence or timeout
            let mut converged = false;
            
            for _ in 0..5000 {
                let output = controller.update();
                
                if let TrimOutput::ForceFeedback { setpoint_nm, .. } = output {
                    let error = (setpoint_nm - target).abs();
                    measurements.push(error);
                    
                    // Check for convergence
                    if error < self.config.fp_tolerance * 1000.0 { // 1e-3 tolerance
                        converged = true;
                        break;
                    }
                }
                
                std::thread::sleep(Duration::from_millis(1));
                
                if start_time.elapsed() > self.config.max_test_duration {
                    break;
                }
            }

            if !converged {
                return Err(format!(
                    "Failed to converge to target {} within {} seconds",
                    target, self.config.max_test_duration.as_secs()
                ));
            }

            Ok(())
        })();

        HilTestResult {
            name: "FFB Trim Convergence".to_string(),
            passed: result.is_ok(),
            duration: start_time.elapsed(),
            error: result.err(),
            measurements,
        }
    }

    /// Test spring trim freeze and ramp behavior
    fn test_spring_trim_freeze_ramp(&self) -> HilTestResult {
        let start_time = Instant::now();
        let mut measurements = Vec::new();

        let result = (|| -> Result<(), String> {
            let mut controller = TrimController::new(15.0);
            controller.set_mode(TrimMode::SpringCentered);

            let change = SetpointChange {
                target_nm: 5.0,
                limits: TrimLimits::default(),
            };

            controller.apply_setpoint_change(change)
                .map_err(|e| format!("Failed to apply setpoint change: {}", e))?;

            // Verify spring is initially frozen
            let output = controller.update();
            if let TrimOutput::SpringCentered { frozen, .. } = output {
                if !frozen {
                    return Err("Spring should be frozen immediately after setpoint change".to_string());
                }
                measurements.push(1.0); // Frozen state
            } else {
                return Err("Expected SpringCentered output".to_string());
            }

            // Wait for ramp to start
            std::thread::sleep(Duration::from_millis(150));

            // Check that spring eventually unfreezes
            let mut unfrozen = false;
            for _ in 0..100 {
                let output = controller.update();
                if let TrimOutput::SpringCentered { frozen, .. } = output {
                    measurements.push(if frozen { 1.0 } else { 0.0 });
                    if !frozen {
                        unfrozen = true;
                        break;
                    }
                }
                std::thread::sleep(Duration::from_millis(10));
            }

            if !unfrozen {
                return Err("Spring should unfreeze after ramp period".to_string());
            }

            Ok(())
        })();

        HilTestResult {
            name: "Spring Trim Freeze/Ramp".to_string(),
            passed: result.is_ok(),
            duration: start_time.elapsed(),
            error: result.err(),
            measurements,
        }
    }

    /// Test spring center position mapping
    fn test_spring_trim_center_mapping(&self) -> HilTestResult {
        let start_time = Instant::now();
        let mut measurements = Vec::new();

        let result = (|| -> Result<(), String> {
            let mut controller = TrimController::new(15.0);
            controller.set_mode(TrimMode::SpringCentered);

            // Test various setpoints and verify center mapping
            let test_cases = vec![
                (0.0, 0.0),    // Zero torque -> center
                (7.5, 0.5),   // Half max -> 0.5 center
                (-7.5, -0.5), // Negative half -> -0.5 center
                (15.0, 1.0),  // Max torque -> 1.0 center
                (-15.0, -1.0), // Min torque -> -1.0 center
            ];

            for (target_nm, expected_center) in test_cases {
                let change = SetpointChange {
                    target_nm,
                    limits: TrimLimits::default(),
                };

                controller.apply_setpoint_change(change)
                    .map_err(|e| format!("Failed to apply setpoint {}: {}", target_nm, e))?;

                let output = controller.update();
                if let TrimOutput::SpringCentered { config, .. } = output {
                    let center_error = (config.center - expected_center).abs();
                    measurements.push(center_error);
                    
                    if center_error > self.config.fp_tolerance * 1000.0 {
                        return Err(format!(
                            "Center mapping error for {} Nm: expected {}, got {} (error: {})",
                            target_nm, expected_center, config.center, center_error
                        ));
                    }
                } else {
                    return Err("Expected SpringCentered output".to_string());
                }
            }

            Ok(())
        })();

        HilTestResult {
            name: "Spring Center Mapping".to_string(),
            passed: result.is_ok(),
            duration: start_time.elapsed(),
            error: result.err(),
            measurements,
        }
    }

    /// Test mode-specific trim limits
    fn test_mode_specific_trim_limits(&self) -> HilTestResult {
        let start_time = Instant::now();
        let mut measurements = Vec::new();

        let result = (|| -> Result<(), String> {
            let negotiator = ModeNegotiator::new();

            // Test different device capabilities and verify trim limits scale appropriately
            let test_cases = vec![
                (
                    "High-end raw torque",
                    DeviceCapabilities {
                        supports_pid: true,
                        supports_raw_torque: true,
                        max_torque_nm: 15.0,
                        min_period_us: 1000,
                        has_health_stream: true,
                        supports_interlock: true,
                    },
                    FfbMode::RawTorque,
                ),
                (
                    "DirectInput device",
                    DeviceCapabilities {
                        supports_pid: true,
                        supports_raw_torque: false,
                        max_torque_nm: 10.0,
                        min_period_us: 0,
                        has_health_stream: true,
                        supports_interlock: false,
                    },
                    FfbMode::DirectInput,
                ),
                (
                    "Telemetry synthesis",
                    DeviceCapabilities {
                        supports_pid: false,
                        supports_raw_torque: false,
                        max_torque_nm: 5.0,
                        min_period_us: 0,
                        has_health_stream: false,
                        supports_interlock: false,
                    },
                    FfbMode::TelemetrySynth,
                ),
            ];

            let mut previous_rate = None;
            
            for (name, capabilities, expected_mode) in test_cases {
                let selection = negotiator.negotiate_mode(&capabilities);
                
                if selection.mode != expected_mode {
                    return Err(format!(
                        "Mode mismatch for {}: expected {:?}, got {:?}",
                        name, expected_mode, selection.mode
                    ));
                }

                measurements.push(selection.trim_limits.max_rate_nm_per_s);

                // Verify limits are reasonable for the mode
                match selection.mode {
                    FfbMode::RawTorque => {
                        // Raw torque should have the most aggressive limits
                        if selection.trim_limits.max_rate_nm_per_s < 8.0 {
                            return Err(format!(
                                "Raw torque limits too conservative: {} Nm/s",
                                selection.trim_limits.max_rate_nm_per_s
                            ));
                        }
                    }
                    FfbMode::DirectInput => {
                        // DirectInput should be moderate
                        if selection.trim_limits.max_rate_nm_per_s < 3.0 || 
                           selection.trim_limits.max_rate_nm_per_s > 10.0 {
                            return Err(format!(
                                "DirectInput limits out of range: {} Nm/s",
                                selection.trim_limits.max_rate_nm_per_s
                            ));
                        }
                    }
                    FfbMode::TelemetrySynth => {
                        // Telemetry synthesis should be most conservative
                        if selection.trim_limits.max_rate_nm_per_s > 3.0 {
                            return Err(format!(
                                "Telemetry synthesis limits too aggressive: {} Nm/s",
                                selection.trim_limits.max_rate_nm_per_s
                            ));
                        }
                    }
                    FfbMode::Auto => {
                        return Err("Auto mode should not appear in final selection".to_string());
                    }
                }

                // Verify limits decrease as we go to less capable modes
                if let Some(prev_rate) = previous_rate {
                    if selection.trim_limits.max_rate_nm_per_s > prev_rate + self.config.fp_tolerance {
                        return Err(format!(
                            "Trim limits should decrease with less capable modes: {} > {}",
                            selection.trim_limits.max_rate_nm_per_s, prev_rate
                        ));
                    }
                }
                previous_rate = Some(selection.trim_limits.max_rate_nm_per_s);
            }

            Ok(())
        })();

        HilTestResult {
            name: "Mode-Specific Trim Limits".to_string(),
            passed: result.is_ok(),
            duration: start_time.elapsed(),
            error: result.err(),
            measurements,
        }
    }

    /// Generate test report
    pub fn generate_test_report(&self, results: &[HilTestResult]) -> String {
        let mut report = String::new();
        report.push_str("# FFB HIL Test Report\n\n");

        let total_tests = results.len();
        let passed_tests = results.iter().filter(|r| r.passed).count();
        let failed_tests = total_tests - passed_tests;

        report.push_str(&format!("## Summary\n"));
        report.push_str(&format!("- Total Tests: {}\n", total_tests));
        report.push_str(&format!("- Passed: {}\n", passed_tests));
        report.push_str(&format!("- Failed: {}\n", failed_tests));
        report.push_str(&format!("- Success Rate: {:.1}%\n\n", 
            (passed_tests as f32 / total_tests as f32) * 100.0));

        report.push_str("## Test Results\n\n");
        
        for result in results {
            let status = if result.passed { "✅ PASS" } else { "❌ FAIL" };
            report.push_str(&format!("### {} - {}\n", status, result.name));
            report.push_str(&format!("- Duration: {:.2}ms\n", result.duration.as_secs_f32() * 1000.0));
            
            if let Some(error) = &result.error {
                report.push_str(&format!("- Error: {}\n", error));
            }
            
            if !result.measurements.is_empty() {
                let avg = result.measurements.iter().sum::<f32>() / result.measurements.len() as f32;
                let max = result.measurements.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
                let min = result.measurements.iter().fold(f32::INFINITY, |a, &b| a.min(b));
                
                report.push_str(&format!("- Measurements: {} samples\n", result.measurements.len()));
                report.push_str(&format!("  - Average: {:.6}\n", avg));
                report.push_str(&format!("  - Min: {:.6}\n", min));
                report.push_str(&format!("  - Max: {:.6}\n", max));
            }
            
            report.push_str("\n");
        }

        report
    }
}

impl Default for HilTestSuite {
    fn default() -> Self {
        Self::new(HilTestConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hil_test_suite_creation() {
        let suite = HilTestSuite::default();
        assert_eq!(suite.config.fp_tolerance, 1e-6);
        assert_eq!(suite.config.sample_rate_hz, 1000);
    }

    #[test]
    fn test_mode_selection_matrix_validation() {
        let suite = HilTestSuite::default();
        let results = suite.run_mode_selection_matrix_test();
        
        // All mode selection tests should pass
        for result in &results {
            if !result.passed {
                panic!("Mode selection test failed: {} - {:?}", result.name, result.error);
            }
        }
        
        assert!(!results.is_empty());
    }

    #[test]
    fn test_trim_validation_basic() {
        let suite = HilTestSuite::default();
        
        // Test individual trim validation functions
        let ffb_rate_result = suite.test_ffb_trim_rate_limiting();
        assert!(ffb_rate_result.passed, "FFB rate limiting test failed: {:?}", ffb_rate_result.error);
        
        let spring_freeze_result = suite.test_spring_trim_freeze_ramp();
        assert!(spring_freeze_result.passed, "Spring freeze/ramp test failed: {:?}", spring_freeze_result.error);
    }

    #[test]
    fn test_report_generation() {
        let suite = HilTestSuite::default();
        
        let mock_results = vec![
            HilTestResult {
                name: "Test 1".to_string(),
                passed: true,
                duration: Duration::from_millis(100),
                error: None,
                measurements: vec![1.0, 2.0, 3.0],
            },
            HilTestResult {
                name: "Test 2".to_string(),
                passed: false,
                duration: Duration::from_millis(50),
                error: Some("Mock error".to_string()),
                measurements: vec![],
            },
        ];
        
        let report = suite.generate_test_report(&mock_results);
        
        assert!(report.contains("Total Tests: 2"));
        assert!(report.contains("Passed: 1"));
        assert!(report.contains("Failed: 1"));
        assert!(report.contains("✅ PASS"));
        assert!(report.contains("❌ FAIL"));
        assert!(report.contains("Mock error"));
    }
}