//! Blackbox annotation system for curve conflicts
//!
//! Provides structured logging and annotation of curve conflict events
//! for diagnostics and replay analysis.

use crate::{CurveConflict, ConflictType, ConflictSeverity};
use std::time::{SystemTime, UNIX_EPOCH, Instant};
use serde::{Serialize, Deserialize};
use tracing::{info, warn, debug};

/// Blackbox event types for curve conflicts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BlackboxEvent {
    /// Curve conflict detected
    ConflictDetected {
        timestamp: u64,
        axis_name: String,
        conflict: ConflictData,
    },
    /// Conflict resolution applied
    ResolutionApplied {
        timestamp: u64,
        axis_name: String,
        resolution_type: String,
        success: bool,
        details: ResolutionDetails,
    },
    /// Conflict cleared/resolved
    ConflictCleared {
        timestamp: u64,
        axis_name: String,
        reason: String,
    },
    /// Pre-fault capture marker
    PreFaultCapture {
        timestamp: u64,
        axis_name: String,
        capture_duration_ms: u64,
        sample_count: usize,
    },
}

/// Serializable conflict data for blackbox
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictData {
    pub conflict_type: String,
    pub severity: String,
    pub description: String,
    pub sim_curve_strength: f32,
    pub profile_curve_strength: f32,
    pub combined_nonlinearity: f32,
    pub test_inputs: Vec<f32>,
    pub expected_outputs: Vec<f32>,
    pub actual_outputs: Vec<f32>,
}

/// Resolution details for blackbox
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionDetails {
    pub resolution_type: String,
    pub parameters: std::collections::HashMap<String, String>,
    pub affected_files: Vec<String>,
    pub backup_path: Option<String>,
    pub verification_passed: bool,
    pub before_metrics: Option<ConflictData>,
    pub after_metrics: Option<ConflictData>,
}

/// Blackbox annotation writer
pub struct BlackboxAnnotator {
    /// Enable blackbox annotations
    enabled: bool,
    /// Buffer for events before writing
    event_buffer: Vec<BlackboxEvent>,
    /// Maximum buffer size before flush
    max_buffer_size: usize,
}

impl BlackboxAnnotator {
    /// Create new blackbox annotator
    pub fn new() -> Self {
        Self {
            enabled: true,
            event_buffer: Vec::new(),
            max_buffer_size: 100,
        }
    }

