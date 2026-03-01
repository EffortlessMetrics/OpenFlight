// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for Moza AB9 safety envelope — verifies torque clamping,
//! boundary behaviour, and ensures hardware over-drive cannot occur.

use flight_ffb_moza::effects::{TorqueCommand, TORQUE_REPORT_LEN};

fn decode_x(r: &[u8; TORQUE_REPORT_LEN]) -> i16 {
    i16::from_le_bytes([r[1], r[2]])
}

fn decode_y(r: &[u8; TORQUE_REPORT_LEN]) -> i16 {
    i16::from_le_bytes([r[3], r[4]])
}

// ── Safety predicate at boundaries ──────────────────────────────────────

#[test]
fn exactly_plus_one_is_safe() {
    assert!(TorqueCommand { x: 1.0, y: 0.0 }.is_safe());
    assert!(TorqueCommand { x: 0.0, y: 1.0 }.is_safe());
    assert!(TorqueCommand { x: 1.0, y: 1.0 }.is_safe());
}

#[test]
fn exactly_minus_one_is_safe() {
    assert!(TorqueCommand { x: -1.0, y: 0.0 }.is_safe());
    assert!(TorqueCommand { x: 0.0, y: -1.0 }.is_safe());
    assert!(TorqueCommand { x: -1.0, y: -1.0 }.is_safe());
}

#[test]
fn just_outside_range_is_unsafe() {
    assert!(!TorqueCommand { x: 1.001, y: 0.0 }.is_safe());
    assert!(!TorqueCommand { x: 0.0, y: -1.001 }.is_safe());
    assert!(!TorqueCommand { x: -1.001, y: 1.001 }.is_safe());
}

#[test]
fn infinity_is_unsafe() {
    assert!(!TorqueCommand { x: f32::INFINITY, y: 0.0 }.is_safe());
    assert!(!TorqueCommand { x: 0.0, y: f32::NEG_INFINITY }.is_safe());
}

#[test]
fn nan_is_unsafe() {
    assert!(!TorqueCommand { x: f32::NAN, y: 0.0 }.is_safe());
    assert!(!TorqueCommand { x: 0.0, y: f32::NAN }.is_safe());
}

// ── Clamping prevents hardware over-drive ───────────────────────────────

#[test]
fn clamping_limits_positive_overflow() {
    let cmd = TorqueCommand { x: 5.0, y: 10.0 };
    let r = cmd.to_report();
    let x = decode_x(&r);
    let y = decode_y(&r);
    assert_eq!(x, 32767, "x must be clamped to i16 max");
    assert_eq!(y, 32767, "y must be clamped to i16 max");
}

#[test]
fn clamping_limits_negative_overflow() {
    let cmd = TorqueCommand { x: -5.0, y: -10.0 };
    let r = cmd.to_report();
    let x = decode_x(&r);
    let y = decode_y(&r);
    assert_eq!(x, -32767, "x must be clamped to -i16 max");
    assert_eq!(y, -32767, "y must be clamped to -i16 max");
}

#[test]
fn infinity_clamped_in_report() {
    let cmd = TorqueCommand { x: f32::INFINITY, y: f32::NEG_INFINITY };
    let r = cmd.to_report();
    let x = decode_x(&r);
    let y = decode_y(&r);
    assert_eq!(x, 32767);
    assert_eq!(y, -32767);
}

#[test]
fn nan_clamped_in_report() {
    // In Rust, f32::clamp propagates NaN and float→int casts saturate,
    // so (f32::NAN * 32767.0) as i16 is defined to be 0.
    let cmd = TorqueCommand { x: f32::NAN, y: f32::NAN };
    let r = cmd.to_report();
    let x = decode_x(&r) as i32;
    let y = decode_y(&r) as i32;
    // The serialised value must not exceed hardware limits
    assert!(x.abs() <= 32767);
    assert!(y.abs() <= 32767);
}

// ── Report never exceeds i16 range ──────────────────────────────────────

#[test]
fn extreme_torque_values_always_produce_valid_i16() {
    let test_values = [
        0.0, 0.5, -0.5, 1.0, -1.0, 0.999, -0.999,
        2.0, -2.0, 100.0, -100.0, f32::INFINITY, f32::NEG_INFINITY,
    ];
    for &x in &test_values {
        for &y in &test_values {
            let cmd = TorqueCommand { x, y };
            let r = cmd.to_report();
            let x_raw = decode_x(&r) as i32;
            let y_raw = decode_y(&r) as i32;
            assert!(
                (-32767..=32767).contains(&x_raw),
                "x_raw={x_raw} out of safe range for x={x}, y={y}"
            );
            assert!(
                (-32767..=32767).contains(&y_raw),
                "y_raw={y_raw} out of safe range for x={x}, y={y}"
            );
        }
    }
}

// ── Monotonicity: increasing input → increasing output ──────────────────

#[test]
fn x_torque_monotonically_increasing() {
    let values: Vec<f32> = (-10..=10).map(|i| i as f32 * 0.1).collect();
    let raw_values: Vec<i16> = values
        .iter()
        .map(|&x| {
            let r = TorqueCommand { x, y: 0.0 }.to_report();
            decode_x(&r)
        })
        .collect();
    for i in 1..raw_values.len() {
        assert!(
            raw_values[i] >= raw_values[i - 1],
            "x torque not monotonic at i={}: {} < {}",
            i,
            raw_values[i],
            raw_values[i - 1]
        );
    }
}

#[test]
fn y_torque_monotonically_increasing() {
    let values: Vec<f32> = (-10..=10).map(|i| i as f32 * 0.1).collect();
    let raw_values: Vec<i16> = values
        .iter()
        .map(|&y| {
            let r = TorqueCommand { x: 0.0, y }.to_report();
            decode_y(&r)
        })
        .collect();
    for i in 1..raw_values.len() {
        assert!(
            raw_values[i] >= raw_values[i - 1],
            "y torque not monotonic at i={i}"
        );
    }
}
