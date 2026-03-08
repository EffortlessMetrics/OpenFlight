// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Depth tests for the `flight-hotas-simucube` crate covering report parsing,
//! model detection, encoder normalization, torque commands, and error handling.

use flight_hotas_simucube::{
    ENCODER_CENTER, ENCODER_MAX, SC2_PRO_PID, SC2_REPORT_MIN_LEN, SC2_SPORT_PID,
    SC2_ULTIMATE_PID, SIMUCUBE_VENDOR_ID, SimucubeError, SimucubeModel, SimucubeReport,
    TorqueCommand, normalize_angle, parse_report,
};

// ── Helpers ──────────────────────────────────────────────────────────────────

fn build_report(encoder: u32, velocity: i16, torque: i16) -> Vec<u8> {
    let enc = encoder.to_le_bytes();
    let vel = velocity.to_le_bytes();
    let tq = torque.to_le_bytes();
    vec![0x01, enc[0], enc[1], enc[2], enc[3], vel[0], vel[1], tq[0], tq[1]]
}

fn build_report_no_torque(encoder: u32, velocity: i16) -> Vec<u8> {
    let enc = encoder.to_le_bytes();
    let vel = velocity.to_le_bytes();
    vec![0x01, enc[0], enc[1], enc[2], enc[3], vel[0], vel[1]]
}

// ── Constants sanity ─────────────────────────────────────────────────────────

#[test]
fn vendor_id_is_correct() {
    assert_eq!(SIMUCUBE_VENDOR_ID, 0x16D0);
}

#[test]
fn encoder_constants_consistent() {
    assert_eq!(ENCODER_MAX, (1u32 << 22) - 1);
    assert_eq!(ENCODER_CENTER, ENCODER_MAX / 2);
}

#[test]
fn report_min_len_is_seven() {
    assert_eq!(SC2_REPORT_MIN_LEN, 7);
}

// ── Model detection ──────────────────────────────────────────────────────────

#[test]
fn model_from_pid_all_variants() {
    assert_eq!(SimucubeModel::from_pid(SC2_SPORT_PID), Some(SimucubeModel::Sport));
    assert_eq!(SimucubeModel::from_pid(SC2_PRO_PID), Some(SimucubeModel::Pro));
    assert_eq!(SimucubeModel::from_pid(SC2_ULTIMATE_PID), Some(SimucubeModel::Ultimate));
}

#[test]
fn model_from_pid_unknown_returns_none() {
    assert_eq!(SimucubeModel::from_pid(0x0000), None);
    assert_eq!(SimucubeModel::from_pid(0xFFFF), None);
    assert_eq!(SimucubeModel::from_pid(0x0D5B), None); // near-miss
}

#[test]
fn model_pid_round_trip_all() {
    for model in [SimucubeModel::Sport, SimucubeModel::Pro, SimucubeModel::Ultimate] {
        assert_eq!(SimucubeModel::from_pid(model.pid()), Some(model));
    }
}

#[test]
fn model_pids_are_distinct() {
    let pids = [SC2_SPORT_PID, SC2_PRO_PID, SC2_ULTIMATE_PID];
    for i in 0..pids.len() {
        for j in (i + 1)..pids.len() {
            assert_ne!(pids[i], pids[j], "PIDs must be distinct");
        }
    }
}

// ── Report parsing ───────────────────────────────────────────────────────────

#[test]
fn parse_center_position() {
    let data = build_report(ENCODER_CENTER, 0, 0);
    let r = parse_report(&data).unwrap();
    assert_eq!(r.encoder_position, ENCODER_CENTER);
    assert_eq!(r.velocity, 0);
    assert_eq!(r.torque_feedback, 0);
}

#[test]
fn parse_encoder_boundaries() {
    let min_data = build_report(0, 0, 0);
    let max_data = build_report(ENCODER_MAX, 0, 0);
    assert_eq!(parse_report(&min_data).unwrap().encoder_position, 0);
    assert_eq!(parse_report(&max_data).unwrap().encoder_position, ENCODER_MAX);
}

