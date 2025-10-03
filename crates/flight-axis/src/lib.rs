//! Flight Axis Processing Engine
//!
//! Real-time 250Hz axis processing pipeline with zero-allocation guarantee.
//! 
//! This crate implements the core axis processing engine for Flight Hub with:
//! - Zero-allocation real-time processing
//! - Atomic pipeline swaps at tick boundaries
//! - Compile-to-function-pointer optimization
//! - Runtime allocation/lock monitoring
//! - Deterministic execution guarantees
//!
//! # Architecture
//!
//! The axis engine processes input frames through a configurable pipeline of nodes:
//!
//! ```text
//! Raw Input → Deadzone → Curve → Slew Limiter → Detents → Mixer → Output
//! ```
//!
//! Each node implements the [`Node`] trait and processes frames in-place with zero allocations.
//!
//! # Examples
//!
//! ## Basic Pipeline Creation
//!
//! ```rust
//! use flight_axis::{AxisEngine, AxisFrame};
//!
//! // Create an axis engine
//! let mut engine = AxisEngine::new_for_axis("pitch".to_string());
//!
//! // Process a frame
//! let mut frame = AxisFrame::new(0.5, 1000);
//! engine.process(&mut frame).expect("Processing should succeed");
//!
//! println!("Input: {:.2} → Output: {:.2}", frame.in_raw, frame.out);
//! ```
//!
//! ## Pipeline Compilation
//!
//! ```rust
//! use flight_axis::{PipelineCompiler, PipelineBuilder};
//! use flight_axis::nodes::{DeadzoneNode, CurveNode};
//!
//! let mut builder = PipelineBuilder::new();
//! builder.add_deadzone(0.03);
//! builder.add_curve(vec![(0.0, 0.0), (0.5, 0.3), (1.0, 1.0)]);
//!
//! let compiler = PipelineCompiler::new();
//! let pipeline = compiler.compile(builder).expect("Compilation should succeed");
//! ```
//!
//! # Performance Guarantees
//!
//! - **Zero Allocations**: Hot path never allocates memory
//! - **Zero Locks**: No mutex/rwlock usage in processing
//! - **Deterministic**: Same inputs always produce same outputs
//! - **Real-time**: Processing completes within 0.5ms p99

pub mod frame;
pub mod nodes;
pub mod pipeline;
pub mod engine;
pub mod compiler;
pub mod counters;
pub mod conflict;
pub mod blackbox;

pub use frame::AxisFrame;
pub use nodes::{Node, NodeId, DeadzoneNode, CurveNode, SlewNode, SlewState, DetentNode, DetentState, DetentRole, DetentZone, DetentEvent, MixerNode, MixerInput, MixerConfig, MixerState};
pub use pipeline::{Pipeline, PipelineState};
pub use engine::{AxisEngine, UpdateResult, EngineConfig, ProcessError, CompileError as EngineCompileError};
pub use compiler::{PipelineCompiler, PipelineBuilder, CompileError};
pub use counters::{RuntimeCounters, AllocationGuard};
pub use conflict::{CurveConflictDetector, ConflictDetectorConfig, CurveConflict, ConflictType, ConflictSeverity, ConflictMetadata, ConflictResolution, ResolutionType};
pub use blackbox::{BlackboxAnnotator, BlackboxEvent, ConflictData, ResolutionDetails};
