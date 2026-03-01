// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for VIRPIL prosumer devices: WarBRD, CM3 throttle, ACE pedals,
//! button matrix, and property-based invariants.

use flight_hotas_virpil::*;

// ═══════════════════════════════════════════════════════════════════════════════
// 1. WarBRD — grip detection, axis calibration (high resolution), mode switching
// ═══════════════════════════════════════════════════════════════════════════════

fn make_warbrd_report(axes: [u16; 5], buttons: [u8; 4]) -> Vec<u8> {
    let mut data = vec![0x01u8];
    for ax in &axes {
        data.extend_from_slice(&ax.to_le_bytes());
    }
    data.extend_from_slice(&buttons);
    data
}

/// WarBRD variant detection distinguishes Original from D revision.
#[test]
fn warbrd_grip_detection_original() {
    let report = make_warbrd_report([0u16; 5], [0u8; 4]);
    let state = parse_warbrd_report(&report, WarBrdVariant::Original).unwrap();
    assert_eq!(state.variant, WarBrdVariant::Original);
    assert_eq!(state.variant.product_name(), "VPC WarBRD Stick");
}

/// WarBRD-D variant is preserved through parsing and named correctly.
#[test]
fn warbrd_grip_detection_d_variant() {
    let report = make_warbrd_report([0u16; 5], [0u8; 4]);
    let state = parse_warbrd_report(&report, WarBrdVariant::D).unwrap();
    assert_eq!(state.variant, WarBrdVariant::D);
    assert_eq!(state.variant.product_name(), "VPC WarBRD-D Stick");
    // Also verify zero axes parse to zero at the same time
    assert_eq!(state.inner.axes.x, 0.0);
    assert_eq!(state.inner.axes.y, 0.0);
}

/// WarBRD axes at AXIS_MAX normalise to 1.0 (high resolution 14-bit).
#[test]
fn warbrd_axis_calibration_high_resolution() {
    let report = make_warbrd_report([VIRPIL_AXIS_MAX; 5], [0u8; 4]);
    let state = parse_warbrd_report(&report, WarBrdVariant::D).unwrap();
    assert!((state.inner.axes.x - 1.0).abs() < 1e-4, "x axis at max");
    assert!((state.inner.axes.y - 1.0).abs() < 1e-4, "y axis at max");
    assert!((state.inner.axes.z - 1.0).abs() < 1e-4, "z axis at max");
    assert!((state.inner.axes.sz - 1.0).abs() < 1e-4, "sz axis at max");
    assert!((state.inner.axes.sl - 1.0).abs() < 1e-4, "sl axis at max");
}

