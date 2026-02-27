// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for Thrustmaster HID report parsers.
//!
//! Exercises all Thrustmaster parsers (T.16000M FCS, TWCS throttle, TFRP
//! rudder pedals, HOTAS Warthog stick and throttle, T.Flight HOTAS) with
//! arbitrary byte slices to ensure no panics, UB, or out-of-range axis values.
//!
//! Run with: `cargo +nightly fuzz run fuzz_thrustmaster_parsers`

#![no_main]

use flight_hotas_thrustmaster::{
    TFlightInputHandler, TFlightModel, parse_t16000m_report, parse_tfrp_report,
    parse_twcs_report, parse_warthog_stick, parse_warthog_throttle,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // T.16000M FCS joystick — x/y/twist in [-1.0, 1.0]
    if let Ok(state) = parse_t16000m_report(data) {
        assert!(
            (-1.0..=1.0).contains(&state.axes.x),
            "t16000m x out of range: {}",
            state.axes.x
        );
        assert!(
            (-1.0..=1.0).contains(&state.axes.y),
            "t16000m y out of range: {}",
            state.axes.y
        );
    }

    // TWCS throttle — main throttle in [0.0, 1.0], mini-stick in [-1.0, 1.0]
    if let Ok(state) = parse_twcs_report(data) {
        assert!(
            (0.0..=1.0).contains(&state.axes.throttle),
            "twcs throttle out of range: {}",
            state.axes.throttle
        );
        assert!(
            (-1.0..=1.0).contains(&state.axes.mini_stick_x),
            "twcs mini_stick_x out of range: {}",
            state.axes.mini_stick_x
        );
    }

    // TFRP rudder pedals — rudder in [0.0, 1.0]
    if let Ok(state) = parse_tfrp_report(data) {
        assert!(
            (0.0..=1.0).contains(&state.axes.rudder),
            "tfrp rudder out of range: {}",
            state.axes.rudder
        );
    }

    // HOTAS Warthog joystick — x/y/rz in [-1.0, 1.0]
    if let Ok(state) = parse_warthog_stick(data) {
        assert!(
            (-1.0..=1.0).contains(&state.axes.x),
            "warthog stick x out of range: {}",
            state.axes.x
        );
        assert!(
            (-1.0..=1.0).contains(&state.axes.y),
            "warthog stick y out of range: {}",
            state.axes.y
        );
        assert!(
            (-1.0..=1.0).contains(&state.axes.rz),
            "warthog stick rz out of range: {}",
            state.axes.rz
        );
    }

    // HOTAS Warthog throttle — throttle levers in [0.0, 1.0], slew in [-1.0, 1.0]
    if let Ok(state) = parse_warthog_throttle(data) {
        assert!(
            (0.0..=1.0).contains(&state.axes.throttle_left),
            "warthog throttle_left out of range: {}",
            state.axes.throttle_left
        );
        assert!(
            (0.0..=1.0).contains(&state.axes.throttle_right),
            "warthog throttle_right out of range: {}",
            state.axes.throttle_right
        );
        assert!(
            (-1.0..=1.0).contains(&state.axes.slew_x),
            "warthog slew_x out of range: {}",
            state.axes.slew_x
        );
    }

    // T.Flight HOTAS (One, 4, X) — stateful handler, one variant per call
    for model in [TFlightModel::HotasOne, TFlightModel::Hotas4, TFlightModel::HotasX] {
        let mut handler = TFlightInputHandler::new(model);
        let state = handler.parse_report(data);
        assert!(
            (-1.0..=1.0).contains(&state.axes.roll),
            "tflight roll out of range: {}",
            state.axes.roll
        );
        assert!(
            (-1.0..=1.0).contains(&state.axes.pitch),
            "tflight pitch out of range: {}",
            state.axes.pitch
        );
        assert!(
            (0.0..=1.0).contains(&state.axes.throttle),
            "tflight throttle out of range: {}",
            state.axes.throttle
        );
    }
});
