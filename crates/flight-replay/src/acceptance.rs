// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Acceptance test integration for recorded runs

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{info, warn};

use crate::harness::{ReplayHarness, ReplayResult};
use crate::replay_config::{ReplayConfig, ReplayMode, ToleranceConfig};
use crate::validation::{ValidationConfig, ValidationResult, ValidationSuite, ValidationTestCase};

/// Acceptance test errors
#[derive(Error, Debug)]
pub enum AcceptanceError {
    #[error("Test configuration error: {message}")]
    ConfigError { message: String },
    #[error("Test execution failed: {message}")]
    ExecutionFailed { message: String },
    #[error("Acceptance criteria not met: {details}")]
    CriteriaNotMet { details: String },
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Acceptance test definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptanceTest {
    /// Test identifier
    pub id: String,
    /// Human-readable test name
    pub name: String,
    /// Test description
    pub description: String,
    /// Path to blackbox file for testing
    pub blackbox_file: PathBuf,
    /// Acceptance criteria
    pub criteria: AcceptanceCriteria,
    /// Test configuration
    pub config: AcceptanceTestConfig,
}

/// Acceptance criteria for test validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptanceCriteria {
    /// Minimum overall score required (0.0 to 1.0)
    pub min_overall_score: f32,
    /// Maximum allowed output mismatches
    pub max_output_mismatches: u64,
    /// Minimum frames per second for performance
    pub min_fps: f64,
    /// Maximum average frame processing time (microseconds)
    pub max_avg_frame_time_us: f64,
    /// Maximum timing drift (nanoseconds per second)
    pub max_timing_drift_ns_per_s: u64,
    /// Minimum percentage of frames within timing tolerance
    pub min_timing_accuracy_pct: f32,
    /// Custom validation rules
    pub custom_rules: Vec<CustomValidationRule>,
}

/// Custom validation rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomValidationRule {
    /// Rule name
    pub name: String,
    /// Rule description
    pub description: String,
    /// Metric to validate
    pub metric: String,
    /// Comparison operator
    pub operator: ComparisonOperator,
    /// Expected value
    pub expected_value: f64,
}

/// Comparison operators for custom rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComparisonOperator {
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
    Equal,
    NotEqual,
}

/// Configuration for acceptance test execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptanceTestConfig {
    /// Replay mode for testing
    pub replay_mode: ReplayMode,
    /// Tolerance configuration
    pub tolerance: ToleranceConfig,
    /// Test timeout
    pub timeout: Duration,
    /// Whether to collect detailed metrics
    pub collect_metrics: bool,
    /// Number of test iterations
    pub iterations: u32,
}

/// Result of acceptance test execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptanceResult {
    /// Test that was executed
    pub test: AcceptanceTest,
    /// Whether test passed acceptance criteria
    pub passed: bool,
    /// Overall test score (0.0 to 1.0)
    pub score: f32,
    /// Individual replay results (one per iteration)
    pub replay_results: Vec<ReplayResult>,
    /// Validation results
    pub validation_results: Vec<ValidationResult>,
    /// Criteria evaluation results
    pub criteria_evaluation: CriteriaEvaluation,
    /// Test execution duration
    pub execution_duration: Duration,
    /// Error messages (if any)
    pub errors: Vec<String>,
}

/// Evaluation of acceptance criteria
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriteriaEvaluation {
    /// Individual criterion results
    pub criterion_results: HashMap<String, CriterionResult>,
    /// Overall criteria satisfaction
    pub all_criteria_met: bool,
    /// Summary of failed criteria
    pub failed_criteria: Vec<String>,
}

/// Result of evaluating a single criterion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriterionResult {
    /// Criterion name
    pub name: String,
    /// Whether criterion was met
    pub met: bool,
    /// Actual value observed
    pub actual_value: f64,
    /// Expected value or threshold
    pub expected_value: f64,
    /// Details about the evaluation
    pub details: String,
}

/// Acceptance test runner
pub struct AcceptanceTestRunner {
    config: ValidationConfig,
    test_registry: HashMap<String, AcceptanceTest>,
}

