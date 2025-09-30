//! Pipeline compiler with function pointer generation
//!
//! The compiler transforms high-level node configurations into optimized
//! function pointer pipelines with Structure-of-Arrays state layout.

use crate::{Node, NodeId, Pipeline, AxisFrame};
use crate::nodes::{DeadzoneNode, CurveNode, SlewNode};
use std::sync::Arc;

/// No-op step function for compilation placeholder
unsafe fn noop_step_function(_frame_ptr: *mut AxisFrame, _state_ptr: *mut u8) {
    // Placeholder implementation
}

/// Pipeline compiler for function pointer generation
pub struct PipelineCompiler {
    nodes: Vec<Arc<dyn Node>>,
    next_node_id: u32,
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
            next_node_id: 1,
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

        // Validate compiled pipeline
        pipeline.validate().map_err(|e| CompileError::StateLayout(e.to_string()))?;

        Ok(pipeline)
    }

    /// Generate optimized step function for node
    fn generate_step_function(
        &self,
        _node: Arc<dyn Node>,
    ) -> Result<unsafe fn(*mut AxisFrame, *mut u8), CompileError> {
        // For now, return a simple no-op function
        // TODO: Implement proper function pointer generation with node-specific optimizations
        Ok(noop_step_function)
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
    use crate::nodes::{DeadzoneNode, CurveNode, SlewNode};

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
}