// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for Virpil VPC device protocols.
//!
//! Covers:
//! 1. Report format (input report structure, axis byte order, button byte map,
//!    mode byte, device type identification, report size validation)
//! 2. Grip capabilities (Alpha axes/buttons/hats, WarBRD differences, ministick
//!    analog, hat 4-way/8-way)
//! 3. Throttle (dual axis range, detent position, encoder direction, mode
//!    switches, idle cutoff, reverse range)
//! 4. Helicopter controls (collective range, twist throttle, governor axis,
//!    rotor brake)
//! 5. Profile (DCS profile per device, combined HOTAS+pedals setup, shift layers)

use flight_hotas_virpil::{
    VIRPIL_AXIS_MAX, VIRPIL_VENDOR_ID,
    // Protocol
    protocol::{
        AXIS_MAX, AXIS_RESOLUTION_BITS, DEVICE_TABLE, INPUT_REPORT_ID, LED_REPORT_ID,
        LED_REPORT_SIZE, LedColor, build_led_report, denormalize_axis, device_info,
        normalize_axis,
    },
    // Grips
    VpcAlphaHat, parse_alpha_report,
    AlphaPrimeVariant, parse_alpha_prime_report,
    parse_mongoost_stick_report,
    WarBrdVariant, parse_warbrd_report,
    // Throttle
    parse_cm3_throttle_report,
    VPC_CM3_THROTTLE_MIN_REPORT_BYTES,
    // Helicopter
    parse_rotor_tcs_report,
    VPC_ROTOR_TCS_MIN_REPORT_BYTES,
    // ACE Torq
    parse_ace_torq_report,
    VPC_ACE_TORQ_MIN_REPORT_BYTES,
    // Pedals
    parse_ace_pedals_report,
    VPC_ACE_PEDALS_MIN_REPORT_BYTES,
    // Profiles
    profiles::{
        AxisRole, HatType, ALPHA_PROFILE, CM3_THROTTLE_PROFILE,
        ACE_PEDALS_PROFILE, ROTOR_TCS_PROFILE, ALL_PROFILES, profile_for_pid,
    },
    // Device IDs
    VIRPIL_CM3_THROTTLE_PID, VIRPIL_CONSTELLATION_ALPHA_LEFT_PID, VIRPIL_WARBRD_PID,
    VIRPIL_WARBRD_D_PID, VIRPIL_MONGOOST_STICK_PID, VIRPIL_ACE_PEDALS_PID,
    VIRPIL_ROTOR_TCS_PLUS_PID, VIRPIL_ACE_TORQ_PID,
    VirpilModel, is_virpil_device, virpil_model,
};

// ═══════════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════════

/// Build a 15-byte stick report (report_id=0x01, 5×u16-LE axes, 4 button bytes).
fn stick_report(axes: [u16; 5], buttons: [u8; 4]) -> Vec<u8> {
    let mut data = vec![INPUT_REPORT_ID];
    for ax in &axes {
        data.extend_from_slice(&ax.to_le_bytes());
    }
    data.extend_from_slice(&buttons);
    data
}

/// Build a 23-byte CM3 throttle report (report_id + 6×u16-LE axes + 10 button bytes).
fn throttle_cm3_report(axes: [u16; 6], buttons: [u8; 10]) -> Vec<u8> {
    let mut data = vec![INPUT_REPORT_ID];
    for ax in &axes {
        data.extend_from_slice(&ax.to_le_bytes());
    }
    data.extend_from_slice(&buttons);
    data
}

/// Build an 11-byte Rotor TCS report (report_id + 3×u16-LE axes + 4 button bytes).
fn rotor_tcs_report(axes: [u16; 3], buttons: [u8; 4]) -> Vec<u8> {
    let mut data = vec![INPUT_REPORT_ID];
    for ax in &axes {
        data.extend_from_slice(&ax.to_le_bytes());
    }
    data.extend_from_slice(&buttons);
    data
}

/// Build a 5-byte ACE Torq report (report_id + 1×u16-LE axis + 2 button bytes).
fn ace_torq_report(throttle: u16, buttons: [u8; 2]) -> Vec<u8> {
    let mut data = vec![INPUT_REPORT_ID];
    data.extend_from_slice(&throttle.to_le_bytes());
    data.extend_from_slice(&buttons);
    data
}

