#![cfg_attr(test, allow(unused_imports, unused_variables, unused_mut, unused_assignments, unused_parens, dead_code))]

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

pub mod harness;
pub mod comparison;
pub mod validation;
pub mod acceptance;
pub mod offline_engine;
pub mod replay_config;
pub mod metrics;

pub use harness::{ReplayHarness, ReplayResult, ReplayError};
pub use comparison::{OutputComparator, ComparisonResult, ComparisonConfig};
pub use replay_config::ToleranceConfig;
pub use validation::{ReplayValidator, ValidationSuite, ValidationResult, ValidationError};
pub use acceptance::{AcceptanceTestRunner, AcceptanceTest, AcceptanceResult};
pub use offline_engine::{OfflineAxisEngine, OfflineFfbEngine, EngineState};
pub use replay_config::{ReplayConfig, ReplayMode, TimingMode};
pub use metrics::{ReplayMetrics, PerformanceMetrics, AccuracyMetrics};

/// Re-export commonly used types from dependencies
pub use flight_core::blackbox::{BlackboxReader, BlackboxRecord, StreamType};
pub use flight_axis::AxisFrame;
pub use flight_ffb::{FfbEngine, SafetyState};
pub use flight_bus::BusSnapshot;