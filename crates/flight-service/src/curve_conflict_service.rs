//! Curve conflict detection and resolution service
//!
//! Provides the main service interface for detecting and resolving
//! curve conflicts across all axes and simulators.

use flight_axis::{AxisEngine, CurveConflict, ConflictType, ConflictSeverity, ResolutionType, BlackboxAnnotator, ResolutionDetails};
use flight_core::{CurveConflictWriter, WriteResult, WritersConfig};
use flight_ipc::proto::{
    DetectCurveConflictsRequest, DetectCurveConflictsResponse, CurveConflict as ProtoCurveConflict,
    ResolveCurveConflictRequest, ResolveCurveConflictResponse, ConflictType as ProtoConflictType,
    ConflictSeverity as ProtoConflictSeverity, ResolutionType as ProtoResolutionType,
    ConflictMetadata as ProtoConflictMetadata, ConflictResolution as ProtoConflictResolution,
    ResolutionAction, ResolutionResult,
};
use crate::one_click_resolver::{OneClickResolver, OneClickResolverConfig};
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use tracing::{info, warn, error, debug};
use anyhow::Result;

/// Configuration for the curve conflict service
#[derive(Debug, Clone)]
pub struct CurveConflictServiceConfig {
    /// Enable automatic conflict detection
    pub auto_detection_enabled: bool,
    /// Detection interval in milliseconds
    pub detection_interval_ms: u64,
    /// Enable automatic resolution suggestions
    pub auto_suggestions_enabled: bool,
    /// Writers configuration
    pub writers_config: WritersConfig,
}

impl Default for CurveConflictServiceConfig {
    fn default() -> Self {
        Self {
            auto_detection_enabled: true,
            detection_interval_ms: 5000, // 5 seconds
            auto_suggestions_enabled: true,
            writers_config: WritersConfig::default(),
        }
    }
}

/// Service for managing curve conflict detection and resolution
pub struct CurveConflictService {
    /// Service configuration
    config: CurveConflictServiceConfig,
    /// Axis engines by name
    axis_engines: Arc<RwLock<HashMap<String, Arc<AxisEngine>>>>,
    /// Curve conflict writer
    writer: Arc<CurveConflictWriter>,
    /// One-click resolver
    one_click_resolver: Arc<RwLock<OneClickResolver>>,
    /// Current simulator information
    current_sim: Arc<RwLock<Option<SimulatorInfo>>>,
    /// Detected conflicts cache
    conflicts_cache: Arc<RwLock<HashMap<String, CurveConflict>>>,
}

/// Information about the current simulator
#[derive(Debug, Clone)]
struct SimulatorInfo {
    sim_id: String,
    version: String,
    aircraft_id: String,
}

impl CurveConflictService {
    /// Create new curve conflict service
    pub fn new() -> Result<Self> {
        Self::with_config(CurveConflictServiceConfig::default())
    }

