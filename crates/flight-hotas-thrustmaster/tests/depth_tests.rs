// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Comprehensive depth tests for flight-hotas-thrustmaster.
//!
//! These integration-style tests exercise public API surfaces across all
//! supported device families: T.Flight, Warthog, T.16000M/TWCS, TFRP/TPR,
//! Cougar, protocol helpers, detent tracking, PC-mode detection, profiles,
//! and preset configurations.

use flight_hotas_thrustmaster::*;

// ═══════════════════════════════════════════════════════════════════════════════
// §1  Device identification — exhaustive catalogue coverage
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn identify_all_device_table_entries_round_trip() {
    for entry in DEVICE_TABLE {
        let found = identify_device(VENDOR_ID, entry.pid);
        assert_eq!(
            found,
            Some(entry.device),
            "round-trip failed for PID 0x{:04X}",
            entry.pid
        );
    }
}

#[test]
fn identify_device_rejects_zero_vendor() {
    assert_eq!(identify_device(0x0000, 0xB10A), None);
}

#[test]
fn identify_device_rejects_close_vendor_id() {
    // 0x044E is one less than the real Thrustmaster VID
    assert_eq!(identify_device(0x044E, 0xB10A), None);
    assert_eq!(identify_device(0x0450, 0xB10A), None);
}

#[test]
fn device_table_pids_are_nonzero() {
    for entry in DEVICE_TABLE {
        assert_ne!(entry.pid, 0, "PID must not be zero for {:?}", entry.device);
    }
}

#[test]
fn device_names_contain_no_leading_trailing_whitespace() {
    for entry in DEVICE_TABLE {
        let name = entry.device.name();
        assert_eq!(name, name.trim(), "name has whitespace: {:?}", entry.device);
    }
}

#[test]
fn vendor_id_is_0x044f() {
    assert_eq!(VENDOR_ID, 0x044F);
}

// ═══════════════════════════════════════════════════════════════════════════════
// §2  Warthog stick parsing — boundary and bit-level tests
// ═══════════════════════════════════════════════════════════════════════════════

fn make_stick_report(x: u16, y: u16, rz: u16, btn_low: u16, btn_high: u8, hat: u8) -> Vec<u8> {
    let mut buf = vec![0u8; 10];
    buf[0..2].copy_from_slice(&x.to_le_bytes());
    buf[2..4].copy_from_slice(&y.to_le_bytes());
    buf[4..6].copy_from_slice(&rz.to_le_bytes());
    buf[6..8].copy_from_slice(&btn_low.to_le_bytes());
    buf[8] = btn_high;
    buf[9] = hat;
    buf
}

#[test]
fn warthog_stick_zero_raw_is_negative_one() {
    let data = make_stick_report(0, 0, 0, 0, 0, 0xFF);
    let state = parse_warthog_stick(&data).unwrap();
    assert!(state.axes.x < -0.99);
    assert!(state.axes.y < -0.99);
    assert!(state.axes.rz < -0.99);
}

#[test]
fn warthog_stick_max_raw_is_positive_one() {
    let data = make_stick_report(65535, 65535, 65535, 0, 0, 0xFF);
    let state = parse_warthog_stick(&data).unwrap();
    assert!(state.axes.x > 0.99);
    assert!(state.axes.y > 0.99);
    assert!(state.axes.rz > 0.99);
}

#[test]
fn warthog_stick_single_button_isolation() {
    for n in 1u8..=19 {
        let (btn_low, btn_high) = if n <= 16 {
            (1u16 << (n - 1), 0u8)
        } else {
            (0u16, 1u8 << (n - 17))
        };
        let data = make_stick_report(32768, 32768, 32768, btn_low, btn_high, 0xFF);
        let state = parse_warthog_stick(&data).unwrap();
        for check in 1u8..=19 {
            assert_eq!(
                state.buttons.button(check),
                check == n,
                "button({check}) wrong when only button {n} pressed"
            );
        }
    }
}

#[test]
fn warthog_stick_hat_all_directions() {
    let directions = [
        (0x00, WarthogHat::North),
        (0x10, WarthogHat::NorthEast),
        (0x20, WarthogHat::East),
        (0x30, WarthogHat::SouthEast),
        (0x40, WarthogHat::South),
        (0x50, WarthogHat::SouthWest),
        (0x60, WarthogHat::West),
        (0x70, WarthogHat::NorthWest),
        (0xF0, WarthogHat::Center),
    ];
    for (raw, expected) in directions {
        let data = make_stick_report(32768, 32768, 32768, 0, 0, raw);
        let state = parse_warthog_stick(&data).unwrap();
        assert_eq!(state.buttons.hat, expected, "hat raw=0x{raw:02X}");
    }
}

