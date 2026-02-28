// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Compiled axis processing pipeline with SoA state layout
//!
//! The pipeline uses a compile-to-function-pointer approach with Structure-of-Arrays
//! state layout for optimal cache performance and zero-allocation execution.

use crate::{AxisFrame, Node, NodeId};
use std::cell::Cell;
use std::sync::Arc;

/// Function pointer type for compiled pipeline steps
type StepFn = unsafe fn(frame: *mut AxisFrame, state_ptr: *mut u8);

/// Compiled pipeline with optimized memory layout
#[derive(Debug)]
pub struct Pipeline {
    /// Static function pointers for each pipeline step
    step_functions: Vec<StepFn>,
    /// Node metadata for debugging and validation
    node_metadata: Vec<NodeMetadata>,
    /// Total state size needed for SoA layout
    total_state_size: usize,
    /// Source nodes for state initialization
    source_nodes: Option<Vec<Arc<dyn Node>>>,
}

/// Metadata for pipeline nodes
#[derive(Debug, Clone)]
pub struct NodeMetadata {
    pub node_id: NodeId,
    pub node_type: &'static str,
    pub state_offset: usize,
    pub state_size: usize,
}

/// Runtime state for pipeline execution with SoA layout
#[derive(Debug)]
pub struct PipelineState {
    /// Pre-allocated state buffer aligned to 64-byte boundaries
    state_buffer: Vec<u8>,
    /// State offsets for each node
    state_offsets: Vec<usize>,
}

impl Pipeline {
    /// Create new empty pipeline
    pub(crate) fn new() -> Self {
        Self {
            step_functions: Vec::new(),
            node_metadata: Vec::new(),
            total_state_size: 0,
            source_nodes: None,
        }
    }

    /// Set source nodes for state initialization
    pub(crate) fn set_source_nodes(&mut self, nodes: Vec<Arc<dyn Node>>) {
        self.source_nodes = Some(nodes);
    }

    /// Add compiled node to pipeline
    pub(crate) fn add_compiled_node(
        &mut self,
        step_fn: StepFn,
        node_id: NodeId,
        node_type: &'static str,
        state_size: usize,
    ) {
        let state_offset = align_to_64(self.total_state_size);

        self.step_functions.push(step_fn);
        self.node_metadata.push(NodeMetadata {
            node_id,
            node_type,
            state_offset,
            state_size,
        });

        self.total_state_size = state_offset + state_size;
    }

    /// Create runtime state for this pipeline
    pub fn create_state(&self) -> PipelineState {
        let aligned_size = align_to_64(self.total_state_size.max(64));

        // Create properly aligned buffer
        let mut state_buffer = vec![0u8; aligned_size];

        // Ensure buffer is aligned to 64-byte boundary
        let ptr = state_buffer.as_mut_ptr();
        let alignment = ptr as usize % 64;
        if alignment != 0 {
            // Reallocate with proper alignment
            let extra = 64 - alignment;
            state_buffer.reserve(extra);
            state_buffer.resize(aligned_size + extra, 0);

            // Find aligned start within buffer
            let new_ptr = state_buffer.as_mut_ptr();
            let aligned_offset = ((new_ptr as usize + 63) & !63) - new_ptr as usize;
            state_buffer.drain(0..aligned_offset);
            state_buffer.truncate(aligned_size);
        }

        let state_offsets = self
            .node_metadata
            .iter()
            .map(|meta| meta.state_offset)
            .collect();

        let mut state = PipelineState {
            state_buffer,
            state_offsets,
        };

        // Initialize state with source nodes if available
        if let Some(ref nodes) = self.source_nodes {
            unsafe {
                state.init_with_nodes(nodes);
            }
        }

        state
    }

    /// Process frame through compiled pipeline (zero allocations)
    #[inline(always)]
    pub fn process(&self, frame: &mut AxisFrame, state: &mut PipelineState) {
        debug_assert_eq!(
            self.step_functions.len(),
            state.state_offsets.len(),
            "Pipeline and state mismatch"
        );

        let base_ptr = state.state_buffer.as_mut_ptr();

        // If we have source nodes, use them directly for better integration
        if let Some(ref nodes) = self.source_nodes {
            for (node, &state_offset) in nodes.iter().zip(&state.state_offsets) {
                unsafe {
                    let state_ptr = base_ptr.add(state_offset);
                    node.step_soa(frame, state_ptr);
                }
            }
        } else {
            // Fallback to function pointers
            let frame_ptr = frame as *mut AxisFrame;
            for (step_fn, &state_offset) in self.step_functions.iter().zip(&state.state_offsets) {
                unsafe {
                    let state_ptr = base_ptr.add(state_offset);
                    step_fn(frame_ptr, state_ptr);
                }
            }
        }
    }

