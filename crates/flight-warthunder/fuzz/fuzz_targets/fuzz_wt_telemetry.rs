// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for War Thunder telemetry JSON deserialization.
//!
//! Run with: `cargo +nightly fuzz run fuzz_wt_telemetry`

#![no_main]

use libfuzzer_sys::fuzz_target;
use flight_warthunder::protocol::WtIndicators;

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    // Fuzz WtIndicators JSON deserialization — must never panic
    let _ = serde_json::from_str::<WtIndicators>(input);
});
