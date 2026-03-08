// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Depth tests for `flight-il2`.
//!
//! Covers binary packet parsing edge cases, state machine transitions,
//! error handling for malformed packets, aircraft data structures,
//! round-trip serialization, and property-based invariants.

use flight_il2::{
    ConnectionState, GearState, Il2Adapter, Il2AdapterError, Il2AircraftType, Il2TelemetryFrame,
    IL2_DEFAULT_PORT, IL2_MAGIC, MIN_FRAME_SIZE, SUPPORTED_VERSION, convert_frame_to_snapshot,
    parse_telemetry_frame,
};
use proptest::prelude::*;

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

/// Build a frame with extra trailing bytes to test oversized packets.
fn build_frame_with_trailer(
    pitch: f32,
    roll: f32,
    yaw: f32,
    speed: f32,
    altitude: f32,
    throttle: f32,
    gear: u8,
    extra: &[u8],
) -> Vec<u8> {
    let mut buf = build_frame(pitch, roll, yaw, speed, altitude, throttle, gear);
    buf.extend_from_slice(extra);
    buf
}

// ═══════════════════════════════════════════════════════════════════════════════
// §1  Telemetry packet parsing — binary format edge cases
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn parse_exact_min_frame_size_succeeds() {
    let data = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0);
    assert_eq!(data.len(), MIN_FRAME_SIZE);
    assert!(parse_telemetry_frame(&data).is_ok());
}

#[test]
fn parse_frame_with_trailing_bytes_succeeds() {
    let data = build_frame_with_trailer(1.0, 2.0, 3.0, 50.0, 100.0, 0.5, 0, &[0xFF; 64]);
    let frame = parse_telemetry_frame(&data).unwrap();
    assert!((frame.pitch - 1.0).abs() < f32::EPSILON);
    assert!((frame.roll - 2.0).abs() < f32::EPSILON);
}

#[test]
fn parse_frame_single_trailing_byte_succeeds() {
    let data = build_frame_with_trailer(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0, &[0xAB]);
    assert!(parse_telemetry_frame(&data).is_ok());
}

#[test]
fn parse_all_zeros_payload_with_valid_header() {
    let data = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0);
    let frame = parse_telemetry_frame(&data).unwrap();
    assert_eq!(frame.pitch, 0.0);
    assert_eq!(frame.roll, 0.0);
    assert_eq!(frame.yaw, 0.0);
    assert_eq!(frame.speed, 0.0);
    assert_eq!(frame.altitude, 0.0);
    assert_eq!(frame.throttle, 0.0);
    assert_eq!(frame.gear, GearState::Up);
}

#[test]
fn parse_negative_values() {
    let data = build_frame(-45.0, -90.0, -180.0, -10.0, -500.0, -1.0, 0);
    let frame = parse_telemetry_frame(&data).unwrap();
    assert!((frame.pitch - (-45.0)).abs() < f32::EPSILON);
    assert!((frame.roll - (-90.0)).abs() < f32::EPSILON);
    assert!((frame.yaw - (-180.0)).abs() < f32::EPSILON);
    // Speed is stored as-is (negative speeds are valid at parse level)
    assert!((frame.speed - (-10.0)).abs() < f32::EPSILON);
    // Throttle clamped to [0.0, 1.0]
    assert_eq!(frame.throttle, 0.0);
}

#[test]
fn parse_f32_max_values() {
    let data = build_frame(f32::MAX, f32::MAX, f32::MAX, f32::MAX, f32::MAX, f32::MAX, 2);
    let frame = parse_telemetry_frame(&data).unwrap();
    assert_eq!(frame.pitch, f32::MAX);
    assert_eq!(frame.altitude, f32::MAX);
    assert_eq!(frame.throttle, 1.0); // clamped
}

#[test]
fn parse_f32_min_positive_values() {
    let data = build_frame(f32::MIN_POSITIVE, f32::MIN_POSITIVE, 0.0, f32::MIN_POSITIVE, 0.0, f32::MIN_POSITIVE, 0);
    let frame = parse_telemetry_frame(&data).unwrap();
    assert_eq!(frame.pitch, f32::MIN_POSITIVE);
    assert_eq!(frame.throttle, f32::MIN_POSITIVE);
}

#[test]
fn parse_nan_values_preserved_in_frame() {
    let data = build_frame(f32::NAN, 0.0, 0.0, 0.0, 0.0, 0.5, 0);
    let frame = parse_telemetry_frame(&data).unwrap();
    // NaN is preserved at the parse level — validation happens elsewhere
    assert!(frame.pitch.is_nan());
}

#[test]
fn parse_infinity_values_preserved_in_frame() {
    let data = build_frame(f32::INFINITY, f32::NEG_INFINITY, 0.0, 0.0, 0.0, 0.5, 0);
    let frame = parse_telemetry_frame(&data).unwrap();
    assert!(frame.pitch.is_infinite());
    assert!(frame.roll.is_infinite());
}

