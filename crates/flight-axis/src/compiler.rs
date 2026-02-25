// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Pipeline compiler with function pointer generation
//!
//! The compiler transforms high-level node configurations into optimized
//! function pointer pipelines with Structure-of-Arrays state layout.
//!
//! # Config Binding Strategy
//!
//! Function pointers cannot capture state, so we use a state-embedded config pattern:
//! - Config parameters are stored at the beginning of each node's state buffer
//! - Runtime state follows the config in the same buffer
//! - The `init_state` method writes both config and initial runtime state
//! - Step functions read config from the state buffer, avoiding runtime allocation
//!
//! This approach maintains zero-allocation guarantees while properly binding config.

use crate::nodes::{
    CurveCompiledState, CurveNode, DeadzoneCompiledState, DeadzoneNode, DetentNode, DetentRole,
    DetentZone, FilterCompiledState, FilterNode, MixerConfig, MixerNode, SlewCompiledState,
    SlewNode,
};
use crate::{AxisFrame, Node, NodeId, Pipeline};
use std::sync::Arc;

// ============================================================================
// Step function generators with config binding
// ============================================================================

/// Generate specialized step function for deadzone nodes
///
/// Config is read from state_ptr at runtime. The state layout is
/// `DeadzoneCompiledState` (threshold, threshold_neg) at offset 0.
fn generate_deadzone_step_fn(_node: Arc<dyn Node>) -> unsafe fn(*mut AxisFrame, *mut u8) {
    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn deadzone_step(frame_ptr: *mut AxisFrame, state_ptr: *mut u8) {
        let frame = &mut *frame_ptr;
        let config = &*(state_ptr as *const DeadzoneCompiledState);

        // Select threshold based on sign of input
        let threshold = if frame.out < 0.0 {
            config.threshold_neg
        } else {
            config.threshold
        };

        if frame.out.abs() < threshold {
            frame.out = 0.0;
        } else {
            let sign = frame.out.signum();
            let abs_val = frame.out.abs();
            // Rescale output to maintain full range after deadzone
            frame.out = sign * ((abs_val - threshold) / (1.0 - threshold));
        }
    }

    deadzone_step
}

/// Generate specialized step function for curve nodes
///
/// Config is read from state_ptr at runtime. The state layout is
/// `CurveCompiledState` (expo, exponent) at offset 0.
fn generate_curve_step_fn(_node: Arc<dyn Node>) -> unsafe fn(*mut AxisFrame, *mut u8) {
    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn curve_step(frame_ptr: *mut AxisFrame, state_ptr: *mut u8) {
        let frame = &mut *frame_ptr;
        let config = &*(state_ptr as *const CurveCompiledState);

        // Skip processing for linear curves (expo == 0)
        if config.expo == 0.0 {
            return;
        }

        let sign = frame.out.signum();
        let abs_val = frame.out.abs();

        // Ensure monotonic curve: f(x) = sign(x) * |x|^(1 + expo)
        // Use precomputed exponent for slightly better performance
        frame.out = sign * abs_val.powf(config.exponent);
    }

    curve_step
}

/// Generate specialized step function for detent nodes
///
/// Detent nodes require access to the zone list which cannot be efficiently
/// embedded in the state buffer. The function pointer path is a no-op stub;
/// the actual processing is handled via the source_nodes path in Pipeline::process().
///
/// This design maintains zero-allocation guarantees while allowing complex
/// zone configurations.
fn generate_detent_step_fn(_node: Arc<dyn Node>) -> unsafe fn(*mut AxisFrame, *mut u8) {
    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn detent_step(_frame_ptr: *mut AxisFrame, _state_ptr: *mut u8) {
        // No-op: Detent processing is handled via source_nodes path
        // The Pipeline::process() method uses node.step_soa() directly
        // when source_nodes is available, which is always the case for
        // pipelines created through PipelineCompiler/PipelineBuilder.
    }

    detent_step
}

