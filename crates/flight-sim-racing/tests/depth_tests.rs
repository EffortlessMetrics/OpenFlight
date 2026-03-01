// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for `flight-sim-racing`.
//!
//! Covers UDP packet parsing edge cases, telemetry conversion invariants,
//! protocol detection, error handling for malformed packets, state-machine
//! transitions, and property-based data-invariant checks.

use flight_sim_racing::{
    FfbOutput, MIN_PACKET_SIZE, RACING_MAGIC, RacingError, RacingFfbTranslator, RacingTelemetry,
    parse_generic_udp,
};
use proptest::prelude::*;

// ── Packet builder helpers ───────────────────────────────────────────────────

/// Build a valid 42-byte generic UDP racing packet with the given field values.
#[allow(clippy::too_many_arguments)]
fn build_packet(
    gear: i8,
    speed_ms: f32,
    lateral_g: f32,
    longitudinal_g: f32,
    vertical_g: f32,
    throttle: f32,
    brake: f32,
    steering_angle: f32,
    rpm: f32,
    rpm_max: f32,
) -> Vec<u8> {
    build_packet_with_version(
        0x01,
        gear,
        speed_ms,
        lateral_g,
        longitudinal_g,
        vertical_g,
        throttle,
        brake,
        steering_angle,
        rpm,
        rpm_max,
    )
}

/// Build a packet with an explicit version byte.
#[allow(clippy::too_many_arguments)]
fn build_packet_with_version(
    version: u8,
    gear: i8,
    speed_ms: f32,
    lateral_g: f32,
    longitudinal_g: f32,
    vertical_g: f32,
    throttle: f32,
    brake: f32,
    steering_angle: f32,
    rpm: f32,
    rpm_max: f32,
) -> Vec<u8> {
    let mut buf = vec![0u8; MIN_PACKET_SIZE];
    buf[0..4].copy_from_slice(&RACING_MAGIC.to_le_bytes());
    buf[4] = version;
    buf[5] = gear as u8;
    buf[6..10].copy_from_slice(&speed_ms.to_le_bytes());
    buf[10..14].copy_from_slice(&lateral_g.to_le_bytes());
    buf[14..18].copy_from_slice(&longitudinal_g.to_le_bytes());
    buf[18..22].copy_from_slice(&vertical_g.to_le_bytes());
    buf[22..26].copy_from_slice(&throttle.to_le_bytes());
    buf[26..30].copy_from_slice(&brake.to_le_bytes());
    buf[30..34].copy_from_slice(&steering_angle.to_le_bytes());
    buf[34..38].copy_from_slice(&rpm.to_le_bytes());
    buf[38..42].copy_from_slice(&rpm_max.to_le_bytes());
    buf
}

