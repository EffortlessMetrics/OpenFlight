// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the `flight-ffb-moza` crate.
//!
//! Covers boundary conditions, round-trip invariants, health-monitor state
//! machines, preset validation, and FFB safety properties across all modules.

use flight_ffb_moza::effects::{FfbMode, TorqueCommand, TORQUE_REPORT_ID, TORQUE_REPORT_LEN};
use flight_ffb_moza::health::MozaHealthMonitor;
use flight_ffb_moza::input::{
    parse_ab9_report, Ab9Buttons, AB9_BASE_PID, AB9_REPORT_LEN, MOZA_VENDOR_ID, MozaParseError,
    R3_BASE_PID,
};
use flight_ffb_moza::presets::ab9_axis_config;
use flight_ffb_moza::MOZA_PIDS;

// ─── helpers ────────────────────────────────────────────────────────────────

fn make_report(
    roll: i16,
    pitch: i16,
    throttle: i16,
    twist: i16,
    mask: u16,
    hat: u8,
) -> [u8; AB9_REPORT_LEN] {
    let mut r = [0u8; AB9_REPORT_LEN];
    r[0] = 0x01;
    r[1..3].copy_from_slice(&roll.to_le_bytes());
    r[3..5].copy_from_slice(&pitch.to_le_bytes());
    r[5..7].copy_from_slice(&throttle.to_le_bytes());
    r[7..9].copy_from_slice(&twist.to_le_bytes());
    r[9..11].copy_from_slice(&mask.to_le_bytes());
    r[11] = hat;
    r
}

fn centred() -> [u8; AB9_REPORT_LEN] {
    make_report(0, 0, 0, 0, 0, 0)
}

// ──────────────────────────────────────────────────────────────────────────────
// Module: input — parse_ab9_report
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn centred_report_produces_neutral_axes() {
    let s = parse_ab9_report(&centred()).unwrap();
    assert!(s.axes.roll.abs() < 1e-4);
    assert!(s.axes.pitch.abs() < 1e-4);
    assert!((s.axes.throttle - 0.5).abs() < 1e-3);
    assert!(s.axes.twist.abs() < 1e-4);
}

#[test]
fn max_positive_all_axes() {
    let r = make_report(i16::MAX, i16::MAX, i16::MAX, i16::MAX, 0, 0);
    let s = parse_ab9_report(&r).unwrap();
    assert!((s.axes.roll - 1.0).abs() < 1e-4);
    assert!((s.axes.pitch - 1.0).abs() < 1e-4);
    assert!((s.axes.throttle - 1.0).abs() < 1e-3);
    assert!((s.axes.twist - 1.0).abs() < 1e-4);
}

#[test]
fn max_negative_all_axes() {
    let r = make_report(i16::MIN, i16::MIN, i16::MIN, i16::MIN, 0, 0);
    let s = parse_ab9_report(&r).unwrap();
    // i16::MIN / 32767 ≈ -1.0000305 → clamped to -1.0
    assert!((s.axes.roll - (-1.0)).abs() < 1e-3);
    assert!((s.axes.pitch - (-1.0)).abs() < 1e-3);
    // throttle maps [-1, 1] → [0, 1], so -1.0 → 0.0
    assert!(s.axes.throttle < 0.01);
    assert!((s.axes.twist - (-1.0)).abs() < 1e-3);
}

#[test]
fn half_positive_deflection() {
    let half = i16::MAX / 2;
    let r = make_report(half, half, half, half, 0, 0);
    let s = parse_ab9_report(&r).unwrap();
    assert!((s.axes.roll - 0.5).abs() < 0.01);
    assert!((s.axes.pitch - 0.5).abs() < 0.01);
    assert!((s.axes.twist - 0.5).abs() < 0.01);
}

#[test]
fn throttle_zero_raw_maps_to_half() {
    let r = make_report(0, 0, 0, 0, 0, 0);
    let s = parse_ab9_report(&r).unwrap();
    assert!((s.axes.throttle - 0.5).abs() < 1e-3);
}

