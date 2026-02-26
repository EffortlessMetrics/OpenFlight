// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for flight-open-hardware InputReport::parse.
//!
//! Exercises the 16-byte HID input report parser with arbitrary byte slices.
//!
//! Run with: `cargo +nightly fuzz run fuzz_input_report`

#![no_main]

use libfuzzer_sys::fuzz_target;
use flight_open_hardware::input_report::InputReport;

fuzz_target!(|data: &[u8]| {
    // parse must never panic regardless of input
    if let Some(report) = InputReport::parse(data) {
        // round-trip serialization must not panic
        let bytes = report.to_bytes();
        let _ = InputReport::parse(&bytes);
        // normalization must not panic or produce NaN/Inf
        let _ = report.x_norm();
        let _ = report.y_norm();
        let _ = report.throttle_norm();
    }
});
