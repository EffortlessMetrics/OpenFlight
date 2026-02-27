// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for the VPforce Rhino FFB joystick HID report parser.
//!
//! Exercises `parse_rhino_report` with arbitrary byte slices to ensure:
//! - No panics on any input
//! - Parsed axis values in correct ranges
//! - Button queries for out-of-range indices always return `false`
//!
//! Run with: `cargo +nightly fuzz run fuzz_vpforce_rhino`

#![no_main]

use flight_hotas_vpforce::{RhinoButtons, parse_rhino_report};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(state) = parse_rhino_report(data) {
        // Signed axes must be in [-1.0, +1.0]
        assert!(
            (-1.0..=1.0).contains(&state.axes.roll),
            "rhino roll out of range: {}",
            state.axes.roll
        );
        assert!(
            (-1.0..=1.0).contains(&state.axes.pitch),
            "rhino pitch out of range: {}",
            state.axes.pitch
        );
        assert!(
            (-1.0..=1.0).contains(&state.axes.rocker),
            "rhino rocker out of range: {}",
            state.axes.rocker
        );
        assert!(
            (-1.0..=1.0).contains(&state.axes.twist),
            "rhino twist out of range: {}",
            state.axes.twist
        );
        assert!(
            (-1.0..=1.0).contains(&state.axes.ry),
            "rhino ry out of range: {}",
            state.axes.ry
        );

        // Throttle is remapped to [0.0, 1.0]
        assert!(
            (0.0..=1.0).contains(&state.axes.throttle),
            "rhino throttle out of range: {}",
            state.axes.throttle
        );

        // Button out-of-range queries must return false
        assert!(!state.buttons.is_pressed(0), "button 0 (out of range) should be false");
        assert!(!state.buttons.is_pressed(33), "button 33 (out of range) should be false");

        // pressed() must return only valid button numbers
        for &n in &state.buttons.pressed() {
            assert!(
                (1..=32).contains(&n),
                "pressed() returned out-of-range button {}",
                n
            );
        }
    }

    // Must never panic on any input
    let _ = parse_rhino_report(data);

    // Button helpers must not panic
    let btns = RhinoButtons { mask: 0xFFFF_FFFF, hat: 0xFF };
    let _ = btns.pressed();
    let _ = btns.is_pressed(0);
    let _ = btns.is_pressed(16);
    let _ = btns.is_pressed(33);
});
