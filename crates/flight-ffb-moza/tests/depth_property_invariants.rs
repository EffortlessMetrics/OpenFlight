// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Property-based depth tests for Moza force invariants — covers torque
//! encoding round-trip properties, clamping invariants, and safety predicates.

use flight_ffb_moza::effects::{TORQUE_REPORT_ID, TORQUE_REPORT_LEN, TorqueCommand};
use flight_ffb_moza::input::{AB9_REPORT_LEN, parse_ab9_report};
use proptest::prelude::*;

fn decode_x(r: &[u8; TORQUE_REPORT_LEN]) -> i16 {
    i16::from_le_bytes([r[1], r[2]])
}

fn decode_y(r: &[u8; TORQUE_REPORT_LEN]) -> i16 {
    i16::from_le_bytes([r[3], r[4]])
}

proptest! {
    /// Every torque report starts with the correct report ID.
    #[test]
    fn prop_report_id_always_correct(x in -10.0f32..10.0, y in -10.0f32..10.0) {
        let r = TorqueCommand { x, y }.to_report();
        prop_assert_eq!(r[0], TORQUE_REPORT_ID);
    }

    /// Serialised X raw value is always within i16 safe range after clamping.
    #[test]
    fn prop_x_raw_in_i16_range(x in -100.0f32..100.0) {
        let r = TorqueCommand { x, y: 0.0 }.to_report();
        let raw = decode_x(&r) as i32;
        prop_assert!((-32767..=32767).contains(&raw), "raw={}", raw);
    }

    /// Serialised Y raw value is always within i16 safe range after clamping.
    #[test]
    fn prop_y_raw_in_i16_range(y in -100.0f32..100.0) {
        let r = TorqueCommand { x: 0.0, y }.to_report();
        let raw = decode_y(&r) as i32;
        prop_assert!((-32767..=32767).contains(&raw), "raw={}", raw);
    }

    /// is_safe returns true iff both components are within [-1.0, 1.0].
    #[test]
    fn prop_is_safe_agrees_with_range(x in -2.0f32..2.0, y in -2.0f32..2.0) {
        let cmd = TorqueCommand { x, y };
        let expected = x.abs() <= 1.0 && y.abs() <= 1.0;
        prop_assert_eq!(cmd.is_safe(), expected, "x={}, y={}", x, y);
    }

    /// X torque is monotonically non-decreasing for increasing input in [-1, 1].
    #[test]
    fn prop_x_monotonic(a in -1.0f32..1.0, b in -1.0f32..1.0) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        let r_lo = TorqueCommand { x: lo, y: 0.0 }.to_report();
        let r_hi = TorqueCommand { x: hi, y: 0.0 }.to_report();
        prop_assert!(decode_x(&r_hi) >= decode_x(&r_lo),
            "not monotonic: x={} → {}, x={} → {}", lo, decode_x(&r_lo), hi, decode_x(&r_hi));
    }

    /// Y torque is monotonically non-decreasing for increasing input in [-1, 1].
    #[test]
    fn prop_y_monotonic(a in -1.0f32..1.0, b in -1.0f32..1.0) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        let r_lo = TorqueCommand { x: 0.0, y: lo }.to_report();
        let r_hi = TorqueCommand { x: 0.0, y: hi }.to_report();
        prop_assert!(decode_y(&r_hi) >= decode_y(&r_lo));
    }

    /// Padding byte (index 5) is always zero.
    #[test]
    fn prop_padding_byte_zero(x in -10.0f32..10.0, y in -10.0f32..10.0) {
        let r = TorqueCommand { x, y }.to_report();
        prop_assert_eq!(r[5], 0, "padding byte should always be 0");
    }

    /// Clamped torque magnitude never exceeds hardware maximum.
    #[test]
    fn prop_clamped_magnitude_bounded(x in -1000.0f32..1000.0, y in -1000.0f32..1000.0) {
        let r = TorqueCommand { x, y }.to_report();
        let x_raw = decode_x(&r).unsigned_abs();
        let y_raw = decode_y(&r).unsigned_abs();
        prop_assert!(x_raw <= 32767);
        prop_assert!(y_raw <= 32767);
    }

    /// Spring-centering torque opposes displacement for any roll value.
    #[test]
    fn prop_spring_centering_opposes_roll(roll_raw in 1i16..=i16::MAX) {
        let mut r = [0u8; AB9_REPORT_LEN];
        r[0] = 0x01;
        r[1..3].copy_from_slice(&roll_raw.to_le_bytes());
        let state = parse_ab9_report(&r).unwrap();
        let torque = TorqueCommand { x: -state.axes.roll * 0.5, y: 0.0 };
        prop_assert!(torque.x < 0.0, "spring must oppose positive roll");
        prop_assert!(torque.is_safe());
    }

    /// Spring-centering torque opposes negative displacement too.
    #[test]
    fn prop_spring_centering_opposes_negative_roll(roll_raw in i16::MIN..=-1i16) {
        let mut r = [0u8; AB9_REPORT_LEN];
        r[0] = 0x01;
        r[1..3].copy_from_slice(&roll_raw.to_le_bytes());
        let state = parse_ab9_report(&r).unwrap();
        let torque = TorqueCommand { x: -state.axes.roll * 0.5, y: 0.0 };
        prop_assert!(torque.x > 0.0, "spring must oppose negative roll");
    }
}
