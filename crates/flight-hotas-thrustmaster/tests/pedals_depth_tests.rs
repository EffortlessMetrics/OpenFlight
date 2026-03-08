// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for Thrustmaster TFRP rudder pedals.
//!
//! Covers axis parsing, calibration properties, profile generation,
//! device identification, and TFRP-specific quirks (plastic flex compensation).

use flight_hotas_thrustmaster::profiles::{device_profile, AxisNormalization};
use flight_hotas_thrustmaster::protocol::{ThrustmasterDevice, VENDOR_ID, identify_device};
use flight_hotas_thrustmaster::{
    TFRP_MIN_REPORT_BYTES, THRUSTMASTER_VENDOR_ID, TFRP_RUDDER_PEDALS_PID, T_RUDDER_PID,
    TPR_PENDULAR_RUDDER_PID, TPR_PENDULAR_RUDDER_BULK_PID,
    parse_tfrp_report, parse_tpr_report,
};
use proptest::prelude::*;

// ─── Report builders ─────────────────────────────────────────────────────────

fn make_tfrp_report(rz: u16, z: u16, rx: u16) -> Vec<u8> {
    let mut data = Vec::with_capacity(6);
    data.extend_from_slice(&rz.to_le_bytes());
    data.extend_from_slice(&z.to_le_bytes());
    data.extend_from_slice(&rx.to_le_bytes());
    data
}

// ═══════════════════════════════════════════════════════════════════════════════
// 1. Axis parsing (TFRP-specific depth)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn tfrp_rudder_axis_full_range() {
    let left = parse_tfrp_report(&make_tfrp_report(0, 0, 0)).unwrap();
    let center = parse_tfrp_report(&make_tfrp_report(32767, 0, 0)).unwrap();
    let right = parse_tfrp_report(&make_tfrp_report(65535, 0, 0)).unwrap();

    assert_eq!(left.axes.rudder, 0.0, "full-left rudder should be 0.0");
    assert!((center.axes.rudder - 0.5).abs() < 0.01, "center rudder should be ~0.5");
    assert!((right.axes.rudder - 1.0).abs() < 1e-4, "full-right rudder should be 1.0");
}

#[test]
fn tfrp_left_toe_brake_independent() {
    let state = parse_tfrp_report(&make_tfrp_report(32767, 0, 65535)).unwrap();
    assert!((state.axes.left_pedal - 1.0).abs() < 1e-4, "left toe fully pressed");
    assert_eq!(state.axes.right_pedal, 0.0, "right toe released");
    assert!((state.axes.rudder - 0.5).abs() < 0.01, "rudder at center");
}

#[test]
fn tfrp_right_toe_brake_independent() {
    let state = parse_tfrp_report(&make_tfrp_report(32767, 65535, 0)).unwrap();
    assert!((state.axes.right_pedal - 1.0).abs() < 1e-4, "right toe fully pressed");
    assert_eq!(state.axes.left_pedal, 0.0, "left toe released");
}

#[test]
fn tfrp_differential_braking_both_pressed() {
    let state = parse_tfrp_report(&make_tfrp_report(32767, 65535, 65535)).unwrap();
    assert!((state.axes.right_pedal - 1.0).abs() < 1e-4);
    assert!((state.axes.left_pedal - 1.0).abs() < 1e-4);
}

#[test]
fn tfrp_combined_mode_rudder_with_brakes() {
    // In combined mode, rudder moves while toe brakes are pressed
    let state = parse_tfrp_report(&make_tfrp_report(65535, 32767, 32767)).unwrap();
    assert!((state.axes.rudder - 1.0).abs() < 1e-4, "full right rudder");
    assert!((state.axes.right_pedal - 0.5).abs() < 0.01, "right brake half");
    assert!((state.axes.left_pedal - 0.5).abs() < 0.01, "left brake half");
}

#[test]
fn tfrp_axis_resolution_16bit() {
    // Verify 16-bit resolution: adjacent values produce different outputs
    let a = parse_tfrp_report(&make_tfrp_report(1000, 0, 0)).unwrap();
    let b = parse_tfrp_report(&make_tfrp_report(1001, 0, 0)).unwrap();
    assert_ne!(a.axes.rudder, b.axes.rudder, "16-bit resolution must distinguish adjacent values");
}

#[test]
fn tfrp_dead_center_region() {
    // Values near center should be close to 0.5
    for raw in [32700u16, 32767, 32800] {
        let state = parse_tfrp_report(&make_tfrp_report(raw, 0, 0)).unwrap();
        assert!(
            (state.axes.rudder - 0.5).abs() < 0.01,
            "raw={raw}: rudder={} should be near 0.5",
            state.axes.rudder
        );
    }
}

