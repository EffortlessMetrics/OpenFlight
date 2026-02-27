// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for the Logitech Extreme 3D Pro HID report parser.
//!
//! Exercises `parse_extreme_3d_pro` with arbitrary byte slices to ensure no
//! panics, UB, or out-of-range axis values on any input.
//!
//! Run with: `cargo +nightly fuzz run fuzz_logitech_parsers`

#![no_main]

use flight_hotas_logitech::parse_extreme_3d_pro;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(state) = parse_extreme_3d_pro(data) {
        assert!(
            (-1.0..=1.0).contains(&state.axes.x),
            "extreme3d x out of range: {}",
            state.axes.x
        );
        assert!(
            (-1.0..=1.0).contains(&state.axes.y),
            "extreme3d y out of range: {}",
            state.axes.y
        );
        assert!(
            (-1.0..=1.0).contains(&state.axes.twist),
            "extreme3d twist out of range: {}",
            state.axes.twist
        );
        assert!(
            (0.0..=1.0).contains(&state.axes.throttle),
            "extreme3d throttle out of range: {}",
            state.axes.throttle
        );
    }
});
