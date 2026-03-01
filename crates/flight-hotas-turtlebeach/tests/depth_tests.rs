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
    FLIGHT_MIN_REPORT_BYTES, decode_all_toggles,
    parse_flight_report, serialize_gear_led_report, serialize_display_command,
    FlightLedState, GearLedState, ToggleSwitchPosition, TrimWheelTracker,
    DisplayCommand, DisplayPage,
    // Protocol — Flightstick
    FLIGHTSTICK_MIN_REPORT_BYTES, parse_flightstick_report,
    // Legacy (VID 0x1432) — Flightdeck / Rudder
    parse_flightdeck_report, parse_rudder_report,
    // Profiles
    profile_for_device,
};

// ── Report builders ──────────────────────────────────────────────────────────

/// Builder for VelocityOne Flight HID reports with sensible centre defaults.
struct FlightInput {
    roll: u16,
    pitch: u16,
    rudder: u16,
    tl: u8,
    tr: u8,
    trim: u16,
    buttons: u64,
    hat: u8,
    toggles: u8,
}

impl Default for FlightInput {
    fn default() -> Self {
        Self {
            roll: 2048, pitch: 2048, rudder: 2048,
            tl: 0, tr: 0, trim: 2048,
            buttons: 0, hat: 15, toggles: 0,
        }
    }
}

fn build_flight(input: &FlightInput) -> [u8; 20] {
    let mut b = [0u8; 20];
    b[0..2].copy_from_slice(&input.roll.to_le_bytes());
    b[2..4].copy_from_slice(&input.pitch.to_le_bytes());
    b[4..6].copy_from_slice(&input.rudder.to_le_bytes());
    b[6] = input.tl;
    b[7] = input.tr;
    b[8..10].copy_from_slice(&input.trim.to_le_bytes());
    b[10..18].copy_from_slice(&input.buttons.to_le_bytes());
    b[18] = input.hat;
    b[19] = input.toggles;
    b
}

