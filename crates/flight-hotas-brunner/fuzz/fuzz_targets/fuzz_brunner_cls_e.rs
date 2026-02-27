// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for the Brunner CLS-E force feedback yoke HID parser.
//!
//! Exercises `parse_cls_e_report` with arbitrary byte slices to ensure:
//! - No panics on any input length
//! - Parsed axis values always in `[−1.0, +1.0]`
//! - Button queries for out-of-range indices always return `false`
//!
//! Run with: `cargo +nightly fuzz run fuzz_brunner_cls_e`

#![no_main]

use flight_hotas_brunner::{ClsEButtons, parse_cls_e_report};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(state) = parse_cls_e_report(data) {
        // Axes must always be in [-1.0, +1.0]
        assert!(
            (-1.0..=1.0).contains(&state.axes.roll),
            "cls_e roll out of range: {}",
            state.axes.roll
        );
        assert!(
            (-1.0..=1.0).contains(&state.axes.pitch),
            "cls_e pitch out of range: {}",
            state.axes.pitch
        );

        // Button out-of-range queries must always return false
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

    // Must never panic regardless of length
    let _ = parse_cls_e_report(data);

    // Button helper must not panic on any raw bytes
    if data.len() >= 4 {
        let mut raw = [0u8; 4];
        raw.copy_from_slice(&data[..4]);
        let btns = ClsEButtons { raw };
        let _ = btns.pressed();
        let _ = btns.is_pressed(0);
        let _ = btns.is_pressed(16);
        let _ = btns.is_pressed(33);
    }
});
