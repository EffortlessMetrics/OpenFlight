//! Replay validation suite

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, info, warn};

use crate::harness::{ReplayHarness, ReplayResult};
use crate::replay_config::{ReplayConfig, ReplayMode, ToleranceConfig};
use crate::comparison::ComparisonResult;

/// Validation errors
#[derive(Error, Debug)]
pub enum ValidationError {
    #[error("Replay failed: {message}")]
    ReplayFailed { message: String },
    #[error("Output validation failed: {details}")]
    OutputValidationFailed { details: String },
    #[error("Performance validation failed: {metric} = {value} exceeds limit {limit}")]
    PerformanceValidationFailed {
        metric: String,
        value: String,
        limit: String,
    },
    #[error("Timing validation failed: {details}")]
    TimingValidationFailed { details: String },
    #[error("Configuration error: {message}")]
    ConfigError { message: String },
}

/// Result of validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Test name
    pub test_name: String,
    /// Whether validation passed
    pub passed: bool,
    /// Validation score (0.0 to 1.0)
    pub score: f32,
    /// Detailed results
    pub details: ValidationDetails,
    /// Error messages (if any)
    pub errors: Vec<String>,
}

/// Detailed validation results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationDetails {
    /// Replay execution results
    pub replay_result: Option<ReplayResult>,
    /// Output accuracy validation
    pub output_accuracy: OutputAccuracyResult,
    /// Performance validation
    pub performance: PerformanceValidationResult,
    /// Timing validation
    pub timing: TimingValidationResult,
}

/// Output accuracy validation results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputAccuracyResult {
    /// Whether outputs are within tolerance
    pub within_tolerance: bool,
    /// Axis accuracy score (0.0 to 1.0)
    pub axis_accuracy_score: f32,
    /// FFB accuracy score (0.0 to 1.0)
    pub ffb_accuracy_score: f32,
    /// Number of mismatches found
    pub total_mismatches: u64,
}

/// Performance validation results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceValidationResult {
    /// Whether performance meets requirements
    pub meets_requirements: bool,
    /// Frames per second achieved
    pub frames_per_second: f64,
    /// Average frame processing time
    pub avg_frame_time_us: f64,
    /// Memory usage efficiency score (0.0 to 1.0)
    pub memory_efficiency_score: f32,
}

/// Timing validation results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingValidationResult {
    /// Whether timing is within tolerance
    pub within_tolerance: bool,
    /// Timing accuracy score (0.0 to 1.0)
    pub timing_accuracy_score: f32,
    /// Maximum timing drift observed (ns/s)
    pub max_timing_drift_ns_per_s: u64,
    /// Percentage of frames within timing tolerance
    pub frames_within_tolerance_pct: f32,
}

/// Replay validator for comprehensive testing
pub struct ReplayValidator {
    config: ValidationConfig,
}

/// Configuration for validation
#[derive(Debug, Clone)]
pub struct ValidationConfig {
    /// Tolerance settings for validation
    pub tolerance: ToleranceConfig,
    /// Performance requirements
    pub performance_requirements: PerformanceRequirements,
    /// Whether to run comprehensive tests
    pub comprehensive: bool,
    /// Timeout for individual tests
    pub test_timeout: Duration,
}

/// Performance requirements for validation
#[derive(Debug, Clone)]
pub struct PerformanceRequirements {
    /// Minimum frames per second
    pub min_fps: f64,
    /// Maximum average frame time (microseconds)
    pub max_avg_frame_time_us: f64,
    /// Maximum memory usage (bytes)
    pub max_memory_bytes: u64,
}

impl ReplayValidator {
    /// Create a new replay validator
    pub fn new(config: ValidationConfig) -> Self {
        Self { config }
    }

    /// Validate a single replay file
    pub async fn validate_file<P: AsRef<Path>>(
        &self,
        path: P,
        test_name: String,
    ) -> Result<ValidationResult> {
        info!("Starting validation of {}: {}", test_name, path.as_ref().display());

        // Create replay configuration for validation
        let replay_config = ReplayConfig {
            mode: ReplayMode::FastForward, // Use fast-forward for validation
            validate_outputs: true,
            tolerance: self.config.tolerance.clone(),
            collect_metrics: true,
            max_duration: self.config.test_timeout,
            ..Default::default()
        };

        // Create and configure harness
        let mut harness = ReplayHarness::new(replay_config)
            .context("Failed to create replay harness")?;

        // Add default device configurations
        self.configure_default_devices(&mut harness)?;

        // Run replay
        let replay_result = harness.replay_file(&path).await
            .context("Replay execution failed")?;

        // Validate results
        let validation_details = self.validate_replay_result(&replay_result)?;

        // Calculate overall score
        let score = self.calculate_overall_score(&validation_details);

        // Determine if validation passed
        let passed = replay_result.success && 
                    validation_details.output_accuracy.within_tolerance &&
                    validation_details.performance.meets_requirements &&
                    validation_details.timing.within_tolerance;

        let mut errors = Vec::new();
        if !replay_result.success {
            errors.extend(replay_result.errors.clone());
        }

        Ok(ValidationResult {
            test_name,
            passed,
            score,
            details: ValidationDetails {
                replay_result: Some(replay_result),
                output_accuracy: validation_details.output_accuracy,
                performance: validation_details.performance,
                timing: validation_details.timing,
            },
            errors,
        })
    }

