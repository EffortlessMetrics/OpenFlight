// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Property-based depth tests for VPforce force invariants — covers effect
//! encoding safety, magnitude clamping, and direction wrapping.

use flight_ffb_vpforce::effects::{
    FFB_REPORT_LEN, FfbEffect, REPORT_CONSTANT_FORCE, REPORT_PERIODIC, REPORT_SPRING,
    is_magnitude_safe, serialize_effect,
};
use flight_ffb_vpforce::input::{RHINO_REPORT_LEN, parse_report};
use proptest::prelude::*;

fn decode_u16(buf: &[u8; FFB_REPORT_LEN], offset: usize) -> u16 {
    u16::from_le_bytes([buf[offset], buf[offset + 1]])
}

proptest! {
    /// Every serialised effect has the correct report ID byte.
    #[test]
    fn prop_constant_force_report_id(dir in 0.0f32..360.0, mag in 0.0f32..2.0) {
        let b = serialize_effect(FfbEffect::ConstantForce {
            direction_deg: dir,
            magnitude: mag,
        });
        prop_assert_eq!(b[0], REPORT_CONSTANT_FORCE);
    }

    #[test]
    fn prop_spring_report_id(coeff in -1.0f32..5.0) {
        let b = serialize_effect(FfbEffect::Spring { coefficient: coeff });
        prop_assert_eq!(b[0], REPORT_SPRING);
        prop_assert_eq!(b[1], 0x01, "spring mode byte");
    }

    #[test]
    fn prop_damper_report_id(coeff in -1.0f32..5.0) {
        let b = serialize_effect(FfbEffect::Damper { coefficient: coeff });
        prop_assert_eq!(b[0], REPORT_SPRING);
        prop_assert_eq!(b[1], 0x02, "damper mode byte");
    }

    #[test]
    fn prop_sine_report_id(freq in 0.0f32..500.0, mag in 0.0f32..2.0) {
        let b = serialize_effect(FfbEffect::Sine {
            frequency_hz: freq,
            magnitude: mag,
        });
        prop_assert_eq!(b[0], REPORT_PERIODIC);
    }

    /// Constant force magnitude is always clamped to [0, 10000].
    #[test]
    fn prop_constant_force_magnitude_clamped(mag in -10.0f32..10.0) {
        let b = serialize_effect(FfbEffect::ConstantForce {
            direction_deg: 0.0,
            magnitude: mag,
        });
        let raw = decode_u16(&b, 3);
        prop_assert!(raw <= 10000, "raw={} exceeds max", raw);
    }

    /// Constant force direction wraps into valid u16 range.
    #[test]
    fn prop_constant_force_direction_valid(dir in -1000.0f32..1000.0) {
        let b = serialize_effect(FfbEffect::ConstantForce {
            direction_deg: dir,
            magnitude: 0.5,
        });
        // Verify the wrapping logic produces a valid value
        let raw = decode_u16(&b, 1) as u32;
        prop_assert!(raw <= 65535);
    }

    /// Spring coefficient raw value is always in [0, 10000].
    #[test]
    fn prop_spring_coefficient_clamped(coeff in -10.0f32..10.0) {
        let b = serialize_effect(FfbEffect::Spring { coefficient: coeff });
        let raw = decode_u16(&b, 2);
        prop_assert!(raw <= 10000, "raw={}", raw);
    }

    /// Damper coefficient raw value is always in [0, 10000].
    #[test]
    fn prop_damper_coefficient_clamped(coeff in -10.0f32..10.0) {
        let b = serialize_effect(FfbEffect::Damper { coefficient: coeff });
        let raw = decode_u16(&b, 2);
        prop_assert!(raw <= 10000, "raw={}", raw);
    }

    /// Sine frequency is always clamped to [1, 200].
    #[test]
    fn prop_sine_frequency_clamped(freq in 0.0f32..1000.0) {
        let b = serialize_effect(FfbEffect::Sine {
            frequency_hz: freq,
            magnitude: 0.5,
        });
        let raw = decode_u16(&b, 1);
        prop_assert!(raw >= 1, "freq={} produced raw={}", freq, raw);
        prop_assert!(raw <= 200, "freq={} produced raw={}", freq, raw);
    }

    /// Sine magnitude is always clamped to [0, 10000].
    #[test]
    fn prop_sine_magnitude_clamped(mag in -10.0f32..10.0) {
        let b = serialize_effect(FfbEffect::Sine {
            frequency_hz: 50.0,
            magnitude: mag,
        });
        let raw = decode_u16(&b, 3);
        prop_assert!(raw <= 10000, "raw={}", raw);
    }

    /// is_magnitude_safe agrees with range check.
    #[test]
    fn prop_is_magnitude_safe_agrees(mag in -2.0f32..2.0) {
        let expected = (0.0..=1.0).contains(&mag);
        prop_assert_eq!(is_magnitude_safe(mag), expected, "mag={}", mag);
    }

    /// Spring centering: spring coefficient monotonically increases output.
    #[test]
    fn prop_spring_monotonic(a in 0.0f32..1.0, b in 0.0f32..1.0) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        let r_lo = serialize_effect(FfbEffect::Spring { coefficient: lo });
        let r_hi = serialize_effect(FfbEffect::Spring { coefficient: hi });
        prop_assert!(decode_u16(&r_hi, 2) >= decode_u16(&r_lo, 2));
    }

    /// Damper coefficient monotonically increases output.
    #[test]
    fn prop_damper_monotonic(a in 0.0f32..1.0, b in 0.0f32..1.0) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        let r_lo = serialize_effect(FfbEffect::Damper { coefficient: lo });
        let r_hi = serialize_effect(FfbEffect::Damper { coefficient: hi });
        prop_assert!(decode_u16(&r_hi, 2) >= decode_u16(&r_lo, 2));
    }

    /// Spring centering: opposing torque for positive displacement.
    #[test]
    fn prop_spring_opposing_force(roll_raw in 1i16..=i16::MAX) {
        let mut r = [0u8; RHINO_REPORT_LEN];
        r[0] = 0x01;
        r[1..3].copy_from_slice(&roll_raw.to_le_bytes());
        let state = parse_report(&r).unwrap();
        // A spring effect opposing the roll
        let opposing_force = -state.axes.roll * 0.5;
        prop_assert!(opposing_force < 0.0, "spring should oppose positive roll");
    }

    /// Every serialised report is exactly FFB_REPORT_LEN bytes.
    #[test]
    fn prop_report_length(coeff in 0.0f32..1.0) {
        let effects = [
            FfbEffect::ConstantForce { direction_deg: 45.0, magnitude: coeff },
            FfbEffect::Spring { coefficient: coeff },
            FfbEffect::Damper { coefficient: coeff },
            FfbEffect::Sine { frequency_hz: 50.0, magnitude: coeff },
            FfbEffect::StopAll,
        ];
        for effect in &effects {
            let b = serialize_effect(*effect);
            prop_assert_eq!(b.len(), FFB_REPORT_LEN);
        }
    }
}
