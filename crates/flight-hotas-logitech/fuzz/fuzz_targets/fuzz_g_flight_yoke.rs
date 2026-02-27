// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for the Logitech G Flight Yoke System HID report parser.
//!
//! Exercises `parse_g_flight_yoke` with arbitrary byte slices to ensure no
//! panics and that all axis values remain within their documented ranges.
//!
//! Run with: `cargo +nightly fuzz run fuzz_g_flight_yoke`

#![no_main]

use flight_hotas_logitech::parse_g_flight_yoke;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(state) = parse_g_flight_yoke(data) {
        assert!(
            (-1.0..=1.0).contains(&state.axes.x),
            "g_flight_yoke x out of range: {}",
            state.axes.x
        );
        assert!(
            (-1.0..=1.0).contains(&state.axes.y),
            "g_flight_yoke y out of range: {}",
            state.axes.y
        );
        assert!(
            (0.0..=1.0).contains(&state.axes.rz),
            "g_flight_yoke rz out of range: {}",
            state.axes.rz
        );
        assert!(
            (0.0..=1.0).contains(&state.axes.slider),
            "g_flight_yoke slider out of range: {}",
            state.axes.slider
        );
        assert!(
            (0.0..=1.0).contains(&state.axes.slider2),
            "g_flight_yoke slider2 out of range: {}",
            state.axes.slider2
        );
    }
});
