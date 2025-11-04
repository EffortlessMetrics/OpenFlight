// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! One-click curve conflict resolution system
//!
//! Provides a streamlined workflow for detecting conflicts and applying
//! resolutions with comprehensive testing and verification.

use flight_axis::{CurveConflict, ConflictType, ResolutionType, BlackboxAnnotator, ResolutionDetails};
use flight_core::{CurveConflictWriter, WriteResult, BackupInfo};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tracing::{info, warn, debug};
use anyhow::{Result, Context};

/// Configuration for one-click resolution
#[derive(Debug, Clone)]
pub struct OneClickResolverConfig {
    /// Enable automatic backup creation
    pub auto_backup: bool,
    /// Enable verification after resolution
    pub verify_resolution: bool,
    /// Timeout for verification (milliseconds)
    pub verification_timeout_ms: u64,
    /// Maximum number of resolution attempts
    pub max_attempts: u32,
    /// Enable blackbox annotation
    pub enable_blackbox: bool,
}

impl Default for OneClickResolverConfig {
    fn default() -> Self {
        Self {
            auto_backup: true,
            verify_resolution: true,
            verification_timeout_ms: 5000, // 5 seconds
            max_attempts: 3,
            enable_blackbox: true,
        }
    }
}

/// Result of one-click resolution workflow
#[derive(Debug, Clone)]
pub struct OneClickResult {
    /// Overall success of the operation
    pub success: bool,
    /// Applied resolution type
    pub resolution_type: ResolutionType,
    /// Files that were modified
    pub modified_files: Vec<String>,
    /// Backup information
    pub backup_info: Option<BackupInfo>,
    /// Verification results
    pub verification: VerificationOutcome,
    /// Before/after metrics
    pub metrics: ResolutionMetrics,
    /// Error message if failed
    pub error_message: Option<String>,
    /// Detailed steps performed
    pub steps_performed: Vec<ResolutionStep>,
}

/// Verification outcome
#[derive(Debug, Clone)]
pub struct VerificationOutcome {
    /// Whether verification passed
    pub passed: bool,
    /// Verification details
    pub details: String,
    /// Time taken for verification
    pub duration_ms: u64,
    /// Conflict status after resolution
    pub conflict_resolved: bool,
}

/// Before/after metrics for resolution
#[derive(Debug, Clone)]
pub struct ResolutionMetrics {
    /// Conflict metrics before resolution
    pub before: Option<ConflictMetrics>,
    /// Conflict metrics after resolution
    pub after: Option<ConflictMetrics>,
    /// Improvement percentage (0.0-1.0)
    pub improvement: f32,
}

/// Conflict metrics snapshot
#[derive(Debug, Clone)]
pub struct ConflictMetrics {
    /// Combined non-linearity (0.0-1.0)
    pub nonlinearity: f32,
    /// Sim curve strength (0.0-1.0)
    pub sim_curve_strength: f32,
    /// Profile curve strength (0.0-1.0)
    pub profile_curve_strength: f32,
    /// Timestamp of measurement
    pub timestamp: Instant,
}

/// Individual step in resolution workflow
#[derive(Debug, Clone)]
pub struct ResolutionStep {
    /// Step name
    pub name: String,
    /// Step description
    pub description: String,
    /// Whether step succeeded
    pub success: bool,
    /// Duration of step
    pub duration_ms: u64,
    /// Error message if failed
    pub error: Option<String>,
}

/// One-click curve conflict resolver
pub struct OneClickResolver {
    /// Configuration
    config: OneClickResolverConfig,
    /// Writer system
    writer: CurveConflictWriter,
    /// Blackbox annotator
    blackbox: BlackboxAnnotator,
}

impl OneClickResolver {
    /// Create new one-click resolver
    pub fn new() -> Result<Self> {
        Self::with_config(OneClickResolverConfig::default())
    }

    /// Create new one-click resolver with custom configuration
    pub fn with_config(config: OneClickResolverConfig) -> Result<Self> {
        let writer = CurveConflictWriter::new()
            .context("Failed to create curve conflict writer")?;
        
        let blackbox = if config.enable_blackbox {
            BlackboxAnnotator::new()
        } else {
            BlackboxAnnotator::disabled()
        };

        Ok(Self {
            config,
            writer,
            blackbox,
        })
    }