    /// Create disabled annotator (for testing)
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            event_buffer: Vec::new(),
            max_buffer_size: 0,
        }
    }

    /// Enable or disable annotations
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.event_buffer.clear();
        }
    }

    /// Check if annotations are enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Annotate curve conflict detection
    pub fn annotate_conflict_detected(&mut self, axis_name: &str, conflict: &CurveConflict) {
        if !self.enabled {
            return;
        }

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let conflict_data = ConflictData {
            conflict_type: format!("{:?}", conflict.conflict_type),
            severity: format!("{:?}", conflict.severity),
            description: conflict.description.clone(),
            sim_curve_strength: conflict.metadata.sim_curve_strength,
            profile_curve_strength: conflict.metadata.profile_curve_strength,
            combined_nonlinearity: conflict.metadata.combined_nonlinearity,
            test_inputs: conflict.metadata.test_inputs.clone(),
            expected_outputs: conflict.metadata.expected_outputs.clone(),
            actual_outputs: conflict.metadata.actual_outputs.clone(),
        };

        let event = BlackboxEvent::ConflictDetected {
            timestamp,
            axis_name: axis_name.to_string(),
            conflict: conflict_data,
        };

        self.add_event(event);

        info!(
            axis = axis_name,
            conflict_type = ?conflict.conflict_type,
            severity = ?conflict.severity,
            nonlinearity = conflict.metadata.combined_nonlinearity,
            "Curve conflict detected and annotated in blackbox"
        );
    }

    /// Annotate resolution application
    pub fn annotate_resolution_applied(
        &mut self,
        axis_name: &str,
        resolution_type: &str,
        success: bool,
        details: ResolutionDetails,
    ) {
        if !self.enabled {
            return;
        }

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let event = BlackboxEvent::ResolutionApplied {
            timestamp,
            axis_name: axis_name.to_string(),
            resolution_type: resolution_type.to_string(),
            success,
            details,
        };

        self.add_event(event);

        if success {
            info!(
                axis = axis_name,
                resolution_type = resolution_type,
                "Curve conflict resolution applied successfully"
            );
        } else {
            warn!(
                axis = axis_name,
                resolution_type = resolution_type,
                "Curve conflict resolution failed"
            );
        }
    }

    /// Annotate conflict cleared
    pub fn annotate_conflict_cleared(&mut self, axis_name: &str, reason: &str) {
        if !self.enabled {
            return;
        }

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let event = BlackboxEvent::ConflictCleared {
            timestamp,
            axis_name: axis_name.to_string(),
            reason: reason.to_string(),
        };

        self.add_event(event);

        debug!(
            axis = axis_name,
            reason = reason,
            "Curve conflict cleared"
        );
    }

    /// Annotate pre-fault capture (2s before fault detection)
    pub fn annotate_pre_fault_capture(
        &mut self,
        axis_name: &str,
        capture_duration_ms: u64,
        sample_count: usize,
    ) {
        if !self.enabled {
            return;
        }

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let event = BlackboxEvent::PreFaultCapture {
            timestamp,
            axis_name: axis_name.to_string(),
            capture_duration_ms,
            sample_count,
        };

        self.add_event(event);

        debug!(
            axis = axis_name,
            duration_ms = capture_duration_ms,
            samples = sample_count,
            "Pre-fault capture annotated"
        );
    }

    /// Add event to buffer
    fn add_event(&mut self, event: BlackboxEvent) {
        self.event_buffer.push(event);

        // Flush if buffer is full
        if self.event_buffer.len() >= self.max_buffer_size {
            self.flush();
        }
    }

    /// Flush events to blackbox system
    pub fn flush(&mut self) {
        if self.event_buffer.is_empty() {
            return;
        }

        // In a real implementation, this would write to the .fbb file format
        // For now, we'll use structured logging
        for event in &self.event_buffer {
            match event {
                BlackboxEvent::ConflictDetected { timestamp, axis_name, conflict } => {
                    info!(
                        target: "blackbox",
                        timestamp = timestamp,
                        axis = axis_name,
                        event_type = "conflict_detected",
                        conflict_type = conflict.conflict_type,
                        severity = conflict.severity,
                        nonlinearity = conflict.combined_nonlinearity,
                        "BLACKBOX: Curve conflict detected"
                    );
                }
                BlackboxEvent::ResolutionApplied { timestamp, axis_name, resolution_type, success, details } => {
                    info!(
                        target: "blackbox",
                        timestamp = timestamp,
                        axis = axis_name,
                        event_type = "resolution_applied",
                        resolution_type = resolution_type,
                        success = success,
                        verification_passed = details.verification_passed,
                        "BLACKBOX: Resolution applied"
                    );
                }
                BlackboxEvent::ConflictCleared { timestamp, axis_name, reason } => {
                    info!(
                        target: "blackbox",
                        timestamp = timestamp,
                        axis = axis_name,
                        event_type = "conflict_cleared",
                        reason = reason,
                        "BLACKBOX: Conflict cleared"
                    );
                }
                BlackboxEvent::PreFaultCapture { timestamp, axis_name, capture_duration_ms, sample_count } => {
                    info!(
                        target: "blackbox",
                        timestamp = timestamp,
                        axis = axis_name,
                        event_type = "pre_fault_capture",
                        duration_ms = capture_duration_ms,
                        samples = sample_count,
                        "BLACKBOX: Pre-fault capture"
                    );
                }
            }
        }

        debug!("Flushed {} blackbox events", self.event_buffer.len());
        self.event_buffer.clear();
    }

    /// Get buffered events (for testing)
    pub fn get_buffered_events(&self) -> &[BlackboxEvent] {
        &self.event_buffer
    }

    /// Clear buffer without flushing
    pub fn clear_buffer(&mut self) {
        self.event_buffer.clear();
    }

    /// Set maximum buffer size
    pub fn set_max_buffer_size(&mut self, size: usize) {
        self.max_buffer_size = size;
        
        // Flush if current buffer exceeds new limit
        if self.event_buffer.len() >= size {
            self.flush();
        }
    }
}

impl Default for BlackboxAnnotator {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert CurveConflict to ConflictData for serialization
impl From<&CurveConflict> for ConflictData {
    fn from(conflict: &CurveConflict) -> Self {
        Self {
            conflict_type: format!("{:?}", conflict.conflict_type),
            severity: format!("{:?}", conflict.severity),
            description: conflict.description.clone(),
            sim_curve_strength: conflict.metadata.sim_curve_strength,
            profile_curve_strength: conflict.metadata.profile_curve_strength,
            combined_nonlinearity: conflict.metadata.combined_nonlinearity,
            test_inputs: conflict.metadata.test_inputs.clone(),
            expected_outputs: conflict.metadata.expected_outputs.clone(),
            actual_outputs: conflict.metadata.actual_outputs.clone(),
        }
    }
}

/// Convert ConflictMetadata to ConflictData for serialization
impl From<&crate::ConflictMetadata> for ConflictData {
    fn from(metadata: &crate::ConflictMetadata) -> Self {
        Self {
            conflict_type: "Unknown".to_string(), // Metadata doesn't have conflict type
            severity: "Unknown".to_string(), // Metadata doesn't have severity
            description: "Metadata only".to_string(),
            sim_curve_strength: metadata.sim_curve_strength,
            profile_curve_strength: metadata.profile_curve_strength,
            combined_nonlinearity: metadata.combined_nonlinearity,
            test_inputs: metadata.test_inputs.clone(),
            expected_outputs: metadata.expected_outputs.clone(),
            actual_outputs: metadata.actual_outputs.clone(),
        }
    }
}

/// Helper to create resolution details
impl ResolutionDetails {
    pub fn new(
        resolution_type: String,
        parameters: std::collections::HashMap<String, String>,
        affected_files: Vec<String>,
        backup_path: Option<String>,
        verification_passed: bool,
    ) -> Self {
        Self {
            resolution_type,
            parameters,
            affected_files,
            backup_path,
            verification_passed,
            before_metrics: None,
            after_metrics: None,
        }
    }

