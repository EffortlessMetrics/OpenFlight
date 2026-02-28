// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Real-time axis engine with atomic pipeline swaps
//!
//! The AxisEngine provides the main interface for real-time axis processing
//! with atomic configuration updates and strict timing guarantees.

use crate::{
    AllocationGuard, AxisFrame, BlackboxAnnotator, ConflictDetectorConfig, CurveConflict,
    CurveConflictDetector, Pipeline, PipelineState, RuntimeCounters,
};
use flight_core::profile::{CapabilityContext, CapabilityMode};
use parking_lot::RwLock;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::Instant;

/// Configuration for axis engine
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EngineConfig {
    /// Enable runtime allocation checking
    pub enable_rt_checks: bool,
    /// Maximum processing time per frame (microseconds)
    pub max_frame_time_us: u32,
    /// Enable performance counters
    pub enable_counters: bool,
    /// Enable curve conflict detection
    pub enable_conflict_detection: bool,
    /// Configuration for conflict detector
    pub conflict_detector_config: ConflictDetectorConfig,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            enable_rt_checks: cfg!(feature = "rt-checks"),
            max_frame_time_us: 500, // 0.5ms at 250Hz
            enable_counters: true,
            enable_conflict_detection: true,
            conflict_detector_config: ConflictDetectorConfig::default(),
        }
    }
}

/// Result of pipeline update operation
#[derive(Debug, Clone, PartialEq)]
pub enum UpdateResult {
    /// Update completed successfully
    Success,
    /// Update pending, will be applied at next tick boundary
    Pending,
    /// Update failed due to compilation error
    Failed(String),
    /// Update rejected due to invalid state
    Rejected(String),
}

/// Real-time axis processing engine with atomic swaps
pub struct AxisEngine {
    /// Current active pipeline (atomic pointer)
    active_pipeline: parking_lot::RwLock<Option<Arc<CompiledPipeline>>>,
    /// Pending pipeline for atomic swap
    pending_pipeline: RwLock<Option<Arc<CompiledPipeline>>>,
    /// Engine configuration
    config: EngineConfig,
    /// Runtime performance counters
    counters: Arc<RuntimeCounters>,
    /// Last frame for derivative calculation
    last_frame: RwLock<Option<AxisFrame>>,
    /// Swap acknowledgment counter
    swap_ack_counter: AtomicU64,
    /// Curve conflict detector
    conflict_detector: RwLock<CurveConflictDetector>,
    /// Blackbox annotator for conflict events
    blackbox_annotator: RwLock<BlackboxAnnotator>,
    /// Axis name for conflict detection
    axis_name: String,
    /// Capability enforcement context
    capability_context: RwLock<CapabilityContext>,
}

/// Compiled pipeline with state
struct CompiledPipeline {
    pipeline: Pipeline,
    state: parking_lot::Mutex<PipelineState>,
    version: u64,
}

impl CompiledPipeline {
    /// Process frame through pipeline (RT-safe)
    ///
    /// # Safety
    /// This function assumes exclusive access to the pipeline state
    /// and must not allocate or block.
    #[inline(always)]
    unsafe fn process_frame(&self, frame: &mut AxisFrame) {
        // Try to get state without blocking - if we can't, skip processing
        // This maintains RT guarantees even under contention
        if let Some(mut state) = self.state.try_lock() {
            self.pipeline.process(frame, &mut state);
        }
        // If we can't get the lock, frame passes through unchanged
        // This is better than blocking in RT context
    }
}

impl AxisEngine {
    /// Create new axis engine with default configuration
    pub fn new() -> Self {
        Self::with_config("default".to_string(), EngineConfig::default())
    }

    /// Create new axis engine for specific axis
    pub fn new_for_axis(axis_name: String) -> Self {
        Self::with_config(axis_name, EngineConfig::default())
    }

    /// Create new axis engine with custom configuration
    pub fn with_config(axis_name: String, config: EngineConfig) -> Self {
        let conflict_detector = if config.enable_conflict_detection {
            CurveConflictDetector::with_config(config.conflict_detector_config.clone())
        } else {
            CurveConflictDetector::new()
        };

        Self {
            active_pipeline: parking_lot::RwLock::new(None),
            pending_pipeline: RwLock::new(None),
            config,
            counters: Arc::new(RuntimeCounters::new()),
            last_frame: RwLock::new(None),
            swap_ack_counter: AtomicU64::new(0),
            conflict_detector: RwLock::new(conflict_detector),
            blackbox_annotator: RwLock::new(BlackboxAnnotator::new()),
            axis_name,
            capability_context: RwLock::new(CapabilityContext::for_mode(CapabilityMode::Full)),
        }
    }

