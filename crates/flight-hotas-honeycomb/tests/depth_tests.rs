// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the `flight-hotas-honeycomb` crate.
//!
//! These integration tests exercise cross-module interactions, boundary
//! conditions, and realistic device scenarios for the Honeycomb Alpha Yoke,
//! Bravo Throttle Quadrant, and Charlie Rudder Pedals.

use flight_hotas_honeycomb::alpha::ALPHA_REPORT_LEN;
use flight_hotas_honeycomb::bravo::BRAVO_REPORT_LEN;
use flight_hotas_honeycomb::bravo_leds::{BravoLedState, serialize_led_report};
use flight_hotas_honeycomb::charlie::CHARLIE_REPORT_LEN;
use flight_hotas_honeycomb::health::{HoneycombHealth, HoneycombHealthMonitor};
use flight_hotas_honeycomb::presets;
use flight_hotas_honeycomb::profiles::{
    ALPHA_PROFILE, BRAVO_PROFILE, CHARLIE_PROFILE, profile_for_model,
};
use flight_hotas_honeycomb::protocol::{
    EncoderTracker, GearIndicatorState, MagnetoPosition, ToggleSwitchState, WrappingEncoder,
    decode_all_toggle_switches, decode_magneto, decode_toggle_switch,
};
use flight_hotas_honeycomb::{
    AlphaParseError, BravoParseError, CharlieParseError, HONEYCOMB_ALPHA_YOKE_PID,
    HONEYCOMB_BRAVO_PID, HONEYCOMB_CHARLIE_PID, HONEYCOMB_VENDOR_ID, HoneycombModel,
    honeycomb_model, is_honeycomb_device, parse_alpha_report, parse_bravo_report,
    parse_charlie_report,
};

// ── Report builders ──────────────────────────────────────────────────────────

fn alpha_report(roll: u16, pitch: u16, buttons: u64, hat: u8) -> [u8; ALPHA_REPORT_LEN] {
    let mut r = [0u8; ALPHA_REPORT_LEN];
    r[0] = 0x01;
    r[1..3].copy_from_slice(&roll.to_le_bytes());
    r[3..5].copy_from_slice(&pitch.to_le_bytes());
    r[5] = (buttons & 0xFF) as u8;
    r[6] = ((buttons >> 8) & 0xFF) as u8;
    r[7] = ((buttons >> 16) & 0xFF) as u8;
    r[8] = ((buttons >> 24) & 0xFF) as u8;
    r[9] = ((buttons >> 32) & 0xFF) as u8;
    r[10] = hat & 0x0F;
    r
}

fn bravo_report(throttles: [u16; 7], buttons: u64) -> [u8; BRAVO_REPORT_LEN] {
    let mut r = [0u8; BRAVO_REPORT_LEN];
    r[0] = 0x01;
    for (i, &t) in throttles.iter().enumerate() {
        let off = 1 + i * 2;
        r[off..off + 2].copy_from_slice(&t.to_le_bytes());
    }
    r[15..23].copy_from_slice(&buttons.to_le_bytes());
    r
}

fn charlie_report(rudder: u16, left: u16, right: u16) -> [u8; CHARLIE_REPORT_LEN] {
    let mut r = [0u8; CHARLIE_REPORT_LEN];
    r[0] = 0x01;
    r[1..3].copy_from_slice(&rudder.to_le_bytes());
    r[3..5].copy_from_slice(&left.to_le_bytes());
    r[5..7].copy_from_slice(&right.to_le_bytes());
    r
}

// ═══════════════════════════════════════════════════════════════════════════════
// §1  Device identification
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn device_id_recognises_all_three_products() {
    assert!(is_honeycomb_device(
        HONEYCOMB_VENDOR_ID,
        HONEYCOMB_ALPHA_YOKE_PID
    ));
    assert!(is_honeycomb_device(
        HONEYCOMB_VENDOR_ID,
        HONEYCOMB_BRAVO_PID
    ));
    assert!(is_honeycomb_device(
        HONEYCOMB_VENDOR_ID,
        HONEYCOMB_CHARLIE_PID
    ));
}

#[test]
fn device_id_rejects_wrong_vendor() {
    assert!(!is_honeycomb_device(0x0000, HONEYCOMB_BRAVO_PID));
    assert!(!is_honeycomb_device(0xFFFF, HONEYCOMB_ALPHA_YOKE_PID));
}

