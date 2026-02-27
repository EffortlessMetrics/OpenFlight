// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for Sony DualShock 4 and DualSense HID report parsers.
//!
//! Run with: `cargo +nightly fuzz run fuzz_sony_report`

#![no_main]

use flight_hotas_sony::{dualshock::parse_ds4_report, dualsense::parse_dualsense_report};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = parse_ds4_report(data);
    let _ = parse_dualsense_report(data);
});
