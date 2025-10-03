// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Trim correctness validation for force feedback devices
//!
//! This module provides comprehensive validation of trim behavior including:
//! - Non-FFB recentre illusion with trim-hold freeze
//! - FFB setpoint changes with rate/jerk limiting
//! - Hardware-in-loop tests for trim behavior
//! - Replay reproducibility validation

use std::time::{Duration, Instant};
use crate::{
    TrimController, TrimMode, SetpointChange, TrimLimits, TrimOutput, TrimState,
    SpringConfig, BlackboxRecorder, BlackboxEntry, BlackboxConfig
};

/// Trim validation configuration
#[derive(Debug, Clone)]
pub struct TrimValidationConfig {
    /// Floating-point tolerance for comparisons
    pub fp_tolerance: f32,
    /// Maximum test duration
    pub max_test_duration: Duration,
    /// Sample rate for measurements (Hz)
    pub sample_rate_hz: u32,
    /// Enable detailed logging
    pub verbose_logging: bool,
}

impl Default for TrimValidationConfig {
    fn default() -> Self {
        Self {
            fp_tolerance: 1e-6,
            max_test_duration: Duration::from_secs(30),
            sample_rate_hz: 1000,
            verbose_logging: false,
        }
    }
}

/// Trim validation result
#[derive(Debug, Clone)]
pub struct TrimValidationResult {
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
    /// Detailed metrics
    pub metrics: TrimValidationMetrics,
}

/// Detailed metrics from trim validation
#[derive(Debug, Clone)]
pub struct TrimValidationMetrics {
    /// Maximum observed rate (Nm/s)
    pub max_rate_nm_per_s: f32,
    /// Maximum observed jerk (Nm/s²)
    pub max_jerk_nm_per_s2: f32,
    /// Final convergence error (Nm)
    pub final_error_nm: f32,
    /// Time to convergence
    pub convergence_time: Option<Duration>,
    /// Number of torque steps detected
    pub torque_steps_detected: u32,
    /// Spring freeze duration (for non-FFB)
    pub spring_freeze_duration: Option<Duration>,
    /// Spring ramp duration (for non-FFB)
    pub spring_ramp_duration: Option<Duration>,
}

impl Default for TrimValidationMetrics {
    fn default() -> Self {
        Self {
            max_rate_nm_per_s: 0.0,
            max_jerk_nm_per_s2: 0.0,
            final_error_nm: 0.0,
            convergence_time: None,
            torque_steps_detected: 0,
            spring_freeze_duration: None,
            spring_ramp_duration: None,
        }
    }
}

/// Trim validation test suite
pub struct TrimValidationSuite {
    config: TrimValidationConfig,
    blackbox: BlackboxRecorder,
}

impl TrimValidationSuite {
    /// Create new trim validation suite
    pub fn new(config: TrimValidationConfig) -> Self {
        let blackbox_config = BlackboxConfig {
            max_entries: 50000, // Large buffer for validation
            pre_fault_duration: Duration::from_secs(5),
            post_fault_duration: Duration::from_secs(5),
            ..Default::default()
        };
        
        let blackbox = BlackboxRecorder::new(blackbox_config)
            .expect("Failed to create blackbox recorder");
        
        Self { config, blackbox }
    }

    /// Run complete trim validation test suite
    pub fn run_complete_validation(&mut self) -> Vec<TrimValidationResult> {
        let mut results = Vec::new();

        // FFB trim validation tests
        results.push(self.test_ffb_rate_limiting());
        results.push(self.test_ffb_jerk_limiting());
        results.push(self.test_ffb_no_torque_steps());
        results.push(self.test_ffb_convergence_accuracy());
        results.push(self.test_ffb_setpoint_overshoot());

        // Non-FFB (spring) trim validation tests
        results.push(self.test_spring_freeze_behavior());
        results.push(self.test_spring_ramp_behavior());
        results.push(self.test_spring_center_mapping());
        results.push(self.test_spring_recentre_illusion());

        // Cross-mode validation tests
        results.push(self.test_mode_switching_stability());
        results.push(self.test_extreme_setpoint_handling());

        // Replay reproducibility tests
        results.push(self.test_replay_reproducibility());

        results
    }

