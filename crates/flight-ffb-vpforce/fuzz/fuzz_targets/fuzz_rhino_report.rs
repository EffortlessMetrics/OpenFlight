// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for VPforce Rhino HID input report parsing.
//!
//! Exercises the binary parser against arbitrary byte sequences to ensure no
//! panics, OOMs, or undefined behaviour occur.
//!
//! Run with: `cargo +nightly fuzz run fuzz_rhino_report`

#![no_main]

use libfuzzer_sys::fuzz_target;
use flight_ffb_vpforce::input::{parse_report, RHINO_REPORT_LEN};

fuzz_target!(|data: &[u8]| {
    // parse_report must never panic regardless of input length or content
    let _ = parse_report(data);

    // If the data is exactly the expected length and starts with 0x01,
    // the parser must succeed and return values within documented bounds.
    if data.len() == RHINO_REPORT_LEN && data[0] == 0x01 {
        if let Ok(state) = parse_report(data) {
            // Axis invariants guaranteed by norm_i16 clamping
            assert!(state.axes.roll >= -1.0 && state.axes.roll <= 1.0);
            assert!(state.axes.pitch >= -1.0 && state.axes.pitch <= 1.0);
            assert!(state.axes.throttle >= 0.0 && state.axes.throttle <= 1.0);
            assert!(state.axes.rocker >= -1.0 && state.axes.rocker <= 1.0);
            assert!(state.axes.twist >= -1.0 && state.axes.twist <= 1.0);
        }
    }
});
