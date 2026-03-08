// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Comprehensive mock/fake infrastructure for OpenFlight integration testing.
//!
//! Provides deterministic clocks, trace recorders, fake devices with signal
//! generators, fake sim backends, domain-specific assertions, and a golden-file
//! testing framework.

pub mod assertions;
pub mod deterministic_clock;
pub mod fake_device;
pub mod fake_sim;
pub mod golden;
pub mod trace_recorder;

pub use assertions::{
    assert_axis_in_range, assert_jitter_p99, assert_latency_under, assert_monotonic,
    assert_no_nan,
};
pub use deterministic_clock::DeterministicClock;
pub use fake_device::{FakeDeviceBuilder, FaultType, SignalPattern};
pub use fake_sim::FakeSimBackend;
pub use golden::golden_test;
pub use trace_recorder::{EventMatcher, TraceRecorder};
