//! Real-time axis engine with atomic pipeline swaps
//!
//! The AxisEngine provides the main interface for real-time axis processing
//! with atomic configuration updates and strict timing guarantees.

use crate::{AxisFrame, Pipeline, PipelineState, RuntimeCounters, AllocationGuard};
use parking_lot::RwLock;
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use std::time::Instant;

/// Configuration for axis engine
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Enable runtime allocation checking
    pub enable_rt_checks: bool,
    /// Maximum processing time per frame (microseconds)
    pub max_frame_time_us: u32,
    /// Enable performance counters
    pub enable_counters: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            enable_rt_checks: cfg!(feature = "rt-checks"),
            max_frame_time_us: 500, // 0.5ms at 250Hz
            enable_counters: true,
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
    /// Current active pipeline version
    active_version: AtomicU64,
    /// Pending pipeline for atomic swap
    pending_pipeline: RwLock<Option<Arc<CompiledPipeline>>>,
    /// Engine configuration
    config: EngineConfig,
    /// Runtime performance counters
    counters: RuntimeCounters,
    /// Last frame for derivative calculation
    last_frame: RwLock<Option<AxisFrame>>,
}

/// Compiled pipeline with state
struct CompiledPipeline {
    pipeline: Pipeline,
    state: PipelineState,
    compile_time: Instant,
    version: u64,
}

impl AxisEngine {
    /// Create new axis engine with default configuration
    pub fn new() -> Self {
        Self::with_config(EngineConfig::default())
    }

    /// Create new axis engine with custom configuration
    pub fn with_config(config: EngineConfig) -> Self {
        Self {
            active_version: AtomicU64::new(0),
            pending_pipeline: RwLock::new(None),
            config,
            counters: RuntimeCounters::new(),
            last_frame: RwLock::new(None),
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

        // Process through active pipeline (simplified for now)
        let result = Ok(());

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

    /// Reset runtime counters
    pub fn reset_counters(&self) {
        self.counters.reset();
    }

    /// Check if pipeline is active
    pub fn has_active_pipeline(&self) -> bool {
        self.active_version.load(Ordering::Relaxed) > 0
    }

    /// Get active pipeline version
    pub fn active_version(&self) -> Option<u64> {
        let version = self.active_version.load(Ordering::Relaxed);
        if version > 0 { Some(version) } else { None }
    }

    /// Try to swap pending pipeline atomically (RT-safe)
    #[inline(always)]
    fn try_swap_pipeline(&self) {
        if let Some(mut pending) = self.pending_pipeline.try_write() {
            if let Some(new_pipeline) = pending.take() {
                // Update active version atomically
                self.active_version.store(new_pipeline.version, Ordering::Relaxed);
                
                // Update counters
                self.counters.increment_pipeline_swaps();
            }
        }
    }

    /// Compile and validate new pipeline (non-RT)
    fn compile_and_validate(&self, pipeline: Pipeline) -> Result<Arc<CompiledPipeline>, CompileError> {
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
            state,
            compile_time: Instant::now(),
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
    use crate::nodes::{DeadzoneNode, CurveNode};

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
}