// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for Moza AB9 HID input report parsing.
//!
//! Exercises the binary parser against arbitrary byte sequences to ensure no
//! panics, OOMs, or undefined behaviour occur.
//!
//! Run with: `cargo +nightly fuzz run fuzz_moza_ab9_report`

#![no_main]

use libfuzzer_sys::fuzz_target;
use flight_ffb_moza::input::{parse_ab9_report, AB9_REPORT_LEN};

fuzz_target!(|data: &[u8]| {
    // parse_ab9_report must never panic regardless of input
    let _ = parse_ab9_report(data);

    // For well-formed reports, axis values must be within documented bounds
    if data.len() == AB9_REPORT_LEN && data[0] == 0x01 {
        if let Ok(state) = parse_ab9_report(data) {
            assert!(state.axes.roll >= -1.0 && state.axes.roll <= 1.0);
            assert!(state.axes.pitch >= -1.0 && state.axes.pitch <= 1.0);
            assert!(state.axes.twist >= -1.0 && state.axes.twist <= 1.0);
        }
    }
});
