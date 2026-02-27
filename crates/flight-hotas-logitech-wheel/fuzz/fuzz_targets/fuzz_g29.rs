// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for the Logitech G29/G920/G923 HID report parser.
//!
//! Exercises `parse_g29` with arbitrary byte slices to ensure no panics or
//! out-of-range axis values on any input.
//!
//! Run with: `cargo +nightly fuzz run fuzz_g29`

#![no_main]

use flight_hotas_logitech_wheel::{WheelError, normalize_pedal, normalize_wheel, parse_g29};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    match parse_g29(data) {
        Ok(state) => {
            let wheel = normalize_wheel(state.wheel);
            let gas = normalize_pedal(state.gas);
            let brake = normalize_pedal(state.brake);
            let clutch = normalize_pedal(state.clutch);
            assert!((-1.0f32..=1.0).contains(&wheel), "wheel out of range: {wheel}");
            assert!((0.0f32..=1.0).contains(&gas), "gas out of range: {gas}");
            assert!((0.0f32..=1.0).contains(&brake), "brake out of range: {brake}");
            assert!((0.0f32..=1.0).contains(&clutch), "clutch out of range: {clutch}");
        }
        Err(WheelError::TooShort { .. }) | Err(WheelError::InvalidReportId(_)) => {}
    }
});