#[test]
fn parse_encoder_overflow_masked() {
    // Values above ENCODER_MAX should be masked to 22 bits
    let data = build_report(ENCODER_MAX + 1, 0, 0);
    let r = parse_report(&data).unwrap();
    assert!(r.encoder_position <= ENCODER_MAX);
}

#[test]
fn parse_encoder_all_bits_set_masked() {
    let data = build_report(0xFFFF_FFFF, 0, 0);
    let r = parse_report(&data).unwrap();
    assert_eq!(r.encoder_position, ENCODER_MAX);
}

#[test]
fn parse_velocity_positive() {
    let data = build_report(ENCODER_CENTER, 1000, 0);
    let r = parse_report(&data).unwrap();
    assert_eq!(r.velocity, 1000);
}

#[test]
fn parse_velocity_negative() {
    let data = build_report(ENCODER_CENTER, -500, 0);
    let r = parse_report(&data).unwrap();
    assert_eq!(r.velocity, -500);
}

#[test]
fn parse_velocity_extremes() {
    let max = build_report(ENCODER_CENTER, i16::MAX, 0);
    let min = build_report(ENCODER_CENTER, i16::MIN, 0);
    assert_eq!(parse_report(&max).unwrap().velocity, i16::MAX);
    assert_eq!(parse_report(&min).unwrap().velocity, i16::MIN);
}

#[test]
fn parse_torque_feedback_present() {
    let data = build_report(ENCODER_CENTER, 0, -12345);
    let r = parse_report(&data).unwrap();
    assert_eq!(r.torque_feedback, -12345);
}

#[test]
fn parse_torque_feedback_extremes() {
    let max = build_report(ENCODER_CENTER, 0, i16::MAX);
    let min = build_report(ENCODER_CENTER, 0, i16::MIN);
    assert_eq!(parse_report(&max).unwrap().torque_feedback, i16::MAX);
    assert_eq!(parse_report(&min).unwrap().torque_feedback, i16::MIN);
}

#[test]
fn parse_min_length_no_torque_defaults_zero() {
    let data = build_report_no_torque(ENCODER_CENTER, 42);
    assert_eq!(data.len(), SC2_REPORT_MIN_LEN);
    let r = parse_report(&data).unwrap();
    assert_eq!(r.torque_feedback, 0);
    assert_eq!(r.velocity, 42);
}

#[test]
fn parse_partial_torque_byte_defaults_zero() {
    // 8 bytes: one torque byte but not two → should default to 0
    let mut data = build_report_no_torque(ENCODER_CENTER, 0);
    data.push(0xFF); // only 1 extra byte (need 2 for torque)
    assert_eq!(data.len(), 8);
    let r = parse_report(&data).unwrap();
    assert_eq!(r.torque_feedback, 0);
}

#[test]
fn parse_extra_trailing_bytes_ok() {
    let mut data = build_report(ENCODER_CENTER, 100, 200);
    data.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
    let r = parse_report(&data).unwrap();
    assert_eq!(r.encoder_position, ENCODER_CENTER);
    assert_eq!(r.velocity, 100);
    assert_eq!(r.torque_feedback, 200);
}

// ── Report parsing — errors ──────────────────────────────────────────────────

#[test]
fn parse_empty_too_short() {
    assert_eq!(parse_report(&[]), Err(SimucubeError::TooShort { got: 0 }));
}

#[test]
fn parse_one_byte_too_short() {
    assert_eq!(parse_report(&[0x01]), Err(SimucubeError::TooShort { got: 1 }));
}

#[test]
fn parse_six_bytes_too_short() {
    assert_eq!(
        parse_report(&[0x01, 0, 0, 0, 0, 0]),
        Err(SimucubeError::TooShort { got: 6 })
    );
}

