// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for the generic UDP racing telemetry parser.
//!
//! Exercises `parse_generic_udp` against arbitrary bytes — it must never panic
//! on any input; errors are expected and silently discarded.
//!
//! Run with: `cargo +nightly fuzz run fuzz_generic_udp`

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = flight_sim_racing::parse_generic_udp(data);
});
