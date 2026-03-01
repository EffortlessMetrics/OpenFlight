// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the `flight-hotas-virpil` crate.
//!
//! These tests exercise cross-device invariants, protocol-level edge cases,
//! profile consistency, LED report encoding, axis normalisation boundaries,
//! button bitmask correctness, and device-table/profile agreement.

use flight_hotas_virpil::{
    // ─── Button counts ───────────────────────────────────────────────────
    ACE_PEDALS_BUTTON_COUNT,
    ACE_TORQ_BUTTON_COUNT,
    // ─── Variants / types ────────────────────────────────────────────────
    AlphaPrimeVariant,
    PANEL2_BUTTON_COUNT,
    ROTOR_TCS_BUTTON_COUNT,
    // ─── Device table PIDs ───────────────────────────────────────────────
    VIRPIL_ACE_PEDALS_PID,
    VIRPIL_ACE_TORQ_PID,
    // ─── Constants ────────────────────────────────────────────────────────
    VIRPIL_AXIS_MAX,
    VIRPIL_CM3_THROTTLE_PID,
    VIRPIL_CONSTELLATION_ALPHA_LEFT_PID,
    VIRPIL_CONSTELLATION_ALPHA_PRIME_LEFT_PID,
    VIRPIL_CONSTELLATION_ALPHA_PRIME_RIGHT_PID,
    VIRPIL_MONGOOST_STICK_PID,
    VIRPIL_PANEL1_PID,
    VIRPIL_PANEL2_PID,
    VIRPIL_ROTOR_TCS_PLUS_PID,
    VIRPIL_VENDOR_ID,
    VIRPIL_WARBRD_D_PID,
    VIRPIL_WARBRD_PID,
    // ─── Min report sizes ────────────────────────────────────────────────
    VPC_ACE_PEDALS_MIN_REPORT_BYTES,
    VPC_ACE_TORQ_MIN_REPORT_BYTES,
    VPC_ALPHA_MIN_REPORT_BYTES,
    VPC_ALPHA_PRIME_MIN_REPORT_BYTES,
    VPC_CM3_THROTTLE_MIN_REPORT_BYTES,
    VPC_MONGOOST_STICK_MIN_REPORT_BYTES,
    VPC_PANEL1_MIN_REPORT_BYTES,
    VPC_PANEL2_MIN_REPORT_BYTES,
    VPC_ROTOR_TCS_MIN_REPORT_BYTES,
    VPC_WARBRD_MIN_REPORT_BYTES,
    VirpilModel,
    VpcAlphaHat,
    VpcMongoostHat,
    WarBrdVariant,
    is_virpil_device,
    // ─── Parsers ─────────────────────────────────────────────────────────
    parse_ace_pedals_report,
    parse_ace_torq_report,
    parse_alpha_prime_report,
    parse_alpha_report,
    parse_cm3_throttle_report,
    parse_mongoost_stick_report,
    parse_panel1_report,
    parse_panel2_report,
    parse_rotor_tcs_report,
    parse_warbrd_report,
    // ─── Profiles ────────────────────────────────────────────────────────
    profiles::{
        ACE_PEDALS_PROFILE, ACE_TORQ_PROFILE, ALL_PROFILES, ALPHA_PROFILE, AxisRole,
        CM3_THROTTLE_PROFILE, HatType, ROTOR_TCS_PROFILE, profile_for_pid,
    },
    // ─── Protocol ────────────────────────────────────────────────────────
    protocol::{
        AXIS_MAX, AXIS_RESOLUTION_BITS, DEVICE_TABLE, INPUT_REPORT_ID, LED_REPORT_ID,
        LED_REPORT_SIZE, LedColor, build_led_report, denormalize_axis, device_info, normalize_axis,
    },
    virpil_model,
};

// ═════════════════════════════════════════════════════════════════════════════
// Report builders (shared helpers)
// ═════════════════════════════════════════════════════════════════════════════

fn make_5ax_report(axes: [u16; 5], buttons: [u8; 4]) -> Vec<u8> {
    let mut data = vec![0x01u8];
    for ax in &axes {
        data.extend_from_slice(&ax.to_le_bytes());
    }
    data.extend_from_slice(&buttons);
    data
}

fn make_cm3_report(axes: [u16; 6], buttons: [u8; 10]) -> Vec<u8> {
    let mut data = vec![0x01u8];
    for ax in &axes {
        data.extend_from_slice(&ax.to_le_bytes());
    }
    data.extend_from_slice(&buttons);
    data
}

fn make_panel1_report(buttons: [u8; 6]) -> Vec<u8> {
    let mut data = vec![0x01u8];
    data.extend_from_slice(&buttons);
    data
}

fn make_panel2_report(a1: u16, a2: u16, buttons: [u8; 6]) -> Vec<u8> {
    let mut data = vec![0x01u8];
    data.extend_from_slice(&a1.to_le_bytes());
    data.extend_from_slice(&a2.to_le_bytes());
    data.extend_from_slice(&buttons);
    data
}

fn make_pedals_report(axes: [u16; 3], buttons: [u8; 2]) -> Vec<u8> {
    let mut data = vec![0x01u8];
    for ax in &axes {
        data.extend_from_slice(&ax.to_le_bytes());
    }
    data.extend_from_slice(&buttons);
    data
}

fn make_rotor_tcs_report(axes: [u16; 3], buttons: [u8; 4]) -> Vec<u8> {
    let mut data = vec![0x01u8];
    for ax in &axes {
        data.extend_from_slice(&ax.to_le_bytes());
    }
    data.extend_from_slice(&buttons);
    data
}

