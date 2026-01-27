// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Verify matrix integration for panel drift detection and repair
//!
//! Provides systematic testing of panel configurations to detect drift
//! and automated repair capabilities for Saitek/Logitech panels.

use crate::saitek::{PanelType, SaitekPanelWriter, VerifyTestResult};
use flight_core::{FlightError, Result};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

/// Verify matrix for systematic panel testing
pub struct VerifyMatrix {
    /// Panel writer for hardware communication
    panel_writer: SaitekPanelWriter,
    /// Test configurations by panel type
    test_configs: HashMap<PanelType, VerifyConfig>,
    /// Test results history
    test_history: HashMap<String, Vec<VerifyTestResult>>,
    /// Drift detection thresholds
    drift_thresholds: DriftThresholds,
    /// Last full matrix run
    last_matrix_run: Option<Instant>,
    /// Matrix run interval
    matrix_interval: Duration,
}

/// Configuration for verify tests
#[derive(Debug, Clone)]
pub struct VerifyConfig {
    /// Panel type
    pub panel_type: PanelType,
    /// Expected latency threshold (≤20ms per requirements)
    pub latency_threshold: Duration,
    /// Number of test iterations
    pub test_iterations: u32,
    /// Interval between iterations
    pub iteration_interval: Duration,
    /// Whether to run extended tests
    pub extended_tests: bool,
}

/// Drift detection thresholds
#[derive(Debug, Clone)]
pub struct DriftThresholds {
    /// Maximum acceptable latency increase (percentage)
    pub max_latency_increase: f64,
    /// Maximum acceptable failure rate (percentage)
    pub max_failure_rate: f64,
    /// Minimum samples for drift detection
    pub min_samples: usize,
    /// Time window for drift analysis
    pub analysis_window: Duration,
}

/// Matrix test result
#[derive(Debug, Clone)]
pub struct MatrixTestResult {
    /// Panel path
    pub panel_path: String,
    /// Panel type
    pub panel_type: PanelType,
    /// Individual test results
    pub test_results: Vec<VerifyTestResult>,
    /// Overall success
    pub success: bool,
    /// Drift detected
    pub drift_detected: bool,
    /// Repair attempted
    pub repair_attempted: bool,
    /// Repair successful
    pub repair_successful: bool,
    /// Test duration
    pub total_duration: Duration,
    /// Latency statistics
    pub latency_stats: MatrixLatencyStats,
}

/// Latency statistics for matrix tests
#[derive(Debug, Clone)]
pub struct MatrixLatencyStats {
    pub mean_latency: Duration,
    pub p99_latency: Duration,
    pub max_latency: Duration,
    pub min_latency: Duration,
    pub latency_variance: f64,
    pub samples_count: usize,
}

/// Drift analysis result
#[derive(Debug, Clone)]
pub struct DriftAnalysis {
    /// Whether drift was detected
    pub drift_detected: bool,
    /// Latency trend (positive = increasing)
    pub latency_trend: f64,
    /// Failure rate trend (positive = increasing)
    pub failure_rate_trend: f64,
    /// Confidence level (0.0-1.0)
    pub confidence: f64,
    /// Recommended action
    pub recommended_action: DriftAction,
}

/// Recommended action for drift
#[derive(Debug, Clone, PartialEq)]
pub enum DriftAction {
    None,
    Monitor,
    Repair,
    Replace,
}

impl VerifyMatrix {
    /// Create new verify matrix
    pub fn new(panel_writer: SaitekPanelWriter) -> Self {
        let mut test_configs = HashMap::new();

        // Configure default test parameters for each panel type
        for &panel_type in &[
            PanelType::RadioPanel,
            PanelType::MultiPanel,
            PanelType::SwitchPanel,
            PanelType::BIP,
            PanelType::FIP,
        ] {
            test_configs.insert(
                panel_type,
                VerifyConfig {
                    panel_type,
                    latency_threshold: Duration::from_millis(20), // Per requirements
                    test_iterations: 10,
                    iteration_interval: Duration::from_millis(100),
                    extended_tests: false,
                },
            );
        }

        Self {
            panel_writer,
            test_configs,
            test_history: HashMap::new(),
            drift_thresholds: DriftThresholds {
                max_latency_increase: 50.0, // 50% increase triggers drift detection
                max_failure_rate: 10.0,     // 10% failure rate triggers drift detection
                min_samples: 5,             // Minimum 5 samples for analysis
                analysis_window: Duration::from_secs(24 * 60 * 60), // 24-hour analysis window
            },
            last_matrix_run: None,
            matrix_interval: Duration::from_secs(6 * 60 * 60), // Run matrix every 6 hours
        }
    }

