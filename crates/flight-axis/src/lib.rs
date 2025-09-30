//! Flight Axis Processing Engine
//!
//! Real-time 250Hz axis processing pipeline with zero-allocation guarantee.

pub mod frame;
pub mod nodes;
pub mod pipeline;

pub use frame::AxisFrame;
pub use nodes::Node;
pub use pipeline::Pipeline;
