// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for VIRPIL Controls VPC HID report parsers.
//!
//! Exercises all six VIRPIL parsers (CM3 throttle, Mongoost stick, Alpha stick,
//! Alpha Prime stick, Panel 1, Panel 2) with arbitrary byte slices to ensure
//! no panics, UB, or out-of-range axis values on valid-length input.
//!
//! Run with: `cargo +nightly fuzz run fuzz_virpil_parsers`

#![no_main]

use flight_hotas_virpil::{
    AlphaPrimeVariant, parse_alpha_prime_report, parse_alpha_report, parse_cm3_throttle_report,
    parse_mongoost_stick_report, parse_panel1_report, parse_panel2_report,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // CM3 throttle — throttle axes in [0.0, 1.0]
    if let Ok(state) = parse_cm3_throttle_report(data) {
        assert!(
            (0.0..=1.0).contains(&state.axes.left_throttle),
            "cm3 left_throttle out of range: {}",
            state.axes.left_throttle
        );
        assert!(
            (0.0..=1.0).contains(&state.axes.right_throttle),
            "cm3 right_throttle out of range: {}",
            state.axes.right_throttle
        );
    }

    // Mongoost stick — x/y in [-1.0, 1.0]
    if let Ok(state) = parse_mongoost_stick_report(data) {
        assert!(
            (-1.0..=1.0).contains(&state.axes.x),
            "mongoost x out of range: {}",
            state.axes.x
        );
        assert!(
            (-1.0..=1.0).contains(&state.axes.y),
            "mongoost y out of range: {}",
            state.axes.y
        );
    }

    // Alpha stick — x/y in [-1.0, 1.0]
    if let Ok(state) = parse_alpha_report(data) {
        assert!(
            (-1.0..=1.0).contains(&state.axes.x),
            "alpha x out of range: {}",
            state.axes.x
        );
        assert!(
            (-1.0..=1.0).contains(&state.axes.y),
            "alpha y out of range: {}",
            state.axes.y
        );
    }

    // Alpha Prime — both grip variants, axes mirror Alpha stick
    for variant in [AlphaPrimeVariant::Left, AlphaPrimeVariant::Right] {
        if let Ok(state) = parse_alpha_prime_report(data, variant) {
            assert!(
                (-1.0..=1.0).contains(&state.axes.x),
                "alpha_prime x out of range: {}",
                state.axes.x
            );
            assert!(
                (-1.0..=1.0).contains(&state.axes.y),
                "alpha_prime y out of range: {}",
                state.axes.y
            );
        }
    }

    // Panel 1 — button-only panel, must not panic
    let _ = parse_panel1_report(data);

    // Panel 2 — mixed axes/buttons panel, must not panic
    let _ = parse_panel2_report(data);
});