/// Generate specialized step function for slew nodes
///
/// Config and state are read from state_ptr. The state layout is
/// `SlewCompiledState` (rate_limit, attack_rate, last_output, last_time_ns) at offset 0.
fn generate_slew_step_fn(_node: Arc<dyn Node>) -> unsafe fn(*mut AxisFrame, *mut u8) {
    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn slew_step(frame_ptr: *mut AxisFrame, state_ptr: *mut u8) {
        let frame = &mut *frame_ptr;
        let state = &mut *(state_ptr as *mut SlewCompiledState);

        // Initialize on first sample
        if state.last_time_ns == 0 {
            state.last_output = frame.out;
            state.last_time_ns = frame.ts_mono_ns;
            return;
        }

        // Calculate time delta, handling timestamp wraparound gracefully
        if frame.ts_mono_ns > state.last_time_ns {
            let dt_s = (frame.ts_mono_ns - state.last_time_ns) as f32 / 1_000_000_000.0;
            let desired_change = frame.out - state.last_output;

            // Select rate based on direction of change (attack vs decay)
            // Attack: moving away from zero, Decay: moving toward zero
            let rate = if desired_change.signum() == frame.out.signum() {
                state.attack_rate
            } else {
                state.rate_limit
            };

            let max_change = rate * dt_s;

            // Apply rate limiting if change exceeds maximum
            if desired_change.abs() > max_change {
                frame.out = state.last_output + desired_change.signum() * max_change;
            }
        }

        // Update state for next iteration
        state.last_output = frame.out;
        state.last_time_ns = frame.ts_mono_ns;
    }

    slew_step
}

/// Generate specialized step function for mixer nodes
///
/// Mixer nodes require access to the input configuration list which cannot be
/// efficiently embedded in the state buffer. The function pointer path is a
/// no-op stub; the actual processing is handled via the source_nodes path
/// in Pipeline::process().
///
/// This design maintains zero-allocation guarantees while allowing dynamic
/// input configurations.
fn generate_mixer_step_fn(_node: Arc<dyn Node>) -> unsafe fn(*mut AxisFrame, *mut u8) {
    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn mixer_step(_frame_ptr: *mut AxisFrame, _state_ptr: *mut u8) {
        // No-op: Mixer processing is handled via source_nodes path
        // The Pipeline::process() method uses node.step_soa() directly
        // when source_nodes is available, which is always the case for
        // pipelines created through PipelineCompiler/PipelineBuilder.
    }

    mixer_step
}

/// Generate specialized step function for filter nodes
///
/// Config and state are read from state_ptr. The state layout is
/// `FilterCompiledState` (alpha, spike_threshold, max_spike_count, prev_output, etc.) at offset 0.
fn generate_filter_step_fn(_node: Arc<dyn Node>) -> unsafe fn(*mut AxisFrame, *mut u8) {
    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn filter_step(frame_ptr: *mut AxisFrame, state_ptr: *mut u8) {
        let frame = &mut *frame_ptr;
        let state = &mut *(state_ptr as *mut FilterCompiledState);
        let input = frame.out;

        // Initialize on first sample
        if !state.initialized {
            state.prev_output = input;
            state.last_raw = input;
            state.initialized = true;
            return;
        }

        // Check for spikes if threshold is configured (non-zero)
        let accept_input = if state.spike_threshold > 0.0 {
            let delta = (input - state.last_raw).abs();

            if delta > state.spike_threshold {
                // Potential spike detected
                state.spike_count = state.spike_count.saturating_add(1);

                if state.spike_count >= state.max_spike_count {
                    // Too many consecutive "spikes" - accept as real change
                    state.spike_count = 0;
                    true
                } else {
                    // Reject spike, keep previous output
                    false
                }
            } else {
                // Normal change, reset spike count
                state.spike_count = 0;
                true
            }
        } else {
            // No spike rejection configured
            true
        };

        if accept_input {
            // Apply EMA: S_t = alpha * Y_t + (1 - alpha) * S_{t-1}
            frame.out = state.alpha * input + (1.0 - state.alpha) * state.prev_output;
            state.prev_output = frame.out;
            state.last_raw = input;
        } else {
            // Spike rejected, output previous filtered value
            frame.out = state.prev_output;
        }
    }

    filter_step
}