    /// Get pipeline metadata for debugging
    pub fn metadata(&self) -> &[NodeMetadata] {
        &self.node_metadata
    }

    /// Get total state size required
    pub fn state_size(&self) -> usize {
        self.total_state_size
    }

    /// Validate pipeline integrity
    pub fn validate(&self) -> Result<(), PipelineError> {
        if self.step_functions.len() != self.node_metadata.len() {
            return Err(PipelineError::MetadataMismatch);
        }

        // Validate state layout alignment
        for meta in &self.node_metadata {
            if meta.state_offset % 8 != 0 {
                return Err(PipelineError::AlignmentError);
            }
        }

        Ok(())
    }
}

impl PipelineState {
    /// Initialize state for given nodes
    #[allow(unsafe_op_in_unsafe_fn)]
    pub(crate) unsafe fn init_with_nodes(&mut self, nodes: &[Arc<dyn Node>]) {
        let base_ptr = self.state_buffer.as_mut_ptr();

        for (node, &offset) in nodes.iter().zip(&self.state_offsets) {
            let state_ptr = base_ptr.add(offset);
            node.init_state(state_ptr);
        }
    }

    /// Get state buffer size
    pub fn buffer_size(&self) -> usize {
        self.state_buffer.len()
    }

    /// Validate state buffer integrity
    pub fn validate(&self) -> bool {
        !self.state_buffer.is_empty()
    }
}

/// Pipeline compilation and validation errors
#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("Pipeline metadata mismatch")]
    MetadataMismatch,
    #[error("State buffer alignment error")]
    AlignmentError,
    #[error("Invalid node configuration")]
    InvalidNode,
}

/// Align size to 64-byte boundary for cache line optimization
#[inline]
fn align_to_64(size: usize) -> usize {
    (size + 63) & !63
}

// ---------------------------------------------------------------------------
// Composable axis processing pipeline
// ---------------------------------------------------------------------------

/// Trait for composable axis processing stages.
pub trait AxisStage: Send + Sync {
    /// Returns the name of this stage.
    fn name(&self) -> &str;
    /// Processes an input value through this stage.
    fn process(&self, input: f64, dt_secs: f64) -> f64;
}

/// Composable axis processing pipeline with per-stage bypass.
pub struct AxisPipeline {
    stages: Vec<Box<dyn AxisStage>>,
    bypass: Vec<bool>,
}

impl AxisPipeline {
    /// Creates a new empty pipeline.
    #[must_use]
    pub fn new() -> Self {
        Self {
            stages: Vec::new(),
            bypass: Vec::new(),
        }
    }

    /// Appends a stage to the pipeline.
    pub fn add_stage(&mut self, stage: Box<dyn AxisStage>) {
        self.stages.push(stage);
        self.bypass.push(false);
    }

    /// Processes a value through all non-bypassed stages.
    #[must_use]
    pub fn process(&self, input: f64, dt_secs: f64) -> f64 {
        let mut value = input;
        for (stage, &bypassed) in self.stages.iter().zip(&self.bypass) {
            if !bypassed {
                value = stage.process(value, dt_secs);
            }
        }
        value
    }

    /// Bypasses the stage at `idx`. Returns `true` if the index was valid.
    pub fn bypass_stage(&mut self, idx: usize) -> bool {
        if let Some(b) = self.bypass.get_mut(idx) {
            *b = true;
            true
        } else {
            false
        }
    }

    /// Re-enables the stage at `idx`. Returns `true` if the index was valid.
    pub fn enable_stage(&mut self, idx: usize) -> bool {
        if let Some(b) = self.bypass.get_mut(idx) {
            *b = false;
            true
        } else {
            false
        }
    }

    /// Returns the number of stages in the pipeline.
    #[must_use]
    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    /// Returns the names of all stages in order.
    #[must_use]
    pub fn stage_names(&self) -> Vec<&str> {
        self.stages.iter().map(|s| s.name()).collect()
    }

    /// Returns a slice of all stages for introspection.
    #[must_use]
    pub fn stages(&self) -> &[Box<dyn AxisStage>] {
        &self.stages
    }

