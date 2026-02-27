// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Integration tests for `flight-aerofly`.
//!
//! Exercises both the binary UDP and JSON parsing paths through the public
//! adapter API — no real network socket or simulator process is needed.

use flight_aerofly::{
    AEROFLY_MAGIC, AeroflyAdapter, AeroflyAdapterError, AeroflyTelemetry, MIN_FRAME_SIZE,
    parse_json_telemetry, parse_telemetry,
};

// ── Helper ─────────────────────────────────────────────────────────────────────

fn build_frame(
    pitch: f32,
    roll: f32,
    heading: f32,
    airspeed: f32,
    altitude: f32,
    throttle_pos: f32,
    gear_down: u8,
    flaps_ratio: f32,
) -> Vec<u8> {
    let mut buf = vec![0u8; MIN_FRAME_SIZE];
    buf[0..4].copy_from_slice(&AEROFLY_MAGIC.to_le_bytes());
    buf[4..8].copy_from_slice(&pitch.to_le_bytes());
    buf[8..12].copy_from_slice(&roll.to_le_bytes());
    buf[12..16].copy_from_slice(&heading.to_le_bytes());
    buf[16..20].copy_from_slice(&airspeed.to_le_bytes());
    buf[20..24].copy_from_slice(&altitude.to_le_bytes());
    buf[24..28].copy_from_slice(&throttle_pos.to_le_bytes());
    buf[28] = gear_down;
    buf[29..33].copy_from_slice(&flaps_ratio.to_le_bytes());
    buf
}

// ── Tests ──────────────────────────────────────────────────────────────────────

/// A valid JSON telemetry packet with all fields is parsed correctly.
#[test]
fn valid_json_telemetry_packet_parsed() {
    let json = r#"{
        "pitch": 5.0,
        "roll": -2.0,
        "heading": 270.0,
        "airspeed": 130.0,
        "altitude": 8000.0,
        "throttle_pos": 0.8,
        "gear_down": false,
        "flaps_ratio": 0.0
    }"#;
    let t = parse_json_telemetry(json).unwrap();
    assert!((t.pitch - 5.0).abs() < 0.01, "pitch={}", t.pitch);
    assert!((t.heading - 270.0).abs() < 0.01, "heading={}", t.heading);
    assert!((t.airspeed - 130.0).abs() < 0.01, "airspeed={}", t.airspeed);
    assert!(
        (t.altitude - 8_000.0).abs() < 0.01,
        "altitude={}",
        t.altitude
    );
    assert!(!t.gear_down);
}

/// JSON that omits required fields returns a `JsonError`, not a panic.
#[test]
fn missing_fields_in_json_returns_error_gracefully() {
    // serde requires all fields (no `#[serde(default)]` applied here).
    let json = r#"{"pitch": 5.0}"#;
    let err = parse_json_telemetry(json).unwrap_err();
    assert!(
        matches!(err, AeroflyAdapterError::JsonError(_)),
        "expected JsonError, got {err:?}"
    );
}

/// Altitude (feet MSL) and heading are decoded to the correct numeric values.
#[test]
fn coordinates_altitude_and_heading_decoded_correctly() {
    let block = build_frame(0.0, 0.0, 45.0, 90.0, 3_500.0, 0.5, 0, 0.0);
    let t = parse_telemetry(&block).unwrap();
    assert!(
        (t.altitude - 3_500.0).abs() < 0.01,
        "altitude={}",
        t.altitude
    );
    assert!((t.heading - 45.0).abs() < 0.01, "heading={}", t.heading);
}

/// Roll, pitch, and heading are stored in degrees in the binary format (no
/// radians-to-degrees conversion is applied by the adapter).
#[test]
fn attitude_roll_pitch_heading_in_correct_units() {
    let block = build_frame(10.0, -15.0, 180.0, 100.0, 5_000.0, 0.6, 0, 0.0);
    let t = parse_telemetry(&block).unwrap();
    assert!((t.pitch - 10.0).abs() < 0.01, "pitch={}", t.pitch);
    assert!((t.roll - (-15.0)).abs() < 0.01, "roll={}", t.roll);
    assert!((t.heading - 180.0).abs() < 0.01, "heading={}", t.heading);
}

/// Airspeed and flaps ratio are present and decoded to the right values.
#[test]
fn airspeed_and_vertical_state_fields_present() {
    let block = build_frame(0.0, 0.0, 0.0, 115.0, 2_000.0, 0.7, 0, 0.25);
    let t = parse_telemetry(&block).unwrap();
    assert!((t.airspeed - 115.0).abs() < 0.01, "airspeed={}", t.airspeed);
    assert!(
        (t.throttle_pos - 0.7).abs() < 0.01,
        "throttle_pos={}",
        t.throttle_pos
    );
    assert!(
        (t.flaps_ratio - 0.25).abs() < 0.01,
        "flaps_ratio={}",
        t.flaps_ratio
    );
}

/// Malformed JSON inputs must return an error — never panic.
#[test]
fn malformed_json_returns_error_not_panic() {
    let bad_inputs = ["", "not json at all", "{unclosed", "null", "42", "[]"];
    for input in bad_inputs {
        let result = parse_json_telemetry(input);
        assert!(result.is_err(), "expected error for: {input:?}");
    }
}

/// The adapter handles both binary and JSON paths in sequence; `last_telemetry`
/// reflects whichever was called most recently.
#[test]
fn adapter_processes_binary_and_json_paths() {
    let mut adapter = AeroflyAdapter::new();

    let block = build_frame(3.0, -1.0, 90.0, 80.0, 1_200.0, 0.4, 1, 0.5);
    let t1 = adapter.process_datagram(&block).unwrap();
    assert!(t1.gear_down, "gear_down should be true");

    let json = r#"{"pitch":1.0,"roll":0.0,"heading":0.0,"airspeed":60.0,"altitude":500.0,"throttle_pos":0.3,"gear_down":false,"flaps_ratio":0.0}"#;
    let t2 = adapter.process_json(json).unwrap();
    assert!(!t2.gear_down, "gear_down should be false after JSON update");

    assert!(adapter.last_telemetry().is_some());
}

/// JSON round-trip through the adapter preserves all fields exactly.
#[test]
fn adapter_json_round_trip_preserves_all_fields() {
    let original = AeroflyTelemetry {
        pitch: 4.0,
        roll: -8.0,
        heading: 315.0,
        airspeed: 145.0,
        altitude: 12_000.0,
        throttle_pos: 0.9,
        gear_down: false,
        flaps_ratio: 0.0,
    };
    // serde_json is a regular dep of flight-aerofly, available in tests.
    let json = serde_json::to_string(&original).expect("serialize");
    let mut adapter = AeroflyAdapter::new();
    let parsed = adapter.process_json(&json).unwrap();
    assert_eq!(parsed, original);
}
