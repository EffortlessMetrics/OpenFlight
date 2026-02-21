// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Reusable testing helpers across OpenFlight crates.

pub mod assertions;
pub mod fixtures;
pub mod integration;
pub mod utils;

pub use assertions::{
    assert_adapter_state_transition, assert_device_connected, assert_snapshot_valid,
};
pub use fixtures::{TestConfig, TestConfigBuilder, TestDeviceBuilder};
pub use integration::TestHarness;
pub use utils::{create_temp_dir, setup_test_logger, wait_for_condition};