#[test]
fn device_id_rejects_unknown_product() {
    assert!(!is_honeycomb_device(HONEYCOMB_VENDOR_ID, 0x0000));
    assert!(!is_honeycomb_device(HONEYCOMB_VENDOR_ID, 0xFFFF));
}

#[test]
fn model_lookup_returns_correct_variant() {
    assert_eq!(
        honeycomb_model(HONEYCOMB_ALPHA_YOKE_PID),
        Some(HoneycombModel::AlphaYoke)
    );
    assert_eq!(
        honeycomb_model(HONEYCOMB_BRAVO_PID),
        Some(HoneycombModel::BravoThrottle)
    );
    assert_eq!(
        honeycomb_model(HONEYCOMB_CHARLIE_PID),
        Some(HoneycombModel::CharliePedals)
    );
}

#[test]
fn model_lookup_returns_none_for_unknown_pid() {
    assert_eq!(honeycomb_model(0x0000), None);
    assert_eq!(honeycomb_model(0x1903), None);
    assert_eq!(honeycomb_model(0xFFFF), None);
}

// ═══════════════════════════════════════════════════════════════════════════════
// §2  Alpha Yoke — axis boundary conditions
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn alpha_axis_symmetry_around_centre() {
    // Equidistant offsets from centre should produce equal magnitudes.
    let above = parse_alpha_report(&alpha_report(2048 + 500, 2048, 0, 15)).unwrap();
    let below = parse_alpha_report(&alpha_report(2048 - 500, 2048, 0, 15)).unwrap();
    assert!(
        (above.axes.roll + below.axes.roll).abs() < 1e-4,
        "symmetric offsets should cancel: {} vs {}",
        above.axes.roll,
        below.axes.roll,
    );
}

#[test]
fn alpha_axis_one_step_above_centre() {
    let state = parse_alpha_report(&alpha_report(2049, 2048, 0, 15)).unwrap();
    assert!(state.axes.roll > 0.0, "2049 should be slightly positive");
    assert!(state.axes.roll < 0.01, "2049 should be very close to zero");
}

#[test]
fn alpha_axis_one_step_below_centre() {
    let state = parse_alpha_report(&alpha_report(2047, 2048, 0, 15)).unwrap();
    assert!(state.axes.roll < 0.0, "2047 should be slightly negative");
    assert!(state.axes.roll > -0.01);
}

#[test]
fn alpha_axis_clamped_above_12bit_max() {
    // Raw value beyond 4095 should be clamped; norm_12bit_centered clamps to 4095 first.
    let state = parse_alpha_report(&alpha_report(5000, 2048, 0, 15)).unwrap();
    assert!((state.axes.roll - 1.0).abs() < 0.01, "should clamp to ~1.0");
}

#[test]
fn alpha_pitch_full_back() {
    let state = parse_alpha_report(&alpha_report(2048, 4095, 0, 15)).unwrap();
    assert!(state.axes.pitch > 0.99, "full back pitch should be ~+1.0");
}

// ═══════════════════════════════════════════════════════════════════════════════
// §3  Alpha Yoke — hat switch exhaustive
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn alpha_hat_all_eight_directions_and_centre() {
    let expected = [
        (0u8, 1, "N"),
        (1, 2, "NE"),
        (2, 3, "E"),
        (3, 4, "SE"),
        (4, 5, "S"),
        (5, 6, "SW"),
        (6, 7, "W"),
        (7, 8, "NW"),
        (15, 0, "center"),
    ];
    for (raw, value, dir) in expected {
        let state = parse_alpha_report(&alpha_report(2048, 2048, 0, raw)).unwrap();
        assert_eq!(state.buttons.hat, value, "raw={raw}");
        assert_eq!(state.buttons.hat_direction(), dir, "raw={raw}");
    }
}

