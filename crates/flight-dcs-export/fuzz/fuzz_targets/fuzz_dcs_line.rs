// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for the DCS TCP line parser.
//!
//! Exercises `parse_dcs_line` against arbitrary strings to ensure
//! no panics on malformed TCP export lines.
//!
//! Run with: `cargo +nightly fuzz run fuzz_dcs_line`

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };
    // Must never panic — errors are expected and ignored.
    let _ = flight_dcs_export::tcp::parse_dcs_line(input);
});
