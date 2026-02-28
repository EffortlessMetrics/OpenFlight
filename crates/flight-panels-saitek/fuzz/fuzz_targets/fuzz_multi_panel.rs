// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for Saitek multi panel HID report parser.
//!
//! Exercises `parse_multi_panel_input` against arbitrary byte slices to
//! ensure no panics on malformed HID reports from the multi panel.
//!
//! Run with: `cargo +nightly fuzz run fuzz_multi_panel`

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Must never panic on any byte sequence — None is expected for short/invalid data.
    let _ = flight_panels_saitek::parse_multi_panel_input(data);
});
