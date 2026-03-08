// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for VIRPIL VPC ACE Collection Pedals.
//!
//! Covers axis parsing, calibration properties, profile generation,
//! and device identification.

use flight_hotas_virpil::{
    VIRPIL_ACE_PEDALS_PID, VIRPIL_AXIS_MAX, VIRPIL_VENDOR_ID, VPC_ACE_PEDALS_MIN_REPORT_BYTES,
    parse_ace_pedals_report,
};
use proptest::prelude::*;

// ─── Report builder ──────────────────────────────────────────────────────────

fn make_ace_report(axes: [u16; 3], buttons: [u8; 2]) -> Vec<u8> {
    let mut data = vec![0x01u8]; // report_id
    for ax in &axes {
        data.extend_from_slice(&ax.to_le_bytes());
    }
    data.extend_from_slice(&buttons);
    data
}

// ═══════════════════════════════════════════════════════════════════════════════
// 1. Axis parsing
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn ace_pedals_rudder_full_range() {
    let min = parse_ace_pedals_report(&make_ace_report([0, 0, 0], [0; 2])).unwrap();
    let max = parse_ace_pedals_report(&make_ace_report([VIRPIL_AXIS_MAX, 0, 0], [0; 2])).unwrap();
    assert_eq!(min.axes.rudder, 0.0);
    assert!((max.axes.rudder - 1.0).abs() < 1e-4);
}

#[test]
fn ace_pedals_left_toe_brake_range() {
    let min = parse_ace_pedals_report(&make_ace_report([0, 0, 0], [0; 2])).unwrap();
    let max = parse_ace_pedals_report(&make_ace_report([0, VIRPIL_AXIS_MAX, 0], [0; 2])).unwrap();
    assert_eq!(min.axes.left_toe_brake, 0.0);
    assert!((max.axes.left_toe_brake - 1.0).abs() < 1e-4);
}

#[test]
fn ace_pedals_right_toe_brake_range() {
    let min = parse_ace_pedals_report(&make_ace_report([0, 0, 0], [0; 2])).unwrap();
    let max = parse_ace_pedals_report(&make_ace_report([0, 0, VIRPIL_AXIS_MAX], [0; 2])).unwrap();
    assert_eq!(min.axes.right_toe_brake, 0.0);
    assert!((max.axes.right_toe_brake - 1.0).abs() < 1e-4);
}

#[test]
fn ace_pedals_differential_mode() {
    // Both brakes at different positions
    let state = parse_ace_pedals_report(&make_ace_report(
        [VIRPIL_AXIS_MAX / 2, VIRPIL_AXIS_MAX, 0],
        [0; 2],
    ))
    .unwrap();
    assert!((state.axes.rudder - 0.5).abs() < 0.01);
    assert!((state.axes.left_toe_brake - 1.0).abs() < 1e-4);
    assert_eq!(state.axes.right_toe_brake, 0.0);
}

#[test]
fn ace_pedals_14bit_resolution() {
    // VIRPIL uses 14-bit: max=16384
    assert_eq!(VIRPIL_AXIS_MAX, 16384);
    let a = parse_ace_pedals_report(&make_ace_report([1000, 0, 0], [0; 2])).unwrap();
    let b = parse_ace_pedals_report(&make_ace_report([1001, 0, 0], [0; 2])).unwrap();
    assert_ne!(a.axes.rudder, b.axes.rudder, "14-bit resolution distinguishes adjacent values");
}

#[test]
fn ace_pedals_midpoint_rudder() {
    let state = parse_ace_pedals_report(&make_ace_report(
        [VIRPIL_AXIS_MAX / 2, 0, 0],
        [0; 2],
    ))
    .unwrap();
    assert!(
        (state.axes.rudder - 0.5).abs() < 0.01,
        "midpoint should be ~0.5, got {}",
        state.axes.rudder
    );
}

