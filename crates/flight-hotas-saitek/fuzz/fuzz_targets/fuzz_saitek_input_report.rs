// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for Saitek HOTAS HID input report parsing.
//!
//! Exercises the binary parser for X52, X52Pro, X55, X56 against arbitrary
//! byte sequences to ensure no panics or undefined behaviour occur.
//!
//! Run with: `cargo +nightly fuzz run fuzz_saitek_input_report`

#![no_main]

use libfuzzer_sys::fuzz_target;
use flight_hotas_saitek::input::HotasInputHandler;
use flight_hotas_saitek::SaitekHotasType;

fuzz_target!(|data: &[u8]| {
    // Test all four device types with the same byte sequence
    for device_type in [
        SaitekHotasType::X52,
        SaitekHotasType::X52Pro,
        SaitekHotasType::X55Stick,
        SaitekHotasType::X56Throttle,
    ] {
        let mut handler = HotasInputHandler::new(device_type);
        // parse_report must never panic regardless of input
        let state = handler.parse_report(data);

        // All axis values must be within [-1.0, 1.0]
        assert!((-1.0..=1.0).contains(&state.axes.stick_x),
            "stick_x OOB: {}", state.axes.stick_x);
        assert!((-1.0..=1.0).contains(&state.axes.throttle),
            "throttle OOB: {}", state.axes.throttle);
        assert!((-1.0..=1.0).contains(&state.axes.throttle2),
            "throttle2 OOB: {}", state.axes.throttle2);
    }
});
