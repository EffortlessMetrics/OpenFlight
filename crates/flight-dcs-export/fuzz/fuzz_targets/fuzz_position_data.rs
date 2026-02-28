// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for the DCS position data parser.
//!
//! Exercises `parse_position_data` against arbitrary strings to ensure
//! no panics on malformed coordinate triples (x,y,z).
//!
//! Run with: `cargo +nightly fuzz run fuzz_position_data`

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };
    // Must never panic — errors are expected and ignored.
    let _ = flight_dcs_export::protocol::parse_position_data(input);
});