#[test]
fn report_id_zero_rejected() {
    let mut r = centred();
    r[0] = 0x00;
    assert!(matches!(
        parse_ab9_report(&r),
        Err(MozaParseError::UnknownReportId { id: 0x00 })
    ));
}

#[test]
fn report_too_short_by_one() {
    let data = vec![0x01; AB9_REPORT_LEN - 1];
    match parse_ab9_report(&data) {
        Err(MozaParseError::TooShort { expected, got }) => {
            assert_eq!(expected, AB9_REPORT_LEN);
            assert_eq!(got, AB9_REPORT_LEN - 1);
        }
        other => panic!("expected TooShort, got {other:?}"),
    }
}

#[test]
fn empty_report_errors() {
    assert!(parse_ab9_report(&[]).is_err());
}

#[test]
fn extra_trailing_bytes_are_ignored() {
    let mut data = vec![0u8; AB9_REPORT_LEN + 10];
    data[0] = 0x01;
    data[1] = 0xFF;
    data[2] = 0x7F; // roll = i16::MAX
    let s = parse_ab9_report(&data).unwrap();
    assert!((s.axes.roll - 1.0).abs() < 1e-4);
}

#[test]
fn hat_byte_preserved() {
    let r = make_report(0, 0, 0, 0, 0, 0x03);
    let s = parse_ab9_report(&r).unwrap();
    assert_eq!(s.buttons.hat, 0x03);
}

#[test]
fn hat_neutral_value() {
    let s = parse_ab9_report(&centred()).unwrap();
    assert_eq!(s.buttons.hat, 0);
}

// ──────────────────────────────────────────────────────────────────────────────
// Module: input — Ab9Buttons
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn button_zero_out_of_range() {
    let b = Ab9Buttons { mask: 0xFFFF, hat: 0 };
    assert!(!b.is_pressed(0), "button 0 is out of valid range 1..=16");
}

#[test]
fn button_17_out_of_range() {
    let b = Ab9Buttons { mask: 0xFFFF, hat: 0 };
    assert!(!b.is_pressed(17), "button 17 is out of valid range 1..=16");
}

#[test]
fn all_buttons_pressed() {
    let b = Ab9Buttons { mask: 0xFFFF, hat: 0 };
    for n in 1..=16u8 {
        assert!(b.is_pressed(n), "button {n} should be pressed");
    }
}

#[test]
fn no_buttons_pressed() {
    let b = Ab9Buttons { mask: 0x0000, hat: 0 };
    for n in 1..=16u8 {
        assert!(!b.is_pressed(n), "button {n} should not be pressed");
    }
}

#[test]
fn single_button_isolation() {
    for target in 1..=16u8 {
        let mask = 1u16 << (target - 1);
        let b = Ab9Buttons { mask, hat: 0 };
        for n in 1..=16u8 {
            assert_eq!(
                b.is_pressed(n),
                n == target,
                "mask=0x{mask:04X} button {n}"
            );
        }
    }
}