#[test]
fn parse_subnormal_f32_preserved() {
    let subnormal: f32 = 1.0e-40;
    assert!(subnormal.is_subnormal());
    let data = build_frame(subnormal, 0.0, 0.0, 0.0, 0.0, subnormal, 0);
    let frame = parse_telemetry_frame(&data).unwrap();
    assert_eq!(frame.pitch.to_bits(), subnormal.to_bits());
    assert_eq!(frame.throttle.to_bits(), subnormal.to_bits());
}

#[test]
fn parse_magic_bytes_verified_as_il2_ascii() {
    // IL2_MAGIC = 0x494C_3200 = "IL2\0" in little-endian
    let magic_bytes = IL2_MAGIC.to_le_bytes();
    assert_eq!(magic_bytes[0], b'\0');
    assert_eq!(magic_bytes[1], b'2');
    assert_eq!(magic_bytes[2], b'L');
    assert_eq!(magic_bytes[3], b'I');
}

// ═══════════════════════════════════════════════════════════════════════════════
// §2  Error handling for malformed packets
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn error_frame_too_short_for_every_length_below_minimum() {
    for len in 0..MIN_FRAME_SIZE {
        match parse_telemetry_frame(&vec![0u8; len]) {
            Err(Il2AdapterError::FrameTooShort { found }) => assert_eq!(found, len),
            other => panic!("len={len}: expected FrameTooShort, got {other:?}"),
        }
    }
}

#[test]
fn error_bad_magic_all_zeros() {
    let mut data = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0);
    data[0..4].copy_from_slice(&0u32.to_le_bytes());
    match parse_telemetry_frame(&data) {
        Err(Il2AdapterError::BadMagic { found: 0 }) => {}
        other => panic!("expected BadMagic(0), got {other:?}"),
    }
}

#[test]
fn error_bad_magic_all_ff() {
    let mut data = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0);
    data[0..4].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
    assert!(matches!(
        parse_telemetry_frame(&data),
        Err(Il2AdapterError::BadMagic { found: 0xFFFF_FFFF })
    ));
}

#[test]
fn error_bad_magic_off_by_one() {
    let mut data = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0);
    let off_by_one = IL2_MAGIC + 1;
    data[0..4].copy_from_slice(&off_by_one.to_le_bytes());
    assert!(matches!(
        parse_telemetry_frame(&data),
        Err(Il2AdapterError::BadMagic { .. })
    ));
}

#[test]
fn error_unsupported_version_zero() {
    let mut data = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0);
    data[4..8].copy_from_slice(&0u32.to_le_bytes());
    assert!(matches!(
        parse_telemetry_frame(&data),
        Err(Il2AdapterError::UnsupportedVersion { found: 0 })
    ));
}

#[test]
fn error_unsupported_version_max() {
    let mut data = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0);
    data[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        parse_telemetry_frame(&data),
        Err(Il2AdapterError::UnsupportedVersion { found }) if found == u32::MAX
    ));
}

#[test]
fn error_unsupported_version_two() {
    let mut data = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0);
    data[4..8].copy_from_slice(&2u32.to_le_bytes());
    assert!(matches!(
        parse_telemetry_frame(&data),
        Err(Il2AdapterError::UnsupportedVersion { found: 2 })
    ));
}

#[test]
fn error_display_messages_are_human_readable() {
    let e1 = Il2AdapterError::FrameTooShort { found: 10 };
    let msg = format!("{e1}");
    assert!(msg.contains("10"), "message: {msg}");
    assert!(msg.contains("33"), "message: {msg}");

    let e2 = Il2AdapterError::BadMagic { found: 0xDEADBEEF };
    let msg = format!("{e2}");
    assert!(msg.contains("0xdeadbeef") || msg.contains("0xDEADBEEF"), "message: {msg}");

    let e3 = Il2AdapterError::UnsupportedVersion { found: 42 };
    let msg = format!("{e3}");
    assert!(msg.contains("42"), "message: {msg}");

    let e4 = Il2AdapterError::ReadError { offset: 8 };
    let msg = format!("{e4}");
    assert!(msg.contains("8"), "message: {msg}");

    let e5 = Il2AdapterError::ConversionError {
        field: "speed",
        reason: "negative".to_string(),
    };
    let msg = format!("{e5}");
    assert!(msg.contains("speed"), "message: {msg}");
    assert!(msg.contains("negative"), "message: {msg}");
}

#[test]
fn error_eq_impl_works() {
    assert_eq!(
        Il2AdapterError::FrameTooShort { found: 5 },
        Il2AdapterError::FrameTooShort { found: 5 }
    );
    assert_ne!(
        Il2AdapterError::FrameTooShort { found: 5 },
        Il2AdapterError::FrameTooShort { found: 6 }
    );
    assert_ne!(
        Il2AdapterError::FrameTooShort { found: 5 },
        Il2AdapterError::BadMagic { found: 5 }
    );
}

