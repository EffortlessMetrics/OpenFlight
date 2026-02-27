// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for the Logitech G940 FFB joystick HID report parser.
//!
//! Exercises `parse_g940_joystick` with arbitrary byte slices to ensure no
//! panics and that all axis values remain within their documented ranges.
//!
//! Run with: `cargo +nightly fuzz run fuzz_g940_joystick`

#![no_main]

use flight_hotas_logitech::parse_g940_joystick;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(state) = parse_g940_joystick(data) {
        assert!(
            (-1.0..=1.0).contains(&state.x),
            "g940 x out of range: {}",
            state.x
        );
        assert!(
            (-1.0..=1.0).contains(&state.y),
            "g940 y out of range: {}",
            state.y
        );
        assert!(
            (-1.0..=1.0).contains(&state.rz),
            "g940 rz out of range: {}",
            state.rz
        );
        assert!(
            (0.0..=1.0).contains(&state.z),
            "g940 z (throttle) out of range: {}",
            state.z
        );
    }
});
