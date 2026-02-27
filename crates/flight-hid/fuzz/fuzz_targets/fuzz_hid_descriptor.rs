// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for HID report descriptor parsing.
//!
//! Exercises: arbitrary bytes → extract_usages() — must never panic.
//!
//! Run with: `cargo +nightly fuzz run fuzz_hid_descriptor`

#![no_main]

use flight_hid::hid_descriptor::extract_usages;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Parser must never panic on arbitrary descriptor bytes
    let _ = extract_usages(data);
});