#[test]
fn error_debug_impl_works() {
    let e = Il2AdapterError::FrameTooShort { found: 0 };
    let dbg = format!("{e:?}");
    assert!(dbg.contains("FrameTooShort"), "debug: {dbg}");
}

// ═══════════════════════════════════════════════════════════════════════════════
// §3  GearState
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn gear_state_try_from_all_valid() {
    assert_eq!(GearState::try_from(0u8), Ok(GearState::Up));
    assert_eq!(GearState::try_from(1u8), Ok(GearState::Transitioning));
    assert_eq!(GearState::try_from(2u8), Ok(GearState::Down));
}

#[test]
fn gear_state_try_from_invalid_returns_err() {
    for byte in 3..=255u8 {
        assert_eq!(GearState::try_from(byte), Err(byte));
    }
}

#[test]
fn gear_state_repr_u8_round_trip() {
    let variants = [GearState::Up, GearState::Transitioning, GearState::Down];
    for variant in variants {
        let byte = variant as u8;
        assert_eq!(GearState::try_from(byte), Ok(variant));
    }
}

#[test]
fn gear_state_clone_and_copy() {
    let g = GearState::Down;
    let g2 = g;
    let g3 = g.clone();
    assert_eq!(g, g2);
    assert_eq!(g, g3);
}

#[test]
fn gear_state_serde_round_trip() {
    for variant in [GearState::Up, GearState::Transitioning, GearState::Down] {
        let json = serde_json::to_string(&variant).unwrap();
        let back: GearState = serde_json::from_str(&json).unwrap();
        assert_eq!(back, variant);
    }
}

