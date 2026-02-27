// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Integration tests for `flight-aerofly`.
//!
//! Exercises both the binary UDP and JSON parsing paths through the public
//! adapter API — no real network socket or simulator process is needed.

use flight_aerofly::{
    AEROFLY_MAGIC, AeroflyAdapter, AeroflyAdapterError, AeroflyTelemetry, MIN_FRAME_SIZE,
    parse_json_telemetry, parse_telemetry, parse_text_telemetry,
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
        vspeed_fpm: 0.0,
    };
    // serde_json is a regular dep of flight-aerofly, available in tests.
    let json = serde_json::to_string(&original).expect("serialize");
    let mut adapter = AeroflyAdapter::new();
    let parsed = adapter.process_json(&json).unwrap();
    assert_eq!(parsed, original);
}

/// Text key=value format is parsed correctly and all three adapter paths can be
/// used on the same adapter instance.
#[test]
fn text_format_parsed_and_all_three_paths_work() {
    let mut adapter = AeroflyAdapter::new();

    // Binary path
    let block = build_frame(1.0, 0.0, 90.0, 100.0, 1_000.0, 0.5, 1, 0.0);
    let t1 = adapter.process_datagram(&block).unwrap();
    assert!(t1.gear_down);

    // JSON path
    let json = r#"{"pitch":2.0,"roll":0.0,"heading":0.0,"airspeed":60.0,"altitude":500.0,"throttle_pos":0.3,"gear_down":false,"flaps_ratio":0.0}"#;
    adapter.process_json(json).unwrap();

    // Text path via standalone function
    let text = "pitch=4.0\nhdg=180.0\nias=90.0\nalt=3000.0\nthrottle=0.6\ngear=0.0\nvspeed=200.0";
    let t3 = parse_text_telemetry(text).unwrap();
    assert!((t3.pitch - 4.0).abs() < 0.01, "pitch={}", t3.pitch);
    assert!((t3.heading - 180.0).abs() < 0.01, "heading={}", t3.heading);
    assert!(!t3.gear_down, "gear should be up");
    assert!(
        (t3.vspeed_fpm - 200.0).abs() < 0.01,
        "vspeed_fpm={}",
        t3.vspeed_fpm
    );

    // Also exercise adapter text path
    let t4 = adapter.process_text(text).unwrap();
    assert!((t4.pitch - 4.0).abs() < 0.01);

    assert!(adapter.last_telemetry().is_some());
}

/// Unit conversion helpers return SI values from the native ft/knots/deg storage.
#[test]
fn unit_conversion_helpers_are_consistent() {
    let t = AeroflyTelemetry {
        pitch: 0.0,
        roll: 0.0,
        heading: 0.0,
        airspeed: 194.384,  // ≈ 100 m/s
        altitude: 32_808.4, // ≈ 10_000 m
        throttle_pos: 1.0,
        gear_down: false,
        flaps_ratio: 0.0,
        vspeed_fpm: 196.85, // ≈ 1 m/s
    };
    assert!(
        (t.airspeed_ms() - 100.0).abs() < 0.5,
        "airspeed_ms={}",
        t.airspeed_ms()
    );
    assert!(
        (t.altitude_m() - 10_000.0).abs() < 1.0,
        "altitude_m={}",
        t.altitude_m()
    );
    assert!(
        (t.vspeed_ms() - 1.0).abs() < 0.05,
        "vspeed_ms={}",
        t.vspeed_ms()
    );
}
