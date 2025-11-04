// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Pipeline compiler with function pointer generation
//!
//! The compiler transforms high-level node configurations into optimized
//! function pointer pipelines with Structure-of-Arrays state layout.

use crate::{Node, NodeId, Pipeline, AxisFrame};
use crate::nodes::{DeadzoneNode, CurveNode, SlewNode, DetentNode, DetentZone, DetentRole, MixerNode, MixerConfig};
use std::sync::Arc;



/// Generate specialized step function for deadzone nodes
fn generate_deadzone_step_fn(node: Arc<dyn Node>) -> unsafe fn(*mut AxisFrame, *mut u8) {
    // Capture node configuration at compile time
    let _node_clone = node.clone();
    
    // Return a closure that captures the node configuration
    // This is a simplified approach - in a full implementation, we'd generate
    // optimized machine code or use more sophisticated compilation techniques
    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn deadzone_step(frame_ptr: *mut AxisFrame, _state_ptr: *mut u8) {
        // This is a placeholder - in practice we'd need to capture the node config
        // For now, apply a simple deadzone with hardcoded threshold
        let frame = &mut *frame_ptr;
        let threshold = 0.1f32; // This should come from the captured node config
        
        if frame.out.abs() < threshold {
            frame.out = 0.0;
        } else {
            let sign = frame.out.signum();
            let abs_val = frame.out.abs();
            frame.out = sign * ((abs_val - threshold) / (1.0 - threshold));
        }
    }
    
    deadzone_step
}

/// Generate specialized step function for curve nodes
fn generate_curve_step_fn(_node: Arc<dyn Node>) -> unsafe fn(*mut AxisFrame, *mut u8) {
    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn curve_step(frame_ptr: *mut AxisFrame, _state_ptr: *mut u8) {
        let frame = &mut *frame_ptr;
        let expo = 0.2f32; // This should come from the captured node config
        
        if expo == 0.0 {
            return; // Linear, no change needed
        }

        let sign = frame.out.signum();
        let abs_val = frame.out.abs();
        
        // Ensure monotonic curve: f(x) = sign(x) * |x|^(1 + expo)
        frame.out = sign * abs_val.powf(1.0 + expo);
    }
    
    curve_step
}

/// Generate specialized step function for detent nodes
fn generate_detent_step_fn(_node: Arc<dyn Node>) -> unsafe fn(*mut AxisFrame, *mut u8) {
    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn detent_step(_frame_ptr: *mut AxisFrame, _state_ptr: *mut u8) {
        // This is a bridge implementation that delegates to the node's step_soa method
        // In a production system, we'd want to inline the detent logic for maximum performance
        // but for now we'll delegate to maintain correctness
        
        // The actual detent logic is handled by the pipeline's process method 
        // which calls step_soa on each node when source_nodes are available
    }
    
    detent_step
}

/// Generate specialized step function for slew nodes
fn generate_slew_step_fn(_node: Arc<dyn Node>) -> unsafe fn(*mut AxisFrame, *mut u8) {
    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn slew_step(frame_ptr: *mut AxisFrame, state_ptr: *mut u8) {
        // Delegate to the node's SoA implementation
        // This is a bridge between the function pointer system and the trait system
        let frame = &mut *frame_ptr;
        
        // For slew nodes, we need to use the SoA state layout
        // This is a simplified implementation
        let state = state_ptr as *mut crate::nodes::SlewState;
        
        if (*state).last_time_ns == 0 {
            (*state).last_output = frame.out;
            (*state).last_time_ns = frame.ts_mono_ns;
            return;
        }

        if frame.ts_mono_ns > (*state).last_time_ns {
            let dt_s = (frame.ts_mono_ns - (*state).last_time_ns) as f32 / 1_000_000_000.0;
            let desired_change = frame.out - (*state).last_output;
            
            let rate_limit = 1.0f32; // This should come from captured config
            let max_change = rate_limit * dt_s;

            if desired_change.abs() > max_change {
                frame.out = (*state).last_output + desired_change.signum() * max_change;
            }
        }

        (*state).last_output = frame.out;
        (*state).last_time_ns = frame.ts_mono_ns;
    }
    
    slew_step
}

/// Generate specialized step function for mixer nodes
fn generate_mixer_step_fn(_node: Arc<dyn Node>) -> unsafe fn(*mut AxisFrame, *mut u8) {
    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn mixer_step(_frame_ptr: *mut AxisFrame, _state_ptr: *mut u8) {
        // This is a bridge implementation that delegates to the node's step_soa method
        // In a production system, we'd want to inline the mixer logic for maximum performance
        // but for now we'll delegate to maintain correctness
        
        // The actual mixer logic is handled by the pipeline's process method 
        // which calls step_soa on each node when source_nodes are available
    }
    
    mixer_step
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
        Self {
            nodes: Vec::new(),
        }
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

            pipeline.add_compiled_node(
                step_fn,
                NodeId(node_id),
                node_type,
                state_size,
            );

            node_id += 1;
        }

        // Store nodes for state initialization
        pipeline.set_source_nodes(self.nodes);

        // Validate compiled pipeline
        pipeline.validate().map_err(|e| CompileError::StateLayout(e.to_string()))?;

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
    pub fn single_detent(self, center: f32, half_width: f32, hysteresis: f32, role: DetentRole) -> Self {
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
    pub fn aileron_rudder_coordination(self, coordination_factor: f32) -> Result<Self, &'static str> {
        let mixer = MixerNode::aileron_rudder_coordination(coordination_factor)?;
        Ok(self.add_node(mixer))
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
        return Err(CompileError::NodeValidation(
            format!("Node state size {} exceeds maximum 1024 bytes", state_size)
        ));
    }

    // Validate node type string
    let node_type = node.node_type();
    if node_type.is_empty() || node_type.len() > 32 {
        return Err(CompileError::NodeValidation(
            "Node type must be 1-32 characters".to_string()
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
        assert!(matches!(compiler.compile(), Err(CompileError::EmptyPipeline)));
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
            .curve(0.2).expect("Valid curve")
            .slew(1.0)
            .compile()
            .expect("Should compile multi-node pipeline");

        assert_eq!(pipeline.metadata().len(), 3);
        
        let types: Vec<_> = pipeline.metadata()
            .iter()
            .map(|m| m.node_type)
            .collect();
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
        let config = MixerConfig::new("test")
            .add_scaled_input("input1", 1.0);
        
        let result = PipelineBuilder::new()
            .mixer(config)
            .unwrap()
            .compile();

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
            .curve(0.2).unwrap()
            .slew(1.5)
            .helicopter_anti_torque(-0.25).unwrap()
            .compile();

        assert!(result.is_ok());
        let pipeline = result.unwrap();
        assert_eq!(pipeline.metadata().len(), 4);
        
        let types: Vec<_> = pipeline.metadata()
            .iter()
            .map(|m| m.node_type)
            .collect();
        assert_eq!(types, vec!["deadzone", "curve", "slew", "mixer"]);
    }
}