#[test]
fn gear_state_unknown_byte_in_frame_defaults_to_up() {
    // The parser uses unwrap_or(GearState::Up) for unknown gear bytes
    for byte in [3u8, 127, 128, 254, 255] {
        let data = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, byte);
        let frame = parse_telemetry_frame(&data).unwrap();
        assert_eq!(frame.gear, GearState::Up, "byte={byte}");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// §4  State machine transitions
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn state_machine_initial_state() {
    let adapter = Il2Adapter::new();
    assert_eq!(adapter.state(), ConnectionState::Disconnected);
    assert_eq!(adapter.error_count(), 0);
    assert!(adapter.last_frame().is_none());
}

#[test]
fn state_machine_disconnected_to_connected() {
    let mut adapter = Il2Adapter::new();
    let data = build_frame(0.0, 0.0, 0.0, 50.0, 100.0, 0.5, 0);
    adapter.process_datagram(&data).unwrap();
    assert_eq!(adapter.state(), ConnectionState::Connected);
}

#[test]
fn state_machine_disconnected_to_error() {
    let mut adapter = Il2Adapter::new();
    let _ = adapter.process_datagram(&[0u8; 4]);
    assert_eq!(adapter.state(), ConnectionState::Error);
}

#[test]
fn state_machine_connected_to_connected() {
    let mut adapter = Il2Adapter::new();
    let data = build_frame(0.0, 0.0, 0.0, 50.0, 100.0, 0.5, 0);
    adapter.process_datagram(&data).unwrap();
    assert_eq!(adapter.state(), ConnectionState::Connected);
    adapter.process_datagram(&data).unwrap();
    assert_eq!(adapter.state(), ConnectionState::Connected);
}

#[test]
fn state_machine_connected_to_error() {
    let mut adapter = Il2Adapter::new();
    let data = build_frame(0.0, 0.0, 0.0, 50.0, 100.0, 0.5, 0);
    adapter.process_datagram(&data).unwrap();
    assert_eq!(adapter.state(), ConnectionState::Connected);
    let _ = adapter.process_datagram(&[0u8; 4]);
    assert_eq!(adapter.state(), ConnectionState::Error);
}

#[test]
fn state_machine_error_to_connected() {
    let mut adapter = Il2Adapter::new();
    let _ = adapter.process_datagram(&[0u8; 4]);
    assert_eq!(adapter.state(), ConnectionState::Error);
    let data = build_frame(0.0, 0.0, 0.0, 50.0, 100.0, 0.5, 0);
    adapter.process_datagram(&data).unwrap();
    assert_eq!(adapter.state(), ConnectionState::Connected);
}

#[test]
fn state_machine_error_to_error() {
    let mut adapter = Il2Adapter::new();
    let _ = adapter.process_datagram(&[0u8; 4]);
    assert_eq!(adapter.state(), ConnectionState::Error);
    assert_eq!(adapter.error_count(), 1);
    let _ = adapter.process_datagram(&[0u8; 4]);
    assert_eq!(adapter.state(), ConnectionState::Error);
    assert_eq!(adapter.error_count(), 2);
}

#[test]
fn state_machine_reset_from_connected() {
    let mut adapter = Il2Adapter::new();
    let data = build_frame(0.0, 0.0, 0.0, 50.0, 100.0, 0.5, 0);
    adapter.process_datagram(&data).unwrap();
    adapter.reset();
    assert_eq!(adapter.state(), ConnectionState::Disconnected);
    assert_eq!(adapter.error_count(), 0);
    assert!(adapter.last_frame().is_none());
}

#[test]
fn state_machine_reset_from_error() {
    let mut adapter = Il2Adapter::new();
    let _ = adapter.process_datagram(&[0u8; 4]);
    adapter.reset();
    assert_eq!(adapter.state(), ConnectionState::Disconnected);
    assert_eq!(adapter.error_count(), 0);
}

#[test]
fn state_machine_full_lifecycle() {
    let mut adapter = Il2Adapter::new();
    let valid = build_frame(5.0, 0.0, 0.0, 100.0, 500.0, 0.5, 0);

    // Disconnected → Connected
    assert_eq!(adapter.state(), ConnectionState::Disconnected);
    adapter.process_datagram(&valid).unwrap();
    assert_eq!(adapter.state(), ConnectionState::Connected);

    // Connected → Error
    let _ = adapter.process_datagram(&[0u8; 4]);
    assert_eq!(adapter.state(), ConnectionState::Error);
    assert_eq!(adapter.error_count(), 1);

    // Error → Connected (recovery)
    adapter.process_datagram(&valid).unwrap();
    assert_eq!(adapter.state(), ConnectionState::Connected);

    // Connected → Error → Error (consecutive errors)
    let _ = adapter.process_datagram(&[0u8; 4]);
    let _ = adapter.process_datagram(&[0u8; 4]);
    assert_eq!(adapter.state(), ConnectionState::Error);
    assert_eq!(adapter.error_count(), 3);

    // Reset → Disconnected
    adapter.reset();
    assert_eq!(adapter.state(), ConnectionState::Disconnected);
    assert_eq!(adapter.error_count(), 0);
    assert!(adapter.last_frame().is_none());
}

#[test]
fn sequence_counter_increments_correctly() {
    let mut adapter = Il2Adapter::new();
    // Verify the error counter increments once per bad datagram
    for _ in 0..1000 {
        let _ = adapter.process_datagram(&[0u8; 4]);
    }
    assert_eq!(adapter.error_count(), 1000);
}

#[test]
fn state_machine_last_frame_preserved_after_error() {
    let mut adapter = Il2Adapter::new();
    let data = build_frame(10.0, 0.0, 0.0, 50.0, 500.0, 0.5, 0);
    adapter.process_datagram(&data).unwrap();
    let cached_pitch = adapter.last_frame().unwrap().pitch;

    // An error does NOT clear last_frame
    let _ = adapter.process_datagram(&[0u8; 4]);
    assert_eq!(adapter.state(), ConnectionState::Error);
    assert!(adapter.last_frame().is_some());
    assert!((adapter.last_frame().unwrap().pitch - cached_pitch).abs() < f32::EPSILON);
}

#[test]
fn connection_state_default_is_disconnected() {
    assert_eq!(ConnectionState::default(), ConnectionState::Disconnected);
}

#[test]
fn connection_state_serde_round_trip() {
    for state in [
        ConnectionState::Disconnected,
        ConnectionState::Connected,
        ConnectionState::Error,
    ] {
        let json = serde_json::to_string(&state).unwrap();
        let back: ConnectionState = serde_json::from_str(&json).unwrap();
        assert_eq!(back, state);
    }
}

#[test]
fn connection_state_clone_copy_eq() {
    let s = ConnectionState::Connected;
    let s2 = s;
    let s3 = s.clone();
    assert_eq!(s, s2);
    assert_eq!(s, s3);
    assert_ne!(ConnectionState::Connected, ConnectionState::Error);
}

// ═══════════════════════════════════════════════════════════════════════════════
// §5  Aircraft data structures
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn aircraft_type_from_name_spitfire_variants() {
    let names = [
        "Spitfire Mk.Vb",
        "SPITFIRE MK IX",
        "spitfire mk.xiv",
        "Supermarine Spitfire",
    ];
    for name in names {
        assert_eq!(
            Il2AircraftType::from_name(name),
            Il2AircraftType::Spitfire,
            "name={name}"
        );
    }
}

#[test]
fn aircraft_type_from_name_bf109_variants() {
    let names = [
        "Bf 109 G-14",
        "BF109 E-4",
        "bf109g6",
        "Bf 109 K-4",
        "bf 109 f-2",
    ];
    for name in names {
        assert_eq!(
            Il2AircraftType::from_name(name),
            Il2AircraftType::Bf109,
            "name={name}"
        );
    }
}

#[test]
fn aircraft_type_from_name_p51_variants() {
    let names = ["P-51D Mustang", "P51D-25", "p-51b", "P51"];
    for name in names {
        assert_eq!(
            Il2AircraftType::from_name(name),
            Il2AircraftType::P51,
            "name={name}"
        );
    }
}

#[test]
fn aircraft_type_from_name_fw190_variants() {
    let names = ["Fw 190 A-8", "FW190 D-9", "fw190a5", "Fw 190 A-3"];
    for name in names {
        assert_eq!(
            Il2AircraftType::from_name(name),
            Il2AircraftType::Fw190,
            "name={name}"
        );
    }
}

#[test]
fn aircraft_type_from_name_il2_variants() {
    let names = [
        "IL-2 mod.1943",
        "il2 shturmovik",
        "IL-2 Type 3",
        "il-2 mod.1941",
    ];
    for name in names {
        assert_eq!(
            Il2AircraftType::from_name(name),
            Il2AircraftType::Il2Shturmovik,
            "name={name}"
        );
    }
}

#[test]
fn aircraft_type_from_name_unknown() {
    let names = [
        "Unknown Aircraft",
        "Boeing 747",
        "",
        "   ",
        "La-5FN",
        "Yak-1",
        "A6M Zero",
    ];
    for name in names {
        assert_eq!(
            Il2AircraftType::from_name(name),
            Il2AircraftType::Unknown,
            "name={name:?}"
        );
    }
}

#[test]
fn aircraft_type_hash_and_eq() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(Il2AircraftType::Spitfire);
    set.insert(Il2AircraftType::Bf109);
    set.insert(Il2AircraftType::Spitfire); // duplicate
    assert_eq!(set.len(), 2);
}

