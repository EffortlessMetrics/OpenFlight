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

pub mod frame;
pub mod nodes;
pub mod pipeline;
pub mod engine;
pub mod compiler;
pub mod counters;

pub use frame::AxisFrame;
pub use nodes::{Node, NodeId};
pub use pipeline::{Pipeline, PipelineState};
pub use engine::{AxisEngine, UpdateResult, EngineConfig};
pub use compiler::{PipelineCompiler, CompileError};
pub use counters::{RuntimeCounters, AllocationGuard};
