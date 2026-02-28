// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for X-Plane RREF (DataRef subscription) response parsing.
//!
//! Exercises `parse_rref_response` against arbitrary bytes, covering the
//! binary RREF response format (pairs of u32 index + f32 value).
//!
//! Run with: `cargo +nightly fuzz run fuzz_rref_response`

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Must never panic on any byte sequence — errors are expected and ignored.
    let _ = flight_xplane::parse_rref_response(data);
});
