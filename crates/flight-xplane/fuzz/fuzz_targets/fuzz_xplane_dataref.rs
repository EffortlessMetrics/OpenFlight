// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for X-Plane DataRef value deserialization.
//!
//! Run with: `cargo +nightly fuzz run fuzz_xplane_dataref`

#![no_main]

use libfuzzer_sys::fuzz_target;
use flight_xplane::DataRefValue;

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    // Fuzz JSON deserialization of DataRef values — must never panic
    let _ = serde_json::from_str::<DataRefValue>(input);
});
