// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for Turtle Beach VelocityOne Flight and Flightstick devices.
//!
//! Covers yoke axes, throttle quadrant, rudder, button/switch mapping, and
//! device identification for the full VelocityOne product family.

use flight_hotas_turtlebeach::{
    // Device database
    TURTLE_BEACH_VID, VelocityOneDevice, capabilities, identify_device, is_turtle_beach_device,
    // Protocol — Flight (yoke)
    FLIGHT_MIN_REPORT_BYTES, decode_all_toggles, decode_toggle_switch,
    parse_flight_report, serialize_gear_led_report, serialize_display_command,
    FlightLedState, GearLedState, ToggleSwitchPosition, TrimWheelTracker,
    DisplayCommand, DisplayPage,
    // Protocol — Flightstick
    FLIGHTSTICK_MIN_REPORT_BYTES, parse_flightstick_report,
    // Legacy (VID 0x1432) — Flightdeck / Rudder
    parse_flightdeck_report, parse_rudder_report, RUDDER_MIN_REPORT_BYTES,
    // Profiles
    profile_for_device,
};

// ── Report builders (shared) ─────────────────────────────────────────────────

mod common;
use common::{FlightInput, build_flight, make_flightstick, make_flightdeck, make_rudder};

// ═══════════════════════════════════════════════════════════════════════════════
// 1. DEVICE IDENTIFICATION — VID/PID matching, model discrimination
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn identify_device_roundtrip_all_variants() {
    for device in VelocityOneDevice::all() {
        let pid = device.product_id();
        let identified = identify_device(pid).unwrap();
        assert_eq!(
            identified, *device,
            "roundtrip failed for {:?} (PID 0x{:04X})",
            device, pid
        );
    }
}

#[test]
fn vid_pid_matching_all_known_devices() {
    let known = [
        (0x1050u16, VelocityOneDevice::Flight),
        (0x1051, VelocityOneDevice::Rudder),
        (0x1052, VelocityOneDevice::Flightstick),
        (0x0210, VelocityOneDevice::FlightPro),
        (0x1073, VelocityOneDevice::FlightUniversal),
        (0x3085, VelocityOneDevice::FlightYoke),
    ];
    for (pid, expected) in known {
        assert!(is_turtle_beach_device(TURTLE_BEACH_VID, pid),
            "VID/PID 0x{:04X}/0x{:04X} should be recognised", TURTLE_BEACH_VID, pid);
        assert_eq!(identify_device(pid), Some(expected),
            "PID 0x{:04X} should identify as {:?}", pid, expected);
    }
}

#[test]
fn is_turtle_beach_rejects_wrong_vid_all_pids() {
    for device in VelocityOneDevice::all() {
        assert!(
            !is_turtle_beach_device(0x0000, device.product_id()),
            "VID=0 should not match {:?}",
            device
        );
        assert!(
            !is_turtle_beach_device(0xFFFF, device.product_id()),
            "VID=0xFFFF should not match {:?}",
            device
        );
    }
}

#[test]
fn is_turtle_beach_accepts_correct_vid_all_pids() {
    for device in VelocityOneDevice::all() {
        assert!(
            is_turtle_beach_device(TURTLE_BEACH_VID, device.product_id()),
            "should match {:?}",
            device
        );
    }
}

#[test]
fn identify_device_returns_none_for_boundary_pids() {
    assert!(identify_device(0x0000).is_none());
    assert!(identify_device(0xFFFF).is_none());
    assert!(identify_device(0x1049).is_none()); // just below Flight PID
    assert!(identify_device(0x1053).is_none()); // just above Flightstick PID
}

#[test]
fn model_discrimination_unique_pids() {
    let devices = VelocityOneDevice::all();
    let mut pids: Vec<u16> = devices.iter().map(|d| d.product_id()).collect();
    let len_before = pids.len();
    pids.sort();
    pids.dedup();
    assert_eq!(pids.len(), len_before, "all PIDs must be unique");
}

#[test]
fn device_names_contain_turtle_beach_and_velocityone() {
    for device in VelocityOneDevice::all() {
        let name = device.name();
        assert!(
            name.contains("Turtle Beach"),
            "missing 'Turtle Beach' in {name}"
        );
        assert!(
            name.contains("VelocityOne"),
            "missing 'VelocityOne' in {name}"
        );
    }
}

