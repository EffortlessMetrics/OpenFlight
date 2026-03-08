// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for Logitech Flight Rudder Pedals (Pro Pedals).
//!
//! Covers axis parsing, profile generation, device identification, and
//! legacy device handling.

use flight_hotas_logitech::profiles::{
    rudder_pedals_profile, AxisKind, DeviceProfile,
};
use flight_hotas_logitech::protocol::{
    DeviceId, LOGITECH_VID, SAITEK_VID, identify_device, DEVICE_TABLE,
};

// ═══════════════════════════════════════════════════════════════════════════════
// 1. Axis parsing — profile-level axis structure
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn logitech_pedals_has_three_axes() {
    let p = rudder_pedals_profile();
    assert_eq!(p.axes.len(), 3, "Logitech pedals should have 3 axes");
}

#[test]
fn logitech_pedals_rudder_is_bipolar() {
    let p = rudder_pedals_profile();
    let rudder = p.axes.iter().find(|a| a.name == "Rudder").unwrap();
    assert_eq!(rudder.kind, AxisKind::Bipolar, "rudder should be bipolar");
}

#[test]
fn logitech_pedals_rudder_has_center_detent() {
    let p = rudder_pedals_profile();
    let rudder = p.axes.iter().find(|a| a.name == "Rudder").unwrap();
    assert!(rudder.center_detent, "rudder should have center spring return");
}

#[test]
fn logitech_pedals_toe_brakes_are_unipolar() {
    let p = rudder_pedals_profile();
    let brakes: Vec<_> = p.axes.iter().filter(|a| a.name.contains("Brake")).collect();
    assert_eq!(brakes.len(), 2, "should have left and right toe brakes");
    for brake in &brakes {
        assert_eq!(brake.kind, AxisKind::Unipolar, "{} should be unipolar", brake.name);
    }
}

#[test]
fn logitech_pedals_toe_brakes_no_center_detent() {
    let p = rudder_pedals_profile();
    for brake in p.axes.iter().filter(|a| a.name.contains("Brake")) {
        assert!(!brake.center_detent, "{} should not have center detent", brake.name);
    }
}

#[test]
fn logitech_pedals_axis_resolution_valid() {
    let p = rudder_pedals_profile();
    for axis in &p.axes {
        assert!(
            axis.resolution_bits > 0 && axis.resolution_bits <= 16,
            "{}: resolution {} out of range",
            axis.name,
            axis.resolution_bits
        );
    }
}

#[test]
fn logitech_pedals_rudder_resolution_10bit() {
    let p = rudder_pedals_profile();
    let rudder = p.axes.iter().find(|a| a.name == "Rudder").unwrap();
    assert_eq!(rudder.resolution_bits, 10, "Logitech rudder should be 10-bit");
}