#[test]
fn aircraft_type_serde_round_trip() {
    let variants = [
        Il2AircraftType::Spitfire,
        Il2AircraftType::Bf109,
        Il2AircraftType::P51,
        Il2AircraftType::Fw190,
        Il2AircraftType::Il2Shturmovik,
        Il2AircraftType::Unknown,
    ];
    for variant in variants {
        let json = serde_json::to_string(&variant).unwrap();
        let back: Il2AircraftType = serde_json::from_str(&json).unwrap();
        assert_eq!(back, variant, "variant={variant:?}");
    }
}

#[test]
fn aircraft_type_clone_copy_debug() {
    let a = Il2AircraftType::P51;
    let a2 = a;
    let a3 = a.clone();
    assert_eq!(a, a2);
    assert_eq!(a, a3);
    let dbg = format!("{a:?}");
    assert!(dbg.contains("P51"), "debug={dbg}");
}

// ═══════════════════════════════════════════════════════════════════════════════
// §6  Round-trip serialization
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn telemetry_frame_serde_round_trip_all_gear_states() {
    for gear in [GearState::Up, GearState::Transitioning, GearState::Down] {
        let frame = Il2TelemetryFrame {
            pitch: 15.5,
            roll: -30.0,
            yaw: 270.0,
            speed: 150.0,
            altitude: 7500.0,
            throttle: 0.95,
            gear,
        };
        let json = serde_json::to_string(&frame).unwrap();
        let back: Il2TelemetryFrame = serde_json::from_str(&json).unwrap();
        assert_eq!(back, frame, "gear={gear:?}");
    }
}

#[test]
fn telemetry_frame_serde_preserves_precision() {
    let frame = Il2TelemetryFrame {
        pitch: 1.23456789,
        roll: -0.00001,
        yaw: 359.99999,
        speed: 0.123456,
        altitude: 99999.999,
        throttle: 0.5000001,
        gear: GearState::Up,
    };
    let json = serde_json::to_string(&frame).unwrap();
    let back: Il2TelemetryFrame = serde_json::from_str(&json).unwrap();
    assert_eq!(back, frame);
}

#[test]
fn binary_build_parse_round_trip_all_gear_states() {
    for gear_byte in 0..=2u8 {
        let pitch = 12.34;
        let roll = -56.78;
        let yaw = 123.45;
        let speed = 200.0;
        let altitude = 8000.0;
        let throttle = 0.65;
        let data = build_frame(pitch, roll, yaw, speed, altitude, throttle, gear_byte);
        let frame = parse_telemetry_frame(&data).unwrap();
        assert!((frame.pitch - pitch).abs() < f32::EPSILON);
        assert!((frame.roll - roll).abs() < f32::EPSILON);
        assert!((frame.yaw - yaw).abs() < f32::EPSILON);
        assert!((frame.speed - speed).abs() < f32::EPSILON);
        assert!((frame.altitude - altitude).abs() < f32::EPSILON);
        assert!((frame.throttle - throttle).abs() < f32::EPSILON);
        assert_eq!(frame.gear, GearState::try_from(gear_byte).unwrap());
    }
}

#[test]
fn telemetry_frame_default_serde_round_trip() {
    let frame = Il2TelemetryFrame::default();
    let json = serde_json::to_string(&frame).unwrap();
    let back: Il2TelemetryFrame = serde_json::from_str(&json).unwrap();
    assert_eq!(back, frame);
}

// ═══════════════════════════════════════════════════════════════════════════════
// §7  BusSnapshot conversion edge cases
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn convert_frame_zero_throttle_engine_not_running() {
    let frame = Il2TelemetryFrame {
        throttle: 0.0,
        ..Il2TelemetryFrame::default()
    };
    let snap = convert_frame_to_snapshot(&frame).unwrap();
    assert!(!snap.engines[0].running);
}

