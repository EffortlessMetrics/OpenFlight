// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Depth tests for flight-ac7-protocol: telemetry parsing, validation,
//! serialization, and edge-case handling.

use flight_ac7_protocol::{
    Ac7Controls, Ac7ProtocolError, Ac7State, Ac7TelemetryPacket, AC7_TELEMETRY_SCHEMA_V1,
};

// ---------------------------------------------------------------------------
// Valid packet parsing
// ---------------------------------------------------------------------------

#[test]
fn parses_minimal_valid_packet() {
    let payload = r#"{"schema":"flight.ac7.telemetry/1"}"#;
    let packet = Ac7TelemetryPacket::from_json_str(payload).unwrap();
    assert_eq!(packet.schema, AC7_TELEMETRY_SCHEMA_V1);
    assert_eq!(packet.timestamp_ms, 0);
    assert!(packet.aircraft.is_empty());
    assert!(packet.mission.is_none());
}

#[test]
fn parses_fully_populated_packet() {
    let payload = r#"{
        "schema":"flight.ac7.telemetry/1",
        "timestamp_ms":999999,
        "aircraft":"Su-57",
        "mission":"Campaign_M18",
        "state":{
            "altitude_m":12000.0,
            "speed_mps":800.0,
            "ground_speed_mps":750.0,
            "vertical_speed_mps":50.0,
            "heading_deg":270.0,
            "pitch_deg":15.0,
            "roll_deg":-30.0,
            "g_force":4.5,
            "health_norm":0.65
        },
        "controls":{
            "pitch":-0.5,
            "roll":0.3,
            "yaw":-0.1,
            "throttle":0.95,
            "brake":0.0
        }
    }"#;

    let pkt = Ac7TelemetryPacket::from_json_str(payload).unwrap();
    assert_eq!(pkt.aircraft, "Su-57");
    assert_eq!(pkt.mission.as_deref(), Some("Campaign_M18"));
    assert_eq!(pkt.state.altitude_m, Some(12000.0));
    assert_eq!(pkt.state.ground_speed_mps, Some(750.0));
    assert_eq!(pkt.state.vertical_speed_mps, Some(50.0));
    assert_eq!(pkt.state.g_force, Some(4.5));
    assert_eq!(pkt.state.health_norm, Some(0.65));
    assert_eq!(pkt.controls.brake, Some(0.0));
}

#[test]
fn parses_packet_with_null_optionals() {
    let payload = r#"{
        "schema":"flight.ac7.telemetry/1",
        "mission":null,
        "state":{"altitude_m":null,"speed_mps":null},
        "controls":{"pitch":null}
    }"#;
    let pkt = Ac7TelemetryPacket::from_json_str(payload).unwrap();
    assert!(pkt.mission.is_none());
    assert!(pkt.state.altitude_m.is_none());
    assert!(pkt.controls.pitch.is_none());
}

#[test]
fn ignores_unknown_fields() {
    let payload = r#"{
        "schema":"flight.ac7.telemetry/1",
        "unknown_field":"ignored",
        "state":{"extra":42}
    }"#;
    // serde default behavior: unknown fields are ignored
    let pkt = Ac7TelemetryPacket::from_json_str(payload).unwrap();
    assert_eq!(pkt.schema, AC7_TELEMETRY_SCHEMA_V1);
}

#[test]
fn from_json_slice_and_str_equivalent() {
    let json = r#"{"schema":"flight.ac7.telemetry/1","aircraft":"F-22A","state":{"altitude_m":5000.0}}"#;
    let from_str = Ac7TelemetryPacket::from_json_str(json).unwrap();
    let from_slice = Ac7TelemetryPacket::from_json_slice(json.as_bytes()).unwrap();
    assert_eq!(from_str, from_slice);
}

#[test]
fn large_timestamp_accepted() {
    let payload = r#"{"schema":"flight.ac7.telemetry/1","timestamp_ms":18446744073709551615}"#;
    let pkt = Ac7TelemetryPacket::from_json_str(payload).unwrap();
    assert_eq!(pkt.timestamp_ms, u64::MAX);
}

// ---------------------------------------------------------------------------
// Malformed / invalid JSON
// ---------------------------------------------------------------------------

