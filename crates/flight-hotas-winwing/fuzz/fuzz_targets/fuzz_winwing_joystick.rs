// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for WinWing Orion joystick parser.
//!
//! Run with: `cargo +nightly fuzz run fuzz_winwing_joystick`

#![no_main]

use flight_hotas_winwing::parse_orion_joystick;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = parse_orion_joystick(data);
});