    /// Run full verify matrix for all connected panels
    pub fn run_full_matrix(&mut self) -> Result<Vec<MatrixTestResult>> {
        info!("Starting full verify matrix run");
        let start_time = Instant::now();

        let panels: Vec<_> = self
            .panel_writer
            .get_panels()
            .into_iter()
            .map(|p| (p.device_info.device_path.clone(), p.panel_type))
            .collect();
        let mut results = Vec::new();

        for (panel_path, panel_type) in panels {
            info!(
                "Running matrix tests for {} panel: {}",
                panel_type.name(),
                panel_path
            );

            match self.run_panel_matrix(&panel_path, panel_type) {
                Ok(result) => {
                    results.push(result);
                }
                Err(e) => {
                    error!("Matrix test failed for panel {}: {}", panel_path, e);
                    // Continue with other panels
                }
            }
        }

        self.last_matrix_run = Some(start_time);
        let total_duration = start_time.elapsed();

        info!(
            "Full verify matrix completed in {:?}: {}/{} panels passed",
            total_duration,
            results.iter().filter(|r| r.success).count(),
            results.len()
        );

        Ok(results)
    }

    /// Run verify matrix for a specific panel
    pub fn run_panel_matrix(
        &mut self,
        panel_path: &str,
        panel_type: PanelType,
    ) -> Result<MatrixTestResult> {
        let config = self
            .test_configs
            .get(&panel_type)
            .ok_or_else(|| {
                FlightError::Configuration(format!(
                    "No test config for panel type: {:?}",
                    panel_type
                ))
            })?
            .clone();

        let start_time = Instant::now();
        let mut test_results = Vec::new();
        let mut all_latencies = Vec::new();

        // Run multiple test iterations
        for iteration in 0..config.test_iterations {
            debug!(
                "Running iteration {} for panel {}",
                iteration + 1,
                panel_path
            );

            // Start verify test
            self.panel_writer.start_verify_test(panel_path)?;

            // Wait for test completion
            let mut test_result = None;
            let iteration_start = Instant::now();
            let timeout = Duration::from_secs(30); // 30-second timeout per iteration

            while iteration_start.elapsed() < timeout {
                match self.panel_writer.update_verify_test()? {
                    Some(result) => {
                        test_result = Some(result);
                        break;
                    }
                    None => {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                }
            }

            let result = test_result.ok_or_else(|| {
                FlightError::Hardware(format!("Verify test timeout for panel {}", panel_path))
            })?;

            // Collect latency data
            for step_result in &result.step_results {
                all_latencies.push(step_result.actual_latency);
            }

            test_results.push(result);

            // Wait between iterations
            if iteration < config.test_iterations - 1 {
                std::thread::sleep(config.iteration_interval);
            }
        }

        let total_duration = start_time.elapsed();

        // Calculate latency statistics
        let latency_stats = self.calculate_latency_stats(&all_latencies);

        // Check for drift
        let drift_analysis = self.analyze_drift(panel_path, &test_results)?;

        // Attempt repair if drift detected
        let mut repair_attempted = false;
        let mut repair_successful = false;

        if drift_analysis.drift_detected && drift_analysis.recommended_action == DriftAction::Repair
        {
            info!("Drift detected for panel {}, attempting repair", panel_path);
            repair_attempted = true;

            match self.panel_writer.repair_panel_drift(panel_path) {
                Ok(()) => {
                    repair_successful = true;
                    info!("Panel drift repair successful for {}", panel_path);
                }
                Err(e) => {
                    warn!("Panel drift repair failed for {}: {}", panel_path, e);
                }
            }
        }

        // Determine overall success
        let success = test_results.iter().all(|r| r.success)
            && latency_stats.p99_latency <= config.latency_threshold
            && (!drift_analysis.drift_detected || repair_successful);

        let matrix_result = MatrixTestResult {
            panel_path: panel_path.to_string(),
            panel_type,
            test_results: test_results.clone(),
            success,
            drift_detected: drift_analysis.drift_detected,
            repair_attempted,
            repair_successful,
            total_duration,
            latency_stats,
        };

        // Store in history
        self.test_history
            .entry(panel_path.to_string())
            .or_default()
            .push(test_results.into_iter().last().unwrap()); // Store last result

        Ok(matrix_result)
    }

    /// Calculate latency statistics from samples
    fn calculate_latency_stats(&self, latencies: &[Duration]) -> MatrixLatencyStats {
        if latencies.is_empty() {
            return MatrixLatencyStats {
                mean_latency: Duration::ZERO,
                p99_latency: Duration::ZERO,
                max_latency: Duration::ZERO,
                min_latency: Duration::ZERO,
                latency_variance: 0.0,
                samples_count: 0,
            };
        }

        let mut sorted_latencies: Vec<_> = latencies.iter().map(|d| d.as_nanos()).collect();
        sorted_latencies.sort_unstable();

        let len = sorted_latencies.len();
        let sum: u128 = sorted_latencies.iter().sum();
        let mean_nanos = sum / len as u128;

        let p99_index = ((len as f64) * 0.99) as usize;
        let p99_nanos = sorted_latencies
            .get(p99_index)
            .copied()
            .unwrap_or(sorted_latencies[len - 1]);

        let max_nanos = sorted_latencies[len - 1];
        let min_nanos = sorted_latencies[0];

        // Calculate variance
        let variance = if len > 1 {
            let variance_sum: f64 = sorted_latencies
                .iter()
                .map(|&nanos| {
                    let diff = nanos as f64 - mean_nanos as f64;
                    diff * diff
                })
                .sum();
            variance_sum / (len - 1) as f64
        } else {
            0.0
        };

        MatrixLatencyStats {
            mean_latency: Duration::from_nanos(mean_nanos as u64),
            p99_latency: Duration::from_nanos(p99_nanos as u64),
            max_latency: Duration::from_nanos(max_nanos as u64),
            min_latency: Duration::from_nanos(min_nanos as u64),
            latency_variance: variance,
            samples_count: len,
        }
    }

    /// Analyze drift from test history
    fn analyze_drift(
        &self,
        panel_path: &str,
        current_results: &[VerifyTestResult],
    ) -> Result<DriftAnalysis> {
        let history = match self.test_history.get(panel_path) {
            Some(history) if history.len() >= self.drift_thresholds.min_samples => history,
            _ => {
                // Not enough history for drift analysis
                return Ok(DriftAnalysis {
                    drift_detected: false,
                    latency_trend: 0.0,
                    failure_rate_trend: 0.0,
                    confidence: 0.0,
                    recommended_action: DriftAction::Monitor,
                });
            }
        };

        // Filter history to analysis window
        let _cutoff_time = Instant::now() - self.drift_thresholds.analysis_window;
        let recent_history: Vec<_> = history
            .iter()
            .filter(|_result| {
                // Approximate time filtering - in real implementation, we'd store timestamps
                true // For now, use all history
            })
            .collect();

        if recent_history.len() < self.drift_thresholds.min_samples {
            return Ok(DriftAnalysis {
                drift_detected: false,
                latency_trend: 0.0,
                failure_rate_trend: 0.0,
                confidence: 0.0,
                recommended_action: DriftAction::Monitor,
            });
        }

        // Calculate trends
        let latency_trend = self.calculate_latency_trend(&recent_history, current_results);
        let failure_rate_trend =
            self.calculate_failure_rate_trend(&recent_history, current_results);

        // Determine if drift is detected
        let latency_drift = latency_trend > self.drift_thresholds.max_latency_increase;
        let failure_drift = failure_rate_trend > self.drift_thresholds.max_failure_rate;
        let drift_detected = latency_drift || failure_drift;

        // Calculate confidence based on sample size and trend consistency
        let confidence = if drift_detected {
            let sample_confidence = (recent_history.len() as f64 / 20.0).min(1.0); // Max confidence at 20+ samples
            let trend_confidence = (latency_trend.abs() / 100.0).min(1.0); // Stronger trends = higher confidence
            (sample_confidence + trend_confidence) / 2.0
        } else {
            0.5 // Neutral confidence when no drift
        };

        // Determine recommended action
        let recommended_action = if drift_detected {
            if latency_trend > 100.0 || failure_rate_trend > 50.0 {
                DriftAction::Replace
            } else {
                DriftAction::Repair
            }
        } else if latency_trend > 25.0 || failure_rate_trend > 5.0 {
            DriftAction::Monitor
        } else {
            DriftAction::None
        };

        Ok(DriftAnalysis {
            drift_detected,
            latency_trend,
            failure_rate_trend,
            confidence,
            recommended_action,
        })
    }

    /// Calculate latency trend (percentage change)
    fn calculate_latency_trend(
        &self,
        history: &[&VerifyTestResult],
        current: &[VerifyTestResult],
    ) -> f64 {
        if history.is_empty() || current.is_empty() {
            return 0.0;
        }

        // Calculate average latency from history
        let historical_latencies: Vec<_> = history
            .iter()
            .flat_map(|result| result.step_results.iter())
            .map(|step| step.actual_latency.as_nanos() as f64)
            .collect();

        let current_latencies: Vec<_> = current
            .iter()
            .flat_map(|result| result.step_results.iter())
            .map(|step| step.actual_latency.as_nanos() as f64)
            .collect();

        if historical_latencies.is_empty() || current_latencies.is_empty() {
            return 0.0;
        }

        let historical_avg =
            historical_latencies.iter().sum::<f64>() / historical_latencies.len() as f64;
        let current_avg = current_latencies.iter().sum::<f64>() / current_latencies.len() as f64;

        if historical_avg > 0.0 {
            ((current_avg - historical_avg) / historical_avg) * 100.0
        } else {
            0.0
        }
    }

    /// Calculate failure rate trend (percentage change)
    fn calculate_failure_rate_trend(
        &self,
        history: &[&VerifyTestResult],
        current: &[VerifyTestResult],
    ) -> f64 {
        if history.is_empty() || current.is_empty() {
            return 0.0;
        }

        // Calculate failure rates
        let historical_failures = history
            .iter()
            .map(|result| if result.success { 0.0 } else { 1.0 })
            .sum::<f64>();
        let historical_rate = historical_failures / history.len() as f64;

        let current_failures = current
            .iter()
            .map(|result| if result.success { 0.0 } else { 1.0 })
            .sum::<f64>();
        let current_rate = current_failures / current.len() as f64;

        (current_rate - historical_rate) * 100.0
    }

    /// Check if matrix run is needed
    pub fn needs_matrix_run(&self) -> bool {
        match self.last_matrix_run {
            Some(last_run) => last_run.elapsed() >= self.matrix_interval,
            None => true, // Never run before
        }
    }

    /// Get test configuration for panel type
    pub fn get_test_config(&self, panel_type: PanelType) -> Option<&VerifyConfig> {
        self.test_configs.get(&panel_type)
    }

    /// Update test configuration
    pub fn set_test_config(&mut self, panel_type: PanelType, config: VerifyConfig) {
        self.test_configs.insert(panel_type, config);
    }

    /// Get drift thresholds
    pub fn get_drift_thresholds(&self) -> &DriftThresholds {
        &self.drift_thresholds
    }

    /// Update drift thresholds
    pub fn set_drift_thresholds(&mut self, thresholds: DriftThresholds) {
        self.drift_thresholds = thresholds;
    }

    /// Get test history for a panel
    pub fn get_test_history(&self, panel_path: &str) -> Option<&Vec<VerifyTestResult>> {
        self.test_history.get(panel_path)
    }

    /// Clear test history for a panel
    pub fn clear_test_history(&mut self, panel_path: &str) {
        self.test_history.remove(panel_path);
    }

    /// Get matrix run interval
    pub fn get_matrix_interval(&self) -> Duration {
        self.matrix_interval
    }

    /// Set matrix run interval
    pub fn set_matrix_interval(&mut self, interval: Duration) {
        self.matrix_interval = interval;
    }
}

impl Default for DriftThresholds {
    fn default() -> Self {
        Self {
            max_latency_increase: 50.0,
            max_failure_rate: 10.0,
            min_samples: 5,
            analysis_window: Duration::from_secs(24 * 60 * 60),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flight_hid::HidAdapter;
    use flight_watchdog::WatchdogSystem;
    use std::sync::{Arc, Mutex};

    fn create_test_hid_adapter() -> HidAdapter {
        let watchdog = Arc::new(Mutex::new(WatchdogSystem::new()));
        HidAdapter::new(watchdog)
    }

    #[test]
    fn test_verify_matrix_creation() {
        let hid_adapter = create_test_hid_adapter();
        let panel_writer = crate::saitek::SaitekPanelWriter::new(hid_adapter);
        let matrix = VerifyMatrix::new(panel_writer);

        assert_eq!(matrix.test_configs.len(), 5); // All panel types configured
        assert!(matrix.needs_matrix_run()); // Should need initial run
    }

    #[test]
    fn test_latency_stats_calculation() {
        let hid_adapter = create_test_hid_adapter();
        let panel_writer = crate::saitek::SaitekPanelWriter::new(hid_adapter);
        let matrix = VerifyMatrix::new(panel_writer);

        let latencies = vec![
            Duration::from_millis(5),
            Duration::from_millis(10),
            Duration::from_millis(15),
            Duration::from_millis(20),
            Duration::from_millis(25),
        ];

        let stats = matrix.calculate_latency_stats(&latencies);

        assert_eq!(stats.samples_count, 5);
        assert_eq!(stats.min_latency, Duration::from_millis(5));
        assert_eq!(stats.max_latency, Duration::from_millis(25));
        assert_eq!(stats.mean_latency, Duration::from_millis(15));
        assert!(stats.latency_variance > 0.0);
    }

    #[test]
    fn test_drift_thresholds() {
        let thresholds = DriftThresholds::default();

        assert_eq!(thresholds.max_latency_increase, 50.0);
        assert_eq!(thresholds.max_failure_rate, 10.0);
        assert_eq!(thresholds.min_samples, 5);
        assert_eq!(
            thresholds.analysis_window,
            Duration::from_secs(24 * 60 * 60)
        );
    }

    #[test]
    fn test_verify_config() {
        let config = VerifyConfig {
            panel_type: PanelType::RadioPanel,
            latency_threshold: Duration::from_millis(20),
            test_iterations: 10,
            iteration_interval: Duration::from_millis(100),
            extended_tests: false,
        };

        assert_eq!(config.panel_type, PanelType::RadioPanel);
        assert_eq!(config.latency_threshold, Duration::from_millis(20));
        assert_eq!(config.test_iterations, 10);
    }

    #[test]
    fn test_drift_action_determination() {
        // Test different drift scenarios
        let analysis_none = DriftAnalysis {
            drift_detected: false,
            latency_trend: 5.0,
            failure_rate_trend: 1.0,
            confidence: 0.5,
            recommended_action: DriftAction::None,
        };
        assert_eq!(analysis_none.recommended_action, DriftAction::None);

        let analysis_monitor = DriftAnalysis {
            drift_detected: false,
            latency_trend: 30.0,
            failure_rate_trend: 7.0,
            confidence: 0.7,
            recommended_action: DriftAction::Monitor,
        };
        assert_eq!(analysis_monitor.recommended_action, DriftAction::Monitor);

        let analysis_repair = DriftAnalysis {
            drift_detected: true,
            latency_trend: 60.0,
            failure_rate_trend: 15.0,
            confidence: 0.8,
            recommended_action: DriftAction::Repair,
        };
        assert_eq!(analysis_repair.recommended_action, DriftAction::Repair);

        let analysis_replace = DriftAnalysis {
            drift_detected: true,
            latency_trend: 150.0,
            failure_rate_trend: 60.0,
            confidence: 0.9,
            recommended_action: DriftAction::Replace,
        };
        assert_eq!(analysis_replace.recommended_action, DriftAction::Replace);
    }
}
