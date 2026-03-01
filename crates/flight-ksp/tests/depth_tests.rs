// SPDX-License-Identifier: MIT OR Apache-2.0
//! Depth tests for the flight-ksp crate.
//!
//! Covers protocol encoding/decoding, telemetry conversion, controls validation,
//! error handling, state machine transitions, and property-based tests.

use flight_bus::{
    snapshot::BusSnapshot,
    types::{AircraftId, SimId},
};
use flight_ksp::{
    KspAdapter, KspConfig, KspControls, KspError,
    protocol::{
        Argument, ConnectionRequest, ConnectionResponse, KrpcError, ProcedureCall,
        ProcedureResult, Request, Response, connection_request, connection_response,
        decode_bool, decode_double, decode_float, decode_int32, decode_object, decode_string,
        encode_bool, encode_float, encode_object,
    },
};
use flight_ksp::mapping::{KspRawTelemetry, apply_telemetry, situation};
use prost::Message;
use std::time::Duration;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn default_snapshot() -> BusSnapshot {
    BusSnapshot::new(SimId::Ksp, AircraftId::new("test"))
}

fn flying_telemetry() -> KspRawTelemetry {
    KspRawTelemetry {
        vessel_name: "TestCraft".to_string(),
        situation: situation::FLYING,
        pitch_deg: 5.0,
        roll_deg: -10.0,
        heading_deg: 90.0,
        speed_mps: 200.0,
        ias_mps: 180.0,
        vertical_speed_mps: 5.0,
        g_force: 1.2,
        altitude_m: 5000.0,
        latitude_deg: -0.097,
        longitude_deg: -74.558,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// §1  PROTOCOL: Encoding / decoding round-trips
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn encode_decode_object_max_u64() {
    let encoded = encode_object(u64::MAX);
    assert_eq!(decode_object(&encoded).unwrap(), u64::MAX);
}

#[test]
fn encode_decode_object_min_nonzero() {
    let encoded = encode_object(1);
    assert_eq!(decode_object(&encoded).unwrap(), 1);
}

#[test]
fn encode_decode_float_zero() {
    let encoded = encode_float(0.0);
    let decoded = decode_float(&encoded).unwrap();
    assert!((decoded).abs() < f32::EPSILON);
}

#[test]
fn encode_decode_float_negative_max() {
    let v = f32::MIN;
    let encoded = encode_float(v);
    let decoded = decode_float(&encoded).unwrap();
    assert_eq!(decoded, v);
}

#[test]
fn encode_decode_float_positive_max() {
    let v = f32::MAX;
    let encoded = encode_float(v);
    let decoded = decode_float(&encoded).unwrap();
    assert_eq!(decoded, v);
}

#[test]
fn encode_decode_float_subnormal() {
    let v = f32::MIN_POSITIVE / 2.0;
    let encoded = encode_float(v);
    let decoded = decode_float(&encoded).unwrap();
    assert_eq!(decoded, v);
}

#[test]
fn encode_decode_bool_roundtrip_true() {
    assert!(decode_bool(&encode_bool(true)).unwrap());
}

#[test]
fn encode_decode_bool_roundtrip_false() {
    assert!(!decode_bool(&encode_bool(false)).unwrap());
}

#[test]
fn decode_double_from_empty_bytes_returns_zero() {
    // Empty protobuf message decodes with default values (0.0 for double)
    let decoded = decode_double(&[]).unwrap();
    assert!((decoded).abs() < f64::EPSILON);
}

#[test]
fn decode_float_from_empty_bytes_returns_zero() {
    let decoded = decode_float(&[]).unwrap();
    assert!((decoded).abs() < f32::EPSILON);
}

#[test]
fn decode_object_from_empty_bytes_returns_zero() {
    let decoded = decode_object(&[]).unwrap();
    assert_eq!(decoded, 0);
}

#[test]
fn decode_bool_from_empty_bytes_returns_false() {
    let decoded = decode_bool(&[]).unwrap();
    assert!(!decoded);
}

#[test]
fn decode_string_from_empty_bytes_returns_empty() {
    let decoded = decode_string(&[]).unwrap();
    assert!(decoded.is_empty());
}

#[test]
fn decode_int32_from_empty_bytes_returns_zero() {
    let decoded = decode_int32(&[]).unwrap();
    assert_eq!(decoded, 0);
}

// ── Protocol: malformed input ────────────────────────────────────────────────

#[test]
fn decode_double_garbage_does_not_panic() {
    let garbage = [0xFF, 0xFE, 0xFD, 0xFC, 0xFB, 0xFA, 0xF9, 0xF8, 0xF7];
    // Should either succeed or return an error, but never panic
    let _ = decode_double(&garbage);
}

#[test]
fn decode_float_garbage_does_not_panic() {
    let garbage = [0xFF, 0xFE, 0xFD, 0xFC, 0xFB];
    let _ = decode_float(&garbage);
}

#[test]
fn decode_object_garbage_does_not_panic() {
    let garbage = [0xFF; 20];
    let _ = decode_object(&garbage);
}

#[test]
fn decode_string_invalid_utf8_graceful() {
    // prost strings must be valid UTF-8; invalid should yield decode error
    // Tag 1 (field 1, length-delimited), length 4, then invalid UTF-8
    let bad = [0x0A, 0x04, 0xFF, 0xFE, 0x80, 0x81];
    let result = decode_string(&bad);
    assert!(result.is_err());
}

// ── Protocol: protobuf message construction ──────────────────────────────────

#[test]
fn connection_request_rpc_serializes() {
    let req = ConnectionRequest {
        r#type: connection_request::Type::Rpc as i32,
        name: "OpenFlight".to_string(),
        client_identifier: vec![],
    };
    let bytes = req.encode_to_vec();
    let decoded = ConnectionRequest::decode(bytes.as_slice()).unwrap();
    assert_eq!(decoded.name, "OpenFlight");
    assert_eq!(decoded.r#type, connection_request::Type::Rpc as i32);
}

#[test]
fn connection_request_stream_type() {
    let req = ConnectionRequest {
        r#type: connection_request::Type::Stream as i32,
        name: "stream-client".to_string(),
        client_identifier: vec![1, 2, 3, 4],
    };
    let bytes = req.encode_to_vec();
    let decoded = ConnectionRequest::decode(bytes.as_slice()).unwrap();
    assert_eq!(decoded.r#type, connection_request::Type::Stream as i32);
    assert_eq!(decoded.client_identifier, vec![1, 2, 3, 4]);
}

#[test]
fn connection_response_ok_roundtrip() {
    let resp = ConnectionResponse {
        status: connection_response::Status::Ok as i32,
        message: String::new(),
        client_identifier: vec![0xAB, 0xCD],
    };
    let bytes = resp.encode_to_vec();
    let decoded = ConnectionResponse::decode(bytes.as_slice()).unwrap();
    assert_eq!(decoded.status, connection_response::Status::Ok as i32);
    assert_eq!(decoded.client_identifier, vec![0xAB, 0xCD]);
}

#[test]
fn connection_response_all_status_variants() {
    for (status, expected) in [
        (connection_response::Status::Ok, 0),
        (connection_response::Status::MalformedMessage, 1),
        (connection_response::Status::Timeout, 2),
        (connection_response::Status::WrongType, 3),
    ] {
        assert_eq!(status as i32, expected);
    }
}

#[test]
fn request_with_multiple_calls_roundtrip() {
    let req = Request {
        calls: vec![
            ProcedureCall {
                service: "SpaceCenter".to_string(),
                procedure: "get_ActiveVessel".to_string(),
                service_id: 0,
                procedure_id: 0,
                arguments: vec![],
            },
            ProcedureCall {
                service: "SpaceCenter".to_string(),
                procedure: "Vessel_get_Name".to_string(),
                service_id: 0,
                procedure_id: 0,
                arguments: vec![Argument {
                    position: 0,
                    value: encode_object(42),
                }],
            },
        ],
    };
    let bytes = req.encode_to_vec();
    let decoded = Request::decode(bytes.as_slice()).unwrap();
    assert_eq!(decoded.calls.len(), 2);
    assert_eq!(decoded.calls[0].procedure, "get_ActiveVessel");
    assert_eq!(decoded.calls[1].arguments.len(), 1);
    assert_eq!(decoded.calls[1].arguments[0].position, 0);
}

#[test]
fn response_with_error_result() {
    let resp = Response {
        time: 123.456,
        results: vec![ProcedureResult {
            error: Some(KrpcError {
                service: "SpaceCenter".to_string(),
                name: "InvalidOperationException".to_string(),
                description: "No active vessel".to_string(),
                stack_trace: "at KSP.SpaceCenter".to_string(),
            }),
            value: vec![],
        }],
    };
    let bytes = resp.encode_to_vec();
    let decoded = Response::decode(bytes.as_slice()).unwrap();
    assert_eq!(decoded.results.len(), 1);
    let err = decoded.results[0].error.as_ref().unwrap();
    assert_eq!(err.name, "InvalidOperationException");
    assert_eq!(err.description, "No active vessel");
}

#[test]
fn response_with_mixed_results() {
    let resp = Response {
        time: 0.0,
        results: vec![
            ProcedureResult {
                error: None,
                value: encode_object(99),
            },
            ProcedureResult {
                error: Some(KrpcError {
                    service: "SpaceCenter".to_string(),
                    name: "err".to_string(),
                    description: "fail".to_string(),
                    stack_trace: String::new(),
                }),
                value: vec![],
            },
        ],
    };
    let bytes = resp.encode_to_vec();
    let decoded = Response::decode(bytes.as_slice()).unwrap();
    assert!(decoded.results[0].error.is_none());
    assert!(decoded.results[1].error.is_some());
}

#[test]
fn procedure_call_with_service_and_procedure_ids() {
    let call = ProcedureCall {
        service: String::new(),
        procedure: String::new(),
        service_id: 5,
        procedure_id: 42,
        arguments: vec![],
    };
    let bytes = call.encode_to_vec();
    let decoded = ProcedureCall::decode(bytes.as_slice()).unwrap();
    assert_eq!(decoded.service_id, 5);
    assert_eq!(decoded.procedure_id, 42);
}

#[test]
fn argument_preserves_position_and_value() {
    let arg = Argument {
        position: 3,
        value: vec![0xDE, 0xAD],
    };
    let bytes = arg.encode_to_vec();
    let decoded = Argument::decode(bytes.as_slice()).unwrap();
    assert_eq!(decoded.position, 3);
    assert_eq!(decoded.value, vec![0xDE, 0xAD]);
}

#[test]
fn empty_request_roundtrip() {
    let req = Request { calls: vec![] };
    let bytes = req.encode_to_vec();
    let decoded = Request::decode(bytes.as_slice()).unwrap();
    assert!(decoded.calls.is_empty());
}

#[test]
fn empty_response_roundtrip() {
    let resp = Response {
        time: 0.0,
        results: vec![],
    };
    let bytes = resp.encode_to_vec();
    let decoded = Response::decode(bytes.as_slice()).unwrap();
    assert!(decoded.results.is_empty());
    assert!((decoded.time).abs() < f64::EPSILON);
}

// ═══════════════════════════════════════════════════════════════════════════════
// §2  TELEMETRY CONVERSION: field normalization and axis mapping
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn telemetry_pitch_negative_90() {
    let mut snap = default_snapshot();
    apply_telemetry(
        &mut snap,
        &KspRawTelemetry {
            pitch_deg: -90.0,
            situation: situation::FLYING,
            ..Default::default()
        },
    );
    assert!((snap.kinematics.pitch.to_degrees() - (-90.0)).abs() < 0.01);
}

#[test]
fn telemetry_roll_full_range() {
    for roll in [-180.0f32, -90.0, 0.0, 90.0, 179.9] {
        let mut snap = default_snapshot();
        apply_telemetry(
            &mut snap,
            &KspRawTelemetry {
                roll_deg: roll,
                situation: situation::FLYING,
                ..Default::default()
            },
        );
        assert!(
            (snap.kinematics.bank.to_degrees() - roll).abs() < 0.1,
            "roll={roll} mapped to {}",
            snap.kinematics.bank.to_degrees()
        );
    }
}

#[test]
fn telemetry_heading_zero_stays_zero() {
    let mut snap = default_snapshot();
    apply_telemetry(
        &mut snap,
        &KspRawTelemetry {
            heading_deg: 0.0,
            situation: situation::FLYING,
            ..Default::default()
        },
    );
    assert!((snap.kinematics.heading.to_degrees()).abs() < 0.01);
}

#[test]
fn telemetry_heading_180_stays_180() {
    let mut snap = default_snapshot();
    apply_telemetry(
        &mut snap,
        &KspRawTelemetry {
            heading_deg: 180.0,
            situation: situation::FLYING,
            ..Default::default()
        },
    );
    assert!((snap.kinematics.heading.to_degrees() - 180.0).abs() < 0.01);
}

#[test]
fn telemetry_heading_359_becomes_negative_1() {
    let mut snap = default_snapshot();
    apply_telemetry(
        &mut snap,
        &KspRawTelemetry {
            heading_deg: 359.0,
            situation: situation::FLYING,
            ..Default::default()
        },
    );
    assert!(
        (snap.kinematics.heading.to_degrees() - (-1.0)).abs() < 0.01,
        "heading 359° should normalize to -1°, got {}",
        snap.kinematics.heading.to_degrees()
    );
}

#[test]
fn telemetry_speed_zero() {
    let mut snap = default_snapshot();
    apply_telemetry(
        &mut snap,
        &KspRawTelemetry {
            speed_mps: 0.0,
            ias_mps: 0.0,
            situation: situation::FLYING,
            ..Default::default()
        },
    );
    assert!(snap.kinematics.tas.to_knots() < 0.01);
    assert!(snap.kinematics.ias.to_knots() < 0.01);
}

#[test]
fn telemetry_speed_clamped_at_1000_knots() {
    let mut snap = default_snapshot();
    // 600 m/s ≈ 1166 knots → should be clamped to 1000
    apply_telemetry(
        &mut snap,
        &KspRawTelemetry {
            speed_mps: 600.0,
            situation: situation::FLYING,
            ..Default::default()
        },
    );
    assert!(
        (snap.kinematics.tas.to_knots() - 1000.0).abs() < 0.1,
        "TAS should be clamped to 1000 kt, got {}",
        snap.kinematics.tas.to_knots()
    );
}

#[test]
fn telemetry_vertical_speed_negative() {
    let mut snap = default_snapshot();
    apply_telemetry(
        &mut snap,
        &KspRawTelemetry {
            vertical_speed_mps: -20.0,
            situation: situation::FLYING,
            ..Default::default()
        },
    );
    // -20 m/s × 196.85 = -3937 fpm
    assert!(
        (snap.kinematics.vertical_speed - (-3937.0)).abs() < 1.0,
        "got {}",
        snap.kinematics.vertical_speed
    );
}

#[test]
fn telemetry_g_force_exact_boundary_values() {
    for g in [-20.0, -1.0, 0.0, 1.0, 20.0] {
        let mut snap = default_snapshot();
        apply_telemetry(
            &mut snap,
            &KspRawTelemetry {
                g_force: g,
                situation: situation::FLYING,
                ..Default::default()
            },
        );
        assert!(
            (snap.kinematics.g_force.value() - g as f32).abs() < 0.01,
            "g_force={g} mapped to {}",
            snap.kinematics.g_force.value()
        );
    }
}

#[test]
fn telemetry_negative_altitude() {
    let mut snap = default_snapshot();
    apply_telemetry(
        &mut snap,
        &KspRawTelemetry {
            altitude_m: -100.0,
            situation: situation::SPLASHED,
            ..Default::default()
        },
    );
    // -100 m × 3.28084 = -328.08 ft
    assert!(
        (snap.environment.altitude - (-328.08)).abs() < 1.0,
        "got {}",
        snap.environment.altitude
    );
}

#[test]
fn telemetry_latitude_longitude_passed_through() {
    let mut snap = default_snapshot();
    apply_telemetry(
        &mut snap,
        &KspRawTelemetry {
            latitude_deg: 28.608,
            longitude_deg: -80.604,
            situation: situation::FLYING,
            ..Default::default()
        },
    );
    assert!((snap.navigation.latitude - 28.608).abs() < 0.001);
    assert!((snap.navigation.longitude - (-80.604)).abs() < 0.001);
}

#[test]
fn telemetry_extreme_latitude_longitude() {
    let mut snap = default_snapshot();
    apply_telemetry(
        &mut snap,
        &KspRawTelemetry {
            latitude_deg: 90.0,
            longitude_deg: -180.0,
            situation: situation::FLYING,
            ..Default::default()
        },
    );
    assert!((snap.navigation.latitude - 90.0).abs() < 0.001);
    assert!((snap.navigation.longitude - (-180.0)).abs() < 0.001);
}

#[test]
fn telemetry_sim_id_always_ksp() {
    let mut snap = default_snapshot();
    apply_telemetry(&mut snap, &flying_telemetry());
    assert_eq!(snap.sim, SimId::Ksp);
}

#[test]
fn telemetry_nan_pitch_is_skipped() {
    let mut snap = default_snapshot();
    // First set a known pitch
    apply_telemetry(
        &mut snap,
        &KspRawTelemetry {
            pitch_deg: 15.0,
            situation: situation::FLYING,
            ..Default::default()
        },
    );
    let prev_pitch = snap.kinematics.pitch.to_degrees();
    // NaN should fail ValidatedAngle::new_degrees and be skipped
    apply_telemetry(
        &mut snap,
        &KspRawTelemetry {
            pitch_deg: f32::NAN,
            situation: situation::FLYING,
            ..Default::default()
        },
    );
    assert!(
        (snap.kinematics.pitch.to_degrees() - prev_pitch).abs() < 0.01,
        "NaN pitch should not overwrite previous value"
    );
}

#[test]
fn telemetry_infinity_speed_is_clamped() {
    let mut snap = default_snapshot();
    apply_telemetry(
        &mut snap,
        &KspRawTelemetry {
            speed_mps: f64::INFINITY,
            situation: situation::FLYING,
            ..Default::default()
        },
    );
    // Infinity cast to f32 → f32::INFINITY, clamped to 1000 knots
    assert!(snap.kinematics.tas.to_knots() <= 1000.0);
}

// ═══════════════════════════════════════════════════════════════════════════════
// §3  SITUATION / VALIDITY FLAGS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn all_situation_constants_are_unique() {
    let situations = [
        situation::LANDED,
        situation::SPLASHED,
        situation::PRELAUNCH,
        situation::FLYING,
        situation::SUB_ORBITAL,
        situation::ORBITING,
        situation::ESCAPING,
        situation::DOCKED,
    ];
    let mut unique = situations.to_vec();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), situations.len(), "situation constants must be unique");
}

#[test]
fn situation_constants_sequential_from_zero() {
    assert_eq!(situation::LANDED, 0);
    assert_eq!(situation::SPLASHED, 1);
    assert_eq!(situation::PRELAUNCH, 2);
    assert_eq!(situation::FLYING, 3);
    assert_eq!(situation::SUB_ORBITAL, 4);
    assert_eq!(situation::ORBITING, 5);
    assert_eq!(situation::ESCAPING, 6);
    assert_eq!(situation::DOCKED, 7);
}

#[test]
fn landed_no_flight_data() {
    let mut snap = default_snapshot();
    apply_telemetry(
        &mut snap,
        &KspRawTelemetry {
            situation: situation::LANDED,
            ..Default::default()
        },
    );
    assert!(!snap.validity.attitude_valid);
    assert!(!snap.validity.velocities_valid);
    assert!(!snap.validity.safe_for_ffb);
    assert!(!snap.validity.aero_valid);
    assert!(!snap.validity.kinematics_valid);
    assert!(snap.validity.position_valid);
}

#[test]
fn splashed_no_flight_data() {
    let mut snap = default_snapshot();
    apply_telemetry(
        &mut snap,
        &KspRawTelemetry {
            situation: situation::SPLASHED,
            ..Default::default()
        },
    );
    assert!(!snap.validity.attitude_valid);
    assert!(!snap.validity.safe_for_ffb);
}

#[test]
fn docked_has_flight_data_no_aero() {
    let mut snap = default_snapshot();
    apply_telemetry(
        &mut snap,
        &KspRawTelemetry {
            situation: situation::DOCKED,
            ..Default::default()
        },
    );
    // DOCKED = 7, >= FLYING = 3, so in_flight = true
    assert!(snap.validity.attitude_valid);
    assert!(snap.validity.velocities_valid);
    // Not FLYING exactly, so not in_atmosphere
    assert!(!snap.validity.safe_for_ffb);
    assert!(!snap.validity.aero_valid);
}

#[test]
fn angular_rates_never_valid() {
    // angular_rates_valid is always false (not yet fetched from kRPC)
    for sit in 0..=7 {
        let mut snap = default_snapshot();
        apply_telemetry(
            &mut snap,
            &KspRawTelemetry {
                situation: sit,
                ..Default::default()
            },
        );
        assert!(!snap.validity.angular_rates_valid);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// §4  CONTROLS: validation and clamping
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn controls_default_is_valid() {
    assert!(KspControls::default().is_valid());
}

#[test]
fn controls_from_axes_boundary_values() {
    let c = KspControls::from_axes(-1.0, 1.0, -1.0, 0.0);
    assert!(c.is_valid());
    let c = KspControls::from_axes(1.0, -1.0, 1.0, 1.0);
    assert!(c.is_valid());
}

#[test]
fn controls_roll_out_of_range_detected() {
    let c = KspControls {
        roll: -1.001,
        ..Default::default()
    };
    assert!(!c.is_valid());
}

#[test]
fn controls_yaw_out_of_range_detected() {
    let c = KspControls {
        yaw: 1.5,
        ..Default::default()
    };
    assert!(!c.is_valid());
}

#[test]
fn controls_clamped_all_axes_extreme() {
    let c = KspControls {
        pitch: 100.0,
        roll: -100.0,
        yaw: 50.0,
        throttle: 999.0,
        gear: Some(false),
    };
    let cl = c.clamped();
    assert_eq!(cl.pitch, 1.0);
    assert_eq!(cl.roll, -1.0);
    assert_eq!(cl.yaw, 1.0);
    assert_eq!(cl.throttle, 1.0);
    assert_eq!(cl.gear, Some(false));
    assert!(cl.is_valid());
}

#[test]
fn controls_clamped_negative_extreme() {
    let c = KspControls {
        pitch: -50.0,
        roll: -50.0,
        yaw: -50.0,
        throttle: -50.0,
        gear: None,
    };
    let cl = c.clamped();
    assert_eq!(cl.pitch, -1.0);
    assert_eq!(cl.roll, -1.0);
    assert_eq!(cl.yaw, -1.0);
    assert_eq!(cl.throttle, 0.0);
    assert!(cl.is_valid());
}

#[test]
fn controls_equality() {
    let a = KspControls::from_axes(0.5, -0.5, 0.25, 0.75);
    let b = KspControls::from_axes(0.5, -0.5, 0.25, 0.75);
    assert_eq!(a, b);
}

#[test]
fn controls_copy_semantics() {
    let a = KspControls::from_axes(0.1, 0.2, 0.3, 0.4);
    let b = a; // Copy
    assert_eq!(a, b);
}

#[test]
fn controls_debug_output() {
    let c = KspControls::default();
    let debug = format!("{c:?}");
    assert!(debug.contains("KspControls"));
    assert!(debug.contains("pitch"));
}

#[test]
fn controls_gear_variants() {
    let mut c = KspControls::default();
    assert!(c.gear.is_none());
    c.gear = Some(true);
    assert_eq!(c.gear, Some(true));
    c.gear = Some(false);
    assert_eq!(c.gear, Some(false));
}

// ═══════════════════════════════════════════════════════════════════════════════
// §5  ERROR HANDLING: all KspError variants and Display
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn error_io_display() {
    let err = KspError::Io(std::io::Error::new(
        std::io::ErrorKind::ConnectionRefused,
        "refused",
    ));
    let msg = format!("{err}");
    assert!(msg.contains("TCP connection error"));
}

#[test]
fn error_connection_rejected_display() {
    let err = KspError::ConnectionRejected("bad client".to_string());
    let msg = format!("{err}");
    assert!(msg.contains("kRPC connection rejected"));
    assert!(msg.contains("bad client"));
}

#[test]
fn error_procedure_error_display() {
    let err = KspError::ProcedureError {
        service: "SpaceCenter".to_string(),
        name: "InvalidOp".to_string(),
        description: "No vessel".to_string(),
    };
    let msg = format!("{err}");
    assert!(msg.contains("SpaceCenter"));
    assert!(msg.contains("InvalidOp"));
    assert!(msg.contains("No vessel"));
}

#[test]
fn error_protocol_display() {
    let err = KspError::Protocol("unexpected EOF".to_string());
    let msg = format!("{err}");
    assert!(msg.contains("kRPC protocol error"));
    assert!(msg.contains("unexpected EOF"));
}

#[test]
fn error_no_active_vessel_display() {
    let err = KspError::NoActiveVessel;
    let msg = format!("{err}");
    assert!(msg.contains("No active vessel"));
}

#[test]
fn error_not_connected_display() {
    let err = KspError::NotConnected;
    let msg = format!("{err}");
    assert!(msg.contains("not connected"));
}

#[test]
fn error_from_io_conversion() {
    let io_err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe broke");
    let ksp_err: KspError = io_err.into();
    assert!(matches!(ksp_err, KspError::Io(_)));
}

#[test]
fn error_from_decode_conversion() {
    // Provoke a real prost::DecodeError by decoding invalid data as a message
    let bad_data = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x02];
    let result = ConnectionResponse::decode(bad_data.as_slice());
    if let Err(decode_err) = result {
        let ksp_err: KspError = decode_err.into();
        assert!(matches!(ksp_err, KspError::Decode(_)));
        let msg = format!("{ksp_err}");
        assert!(msg.contains("Protobuf decode error"));
    }
}

#[test]
fn error_is_debug() {
    let err = KspError::NotConnected;
    let debug = format!("{err:?}");
    assert!(debug.contains("NotConnected"));
}

// ═══════════════════════════════════════════════════════════════════════════════
// §6  STATE MACHINE: adapter lifecycle transitions
// ═══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn adapter_initial_state() {
    let adapter = KspAdapter::new(KspConfig::default());
    assert_eq!(adapter.state().await, flight_adapter_common::AdapterState::Disconnected);
    assert!(adapter.current_snapshot().await.is_none());
}

#[tokio::test]
async fn adapter_sim_id() {
    let adapter = KspAdapter::new(KspConfig::default());
    assert_eq!(adapter.sim_id(), SimId::Ksp);
}

#[tokio::test]
async fn adapter_stop_clears_state() {
    let adapter = KspAdapter::new(KspConfig::default());
    adapter.stop().await;
    assert_eq!(adapter.state().await, flight_adapter_common::AdapterState::Disconnected);
    assert!(adapter.current_snapshot().await.is_none());
}

#[tokio::test]
async fn adapter_write_controls_queues_without_panic() {
    let adapter = KspAdapter::new(KspConfig::default());
    adapter
        .write_controls(KspControls::from_axes(0.5, -0.5, 0.1, 0.9))
        .await;
    adapter.write_controls(KspControls::default()).await;
}

#[tokio::test]
async fn adapter_multiple_stop_calls_safe() {
    let adapter = KspAdapter::new(KspConfig::default());
    for _ in 0..5 {
        adapter.stop().await;
    }
    assert_eq!(adapter.state().await, flight_adapter_common::AdapterState::Disconnected);
}

// ── Config serialization ────────────────────────────────────────────────────

#[test]
fn config_serde_roundtrip() {
    let cfg = KspConfig::default();
    let json = serde_json::to_string(&cfg).unwrap();
    let parsed: KspConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.krpc_host, cfg.krpc_host);
    assert_eq!(parsed.krpc_port, cfg.krpc_port);
    assert!((parsed.poll_rate_hz - cfg.poll_rate_hz).abs() < f32::EPSILON);
}