#[test]
fn parse_unknown_report_id_zero() {
    let mut data = build_report(0, 0, 0);
    data[0] = 0x00;
    assert_eq!(parse_report(&data), Err(SimucubeError::UnknownReportId { id: 0x00 }));
}

#[test]
fn parse_unknown_report_id_ff() {
    let mut data = build_report(0, 0, 0);
    data[0] = 0xFF;
    assert_eq!(parse_report(&data), Err(SimucubeError::UnknownReportId { id: 0xFF }));
}

#[test]
fn error_display_too_short() {
    let e = SimucubeError::TooShort { got: 3 };
    let msg = format!("{e}");
    assert!(msg.contains("too short"), "error message: {msg}");
    assert!(msg.contains("3"), "error message should contain actual length: {msg}");
}

#[test]
fn error_display_unknown_id() {
    let e = SimucubeError::UnknownReportId { id: 0xAB };
    let msg = format!("{e}");
    assert!(msg.contains("AB") || msg.contains("ab"), "should show hex id: {msg}");
}

// ── normalize_angle ──────────────────────────────────────────────────────────

#[test]
fn normalize_angle_center_22bit() {
    let v = normalize_angle(ENCODER_CENTER, 22);
    assert!(v.abs() < 1e-4, "center should be ~0.0: {v}");
}

#[test]
fn normalize_angle_min_22bit() {
    let v = normalize_angle(0, 22);
    assert!((v - (-1.0)).abs() < 1e-4, "min should be ~-1.0: {v}");
}

#[test]
fn normalize_angle_max_22bit() {
    let v = normalize_angle(ENCODER_MAX, 22);
    assert!((v - 1.0).abs() < 1e-4, "max should be ~1.0: {v}");
}

#[test]
fn normalize_angle_16bit() {
    let max_16 = (1u32 << 16) - 1;
    let center_16 = max_16 / 2;
    assert!(normalize_angle(center_16, 16).abs() < 1e-3);
    assert!((normalize_angle(0, 16) - (-1.0)).abs() < 1e-3);
    assert!((normalize_angle(max_16, 16) - 1.0).abs() < 1e-3);
}

#[test]
fn normalize_angle_8bit() {
    assert!(normalize_angle(127, 8).abs() < 0.01);
    assert!((normalize_angle(0, 8) - (-1.0)).abs() < 0.01);
    assert!((normalize_angle(255, 8) - 1.0).abs() < 0.01);
}

#[test]
fn normalize_angle_1bit() {
    // 1-bit encoder: max=1, center=0, half=0 → returns 0.0 (degenerate case)
    assert_eq!(normalize_angle(0, 1), 0.0);
}

#[test]
fn normalize_angle_0bit_returns_zero() {
    // 0-bit resolution → max=0, center=0, half=0 → should return 0.0
    assert_eq!(normalize_angle(0, 0), 0.0);
}

#[test]
fn normalize_angle_clamps_over_max() {
    // Position beyond the 22-bit max should still clamp to valid range
    let v = normalize_angle(ENCODER_MAX + 100, 22);
    assert!((-1.0..=1.0).contains(&v), "clamped: {v}");
}

#[test]
fn normalize_angle_32bit_does_not_panic() {
    // Resolution of 32 should not overflow
    let v = normalize_angle(u32::MAX / 2, 32);
    assert!((-1.0..=1.0).contains(&v), "32-bit: {v}");
}

// ── TorqueCommand ────────────────────────────────────────────────────────────

#[test]
fn torque_zero() {
    assert_eq!(TorqueCommand::new(0.0).to_i16(), 0);
}

#[test]
fn torque_max() {
    assert_eq!(TorqueCommand::new(1.0).to_i16(), 32767);
}

#[test]
fn torque_min() {
    assert_eq!(TorqueCommand::new(-1.0).to_i16(), -32767);
}

#[test]
fn torque_half_positive() {
    let t = TorqueCommand::new(0.5).to_i16();
    // 0.5 * 32767 ≈ 16383
    assert!((16383..=16384).contains(&t), "half: {t}");
}

