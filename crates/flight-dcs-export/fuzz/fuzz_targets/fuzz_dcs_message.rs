// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for DCS Export message deserialization.
//!
//! Run with: `cargo +nightly fuzz run fuzz_dcs_message`

#![no_main]

use libfuzzer_sys::fuzz_target;
use flight_dcs_export::DcsMessage;

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    // Fuzz JSON message parsing — must never panic
    let _ = serde_json::from_str::<DcsMessage>(input.trim());
});