fn make_ace_torq_report(throttle: u16, buttons: [u8; 2]) -> Vec<u8> {
    let mut data = vec![0x01u8];
    data.extend_from_slice(&throttle.to_le_bytes());
    data.extend_from_slice(&buttons);
    data
}

// ═════════════════════════════════════════════════════════════════════════════
// 1. Cross-device VID/PID integrity
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn vendor_id_matches_virpil_controls() {
    assert_eq!(VIRPIL_VENDOR_ID, 0x3344, "VIRPIL Controls UAB VID");
}

#[test]
fn all_known_pids_recognised_by_is_virpil_device() {
    let pids = [
        VIRPIL_MONGOOST_STICK_PID,
        VIRPIL_CONSTELLATION_ALPHA_LEFT_PID,
        VIRPIL_CONSTELLATION_ALPHA_PRIME_LEFT_PID,
        VIRPIL_CONSTELLATION_ALPHA_PRIME_RIGHT_PID,
        VIRPIL_WARBRD_PID,
        VIRPIL_WARBRD_D_PID,
        VIRPIL_CM3_THROTTLE_PID,
        VIRPIL_PANEL1_PID,
        VIRPIL_PANEL2_PID,
        VIRPIL_ACE_TORQ_PID,
        VIRPIL_ACE_PEDALS_PID,
        VIRPIL_ROTOR_TCS_PLUS_PID,
    ];
    for pid in pids {
        assert!(
            is_virpil_device(VIRPIL_VENDOR_ID, pid),
            "PID 0x{pid:04X} should be recognised"
        );
    }
}

#[test]
fn unknown_pid_not_recognised() {
    assert!(!is_virpil_device(VIRPIL_VENDOR_ID, 0xFFFF));
}

#[test]
fn wrong_vid_not_recognised() {
    assert!(!is_virpil_device(0x0000, VIRPIL_CM3_THROTTLE_PID));
}

#[test]
fn virpil_model_returns_some_for_all_known_pids() {
    let pids_and_models = [
        (VIRPIL_MONGOOST_STICK_PID, VirpilModel::MongoostStick),
        (VIRPIL_CM3_THROTTLE_PID, VirpilModel::Cm3Throttle),
        (VIRPIL_PANEL1_PID, VirpilModel::ControlPanel1),
        (VIRPIL_PANEL2_PID, VirpilModel::ControlPanel2),
        (VIRPIL_WARBRD_PID, VirpilModel::WarBrd),
        (VIRPIL_WARBRD_D_PID, VirpilModel::WarBrdD),
        (VIRPIL_ACE_TORQ_PID, VirpilModel::AceTorq),
        (VIRPIL_ACE_PEDALS_PID, VirpilModel::AcePedals),
        (VIRPIL_ROTOR_TCS_PLUS_PID, VirpilModel::RotorTcsPlus),
        (
            VIRPIL_CONSTELLATION_ALPHA_LEFT_PID,
            VirpilModel::ConstellationAlphaLeft,
        ),
        (
            VIRPIL_CONSTELLATION_ALPHA_PRIME_LEFT_PID,
            VirpilModel::ConstellationAlphaPrimeLeft,
        ),
        (
            VIRPIL_CONSTELLATION_ALPHA_PRIME_RIGHT_PID,
            VirpilModel::ConstellationAlphaPrimeRight,
        ),
    ];
    for (pid, expected_model) in pids_and_models {
        assert_eq!(
            virpil_model(pid),
            Some(expected_model),
            "PID 0x{pid:04X} model mismatch"
        );
    }
}

#[test]
fn virpil_model_returns_none_for_unknown_pid() {
    assert_eq!(virpil_model(0xBEEF), None);
}

// ═════════════════════════════════════════════════════════════════════════════
// 2. Device table consistency
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn device_table_pids_are_all_unique() {
    let mut pids: Vec<u16> = DEVICE_TABLE.iter().map(|d| d.pid).collect();
    pids.sort_unstable();
    let deduped_len = {
        pids.dedup();
        pids.len()
    };
    assert_eq!(deduped_len, DEVICE_TABLE.len());
}

#[test]
fn device_table_all_entries_have_nonzero_min_report_bytes() {
    for entry in DEVICE_TABLE {
        assert!(
            entry.min_report_bytes > 1,
            "{}: min_report_bytes must be > 1 (at least report_id + data)",
            entry.name
        );
    }
}

#[test]
fn device_table_report_size_agrees_with_module_constants() {
    assert_eq!(
        device_info(VIRPIL_MONGOOST_STICK_PID)
            .unwrap()
            .min_report_bytes,
        VPC_MONGOOST_STICK_MIN_REPORT_BYTES
    );
    assert_eq!(
        device_info(VIRPIL_CM3_THROTTLE_PID)
            .unwrap()
            .min_report_bytes,
        VPC_CM3_THROTTLE_MIN_REPORT_BYTES
    );
    assert_eq!(
        device_info(VIRPIL_PANEL1_PID).unwrap().min_report_bytes,
        VPC_PANEL1_MIN_REPORT_BYTES
    );
    assert_eq!(
        device_info(VIRPIL_PANEL2_PID).unwrap().min_report_bytes,
        VPC_PANEL2_MIN_REPORT_BYTES
    );
    assert_eq!(
        device_info(VIRPIL_ACE_PEDALS_PID).unwrap().min_report_bytes,
        VPC_ACE_PEDALS_MIN_REPORT_BYTES
    );
    assert_eq!(
        device_info(VIRPIL_ROTOR_TCS_PLUS_PID)
            .unwrap()
            .min_report_bytes,
        VPC_ROTOR_TCS_MIN_REPORT_BYTES
    );
    assert_eq!(
        device_info(VIRPIL_ACE_TORQ_PID).unwrap().min_report_bytes,
        VPC_ACE_TORQ_MIN_REPORT_BYTES
    );
}

