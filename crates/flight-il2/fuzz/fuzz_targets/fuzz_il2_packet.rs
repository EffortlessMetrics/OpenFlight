// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Fuzz target for IL-2 UDP telemetry packet parsing.
//!
//! Run with: `cargo +nightly fuzz run fuzz_il2_packet`

#![no_main]

use libfuzzer_sys::fuzz_target;
use flight_il2::parse_telemetry_frame;

fuzz_target!(|data: &[u8]| {
    // Must never panic regardless of input
    let _ = parse_telemetry_frame(data);
});
