// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Depth tests for the `flight-opentrack` crate.
//!
//! Covers: UDP packet parsing, boundary values, malformed packets,
//! 6-DOF normalization, adapter state transitions, round-trip
//! serialization, error display, and property-based fuzzing.

use flight_opentrack::{
    parse_packet, pitch_to_normalized, yaw_to_normalized, HeadPosition, OpenTrackAdapter,
    OpenTrackError, OPENTRACK_PACKET_SIZE, OPENTRACK_PORT,
};
use proptest::prelude::*;

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Build a valid 48-byte OpenTrack packet from six f64 values.
fn build_packet(x: f64, y: f64, z: f64, yaw: f64, pitch: f64, roll: f64) -> Vec<u8> {
    let mut buf = vec![0u8; OPENTRACK_PACKET_SIZE];
    buf[0..8].copy_from_slice(&x.to_le_bytes());
    buf[8..16].copy_from_slice(&y.to_le_bytes());
    buf[16..24].copy_from_slice(&z.to_le_bytes());
    buf[24..32].copy_from_slice(&yaw.to_le_bytes());
    buf[32..40].copy_from_slice(&pitch.to_le_bytes());
    buf[40..48].copy_from_slice(&roll.to_le_bytes());
    buf
}

/// Inject a single f64 at a given field index (0–5) into an otherwise-zero packet.
fn packet_with_field(field_idx: usize, value: f64) -> Vec<u8> {
    let mut vals = [0.0_f64; 6];
    vals[field_idx] = value;
    build_packet(vals[0], vals[1], vals[2], vals[3], vals[4], vals[5])
}

// ═══════════════════════════════════════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn default_port_is_4242() {
    assert_eq!(OPENTRACK_PORT, 4242);
}

#[test]
fn packet_size_is_48() {
    assert_eq!(OPENTRACK_PACKET_SIZE, 48);
}

#[test]
fn packet_size_equals_six_f64s() {
    assert_eq!(OPENTRACK_PACKET_SIZE, 6 * std::mem::size_of::<f64>());
}

// ═══════════════════════════════════════════════════════════════════════════════
// UDP Packet Parsing — valid packets
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn parse_each_field_independently() {
    for field in 0..6 {
        let pkt = packet_with_field(field, 42.0);
        let pos = parse_packet(&pkt).unwrap();
        let vals = [
            pos.x_mm,
            pos.y_mm,
            pos.z_mm,
            pos.yaw_deg,
            pos.pitch_deg,
            pos.roll_deg,
        ];
        for (i, &v) in vals.iter().enumerate() {
            if i == field {
                assert!((v - 42.0).abs() < 1e-10, "field {field} should be 42.0");
            } else {
                assert_eq!(v, 0.0, "field {i} should be 0.0 when field {field} is set");
            }
        }
    }
}

#[test]
fn parse_all_negative_values() {
    let pkt = build_packet(-100.0, -200.0, -300.0, -45.0, -30.0, -15.0);
    let pos = parse_packet(&pkt).unwrap();
    assert!((pos.x_mm - (-100.0)).abs() < 1e-10);
    assert!((pos.y_mm - (-200.0)).abs() < 1e-10);
    assert!((pos.z_mm - (-300.0)).abs() < 1e-10);
    assert!((pos.yaw_deg - (-45.0)).abs() < 1e-10);
    assert!((pos.pitch_deg - (-30.0)).abs() < 1e-10);
    assert!((pos.roll_deg - (-15.0)).abs() < 1e-10);
}

#[test]
fn parse_very_small_positive_values() {
    let tiny = 1e-300;
    let pkt = build_packet(tiny, tiny, tiny, tiny, tiny, tiny);
    let pos = parse_packet(&pkt).unwrap();
    assert!((pos.x_mm - tiny).abs() < 1e-310);
}

#[test]
fn parse_negative_zero_is_accepted() {
    let pkt = build_packet(-0.0, -0.0, -0.0, -0.0, -0.0, -0.0);
    let pos = parse_packet(&pkt).unwrap();
    // -0.0 == 0.0 in IEEE 754
    assert_eq!(pos.x_mm, 0.0);
    assert_eq!(pos.yaw_deg, 0.0);
}

