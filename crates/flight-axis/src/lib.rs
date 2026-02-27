#![cfg_attr(
    test,
    allow(
        unused_imports,
        unused_variables,
        unused_mut,
        unused_assignments,
        unused_parens,
        dead_code
    )
)]
// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

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
//! let pipeline = PipelineBuilder::new()
//!     .deadzone(0.03)
//!     .curve(0.3).unwrap()
//!     .compile()
//!     .expect("Compilation should succeed");
//! ```
//!
//! # Performance Guarantees
//!
//! - **Zero Allocations**: Hot path never allocates memory
//! - **Zero Locks**: No mutex/rwlock usage in processing
//! - **Deterministic**: Same inputs always produce same outputs
//! - **Real-time**: Processing completes within 0.5ms p99

#[cfg(test)]
mod proptest_tests;

pub mod blackbox;
pub mod combine;
pub mod compiler;
pub mod conflict;
pub mod counters;
pub mod engine;
pub mod frame;
pub mod nodes;
pub mod pipeline;
pub mod rate_limit;
pub mod smoothing;
pub mod trim;

pub use blackbox::{BlackboxAnnotator, BlackboxEvent, ConflictData, ResolutionDetails};
pub use combine::{combine_average, combine_differential, split_bipolar};
pub use compiler::{CompileError, PipelineBuilder, PipelineCompiler};
pub use conflict::{
    ConflictDetectorConfig, ConflictMetadata, ConflictResolution, ConflictSeverity, ConflictType,
    CurveConflict, CurveConflictDetector, ResolutionType,
};
pub use counters::{AllocationGuard, RuntimeCounters};
pub use engine::{
    AxisEngine, CompileError as EngineCompileError, EngineConfig, ProcessError, UpdateResult,
};
pub use frame::{AxisFrame, FrameError};
pub use nodes::{
    CurveCompiledState, CurveNode, DeadzoneCompiledState, DeadzoneNode, DetentEvent, DetentNode,
    DetentRole, DetentState, DetentZone, FilterCompiledState, FilterNode, FilterState, MixerConfig,
    MixerInput, MixerNode, MixerState, Node, NodeId, SlewCompiledState, SlewNode, SlewState,
};
pub use pipeline::{Pipeline, PipelineState};
pub use rate_limit::{AxisRateLimiter, AxisRateLimiterBank};
pub use smoothing::{EmaFilter, EmaFilterBank};
pub use trim::{AxisTrim, AxisTrimBank};
