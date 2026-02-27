// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Fuzz target for CH Products Pro Pedals HID report parsing.
//!
//! Run with: `cargo +nightly fuzz run fuzz_pro_pedals`

#![no_main]

use libfuzzer_sys::fuzz_target;
use flight_hotas_ch::parse_pro_pedals;

fuzz_target!(|data: &[u8]| {
    // Must not panic regardless of input
    let _ = parse_pro_pedals(data);
});
