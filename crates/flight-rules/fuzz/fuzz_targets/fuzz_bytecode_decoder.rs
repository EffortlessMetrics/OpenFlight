// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for the bytecode decoder (JSON deserialization).
//!
//! Feeds arbitrary bytes to `serde_json::from_slice::<BytecodeProgram>()` and
//! verifies it never panics — only returns `Ok` or `Err`.
//!
//! Run with: `cargo +nightly fuzz run fuzz_bytecode_decoder`

#![no_main]

use flight_rules::BytecodeProgram;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Attempt to deserialize arbitrary bytes as a BytecodeProgram.
    // Must never panic regardless of input.
    let _ = serde_json::from_slice::<BytecodeProgram>(data);
});