#[test]
fn convert_frame_tiny_throttle_engine_running() {
    let frame = Il2TelemetryFrame {
        throttle: 0.001,
        ..Il2TelemetryFrame::default()
    };
    let snap = convert_frame_to_snapshot(&frame).unwrap();
    assert!(snap.engines[0].running);
}

#[test]
fn convert_frame_full_throttle() {
    let frame = Il2TelemetryFrame {
        throttle: 1.0,
        speed: 100.0,
        altitude: 1000.0,
        ..Il2TelemetryFrame::default()
    };
    let snap = convert_frame_to_snapshot(&frame).unwrap();
    assert!(snap.engines[0].running);
    assert!((snap.control_inputs.throttle[0] - 1.0).abs() < f32::EPSILON);
}

#[test]
fn convert_frame_altitude_meters_to_feet() {
    let frame = Il2TelemetryFrame {
        altitude: 1000.0,
        ..Il2TelemetryFrame::default()
    };
    let snap = convert_frame_to_snapshot(&frame).unwrap();
    // 1000m = 3280.84ft
    assert!((snap.environment.altitude - 3280.84).abs() < 1.0);
    assert_eq!(snap.environment.altitude, snap.environment.pressure_altitude);
}

#[test]
fn convert_frame_zero_altitude() {
    let frame = Il2TelemetryFrame::default();
    let snap = convert_frame_to_snapshot(&frame).unwrap();
    assert!((snap.environment.altitude - 0.0).abs() < 0.01);
}

#[test]
fn convert_frame_validity_flags_set() {
    let frame = Il2TelemetryFrame {
        speed: 50.0,
        ..Il2TelemetryFrame::default()
    };
    let snap = convert_frame_to_snapshot(&frame).unwrap();
    assert!(snap.validity.attitude_valid);
    assert!(snap.validity.velocities_valid);
}

#[test]
fn convert_frame_single_engine_data() {
    let frame = Il2TelemetryFrame {
        throttle: 0.5,
        ..Il2TelemetryFrame::default()
    };
    let snap = convert_frame_to_snapshot(&frame).unwrap();
    assert_eq!(snap.engines.len(), 1);
    assert_eq!(snap.engines[0].index, 0);
}

#[test]
fn convert_frame_single_throttle_in_control_inputs() {
    let frame = Il2TelemetryFrame {
        throttle: 0.42,
        ..Il2TelemetryFrame::default()
    };
    let snap = convert_frame_to_snapshot(&frame).unwrap();
    assert_eq!(snap.control_inputs.throttle.len(), 1);
    assert!((snap.control_inputs.throttle[0] - 0.42).abs() < f32::EPSILON);
}

// ═══════════════════════════════════════════════════════════════════════════════
// §8  Adapter construction and configuration
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn adapter_default_port_constant() {
    assert_eq!(IL2_DEFAULT_PORT, 34385);
}

#[test]
fn adapter_new_uses_default_port() {
    let adapter = Il2Adapter::new();
    assert_eq!(adapter.port, IL2_DEFAULT_PORT);
}

#[test]
fn adapter_with_port_zero() {
    let adapter = Il2Adapter::with_port(0);
    assert_eq!(adapter.port, 0);
}

#[test]
fn adapter_with_port_max() {
    let adapter = Il2Adapter::with_port(u16::MAX);
    assert_eq!(adapter.port, u16::MAX);
}

#[test]
fn adapter_default_trait_matches_new() {
    let a = Il2Adapter::new();
    let b = Il2Adapter::default();
    assert_eq!(a.port, b.port);
    assert_eq!(a.state(), b.state());
    assert_eq!(a.error_count(), b.error_count());
}

// ═══════════════════════════════════════════════════════════════════════════════
// §9  SimAdapter trait
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn sim_adapter_validate_accepts_finite_positive_speed() {
    use flight_bus::adapters::SimAdapter;
    let adapter = Il2Adapter::new();
    let frame = Il2TelemetryFrame {
        speed: 100.0,
        altitude: 5000.0,
        pitch: 10.0,
        roll: -5.0,
        yaw: 180.0,
        throttle: 0.5,
        gear: GearState::Down,
    };
    assert!(adapter.validate_raw_data(&frame).is_ok());
}

#[test]
fn sim_adapter_validate_rejects_nan_altitude() {
    use flight_bus::adapters::SimAdapter;
    let adapter = Il2Adapter::new();
    let frame = Il2TelemetryFrame {
        altitude: f32::NAN,
        ..Il2TelemetryFrame::default()
    };
    assert!(adapter.validate_raw_data(&frame).is_err());
}

#[test]
fn sim_adapter_validate_rejects_infinite_altitude() {
    use flight_bus::adapters::SimAdapter;
    let adapter = Il2Adapter::new();
    let frame = Il2TelemetryFrame {
        altitude: f32::INFINITY,
        ..Il2TelemetryFrame::default()
    };
    assert!(adapter.validate_raw_data(&frame).is_err());
}

