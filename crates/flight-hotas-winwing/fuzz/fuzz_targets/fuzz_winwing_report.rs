// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for WinWing HID report parsers.
//!
//! Exercises all WinWing parsers (Orion2 throttle/stick, TFRP rudder,
//! F-16EX grip, SuperTaurus dual throttle, UFC panel, Skywalker rudder)
//! with arbitrary byte slices.  All parsers must never panic, cause UB,
//! or produce axis values outside their documented ranges.
//!
//! Run with: `cargo +nightly fuzz run fuzz_winwing_report`

#![no_main]

use flight_hotas_winwing::{
    parse_f16ex_stick_report, parse_rudder_report, parse_skywalker_rudder_report,
    parse_stick_report, parse_super_taurus_report, parse_throttle_report,
    parse_ufc_panel_report,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Orion2 throttle — axes in [0.0, 1.0] (throttles) / [-1.0, 1.0] (mouse)
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

    // Orion2 F/A-18C stick — axes in [-1.0, 1.0]
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

    // TFRP rudder pedals — rudder in [-1.0, 1.0], brakes in [0.0, 1.0]
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

    // F-16EX grip — axes in [-1.0, 1.0]
    if let Ok(state) = parse_f16ex_stick_report(data) {
        let axes = &state.axes;
        assert!(
            (-1.0..=1.0).contains(&axes.roll),
            "f16ex roll out of range: {}",
            axes.roll
        );
        assert!(
            (-1.0..=1.0).contains(&axes.pitch),
            "f16ex pitch out of range: {}",
            axes.pitch
        );
    }

    // SuperTaurus dual throttle — axes in [0.0, 1.0]
    if let Ok(state) = parse_super_taurus_report(data) {
        let axes = &state.axes;
        assert!(
            (0.0..=1.0).contains(&axes.throttle_left),
            "super_taurus throttle_left out of range: {}",
            axes.throttle_left
        );
        assert!(
            (0.0..=1.0).contains(&axes.throttle_right),
            "super_taurus throttle_right out of range: {}",
            axes.throttle_right
        );
        assert!(
            (0.0..=1.0).contains(&axes.throttle_combined),
            "super_taurus throttle_combined out of range: {}",
            axes.throttle_combined
        );
    }

    // UFC panel — button-only panel, must not panic
    let _ = parse_ufc_panel_report(data);

    // Skywalker metal rudder pedals — rudder in [-1.0, 1.0], brakes in [0.0, 1.0]
    if let Ok(state) = parse_skywalker_rudder_report(data) {
        let axes = &state.axes;
        assert!(
            (-1.0..=1.0).contains(&axes.rudder),
            "skywalker rudder out of range: {}",
            axes.rudder
        );
        assert!(
            (0.0..=1.0).contains(&axes.brake_left),
            "skywalker brake_left out of range: {}",
            axes.brake_left
        );
        assert!(
            (0.0..=1.0).contains(&axes.brake_right),
            "skywalker brake_right out of range: {}",
            axes.brake_right
        );
    }
});