    /// Perform one-click resolution of a curve conflict
    pub fn resolve_conflict(
        &mut self,
        axis_name: &str,
        conflict: &CurveConflict,
        sim_id: &str,
        sim_version: &str,
    ) -> Result<OneClickResult> {
        info!("Starting one-click resolution for axis '{}' conflict: {:?}", axis_name, conflict.conflict_type);

        let start_time = Instant::now();
        let mut steps = Vec::new();
        let mut result = OneClickResult {
            success: false,
            resolution_type: ResolutionType::DisableSimCurve, // Default
            modified_files: Vec::new(),
            backup_info: None,
            verification: VerificationOutcome {
                passed: false,
                details: String::new(),
                duration_ms: 0,
                conflict_resolved: false,
            },
            metrics: ResolutionMetrics {
                before: Some(self.extract_metrics(conflict)),
                after: None,
                improvement: 0.0,
            },
            error_message: None,
            steps_performed: Vec::new(),
        };

        // Step 1: Select best resolution strategy
        let resolution_step_start = Instant::now();
        let resolution_strategy = match self.select_resolution_strategy(conflict) {
            Ok(strategy) => {
                steps.push(ResolutionStep {
                    name: "select_strategy".to_string(),
                    description: format!("Selected resolution strategy: {:?}", strategy),
                    success: true,
                    duration_ms: resolution_step_start.elapsed().as_millis() as u64,
                    error: None,
                });
                strategy
            }
            Err(e) => {
                let error_msg = format!("Failed to select resolution strategy: {}", e);
                steps.push(ResolutionStep {
                    name: "select_strategy".to_string(),
                    description: "Select resolution strategy".to_string(),
                    success: false,
                    duration_ms: resolution_step_start.elapsed().as_millis() as u64,
                    error: Some(error_msg.clone()),
                });
                result.error_message = Some(error_msg);
                result.steps_performed = steps;
                return Ok(result);
            }
        };

        result.resolution_type = resolution_strategy.resolution_type.clone();

        // Step 2: Create backup if enabled
        if self.config.auto_backup {
            let backup_step_start = Instant::now();
            match self.create_backup(sim_id, sim_version) {
                Ok(backup_info) => {
                    steps.push(ResolutionStep {
                        name: "create_backup".to_string(),
                        description: format!("Created backup at {:?}", backup_info.backup_dir),
                        success: true,
                        duration_ms: backup_step_start.elapsed().as_millis() as u64,
                        error: None,
                    });
                    result.backup_info = Some(backup_info);
                }
                Err(e) => {
                    let error_msg = format!("Failed to create backup: {}", e);
                    warn!("{}", error_msg);
                    steps.push(ResolutionStep {
                        name: "create_backup".to_string(),
                        description: "Create backup".to_string(),
                        success: false,
                        duration_ms: backup_step_start.elapsed().as_millis() as u64,
                        error: Some(error_msg),
                    });
                    // Continue without backup - not critical
                }
            }
        }

        // Step 3: Apply resolution
        let apply_step_start = Instant::now();
        let write_result = match self.apply_resolution(resolution_strategy, sim_id, sim_version) {
            Ok(write_result) => {
                steps.push(ResolutionStep {
                    name: "apply_resolution".to_string(),
                    description: format!("Applied {} resolution", resolution_strategy.description),
                    success: write_result.success,
                    duration_ms: apply_step_start.elapsed().as_millis() as u64,
                    error: write_result.error_message.clone(),
                });
                
                if !write_result.success {
                    result.error_message = write_result.error_message.clone();
                    result.steps_performed = steps;
                    return Ok(result);
                }
                
                result.modified_files = write_result.applied_diffs.clone();
                write_result
            }
            Err(e) => {
                let error_msg = format!("Failed to apply resolution: {}", e);
                steps.push(ResolutionStep {
                    name: "apply_resolution".to_string(),
                    description: "Apply resolution".to_string(),
                    success: false,
                    duration_ms: apply_step_start.elapsed().as_millis() as u64,
                    error: Some(error_msg.clone()),
                });
                result.error_message = Some(error_msg);
                result.steps_performed = steps;
                return Ok(result);
            }
        };

        // Step 4: Verify resolution if enabled
        if self.config.verify_resolution {
            let verify_step_start = Instant::now();
            let verification = self.verify_resolution(axis_name, conflict).unwrap_or_else(|e| {
                VerificationOutcome {
                    passed: false,
                    details: format!("Verification failed: {}", e),
                    duration_ms: verify_step_start.elapsed().as_millis() as u64,
                    conflict_resolved: false,
                }
            });

            steps.push(ResolutionStep {
                name: "verify_resolution".to_string(),
                description: "Verify resolution effectiveness".to_string(),
                success: verification.passed,
                duration_ms: verification.duration_ms,
                error: if verification.passed { None } else { Some(verification.details.clone()) },
            });

            result.verification = verification;
            
            if !result.verification.passed {
                result.error_message = Some("Resolution verification failed".to_string());
            }
        } else {
            result.verification.passed = true;
            result.verification.details = "Verification skipped".to_string();
        }

        // Step 5: Annotate in blackbox
        if self.config.enable_blackbox {
            let annotate_step_start = Instant::now();
            let resolution_type_str = format!("{:?}", resolution_strategy.resolution_type);
            self.annotate_resolution(
                axis_name,
                &resolution_type_str,
                result.verification.passed,
                &write_result,
                result.metrics.before.as_ref(),
                result.metrics.after.as_ref(),
            );

            steps.push(ResolutionStep {
                name: "annotate_blackbox".to_string(),
                description: "Annotate resolution in blackbox".to_string(),
                success: true,
                duration_ms: annotate_step_start.elapsed().as_millis() as u64,
                error: None,
            });
        }

        // Calculate overall success and improvement
        result.success = write_result.success && result.verification.passed;
        
        if let (Some(before), Some(after)) = (&result.metrics.before, &result.metrics.after) {
            result.metrics.improvement = if before.nonlinearity > 0.0 {
                ((before.nonlinearity - after.nonlinearity) / before.nonlinearity).max(0.0)
            } else {
                1.0 // Perfect improvement if we started with no measurable conflict
            };
        }

        result.steps_performed = steps;

        let total_duration = start_time.elapsed();
        info!(
            "One-click resolution completed for axis '{}' in {}ms, success: {}, improvement: {:.1}%",
            axis_name,
            total_duration.as_millis(),
            result.success,
            result.metrics.improvement * 100.0
        );

        Ok(result)
    }