#[test]
fn ace_pedals_above_max_clamped() {
    // Values > VIRPIL_AXIS_MAX should clamp to 1.0
    let state = parse_ace_pedals_report(&make_ace_report([65535, 65535, 65535], [0; 2])).unwrap();
    assert!((state.axes.rudder - 1.0).abs() < 1e-4, "above max should clamp to 1.0");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. Calibration
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn ace_pedals_linearity_monotonic() {
    let mut prev = 0.0f32;
    for raw in (0..=VIRPIL_AXIS_MAX).step_by(64) {
        let state = parse_ace_pedals_report(&make_ace_report([raw, 0, 0], [0; 2])).unwrap();
        assert!(
            state.axes.rudder >= prev,
            "monotonicity at raw={raw}: prev={prev}, cur={}",
            state.axes.rudder
        );
        prev = state.axes.rudder;
    }
}

#[test]
fn ace_pedals_no_hysteresis_in_parser() {
    let up = parse_ace_pedals_report(&make_ace_report([5000, 0, 0], [0; 2])).unwrap();
    let _peak = parse_ace_pedals_report(&make_ace_report([10000, 0, 0], [0; 2])).unwrap();
    let down = parse_ace_pedals_report(&make_ace_report([5000, 0, 0], [0; 2])).unwrap();
    assert_eq!(up.axes.rudder, down.axes.rudder, "parser should have no hysteresis");
}

#[test]
fn ace_pedals_center_symmetry() {
    let half = VIRPIL_AXIS_MAX / 2;
    let offset = 1000u16;
    let lo = parse_ace_pedals_report(&make_ace_report([half - offset, 0, 0], [0; 2])).unwrap();
    let hi = parse_ace_pedals_report(&make_ace_report([half + offset, 0, 0], [0; 2])).unwrap();
    let lo_dist = (lo.axes.rudder - 0.5).abs();
    let hi_dist = (hi.axes.rudder - 0.5).abs();
    assert!(
        (lo_dist - hi_dist).abs() < 0.01,
        "symmetric offsets should produce symmetric distances"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. Button parsing
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn ace_pedals_button_individual_bits() {
    for btn in 1..=16u8 {
        let byte_idx = ((btn - 1) / 8) as usize;
        let bit = (btn - 1) % 8;
        let mut buttons = [0u8; 2];
        buttons[byte_idx] = 1 << bit;
        let state = parse_ace_pedals_report(&make_ace_report([0; 3], buttons)).unwrap();
        assert!(state.buttons.is_pressed(btn), "button {btn} should be pressed");
        // Others should not be pressed
        for other in 1..=16u8 {
            if other != btn {
                assert!(!state.buttons.is_pressed(other), "button {other} should not be pressed when only {btn} is set");
            }
        }
    }
}

#[test]
fn ace_pedals_all_buttons_pressed() {
    let state = parse_ace_pedals_report(&make_ace_report([0; 3], [0xFF, 0xFF])).unwrap();
    let pressed = state.buttons.pressed();
    assert_eq!(pressed.len(), 16, "all 16 buttons should be pressed");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. Device identification
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn virpil_vendor_id_correct() {
    assert_eq!(VIRPIL_VENDOR_ID, 0x3344);
}

#[test]
fn virpil_ace_pedals_pid_correct() {
    assert_eq!(VIRPIL_ACE_PEDALS_PID, 0x019C);
}

#[test]
fn ace_pedals_min_report_size() {
    assert_eq!(VPC_ACE_PEDALS_MIN_REPORT_BYTES, 9, "ACE pedals report: 1 ID + 6 axes + 2 buttons");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. Error handling
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn ace_pedals_error_on_empty() {
    assert!(parse_ace_pedals_report(&[]).is_err());
}

#[test]
fn ace_pedals_error_on_short_report() {
    for len in 0..VPC_ACE_PEDALS_MIN_REPORT_BYTES {
        let data = vec![0x01; len];
        assert!(parse_ace_pedals_report(&data).is_err(), "len={len} should fail");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Proptest
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #[test]
    fn ace_pedals_axes_always_in_range(
        raw0 in 0u16..=u16::MAX,
        raw1 in 0u16..=u16::MAX,
        raw2 in 0u16..=u16::MAX,
    ) {
        let report = make_ace_report([raw0, raw1, raw2], [0; 2]);
        let state = parse_ace_pedals_report(&report).unwrap();
        prop_assert!((0.0..=1.0).contains(&state.axes.rudder));
        prop_assert!((0.0..=1.0).contains(&state.axes.left_toe_brake));
        prop_assert!((0.0..=1.0).contains(&state.axes.right_toe_brake));
    }

    #[test]
    fn ace_pedals_rudder_monotonic(a in 0u16..VIRPIL_AXIS_MAX) {
        let lo = parse_ace_pedals_report(&make_ace_report([a, 0, 0], [0; 2])).unwrap();
        let hi = parse_ace_pedals_report(&make_ace_report([a + 1, 0, 0], [0; 2])).unwrap();
        prop_assert!(hi.axes.rudder >= lo.axes.rudder);
    }

    #[test]
    fn ace_pedals_random_no_panic(data in proptest::collection::vec(0u8..=255u8, 0..32)) {
        let _ = parse_ace_pedals_report(&data);
    }
}
