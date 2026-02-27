// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests for `flight-sim-racing`.
//!
//! All tests use hand-constructed packets — no network socket is needed.

use flight_sim_racing::{
    FfbOutput, MIN_PACKET_SIZE, RACING_MAGIC, RacingError, RacingFfbTranslator, RacingTelemetry,
    parse_generic_udp,
};

// ── Helper ────────────────────────────────────────────────────────────────────

/// Build a valid 42-byte generic UDP racing packet.
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
    let mut buf = vec![0u8; MIN_PACKET_SIZE];
    buf[0..4].copy_from_slice(&RACING_MAGIC.to_le_bytes());
    buf[4] = 0x01; // version
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

// ── Tests ─────────────────────────────────────────────────────────────────────

/// A correctly formed packet is parsed with all fields intact.
#[test]
fn test_parse_generic_udp_valid() {
    let pkt = build_packet(3, 55.0, 1.2, -0.8, 0.1, 0.7, 0.0, 0.3, 6000.0, 8000.0);
    let t = parse_generic_udp(&pkt).expect("valid packet");
    assert!((t.speed_ms - 55.0).abs() < 0.001, "speed_ms={}", t.speed_ms);
    assert!(
        (t.lateral_g - 1.2).abs() < 0.001,
        "lateral_g={}",
        t.lateral_g
    );
    assert!((t.longitudinal_g - (-0.8)).abs() < 0.001);
    assert!((t.throttle - 0.7).abs() < 0.001, "throttle={}", t.throttle);
    assert!((t.rpm - 6000.0).abs() < 0.001, "rpm={}", t.rpm);
    assert!((t.rpm_max - 8000.0).abs() < 0.001, "rpm_max={}", t.rpm_max);
    assert!(t.is_valid);
    assert!(t.is_on_track);
}

/// A wrong magic number produces a `BadMagic` error without panicking.
#[test]
fn test_parse_generic_udp_wrong_magic() {
    let mut pkt = build_packet(1, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    pkt[0..4].copy_from_slice(&0xDEAD_BEEFu32.to_le_bytes());
    let err = parse_generic_udp(&pkt).unwrap_err();
    assert!(
        matches!(err, RacingError::BadMagic { found: 0xDEAD_BEEF }),
        "unexpected error: {err:?}"
    );
}

/// Any packet shorter than `MIN_PACKET_SIZE` produces a `TooShort` error.
#[test]
fn test_parse_generic_udp_too_short() {
    for len in 0..MIN_PACKET_SIZE {
        let data = vec![0u8; len];
        let result = parse_generic_udp(&data);
        assert!(
            matches!(result, Err(RacingError::TooShort { found }) if found == len),
            "length {len} should return TooShort"
        );
    }
}

/// The gear byte is decoded correctly for reverse, neutral, and forward gears.
#[test]
fn test_parse_generic_udp_gear() {
    for gear in [-1i8, 0, 1, 5, 8] {
        let pkt = build_packet(gear, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let t = parse_generic_udp(&pkt).unwrap();
        assert_eq!(t.gear, gear, "gear={gear}");
    }
}

/// `rpm_normalized` returns the correct fraction of the redline.
#[test]
fn test_telemetry_rpm_normalized() {
    let t = RacingTelemetry {
        rpm: 4000.0,
        rpm_max: 8000.0,
        ..Default::default()
    };
    let norm = t.rpm_normalized();
    assert!((norm - 0.5).abs() < 0.001, "rpm_normalized={norm}");
}

/// `is_braking` returns `true` only when brake > 0.05.
#[test]
fn test_telemetry_is_braking() {
    let mut t = RacingTelemetry::default();
    assert!(!t.is_braking(), "should not be braking at 0.0");
    t.brake = 0.04;
    assert!(!t.is_braking(), "0.04 is below threshold");
    t.brake = 0.06;
    assert!(t.is_braking(), "0.06 is above threshold");
}

/// The default `RacingTelemetry` is zero-initialised and marked invalid.
#[test]
fn test_telemetry_default_invalid() {
    let t = RacingTelemetry::default();
    assert!(!t.is_valid, "default should be invalid");
    assert!(!t.is_on_track, "default should not be on track");
    assert_eq!(t.speed_ms, 0.0);
    assert_eq!(t.gear, 0);
    assert_eq!(t.rpm, 0.0);
}

/// `lateral_force` is correctly normalised from lateral_g and max_force_n.
#[test]
fn test_ffb_translator_lateral_force() {
    // With max_force_n=10.0, denominator = 10.0 * 0.1 = 1.0, so lateral_force = lateral_g.
    let translator = RacingFfbTranslator::new();
    let t = RacingTelemetry {
        lateral_g: 0.75,
        ..Default::default()
    };
    let out = translator.translate(&t);
    assert!(
        (out.lateral_force - 0.75).abs() < 0.001,
        "lateral_force={}",
        out.lateral_force
    );

    // Should clamp at 1.0 for large G values.
    let t_heavy = RacingTelemetry {
        lateral_g: 5.0,
        ..Default::default()
    };
    let out_clamped = translator.translate(&t_heavy);
    assert_eq!(out_clamped.lateral_force, 1.0, "should clamp to 1.0");
}

/// `vibration_hz` and `vibration_amp` are derived from RPM.
#[test]
fn test_ffb_translator_vibration() {
    let translator = RacingFfbTranslator::new();
    let t = RacingTelemetry {
        rpm: 6000.0,
        rpm_max: 8000.0,
        ..Default::default()
    };
    let out = translator.translate(&t);
    assert!(
        (out.vibration_hz - 100.0).abs() < 0.001,
        "vibration_hz={}",
        out.vibration_hz
    );
    assert!(
        (out.vibration_amp - 0.75).abs() < 0.001,
        "vibration_amp={}",
        out.vibration_amp
    );
}

/// `rumble_amp` is derived from `vertical_g` (kerb / bump impacts).
#[test]
fn test_ffb_translator_rumble_from_vertical_g() {
    let translator = RacingFfbTranslator::new();
    let t = RacingTelemetry {
        vertical_g: 0.6,
        ..Default::default()
    };
    let out = translator.translate(&t);
    assert!(
        (out.rumble_amp - 0.6).abs() < 0.001,
        "rumble_amp={}",
        out.rumble_amp
    );

    // Large vertical_g should clamp at 1.0.
    let t_big = RacingTelemetry {
        vertical_g: 3.0,
        ..Default::default()
    };
    let out_big = translator.translate(&t_big);
    assert_eq!(out_big.rumble_amp, 1.0, "should clamp to 1.0");
}

/// Unused import suppression – verify `FfbOutput` is re-exported and usable.
#[test]
fn test_ffb_output_default() {
    let out = FfbOutput::default();
    assert_eq!(out.lateral_force, 0.0);
    assert_eq!(out.vibration_hz, 0.0);
    assert_eq!(out.vibration_amp, 0.0);
    assert_eq!(out.rumble_amp, 0.0);
}
