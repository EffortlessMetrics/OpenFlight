// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for Saitek switch panel HID report parser.
//!
//! Exercises `parse_switch_panel_input` against arbitrary byte slices to
//! ensure no panics on malformed HID reports from the switch panel.
//!
//! Run with: `cargo +nightly fuzz run fuzz_switch_panel`

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Must never panic on any byte sequence — None is expected for short/invalid data.
    let _ = flight_panels_saitek::parse_switch_panel_input(data);
});