#[test]
fn torque_half_negative() {
    let t = TorqueCommand::new(-0.5).to_i16();
    assert!((-16384..=-16383).contains(&t), "neg half: {t}");
}

#[test]
fn torque_clamped_above() {
    let t = TorqueCommand::new(5.0);
    assert_eq!(t.value, 1.0);
    assert_eq!(t.to_i16(), 32767);
}

#[test]
fn torque_clamped_below() {
    let t = TorqueCommand::new(-5.0);
    assert_eq!(t.value, -1.0);
    assert_eq!(t.to_i16(), -32767);
}

#[test]
fn torque_nan_clamped() {
    // f32::NAN.clamp() returns NAN on some platforms, but our to_i16 should
    // not panic. We just verify no panic occurs.
    let _t = TorqueCommand::new(f32::NAN);
    let _ = _t.to_i16(); // Actually exercise the method
}

#[test]
fn torque_inf_clamped() {
    let t_pos = TorqueCommand::new(f32::INFINITY);
    assert_eq!(t_pos.to_i16(), 32767);
    let t_neg = TorqueCommand::new(f32::NEG_INFINITY);
    assert_eq!(t_neg.to_i16(), -32767);
}

#[test]
fn torque_symmetry() {
    // +x and -x should produce symmetric i16 outputs
    for &v in &[0.1, 0.25, 0.5, 0.75, 0.99] {
        let pos = TorqueCommand::new(v).to_i16();
        let neg = TorqueCommand::new(-v).to_i16();
        assert_eq!(pos, -neg, "symmetry for {v}");
    }
}

// ── SimucubeReport equality ──────────────────────────────────────────────────

#[test]
fn report_struct_equality() {
    let a = SimucubeReport {
        encoder_position: 100,
        velocity: 50,
        torque_feedback: -10,
    };
    let b = a.clone();
    assert_eq!(a, b);
}

// ── Property-based tests ─────────────────────────────────────────────────────

mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn normalize_angle_always_in_range(pos in 0u32..=ENCODER_MAX) {
            let v = normalize_angle(pos, 22);
            prop_assert!((-1.0..=1.0).contains(&v), "got {v}");
        }

        #[test]
        fn normalize_angle_monotonic(a in 0u32..ENCODER_MAX) {
            let va = normalize_angle(a, 22);
            let vb = normalize_angle(a + 1, 22);
            prop_assert!(vb >= va, "not monotonic: {a}→{va}, {}→{vb}", a + 1);
        }

        #[test]
        fn torque_i16_in_range(v in -1.0f32..=1.0) {
            let t = TorqueCommand::new(v).to_i16();
            prop_assert!((-32767..=32767).contains(&t), "got {t}");
        }

        #[test]
        fn torque_round_trip_approximate(v in -1.0f32..=1.0) {
            let wire = TorqueCommand::new(v).to_i16();
            let back = wire as f32 / 32767.0;
            prop_assert!((back - v).abs() < 0.001, "v={v}, wire={wire}, back={back}");
        }

        #[test]
        fn parse_any_valid_report(
            enc in 0u32..=ENCODER_MAX,
            vel in i16::MIN..=i16::MAX,
            tq in i16::MIN..=i16::MAX,
        ) {
            let data = build_report(enc, vel, tq);
            let r = parse_report(&data).unwrap();
            prop_assert_eq!(r.encoder_position, enc & ENCODER_MAX);
            prop_assert_eq!(r.velocity, vel);
            prop_assert_eq!(r.torque_feedback, tq);
        }

        #[test]
        fn parse_random_bytes_at_least_7(data in proptest::collection::vec(any::<u8>(), 7..64)) {
            let result = parse_report(&data);
            match data[0] {
                0x01 => prop_assert!(result.is_ok()),
                id => prop_assert_eq!(result, Err(SimucubeError::UnknownReportId { id })),
            }
        }
    }
}