    /// Test FFB rate limiting compliance
    fn test_ffb_rate_limiting(&mut self) -> TrimValidationResult {
        let start_time = Instant::now();
        let mut measurements = Vec::new();
        let mut metrics = TrimValidationMetrics::default();

        let result = (|| -> Result<(), String> {
            let mut controller = TrimController::new(15.0);
            controller.set_mode(TrimMode::ForceFeedback);

            let limits = TrimLimits {
                max_rate_nm_per_s: 8.0,
                max_jerk_nm_per_s2: 25.0,
            };

            let change = SetpointChange {
                target_nm: 12.0,
                limits: limits.clone(),
            };

            controller.apply_setpoint_change(change)
                .map_err(|e| format!("Failed to apply setpoint change: {}", e))?;

            let sample_interval = Duration::from_millis(1);
            let mut max_observed_rate = 0.0f32;
            
            for i in 0..5000 {
                let output = controller.update();
                
                if let TrimOutput::ForceFeedback { rate_nm_per_s, setpoint_nm } = output {
                    max_observed_rate = max_observed_rate.max(rate_nm_per_s.abs());
                    measurements.push(rate_nm_per_s.abs());
                    
                    // Record in blackbox
                    self.blackbox.record(BlackboxEntry::FfbState {
                        timestamp: Instant::now(),
                        safety_state: "TRIM_TEST".to_string(),
                        torque_setpoint: setpoint_nm,
                        actual_torque: setpoint_nm,
                    }).map_err(|e| format!("Blackbox error: {}", e))?;
                    
                    // Check rate limit compliance
                    if rate_nm_per_s.abs() > limits.max_rate_nm_per_s + self.config.fp_tolerance {
                        return Err(format!(
                            "Rate limit exceeded at sample {}: {} > {} Nm/s",
                            i, rate_nm_per_s.abs(), limits.max_rate_nm_per_s
                        ));
                    }
                }
                
                std::thread::sleep(sample_interval);
                
                if start_time.elapsed() > self.config.max_test_duration {
                    break;
                }
            }

            metrics.max_rate_nm_per_s = max_observed_rate;

            // Verify we actually used the available rate (within 80% utilization)
            if max_observed_rate < limits.max_rate_nm_per_s * 0.8 {
                return Err(format!(
                    "Rate limit underutilized: max {} < 80% of {} Nm/s",
                    max_observed_rate, limits.max_rate_nm_per_s
                ));
            }

            Ok(())
        })();

        TrimValidationResult {
            name: "FFB Rate Limiting".to_string(),
            passed: result.is_ok(),
            duration: start_time.elapsed(),
            error: result.err(),
            measurements,
            metrics,
        }
    }

