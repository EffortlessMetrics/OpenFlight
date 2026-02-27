// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Integration tests for `flight-il2`.
//!
//! These tests exercise the public parsing and adapter API end-to-end using
//! hand-constructed UDP datagrams — no real network socket is needed.

use flight_il2::{
    GearState, IL2_MAGIC, Il2Adapter, Il2AdapterError, Il2TelemetryFrame, MIN_FRAME_SIZE,
    SUPPORTED_VERSION, parse_telemetry_frame,
};

// ── Helper ─────────────────────────────────────────────────────────────────────

fn build_frame(
    pitch: f32,
    roll: f32,
    yaw: f32,
    speed: f32,
    altitude: f32,
    throttle: f32,
    gear: u8,
) -> Vec<u8> {
    let mut buf = vec![0u8; MIN_FRAME_SIZE];
    buf[0..4].copy_from_slice(&IL2_MAGIC.to_le_bytes());
    buf[4..8].copy_from_slice(&SUPPORTED_VERSION.to_le_bytes());
    buf[8..12].copy_from_slice(&pitch.to_le_bytes());
    buf[12..16].copy_from_slice(&roll.to_le_bytes());
    buf[16..20].copy_from_slice(&yaw.to_le_bytes());
    buf[20..24].copy_from_slice(&speed.to_le_bytes());
    buf[24..28].copy_from_slice(&altitude.to_le_bytes());
    buf[28..32].copy_from_slice(&throttle.to_le_bytes());
    buf[32] = gear;
    buf
}

// ── Tests ──────────────────────────────────────────────────────────────────────

/// A correctly formed IL-2 UDP packet is parsed with all fields intact.
#[test]
fn valid_udp_packet_parsed_correctly() {
    let data = build_frame(8.0, -4.0, 270.0, 95.0, 2_500.0, 0.75, GearState::Down as u8);
    let frame = parse_telemetry_frame(&data).unwrap();
    assert!((frame.pitch - 8.0).abs() < 0.01, "pitch={}", frame.pitch);
    assert!((frame.roll - (-4.0)).abs() < 0.01, "roll={}", frame.roll);
    assert!((frame.yaw - 270.0).abs() < 0.01, "yaw={}", frame.yaw);
    assert!((frame.speed - 95.0).abs() < 0.01, "speed={}", frame.speed);
    assert!(
        (frame.altitude - 2_500.0).abs() < 0.01,
        "altitude={}",
        frame.altitude
    );
    assert!(
        (frame.throttle - 0.75).abs() < 0.01,
        "throttle={}",
        frame.throttle
    );
    assert_eq!(frame.gear, GearState::Down);
}

/// Any incorrect magic bytes produce a `BadMagic` error, not a panic.
#[test]
fn invalid_magic_bytes_returns_error() {
    let mut data = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0);
    data[0..4].copy_from_slice(&0xCAFE_BABEu32.to_le_bytes());
    let err = parse_telemetry_frame(&data).unwrap_err();
    assert!(
        matches!(err, Il2AdapterError::BadMagic { .. }),
        "expected BadMagic, got {err:?}"
    );
}

/// Every truncated length (0 to MIN_FRAME_SIZE-1) must return an error without
/// panicking.
#[test]
fn packet_truncated_mid_field_returns_error_no_panic() {
    for len in 0..MIN_FRAME_SIZE {
        let data = vec![0u8; len];
        let result = parse_telemetry_frame(&data);
        assert!(result.is_err(), "length {len} should fail");
    }
}

/// Airspeed, altitude, and attitude decoded from realistic WWII-era values.
#[test]
fn airspeed_altitude_attitude_decoded_from_realistic_values() {
    // Bf 109 in typical cruise: ~97 m/s (≈350 km/h), 4 000 m, heading south.
    let data = build_frame(2.0, 0.0, 180.0, 97.0, 4_000.0, 0.7, 0);
    let frame = parse_telemetry_frame(&data).unwrap();
    assert!((frame.speed - 97.0).abs() < 0.01, "speed={}", frame.speed);
    assert!(
        (frame.altitude - 4_000.0).abs() < 0.01,
        "altitude={}",
        frame.altitude
    );
    assert!((frame.yaw - 180.0).abs() < 0.01, "yaw={}", frame.yaw);
    assert!((frame.pitch - 2.0).abs() < 0.01, "pitch={}", frame.pitch);
}

/// All three GearState variants (Up, Transitioning, Down) are decoded correctly.
#[test]
fn gear_state_bits_decoded_for_all_variants() {
    for (byte, expected) in [
        (GearState::Up as u8, GearState::Up),
        (GearState::Transitioning as u8, GearState::Transitioning),
        (GearState::Down as u8, GearState::Down),
    ] {
        let data = build_frame(0.0, 0.0, 0.0, 100.0, 500.0, 0.3, byte);
        let frame = parse_telemetry_frame(&data).unwrap();
        assert_eq!(frame.gear, expected, "gear byte {byte}");
    }
}

/// Complete round-trip: manually assemble bytes → parse → verify every field.
#[test]
fn complete_round_trip_build_parse_verify_all_fields() {
    let pitch = 2.5_f32;
    let roll = 30.0_f32;
    let yaw = 90.0_f32;
    let speed = 120.0_f32;
    let altitude = 5_000.0_f32;
    let throttle = 0.85_f32;

    let data = build_frame(
        pitch,
        roll,
        yaw,
        speed,
        altitude,
        throttle,
        GearState::Up as u8,
    );
    let frame = parse_telemetry_frame(&data).unwrap();

    assert!((frame.pitch - pitch).abs() < 0.001);
    assert!((frame.roll - roll).abs() < 0.001);
    assert!((frame.yaw - yaw).abs() < 0.001);
    assert!((frame.speed - speed).abs() < 0.001);
    assert!((frame.altitude - altitude).abs() < 0.001);
    assert!((frame.throttle - throttle).abs() < 0.001);
    assert_eq!(frame.gear, GearState::Up);
}

/// The adapter processes multiple datagrams sequentially and always caches the
/// most recent frame.
#[test]
fn adapter_processes_multiple_sequential_datagrams() {
    let mut adapter = Il2Adapter::new();

    let data1 = build_frame(5.0, 0.0, 0.0, 100.0, 1_000.0, 0.5, GearState::Up as u8);
    let frame1 = adapter.process_datagram(&data1).unwrap();
    assert!((frame1.pitch - 5.0).abs() < 0.01);

    let data2 = build_frame(-3.0, 10.0, 90.0, 130.0, 2_000.0, 0.6, GearState::Down as u8);
    let frame2 = adapter.process_datagram(&data2).unwrap();
    assert!((frame2.pitch - (-3.0)).abs() < 0.01);
    assert_eq!(frame2.gear, GearState::Down);

    // last_frame must reflect the most recent datagram.
    let last = adapter.last_frame().unwrap();
    assert!((last.pitch - (-3.0)).abs() < 0.01);
    assert_eq!(last.gear, GearState::Down);
}

/// The default `Il2TelemetryFrame` matches the expected zero state.
#[test]
fn default_telemetry_frame_is_all_zero_gear_up() {
    let frame = Il2TelemetryFrame::default();
    assert_eq!(frame.pitch, 0.0);
    assert_eq!(frame.roll, 0.0);
    assert_eq!(frame.yaw, 0.0);
    assert_eq!(frame.speed, 0.0);
    assert_eq!(frame.altitude, 0.0);
    assert_eq!(frame.throttle, 0.0);
    assert_eq!(frame.gear, GearState::Up);
}
