// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Depth tests for `flight-aerofly`.
//!
//! Covers telemetry packet parsing, protocol format validation, adapter state
//! transitions, error handling for malformed data, round-trip serialisation,
//! and property-based invariants.

use flight_aerofly::{
    AEROFLY_DEFAULT_PORT, AEROFLY_MAGIC, AeroflyAdapter, AeroflyAdapterError,
    AeroflyAircraftType, AeroflyTelemetry, MIN_FRAME_SIZE, parse_json_telemetry,
    parse_telemetry, parse_text_telemetry,
};
use proptest::prelude::*;

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

fn telemetry_to_text(t: &AeroflyTelemetry) -> String {
    format!(
        "pitch={}\nroll={}\nhdg={}\nias={}\nalt={}\nthrottle={}\ngear={}\nflaps={}\nvspeed={}",
        t.pitch,
        t.roll,
        t.heading,
        t.airspeed,
        t.altitude,
        t.throttle_pos,
        if t.gear_down { "1.0" } else { "0.0" },
        t.flaps_ratio,
        t.vspeed_fpm,
    )
}

// ── Telemetry packet parsing depth ─────────────────────────────────────────────

#[test]
fn binary_frame_all_zeros_with_valid_magic() {
    let data = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0, 0.0);
    let t = parse_telemetry(&data).unwrap();
    assert_eq!(t, AeroflyTelemetry::default());
}

#[test]
fn binary_frame_with_trailing_bytes_ignored() {
    let mut data = build_frame(5.0, -3.0, 90.0, 100.0, 2000.0, 0.5, 1, 0.2);
    data.extend_from_slice(&[0xAA; 64]);
    let t = parse_telemetry(&data).unwrap();
    assert!((t.pitch - 5.0).abs() < 0.01);
    assert!(t.gear_down);
}

#[test]
fn binary_frame_exactly_min_size() {
    let data = build_frame(1.0, 2.0, 3.0, 4.0, 5.0, 0.5, 0, 0.5);
    assert_eq!(data.len(), MIN_FRAME_SIZE);
    let t = parse_telemetry(&data).unwrap();
    assert!((t.pitch - 1.0).abs() < 0.01);
}

#[test]
fn binary_frame_negative_values() {
    let data = build_frame(-45.0, -90.0, 0.0, 0.0, -200.0, 0.0, 0, 0.0);
    let t = parse_telemetry(&data).unwrap();
    assert!((t.pitch - (-45.0)).abs() < 0.01);
    assert!((t.roll - (-90.0)).abs() < 0.01);
    assert!((t.altitude - (-200.0)).abs() < 0.01);
}

#[test]
fn binary_frame_large_values() {
    let data = build_frame(89.9, -89.9, 359.9, 600.0, 60_000.0, 1.0, 1, 1.0);
    let t = parse_telemetry(&data).unwrap();
    assert!((t.pitch - 89.9).abs() < 0.01);
    assert!((t.altitude - 60_000.0).abs() < 0.1);
    assert!((t.airspeed - 600.0).abs() < 0.01);
}

#[test]
fn binary_frame_nan_in_float_fields() {
    let data = build_frame(f32::NAN, 0.0, 0.0, 0.0, 0.0, 0.0, 0, 0.0);
    let t = parse_telemetry(&data).unwrap();
    assert!(t.pitch.is_nan(), "NaN should propagate through parsing");
}

#[test]
fn binary_frame_inf_in_float_fields() {
    let data = build_frame(f32::INFINITY, f32::NEG_INFINITY, 0.0, 0.0, 0.0, 0.0, 0, 0.0);
    let t = parse_telemetry(&data).unwrap();
    assert!(t.pitch.is_infinite());
    assert!(t.roll.is_infinite());
}

#[test]
fn binary_frame_subnormal_floats() {
    let pos_subnormal = f32::from_bits(1);
    let neg_subnormal = -f32::from_bits(1);
    let data = build_frame(pos_subnormal, neg_subnormal, 0.0, 0.0, 0.0, 0.0, 0, 0.0);
    let t = parse_telemetry(&data).unwrap();
    assert_eq!(t.pitch.to_bits(), pos_subnormal.to_bits());
    assert_eq!(t.roll.to_bits(), neg_subnormal.to_bits());
}