    pub fn with_metrics(
        mut self,
        before: Option<ConflictData>,
        after: Option<ConflictData>,
    ) -> Self {
        self.before_metrics = before;
        self.after_metrics = after;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ConflictMetadata, ConflictResolution, ResolutionType};
    use std::collections::HashMap;

    fn create_test_conflict() -> CurveConflict {
        CurveConflict {
            axis_name: "test_axis".to_string(),
            conflict_type: ConflictType::DoubleCurve,
            severity: ConflictSeverity::Medium,
            description: "Test conflict".to_string(),
            metadata: ConflictMetadata {
                sim_curve_strength: 0.3,
                profile_curve_strength: 0.2,
                combined_nonlinearity: 0.4,
                test_inputs: vec![0.0, 0.5, 1.0],
                expected_outputs: vec![0.0, 0.5, 1.0],
                actual_outputs: vec![0.0, 0.3, 1.0],
                detection_timestamp: Instant::now(),
            },
            suggested_resolutions: vec![],
            detected_at: Instant::now(),
        }
    }

    #[test]
    fn test_annotator_creation() {
        let annotator = BlackboxAnnotator::new();
        assert!(annotator.is_enabled());
        assert_eq!(annotator.get_buffered_events().len(), 0);
    }

    #[test]
    fn test_disabled_annotator() {
        let annotator = BlackboxAnnotator::disabled();
        assert!(!annotator.is_enabled());
    }

    #[test]
    fn test_conflict_annotation() {
        let mut annotator = BlackboxAnnotator::new();
        let conflict = create_test_conflict();
        
        annotator.annotate_conflict_detected("test_axis", &conflict);
        
        let events = annotator.get_buffered_events();
        assert_eq!(events.len(), 1);
        
        match &events[0] {
            BlackboxEvent::ConflictDetected { axis_name, conflict, .. } => {
                assert_eq!(axis_name, "test_axis");
                assert_eq!(conflict.conflict_type, "DoubleCurve");
                assert_eq!(conflict.severity, "Medium");
            }
            _ => panic!("Expected ConflictDetected event"),
        }
    }

    #[test]
    fn test_resolution_annotation() {
        let mut annotator = BlackboxAnnotator::new();
        let details = ResolutionDetails::new(
            "DisableSimCurve".to_string(),
            HashMap::new(),
            vec!["test.cfg".to_string()],
            Some("/backup/path".to_string()),
            true,
        );
        
        annotator.annotate_resolution_applied("test_axis", "DisableSimCurve", true, details);
        
        let events = annotator.get_buffered_events();
        assert_eq!(events.len(), 1);
        
        match &events[0] {
            BlackboxEvent::ResolutionApplied { axis_name, resolution_type, success, .. } => {
                assert_eq!(axis_name, "test_axis");
                assert_eq!(resolution_type, "DisableSimCurve");
                assert!(success);
            }
            _ => panic!("Expected ResolutionApplied event"),
        }
    }

    #[test]
    fn test_buffer_flush() {
        let mut annotator = BlackboxAnnotator::new();
        annotator.set_max_buffer_size(2);
        
        let conflict = create_test_conflict();
        
        // Add first event
        annotator.annotate_conflict_detected("axis1", &conflict);
        assert_eq!(annotator.get_buffered_events().len(), 1);
        
        // Add second event - should trigger flush
        annotator.annotate_conflict_detected("axis2", &conflict);
        assert_eq!(annotator.get_buffered_events().len(), 0); // Buffer should be empty after flush
    }

    #[test]
    fn test_disabled_annotation() {
        let mut annotator = BlackboxAnnotator::new();
        annotator.set_enabled(false);
        
        let conflict = create_test_conflict();
        annotator.annotate_conflict_detected("test_axis", &conflict);
        
        assert_eq!(annotator.get_buffered_events().len(), 0);
    }

    #[test]
    fn test_conflict_data_conversion() {
        let conflict = create_test_conflict();
        let conflict_data: ConflictData = (&conflict).into();
        
        assert_eq!(conflict_data.conflict_type, "DoubleCurve");
        assert_eq!(conflict_data.severity, "Medium");
        assert_eq!(conflict_data.description, "Test conflict");
        assert_eq!(conflict_data.combined_nonlinearity, 0.4);
    }
}