#[test]
fn parse_f64_max_is_accepted() {
    let pkt = build_packet(f64::MAX, 0.0, 0.0, 0.0, 0.0, 0.0);
    let pos = parse_packet(&pkt).unwrap();
    assert_eq!(pos.x_mm, f64::MAX);
}

#[test]
fn parse_f64_min_is_accepted() {
    let pkt = build_packet(f64::MIN, 0.0, 0.0, 0.0, 0.0, 0.0);
    let pos = parse_packet(&pkt).unwrap();
    assert_eq!(pos.x_mm, f64::MIN);
}

#[test]
fn parse_f64_min_positive_is_accepted() {
    let pkt = build_packet(f64::MIN_POSITIVE, 0.0, 0.0, 0.0, 0.0, 0.0);
    let pos = parse_packet(&pkt).unwrap();
    assert_eq!(pos.x_mm, f64::MIN_POSITIVE);
}

#[test]
fn parse_f64_epsilon_is_accepted() {
    let pkt = build_packet(f64::EPSILON, 0.0, 0.0, 0.0, 0.0, 0.0);
    let pos = parse_packet(&pkt).unwrap();
    assert_eq!(pos.x_mm, f64::EPSILON);
}

#[test]
fn parse_subnormal_value_is_accepted() {
    let subnormal = f64::MIN_POSITIVE / 2.0;
    assert!(subnormal > 0.0 && subnormal < f64::MIN_POSITIVE);
    let pkt = build_packet(subnormal, 0.0, 0.0, 0.0, 0.0, 0.0);
    let pos = parse_packet(&pkt).unwrap();
    assert_eq!(pos.x_mm, subnormal);
}

#[test]
fn parse_extreme_all_fields_f64_max() {
    let pkt = build_packet(f64::MAX, f64::MAX, f64::MAX, f64::MAX, f64::MAX, f64::MAX);
    let pos = parse_packet(&pkt).unwrap();
    assert_eq!(pos.x_mm, f64::MAX);
    assert_eq!(pos.roll_deg, f64::MAX);
}

#[test]
fn parse_typical_opentrack_values() {
    let pkt = build_packet(15.3, -2.1, 50.0, -22.5, -8.0, 3.2);
    let pos = parse_packet(&pkt).unwrap();
    assert!((pos.x_mm - 15.3).abs() < 1e-10);
    assert!((pos.y_mm - (-2.1)).abs() < 1e-10);
    assert!((pos.z_mm - 50.0).abs() < 1e-10);
    assert!((pos.yaw_deg - (-22.5)).abs() < 1e-10);
    assert!((pos.pitch_deg - (-8.0)).abs() < 1e-10);
    assert!((pos.roll_deg - 3.2).abs() < 1e-10);
}

#[test]
fn parse_exactly_48_bytes_succeeds() {
    let pkt = build_packet(1.0, 2.0, 3.0, 4.0, 5.0, 6.0);
    assert_eq!(pkt.len(), 48);
    assert!(parse_packet(&pkt).is_ok());
}

#[test]
fn parse_49_bytes_succeeds() {
    let mut pkt = build_packet(1.0, 2.0, 3.0, 4.0, 5.0, 6.0);
    pkt.push(0xAA);
    assert!(parse_packet(&pkt).is_ok());
}

