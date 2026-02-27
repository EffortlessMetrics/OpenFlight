// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for GoFlight HID report parser across all module variants.
//!
//! Run with: `cargo +nightly fuzz run fuzz_goflight_report`

#![no_main]

use flight_panels_goflight::{GoFlightModule, parse_report};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    for module in [
        GoFlightModule::Gf46,
        GoFlightModule::Gf45,
        GoFlightModule::GfLgt,
        GoFlightModule::GfWcp,
    ] {
        let _ = parse_report(data, module);
    }
});