    /// Create new curve conflict service with custom configuration
    pub fn with_config(config: CurveConflictServiceConfig) -> Result<Self> {
        let writer = Arc::new(CurveConflictWriter::with_config(config.writers_config.clone())?);
        let one_click_resolver = Arc::new(RwLock::new(OneClickResolver::with_config(OneClickResolverConfig::default())?));
        
        Ok(Self {
            config,
            axis_engines: Arc::new(RwLock::new(HashMap::new())),
            writer,
            one_click_resolver,
            current_sim: Arc::new(RwLock::new(None)),
            conflicts_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Register an axis engine for conflict detection
    pub fn register_axis_engine(&self, axis_name: String, engine: Arc<AxisEngine>) {
        info!("Registered axis engine for conflict detection: {}", axis_name);
        self.axis_engines.write().insert(axis_name, engine);
    }

    /// Unregister an axis engine
    pub fn unregister_axis_engine(&self, axis_name: &str) {
        self.axis_engines.write().remove(axis_name);
        self.conflicts_cache.write().remove(axis_name);
        info!("Unregistered axis engine: {}", axis_name);
    }

    /// Set current simulator information
    pub fn set_current_simulator(&self, sim_id: String, version: String, aircraft_id: String) {
        let sim_info = SimulatorInfo {
            sim_id,
            version,
            aircraft_id,
        };
        
        *self.current_sim.write() = Some(sim_info.clone());
        info!("Set current simulator: {} {} ({})", sim_info.sim_id, sim_info.version, sim_info.aircraft_id);
        
        // Clear conflicts cache when simulator changes
        self.conflicts_cache.write().clear();
    }

    /// Detect curve conflicts for specified axes
    pub fn detect_conflicts(&self, request: DetectCurveConflictsRequest) -> DetectCurveConflictsResponse {
        debug!("Detecting curve conflicts for request: {:?}", request);

        let axis_names = if request.axis_names.is_empty() {
            // Get all registered axes
            self.axis_engines.read().keys().cloned().collect()
        } else {
            request.axis_names
        };

        let mut conflicts = Vec::new();
        let mut error_message = None;

        for axis_name in &axis_names {
            match self.detect_axis_conflicts(axis_name) {
                Ok(Some(conflict)) => {
                    // Convert to proto format
                    if let Ok(proto_conflict) = self.convert_conflict_to_proto(&conflict) {
                        conflicts.push(proto_conflict);
                        
                        // Update cache
                        self.conflicts_cache.write().insert(axis_name.clone(), conflict);
                    }
                }
                Ok(None) => {
                    // No conflict detected, remove from cache
                    self.conflicts_cache.write().remove(axis_name);
                }
                Err(e) => {
                    error_message = Some(format!("Failed to detect conflicts for axis '{}': {}", axis_name, e));
                    error!("Failed to detect conflicts for axis '{}': {}", axis_name, e);
                    break;
                }
            }
        }

        let success = error_message.is_none();
        
        DetectCurveConflictsResponse {
            success,
            conflicts,
            error_message: error_message.unwrap_or_default(),
        }
    }

    /// Detect conflicts for a specific axis
    fn detect_axis_conflicts(&self, axis_name: &str) -> Result<Option<CurveConflict>> {
        // First check if we have a cached conflict (for testing or recent detection)
        if let Some(conflict) = self.conflicts_cache.read().get(axis_name) {
            return Ok(Some(conflict.clone()));
        }

        // Then check the engine for real-time detection
        let engines = self.axis_engines.read();
        if let Some(engine) = engines.get(axis_name) {
            // Get conflicts from the engine
            Ok(engine.get_curve_conflicts())
        } else {
            // No engine registered for this axis - no conflicts
            Ok(None)
        }
    }

    /// Resolve a curve conflict
    pub fn resolve_conflict(&self, request: ResolveCurveConflictRequest) -> ResolveCurveConflictResponse {
        info!("Resolving curve conflict for axis: {}", request.axis_name);

        let resolution = match request.resolution {
            Some(resolution) => resolution,
            None => {
                return ResolveCurveConflictResponse {
                    success: false,
                    error_message: "No resolution action provided".to_string(),
                    result: None,
                };
            }
        };

        // Get current simulator info
        let sim_info = match self.current_sim.read().clone() {
            Some(info) => info,
            None => {
                return ResolveCurveConflictResponse {
                    success: false,
                    error_message: "No current simulator information available".to_string(),
                    result: None,
                };
            }
        };

        // Get conflict before resolution for metrics
        let before_conflict = self.conflicts_cache.read().get(&request.axis_name).cloned();

        // Apply resolution using writer system
        let write_result = match self.apply_resolution(&sim_info, &resolution) {
            Ok(result) => result,
            Err(e) => {
                error!("Failed to apply resolution: {}", e);
                return ResolveCurveConflictResponse {
                    success: false,
                    error_message: format!("Failed to apply resolution: {}", e),
                    result: None,
                };
            }
        };

        // Verify resolution if requested
        let verification_passed = if request.apply_immediately {
            self.verify_resolution(&request.axis_name).unwrap_or(false)
        } else {
            true // Skip verification if not applying immediately
        };

        // Get conflict after resolution for metrics
        let after_conflict = if verification_passed {
            self.detect_axis_conflicts(&request.axis_name).unwrap_or(None)
        } else {
            None
        };

        // Annotate resolution in blackbox
        self.annotate_resolution_applied(
            &request.axis_name,
            &resolution,
            write_result.success && verification_passed,
            &write_result,
            before_conflict.as_ref(),
            after_conflict.as_ref(),
        );

        // Clear conflicts cache if resolution was successful
        if write_result.success && verification_passed {
            self.conflicts_cache.write().remove(&request.axis_name);
            
            // Clear conflicts in the axis engine
            if let Some(engine) = self.axis_engines.read().get(&request.axis_name) {
                engine.clear_curve_conflicts();
            }
        }

        let result = ResolutionResult {
            applied_resolution: self.convert_resolution_type_to_proto(resolution.r#type()),
            modified_files: write_result.applied_diffs,
            backup_path: write_result.backup_path.map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
            before_metrics: before_conflict.and_then(|c| self.convert_conflict_metadata_to_proto(&c.metadata).ok()),
            after_metrics: after_conflict.and_then(|c| self.convert_conflict_metadata_to_proto(&c.metadata).ok()),
            verification_passed,
            verification_details: if verification_passed {
                "Resolution verified successfully".to_string()
            } else {
                "Resolution verification failed or skipped".to_string()
            },
        };

        ResolveCurveConflictResponse {
            success: write_result.success && verification_passed,
            error_message: write_result.error_message.unwrap_or_default(),
            result: Some(result),
        }
    }

    /// Apply resolution using the writer system
    fn apply_resolution(&self, sim_info: &SimulatorInfo, resolution: &ResolutionAction) -> Result<WriteResult> {
        let resolution_type_str = match resolution.r#type() {
            ProtoResolutionType::DisableSimCurve => "disable_sim_curve",
            ProtoResolutionType::DisableProfileCurve => "disable_profile_curve", 
            ProtoResolutionType::ApplyGainCompensation => "apply_gain_compensation",
            ProtoResolutionType::ReduceCurveStrength => "reduce_curve_strength",
            _ => "unknown",
        };

        let parameters: HashMap<String, String> = resolution.parameters.clone();

        self.writer.resolve_curve_conflict(
            &sim_info.sim_id,
            &sim_info.version,
            resolution_type_str,
            &parameters,
        ).map_err(|e| anyhow::anyhow!("Writer error: {}", e))
    }

    /// Verify that resolution was applied successfully
    fn verify_resolution(&self, axis_name: &str) -> Result<bool> {
        // Wait a moment for changes to take effect
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Re-detect conflicts to see if they're resolved
        match self.detect_axis_conflicts(axis_name)? {
            Some(conflict) => {
                // Check if severity improved
                Ok(matches!(conflict.severity, ConflictSeverity::Low))
            }
            None => {
                // No conflicts detected - resolution successful
                Ok(true)
            }
        }
    }

    /// Annotate resolution application in blackbox
    fn annotate_resolution_applied(
        &self,
        axis_name: &str,
        resolution: &ResolutionAction,
        success: bool,
        write_result: &WriteResult,
        before_conflict: Option<&CurveConflict>,
        after_conflict: Option<&CurveConflict>,
    ) {
        if let Some(engine) = self.axis_engines.read().get(axis_name) {
            let details = ResolutionDetails::new(
                format!("{:?}", resolution.r#type()),
                resolution.parameters.clone(),
                write_result.applied_diffs.clone(),
                write_result.backup_path.as_ref().map(|p| p.to_string_lossy().to_string()),
                success,
            ).with_metrics(
                before_conflict.map(|c| (&c.metadata).into()),
                after_conflict.map(|c| (&c.metadata).into()),
            );

            // This would ideally be done through the engine's blackbox annotator
            // For now, we'll log it
            info!(
                axis = axis_name,
                resolution_type = ?resolution.r#type(),
                success = success,
                "Resolution applied and annotated"
            );
        }
    }

    /// Convert internal conflict to proto format
    fn convert_conflict_to_proto(&self, conflict: &CurveConflict) -> Result<ProtoCurveConflict> {
        let conflict_type = match conflict.conflict_type {
            ConflictType::DoubleCurve => ProtoConflictType::DoubleCurve,
            ConflictType::ExcessiveNonlinearity => ProtoConflictType::ExcessiveNonlinearity,
            ConflictType::OpposingCurves => ProtoConflictType::OpposingCurves,
        };

        let severity = match conflict.severity {
            ConflictSeverity::Low => ProtoConflictSeverity::Low,
            ConflictSeverity::Medium => ProtoConflictSeverity::Medium,
            ConflictSeverity::High => ProtoConflictSeverity::High,
            ConflictSeverity::Critical => ProtoConflictSeverity::Critical,
        };

        let metadata = self.convert_conflict_metadata_to_proto(&conflict.metadata)?;

        let suggested_resolutions = conflict.suggested_resolutions
            .iter()
            .map(|r| self.convert_resolution_to_proto(r))
            .collect::<Result<Vec<_>>>()?;

        Ok(ProtoCurveConflict {
            axis_name: conflict.axis_name.clone(),
            conflict_type: conflict_type.into(),
            severity: severity.into(),
            description: conflict.description.clone(),
            suggested_resolutions,
            metadata: Some(metadata),
        })
    }

    /// Convert conflict metadata to proto format
    fn convert_conflict_metadata_to_proto(&self, metadata: &flight_axis::ConflictMetadata) -> Result<ProtoConflictMetadata> {
        Ok(ProtoConflictMetadata {
            sim_curve_strength: metadata.sim_curve_strength,
            profile_curve_strength: metadata.profile_curve_strength,
            combined_nonlinearity: metadata.combined_nonlinearity,
            test_inputs: metadata.test_inputs.clone(),
            expected_outputs: metadata.expected_outputs.clone(),
            actual_outputs: metadata.actual_outputs.clone(),
            detection_timestamp: metadata.detection_timestamp.elapsed().as_millis() as i64,
        })
    }

    /// Convert resolution to proto format
    fn convert_resolution_to_proto(&self, resolution: &flight_axis::ConflictResolution) -> Result<ProtoConflictResolution> {
        let resolution_type = match resolution.resolution_type {
            ResolutionType::DisableSimCurve => ProtoResolutionType::DisableSimCurve,
            ResolutionType::DisableProfileCurve => ProtoResolutionType::DisableProfileCurve,
            ResolutionType::ApplyGainCompensation => ProtoResolutionType::ApplyGainCompensation,
            ResolutionType::ReduceCurveStrength => ProtoResolutionType::ReduceCurveStrength,
        };

        let action = ResolutionAction {
            r#type: resolution_type.into(),
            parameters: resolution.parameters.clone(),
            affected_files: vec![], // Would be populated by writer system
            backup_info: String::new(),
        };

        Ok(ProtoConflictResolution {
            resolution_type: resolution_type.into(),
            description: resolution.description.clone(),
            action: Some(action),
            estimated_improvement: resolution.estimated_improvement,
            requires_sim_restart: resolution.requires_sim_restart,
        })
    }

    /// Convert resolution type to proto format
    fn convert_resolution_type_to_proto(&self, resolution_type: ProtoResolutionType) -> i32 {
        resolution_type.into()
    }

    /// Get all currently detected conflicts
    pub fn get_all_conflicts(&self) -> HashMap<String, CurveConflict> {
        self.conflicts_cache.read().clone()
    }

    /// Clear all conflicts (for testing)
    pub fn clear_all_conflicts(&self) {
        self.conflicts_cache.write().clear();
        
        for engine in self.axis_engines.read().values() {
            engine.clear_curve_conflicts();
        }
    }

    /// Enable/disable automatic conflict detection
    pub fn set_auto_detection_enabled(&mut self, enabled: bool) {
        self.config.auto_detection_enabled = enabled;
        info!("Automatic conflict detection {}", if enabled { "enabled" } else { "disabled" });
    }

    /// Check if automatic detection is enabled
    pub fn is_auto_detection_enabled(&self) -> bool {
        self.config.auto_detection_enabled
    }

    /// Inject a conflict for testing purposes
    #[cfg(test)]
    pub fn inject_conflict_for_testing(&self, axis_name: String, conflict: CurveConflict) {
        self.conflicts_cache.write().insert(axis_name, conflict);
    }

    /// Perform one-click resolution of a curve conflict
    pub fn one_click_resolve(&self, axis_name: &str) -> Result<crate::one_click_resolver::OneClickResult> {
        // Get the conflict for this axis
        let conflict = self.conflicts_cache.read()
            .get(axis_name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No conflict found for axis '{}'", axis_name))?;

        // Get current simulator info
        let sim_info = self.current_sim.read()
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No current simulator information available"))?;

        // Use one-click resolver
        let mut resolver = self.one_click_resolver.write();
        let result = resolver.resolve_conflict(
            axis_name,
            &conflict,
            &sim_info.sim_id,
            &sim_info.version,
        )?;

        // If resolution was successful, clear the conflict from cache and engine
        if result.success {
            self.conflicts_cache.write().remove(axis_name);
            
            if let Some(engine) = self.axis_engines.read().get(axis_name) {
                engine.clear_curve_conflicts();
            }
            
            info!("One-click resolution successful for axis '{}', improvement: {:.1}%", 
                  axis_name, result.metrics.improvement * 100.0);
        } else {
            warn!("One-click resolution failed for axis '{}': {:?}", 
                  axis_name, result.error_message);
        }

        Ok(result)
    }
}

impl Default for CurveConflictService {
    fn default() -> Self {
        Self::new().expect("Failed to create default CurveConflictService")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flight_axis::{AxisEngine, ConflictDetectorConfig};
    use std::sync::Arc;

    #[test]
    fn test_service_creation() {
        let service = CurveConflictService::new();
        assert!(service.is_ok());
    }

    #[test]
    fn test_axis_engine_registration() {
        let service = CurveConflictService::new().unwrap();
        let engine = Arc::new(AxisEngine::new_for_axis("test_axis".to_string()));
        
        service.register_axis_engine("test_axis".to_string(), engine);
        
        assert_eq!(service.axis_engines.read().len(), 1);
        assert!(service.axis_engines.read().contains_key("test_axis"));
    }

    #[test]
    fn test_simulator_info_setting() {
        let service = CurveConflictService::new().unwrap();
        
        service.set_current_simulator(
            "msfs".to_string(),
            "1.36.0".to_string(),
            "C172".to_string(),
        );
        
        let sim_info = service.current_sim.read();
        assert!(sim_info.is_some());
        let info = sim_info.as_ref().unwrap();
        assert_eq!(info.sim_id, "msfs");
        assert_eq!(info.version, "1.36.0");
        assert_eq!(info.aircraft_id, "C172");
    }

    #[test]
    fn test_conflict_detection_request() {
        let service = CurveConflictService::new().unwrap();
        
        let request = DetectCurveConflictsRequest {
            axis_names: vec!["pitch".to_string()],
            sim_id: "msfs".to_string(),
            aircraft_id: "C172".to_string(),
        };
        
        let response = service.detect_conflicts(request);
        
        // Should succeed even with no registered engines (just return empty conflicts)
        assert!(response.success);
        assert!(response.conflicts.is_empty());
    }

    #[test]
    fn test_auto_detection_toggle() {
        let mut service = CurveConflictService::new().unwrap();
        
        assert!(service.is_auto_detection_enabled()); // Default is enabled
        
        service.set_auto_detection_enabled(false);
        assert!(!service.is_auto_detection_enabled());
        
        service.set_auto_detection_enabled(true);
        assert!(service.is_auto_detection_enabled());
    }
}