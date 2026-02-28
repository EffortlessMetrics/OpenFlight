// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for SimConnect weather SimVar parser.
//!
//! Exercises `parse_weather_simvars` against arbitrary byte sequences to
//! ensure no panics on malformed or truncated SimConnect data buffers.
//!
//! Run with: `cargo +nightly fuzz run fuzz_weather_simvars`

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Must never panic on any byte sequence — short buffers are expected.
    let _ = flight_simconnect::parse_weather_simvars(data);
});