/// Pipeline compiler for function pointer generation
pub struct PipelineCompiler {
    nodes: Vec<Arc<dyn Node>>,
}

/// Compilation errors
#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    #[error("Empty pipeline")]
    EmptyPipeline,
    #[error("Node validation failed: {0}")]
    NodeValidation(String),
    #[error("State layout error: {0}")]
    StateLayout(String),
    #[error("Function pointer generation failed")]
    FunctionGeneration,
}

impl PipelineCompiler {
    /// Create new pipeline compiler
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    /// Add node to compilation pipeline
    pub fn add_node<N: Node + 'static>(mut self, node: N) -> Self {
        self.nodes.push(Arc::new(node));
        self
    }

    /// Compile pipeline to optimized function pointers
    pub fn compile(self) -> Result<Pipeline, CompileError> {
        if self.nodes.is_empty() {
            return Err(CompileError::EmptyPipeline);
        }

        let mut pipeline = Pipeline::new();
        let mut node_id = 1u32;

        // Generate function pointers for each node
        for node in &self.nodes {
            let step_fn = self.generate_step_function(node.clone())?;
            let state_size = node.state_size();
            let node_type = node.node_type();

            pipeline.add_compiled_node(step_fn, NodeId(node_id), node_type, state_size);

            node_id += 1;
        }

        // Store nodes for state initialization
        pipeline.set_source_nodes(self.nodes);

        // Validate compiled pipeline
        pipeline
            .validate()
            .map_err(|e| CompileError::StateLayout(e.to_string()))?;

        Ok(pipeline)
    }

    /// Generate optimized step function for node
    fn generate_step_function(
        &self,
        node: Arc<dyn Node>,
    ) -> Result<unsafe fn(*mut AxisFrame, *mut u8), CompileError> {
        // Generate specialized function pointer based on node type
        match node.node_type() {
            "deadzone" => Ok(generate_deadzone_step_fn(node)),
            "curve" => Ok(generate_curve_step_fn(node)),
            "slew" => Ok(generate_slew_step_fn(node)),
            "detent" => Ok(generate_detent_step_fn(node)),
            "mixer" => Ok(generate_mixer_step_fn(node)),
            "filter" => Ok(generate_filter_step_fn(node)),
            _ => Err(CompileError::FunctionGeneration),
        }
    }
}

impl Default for PipelineCompiler {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder pattern for pipeline construction
pub struct PipelineBuilder {
    compiler: PipelineCompiler,
}

impl PipelineBuilder {
    /// Create new pipeline builder
    pub fn new() -> Self {
        Self {
            compiler: PipelineCompiler::new(),
        }
    }

    /// Add deadzone node
    pub fn deadzone(self, threshold: f32) -> Self {
        self.add_node(DeadzoneNode::new(threshold))
    }

    /// Add exponential curve node
    pub fn curve(self, expo: f32) -> Result<Self, &'static str> {
        Ok(self.add_node(CurveNode::exponential(expo)?))
    }

    /// Add slew rate limiter
    pub fn slew(self, rate_limit: f32) -> Self {
        self.add_node(SlewNode::new(rate_limit))
    }

    /// Add detent mapper
    pub fn detent(self, zones: Vec<DetentZone>) -> Self {
        self.add_node(DetentNode::new(zones))
    }

    /// Add single detent zone
    pub fn single_detent(
        self,
        center: f32,
        half_width: f32,
        hysteresis: f32,
        role: DetentRole,
    ) -> Self {
        let zone = DetentZone::new(center, half_width, hysteresis, role);
        self.detent(vec![zone])
    }

