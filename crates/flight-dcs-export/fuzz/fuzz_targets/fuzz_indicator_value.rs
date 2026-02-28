// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for the DCS indicator value parser.
//!
//! Exercises `parse_indicator_value` against arbitrary strings to ensure
//! no panics on malformed numeric values (including DCS Lua quirks like
//! `inf`, `nan`, and scientific notation).
//!
//! Run with: `cargo +nightly fuzz run fuzz_indicator_value`

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };
    // Must never panic — errors are expected and ignored.
    let _ = flight_dcs_export::protocol::parse_indicator_value(input);
});