/// Build a 9-byte ACE Pedals report (report_id + 3×u16-LE axes + 2 button bytes).
fn ace_pedals_report(axes: [u16; 3], buttons: [u8; 2]) -> Vec<u8> {
    let mut data = vec![INPUT_REPORT_ID];
    for ax in &axes {
        data.extend_from_slice(&ax.to_le_bytes());
    }
    data.extend_from_slice(&buttons);
    data
}

// ═══════════════════════════════════════════════════════════════════════════════
// 1. Report format tests (8)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn report_format_input_report_id_is_0x01() {
    assert_eq!(INPUT_REPORT_ID, 0x01, "VIRPIL input reports use report_id 0x01");
}

#[test]
fn report_format_axis_byte_order_is_little_endian() {
    // Place a known value (0x1234) into axis slot 0 as LE bytes.
    // Verify parser reads it correctly as u16 LE.
    let raw_val: u16 = 0x1234;
    let le = raw_val.to_le_bytes(); // [0x34, 0x12]
    let mut report = vec![0x01]; // report_id
    report.extend_from_slice(&le); // axis 0
    for _ in 1..5 {
        report.extend_from_slice(&0u16.to_le_bytes());
    }
    report.extend_from_slice(&[0u8; 4]); // buttons
    let state = parse_alpha_report(&report).unwrap();
    let expected = raw_val as f32 / VIRPIL_AXIS_MAX as f32;
    assert!(
        (state.axes.x - expected).abs() < 1e-4,
        "axis X must be parsed from LE bytes: expected {expected}, got {}",
        state.axes.x
    );
}

#[test]
fn report_format_button_byte_map_lsb_first() {
    // Button 1 is bit 0 of byte 0; button 9 is bit 0 of byte 1.
    let buttons = [0x01, 0x01, 0x00, 0xF0]; // btn 1 + btn 9 + hat=center
    let report = stick_report([0u16; 5], buttons);
    let state = parse_alpha_report(&report).unwrap();
    assert!(state.buttons.is_pressed(1), "button 1 = byte0 bit0");
    assert!(state.buttons.is_pressed(9), "button 9 = byte1 bit0");
    assert!(!state.buttons.is_pressed(2));
    assert!(!state.buttons.is_pressed(10));
}

#[test]
fn report_format_mode_byte_hat_nibble_encoding() {
    // Hat directions are encoded in high nibble (bits 4-7) of last button byte.
    // Verify all 8 directions + center.
    let directions = [
        (0u8, VpcAlphaHat::North),
        (1, VpcAlphaHat::NorthEast),
        (2, VpcAlphaHat::East),
        (3, VpcAlphaHat::SouthEast),
        (4, VpcAlphaHat::South),
        (5, VpcAlphaHat::SouthWest),
        (6, VpcAlphaHat::West),
        (7, VpcAlphaHat::NorthWest),
        (0x0F, VpcAlphaHat::Center),
    ];
    for (nibble, expected_hat) in &directions {
        let buttons = [0x00, 0x00, 0x00, nibble << 4];
        let report = stick_report([0u16; 5], buttons);
        let state = parse_alpha_report(&report).unwrap();
        assert_eq!(
            state.buttons.hat, *expected_hat,
            "nibble {nibble:#x} should map to {expected_hat:?}"
        );
    }
}

#[test]
fn report_format_device_type_identification_by_pid() {
    // Every known PID should resolve to a VirpilModel variant.
    let known_pids = [
        (VIRPIL_CONSTELLATION_ALPHA_LEFT_PID, VirpilModel::ConstellationAlphaLeft),
        (VIRPIL_MONGOOST_STICK_PID, VirpilModel::MongoostStick),
        (VIRPIL_WARBRD_PID, VirpilModel::WarBrd),
        (VIRPIL_WARBRD_D_PID, VirpilModel::WarBrdD),
        (VIRPIL_CM3_THROTTLE_PID, VirpilModel::Cm3Throttle),
        (VIRPIL_ACE_PEDALS_PID, VirpilModel::AcePedals),
        (VIRPIL_ROTOR_TCS_PLUS_PID, VirpilModel::RotorTcsPlus),
        (VIRPIL_ACE_TORQ_PID, VirpilModel::AceTorq),
    ];
    for (pid, expected_model) in &known_pids {
        assert!(
            is_virpil_device(VIRPIL_VENDOR_ID, *pid),
            "PID {pid:#06x} should be recognized as a Virpil device"
        );
        assert_eq!(
            virpil_model(*pid),
            Some(*expected_model),
            "PID {pid:#06x} model mismatch"
        );
    }
}

