// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for the HID descriptor parser.
//!
//! Exercises `parse_descriptor` against arbitrary byte sequences to ensure
//! no panics or undefined behaviour on malformed input.
//!
//! Run with: `cargo +nightly fuzz run fuzz_descriptor`

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Must never panic on any byte sequence — errors are expected and ignored.
    let _ = flight_hid::descriptor_parser::parse_descriptor(data);
});
