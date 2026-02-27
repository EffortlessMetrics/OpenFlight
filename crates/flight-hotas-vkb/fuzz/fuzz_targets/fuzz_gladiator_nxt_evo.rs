// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for the VKB Gladiator NXT EVO HID input report parser.
//!
//! Exercises both hand variants against arbitrary byte sequences to ensure no
//! panics and that all axis values remain within their documented ranges.
//!
//! Run with: `cargo +nightly fuzz run fuzz_gladiator_nxt_evo`

#![no_main]

use flight_hotas_vkb::{GladiatorInputHandler, VkbGladiatorVariant};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    for variant in [VkbGladiatorVariant::NxtEvoRight, VkbGladiatorVariant::NxtEvoLeft] {
        let handler = GladiatorInputHandler::new(variant);
        if let Ok(state) = handler.parse_report(data) {
            assert!(
                (-1.0..=1.0).contains(&state.axes.roll),
                "roll out of range: {}",
                state.axes.roll
            );
            assert!(
                (-1.0..=1.0).contains(&state.axes.pitch),
                "pitch out of range: {}",
                state.axes.pitch
            );
            assert!(
                (-1.0..=1.0).contains(&state.axes.yaw),
                "yaw out of range: {}",
                state.axes.yaw
            );
            assert!(
                (0.0..=1.0).contains(&state.axes.throttle),
                "throttle out of range: {}",
                state.axes.throttle
            );
            assert!(
                (-1.0..=1.0).contains(&state.axes.mini_x),
                "mini_x out of range: {}",
                state.axes.mini_x
            );
            assert!(
                (-1.0..=1.0).contains(&state.axes.mini_y),
                "mini_y out of range: {}",
                state.axes.mini_y
            );
        }
    }
});