fn make_flightstick(
    x: u16, y: u16, twist: u16,
    throttle: u8, buttons: u16, hat: u8,
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

fn make_flightdeck(roll: u16, pitch: u16, tl: u8, tr: u8, buttons: u32) -> [u8; 16] {
    let mut b = [0u8; 16];
    b[0..2].copy_from_slice(&roll.to_le_bytes());
    b[2..4].copy_from_slice(&pitch.to_le_bytes());
    b[4] = tl;
    b[5] = tr;
    b[6..10].copy_from_slice(&buttons.to_le_bytes());
    b
}

fn make_rudder(rudder: u16, bl: u8, br: u8) -> [u8; 8] {
    let mut b = [0u8; 8];
    b[0..2].copy_from_slice(&rudder.to_le_bytes());
    b[2] = bl;
    b[3] = br;
    b
}

// ═══════════════════════════════════════════════════════════════════════════════
// 1. YOKE AXES — pitch/roll range, center calibration, linearity, resolution,
//    travel limits
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

// ═══════════════════════════════════════════════════════════════════════════════
// 2. THROTTLE QUADRANT — lever independence, reverse range, detent positions
// ═══════════════════════════════════════════════════════════════════════════════

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
fn throttle_lever_monotonicity() {
    let mut prev_left = -1.0f32;
    for raw in (0u8..=255).step_by(16) {
        let r = parse_flight_report(&build_flight(&FlightInput { tl: raw, ..Default::default() })).unwrap();
        assert!(r.throttle_left >= prev_left,
            "throttle should be monotonically increasing: raw={} gave {} < {}", raw, r.throttle_left, prev_left);
        prev_left = r.throttle_left;
    }
}

#[test]
fn throttle_legacy_flightdeck_lever_independence() {
    let left = parse_flightdeck_report(&make_flightdeck(32767, 32767, 255, 0, 0)).unwrap();
    let right = parse_flightdeck_report(&make_flightdeck(32767, 32767, 0, 255, 0)).unwrap();
    assert!(left.throttle_left > 0.99);
    assert!(left.throttle_right < 0.01);
    assert!(right.throttle_right > 0.99);
    assert!(right.throttle_left < 0.01);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. RUDDER — rudder + differential toe brakes, center calibration, dead zone
// ═══════════════════════════════════════════════════════════════════════════════

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
fn rudder_differential_toe_brakes() {
    let left_brake = parse_rudder_report(&make_rudder(32767, 255, 0)).unwrap();
    let right_brake = parse_rudder_report(&make_rudder(32767, 0, 255)).unwrap();
    assert!(left_brake.brake_left > 0.99, "full left brake should be ~1.0");
    assert!(left_brake.brake_right < 0.01, "right brake should be off");
    assert!(right_brake.brake_right > 0.99, "full right brake should be ~1.0");
    assert!(right_brake.brake_left < 0.01, "left brake should be off");
}

#[test]
fn rudder_deadzone_small_inputs_near_center() {
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

// ═══════════════════════════════════════════════════════════════════════════════
// 4. BUTTON/SWITCH MAPPING — gear switch, flap lever, trim, buttons, mode switch
// ═══════════════════════════════════════════════════════════════════════════════

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
fn trim_wheel_delta_tracking() {
    let mut tracker = TrimWheelTracker::new();
    assert_eq!(tracker.update(2048), 0);
    assert_eq!(tracker.update(2058), 10);
    assert_eq!(tracker.update(2038), -20);
    tracker.reset();
    assert_eq!(tracker.update(1000), 0);
}

#[test]
fn flightstick_buttons_16bit_mask() {
    let r = parse_flightstick_report(&make_flightstick(2048, 2048, 2048, 0, 0xFFFF, 15)).unwrap();
    assert_eq!(r.buttons, 0xFFFF, "all 16 buttons should be set");

    let r2 = parse_flightstick_report(&make_flightstick(2048, 2048, 2048, 0, 0x0001, 15)).unwrap();
    assert_eq!(r2.buttons & 1, 1, "trigger button (bit 0) should be set");
    assert_eq!(r2.buttons >> 1, 0, "no other buttons should be set");
}

#[test]
fn display_mode_switch_pages() {
    for (page, expected_byte) in [
        (DisplayPage::Nav, 0u8),
        (DisplayPage::Engine, 1),
        (DisplayPage::Systems, 2),
        (DisplayPage::Custom, 3),
    ] {
        let cmd = DisplayCommand { page, brightness: 128 };
        let report = serialize_display_command(&cmd);
        assert_eq!(report[0], 0x02, "report ID should be 0x02");
        assert_eq!(report[2], expected_byte, "page byte mismatch for {:?}", page);
        assert_eq!(report[3], 128, "brightness should be 128");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. DEVICE IDENTIFICATION — VID/PID matching, model discrimination, combined
//    unit detection
// ═══════════════════════════════════════════════════════════════════════════════

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
fn model_discrimination_unique_pids() {
    let devices = VelocityOneDevice::all();
    let mut pids: Vec<u16> = devices.iter().map(|d| d.product_id()).collect();
    let len_before = pids.len();
    pids.sort();
    pids.dedup();
    assert_eq!(pids.len(), len_before, "all PIDs must be unique");
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
fn unknown_vid_pid_rejected() {
    assert!(!is_turtle_beach_device(0x0000, 0x1050), "wrong VID should be rejected");
    assert!(!is_turtle_beach_device(TURTLE_BEACH_VID, 0xDEAD), "unknown PID should be rejected");
    assert_eq!(identify_device(0xFFFF), None);
    assert_eq!(identify_device(0x0000), None);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Additional depth coverage — LED output, profiles, error handling
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn gear_led_serialization_roundtrip() {
    let mut leds = FlightLedState::all_off();
    leds.set_from_gear_state(GearLedState::Down);
    let report = serialize_gear_led_report(&leds);
    assert_eq!(report[1] & 0b0001_0101, 0b0001_0101);
    assert_eq!(report[1] & 0b0010_1010, 0);
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
fn hat_switch_directions_flightstick() {
    for raw_hat in 0u8..=7 {
        let r = parse_flightstick_report(&make_flightstick(2048, 2048, 2048, 0, 0, raw_hat)).unwrap();
        assert_eq!(r.hat, raw_hat + 1, "raw hat {} should map to {}", raw_hat, raw_hat + 1);
    }
    let r = parse_flightstick_report(&make_flightstick(2048, 2048, 2048, 0, 0, 15)).unwrap();
    assert_eq!(r.hat, 0, "hat ≥8 should map to 0 (centered)");
}

#[test]
fn flight_report_too_short_errors() {
    for len in [0, 1, 10, 19] {
        let data = vec![0u8; len];
        let err = parse_flight_report(&data).unwrap_err();
        match err {
            flight_hotas_turtlebeach::TurtleBeachError::TooShort { expected, actual } => {
                assert_eq!(expected, FLIGHT_MIN_REPORT_BYTES);
                assert_eq!(actual, len);
            }
        }
    }
}

#[test]
fn flightstick_report_too_short_errors() {
    for len in [0, 1, 6, 11] {
        let data = vec![0u8; len];
        let err = parse_flightstick_report(&data).unwrap_err();
        match err {
            flight_hotas_turtlebeach::TurtleBeachError::TooShort { expected, actual } => {
                assert_eq!(expected, FLIGHTSTICK_MIN_REPORT_BYTES);
                assert_eq!(actual, len);
            }
        }
    }
}

#[test]
fn profile_axis_count_within_capabilities() {
    for device in [VelocityOneDevice::Flight, VelocityOneDevice::Flightstick, VelocityOneDevice::Rudder] {
        let profile = profile_for_device(device);
        let caps = capabilities(device);
        assert!(profile.axes.len() as u8 <= caps.axes,
            "profile axis count should not exceed capabilities for {:?}: profile={} caps={}",
            device, profile.axes.len(), caps.axes);
        assert!(!profile.axes.is_empty(),
            "profile should have at least one axis for {:?}", device);
    }
}

#[test]
fn profile_vendor_id_consistent() {
    for device in VelocityOneDevice::all() {
        let profile = profile_for_device(*device);
        assert_eq!(profile.vendor_id, TURTLE_BEACH_VID,
            "profile vendor_id should be TURTLE_BEACH_VID for {:?}", device);
    }
}