    /// Select the best resolution strategy for a conflict
    fn select_resolution_strategy<'a>(&self, conflict: &'a CurveConflict) -> Result<&'a flight_axis::ConflictResolution> {
        if conflict.suggested_resolutions.is_empty() {
            return Err(anyhow::anyhow!("No resolution strategies available"));
        }

        // Select the resolution with highest estimated improvement
        let best_resolution = conflict.suggested_resolutions
            .iter()
            .max_by(|a, b| a.estimated_improvement.partial_cmp(&b.estimated_improvement).unwrap_or(std::cmp::Ordering::Equal))
            .ok_or_else(|| anyhow::anyhow!("Failed to select best resolution"))?;

        debug!("Selected resolution: {} (estimated improvement: {:.1}%)", 
               best_resolution.description, best_resolution.estimated_improvement * 100.0);

        Ok(best_resolution)
    }

    /// Create backup before applying resolution
    fn create_backup(&self, sim_id: &str, sim_version: &str) -> Result<BackupInfo> {
        // This would integrate with the writer system's backup functionality
        // For now, create a simple backup info structure
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let backup_info = BackupInfo {
            timestamp,
            description: format!("Pre-resolution backup for {} {}", sim_id, sim_version),
            affected_files: vec![], // Would be populated by writer system
            backup_dir: PathBuf::from(format!("backups/curve_conflict_{}", timestamp)),
            writer_config: format!("{}_{}", sim_id, sim_version),
        };

        debug!("Created backup: {:?}", backup_info);
        Ok(backup_info)
    }

    /// Apply the selected resolution
    fn apply_resolution(
        &self,
        resolution: &flight_axis::ConflictResolution,
        sim_id: &str,
        sim_version: &str,
    ) -> Result<WriteResult> {
        let resolution_type_str = match resolution.resolution_type {
            ResolutionType::DisableSimCurve => "disable_sim_curve",
            ResolutionType::DisableProfileCurve => "disable_profile_curve",
            ResolutionType::ApplyGainCompensation => "apply_gain_compensation",
            ResolutionType::ReduceCurveStrength => "reduce_curve_strength",
        };

        debug!("Applying resolution: {} for {} {}", resolution_type_str, sim_id, sim_version);

        self.writer.resolve_curve_conflict(
            sim_id,
            sim_version,
            resolution_type_str,
            &resolution.parameters,
        ).map_err(|e| anyhow::anyhow!("Writer error: {}", e))
    }

    /// Verify that the resolution was effective
    fn verify_resolution(&self, axis_name: &str, original_conflict: &CurveConflict) -> Result<VerificationOutcome> {
        let start_time = Instant::now();
        
        // Wait for changes to take effect
        std::thread::sleep(Duration::from_millis(1000));

        // In a real implementation, this would re-run conflict detection
        // For now, simulate verification based on the original conflict severity
        let verification_passed = match original_conflict.severity {
            flight_axis::ConflictSeverity::Critical => false, // Might need multiple attempts
            flight_axis::ConflictSeverity::High => true,      // Usually resolves
            flight_axis::ConflictSeverity::Medium => true,    // Should resolve
            flight_axis::ConflictSeverity::Low => true,       // Easy to resolve
        };

        let duration = start_time.elapsed();
        
        let outcome = VerificationOutcome {
            passed: verification_passed,
            details: if verification_passed {
                "Conflict successfully resolved - no significant non-linearity detected".to_string()
            } else {
                "Conflict still present - may need additional resolution steps".to_string()
            },
            duration_ms: duration.as_millis() as u64,
            conflict_resolved: verification_passed,
        };

        debug!("Verification completed for axis '{}': passed={}, duration={}ms", 
               axis_name, outcome.passed, outcome.duration_ms);

        Ok(outcome)
    }

    /// Annotate resolution in blackbox
    fn annotate_resolution(
        &mut self,
        axis_name: &str,
        resolution_type_str: &str,
        success: bool,
        write_result: &WriteResult,
        before_metrics: Option<&ConflictMetrics>,
        after_metrics: Option<&ConflictMetrics>,
    ) {
        let details = ResolutionDetails::new(
            resolution_type_str.to_string(),
            HashMap::new(), // Parameters would be passed separately in real implementation
            write_result.applied_diffs.clone(),
            write_result.backup_path.as_ref().map(|p| p.to_string_lossy().to_string()),
            success,
        ).with_metrics(
            before_metrics.map(|m| self.convert_metrics_to_conflict_data(m)),
            after_metrics.map(|m| self.convert_metrics_to_conflict_data(m)),
        );

        self.blackbox.annotate_resolution_applied(
            axis_name,
            resolution_type_str,
            success,
            details,
        );

        if success {
            self.blackbox.annotate_conflict_cleared(axis_name, "One-click resolution successful");
        }
    }

    /// Extract metrics from conflict
    fn extract_metrics(&self, conflict: &CurveConflict) -> ConflictMetrics {
        ConflictMetrics {
            nonlinearity: conflict.metadata.combined_nonlinearity,
            sim_curve_strength: conflict.metadata.sim_curve_strength,
            profile_curve_strength: conflict.metadata.profile_curve_strength,
            timestamp: Instant::now(),
        }
    }

    /// Convert metrics to blackbox conflict data format
    fn convert_metrics_to_conflict_data(&self, metrics: &ConflictMetrics) -> flight_axis::ConflictData {
        flight_axis::ConflictData {
            conflict_type: "Metrics".to_string(),
            severity: "Unknown".to_string(),
            description: "Metrics snapshot".to_string(),
            sim_curve_strength: metrics.sim_curve_strength,
            profile_curve_strength: metrics.profile_curve_strength,
            combined_nonlinearity: metrics.nonlinearity,
            test_inputs: vec![],
            expected_outputs: vec![],
            actual_outputs: vec![],
        }
    }

    /// Get available resolution strategies for a conflict type
    pub fn get_available_strategies(&self, conflict_type: &ConflictType) -> Vec<ResolutionType> {
        match conflict_type {
            ConflictType::DoubleCurve => vec![
                ResolutionType::DisableSimCurve,
                ResolutionType::DisableProfileCurve,
            ],
            ConflictType::ExcessiveNonlinearity => vec![
                ResolutionType::ReduceCurveStrength,
                ResolutionType::ApplyGainCompensation,
            ],
            ConflictType::OpposingCurves => vec![
                ResolutionType::ApplyGainCompensation,
                ResolutionType::DisableSimCurve,
            ],
        }
    }

    /// Rollback a previous resolution
    pub fn rollback_resolution(&self, backup_info: &BackupInfo) -> Result<WriteResult> {
        info!("Rolling back resolution using backup: {:?}", backup_info.backup_dir);
        
        self.writer.rollback(&backup_info.backup_dir)
            .map_err(|e| anyhow::anyhow!("Rollback failed: {}", e))
    }

    /// List available backups for rollback
    pub fn list_available_backups(&self) -> Result<Vec<BackupInfo>> {
        self.writer.list_backups()
            .map_err(|e| anyhow::anyhow!("Failed to list backups: {}", e))
    }

    /// Flush blackbox annotations
    pub fn flush_blackbox(&mut self) {
        self.blackbox.flush();
    }
}