#[test]
fn logitech_pedals_brake_resolution_8bit() {
    let p = rudder_pedals_profile();
    for brake in p.axes.iter().filter(|a| a.name.contains("Brake")) {
        assert_eq!(brake.resolution_bits, 8, "{} should be 8-bit", brake.name);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. Calibration — axis max raw values
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn logitech_pedals_8bit_max_raw() {
    assert_eq!(DeviceProfile::axis_max_raw(8), 255, "8-bit max should be 255");
}

#[test]
fn logitech_pedals_10bit_max_raw() {
    assert_eq!(DeviceProfile::axis_max_raw(10), 1023, "10-bit max should be 1023");
}

#[test]
fn logitech_pedals_centered_axes_contains_rudder() {
    let p = rudder_pedals_profile();
    let centered = p.centered_axes();
    assert_eq!(centered.len(), 1, "only rudder should be centered");
    assert_eq!(centered[0].name, "Rudder");
}

#[test]
fn logitech_pedals_bipolar_axes_count() {
    let p = rudder_pedals_profile();
    let bipolar = p.bipolar_axes();
    assert_eq!(bipolar.len(), 1, "only rudder should be bipolar");
}

#[test]
fn logitech_pedals_unipolar_axes_count() {
    let p = rudder_pedals_profile();
    let unipolar = p.unipolar_axes();
    assert_eq!(unipolar.len(), 2, "two toe brakes should be unipolar");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. Profile generation
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn logitech_pedals_profile_name() {
    let p = rudder_pedals_profile();
    assert!(
        p.name.contains("Rudder"),
        "profile name should mention 'Rudder', got '{}'",
        p.name
    );
    assert!(p.name.contains("Logitech"), "profile should mention 'Logitech'");
}

#[test]
fn logitech_pedals_no_buttons() {
    let p = rudder_pedals_profile();
    assert_eq!(p.button_count, 0, "Logitech pedals have no buttons");
}

#[test]
fn logitech_pedals_no_hats() {
    let p = rudder_pedals_profile();
    assert_eq!(p.hat_count, 0, "Logitech pedals have no hat switches");
}

#[test]
fn logitech_pedals_no_rotaries() {
    let p = rudder_pedals_profile();
    assert!(p.rotaries.is_empty(), "Logitech pedals have no rotary encoders");
}

#[test]
fn logitech_pedals_no_extras() {
    let p = rudder_pedals_profile();
    assert!(p.mfd_pages.is_empty());
    assert!(p.rgb_presets.is_empty());
    assert!(p.led_defaults.is_empty());
    assert!(!p.has_mode_selector);
}

#[test]
fn logitech_pedals_axis_names_nonempty() {
    let p = rudder_pedals_profile();
    for axis in &p.axes {
        assert!(!axis.name.is_empty(), "axis name must not be empty");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. Device identification
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn logitech_flight_rudder_pedals_vid_pid() {
    let info = identify_device(LOGITECH_VID, 0xC264);
    assert!(info.is_some(), "Logitech Flight Rudder Pedals should be identifiable");
    assert_eq!(info.unwrap().id, DeviceId::FlightRudderPedals);
}

#[test]
fn saitek_pro_flight_rudder_pedals_vid_pid() {
    let info = identify_device(SAITEK_VID, 0x0763);
    assert!(info.is_some(), "Saitek Pro Flight Rudder Pedals should be identifiable");
    assert_eq!(info.unwrap().id, DeviceId::ProFlightRudderPedals);
}

#[test]
fn saitek_pro_flight_combat_rudder_vid_pid() {
    let info = identify_device(SAITEK_VID, 0x0764);
    assert!(info.is_some(), "Saitek Pro Flight Combat Rudder should be identifiable");
    assert_eq!(info.unwrap().id, DeviceId::ProFlightCombatRudder);
}

#[test]
fn logitech_unknown_pid_returns_none() {
    assert!(identify_device(LOGITECH_VID, 0xFFFF).is_none());
}

#[test]
fn logitech_pedal_devices_in_device_table() {
    // Verify all three rudder pedal variants are in the device table
    let rudder_entries: Vec<_> = DEVICE_TABLE
        .iter()
        .filter(|d| {
            matches!(
                d.id,
                DeviceId::FlightRudderPedals
                    | DeviceId::ProFlightRudderPedals
                    | DeviceId::ProFlightCombatRudder
            )
        })
        .collect();
    assert_eq!(rudder_entries.len(), 3, "should have 3 rudder pedal entries");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. Legacy device quirks
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn logitech_vid_is_correct() {
    assert_eq!(LOGITECH_VID, 0x046D, "Logitech VID should be 0x046D");
}

#[test]
fn saitek_vid_is_correct() {
    assert_eq!(SAITEK_VID, 0x06A3, "Saitek VID should be 0x06A3");
}

#[test]
fn logitech_pedals_hid_usages_are_set() {
    let p = rudder_pedals_profile();
    for axis in &p.axes {
        assert!(axis.hid_usage > 0, "{}: HID usage should be set", axis.name);
    }
}