#[test]
fn alpha_hat_values_8_to_14_map_to_centre() {
    for raw in 8u8..=14 {
        let state = parse_alpha_report(&alpha_report(2048, 2048, 0, raw)).unwrap();
        assert_eq!(state.buttons.hat, 0, "raw {raw} should map to centred");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// §4  Alpha Yoke — button edge cases
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn alpha_all_36_buttons_pressed() {
    let mask: u64 = (1u64 << 36) - 1; // bits 0–35
    let state = parse_alpha_report(&alpha_report(2048, 2048, mask, 15)).unwrap();
    for n in 1u8..=36 {
        assert!(state.buttons.is_pressed(n), "button {n} should be pressed");
    }
}

#[test]
fn alpha_button_out_of_range_always_false() {
    let mask = u64::MAX;
    let state = parse_alpha_report(&alpha_report(2048, 2048, mask, 15)).unwrap();
    assert!(!state.buttons.is_pressed(0), "button 0 is out of range");
    assert!(!state.buttons.is_pressed(37), "button 37 is out of range");
    assert!(!state.buttons.is_pressed(255), "button 255 is out of range");
}

#[test]
fn alpha_single_button_isolation() {
    for n in 1u8..=36 {
        let mask: u64 = 1 << (n - 1);
        let state = parse_alpha_report(&alpha_report(2048, 2048, mask, 15)).unwrap();
        assert!(state.buttons.is_pressed(n), "button {n} should be pressed");
        // Check adjacent buttons are not pressed
        if n > 1 {
            assert!(!state.buttons.is_pressed(n - 1));
        }
        if n < 36 {
            assert!(!state.buttons.is_pressed(n + 1));
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// §5  Alpha Yoke — error conditions
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn alpha_empty_report_error() {
    let err = parse_alpha_report(&[]).unwrap_err();
    assert!(matches!(
        err,
        AlphaParseError::TooShort {
            expected: ALPHA_REPORT_LEN,
            got: 0
        }
    ));
}

#[test]
fn alpha_report_id_zero_rejected() {
    let mut r = [0u8; ALPHA_REPORT_LEN];
    r[0] = 0x00;
    assert!(matches!(
        parse_alpha_report(&r),
        Err(AlphaParseError::UnknownReportId { id: 0x00 })
    ));
}

#[test]
fn alpha_report_exactly_minimum_length_parses() {
    let r = alpha_report(2048, 2048, 0, 15);
    assert_eq!(r.len(), ALPHA_REPORT_LEN);
    assert!(parse_alpha_report(&r).is_ok());
}

#[test]
fn alpha_longer_report_still_parses() {
    let mut long = vec![0u8; ALPHA_REPORT_LEN + 10];
    let r = alpha_report(2048, 2048, 0, 15);
    long[..ALPHA_REPORT_LEN].copy_from_slice(&r);
    assert!(parse_alpha_report(&long).is_ok());
}

#[test]
fn alpha_error_display_formatting() {
    let err = AlphaParseError::TooShort {
        expected: ALPHA_REPORT_LEN,
        got: 3,
    };
    let msg = format!("{err}");
    assert!(msg.contains("11"), "should mention expected length");
    assert!(msg.contains("3"), "should mention actual length");

    let err2 = AlphaParseError::UnknownReportId { id: 0xAB };
    let msg2 = format!("{err2}");
    assert!(msg2.contains("AB"), "should contain hex report ID");
}

// ═══════════════════════════════════════════════════════════════════════════════
// §6  Bravo Throttle — axis isolation
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn bravo_individual_throttle_isolation() {
    // Setting only throttle 3 to max should not affect others.
    let state = parse_bravo_report(&bravo_report([0, 0, 4095, 0, 0, 0, 0], 0)).unwrap();
    assert!(state.axes.throttle1 < 0.001);
    assert!(state.axes.throttle2 < 0.001);
    assert!((state.axes.throttle3 - 1.0).abs() < 1e-4);
    assert!(state.axes.throttle4 < 0.001);
    assert!(state.axes.throttle5 < 0.001);
    assert!(state.axes.flap_lever < 0.001);
    assert!(state.axes.spoiler < 0.001);
}

#[test]
fn bravo_flap_and_spoiler_independent() {
    let state = parse_bravo_report(&bravo_report([0, 0, 0, 0, 0, 4095, 2048], 0)).unwrap();
    assert!((state.axes.flap_lever - 1.0).abs() < 1e-4);
    let expected_spoiler = 2048.0 / 4095.0;
    assert!((state.axes.spoiler - expected_spoiler).abs() < 1e-3);
}

#[test]
fn bravo_quarter_throttle_values() {
    let quarter = 1024u16; // ~25%
    let state = parse_bravo_report(&bravo_report([quarter; 7], 0)).unwrap();
    let expected = 1024.0 / 4095.0;
    assert!((state.axes.throttle1 - expected).abs() < 1e-3);
    assert!((state.axes.throttle5 - expected).abs() < 1e-3);
}

// ═══════════════════════════════════════════════════════════════════════════════
// §7  Bravo Throttle — button convenience methods
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn bravo_button_out_of_range_always_false() {
    let state = parse_bravo_report(&bravo_report([0; 7], u64::MAX)).unwrap();
    assert!(!state.buttons.is_pressed(0), "button 0 out of range");
    assert!(!state.buttons.is_pressed(65), "button 65 out of range");
}

#[test]
fn bravo_all_64_buttons_pressed() {
    let state = parse_bravo_report(&bravo_report([0; 7], u64::MAX)).unwrap();
    for n in 1u8..=64 {
        assert!(state.buttons.is_pressed(n), "button {n} should be pressed");
    }
}

#[test]
fn bravo_ap_master_and_gear_independent() {
    let mask: u64 = (1 << 7) | (1 << 30); // AP master + gear up
    let state = parse_bravo_report(&bravo_report([0; 7], mask)).unwrap();
    assert!(state.buttons.ap_master());
    assert!(state.buttons.gear_up());
    assert!(!state.buttons.gear_down());
}

#[test]
fn bravo_no_buttons_all_convenience_false() {
    let state = parse_bravo_report(&bravo_report([0; 7], 0)).unwrap();
    assert!(!state.buttons.ap_master());
    assert!(!state.buttons.gear_up());
    assert!(!state.buttons.gear_down());
}

// ═══════════════════════════════════════════════════════════════════════════════
// §8  Bravo Throttle — error conditions
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn bravo_empty_report_error() {
    let err = parse_bravo_report(&[]).unwrap_err();
    assert!(matches!(
        err,
        BravoParseError::TooShort {
            expected: BRAVO_REPORT_LEN,
            got: 0
        }
    ));
}

#[test]
fn bravo_report_id_zero_rejected() {
    let mut r = [0u8; BRAVO_REPORT_LEN];
    r[0] = 0x00;
    assert!(matches!(
        parse_bravo_report(&r),
        Err(BravoParseError::UnknownReportId { id: 0x00 })
    ));
}

#[test]
fn bravo_report_id_0xff_rejected() {
    let mut r = [0u8; BRAVO_REPORT_LEN];
    r[0] = 0xFF;
    assert!(matches!(
        parse_bravo_report(&r),
        Err(BravoParseError::UnknownReportId { id: 0xFF })
    ));
}

#[test]
fn bravo_error_display_formatting() {
    let err = BravoParseError::TooShort {
        expected: BRAVO_REPORT_LEN,
        got: 5,
    };
    let msg = format!("{err}");
    assert!(msg.contains("23"));
    assert!(msg.contains("5"));
}

// ═══════════════════════════════════════════════════════════════════════════════
// §9  Bravo LED serialisation
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn led_report_id_always_zero() {
    let report = serialize_led_report(&BravoLedState::all_on());
    assert_eq!(report[0], 0x00);
    let report = serialize_led_report(&BravoLedState::all_off());
    assert_eq!(report[0], 0x00);
}

#[test]
fn led_individual_ap_mode_bits() {
    let modes: &[(fn(&mut BravoLedState), u8)] = &[
        (|l| l.hdg = true, 0b0000_0001),
        (|l| l.nav = true, 0b0000_0010),
        (|l| l.apr = true, 0b0000_0100),
        (|l| l.rev = true, 0b0000_1000),
        (|l| l.alt = true, 0b0001_0000),
        (|l| l.vs = true, 0b0010_0000),
        (|l| l.ias = true, 0b0100_0000),
        (|l| l.autopilot = true, 0b1000_0000),
    ];
    for (setter, expected_byte) in modes {
        let mut leds = BravoLedState::all_off();
        setter(&mut leds);
        let report = serialize_led_report(&leds);
        assert_eq!(report[1], *expected_byte, "AP mode bit mismatch");
        assert_eq!(report[2], 0);
        assert_eq!(report[3], 0);
        assert_eq!(report[4], 0);
    }
}

#[test]
fn led_gear_transit_shows_all_red() {
    let mut leds = BravoLedState::all_off();
    let (green, red) = GearIndicatorState::Transit.led_colors();
    leds.set_all_gear(green); // green=false → sets red
    // Verify red bits are on, green bits are off
    let report = serialize_led_report(&leds);
    assert_eq!(report[2] & 0b0001_0101, 0, "green bits should be off");
    assert_eq!(
        report[2] & 0b0010_1010,
        0b0010_1010,
        "red bits should be on"
    );
    // Confirm the led_colors helper returned expected values
    assert!(!green);
    assert!(red);
}

#[test]
fn led_gear_down_shows_all_green() {
    let mut leds = BravoLedState::all_off();
    let (green, _red) = GearIndicatorState::Down.led_colors();
    leds.set_all_gear(green); // green=true
    let report = serialize_led_report(&leds);
    assert_eq!(report[2] & 0b0001_0101, 0b0001_0101, "green bits");
    assert_eq!(report[2] & 0b0010_1010, 0, "red bits should be off");
}

#[test]
fn led_annunciator_row2_upper_nibble_always_zero() {
    // Even with all LEDs on, bits 4-7 of byte 4 must be 0.
    let report = serialize_led_report(&BravoLedState::all_on());
    assert_eq!(report[4] & 0xF0, 0);
}

#[test]
fn led_clone_and_eq() {
    let a = BravoLedState::all_on();
    let b = a.clone();
    assert_eq!(a, b);
    assert_ne!(BravoLedState::all_off(), BravoLedState::all_on());
}

// ═══════════════════════════════════════════════════════════════════════════════
// §10  Charlie Rudder Pedals — boundary & error
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn charlie_rudder_half_deflections() {
    // Quarter deflection right (~1024 above centre)
    let state = parse_charlie_report(&charlie_report(2048 + 1024, 0, 0)).unwrap();
    assert!(state.axes.rudder > 0.49 && state.axes.rudder < 0.51);
}

#[test]
fn charlie_brake_mid_travel() {
    let mid = 2048u16;
    let state = parse_charlie_report(&charlie_report(2048, mid, mid)).unwrap();
    let expected = 2048.0 / 4095.0;
    assert!((state.axes.left_brake - expected).abs() < 1e-3);
    assert!((state.axes.right_brake - expected).abs() < 1e-3);
}

#[test]
fn charlie_empty_report_error() {
    let err = parse_charlie_report(&[]).unwrap_err();
    assert!(matches!(
        err,
        CharlieParseError::TooShort {
            expected: CHARLIE_REPORT_LEN,
            got: 0
        }
    ));
}

#[test]
fn charlie_report_id_0xff_rejected() {
    let mut r = [0u8; CHARLIE_REPORT_LEN];
    r[0] = 0xFF;
    assert!(matches!(
        parse_charlie_report(&r),
        Err(CharlieParseError::UnknownReportId { id: 0xFF })
    ));
}

#[test]
fn charlie_longer_report_still_parses() {
    let mut long = vec![0u8; CHARLIE_REPORT_LEN + 20];
    let r = charlie_report(2048, 0, 0);
    long[..CHARLIE_REPORT_LEN].copy_from_slice(&r);
    assert!(parse_charlie_report(&long).is_ok());
}

#[test]
fn charlie_error_display_formatting() {
    let err = CharlieParseError::UnknownReportId { id: 0xFE };
    let msg = format!("{err}");
    assert!(msg.contains("FE"));
}

// ═══════════════════════════════════════════════════════════════════════════════
// §11  Protocol — magneto cross-module with Alpha
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn magneto_decoded_from_alpha_report() {
    // Build an alpha report with magneto buttons 25+26 (bits 24+25) set → Both
    let mask: u64 = (1 << 24) | (1 << 25);
    let state = parse_alpha_report(&alpha_report(2048, 2048, mask, 15)).unwrap();
    let pos = decode_magneto(state.buttons.mask);
    assert_eq!(pos, MagnetoPosition::Both);
}

#[test]
fn magneto_start_from_alpha_report() {
    let mask: u64 = 1 << 26; // button 27 = bit 26
    let state = parse_alpha_report(&alpha_report(2048, 2048, mask, 15)).unwrap();
    assert_eq!(decode_magneto(state.buttons.mask), MagnetoPosition::Start);
}

#[test]
fn magneto_display_all_positions() {
    for pos in MagnetoPosition::all() {
        let s = format!("{pos}");
        assert!(!s.is_empty());
        assert_eq!(s, pos.label());
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// §12  Protocol — encoder tracker edge cases
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn encoder_rapid_cw_pulses_count_correctly() {
    let mut tracker = EncoderTracker::new();
    let cw: u64 = 1 << 12;
    let mut total = 0i32;
    for _ in 0..10 {
        total += tracker.update(cw);
        tracker.update(0); // release
    }
    assert_eq!(total, 10);
}

#[test]
fn encoder_held_button_only_counts_once() {
    let mut tracker = EncoderTracker::new();
    let cw: u64 = 1 << 12;
    let first = tracker.update(cw);
    let second = tracker.update(cw);
    let third = tracker.update(cw);
    assert_eq!(first, 1);
    assert_eq!(second, 0);
    assert_eq!(third, 0);
}

#[test]
fn encoder_interleaved_with_bravo_report() {
    let mut tracker = EncoderTracker::new();
    // CW button pressed in Bravo report (bit 12)
    let cw_mask: u64 = 1 << 12;
    let state = parse_bravo_report(&bravo_report([0; 7], cw_mask)).unwrap();
    let delta = tracker.update(state.buttons.mask);
    assert_eq!(delta, 1);

    // Same button held
    let delta2 = tracker.update(state.buttons.mask);
    assert_eq!(delta2, 0);
}

// ═══════════════════════════════════════════════════════════════════════════════
// §13  Protocol — wrapping encoder realistic sequences
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn heading_encoder_full_rotation_returns_to_start() {
    let mut enc = WrappingEncoder::heading();
    enc.set_value(0);
    let cw: u64 = 1 << 12;
    // Rotate 360 clicks CW
    for _ in 0..360 {
        enc.update(cw);
        enc.update(0);
    }
    assert_eq!(enc.value(), 0, "full 360° rotation should return to 0");
}

#[test]
fn altitude_encoder_step_100() {
    let mut enc = WrappingEncoder::altitude();
    enc.set_value(3000);
    let cw: u64 = 1 << 12;
    enc.update(cw);
    assert_eq!(enc.value(), 3100);
    enc.update(0);
    let ccw: u64 = 1 << 13;
    enc.update(ccw);
    assert_eq!(enc.value(), 3000);
}

#[test]
fn vs_encoder_clamps_at_negative_limit() {
    let mut enc = WrappingEncoder::vertical_speed();
    enc.set_value(-9999);
    let ccw: u64 = 1 << 13;
    enc.update(ccw);
    assert_eq!(enc.value(), -9999, "should clamp at min");
}

#[test]
fn wrapping_encoder_set_value_out_of_range_clamps() {
    let mut enc = WrappingEncoder::heading();
    enc.set_value(-100);
    assert_eq!(enc.value(), 0);
    enc.set_value(1000);
    assert_eq!(enc.value(), 359);
}

#[test]
fn wrapping_encoder_reset_clamps_initial() {
    let mut enc = WrappingEncoder::heading();
    enc.reset(999);
    assert_eq!(enc.value(), 359);
    enc.reset(-5);
    assert_eq!(enc.value(), 0);
}

// ═══════════════════════════════════════════════════════════════════════════════
// §14  Protocol — gear indicator with LED integration
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn gear_state_from_bravo_report_integration() {
    let gear_down: u64 = 1 << 31;
    let state = parse_bravo_report(&bravo_report([0; 7], gear_down)).unwrap();
    let gear = GearIndicatorState::from_button_mask(state.buttons.mask);
    assert_eq!(gear, GearIndicatorState::Down);
}

#[test]
fn gear_display_all_variants() {
    assert_eq!(format!("{}", GearIndicatorState::Up), "UP");
    assert_eq!(format!("{}", GearIndicatorState::Down), "DOWN");
    assert_eq!(format!("{}", GearIndicatorState::Transit), "TRANSIT");
}

#[test]
fn gear_led_colors_match_set_all_gear() {
    for gear_state in [
        GearIndicatorState::Up,
        GearIndicatorState::Down,
        GearIndicatorState::Transit,
    ] {
        let (green, red) = gear_state.led_colors();
        let mut leds = BravoLedState::all_off();
        if green {
            leds.set_all_gear(true);
        } else if red {
            leds.set_all_gear(false);
        }
        let report = serialize_led_report(&leds);
        if green {
            assert_ne!(report[2] & 0b0001_0101, 0, "green LEDs for {gear_state:?}");
        }
        if red {
            assert_ne!(report[2] & 0b0010_1010, 0, "red LEDs for {gear_state:?}");
        }
        if !green && !red {
            assert_eq!(
                report[2] & 0b0011_1111,
                0,
                "no gear LEDs for {gear_state:?}"
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// §15  Protocol — toggle switches
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn toggle_all_seven_up() {
    let mut mask: u64 = 0;
    for sw in 1u8..=7 {
        let bit = 33 + (sw - 1) as u32 * 2;
        mask |= 1 << bit;
    }
    let switches = decode_all_toggle_switches(mask);
    for (i, s) in switches.iter().enumerate() {
        assert_eq!(*s, ToggleSwitchState::Up, "switch {} should be Up", i + 1);
    }
}

#[test]
fn toggle_all_seven_down() {
    let mut mask: u64 = 0;
    for sw in 1u8..=7 {
        let bit = 33 + (sw - 1) as u32 * 2 + 1;
        mask |= 1 << bit;
    }
    let switches = decode_all_toggle_switches(mask);
    for (i, s) in switches.iter().enumerate() {
        assert_eq!(
            *s,
            ToggleSwitchState::Down,
            "switch {} should be Down",
            i + 1
        );
    }
}

#[test]
fn toggle_switch_from_bravo_report() {
    // Switch 2 UP = bit 35
    let mask: u64 = 1 << 35;
    let state = parse_bravo_report(&bravo_report([0; 7], mask)).unwrap();
    let sw = decode_toggle_switch(state.buttons.mask, 2);
    assert_eq!(sw, ToggleSwitchState::Up);
}

#[test]
fn toggle_switch_invalid_numbers_are_center() {
    for n in [0u8, 8, 9, 100, 255] {
        assert_eq!(
            decode_toggle_switch(u64::MAX, n),
            ToggleSwitchState::Center,
            "invalid switch_num={n} should be Center"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// §16  Health monitor
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn health_interleaved_success_error_recovery() {
    let mut m = HoneycombHealthMonitor::new();
    m.record_success();
    assert_eq!(m.status(), HoneycombHealth::Healthy);

    m.record_error();
    m.record_error();
    assert_eq!(m.status(), HoneycombHealth::Degraded);

    // A single success should reset error count
    m.record_success();
    assert_eq!(m.status(), HoneycombHealth::Healthy);
}

#[test]
fn health_saturating_errors_do_not_overflow() {
    let mut m = HoneycombHealthMonitor::new();
    for _ in 0..1000 {
        m.record_error();
    }
    assert_eq!(m.status(), HoneycombHealth::Disconnected);
}

#[test]
fn health_default_is_same_as_new() {
    let a = HoneycombHealthMonitor::new();
    let b = HoneycombHealthMonitor::default();
    assert_eq!(a.status(), b.status());
    assert_eq!(a.status(), HoneycombHealth::Stale);
}

// ═══════════════════════════════════════════════════════════════════════════════
// §17  Presets — constant validation
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn preset_deadzones_are_positive_and_small() {
    for dz in [
        presets::ALPHA_AXIS_DEADZONE,
        presets::BRAVO_THROTTLE_DEADZONE_IDLE,
        presets::BRAVO_THROTTLE_DEADZONE_FULL,
        presets::CHARLIE_RUDDER_DEADZONE,
        presets::CHARLIE_BRAKE_DEADZONE,
    ] {
        assert!(
            dz >= 0.0 && dz <= 0.1,
            "deadzone {dz} out of expected range"
        );
    }
}

#[test]
fn preset_expos_in_range() {
    for expo in [presets::ALPHA_AXIS_EXPO, presets::CHARLIE_RUDDER_EXPO] {
        assert!(expo >= 0.0 && expo <= 1.0, "expo {expo} out of range");
    }
}

#[test]
fn preset_axis_names_are_unique_per_device() {
    use std::collections::HashSet;
    let mut alpha_names: HashSet<&str> = HashSet::new();
    for name in presets::ALPHA_AXIS_NAMES {
        assert!(
            alpha_names.insert(name),
            "duplicate Alpha axis name: {name}"
        );
    }
    let mut bravo_names: HashSet<&str> = HashSet::new();
    for name in presets::BRAVO_AXIS_NAMES {
        assert!(
            bravo_names.insert(name),
            "duplicate Bravo axis name: {name}"
        );
    }
    let mut charlie_names: HashSet<&str> = HashSet::new();
    for name in presets::CHARLIE_AXIS_NAMES {
        assert!(
            charlie_names.insert(name),
            "duplicate Charlie axis name: {name}"
        );
    }
}

#[test]
fn preset_bravo_axis_count_matches_report() {
    assert_eq!(presets::BRAVO_AXIS_NAMES.len(), 7);
}

#[test]
fn preset_alpha_axis_count_matches_report() {
    assert_eq!(presets::ALPHA_AXIS_NAMES.len(), 2);
}

#[test]
fn preset_charlie_axis_count_matches_report() {
    assert_eq!(presets::CHARLIE_AXIS_NAMES.len(), 3);
}

// ═══════════════════════════════════════════════════════════════════════════════
// §18  Profiles — cross-module consistency
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn profile_pids_match_constants() {
    assert_eq!(ALPHA_PROFILE.product_id, HONEYCOMB_ALPHA_YOKE_PID);
    assert_eq!(BRAVO_PROFILE.product_id, HONEYCOMB_BRAVO_PID);
    assert_eq!(CHARLIE_PROFILE.product_id, HONEYCOMB_CHARLIE_PID);
}

#[test]
fn profile_for_model_matches_static_profiles() {
    assert_eq!(
        profile_for_model(HoneycombModel::AlphaYoke).name,
        ALPHA_PROFILE.name
    );
    assert_eq!(
        profile_for_model(HoneycombModel::BravoThrottle).name,
        BRAVO_PROFILE.name
    );
    assert_eq!(
        profile_for_model(HoneycombModel::CharliePedals).name,
        CHARLIE_PROFILE.name
    );
}

#[test]
fn profile_axis_names_match_presets() {
    for (i, axis) in ALPHA_PROFILE.axes.iter().enumerate() {
        assert_eq!(axis.name, presets::ALPHA_AXIS_NAMES[i]);
    }
    for axis in CHARLIE_PROFILE.axes.iter() {
        assert!(
            presets::CHARLIE_AXIS_NAMES.contains(&axis.name),
            "Charlie axis '{}' not in presets",
            axis.name
        );
    }
}

#[test]
fn profile_no_duplicate_axis_indices() {
    use std::collections::HashSet;
    for profile in [&ALPHA_PROFILE, &BRAVO_PROFILE, &CHARLIE_PROFILE] {
        let mut indices: HashSet<u8> = HashSet::new();
        for axis in profile.axes.iter() {
            assert!(
                indices.insert(axis.index),
                "duplicate axis index {} in {}",
                axis.index,
                profile.name
            );
        }
    }
}

#[test]
fn profile_bravo_has_led_and_encoder_support() {
    assert!(BRAVO_PROFILE.has_leds);
    assert!(BRAVO_PROFILE.has_encoders);
    assert!(!ALPHA_PROFILE.has_leds);
    assert!(!CHARLIE_PROFILE.has_encoders);
}

#[test]
fn profile_button_numbers_within_device_range() {
    for btn in ALPHA_PROFILE.buttons {
        assert!(
            btn.button_num >= 1 && btn.button_num <= 36,
            "Alpha button {} out of range",
            btn.button_num
        );
    }
    for btn in BRAVO_PROFILE.buttons {
        assert!(
            btn.button_num >= 1 && btn.button_num <= 64,
            "Bravo button {} out of range",
            btn.button_num
        );
    }
    assert!(CHARLIE_PROFILE.buttons.is_empty());
}

#[test]
fn all_profiles_axis_mappings_have_sim_var_hints() {
    for profile in [&ALPHA_PROFILE, &BRAVO_PROFILE, &CHARLIE_PROFILE] {
        for axis in profile.axes.iter() {
            assert!(
                !axis.sim_var_hint.is_empty(),
                "missing sim_var_hint for {} in {}",
                axis.name,
                profile.name
            );
        }
    }
}

#[test]
fn all_profiles_button_mappings_have_sim_event_hints() {
    for profile in [&ALPHA_PROFILE, &BRAVO_PROFILE, &CHARLIE_PROFILE] {
        for btn in profile.buttons {
            assert!(
                !btn.sim_event_hint.is_empty(),
                "missing sim_event_hint for {} in {}",
                btn.name,
                profile.name
            );
        }
    }
}