#[test]
fn binary_gear_byte_any_nonzero_is_down() {
    for val in [1u8, 2, 127, 128, 255] {
        let data = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, val, 0.0);
        let t = parse_telemetry(&data).unwrap();
        assert!(t.gear_down, "gear byte {val} should mean down");
    }
}

// ── Protocol format validation ─────────────────────────────────────────────────

#[test]
fn truncated_at_each_field_boundary() {
    let full = build_frame(1.0, 2.0, 3.0, 4.0, 5.0, 0.5, 0, 0.5);
    // Field boundaries: magic(4), pitch(8), roll(12), heading(16),
    // airspeed(20), altitude(24), throttle(28), gear(29), flaps ends at MIN_FRAME_SIZE(33)
    let boundaries = [0, 4, 8, 12, 16, 20, 24, 28, 29, MIN_FRAME_SIZE - 1];
    for &boundary in &boundaries {
        let truncated = &full[..boundary];
        let result = parse_telemetry(truncated);
        assert!(
            result.is_err(),
            "frame truncated at offset {boundary} should fail"
        );
    }
}

#[test]
fn magic_variants_all_rejected() {
    let magics: [u32; 5] = [0x0000_0000, 0xFFFF_FFFF, 0x4146_4652, 0x5346_4641, 0x4146_4654];
    for magic in magics {
        let mut data = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0, 0.0);
        data[0..4].copy_from_slice(&magic.to_le_bytes());
        let err = parse_telemetry(&data).unwrap_err();
        assert!(
            matches!(err, AeroflyAdapterError::BadMagic { found } if found == magic),
            "magic {magic:#010x} should be rejected"
        );
    }
}

#[test]
fn min_frame_size_constant_matches_layout() {
    // magic(4) + pitch(4) + roll(4) + heading(4) + airspeed(4)
    // + altitude(4) + throttle(4) + gear(1) + flaps(4) = 33
    assert_eq!(MIN_FRAME_SIZE, 33);
}

#[test]
fn aerofly_magic_is_affs_ascii() {
    // 0x4146_4653 stored as little-endian: LSB first → [0x53, 0x46, 0x46, 0x41]
    let bytes = AEROFLY_MAGIC.to_le_bytes();
    assert_eq!(&bytes, b"SFFA");
    // The big-endian (human-readable) byte order spells "AFFS"
    let be_bytes = AEROFLY_MAGIC.to_be_bytes();
    assert_eq!(&be_bytes, b"AFFS");
}

#[test]
fn default_port_is_49002() {
    assert_eq!(AEROFLY_DEFAULT_PORT, 49002);
}

// ── State machine / adapter transitions ────────────────────────────────────────

#[test]
fn adapter_sequential_datagrams_update_state() {
    let mut adapter = AeroflyAdapter::new();

    let d1 = build_frame(10.0, 0.0, 0.0, 100.0, 1000.0, 0.5, 0, 0.0);
    adapter.process_datagram(&d1).unwrap();
    assert!((adapter.last_telemetry().unwrap().pitch - 10.0).abs() < 0.01);

    let d2 = build_frame(20.0, 0.0, 0.0, 200.0, 2000.0, 0.8, 1, 0.5);
    adapter.process_datagram(&d2).unwrap();
    let last = adapter.last_telemetry().unwrap();
    assert!((last.pitch - 20.0).abs() < 0.01, "should reflect latest");
    assert!(last.gear_down, "gear should now be down");
}

#[test]
fn adapter_error_does_not_clear_previous_good_state() {
    let mut adapter = AeroflyAdapter::new();

    let good = build_frame(5.0, 0.0, 0.0, 100.0, 3000.0, 0.5, 0, 0.0);
    adapter.process_datagram(&good).unwrap();
    assert!(adapter.last_telemetry().is_some());

    // Bad datagram should fail but not clear cached telemetry
    let bad = vec![0u8; 4];
    assert!(adapter.process_datagram(&bad).is_err());
    assert!(
        adapter.last_telemetry().is_some(),
        "previous good state should be preserved"
    );
    assert!((adapter.last_telemetry().unwrap().pitch - 5.0).abs() < 0.01);
}

