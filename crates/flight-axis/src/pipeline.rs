// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Compiled axis processing pipeline with SoA state layout
//!
//! The pipeline uses a compile-to-function-pointer approach with Structure-of-Arrays
//! state layout for optimal cache performance and zero-allocation execution.

use crate::{AxisFrame, Node, NodeId};
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
}
