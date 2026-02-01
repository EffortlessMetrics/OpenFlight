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
// Allow unused code in this WIP crate
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(unused_parens)]
#![allow(unused_assignments)]
#![allow(dead_code)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::unnecessary_map_or)]
#![allow(clippy::manual_abs_diff)]
#![allow(clippy::derivable_impls)]
#![allow(clippy::manual_map)]
#![allow(clippy::let_and_return)]
// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Flight Replay Harness
//!
//! Provides offline replay capabilities for recorded flight data with:
//! - Offline axis/FFB engine feeding system
//! - FP-tolerant output comparison
//! - Acceptance test integration for recorded runs
//! - Comprehensive replay validation suite
//!
//! This crate enables deterministic replay of recorded flight sessions for:
//! - Regression testing
//! - Performance validation
//! - Bug reproduction
//! - Algorithm verification

pub mod acceptance;
pub mod comparison;
pub mod harness;
pub mod metrics;
pub mod offline_engine;
pub mod replay_config;
pub mod synthetic_harness;
pub mod validation;

pub use acceptance::{AcceptanceResult, AcceptanceTest, AcceptanceTestRunner};
pub use comparison::{ComparisonConfig, ComparisonResult, OutputComparator};
pub use harness::{ReplayError, ReplayHarness, ReplayResult};
pub use metrics::{AccuracyMetrics, PerformanceMetrics, ReplayMetrics};
pub use offline_engine::{EngineState, OfflineAxisEngine, OfflineFfbEngine};
pub use replay_config::ToleranceConfig;
pub use replay_config::{ReplayConfig, ReplayMode, TimingMode};
pub use synthetic_harness::{
    HarnessResults, SyntheticHarness, SyntheticHarnessConfig, TelemetryPattern,
};
pub use validation::{ReplayValidator, ValidationError, ValidationResult, ValidationSuite};

pub use flight_axis::AxisFrame;
pub use flight_bus::BusSnapshot;
/// Re-export commonly used types from dependencies
pub use flight_core::blackbox::{BlackboxReader, BlackboxRecord, StreamType};
pub use flight_ffb::{FfbEngine, SafetyState};