#[test]
fn report_format_report_size_validation_rejects_undersized() {
    // Each parser must reject reports shorter than minimum.
    assert!(parse_alpha_report(&[0x01; 14]).is_err());
    assert!(parse_cm3_throttle_report(&[0x01; 22]).is_err());
    assert!(parse_rotor_tcs_report(&[0x01; 10]).is_err());
    assert!(parse_ace_torq_report(&[0x01; 4]).is_err());
    assert!(parse_ace_pedals_report(&[0x01; 8]).is_err());
    assert!(parse_mongoost_stick_report(&[0x01; 14]).is_err());
    assert!(parse_warbrd_report(&[0x01; 14], WarBrdVariant::Original).is_err());
}

#[test]
fn report_format_all_device_table_entries_consistent_with_parsers() {
    // Every device_table entry's min_report_bytes should match the constant
    // exported by its parser module.
    let info_cm3 = device_info(VIRPIL_CM3_THROTTLE_PID).unwrap();
    assert_eq!(info_cm3.min_report_bytes, VPC_CM3_THROTTLE_MIN_REPORT_BYTES);

    let info_rotor = device_info(VIRPIL_ROTOR_TCS_PLUS_PID).unwrap();
    assert_eq!(info_rotor.min_report_bytes, VPC_ROTOR_TCS_MIN_REPORT_BYTES);

    let info_torq = device_info(VIRPIL_ACE_TORQ_PID).unwrap();
    assert_eq!(info_torq.min_report_bytes, VPC_ACE_TORQ_MIN_REPORT_BYTES);

    let info_pedals = device_info(VIRPIL_ACE_PEDALS_PID).unwrap();
    assert_eq!(info_pedals.min_report_bytes, VPC_ACE_PEDALS_MIN_REPORT_BYTES);
}

