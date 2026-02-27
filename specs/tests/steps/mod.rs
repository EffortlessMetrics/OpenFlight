// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2025 Flight Hub Team

//! Step definitions for BDD scenarios

pub mod axis_processing;
pub mod coverage;
pub mod documentation;
pub mod new_adapters;
pub mod tflight_hotas4;

use crate::FlightWorld;
use cucumber::given;

// Re-export step definitions
pub use axis_processing::*;
pub use coverage::*;
pub use documentation::*;
pub use new_adapters::*;
pub use tflight_hotas4::*;

#[given("the blackbox recording system is available")]
async fn given_blackbox_recording_system_available(_world: &mut FlightWorld) {}

#[given("the HID device info type system is available")]
async fn given_hid_device_info_type_system_available(_world: &mut FlightWorld) {}
