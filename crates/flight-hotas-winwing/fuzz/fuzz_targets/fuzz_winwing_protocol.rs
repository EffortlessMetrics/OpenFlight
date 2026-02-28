// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for WinWing feature report and detent protocol parsers.
//!
//! Exercises `parse_feature_report` and `parse_detent_response` against
//! arbitrary byte slices to ensure no panics on malformed USB feature
//! report frames or detent configuration responses.
//!
//! Run with: `cargo +nightly fuzz run fuzz_winwing_protocol`

#![no_main]

use flight_hotas_winwing::{parse_detent_response, parse_feature_report};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Feature report frame parser — must never panic
    let _ = parse_feature_report(data);

    // Detent response parser — must never panic
    let _ = parse_detent_response(data);
});