    /// Process axis frame through active pipeline (RT-safe)
    ///
    /// This is the main real-time processing function that must maintain
    /// strict timing guarantees and zero allocations.
    #[inline(always)]
    pub fn process(&self, frame: &mut AxisFrame) -> Result<(), ProcessError> {
        // Enable allocation guard if RT checks are enabled
        let _guard = if self.config.enable_rt_checks {
            Some(AllocationGuard::new())
        } else {
            None
        };

        let start_time = if self.config.enable_counters {
            Some(Instant::now())
        } else {
            None
        };

        // Update derivative from last frame
        if let Some(last_frame) = *self.last_frame.read() {
            frame.update_derivative(&last_frame);
        }

        // Check for pending pipeline swap at tick boundary
        self.try_swap_pipeline();

        // Process through active pipeline
        let result = if let Some(pipeline) = self.active_pipeline.read().as_ref() {
            // SAFETY: We have exclusive access to the pipeline state through the engine
            unsafe {
                pipeline.process_frame(frame);
            }
            Ok(())
        } else {
            // No pipeline active - pass through unchanged
            Ok(())
        };

        // Apply capability enforcement clamps
        let _ = self.apply_capability_clamps(frame);

        // Check for RT violations if guard is active
        if self.config.enable_rt_checks && AllocationGuard::allocations_detected() {
            self.counters.increment_rt_allocations();
            AllocationGuard::reset();
        }

        // Update counters and timing
        if let Some(start) = start_time {
            let elapsed = start.elapsed();
            self.counters.record_frame_time(elapsed);

            if elapsed.as_micros() > self.config.max_frame_time_us as u128 {
                self.counters.increment_deadline_misses();
            }
        }

        // Store frame for next derivative calculation
        *self.last_frame.write() = Some(*frame);

        // Add sample to conflict detector if enabled (RT-safe)
        if self.config.enable_conflict_detection
            && let Some(mut detector) = self.conflict_detector.try_write()
        {
            detector.add_sample(&self.axis_name, frame);

            // Check for new conflicts and annotate them
            if let Some(conflict) = detector.get_conflicts(&self.axis_name) {
                // Try to annotate without blocking RT thread
                if let Some(mut annotator) = self.blackbox_annotator.try_write() {
                    annotator.annotate_conflict_detected(&self.axis_name, conflict);
                }
            }
        }
        // If we can't get the lock, skip conflict detection for this frame
        // This maintains RT guarantees

        result
    }

    /// Update pipeline atomically (non-RT thread)
    ///
    /// The new pipeline will be compiled and validated off the RT thread,
    /// then swapped atomically at the next tick boundary.
    pub fn update_pipeline(&self, new_pipeline: Pipeline) -> UpdateResult {
        match self.compile_and_validate(new_pipeline) {
            Ok(compiled) => {
                // Store pending pipeline for atomic swap
                *self.pending_pipeline.write() = Some(compiled);
                UpdateResult::Pending
            }
            Err(e) => UpdateResult::Failed(e.to_string()),
        }
    }

    /// Get current runtime counters
    pub fn counters(&self) -> &RuntimeCounters {
        &self.counters
    }

    /// Get shared reference to counters for external monitoring
    pub fn counters_shared(&self) -> Arc<RuntimeCounters> {
        Arc::clone(&self.counters)
    }

    /// Reset runtime counters
    pub fn reset_counters(&self) {
        self.counters.reset();
    }

    /// Check if pipeline is active
    pub fn has_active_pipeline(&self) -> bool {
        self.active_pipeline.read().is_some()
    }

    /// Get active pipeline version
    pub fn active_version(&self) -> Option<u64> {
        self.active_pipeline.read().as_ref().map(|p| p.version)
    }

    /// Get swap acknowledgment counter (increments on each successful swap)
    pub fn swap_ack_count(&self) -> u64 {
        self.swap_ack_counter.load(Ordering::Relaxed)
    }

    /// Get detected curve conflicts
    pub fn get_curve_conflicts(&self) -> Option<CurveConflict> {
        self.conflict_detector
            .read()
            .get_conflicts(&self.axis_name)
            .cloned()
    }

