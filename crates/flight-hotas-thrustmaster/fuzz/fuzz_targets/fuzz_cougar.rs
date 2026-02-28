// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for Thrustmaster HOTAS Cougar HID report parser.
//!
//! Exercises `parse_cougar` against arbitrary byte slices to ensure
//! no panics and that all axis values remain within their documented ranges.
//!
//! Run with: `cargo +nightly fuzz run fuzz_cougar`

#![no_main]

use flight_hotas_thrustmaster::parse_cougar;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(state) = parse_cougar(data) {
        assert!(
            (-1.0..=1.0).contains(&state.axes.x),
            "cougar x out of range: {}",
            state.axes.x
        );
        assert!(
            (-1.0..=1.0).contains(&state.axes.y),
            "cougar y out of range: {}",
            state.axes.y
        );
        assert!(
            (0.0..=1.0).contains(&state.axes.throttle),
            "cougar throttle out of range: {}",
            state.axes.throttle
        );
    }
});