#[test]
fn device_display_matches_name() {
    for device in VelocityOneDevice::all() {
        assert_eq!(format!("{device}"), device.name());
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. CAPABILITIES & PROFILES — consistency checks
// ═══════════════════════════════════════════════════════════════════════════════

/// The four primary devices (Flight, Flightstick, Rudder) have dedicated profiles.
/// FlightPro, FlightUniversal, and FlightYoke intentionally share the Flight
/// profile as a baseline, so caps vs profile checks only apply to the primaries.
const PRIMARY_DEVICES: &[VelocityOneDevice] = &[
    VelocityOneDevice::Flight,
    VelocityOneDevice::Flightstick,
    VelocityOneDevice::Rudder,
];

#[test]
fn capabilities_axis_count_matches_profile_primary() {
    for dev in PRIMARY_DEVICES {
        let caps = capabilities(*dev);
        let profile = profile_for_device(*dev);
        assert_eq!(
            caps.axes as usize,
            profile.axes.len(),
            "axis count mismatch for {:?}",
            dev
        );
    }
}

#[test]
fn capabilities_led_flag_matches_profile_primary() {
    for dev in PRIMARY_DEVICES {
        let caps = capabilities(*dev);
        let profile = profile_for_device(*dev);
        assert_eq!(
            caps.has_leds, profile.has_leds,
            "has_leds mismatch for {:?}",
            dev
        );
    }
}

#[test]
fn capabilities_display_flag_matches_profile_primary() {
    for dev in PRIMARY_DEVICES {
        let caps = capabilities(*dev);
        let profile = profile_for_device(*dev);
        assert_eq!(
            caps.has_display, profile.has_display,
            "has_display mismatch for {:?}",
            dev
        );
    }
}

#[test]
fn capabilities_gear_lever_matches_profile_primary() {
    for dev in PRIMARY_DEVICES {
        let caps = capabilities(*dev);
        let profile = profile_for_device(*dev);
        assert_eq!(
            caps.has_gear_lever, profile.has_gear_lever,
            "has_gear_lever mismatch for {:?}",
            dev
        );
    }
}

#[test]
fn capabilities_toggle_count_matches_profile_primary() {
    for dev in PRIMARY_DEVICES {
        let caps = capabilities(*dev);
        let profile = profile_for_device(*dev);
        assert_eq!(
            caps.toggle_switch_count, profile.toggle_switch_count,
            "toggle_switch_count mismatch for {:?}",
            dev
        );
    }
}

#[test]
fn combined_unit_detection_flight_has_all_features() {
    let caps = capabilities(VelocityOneDevice::Flight);
    assert_eq!(caps.axes, 6, "Flight should have 6 axes (yoke 2 + rudder twist + throttle 2 + trim)");
    assert!(caps.has_leds, "Flight should have gear LEDs");
    assert!(caps.has_display, "Flight should have a display");
    assert!(caps.has_trim_wheel, "Flight should have a trim wheel");
    assert!(caps.has_gear_lever, "Flight should have a gear lever");
    assert!(caps.toggle_switch_count >= 7, "Flight should have ≥7 toggle switches");
}

#[test]
fn device_family_capabilities_differ() {
    let flight = capabilities(VelocityOneDevice::Flight);
    let stick = capabilities(VelocityOneDevice::Flightstick);
    let rudder = capabilities(VelocityOneDevice::Rudder);

    assert!(stick.axes < flight.axes);
    assert!(!stick.has_leds);
    assert!(!stick.has_display);
    assert!(!stick.has_gear_lever);

    assert_eq!(rudder.axes, 3);
    assert_eq!(rudder.buttons, 0);
    assert_eq!(rudder.hats, 0);
}

#[test]
fn fallback_profiles_use_flight_baseline() {
    let flight_profile = profile_for_device(VelocityOneDevice::Flight);
    for dev in &[
        VelocityOneDevice::FlightPro,
        VelocityOneDevice::FlightUniversal,
        VelocityOneDevice::FlightYoke,
    ] {
        let profile = profile_for_device(*dev);
        assert_eq!(
            profile.name, flight_profile.name,
            "{:?} should share Flight baseline profile",
            dev
        );
    }
}

#[test]
fn profile_vendor_id_matches_turtle_beach_vid() {
    for device in VelocityOneDevice::all() {
        let profile = profile_for_device(*device);
        assert_eq!(
            profile.vendor_id, TURTLE_BEACH_VID,
            "vendor_id mismatch in profile for {:?}",
            device
        );
    }
}

#[test]
fn profile_axis_count_within_capabilities() {
    for device in [VelocityOneDevice::Flight, VelocityOneDevice::Flightstick, VelocityOneDevice::Rudder] {
        let profile = profile_for_device(device);
        let caps = capabilities(device);
        assert!(profile.axes.len() <= usize::from(caps.axes),
            "profile axis count should not exceed capabilities for {:?}: profile={} caps={}",
            device, profile.axes.len(), caps.axes);
        assert!(!profile.axes.is_empty(),
            "profile should have at least one axis for {:?}", device);
    }
}

#[test]
fn profile_axis_indices_unique() {
    for device in [
        VelocityOneDevice::Flight,
        VelocityOneDevice::Flightstick,
        VelocityOneDevice::Rudder,
    ] {
        let profile = profile_for_device(device);
        let mut seen = std::collections::HashSet::new();
        for axis in profile.axes {
            assert!(
                seen.insert(axis.index),
                "duplicate axis index {} in {:?}",
                axis.index,
                device
            );
        }
    }
}

#[test]
fn profile_axis_names_unique() {
    for device in [
        VelocityOneDevice::Flight,
        VelocityOneDevice::Flightstick,
        VelocityOneDevice::Rudder,
    ] {
        let profile = profile_for_device(device);
        let mut seen = std::collections::HashSet::new();
        for axis in profile.axes {
            assert!(
                seen.insert(axis.name),
                "duplicate axis name '{}' in {:?}",
                axis.name,
                device
            );
        }
    }
}

#[test]
fn profile_button_numbers_unique() {
    for device in [VelocityOneDevice::Flight, VelocityOneDevice::Flightstick] {
        let profile = profile_for_device(device);
        let mut seen = std::collections::HashSet::new();
        for btn in profile.buttons {
            assert!(
                seen.insert(btn.button_num),
                "duplicate button number {} in {:?}",
                btn.button_num,
                device
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. VELOCITYONE FLIGHT (YOKE) — axes, throttle, buttons, leds, display
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn yoke_pitch_full_range() {
    let fwd = parse_flight_report(&build_flight(&FlightInput { pitch: 0, ..Default::default() })).unwrap();
    let back = parse_flight_report(&build_flight(&FlightInput { pitch: 4095, ..Default::default() })).unwrap();
    assert!(fwd.pitch < -0.99, "full forward pitch should be ~-1.0, got {}", fwd.pitch);
    assert!(back.pitch > 0.99, "full back pitch should be ~1.0, got {}", back.pitch);
}

#[test]
fn yoke_roll_full_range() {
    let left = parse_flight_report(&build_flight(&FlightInput { roll: 0, ..Default::default() })).unwrap();
    let right = parse_flight_report(&build_flight(&FlightInput { roll: 4095, ..Default::default() })).unwrap();
    assert!(left.roll < -0.99, "full left roll should be ~-1.0, got {}", left.roll);
    assert!(right.roll > 0.99, "full right roll should be ~1.0, got {}", right.roll);
}

#[test]
fn yoke_center_calibration() {
    let r = parse_flight_report(&build_flight(&FlightInput::default())).unwrap();
    assert!(r.roll.abs() < 0.001, "roll center should be ~0, got {}", r.roll);
    assert!(r.pitch.abs() < 0.001, "pitch center should be ~0, got {}", r.pitch);
    assert!(r.rudder_twist.abs() < 0.001, "rudder_twist center should be ~0, got {}", r.rudder_twist);
}

#[test]
fn yoke_linearity_quarter_deflections() {
    let q1 = parse_flight_report(&build_flight(&FlightInput { roll: 1024, ..Default::default() })).unwrap();
    let q3 = parse_flight_report(&build_flight(&FlightInput { roll: 3072, ..Default::default() })).unwrap();
    // Quarter deflections should be roughly symmetric around center
    assert!((q1.roll + q3.roll).abs() < 0.01,
        "quarter deflections should be symmetric: left={}, right={}", q1.roll, q3.roll);
    // Quarter deflection should be ~0.5 magnitude
    assert!(q1.roll < -0.45 && q1.roll > -0.55,
        "25% left roll should be ~-0.5, got {}", q1.roll);
}

#[test]
fn yoke_12bit_resolution_distinguishes_adjacent_values() {
    let a = parse_flight_report(&build_flight(&FlightInput::default())).unwrap();
    let b = parse_flight_report(&build_flight(&FlightInput { roll: 2049, ..Default::default() })).unwrap();
    let diff = (b.roll - a.roll).abs();
    assert!(diff > 0.0, "adjacent 12-bit values should produce different outputs");
    // 1 LSB in 12-bit ≈ 1/2048 ≈ 0.000488
    assert!(diff < 0.002, "1 LSB delta should be small, got {}", diff);
}

#[test]
fn throttle_lever_independence() {
    let left_only = parse_flight_report(&build_flight(&FlightInput { tl: 255, ..Default::default() })).unwrap();
    let right_only = parse_flight_report(&build_flight(&FlightInput { tr: 255, ..Default::default() })).unwrap();
    assert!(left_only.throttle_left > 0.99, "left throttle full, got {}", left_only.throttle_left);
    assert!(left_only.throttle_right < 0.01, "right should be idle when only left pushed");
    assert!(right_only.throttle_right > 0.99, "right throttle full, got {}", right_only.throttle_right);
    assert!(right_only.throttle_left < 0.01, "left should be idle when only right pushed");
}

#[test]
fn throttle_idle_to_full_range() {
    let idle = parse_flight_report(&build_flight(&FlightInput::default())).unwrap();
    let full = parse_flight_report(&build_flight(&FlightInput { tl: 255, tr: 255, ..Default::default() })).unwrap();
    assert!(idle.throttle_left < 0.01, "idle left should be ~0.0");
    assert!(idle.throttle_right < 0.01, "idle right should be ~0.0");
    assert!(full.throttle_left > 0.99, "full left should be ~1.0");
    assert!(full.throttle_right > 0.99, "full right should be ~1.0");
}

#[test]
fn throttle_mid_position() {
    let mid = parse_flight_report(&build_flight(&FlightInput { tl: 128, tr: 128, ..Default::default() })).unwrap();
    assert!((mid.throttle_left - 0.502).abs() < 0.01,
        "50% throttle_left should be ~0.502, got {}", mid.throttle_left);
    assert!((mid.throttle_right - 0.502).abs() < 0.01,
        "50% throttle_right should be ~0.502, got {}", mid.throttle_right);
}

#[test]
fn flight_throttle_monotonically_increases() {
    let mut prev_left = -1.0f32;
    for raw in (0u8..=255).step_by(16) {
        let r = parse_flight_report(&build_flight(&FlightInput { tl: raw, ..Default::default() })).unwrap();
        assert!(r.throttle_left >= prev_left,
            "throttle should be monotonically increasing: raw={} gave {} < {}", raw, r.throttle_left, prev_left);
        prev_left = r.throttle_left;
    }
}

#[test]
fn flight_roll_monotonically_increases() {
    let mut prev = -2.0f32;
    for raw in (0..=4095u16).step_by(16) {
        let r = parse_flight_report(&build_flight(&FlightInput { roll: raw, ..Default::default() })).unwrap();
        assert!(
            r.roll >= prev,
            "roll not monotonic: raw={raw}, prev={prev}, cur={}",
            r.roll
        );
        prev = r.roll;
    }
}

#[test]
fn flight_report_minimum_axis_values() {
    let data = build_flight(&FlightInput { roll: 0, pitch: 0, rudder: 0, tl: 0, tr: 0, trim: 0, ..Default::default() });
    let r = parse_flight_report(&data).unwrap();
    assert!(r.roll < -0.99);
    assert!(r.pitch < -0.99);
    assert!(r.rudder_twist < -0.99);
    assert!(r.trim_wheel < -0.99);
    assert!(r.throttle_left.abs() < 0.001);
    assert!(r.throttle_right.abs() < 0.001);
}

#[test]
fn flight_report_maximum_axis_values() {
    let data = build_flight(&FlightInput { roll: 4095, pitch: 4095, rudder: 4095, tl: 255, tr: 255, trim: 4095, ..Default::default() });
    let r = parse_flight_report(&data).unwrap();
    assert!(r.roll > 0.99);
    assert!(r.pitch > 0.99);
    assert!(r.rudder_twist > 0.99);
    assert!(r.trim_wheel > 0.99);
    assert!(r.throttle_left > 0.99);
    assert!(r.throttle_right > 0.99);
}

#[test]
fn flight_report_oversized_12bit_value_clamped() {
    let data = build_flight(&FlightInput { roll: 8000, ..Default::default() });
    let r = parse_flight_report(&data).unwrap();
    assert!(r.roll <= 1.0);
}

#[test]
fn flight_report_extra_bytes_ignored() {
    let mut data = [0u8; 32];
    let base = build_flight(&FlightInput { tl: 128, tr: 128, ..Default::default() });
    data[..20].copy_from_slice(&base);
    data[20..].fill(0xFF);
    let r = parse_flight_report(&data).unwrap();
    assert!(r.roll.abs() < 0.01);
    assert!(r.throttle_left > 0.49 && r.throttle_left < 0.51);
}

#[test]
fn flight_report_exactly_min_length() {
    let data = [0u8; FLIGHT_MIN_REPORT_BYTES];
    assert!(parse_flight_report(&data).is_ok());
}

#[test]
fn flight_report_one_below_min_length() {
    let data = [0u8; FLIGHT_MIN_REPORT_BYTES - 1];
    let err = parse_flight_report(&data).unwrap_err();
    assert_eq!(
        err,
        flight_hotas_turtlebeach::velocityone::TurtleBeachError::TooShort {
            expected: FLIGHT_MIN_REPORT_BYTES,
            actual: FLIGHT_MIN_REPORT_BYTES - 1,
        }
    );
}

#[test]
fn flight_report_all_buttons_set() {
    let data = build_flight(&FlightInput { buttons: u64::MAX, ..Default::default() });
    let r = parse_flight_report(&data).unwrap();
    assert_eq!(r.buttons, u64::MAX);
}

#[test]
fn flight_report_single_button_bits() {
    for bit in 0..64u32 {
        let mask: u64 = 1 << bit;
        let data = build_flight(&FlightInput { buttons: mask, ..Default::default() });
        let r = parse_flight_report(&data).unwrap();
        assert_eq!(r.buttons, mask, "button bit {bit} not preserved");
    }
}

#[test]
fn flight_report_hat_all_directions() {
    let expected_dirs: &[(u8, u8)] = &[
        (0, 1),  // N
        (1, 2),  // NE
        (2, 3),  // E
        (3, 4),  // SE
        (4, 5),  // S
        (5, 6),  // SW
        (6, 7),  // W
        (7, 8),  // NW
        (8, 0),  // centred
        (15, 0), // centred (any ≥8)
    ];
    for &(raw_hat, expected) in expected_dirs {
        let data = build_flight(&FlightInput { hat: raw_hat, ..Default::default() });
        let r = parse_flight_report(&data).unwrap();
        assert_eq!(r.hat, expected, "hat raw={raw_hat} expected={expected}");
    }
}

#[test]
fn hat_switch_directions_flight() {
    for raw_hat in 0u8..=7 {
        let r = parse_flight_report(&build_flight(&FlightInput { hat: raw_hat, ..Default::default() })).unwrap();
        assert_eq!(r.hat, raw_hat + 1, "raw hat {} should map to {}", raw_hat, raw_hat + 1);
    }
    let r = parse_flight_report(&build_flight(&FlightInput::default())).unwrap();
    assert_eq!(r.hat, 0, "hat ≥8 should map to 0 (centered)");
}

#[test]
fn toggle_switches_individual_and_batch() {
    let all_on = decode_all_toggles(0x7F);
    for (i, t) in all_on.iter().enumerate() {
        assert_eq!(*t, ToggleSwitchPosition::On, "switch {} should be on", i + 1);
    }
    let alt = decode_all_toggles(0b0101_0101);
    assert_eq!(alt[0], ToggleSwitchPosition::On);
    assert_eq!(alt[1], ToggleSwitchPosition::Off);
    assert_eq!(alt[2], ToggleSwitchPosition::On);
    assert_eq!(alt[3], ToggleSwitchPosition::Off);
}

#[test]
fn toggle_switches_round_trip_through_report() {
    for mask in 0..=0x7Fu8 {
        let data = build_flight(&FlightInput { toggles: mask, ..Default::default() });
        let r = parse_flight_report(&data).unwrap();
        let decoded = decode_all_toggles(r.toggle_switches);
        for sw in 1..=7u8 {
            let expected = if (mask >> (sw - 1)) & 1 != 0 {
                ToggleSwitchPosition::On
            } else {
                ToggleSwitchPosition::Off
            };
            assert_eq!(
                decoded[(sw - 1) as usize],
                expected,
                "switch {sw} mismatch for mask 0x{mask:02X}"
            );
        }
    }
}

#[test]
fn toggle_high_bit_masked_in_report() {
    let data = build_flight(&FlightInput { toggles: 0xFF, ..Default::default() });
    let r = parse_flight_report(&data).unwrap();
    assert_eq!(r.toggle_switches, 0x7F, "bit 7 should be masked off");
}

#[test]
fn decode_toggle_switch_boundary_values() {
    assert_eq!(decode_toggle_switch(0xFF, 0), ToggleSwitchPosition::Off);
    assert_eq!(decode_toggle_switch(0xFF, 1), ToggleSwitchPosition::On);
    assert_eq!(decode_toggle_switch(0xFF, 7), ToggleSwitchPosition::On);
    assert_eq!(decode_toggle_switch(0xFF, 8), ToggleSwitchPosition::Off);
    assert_eq!(decode_toggle_switch(0xFF, 255), ToggleSwitchPosition::Off);
}

#[test]
fn gear_switch_up_down_transit() {
    let gear_down_mask: u64 = 1 << 31;
    let gear_up_mask: u64 = 1 << 30;

    assert_eq!(GearLedState::from_button_mask(gear_down_mask), GearLedState::Down);
    assert_eq!(GearLedState::from_button_mask(gear_up_mask), GearLedState::Up);
    assert_eq!(GearLedState::from_button_mask(0), GearLedState::Transit);
    assert_eq!(GearLedState::from_button_mask(gear_down_mask | gear_up_mask), GearLedState::Down);
}

#[test]
fn gear_state_from_flight_report_down() {
    let mask: u64 = 1 << 31;
    let data = build_flight(&FlightInput { buttons: mask, ..Default::default() });
    let r = parse_flight_report(&data).unwrap();
    let gear = GearLedState::from_button_mask(r.buttons);
    assert_eq!(gear, GearLedState::Down);
}

#[test]
fn gear_state_from_flight_report_up() {
    let mask: u64 = 1 << 30;
    let data = build_flight(&FlightInput { buttons: mask, ..Default::default() });
    let r = parse_flight_report(&data).unwrap();
    let gear = GearLedState::from_button_mask(r.buttons);
    assert_eq!(gear, GearLedState::Up);
}

#[test]
fn gear_led_serialization_roundtrip() {
    let mut leds = FlightLedState::all_off();
    leds.set_from_gear_state(GearLedState::Down);
    let report = serialize_gear_led_report(&leds);
    assert_eq!(report[1] & 0b0001_0101, 0b0001_0101);
    assert_eq!(report[1] & 0b0010_1010, 0);
}

#[test]
fn gear_led_end_to_end_down_and_locked() {
    let mask: u64 = 1 << 31;
    let gear = GearLedState::from_button_mask(mask);
    let mut leds = FlightLedState::all_off();
    leds.set_from_gear_state(gear);
    let report = serialize_gear_led_report(&leds);
    // Green LEDs on (bits 0,2,4), Red off
    assert_eq!(
        report[1] & 0b0001_0101,
        0b0001_0101,
        "green LEDs should be on"
    );
    assert_eq!(report[1] & 0b0010_1010, 0, "red LEDs should be off");
}

#[test]
fn gear_led_end_to_end_transit() {
    let gear = GearLedState::from_button_mask(0);
    let mut leds = FlightLedState::all_off();
    leds.set_from_gear_state(gear);
    let report = serialize_gear_led_report(&leds);
    assert_eq!(report[1] & 0b0001_0101, 0, "green LEDs should be off");
    assert_eq!(
        report[1] & 0b0010_1010,
        0b0010_1010,
        "red LEDs should be on"
    );
}

#[test]
fn gear_led_report_id_always_zero() {
    for state in &[GearLedState::Up, GearLedState::Down, GearLedState::Transit] {
        let mut leds = FlightLedState::all_off();
        leds.set_from_gear_state(*state);
        let report = serialize_gear_led_report(&leds);
        assert_eq!(report[0], 0x00, "report ID should always be 0x00");
        assert_eq!(report[2], 0x00, "reserved byte 2 should be 0");
        assert_eq!(report[3], 0x00, "reserved byte 3 should be 0");
    }
}

#[test]
fn gear_led_state_display_values() {
    assert_eq!(GearLedState::Up.to_string(), "UP");
    assert_eq!(GearLedState::Down.to_string(), "DOWN");
    assert_eq!(GearLedState::Transit.to_string(), "TRANSIT");
}

#[test]
fn flight_led_state_all_off_is_default() {
    assert_eq!(FlightLedState::all_off(), FlightLedState::default());
}

#[test]
fn flight_led_state_all_on_all_fields_true() {
    let leds = FlightLedState::all_on();
    assert!(leds.gear_nose_green);
    assert!(leds.gear_nose_red);
    assert!(leds.gear_left_green);
    assert!(leds.gear_left_red);
    assert!(leds.gear_right_green);
    assert!(leds.gear_right_red);
}

#[test]
fn flight_led_state_set_from_gear_state_overwrites_all() {
    let mut leds = FlightLedState::all_on();
    leds.set_from_gear_state(GearLedState::Up);
    // Up = all LEDs off
    assert!(!leds.gear_nose_green);
    assert!(!leds.gear_nose_red);
    assert!(!leds.gear_left_green);
    assert!(!leds.gear_left_red);
    assert!(!leds.gear_right_green);
    assert!(!leds.gear_right_red);
}

#[test]
fn display_command_all_pages() {
    let pages = [
        (DisplayPage::Nav, 0u8),
        (DisplayPage::Engine, 1),
        (DisplayPage::Systems, 2),
        (DisplayPage::Custom, 3),
    ];
    for (page, expected_byte) in pages {
        let cmd = DisplayCommand { page, brightness: 128 };
        let report = serialize_display_command(&cmd);
        assert_eq!(report[0], 0x02, "report ID");
        assert_eq!(report[1], 0x01, "command type");
        assert_eq!(report[2], expected_byte, "page byte for {page:?}");
        assert_eq!(report[3], 128, "brightness");
        assert_eq!(&report[4..], &[0, 0, 0, 0], "reserved bytes");
    }
}

#[test]
fn display_command_brightness_extremes() {
    let cmd_off = DisplayCommand {
        page: DisplayPage::Nav,
        brightness: 0,
    };
    let cmd_max = DisplayCommand {
        page: DisplayPage::Nav,
        brightness: 255,
    };
    assert_eq!(serialize_display_command(&cmd_off)[3], 0);
    assert_eq!(serialize_display_command(&cmd_max)[3], 255);
}

#[test]
fn flight_report_too_short_errors() {
    for len in [0, 1, 10, 19] {
        let data = vec![0u8; len];
        let err = parse_flight_report(&data).unwrap_err();
        match err {
            flight_hotas_turtlebeach::velocityone::TurtleBeachError::TooShort { expected, actual } => {
                assert_eq!(expected, FLIGHT_MIN_REPORT_BYTES);
                assert_eq!(actual, len);
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. VELOCITYONE FLIGHTSTICK — axes, buttons, hat
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn flightstick_buttons_16bit_mask() {
    let r = parse_flightstick_report(&make_flightstick(2048, 2048, 2048, 0, 0xFFFF, 15)).unwrap();
    assert_eq!(r.buttons, 0xFFFF, "all 16 buttons should be set");

    let r2 = parse_flightstick_report(&make_flightstick(2048, 2048, 2048, 0, 0x0001, 15)).unwrap();
    assert_eq!(r2.buttons & 1, 1, "trigger button (bit 0) should be set");
    assert_eq!(r2.buttons >> 1, 0, "no other buttons should be set");
}

#[test]
fn hat_switch_directions_flightstick() {
    for raw_hat in 0u8..=7 {
        let r = parse_flightstick_report(&make_flightstick(2048, 2048, 2048, 0, 0, raw_hat)).unwrap();
        assert_eq!(r.hat, raw_hat + 1, "raw hat {} should map to {}", raw_hat, raw_hat + 1);
    }
    let r = parse_flightstick_report(&make_flightstick(2048, 2048, 2048, 0, 0, 15)).unwrap();
    assert_eq!(r.hat, 0, "hat ≥8 should map to 0 (centered)");
}

#[test]
fn flightstick_report_quarter_deflection() {
    // ~25% travel: raw 1024 on a 0–4095 range = about -0.5
    let data = make_flightstick(1024, 3072, 2048, 64, 0, 15);
    let r = parse_flightstick_report(&data).unwrap();
    assert!(
        r.x < -0.4 && r.x > -0.6,
        "quarter left x should be ~-0.5, got {}",
        r.x
    );
    assert!(
        r.y > 0.4 && r.y < 0.6,
        "quarter back y should be ~0.5, got {}",
        r.y
    );
    assert!(r.twist.abs() < 0.01, "twist at centre should be ~0");
    assert!(
        r.throttle > 0.24 && r.throttle < 0.26,
        "~25% throttle, got {}",
        r.throttle
    );
}

#[test]
fn flightstick_report_all_buttons_u16_max() {
    let data = make_flightstick(2048, 2048, 2048, 0, 0xFFFF, 15);
    let r = parse_flightstick_report(&data).unwrap();
    assert_eq!(r.buttons, 0xFFFF);
}

#[test]
fn flightstick_hat_all_directions() {
    let expected: &[(u8, u8)] = &[
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 4),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 8),
        (8, 0),
        (9, 0),
        (15, 0),
    ];
    for &(raw, exp) in expected {
        let data = make_flightstick(2048, 2048, 2048, 0, 0, raw);
        let r = parse_flightstick_report(&data).unwrap();
        assert_eq!(r.hat, exp, "flightstick hat raw={raw}");
    }
}

#[test]
fn flightstick_throttle_monotonically_increases() {
    let mut prev = -1.0f32;
    for raw in 0..=255u8 {
        let data = make_flightstick(2048, 2048, 2048, raw, 0, 15);
        let r = parse_flightstick_report(&data).unwrap();
        assert!(
            r.throttle >= prev,
            "throttle not monotonic: raw={raw}, prev={prev}, cur={}",
            r.throttle
        );
        prev = r.throttle;
    }
}

#[test]
fn flightstick_report_exactly_min_length() {
    let data = [0u8; FLIGHTSTICK_MIN_REPORT_BYTES];
    assert!(parse_flightstick_report(&data).is_ok());
}

#[test]
fn flightstick_report_too_short_errors() {
    for len in [0, 1, 6, 11] {
        let data = vec![0u8; len];
        let err = parse_flightstick_report(&data).unwrap_err();
        match err {
            flight_hotas_turtlebeach::velocityone::TurtleBeachError::TooShort { expected, actual } => {
                assert_eq!(expected, FLIGHTSTICK_MIN_REPORT_BYTES);
                assert_eq!(actual, len);
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. LEGACY DEVICES (VID 0x1432) — Flightdeck and Rudder
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn throttle_legacy_flightdeck_lever_independence() {
    let left = parse_flightdeck_report(&make_flightdeck(32767, 32767, 255, 0, 0)).unwrap();
    let right = parse_flightdeck_report(&make_flightdeck(32767, 32767, 0, 255, 0)).unwrap();
    assert!(left.throttle_left > 0.99);
    assert!(left.throttle_right < 0.01);
    assert!(right.throttle_right > 0.99);
    assert!(right.throttle_left < 0.01);
}

#[test]
fn flightdeck_report_center_and_extremes() {
    let centre = make_flightdeck(32767, 32767, 128, 128, 0);
    let r = parse_flightdeck_report(&centre).unwrap();
    assert!(r.roll.abs() < 0.01, "flightdeck roll centre");
    assert!(r.pitch.abs() < 0.01, "flightdeck pitch centre");
    assert!(
        r.throttle_left > 0.49 && r.throttle_left < 0.51,
        "flightdeck tl ~50%"
    );
    assert!(
        r.throttle_right > 0.49 && r.throttle_right < 0.51,
        "flightdeck tr ~50%"
    );

    let max = make_flightdeck(65535, 65535, 255, 255, 0);
    let r = parse_flightdeck_report(&max).unwrap();
    assert!(r.roll > 0.99, "flightdeck roll max");
    assert!(r.pitch > 0.99, "flightdeck pitch max");
    assert!(r.throttle_left > 0.99, "flightdeck tl max");
    assert!(r.throttle_right > 0.99, "flightdeck tr max");
}

#[test]
fn flightdeck_report_buttons_preserved() {
    let data = make_flightdeck(32767, 32767, 0, 0, 0xDEAD_BEEF);
    let r = parse_flightdeck_report(&data).unwrap();
    assert_eq!(r.buttons, 0xDEAD_BEEF);
}

#[test]
fn flightdeck_report_too_short_error_message() {
    let err = parse_flightdeck_report(&[0u8; 10]).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("too short"), "error message: {msg}");
    assert!(msg.contains("16"), "should mention expected=16: {msg}");
    assert!(msg.contains("10"), "should mention actual=10: {msg}");
}

#[test]
fn rudder_center_calibration() {
    let r = parse_rudder_report(&make_rudder(32767, 0, 0)).unwrap();
    assert!(r.rudder.abs() < 0.001, "rudder center should be ~0, got {}", r.rudder);
    assert!(r.brake_left < 0.01, "brake_left at rest should be ~0");
    assert!(r.brake_right < 0.01, "brake_right at rest should be ~0");
}

#[test]
fn rudder_full_deflection_left_right() {
    let left = parse_rudder_report(&make_rudder(0, 0, 0)).unwrap();
    let right = parse_rudder_report(&make_rudder(65535, 0, 0)).unwrap();
    assert!(left.rudder < -0.99, "full left rudder should be ~-1.0, got {}", left.rudder);
    assert!(right.rudder > 0.99, "full right rudder should be ~1.0, got {}", right.rudder);
}

#[test]
fn rudder_report_full_deflections() {
    let full_left = make_rudder(0, 0, 0);
    let r = parse_rudder_report(&full_left).unwrap();
    assert!(r.rudder < -0.99, "rudder full left");
    assert!(r.brake_left.abs() < 0.01, "brake_left released");
    assert!(r.brake_right.abs() < 0.01, "brake_right released");

    let full_right_brakes = make_rudder(65535, 255, 255);
    let r = parse_rudder_report(&full_right_brakes).unwrap();
    assert!(r.rudder > 0.99, "rudder full right");
    assert!(r.brake_left > 0.99, "brake_left full");
    assert!(r.brake_right > 0.99, "brake_right full");
}

#[test]
fn rudder_differential_toe_brakes() {
    let left_brake = parse_rudder_report(&make_rudder(32767, 255, 0)).unwrap();
    let right_brake = parse_rudder_report(&make_rudder(32767, 0, 255)).unwrap();
    assert!(left_brake.brake_left > 0.99, "full left brake should be ~1.0");
    assert!(left_brake.brake_right < 0.01, "right brake should be off");
    assert!(right_brake.brake_right > 0.99, "full right brake should be ~1.0");
    assert!(right_brake.brake_left < 0.01, "left brake should be off");
}

#[test]
fn rudder_small_inputs_near_center() {
    let slight = parse_rudder_report(&make_rudder(32770, 2, 2)).unwrap();
    assert!(slight.rudder.abs() < 0.01, "tiny rudder deflection should be near 0, got {}", slight.rudder);
    assert!(slight.brake_left < 0.01, "tiny brake_left should be near 0, got {}", slight.brake_left);
    assert!(slight.brake_right < 0.01, "tiny brake_right should be near 0, got {}", slight.brake_right);
}

#[test]
fn rudder_twist_on_yoke_full_range() {
    let left = parse_flight_report(&build_flight(&FlightInput { rudder: 0, ..Default::default() })).unwrap();
    let right = parse_flight_report(&build_flight(&FlightInput { rudder: 4095, ..Default::default() })).unwrap();
    assert!(left.rudder_twist < -0.99, "full left rudder twist should be ~-1.0");
    assert!(right.rudder_twist > 0.99, "full right rudder twist should be ~1.0");
}

#[test]
fn rudder_report_too_short_variants() {
    assert!(parse_rudder_report(&[]).is_err());
    assert!(parse_rudder_report(&[0u8; 1]).is_err());
    assert!(parse_rudder_report(&[0u8; RUDDER_MIN_REPORT_BYTES - 1]).is_err());
    assert!(parse_rudder_report(&[0u8; RUDDER_MIN_REPORT_BYTES]).is_ok());
}

// ═══════════════════════════════════════════════════════════════════════════════
// 6. UTILITIES — Trim Wheel Tracker, Errors
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn trim_wheel_delta_tracking() {
    let mut tracker = TrimWheelTracker::new();
    assert_eq!(tracker.update(2048), 0);
    assert_eq!(tracker.update(2058), 10);
    assert_eq!(tracker.update(2038), -20);
    tracker.reset();
    assert_eq!(tracker.update(1000), 0);
}

#[test]
fn trim_wheel_tracker_accumulates_deltas() {
    let mut t = TrimWheelTracker::new();
    assert_eq!(t.update(2048), 0);
    assert_eq!(t.update(2058), 10);
    assert_eq!(t.update(2068), 10);
    assert_eq!(t.update(2048), -20);
}

#[test]
fn trim_wheel_tracker_handles_extremes() {
    let mut t = TrimWheelTracker::new();
    t.update(0);
    assert_eq!(t.update(4095), 4095);
    assert_eq!(t.update(0), -4095);
}

#[test]
fn trim_wheel_tracker_reset_reestablishes_baseline() {
    let mut t = TrimWheelTracker::new();
    t.update(1000);
    t.update(2000);
    t.reset();
    // After reset, next update returns 0 (baseline)
    assert_eq!(t.update(3000), 0);
    assert_eq!(t.update(3010), 10);
}

#[test]
fn trim_wheel_with_flight_report_integration() {
    let mut tracker = TrimWheelTracker::new();

    let data1 = build_flight(&FlightInput { trim: 2048, ..Default::default() });
    let _r1 = parse_flight_report(&data1).unwrap();
    // Derive raw trim from report bytes (LE u16 at offset 8)
    let raw_trim1 = u16::from_le_bytes([data1[8], data1[9]]);
    let delta1 = tracker.update(raw_trim1);
    assert_eq!(delta1, 0, "first update should return 0");

    // Simulate trim nose-up movement via a second report
    let data2 = build_flight(&FlightInput { trim: 2148, ..Default::default() });
    let _r2 = parse_flight_report(&data2).unwrap();
    let raw_trim2 = u16::from_le_bytes([data2[8], data2[9]]);
    let delta2 = tracker.update(raw_trim2);
    assert_eq!(delta2, 100, "delta should be +100");
}

#[test]
fn error_type_is_clone_eq_debug() {
    let err = flight_hotas_turtlebeach::velocityone::TurtleBeachError::TooShort {
        expected: 20,
        actual: 5,
    };
    let cloned = err.clone();
    assert_eq!(err, cloned);
    let dbg = format!("{err:?}");
    assert!(dbg.contains("TooShort"));
}

#[test]
fn error_display_contains_sizes() {
    let err = flight_hotas_turtlebeach::velocityone::TurtleBeachError::TooShort {
        expected: 16,
        actual: 3,
    };
    let msg = err.to_string();
    assert!(msg.contains("16"), "should contain expected: {msg}");
    assert!(msg.contains("3"), "should contain actual: {msg}");
}
