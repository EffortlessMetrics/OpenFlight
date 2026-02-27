// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for VKB STECS throttle HID report parsers.
//!
//! Exercises the StecsInputHandler (legacy interface) and parse_stecs_mt_report
//! (modern standalone) against arbitrary byte sequences to ensure no panics
//! and that all axis values remain within their documented ranges.
//!
//! Run with: `cargo +nightly fuzz run fuzz_stecs`

#![no_main]

use flight_hotas_vkb::{
    StecsInputHandler, StecsMtVariant, VkbStecsVariant, parse_stecs_mt_report,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
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
                    "stecs z out of range: {}",
                    axes.z
                );
            }
        }
    }

    for variant in [StecsMtVariant::Mini, StecsMtVariant::Max] {
        if let Ok(state) = parse_stecs_mt_report(data, variant) {
            assert!(
                (0.0..=1.0).contains(&state.axes.throttle),
                "stecs_mt throttle out of range: {}",
                state.axes.throttle
            );
            assert!(
                (0.0..=1.0).contains(&state.axes.mini_left),
                "stecs_mt mini_left out of range: {}",
                state.axes.mini_left
            );
            assert!(
                (0.0..=1.0).contains(&state.axes.mini_right),
                "stecs_mt mini_right out of range: {}",
                state.axes.mini_right
            );
        }
    }
});