#[test]
fn button_mask_high_byte() {
    let r = make_report(0, 0, 0, 0, 0xFF00, 0);
    let s = parse_ab9_report(&r).unwrap();
    for n in 1..=8u8 {
        assert!(!s.buttons.is_pressed(n));
    }
    for n in 9..=16u8 {
        assert!(s.buttons.is_pressed(n));
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Module: input — constants
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn vendor_id_constant() {
    assert_eq!(MOZA_VENDOR_ID, 0x346E);
}

#[test]
fn pid_constants_are_distinct() {
    assert_ne!(AB9_BASE_PID, R3_BASE_PID);
}

#[test]
fn moza_pids_contains_all_known() {
    assert!(MOZA_PIDS.contains(&AB9_BASE_PID));
    assert!(MOZA_PIDS.contains(&R3_BASE_PID));
    assert_eq!(MOZA_PIDS.len(), 2);
}

#[test]
fn report_len_sufficient_for_all_fields() {
    // 1 report-ID + 8 axes (4×2) + 2 button mask + 1 hat + padding
    let min_required: usize = 12;
    assert!(AB9_REPORT_LEN >= min_required);
}

// ──────────────────────────────────────────────────────────────────────────────
// Module: effects — TorqueCommand
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn torque_zero_serialises_to_all_zeroes_in_payload() {
    let report = TorqueCommand::ZERO.to_report();
    assert_eq!(report[0], TORQUE_REPORT_ID);
    assert_eq!(&report[1..5], &[0, 0, 0, 0]);
}

#[test]
fn torque_report_length_matches_constant() {
    let report = TorqueCommand::ZERO.to_report();
    assert_eq!(report.len(), TORQUE_REPORT_LEN);
}

#[test]
fn torque_full_positive_both_axes() {
    let cmd = TorqueCommand { x: 1.0, y: 1.0 };
    let r = cmd.to_report();
    let x = i16::from_le_bytes([r[1], r[2]]);
    let y = i16::from_le_bytes([r[3], r[4]]);
    assert_eq!(x, 32767);
    assert_eq!(y, 32767);
}

#[test]
fn torque_full_negative_both_axes() {
    let cmd = TorqueCommand { x: -1.0, y: -1.0 };
    let r = cmd.to_report();
    let x = i16::from_le_bytes([r[1], r[2]]);
    let y = i16::from_le_bytes([r[3], r[4]]);
    assert_eq!(x, -32767);
    assert_eq!(y, -32767);
}

#[test]
fn torque_half_values_encode_correctly() {
    let cmd = TorqueCommand { x: 0.5, y: -0.25 };
    let r = cmd.to_report();
    let x = i16::from_le_bytes([r[1], r[2]]);
    let y = i16::from_le_bytes([r[3], r[4]]);
    let expected_x = (0.5_f32 * 32767.0) as i16;
    let expected_y = (-0.25_f32 * 32767.0) as i16;
    assert_eq!(x, expected_x);
    assert_eq!(y, expected_y);
}

#[test]
fn torque_clamps_excessive_positive() {
    let cmd = TorqueCommand { x: 5.0, y: 100.0 };
    let r = cmd.to_report();
    let x = i16::from_le_bytes([r[1], r[2]]);
    let y = i16::from_le_bytes([r[3], r[4]]);
    assert_eq!(x, 32767);
    assert_eq!(y, 32767);
}

#[test]
fn torque_clamps_excessive_negative() {
    let cmd = TorqueCommand { x: -5.0, y: -100.0 };
    let r = cmd.to_report();
    let x = i16::from_le_bytes([r[1], r[2]]);
    let y = i16::from_le_bytes([r[3], r[4]]);
    assert_eq!(x, -32767);
    assert_eq!(y, -32767);
}

#[test]
fn torque_is_safe_boundary() {
    assert!(TorqueCommand { x: 1.0, y: -1.0 }.is_safe());
    assert!(TorqueCommand { x: -1.0, y: 1.0 }.is_safe());
    assert!(TorqueCommand { x: 0.0, y: 0.0 }.is_safe());
}

#[test]
fn torque_is_not_safe_over_boundary() {
    let eps = 1.0 + f32::EPSILON;
    assert!(!TorqueCommand { x: eps, y: 0.0 }.is_safe());
    assert!(!TorqueCommand { x: 0.0, y: -eps }.is_safe());
}

#[test]
fn torque_nan_is_not_safe() {
    assert!(!TorqueCommand { x: f32::NAN, y: 0.0 }.is_safe());
    assert!(!TorqueCommand { x: 0.0, y: f32::NAN }.is_safe());
}

#[test]
fn torque_infinity_is_not_safe() {
    assert!(!TorqueCommand { x: f32::INFINITY, y: 0.0 }.is_safe());
    assert!(!TorqueCommand { x: f32::NEG_INFINITY, y: 0.0 }.is_safe());
}

#[test]
fn torque_nan_clamped_in_report() {
    // NaN.clamp(-1, 1) returns NaN per IEEE 754, but the report should not
    // produce unconstrained values.  This tests the current behaviour.
    let cmd = TorqueCommand { x: f32::NAN, y: 0.0 };
    let _report = cmd.to_report(); // must not panic
}

#[test]
fn torque_report_id_constant() {
    assert_eq!(TORQUE_REPORT_ID, 0x20);
}

#[test]
fn torque_report_len_constant() {
    assert_eq!(TORQUE_REPORT_LEN, 6);
}

// ──────────────────────────────────────────────────────────────────────────────
// Module: effects — FfbMode
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn ffb_mode_display_all_variants() {
    assert_eq!(FfbMode::Passive.to_string(), "Passive");
    assert_eq!(FfbMode::Spring.to_string(), "Spring");
    assert_eq!(FfbMode::Damper.to_string(), "Damper");
    assert_eq!(FfbMode::Direct.to_string(), "Direct");
}

#[test]
fn ffb_mode_equality() {
    assert_eq!(FfbMode::Spring, FfbMode::Spring);
    assert_ne!(FfbMode::Spring, FfbMode::Damper);
}

#[test]
fn ffb_mode_clone() {
    let mode = FfbMode::Direct;
    let cloned = mode;
    assert_eq!(mode, cloned);
}

// ──────────────────────────────────────────────────────────────────────────────
// Module: health — MozaHealthMonitor
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn fresh_monitor_is_healthy_and_online() {
    let m = MozaHealthMonitor::new();
    let s = m.status();
    assert!(s.is_healthy());
    assert!(s.connected);
    assert_eq!(s.consecutive_failures, 0);
    assert!(!s.torque_fault);
}

#[test]
fn default_monitor_equals_new() {
    let a = MozaHealthMonitor::new();
    let b = MozaHealthMonitor::default();
    assert_eq!(a.status().connected, b.status().connected);
    assert_eq!(a.status().consecutive_failures, b.status().consecutive_failures);
}

#[test]
fn one_failure_still_online() {
    let mut m = MozaHealthMonitor::new();
    m.record_failure();
    assert!(!m.is_offline());
    assert!(m.status().connected);
}

#[test]
fn threshold_minus_one_failures_still_online() {
    let mut m = MozaHealthMonitor::new();
    for _ in 0..MozaHealthMonitor::DEFAULT_FAILURE_THRESHOLD - 1 {
        m.record_failure();
    }
    assert!(!m.is_offline());
}

#[test]
fn exact_threshold_failures_goes_offline() {
    let mut m = MozaHealthMonitor::new();
    for _ in 0..MozaHealthMonitor::DEFAULT_FAILURE_THRESHOLD {
        m.record_failure();
    }
    assert!(m.is_offline());
    assert!(!m.status().connected);
    assert!(!m.status().is_healthy());
}

#[test]
fn success_resets_failure_count() {
    let mut m = MozaHealthMonitor::new();
    m.record_failure();
    m.record_failure();
    m.record_success();
    assert_eq!(m.status().consecutive_failures, 0);
    assert!(!m.is_offline());
}

#[test]
fn success_after_offline_restores_connectivity() {
    let mut m = MozaHealthMonitor::new();
    for _ in 0..MozaHealthMonitor::DEFAULT_FAILURE_THRESHOLD {
        m.record_failure();
    }
    assert!(m.is_offline());
    m.record_success();
    assert!(!m.is_offline());
    assert!(m.status().connected);
}

#[test]
fn torque_fault_makes_unhealthy_even_if_connected() {
    let mut m = MozaHealthMonitor::new();
    m.record_success();
    m.set_torque_fault(true);
    let s = m.status();
    assert!(s.connected);
    assert!(s.torque_fault);
    assert!(!s.is_healthy());
}

#[test]
fn clearing_torque_fault_restores_health() {
    let mut m = MozaHealthMonitor::new();
    m.set_torque_fault(true);
    assert!(!m.status().is_healthy());
    m.set_torque_fault(false);
    assert!(m.status().is_healthy());
}

#[test]
fn time_since_last_success_none_initially() {
    let m = MozaHealthMonitor::new();
    assert!(m.time_since_last_success().is_none());
}

#[test]
fn time_since_last_success_some_after_record() {
    let mut m = MozaHealthMonitor::new();
    m.record_success();
    let d = m.time_since_last_success();
    assert!(d.is_some());
    // Should be very recent (< 1 second)
    assert!(d.unwrap().as_secs() < 1);
}

#[test]
fn consecutive_failures_count_increments() {
    let mut m = MozaHealthMonitor::new();
    for i in 1..=10u32 {
        m.record_failure();
        assert_eq!(m.status().consecutive_failures, i);
    }
}

#[test]
fn status_last_success_populated_after_success() {
    let mut m = MozaHealthMonitor::new();
    assert!(m.status().last_success.is_none());
    m.record_success();
    assert!(m.status().last_success.is_some());
}

// ──────────────────────────────────────────────────────────────────────────────
// Module: presets
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn preset_returns_four_axes() {
    assert_eq!(ab9_axis_config().len(), 4);
}

#[test]
fn preset_axis_names_correct() {
    let cfg = ab9_axis_config();
    assert_eq!(cfg[0].name, "roll");
    assert_eq!(cfg[1].name, "pitch");
    assert_eq!(cfg[2].name, "throttle");
    assert_eq!(cfg[3].name, "twist");
}

#[test]
fn preset_deadzones_are_positive_and_small() {
    for c in &ab9_axis_config() {
        assert!(c.deadzone > 0.0, "{}: deadzone must be positive", c.name);
        assert!(c.deadzone < 0.1, "{}: deadzone too large", c.name);
    }
}

#[test]
fn preset_servo_axes_have_no_filter() {
    let cfg = ab9_axis_config();
    assert!(cfg[0].filter_alpha.is_none(), "roll should have no filter");
    assert!(cfg[1].filter_alpha.is_none(), "pitch should have no filter");
}

#[test]
fn preset_resistive_axes_have_filter() {
    let cfg = ab9_axis_config();
    assert!(cfg[2].filter_alpha.is_some(), "throttle needs filter");
    assert!(cfg[3].filter_alpha.is_some(), "twist needs filter");
}

#[test]
fn preset_filter_alphas_in_valid_range() {
    for c in &ab9_axis_config() {
        if let Some(alpha) = c.filter_alpha {
            assert!(alpha > 0.0 && alpha < 1.0, "{}: alpha={alpha} out of (0,1)", c.name);
        }
    }
}

#[test]
fn preset_notes_are_non_empty() {
    for c in &ab9_axis_config() {
        assert!(!c.notes.is_empty(), "{}: notes should not be empty", c.name);
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Cross-module: round-trip and integration
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn parse_then_spring_torque_round_trip() {
    let r = make_report(16383, -8000, 0, 0, 0, 0);
    let s = parse_ab9_report(&r).unwrap();
    let stiffness = 0.5;
    let cmd = TorqueCommand {
        x: -s.axes.roll * stiffness,
        y: -s.axes.pitch * stiffness,
    };
    // Restoring torque opposes displacement
    assert!(cmd.x < 0.0);
    assert!(cmd.y > 0.0);
    assert!(cmd.is_safe());
}

#[test]
fn torque_command_copy_semantics() {
    let a = TorqueCommand { x: 0.3, y: -0.7 };
    let b = a; // Copy
    assert_eq!(a.to_report(), b.to_report());
}

#[test]
fn health_status_is_clone() {
    let mut m = MozaHealthMonitor::new();
    m.record_success();
    m.record_failure();
    let s1 = m.status();
    let s2 = s1.clone();
    assert_eq!(s1.connected, s2.connected);
    assert_eq!(s1.consecutive_failures, s2.consecutive_failures);
    assert_eq!(s1.torque_fault, s2.torque_fault);
}