    /// Inserts a stage at the given index, shifting later stages right.
    ///
    /// If `index` is beyond the current length, the stage is appended.
    pub fn insert_stage(&mut self, index: usize, stage: Box<dyn AxisStage>) {
        let clamped = index.min(self.stages.len());
        self.stages.insert(clamped, stage);
        self.bypass.insert(clamped, false);
    }

    /// Removes and returns the stage at the given index.
    ///
    /// Returns `None` if the index is out of bounds.
    pub fn remove_stage(&mut self, index: usize) -> Option<Box<dyn AxisStage>> {
        if index >= self.stages.len() {
            return None;
        }
        self.bypass.remove(index);
        Some(self.stages.remove(index))
    }

    /// Runs the pipeline and returns per-stage input/output diagnostics.
    ///
    /// **Warning**: calling this modifies stateful stages (e.g. smoothing).
    #[must_use]
    pub fn diagnostics(&self, input: f64, dt_secs: f64) -> Vec<(&str, f64, f64)> {
        let mut result = Vec::with_capacity(self.stages.len());
        let mut value = input;
        for (stage, &bypassed) in self.stages.iter().zip(&self.bypass) {
            let stage_input = value;
            if !bypassed {
                value = stage.process(value, dt_secs);
            }
            result.push((stage.name(), stage_input, value));
        }
        result
    }
}

impl Default for AxisPipeline {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Built-in stages
// ---------------------------------------------------------------------------

/// Deadzone stage: eliminates input noise near center and rescales.
pub struct DeadzoneStage {
    /// Inner deadzone radius (values below this map to zero).
    pub inner: f64,
    /// Outer deadzone radius (values above this map to ±1).
    pub outer: f64,
}

impl AxisStage for DeadzoneStage {
    fn name(&self) -> &str {
        "deadzone"
    }

    fn process(&self, input: f64, _dt_secs: f64) -> f64 {
        let abs_in = input.abs();
        if abs_in <= self.inner {
            0.0
        } else if abs_in >= self.outer {
            input.signum()
        } else {
            let range = self.outer - self.inner;
            if range <= 0.0 {
                return 0.0;
            }
            input.signum() * (abs_in - self.inner) / range
        }
    }
}

/// Curve stage: applies exponential response shaping.
///
/// `expo = 0.0` is linear; positive values reduce sensitivity near center.
pub struct CurveStage {
    pub expo: f64,
}

impl AxisStage for CurveStage {
    fn name(&self) -> &str {
        "curve"
    }

    fn process(&self, input: f64, _dt_secs: f64) -> f64 {
        input.signum() * input.abs().powf(1.0 + self.expo)
    }
}

/// Sensitivity stage: scales input by a multiplier.
pub struct SensitivityStage {
    pub multiplier: f64,
}

impl AxisStage for SensitivityStage {
    fn name(&self) -> &str {
        "sensitivity"
    }

    fn process(&self, input: f64, _dt_secs: f64) -> f64 {
        input * self.multiplier
    }
}

/// Clamp stage: restricts output to a range.
pub struct ClampStage {
    pub min: f64,
    pub max: f64,
}

impl AxisStage for ClampStage {
    fn name(&self) -> &str {
        "clamp"
    }

    fn process(&self, input: f64, _dt_secs: f64) -> f64 {
        input.clamp(self.min, self.max)
    }
}

/// Smoothing stage: exponential moving-average filter.
///
/// `alpha` controls responsiveness: 1.0 = no smoothing, 0.0 = fully damped.
pub struct SmoothingStage {
    pub alpha: f64,
    pub prev: Cell<f64>,
}

// SAFETY: SmoothingStage is used exclusively from the single-threaded RT spine.
// The Cell interior mutability is safe because the pipeline is never shared
// across threads concurrently.
unsafe impl Sync for SmoothingStage {}

impl SmoothingStage {
    /// Creates a new smoothing stage with the given alpha and initial value of 0.
    #[must_use]
    pub fn new(alpha: f64) -> Self {
        Self {
            alpha,
            prev: Cell::new(0.0),
        }
    }
}

impl AxisStage for SmoothingStage {
    fn name(&self) -> &str {
        "smoothing"
    }