    /// Add mixer node
    pub fn mixer(self, config: MixerConfig) -> Result<Self, &'static str> {
        let mixer = MixerNode::new(config)?;
        Ok(self.add_node(mixer))
    }

    /// Add helicopter anti-torque mixer
    pub fn helicopter_anti_torque(self, collective_scale: f32) -> Result<Self, &'static str> {
        let mixer = MixerNode::helicopter_anti_torque(collective_scale)?;
        Ok(self.add_node(mixer))
    }

    /// Add aileron-rudder coordination mixer
    pub fn aileron_rudder_coordination(
        self,
        coordination_factor: f32,
    ) -> Result<Self, &'static str> {
        let mixer = MixerNode::aileron_rudder_coordination(coordination_factor)?;
        Ok(self.add_node(mixer))
    }

    /// Add EMA filter for noise reduction
    ///
    /// # Arguments
    /// * `alpha` - Smoothing factor in [0.0, 1.0]. Lower values = more smoothing.
    ///   0.1 = heavy smoothing, 0.5 = moderate, 1.0 = no filtering.
    pub fn filter(self, alpha: f32) -> Self {
        self.add_node(FilterNode::new(alpha))
    }

    /// Add EMA filter with spike rejection
    ///
    /// # Arguments
    /// * `alpha` - Smoothing factor in [0.0, 1.0]
    /// * `threshold` - Spike rejection threshold in normalized units.
    ///   Changes larger than this are considered spikes.
    pub fn filter_with_spike_rejection(self, alpha: f32, threshold: f32) -> Self {
        self.add_node(FilterNode::with_spike_rejection(alpha, threshold))
    }

    /// Add B104 potentiometer preset filter (for T.Flight HOTAS 4)
    ///
    /// Pre-configured for the B104 linear pot's noise characteristics:
    /// alpha=0.15, spike_threshold=0.4, max_spike_count=5
    pub fn b104_filter(self) -> Self {
        self.add_node(FilterNode::b104_preset())
    }

    /// Add custom node
    pub fn add_node<N: Node + 'static>(mut self, node: N) -> Self {
        self.compiler = self.compiler.add_node(node);
        self
    }

    /// Compile pipeline
    pub fn compile(self) -> Result<Pipeline, CompileError> {
        self.compiler.compile()
    }
}

