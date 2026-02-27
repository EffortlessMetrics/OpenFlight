// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for the WinWing TFRP rudder pedal HID report parser.
//!
//! Exercises `parse_tfrp_report` with arbitrary byte slices to ensure no
//! panics and that all axis values remain within their documented ranges.
//!
//! Run with: `cargo +nightly fuzz run fuzz_tfrp_rudder`

#![no_main]

use flight_hotas_winwing::parse_tfrp_report;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(state) = parse_tfrp_report(data) {
        assert!(
            (-1.0..=1.0).contains(&state.axes.rudder),
            "tfrp rudder out of range: {}",
            state.axes.rudder
        );
        assert!(
            (0.0..=1.0).contains(&state.axes.brake_left),
            "tfrp brake_left out of range: {}",
            state.axes.brake_left
        );
        assert!(
            (0.0..=1.0).contains(&state.axes.brake_right),
            "tfrp brake_right out of range: {}",
            state.axes.brake_right
        );
    }
});