    /// Clear curve conflicts (after resolution)
    pub fn clear_curve_conflicts(&self) {
        self.conflict_detector
            .write()
            .clear_conflicts(&self.axis_name);

        // Annotate conflict cleared
        if let Some(mut annotator) = self.blackbox_annotator.try_write() {
            annotator.annotate_conflict_cleared(&self.axis_name, "Manual clear");
        }
    }

    /// Get axis name
    pub fn axis_name(&self) -> &str {
        &self.axis_name
    }

    /// Perform one-shot conflict detection with current pipeline
    pub fn detect_conflicts_now(&self) -> Option<CurveConflict> {
        if let Some(_pipeline) = self.active_pipeline.read().as_ref() {
            // This would need access to the pipeline nodes for testing
            // For now, return existing conflicts
            self.get_curve_conflicts()
        } else {
            None
        }
    }

    /// Flush blackbox annotations
    pub fn flush_blackbox(&self) {
        if let Some(mut annotator) = self.blackbox_annotator.try_write() {
            annotator.flush();
        }
    }

    /// Enable/disable blackbox annotations
    pub fn set_blackbox_enabled(&self, enabled: bool) {
        if let Some(mut annotator) = self.blackbox_annotator.try_write() {
            annotator.set_enabled(enabled);
        }
    }

    /// Set capability mode for safety enforcement
    pub fn set_capability_mode(&self, mode: CapabilityMode) {
        let new_context = CapabilityContext::for_mode(mode);
        *self.capability_context.write() = new_context;

        // Log capability mode change for audit trail
        if let Some(mut annotator) = self.blackbox_annotator.try_write() {
            annotator.annotate_capability_mode_changed(&self.axis_name, mode);
        }
    }

    /// Enable/disable capability audit logging without changing mode limits.
    pub fn set_capability_audit_enabled(&self, enabled: bool) {
        self.capability_context.write().audit_enabled = enabled;
    }

    /// Get current capability mode
    pub fn capability_mode(&self) -> CapabilityMode {
        self.capability_context.read().mode
    }

    /// Get current capability context
    pub fn capability_context(&self) -> CapabilityContext {
        self.capability_context.read().clone()
    }

    /// Try to swap pending pipeline atomically (RT-safe)
    #[inline(always)]
    fn try_swap_pipeline(&self) {
        // Try to acquire pending pipeline without blocking
        if let Some(mut pending) = self.pending_pipeline.try_write()
            && let Some(new_pipeline) = pending.take()
        {
            // Atomic swap at tick boundary
            *self.active_pipeline.write() = Some(new_pipeline);

            // Increment acknowledgment counter
            self.swap_ack_counter.fetch_add(1, Ordering::Relaxed);

            // Update counters
            self.counters.increment_pipeline_swaps();
        }
    }

    /// Apply capability enforcement clamps to frame output (RT-safe)
    #[inline(always)]
    fn apply_capability_clamps(&self, frame: &mut AxisFrame) -> Result<(), ProcessError> {
        // Try to get capability context without blocking
        if let Some(context) = self.capability_context.try_read() {
            let original_output = frame.out;

            // Clamp output magnitude to capability limits
            let max_output = context.limits.max_axis_output;
            if frame.out.abs() > max_output {
                let original_abs = frame.out.abs();
                frame.out = frame.out.signum() * max_output;

                // Record the clamp event in counters (used by capability_service reporting)
                self.counters.increment_capability_clamps(original_abs);

                // Log clamping event for audit trail if enabled
                if context.audit_enabled {
                    // Try to annotate without blocking RT thread
                    if let Some(mut annotator) = self.blackbox_annotator.try_write() {
                        annotator.annotate_output_clamped(
                            &self.axis_name,
                            original_output,
                            frame.out,
                            context.mode,
                        );
                    }
                }
            }
        }
        // If we can't get the context, skip clamping to maintain RT guarantees

        Ok(())
    }

    /// Compile and validate new pipeline (non-RT)
    fn compile_and_validate(
        &self,
        pipeline: Pipeline,
    ) -> Result<Arc<CompiledPipeline>, CompileError> {
        // Validate pipeline structure
        pipeline.validate().map_err(CompileError::Pipeline)?;

        // Create state for pipeline
        let state = pipeline.create_state();

        // Validate state buffer
        if !state.validate() {
            return Err(CompileError::InvalidState);
        }

        let version = self.counters.pipeline_swaps() + 1;

        Ok(Arc::new(CompiledPipeline {
            pipeline,
            state: parking_lot::Mutex::new(state),
            version,
        }))
    }
}