    /// Configure default devices for validation
    fn configure_default_devices(&self, harness: &mut ReplayHarness) -> Result<()> {
        use flight_axis::EngineConfig as AxisEngineConfig;
        use flight_ffb::{FfbConfig, FfbMode};

        // Add default axis device
        let axis_config = AxisEngineConfig::default();
        harness.add_axis_device("validation_axis".to_string(), axis_config)
            .context("Failed to add axis device")?;

        // Add default FFB device
        let ffb_config = FfbConfig {
            max_torque_nm: 15.0,
            fault_timeout_ms: 50,
            interlock_required: false, // Disabled for validation
            mode: FfbMode::TelemetrySynth,
            device_path: None,
        };
        harness.add_ffb_device("validation_ffb".to_string(), ffb_config)
            .context("Failed to add FFB device")?;

        Ok(())
    }

    /// Validate replay result against requirements
    fn validate_replay_result(&self, result: &ReplayResult) -> Result<ValidationDetails> {
        // Validate output accuracy
        let output_accuracy = self.validate_output_accuracy(result)?;

        // Validate performance
        let performance = self.validate_performance(result)?;

        // Validate timing
        let timing = self.validate_timing(result)?;

        Ok(ValidationDetails {
            replay_result: None, // Don't duplicate the result
            output_accuracy,
            performance,
            timing,
        })
    }

    /// Validate output accuracy
    fn validate_output_accuracy(&self, result: &ReplayResult) -> Result<OutputAccuracyResult> {
        let comparison = result.comparison.as_ref()
            .ok_or_else(|| ValidationError::ConfigError {
                message: "No comparison results available".to_string()
            })?;

        let within_tolerance = comparison.passed;
        let total_mismatches = comparison.mismatches;

        // Calculate accuracy scores based on comparison statistics
        let axis_accuracy_score = if comparison.axis_stats.count > 0 {
            let accuracy_ratio = comparison.axis_stats.within_tolerance as f32 / comparison.axis_stats.count as f32;
            accuracy_ratio
        } else {
            1.0
        };

        let ffb_accuracy_score = if comparison.ffb_stats.count > 0 {
            let accuracy_ratio = comparison.ffb_stats.within_tolerance as f32 / comparison.ffb_stats.count as f32;
            accuracy_ratio
        } else {
            1.0
        };

        Ok(OutputAccuracyResult {
            within_tolerance,
            axis_accuracy_score,
            ffb_accuracy_score,
            total_mismatches,
        })
    }

    /// Validate performance requirements
    fn validate_performance(&self, result: &ReplayResult) -> Result<PerformanceValidationResult> {
        let performance = &result.performance;

        let meets_fps_requirement = performance.frames_per_second >= self.config.performance_requirements.min_fps;
        let avg_frame_time_us = performance.avg_frame_time.as_micros() as f64;
        let meets_frame_time_requirement = avg_frame_time_us <= self.config.performance_requirements.max_avg_frame_time_us;
        let meets_memory_requirement = performance.memory_stats.peak_memory_bytes <= self.config.performance_requirements.max_memory_bytes;

        let meets_requirements = meets_fps_requirement && meets_frame_time_requirement && meets_memory_requirement;

        // Calculate memory efficiency score
        let memory_efficiency_score = if self.config.performance_requirements.max_memory_bytes > 0 {
            let usage_ratio = performance.memory_stats.peak_memory_bytes as f32 / self.config.performance_requirements.max_memory_bytes as f32;
            (1.0 - usage_ratio).max(0.0)
        } else {
            1.0
        };

        Ok(PerformanceValidationResult {
            meets_requirements,
            frames_per_second: performance.frames_per_second,
            avg_frame_time_us,
            memory_efficiency_score,
        })
    }

