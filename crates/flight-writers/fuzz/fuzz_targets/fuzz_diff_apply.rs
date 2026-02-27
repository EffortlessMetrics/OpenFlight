// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for WriterConfig and FileDiff parsing in flight-writers.
//!
//! Run with: `cargo +nightly fuzz run fuzz_diff_apply`

#![no_main]

use flight_writers::{DiffOperation, FileDiff, WriterConfig};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    // Fuzz WriterConfig JSON deserialization — must never panic
    let _ = serde_json::from_str::<WriterConfig>(input);

    // Fuzz FileDiff JSON deserialization — must never panic
    let _ = serde_json::from_str::<FileDiff>(input);

    // Fuzz DiffOperation JSON deserialization — must never panic
    let _ = serde_json::from_str::<DiffOperation>(input);
});