#[test]
fn adapter_alternating_paths_binary_json_text() {
    let mut adapter = AeroflyAdapter::new();

    // Binary
    let bin = build_frame(1.0, 0.0, 0.0, 80.0, 500.0, 0.4, 0, 0.0);
    adapter.process_datagram(&bin).unwrap();
    assert!((adapter.last_telemetry().unwrap().pitch - 1.0).abs() < 0.01);

    // JSON overwrites
    let json = r#"{"pitch":2.0,"roll":0.0,"heading":0.0,"airspeed":60.0,"altitude":300.0,"throttle_pos":0.3,"gear_down":true,"flaps_ratio":0.1}"#;
    adapter.process_json(json).unwrap();
    assert!(adapter.last_telemetry().unwrap().gear_down);
    assert!((adapter.last_telemetry().unwrap().pitch - 2.0).abs() < 0.01);

    // Text overwrites
    let text = "pitch=3.0\nalt=100.0\ngear=0.0";
    adapter.process_text(text).unwrap();
    assert!((adapter.last_telemetry().unwrap().pitch - 3.0).abs() < 0.01);
    assert!(!adapter.last_telemetry().unwrap().gear_down);
}

#[test]
fn adapter_error_recovery_after_multiple_failures() {
    let mut adapter = AeroflyAdapter::new();

    // Several failures
    assert!(adapter.process_datagram(&[]).is_err());
    assert!(adapter.process_json("nope").is_err());
    assert!(adapter.process_text("").is_err());
    assert!(adapter.last_telemetry().is_none());

    // Recovery with valid data
    let good = build_frame(7.0, 0.0, 90.0, 110.0, 4000.0, 0.6, 1, 0.3);
    adapter.process_datagram(&good).unwrap();
    assert!(adapter.last_telemetry().is_some());
    assert!((adapter.last_telemetry().unwrap().pitch - 7.0).abs() < 0.01);
}

#[test]
fn adapter_with_port_preserves_port_across_operations() {
    let mut adapter = AeroflyAdapter::with_port(55555);
    assert_eq!(adapter.port, 55555);

    let data = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0, 0.0);
    adapter.process_datagram(&data).unwrap();
    assert_eq!(adapter.port, 55555, "port unchanged after processing");
}

// ── Error handling for malformed data ──────────────────────────────────────────

#[test]
fn error_display_frame_too_short() {
    let err = AeroflyAdapterError::FrameTooShort { found: 10 };
    let msg = err.to_string();
    assert!(msg.contains("10"), "should mention actual size");
    assert!(msg.contains(&MIN_FRAME_SIZE.to_string()));
}

#[test]
fn error_display_bad_magic() {
    let err = AeroflyAdapterError::BadMagic {
        found: 0xDEAD_BEEF,
    };
    let msg = err.to_string();
    assert!(msg.contains("0xdeadbeef") || msg.contains("DEADBEEF") || msg.contains("deadbeef"));
}

#[test]
fn error_display_json_error() {
    let err = AeroflyAdapterError::JsonError("unexpected token".into());
    assert!(err.to_string().contains("unexpected token"));
}

#[test]
fn error_display_empty_data() {
    let err = AeroflyAdapterError::EmptyData;
    assert!(err.to_string().contains("empty"));
}

#[test]
fn error_equality() {
    assert_eq!(
        AeroflyAdapterError::FrameTooShort { found: 5 },
        AeroflyAdapterError::FrameTooShort { found: 5 }
    );
    assert_ne!(
        AeroflyAdapterError::FrameTooShort { found: 5 },
        AeroflyAdapterError::FrameTooShort { found: 6 }
    );
    assert_ne!(
        AeroflyAdapterError::EmptyData,
        AeroflyAdapterError::FrameTooShort { found: 0 }
    );
}

