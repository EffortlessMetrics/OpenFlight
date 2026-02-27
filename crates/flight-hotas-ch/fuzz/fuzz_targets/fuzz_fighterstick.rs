// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Fuzz target for CH Products Fighterstick HID report parsing.
//!
//! Run with: `cargo +nightly fuzz run fuzz_fighterstick`

#![no_main]

use libfuzzer_sys::fuzz_target;
use flight_hotas_ch::parse_fighterstick;

fuzz_target!(|data: &[u8]| {
    // Must not panic regardless of input
    let _ = parse_fighterstick(data);
});