#[test]
fn config_custom_serde_roundtrip() {
    let cfg = KspConfig {
        krpc_host: "10.0.0.1".to_string(),
        krpc_port: 12345,
        poll_rate_hz: 50.0,
        connection_timeout: Duration::from_millis(2500),
        reconnect_delay: Duration::from_secs(1),
        max_reconnect_delay: Duration::from_secs(30),
    };
    let json = serde_json::to_string_pretty(&cfg).unwrap();
    let parsed: KspConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.krpc_port, 12345);
    assert_eq!(parsed.max_reconnect_delay, Duration::from_secs(30));
}

// ── Backoff logic ───────────────────────────────────────────────────────────

#[test]
fn backoff_never_exceeds_max_delay() {
    let max = Duration::from_secs(60);
    let mut delay = Duration::from_secs(2);
    for _ in 0..20 {
        delay = (delay * 2).min(max);
        assert!(delay <= max);
    }
}

#[test]
fn backoff_reaches_max_in_expected_steps() {
    let max = Duration::from_secs(60);
    let mut delay = Duration::from_secs(2);
    let mut steps = 0;
    while delay < max {
        delay = (delay * 2).min(max);
        steps += 1;
    }
    assert_eq!(steps, 5, "2→4→8→16→32→60 = 5 steps");
}