#[test]
fn json_various_malformed_inputs() {
    let cases = [
        ("", "empty string"),
        ("{}", "empty object"),
        ("[]", "array"),
        ("null", "null"),
        ("true", "boolean"),
        ("42", "number"),
        (r#"{"pitch": "not_a_number"}"#, "string in float field"),
        (r#"{"pitch": null}"#, "null in float field"),
        (r#"{"extra_field": 1}"#, "missing required fields"),
    ];
    for (input, label) in cases {
        assert!(
            parse_json_telemetry(input).is_err(),
            "should fail for: {label}"
        );
    }
}

#[test]
fn text_lines_with_no_equals_sign() {
    let text = "pitch5.0\nalt1000";
    let t = parse_text_telemetry(text).unwrap();
    // These are treated as keys without values → unparseable → default to 0
    assert_eq!(t.pitch, 0.0);
    assert_eq!(t.altitude, 0.0);
}

#[test]
fn text_lines_with_multiple_equals() {
    let text = "pitch=5.0=extra\nalt=1000.0";
    let t = parse_text_telemetry(text).unwrap();
    // splitn(2, '=') gives key="pitch", val="5.0=extra" → parse fails → 0.0
    assert_eq!(t.pitch, 0.0);
    assert!((t.altitude - 1000.0).abs() < 0.01);
}

#[test]
fn text_empty_value_after_equals() {
    let text = "pitch=\nalt=1000.0";
    let t = parse_text_telemetry(text).unwrap();
    assert_eq!(t.pitch, 0.0, "empty value should default to 0");
    assert!((t.altitude - 1000.0).abs() < 0.01);
}

#[test]
fn text_whitespace_only_lines_ignored() {
    let text = "pitch=5.0\n   \n\n  \nalt=1000.0";
    let t = parse_text_telemetry(text).unwrap();
    assert!((t.pitch - 5.0).abs() < 0.01);
    assert!((t.altitude - 1000.0).abs() < 0.01);
}

#[test]
fn text_duplicate_keys_last_wins() {
    let text = "pitch=5.0\npitch=10.0\nalt=1000.0";
    let t = parse_text_telemetry(text).unwrap();
    assert!((t.pitch - 10.0).abs() < 0.01, "last pitch value wins");
}

#[test]
fn text_gear_boundary_values() {
    // gear=0.0 → up, gear=0.49 → up, gear=0.5 → up (not > 0.5), gear=0.51 → down
    let cases = [
        (0.0, false),
        (0.49, false),
        (0.5, false),
        (0.51, true),
        (1.0, true),
        (100.0, true),
        (-1.0, false),
    ];
    for (val, expected) in cases {
        let text = format!("gear={val}\nalt=0");
        let t = parse_text_telemetry(&text).unwrap();
        assert_eq!(t.gear_down, expected, "gear={val} expected {expected}");
    }
}

#[test]
fn text_flaps_clamped() {
    let t = parse_text_telemetry("flaps=2.0\nalt=0").unwrap();
    assert!((t.flaps_ratio - 1.0).abs() < 0.01, "flaps clamped to 1.0");

    let t2 = parse_text_telemetry("flaps=-0.5\nalt=0").unwrap();
    assert!((t2.flaps_ratio).abs() < 0.01, "flaps clamped to 0.0");
}

// ── Round-trip serialisation ───────────────────────────────────────────────────

#[test]
fn json_round_trip_all_fields() {
    let original = AeroflyTelemetry {
        pitch: -12.5,
        roll: 45.3,
        heading: 270.0,
        airspeed: 180.0,
        altitude: 35_000.0,
        throttle_pos: 0.92,
        gear_down: true,
        flaps_ratio: 0.15,
        vspeed_fpm: -500.0,
    };
    let json = serde_json::to_string(&original).unwrap();
    let restored: AeroflyTelemetry = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, original);
}

#[test]
fn json_round_trip_with_vspeed_present() {
    let json = r#"{"pitch":0.0,"roll":0.0,"heading":0.0,"airspeed":0.0,"altitude":0.0,"throttle_pos":0.0,"gear_down":false,"flaps_ratio":0.0,"vspeed_fpm":123.4}"#;
    let t = parse_json_telemetry(json).unwrap();
    assert!((t.vspeed_fpm - 123.4).abs() < 0.01);

    let re_json = serde_json::to_string(&t).unwrap();
    let t2: AeroflyTelemetry = serde_json::from_str(&re_json).unwrap();
    assert_eq!(t, t2);
}

#[test]
fn binary_build_parse_round_trip() {
    let frames = [
        (5.0f32, -3.0, 270.0, 120.0, 3_000.0, 0.8, 1u8, 0.3),
        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0, 0.0),
        (-45.0, 89.9, 359.9, 250.0, 40_000.0, 1.0, 1, 1.0),
        (0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0, 0.1),
    ];
    for (pitch, roll, hdg, ias, alt, thr, gear, flaps) in frames {
        let data = build_frame(pitch, roll, hdg, ias, alt, thr, gear, flaps);
        let t = parse_telemetry(&data).unwrap();
        assert!((t.pitch - pitch).abs() < 0.001, "pitch mismatch");
        assert!((t.roll - roll).abs() < 0.001, "roll mismatch");
        assert!((t.heading - hdg).abs() < 0.001, "heading mismatch");
        assert!((t.airspeed - ias).abs() < 0.001, "airspeed mismatch");
        assert!((t.altitude - alt).abs() < 0.001, "altitude mismatch");
        assert!((t.throttle_pos - thr.clamp(0.0, 1.0)).abs() < 0.001);
        assert_eq!(t.gear_down, gear != 0);
        assert!((t.flaps_ratio - flaps.clamp(0.0, 1.0)).abs() < 0.001);
    }
}

#[test]
fn text_round_trip_via_format() {
    let original = AeroflyTelemetry {
        pitch: 15.0,
        roll: -8.0,
        heading: 90.0,
        airspeed: 200.0,
        altitude: 10_000.0,
        throttle_pos: 0.7,
        gear_down: true,
        flaps_ratio: 0.5,
        vspeed_fpm: 300.0,
    };
    let text = telemetry_to_text(&original);
    let restored = parse_text_telemetry(&text).unwrap();

    assert!((restored.pitch - original.pitch).abs() < 0.01);
    assert!((restored.roll - original.roll).abs() < 0.01);
    assert!((restored.heading - original.heading).abs() < 0.01);
    assert!((restored.airspeed - original.airspeed).abs() < 0.01);
    assert!((restored.altitude - original.altitude).abs() < 0.01);
    assert!((restored.throttle_pos - original.throttle_pos).abs() < 0.01);
    assert_eq!(restored.gear_down, original.gear_down);
    assert!((restored.flaps_ratio - original.flaps_ratio).abs() < 0.01);
    assert!((restored.vspeed_fpm - original.vspeed_fpm).abs() < 0.01);
}

// ── Aircraft type depth ────────────────────────────────────────────────────────

#[test]
fn aircraft_type_all_variants_distinguishable() {
    let types = [
        AeroflyAircraftType::Cessna172,
        AeroflyAircraftType::AirbusA320,
        AeroflyAircraftType::BoeingB737,
        AeroflyAircraftType::PiperCherokee,
        AeroflyAircraftType::Extra330,
        AeroflyAircraftType::Unknown,
    ];
    // All variants are distinct
    for (i, a) in types.iter().enumerate() {
        for (j, b) in types.iter().enumerate() {
            if i != j {
                assert_ne!(a, b, "variants at {i} and {j} should differ");
            }
        }
    }
}

#[test]
fn aircraft_type_alternative_name_patterns() {
    // PA28 without hyphen
    assert_eq!(
        AeroflyAircraftType::from_name("PA28 Warrior"),
        AeroflyAircraftType::PiperCherokee
    );
    // C172 shorthand
    assert_eq!(
        AeroflyAircraftType::from_name("C172"),
        AeroflyAircraftType::Cessna172
    );
    // extra330 without space
    assert_eq!(
        AeroflyAircraftType::from_name("extra330"),
        AeroflyAircraftType::Extra330
    );
    // Empty string
    assert_eq!(
        AeroflyAircraftType::from_name(""),
        AeroflyAircraftType::Unknown
    );
}

#[test]
fn aircraft_type_serde_round_trip() {
    let types = [
        AeroflyAircraftType::Cessna172,
        AeroflyAircraftType::AirbusA320,
        AeroflyAircraftType::BoeingB737,
        AeroflyAircraftType::PiperCherokee,
        AeroflyAircraftType::Extra330,
        AeroflyAircraftType::Unknown,
    ];
    for ty in types {
        let json = serde_json::to_string(&ty).unwrap();
        let restored: AeroflyAircraftType = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, ty);
    }
}

