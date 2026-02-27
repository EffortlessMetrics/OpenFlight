// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Fuzz target for AeroFly FS UDP telemetry packet parsing.
//!
//! Run with: `cargo +nightly fuzz run fuzz_aerofly_packet`

#![no_main]

use libfuzzer_sys::fuzz_target;
use flight_aerofly::{parse_telemetry, parse_json_telemetry};

fuzz_target!(|data: &[u8]| {
    // Fuzz binary parser — must never panic
    let _ = parse_telemetry(data);

    // Fuzz JSON parser with the same bytes interpreted as UTF-8
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = parse_json_telemetry(s);
    }
});