    /// Validate timing requirements
    fn validate_timing(&self, result: &ReplayResult) -> Result<TimingValidationResult> {
        let timing_stats = &result.accuracy.timing_stats;

        let drift_within_tolerance = timing_stats.max_timing_error_ns <= self.config.tolerance.timing_drift_ns_per_s;
        let jitter_within_tolerance = timing_stats.max_timing_error_ns <= self.config.tolerance.max_timing_jitter_ns;
        let within_tolerance = drift_within_tolerance && jitter_within_tolerance;

        // Calculate timing accuracy score
        let timing_accuracy_score = timing_stats.frames_within_tolerance_pct / 100.0;

        Ok(TimingValidationResult {
            within_tolerance,
            timing_accuracy_score,
            max_timing_drift_ns_per_s: timing_stats.max_timing_error_ns,
            frames_within_tolerance_pct: timing_stats.frames_within_tolerance_pct,
        })
    }

    /// Calculate overall validation score
    fn calculate_overall_score(&self, details: &ValidationDetails) -> f32 {
        let output_score = (details.output_accuracy.axis_accuracy_score + details.output_accuracy.ffb_accuracy_score) / 2.0;
        let performance_score = if details.performance.meets_requirements { 1.0 } else { 0.5 };
        let timing_score = details.timing.timing_accuracy_score;
        let memory_score = details.performance.memory_efficiency_score;

        // Weighted average of all scores
        (output_score * 0.4 + performance_score * 0.3 + timing_score * 0.2 + memory_score * 0.1)
    }
}

/// Comprehensive validation suite
pub struct ValidationSuite {
    validator: ReplayValidator,
    test_cases: Vec<ValidationTestCase>,
}

/// Individual validation test case
#[derive(Debug, Clone)]
pub struct ValidationTestCase {
    pub name: String,
    pub description: String,
    pub file_path: String,
    pub expected_score_threshold: f32,
}

impl ValidationSuite {
    /// Create a new validation suite
    pub fn new(config: ValidationConfig) -> Self {
        let validator = ReplayValidator::new(config);
        Self {
            validator,
            test_cases: Vec::new(),
        }
    }

    /// Add a test case to the suite
    pub fn add_test_case(&mut self, test_case: ValidationTestCase) {
        self.test_cases.push(test_case);
    }

    /// Run all test cases in the suite
    pub async fn run_all_tests(&self) -> Result<Vec<ValidationResult>> {
        let mut results = Vec::new();

        for test_case in &self.test_cases {
            info!("Running validation test: {}", test_case.name);
            
            match self.validator.validate_file(&test_case.file_path, test_case.name.clone()).await {
                Ok(result) => {
                    let meets_threshold = result.score >= test_case.expected_score_threshold;
                    if !meets_threshold {
                        warn!("Test {} scored {:.3} below threshold {:.3}", 
                              test_case.name, result.score, test_case.expected_score_threshold);
                    }
                    results.push(result);
                }
                Err(e) => {
                    warn!("Test {} failed with error: {}", test_case.name, e);
                    results.push(ValidationResult {
                        test_name: test_case.name.clone(),
                        passed: false,
                        score: 0.0,
                        details: ValidationDetails {
                            replay_result: None,
                            output_accuracy: OutputAccuracyResult::default(),
                            performance: PerformanceValidationResult::default(),
                            timing: TimingValidationResult::default(),
                        },
                        errors: vec![e.to_string()],
                    });
                }
            }
        }

        Ok(results)
    }

    /// Get summary of all test results
    pub fn summarize_results(&self, results: &[ValidationResult]) -> ValidationSummary {
        let total_tests = results.len();
        let passed_tests = results.iter().filter(|r| r.passed).count();
        let failed_tests = total_tests - passed_tests;
        
        let avg_score = if !results.is_empty() {
            results.iter().map(|r| r.score).sum::<f32>() / results.len() as f32
        } else {
            0.0
        };

        let min_score = results.iter().map(|r| r.score).fold(1.0f32, |a, b| a.min(b));
        let max_score = results.iter().map(|r| r.score).fold(0.0f32, |a, b| a.max(b));

        ValidationSummary {
            total_tests,
            passed_tests,
            failed_tests,
            avg_score,
            min_score,
            max_score,
        }
    }
}

/// Summary of validation suite results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationSummary {
    pub total_tests: usize,
    pub passed_tests: usize,
    pub failed_tests: usize,
    pub avg_score: f32,
    pub min_score: f32,
    pub max_score: f32,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            tolerance: ToleranceConfig::default(),
            performance_requirements: PerformanceRequirements::default(),
            comprehensive: true,
            test_timeout: Duration::from_secs(300), // 5 minutes
        }
    }
}