// ═══════════════════════════════════════════════════════════════════════════════
// UDP Packet Parsing — malformed / short / empty packets
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn parse_every_length_below_48_fails() {
    for len in 0..OPENTRACK_PACKET_SIZE {
        let data = vec![0u8; len];
        assert_eq!(
            parse_packet(&data),
            Err(OpenTrackError::PacketTooShort { actual: len }),
            "length {len} should produce PacketTooShort"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Non-finite value rejection
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn nan_in_every_field_is_rejected() {
    for field in 0..6 {
        let pkt = packet_with_field(field, f64::NAN);
        assert_eq!(
            parse_packet(&pkt),
            Err(OpenTrackError::NonFiniteValue),
            "NaN in field {field} should be rejected"
        );
    }
}

#[test]
fn positive_infinity_in_every_field_is_rejected() {
    for field in 0..6 {
        let pkt = packet_with_field(field, f64::INFINITY);
        assert_eq!(
            parse_packet(&pkt),
            Err(OpenTrackError::NonFiniteValue),
            "+Inf in field {field} should be rejected"
        );
    }
}

#[test]
fn negative_infinity_in_every_field_is_rejected() {
    for field in 0..6 {
        let pkt = packet_with_field(field, f64::NEG_INFINITY);
        assert_eq!(
            parse_packet(&pkt),
            Err(OpenTrackError::NonFiniteValue),
            "-Inf in field {field} should be rejected"
        );
    }
}

#[test]
fn all_nan_packet_is_rejected() {
    let pkt = build_packet(f64::NAN, f64::NAN, f64::NAN, f64::NAN, f64::NAN, f64::NAN);
    assert_eq!(parse_packet(&pkt), Err(OpenTrackError::NonFiniteValue));
}

#[test]
fn mixed_nan_and_infinity_is_rejected() {
    let pkt = build_packet(
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        0.0,
        0.0,
        0.0,
    );
    assert_eq!(parse_packet(&pkt), Err(OpenTrackError::NonFiniteValue));
}

#[test]
fn quiet_nan_is_rejected() {
    let qnan = f64::NAN;
    assert!(qnan.is_nan());
    let pkt = packet_with_field(3, qnan);
    assert_eq!(parse_packet(&pkt), Err(OpenTrackError::NonFiniteValue));
}

#[test]
fn signaling_nan_bitpattern_is_rejected() {
    // Signaling NaN: exponent all 1s, non-zero mantissa with MSB clear
    let snan_bits: u64 = 0x7FF0_0000_0000_0001;
    let snan = f64::from_bits(snan_bits);
    assert!(snan.is_nan());
    let pkt = packet_with_field(0, snan);
    assert_eq!(parse_packet(&pkt), Err(OpenTrackError::NonFiniteValue));
}

#[test]
fn five_valid_one_nan_last_field_is_rejected() {
    let pkt = build_packet(1.0, 2.0, 3.0, 4.0, 5.0, f64::NAN);
    assert_eq!(parse_packet(&pkt), Err(OpenTrackError::NonFiniteValue));
}

// ═══════════════════════════════════════════════════════════════════════════════
// 6-DOF Normalization — yaw
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn yaw_minus_180_normalizes_to_0() {
    assert!((yaw_to_normalized(-180.0)).abs() < 1e-10);
}

#[test]
fn yaw_0_normalizes_to_half() {
    assert!((yaw_to_normalized(0.0) - 0.5).abs() < 1e-10);
}

#[test]
fn yaw_plus_180_normalizes_to_1() {
    assert!((yaw_to_normalized(180.0) - 1.0).abs() < 1e-10);
}

#[test]
fn yaw_minus_90_normalizes_to_quarter() {
    assert!((yaw_to_normalized(-90.0) - 0.25).abs() < 1e-10);
}

#[test]
fn yaw_plus_90_normalizes_to_three_quarters() {
    assert!((yaw_to_normalized(90.0) - 0.75).abs() < 1e-10);
}

#[test]
fn yaw_normalization_is_monotonic() {
    let mut prev = yaw_to_normalized(-180.0);
    for deg in (-179..=180).map(|d| d as f64) {
        let cur = yaw_to_normalized(deg);
        assert!(
            cur >= prev,
            "yaw normalization should be monotonically non-decreasing: {prev} > {cur} at {deg}°"
        );
        prev = cur;
    }
}

#[test]
fn yaw_beyond_range_still_computes() {
    let result = yaw_to_normalized(270.0);
    assert!((result - 1.25).abs() < 1e-10);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 6-DOF Normalization — pitch
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn pitch_minus_90_normalizes_to_0() {
    assert!((pitch_to_normalized(-90.0)).abs() < 1e-10);
}

#[test]
fn pitch_0_normalizes_to_half() {
    assert!((pitch_to_normalized(0.0) - 0.5).abs() < 1e-10);
}

#[test]
fn pitch_plus_90_normalizes_to_1() {
    assert!((pitch_to_normalized(90.0) - 1.0).abs() < 1e-10);
}

#[test]
fn pitch_minus_45_normalizes_to_quarter() {
    assert!((pitch_to_normalized(-45.0) - 0.25).abs() < 1e-10);
}

#[test]
fn pitch_plus_45_normalizes_to_three_quarters() {
    assert!((pitch_to_normalized(45.0) - 0.75).abs() < 1e-10);
}

#[test]
fn pitch_normalization_is_monotonic() {
    let mut prev = pitch_to_normalized(-90.0);
    for deg in (-89..=90).map(|d| d as f64) {
        let cur = pitch_to_normalized(deg);
        assert!(
            cur >= prev,
            "pitch normalization should be monotonically non-decreasing"
        );
        prev = cur;
    }
}

#[test]
fn pitch_beyond_range_still_computes() {
    let result = pitch_to_normalized(180.0);
    assert!((result - 1.5).abs() < 1e-10);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Round-trip serialization
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn serde_round_trip_all_zeros() {
    let pos = HeadPosition::default();
    let json = serde_json::to_string(&pos).unwrap();
    let back: HeadPosition = serde_json::from_str(&json).unwrap();
    assert_eq!(back, pos);
}

#[test]
fn serde_round_trip_extreme_values() {
    let pos = HeadPosition {
        x_mm: f64::MAX,
        y_mm: f64::MIN,
        z_mm: f64::MIN_POSITIVE,
        yaw_deg: f64::EPSILON,
        pitch_deg: -0.0,
        roll_deg: 1e-300,
    };
    let json = serde_json::to_string(&pos).unwrap();
    let back: HeadPosition = serde_json::from_str(&json).unwrap();
    assert_eq!(back.x_mm, pos.x_mm);
    assert_eq!(back.y_mm, pos.y_mm);
    assert_eq!(back.z_mm, pos.z_mm);
}

#[test]
fn serde_json_field_names_match_struct() {
    let pos = HeadPosition {
        x_mm: 1.0,
        y_mm: 2.0,
        z_mm: 3.0,
        yaw_deg: 4.0,
        pitch_deg: 5.0,
        roll_deg: 6.0,
    };
    let json = serde_json::to_string(&pos).unwrap();
    assert!(json.contains("\"x_mm\""));
    assert!(json.contains("\"y_mm\""));
    assert!(json.contains("\"z_mm\""));
    assert!(json.contains("\"yaw_deg\""));
    assert!(json.contains("\"pitch_deg\""));
    assert!(json.contains("\"roll_deg\""));
}

#[test]
fn serde_deserialize_from_known_json() {
    let json = r#"{"x_mm":10.0,"y_mm":-5.0,"z_mm":3.5,"yaw_deg":45.0,"pitch_deg":-15.0,"roll_deg":2.0}"#;
    let pos: HeadPosition = serde_json::from_str(json).unwrap();
    assert!((pos.x_mm - 10.0).abs() < 1e-10);
    assert!((pos.yaw_deg - 45.0).abs() < 1e-10);
}

#[test]
fn serde_rejects_missing_field() {
    let json = r#"{"x_mm":1.0,"y_mm":2.0}"#;
    let result: Result<HeadPosition, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn binary_round_trip_parse_to_bytes_to_parse() {
    let original = build_packet(99.9, -44.4, 0.001, 179.9, -89.9, 0.0);
    let pos1 = parse_packet(&original).unwrap();
    let rebuilt = build_packet(
        pos1.x_mm,
        pos1.y_mm,
        pos1.z_mm,
        pos1.yaw_deg,
        pos1.pitch_deg,
        pos1.roll_deg,
    );
    assert_eq!(
        original, rebuilt,
        "binary round-trip should produce identical bytes"
    );
    let pos2 = parse_packet(&rebuilt).unwrap();
    assert_eq!(pos1, pos2);
}

// ═══════════════════════════════════════════════════════════════════════════════
// HeadPosition type
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn head_position_default_all_zeros() {
    let pos = HeadPosition::default();
    assert_eq!(pos.x_mm, 0.0);
    assert_eq!(pos.y_mm, 0.0);
    assert_eq!(pos.z_mm, 0.0);
    assert_eq!(pos.yaw_deg, 0.0);
    assert_eq!(pos.pitch_deg, 0.0);
    assert_eq!(pos.roll_deg, 0.0);
}

#[test]
fn head_position_clone_equals_original() {
    let pos = HeadPosition {
        x_mm: 1.5,
        y_mm: -2.5,
        z_mm: 3.5,
        yaw_deg: 45.0,
        pitch_deg: -30.0,
        roll_deg: 10.0,
    };
    let cloned = pos.clone();
    assert_eq!(pos, cloned);
}

#[test]
fn head_position_partial_eq_different_values() {
    let a = HeadPosition {
        x_mm: 1.0,
        ..HeadPosition::default()
    };
    let b = HeadPosition {
        x_mm: 2.0,
        ..HeadPosition::default()
    };
    assert_ne!(a, b);
}

#[test]
fn head_position_debug_format_is_populated() {
    let pos = HeadPosition::default();
    let debug = format!("{pos:?}");
    assert!(debug.contains("HeadPosition"));
    assert!(debug.contains("x_mm"));
}

// ═══════════════════════════════════════════════════════════════════════════════
// Adapter state machine
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn adapter_initial_state_has_no_position() {
    let adapter = OpenTrackAdapter::new();
    assert!(adapter.last_position().is_none());
}

#[test]
fn adapter_default_uses_standard_port() {
    let adapter = OpenTrackAdapter::default();
    assert_eq!(adapter.port, OPENTRACK_PORT);
}

#[test]
fn adapter_with_custom_port() {
    let adapter = OpenTrackAdapter::with_port(5555);
    assert_eq!(adapter.port, 5555);
}

#[test]
fn adapter_with_port_zero() {
    let adapter = OpenTrackAdapter::with_port(0);
    assert_eq!(adapter.port, 0);
}

#[test]
fn adapter_with_port_max() {
    let adapter = OpenTrackAdapter::with_port(u16::MAX);
    assert_eq!(adapter.port, u16::MAX);
}

#[test]
fn adapter_transitions_to_receiving_after_valid_datagram() {
    let mut adapter = OpenTrackAdapter::new();
    assert!(adapter.last_position().is_none());

    let pkt = build_packet(1.0, 2.0, 3.0, 4.0, 5.0, 6.0);
    let result = adapter.process_datagram(&pkt);
    assert!(result.is_ok());
    assert!(adapter.last_position().is_some());
}

#[test]
fn adapter_stays_in_listening_after_error() {
    let mut adapter = OpenTrackAdapter::new();
    let result = adapter.process_datagram(&[0u8; 10]);
    assert!(result.is_err());
    assert!(adapter.last_position().is_none());
}

#[test]
fn adapter_preserves_last_valid_after_subsequent_short_packet() {
    let mut adapter = OpenTrackAdapter::new();

    let pkt = build_packet(10.0, 20.0, 30.0, 40.0, 50.0, 60.0);
    adapter.process_datagram(&pkt).unwrap();
    assert!(adapter.last_position().is_some());

    let err = adapter.process_datagram(&[0u8; 5]);
    assert!(err.is_err());

    let pos = adapter.last_position().unwrap();
    assert!((pos.x_mm - 10.0).abs() < 1e-10);
}

#[test]
fn adapter_preserves_last_valid_after_non_finite_error() {
    let mut adapter = OpenTrackAdapter::new();

    let pkt = build_packet(1.0, 2.0, 3.0, 4.0, 5.0, 6.0);
    adapter.process_datagram(&pkt).unwrap();

    let nan_pkt = build_packet(f64::NAN, 0.0, 0.0, 0.0, 0.0, 0.0);
    assert!(adapter.process_datagram(&nan_pkt).is_err());

    let pos = adapter.last_position().unwrap();
    assert!((pos.x_mm - 1.0).abs() < 1e-10);
}

#[test]
fn adapter_updates_position_on_each_valid_datagram() {
    let mut adapter = OpenTrackAdapter::new();

    let pkt1 = build_packet(1.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    adapter.process_datagram(&pkt1).unwrap();
    assert!((adapter.last_position().unwrap().x_mm - 1.0).abs() < 1e-10);

    let pkt2 = build_packet(99.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    adapter.process_datagram(&pkt2).unwrap();
    assert!((adapter.last_position().unwrap().x_mm - 99.0).abs() < 1e-10);
}

#[test]
fn adapter_many_sequential_updates() {
    let mut adapter = OpenTrackAdapter::new();
    for i in 0..100 {
        let val = i as f64;
        let pkt = build_packet(val, val, val, val % 180.0, val % 90.0, val % 180.0);
        let pos = adapter.process_datagram(&pkt).unwrap();
        assert!((pos.x_mm - val).abs() < 1e-10);
    }
    let last = adapter.last_position().unwrap();
    assert!((last.x_mm - 99.0).abs() < 1e-10);
}

#[test]
fn adapter_process_returns_same_data_as_cached() {
    let mut adapter = OpenTrackAdapter::new();
    let pkt = build_packet(5.5, 6.6, 7.7, 8.8, 9.9, 10.1);
    let returned = adapter.process_datagram(&pkt).unwrap();
    let cached = adapter.last_position().unwrap();
    assert_eq!(&returned, cached);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Error handling & Display
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn error_packet_too_short_display() {
    let err = OpenTrackError::PacketTooShort { actual: 12 };
    let msg = err.to_string();
    assert!(msg.contains("48"), "should mention expected size");
    assert!(msg.contains("12"), "should mention actual size");
}

#[test]
fn error_non_finite_display() {
    let err = OpenTrackError::NonFiniteValue;
    let msg = err.to_string();
    assert!(msg.to_lowercase().contains("non-finite"));
}

#[test]
fn error_equality_packet_too_short() {
    let a = OpenTrackError::PacketTooShort { actual: 10 };
    let b = OpenTrackError::PacketTooShort { actual: 10 };
    assert_eq!(a, b);
}

#[test]
fn error_inequality_different_actual_sizes() {
    let a = OpenTrackError::PacketTooShort { actual: 10 };
    let b = OpenTrackError::PacketTooShort { actual: 20 };
    assert_ne!(a, b);
}

#[test]
fn error_inequality_different_variants() {
    let a = OpenTrackError::PacketTooShort { actual: 10 };
    let b = OpenTrackError::NonFiniteValue;
    assert_ne!(a, b);
}

#[test]
fn error_non_finite_equality() {
    let a = OpenTrackError::NonFiniteValue;
    let b = OpenTrackError::NonFiniteValue;
    assert_eq!(a, b);
}

#[test]
fn error_debug_format_is_populated() {
    let err = OpenTrackError::PacketTooShort { actual: 0 };
    let debug = format!("{err:?}");
    assert!(debug.contains("PacketTooShort"));
}

// ═══════════════════════════════════════════════════════════════════════════════
// Byte-level packet construction verification
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn raw_bytes_manual_construction_matches_parse() {
    let mut data = [0u8; 48];
    data[0..8].copy_from_slice(&1.0_f64.to_le_bytes());
    let pos = parse_packet(&data).unwrap();
    assert_eq!(pos.x_mm, 1.0);
    assert_eq!(pos.y_mm, 0.0);
}

#[test]
fn all_0xff_bytes_produces_nan_error() {
    let data = [0xFF; 48];
    assert_eq!(parse_packet(&data), Err(OpenTrackError::NonFiniteValue));
}

#[test]
fn infinity_bytes_in_first_field_produces_error() {
    let inf_bytes = f64::INFINITY.to_le_bytes();
    let mut data = [0u8; 48];
    data[0..8].copy_from_slice(&inf_bytes);
    assert_eq!(parse_packet(&data), Err(OpenTrackError::NonFiniteValue));
}

#[test]
fn endianness_is_little_endian() {
    let val: f64 = 42.5;
    let le_bytes = val.to_le_bytes();
    let mut data = [0u8; 48];
    data[0..8].copy_from_slice(&le_bytes);
    let pos = parse_packet(&data).unwrap();
    assert!((pos.x_mm - 42.5).abs() < 1e-10);
}

#[test]
fn big_endian_bytes_produce_different_value() {
    let val: f64 = 42.5;
    let be_bytes = val.to_be_bytes();
    let le_bytes = val.to_le_bytes();
    // BE and LE differ for non-palindromic representations
    if be_bytes != le_bytes {
        let mut data = [0u8; 48];
        data[0..8].copy_from_slice(&be_bytes);
        match parse_packet(&data) {
            Ok(pos) => {
                // Parsed value from BE bytes should not equal the original LE-interpreted value.
                assert!((pos.x_mm - 42.5).abs() > 1e-10);
            }
            Err(OpenTrackError::NonFiniteValue) => {
                // Acceptable: BE representation decoded to a non-finite value and was rejected.
            }
            Err(e) => panic!("unexpected error: {e:?}"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Normalization combined with parsing
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn parsed_yaw_normalizes_correctly() {
    let pkt = build_packet(0.0, 0.0, 0.0, 90.0, 0.0, 0.0);
    let pos = parse_packet(&pkt).unwrap();
    let norm = yaw_to_normalized(pos.yaw_deg);
    assert!((norm - 0.75).abs() < 1e-10);
}

#[test]
fn parsed_pitch_normalizes_correctly() {
    let pkt = build_packet(0.0, 0.0, 0.0, 0.0, -45.0, 0.0);
    let pos = parse_packet(&pkt).unwrap();
    let norm = pitch_to_normalized(pos.pitch_deg);
    assert!((norm - 0.25).abs() < 1e-10);
}

#[test]
fn full_pipeline_parse_then_normalize_all_axes() {
    let pkt = build_packet(50.0, -25.0, 100.0, -180.0, 90.0, 0.0);
    let pos = parse_packet(&pkt).unwrap();
    assert!((yaw_to_normalized(pos.yaw_deg)).abs() < 1e-10);
    assert!((pitch_to_normalized(pos.pitch_deg) - 1.0).abs() < 1e-10);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Property-based tests (proptest)
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #[test]
    fn arbitrary_48_bytes_never_panics(data: [u8; 48]) {
        let _ = parse_packet(&data);
    }

    #[test]
    fn arbitrary_short_buffers_never_panic(len in 0..48_usize) {
        let data = vec![0u8; len];
        let result = parse_packet(&data);
        prop_assert_eq!(result, Err(OpenTrackError::PacketTooShort { actual: len }));
    }

    #[test]
    fn arbitrary_long_buffers_parse_successfully(
        base in prop::collection::vec(any::<u8>(), OPENTRACK_PACKET_SIZE..=OPENTRACK_PACKET_SIZE),
        extra in prop::collection::vec(any::<u8>(), 0..256),
    ) {
        let mut data = base;
        data.extend_from_slice(&extra);
        let _ = parse_packet(&data);
    }

    #[test]
    fn finite_values_round_trip_perfectly(
        x in -1e6_f64..1e6,
        y in -1e6_f64..1e6,
        z in -1e6_f64..1e6,
        yaw in -180.0_f64..180.0,
        pitch in -90.0_f64..90.0,
        roll in -180.0_f64..180.0,
    ) {
        let pkt = build_packet(x, y, z, yaw, pitch, roll);
        let pos = parse_packet(&pkt).expect("finite values must parse");
        prop_assert!((pos.x_mm - x).abs() < 1e-10);
        prop_assert!((pos.y_mm - y).abs() < 1e-10);
        prop_assert!((pos.z_mm - z).abs() < 1e-10);
        prop_assert!((pos.yaw_deg - yaw).abs() < 1e-10);
        prop_assert!((pos.pitch_deg - pitch).abs() < 1e-10);
        prop_assert!((pos.roll_deg - roll).abs() < 1e-10);
    }

    #[test]
    fn binary_round_trip_preserves_bytes(
        x in -1e6_f64..1e6,
        y in -1e6_f64..1e6,
        z in -1e6_f64..1e6,
        yaw in -180.0_f64..180.0,
        pitch in -90.0_f64..90.0,
        roll in -180.0_f64..180.0,
    ) {
        let original = build_packet(x, y, z, yaw, pitch, roll);
        let pos = parse_packet(&original).unwrap();
        let rebuilt = build_packet(
            pos.x_mm, pos.y_mm, pos.z_mm,
            pos.yaw_deg, pos.pitch_deg, pos.roll_deg,
        );
        prop_assert_eq!(original, rebuilt);
    }

    #[test]
    fn yaw_normalization_in_range(yaw in -180.0_f64..=180.0) {
        let norm = yaw_to_normalized(yaw);
        prop_assert!(norm >= -1e-10, "normalized yaw should be >= 0, got {norm}");
        prop_assert!(norm <= 1.0 + 1e-10, "normalized yaw should be <= 1, got {norm}");
    }

    #[test]
    fn pitch_normalization_in_range(pitch in -90.0_f64..=90.0) {
        let norm = pitch_to_normalized(pitch);
        prop_assert!(norm >= -1e-10, "normalized pitch should be >= 0, got {norm}");
        prop_assert!(norm <= 1.0 + 1e-10, "normalized pitch should be <= 1, got {norm}");
    }

    #[test]
    fn serde_json_round_trip_arbitrary(
        x in -1e6_f64..1e6,
        y in -1e6_f64..1e6,
        z in -1e6_f64..1e6,
        yaw in -180.0_f64..180.0,
        pitch in -90.0_f64..90.0,
        roll in -180.0_f64..180.0,
    ) {
        let pos = HeadPosition {
            x_mm: x, y_mm: y, z_mm: z,
            yaw_deg: yaw, pitch_deg: pitch, roll_deg: roll,
        };
        let json = serde_json::to_string(&pos).expect("serialize");
        let back: HeadPosition = serde_json::from_str(&json).expect("deserialize");
        // JSON decimal representation may lose the least-significant bits;
        // use a relative tolerance for values that may be large.
        let tol = |a: f64, b: f64| -> bool {
            let diff = (a - b).abs();
            diff <= 1e-10_f64.max(a.abs() * 1e-14)
        };
        prop_assert!(tol(back.x_mm, pos.x_mm));
        prop_assert!(tol(back.y_mm, pos.y_mm));
        prop_assert!(tol(back.z_mm, pos.z_mm));
        prop_assert!(tol(back.yaw_deg, pos.yaw_deg));
        prop_assert!(tol(back.pitch_deg, pos.pitch_deg));
        prop_assert!(tol(back.roll_deg, pos.roll_deg));
    }

    #[test]
    fn adapter_process_then_last_position_matches(
        x in -1e6_f64..1e6,
        y in -1e6_f64..1e6,
        z in -1e6_f64..1e6,
        yaw in -180.0_f64..180.0,
        pitch in -90.0_f64..90.0,
        roll in -180.0_f64..180.0,
    ) {
        let mut adapter = OpenTrackAdapter::new();
        let pkt = build_packet(x, y, z, yaw, pitch, roll);
        let returned = adapter.process_datagram(&pkt).unwrap();
        let cached = adapter.last_position().unwrap();
        prop_assert_eq!(&returned, cached);
    }

    #[test]
    fn yaw_monotonicity_property(a in -180.0_f64..180.0, b in -180.0_f64..180.0) {
        if a < b {
            prop_assert!(yaw_to_normalized(a) <= yaw_to_normalized(b));
        } else if a > b {
            prop_assert!(yaw_to_normalized(a) >= yaw_to_normalized(b));
        } else {
            prop_assert!((yaw_to_normalized(a) - yaw_to_normalized(b)).abs() < 1e-15);
        }
    }

    #[test]
    fn pitch_monotonicity_property(a in -90.0_f64..90.0, b in -90.0_f64..90.0) {
        if a < b {
            prop_assert!(pitch_to_normalized(a) <= pitch_to_normalized(b));
        } else if a > b {
            prop_assert!(pitch_to_normalized(a) >= pitch_to_normalized(b));
        } else {
            prop_assert!((pitch_to_normalized(a) - pitch_to_normalized(b)).abs() < 1e-15);
        }
    }
}