#[test]
fn device_table_button_counts_agree_with_module_constants() {
    assert_eq!(
        device_info(VIRPIL_ACE_PEDALS_PID).unwrap().button_count,
        ACE_PEDALS_BUTTON_COUNT
    );
    assert_eq!(
        device_info(VIRPIL_ROTOR_TCS_PLUS_PID).unwrap().button_count,
        ROTOR_TCS_BUTTON_COUNT
    );
    assert_eq!(
        device_info(VIRPIL_ACE_TORQ_PID).unwrap().button_count,
        ACE_TORQ_BUTTON_COUNT
    );
    assert_eq!(
        device_info(VIRPIL_PANEL2_PID).unwrap().button_count,
        PANEL2_BUTTON_COUNT
    );
}

#[test]
fn device_table_axis_count_fits_report_size() {
    for entry in DEVICE_TABLE {
        // Report layout: 1 (report_id) + axis_count*2 + ceil(button_count/8)
        let min_needed =
            1 + (entry.axis_count as usize) * 2 + (entry.button_count as usize).div_ceil(8);
        assert!(
            entry.min_report_bytes >= min_needed,
            "{}: min_report_bytes {} < needed {} (axes={}, buttons={})",
            entry.name,
            entry.min_report_bytes,
            min_needed,
            entry.axis_count,
            entry.button_count
        );
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// 3. Protocol constants and axis normalisation
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn axis_max_equals_virpil_axis_max() {
    assert_eq!(AXIS_MAX, VIRPIL_AXIS_MAX);
}

#[test]
fn axis_resolution_bits_power_of_two_matches_max() {
    assert_eq!(1u32 << AXIS_RESOLUTION_BITS, AXIS_MAX as u32);
}

#[test]
fn normalize_axis_boundary_values() {
    assert_eq!(normalize_axis(0), 0.0);
    assert!((normalize_axis(AXIS_MAX) - 1.0).abs() < 1e-6);
    assert_eq!(normalize_axis(u16::MAX), 1.0); // clamped
}

#[test]
fn denormalize_axis_boundary_values() {
    assert_eq!(denormalize_axis(0.0), 0);
    assert_eq!(denormalize_axis(1.0), AXIS_MAX);
    assert_eq!(denormalize_axis(-1.0), 0);
    assert_eq!(denormalize_axis(2.0), AXIS_MAX);
}

#[test]
fn normalize_denormalize_roundtrip_representative_values() {
    for raw in [0u16, 1, 100, 4096, 8192, 12288, 16383, AXIS_MAX] {
        let norm = normalize_axis(raw);
        let back = denormalize_axis(norm);
        assert!(
            (raw as i32 - back as i32).unsigned_abs() <= 1,
            "roundtrip failed for raw={raw}: norm={norm}, back={back}"
        );
    }
}

#[test]
fn normalize_axis_quarter_points() {
    let quarter = AXIS_MAX / 4;
    let n = normalize_axis(quarter);
    assert!((n - 0.25).abs() < 0.01, "quarter = {n}");
    let three_q = (AXIS_MAX as u32 * 3 / 4) as u16;
    let n3 = normalize_axis(three_q);
    assert!((n3 - 0.75).abs() < 0.01, "three-quarter = {n3}");
}

#[test]
fn input_report_id_constant() {
    assert_eq!(INPUT_REPORT_ID, 0x01);
}

#[test]
fn led_report_id_constant() {
    assert_eq!(LED_REPORT_ID, 0x02);
}

// ═════════════════════════════════════════════════════════════════════════════
// 4. LED report building
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn led_report_size_is_five() {
    assert_eq!(LED_REPORT_SIZE, 5);
}

#[test]
fn led_report_encodes_correctly() {
    let buf = build_led_report(7, LedColor::new(0x11, 0x22, 0x33));
    assert_eq!(buf, [0x02, 7, 0x11, 0x22, 0x33]);
}

#[test]
fn led_report_off_is_all_zero_rgb() {
    let buf = build_led_report(0, LedColor::OFF);
    assert_eq!(buf, [0x02, 0, 0, 0, 0]);
}

#[test]
fn led_report_white_is_all_ff() {
    let buf = build_led_report(0, LedColor::WHITE);
    assert_eq!(buf, [0x02, 0, 0xFF, 0xFF, 0xFF]);
}

#[test]
fn led_report_preset_colors() {
    assert_eq!(LedColor::RED, LedColor::new(0xFF, 0, 0));
    assert_eq!(LedColor::GREEN, LedColor::new(0, 0xFF, 0));
    assert_eq!(LedColor::BLUE, LedColor::new(0, 0, 0xFF));
}

#[test]
fn led_report_max_led_index() {
    let buf = build_led_report(u8::MAX, LedColor::RED);
    assert_eq!(buf[1], u8::MAX);
}

#[test]
fn led_color_default_is_off() {
    assert_eq!(LedColor::default(), LedColor::OFF);
}

// ═════════════════════════════════════════════════════════════════════════════
// 5. Profile consistency
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn all_profiles_have_unique_pids() {
    let mut pids: Vec<u16> = ALL_PROFILES.iter().map(|p| p.pid).collect();
    pids.sort_unstable();
    pids.dedup();
    assert_eq!(pids.len(), ALL_PROFILES.len());
}

#[test]
fn profile_axis_indices_are_sequential_zero_based() {
    for profile in ALL_PROFILES {
        for (i, axis) in profile.axes.iter().enumerate() {
            assert_eq!(
                axis.index as usize, i,
                "{}: axis {} has non-sequential index {}",
                profile.name, i, axis.index
            );
        }
    }
}

#[test]
fn profile_for_pid_returns_correct_names() {
    let cases: &[(u16, &str)] = &[
        (ALPHA_PROFILE.pid, "VPC Constellation Alpha"),
        (CM3_THROTTLE_PROFILE.pid, "VPC Throttle CM3"),
        (ACE_PEDALS_PROFILE.pid, "VPC ACE Collection Pedals"),
        (ROTOR_TCS_PROFILE.pid, "VPC Rotor TCS Plus"),
        (ACE_TORQ_PROFILE.pid, "VPC ACE Torq"),
    ];
    for &(pid, expected_name) in cases {
        let p = profile_for_pid(pid).expect("profile should exist");
        assert_eq!(p.name, expected_name);
    }
}

#[test]
fn profile_for_pid_unknown_returns_none() {
    assert!(profile_for_pid(0xDEAD).is_none());
}

#[test]
fn alpha_profile_stick_axes_centred_throttle_axes_not() {
    let alpha = &ALPHA_PROFILE;
    // X, Y, Twist should be centred
    assert!(alpha.axes[0].centred);
    assert!(alpha.axes[1].centred);
    assert!(alpha.axes[2].centred);
    // Secondary rotary and slew lever are not centred
    assert!(!alpha.axes[3].centred);
    assert!(!alpha.axes[4].centred);
}

#[test]
fn cm3_throttle_profile_roles() {
    let cm3 = &CM3_THROTTLE_PROFILE;
    assert_eq!(cm3.axes[0].role, AxisRole::ThrottleLeft);
    assert_eq!(cm3.axes[1].role, AxisRole::ThrottleRight);
    assert_eq!(cm3.axes[2].role, AxisRole::Flaps);
    assert_eq!(cm3.axes[3].role, AxisRole::SlewX);
    assert_eq!(cm3.axes[4].role, AxisRole::SlewY);
    assert_eq!(cm3.axes[5].role, AxisRole::Slider);
}

#[test]
fn ace_pedals_profile_roles() {
    let pedals = &ACE_PEDALS_PROFILE;
    assert_eq!(pedals.axes[0].role, AxisRole::Rudder);
    assert_eq!(pedals.axes[1].role, AxisRole::LeftToeBrake);
    assert_eq!(pedals.axes[2].role, AxisRole::RightToeBrake);
}

#[test]
fn rotor_tcs_profile_has_collective_role() {
    assert_eq!(ROTOR_TCS_PROFILE.axes[0].role, AxisRole::Collective);
}

#[test]
fn ace_torq_profile_single_throttle() {
    assert_eq!(ACE_TORQ_PROFILE.axes.len(), 1);
    assert_eq!(ACE_TORQ_PROFILE.axes[0].role, AxisRole::Throttle);
    assert!(!ACE_TORQ_PROFILE.axes[0].centred);
}

#[test]
fn alpha_profile_has_eight_way_hat() {
    assert_eq!(ALPHA_PROFILE.hats.len(), 1);
    assert_eq!(ALPHA_PROFILE.hats[0].hat_type, HatType::EightWay);
}

#[test]
fn cm3_throttle_has_no_hats() {
    assert!(CM3_THROTTLE_PROFILE.hats.is_empty());
}

#[test]
fn cm3_throttle_has_four_rotary_encoders() {
    assert_eq!(CM3_THROTTLE_PROFILE.rotary_encoders, 4);
}

// ═════════════════════════════════════════════════════════════════════════════
// 6. Cross-parser: every parser rejects empty input
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn all_parsers_reject_empty_input() {
    let empty: &[u8] = &[];
    assert!(parse_alpha_report(empty).is_err());
    assert!(parse_alpha_prime_report(empty, AlphaPrimeVariant::Left).is_err());
    assert!(parse_mongoost_stick_report(empty).is_err());
    assert!(parse_warbrd_report(empty, WarBrdVariant::D).is_err());
    assert!(parse_cm3_throttle_report(empty).is_err());
    assert!(parse_panel1_report(empty).is_err());
    assert!(parse_panel2_report(empty).is_err());
    assert!(parse_ace_pedals_report(empty).is_err());
    assert!(parse_rotor_tcs_report(empty).is_err());
    assert!(parse_ace_torq_report(empty).is_err());
}

// ═════════════════════════════════════════════════════════════════════════════
// 7. Cross-parser: every parser rejects one-byte-short input
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn all_parsers_reject_one_byte_short() {
    assert!(parse_alpha_report(&[0x01; VPC_ALPHA_MIN_REPORT_BYTES - 1]).is_err());
    assert!(
        parse_alpha_prime_report(
            &[0x01; VPC_ALPHA_PRIME_MIN_REPORT_BYTES - 1],
            AlphaPrimeVariant::Right
        )
        .is_err()
    );
    assert!(parse_mongoost_stick_report(&[0x01; VPC_MONGOOST_STICK_MIN_REPORT_BYTES - 1]).is_err());
    assert!(
        parse_warbrd_report(&[0x01; VPC_WARBRD_MIN_REPORT_BYTES - 1], WarBrdVariant::D).is_err()
    );
    assert!(parse_cm3_throttle_report(&[0x01; VPC_CM3_THROTTLE_MIN_REPORT_BYTES - 1]).is_err());
    assert!(parse_panel1_report(&[0x01; VPC_PANEL1_MIN_REPORT_BYTES - 1]).is_err());
    assert!(parse_panel2_report(&[0x01; VPC_PANEL2_MIN_REPORT_BYTES - 1]).is_err());
    assert!(parse_ace_pedals_report(&[0x01; VPC_ACE_PEDALS_MIN_REPORT_BYTES - 1]).is_err());
    assert!(parse_rotor_tcs_report(&[0x01; VPC_ROTOR_TCS_MIN_REPORT_BYTES - 1]).is_err());
    assert!(parse_ace_torq_report(&[0x01; VPC_ACE_TORQ_MIN_REPORT_BYTES - 1]).is_err());
}

// ═════════════════════════════════════════════════════════════════════════════
// 8. Cross-parser: every parser accepts exact minimum length
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn all_parsers_accept_exact_min_length() {
    assert!(parse_alpha_report(&make_5ax_report([0; 5], [0; 4])).is_ok());
    assert!(
        parse_alpha_prime_report(&make_5ax_report([0; 5], [0; 4]), AlphaPrimeVariant::Left).is_ok()
    );
    assert!(parse_mongoost_stick_report(&make_5ax_report([0; 5], [0; 4])).is_ok());
    assert!(parse_warbrd_report(&make_5ax_report([0; 5], [0; 4]), WarBrdVariant::Original).is_ok());
    assert!(parse_cm3_throttle_report(&make_cm3_report([0; 6], [0; 10])).is_ok());
    assert!(parse_panel1_report(&make_panel1_report([0; 6])).is_ok());
    assert!(parse_panel2_report(&make_panel2_report(0, 0, [0; 6])).is_ok());
    assert!(parse_ace_pedals_report(&make_pedals_report([0; 3], [0; 2])).is_ok());
    assert!(parse_rotor_tcs_report(&make_rotor_tcs_report([0; 3], [0; 4])).is_ok());
    assert!(parse_ace_torq_report(&make_ace_torq_report(0, [0; 2])).is_ok());
}

// ═════════════════════════════════════════════════════════════════════════════
// 9. Cross-parser: all parsers accept over-length reports
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn all_parsers_accept_oversized_reports() {
    let pad = [0xAAu8; 32];

    let mut r = make_5ax_report([0; 5], [0; 4]);
    r.extend_from_slice(&pad);
    assert!(parse_alpha_report(&r).is_ok());
    assert!(parse_mongoost_stick_report(&r).is_ok());
    assert!(parse_warbrd_report(&r, WarBrdVariant::D).is_ok());
    assert!(parse_alpha_prime_report(&r, AlphaPrimeVariant::Right).is_ok());

    let mut r = make_cm3_report([0; 6], [0; 10]);
    r.extend_from_slice(&pad);
    assert!(parse_cm3_throttle_report(&r).is_ok());

    let mut r = make_panel1_report([0; 6]);
    r.extend_from_slice(&pad);
    assert!(parse_panel1_report(&r).is_ok());

    let mut r = make_panel2_report(0, 0, [0; 6]);
    r.extend_from_slice(&pad);
    assert!(parse_panel2_report(&r).is_ok());

    let mut r = make_pedals_report([0; 3], [0; 2]);
    r.extend_from_slice(&pad);
    assert!(parse_ace_pedals_report(&r).is_ok());

    let mut r = make_rotor_tcs_report([0; 3], [0; 4]);
    r.extend_from_slice(&pad);
    assert!(parse_rotor_tcs_report(&r).is_ok());

    let mut r = make_ace_torq_report(0, [0; 2]);
    r.extend_from_slice(&pad);
    assert!(parse_ace_torq_report(&r).is_ok());
}

// ═════════════════════════════════════════════════════════════════════════════
// 10. Axis clamping: above AXIS_MAX saturates to 1.0
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn alpha_axis_above_max_clamps_to_one() {
    let report = make_5ax_report([u16::MAX; 5], [0; 4]);
    let s = parse_alpha_report(&report).unwrap();
    assert_eq!(s.axes.x, 1.0);
    assert_eq!(s.axes.y, 1.0);
    assert_eq!(s.axes.z, 1.0);
    assert_eq!(s.axes.sz, 1.0);
    assert_eq!(s.axes.sl, 1.0);
}

#[test]
fn cm3_axis_above_max_clamps_to_one() {
    let report = make_cm3_report([u16::MAX; 6], [0; 10]);
    let s = parse_cm3_throttle_report(&report).unwrap();
    assert_eq!(s.axes.left_throttle, 1.0);
    assert_eq!(s.axes.right_throttle, 1.0);
    assert_eq!(s.axes.flaps, 1.0);
    assert_eq!(s.axes.scx, 1.0);
    assert_eq!(s.axes.scy, 1.0);
    assert_eq!(s.axes.slider, 1.0);
}

#[test]
fn pedals_axis_above_max_clamps_to_one() {
    let report = make_pedals_report([u16::MAX; 3], [0; 2]);
    let s = parse_ace_pedals_report(&report).unwrap();
    assert_eq!(s.axes.rudder, 1.0);
    assert_eq!(s.axes.left_toe_brake, 1.0);
    assert_eq!(s.axes.right_toe_brake, 1.0);
}

#[test]
fn rotor_tcs_axis_above_max_clamps_to_one() {
    let report = make_rotor_tcs_report([u16::MAX; 3], [0; 4]);
    let s = parse_rotor_tcs_report(&report).unwrap();
    assert_eq!(s.axes.collective, 1.0);
    assert_eq!(s.axes.throttle_idle, 1.0);
    assert_eq!(s.axes.rotary, 1.0);
}

#[test]
fn ace_torq_axis_above_max_clamps_to_one() {
    let report = make_ace_torq_report(u16::MAX, [0; 2]);
    let s = parse_ace_torq_report(&report).unwrap();
    assert_eq!(s.axis.throttle, 1.0);
}

// ═════════════════════════════════════════════════════════════════════════════
// 11. Hat switch edge cases
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn alpha_hat_all_directions() {
    let directions = [
        (0u8, VpcAlphaHat::North),
        (1, VpcAlphaHat::NorthEast),
        (2, VpcAlphaHat::East),
        (3, VpcAlphaHat::SouthEast),
        (4, VpcAlphaHat::South),
        (5, VpcAlphaHat::SouthWest),
        (6, VpcAlphaHat::West),
        (7, VpcAlphaHat::NorthWest),
    ];
    for (raw_val, expected) in directions {
        let mut buttons = [0u8; 4];
        buttons[3] = raw_val << 4;
        let report = make_5ax_report([0; 5], buttons);
        let s = parse_alpha_report(&report).unwrap();
        assert_eq!(s.buttons.hat, expected, "hat raw={raw_val}");
    }
}

#[test]
fn alpha_hat_values_8_through_15_are_center() {
    for raw_val in 8u8..=15 {
        let mut buttons = [0u8; 4];
        buttons[3] = raw_val << 4;
        let report = make_5ax_report([0; 5], buttons);
        let s = parse_alpha_report(&report).unwrap();
        assert_eq!(s.buttons.hat, VpcAlphaHat::Center, "raw_val={raw_val}");
    }
}

#[test]
fn mongoost_hat_all_directions() {
    let directions = [
        (0u8, VpcMongoostHat::North),
        (1, VpcMongoostHat::NorthEast),
        (2, VpcMongoostHat::East),
        (3, VpcMongoostHat::SouthEast),
        (4, VpcMongoostHat::South),
        (5, VpcMongoostHat::SouthWest),
        (6, VpcMongoostHat::West),
        (7, VpcMongoostHat::NorthWest),
    ];
    for (raw_val, expected) in directions {
        let mut buttons = [0u8; 4];
        buttons[3] = raw_val << 4;
        let report = make_5ax_report([0; 5], buttons);
        let s = parse_mongoost_stick_report(&report).unwrap();
        assert_eq!(s.buttons.hat, expected, "hat raw={raw_val}");
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// 12. Button edge cases: simultaneous buttons and hat
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn alpha_buttons_and_hat_coexist() {
    let mut buttons = [0u8; 4];
    buttons[0] = 0xFF; // buttons 1–8 all pressed
    buttons[3] = (5u8 << 4) | 0x0F; // hat=SouthWest + buttons 25–28
    let report = make_5ax_report([0; 5], buttons);
    let s = parse_alpha_report(&report).unwrap();
    for i in 1u8..=8 {
        assert!(s.buttons.is_pressed(i), "button {i} should be pressed");
    }
    assert_eq!(s.buttons.hat, VpcAlphaHat::SouthWest);
    // buttons 25–28 in low nibble of byte 3
    for i in 25u8..=28 {
        assert!(s.buttons.is_pressed(i), "button {i} should be pressed");
    }
}

#[test]
fn cm3_multiple_buttons_in_different_bytes() {
    let mut buttons = [0u8; 10];
    buttons[0] = 0x01; // button 1
    buttons[4] = 0x01; // button 33
    buttons[9] = 1 << 5; // button 78
    let report = make_cm3_report([0; 6], buttons);
    let s = parse_cm3_throttle_report(&report).unwrap();
    assert!(s.buttons.is_pressed(1));
    assert!(s.buttons.is_pressed(33));
    assert!(s.buttons.is_pressed(78));
    assert!(!s.buttons.is_pressed(2));
    assert!(!s.buttons.is_pressed(34));
}

// ═════════════════════════════════════════════════════════════════════════════
// 13. Panel 2 raw axis round-trip
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn panel2_raw_axes_preserved() {
    let report = make_panel2_report(12345, 54321, [0; 6]);
    let s = parse_panel2_report(&report).unwrap();
    assert_eq!(s.axes.a1_raw, 12345);
    assert_eq!(s.axes.a2_raw, 54321);
}

#[test]
fn panel2_normalised_axes_zero_and_max() {
    let report = make_panel2_report(0, VIRPIL_AXIS_MAX, [0; 6]);
    let s = parse_panel2_report(&report).unwrap();
    assert_eq!(s.axes.a1_normalised(), 0.0);
    assert!((s.axes.a2_normalised() - 1.0).abs() < 1e-6);
}

// ═════════════════════════════════════════════════════════════════════════════
// 14. Variant-wrapping parsers preserve variant
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn warbrd_variant_original_preserved() {
    let report = make_5ax_report([0; 5], [0; 4]);
    let s = parse_warbrd_report(&report, WarBrdVariant::Original).unwrap();
    assert_eq!(s.variant, WarBrdVariant::Original);
}

#[test]
fn warbrd_variant_d_preserved() {
    let report = make_5ax_report([0; 5], [0; 4]);
    let s = parse_warbrd_report(&report, WarBrdVariant::D).unwrap();
    assert_eq!(s.variant, WarBrdVariant::D);
}

#[test]
fn alpha_prime_variant_left_preserved() {
    let report = make_5ax_report([0; 5], [0; 4]);
    let s = parse_alpha_prime_report(&report, AlphaPrimeVariant::Left).unwrap();
    assert_eq!(s.variant, AlphaPrimeVariant::Left);
}

#[test]
fn alpha_prime_variant_right_preserved() {
    let report = make_5ax_report([0; 5], [0; 4]);
    let s = parse_alpha_prime_report(&report, AlphaPrimeVariant::Right).unwrap();
    assert_eq!(s.variant, AlphaPrimeVariant::Right);
}

// ═════════════════════════════════════════════════════════════════════════════
// 15. Variant product names
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn warbrd_variant_product_names() {
    assert_eq!(WarBrdVariant::Original.product_name(), "VPC WarBRD Stick");
    assert_eq!(WarBrdVariant::D.product_name(), "VPC WarBRD-D Stick");
}

#[test]
fn alpha_prime_variant_product_names() {
    assert_eq!(
        AlphaPrimeVariant::Left.product_name(),
        "VPC Constellation Alpha Prime Left"
    );
    assert_eq!(
        AlphaPrimeVariant::Right.product_name(),
        "VPC Constellation Alpha Prime Right"
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// 16. Wrapper parsers inherit axis values from base parser
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn warbrd_axes_match_mongoost_for_same_data() {
    let report = make_5ax_report([1000, 2000, 3000, 4000, 5000], [0; 4]);
    let mongoost = parse_mongoost_stick_report(&report).unwrap();
    let warbrd = parse_warbrd_report(&report, WarBrdVariant::D).unwrap();
    assert_eq!(warbrd.inner.axes, mongoost.axes);
    assert_eq!(warbrd.inner.buttons, mongoost.buttons);
}

#[test]
fn alpha_prime_axes_match_alpha_for_same_data() {
    let report = make_5ax_report([500, 1500, 2500, 3500, 4500], [0x0F; 4]);
    let alpha = parse_alpha_report(&report).unwrap();
    let prime = parse_alpha_prime_report(&report, AlphaPrimeVariant::Left).unwrap();
    assert_eq!(prime.axes, alpha.axes);
    assert_eq!(prime.buttons, alpha.buttons);
}

// ═════════════════════════════════════════════════════════════════════════════
// 17. Min report size constants match expected formula
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn min_report_bytes_5ax_sticks() {
    // 1 (report_id) + 5*2 (axes) + 4 (buttons) = 15
    assert_eq!(VPC_ALPHA_MIN_REPORT_BYTES, 15);
    assert_eq!(VPC_ALPHA_PRIME_MIN_REPORT_BYTES, 15);
    assert_eq!(VPC_MONGOOST_STICK_MIN_REPORT_BYTES, 15);
    assert_eq!(VPC_WARBRD_MIN_REPORT_BYTES, 15);
}

#[test]
fn min_report_bytes_cm3_throttle() {
    // 1 (report_id) + 6*2 (axes) + 10 (buttons) = 23
    assert_eq!(VPC_CM3_THROTTLE_MIN_REPORT_BYTES, 23);
}

#[test]
fn min_report_bytes_panels() {
    // Panel 1: 1 + 0*2 + 6 = 7
    assert_eq!(VPC_PANEL1_MIN_REPORT_BYTES, 7);
    // Panel 2: 1 + 2*2 + 6 = 11
    assert_eq!(VPC_PANEL2_MIN_REPORT_BYTES, 11);
}

#[test]
fn min_report_bytes_pedals_tcs_torq() {
    // Pedals: 1 + 3*2 + 2 = 9
    assert_eq!(VPC_ACE_PEDALS_MIN_REPORT_BYTES, 9);
    // Rotor TCS: 1 + 3*2 + 4 = 11
    assert_eq!(VPC_ROTOR_TCS_MIN_REPORT_BYTES, 11);
    // ACE Torq: 1 + 1*2 + 2 = 5
    assert_eq!(VPC_ACE_TORQ_MIN_REPORT_BYTES, 5);
}

// ═════════════════════════════════════════════════════════════════════════════
// 18. Error messages contain useful context
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn error_messages_contain_received_byte_count() {
    let err = parse_alpha_report(&[0x01; 3]).unwrap_err();
    assert!(err.to_string().contains('3'), "err: {err}");

    let err = parse_cm3_throttle_report(&[0x01; 10]).unwrap_err();
    assert!(err.to_string().contains("10"), "err: {err}");

    let err = parse_ace_torq_report(&[0x01; 2]).unwrap_err();
    assert!(err.to_string().contains('2'), "err: {err}");

    let err = parse_ace_pedals_report(&[0x01; 4]).unwrap_err();
    assert!(err.to_string().contains('4'), "err: {err}");

    let err = parse_rotor_tcs_report(&[0x01; 6]).unwrap_err();
    assert!(err.to_string().contains('6'), "err: {err}");
}

// ═════════════════════════════════════════════════════════════════════════════
// 19. Default trait implementations
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn default_input_states_have_zero_axes_and_no_buttons() {
    use flight_hotas_virpil::{
        VpcAcePedalsInputState, VpcAceTorqInputState, VpcAlphaInputState, VpcCm3ThrottleInputState,
        VpcMongoostInputState, VpcPanel1InputState, VpcPanel2InputState, VpcRotorTcsInputState,
    };

    let alpha = VpcAlphaInputState::default();
    assert_eq!(alpha.axes.x, 0.0);
    assert!(alpha.buttons.pressed().is_empty());

    let mongoost = VpcMongoostInputState::default();
    assert_eq!(mongoost.axes.x, 0.0);
    assert!(mongoost.buttons.pressed().is_empty());

    let cm3 = VpcCm3ThrottleInputState::default();
    assert_eq!(cm3.axes.left_throttle, 0.0);
    assert!(cm3.buttons.pressed().is_empty());

    let p1 = VpcPanel1InputState::default();
    assert!(p1.buttons.pressed().is_empty());

    let p2 = VpcPanel2InputState::default();
    assert_eq!(p2.axes.a1_raw, 0);
    assert!(p2.buttons.pressed().is_empty());

    let pedals = VpcAcePedalsInputState::default();
    assert_eq!(pedals.axes.rudder, 0.0);
    assert!(pedals.buttons.pressed().is_empty());

    let tcs = VpcRotorTcsInputState::default();
    assert_eq!(tcs.axes.collective, 0.0);
    assert!(tcs.buttons.pressed().is_empty());

    let torq = VpcAceTorqInputState::default();
    assert_eq!(torq.axis.throttle, 0.0);
    assert!(torq.buttons.pressed().is_empty());
}

// ═════════════════════════════════════════════════════════════════════════════
// 20. Individual button bit-position correctness
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn panel1_each_button_bit_position() {
    for n in 1u8..=48 {
        let idx = (n - 1) as usize;
        let byte_idx = idx / 8;
        let bit_idx = idx % 8;
        let mut buttons = [0u8; 6];
        buttons[byte_idx] = 1 << bit_idx;
        let report = make_panel1_report(buttons);
        let s = parse_panel1_report(&report).unwrap();
        assert!(s.buttons.is_pressed(n), "button {n} not detected");
        assert_eq!(s.buttons.pressed(), vec![n], "extra buttons for button {n}");
    }
}

#[test]
fn ace_torq_each_button_bit_position() {
    for n in 1u8..=ACE_TORQ_BUTTON_COUNT {
        let idx = (n - 1) as usize;
        let byte_idx = idx / 8;
        let bit_idx = idx % 8;
        let mut buttons = [0u8; 2];
        buttons[byte_idx] = 1 << bit_idx;
        let report = make_ace_torq_report(0, buttons);
        let s = parse_ace_torq_report(&report).unwrap();
        assert!(s.buttons.is_pressed(n), "button {n} not detected");
        assert_eq!(s.buttons.pressed(), vec![n]);
    }
}

#[test]
fn pedals_each_button_bit_position() {
    for n in 1u8..=ACE_PEDALS_BUTTON_COUNT {
        let idx = (n - 1) as usize;
        let byte_idx = idx / 8;
        let bit_idx = idx % 8;
        let mut buttons = [0u8; 2];
        buttons[byte_idx] = 1 << bit_idx;
        let report = make_pedals_report([0; 3], buttons);
        let s = parse_ace_pedals_report(&report).unwrap();
        assert!(s.buttons.is_pressed(n), "button {n} not detected");
        assert_eq!(s.buttons.pressed(), vec![n]);
    }
}

#[test]
fn rotor_tcs_each_button_bit_position() {
    for n in 1u8..=ROTOR_TCS_BUTTON_COUNT {
        let idx = (n - 1) as usize;
        let byte_idx = idx / 8;
        let bit_idx = idx % 8;
        let mut buttons = [0u8; 4];
        buttons[byte_idx] = 1 << bit_idx;
        let report = make_rotor_tcs_report([0; 3], buttons);
        let s = parse_rotor_tcs_report(&report).unwrap();
        assert!(s.buttons.is_pressed(n), "button {n} not detected");
        assert_eq!(s.buttons.pressed(), vec![n]);
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// 21. Axis endianness: LE u16 encoding
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn alpha_axis_little_endian_encoding() {
    // Encode 0x0102 = 258 as LE [0x02, 0x01]
    let report = make_5ax_report([258, 0, 0, 0, 0], [0; 4]);
    let s = parse_alpha_report(&report).unwrap();
    let expected = 258.0 / VIRPIL_AXIS_MAX as f32;
    assert!((s.axes.x - expected).abs() < 1e-5);
}

#[test]
fn cm3_axis_individual_values() {
    let report = make_cm3_report([100, 200, 300, 400, 500, 600], [0; 10]);
    let s = parse_cm3_throttle_report(&report).unwrap();
    let eps = 1e-4;
    assert!((s.axes.left_throttle - 100.0 / VIRPIL_AXIS_MAX as f32).abs() < eps);
    assert!((s.axes.right_throttle - 200.0 / VIRPIL_AXIS_MAX as f32).abs() < eps);
    assert!((s.axes.flaps - 300.0 / VIRPIL_AXIS_MAX as f32).abs() < eps);
    assert!((s.axes.scx - 400.0 / VIRPIL_AXIS_MAX as f32).abs() < eps);
    assert!((s.axes.scy - 500.0 / VIRPIL_AXIS_MAX as f32).abs() < eps);
    assert!((s.axes.slider - 600.0 / VIRPIL_AXIS_MAX as f32).abs() < eps);
}

// ═════════════════════════════════════════════════════════════════════════════
// 22. Profile–device table cross-validation
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn profile_axis_count_matches_device_table_where_available() {
    for profile in ALL_PROFILES {
        if let Some(info) = device_info(profile.pid) {
            assert_eq!(
                profile.axes.len(),
                info.axis_count as usize,
                "profile '{}' axis count mismatch with device table",
                profile.name
            );
        }
    }
}

#[test]
fn profile_button_count_matches_device_table_where_available() {
    for profile in ALL_PROFILES {
        if let Some(info) = device_info(profile.pid) {
            assert_eq!(
                profile.button_count, info.button_count,
                "profile '{}' button count mismatch with device table",
                profile.name
            );
        }
    }
}
