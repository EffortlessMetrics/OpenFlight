// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the `flight-hotas-turtlebeach` crate.
//!
//! These integration tests exercise cross-module interactions, edge cases,
//! and invariants that go beyond the per-module unit tests.

use flight_hotas_turtlebeach::*;

// ── Helpers ──────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn make_flight_report_bytes(
    roll: u16,
    pitch: u16,
    rudder: u16,
    tl: u8,
    tr: u8,
    trim: u16,
    buttons: u64,
    hat: u8,
    toggles: u8,
) -> [u8; 20] {
    let mut b = [0u8; 20];
    b[0..2].copy_from_slice(&roll.to_le_bytes());
    b[2..4].copy_from_slice(&pitch.to_le_bytes());
    b[4..6].copy_from_slice(&rudder.to_le_bytes());
    b[6] = tl;
    b[7] = tr;
    b[8..10].copy_from_slice(&trim.to_le_bytes());
    b[10..18].copy_from_slice(&buttons.to_le_bytes());
    b[18] = hat;
    b[19] = toggles;
    b
}

fn make_flightstick_report_bytes(
    x: u16,
    y: u16,
    twist: u16,
    throttle: u8,
    buttons: u16,
    hat: u8,
) -> [u8; 12] {
    let mut b = [0u8; 12];
    b[0..2].copy_from_slice(&x.to_le_bytes());
    b[2..4].copy_from_slice(&y.to_le_bytes());
    b[4..6].copy_from_slice(&twist.to_le_bytes());
    b[6] = throttle;
    b[7..9].copy_from_slice(&buttons.to_le_bytes());
    b[9] = hat;
    b
}

fn make_flightdeck_bytes(roll: u16, pitch: u16, tl: u8, tr: u8, buttons: u32) -> [u8; 16] {
    let mut b = [0u8; 16];
    b[0..2].copy_from_slice(&roll.to_le_bytes());
    b[2..4].copy_from_slice(&pitch.to_le_bytes());
    b[4] = tl;
    b[5] = tr;
    b[6..10].copy_from_slice(&buttons.to_le_bytes());
    b
}

fn make_rudder_bytes(rudder: u16, bl: u8, br: u8) -> [u8; 8] {
    let mut b = [0u8; 8];
    b[0..2].copy_from_slice(&rudder.to_le_bytes());
    b[2] = bl;
    b[3] = br;
    b
}

// ═══════════════════════════════════════════════════════════════════════════════
// 1. Device identification cross-checks
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
fn device_names_contain_turtle_beach_and_velocityone() {
    for device in VelocityOneDevice::all() {
        let name = device.name();
        assert!(name.contains("Turtle Beach"), "missing 'Turtle Beach' in {name}");
        assert!(name.contains("VelocityOne"), "missing 'VelocityOne' in {name}");
    }
}

