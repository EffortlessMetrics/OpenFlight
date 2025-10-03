// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Replay configuration and settings

use std::time::Duration;
use serde::{Deserialize, Serialize};

/// Configuration for replay operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayConfig {
    /// Replay mode (real-time vs fast-forward)
    pub mode: ReplayMode,
    /// Timing mode for replay
    pub timing_mode: TimingMode,
    /// Maximum replay duration (safety limit)
    pub max_duration: Duration,
    /// Whether to validate outputs during replay
    pub validate_outputs: bool,
    /// Tolerance configuration for comparisons
    pub tolerance: ToleranceConfig,
    /// Whether to collect detailed metrics
    pub collect_metrics: bool,
    /// Number of warm-up frames to skip
    pub warmup_frames: usize,
}

/// Replay execution mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReplayMode {
    /// Real-time replay matching original timing
    RealTime,
    /// Fast-forward replay (as fast as possible)
    FastForward,
    /// Step-by-step replay for debugging
    StepByStep,
}

/// Timing mode for replay synchronization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimingMode {
    /// Use original timestamps from recording
    OriginalTimestamps,
    /// Use synthetic timestamps at fixed intervals
    SyntheticTiming { interval_ns: u64 },
    /// Use wall clock time for real-time replay
    WallClock,
}

/// Tolerance configuration for output comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToleranceConfig {
    /// Axis output tolerance (normalized units)
    pub axis_epsilon: f32,
    /// FFB torque tolerance (Newton-meters)
    pub ffb_epsilon: f32,
    /// Timing drift tolerance (nanoseconds per second)
    pub timing_drift_ns_per_s: u64,
    /// Maximum allowed timing jitter (nanoseconds)
    pub max_timing_jitter_ns: u64,
}

impl Default for ReplayConfig {
    fn default() -> Self {
        Self {
            mode: ReplayMode::FastForward,
            timing_mode: TimingMode::OriginalTimestamps,
            max_duration: Duration::from_secs(3600), // 1 hour safety limit
            validate_outputs: true,
            tolerance: ToleranceConfig::default(),
            collect_metrics: true,
            warmup_frames: 0,
        }
    }
}

impl Default for ToleranceConfig {
    fn default() -> Self {
        Self {
            axis_epsilon: 1e-6,      // 1 micro-unit for axis outputs
            ffb_epsilon: 1e-4,       // 0.1 mNm for FFB torque
            timing_drift_ns_per_s: 100_000, // 0.1ms drift per second
            max_timing_jitter_ns: 500_000,   // 0.5ms max jitter
        }
    }
}

impl ToleranceConfig {
    /// Create strict tolerance configuration for regression testing
    pub fn strict() -> Self {
        Self {
            axis_epsilon: 1e-8,
            ffb_epsilon: 1e-6,
            timing_drift_ns_per_s: 10_000,  // 10μs drift per second
            max_timing_jitter_ns: 50_000,   // 50μs max jitter
        }
    }

    /// Create relaxed tolerance configuration for acceptance testing
    pub fn relaxed() -> Self {
        Self {
            axis_epsilon: 1e-4,
            ffb_epsilon: 1e-2,
            timing_drift_ns_per_s: 1_000_000, // 1ms drift per second
            max_timing_jitter_ns: 2_000_000,  // 2ms max jitter
        }
    }
}