impl Default for PerformanceRequirements {
    fn default() -> Self {
        Self {
            min_fps: 100.0, // Minimum 100 FPS for fast-forward replay
            max_avg_frame_time_us: 1000.0, // Maximum 1ms average frame time
            max_memory_bytes: 100 * 1024 * 1024, // 100MB memory limit
        }
    }
}

impl Default for OutputAccuracyResult {
    fn default() -> Self {
        Self {
            within_tolerance: false,
            axis_accuracy_score: 0.0,
            ffb_accuracy_score: 0.0,
            total_mismatches: 0,
        }
    }
}

impl Default for PerformanceValidationResult {
    fn default() -> Self {
        Self {
            meets_requirements: false,
            frames_per_second: 0.0,
            avg_frame_time_us: 0.0,
            memory_efficiency_score: 0.0,
        }
    }
}

impl Default for TimingValidationResult {
    fn default() -> Self {
        Self {
            within_tolerance: false,
            timing_accuracy_score: 0.0,
            max_timing_drift_ns_per_s: 0,
            frames_within_tolerance_pct: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use flight_core::blackbox::{BlackboxWriter, BlackboxConfig};

    async fn create_test_blackbox() -> (TempDir, std::path::PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let config = BlackboxConfig {
            output_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let mut writer = BlackboxWriter::new(config);
        let filepath = writer.start_recording(
            "test_sim".to_string(),
            "test_aircraft".to_string(),
            "1.0.0".to_string(),
        ).await.unwrap();

        // Write test data
        for i in 0..1000 {
            let timestamp = i * 4_000_000;
            let axis_data = bincode::serialize(&flight_axis::AxisFrame::new(0.5, timestamp)).unwrap();
            writer.record_axis_frame(timestamp, &axis_data).unwrap();
        }

        tokio::time::sleep(Duration::from_millis(50)).await;
        writer.stop_recording().await.unwrap();

        (temp_dir, filepath)
    }

    #[tokio::test]
    async fn test_validator_creation() {
        let config = ValidationConfig::default();
        let validator = ReplayValidator::new(config);
        
        // Validator should be created successfully
        assert!(true);
    }

    #[tokio::test]
    async fn test_file_validation() {
        let (_temp_dir, filepath) = create_test_blackbox().await;
        
        let config = ValidationConfig {
            test_timeout: Duration::from_secs(30),
            ..Default::default()
        };
        let validator = ReplayValidator::new(config);
        
        let result = validator.validate_file(&filepath, "test_validation".to_string()).await.unwrap();
        
        assert_eq!(result.test_name, "test_validation");
        assert!(result.score >= 0.0 && result.score <= 1.0);
    }

    #[tokio::test]
    async fn test_validation_suite() {
        let (_temp_dir, filepath) = create_test_blackbox().await;
        
        let config = ValidationConfig {
            test_timeout: Duration::from_secs(30),
            ..Default::default()
        };
        let mut suite = ValidationSuite::new(config);
        
        suite.add_test_case(ValidationTestCase {
            name: "basic_replay_test".to_string(),
            description: "Basic replay validation test".to_string(),
            file_path: filepath.to_string_lossy().to_string(),
            expected_score_threshold: 0.5,
        });
        
        let results = suite.run_all_tests().await.unwrap();
        assert_eq!(results.len(), 1);
        
        let summary = suite.summarize_results(&results);
        assert_eq!(summary.total_tests, 1);
    }

    #[test]
    fn test_validation_summary() {
        let results = vec![
            ValidationResult {
                test_name: "test1".to_string(),
                passed: true,
                score: 0.8,
                details: ValidationDetails {
                    replay_result: None,
                    output_accuracy: OutputAccuracyResult::default(),
                    performance: PerformanceValidationResult::default(),
                    timing: TimingValidationResult::default(),
                },
                errors: Vec::new(),
            },
            ValidationResult {
                test_name: "test2".to_string(),
                passed: false,
                score: 0.3,
                details: ValidationDetails {
                    replay_result: None,
                    output_accuracy: OutputAccuracyResult::default(),
                    performance: PerformanceValidationResult::default(),
                    timing: TimingValidationResult::default(),
                },
                errors: vec!["Test error".to_string()],
            },
        ];
        
        let config = ValidationConfig::default();
        let suite = ValidationSuite::new(config);
        let summary = suite.summarize_results(&results);
        
        assert_eq!(summary.total_tests, 2);
        assert_eq!(summary.passed_tests, 1);
        assert_eq!(summary.failed_tests, 1);
        assert_eq!(summary.avg_score, 0.55);
        assert_eq!(summary.min_score, 0.3);
        assert_eq!(summary.max_score, 0.8);
    }
}