impl Default for PipelineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Validate node configuration for compilation
pub fn validate_node_config<N: Node>(node: &N) -> Result<(), CompileError> {
    // Check state size is reasonable
    let state_size = node.state_size();
    if state_size > 1024 {
        return Err(CompileError::NodeValidation(format!(
            "Node state size {} exceeds maximum 1024 bytes",
            state_size
        )));
    }

    // Validate node type string
    let node_type = node.node_type();
    if node_type.is_empty() || node_type.len() > 32 {
        return Err(CompileError::NodeValidation(
            "Node type must be 1-32 characters".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nodes::DeadzoneNode;

    #[test]
    fn test_empty_pipeline_compilation() {
        let compiler = PipelineCompiler::new();
        assert!(matches!(
            compiler.compile(),
            Err(CompileError::EmptyPipeline)
        ));
    }

    #[test]
    fn test_single_node_compilation() {
        let pipeline = PipelineBuilder::new()
            .deadzone(0.1)
            .compile()
            .expect("Should compile single node");

        assert_eq!(pipeline.metadata().len(), 1);
        assert_eq!(pipeline.metadata()[0].node_type, "deadzone");
    }

    #[test]
    fn test_multi_node_compilation() {
        let pipeline = PipelineBuilder::new()
            .deadzone(0.05)
            .curve(0.2)
            .expect("Valid curve")
            .slew(1.0)
            .compile()
            .expect("Should compile multi-node pipeline");

        assert_eq!(pipeline.metadata().len(), 3);

        let types: Vec<_> = pipeline.metadata().iter().map(|m| m.node_type).collect();
        assert_eq!(types, vec!["deadzone", "curve", "slew"]);
    }

    #[test]
    fn test_node_validation() {
        let node = DeadzoneNode::new(0.1);
        assert!(validate_node_config(&node).is_ok());
    }

    #[test]
    fn test_pipeline_builder_fluent_api() {
        let result = PipelineBuilder::new()
            .deadzone(0.03)
            .curve(0.15)
            .unwrap()
            .slew(2.0)
            .compile();

        assert!(result.is_ok());
    }

    #[test]
    fn test_mixer_compilation() {
        let config = MixerConfig::new("test").add_scaled_input("input1", 1.0);

        let result = PipelineBuilder::new().mixer(config).unwrap().compile();

        assert!(result.is_ok());
        let pipeline = result.unwrap();
        assert_eq!(pipeline.metadata().len(), 1);
        assert_eq!(pipeline.metadata()[0].node_type, "mixer");
    }

    #[test]
    fn test_helicopter_anti_torque_compilation() {
        let result = PipelineBuilder::new()
            .helicopter_anti_torque(-0.3)
            .unwrap()
            .compile();

        assert!(result.is_ok());
        let pipeline = result.unwrap();
        assert_eq!(pipeline.metadata().len(), 1);
        assert_eq!(pipeline.metadata()[0].node_type, "mixer");
    }

    #[test]
    fn test_aileron_rudder_coordination_compilation() {
        let result = PipelineBuilder::new()
            .aileron_rudder_coordination(0.15)
            .unwrap()
            .compile();

        assert!(result.is_ok());
        let pipeline = result.unwrap();
        assert_eq!(pipeline.metadata().len(), 1);
        assert_eq!(pipeline.metadata()[0].node_type, "mixer");
    }

    #[test]
    fn test_complex_pipeline_with_mixer() {
        let result = PipelineBuilder::new()
            .deadzone(0.05)
            .curve(0.2)
            .unwrap()
            .slew(1.5)
            .helicopter_anti_torque(-0.25)
            .unwrap()
            .compile();

        assert!(result.is_ok());
        let pipeline = result.unwrap();
        assert_eq!(pipeline.metadata().len(), 4);

        let types: Vec<_> = pipeline.metadata().iter().map(|m| m.node_type).collect();
        assert_eq!(types, vec!["deadzone", "curve", "slew", "mixer"]);
    }

    #[test]
    fn test_filter_compilation() {
        let result = PipelineBuilder::new().filter(0.15).compile();

        assert!(result.is_ok());
        let pipeline = result.unwrap();
        assert_eq!(pipeline.metadata().len(), 1);
        assert_eq!(pipeline.metadata()[0].node_type, "filter");
    }

    #[test]
    fn test_filter_with_spike_rejection_compilation() {
        let result = PipelineBuilder::new()
            .filter_with_spike_rejection(0.2, 0.1)
            .compile();

        assert!(result.is_ok());
        let pipeline = result.unwrap();
        assert_eq!(pipeline.metadata().len(), 1);
        assert_eq!(pipeline.metadata()[0].node_type, "filter");
    }

    #[test]
    fn test_b104_filter_compilation() {
        let result = PipelineBuilder::new().b104_filter().compile();

        assert!(result.is_ok());
        let pipeline = result.unwrap();
        assert_eq!(pipeline.metadata().len(), 1);
        assert_eq!(pipeline.metadata()[0].node_type, "filter");
    }

    #[test]
    fn test_pipeline_with_filter_and_other_nodes() {
        let result = PipelineBuilder::new()
            .filter(0.15)
            .deadzone(0.05)
            .curve(0.2)
            .unwrap()
            .slew(1.5)
            .compile();

        assert!(result.is_ok());
        let pipeline = result.unwrap();
        assert_eq!(pipeline.metadata().len(), 4);

        let types: Vec<_> = pipeline.metadata().iter().map(|m| m.node_type).collect();
        assert_eq!(types, vec!["filter", "deadzone", "curve", "slew"]);
    }

    // ========================================================================
    // Config binding tests - verify configured parameters are captured
    // ========================================================================

    #[test]
    fn test_deadzone_config_binding() {
        use crate::AxisFrame;

        // Create pipeline with specific deadzone threshold
        let pipeline = PipelineBuilder::new()
            .deadzone(0.2) // 20% deadzone
            .compile()
            .expect("Should compile");

        let mut state = pipeline.create_state();

        // Input within deadzone should output 0
        let mut frame = AxisFrame::new(0.15, 1000);
        frame.out = frame.in_raw;
        pipeline.process(&mut frame, &mut state);
        assert_eq!(frame.out, 0.0, "Input within 20% deadzone should be zeroed");

        // Input outside deadzone should be rescaled
        let mut frame = AxisFrame::new(0.5, 2000);
        frame.out = frame.in_raw;
        pipeline.process(&mut frame, &mut state);
        // Expected: (0.5 - 0.2) / (1.0 - 0.2) = 0.3 / 0.8 = 0.375
        assert!(
            (frame.out - 0.375).abs() < 0.001,
            "Input 0.5 with 20% deadzone should rescale to ~0.375, got {}",
            frame.out
        );
    }

    #[test]
    fn test_curve_config_binding() {
        use crate::AxisFrame;

        // Create pipeline with expo curve
        let pipeline = PipelineBuilder::new()
            .curve(0.5) // Expo factor of 0.5
            .unwrap()
            .compile()
            .expect("Should compile");

        let mut state = pipeline.create_state();

        // Test curve application: f(x) = sign(x) * |x|^(1 + expo)
        let mut frame = AxisFrame::new(0.5, 1000);
        frame.out = frame.in_raw;
        pipeline.process(&mut frame, &mut state);

        // Expected: 0.5^1.5 = 0.353553...
        let expected = 0.5f32.powf(1.5);
        assert!(
            (frame.out - expected).abs() < 0.001,
            "Curve with expo=0.5 should transform 0.5 to ~{}, got {}",
            expected,
            frame.out
        );
    }

    #[test]
    fn test_slew_config_binding() {
        use crate::AxisFrame;

        // Create pipeline with slew rate limit of 1.0 units/second
        let pipeline = PipelineBuilder::new()
            .slew(1.0)
            .compile()
            .expect("Should compile");

        let mut state = pipeline.create_state();

        // First frame initializes state (use non-zero timestamp to avoid re-init)
        let mut frame = AxisFrame::new(0.0, 1_000_000); // 1ms
        frame.out = frame.in_raw;
        pipeline.process(&mut frame, &mut state);
        assert_eq!(frame.out, 0.0);

        // Large jump should be rate-limited
        // 100ms later, with rate_limit=1.0, max_change = 1.0 * 0.1 = 0.1
        let mut frame = AxisFrame::new(1.0, 101_000_000); // 101ms (100ms delta)
        frame.out = frame.in_raw;
        pipeline.process(&mut frame, &mut state);

        // Should be limited to ~0.1 (max change in 100ms at 1.0 units/sec)
        assert!(
            (frame.out - 0.1).abs() < 0.02,
            "Slew with rate=1.0 should limit 0->1 jump over 100ms to ~0.1, got {}",
            frame.out
        );
    }

    #[test]
    fn test_filter_config_binding() {
        use crate::AxisFrame;

        // Create pipeline with EMA filter (alpha=0.5)
        let pipeline = PipelineBuilder::new()
            .filter(0.5)
            .compile()
            .expect("Should compile");

        let mut state = pipeline.create_state();

        // First frame initializes (output = input)
        let mut frame = AxisFrame::new(1.0, 1000);
        frame.out = frame.in_raw;
        pipeline.process(&mut frame, &mut state);
        assert_eq!(frame.out, 1.0, "First frame should pass through");

        // Second frame applies EMA: S_t = alpha * Y_t + (1-alpha) * S_{t-1}
        let mut frame = AxisFrame::new(0.0, 2000);
        frame.out = frame.in_raw;
        pipeline.process(&mut frame, &mut state);

        // Expected: 0.5 * 0.0 + 0.5 * 1.0 = 0.5
        assert!(
            (frame.out - 0.5).abs() < 0.001,
            "EMA with alpha=0.5 should average to 0.5, got {}",
            frame.out
        );
    }

    #[test]
    fn test_asymmetric_deadzone_config_binding() {
        use crate::AxisFrame;
        use crate::nodes::DeadzoneNode;

        // Create asymmetric deadzone with different positive/negative thresholds
        let pipeline = PipelineCompiler::new()
            .add_node(DeadzoneNode::asymmetric(0.1, 0.3))
            .compile()
            .expect("Should compile");

        let mut state = pipeline.create_state();

        // Positive input with 10% deadzone
        let mut frame = AxisFrame::new(0.05, 1000);
        frame.out = frame.in_raw;
        pipeline.process(&mut frame, &mut state);
        assert_eq!(frame.out, 0.0, "0.05 should be in positive 10% deadzone");

        // Negative input with 30% deadzone
        let mut frame = AxisFrame::new(-0.25, 2000);
        frame.out = frame.in_raw;
        pipeline.process(&mut frame, &mut state);
        assert_eq!(frame.out, 0.0, "-0.25 should be in negative 30% deadzone");

        // Negative input outside 30% deadzone
        let mut frame = AxisFrame::new(-0.5, 3000);
        frame.out = frame.in_raw;
        pipeline.process(&mut frame, &mut state);
        // Expected: -1 * (0.5 - 0.3) / (1.0 - 0.3) = -0.2 / 0.7 = -0.2857...
        let expected = -0.2 / 0.7;
        assert!(
            (frame.out - expected).abs() < 0.001,
            "Input -0.5 with 30% neg deadzone should rescale to ~{}, got {}",
            expected,
            frame.out
        );
    }

    #[test]
    fn test_multi_node_config_binding() {
        use crate::AxisFrame;

        // Create pipeline with multiple nodes, each with specific config
        let pipeline = PipelineBuilder::new()
            .deadzone(0.1) // 10% deadzone
            .curve(0.3) // expo=0.3
            .unwrap()
            .compile()
            .expect("Should compile");

        let mut state = pipeline.create_state();

        // Input outside deadzone
        let mut frame = AxisFrame::new(0.6, 1000);
        frame.out = frame.in_raw;
        pipeline.process(&mut frame, &mut state);

        // After deadzone: (0.6 - 0.1) / 0.9 = 0.5555...
        // After curve: 0.5555^1.3 = ~0.4607
        let after_deadzone: f32 = (0.6 - 0.1) / 0.9;
        let expected = after_deadzone.powf(1.3);
        assert!(
            (frame.out - expected).abs() < 0.01,
            "Multi-node pipeline should apply both configs, expected ~{}, got {}",
            expected,
            frame.out
        );
    }

    // ── snapshot tests ────────────────────────────────────────────────────────

    #[test]
    fn snapshot_pipeline_metadata_full() {
        let pipeline = PipelineBuilder::new()
            .deadzone(0.03)
            .curve(0.2)
            .unwrap()
            .slew(1.2)
            .filter(0.15)
            .compile()
            .expect("Should compile full pipeline");

        // Snapshot the node-type sequence and state layout so regressions in
        // node ordering, naming, or state sizing are immediately visible.
        let meta: Vec<_> = pipeline
            .metadata()
            .iter()
            .map(|m| {
                format!(
                    "id={} type={} state_offset={} state_size={}",
                    m.node_id.0, m.node_type, m.state_offset, m.state_size
                )
            })
            .collect();
        insta::assert_debug_snapshot!("pipeline_metadata_full", meta);
    }

    #[test]
    fn snapshot_pipeline_metadata_deadzone_only() {
        let pipeline = PipelineBuilder::new()
            .deadzone(0.05)
            .compile()
            .expect("Should compile");

        let meta: Vec<_> = pipeline
            .metadata()
            .iter()
            .map(|m| {
                format!(
                    "id={} type={} state_offset={} state_size={}",
                    m.node_id.0, m.node_type, m.state_offset, m.state_size
                )
            })
            .collect();
        insta::assert_debug_snapshot!("pipeline_metadata_deadzone_only", meta);
    }
}