// ═══════════════════════════════════════════════════════════════════════════════
// §7  RAW TELEMETRY DEFAULT
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn raw_telemetry_default_all_zeroed() {
    let raw = KspRawTelemetry::default();
    assert!(raw.vessel_name.is_empty());
    assert_eq!(raw.situation, 0);
    assert!((raw.pitch_deg).abs() < f32::EPSILON);
    assert!((raw.roll_deg).abs() < f32::EPSILON);
    assert!((raw.heading_deg).abs() < f32::EPSILON);
    assert!((raw.speed_mps).abs() < f64::EPSILON);
    assert!((raw.ias_mps).abs() < f64::EPSILON);
    assert!((raw.vertical_speed_mps).abs() < f64::EPSILON);
    assert!((raw.g_force).abs() < f64::EPSILON);
    assert!((raw.altitude_m).abs() < f64::EPSILON);
    assert!((raw.latitude_deg).abs() < f64::EPSILON);
    assert!((raw.longitude_deg).abs() < f64::EPSILON);
}

#[test]
fn raw_telemetry_clone() {
    let raw = flying_telemetry();
    let cloned = raw.clone();
    assert_eq!(cloned.vessel_name, raw.vessel_name);
    assert_eq!(cloned.situation, raw.situation);
    assert!((cloned.pitch_deg - raw.pitch_deg).abs() < f32::EPSILON);
}

