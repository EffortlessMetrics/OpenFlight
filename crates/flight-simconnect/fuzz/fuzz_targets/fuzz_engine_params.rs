// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for SimConnect engine parameters parser.
//!
//! Exercises `parse_engine_params` against arbitrary byte sequences and
//! engine counts to ensure no panics on malformed SimConnect data buffers.
//!
//! Run with: `cargo +nightly fuzz run fuzz_engine_params`

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }
    // Use the first byte to select engine count (1–4), fuzz the rest as data
    let engine_count = ((data[0] % 4) + 1) as usize;
    let payload = &data[1..];
    // Must never panic on any byte sequence — short buffers are expected.
    let _ = flight_simconnect::parse_engine_params(payload, engine_count);
});
