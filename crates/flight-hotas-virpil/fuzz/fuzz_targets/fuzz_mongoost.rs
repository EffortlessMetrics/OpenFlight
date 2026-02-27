// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for VIRPIL VPC MongoosT-50CM HID report parsing.
//!
//! Run with: `cargo +nightly fuzz run fuzz_mongoost`

#![no_main]

use libfuzzer_sys::fuzz_target;
use flight_hotas_virpil::parse_mongoost_stick_report;

fuzz_target!(|data: &[u8]| {
    // Must not panic regardless of input
    let _ = parse_mongoost_stick_report(data);
});
