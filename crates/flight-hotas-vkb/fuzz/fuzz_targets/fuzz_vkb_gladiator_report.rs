// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for VKB Gladiator NXT EVO HID input report parsing.
//!
//! Exercises the binary parser against arbitrary byte sequences to ensure no
//! panics, OOMs, or undefined behaviour occur.
//!
//! Run with: `cargo +nightly fuzz run fuzz_vkb_gladiator_report`

#![no_main]

use libfuzzer_sys::fuzz_target;
use flight_hotas_vkb::input::GladiatorInputHandler;
use flight_hid_support::device_support::VkbGladiatorVariant;

fuzz_target!(|data: &[u8]| {
    // Test both hand variants with the same byte sequence
    for variant in [VkbGladiatorVariant::NxtEvoRight, VkbGladiatorVariant::NxtEvoLeft] {
        let handler = GladiatorInputHandler::new(variant);

        // parse_report must never panic regardless of input bytes
        let result = handler.parse_report(data);

        // For well-formed reports, axis values must be within documented bounds
        if let Ok(state) = result {
            assert!(state.axes.roll >= -1.0 && state.axes.roll <= 1.0,
                "roll out of bounds: {}", state.axes.roll);
            assert!(state.axes.pitch >= -1.0 && state.axes.pitch <= 1.0,
                "pitch out of bounds: {}", state.axes.pitch);
            assert!(state.axes.yaw >= -1.0 && state.axes.yaw <= 1.0,
                "yaw out of bounds: {}", state.axes.yaw);
            assert!(state.axes.throttle >= -1.0 && state.axes.throttle <= 1.0,
                "throttle out of bounds: {}", state.axes.throttle);
            assert!(state.axes.mini_x >= -1.0 && state.axes.mini_x <= 1.0,
                "mini_x out of bounds: {}", state.axes.mini_x);
            assert!(state.axes.mini_y >= -1.0 && state.axes.mini_y <= 1.0,
                "mini_y out of bounds: {}", state.axes.mini_y);
        }
    }
});