#[test]
fn report_format_axis_resolution_is_14_bit() {
    assert_eq!(AXIS_RESOLUTION_BITS, 14);
    assert_eq!(AXIS_MAX, 16384);
    assert_eq!(1u32 << AXIS_RESOLUTION_BITS, AXIS_MAX as u32);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. Grip capabilities tests (6)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn grip_alpha_has_5_axes_28_buttons_1_hat() {
    let info = device_info(VIRPIL_CONSTELLATION_ALPHA_LEFT_PID).unwrap();
    assert_eq!(info.axis_count, 5, "Alpha grip: 5 axes (X, Y, Z, SZ, SL)");
    assert_eq!(info.button_count, 28, "Alpha grip: 28 discrete buttons");

    // Profile also confirms hat
    assert_eq!(ALPHA_PROFILE.hats.len(), 1);
    assert_eq!(ALPHA_PROFILE.hats[0].hat_type, HatType::EightWay);
}

#[test]
fn grip_warbrd_shares_report_format_with_mongoost() {
    // WarBRD and WarBRD-D use the exact same 15-byte report format as MongoosT.
    let axes = [4096, 8192, 12288, 2048, 16000];
    let buttons = [0x55, 0xAA, 0x0F, 0x30]; // various buttons + hat
    let report = stick_report(axes, buttons);

    let mongoost = parse_mongoost_stick_report(&report).unwrap();
    let warbrd = parse_warbrd_report(&report, WarBrdVariant::Original).unwrap();
    let warbrd_d = parse_warbrd_report(&report, WarBrdVariant::D).unwrap();

    // Inner state must be identical
    assert_eq!(mongoost.axes, warbrd.inner.axes);
    assert_eq!(mongoost.buttons, warbrd.inner.buttons);
    assert_eq!(mongoost.axes, warbrd_d.inner.axes);

    // But variant differs
    assert_eq!(warbrd.variant, WarBrdVariant::Original);
    assert_eq!(warbrd_d.variant, WarBrdVariant::D);
}

#[test]
fn grip_alpha_ministick_analog_axes_sz_sl() {
    // SZ (secondary rotary / ministick X) and SL (slew lever / ministick Y)
    // are analog axes at indices 3 and 4.
    let axes = [0, 0, 0, 10000, 5000]; // only SZ and SL set
    let report = stick_report(axes, [0x00, 0x00, 0x00, 0xF0]);
    let state = parse_alpha_report(&report).unwrap();

    let expected_sz = 10000.0 / VIRPIL_AXIS_MAX as f32;
    let expected_sl = 5000.0 / VIRPIL_AXIS_MAX as f32;
    assert!((state.axes.sz - expected_sz).abs() < 1e-4);
    assert!((state.axes.sl - expected_sl).abs() < 1e-4);
    assert_eq!(state.axes.x, 0.0);
    assert_eq!(state.axes.y, 0.0);
}

#[test]
fn grip_hat_8_way_all_directions() {
    // Alpha hat supports full 8-way: N, NE, E, SE, S, SW, W, NW.
    let all_dirs = [
        VpcAlphaHat::North,
        VpcAlphaHat::NorthEast,
        VpcAlphaHat::East,
        VpcAlphaHat::SouthEast,
        VpcAlphaHat::South,
        VpcAlphaHat::SouthWest,
        VpcAlphaHat::West,
        VpcAlphaHat::NorthWest,
    ];
    for (i, expected_dir) in all_dirs.iter().enumerate() {
        let buttons = [0x00, 0x00, 0x00, (i as u8) << 4];
        let report = stick_report([8192; 5], buttons);
        let state = parse_alpha_report(&report).unwrap();
        assert_eq!(state.buttons.hat, *expected_dir, "direction index {i}");
    }
}

#[test]
fn grip_alpha_prime_left_right_variants() {
    let report = stick_report([8192; 5], [0x00, 0x00, 0x00, 0xF0]);
    let left = parse_alpha_prime_report(&report, AlphaPrimeVariant::Left).unwrap();
    let right = parse_alpha_prime_report(&report, AlphaPrimeVariant::Right).unwrap();

    assert_eq!(left.variant, AlphaPrimeVariant::Left);
    assert_eq!(right.variant, AlphaPrimeVariant::Right);
    // Same axes/buttons parsed from identical report
    assert_eq!(left.axes, right.axes);
    assert_eq!(left.buttons, right.buttons);
}

#[test]
fn grip_multiple_buttons_and_hat_simultaneous() {
    // Real-world: trigger (btn 1) + weapon release (btn 5) + hat NE
    let mut buttons = [0u8; 4];
    buttons[0] = 0x11; // bit 0 (btn 1) + bit 4 (btn 5)
    buttons[3] = 0x10; // hat NE = 1 in high nibble
    let report = stick_report([8192; 5], buttons);
    let state = parse_alpha_report(&report).unwrap();

    assert!(state.buttons.is_pressed(1));
    assert!(state.buttons.is_pressed(5));
    assert!(!state.buttons.is_pressed(2));
    assert_eq!(state.buttons.hat, VpcAlphaHat::NorthEast);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. Throttle tests (6)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn throttle_dual_axis_independent_range() {
    // Left at idle (0), right at full power (AXIS_MAX)
    let axes = [0, VIRPIL_AXIS_MAX, 0, 0, 0, 0];
    let report = throttle_cm3_report(axes, [0u8; 10]);
    let state = parse_cm3_throttle_report(&report).unwrap();

    assert_eq!(state.axes.left_throttle, 0.0, "left at idle");
    assert!((state.axes.right_throttle - 1.0).abs() < 1e-4, "right at full");
}

#[test]
fn throttle_detent_position_flaps_axis() {
    // Flaps lever at ~25% detent position (common take-off flap setting)
    let detent_raw = VIRPIL_AXIS_MAX / 4;
    let axes = [8192, 8192, detent_raw, 0, 0, 0];
    let report = throttle_cm3_report(axes, [0u8; 10]);
    let state = parse_cm3_throttle_report(&report).unwrap();

    let expected = detent_raw as f32 / VIRPIL_AXIS_MAX as f32;
    assert!(
        (state.axes.flaps - expected).abs() < 0.01,
        "flaps at 25% detent: expected ~{expected}, got {}",
        state.axes.flaps
    );
}

#[test]
fn throttle_encoder_direction_buttons() {
    // CM3 rotary encoders report as button pairs. Buttons in the upper range
    // represent CW/CCW encoder ticks. Test that individual encoder buttons
    // (high-numbered) are correctly decoded from the 10-byte button field.
    let mut buttons = [0u8; 10];
    // Encoder button 65: index 64 → byte 8, bit 0
    buttons[8] = 0x01;
    // Encoder button 66: index 65 → byte 8, bit 1
    buttons[8] |= 0x02;
    let report = throttle_cm3_report([0u16; 6], buttons);
    let state = parse_cm3_throttle_report(&report).unwrap();

    assert!(state.buttons.is_pressed(65), "encoder CW button");
    assert!(state.buttons.is_pressed(66), "encoder CCW button");
    assert!(!state.buttons.is_pressed(64));
    assert!(!state.buttons.is_pressed(67));
}

#[test]
fn throttle_mode_switch_buttons() {
    // Mode switch positions are regular buttons on the CM3.
    // Test a set of mode-switch buttons (e.g. buttons 30-34 area).
    let mut buttons = [0u8; 10];
    // Button 30 = index 29 → byte 3, bit 5
    buttons[3] = 1 << 5;
    let report = throttle_cm3_report([0u16; 6], buttons);
    let state = parse_cm3_throttle_report(&report).unwrap();

    assert!(state.buttons.is_pressed(30));
    assert!(!state.buttons.is_pressed(29));
    assert!(!state.buttons.is_pressed(31));
}

#[test]
fn throttle_idle_cutoff_at_zero() {
    // When both throttles are at 0, they're in idle cutoff position.
    let axes = [0, 0, 0, 8192, 8192, 0];
    let report = throttle_cm3_report(axes, [0u8; 10]);
    let state = parse_cm3_throttle_report(&report).unwrap();

    assert_eq!(state.axes.left_throttle, 0.0);
    assert_eq!(state.axes.right_throttle, 0.0);
    // Slew controls centred
    assert!((state.axes.scx - 0.5).abs() < 0.01);
    assert!((state.axes.scy - 0.5).abs() < 0.01);
}

#[test]
fn throttle_full_reverse_range_boundary() {
    // Some throttle setups use the low end of the axis range for reverse.
    // Verify that the lowest non-zero raw values produce small positive floats.
    let axes = [1, 1, 0, 0, 0, 0]; // raw=1 for both throttles
    let report = throttle_cm3_report(axes, [0u8; 10]);
    let state = parse_cm3_throttle_report(&report).unwrap();

    assert!(state.axes.left_throttle > 0.0, "raw=1 must be > 0.0");
    assert!(state.axes.left_throttle < 0.001, "raw=1 must be very small");
    assert!(state.axes.right_throttle > 0.0);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. Helicopter controls tests (5)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn heli_collective_full_range() {
    // Collective: 0.0 = full down, 1.0 = full up
    let report_down = rotor_tcs_report([0, 0, 0], [0u8; 4]);
    let report_up = rotor_tcs_report([VIRPIL_AXIS_MAX, 0, 0], [0u8; 4]);

    let state_down = parse_rotor_tcs_report(&report_down).unwrap();
    let state_up = parse_rotor_tcs_report(&report_up).unwrap();

    assert_eq!(state_down.axes.collective, 0.0, "collective full down");
    assert!((state_up.axes.collective - 1.0).abs() < 1e-4, "collective full up");

    // Midpoint hover
    let report_mid = rotor_tcs_report([VIRPIL_AXIS_MAX / 2, 0, 0], [0u8; 4]);
    let state_mid = parse_rotor_tcs_report(&report_mid).unwrap();
    assert!((state_mid.axes.collective - 0.5).abs() < 0.01, "collective hover mid");
}

#[test]
fn heli_twist_throttle_idle_cutoff() {
    // Throttle/idle cutoff axis at index 1
    let report_cutoff = rotor_tcs_report([8192, 0, 0], [0u8; 4]);
    let report_full = rotor_tcs_report([8192, VIRPIL_AXIS_MAX, 0], [0u8; 4]);

    let state_cutoff = parse_rotor_tcs_report(&report_cutoff).unwrap();
    let state_full = parse_rotor_tcs_report(&report_full).unwrap();

    assert_eq!(state_cutoff.axes.throttle_idle, 0.0, "idle cutoff");
    assert!((state_full.axes.throttle_idle - 1.0).abs() < 1e-4, "full throttle");
}

#[test]
fn heli_governor_rotary_axis() {
    // Rotary axis at index 2: used for governor/RPM control
    let report = rotor_tcs_report([8192, 8192, 12000], [0u8; 4]);
    let state = parse_rotor_tcs_report(&report).unwrap();

    let expected = 12000.0 / VIRPIL_AXIS_MAX as f32;
    assert!(
        (state.axes.rotary - expected).abs() < 1e-4,
        "rotary (governor) axis"
    );
}

#[test]
fn heli_rotor_brake_button() {
    // Rotor brake is a momentary button on the TCS.
    // Test button activation at various positions.
    let mut buttons = [0u8; 4];
    // Button 1 as rotor brake
    buttons[0] = 0x01;
    let report = rotor_tcs_report([8192, 8192, 0], buttons);
    let state = parse_rotor_tcs_report(&report).unwrap();
    assert!(state.buttons.is_pressed(1), "rotor brake button pressed");

    // All TCS buttons exercised
    let report_all = rotor_tcs_report([0u16; 3], [0xFF, 0xFF, 0xFF, 0x00]);
    let state_all = parse_rotor_tcs_report(&report_all).unwrap();
    for i in 1..=24 {
        assert!(state_all.buttons.is_pressed(i), "button {i}");
    }
}

#[test]
fn heli_collective_and_throttle_independent() {
    // Verify collective and throttle/idle cutoff are truly independent axes.
    let report = rotor_tcs_report([16000, 500, 8192], [0u8; 4]);
    let state = parse_rotor_tcs_report(&report).unwrap();

    let expected_collective = 16000.0 / VIRPIL_AXIS_MAX as f32;
    let expected_throttle = 500.0 / VIRPIL_AXIS_MAX as f32;
    let expected_rotary = 8192.0 / VIRPIL_AXIS_MAX as f32;

    assert!((state.axes.collective - expected_collective).abs() < 1e-4);
    assert!((state.axes.throttle_idle - expected_throttle).abs() < 1e-4);
    assert!((state.axes.rotary - expected_rotary).abs() < 1e-4);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. Profile tests (5)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn profile_dcs_per_device_axis_roles() {
    // Each device profile must have correct axis roles for DCS mapping.
    // Alpha: StickX, StickY, Twist + secondary
    assert_eq!(ALPHA_PROFILE.axes[0].role, AxisRole::StickX);
    assert_eq!(ALPHA_PROFILE.axes[1].role, AxisRole::StickY);
    assert_eq!(ALPHA_PROFILE.axes[2].role, AxisRole::Twist);

    // CM3: ThrottleLeft, ThrottleRight, Flaps, SlewX, SlewY, Slider
    assert_eq!(CM3_THROTTLE_PROFILE.axes[0].role, AxisRole::ThrottleLeft);
    assert_eq!(CM3_THROTTLE_PROFILE.axes[1].role, AxisRole::ThrottleRight);
    assert_eq!(CM3_THROTTLE_PROFILE.axes[2].role, AxisRole::Flaps);
    assert_eq!(CM3_THROTTLE_PROFILE.axes[3].role, AxisRole::SlewX);
    assert_eq!(CM3_THROTTLE_PROFILE.axes[4].role, AxisRole::SlewY);
    assert_eq!(CM3_THROTTLE_PROFILE.axes[5].role, AxisRole::Slider);

    // Rotor TCS: Collective, ThrottleIdleCutoff, Rotary
    assert_eq!(ROTOR_TCS_PROFILE.axes[0].role, AxisRole::Collective);
    assert_eq!(ROTOR_TCS_PROFILE.axes[1].role, AxisRole::ThrottleIdleCutoff);
    assert_eq!(ROTOR_TCS_PROFILE.axes[2].role, AxisRole::Rotary);
}

#[test]
fn profile_combined_hotas_pedals_setup() {
    // A full HOTAS+pedals setup: Alpha grip + CM3 throttle + ACE pedals.
    // Verify all three profiles exist and are independently addressable by PID.
    let grip = profile_for_pid(VIRPIL_CONSTELLATION_ALPHA_LEFT_PID).unwrap();
    let throttle = profile_for_pid(VIRPIL_CM3_THROTTLE_PID).unwrap();
    let pedals = profile_for_pid(VIRPIL_ACE_PEDALS_PID).unwrap();

    // Total axes across the setup
    let total_axes = grip.axes.len() + throttle.axes.len() + pedals.axes.len();
    assert_eq!(total_axes, 5 + 6 + 3, "14 axes in full HOTAS+pedals setup");

    // Total buttons
    let total_buttons =
        grip.button_count as u32 + throttle.button_count as u32 + pedals.button_count as u32;
    assert_eq!(total_buttons, 28 + 78 + 16, "122 buttons in full setup");

    // Each has a distinct PID
    assert_ne!(grip.pid, throttle.pid);
    assert_ne!(grip.pid, pedals.pid);
    assert_ne!(throttle.pid, pedals.pid);
}

#[test]
fn profile_shift_layers_button_count_capacity() {
    // Shift layers multiply effective buttons. Verify each profile has enough
    // button count to support at least 2 shift layers worth of unique bindings.
    for profile in ALL_PROFILES {
        // With 2 shift layers, effective buttons = button_count * 3
        // (unshifted + shift1 + shift2). The profile.button_count is the
        // hardware count; software layers are multiplicative.
        assert!(
            profile.button_count >= 8,
            "{}: must have ≥8 buttons for meaningful shift layers",
            profile.name
        );
    }
}

#[test]
fn profile_axis_centering_semantic_correctness() {
    // Stick axes should be centred, throttle/brake axes should not.
    for axis in ALPHA_PROFILE.axes {
        match axis.role {
            AxisRole::StickX | AxisRole::StickY | AxisRole::Twist => {
                assert!(axis.centred, "{}: {} should be centred", ALPHA_PROFILE.name, axis.label);
            }
            _ => {}
        }
    }

    for axis in CM3_THROTTLE_PROFILE.axes {
        match axis.role {
            AxisRole::ThrottleLeft | AxisRole::ThrottleRight | AxisRole::Flaps | AxisRole::Slider => {
                assert!(!axis.centred, "{}: {} should NOT be centred", CM3_THROTTLE_PROFILE.name, axis.label);
            }
            AxisRole::SlewX | AxisRole::SlewY => {
                assert!(axis.centred, "{}: {} should be centred", CM3_THROTTLE_PROFILE.name, axis.label);
            }
            _ => {}
        }
    }

    for axis in ACE_PEDALS_PROFILE.axes {
        match axis.role {
            AxisRole::Rudder => assert!(axis.centred, "rudder should be centred"),
            AxisRole::LeftToeBrake | AxisRole::RightToeBrake => {
                assert!(!axis.centred, "toe brakes should not be centred");
            }
            _ => {}
        }
    }
}

#[test]
fn profile_heli_setup_rotor_tcs_plus_ace_torq() {
    // Helicopter setup: Rotor TCS (collective) + ACE Torq (throttle quadrant)
    let collective_profile = profile_for_pid(VIRPIL_ROTOR_TCS_PLUS_PID).unwrap();
    let torq_profile = profile_for_pid(VIRPIL_ACE_TORQ_PID).unwrap();

    // Rotor TCS has collective + throttle idle cutoff + rotary
    assert_eq!(collective_profile.axes.len(), 3);
    assert_eq!(collective_profile.axes[0].role, AxisRole::Collective);
    assert_eq!(collective_profile.rotary_encoders, 1);

    // ACE Torq has single throttle axis
    assert_eq!(torq_profile.axes.len(), 1);
    assert_eq!(torq_profile.axes[0].role, AxisRole::Throttle);

    // Combined setup
    let total_axes = collective_profile.axes.len() + torq_profile.axes.len();
    assert_eq!(total_axes, 4, "4 axes in heli setup");
    let total_buttons =
        collective_profile.button_count as u32 + torq_profile.button_count as u32;
    assert_eq!(total_buttons, 24 + 8, "32 buttons in heli setup");
}

// ═══════════════════════════════════════════════════════════════════════════════
// Additional depth tests to reach 30+ total
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn vid_is_0x3344() {
    assert_eq!(VIRPIL_VENDOR_ID, 0x3344, "VIRPIL Controls USB Vendor ID");
}

#[test]
fn normalize_denormalize_roundtrip_at_boundaries() {
    // 0 → 0.0 → 0
    assert_eq!(denormalize_axis(normalize_axis(0)), 0);
    // AXIS_MAX → 1.0 → AXIS_MAX
    assert_eq!(denormalize_axis(normalize_axis(AXIS_MAX)), AXIS_MAX);
    // Midpoint roundtrip within ±1 LSB
    let mid = AXIS_MAX / 2;
    let back = denormalize_axis(normalize_axis(mid));
    assert!((mid as i32 - back as i32).unsigned_abs() <= 1);
}

#[test]
fn led_report_structure() {
    let report = build_led_report(7, LedColor::new(0x10, 0x20, 0x30));
    assert_eq!(report[0], LED_REPORT_ID, "LED report_id = 0x02");
    assert_eq!(report[1], 7, "LED index");
    assert_eq!(report[2], 0x10, "red channel");
    assert_eq!(report[3], 0x20, "green channel");
    assert_eq!(report[4], 0x30, "blue channel");
    assert_eq!(report.len(), LED_REPORT_SIZE);
}

#[test]
fn device_table_covers_all_major_product_families() {
    // Sticks, throttles, panels, pedals, heli — all represented
    let has_stick = DEVICE_TABLE.iter().any(|d| d.model == VirpilModel::MongoostStick);
    let has_alpha = DEVICE_TABLE.iter().any(|d| d.model == VirpilModel::ConstellationAlphaLeft);
    let has_throttle = DEVICE_TABLE.iter().any(|d| d.model == VirpilModel::Cm3Throttle);
    let has_pedals = DEVICE_TABLE.iter().any(|d| d.model == VirpilModel::AcePedals);
    let has_heli = DEVICE_TABLE.iter().any(|d| d.model == VirpilModel::RotorTcsPlus);
    let has_panel = DEVICE_TABLE.iter().any(|d| d.model == VirpilModel::ControlPanel1);

    assert!(has_stick, "MongoosT stick in table");
    assert!(has_alpha, "Constellation Alpha in table");
    assert!(has_throttle, "CM3 Throttle in table");
    assert!(has_pedals, "ACE Pedals in table");
    assert!(has_heli, "Rotor TCS in table");
    assert!(has_panel, "Control Panel in table");
}

#[test]
fn ace_torq_single_axis_throttle_protocol() {
    // ACE Torq is the simplest Virpil device: 1 axis + 8 buttons in 5 bytes.
    let report = ace_torq_report(8000, [0xFF, 0x00]);
    let state = parse_ace_torq_report(&report).unwrap();

    let expected = 8000.0 / VIRPIL_AXIS_MAX as f32;
    assert!((state.axis.throttle - expected).abs() < 1e-4);
    // All 8 buttons pressed in first byte
    for i in 1..=8 {
        assert!(state.buttons.is_pressed(i), "btn {i}");
    }
}

#[test]
fn ace_pedals_rudder_and_brakes_protocol() {
    // ACE Pedals: rudder at center, left brake half, right brake full
    let axes = [VIRPIL_AXIS_MAX / 2, VIRPIL_AXIS_MAX / 2, VIRPIL_AXIS_MAX];
    let report = ace_pedals_report(axes, [0u8; 2]);
    let state = parse_ace_pedals_report(&report).unwrap();

    assert!((state.axes.rudder - 0.5).abs() < 0.01, "rudder centred");
    assert!((state.axes.left_toe_brake - 0.5).abs() < 0.01, "left brake half");
    assert!((state.axes.right_toe_brake - 1.0).abs() < 1e-4, "right brake full");
}