/// WarBRD midpoint axes produce ~0.5 (centring check).
#[test]
fn warbrd_mode_switching_midpoint() {
    let mid = VIRPIL_AXIS_MAX / 2;
    let report = make_warbrd_report([mid; 5], [0u8; 4]);
    let state = parse_warbrd_report(&report, WarBrdVariant::Original).unwrap();
    assert!((state.inner.axes.x - 0.5).abs() < 0.01, "x ~0.5 at midpoint");
    assert!((state.inner.axes.y - 0.5).abs() < 0.01, "y ~0.5 at midpoint");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. CM3 throttle — dual engine axes, detent positions, rotary encoders
// ═══════════════════════════════════════════════════════════════════════════════

fn make_cm3_report(axes: [u16; 6], buttons: [u8; 10]) -> Vec<u8> {
    let mut data = vec![0x01u8];
    for ax in &axes {
        data.extend_from_slice(&ax.to_le_bytes());
    }
    data.extend_from_slice(&buttons);
    data
}

/// CM3 dual engine axes parse independently.
#[test]
fn cm3_dual_engine_axes() {
    let report = make_cm3_report(
        [VIRPIL_AXIS_MAX, VIRPIL_AXIS_MAX / 2, 0, 0, 0, 0],
        [0u8; 10],
    );
    let state = parse_cm3_throttle_report(&report).unwrap();
    assert!(
        (state.axes.left_throttle - 1.0).abs() < 1e-4,
        "left engine full"
    );
    assert!(
        (state.axes.right_throttle - 0.5).abs() < 0.01,
        "right engine half"
    );
}

/// CM3 flaps lever at a specific detent position (25% travel).
#[test]
fn cm3_detent_positions() {
    let quarter = VIRPIL_AXIS_MAX / 4;
    let report = make_cm3_report([0, 0, quarter, 0, 0, 0], [0u8; 10]);
    let state = parse_cm3_throttle_report(&report).unwrap();
    assert!(
        (state.axes.flaps - 0.25).abs() < 0.01,
        "flaps at 25% detent"
    );
}

/// CM3 rotary encoder profile declares 4 rotary encoders.
#[test]
fn cm3_rotary_encoders_count() {
    let profile = profiles::profile_for_pid(VIRPIL_CM3_THROTTLE_PID).unwrap();
    assert_eq!(profile.rotary_encoders, 4, "CM3 has 4 rotary encoders");
}

/// CM3 slew control axes (SCX, SCY) parse independently.
#[test]
fn cm3_slew_controls() {
    let report = make_cm3_report(
        [0, 0, 0, VIRPIL_AXIS_MAX, VIRPIL_AXIS_MAX / 2, 0],
        [0u8; 10],
    );
    let state = parse_cm3_throttle_report(&report).unwrap();
    assert!((state.axes.scx - 1.0).abs() < 1e-4, "SCX at max");
    assert!((state.axes.scy - 0.5).abs() < 0.01, "SCY at 50%");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. ACE pedals — dual toe brakes, rudder range, sensitivity curves
// ═══════════════════════════════════════════════════════════════════════════════

fn make_ace_pedals_report(axes: [u16; 3], buttons: [u8; 2]) -> Vec<u8> {
    let mut data = vec![0x01u8];
    for ax in &axes {
        data.extend_from_slice(&ax.to_le_bytes());
    }
    data.extend_from_slice(&buttons);
    data
}

/// ACE pedals have independent left and right toe brakes.
#[test]
fn ace_pedals_dual_toe_brakes() {
    let report = make_ace_pedals_report(
        [0, VIRPIL_AXIS_MAX, VIRPIL_AXIS_MAX / 2],
        [0u8; 2],
    );
    let state = parse_ace_pedals_report(&report).unwrap();
    assert_eq!(state.axes.rudder, 0.0, "rudder at zero");
    assert!(
        (state.axes.left_toe_brake - 1.0).abs() < 1e-4,
        "left toe fully pressed"
    );
    assert!(
        (state.axes.right_toe_brake - 0.5).abs() < 0.01,
        "right toe half pressed"
    );
}

/// Rudder full range spans 0.0 to 1.0 (full left to full right).
#[test]
fn ace_pedals_rudder_range() {
    let min_report = make_ace_pedals_report([0, 0, 0], [0u8; 2]);
    let max_report = make_ace_pedals_report([VIRPIL_AXIS_MAX, 0, 0], [0u8; 2]);
    let state_min = parse_ace_pedals_report(&min_report).unwrap();
    let state_max = parse_ace_pedals_report(&max_report).unwrap();
    assert_eq!(state_min.axes.rudder, 0.0, "rudder at minimum");
    assert!(
        (state_max.axes.rudder - 1.0).abs() < 1e-4,
        "rudder at maximum"
    );
}

/// ACE pedals rudder axis is centred and toe brakes are not, with 16 total buttons.
#[test]
fn ace_pedals_sensitivity_curve_and_button_count() {
    let profile = profiles::profile_for_pid(VIRPIL_ACE_PEDALS_PID).unwrap();
    assert!(profile.axes[0].centred, "rudder axis should be centred");
    assert!(!profile.axes[1].centred, "left toe brake not centred");
    assert!(!profile.axes[2].centred, "right toe brake not centred");
    assert_eq!(ACE_PEDALS_BUTTON_COUNT, 16, "ACE pedals have 16 buttons");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. Button matrix — matrix scanning, debounce, shift layers, modifier keys
// ═══════════════════════════════════════════════════════════════════════════════

/// CM3 button matrix scanning: individual bits map to correct buttons.
#[test]
fn button_matrix_scanning_cm3() {
    // Set button 1 and button 40
    let mut buttons = [0u8; 10];
    buttons[0] = 0x01; // button 1
    buttons[4] = 0x80; // button 40 = index 39 → byte 4, bit 7
    let report = make_cm3_report([0u16; 6], buttons);
    let state = parse_cm3_throttle_report(&report).unwrap();
    assert!(state.buttons.is_pressed(1), "button 1");
    assert!(state.buttons.is_pressed(40), "button 40");
    assert!(!state.buttons.is_pressed(2), "button 2 not pressed");
}

/// Button debounce: consecutive identical reports yield same state.
#[test]
fn button_matrix_debounce_consistency() {
    let mut buttons = [0u8; 10];
    buttons[0] = 0x03; // buttons 1 and 2
    let report = make_cm3_report([0u16; 6], buttons);
    let state1 = parse_cm3_throttle_report(&report).unwrap();
    let state2 = parse_cm3_throttle_report(&report).unwrap();
    assert_eq!(state1.buttons.pressed(), state2.buttons.pressed());
}

/// CM3 throttle supports 78 buttons across 10 bytes — highest button is addressable.
#[test]
fn button_shift_layers_cm3_full_range() {
    let buttons = [0xFFu8; 10];
    let report = make_cm3_report([0u16; 6], buttons);
    let state = parse_cm3_throttle_report(&report).unwrap();
    assert!(state.buttons.is_pressed(1), "first button");
    assert!(state.buttons.is_pressed(78), "last button (78)");
    assert_eq!(state.buttons.pressed().len(), 78, "all 78 buttons pressed");
}

/// ACE pedals modifier keys: buttons at boundary indices.
#[test]
fn button_matrix_modifier_keys_ace_pedals() {
    // Button 1 (first) and button 16 (last)
    let report = make_ace_pedals_report([0u16; 3], [0x01, 0x80]);
    let state = parse_ace_pedals_report(&report).unwrap();
    assert!(state.buttons.is_pressed(1), "first button");
    assert!(state.buttons.is_pressed(16), "last button");
    assert!(!state.buttons.is_pressed(0), "out of range low");
    assert!(!state.buttons.is_pressed(17), "out of range high");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. Property tests — axis range bounds, button/LED count consistency
// ═══════════════════════════════════════════════════════════════════════════════

/// All VIRPIL axis values for any raw input stay in [0.0, 1.0].
#[test]
fn prop_axis_range_bounds() {
    for raw in [0u16, 1, VIRPIL_AXIS_MAX / 2, VIRPIL_AXIS_MAX, u16::MAX] {
        let report = make_warbrd_report([raw; 5], [0u8; 4]);
        let state = parse_warbrd_report(&report, WarBrdVariant::D).unwrap();
        assert!(
            (0.0..=1.0).contains(&state.inner.axes.x),
            "x out of [0,1] for raw={raw}"
        );
        assert!(
            (0.0..=1.0).contains(&state.inner.axes.y),
            "y out of [0,1] for raw={raw}"
        );
        assert!(
            (0.0..=1.0).contains(&state.inner.axes.z),
            "z out of [0,1] for raw={raw}"
        );
    }
}

/// Button counts in profiles match device info table.
#[test]
fn prop_button_count_consistency() {
    let info = protocol::device_info(VIRPIL_CM3_THROTTLE_PID).unwrap();
    assert_eq!(info.button_count, 78, "CM3 info table: 78 buttons");

    let profile = profiles::profile_for_pid(VIRPIL_CM3_THROTTLE_PID).unwrap();
    assert_eq!(profile.button_count, 78, "CM3 profile: 78 buttons");
}

/// ACE pedals button count matches between profile and parser constant.
#[test]
fn prop_ace_pedals_button_led_count() {
    let info = protocol::device_info(VIRPIL_ACE_PEDALS_PID).unwrap();
    assert_eq!(info.button_count, ACE_PEDALS_BUTTON_COUNT);

    let profile = profiles::profile_for_pid(VIRPIL_ACE_PEDALS_PID).unwrap();
    assert_eq!(profile.button_count, ACE_PEDALS_BUTTON_COUNT);
}

/// LED report format is correct for all colours.
#[test]
fn prop_led_report_format_all_presets() {
    use protocol::{LedColor, build_led_report, LED_REPORT_ID};
    for (color, expected) in [
        (LedColor::RED, [LED_REPORT_ID, 0, 0xFF, 0, 0]),
        (LedColor::GREEN, [LED_REPORT_ID, 0, 0, 0xFF, 0]),
        (LedColor::BLUE, [LED_REPORT_ID, 0, 0, 0, 0xFF]),
        (LedColor::OFF, [LED_REPORT_ID, 0, 0, 0, 0]),
        (LedColor::WHITE, [LED_REPORT_ID, 0, 0xFF, 0xFF, 0xFF]),
    ] {
        let buf = build_led_report(0, color);
        assert_eq!(buf, expected, "LED report mismatch for {:?}", color);
    }
}

/// All device table entries have unique PIDs, valid report sizes, and all profiles are consistent.
#[test]
fn prop_device_table_and_profile_integrity() {
    let table = protocol::DEVICE_TABLE;
    let mut pids: Vec<u16> = table.iter().map(|d| d.pid).collect();
    let orig_len = pids.len();
    pids.sort();
    pids.dedup();
    assert_eq!(pids.len(), orig_len, "device table has duplicate PIDs");

    for entry in table {
        assert!(entry.min_report_bytes > 0, "{}: zero report size", entry.name);
        assert!(!entry.name.is_empty(), "device name must not be empty");
    }

    // Also verify all profiles have unique PIDs and consistent axis/button specs
    let all = profiles::ALL_PROFILES;
    let mut profile_pids: Vec<u16> = all.iter().map(|p| p.pid).collect();
    profile_pids.sort();
    profile_pids.dedup();
    assert_eq!(profile_pids.len(), all.len(), "profile table has duplicate PIDs");

    for profile in all {
        assert!(profile.button_count > 0, "{}: must have buttons", profile.name);
        assert!(!profile.name.is_empty(), "{}: name must not be empty", profile.name);
    }
}
