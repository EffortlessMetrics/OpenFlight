// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for the WinWing Orion 2 stick HID report parser.
//!
//! Exercises `parse_orion2_stick_report` with arbitrary byte slices to ensure
//! no panics and that all axis values remain within their documented ranges.
//!
//! Run with: `cargo +nightly fuzz run fuzz_orion2_stick`

#![no_main]

use flight_hotas_winwing::parse_orion2_stick_report;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(state) = parse_orion2_stick_report(data) {
        assert!(
            (-1.0..=1.0).contains(&state.axes.roll),
            "orion2_stick roll out of range: {}",
            state.axes.roll
        );
        assert!(
            (-1.0..=1.0).contains(&state.axes.pitch),
            "orion2_stick pitch out of range: {}",
            state.axes.pitch
        );
    }
});