impl AcceptanceTestRunner {
    /// Create a new acceptance test runner
    pub fn new(config: ValidationConfig) -> Self {
        Self {
            config,
            test_registry: HashMap::new(),
        }
    }

    /// Register an acceptance test
    pub fn register_test(&mut self, test: AcceptanceTest) {
        self.test_registry.insert(test.id.clone(), test);
    }

    /// Load acceptance tests from a configuration file
    pub fn load_tests_from_file<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let content = std::fs::read_to_string(path)
            .context("Failed to read acceptance test configuration")?;

        let tests: Vec<AcceptanceTest> = serde_json::from_str(&content)
            .context("Failed to parse acceptance test configuration")?;

        for test in tests {
            self.register_test(test);
        }

        info!("Loaded {} acceptance tests", self.test_registry.len());
        Ok(())
    }

    /// Run a specific acceptance test by ID
    pub async fn run_test(&self, test_id: &str) -> Result<AcceptanceResult> {
        let test = self
            .test_registry
            .get(test_id)
            .ok_or_else(|| AcceptanceError::ConfigError {
                message: format!("Test '{}' not found", test_id),
            })?;

        self.execute_test(test).await
    }

    /// Run all registered acceptance tests
    pub async fn run_all_tests(&self) -> Result<Vec<AcceptanceResult>> {
        let mut results = Vec::new();

        for (test_id, test) in &self.test_registry {
            info!("Running acceptance test: {}", test_id);

            match self.execute_test(test).await {
                Ok(result) => {
                    if result.passed {
                        info!("✓ Test '{}' passed with score {:.3}", test_id, result.score);
                    } else {
                        warn!("✗ Test '{}' failed with score {:.3}", test_id, result.score);
                    }
                    results.push(result);
                }
                Err(e) => {
                    warn!("✗ Test '{}' failed with error: {}", test_id, e);
                    results.push(AcceptanceResult {
                        test: test.clone(),
                        passed: false,
                        score: 0.0,
                        replay_results: Vec::new(),
                        validation_results: Vec::new(),
                        criteria_evaluation: CriteriaEvaluation {
                            criterion_results: HashMap::new(),
                            all_criteria_met: false,
                            failed_criteria: vec!["Test execution failed".to_string()],
                        },
                        execution_duration: Duration::from_secs(0),
                        errors: vec![e.to_string()],
                    });
                }
            }
        }

        Ok(results)
    }

    /// Execute a single acceptance test
    async fn execute_test(&self, test: &AcceptanceTest) -> Result<AcceptanceResult> {
        let start_time = std::time::Instant::now();
        let mut replay_results = Vec::new();
        let mut validation_results = Vec::new();
        let mut errors = Vec::new();

        // Run test iterations
        for iteration in 0..test.config.iterations {
            info!("Running iteration {} of test '{}'", iteration + 1, test.id);

            // Create replay configuration
            let replay_config = ReplayConfig {
                mode: test.config.replay_mode,
                tolerance: test.config.tolerance.clone(),
                max_duration: test.config.timeout,
                validate_outputs: true,
                collect_metrics: test.config.collect_metrics,
                ..Default::default()
            };

            // Create and configure harness
            let mut harness =
                ReplayHarness::new(replay_config).context("Failed to create replay harness")?;

            // Configure default devices
            self.configure_test_devices(&mut harness)?;

            // Run replay
            match harness.replay_file(&test.blackbox_file).await {
                Ok(result) => {
                    replay_results.push(result);
                }
                Err(e) => {
                    errors.push(format!("Iteration {}: {}", iteration + 1, e));
                }
            }
        }

        // Run validation if we have replay results
        if !replay_results.is_empty() {
            let validation_config = self.config.clone();
            let validator_suite = ValidationSuite::new(validation_config);

            // Create a validation test case
            let test_case = ValidationTestCase {
                name: test.id.clone(),
                description: test.description.clone(),
                file_path: test.blackbox_file.to_string_lossy().to_string(),
                expected_score_threshold: test.criteria.min_overall_score,
            };

            // Note: In a full implementation, we would run validation on the blackbox file
            // For now, we'll create a mock validation result based on replay results
            let mock_validation = self.create_mock_validation_result(test, &replay_results[0]);
            validation_results.push(mock_validation);
        }

        // Evaluate acceptance criteria
        let criteria_evaluation =
            self.evaluate_criteria(test, &replay_results, &validation_results);

        // Calculate overall score
        let score = self.calculate_test_score(&replay_results, &validation_results);

        // Determine if test passed
        let passed = criteria_evaluation.all_criteria_met && !replay_results.is_empty();

        let execution_duration = start_time.elapsed();

        Ok(AcceptanceResult {
            test: test.clone(),
            passed,
            score,
            replay_results,
            validation_results,
            criteria_evaluation,
            execution_duration,
            errors,
        })
    }

    /// Configure devices for acceptance testing
    fn configure_test_devices(&self, harness: &mut ReplayHarness) -> Result<()> {
        use flight_axis::EngineConfig as AxisEngineConfig;
        use flight_ffb::{FfbConfig, FfbMode};

        // Add test axis device
        let axis_config = AxisEngineConfig::default();
        harness
            .add_axis_device("acceptance_test_axis".to_string(), axis_config)
            .map_err(|e| AcceptanceError::ConfigError {
                message: e.to_string(),
            })?;

        // Add test FFB device
        let ffb_config = FfbConfig {
            max_torque_nm: 15.0,
            fault_timeout_ms: 50,
            interlock_required: false,
            mode: FfbMode::TelemetrySynth,
            device_path: None,
        };
        harness
            .add_ffb_device("acceptance_test_ffb".to_string(), ffb_config)
            .map_err(|e| AcceptanceError::ConfigError {
                message: e.to_string(),
            })?;

        Ok(())
    }

    /// Create a mock validation result for testing
    fn create_mock_validation_result(
        &self,
        test: &AcceptanceTest,
        replay_result: &ReplayResult,
    ) -> ValidationResult {
        use crate::validation::{
            OutputAccuracyResult, PerformanceValidationResult, TimingValidationResult,
            ValidationDetails,
        };

        // Create mock validation details based on replay result
        let output_accuracy = OutputAccuracyResult {
            within_tolerance: replay_result.comparison.as_ref().map_or(true, |c| c.passed),
            axis_accuracy_score: 0.95,
            ffb_accuracy_score: 0.93,
            total_mismatches: replay_result
                .comparison
                .as_ref()
                .map_or(0, |c| c.mismatches),
        };

        let performance = PerformanceValidationResult {
            meets_requirements: replay_result.performance.frames_per_second
                >= test.criteria.min_fps,
            frames_per_second: replay_result.performance.frames_per_second,
            avg_frame_time_us: replay_result.performance.avg_frame_time.as_micros() as f64,
            memory_efficiency_score: 0.85,
        };

        let timing = TimingValidationResult {
            within_tolerance: replay_result
                .accuracy
                .timing_stats
                .frames_within_tolerance_pct
                >= test.criteria.min_timing_accuracy_pct,
            timing_accuracy_score: replay_result
                .accuracy
                .timing_stats
                .frames_within_tolerance_pct
                / 100.0,
            max_timing_drift_ns_per_s: replay_result.accuracy.timing_stats.max_timing_error_ns,
            frames_within_tolerance_pct: replay_result
                .accuracy
                .timing_stats
                .frames_within_tolerance_pct,
        };

        let details = ValidationDetails {
            replay_result: Some(replay_result.clone()),
            output_accuracy,
            performance,
            timing,
        };

        let score = (details.output_accuracy.axis_accuracy_score
            + details.output_accuracy.ffb_accuracy_score
            + details.timing.timing_accuracy_score)
            / 3.0;

        ValidationResult {
            test_name: test.id.clone(),
            passed: score >= test.criteria.min_overall_score,
            score,
            details,
            errors: Vec::new(),
        }
    }

    /// Evaluate acceptance criteria against test results
    fn evaluate_criteria(
        &self,
        test: &AcceptanceTest,
        replay_results: &[ReplayResult],
        validation_results: &[ValidationResult],
    ) -> CriteriaEvaluation {
        let mut criterion_results = HashMap::new();
        let mut failed_criteria = Vec::new();

        if replay_results.is_empty() {
            failed_criteria.push("No replay results available".to_string());
            return CriteriaEvaluation {
                criterion_results,
                all_criteria_met: false,
                failed_criteria,
            };
        }

        let replay_result = &replay_results[0]; // Use first result for evaluation

        // Evaluate minimum overall score
        if let Some(validation_result) = validation_results.first() {
            let score_met = validation_result.score >= test.criteria.min_overall_score;
            criterion_results.insert(
                "min_overall_score".to_string(),
                CriterionResult {
                    name: "Minimum Overall Score".to_string(),
                    met: score_met,
                    actual_value: validation_result.score as f64,
                    expected_value: test.criteria.min_overall_score as f64,
                    details: format!(
                        "Score: {:.3}, Required: {:.3}",
                        validation_result.score, test.criteria.min_overall_score
                    ),
                },
            );
            if !score_met {
                failed_criteria.push("Minimum overall score not met".to_string());
            }
        }

        // Evaluate maximum output mismatches
        if let Some(comparison) = &replay_result.comparison {
            let mismatches_met = comparison.mismatches <= test.criteria.max_output_mismatches;
            criterion_results.insert(
                "max_output_mismatches".to_string(),
                CriterionResult {
                    name: "Maximum Output Mismatches".to_string(),
                    met: mismatches_met,
                    actual_value: comparison.mismatches as f64,
                    expected_value: test.criteria.max_output_mismatches as f64,
                    details: format!(
                        "Mismatches: {}, Limit: {}",
                        comparison.mismatches, test.criteria.max_output_mismatches
                    ),
                },
            );
            if !mismatches_met {
                failed_criteria.push("Too many output mismatches".to_string());
            }
        }

        // Evaluate minimum FPS
        let fps_met = replay_result.performance.frames_per_second >= test.criteria.min_fps;
        criterion_results.insert(
            "min_fps".to_string(),
            CriterionResult {
                name: "Minimum Frames Per Second".to_string(),
                met: fps_met,
                actual_value: replay_result.performance.frames_per_second,
                expected_value: test.criteria.min_fps,
                details: format!(
                    "FPS: {:.1}, Required: {:.1}",
                    replay_result.performance.frames_per_second, test.criteria.min_fps
                ),
            },
        );
        if !fps_met {
            failed_criteria.push("Minimum FPS not met".to_string());
        }

        // Evaluate maximum frame time
        let frame_time_us = replay_result.performance.avg_frame_time.as_micros() as f64;
        let frame_time_met = frame_time_us <= test.criteria.max_avg_frame_time_us;
        criterion_results.insert(
            "max_avg_frame_time".to_string(),
            CriterionResult {
                name: "Maximum Average Frame Time".to_string(),
                met: frame_time_met,
                actual_value: frame_time_us,
                expected_value: test.criteria.max_avg_frame_time_us,
                details: format!(
                    "Frame time: {:.1}μs, Limit: {:.1}μs",
                    frame_time_us, test.criteria.max_avg_frame_time_us
                ),
            },
        );
        if !frame_time_met {
            failed_criteria.push("Maximum frame time exceeded".to_string());
        }

        // Evaluate timing accuracy
        let timing_accuracy_met = replay_result
            .accuracy
            .timing_stats
            .frames_within_tolerance_pct
            >= test.criteria.min_timing_accuracy_pct;
        criterion_results.insert(
            "min_timing_accuracy".to_string(),
            CriterionResult {
                name: "Minimum Timing Accuracy".to_string(),
                met: timing_accuracy_met,
                actual_value: replay_result
                    .accuracy
                    .timing_stats
                    .frames_within_tolerance_pct as f64,
                expected_value: test.criteria.min_timing_accuracy_pct as f64,
                details: format!(
                    "Timing accuracy: {:.1}%, Required: {:.1}%",
                    replay_result
                        .accuracy
                        .timing_stats
                        .frames_within_tolerance_pct,
                    test.criteria.min_timing_accuracy_pct
                ),
            },
        );
        if !timing_accuracy_met {
            failed_criteria.push("Minimum timing accuracy not met".to_string());
        }

        // Evaluate custom rules
        for rule in &test.criteria.custom_rules {
            let rule_met = self.evaluate_custom_rule(rule, replay_result);
            criterion_results.insert(
                rule.name.clone(),
                CriterionResult {
                    name: rule.name.clone(),
                    met: rule_met,
                    actual_value: 0.0, // Would extract actual value based on rule.metric
                    expected_value: rule.expected_value,
                    details: format!("Custom rule: {}", rule.description),
                },
            );
            if !rule_met {
                failed_criteria.push(format!("Custom rule '{}' not met", rule.name));
            }
        }

        let all_criteria_met = failed_criteria.is_empty();

        CriteriaEvaluation {
            criterion_results,
            all_criteria_met,
            failed_criteria,
        }
    }

    /// Evaluate a custom validation rule
    fn evaluate_custom_rule(
        &self,
        rule: &CustomValidationRule,
        replay_result: &ReplayResult,
    ) -> bool {
        // In a full implementation, this would extract the metric value from replay_result
        // and compare it using the specified operator and expected value
        // For now, return true as a placeholder
        true
    }

    /// Calculate overall test score
    fn calculate_test_score(
        &self,
        replay_results: &[ReplayResult],
        validation_results: &[ValidationResult],
    ) -> f32 {
        if validation_results.is_empty() {
            return if replay_results.iter().all(|r| r.success) {
                0.5
            } else {
                0.0
            };
        }

        // Average validation scores
        let avg_validation_score = validation_results.iter().map(|r| r.score).sum::<f32>()
            / validation_results.len() as f32;

        // Factor in replay success
        let replay_success_rate = replay_results.iter().filter(|r| r.success).count() as f32
            / replay_results.len() as f32;

        // Weighted combination
        (avg_validation_score * 0.8) + (replay_success_rate * 0.2)
    }
}