#[test]
fn raw_telemetry_debug() {
    let raw = flying_telemetry();
    let debug = format!("{raw:?}");
    assert!(debug.contains("KspRawTelemetry"));
    assert!(debug.contains("TestCraft"));
}

// ═══════════════════════════════════════════════════════════════════════════════
// §8  PROPERTY-BASED TESTS (proptest)
// ═══════════════════════════════════════════════════════════════════════════════

mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// Encoding then decoding any u64 must round-trip exactly.
        #[test]
        fn prop_object_roundtrip(handle: u64) {
            let decoded = decode_object(&encode_object(handle)).unwrap();
            prop_assert_eq!(decoded, handle);
        }

        /// Encoding then decoding any finite f32 must round-trip exactly.
        #[test]
        fn prop_float_roundtrip(v in proptest::num::f32::NORMAL) {
            let decoded = decode_float(&encode_float(v)).unwrap();
            prop_assert!((decoded - v).abs() < f32::EPSILON || decoded == v);
        }

        /// Bool encoding round-trip.
        #[test]
        fn prop_bool_roundtrip(v: bool) {
            let decoded = decode_bool(&encode_bool(v)).unwrap();
            prop_assert_eq!(decoded, v);
        }

        /// Decoding arbitrary bytes must never panic for any decoder.
        #[test]
        fn prop_decoders_never_panic(data: Vec<u8>) {
            let _ = decode_object(&data);
            let _ = decode_double(&data);
            let _ = decode_float(&data);
            let _ = decode_bool(&data);
            let _ = decode_string(&data);
            let _ = decode_int32(&data);
            let _ = ConnectionResponse::decode(data.as_slice());
            let _ = Response::decode(data.as_slice());
        }

        /// apply_telemetry must never panic for arbitrary f32/f64/i32 inputs.
        #[test]
        fn prop_telemetry_never_panics(
            pitch in proptest::num::f32::ANY,
            roll in proptest::num::f32::ANY,
            heading in proptest::num::f32::ANY,
            speed in proptest::num::f64::ANY,
            ias in proptest::num::f64::ANY,
            vs in proptest::num::f64::ANY,
            g in proptest::num::f64::ANY,
            alt in proptest::num::f64::ANY,
            lat in proptest::num::f64::ANY,
            lon in proptest::num::f64::ANY,
            sit in -10i32..20i32,
        ) {
            let raw = KspRawTelemetry {
                vessel_name: "prop-test".to_string(),
                situation: sit,
                pitch_deg: pitch,
                roll_deg: roll,
                heading_deg: heading,
                speed_mps: speed,
                ias_mps: ias,
                vertical_speed_mps: vs,
                g_force: g,
                altitude_m: alt,
                latitude_deg: lat,
                longitude_deg: lon,
            };
            let mut snap = default_snapshot();
            apply_telemetry(&mut snap, &raw);
            // GForce must always be within bounds after clamping
            let g_val = snap.kinematics.g_force.value();
            prop_assert!((-20.0..=20.0).contains(&g_val), "g_force out of range: {g_val}");
        }

        /// KspControls::clamped() must always produce a valid result.
        #[test]
        fn prop_clamped_always_valid(
            pitch in proptest::num::f32::ANY,
            roll in proptest::num::f32::ANY,
            yaw in proptest::num::f32::ANY,
            throttle in proptest::num::f32::ANY,
        ) {
            let c = KspControls { pitch, roll, yaw, throttle, gear: None };
            if !pitch.is_nan() && !roll.is_nan() && !yaw.is_nan() && !throttle.is_nan() {
                let cl = c.clamped();
                prop_assert!(cl.is_valid(), "clamped() must produce valid controls: {cl:?}");
            }
        }

        /// TAS must never exceed 1000 knots after apply_telemetry.
        #[test]
        fn prop_speed_clamped_to_1000_knots(speed_mps in 0.0f64..100_000.0f64) {
            let raw = KspRawTelemetry {
                speed_mps,
                situation: situation::FLYING,
                ..Default::default()
            };
            let mut snap = default_snapshot();
            apply_telemetry(&mut snap, &raw);
            prop_assert!(snap.kinematics.tas.to_knots() <= 1000.0,
                "TAS {} kt exceeds limit for {speed_mps} m/s",
                snap.kinematics.tas.to_knots());
        }
    }
}
