// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Blackbox annotation system for curve conflicts
//!
//! Provides structured logging and annotation of curve conflict events
//! for diagnostics and replay analysis.

use crate::CurveConflict;
#[cfg(test)]
use crate::{ConflictSeverity, ConflictType};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::time::Instant;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};

/// Blackbox event types for curve conflicts
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
        details: Box<ResolutionDetails>,
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
    /// Capability mode changed
    CapabilityModeChanged {
        timestamp: u64,
        axis_name: String,
        old_mode: String,
        new_mode: String,
    },
    /// Output clamped due to capability limits
    OutputClamped {
        timestamp: u64,
        axis_name: String,
        original_output: f32,
        clamped_output: f32,
        capability_mode: String,
        limit_type: String,
    },
}

/// Serializable conflict data for blackbox
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
            details: Box::new(details),
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

        debug!(axis = axis_name, reason = reason, "Curve conflict cleared");
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
                BlackboxEvent::ConflictDetected {
                    timestamp,
                    axis_name,
                    conflict,
                } => {
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
                BlackboxEvent::ResolutionApplied {
                    timestamp,
                    axis_name,
                    resolution_type,
                    success,
                    details,
                } => {
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
                BlackboxEvent::ConflictCleared {
                    timestamp,
                    axis_name,
                    reason,
                } => {
                    info!(
                        target: "blackbox",
                        timestamp = timestamp,
                        axis = axis_name,
                        event_type = "conflict_cleared",
                        reason = reason,
                        "BLACKBOX: Conflict cleared"
                    );
                }
                BlackboxEvent::PreFaultCapture {
                    timestamp,
                    axis_name,
                    capture_duration_ms,
                    sample_count,
                } => {
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
                BlackboxEvent::CapabilityModeChanged {
                    timestamp,
                    axis_name,
                    old_mode,
                    new_mode,
                } => {
                    info!(
                        target: "blackbox",
                        timestamp = timestamp,
                        axis = axis_name,
                        event_type = "capability_mode_changed",
                        old_mode = old_mode,
                        new_mode = new_mode,
                        "BLACKBOX: Capability mode changed"
                    );
                }
                BlackboxEvent::OutputClamped {
                    timestamp,
                    axis_name,
                    original_output,
                    clamped_output,
                    capability_mode,
                    limit_type,
                } => {
                    info!(
                        target: "blackbox",
                        timestamp = timestamp,
                        axis = axis_name,
                        event_type = "output_clamped",
                        original_output = original_output,
                        clamped_output = clamped_output,
                        capability_mode = capability_mode,
                        limit_type = limit_type,
                        "BLACKBOX: Output clamped"
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

    /// Annotate capability mode change
    pub fn annotate_capability_mode_changed(
        &mut self,
        axis_name: &str,
        new_mode: flight_core::profile::CapabilityMode,
    ) {
        if !self.enabled {
            return;
        }

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let new_mode_str = match new_mode {
            flight_core::profile::CapabilityMode::Full => "full",
            flight_core::profile::CapabilityMode::Demo => "demo",
            flight_core::profile::CapabilityMode::Kid => "kid",
        };

        let event = BlackboxEvent::CapabilityModeChanged {
            timestamp,
            axis_name: axis_name.to_string(),
            old_mode: "unknown".to_string(), // We don't track previous mode for now
            new_mode: new_mode_str.to_string(),
        };

        self.add_event(event);

        info!(
            axis = axis_name,
            new_mode = new_mode_str,
            "Capability mode changed"
        );
    }

    /// Annotate output clamping due to capability limits
    pub fn annotate_output_clamped(
        &mut self,
        axis_name: &str,
        original_output: f32,
        clamped_output: f32,
        capability_mode: flight_core::profile::CapabilityMode,
    ) {
        if !self.enabled {
            return;
        }

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let mode_str = match capability_mode {
            flight_core::profile::CapabilityMode::Full => "full",
            flight_core::profile::CapabilityMode::Demo => "demo",
            flight_core::profile::CapabilityMode::Kid => "kid",
        };

        let event = BlackboxEvent::OutputClamped {
            timestamp,
            axis_name: axis_name.to_string(),
            original_output,
            clamped_output,
            capability_mode: mode_str.to_string(),
            limit_type: "max_axis_output".to_string(),
        };

        self.add_event(event);

        debug!(
            axis = axis_name,
            original = original_output,
            clamped = clamped_output,
            mode = mode_str,
            "Output clamped due to capability limits"
        );
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
            severity: "Unknown".to_string(),      // Metadata doesn't have severity
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
            BlackboxEvent::ConflictDetected {
                axis_name,
                conflict,
                ..
            } => {
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
            BlackboxEvent::ResolutionApplied {
                axis_name,
                resolution_type,
                success,
                ..
            } => {
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

    // ------------------------------------------------------------------
    // Additional unit-test coverage for blackbox annotator.
    // NOTE: Test code runs on the test harness thread, NOT the RT spine.
    // Allocations (Vec/String) in these tests are explicitly OK per ADR-004
    // because they execute off the real-time path.
    // ------------------------------------------------------------------

    #[test]
    fn test_default_impl_matches_new() {
        // Default must mirror BlackboxAnnotator::new(): enabled, empty buffer.
        let annotator: BlackboxAnnotator = Default::default();
        assert!(annotator.is_enabled());
        assert_eq!(annotator.get_buffered_events().len(), 0);
    }

    #[test]
    fn test_set_enabled_clears_buffer() {
        // Disabling an annotator must drop any pending events.
        let mut annotator = BlackboxAnnotator::new();
        annotator.set_max_buffer_size(100); // ensure flush is not triggered
        annotator.annotate_conflict_cleared("axis", "test reason");
        assert_eq!(annotator.get_buffered_events().len(), 1);

        annotator.set_enabled(false);
        assert!(!annotator.is_enabled());
        assert_eq!(annotator.get_buffered_events().len(), 0);

        // Re-enabling should not resurrect anything.
        annotator.set_enabled(true);
        assert!(annotator.is_enabled());
        assert_eq!(annotator.get_buffered_events().len(), 0);
    }

    #[test]
    fn test_clear_buffer_without_flush() {
        // clear_buffer() must not invoke the flush path; it just empties.
        let mut annotator = BlackboxAnnotator::new();
        annotator.set_max_buffer_size(100);
        annotator.annotate_conflict_cleared("axis_a", "r1");
        annotator.annotate_conflict_cleared("axis_b", "r2");
        assert_eq!(annotator.get_buffered_events().len(), 2);

        annotator.clear_buffer();
        assert_eq!(annotator.get_buffered_events().len(), 0);
    }

    #[test]
    fn test_flush_empty_buffer_is_noop() {
        // Flushing with nothing buffered should be safe.
        let mut annotator = BlackboxAnnotator::new();
        assert_eq!(annotator.get_buffered_events().len(), 0);
        annotator.flush();
        assert_eq!(annotator.get_buffered_events().len(), 0);
    }

    #[test]
    fn test_annotate_conflict_cleared_event() {
        let mut annotator = BlackboxAnnotator::new();
        annotator.set_max_buffer_size(100);
        annotator.annotate_conflict_cleared("yoke_pitch", "user_dismissed");
        let events = annotator.get_buffered_events();
        assert_eq!(events.len(), 1);
        match &events[0] {
            BlackboxEvent::ConflictCleared {
                axis_name, reason, ..
            } => {
                assert_eq!(axis_name, "yoke_pitch");
                assert_eq!(reason, "user_dismissed");
            }
            other => panic!("Expected ConflictCleared, got {:?}", other),
        }
    }

    #[test]
    fn test_annotate_pre_fault_capture_event() {
        let mut annotator = BlackboxAnnotator::new();
        annotator.set_max_buffer_size(100);
        annotator.annotate_pre_fault_capture("throttle_l", 2000, 500);
        let events = annotator.get_buffered_events();
        assert_eq!(events.len(), 1);
        match &events[0] {
            BlackboxEvent::PreFaultCapture {
                axis_name,
                capture_duration_ms,
                sample_count,
                ..
            } => {
                assert_eq!(axis_name, "throttle_l");
                assert_eq!(*capture_duration_ms, 2000);
                assert_eq!(*sample_count, 500);
            }
            other => panic!("Expected PreFaultCapture, got {:?}", other),
        }
    }

    #[test]
    fn test_annotate_capability_mode_changed_all_modes() {
        // Each CapabilityMode variant must map to the correct string label.
        use flight_core::profile::CapabilityMode;
        let cases = [
            (CapabilityMode::Full, "full"),
            (CapabilityMode::Demo, "demo"),
            (CapabilityMode::Kid, "kid"),
        ];
        for (mode, expected) in cases {
            let mut annotator = BlackboxAnnotator::new();
            annotator.set_max_buffer_size(100);
            annotator.annotate_capability_mode_changed("rudder", mode);
            let events = annotator.get_buffered_events();
            assert_eq!(events.len(), 1);
            match &events[0] {
                BlackboxEvent::CapabilityModeChanged {
                    axis_name,
                    old_mode,
                    new_mode,
                    ..
                } => {
                    assert_eq!(axis_name, "rudder");
                    // Current implementation does not track previous mode.
                    assert_eq!(old_mode, "unknown");
                    assert_eq!(new_mode, expected);
                }
                other => panic!("Expected CapabilityModeChanged, got {:?}", other),
            }
        }
    }

    #[test]
    fn test_annotate_output_clamped_all_modes() {
        use flight_core::profile::CapabilityMode;
        let cases = [
            (CapabilityMode::Full, "full"),
            (CapabilityMode::Demo, "demo"),
            (CapabilityMode::Kid, "kid"),
        ];
        for (mode, expected) in cases {
            let mut annotator = BlackboxAnnotator::new();
            annotator.set_max_buffer_size(100);
            annotator.annotate_output_clamped("aileron", 1.2, 1.0, mode);
            let events = annotator.get_buffered_events();
            assert_eq!(events.len(), 1);
            match &events[0] {
                BlackboxEvent::OutputClamped {
                    axis_name,
                    original_output,
                    clamped_output,
                    capability_mode,
                    limit_type,
                    ..
                } => {
                    assert_eq!(axis_name, "aileron");
                    assert_eq!(*original_output, 1.2);
                    assert_eq!(*clamped_output, 1.0);
                    assert_eq!(capability_mode, expected);
                    assert_eq!(limit_type, "max_axis_output");
                }
                other => panic!("Expected OutputClamped, got {:?}", other),
            }
        }
    }

    #[test]
    fn test_resolution_failure_recorded() {
        // success=false should still record an event, but as a failure.
        let mut annotator = BlackboxAnnotator::new();
        annotator.set_max_buffer_size(100);
        let details = ResolutionDetails::new(
            "ApplyGainCompensation".to_string(),
            HashMap::new(),
            vec![],
            None,
            false,
        );
        annotator.annotate_resolution_applied("axis", "ApplyGainCompensation", false, details);
        let events = annotator.get_buffered_events();
        assert_eq!(events.len(), 1);
        match &events[0] {
            BlackboxEvent::ResolutionApplied {
                success, details, ..
            } => {
                assert!(!*success);
                assert!(!details.verification_passed);
                assert!(details.backup_path.is_none());
                assert!(details.affected_files.is_empty());
            }
            other => panic!("Expected ResolutionApplied, got {:?}", other),
        }
    }

    #[test]
    fn test_disabled_annotator_ignores_all_event_kinds() {
        // When disabled, none of the annotate_* methods should buffer anything.
        use flight_core::profile::CapabilityMode;
        let mut annotator = BlackboxAnnotator::disabled();
        let conflict = create_test_conflict();
        let details = ResolutionDetails::new("X".to_string(), HashMap::new(), vec![], None, true);
        annotator.annotate_conflict_detected("a", &conflict);
        annotator.annotate_resolution_applied("a", "X", true, details);
        annotator.annotate_conflict_cleared("a", "r");
        annotator.annotate_pre_fault_capture("a", 1, 1);
        annotator.annotate_capability_mode_changed("a", CapabilityMode::Full);
        annotator.annotate_output_clamped("a", 1.0, 0.5, CapabilityMode::Kid);
        assert_eq!(annotator.get_buffered_events().len(), 0);
    }

    #[test]
    fn test_set_max_buffer_size_triggers_flush() {
        // Shrinking the buffer below current occupancy must auto-flush.
        let mut annotator = BlackboxAnnotator::new();
        annotator.set_max_buffer_size(100);
        annotator.annotate_conflict_cleared("axis_a", "r1");
        annotator.annotate_conflict_cleared("axis_b", "r2");
        annotator.annotate_conflict_cleared("axis_c", "r3");
        assert_eq!(annotator.get_buffered_events().len(), 3);

        // New max=2, occupancy=3 -> flush triggered.
        annotator.set_max_buffer_size(2);
        assert_eq!(annotator.get_buffered_events().len(), 0);
    }

    #[test]
    fn test_set_max_buffer_size_no_flush_when_under_limit() {
        // Resizing larger or to an unmet limit must not flush.
        let mut annotator = BlackboxAnnotator::new();
        annotator.set_max_buffer_size(100);
        annotator.annotate_conflict_cleared("axis_a", "r1");
        annotator.annotate_conflict_cleared("axis_b", "r2");
        assert_eq!(annotator.get_buffered_events().len(), 2);

        // New max=10, occupancy=2 -> no flush.
        annotator.set_max_buffer_size(10);
        assert_eq!(annotator.get_buffered_events().len(), 2);
    }

    #[test]
    fn test_conflict_metadata_to_conflict_data_conversion() {
        // ConflictMetadata-only conversion uses placeholder type/severity.
        let metadata = ConflictMetadata {
            sim_curve_strength: 0.7,
            profile_curve_strength: 0.1,
            combined_nonlinearity: 0.25,
            test_inputs: vec![0.0, 0.25, 0.5, 0.75, 1.0],
            expected_outputs: vec![0.0, 0.25, 0.5, 0.75, 1.0],
            actual_outputs: vec![0.0, 0.20, 0.45, 0.70, 1.0],
            detection_timestamp: Instant::now(),
        };
        let data: ConflictData = (&metadata).into();
        assert_eq!(data.conflict_type, "Unknown");
        assert_eq!(data.severity, "Unknown");
        assert_eq!(data.description, "Metadata only");
        assert_eq!(data.sim_curve_strength, 0.7);
        assert_eq!(data.profile_curve_strength, 0.1);
        assert_eq!(data.combined_nonlinearity, 0.25);
        assert_eq!(data.test_inputs.len(), 5);
        assert_eq!(data.expected_outputs.len(), 5);
        assert_eq!(data.actual_outputs.len(), 5);
    }

    #[test]
    fn test_resolution_details_new_defaults_metrics_to_none() {
        let mut params = HashMap::new();
        params.insert("strength".to_string(), "0.5".to_string());
        let details = ResolutionDetails::new(
            "ReduceCurveStrength".to_string(),
            params.clone(),
            vec!["axis.cfg".to_string()],
            Some("/tmp/backup".to_string()),
            true,
        );
        assert_eq!(details.resolution_type, "ReduceCurveStrength");
        assert_eq!(
            details.parameters.get("strength").map(String::as_str),
            Some("0.5")
        );
        assert_eq!(details.affected_files, vec!["axis.cfg".to_string()]);
        assert_eq!(details.backup_path.as_deref(), Some("/tmp/backup"));
        assert!(details.verification_passed);
        assert!(details.before_metrics.is_none());
        assert!(details.after_metrics.is_none());
    }

    #[test]
    fn test_resolution_details_with_metrics_attaches_both() {
        let conflict = create_test_conflict();
        let before: ConflictData = (&conflict).into();
        let after: ConflictData = (&conflict).into();
        let details = ResolutionDetails::new(
            "DisableSimCurve".to_string(),
            HashMap::new(),
            vec![],
            None,
            true,
        )
        .with_metrics(Some(before), Some(after));
        assert!(details.before_metrics.is_some());
        assert!(details.after_metrics.is_some());

        // Calling with_metrics with None should clear them back out.
        let cleared = details.with_metrics(None, None);
        assert!(cleared.before_metrics.is_none());
        assert!(cleared.after_metrics.is_none());
    }

    #[test]
    fn test_buffer_flush_threshold_exact_boundary() {
        // Adding the Nth event when max=N must trigger a flush, not require N+1.
        let mut annotator = BlackboxAnnotator::new();
        annotator.set_max_buffer_size(3);
        annotator.annotate_conflict_cleared("a", "1");
        annotator.annotate_conflict_cleared("a", "2");
        assert_eq!(annotator.get_buffered_events().len(), 2);
        annotator.annotate_conflict_cleared("a", "3");
        // The third push reaches len == max_buffer_size, which triggers flush.
        assert_eq!(annotator.get_buffered_events().len(), 0);
    }

    #[test]
    fn test_get_buffered_events_returns_chronological_order() {
        // Events must be retained in insertion order before flush.
        let mut annotator = BlackboxAnnotator::new();
        annotator.set_max_buffer_size(100);
        annotator.annotate_conflict_cleared("axis_a", "first");
        annotator.annotate_conflict_cleared("axis_b", "second");
        annotator.annotate_conflict_cleared("axis_c", "third");
        let events = annotator.get_buffered_events();
        assert_eq!(events.len(), 3);
        let reasons: Vec<&str> = events
            .iter()
            .map(|e| match e {
                BlackboxEvent::ConflictCleared { reason, .. } => reason.as_str(),
                _ => panic!("unexpected variant"),
            })
            .collect();
        assert_eq!(reasons, vec!["first", "second", "third"]);
    }

    #[test]
    fn test_conflict_data_round_trips_input_vectors() {
        // From<&CurveConflict> must preserve the full test_inputs/outputs vectors.
        let conflict = create_test_conflict();
        let data: ConflictData = (&conflict).into();
        assert_eq!(data.test_inputs, vec![0.0, 0.5, 1.0]);
        assert_eq!(data.expected_outputs, vec![0.0, 0.5, 1.0]);
        assert_eq!(data.actual_outputs, vec![0.0, 0.3, 1.0]);
        assert_eq!(data.sim_curve_strength, 0.3);
        assert_eq!(data.profile_curve_strength, 0.2);
    }

    #[test]
    fn test_unused_resolution_type_import_compiles() {
        // Sanity: ConflictResolution / ResolutionType from the parent crate
        // are reachable from the test module (used by existing tests via path).
        // This guards against accidental visibility regressions.
        let _ = ResolutionType::DisableSimCurve;
        let _ = ConflictResolution {
            resolution_type: ResolutionType::DisableProfileCurve,
            description: "noop".to_string(),
            estimated_improvement: 0.0,
            requires_sim_restart: false,
            parameters: HashMap::new(),
        };
    }
}