#[test]
fn tfrp_report_too_short_errors() {
    for len in 0..TFRP_MIN_REPORT_BYTES {
        let data = vec![0u8; len];
        assert!(parse_tfrp_report(&data).is_err(), "len={len} should fail");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. Calibration properties
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn tfrp_center_calibration_symmetry() {
    // Check that values equidistant from center produce symmetric outputs
    let left_offset = parse_tfrp_report(&make_tfrp_report(32767 - 1000, 0, 0)).unwrap();
    let right_offset = parse_tfrp_report(&make_tfrp_report(32767 + 1000, 0, 0)).unwrap();
    let left_dist = (left_offset.axes.rudder - 0.5).abs();
    let right_dist = (right_offset.axes.rudder - 0.5).abs();
    assert!(
        (left_dist - right_dist).abs() < 0.001,
        "symmetric offsets should produce symmetric distances: left={left_dist}, right={right_dist}"
    );
}

#[test]
fn tfrp_range_calibration_full_span() {
    let min = parse_tfrp_report(&make_tfrp_report(0, 0, 0)).unwrap();
    let max = parse_tfrp_report(&make_tfrp_report(65535, 65535, 65535)).unwrap();
    let span = max.axes.rudder - min.axes.rudder;
    assert!(
        (span - 1.0).abs() < 1e-4,
        "full range should span exactly 1.0, got {span}"
    );
}

#[test]
fn tfrp_linearity_monotonic_increase() {
    let mut prev = 0.0f32;
    for raw in (0..=65535u16).step_by(256) {
        let state = parse_tfrp_report(&make_tfrp_report(raw, 0, 0)).unwrap();
        assert!(
            state.axes.rudder >= prev,
            "rudder must monotonically increase: raw={raw}, prev={prev}, cur={}",
            state.axes.rudder
        );
        prev = state.axes.rudder;
    }
}

#[test]
fn tfrp_hysteresis_no_overshoot() {
    // Stepping up then down should return to same value (no hysteresis in parser)
    let up = parse_tfrp_report(&make_tfrp_report(30000, 0, 0)).unwrap();
    let peak = parse_tfrp_report(&make_tfrp_report(40000, 0, 0)).unwrap();
    let down = parse_tfrp_report(&make_tfrp_report(30000, 0, 0)).unwrap();
    assert_eq!(up.axes.rudder, down.axes.rudder, "no hysteresis in parser");
    assert!(peak.axes.rudder > up.axes.rudder);
}

#[test]
fn tfrp_temperature_drift_tolerance() {
    // Small drift (±50 counts) near center should stay within acceptable tolerance
    let nominal = parse_tfrp_report(&make_tfrp_report(32767, 0, 0)).unwrap();
    let drifted_pos = parse_tfrp_report(&make_tfrp_report(32817, 0, 0)).unwrap();
    let drifted_neg = parse_tfrp_report(&make_tfrp_report(32717, 0, 0)).unwrap();
    let max_drift = (drifted_pos.axes.rudder - nominal.axes.rudder)
        .abs()
        .max((drifted_neg.axes.rudder - nominal.axes.rudder).abs());
    assert!(
        max_drift < 0.01,
        "±50 count drift should be < 1%: {max_drift}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. Profile generation
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn tfrp_profile_has_three_axes() {
    let p = device_profile(ThrustmasterDevice::TfrpRudderPedals).unwrap();
    assert_eq!(p.axes.len(), 3, "TFRP should have rudder + 2 toe brakes");
}

#[test]
fn tfrp_profile_axes_are_unipolar() {
    let p = device_profile(ThrustmasterDevice::TfrpRudderPedals).unwrap();
    for ax in &p.axes {
        assert!(
            matches!(ax.normalization, AxisNormalization::Unipolar { .. }),
            "TFRP axis '{}' should be unipolar (0.0-1.0)",
            ax.id
        );
    }
}

#[test]
fn tfrp_profile_has_filter_alpha() {
    let p = device_profile(ThrustmasterDevice::TfrpRudderPedals).unwrap();
    for ax in &p.axes {
        assert!(
            ax.filter_alpha.is_some(),
            "TFRP axis '{}' should have EMA filter (plastic flex compensation)",
            ax.id
        );
    }
}

#[test]
fn tfrp_profile_deadzones_reasonable() {
    let p = device_profile(ThrustmasterDevice::TfrpRudderPedals).unwrap();
    for ax in &p.axes {
        assert!(
            (0.01..=0.10).contains(&ax.deadzone),
            "TFRP axis '{}' deadzone {} not in reasonable range",
            ax.id,
            ax.deadzone
        );
    }
}

#[test]
fn tfrp_profile_no_buttons_or_hats() {
    let p = device_profile(ThrustmasterDevice::TfrpRudderPedals).unwrap();
    assert_eq!(p.button_count, 0, "TFRP has no buttons");
    assert_eq!(p.hat_count, 0, "TFRP has no hat switches");
}

#[test]
fn tpr_profile_differs_from_tfrp() {
    let tfrp = device_profile(ThrustmasterDevice::TfrpRudderPedals).unwrap();
    let tpr = device_profile(ThrustmasterDevice::TprPendular).unwrap();
    assert_ne!(tfrp.name, tpr.name, "TFRP and TPR should have different names");
    // TPR has no filter (pendular design, no plastic flex)
    for ax in &tpr.axes {
        assert!(
            ax.filter_alpha.is_none(),
            "TPR axis '{}' should have no filter (no plastic flex)",
            ax.id
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. Device identification
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn tfrp_vid_pid_identification() {
    let dev = identify_device(VENDOR_ID, TFRP_RUDDER_PEDALS_PID);
    assert_eq!(dev, Some(ThrustmasterDevice::TfrpRudderPedals));
}

#[test]
fn t_rudder_vid_pid_identification() {
    let dev = identify_device(VENDOR_ID, T_RUDDER_PID);
    assert_eq!(dev, Some(ThrustmasterDevice::TRudder));
}

#[test]
fn tpr_standard_vid_pid_identification() {
    let dev = identify_device(VENDOR_ID, TPR_PENDULAR_RUDDER_PID);
    assert_eq!(dev, Some(ThrustmasterDevice::TprPendular));
}

#[test]
fn tpr_bulk_vid_pid_identification() {
    let dev = identify_device(VENDOR_ID, TPR_PENDULAR_RUDDER_BULK_PID);
    assert_eq!(dev, Some(ThrustmasterDevice::TprPendularBulk));
}

#[test]
fn thrustmaster_pedal_model_discrimination() {
    let pids = [TFRP_RUDDER_PEDALS_PID, T_RUDDER_PID, TPR_PENDULAR_RUDDER_PID, TPR_PENDULAR_RUDDER_BULK_PID];
    let devices: Vec<_> = pids
        .iter()
        .map(|&pid| identify_device(VENDOR_ID, pid).unwrap())
        .collect();
    // TFRP and T-Rudder are distinct
    assert_ne!(devices[0], devices[1], "TFRP and T-Rudder should be distinct");
    // TPR standard and bulk are distinct
    assert_ne!(devices[2], devices[3], "TPR standard and bulk should be distinct");
    assert_eq!(THRUSTMASTER_VENDOR_ID, 0x044F, "VID should be 0x044F");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. TFRP plastic flex compensation quirk
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn tfrp_plastic_flex_filter_present() {
    let p = device_profile(ThrustmasterDevice::TfrpRudderPedals).unwrap();
    let rudder = p.axes.iter().find(|a| a.id == "rudder").unwrap();
    let alpha = rudder.filter_alpha.expect("TFRP rudder must have EMA filter for plastic flex");
    assert!(
        (0.05..=0.20).contains(&alpha),
        "TFRP filter alpha {} out of expected range for plastic flex compensation",
        alpha
    );
}

#[test]
fn tfrp_plastic_flex_higher_deadzone_than_tpr() {
    let tfrp = device_profile(ThrustmasterDevice::TfrpRudderPedals).unwrap();
    let tpr = device_profile(ThrustmasterDevice::TprPendular).unwrap();
    let tfrp_dz = tfrp.axes.iter().find(|a| a.id == "rudder").unwrap().deadzone;
    let tpr_dz = tpr.axes.iter().find(|a| a.id == "rudder").unwrap().deadzone;
    assert!(
        tfrp_dz >= tpr_dz,
        "TFRP deadzone ({tfrp_dz}) should be >= TPR deadzone ({tpr_dz}) due to plastic flex"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Proptest invariants
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #[test]
    fn tfrp_axes_always_in_unit_range(
        rz in 0u16..=u16::MAX,
        z in 0u16..=u16::MAX,
        rx in 0u16..=u16::MAX,
    ) {
        let report = make_tfrp_report(rz, z, rx);
        let state = parse_tfrp_report(&report).unwrap();
        prop_assert!((0.0..=1.0).contains(&state.axes.rudder));
        prop_assert!((0.0..=1.0).contains(&state.axes.right_pedal));
        prop_assert!((0.0..=1.0).contains(&state.axes.left_pedal));
    }

    #[test]
    fn tfrp_rudder_monotonic(a in 0u16..65535u16) {
        let lo = parse_tfrp_report(&make_tfrp_report(a, 0, 0)).unwrap();
        let hi = parse_tfrp_report(&make_tfrp_report(a + 1, 0, 0)).unwrap();
        prop_assert!(hi.axes.rudder >= lo.axes.rudder, "monotonicity violated at {a}");
    }

    #[test]
    fn tfrp_random_report_no_panic(data in proptest::collection::vec(0u8..=255u8, 0..32)) {
        let _ = parse_tfrp_report(&data);
    }

    #[test]
    fn tpr_axes_always_in_unit_range(
        rz in 0u16..=u16::MAX,
        z in 0u16..=u16::MAX,
        rx in 0u16..=u16::MAX,
    ) {
        let mut report = Vec::with_capacity(6);
        report.extend_from_slice(&rz.to_le_bytes());
        report.extend_from_slice(&z.to_le_bytes());
        report.extend_from_slice(&rx.to_le_bytes());
        let state = parse_tpr_report(&report).unwrap();
        prop_assert!((0.0..=1.0).contains(&state.axes.rudder));
        prop_assert!((0.0..=1.0).contains(&state.axes.right_pedal));
        prop_assert!((0.0..=1.0).contains(&state.axes.left_pedal));
    }
}