#[test]
fn aircraft_type_is_copy_and_hash() {
    let a = AeroflyAircraftType::Cessna172;
    let b = a; // Copy
    assert_eq!(a, b);

    // Hash: can be used in a HashSet
    let mut set = std::collections::HashSet::new();
    set.insert(a);
    assert!(set.contains(&b));
}

// ── Unit conversion depth ──────────────────────────────────────────────────────

#[test]
fn conversion_negative_altitude() {
    let t = AeroflyTelemetry {
        altitude: -100.0,
        ..Default::default()
    };
    assert!(t.altitude_m() < 0.0);
}

#[test]
fn conversion_negative_vspeed() {
    let t = AeroflyTelemetry {
        vspeed_fpm: -1000.0,
        ..Default::default()
    };
    assert!(t.vspeed_ms() < 0.0);
}

#[test]
fn conversion_heading_full_circle() {
    let t = AeroflyTelemetry {
        heading: 360.0,
        ..Default::default()
    };
    assert!((t.heading_rad() - std::f32::consts::TAU).abs() < 0.001);
}

#[test]
fn conversion_pitch_roll_symmetry() {
    let t = AeroflyTelemetry {
        pitch: 45.0,
        roll: -45.0,
        ..Default::default()
    };
    assert!((t.pitch_rad() + t.roll_rad()).abs() < 0.001, "±45° should cancel");
}