#[test]
fn sim_adapter_validate_rejects_nan_pitch() {
    use flight_bus::adapters::SimAdapter;
    let adapter = Il2Adapter::new();
    let frame = Il2TelemetryFrame {
        pitch: f32::NAN,
        ..Il2TelemetryFrame::default()
    };
    assert!(adapter.validate_raw_data(&frame).is_err());
}

#[test]
fn sim_adapter_validate_rejects_infinite_roll() {
    use flight_bus::adapters::SimAdapter;
    let adapter = Il2Adapter::new();
    let frame = Il2TelemetryFrame {
        roll: f32::INFINITY,
        ..Il2TelemetryFrame::default()
    };
    assert!(adapter.validate_raw_data(&frame).is_err());
}

#[test]
fn sim_adapter_validate_rejects_nan_yaw() {
    use flight_bus::adapters::SimAdapter;
    let adapter = Il2Adapter::new();
    let frame = Il2TelemetryFrame {
        yaw: f32::NAN,
        ..Il2TelemetryFrame::default()
    };
    assert!(adapter.validate_raw_data(&frame).is_err());
}

#[test]
fn sim_adapter_convert_to_snapshot_succeeds_for_valid_frame() {
    use flight_bus::adapters::SimAdapter;
    use flight_bus::types::SimId;
    let adapter = Il2Adapter::new();
    let frame = Il2TelemetryFrame {
        pitch: 5.0,
        roll: -10.0,
        yaw: 45.0,
        speed: 80.0,
        altitude: 2000.0,
        throttle: 0.7,
        gear: GearState::Down,
    };
    let snap = adapter.convert_to_snapshot(frame).unwrap();
    assert_eq!(snap.sim, SimId::Il2);
}

// ═══════════════════════════════════════════════════════════════════════════════
// §10  Throttle clamping
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn throttle_clamped_to_zero_when_negative() {
    let data = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0);
    let frame = parse_telemetry_frame(&data).unwrap();
    assert_eq!(frame.throttle, 0.0);
}

#[test]
fn throttle_clamped_to_one_when_above() {
    let data = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, 2.5, 0);
    let frame = parse_telemetry_frame(&data).unwrap();
    assert_eq!(frame.throttle, 1.0);
}

#[test]
fn throttle_exact_boundaries() {
    // Exactly 0.0
    let data0 = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0);
    assert_eq!(parse_telemetry_frame(&data0).unwrap().throttle, 0.0);
    // Exactly 1.0
    let data1 = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0);
    assert_eq!(parse_telemetry_frame(&data1).unwrap().throttle, 1.0);
}

#[test]
fn throttle_just_inside_boundaries() {
    let eps = f32::EPSILON;
    let data_low = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, eps, 0);
    let t_low = parse_telemetry_frame(&data_low).unwrap().throttle;
    assert!(t_low > 0.0 && t_low <= 1.0);

    let data_high = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, 1.0 - eps, 0);
    let t_high = parse_telemetry_frame(&data_high).unwrap().throttle;
    assert!(t_high >= 0.0 && t_high < 1.0);
}

#[test]
fn throttle_nan_sanitized_to_zero() {
    let data = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, f32::NAN, 0);
    let frame = parse_telemetry_frame(&data).unwrap();
    assert_eq!(frame.throttle, 0.0, "NaN throttle should be sanitized to 0.0");
}

#[test]
fn throttle_nan_frame_converts_to_snapshot() {
    // After parser sanitizes NaN → 0.0, conversion should succeed
    let data = build_frame(0.0, 0.0, 0.0, 50.0, 100.0, f32::NAN, 0);
    let frame = parse_telemetry_frame(&data).unwrap();
    assert_eq!(frame.throttle, 0.0);
    let snap = convert_frame_to_snapshot(&frame).unwrap();
    assert!(!snap.engines[0].running);
}

// ═══════════════════════════════════════════════════════════════════════════════
// §11  Constants
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn constants_are_correct() {
    assert_eq!(IL2_MAGIC, 0x494C_3200);
    assert_eq!(IL2_DEFAULT_PORT, 34385);
    assert_eq!(MIN_FRAME_SIZE, 33);
    assert_eq!(SUPPORTED_VERSION, 1);
}

#[test]
fn min_frame_size_matches_layout() {
    // 4 (magic) + 4 (version) + 4*6 (floats) + 1 (gear) = 33
    assert_eq!(4 + 4 + 4 * 6 + 1, MIN_FRAME_SIZE);
}

