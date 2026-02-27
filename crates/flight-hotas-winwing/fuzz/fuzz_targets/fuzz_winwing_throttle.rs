// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for WinWing Orion 2 throttle simple parser.
//!
//! Run with: `cargo +nightly fuzz run fuzz_winwing_throttle`

#![no_main]

use flight_hotas_winwing::parse_orion2_throttle;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = parse_orion2_throttle(data);
});