    /// Test FFB jerk limiting compliance
    fn test_ffb_jerk_limiting(&mut self) -> TrimValidationResult {
        let start_time = Instant::now();
        let mut measurements = Vec::new();
        let mut metrics = TrimValidationMetrics::default();

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

            let dt = 0.001f32; // 1ms timestep
            let mut previous_rate = 0.0f32;
            let mut max_observed_jerk = 0.0f32;
            
            for i in 0..3000 {
                let output = controller.update();
                
                if let TrimOutput::ForceFeedback { rate_nm_per_s, .. } = output {
                    let jerk = (rate_nm_per_s - previous_rate).abs() / dt;
                    max_observed_jerk = max_observed_jerk.max(jerk);
                    measurements.push(jerk);
                    
                    // Check jerk limit compliance (with tolerance for discrete sampling)
                    let jerk_tolerance = self.config.fp_tolerance * 100.0; // More tolerance for jerk
                    if jerk > limits.max_jerk_nm_per_s2 + jerk_tolerance {
                        return Err(format!(
                            "Jerk limit exceeded at sample {}: {} > {} Nm/s²",
                            i, jerk, limits.max_jerk_nm_per_s2
                        ));
                    }
                    
                    previous_rate = rate_nm_per_s;
                }
                
                std::thread::sleep(Duration::from_millis(1));
                
                if start_time.elapsed() > self.config.max_test_duration {
                    break;
                }
            }

            metrics.max_jerk_nm_per_s2 = max_observed_jerk;

            Ok(())
        })();

        TrimValidationResult {
            name: "FFB Jerk Limiting".to_string(),
            passed: result.is_ok(),
            duration: start_time.elapsed(),
            error: result.err(),
            measurements,
            metrics,
        }
    }

    /// Test that no torque steps occur during FFB setpoint changes
    fn test_ffb_no_torque_steps(&mut self) -> TrimValidationResult {
        let start_time = Instant::now();
        let mut measurements = Vec::new();
        let mut metrics = TrimValidationMetrics::default();

        let result = (|| -> Result<(), String> {
            let mut controller = TrimController::new(15.0);
            controller.set_mode(TrimMode::ForceFeedback);

            let limits = TrimLimits {
                max_rate_nm_per_s: 6.0,
                max_jerk_nm_per_s2: 20.0,
            };

            let change = SetpointChange {
                target_nm: 9.0,
                limits: limits.clone(),
            };

            controller.apply_setpoint_change(change)
                .map_err(|e| format!("Failed to apply setpoint change: {}", e))?;

            let dt = 0.001f32; // 1ms timestep
            let mut previous_output = 0.0f32;
            let mut torque_steps = 0u32;
            
            for i in 0..4000 {
                let output = controller.update();
                
                if let TrimOutput::ForceFeedback { setpoint_nm, .. } = output {
                    // Validate no torque steps
                    if i > 0 {
                        if let Err(_) = controller.validate_no_torque_steps(previous_output, setpoint_nm, dt) {
                            torque_steps += 1;
                        }
                    }
                    
                    let torque_change = (setpoint_nm - previous_output).abs();
                    measurements.push(torque_change);
                    previous_output = setpoint_nm;
                }
                
                std::thread::sleep(Duration::from_millis(1));
                
                if start_time.elapsed() > self.config.max_test_duration {
                    break;
                }
            }

            metrics.torque_steps_detected = torque_steps;

            if torque_steps > 0 {
                return Err(format!("Detected {} torque steps during setpoint change", torque_steps));
            }

            Ok(())
        })();

        TrimValidationResult {
            name: "FFB No Torque Steps".to_string(),
            passed: result.is_ok(),
            duration: start_time.elapsed(),
            error: result.err(),
            measurements,
            metrics,
        }
    }

    /// Test FFB convergence accuracy
    fn test_ffb_convergence_accuracy(&mut self) -> TrimValidationResult {
        let start_time = Instant::now();
        let mut measurements = Vec::new();
        let mut metrics = TrimValidationMetrics::default();

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

            let convergence_tolerance = 0.01f32; // 10mNm tolerance
            let mut converged = false;
            let mut convergence_time = None;
            
            for _ in 0..10000 {
                let output = controller.update();
                
                if let TrimOutput::ForceFeedback { setpoint_nm, .. } = output {
                    let error = (setpoint_nm - target).abs();
                    measurements.push(error);
                    
                    // Check for convergence
                    if error < convergence_tolerance && !converged {
                        converged = true;
                        convergence_time = Some(start_time.elapsed());
                    }
                    
                    metrics.final_error_nm = error;
                }
                
                std::thread::sleep(Duration::from_millis(1));
                
                if start_time.elapsed() > self.config.max_test_duration {
                    break;
                }
            }

            metrics.convergence_time = convergence_time;

            if !converged {
                return Err(format!(
                    "Failed to converge to target {} within {} seconds (final error: {} Nm)",
                    target, self.config.max_test_duration.as_secs(), metrics.final_error_nm
                ));
            }

            Ok(())
        })();

        TrimValidationResult {
            name: "FFB Convergence Accuracy".to_string(),
            passed: result.is_ok(),
            duration: start_time.elapsed(),
            error: result.err(),
            measurements,
            metrics,
        }
    }

    /// Test FFB setpoint overshoot behavior
    fn test_ffb_setpoint_overshoot(&mut self) -> TrimValidationResult {
        let start_time = Instant::now();
        let mut measurements = Vec::new();
        let mut metrics = TrimValidationMetrics::default();

        let result = (|| -> Result<(), String> {
            let mut controller = TrimController::new(15.0);
            controller.set_mode(TrimMode::ForceFeedback);

            let target = 5.0f32;
            let change = SetpointChange {
                target_nm: target,
                limits: TrimLimits {
                    max_rate_nm_per_s: 3.0,
                    max_jerk_nm_per_s2: 10.0,
                },
            };

            controller.apply_setpoint_change(change)
                .map_err(|e| format!("Failed to apply setpoint change: {}", e))?;

            let mut max_overshoot = 0.0f32;
            
            for _ in 0..5000 {
                let output = controller.update();
                
                if let TrimOutput::ForceFeedback { setpoint_nm, .. } = output {
                    // Measure overshoot (setpoint exceeding target)
                    let overshoot = (setpoint_nm - target).max(0.0);
                    max_overshoot = max_overshoot.max(overshoot);
                    measurements.push(overshoot);
                }
                
                std::thread::sleep(Duration::from_millis(1));
                
                if start_time.elapsed() > self.config.max_test_duration {
                    break;
                }
            }

            // Overshoot should be minimal with proper jerk limiting
            let max_acceptable_overshoot = target * 0.05; // 5% overshoot tolerance
            if max_overshoot > max_acceptable_overshoot {
                return Err(format!(
                    "Excessive overshoot: {} Nm > {} Nm ({}% of target)",
                    max_overshoot, max_acceptable_overshoot, (max_overshoot / target) * 100.0
                ));
            }

            Ok(())
        })();

        TrimValidationResult {
            name: "FFB Setpoint Overshoot".to_string(),
            passed: result.is_ok(),
            duration: start_time.elapsed(),
            error: result.err(),
            measurements,
            metrics,
        }
    }

    /// Test spring freeze behavior for non-FFB devices
    fn test_spring_freeze_behavior(&mut self) -> TrimValidationResult {
        let start_time = Instant::now();
        let mut measurements = Vec::new();
        let mut metrics = TrimValidationMetrics::default();

        let result = (|| -> Result<(), String> {
            let mut controller = TrimController::new(15.0);
            controller.set_mode(TrimMode::SpringCentered);

            let change = SetpointChange {
                target_nm: 6.0,
                limits: TrimLimits::default(),
            };

            controller.apply_setpoint_change(change)
                .map_err(|e| format!("Failed to apply setpoint change: {}", e))?;

            // Verify spring is immediately frozen
            let output = controller.update();
            if let TrimOutput::SpringCentered { frozen, .. } = output {
                if !frozen {
                    return Err("Spring should be frozen immediately after setpoint change".to_string());
                }
                measurements.push(1.0); // Frozen state
            } else {
                return Err("Expected SpringCentered output".to_string());
            }

            // Monitor freeze duration
            let freeze_start = Instant::now();
            let mut freeze_ended = false;
            
            for _ in 0..1000 {
                let output = controller.update();
                if let TrimOutput::SpringCentered { frozen, .. } = output {
                    measurements.push(if frozen { 1.0 } else { 0.0 });
                    
                    if frozen && !freeze_ended {
                        // Still frozen
                    } else if !frozen && !freeze_ended {
                        // Freeze just ended
                        freeze_ended = true;
                        metrics.spring_freeze_duration = Some(freeze_start.elapsed());
                    }
                }
                
                std::thread::sleep(Duration::from_millis(10));
                
                if start_time.elapsed() > self.config.max_test_duration {
                    break;
                }
            }

            if !freeze_ended {
                return Err("Spring should unfreeze after hold period".to_string());
            }

            // Verify freeze duration is reasonable (should be ~100ms hold + ramp time)
            if let Some(freeze_duration) = metrics.spring_freeze_duration {
                if freeze_duration < Duration::from_millis(50) || freeze_duration > Duration::from_millis(500) {
                    return Err(format!(
                        "Spring freeze duration {} ms is outside expected range (50-500ms)",
                        freeze_duration.as_millis()
                    ));
                }
            }

            Ok(())
        })();

        TrimValidationResult {
            name: "Spring Freeze Behavior".to_string(),
            passed: result.is_ok(),
            duration: start_time.elapsed(),
            error: result.err(),
            measurements,
            metrics,
        }
    }

    /// Test spring ramp behavior for gradual re-enable
    fn test_spring_ramp_behavior(&mut self) -> TrimValidationResult {
        let start_time = Instant::now();
        let mut measurements = Vec::new();
        let mut metrics = TrimValidationMetrics::default();

        let result = (|| -> Result<(), String> {
            let mut controller = TrimController::new(15.0);
            controller.set_mode(TrimMode::SpringCentered);

            let change = SetpointChange {
                target_nm: 4.0,
                limits: TrimLimits::default(),
            };

            controller.apply_setpoint_change(change)
                .map_err(|e| format!("Failed to apply setpoint change: {}", e))?;

            // Wait for freeze period to end and ramp to start
            std::thread::sleep(Duration::from_millis(150));

            let mut ramp_started = false;
            let mut ramp_completed = false;
            let mut ramp_start_time = None;
            let mut observed_strengths = Vec::new();
            
            for _ in 0..500 {
                let state = controller.get_trim_state();
                
                if state.spring_ramping && !ramp_started {
                    ramp_started = true;
                    ramp_start_time = Some(Instant::now());
                } else if !state.spring_ramping && ramp_started && !ramp_completed {
                    ramp_completed = true;
                    if let Some(start) = ramp_start_time {
                        metrics.spring_ramp_duration = Some(start.elapsed());
                    }
                }

                let output = controller.update();
                if let TrimOutput::SpringCentered { config, .. } = output {
                    observed_strengths.push(config.strength);
                    measurements.push(config.strength);
                }
                
                std::thread::sleep(Duration::from_millis(10));
                
                if start_time.elapsed() > self.config.max_test_duration {
                    break;
                }
            }

            if !ramp_started {
                return Err("Spring ramp should have started".to_string());
            }

            if !ramp_completed {
                return Err("Spring ramp should have completed".to_string());
            }

            // Verify ramp shows gradual strength increase
            if observed_strengths.len() > 10 {
                let first_strength = observed_strengths[0];
                let last_strength = observed_strengths[observed_strengths.len() - 1];
                
                if last_strength <= first_strength {
                    return Err("Spring strength should increase during ramp".to_string());
                }
            }

            Ok(())
        })();

        TrimValidationResult {
            name: "Spring Ramp Behavior".to_string(),
            passed: result.is_ok(),
            duration: start_time.elapsed(),
            error: result.err(),
            measurements,
            metrics,
        }
    }

    /// Test spring center position mapping accuracy
    fn test_spring_center_mapping(&mut self) -> TrimValidationResult {
        let start_time = Instant::now();
        let mut measurements = Vec::new();
        let mut metrics = TrimValidationMetrics::default();

        let result = (|| -> Result<(), String> {
            let mut controller = TrimController::new(15.0);
            controller.set_mode(TrimMode::SpringCentered);

            // Test various setpoints and verify center mapping
            let test_cases = vec![
                (0.0, 0.0),     // Zero torque -> center
                (7.5, 0.5),    // Half max -> 0.5 center
                (-7.5, -0.5),  // Negative half -> -0.5 center
                (15.0, 1.0),   // Max torque -> 1.0 center
                (-15.0, -1.0), // Min torque -> -1.0 center
                (3.75, 0.25),  // Quarter max -> 0.25 center
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

        TrimValidationResult {
            name: "Spring Center Mapping".to_string(),
            passed: result.is_ok(),
            duration: start_time.elapsed(),
            error: result.err(),
            measurements,
            metrics,
        }
    }

    /// Test non-FFB recentre illusion implementation
    fn test_spring_recentre_illusion(&mut self) -> TrimValidationResult {
        let start_time = Instant::now();
        let mut measurements = Vec::new();
        let mut metrics = TrimValidationMetrics::default();

        let result = (|| -> Result<(), String> {
            let mut controller = TrimController::new(15.0);
            controller.set_mode(TrimMode::SpringCentered);

            // Set initial spring configuration
            let initial_config = SpringConfig {
                strength: 0.8,
                center: 0.0,
                deadband: 0.05,
            };
            controller.set_spring_config(initial_config.clone());

            // Apply trim change to simulate user input
            let change = SetpointChange {
                target_nm: 5.0, // Should map to 0.33 center position
                limits: TrimLimits::default(),
            };

            controller.apply_setpoint_change(change)
                .map_err(|e| format!("Failed to apply setpoint change: {}", e))?;

            // Phase 1: Verify spring freeze (recentre illusion)
            let output = controller.update();
            if let TrimOutput::SpringCentered { frozen, config } = output {
                if !frozen {
                    return Err("Spring should be frozen during recentre illusion".to_string());
                }
                
                // Center should be updated but spring frozen
                let expected_center = 5.0 / 15.0; // 0.333...
                if (config.center - expected_center).abs() > self.config.fp_tolerance * 10.0 {
                    return Err(format!(
                        "Center not updated correctly: expected {}, got {}",
                        expected_center, config.center
                    ));
                }
                
                measurements.push(1.0); // Frozen phase
            } else {
                return Err("Expected SpringCentered output".to_string());
            }

            // Phase 2: Wait for ramp to start
            std::thread::sleep(Duration::from_millis(150));

            // Phase 3: Verify gradual spring re-enable
            let mut ramp_observed = false;
            for _ in 0..200 {
                let output = controller.update();
                if let TrimOutput::SpringCentered { frozen, config } = output {
                    if !frozen && config.strength < initial_config.strength {
                        ramp_observed = true;
                        measurements.push(config.strength);
                    }
                }
                
                std::thread::sleep(Duration::from_millis(5));
            }

            if !ramp_observed {
                return Err("Spring ramp not observed during recentre illusion".to_string());
            }

            Ok(())
        })();

        TrimValidationResult {
            name: "Spring Recentre Illusion".to_string(),
            passed: result.is_ok(),
            duration: start_time.elapsed(),
            error: result.err(),
            measurements,
            metrics,
        }
    }

    /// Test stability when switching between trim modes
    fn test_mode_switching_stability(&mut self) -> TrimValidationResult {
        let start_time = Instant::now();
        let mut measurements = Vec::new();
        let mut metrics = TrimValidationMetrics::default();

        let result = (|| -> Result<(), String> {
            let mut controller = TrimController::new(15.0);

            // Test switching between modes multiple times
            for cycle in 0..5 {
                // Start in FFB mode
                controller.set_mode(TrimMode::ForceFeedback);
                
                let ffb_change = SetpointChange {
                    target_nm: 3.0 + cycle as f32,
                    limits: TrimLimits::default(),
                };
                
                controller.apply_setpoint_change(ffb_change)
                    .map_err(|e| format!("FFB setpoint failed in cycle {}: {}", cycle, e))?;

                // Run a few updates
                for _ in 0..10 {
                    let output = controller.update();
                    if let TrimOutput::ForceFeedback { setpoint_nm, .. } = output {
                        measurements.push(setpoint_nm);
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }

                // Switch to spring mode
                controller.set_mode(TrimMode::SpringCentered);
                
                let spring_change = SetpointChange {
                    target_nm: 2.0 + cycle as f32,
                    limits: TrimLimits::default(),
                };
                
                controller.apply_setpoint_change(spring_change)
                    .map_err(|e| format!("Spring setpoint failed in cycle {}: {}", cycle, e))?;

                // Run a few updates
                for _ in 0..10 {
                    let output = controller.update();
                    if let TrimOutput::SpringCentered { config, .. } = output {
                        measurements.push(config.center);
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
            }

            // Verify no NaN or infinite values in measurements
            for (i, &value) in measurements.iter().enumerate() {
                if !value.is_finite() {
                    return Err(format!("Non-finite value at measurement {}: {}", i, value));
                }
            }

            Ok(())
        })();

        TrimValidationResult {
            name: "Mode Switching Stability".to_string(),
            passed: result.is_ok(),
            duration: start_time.elapsed(),
            error: result.err(),
            measurements,
            metrics,
        }
    }

    /// Test handling of extreme setpoint values
    fn test_extreme_setpoint_handling(&mut self) -> TrimValidationResult {
        let start_time = Instant::now();
        let mut measurements = Vec::new();
        let mut metrics = TrimValidationMetrics::default();

        let result = (|| -> Result<(), String> {
            let mut controller = TrimController::new(15.0);
            controller.set_mode(TrimMode::ForceFeedback);

            // Test extreme values
            let extreme_cases = vec![
                (20.0, true),   // Above device limit - should fail
                (-20.0, true),  // Below device limit - should fail
                (15.0, false),  // At device limit - should succeed
                (-15.0, false), // At negative device limit - should succeed
                (0.0, false),   // Zero - should succeed
            ];

            for (target_nm, should_fail) in extreme_cases {
                let change = SetpointChange {
                    target_nm,
                    limits: TrimLimits::default(),
                };

                let result = controller.apply_setpoint_change(change);
                
                if should_fail && result.is_ok() {
                    return Err(format!("Expected failure for setpoint {} Nm", target_nm));
                } else if !should_fail && result.is_err() {
                    return Err(format!("Unexpected failure for setpoint {} Nm: {:?}", target_nm, result));
                }

                if result.is_ok() {
                    // Run a few updates to verify stability
                    for _ in 0..10 {
                        let output = controller.update();
                        if let TrimOutput::ForceFeedback { setpoint_nm, .. } = output {
                            measurements.push(setpoint_nm);
                            
                            // Verify output is within device limits
                            if setpoint_nm.abs() > 15.0 + self.config.fp_tolerance {
                                return Err(format!(
                                    "Output {} Nm exceeds device limit 15.0 Nm",
                                    setpoint_nm
                                ));
                            }
                        }
                        std::thread::sleep(Duration::from_millis(1));
                    }
                }
            }

            Ok(())
        })();

        TrimValidationResult {
            name: "Extreme Setpoint Handling".to_string(),
            passed: result.is_ok(),
            duration: start_time.elapsed(),
            error: result.err(),
            measurements,
            metrics,
        }
    }

    /// Test replay reproducibility of trim behavior
    pub fn test_replay_reproducibility(&mut self) -> TrimValidationResult {
        let start_time = Instant::now();
        let mut measurements = Vec::new();
        let mut metrics = TrimValidationMetrics::default();

        let result = (|| -> Result<(), String> {
            // Record first run
            let first_run = self.record_trim_sequence()?;
            
            // Record second run with identical inputs
            let second_run = self.record_trim_sequence()?;
            
            // Compare outputs for reproducibility
            if first_run.len() != second_run.len() {
                return Err(format!(
                    "Run length mismatch: {} vs {} samples",
                    first_run.len(), second_run.len()
                ));
            }

            let mut max_difference = 0.0f32;
            for (i, (&first, &second)) in first_run.iter().zip(second_run.iter()).enumerate() {
                let difference = (first - second).abs();
                max_difference = max_difference.max(difference);
                measurements.push(difference);
                
                // Check reproducibility tolerance
                if difference > self.config.fp_tolerance * 1000.0 {
                    return Err(format!(
                        "Reproducibility error at sample {}: {} vs {} (diff: {})",
                        i, first, second, difference
                    ));
                }
            }

            if self.config.verbose_logging {
                println!("Replay reproducibility: max difference = {} Nm", max_difference);
            }

            Ok(())
        })();

        TrimValidationResult {
            name: "Replay Reproducibility".to_string(),
            passed: result.is_ok(),
            duration: start_time.elapsed(),
            error: result.err(),
            measurements,
            metrics,
        }
    }

    /// Record a deterministic trim sequence for replay testing
    fn record_trim_sequence(&mut self) -> Result<Vec<f32>, String> {
        let mut controller = TrimController::new(15.0);
        controller.set_mode(TrimMode::ForceFeedback);
        
        let mut outputs = Vec::new();

        // Apply a series of setpoint changes
        let setpoints = vec![5.0, -3.0, 8.0, 0.0, -7.0];
        
        for &target in &setpoints {
            let change = SetpointChange {
                target_nm: target,
                limits: TrimLimits {
                    max_rate_nm_per_s: 4.0,
                    max_jerk_nm_per_s2: 15.0,
                },
            };

            controller.apply_setpoint_change(change)
                .map_err(|e| format!("Failed to apply setpoint {}: {}", target, e))?;

            // Record outputs for a fixed number of steps
            for _ in 0..100 {
                let output = controller.update();
                if let TrimOutput::ForceFeedback { setpoint_nm, .. } = output {
                    outputs.push(setpoint_nm);
                }
                // Use fixed time step for deterministic replay
                std::thread::sleep(Duration::from_millis(1));
            }
        }

        Ok(outputs)
    }

    /// Generate comprehensive validation report
    pub fn generate_validation_report(&self, results: &[TrimValidationResult]) -> String {
        let mut report = String::new();
        report.push_str("# Trim Correctness Validation Report\n\n");

        let total_tests = results.len();
        let passed_tests = results.iter().filter(|r| r.passed).count();
        let failed_tests = total_tests - passed_tests;

        report.push_str("## Executive Summary\n\n");
        report.push_str(&format!("- **Total Tests**: {}\n", total_tests));
        report.push_str(&format!("- **Passed**: {} ({}%)\n", passed_tests, 
            (passed_tests as f32 / total_tests as f32 * 100.0) as u32));
        report.push_str(&format!("- **Failed**: {} ({}%)\n", failed_tests,
            (failed_tests as f32 / total_tests as f32 * 100.0) as u32));
        
        let overall_status = if failed_tests == 0 { "✅ PASS" } else { "❌ FAIL" };
        report.push_str(&format!("- **Overall Status**: {}\n\n", overall_status));

        report.push_str("## Test Categories\n\n");
        
        let ffb_tests: Vec<_> = results.iter().filter(|r| r.name.starts_with("FFB")).collect();
        let spring_tests: Vec<_> = results.iter().filter(|r| r.name.starts_with("Spring")).collect();
        let other_tests: Vec<_> = results.iter().filter(|r| 
            !r.name.starts_with("FFB") && !r.name.starts_with("Spring")).collect();

        report.push_str(&format!("### FFB Tests ({} tests)\n", ffb_tests.len()));
        let ffb_passed = ffb_tests.iter().filter(|r| r.passed).count();
        report.push_str(&format!("- Passed: {}/{}\n", ffb_passed, ffb_tests.len()));
        
        report.push_str(&format!("\n### Spring Tests ({} tests)\n", spring_tests.len()));
        let spring_passed = spring_tests.iter().filter(|r| r.passed).count();
        report.push_str(&format!("- Passed: {}/{}\n", spring_passed, spring_tests.len()));
        
        report.push_str(&format!("\n### Other Tests ({} tests)\n", other_tests.len()));
        let other_passed = other_tests.iter().filter(|r| r.passed).count();
        report.push_str(&format!("- Passed: {}/{}\n\n", other_passed, other_tests.len()));

        report.push_str("## Detailed Results\n\n");
        
        for result in results {
            let status = if result.passed { "✅ PASS" } else { "❌ FAIL" };
            report.push_str(&format!("### {} - {}\n\n", status, result.name));
            
            report.push_str(&format!("- **Duration**: {:.2}ms\n", 
                result.duration.as_secs_f32() * 1000.0));
            
            if let Some(error) = &result.error {
                report.push_str(&format!("- **Error**: {}\n", error));
            }
            
            if !result.measurements.is_empty() {
                let count = result.measurements.len();
                let avg = result.measurements.iter().sum::<f32>() / count as f32;
                let max = result.measurements.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
                let min = result.measurements.iter().fold(f32::INFINITY, |a, &b| a.min(b));
                
                report.push_str(&format!("- **Measurements**: {} samples\n", count));
                report.push_str(&format!("  - Average: {:.6}\n", avg));
                report.push_str(&format!("  - Min: {:.6}\n", min));
                report.push_str(&format!("  - Max: {:.6}\n", max));
            }

            // Add metrics if available
            let metrics = &result.metrics;
            if metrics.max_rate_nm_per_s > 0.0 {
                report.push_str(&format!("- **Max Rate**: {:.3} Nm/s\n", metrics.max_rate_nm_per_s));
            }
            if metrics.max_jerk_nm_per_s2 > 0.0 {
                report.push_str(&format!("- **Max Jerk**: {:.3} Nm/s²\n", metrics.max_jerk_nm_per_s2));
            }
            if let Some(convergence_time) = metrics.convergence_time {
                report.push_str(&format!("- **Convergence Time**: {:.1}ms\n", 
                    convergence_time.as_secs_f32() * 1000.0));
            }
            if metrics.torque_steps_detected > 0 {
                report.push_str(&format!("- **Torque Steps**: {}\n", metrics.torque_steps_detected));
            }
            
            report.push_str("\n");
        }

        report.push_str("## Requirements Compliance\n\n");
        report.push_str("This validation suite verifies compliance with:\n\n");
        report.push_str("- **FFB-01**: Force feedback safety and control\n");
        report.push_str("  - Rate/jerk limiting prevents torque steps\n");
        report.push_str("  - Setpoint changes are smooth and controlled\n");
        report.push_str("  - No overshoot beyond acceptable limits\n\n");
        report.push_str("- **Trim Correctness**: Non-FFB recentre illusion\n");
        report.push_str("  - Spring freeze during trim hold\n");
        report.push_str("  - Gradual spring re-enable with ramp\n");
        report.push_str("  - Accurate center position mapping\n\n");
        report.push_str("- **Replay Reproducibility**: Deterministic behavior\n");
        report.push_str("  - Identical inputs produce identical outputs\n");
        report.push_str("  - Floating-point precision maintained\n");
        report.push_str("  - Suitable for automated testing and validation\n\n");

        report
    }
}

impl Default for TrimValidationSuite {
    fn default() -> Self {
        Self::new(TrimValidationConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trim_validation_suite_creation() {
        let suite = TrimValidationSuite::default();
        assert_eq!(suite.config.fp_tolerance, 1e-6);
        assert_eq!(suite.config.sample_rate_hz, 1000);
    }

    #[test]
    fn test_ffb_rate_limiting_validation() {
        let mut suite = TrimValidationSuite::default();
        let result = suite.test_ffb_rate_limiting();
        
        assert!(result.passed, "FFB rate limiting test failed: {:?}", result.error);
        assert!(!result.measurements.is_empty());
        assert!(result.metrics.max_rate_nm_per_s > 0.0);
    }

    #[test]
    fn test_spring_freeze_validation() {
        let mut suite = TrimValidationSuite::default();
        let result = suite.test_spring_freeze_behavior();
        
        assert!(result.passed, "Spring freeze test failed: {:?}", result.error);
        assert!(!result.measurements.is_empty());
    }

    #[test]
    fn test_replay_reproducibility_validation() {
        let mut suite = TrimValidationSuite::default();
        let result = suite.test_replay_reproducibility();
        
        assert!(result.passed, "Replay reproducibility test failed: {:?}", result.error);
        assert!(!result.measurements.is_empty());
    }

    #[test]
    fn test_complete_validation_suite() {
        let mut suite = TrimValidationSuite::default();
        let results = suite.run_complete_validation();
        
        assert!(!results.is_empty());
        
        // All tests should pass
        for result in &results {
            if !result.passed {
                println!("Failed test: {} - {:?}", result.name, result.error);
            }
        }
        
        let failed_count = results.iter().filter(|r| !r.passed).count();
        assert_eq!(failed_count, 0, "Some validation tests failed");
    }

    #[test]
    fn test_validation_report_generation() {
        let mut suite = TrimValidationSuite::default();
        
        let mock_results = vec![
            TrimValidationResult {
                name: "FFB Test 1".to_string(),
                passed: true,
                duration: Duration::from_millis(100),
                error: None,
                measurements: vec![1.0, 2.0, 3.0],
                metrics: TrimValidationMetrics::default(),
            },
            TrimValidationResult {
                name: "Spring Test 1".to_string(),
                passed: false,
                duration: Duration::from_millis(50),
                error: Some("Mock error".to_string()),
                measurements: vec![],
                metrics: TrimValidationMetrics::default(),
            },
        ];
        
        let report = suite.generate_validation_report(&mock_results);
        
        assert!(report.contains("Total Tests: 2"));
        assert!(report.contains("Passed: 1"));
        assert!(report.contains("Failed: 1"));
        assert!(report.contains("✅ PASS"));
        assert!(report.contains("❌ FAIL"));
        assert!(report.contains("Mock error"));
        assert!(report.contains("Requirements Compliance"));
    }
}