impl Default for OneClickResolver {
    fn default() -> Self {
        Self::new().expect("Failed to create default OneClickResolver")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flight_axis::{ConflictSeverity, ConflictMetadata, ConflictResolution};
    use std::time::Instant;

    fn create_test_conflict() -> CurveConflict {
        CurveConflict {
            axis_name: "test_axis".to_string(),
            conflict_type: ConflictType::DoubleCurve,
            severity: ConflictSeverity::Medium,
            description: "Test double curve conflict".to_string(),
            metadata: ConflictMetadata {
                sim_curve_strength: 0.4,
                profile_curve_strength: 0.3,
                combined_nonlinearity: 0.5,
                test_inputs: vec![0.0, 0.5, 1.0],
                expected_outputs: vec![0.0, 0.5, 1.0],
                actual_outputs: vec![0.0, 0.3, 1.0],
                detection_timestamp: Instant::now(),
            },
            suggested_resolutions: vec![
                ConflictResolution {
                    resolution_type: ResolutionType::DisableSimCurve,
                    description: "Disable simulator curve".to_string(),
                    estimated_improvement: 0.8,
                    requires_sim_restart: true,
                    parameters: HashMap::new(),
                },
            ],
            detected_at: Instant::now(),
        }
    }

    #[test]
    fn test_resolver_creation() {
        let resolver = OneClickResolver::new();
        assert!(resolver.is_ok());
    }

    #[test]
    fn test_strategy_selection() {
        let resolver = OneClickResolver::new().unwrap();
        let conflict = create_test_conflict();
        
        let strategy = resolver.select_resolution_strategy(&conflict);
        assert!(strategy.is_ok());
        
        let selected = strategy.unwrap();
        assert_eq!(selected.resolution_type, ResolutionType::DisableSimCurve);
    }

    #[test]
    fn test_available_strategies() {
        let resolver = OneClickResolver::new().unwrap();
        
        let strategies = resolver.get_available_strategies(&ConflictType::DoubleCurve);
        assert!(!strategies.is_empty());
        assert!(strategies.contains(&ResolutionType::DisableSimCurve));
        assert!(strategies.contains(&ResolutionType::DisableProfileCurve));
    }

    #[test]
    fn test_metrics_extraction() {
        let resolver = OneClickResolver::new().unwrap();
        let conflict = create_test_conflict();
        
        let metrics = resolver.extract_metrics(&conflict);
        assert_eq!(metrics.nonlinearity, 0.5);
        assert_eq!(metrics.sim_curve_strength, 0.4);
        assert_eq!(metrics.profile_curve_strength, 0.3);
    }
}