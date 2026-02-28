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

pub mod accumulator;
pub mod blackbox;
pub mod buttons;
pub mod bypass;
pub mod calibration;
pub mod calibration_wizard;
pub mod chain;
pub mod combine;
pub mod compiler;
pub mod conditional_scale;
pub mod conflict;
pub mod counters;
pub mod curve;
pub mod deadzone;
pub mod debounce;
pub mod detent;
pub mod emergency_stop;
pub mod engine;
pub mod frame;
pub mod freeze_detect;
pub mod hat;
pub mod histogram;
pub mod history;
pub mod input_validator;
pub mod invert;
pub mod lag_compensator;
pub mod mixer;
pub mod mode_switch;
pub mod nodes;
pub mod noise_floor;
pub mod normalize;
pub mod peak_hold;
pub mod pid;
pub mod pipeline;
pub mod pipeline_bypass;
pub mod quantize;
pub mod rate_limit;
pub mod recording;
pub mod scale;
pub mod smoothing;
pub mod stages;
pub mod throttle_zone;
pub mod trace_replay;
pub mod trim;
pub mod velocity;

pub use accumulator::{AccumulatorConfig, AxisAccumulator};
pub use blackbox::{BlackboxAnnotator, BlackboxEvent, ConflictData, ResolutionDetails};
pub use buttons::{
    ButtonChord, ButtonError, ButtonMacro, ButtonProcessor, MacroAction, MacroTrigger,
};
pub use bypass::{BypassBank, BypassConfig, BypassGate};
pub use calibration::{AxisCalibration, CalibrationBank};
pub use calibration_wizard::{
    CalibrationError, CalibrationResult, CalibrationSample, CalibrationStep, CalibrationWizard,
};
pub use chain::{AxisChain, AxisChainConfig, ChainStageValues};
pub use combine::{combine_average, combine_differential, split_bipolar};
pub use compiler::{CompileError, PipelineBuilder, PipelineCompiler};
pub use conditional_scale::{ConditionalScale, MAX_CONDITIONS, ScaleCondition};
pub use conflict::{
    ConflictDetectorConfig, ConflictMetadata, ConflictResolution, ConflictSeverity, ConflictType,
    CurveConflict, CurveConflictDetector, ResolutionType,
};
pub use counters::{AllocationGuard, RuntimeCounters};
pub use curve::{ControlPoint, CurveError, ExpoCurveConfig, InterpolationMode, ResponseCurve};
pub use deadzone::{
    AsymmetricDeadzoneConfig, DeadzoneBank, DeadzoneConfig, DeadzoneError, DeadzoneProcessor,
};
pub use debounce::AxisDebounce;
pub use detent::{Detent, DetentBand, DetentConfig, DetentProcessor, RtDetentProcessor};
pub use emergency_stop::EmergencyStop;
pub use engine::{
    AxisEngine, CompileError as EngineCompileError, EngineConfig, ProcessError, UpdateResult,
};
pub use frame::{AxisFrame, FrameError};
pub use freeze_detect::FreezeDetector;
pub use hat::{HatBank, HatDecoder, HatError, HatOutput, HatResolution};
pub use histogram::{AxisHistogram, HISTOGRAM_BUCKETS};
pub use history::{
    AxisHistory, AxisHistory64, AxisHistory256, AxisHistory1024, HistorySample, HistoryStats,
};
pub use input_validator::InputValidator;
pub use invert::{AxisInvert, InvertBank};
pub use lag_compensator::{LagCompensator, LagCompensatorConfig};
pub use mixer::{AxisMixer, MAX_MIXER_INPUTS, MixMode};
pub use mode_switch::{AxisMode, ModeSwitcher};
pub use nodes::{
    CurveCompiledState, CurveNode, DeadzoneCompiledState, DeadzoneNode, DetentEvent, DetentNode,
    DetentRole, DetentState, DetentZone, FilterCompiledState, FilterNode, FilterState, MixerConfig,
    MixerInput, MixerNode, MixerState, Node, NodeId, SlewCompiledState, SlewNode, SlewState,
};
pub use noise_floor::{NoiseFloorConfig, NoiseFloorDetector, NoiseFloorDetector64};
pub use normalize::{AxisNormalizer, NormalizeConfig, NormalizerBank};
pub use peak_hold::PeakHold;
pub use pid::{PidBank, PidConfig, PidController};
pub use pipeline::{
    AxisPipeline, AxisStage, ClampStage, CurveStage, DeadzoneStage, Pipeline, PipelineState,
    SensitivityStage, SmoothingStage,
};
pub use pipeline_bypass::{PipelineStage, StageBypass};
pub use quantize::{AxisQuantize, QuantizeConfig};
pub use rate_limit::{AxisRateLimiter, AxisRateLimiterBank};
pub use recording::{AxisPlayback, AxisRecording, AxisSample};
pub use scale::{AxisScale, ScaleBank, ScaleError};
pub use smoothing::{EmaFilter, EmaFilterBank};
pub use stages::{
    ClampStage as RtClampStage, CurveType, DeadzoneShape, DeadzoneStage as RtDeadzoneStage,
    InvertStage as RtInvertStage, MAX_CURVE_POINTS, MAX_SMA_WINDOW, MAX_STAGES, NoiseGate,
    PipelineDiagnostics, RescaleStage, RtAxisPipeline, RtPipelineBuilder, SlewRateLimiter,
    SmoothingStage as RtSmoothingStage, SmoothingType, Stage, StageDiagnostic, StageSlot,
};
pub use throttle_zone::{
    ThrottleZoneConfig, ThrottleZoneProcessor, ZoneError, ZoneEvent, ZoneName,
};
pub use trace_replay::{
    AxisTrace, TraceRecorder, TraceReplayer, TraceSample, assert_trace_matches,
};
pub use trim::{AxisTrim, AxisTrimBank};
