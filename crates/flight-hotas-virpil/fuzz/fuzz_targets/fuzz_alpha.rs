// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for VIRPIL VPC Constellation Alpha HID report parsing.
//!
//! Run with: `cargo +nightly fuzz run fuzz_alpha`

#![no_main]

use libfuzzer_sys::fuzz_target;
use flight_hotas_virpil::parse_alpha_report;

fuzz_target!(|data: &[u8]| {
    // Must not panic regardless of input
    let _ = parse_alpha_report(data);
});