impl Default for AxisEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur during frame processing
#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    #[error("No active pipeline")]
    NoPipeline,
    #[error("Pipeline execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Deadline miss: processing took too long")]
    DeadlineMiss,
    #[error("Allocation detected in RT path")]
    AllocationViolation,
}

/// Errors that can occur during pipeline compilation
#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    #[error("Pipeline validation failed: {0}")]
    Pipeline(#[from] crate::pipeline::PipelineError),
    #[error("Invalid state configuration")]
    InvalidState,
    #[error("Node compilation failed: {0}")]
    NodeCompilation(String),
}

// Ensure AxisEngine is Send + Sync for multi-threaded use
unsafe impl Send for AxisEngine {}
unsafe impl Sync for AxisEngine {}

#[cfg(test)]
mod tests {
    use super::*;
    // use crate::nodes::{DeadzoneNode, CurveNode};

    #[test]
    fn test_engine_creation() {
        let engine = AxisEngine::new();
        assert!(!engine.has_active_pipeline());
        assert_eq!(engine.active_version(), None);
    }

    #[test]
    fn test_process_without_pipeline() {
        let engine = AxisEngine::new();
        let mut frame = AxisFrame::new(0.5, 1000);

        // Should pass through without error
        assert!(engine.process(&mut frame).is_ok());
        assert_eq!(frame.out, 0.5);
    }

    #[test]
    fn test_counters() {
        let engine = AxisEngine::new();
        let counters = engine.counters();

        assert_eq!(counters.frames_processed(), 0);
        assert_eq!(counters.pipeline_swaps(), 0);
        assert_eq!(counters.deadline_misses(), 0);
    }

    #[test]
    fn test_capability_mode_setting() {
        let engine = AxisEngine::new_for_axis("test_axis".to_string());

        // Default should be full mode
        assert_eq!(engine.capability_mode(), CapabilityMode::Full);

        // Set to demo mode
        engine.set_capability_mode(CapabilityMode::Demo);
        assert_eq!(engine.capability_mode(), CapabilityMode::Demo);

        // Set to kid mode
        engine.set_capability_mode(CapabilityMode::Kid);
        assert_eq!(engine.capability_mode(), CapabilityMode::Kid);
    }

    #[test]
    fn test_output_clamping() {
        let engine = AxisEngine::new_for_axis("test_axis".to_string());

        // Set to kid mode (50% max output)
        engine.set_capability_mode(CapabilityMode::Kid);

        // Test frame with high output
        let mut frame = AxisFrame::new(0.8, 1000); // 80% input
        frame.out = 0.8; // Simulate pipeline output

        // Process frame - should clamp output to 50%
        let result = engine.process(&mut frame);
        assert!(result.is_ok());
        assert_eq!(frame.out, 0.5); // Should be clamped to kid mode limit

        // Test negative output
        let mut frame = AxisFrame::new(-0.8, 2000);
        frame.out = -0.8;

        let result = engine.process(&mut frame);
        assert!(result.is_ok());
        assert_eq!(frame.out, -0.5); // Should be clamped to -50%
    }

    #[test]
    fn test_no_clamping_in_full_mode() {
        let engine = AxisEngine::new_for_axis("test_axis".to_string());

        // Default is full mode
        assert_eq!(engine.capability_mode(), CapabilityMode::Full);

        // Test frame with high output
        let mut frame = AxisFrame::new(0.9, 1000);
        frame.out = 0.9;

        // Process frame - should not clamp in full mode
        let result = engine.process(&mut frame);
        assert!(result.is_ok());
        assert_eq!(frame.out, 0.9); // Should remain unchanged
    }

    #[test]
    fn test_demo_mode_clamping() {
        let engine = AxisEngine::new_for_axis("test_axis".to_string());

        // Set to demo mode (80% max output)
        engine.set_capability_mode(CapabilityMode::Demo);

        // Test frame with output within demo limits
        let mut frame = AxisFrame::new(0.7, 1000);
        frame.out = 0.7;

        let result = engine.process(&mut frame);
        assert!(result.is_ok());
        assert_eq!(frame.out, 0.7); // Should remain unchanged

        // Test frame with output exceeding demo limits
        let mut frame = AxisFrame::new(0.9, 2000);
        frame.out = 0.9;

        let result = engine.process(&mut frame);
        assert!(result.is_ok());
        assert_eq!(frame.out, 0.8); // Should be clamped to demo mode limit
    }
}