#[test]
fn warthog_stick_exact_min_length_accepted() {
    let data = vec![0u8; WARTHOG_STICK_MIN_REPORT_BYTES];
    assert!(parse_warthog_stick(&data).is_ok());
}

#[test]
fn warthog_stick_one_below_min_length_rejected() {
    let data = vec![0u8; WARTHOG_STICK_MIN_REPORT_BYTES - 1];
    assert!(parse_warthog_stick(&data).is_err());
}

#[test]
fn warthog_stick_error_contains_actual_length() {
    let data = vec![0u8; 3];
    let err = parse_warthog_stick(&data).unwrap_err();
    match err {
        WarthogParseError::TooShort { actual, .. } => assert_eq!(actual, 3),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// §3  Warthog throttle parsing
// ═══════════════════════════════════════════════════════════════════════════════

#[allow(clippy::too_many_arguments)]
fn make_throttle_report(
    scx: u16, scy: u16, tl: u16, tr: u16, tc: u16,
    btn_low: u16, btn_mid: u16, btn_high: u8, toggles: u8,
    hat_dms: u8, hat_csl: u8,
) -> Vec<u8> {
    let mut buf = vec![0u8; 20];
    buf[0..2].copy_from_slice(&scx.to_le_bytes());
    buf[2..4].copy_from_slice(&scy.to_le_bytes());
    buf[4..6].copy_from_slice(&tl.to_le_bytes());
    buf[6..8].copy_from_slice(&tr.to_le_bytes());
    buf[8..10].copy_from_slice(&tc.to_le_bytes());
    buf[10..12].copy_from_slice(&btn_low.to_le_bytes());
    buf[12..14].copy_from_slice(&btn_mid.to_le_bytes());
    buf[14] = btn_high;
    buf[15] = toggles;
    buf[16] = hat_dms;
    buf[17] = hat_csl;
    buf
}

#[test]
fn warthog_throttle_full_range_left_right() {
    let data = make_throttle_report(32768, 32768, 0, 65535, 32768, 0, 0, 0, 0, 0xFF, 0xFF);
    let state = parse_warthog_throttle(&data).unwrap();
    assert!(state.axes.throttle_left < 0.001, "left should be idle");
    assert!(state.axes.throttle_right > 0.999, "right should be full");
}

#[test]
fn warthog_throttle_button_40_isolated() {
    // Button 40 is bit 7 of buttons_high
    let data = make_throttle_report(32768, 32768, 0, 0, 0, 0, 0, 0x80, 0, 0xFF, 0xFF);
    let state = parse_warthog_throttle(&data).unwrap();
    assert!(state.buttons.button(40), "button 40 should be pressed");
    assert!(!state.buttons.button(39), "button 39 should not be pressed");
}

#[test]
fn warthog_throttle_toggles_bitmask_preserved() {
    let data = make_throttle_report(32768, 32768, 0, 0, 0, 0, 0, 0, 0b1010_0101, 0xFF, 0xFF);
    let state = parse_warthog_throttle(&data).unwrap();
    assert_eq!(state.buttons.toggles, 0b1010_0101);
}

#[test]
fn warthog_throttle_dual_hats_independent() {
    // DMS = South (4), CSL = West (6)
    let data = make_throttle_report(32768, 32768, 0, 0, 0, 0, 0, 0, 0, 0x04, 0x06);
    let state = parse_warthog_throttle(&data).unwrap();
    assert_eq!(state.buttons.hat_dms, WarthogHat::South);
    assert_eq!(state.buttons.hat_csl, WarthogHat::West);
}

#[test]
fn warthog_throttle_slew_extremes() {
    let data = make_throttle_report(0, 65535, 0, 0, 0, 0, 0, 0, 0, 0xFF, 0xFF);
    let state = parse_warthog_throttle(&data).unwrap();
    assert!(state.axes.slew_x < -0.99, "slew_x full left");
    assert!(state.axes.slew_y > 0.99, "slew_y full down");
}

#[test]
fn warthog_throttle_exact_min_length() {
    let data = vec![0u8; WARTHOG_THROTTLE_MIN_REPORT_BYTES];
    assert!(parse_warthog_throttle(&data).is_ok());
}

// ═══════════════════════════════════════════════════════════════════════════════
// §4  Shifted button resolution — comprehensive coverage
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn shifted_layer_produces_contiguous_range_20_to_37() {
    let mut logical: Vec<u8> = Vec::new();
    for n in 1..=19u8 {
        if let Some(l) = resolve_shifted_button(n, true) {
            if l >= 20 {
                logical.push(l);
            }
        }
    }
    logical.sort();
    // Should be exactly 20..=37 (18 values: buttons 1,3-19 shifted)
    assert_eq!(logical.len(), 18);
    assert_eq!(*logical.first().unwrap(), 20);
    assert_eq!(*logical.last().unwrap(), 37);
}

#[test]
fn shifted_and_unshifted_ranges_do_not_overlap() {
    let unshifted: Vec<u8> = (1..=19).filter_map(|n| resolve_shifted_button(n, false)).collect();
    let shifted: Vec<u8> = (1..=19)
        .filter_map(|n| resolve_shifted_button(n, true))
        .filter(|&l| l >= 20)
        .collect();
    for s in &shifted {
        assert!(!unshifted.contains(s), "overlap at logical {s}");
    }
}

#[test]
fn shifted_button_255_returns_none() {
    assert_eq!(resolve_shifted_button(255, false), None);
    assert_eq!(resolve_shifted_button(255, true), None);
}

// ═══════════════════════════════════════════════════════════════════════════════
// §5  Throttle split/merge detection
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn throttle_split_symmetric_difference() {
    // Exactly at threshold (0.02) should not be split
    assert!(!is_throttle_split(0.50, 0.52));
    // Just over threshold should be split
    assert!(is_throttle_split(0.50, 0.53));
}

#[test]
fn throttle_split_negative_difference() {
    assert!(is_throttle_split(0.80, 0.20));
}

#[test]
fn throttle_split_identical_values() {
    assert!(!is_throttle_split(0.0, 0.0));
    assert!(!is_throttle_split(1.0, 1.0));
    assert!(!is_throttle_split(0.5, 0.5));
}

// ═══════════════════════════════════════════════════════════════════════════════
// §6  LED control reports
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn led_report_off_has_correct_id() {
    let r = build_led_report(LedState::Off);
    assert_eq!(r[0], 0x01);
    assert_eq!(r[1], 0x00);
}

#[test]
fn led_report_brightness_zero_acts_like_off() {
    let r = build_led_report(LedState::Brightness(0));
    assert_eq!(r[1], 0);
}

#[test]
fn led_report_all_valid_brightness_levels() {
    for level in 1..=LedState::MAX_BRIGHTNESS {
        let r = build_led_report(LedState::Brightness(level));
        assert_eq!(r[1], level);
    }
}

#[test]
fn led_report_clamps_above_max() {
    for level in [6, 10, 100, 255] {
        let r = build_led_report(LedState::Brightness(level));
        assert_eq!(r[1], LedState::MAX_BRIGHTNESS);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// §7  T.16000M / TWCS parsing
// ═══════════════════════════════════════════════════════════════════════════════

fn make_t16000m_report(x: u16, y: u16, rz: u16, slider: u16, buttons: u16, hat: u8) -> Vec<u8> {
    let mut r = vec![0u8; 11];
    r[0..2].copy_from_slice(&x.to_le_bytes());
    r[2..4].copy_from_slice(&y.to_le_bytes());
    r[4..6].copy_from_slice(&rz.to_le_bytes());
    r[6..8].copy_from_slice(&slider.to_le_bytes());
    r[8..10].copy_from_slice(&buttons.to_le_bytes());
    r[10] = hat;
    r
}

fn make_twcs_report(throttle: u16, rx: u16, ry: u16, rz: u16, buttons: u16) -> Vec<u8> {
    let mut r = vec![0u8; 10];
    r[0..2].copy_from_slice(&throttle.to_le_bytes());
    r[2..4].copy_from_slice(&rx.to_le_bytes());
    r[4..6].copy_from_slice(&ry.to_le_bytes());
    r[6..8].copy_from_slice(&rz.to_le_bytes());
    r[8..10].copy_from_slice(&buttons.to_le_bytes());
    r
}

#[test]
fn t16000m_14bit_axis_masks_upper_bits() {
    // 0x4000 has bit 14 set — should be masked to 0 by the 14-bit logic
    let report = make_t16000m_report(0x4000, 8192, 8192, 0, 0, 0x0F);
    let state = parse_t16000m_report(&report).unwrap();
    // With masking, 0x4000 & 0x3FFF = 0, which normalises to ~ -1.0
    assert!(state.axes.x < -0.99, "upper bits should be masked: x={}", state.axes.x);
}

#[test]
fn t16000m_slider_full_range() {
    let report_min = make_t16000m_report(8192, 8192, 8192, 0, 0, 0x0F);
    let report_max = make_t16000m_report(8192, 8192, 8192, 65535, 0, 0x0F);
    let s_min = parse_t16000m_report(&report_min).unwrap();
    let s_max = parse_t16000m_report(&report_max).unwrap();
    assert!(s_min.axes.throttle < 0.001);
    assert!(s_max.axes.throttle > 0.999);
}

#[test]
fn t16000m_hat_all_8_directions_and_center() {
    let expected_dirs = [
        (0x00, 1), // N
        (0x01, 2), // NE
        (0x02, 3), // E
        (0x03, 4), // SE
        (0x04, 5), // S
        (0x05, 6), // SW
        (0x06, 7), // W
        (0x07, 8), // NW
        (0x0F, 0), // center
    ];
    for (raw, expected) in expected_dirs {
        let report = make_t16000m_report(8192, 8192, 8192, 0, 0, raw);
        let state = parse_t16000m_report(&report).unwrap();
        assert_eq!(state.buttons.hat, expected, "hat raw=0x{raw:02X}");
    }
}

#[test]
fn t16000m_report_id_0x01_is_stripped() {
    let mut report = vec![0x01u8];
    report.extend_from_slice(&make_t16000m_report(8192, 8192, 8192, 0, 0, 0x0F));
    let state = parse_t16000m_report(&report).unwrap();
    assert!(state.axes.x.abs() < 0.01);
}

#[test]
fn t16000m_report_id_0x02_is_rejected() {
    let mut report = vec![0x02u8];
    report.extend_from_slice(&make_t16000m_report(8192, 8192, 8192, 0, 0, 0x0F));
    assert!(parse_t16000m_report(&report).is_err());
}

#[test]
fn twcs_button_mask_limits_to_14_bits() {
    let report = make_twcs_report(0, 32768, 32768, 32768, 0xFFFF);
    let state = parse_twcs_report(&report).unwrap();
    assert_eq!(state.buttons.buttons, 0x3FFF);
}

#[test]
fn twcs_rocker_extremes() {
    let left = make_twcs_report(0, 32768, 32768, 0, 0);
    let right = make_twcs_report(0, 32768, 32768, 65535, 0);
    let s_left = parse_twcs_report(&left).unwrap();
    let s_right = parse_twcs_report(&right).unwrap();
    assert!(s_left.axes.rocker < -0.99);
    assert!(s_right.axes.rocker > 0.99);
}

#[test]
fn twcs_too_short_report_errors() {
    assert!(parse_twcs_report(&[0u8; 9]).is_err());
}

// ═══════════════════════════════════════════════════════════════════════════════
// §8  TFRP / TPR pedal parsing
// ═══════════════════════════════════════════════════════════════════════════════

fn make_tfrp(rz: u16, z: u16, rx: u16) -> Vec<u8> {
    let mut data = Vec::with_capacity(6);
    data.extend_from_slice(&rz.to_le_bytes());
    data.extend_from_slice(&z.to_le_bytes());
    data.extend_from_slice(&rx.to_le_bytes());
    data
}

#[test]
fn tfrp_center_is_approximately_half() {
    let report = make_tfrp(32768, 32768, 32768);
    let state = parse_tfrp_report(&report).unwrap();
    assert!((state.axes.rudder - 0.5).abs() < 0.01);
    assert!((state.axes.right_pedal - 0.5).abs() < 0.01);
    assert!((state.axes.left_pedal - 0.5).abs() < 0.01);
}

#[test]
fn tfrp_min_bytes_constant_is_six() {
    assert_eq!(TFRP_MIN_REPORT_BYTES, 6);
}

#[test]
fn tpr_min_bytes_matches_tfrp() {
    assert_eq!(TPR_MIN_REPORT_BYTES, TFRP_MIN_REPORT_BYTES);
}

#[test]
fn tpr_parse_is_same_as_tfrp() {
    let report = make_tfrp(12345, 54321, 33333);
    let tfrp_state = parse_tfrp_report(&report).unwrap();
    let tpr_state = parse_tpr_report(&report).unwrap();
    assert_eq!(tfrp_state.axes.rudder, tpr_state.axes.rudder);
    assert_eq!(tfrp_state.axes.right_pedal, tpr_state.axes.right_pedal);
    assert_eq!(tfrp_state.axes.left_pedal, tpr_state.axes.left_pedal);
}

#[test]
fn tfrp_extra_bytes_ignored() {
    let mut report = make_tfrp(65535, 0, 0);
    report.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
    let state = parse_tfrp_report(&report).unwrap();
    assert!((state.axes.rudder - 1.0).abs() < 1e-4);
}

// ═══════════════════════════════════════════════════════════════════════════════
// §9  Cougar parsing
// ═══════════════════════════════════════════════════════════════════════════════

fn make_cougar(x: u16, y: u16, throttle: u16, buttons: u16, hat: u8, switches: u8) -> Vec<u8> {
    let mut buf = vec![0u8; 10];
    buf[0..2].copy_from_slice(&x.to_le_bytes());
    buf[2..4].copy_from_slice(&y.to_le_bytes());
    buf[4..6].copy_from_slice(&throttle.to_le_bytes());
    buf[6..8].copy_from_slice(&buttons.to_le_bytes());
    buf[8] = hat;
    buf[9] = switches;
    buf
}

#[test]
fn cougar_pid_matches_device_table() {
    assert_eq!(COUGAR_STICK_PID, 0x0400);
    let dev = identify_device(VENDOR_ID, COUGAR_STICK_PID);
    assert_eq!(dev, Some(ThrustmasterDevice::HotasCougar));
}

#[test]
fn cougar_min_report_constant() {
    assert_eq!(COUGAR_MIN_REPORT_BYTES, 10);
}

#[test]
fn cougar_all_buttons_pressed() {
    let data = make_cougar(32768, 32768, 0, 0xFFFF, 0xFF, 0xFF);
    let state = parse_cougar(&data).unwrap();
    for n in 1u8..=16 {
        assert!(state.buttons.button(n), "button {n} should be pressed");
    }
    assert!(!state.buttons.button(0), "button 0 out of range");
    assert!(!state.buttons.button(17), "button 17 out of range");
}

#[test]
fn cougar_hat_all_directions() {
    let hat_values = [
        (0x00, CougarHat::North),
        (0x02, CougarHat::East),
        (0x04, CougarHat::South),
        (0x06, CougarHat::West),
        (0x0F, CougarHat::Center),
    ];
    for (raw, expected) in hat_values {
        let data = make_cougar(32768, 32768, 0, 0, raw, 0);
        let state = parse_cougar(&data).unwrap();
        assert_eq!(state.buttons.tms_hat, expected, "hat raw=0x{raw:02X}");
    }
}

#[test]
fn cougar_throttle_switches_byte_preserved() {
    let data = make_cougar(32768, 32768, 0, 0, 0xFF, 0b1111_0000);
    let state = parse_cougar(&data).unwrap();
    assert_eq!(state.buttons.throttle_switches, 0b1111_0000);
}

// ═══════════════════════════════════════════════════════════════════════════════
// §10  Detent tracker — state machine depth
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn detent_config_boundary_values() {
    let d = ThrottleDetentConfig::hotas4_idle();
    assert!(d.contains(d.lower() + 0.001));
    assert!(d.contains(d.upper() - 0.001));
}

#[test]
fn detent_tracker_full_sweep_enter_exit() {
    let mut tracker = ThrottleDetentTracker::hotas4_default();
    let enter = tracker.update(0.05);
    assert_eq!(enter.len(), 1);
    assert!(matches!(enter[0], DetentEvent::Entered { detent_index: 0, .. }));

    let inside = tracker.update(0.04);
    assert!(inside.is_empty(), "no event while inside");

    let exit = tracker.update(0.20);
    assert_eq!(exit.len(), 1);
    assert!(matches!(exit[0], DetentEvent::Exited { detent_index: 0, .. }));

    let outside = tracker.update(0.30);
    assert!(outside.is_empty(), "no event while outside");
}

#[test]
fn detent_tracker_is_active_by_index() {
    let configs = vec![
        ThrottleDetentConfig { name: "a", index: 0, position: 0.10, half_width: 0.02 },
        ThrottleDetentConfig { name: "b", index: 1, position: 0.90, half_width: 0.02 },
    ];
    let mut tracker = ThrottleDetentTracker::new(configs);
    tracker.update(0.10);
    assert!(tracker.is_active(0));
    assert!(!tracker.is_active(1));

    tracker.update(0.50); // exit detent 0
    tracker.update(0.90); // enter detent 1
    assert!(!tracker.is_active(0));
    assert!(tracker.is_active(1));
}

#[test]
fn detent_tracker_reset_allows_re_enter() {
    let mut tracker = ThrottleDetentTracker::hotas4_default();
    tracker.update(0.05); // enter
    tracker.reset();
    assert!(!tracker.any_active());
    let events = tracker.update(0.05); // should fire Entered again
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], DetentEvent::Entered { .. }));
}

#[test]
fn detent_tracker_iterates_configs() {
    let tracker = ThrottleDetentTracker::hotas4_default();
    let names: Vec<&str> = tracker.detents().map(|d| d.name).collect();
    assert_eq!(names, vec!["idle"]);
}

// ═══════════════════════════════════════════════════════════════════════════════
// §11  PC-mode detection state machine
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn pc_mode_detector_default_is_unknown() {
    let d = PcModeDetector::new();
    assert_eq!(d.status(), PcModeStatus::Unknown);
    assert!(!d.is_pc_mode());
    assert!(!d.is_console_mode());
}

#[test]
fn pc_mode_single_report_with_confirm_1() {
    let mut d = PcModeDetector::with_confirm_count(1);
    assert_eq!(d.update(&[0u8; 8]), PcModeStatus::PcMode);
    assert!(d.is_pc_mode());
}

#[test]
fn pc_mode_transitions_require_confirmation() {
    let mut d = PcModeDetector::with_confirm_count(3);
    // 3 PC reports to confirm
    d.update(&[0u8; 9]);
    d.update(&[0u8; 9]);
    d.update(&[0u8; 9]);
    assert!(d.is_pc_mode());

    // 2 console reports not enough to switch
    d.update(&[0u8; 5]);
    d.update(&[0u8; 5]);
    assert!(d.is_pc_mode(), "should still be PC mode before 3 console reports");

    // 3rd console report confirms transition
    d.update(&[0u8; 5]);
    assert!(d.is_console_mode());
}

#[test]
fn pc_mode_reset_returns_to_unknown() {
    let mut d = PcModeDetector::with_confirm_count(1);
    d.update(&[0u8; 8]);
    assert!(d.is_pc_mode());
    d.reset();
    assert_eq!(d.status(), PcModeStatus::Unknown);
}

#[test]
fn pc_mode_display_formatting() {
    assert_eq!(format!("{}", PcModeStatus::PcMode), "PC mode (Green LED)");
    assert_eq!(format!("{}", PcModeStatus::ConsoleMode), "Console mode (Red LED)");
    assert_eq!(format!("{}", PcModeStatus::Unknown), "Unknown");
}

#[test]
fn pc_mode_min_report_len_constant() {
    assert_eq!(PC_MODE_MIN_REPORT_LEN, 8);
}

#[test]
fn pc_mode_handshake_instructions_not_empty() {
    assert!(!PC_MODE_HANDSHAKE_INSTRUCTIONS.is_empty());
    assert!(PC_MODE_HANDSHAKE_INSTRUCTIONS.contains("PC"));
}

#[test]
fn pc_mode_console_guidance_only_when_console() {
    let mut d = PcModeDetector::with_confirm_count(1);
    d.update(&[0u8; 5]);
    assert!(d.console_mode_guidance().is_some());

    d.reset();
    d.update(&[0u8; 10]);
    assert!(d.console_mode_guidance().is_none());
}

// ═══════════════════════════════════════════════════════════════════════════════
// §12  Profiles — structural invariants
// ═══════════════════════════════════════════════════════════════════════════════

use flight_hotas_thrustmaster::profiles::{
    AxisNormalization, device_profile, profiled_devices,
};
use flight_hotas_thrustmaster::protocol::ThrustmasterDevice;

#[test]
fn all_profiled_devices_have_valid_axis_normalization() {
    for dev in profiled_devices() {
        let profile = device_profile(dev).unwrap();
        for ax in &profile.axes {
            match ax.normalization {
                AxisNormalization::Bipolar { center, half_span } => {
                    assert!(center > 0.0, "{:?}/{}: center must be positive", dev, ax.id);
                    assert!(half_span > 0.0, "{:?}/{}: half_span must be positive", dev, ax.id);
                }
                AxisNormalization::Unipolar { max } => {
                    assert!(max > 0.0, "{:?}/{}: max must be positive", dev, ax.id);
                }
            }
        }
    }
}

#[test]
fn warthog_throttle_is_only_device_with_leds() {
    for dev in profiled_devices() {
        let p = device_profile(dev).unwrap();
        if dev == ThrustmasterDevice::WarthogThrottle {
            assert!(p.has_leds, "Warthog throttle should have LEDs");
        } else {
            assert!(!p.has_leds, "{:?} should not have LEDs", dev);
        }
    }
}

#[test]
fn profiles_device_field_matches_query() {
    for dev in profiled_devices() {
        let p = device_profile(dev).unwrap();
        assert_eq!(p.device, dev);
    }
}

#[test]
fn unsupported_devices_return_none() {
    assert!(device_profile(ThrustmasterDevice::TFlightHotasX).is_none());
    assert!(device_profile(ThrustmasterDevice::TFlightHotas4).is_none());
    assert!(device_profile(ThrustmasterDevice::TcaYokeBoeing).is_none());
}

#[test]
fn profile_notes_not_empty() {
    for dev in profiled_devices() {
        let p = device_profile(dev).unwrap();
        assert!(!p.notes.is_empty(), "{:?} should have notes", dev);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// §13  Presets — recommended axis configuration
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn recommended_config_has_all_four_axes() {
    let configs = recommended_axis_config();
    assert_eq!(configs.len(), 4);
    let names: Vec<&str> = configs.iter().map(|c| c.name).collect();
    assert!(names.contains(&"roll"));
    assert!(names.contains(&"pitch"));
    assert!(names.contains(&"yaw"));
    assert!(names.contains(&"throttle"));
}

#[test]
fn recommended_config_throttle_has_slew_rate() {
    let configs = recommended_axis_config();
    let throttle = configs.iter().find(|c| c.name == "throttle").unwrap();
    assert!(throttle.slew_rate.is_some());
    assert!(throttle.slew_rate.unwrap() > 0.0);
}

#[test]
fn recommended_config_yaw_has_largest_deadzone() {
    let configs = recommended_axis_config();
    let yaw = configs.iter().find(|c| c.name == "yaw").unwrap();
    let max_other = configs
        .iter()
        .filter(|c| c.name != "yaw")
        .map(|c| c.deadzone)
        .fold(0.0f32, f32::max);
    assert!(yaw.deadzone >= max_other, "yaw deadzone should be largest");
}

// ═══════════════════════════════════════════════════════════════════════════════
// §14  T.Flight input handler — axis mode and yaw resolution
// ═══════════════════════════════════════════════════════════════════════════════

use flight_hid_support::device_support::{AxisMode, TFlightModel};

#[test]
fn tflight_handler_auto_detects_merged_from_8_bytes() {
    let mut handler = TFlightInputHandler::new(TFlightModel::Hotas4);
    let report = vec![0x00, 0x80, 0x00, 0x80, 0x80, 0x80, 0x00, 0x00];
    let state = handler.try_parse_report(&report).unwrap();
    assert_eq!(state.axis_mode, AxisMode::Merged);
    assert!(state.axes.rocker.is_none());
}

#[test]
fn tflight_handler_auto_detects_separate_from_9_bytes() {
    let mut handler = TFlightInputHandler::new(TFlightModel::Hotas4);
    let report = vec![0x00, 0x80, 0x00, 0x80, 0x80, 0x80, 0x80, 0x00, 0x00];
    let state = handler.try_parse_report(&report).unwrap();
    assert_eq!(state.axis_mode, AxisMode::Separate);
    assert!(state.axes.rocker.is_some());
}

#[test]
fn tflight_handler_report_id_stripping() {
    let mut handler = TFlightInputHandler::new(TFlightModel::Hotas4).with_report_id(true);
    // 1 byte report ID + 8 bytes payload
    let report = vec![0x01, 0x00, 0x80, 0x00, 0x80, 0x80, 0x80, 0x00, 0x00];
    let state = handler.try_parse_report(&report).unwrap();
    assert_eq!(state.axis_mode, AxisMode::Merged);
}

#[test]
fn tflight_yaw_resolution_merged_always_combined() {
    let state = TFlightInputState {
        axes: TFlightAxes {
            roll: 0.0,
            pitch: 0.0,
            throttle: 0.0,
            twist: 0.5,
            rocker: None,
        },
        buttons: TFlightButtons::default(),
        axis_mode: AxisMode::Merged,
    };
    let yaw = state.resolve_yaw(TFlightYawPolicy::Auto);
    assert_eq!(yaw.source, TFlightYawSource::Combined);
    assert!((yaw.value - 0.5).abs() < 0.001);
}

#[test]
fn tflight_yaw_resolution_separate_auto_prefers_aux() {
    let state = TFlightInputState {
        axes: TFlightAxes {
            roll: 0.0,
            pitch: 0.0,
            throttle: 0.0,
            twist: 0.1,
            rocker: Some(0.9),
        },
        buttons: TFlightButtons::default(),
        axis_mode: AxisMode::Separate,
    };
    let yaw = state.resolve_yaw(TFlightYawPolicy::Auto);
    assert_eq!(yaw.source, TFlightYawSource::Aux);
    assert!((yaw.value - 0.9).abs() < 0.001);
}

#[test]
fn tflight_yaw_resolution_separate_twist_policy() {
    let state = TFlightInputState {
        axes: TFlightAxes {
            roll: 0.0,
            pitch: 0.0,
            throttle: 0.0,
            twist: 0.3,
            rocker: Some(0.7),
        },
        buttons: TFlightButtons::default(),
        axis_mode: AxisMode::Separate,
    };
    let yaw = state.resolve_yaw(TFlightYawPolicy::Twist);
    assert_eq!(yaw.source, TFlightYawSource::Twist);
    assert!((yaw.value - 0.3).abs() < 0.001);
}

#[test]
fn tflight_yaw_resolution_aux_fallback_when_none() {
    let state = TFlightInputState {
        axes: TFlightAxes {
            roll: 0.0,
            pitch: 0.0,
            throttle: 0.0,
            twist: 0.4,
            rocker: None,
        },
        buttons: TFlightButtons::default(),
        axis_mode: AxisMode::Separate,
    };
    let yaw = state.resolve_yaw(TFlightYawPolicy::Aux);
    assert_eq!(yaw.source, TFlightYawSource::Twist);
    assert!((yaw.value - 0.4).abs() < 0.001);
}

// ═══════════════════════════════════════════════════════════════════════════════
// §15  Toggle switch helpers
// ═══════════════════════════════════════════════════════════════════════════════

use flight_hotas_thrustmaster::protocol::toggles;

#[test]
fn toggle_every_bit_position() {
    for bit in 0..8u8 {
        let mask = 1u8 << bit;
        assert!(toggles::is_set(mask, bit), "bit {bit} should be set");
        assert!(!toggles::is_set(!mask, bit), "bit {bit} should not be set");
    }
}

#[test]
fn toggle_bit_8_and_above_always_false() {
    for bit in 8..=255u8 {
        assert!(!toggles::is_set(0xFF, bit), "bit {bit} should always be false");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// §16  Cross-module integration — device ID ↔ profile consistency
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn device_table_entries_with_profiles_have_nonempty_names() {
    for entry in DEVICE_TABLE {
        if let Some(profile) = device_profile(entry.device) {
            assert!(
                !profile.name.is_empty(),
                "profile name empty for {:?}",
                entry.device
            );
            // Profile name and device name may differ when variants share
            // a parser (e.g. TRudder uses TFRP profile).
            assert!(
                !entry.device.name().is_empty(),
                "device name empty for {:?}",
                entry.device
            );
        }
    }
}

#[test]
fn re_exported_pid_constants_match_device_table() {
    let check = |pid: u16, expected: ThrustmasterDevice| {
        let found = identify_device(VENDOR_ID, pid);
        assert_eq!(found, Some(expected), "PID 0x{pid:04X} mismatch");
    };
    check(T16000M_JOYSTICK_PID, ThrustmasterDevice::T16000mJoystick);
    check(TWCS_THROTTLE_PID, ThrustmasterDevice::TwcsThrottle);
    check(WARTHOG_JOYSTICK_PID, ThrustmasterDevice::WarthogJoystick);
    check(WARTHOG_THROTTLE_PID, ThrustmasterDevice::WarthogThrottle);
    check(TFRP_RUDDER_PEDALS_PID, ThrustmasterDevice::TfrpRudderPedals);
    check(T_RUDDER_PID, ThrustmasterDevice::TRudder);
    check(TPR_PENDULAR_RUDDER_PID, ThrustmasterDevice::TprPendular);
    check(TPR_PENDULAR_RUDDER_BULK_PID, ThrustmasterDevice::TprPendularBulk);
    check(TFLIGHT_HOTAS_4_PID, ThrustmasterDevice::TFlightHotas4);
    check(TFLIGHT_HOTAS_4_PID_LEGACY, ThrustmasterDevice::TFlightHotas4Legacy);
    check(TFLIGHT_HOTAS_ONE_PID, ThrustmasterDevice::TFlightHotasOne);
    check(TFLIGHT_HOTAS_X_PID, ThrustmasterDevice::TFlightHotasX);
    // Note: TCA Boeing PIDs in flight-hid-support (0x0409..0x040B) differ
    // from the protocol device table (0xB68C..0xB695). We verify the
    // protocol table entries separately.
    assert!(identify_device(VENDOR_ID, 0xB68C).is_some());
    assert!(identify_device(VENDOR_ID, 0xB694).is_some());
    assert!(identify_device(VENDOR_ID, 0xB695).is_some());
}
