// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Deterministic test infrastructure for OpenFlight.
//!
//! Provides fake backends, a manually-advanceable clock, fluent fixture
//! builders, and golden-file testing utilities so the full axis pipeline
//! can be exercised without real hardware.

pub mod builders;
pub mod fake_clock;
pub mod fake_hid_device;
pub mod fake_sim_adapter;
pub mod golden;

pub use builders::{AxisPipelineBuilder, DeviceBuilder, ProfileBuilder, SnapshotBuilder};
pub use fake_clock::FakeClock;
pub use fake_hid_device::{FakeHidDevice, HidReport};
pub use fake_sim_adapter::{ConnectionBehavior, FakeSimAdapter, RecordedOutput};
pub use golden::{assert_golden, golden_dir};