// ── Property-based tests ───────────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn prop_arbitrary_binary_never_panics(
        bytes in proptest::collection::vec(any::<u8>(), 0usize..256)
    ) {
        let _ = parse_telemetry(&bytes);
    }

    #[test]
    fn prop_arbitrary_string_text_never_panics(s in ".*") {
        let _ = parse_text_telemetry(&s);
    }

    #[test]
    fn prop_arbitrary_string_json_never_panics(s in ".*") {
        let _ = parse_json_telemetry(&s);
    }

    #[test]
    fn prop_valid_frame_always_parses(
        pitch in any::<f32>(),
        roll in any::<f32>(),
        heading in any::<f32>(),
        airspeed in any::<f32>(),
        altitude in any::<f32>(),
        throttle in any::<f32>(),
        gear in any::<u8>(),
        flaps in any::<f32>(),
    ) {
        let data = build_frame(pitch, roll, heading, airspeed, altitude, throttle, gear, flaps);
        let result = parse_telemetry(&data);
        prop_assert!(result.is_ok(), "valid frame must always parse");
    }

    #[test]
    fn prop_throttle_always_clamped(throttle in any::<f32>()) {
        let data = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, throttle, 0, 0.0);
        if let Ok(t) = parse_telemetry(&data) {
            prop_assert!(t.throttle_pos.is_nan() || (0.0..=1.0).contains(&t.throttle_pos),
                "throttle_pos must be NaN or in [0,1], got {}", t.throttle_pos);
        }
    }

    #[test]
    fn prop_flaps_always_clamped(flaps in any::<f32>()) {
        let data = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0, flaps);
        if let Ok(t) = parse_telemetry(&data) {
            prop_assert!(t.flaps_ratio.is_nan() || (0.0..=1.0).contains(&t.flaps_ratio),
                "flaps_ratio must be NaN or in [0,1], got {}", t.flaps_ratio);
        }
    }

    #[test]
    fn prop_binary_round_trip_preserves_fields(
        pitch in -180.0f32..180.0f32,
        roll in -180.0f32..180.0f32,
        heading in 0.0f32..360.0f32,
        airspeed in 0.0f32..1000.0f32,
        altitude in -2000.0f32..60000.0f32,
        throttle in 0.0f32..1.0f32,
        gear in proptest::bool::ANY,
        flaps in 0.0f32..1.0f32,
    ) {
        let gear_byte = if gear { 1u8 } else { 0u8 };
        let data = build_frame(pitch, roll, heading, airspeed, altitude, throttle, gear_byte, flaps);
        let t = parse_telemetry(&data).unwrap();
        prop_assert!((t.pitch - pitch).abs() < 0.001, "pitch mismatch");
        prop_assert!((t.roll - roll).abs() < 0.001, "roll mismatch");
        prop_assert!((t.heading - heading).abs() < 0.001, "heading mismatch");
        prop_assert!((t.airspeed - airspeed).abs() < 0.001, "airspeed mismatch");
        prop_assert!((t.altitude - altitude).abs() < 0.001, "altitude mismatch");
        prop_assert!((t.throttle_pos - throttle).abs() < 0.001);
        prop_assert_eq!(t.gear_down, gear);
        prop_assert!((t.flaps_ratio - flaps).abs() < 0.001);
    }

    #[test]
    fn prop_json_round_trip_preserves_fields(
        pitch in -180.0f32..180.0f32,
        roll in -180.0f32..180.0f32,
        heading in 0.0f32..360.0f32,
        airspeed in 0.0f32..1000.0f32,
        altitude in -2000.0f32..60000.0f32,
        throttle in 0.0f32..1.0f32,
        gear in proptest::bool::ANY,
        flaps in 0.0f32..1.0f32,
        vspeed in -5000.0f32..5000.0f32,
    ) {
        let t = AeroflyTelemetry {
            pitch, roll, heading, airspeed, altitude,
            throttle_pos: throttle, gear_down: gear,
            flaps_ratio: flaps, vspeed_fpm: vspeed,
        };
        let json = serde_json::to_string(&t).unwrap();
        let restored: AeroflyTelemetry = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored, t);
    }

    #[test]
    fn prop_text_throttle_always_clamped(throttle in -10.0f32..10.0f32) {
        let text = format!("throttle={throttle}\nalt=0");
        let t = parse_text_telemetry(&text).unwrap();
        prop_assert!(t.throttle_pos >= 0.0);
        prop_assert!(t.throttle_pos <= 1.0);
    }

    #[test]
    fn prop_text_flaps_always_clamped(flaps in -10.0f32..10.0f32) {
        let text = format!("flaps={flaps}\nalt=0");
        let t = parse_text_telemetry(&text).unwrap();
        prop_assert!(t.flaps_ratio >= 0.0);
        prop_assert!(t.flaps_ratio <= 1.0);
    }

    #[test]
    fn prop_frame_too_short_for_any_small_buffer(len in 0usize..MIN_FRAME_SIZE) {
        let mut buf = vec![0u8; len];
        // Put valid magic if buffer is large enough
        if len >= 4 {
            buf[0..4].copy_from_slice(&AEROFLY_MAGIC.to_le_bytes());
        }
        let result = parse_telemetry(&buf);
        prop_assert!(result.is_err(), "buffer of len {} should fail", len);
    }

    #[test]
    fn prop_aircraft_type_from_name_never_panics(name in "\\PC{0,100}") {
        let _ = AeroflyAircraftType::from_name(&name);
    }
}

// ── Telemetry Clone / Debug / Default depth ────────────────────────────────────

#[test]
fn telemetry_clone_is_independent() {
    let mut t1 = AeroflyTelemetry {
        pitch: 10.0,
        ..Default::default()
    };
    let t2 = t1.clone();
    t1.pitch = 20.0;
    assert!((t2.pitch - 10.0).abs() < 0.01, "clone should be independent");
    assert!((t1.pitch - 20.0).abs() < 0.01);
}

#[test]
fn telemetry_debug_contains_field_names() {
    let t = AeroflyTelemetry::default();
    let dbg = format!("{t:?}");
    assert!(dbg.contains("pitch"));
    assert!(dbg.contains("roll"));
    assert!(dbg.contains("heading"));
    assert!(dbg.contains("airspeed"));
    assert!(dbg.contains("altitude"));
    assert!(dbg.contains("throttle_pos"));
    assert!(dbg.contains("gear_down"));
    assert!(dbg.contains("flaps_ratio"));
    assert!(dbg.contains("vspeed_fpm"));
}

#[test]
fn adapter_port_accessible() {
    let adapter = AeroflyAdapter::new();
    assert_eq!(adapter.port, AEROFLY_DEFAULT_PORT, "default port should match constant");
}
