// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for T.16000M FCS HID report parsing.
//!
//! Run with: `cargo +nightly fuzz run fuzz_t16000m`

#![no_main]

use libfuzzer_sys::fuzz_target;
use flight_hotas_thrustmaster::parse_t16000m_report;

fuzz_target!(|data: &[u8]| {
    // Must not panic regardless of input
    let _ = parse_t16000m_report(data);
});
