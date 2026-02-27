// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for the WinWing Orion 2 throttle HID report parser.
//!
//! Exercises `parse_orion2_throttle_report` with arbitrary byte slices to
//! ensure no panics and that all axis values remain within their documented
//! ranges.
//!
//! Run with: `cargo +nightly fuzz run fuzz_orion2_throttle`

#![no_main]

use flight_hotas_winwing::parse_orion2_throttle_report;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(state) = parse_orion2_throttle_report(data) {
        let axes = &state.axes;
        assert!(
            (0.0..=1.0).contains(&axes.throttle_left),
            "orion2_throttle throttle_left out of range: {}",
            axes.throttle_left
        );
        assert!(
            (0.0..=1.0).contains(&axes.throttle_right),
            "orion2_throttle throttle_right out of range: {}",
            axes.throttle_right
        );
        assert!(
            (0.0..=1.0).contains(&axes.throttle_combined),
            "orion2_throttle throttle_combined out of range: {}",
            axes.throttle_combined
        );
    }
});