#[test]
fn rejects_empty_string() {
    let err = Ac7TelemetryPacket::from_json_str("").unwrap_err();
    assert!(matches!(err, Ac7ProtocolError::InvalidJson(_)));
}

#[test]
fn rejects_empty_bytes() {
    let err = Ac7TelemetryPacket::from_json_slice(b"").unwrap_err();
    assert!(matches!(err, Ac7ProtocolError::InvalidJson(_)));
}

#[test]
fn rejects_bare_string() {
    let err = Ac7TelemetryPacket::from_json_str(r#""hello""#).unwrap_err();
    assert!(matches!(err, Ac7ProtocolError::InvalidJson(_)));
}

#[test]
fn rejects_bare_array() {
    let err = Ac7TelemetryPacket::from_json_str("[1,2,3]").unwrap_err();
    assert!(matches!(err, Ac7ProtocolError::InvalidJson(_)));
}

#[test]
fn rejects_truncated_json() {
    let err = Ac7TelemetryPacket::from_json_str(r#"{"schema":"flight.ac7.telemetry/1""#)
        .unwrap_err();
    assert!(matches!(err, Ac7ProtocolError::InvalidJson(_)));
}

#[test]
fn rejects_wrong_type_for_timestamp() {
    let err =
        Ac7TelemetryPacket::from_json_str(r#"{"schema":"flight.ac7.telemetry/1","timestamp_ms":"not_a_number"}"#)
            .unwrap_err();
    assert!(matches!(err, Ac7ProtocolError::InvalidJson(_)));
}

#[test]
fn rejects_negative_timestamp() {
    let err =
        Ac7TelemetryPacket::from_json_str(r#"{"schema":"flight.ac7.telemetry/1","timestamp_ms":-1}"#)
            .unwrap_err();
    assert!(matches!(err, Ac7ProtocolError::InvalidJson(_)));
}

// ---------------------------------------------------------------------------
// Schema validation
// ---------------------------------------------------------------------------

#[test]
fn rejects_empty_schema() {
    let payload = r#"{"schema":""}"#;
    let err = Ac7TelemetryPacket::from_json_str(payload).unwrap_err();
    assert!(matches!(err, Ac7ProtocolError::UnsupportedSchema { .. }));
}

#[test]
fn rejects_future_schema_version() {
    let payload = r#"{"schema":"flight.ac7.telemetry/2"}"#;
    let err = Ac7TelemetryPacket::from_json_str(payload).unwrap_err();
    match err {
        Ac7ProtocolError::UnsupportedSchema { schema } => {
            assert_eq!(schema, "flight.ac7.telemetry/2");
        }
        other => panic!("expected UnsupportedSchema, got {other:?}"),
    }
}

#[test]
fn rejects_schema_with_trailing_whitespace() {
    let payload = r#"{"schema":"flight.ac7.telemetry/1 "}"#;
    let err = Ac7TelemetryPacket::from_json_str(payload).unwrap_err();
    assert!(matches!(err, Ac7ProtocolError::UnsupportedSchema { .. }));
}

#[test]
fn rejects_case_variant_schema() {
    let payload = r#"{"schema":"Flight.AC7.Telemetry/1"}"#;
    let err = Ac7TelemetryPacket::from_json_str(payload).unwrap_err();
    assert!(matches!(err, Ac7ProtocolError::UnsupportedSchema { .. }));
}

// ---------------------------------------------------------------------------
// State field boundary validation
// ---------------------------------------------------------------------------

#[test]
fn accepts_boundary_altitude_values() {
    for alt in [-2000.0_f32, 0.0, 100_000.0] {
        let pkt = Ac7TelemetryPacket {
            state: Ac7State {
                altitude_m: Some(alt),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(pkt.validate().is_ok(), "altitude {alt} should be valid");
    }
}

#[test]
fn rejects_altitude_just_beyond_bounds() {
    for alt in [-2000.1_f32, 100_000.1] {
        let pkt = Ac7TelemetryPacket {
            state: Ac7State {
                altitude_m: Some(alt),
                ..Default::default()
            },
            ..Default::default()
        };
        let err = pkt.validate().unwrap_err();
        assert!(
            matches!(err, Ac7ProtocolError::OutOfRange { field: "state.altitude_m", .. }),
            "altitude {alt} should be rejected"
        );
    }
}

#[test]
fn accepts_boundary_speed_values() {
    for speed in [0.0_f32, 1250.0, 2500.0] {
        let pkt = Ac7TelemetryPacket {
            state: Ac7State {
                speed_mps: Some(speed),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(pkt.validate().is_ok(), "speed {speed} should be valid");
    }
}

#[test]
fn rejects_speed_beyond_bounds() {
    let pkt = Ac7TelemetryPacket {
        state: Ac7State {
            speed_mps: Some(2500.1),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(matches!(
        pkt.validate().unwrap_err(),
        Ac7ProtocolError::OutOfRange { field: "state.speed_mps", .. }
    ));
}

#[test]
fn rejects_negative_ground_speed() {
    let pkt = Ac7TelemetryPacket {
        state: Ac7State {
            ground_speed_mps: Some(-0.1),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(matches!(
        pkt.validate().unwrap_err(),
        Ac7ProtocolError::OutOfRange { field: "state.ground_speed_mps", .. }
    ));
}

#[test]
fn accepts_boundary_vertical_speed() {
    for vs in [-500.0_f32, 0.0, 500.0] {
        let pkt = Ac7TelemetryPacket {
            state: Ac7State {
                vertical_speed_mps: Some(vs),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(pkt.validate().is_ok(), "vertical speed {vs} should be valid");
    }
}

#[test]
fn rejects_vertical_speed_beyond_bounds() {
    let pkt = Ac7TelemetryPacket {
        state: Ac7State {
            vertical_speed_mps: Some(501.0),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(matches!(
        pkt.validate().unwrap_err(),
        Ac7ProtocolError::OutOfRange { field: "state.vertical_speed_mps", .. }
    ));
}

#[test]
fn accepts_boundary_heading() {
    for h in [-360.0_f32, 0.0, 360.0] {
        let pkt = Ac7TelemetryPacket {
            state: Ac7State {
                heading_deg: Some(h),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(pkt.validate().is_ok(), "heading {h} should be valid");
    }
}

#[test]
fn rejects_heading_beyond_bounds() {
    let pkt = Ac7TelemetryPacket {
        state: Ac7State {
            heading_deg: Some(361.0),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(matches!(
        pkt.validate().unwrap_err(),
        Ac7ProtocolError::OutOfRange { field: "state.heading_deg", .. }
    ));
}

#[test]
fn accepts_boundary_pitch_roll() {
    for deg in [-180.0_f32, 0.0, 180.0] {
        let pkt = Ac7TelemetryPacket {
            state: Ac7State {
                pitch_deg: Some(deg),
                roll_deg: Some(deg),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(pkt.validate().is_ok(), "pitch/roll {deg} should be valid");
    }
}

#[test]
fn rejects_pitch_beyond_bounds() {
    let pkt = Ac7TelemetryPacket {
        state: Ac7State {
            pitch_deg: Some(-181.0),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(matches!(
        pkt.validate().unwrap_err(),
        Ac7ProtocolError::OutOfRange { field: "state.pitch_deg", .. }
    ));
}

#[test]
fn rejects_roll_beyond_bounds() {
    let pkt = Ac7TelemetryPacket {
        state: Ac7State {
            roll_deg: Some(180.5),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(matches!(
        pkt.validate().unwrap_err(),
        Ac7ProtocolError::OutOfRange { field: "state.roll_deg", .. }
    ));
}

#[test]
fn accepts_boundary_g_force() {
    for g in [-20.0_f32, 0.0, 20.0] {
        let pkt = Ac7TelemetryPacket {
            state: Ac7State {
                g_force: Some(g),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(pkt.validate().is_ok(), "g_force {g} should be valid");
    }
}

#[test]
fn rejects_g_force_beyond_bounds() {
    let pkt = Ac7TelemetryPacket {
        state: Ac7State {
            g_force: Some(20.1),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(matches!(
        pkt.validate().unwrap_err(),
        Ac7ProtocolError::OutOfRange { field: "state.g_force", .. }
    ));
}

#[test]
fn accepts_boundary_health_norm() {
    for h in [0.0_f32, 0.5, 1.0] {
        let pkt = Ac7TelemetryPacket {
            state: Ac7State {
                health_norm: Some(h),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(pkt.validate().is_ok(), "health_norm {h} should be valid");
    }
}

#[test]
fn rejects_health_norm_negative() {
    let pkt = Ac7TelemetryPacket {
        state: Ac7State {
            health_norm: Some(-0.01),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(matches!(
        pkt.validate().unwrap_err(),
        Ac7ProtocolError::OutOfRange { field: "state.health_norm", .. }
    ));
}

#[test]
fn rejects_health_norm_above_one() {
    let pkt = Ac7TelemetryPacket {
        state: Ac7State {
            health_norm: Some(1.01),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(matches!(
        pkt.validate().unwrap_err(),
        Ac7ProtocolError::OutOfRange { field: "state.health_norm", .. }
    ));
}

// ---------------------------------------------------------------------------
// Controls field boundary validation
// ---------------------------------------------------------------------------

#[test]
fn accepts_boundary_control_inputs() {
    let pkt = Ac7TelemetryPacket {
        controls: Ac7Controls {
            pitch: Some(-1.0),
            roll: Some(1.0),
            yaw: Some(0.0),
            throttle: Some(0.0),
            brake: Some(1.0),
        },
        ..Default::default()
    };
    assert!(pkt.validate().is_ok());
}

#[test]
fn rejects_pitch_below_neg_one() {
    let pkt = Ac7TelemetryPacket {
        controls: Ac7Controls {
            pitch: Some(-1.01),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(matches!(
        pkt.validate().unwrap_err(),
        Ac7ProtocolError::OutOfRange { field: "controls.pitch", .. }
    ));
}

#[test]
fn rejects_roll_above_one() {
    let pkt = Ac7TelemetryPacket {
        controls: Ac7Controls {
            roll: Some(1.01),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(matches!(
        pkt.validate().unwrap_err(),
        Ac7ProtocolError::OutOfRange { field: "controls.roll", .. }
    ));
}

#[test]
fn rejects_yaw_out_of_range() {
    let pkt = Ac7TelemetryPacket {
        controls: Ac7Controls {
            yaw: Some(1.5),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(matches!(
        pkt.validate().unwrap_err(),
        Ac7ProtocolError::OutOfRange { field: "controls.yaw", .. }
    ));
}

#[test]
fn rejects_brake_negative() {
    let pkt = Ac7TelemetryPacket {
        controls: Ac7Controls {
            brake: Some(-0.1),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(matches!(
        pkt.validate().unwrap_err(),
        Ac7ProtocolError::OutOfRange { field: "controls.brake", .. }
    ));
}

#[test]
fn rejects_throttle_above_one() {
    let pkt = Ac7TelemetryPacket {
        controls: Ac7Controls {
            throttle: Some(1.001),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(matches!(
        pkt.validate().unwrap_err(),
        Ac7ProtocolError::OutOfRange { field: "controls.throttle", .. }
    ));
}

// ---------------------------------------------------------------------------
// NaN / Infinity handling
// ---------------------------------------------------------------------------

#[test]
fn rejects_nan_altitude() {
    let pkt = Ac7TelemetryPacket {
        state: Ac7State {
            altitude_m: Some(f32::NAN),
            ..Default::default()
        },
        ..Default::default()
    };
    // NaN is not contained in any range
    assert!(pkt.validate().is_err());
}

#[test]
fn rejects_positive_infinity_speed() {
    let pkt = Ac7TelemetryPacket {
        state: Ac7State {
            speed_mps: Some(f32::INFINITY),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(pkt.validate().is_err());
}

#[test]
fn rejects_negative_infinity_pitch_control() {
    let pkt = Ac7TelemetryPacket {
        controls: Ac7Controls {
            pitch: Some(f32::NEG_INFINITY),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(pkt.validate().is_err());
}

// ---------------------------------------------------------------------------
// Validation reports first failing field
// ---------------------------------------------------------------------------

#[test]
fn validation_reports_first_failing_state_field() {
    // altitude is validated before speed, so altitude error should surface
    let pkt = Ac7TelemetryPacket {
        state: Ac7State {
            altitude_m: Some(200_000.0),
            speed_mps: Some(-10.0),
            ..Default::default()
        },
        ..Default::default()
    };
    match pkt.validate().unwrap_err() {
        Ac7ProtocolError::OutOfRange { field, .. } => {
            assert_eq!(field, "state.altitude_m");
        }
        other => panic!("expected OutOfRange for altitude, got {other:?}"),
    }
}

#[test]
fn validation_reports_state_before_controls() {
    let pkt = Ac7TelemetryPacket {
        state: Ac7State {
            g_force: Some(25.0),
            ..Default::default()
        },
        controls: Ac7Controls {
            throttle: Some(5.0),
            ..Default::default()
        },
        ..Default::default()
    };
    match pkt.validate().unwrap_err() {
        Ac7ProtocolError::OutOfRange { field, .. } => {
            assert_eq!(field, "state.g_force");
        }
        other => panic!("expected OutOfRange for g_force, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Aircraft label
// ---------------------------------------------------------------------------

#[test]
fn aircraft_label_returns_default_for_empty() {
    let pkt = Ac7TelemetryPacket::default();
    assert_eq!(pkt.aircraft_label(), "AC7");
}

#[test]
fn aircraft_label_returns_default_for_whitespace_only() {
    let pkt = Ac7TelemetryPacket {
        aircraft: "   \t  ".to_string(),
        ..Default::default()
    };
    assert_eq!(pkt.aircraft_label(), "AC7");
}

#[test]
fn aircraft_label_trims_and_returns() {
    let pkt = Ac7TelemetryPacket {
        aircraft: "  F/A-18E  ".to_string(),
        ..Default::default()
    };
    assert_eq!(pkt.aircraft_label(), "F/A-18E");
}

#[test]
fn aircraft_label_preserves_unicode() {
    let pkt = Ac7TelemetryPacket {
        aircraft: "震電II".to_string(),
        ..Default::default()
    };
    assert_eq!(pkt.aircraft_label(), "震電II");
}

// ---------------------------------------------------------------------------
// JSON round-trip serialization
// ---------------------------------------------------------------------------

#[test]
fn round_trip_default_packet() {
    let original = Ac7TelemetryPacket::default();
    let bytes = original.to_json_vec().unwrap();
    let restored = Ac7TelemetryPacket::from_json_slice(&bytes).unwrap();
    assert_eq!(original, restored);
}

#[test]
fn round_trip_all_none_optionals() {
    let original = Ac7TelemetryPacket {
        schema: AC7_TELEMETRY_SCHEMA_V1.to_string(),
        timestamp_ms: 42,
        aircraft: "Test".to_string(),
        mission: None,
        state: Ac7State::default(),
        controls: Ac7Controls::default(),
    };
    let bytes = original.to_json_vec().unwrap();
    let restored = Ac7TelemetryPacket::from_json_slice(&bytes).unwrap();
    assert_eq!(original, restored);
}

#[test]
fn round_trip_all_fields_populated() {
    let original = Ac7TelemetryPacket {
        schema: AC7_TELEMETRY_SCHEMA_V1.to_string(),
        timestamp_ms: 123456789,
        aircraft: "ADF-11F".to_string(),
        mission: Some("FinalMission".to_string()),
        state: Ac7State {
            altitude_m: Some(99_999.0),
            speed_mps: Some(2499.0),
            ground_speed_mps: Some(2400.0),
            vertical_speed_mps: Some(-499.0),
            heading_deg: Some(-359.0),
            pitch_deg: Some(-179.0),
            roll_deg: Some(179.0),
            g_force: Some(-19.0),
            health_norm: Some(0.01),
        },
        controls: Ac7Controls {
            pitch: Some(-0.99),
            roll: Some(0.99),
            yaw: Some(-0.5),
            throttle: Some(0.5),
            brake: Some(0.5),
        },
    };
    let bytes = original.to_json_vec().unwrap();
    let restored = Ac7TelemetryPacket::from_json_slice(&bytes).unwrap();
    assert_eq!(original, restored);
}

#[test]
fn to_json_vec_produces_valid_utf8() {
    let pkt = Ac7TelemetryPacket {
        aircraft: "MiG-31BM «Foxhound»".to_string(),
        ..Default::default()
    };
    let bytes = pkt.to_json_vec().unwrap();
    let text = std::str::from_utf8(&bytes).expect("should be valid UTF-8");
    assert!(text.contains("MiG-31BM"));
}

// ---------------------------------------------------------------------------
// Error Display formatting
// ---------------------------------------------------------------------------

#[test]
fn error_display_invalid_json() {
    let err = Ac7TelemetryPacket::from_json_str("}{").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("invalid telemetry JSON"), "got: {msg}");
}

#[test]
fn error_display_unsupported_schema() {
    let err = Ac7TelemetryPacket::from_json_str(r#"{"schema":"bad"}"#).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("unsupported schema: bad"), "got: {msg}");
}

#[test]
fn error_display_out_of_range() {
    let pkt = Ac7TelemetryPacket {
        state: Ac7State {
            altitude_m: Some(200_000.0),
            ..Default::default()
        },
        ..Default::default()
    };
    let msg = pkt.validate().unwrap_err().to_string();
    assert!(msg.contains("state.altitude_m"), "got: {msg}");
    assert!(msg.contains("200000"), "got: {msg}");
}

// ---------------------------------------------------------------------------
// Default packet validity
// ---------------------------------------------------------------------------

#[test]
fn default_packet_validates_successfully() {
    let pkt = Ac7TelemetryPacket::default();
    assert!(pkt.validate().is_ok());
}

#[test]
fn default_state_all_none() {
    let state = Ac7State::default();
    assert!(state.altitude_m.is_none());
    assert!(state.speed_mps.is_none());
    assert!(state.ground_speed_mps.is_none());
    assert!(state.vertical_speed_mps.is_none());
    assert!(state.heading_deg.is_none());
    assert!(state.pitch_deg.is_none());
    assert!(state.roll_deg.is_none());
    assert!(state.g_force.is_none());
    assert!(state.health_norm.is_none());
}

#[test]
fn default_controls_all_none() {
    let controls = Ac7Controls::default();
    assert!(controls.pitch.is_none());
    assert!(controls.roll.is_none());
    assert!(controls.yaw.is_none());
    assert!(controls.throttle.is_none());
    assert!(controls.brake.is_none());
}

// ---------------------------------------------------------------------------
// Schema constant
// ---------------------------------------------------------------------------

#[test]
fn schema_constant_matches_expected_format() {
    assert_eq!(AC7_TELEMETRY_SCHEMA_V1, "flight.ac7.telemetry/1");
    // Ensure it contains the version discriminator
    assert!(AC7_TELEMETRY_SCHEMA_V1.ends_with("/1"));
}

// ---------------------------------------------------------------------------
// Clone / PartialEq derived trait coverage
// ---------------------------------------------------------------------------

#[test]
fn packet_clone_equals_original() {
    let original = Ac7TelemetryPacket {
        aircraft: "F-14D".to_string(),
        timestamp_ms: 5000,
        state: Ac7State {
            altitude_m: Some(3000.0),
            ..Default::default()
        },
        ..Default::default()
    };
    let cloned = original.clone();
    assert_eq!(original, cloned);
}

#[test]
fn packets_with_different_aircraft_not_equal() {
    let a = Ac7TelemetryPacket {
        aircraft: "F-15C".to_string(),
        ..Default::default()
    };
    let b = Ac7TelemetryPacket {
        aircraft: "F-15E".to_string(),
        ..Default::default()
    };
    assert_ne!(a, b);
}

// ---------------------------------------------------------------------------
// JSON edge cases from wire
// ---------------------------------------------------------------------------

#[test]
fn accepts_integer_for_float_field() {
    // JSON `100` should deserialize as f32 `100.0`
    let payload = r#"{"schema":"flight.ac7.telemetry/1","state":{"altitude_m":100}}"#;
    let pkt = Ac7TelemetryPacket::from_json_str(payload).unwrap();
    assert_eq!(pkt.state.altitude_m, Some(100.0));
}

#[test]
fn accepts_zero_controls() {
    let payload = r#"{
        "schema":"flight.ac7.telemetry/1",
        "controls":{"pitch":0,"roll":0,"yaw":0,"throttle":0,"brake":0}
    }"#;
    let pkt = Ac7TelemetryPacket::from_json_str(payload).unwrap();
    assert_eq!(pkt.controls.pitch, Some(0.0));
    assert_eq!(pkt.controls.throttle, Some(0.0));
}

#[test]
fn parses_packet_with_empty_state_and_controls() {
    let payload = r#"{"schema":"flight.ac7.telemetry/1","state":{},"controls":{}}"#;
    let pkt = Ac7TelemetryPacket::from_json_str(payload).unwrap();
    assert!(pkt.state.altitude_m.is_none());
    assert!(pkt.controls.pitch.is_none());
}
