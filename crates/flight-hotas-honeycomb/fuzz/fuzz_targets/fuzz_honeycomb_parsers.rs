// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for Honeycomb Aeronautical HID report parsers.
//!
//! Exercises `parse_alpha_report` (Alpha yoke) and `parse_bravo_report`
//! (Bravo throttle quadrant) with arbitrary byte slices to ensure no panics,
//! UB, or out-of-range axis values on any input.
//!
//! Run with: `cargo +nightly fuzz run fuzz_honeycomb_parsers`

#![no_main]

use flight_hotas_honeycomb::{parse_alpha_report, parse_bravo_report};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Alpha yoke — roll/pitch in [-1.0, 1.0]
    if let Ok(state) = parse_alpha_report(data) {
        assert!(
            (-1.0..=1.0).contains(&state.axes.roll),
            "alpha roll out of range: {}",
            state.axes.roll
        );
        assert!(
            (-1.0..=1.0).contains(&state.axes.pitch),
            "alpha pitch out of range: {}",
            state.axes.pitch
        );
    }

    // Bravo throttle quadrant — throttle levers in [0.0, 1.0]
    if let Ok(state) = parse_bravo_report(data) {
        for (name, val) in [
            ("throttle1", state.axes.throttle1),
            ("throttle2", state.axes.throttle2),
            ("throttle3", state.axes.throttle3),
            ("throttle4", state.axes.throttle4),
            ("throttle5", state.axes.throttle5),
        ] {
            assert!(
                (0.0..=1.0).contains(&val),
                "bravo {name} out of range: {val}"
            );
        }
    }
});
