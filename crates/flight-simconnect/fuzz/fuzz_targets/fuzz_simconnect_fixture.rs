// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for SimConnect session fixture JSON deserialization.
//!
//! Exercises `SessionFixture` serde deserialization against arbitrary byte
//! sequences to ensure no panics or undefined behaviour.
//!
//! Run with: `cargo +nightly fuzz run fuzz_simconnect_fixture`

#![no_main]

use libfuzzer_sys::fuzz_target;
use flight_simconnect::fixtures::SessionFixture;

fuzz_target!(|data: &[u8]| {
    // JSON deserialization of complex nested structures must never panic
    let _ = serde_json::from_slice::<SessionFixture>(data);
});
