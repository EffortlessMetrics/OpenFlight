// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for JSON deserialization of BusSnapshot.
//!
//! Run with: `cargo +nightly fuzz run fuzz_bus_snapshot`

#![no_main]

use libfuzzer_sys::fuzz_target;
use flight_bus::BusSnapshot;

fuzz_target!(|data: &[u8]| {
    // Fuzz JSON deserialization of BusSnapshot — must never panic
    let _ = serde_json::from_slice::<BusSnapshot>(data);
});