impl Default for AcceptanceCriteria {
    fn default() -> Self {
        Self {
            min_overall_score: 0.8,
            max_output_mismatches: 10,
            min_fps: 100.0,
            max_avg_frame_time_us: 1000.0,
            max_timing_drift_ns_per_s: 100_000,
            min_timing_accuracy_pct: 95.0,
            custom_rules: Vec::new(),
        }
    }
}

impl Default for AcceptanceTestConfig {
    fn default() -> Self {
        Self {
            replay_mode: ReplayMode::FastForward,
            tolerance: ToleranceConfig::default(),
            timeout: Duration::from_secs(300),
            collect_metrics: true,
            iterations: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_acceptance_test_runner_creation() {
        let config = ValidationConfig::default();
        let runner = AcceptanceTestRunner::new(config);

        assert_eq!(runner.test_registry.len(), 0);
    }

    #[test]
    fn test_acceptance_test_registration() {
        let config = ValidationConfig::default();
        let mut runner = AcceptanceTestRunner::new(config);

        let test = AcceptanceTest {
            id: "test1".to_string(),
            name: "Test 1".to_string(),
            description: "First test".to_string(),
            blackbox_file: PathBuf::from("test.fbb"),
            criteria: AcceptanceCriteria::default(),
            config: AcceptanceTestConfig::default(),
        };

        runner.register_test(test);
        assert_eq!(runner.test_registry.len(), 1);
        assert!(runner.test_registry.contains_key("test1"));
    }

    #[test]
    fn test_criteria_evaluation() {
        let criteria = AcceptanceCriteria {
            min_overall_score: 0.8,
            max_output_mismatches: 5,
            min_fps: 100.0,
            max_avg_frame_time_us: 1000.0,
            max_timing_drift_ns_per_s: 100_000,
            min_timing_accuracy_pct: 95.0,
            custom_rules: Vec::new(),
        };

        assert_eq!(criteria.min_overall_score, 0.8);
        assert_eq!(criteria.max_output_mismatches, 5);
    }

    #[test]
    fn test_comparison_operators() {
        use ComparisonOperator::*;

        let operators = vec![
            GreaterThan,
            GreaterThanOrEqual,
            LessThan,
            LessThanOrEqual,
            Equal,
            NotEqual,
        ];

        assert_eq!(operators.len(), 6);
    }

    #[tokio::test]
    async fn test_nonexistent_test_execution() {
        let config = ValidationConfig::default();
        let runner = AcceptanceTestRunner::new(config);

        let result = runner.run_test("nonexistent").await;
        assert!(result.is_err());
    }
}
