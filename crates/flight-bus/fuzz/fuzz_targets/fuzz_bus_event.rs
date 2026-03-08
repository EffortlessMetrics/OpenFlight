// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for bus telemetry type deserialization.
//!
//! Exercises JSON deserialization of validated bus types (Percentage, GForce,
//! Mach, SimId, GearState, AutopilotState, etc.) to ensure no panics on
//! malformed input.
//!
//! Run with: `cargo +nightly fuzz run fuzz_bus_event`

#![no_main]

use flight_bus::{
    AircraftId, AutopilotState, GForce, GearPosition, GearState, Mach, Percentage, SimId,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz JSON deserialization of each validated bus type — must never panic
    let _ = serde_json::from_slice::<Percentage>(data);
    let _ = serde_json::from_slice::<GForce>(data);
    let _ = serde_json::from_slice::<Mach>(data);
    let _ = serde_json::from_slice::<SimId>(data);
    let _ = serde_json::from_slice::<AircraftId>(data);
    let _ = serde_json::from_slice::<AutopilotState>(data);
    let _ = serde_json::from_slice::<GearState>(data);
    let _ = serde_json::from_slice::<GearPosition>(data);
});
