// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for Turtle Beach VelocityOne HID report parsers.
//!
//! Run with: `cargo +nightly fuzz run fuzz_velocityone`

#![no_main]

use flight_hotas_turtlebeach::{parse_flightdeck_report, parse_rudder_report};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = parse_flightdeck_report(data);
    let _ = parse_rudder_report(data);
});