    fn process(&self, input: f64, _dt_secs: f64) -> f64 {
        let output = self.alpha * input + (1.0 - self.alpha) * self.prev.get();
        self.prev.set(output);
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alignment() {
        assert_eq!(align_to_64(0), 0);
        assert_eq!(align_to_64(1), 64);
        assert_eq!(align_to_64(64), 64);
        assert_eq!(align_to_64(65), 128);
    }

    #[test]
    fn test_pipeline_state_creation() {
        let pipeline = Pipeline::new();
        let state = pipeline.create_state();
        assert!(state.validate());
    }

    // --- AxisPipeline tests ---

    #[test]
    fn test_empty_pipeline_passthrough() {
        let pipeline = AxisPipeline::new();
        assert!((pipeline.process(0.75, 0.004) - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn test_deadzone_stage_center() {
        let stage = DeadzoneStage {
            inner: 0.05,
            outer: 1.0,
        };
        assert!((stage.process(0.03, 0.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_deadzone_stage_full_deflection() {
        let stage = DeadzoneStage {
            inner: 0.05,
            outer: 1.0,
        };
        assert!((stage.process(1.0, 0.0) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_deadzone_stage_rescale() {
        let stage = DeadzoneStage {
            inner: 0.1,
            outer: 1.0,
        };
        let out = stage.process(0.55, 0.0);
        assert!((out - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_deadzone_stage_negative() {
        let stage = DeadzoneStage {
            inner: 0.1,
            outer: 1.0,
        };
        let out = stage.process(-0.55, 0.0);
        assert!((out - (-0.5)).abs() < 1e-10);
    }

    #[test]
    fn test_curve_stage_linear() {
        let stage = CurveStage { expo: 0.0 };
        assert!((stage.process(0.5, 0.0) - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_curve_stage_expo() {
        let stage = CurveStage { expo: 1.0 };
        // 0.5^(1+1) = 0.5^2 = 0.25
        assert!((stage.process(0.5, 0.0) - 0.25).abs() < 1e-10);
    }

    #[test]
    fn test_sensitivity_stage() {
        let stage = SensitivityStage { multiplier: 2.0 };
        assert!((stage.process(0.5, 0.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_clamp_stage() {
        let stage = ClampStage {
            min: -0.5,
            max: 0.5,
        };
        assert!((stage.process(0.8, 0.0) - 0.5).abs() < 1e-10);
        assert!((stage.process(-0.8, 0.0) - (-0.5)).abs() < 1e-10);
        assert!((stage.process(0.3, 0.0) - 0.3).abs() < 1e-10);
    }

    #[test]
    fn test_smoothing_stage() {
        let stage = SmoothingStage::new(0.5);
        let out1 = stage.process(1.0, 0.0);
        assert!((out1 - 0.5).abs() < 1e-10);
        let out2 = stage.process(1.0, 0.0);
        assert!((out2 - 0.75).abs() < 1e-10);
    }

    #[test]
    fn test_pipeline_multi_stage() {
        let mut pipeline = AxisPipeline::new();
        pipeline.add_stage(Box::new(SensitivityStage { multiplier: 2.0 }));
        pipeline.add_stage(Box::new(ClampStage {
            min: -1.0,
            max: 1.0,
        }));
        let out = pipeline.process(0.8, 0.004);
        assert!((out - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_bypass_stage() {
        let mut pipeline = AxisPipeline::new();
        pipeline.add_stage(Box::new(SensitivityStage { multiplier: 2.0 }));
        assert!(pipeline.bypass_stage(0));
        let out = pipeline.process(0.5, 0.004);
        assert!((out - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_enable_stage_after_bypass() {
        let mut pipeline = AxisPipeline::new();
        pipeline.add_stage(Box::new(SensitivityStage { multiplier: 2.0 }));
        pipeline.bypass_stage(0);
        pipeline.enable_stage(0);
        let out = pipeline.process(0.5, 0.004);
        assert!((out - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_stage_count() {
        let mut pipeline = AxisPipeline::new();
        assert_eq!(pipeline.stage_count(), 0);
        pipeline.add_stage(Box::new(SensitivityStage { multiplier: 1.0 }));
        pipeline.add_stage(Box::new(ClampStage {
            min: -1.0,
            max: 1.0,
        }));
        assert_eq!(pipeline.stage_count(), 2);
    }

    #[test]
    fn test_stage_names() {
        let mut pipeline = AxisPipeline::new();
        pipeline.add_stage(Box::new(DeadzoneStage {
            inner: 0.05,
            outer: 1.0,
        }));
        pipeline.add_stage(Box::new(CurveStage { expo: 0.3 }));
        assert_eq!(pipeline.stage_names(), vec!["deadzone", "curve"]);
    }

    #[test]
    fn test_bypass_out_of_bounds() {
        let mut pipeline = AxisPipeline::new();
        assert!(!pipeline.bypass_stage(0));
        assert!(!pipeline.enable_stage(0));
    }

    #[test]
    fn test_default_pipeline_is_empty() {
        let pipeline = AxisPipeline::default();
        assert_eq!(pipeline.stage_count(), 0);
    }

    // --- insert_stage / remove_stage / stages / diagnostics tests ---

    #[test]
    fn test_insert_stage_at_beginning() {
        let mut pipeline = AxisPipeline::new();
        pipeline.add_stage(Box::new(ClampStage {
            min: -1.0,
            max: 1.0,
        }));
        pipeline.insert_stage(0, Box::new(SensitivityStage { multiplier: 2.0 }));
        assert_eq!(pipeline.stage_count(), 2);
        assert_eq!(pipeline.stage_names(), vec!["sensitivity", "clamp"]);
        let out = pipeline.process(0.8, 0.004);
        assert!((out - 1.0).abs() < 1e-10); // 0.8*2=1.6, clamped to 1.0
    }

    #[test]
    fn test_insert_stage_at_end() {
        let mut pipeline = AxisPipeline::new();
        pipeline.add_stage(Box::new(SensitivityStage { multiplier: 2.0 }));
        pipeline.insert_stage(
            99,
            Box::new(ClampStage {
                min: -1.0,
                max: 1.0,
            }),
        ); // beyond length → appends
        assert_eq!(pipeline.stage_count(), 2);
        assert_eq!(pipeline.stage_names(), vec!["sensitivity", "clamp"]);
    }

    #[test]
    fn test_remove_stage_returns_stage() {
        let mut pipeline = AxisPipeline::new();
        pipeline.add_stage(Box::new(SensitivityStage { multiplier: 2.0 }));
        pipeline.add_stage(Box::new(ClampStage {
            min: -1.0,
            max: 1.0,
        }));
        let removed = pipeline.remove_stage(0);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().name(), "sensitivity");
        assert_eq!(pipeline.stage_count(), 1);
        assert_eq!(pipeline.stage_names(), vec!["clamp"]);
    }

    #[test]
    fn test_remove_stage_out_of_bounds() {
        let mut pipeline = AxisPipeline::new();
        assert!(pipeline.remove_stage(0).is_none());
    }

    #[test]
    fn test_stages_accessor() {
        let mut pipeline = AxisPipeline::new();
        pipeline.add_stage(Box::new(SensitivityStage { multiplier: 1.0 }));
        pipeline.add_stage(Box::new(ClampStage {
            min: -1.0,
            max: 1.0,
        }));
        let stages = pipeline.stages();
        assert_eq!(stages.len(), 2);
        assert_eq!(stages[0].name(), "sensitivity");
        assert_eq!(stages[1].name(), "clamp");
    }

    #[test]
    fn test_diagnostics_captures_per_stage_io() {
        let mut pipeline = AxisPipeline::new();
        pipeline.add_stage(Box::new(SensitivityStage { multiplier: 2.0 }));
        pipeline.add_stage(Box::new(ClampStage {
            min: -1.0,
            max: 1.0,
        }));
        let diag = pipeline.diagnostics(0.4, 0.004);
        assert_eq!(diag.len(), 2);
        assert_eq!(diag[0].0, "sensitivity");
        assert!((diag[0].1 - 0.4).abs() < 1e-10); // input to sensitivity
        assert!((diag[0].2 - 0.8).abs() < 1e-10); // output of sensitivity
        assert_eq!(diag[1].0, "clamp");
        assert!((diag[1].1 - 0.8).abs() < 1e-10); // input to clamp
        assert!((diag[1].2 - 0.8).abs() < 1e-10); // output of clamp (within range)
    }

    #[test]
    fn test_diagnostics_respects_bypass() {
        let mut pipeline = AxisPipeline::new();
        pipeline.add_stage(Box::new(SensitivityStage { multiplier: 2.0 }));
        pipeline.add_stage(Box::new(ClampStage {
            min: -1.0,
            max: 1.0,
        }));
        pipeline.bypass_stage(0); // bypass sensitivity
        let diag = pipeline.diagnostics(0.4, 0.004);
        // Sensitivity is bypassed: input=output=0.4
        assert!((diag[0].1 - 0.4).abs() < 1e-10);
        assert!((diag[0].2 - 0.4).abs() < 1e-10);
    }
}