#[test]
fn device_display_matches_name() {
    for device in VelocityOneDevice::all() {
        assert_eq!(format!("{device}"), device.name());
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. Capabilities consistency with profiles
// ═══════════════════════════════════════════════════════════════════════════════

/// The three primary devices (Flight, Flightstick, Rudder) have dedicated profiles.
/// FlightPro, FlightUniversal, and FlightYoke intentionally share the Flight
/// profile as a baseline, so caps vs profile checks only apply to the primaries.
const PRIMARY_DEVICES: &[VelocityOneDevice] = &[
    VelocityOneDevice::Flight,
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

// ═══════════════════════════════════════════════════════════════════════════════
// 3. VelocityOne Flight report — axis boundary values
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn flight_report_minimum_axis_values() {
    let data = make_flight_report_bytes(0, 0, 0, 0, 0, 0, 0, 15, 0);
    let r = parse_flight_report(&data).unwrap();
    assert!(r.roll < -0.99, "roll min should be near -1.0, got {}", r.roll);
    assert!(r.pitch < -0.99, "pitch min should be near -1.0, got {}", r.pitch);
    assert!(r.rudder_twist < -0.99, "rudder min should be near -1.0, got {}", r.rudder_twist);
    assert!(r.trim_wheel < -0.99, "trim min should be near -1.0, got {}", r.trim_wheel);
    assert!(r.throttle_left.abs() < 0.001, "throttle_left at 0 should be ~0");
    assert!(r.throttle_right.abs() < 0.001, "throttle_right at 0 should be ~0");
}

#[test]
fn flight_report_maximum_axis_values() {
    let data = make_flight_report_bytes(4095, 4095, 4095, 255, 255, 4095, 0, 15, 0);
    let r = parse_flight_report(&data).unwrap();
    assert!(r.roll > 0.99, "roll max should be near 1.0, got {}", r.roll);
    assert!(r.pitch > 0.99, "pitch max should be near 1.0, got {}", r.pitch);
    assert!(r.rudder_twist > 0.99, "rudder max should be near 1.0, got {}", r.rudder_twist);
    assert!(r.trim_wheel > 0.99, "trim max should be near 1.0, got {}", r.trim_wheel);
    assert!(r.throttle_left > 0.99, "throttle_left at 255 should be near 1.0");
    assert!(r.throttle_right > 0.99, "throttle_right at 255 should be near 1.0");
}

#[test]
fn flight_report_oversized_12bit_value_clamped() {
    // Raw u16 above 4095 — normalization should clamp
    let data = make_flight_report_bytes(8000, 2048, 2048, 0, 0, 2048, 0, 15, 0);
    let r = parse_flight_report(&data).unwrap();
    assert!(
        r.roll <= 1.0,
        "roll with raw > 4095 must not exceed 1.0, got {}",
        r.roll
    );
}

#[test]
fn flight_report_extra_bytes_ignored() {
    let mut data = [0u8; 32];
    let base = make_flight_report_bytes(2048, 2048, 2048, 128, 128, 2048, 0, 15, 0);
    data[..20].copy_from_slice(&base);
    data[20..].fill(0xFF); // junk padding
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

// ═══════════════════════════════════════════════════════════════════════════════
// 4. VelocityOne Flight — button mask and hat interactions
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn flight_report_all_buttons_set() {
    let data = make_flight_report_bytes(2048, 2048, 2048, 0, 0, 2048, u64::MAX, 15, 0);
    let r = parse_flight_report(&data).unwrap();
    assert_eq!(r.buttons, u64::MAX);
}

#[test]
fn flight_report_single_button_bits() {
    for bit in 0..64u32 {
        let mask: u64 = 1 << bit;
        let data = make_flight_report_bytes(2048, 2048, 2048, 0, 0, 2048, mask, 15, 0);
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
        let data = make_flight_report_bytes(2048, 2048, 2048, 0, 0, 2048, 0, raw_hat, 0);
        let r = parse_flight_report(&data).unwrap();
        assert_eq!(r.hat, expected, "hat raw={raw_hat} expected={expected}");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. Toggle switch cross-module verification
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn toggle_switches_round_trip_through_report() {
    for mask in 0..=0x7Fu8 {
        let data = make_flight_report_bytes(2048, 2048, 2048, 0, 0, 2048, 0, 15, mask);
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
    // Bit 7 is outside the 7-switch range; the report masks with 0x7F
    let data = make_flight_report_bytes(2048, 2048, 2048, 0, 0, 2048, 0, 15, 0xFF);
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

// ═══════════════════════════════════════════════════════════════════════════════
// 6. Gear LED state — from button mask through to serialized report
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn gear_state_from_flight_report_down() {
    let mask: u64 = 1 << 31;
    let data = make_flight_report_bytes(2048, 2048, 2048, 0, 0, 2048, mask, 15, 0);
    let r = parse_flight_report(&data).unwrap();
    let gear = GearLedState::from_button_mask(r.buttons);
    assert_eq!(gear, GearLedState::Down);
}

#[test]
fn gear_state_from_flight_report_up() {
    let mask: u64 = 1 << 30;
    let data = make_flight_report_bytes(2048, 2048, 2048, 0, 0, 2048, mask, 15, 0);
    let r = parse_flight_report(&data).unwrap();
    let gear = GearLedState::from_button_mask(r.buttons);
    assert_eq!(gear, GearLedState::Up);
}

#[test]
fn gear_led_end_to_end_down_and_locked() {
    let mask: u64 = 1 << 31;
    let gear = GearLedState::from_button_mask(mask);
    let mut leds = FlightLedState::all_off();
    leds.set_from_gear_state(gear);
    let report = serialize_gear_led_report(&leds);
    // Green LEDs on (bits 0,2,4), Red off
    assert_eq!(report[1] & 0b0001_0101, 0b0001_0101, "green LEDs should be on");
    assert_eq!(report[1] & 0b0010_1010, 0, "red LEDs should be off");
}

#[test]
fn gear_led_end_to_end_transit() {
    let gear = GearLedState::from_button_mask(0);
    let mut leds = FlightLedState::all_off();
    leds.set_from_gear_state(gear);
    let report = serialize_gear_led_report(&leds);
    assert_eq!(report[1] & 0b0001_0101, 0, "green LEDs should be off");
    assert_eq!(report[1] & 0b0010_1010, 0b0010_1010, "red LEDs should be on");
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

// ═══════════════════════════════════════════════════════════════════════════════
// 7. Display command serialization
// ═══════════════════════════════════════════════════════════════════════════════

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
    let cmd_off = DisplayCommand { page: DisplayPage::Nav, brightness: 0 };
    let cmd_max = DisplayCommand { page: DisplayPage::Nav, brightness: 255 };
    assert_eq!(serialize_display_command(&cmd_off)[3], 0);
    assert_eq!(serialize_display_command(&cmd_max)[3], 255);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 8. VelocityOne Flightstick report edge cases
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn flightstick_report_quarter_deflection() {
    // ~25% travel: raw 1024 on a 0–4095 range = about -0.5
    let data = make_flightstick_report_bytes(1024, 3072, 2048, 64, 0, 15);
    let r = parse_flightstick_report(&data).unwrap();
    assert!(r.x < -0.4 && r.x > -0.6, "quarter left x should be ~-0.5, got {}", r.x);
    assert!(r.y > 0.4 && r.y < 0.6, "quarter back y should be ~0.5, got {}", r.y);
    assert!(r.twist.abs() < 0.01, "twist at centre should be ~0");
    assert!(r.throttle > 0.24 && r.throttle < 0.26, "~25% throttle, got {}", r.throttle);
}

#[test]
fn flightstick_report_all_buttons_u16_max() {
    let data = make_flightstick_report_bytes(2048, 2048, 2048, 0, 0xFFFF, 15);
    let r = parse_flightstick_report(&data).unwrap();
    assert_eq!(r.buttons, 0xFFFF);
}

#[test]
fn flightstick_hat_all_directions() {
    let expected: &[(u8, u8)] = &[
        (0, 1), (1, 2), (2, 3), (3, 4),
        (4, 5), (5, 6), (6, 7), (7, 8),
        (8, 0), (9, 0), (15, 0),
    ];
    for &(raw, exp) in expected {
        let data = make_flightstick_report_bytes(2048, 2048, 2048, 0, 0, raw);
        let r = parse_flightstick_report(&data).unwrap();
        assert_eq!(r.hat, exp, "flightstick hat raw={raw}");
    }
}

#[test]
fn flightstick_report_exactly_min_length() {
    let data = [0u8; FLIGHTSTICK_MIN_REPORT_BYTES];
    assert!(parse_flightstick_report(&data).is_ok());
}

// ═══════════════════════════════════════════════════════════════════════════════
// 9. Legacy VelocityOne (VID 0x1432) — Flightdeck and Rudder parsing
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn flightdeck_report_center_and_extremes() {
    let centre = make_flightdeck_bytes(32767, 32767, 128, 128, 0);
    let r = parse_flightdeck_report(&centre).unwrap();
    assert!(r.roll.abs() < 0.01, "flightdeck roll centre");
    assert!(r.pitch.abs() < 0.01, "flightdeck pitch centre");
    assert!(r.throttle_left > 0.49 && r.throttle_left < 0.51, "flightdeck tl ~50%");
    assert!(r.throttle_right > 0.49 && r.throttle_right < 0.51, "flightdeck tr ~50%");

    let max = make_flightdeck_bytes(65535, 65535, 255, 255, 0);
    let r = parse_flightdeck_report(&max).unwrap();
    assert!(r.roll > 0.99, "flightdeck roll max");
    assert!(r.pitch > 0.99, "flightdeck pitch max");
    assert!(r.throttle_left > 0.99, "flightdeck tl max");
    assert!(r.throttle_right > 0.99, "flightdeck tr max");
}

#[test]
fn flightdeck_report_buttons_preserved() {
    let data = make_flightdeck_bytes(32767, 32767, 0, 0, 0xDEAD_BEEF);
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
fn rudder_report_full_deflections() {
    let full_left = make_rudder_bytes(0, 0, 0);
    let r = parse_rudder_report(&full_left).unwrap();
    assert!(r.rudder < -0.99, "rudder full left");
    assert!(r.brake_left.abs() < 0.01, "brake_left released");
    assert!(r.brake_right.abs() < 0.01, "brake_right released");

    let full_right_brakes = make_rudder_bytes(65535, 255, 255);
    let r = parse_rudder_report(&full_right_brakes).unwrap();
    assert!(r.rudder > 0.99, "rudder full right");
    assert!(r.brake_left > 0.99, "brake_left full");
    assert!(r.brake_right > 0.99, "brake_right full");
}

#[test]
fn rudder_report_too_short_variants() {
    assert!(parse_rudder_report(&[]).is_err());
    assert!(parse_rudder_report(&[0u8; 1]).is_err());
    assert!(parse_rudder_report(&[0u8; RUDDER_MIN_REPORT_BYTES - 1]).is_err());
    assert!(parse_rudder_report(&[0u8; RUDDER_MIN_REPORT_BYTES]).is_ok());
}

// ═══════════════════════════════════════════════════════════════════════════════
// 10. Trim wheel tracker — stateful sequences
// ═══════════════════════════════════════════════════════════════════════════════

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

    let data1 = make_flight_report_bytes(2048, 2048, 2048, 0, 0, 2048, 0, 15, 0);
    let r1 = parse_flight_report(&data1).unwrap();
    // First update: extract raw trim from report (centre = 2048)
    let raw_trim1: u16 = 2048;
    let delta1 = tracker.update(raw_trim1);
    assert_eq!(delta1, 0, "first update should return 0");
    assert!(r1.trim_wheel.abs() < 0.01);

    // Simulate trim nose-up movement
    let raw_trim2: u16 = 2148;
    let delta2 = tracker.update(raw_trim2);
    assert_eq!(delta2, 100, "delta should be +100");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 11. Profile axis index uniqueness
// ═══════════════════════════════════════════════════════════════════════════════

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
    for device in [
        VelocityOneDevice::Flight,
        VelocityOneDevice::Flightstick,
    ] {
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
// 12. Error type consistency
// ═══════════════════════════════════════════════════════════════════════════════

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

// ═══════════════════════════════════════════════════════════════════════════════
// 13. Flight LED state mutations
// ═══════════════════════════════════════════════════════════════════════════════

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

// ═══════════════════════════════════════════════════════════════════════════════
// 14. Monotonicity: axis output increases with raw input
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn flight_roll_monotonically_increases() {
    let mut prev = -2.0f32;
    for raw in (0..=4095u16).step_by(16) {
        let data = make_flight_report_bytes(raw, 2048, 2048, 0, 0, 2048, 0, 15, 0);
        let r = parse_flight_report(&data).unwrap();
        assert!(
            r.roll >= prev,
            "roll not monotonic: raw={raw}, prev={prev}, cur={}",
            r.roll
        );
        prev = r.roll;
    }
}

#[test]
fn flightstick_throttle_monotonically_increases() {
    let mut prev = -1.0f32;
    for raw in 0..=255u8 {
        let data = make_flightstick_report_bytes(2048, 2048, 2048, raw, 0, 15);
        let r = parse_flightstick_report(&data).unwrap();
        assert!(
            r.throttle >= prev,
            "throttle not monotonic: raw={raw}, prev={prev}, cur={}",
            r.throttle
        );
        prev = r.throttle;
    }
}
