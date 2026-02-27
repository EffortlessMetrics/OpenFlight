// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Reusable testing helpers across OpenFlight crates.

pub mod assert_helpers;
pub mod assertions;
pub mod deterministic_clock;
pub mod fake_device;
pub mod fake_sim;
pub mod fixture_builder;
pub mod fixtures;
pub mod integration;
pub mod snapshot;
pub mod trace_replay;
pub mod utils;

pub use assertions::{
    assert_adapter_state_transition, assert_approx_eq, assert_bounded_rate,
    assert_device_connected, assert_in_range, assert_monotonic, assert_snapshot_valid,
};
pub use deterministic_clock::DeterministicClock;
pub use fake_device::{FakeDevice, FakeInput};
pub use fake_sim::{FakeSim, FakeSnapshot};
pub use fixture_builder::{
    DeviceFixture, DeviceFixtureBuilder, ProfileFixture, ProfileFixtureBuilder, TelemetryFixture,
    TelemetryFixtureBuilder,
};
pub use fixtures::{TestConfig, TestConfigBuilder, TestDeviceBuilder};
pub use integration::TestHarness;
pub use snapshot::{SnapshotResult, SnapshotStore};
pub use trace_replay::{TraceEvent, TraceEventType, TracePlayer, TraceRecording};
pub use utils::{create_temp_dir, setup_test_logger, wait_for_condition};
