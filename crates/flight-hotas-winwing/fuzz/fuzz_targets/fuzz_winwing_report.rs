// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for WinWing HID report parsers.
//!
//! Exercises `parse_throttle_report`, `parse_stick_report`, and
//! `parse_rudder_report` with arbitrary byte slices.  All three are
//! infallible from a safety perspective — they may return `Err` but must
//! never panic, cause UB, or produce axes outside expected ranges.

#![no_main]

use flight_hotas_winwing::input::{
    parse_rudder_report, parse_stick_report, parse_throttle_report,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Each parser is length-validated internally and returns Err on short input.
    // We assert axis values stay in expected normalised ranges on success.

    if let Ok(state) = parse_throttle_report(data) {
        let axes = &state.axes;
        assert!(
            (0.0..=1.0).contains(&axes.throttle_left),
            "throttle_left out of range: {}",
            axes.throttle_left
        );
        assert!(
            (0.0..=1.0).contains(&axes.throttle_right),
            "throttle_right out of range: {}",
            axes.throttle_right
        );
        assert!(
            (0.0..=1.0).contains(&axes.throttle_combined),
            "throttle_combined out of range: {}",
            axes.throttle_combined
        );
        assert!(
            (-1.0..=1.0).contains(&axes.mouse_x),
            "mouse_x out of range: {}",
            axes.mouse_x
        );
        assert!(
            (-1.0..=1.0).contains(&axes.mouse_y),
            "mouse_y out of range: {}",
            axes.mouse_y
        );
    }

    if let Ok(state) = parse_stick_report(data) {
        let axes = &state.axes;
        assert!(
            (-1.0..=1.0).contains(&axes.roll),
            "roll out of range: {}",
            axes.roll
        );
        assert!(
            (-1.0..=1.0).contains(&axes.pitch),
            "pitch out of range: {}",
            axes.pitch
        );
    }

    if let Ok(axes) = parse_rudder_report(data) {
        assert!(
            (-1.0..=1.0).contains(&axes.rudder),
            "rudder out of range: {}",
            axes.rudder
        );
        assert!(
            (0.0..=1.0).contains(&axes.brake_left),
            "brake_left out of range: {}",
            axes.brake_left
        );
        assert!(
            (0.0..=1.0).contains(&axes.brake_right),
            "brake_right out of range: {}",
            axes.brake_right
        );
    }
});
