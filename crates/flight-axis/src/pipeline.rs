//! Axis processing pipeline

use crate::{AxisFrame, Node};

/// Compiled axis processing pipeline
pub struct Pipeline {
    nodes: Vec<Box<dyn Node + Send>>,
}

impl Pipeline {
    /// Create new empty pipeline
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    /// Add node to pipeline
    pub fn add_node<N: Node + Send + 'static>(mut self, node: N) -> Self {
        self.nodes.push(Box::new(node));
        self
    }

    /// Process frame through pipeline
    #[inline(always)]
    pub fn process(&mut self, frame: &mut AxisFrame) {
        for node in &mut self.nodes {
            node.step(frame);
        }
    }
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::new()
    }
}