// ═══════════════════════════════════════════════════════════════════════════════
// §12  Property-based tests
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #[test]
    fn prop_parse_never_panics_on_arbitrary_bytes(data in proptest::collection::vec(any::<u8>(), 0..256)) {
        // Must never panic regardless of input
        let _ = parse_telemetry_frame(&data);
    }

    #[test]
    fn prop_valid_frame_round_trips(
        pitch in -180.0f32..180.0,
        roll in -180.0f32..180.0,
        yaw in 0.0f32..360.0,
        speed in 0.0f32..500.0,
        altitude in 0.0f32..15000.0,
        throttle in 0.0f32..1.0,
        gear in 0u8..=2,
    ) {
        let data = build_frame(pitch, roll, yaw, speed, altitude, throttle, gear);
        let frame = parse_telemetry_frame(&data).unwrap();
        prop_assert!((frame.pitch - pitch).abs() < f32::EPSILON);
        prop_assert!((frame.roll - roll).abs() < f32::EPSILON);
        prop_assert!((frame.yaw - yaw).abs() < f32::EPSILON);
        prop_assert!((frame.speed - speed).abs() < f32::EPSILON);
        prop_assert!((frame.altitude - altitude).abs() < f32::EPSILON);
        prop_assert!((frame.throttle - throttle).abs() < f32::EPSILON);
        prop_assert_eq!(frame.gear, GearState::try_from(gear).unwrap());
    }

    #[test]
    fn prop_throttle_always_clamped_to_unit_interval(raw_throttle in proptest::num::f32::ANY) {
        let data = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, raw_throttle, 0);
        let frame = parse_telemetry_frame(&data).unwrap();
        prop_assert!(frame.throttle >= 0.0, "throttle={} from raw={}", frame.throttle, raw_throttle);
        prop_assert!(frame.throttle <= 1.0, "throttle={} from raw={}", frame.throttle, raw_throttle);
    }

    #[test]
    fn prop_bad_magic_always_rejected(magic in any::<u32>().prop_filter("not IL2 magic", |m| *m != IL2_MAGIC)) {
        let mut data = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0);
        data[0..4].copy_from_slice(&magic.to_le_bytes());
        let result = parse_telemetry_frame(&data);
        prop_assert!(result.is_err(), "expected BadMagic error");
        if let Err(Il2AdapterError::BadMagic { found }) = result {
            prop_assert_eq!(found, magic);
        } else {
            prop_assert!(false, "expected BadMagic variant");
        }
    }

    #[test]
    fn prop_bad_version_always_rejected(version in any::<u32>().prop_filter("not supported", |v| *v != SUPPORTED_VERSION)) {
        let mut data = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0);
        data[4..8].copy_from_slice(&version.to_le_bytes());
        let result = parse_telemetry_frame(&data);
        prop_assert!(result.is_err(), "expected UnsupportedVersion error");
        if let Err(Il2AdapterError::UnsupportedVersion { found }) = result {
            prop_assert_eq!(found, version);
        } else {
            prop_assert!(false, "expected UnsupportedVersion variant");
        }
    }

    #[test]
    fn prop_frame_shorter_than_min_always_rejected(len in 0..MIN_FRAME_SIZE) {
        let data = vec![0u8; len];
        let result = parse_telemetry_frame(&data);
        prop_assert!(result.is_err(), "expected FrameTooShort error");
        if let Err(Il2AdapterError::FrameTooShort { found }) = result {
            prop_assert_eq!(found, len);
        } else {
            prop_assert!(false, "expected FrameTooShort variant");
        }
    }

    #[test]
    fn prop_trailing_bytes_ignored(
        extra_len in 1usize..128,
        fill in any::<u8>(),
    ) {
        let extra = vec![fill; extra_len];
        let data = build_frame_with_trailer(1.0, 2.0, 3.0, 50.0, 100.0, 0.5, 0, &extra);
        let frame = parse_telemetry_frame(&data).unwrap();
        prop_assert!((frame.pitch - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn prop_gear_unknown_bytes_default_to_up(gear in 3u8..=255) {
        let data = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, gear);
        let frame = parse_telemetry_frame(&data).unwrap();
        prop_assert_eq!(frame.gear, GearState::Up);
    }

    #[test]
    fn prop_serde_round_trip_preserves_frame(
        pitch in proptest::num::f32::NORMAL,
        roll in proptest::num::f32::NORMAL,
        yaw in proptest::num::f32::NORMAL,
        speed in proptest::num::f32::NORMAL,
        altitude in proptest::num::f32::NORMAL,
        throttle in 0.0f32..=1.0,
        gear in 0u8..=2,
    ) {
        let frame = Il2TelemetryFrame {
            pitch,
            roll,
            yaw,
            speed,
            altitude,
            throttle,
            gear: GearState::try_from(gear).unwrap(),
        };
        let json = serde_json::to_string(&frame).unwrap();
        let back: Il2TelemetryFrame = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(back, frame);
    }

    #[test]
    fn prop_adapter_state_connected_after_valid_datagram(
        speed in 0.0f32..500.0,
        altitude in 0.0f32..15000.0,
    ) {
        let mut adapter = Il2Adapter::new();
        let data = build_frame(0.0, 0.0, 0.0, speed, altitude, 0.5, 0);
        adapter.process_datagram(&data).unwrap();
        prop_assert_eq!(adapter.state(), ConnectionState::Connected);
    }

    #[test]
    fn prop_aircraft_from_name_never_panics(name in ".*") {
        let _ = Il2AircraftType::from_name(&name);
    }
}