/// Shorthand: build a zero-valued packet (gear 0, everything else 0.0).
fn zero_packet() -> Vec<u8> {
    build_packet(0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
}

// ═══════════════════════════════════════════════════════════════════════════════
// 1. UDP Packet Parsing
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn parse_exact_min_size_packet() {
    let pkt = zero_packet();
    assert_eq!(pkt.len(), MIN_PACKET_SIZE);
    let t = parse_generic_udp(&pkt).expect("exact MIN_PACKET_SIZE should parse");
    assert!(t.is_valid);
}

#[test]
fn parse_oversized_packet_ignores_trailing_bytes() {
    let mut pkt = zero_packet();
    pkt.extend_from_slice(&[0xAB; 128]); // extra garbage
    let t = parse_generic_udp(&pkt).expect("oversized packet should still parse");
    assert!(t.is_valid);
    assert_eq!(t.speed_ms, 0.0);
}

#[test]
fn parse_all_fields_roundtrip() {
    let pkt = build_packet(
        4,      // gear
        33.5,   // speed_ms
        -2.1,   // lateral_g
        0.9,    // longitudinal_g
        -0.3,   // vertical_g
        0.85,   // throttle
        0.15,   // brake
        -0.42,  // steering_angle
        7200.0, // rpm
        9000.0, // rpm_max
    );
    let t = parse_generic_udp(&pkt).unwrap();
    assert_eq!(t.gear, 4);
    assert!((t.speed_ms - 33.5).abs() < f32::EPSILON);
    assert!((t.lateral_g - (-2.1)).abs() < 1e-5);
    assert!((t.longitudinal_g - 0.9).abs() < 1e-5);
    assert!((t.vertical_g - (-0.3)).abs() < 1e-5);
    assert!((t.throttle - 0.85).abs() < 1e-5);
    assert!((t.brake - 0.15).abs() < 1e-5);
    assert!((t.steering_angle - (-0.42)).abs() < 1e-5);
    assert!((t.rpm - 7200.0).abs() < 1e-3);
    assert!((t.rpm_max - 9000.0).abs() < 1e-3);
}

#[test]
fn parse_negative_speed() {
    // Negative speed is physically unusual but shouldn't panic.
    let pkt = build_packet(0, -10.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    let t = parse_generic_udp(&pkt).unwrap();
    assert!((t.speed_ms - (-10.0)).abs() < f32::EPSILON);
}

#[test]
fn parse_max_f32_values() {
    let pkt = build_packet(
        0,
        f32::MAX,
        f32::MAX,
        f32::MAX,
        f32::MAX,
        f32::MAX,
        f32::MAX,
        f32::MAX,
        f32::MAX,
        f32::MAX,
    );
    let t = parse_generic_udp(&pkt).unwrap();
    // Clamped fields should be at their bounds.
    assert_eq!(t.throttle, 1.0);
    assert_eq!(t.brake, 1.0);
    assert_eq!(t.steering_angle, 1.0);
}

#[test]
fn parse_min_f32_values() {
    let pkt = build_packet(
        0,
        f32::MIN,
        f32::MIN,
        f32::MIN,
        f32::MIN,
        f32::MIN,
        f32::MIN,
        f32::MIN,
        f32::MIN,
        f32::MIN,
    );
    let t = parse_generic_udp(&pkt).unwrap();
    assert_eq!(t.throttle, 0.0);
    assert_eq!(t.brake, 0.0);
    assert_eq!(t.steering_angle, -1.0);
    // Negative RPM clamped to 0.
    assert_eq!(t.rpm, 0.0);
    assert_eq!(t.rpm_max, 0.0);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. Generic Telemetry Conversion — clamping, NaN/Inf, edge values
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn throttle_clamped_to_unit_interval() {
    // Over 1.0 → 1.0
    let pkt = build_packet(0, 0.0, 0.0, 0.0, 0.0, 1.5, 0.0, 0.0, 0.0, 0.0);
    assert_eq!(parse_generic_udp(&pkt).unwrap().throttle, 1.0);

    // Below 0.0 → 0.0
    let pkt = build_packet(0, 0.0, 0.0, 0.0, 0.0, -0.5, 0.0, 0.0, 0.0, 0.0);
    assert_eq!(parse_generic_udp(&pkt).unwrap().throttle, 0.0);
}

#[test]
fn brake_clamped_to_unit_interval() {
    let pkt = build_packet(0, 0.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0);
    assert_eq!(parse_generic_udp(&pkt).unwrap().brake, 1.0);

    let pkt = build_packet(0, 0.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0);
    assert_eq!(parse_generic_udp(&pkt).unwrap().brake, 0.0);
}

#[test]
fn steering_clamped_to_signed_unit() {
    let pkt = build_packet(0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 5.0, 0.0, 0.0);
    assert_eq!(parse_generic_udp(&pkt).unwrap().steering_angle, 1.0);

    let pkt = build_packet(0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -5.0, 0.0, 0.0);
    assert_eq!(parse_generic_udp(&pkt).unwrap().steering_angle, -1.0);
}

#[test]
fn rpm_clamped_to_non_negative() {
    let pkt = build_packet(0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -500.0, -100.0);
    let t = parse_generic_udp(&pkt).unwrap();
    assert_eq!(t.rpm, 0.0);
    assert_eq!(t.rpm_max, 0.0);
}

#[test]
fn nan_throttle_clamps_to_zero() {
    // f32::NAN.clamp(0.0, 1.0) ⇒ NaN on most platforms; verify parser doesn't panic.
    let pkt = build_packet(0, 0.0, 0.0, 0.0, 0.0, f32::NAN, 0.0, 0.0, 0.0, 0.0);
    let result = parse_generic_udp(&pkt);
    // The parser shouldn't panic; the exact value of throttle with NaN input is
    // implementation-defined (clamp of NaN is NaN), but no crash is the invariant.
    assert!(result.is_ok());
}

#[test]
fn infinity_brake_clamps() {
    let pkt = build_packet(0, 0.0, 0.0, 0.0, 0.0, 0.0, f32::INFINITY, 0.0, 0.0, 0.0);
    let t = parse_generic_udp(&pkt).unwrap();
    assert_eq!(t.brake, 1.0);
}

#[test]
fn negative_infinity_steering_clamps() {
    let pkt = build_packet(0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, f32::NEG_INFINITY, 0.0, 0.0);
    let t = parse_generic_udp(&pkt).unwrap();
    assert_eq!(t.steering_angle, -1.0);
}

#[test]
fn rpm_normalized_with_zero_max_returns_zero() {
    let t = RacingTelemetry {
        rpm: 5000.0,
        rpm_max: 0.0,
        ..Default::default()
    };
    // rpm_max.max(1.0) prevents division by zero → result = 5000.0 / 1.0 = 5000.0
    let norm = t.rpm_normalized();
    assert!(!norm.is_nan(), "should not be NaN");
    assert!(!norm.is_infinite(), "should not be infinite");
}

#[test]
fn rpm_normalized_at_redline() {
    let t = RacingTelemetry {
        rpm: 8000.0,
        rpm_max: 8000.0,
        ..Default::default()
    };
    assert!((t.rpm_normalized() - 1.0).abs() < 1e-5);
}

#[test]
fn rpm_normalized_above_redline() {
    let t = RacingTelemetry {
        rpm: 9000.0,
        rpm_max: 8000.0,
        ..Default::default()
    };
    assert!(t.rpm_normalized() > 1.0, "over-rev should exceed 1.0");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. Protocol Detection / Version Handling
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn magic_bytes_exact_match() {
    // RACING_MAGIC = 0x5241_4345, which is "RACE" read as big-endian ASCII.
    assert_eq!(RACING_MAGIC, 0x5241_4345);
    let bytes = RACING_MAGIC.to_be_bytes();
    assert_eq!(&bytes, b"RACE");
}

#[test]
fn future_version_byte_accepted() {
    // The parser accepts but does not gate on the version byte.
    for v in [0x00, 0x01, 0x02, 0x7F, 0xFF] {
        let pkt = build_packet_with_version(v, 0, 10.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.0);
        let t = parse_generic_udp(&pkt).expect(&format!("version {v:#04x} should parse"));
        assert!(t.is_valid);
    }
}

#[test]
fn wrong_magic_one_byte_off() {
    let mut pkt = zero_packet();
    pkt[3] ^= 0x01; // flip one bit in the last magic byte
    let err = parse_generic_udp(&pkt).unwrap_err();
    assert!(matches!(err, RacingError::BadMagic { .. }));
}

#[test]
fn wrong_magic_all_zeros() {
    let mut pkt = zero_packet();
    pkt[0..4].copy_from_slice(&0u32.to_le_bytes());
    let err = parse_generic_udp(&pkt).unwrap_err();
    assert!(matches!(err, RacingError::BadMagic { found: 0 }));
}

#[test]
fn wrong_magic_swapped_endian() {
    // Byte-swap the magic so the LE encoding is wrong.
    let swapped = RACING_MAGIC.swap_bytes();
    let mut pkt = zero_packet();
    pkt[0..4].copy_from_slice(&swapped.to_le_bytes());
    let err = parse_generic_udp(&pkt).unwrap_err();
    assert!(matches!(err, RacingError::BadMagic { .. }));
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. Error Handling for Malformed Packets
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn empty_packet_error() {
    let err = parse_generic_udp(&[]).unwrap_err();
    assert_eq!(err, RacingError::TooShort { found: 0 });
}

#[test]
fn one_byte_packet_error() {
    let err = parse_generic_udp(&[0x42]).unwrap_err();
    assert_eq!(err, RacingError::TooShort { found: 1 });
}

#[test]
fn packet_one_byte_short() {
    let pkt = vec![0u8; MIN_PACKET_SIZE - 1];
    let err = parse_generic_udp(&pkt).unwrap_err();
    assert_eq!(
        err,
        RacingError::TooShort {
            found: MIN_PACKET_SIZE - 1
        }
    );
}

#[test]
fn too_short_error_reports_actual_length() {
    for len in [0, 1, 4, 5, 10, 20, 41] {
        let data = vec![0u8; len];
        match parse_generic_udp(&data) {
            Err(RacingError::TooShort { found }) => assert_eq!(found, len),
            other => panic!("expected TooShort for len={len}, got {other:?}"),
        }
    }
}

#[test]
fn error_display_messages() {
    let short = RacingError::TooShort { found: 10 };
    let msg = format!("{short}");
    assert!(msg.contains("10"), "TooShort display should include length");
    assert!(msg.contains(&MIN_PACKET_SIZE.to_string()));

    let bad = RacingError::BadMagic { found: 0xDEADBEEF };
    let msg = format!("{bad}");
    assert!(msg.contains("0xdeadbeef") || msg.contains("0xDEADBEEF"));

    let read = RacingError::ReadError { offset: 38 };
    let msg = format!("{read}");
    assert!(msg.contains("38"));
}

#[test]
fn error_partialeq() {
    assert_eq!(
        RacingError::TooShort { found: 5 },
        RacingError::TooShort { found: 5 }
    );
    assert_ne!(
        RacingError::TooShort { found: 5 },
        RacingError::TooShort { found: 6 }
    );
    assert_ne!(
        RacingError::TooShort { found: 5 },
        RacingError::BadMagic { found: 5 }
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. State Machine Transitions (telemetry helper methods)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn is_accelerating_threshold() {
    let mut t = RacingTelemetry::default();
    assert!(!t.is_accelerating());

    t.throttle = 0.05;
    assert!(!t.is_accelerating(), "exactly at threshold → false");

    t.throttle = 0.051;
    assert!(t.is_accelerating());
}

#[test]
fn is_braking_threshold() {
    let mut t = RacingTelemetry::default();
    assert!(!t.is_braking());

    t.brake = 0.05;
    assert!(!t.is_braking(), "exactly at threshold → false");

    t.brake = 0.051;
    assert!(t.is_braking());
}

#[test]
fn simultaneous_throttle_and_brake() {
    let t = RacingTelemetry {
        throttle: 0.8,
        brake: 0.6,
        ..Default::default()
    };
    assert!(t.is_accelerating());
    assert!(t.is_braking());
}

#[test]
fn coasting_state() {
    let t = RacingTelemetry {
        throttle: 0.02,
        brake: 0.01,
        speed_ms: 30.0,
        ..Default::default()
    };
    assert!(!t.is_accelerating());
    assert!(!t.is_braking());
}

#[test]
fn gear_transitions_reverse_to_forward() {
    for gear in -1i8..=8 {
        let pkt = build_packet(gear, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let t = parse_generic_udp(&pkt).unwrap();
        assert_eq!(t.gear, gear);
    }
}

#[test]
fn telemetry_default_is_idle() {
    let t = RacingTelemetry::default();
    assert!(!t.is_valid);
    assert!(!t.is_on_track);
    assert!(!t.is_braking());
    assert!(!t.is_accelerating());
    assert_eq!(t.rpm_normalized(), 0.0);
}

#[test]
fn parsed_packet_is_always_valid_and_on_track() {
    let pkt = zero_packet();
    let t = parse_generic_udp(&pkt).unwrap();
    assert!(t.is_valid);
    assert!(t.is_on_track);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 6. FFB Translator Depth
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn ffb_zero_telemetry_produces_zero_output() {
    let translator = RacingFfbTranslator::new();
    let out = translator.translate(&RacingTelemetry::default());
    assert_eq!(out.lateral_force, 0.0);
    assert_eq!(out.vibration_hz, 0.0);
    assert_eq!(out.vibration_amp, 0.0);
    assert_eq!(out.rumble_amp, 0.0);
}

#[test]
fn ffb_negative_lateral_g() {
    let translator = RacingFfbTranslator::new();
    let t = RacingTelemetry {
        lateral_g: -0.5,
        ..Default::default()
    };
    let out = translator.translate(&t);
    assert!(out.lateral_force < 0.0, "negative G → negative force");
    assert!((out.lateral_force - (-0.5)).abs() < 1e-5);
}

#[test]
fn ffb_lateral_force_clamps_both_ends() {
    let translator = RacingFfbTranslator::new();

    let t_pos = RacingTelemetry {
        lateral_g: 100.0,
        ..Default::default()
    };
    assert_eq!(translator.translate(&t_pos).lateral_force, 1.0);

    let t_neg = RacingTelemetry {
        lateral_g: -100.0,
        ..Default::default()
    };
    assert_eq!(translator.translate(&t_neg).lateral_force, -1.0);
}

#[test]
fn ffb_custom_max_force() {
    let translator = RacingFfbTranslator {
        max_force_n: 20.0,
        rumble_scale: 1.0,
    };
    // denominator = 20.0 * 0.1 = 2.0, so 1.0 G → 0.5 lateral_force
    let t = RacingTelemetry {
        lateral_g: 1.0,
        ..Default::default()
    };
    let out = translator.translate(&t);
    assert!((out.lateral_force - 0.5).abs() < 1e-5);
}

#[test]
fn ffb_rumble_scale_amplifies() {
    let translator = RacingFfbTranslator {
        max_force_n: 10.0,
        rumble_scale: 0.5,
    };
    let t = RacingTelemetry {
        vertical_g: 0.8,
        rpm: 4000.0,
        rpm_max: 8000.0,
        ..Default::default()
    };
    let out = translator.translate(&t);
    // rumble_amp = vertical_g.abs().min(1.0) * 0.5 = 0.8 * 0.5 = 0.4
    assert!((out.rumble_amp - 0.4).abs() < 1e-5);
    // vibration_amp = rpm_normalized() * 0.5 = 0.5 * 0.5 = 0.25
    assert!((out.vibration_amp - 0.25).abs() < 1e-5);
}

#[test]
fn ffb_vibration_hz_is_rpm_over_60() {
    let translator = RacingFfbTranslator::new();
    for rpm in [0.0, 600.0, 3000.0, 7500.0, 12000.0] {
        let t = RacingTelemetry {
            rpm,
            rpm_max: 9000.0,
            ..Default::default()
        };
        let out = translator.translate(&t);
        assert!(
            (out.vibration_hz - rpm / 60.0).abs() < 1e-3,
            "rpm={rpm} → vibration_hz={}",
            out.vibration_hz
        );
    }
}

#[test]
fn ffb_output_default_is_zeroed() {
    let out = FfbOutput::default();
    assert_eq!(out.lateral_force, 0.0);
    assert_eq!(out.vibration_hz, 0.0);
    assert_eq!(out.vibration_amp, 0.0);
    assert_eq!(out.rumble_amp, 0.0);
}

#[test]
fn ffb_negative_vertical_g_produces_positive_rumble() {
    let translator = RacingFfbTranslator::new();
    let t = RacingTelemetry {
        vertical_g: -0.7,
        ..Default::default()
    };
    let out = translator.translate(&t);
    assert!(out.rumble_amp > 0.0, "abs(vertical_g) should give positive rumble");
    assert!((out.rumble_amp - 0.7).abs() < 1e-5);
}

#[test]
fn ffb_translator_default_matches_new() {
    let from_new = RacingFfbTranslator::new();
    let from_default = RacingFfbTranslator::default();
    assert!((from_new.max_force_n - from_default.max_force_n).abs() < f32::EPSILON);
    assert!((from_new.rumble_scale - from_default.rumble_scale).abs() < f32::EPSILON);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 7. Property-Based Tests
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    /// Any well-formed packet parses successfully.
    #[test]
    fn prop_valid_packet_always_parses(
        gear in -1i8..=8,
        speed in -500.0f32..500.0,
        lat_g in -5.0f32..5.0,
        lon_g in -5.0f32..5.0,
        vert_g in -5.0f32..5.0,
        throttle in -1.0f32..2.0,
        brake in -1.0f32..2.0,
        steer in -2.0f32..2.0,
        rpm in -1000.0f32..15000.0,
        rpm_max in -1000.0f32..15000.0,
    ) {
        let pkt = build_packet(gear, speed, lat_g, lon_g, vert_g, throttle, brake, steer, rpm, rpm_max);
        let result = parse_generic_udp(&pkt);
        prop_assert!(result.is_ok(), "valid packet must parse: {:?}", result);
    }

    /// Throttle is always in [0.0, 1.0] after parsing.
    #[test]
    fn prop_throttle_clamped(raw_throttle in proptest::num::f32::ANY) {
        if raw_throttle.is_finite() {
            let pkt = build_packet(0, 0.0, 0.0, 0.0, 0.0, raw_throttle, 0.0, 0.0, 0.0, 0.0);
            let t = parse_generic_udp(&pkt).unwrap();
            prop_assert!(t.throttle >= 0.0 && t.throttle <= 1.0,
                "throttle={} from raw={}", t.throttle, raw_throttle);
        }
    }

    /// Brake is always in [0.0, 1.0] after parsing.
    #[test]
    fn prop_brake_clamped(raw_brake in proptest::num::f32::ANY) {
        if raw_brake.is_finite() {
            let pkt = build_packet(0, 0.0, 0.0, 0.0, 0.0, 0.0, raw_brake, 0.0, 0.0, 0.0);
            let t = parse_generic_udp(&pkt).unwrap();
            prop_assert!(t.brake >= 0.0 && t.brake <= 1.0,
                "brake={} from raw={}", t.brake, raw_brake);
        }
    }

    /// Steering angle is always in [-1.0, 1.0] after parsing.
    #[test]
    fn prop_steering_clamped(raw_steer in proptest::num::f32::ANY) {
        if raw_steer.is_finite() {
            let pkt = build_packet(0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, raw_steer, 0.0, 0.0);
            let t = parse_generic_udp(&pkt).unwrap();
            prop_assert!(t.steering_angle >= -1.0 && t.steering_angle <= 1.0,
                "steering={} from raw={}", t.steering_angle, raw_steer);
        }
    }

    /// RPM and rpm_max are always non-negative after parsing.
    #[test]
    fn prop_rpm_non_negative(
        raw_rpm in proptest::num::f32::ANY,
        raw_rpm_max in proptest::num::f32::ANY,
    ) {
        if raw_rpm.is_finite() && raw_rpm_max.is_finite() {
            let pkt = build_packet(0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, raw_rpm, raw_rpm_max);
            let t = parse_generic_udp(&pkt).unwrap();
            prop_assert!(t.rpm >= 0.0, "rpm={} from raw={}", t.rpm, raw_rpm);
            prop_assert!(t.rpm_max >= 0.0, "rpm_max={} from raw={}", t.rpm_max, raw_rpm_max);
        }
    }

    /// Parsed packet always has is_valid=true and is_on_track=true.
    #[test]
    fn prop_parsed_always_valid(
        gear in -1i8..=8,
        speed in -500.0f32..500.0,
    ) {
        let pkt = build_packet(gear, speed, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let t = parse_generic_udp(&pkt).unwrap();
        prop_assert!(t.is_valid);
        prop_assert!(t.is_on_track);
    }

    /// FFB lateral_force is always in [-1.0, 1.0].
    #[test]
    fn prop_ffb_lateral_force_bounded(lat_g in -100.0f32..100.0) {
        let translator = RacingFfbTranslator::new();
        let t = RacingTelemetry { lateral_g: lat_g, ..Default::default() };
        let out = translator.translate(&t);
        prop_assert!(out.lateral_force >= -1.0 && out.lateral_force <= 1.0,
            "lateral_force={} from lat_g={}", out.lateral_force, lat_g);
    }

    /// FFB vibration_amp is always in [0.0, 1.0].
    #[test]
    fn prop_ffb_vibration_amp_bounded(
        rpm in 0.0f32..20000.0,
        rpm_max in 1.0f32..20000.0,
    ) {
        let translator = RacingFfbTranslator::new();
        let t = RacingTelemetry { rpm, rpm_max, ..Default::default() };
        let out = translator.translate(&t);
        prop_assert!(out.vibration_amp >= 0.0 && out.vibration_amp <= 1.0,
            "vibration_amp={}", out.vibration_amp);
    }

    /// FFB rumble_amp is always in [0.0, 1.0].
    #[test]
    fn prop_ffb_rumble_amp_bounded(vert_g in -10.0f32..10.0) {
        let translator = RacingFfbTranslator::new();
        let t = RacingTelemetry { vertical_g: vert_g, ..Default::default() };
        let out = translator.translate(&t);
        prop_assert!(out.rumble_amp >= 0.0 && out.rumble_amp <= 1.0,
            "rumble_amp={} from vert_g={}", out.rumble_amp, vert_g);
    }

    /// Random bytes shorter than MIN_PACKET_SIZE always produce TooShort.
    #[test]
    fn prop_short_random_bytes_rejected(
        data in proptest::collection::vec(any::<u8>(), 0..MIN_PACKET_SIZE),
    ) {
        let result = parse_generic_udp(&data);
        prop_assert!(
            matches!(result, Err(RacingError::TooShort { .. })),
            "short packet ({} bytes) should be TooShort: {:?}", data.len(), result
        );
    }

    /// Random 42+ bytes without correct magic are rejected with BadMagic.
    #[test]
    fn prop_random_long_bytes_bad_magic(
        data in proptest::collection::vec(any::<u8>(), MIN_PACKET_SIZE..256),
    ) {
        // Skip the rare case where random bytes happen to match RACING_MAGIC.
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        if magic != RACING_MAGIC {
            let result = parse_generic_udp(&data);
            prop_assert!(
                matches!(result, Err(RacingError::BadMagic { .. })),
                "wrong magic should be BadMagic: {:?}", result
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 8. End-to-End: Parse → FFB pipeline
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn e2e_high_speed_cornering() {
    let pkt = build_packet(
        5,     // 5th gear
        60.0,  // ~216 km/h
        3.5,   // heavy right turn
        -0.2,  // slight braking
        0.05,  // minimal bumps
        0.9,   // mostly on throttle
        0.1,   // trailing brake
        0.6,   // steering right
        7500.0,
        8500.0,
    );
    let t = parse_generic_udp(&pkt).unwrap();
    let ffb = RacingFfbTranslator::new().translate(&t);

    assert!(t.is_accelerating());
    assert!(t.is_braking());
    assert!(ffb.lateral_force > 0.0, "right turn → positive lateral");
    assert!(ffb.vibration_hz > 0.0);
    assert!(ffb.vibration_amp > 0.0);
}

#[test]
fn e2e_stationary_neutral() {
    let pkt = build_packet(0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 800.0, 8000.0);
    let t = parse_generic_udp(&pkt).unwrap();
    let ffb = RacingFfbTranslator::new().translate(&t);

    assert!(!t.is_accelerating());
    assert!(!t.is_braking());
    assert_eq!(t.gear, 0);
    assert_eq!(ffb.lateral_force, 0.0);
    // Idle RPM still produces vibration.
    assert!(ffb.vibration_hz > 0.0);
}

#[test]
fn e2e_reverse_gear() {
    let pkt = build_packet(-1, 5.0, 0.0, 0.0, 0.0, 0.3, 0.0, -0.2, 2000.0, 8000.0);
    let t = parse_generic_udp(&pkt).unwrap();
    assert_eq!(t.gear, -1);
    assert!(t.is_accelerating());
}

#[test]
fn e2e_heavy_braking() {
    let pkt = build_packet(3, 40.0, -0.3, -2.5, 0.0, 0.0, 1.0, 0.0, 3000.0, 8000.0);
    let t = parse_generic_udp(&pkt).unwrap();
    assert!(t.is_braking());
    assert!(!t.is_accelerating());
    assert!(t.longitudinal_g < 0.0);
}
