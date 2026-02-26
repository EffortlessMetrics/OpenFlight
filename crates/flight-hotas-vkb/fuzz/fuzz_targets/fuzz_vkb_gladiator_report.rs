// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for VKB HID input report parsing.
//!
//! Exercises the Gladiator NXT EVO handler, STECS throttle handler, and the
//! modern STECS MT standalone parser against arbitrary byte sequences.
//!
//! Run with: `cargo +nightly fuzz run fuzz_vkb_gladiator_report`

#![no_main]

use flight_hotas_vkb::{
    GladiatorInputHandler, StecsInputHandler, StecsMtVariant, VkbGladiatorVariant,
    VkbStecsVariant, parse_stecs_mt_report,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Gladiator NXT EVO — both hand variants
    for variant in [VkbGladiatorVariant::NxtEvoRight, VkbGladiatorVariant::NxtEvoLeft] {
        let handler = GladiatorInputHandler::new(variant);
        if let Ok(state) = handler.parse_report(data) {
            assert!(
                (-1.0..=1.0).contains(&state.axes.roll),
                "roll out of bounds: {}",
                state.axes.roll
            );
            assert!(
                (-1.0..=1.0).contains(&state.axes.pitch),
                "pitch out of bounds: {}",
                state.axes.pitch
            );
            assert!(
                (-1.0..=1.0).contains(&state.axes.yaw),
                "yaw out of bounds: {}",
                state.axes.yaw
            );
            assert!(
                (0.0..=1.0).contains(&state.axes.throttle),
                "throttle out of bounds: {}",
                state.axes.throttle
            );
            assert!(
                (-1.0..=1.0).contains(&state.axes.mini_x),
                "mini_x out of bounds: {}",
                state.axes.mini_x
            );
            assert!(
                (-1.0..=1.0).contains(&state.axes.mini_y),
                "mini_y out of bounds: {}",
                state.axes.mini_y
            );
        }
    }

    // STECS throttle handler — all hand/size variants
    for variant in [
        VkbStecsVariant::RightSpaceThrottleGripMini,
        VkbStecsVariant::LeftSpaceThrottleGripMini,
        VkbStecsVariant::RightSpaceThrottleGripStandard,
        VkbStecsVariant::LeftSpaceThrottleGripStandard,
    ] {
        let handler = StecsInputHandler::new(variant);
        if let Ok(iface) = handler.parse_interface_report(data) {
            if let Some(axes) = iface.axes {
                assert!(
                    (-1.0..=1.0).contains(&axes.z),
                    "stecs z out of bounds: {}",
                    axes.z
                );
            }
        }
    }

    // Modern STECS MT standalone parser — Mini and Max variants
    for variant in [StecsMtVariant::Mini, StecsMtVariant::Max] {
        if let Ok(state) = parse_stecs_mt_report(data, variant) {
            assert!(
                (0.0..=1.0).contains(&state.axes.throttle),
                "stecs_mt throttle out of bounds: {}",
                state.axes.throttle
            );
            assert!(
                (0.0..=1.0).contains(&state.axes.mini_left),
                "stecs_mt mini_left out of bounds: {}",
                state.axes.mini_left
            );
            assert!(
                (0.0..=1.0).contains(&state.axes.mini_right),
                "stecs_mt mini_right out of bounds: {}",
                state.axes.mini_right
            );
        }
    }
});
