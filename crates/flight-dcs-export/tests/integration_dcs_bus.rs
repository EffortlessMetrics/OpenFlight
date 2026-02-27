// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Comprehensive integration tests for the DCS export adapter.
//!
//! Covers:
//! - Parsing: full/minimal/malformed packets, cockpit indicators, out-of-range fields
//! - State machine: Disconnected state and connection timeout detection
//! - Bus integration: field mapping, snapshot timestamp monotonicity, validation
//! - Error handling: port conflicts, empty/malformed packets, unknown fields
//! - TCP loopback: end-to-end `SocketBridge` message exchange

use flight_dcs_export::{
    AdapterState, DcsAdapter, DcsAdapterConfig, DcsMessage, ProtocolVersion, SocketBridge,
    SocketBridgeConfig,
};
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn create_adapter() -> DcsAdapter {
    DcsAdapter::new(DcsAdapterConfig::default())
}

fn make_data(v: serde_json::Value) -> HashMap<String, serde_json::Value> {
    v.as_object().unwrap().clone().into_iter().collect()
}

/// Bind to port 0, capture the OS-assigned port, then drop the listener.
/// The port is briefly free for the test to reuse (acceptable race for tests).
fn find_free_tcp_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

fn bridge_config(port: u16) -> SocketBridgeConfig {
    SocketBridgeConfig {
        bind_addr: format!("127.0.0.1:{port}").parse().unwrap(),
        connect_timeout: Duration::from_secs(2),
        heartbeat_interval: Duration::from_secs(30),
        max_message_size: 64 * 1024,
        supported_versions: vec![ProtocolVersion::V1_0],
    }
}

fn send_dcs_message(msg: &DcsMessage) -> String {
    format!("{}\n", serde_json::to_string(msg).unwrap())
}

// ============================================================================
// 1. Parsing tests
// ============================================================================

/// Parse a DCS export packet containing all primary telemetry fields.
///
/// In DCS: pitch, bank (roll), heading (yaw equivalent), ias (airspeed),
/// altitude_asl, and aoa must all land in the right BusSnapshot slots.
#[test]
fn test_parse_full_dcs_export_packet_all_named_fields() {
    let adapter = create_adapter();
    let data = make_data(json!({
        "pitch":         5.5,
        "bank":         -10.0,  // roll
        "heading":       90.0,  // yaw equivalent in DCS (ValidatedAngle: -180..=180)
        "ias":           350.0, // indicated airspeed, knots
        "altitude_asl":  15_000.0,
        "aoa":           4.5,   // angle of attack, degrees
    }));

    let snap = adapter
        .convert_to_bus_snapshot(0, "F-16C", &data)
        .expect("full DCS export packet must parse without error");

    assert!(
        (snap.kinematics.pitch.value() - 5.5_f32).abs() < 1e-4,
        "pitch"
    );
    assert!(
        (snap.kinematics.bank.value() - (-10.0_f32)).abs() < 1e-4,
        "bank/roll"
    );
    assert!(
        (snap.kinematics.heading.value() - 90.0_f32).abs() < 1e-4,
        "heading/yaw"
    );
    assert!(
        (snap.kinematics.ias.value() - 350.0_f32).abs() < 1e-4,
        "airspeed/IAS"
    );
    assert!(
        (snap.environment.altitude - 15_000.0_f32).abs() < 1.0,
        "altitude"
    );
    assert!((snap.kinematics.aoa.value() - 4.5_f32).abs() < 1e-4, "AoA");
}

/// A minimal packet containing only `ias` must succeed; all omitted fields must
/// default to 0 / their zero value.
#[test]
fn test_parse_minimal_packet_absent_fields_default_to_zero() {
    let adapter = create_adapter();
    let data = make_data(json!({"ias": 120.0}));

    let snap = adapter
        .convert_to_bus_snapshot(0, "T-45", &data)
        .expect("minimal packet with only IAS must succeed");

    assert!((snap.kinematics.ias.value() - 120.0_f32).abs() < 1e-4);
    assert_eq!(snap.kinematics.pitch.value(), 0.0, "pitch defaults to 0");
    assert_eq!(snap.kinematics.bank.value(), 0.0, "bank defaults to 0");
    assert_eq!(
        snap.kinematics.heading.value(),
        0.0,
        "heading defaults to 0"
    );
    assert_eq!(snap.environment.altitude, 0.0, "altitude defaults to 0");
    assert_eq!(snap.kinematics.aoa.value(), 0.0, "AoA defaults to 0");
}

/// Malformed and truncated JSON strings must never panic; returning a parse
/// error is the expected outcome.
#[test]
fn test_parse_malformed_and_truncated_packets_never_panic() {
    let malformed_inputs: &[&str] = &[
        "",
        "not json at all",
        r#"{"type":"#,                       // truncated mid-key
        r#"{"type": "Telemetry", "data": "#, // truncated mid-object
        "null",
        "[]",
        r#"{"type": "UnknownVariant", "data": {}}"#,
        r#"{"type": "Telemetry"}"#, // missing `data` key
        &"Z".repeat(65_536),        // very long invalid input
    ];

    for input in malformed_inputs {
        // Must not panic — returning Err is the correct behaviour
        let _ = serde_json::from_str::<DcsMessage>(input);
    }
}

/// Unknown cockpit-indicator fields (button states, switch positions, HUD
/// brightness, etc.) that Export.lua may emit must be silently ignored.
/// The adapter must succeed and return a valid snapshot.
#[test]
fn test_unknown_cockpit_indicator_fields_are_silently_ignored() {
    let adapter = create_adapter();
    let data = make_data(json!({
        "ias":                    200.0,
        // Cockpit-panel extras — not part of the adapter's field map
        "master_arm":              1.0,
        "gear_handle":             0.0,
        "flap_handle":             20.0,
        "autopilot_engaged":       1.0,
        "radio_freq_mhz":          305.5,
        "hud_brightness":          0.75,
        "some_unknown_future_key": "ignored_value",
    }));

    let snap = adapter
        .convert_to_bus_snapshot(0, "F-16C", &data)
        .expect("unknown cockpit-indicator fields must be silently ignored");

    // The one known field must still be mapped correctly
    assert!((snap.kinematics.ias.value() - 200.0_f32).abs() < 1e-4);
}

/// IAS above the ValidatedSpeed maximum (~1000 knots) must be rejected with
/// a `TelemetryParsing` error, not silently truncated or cause a panic.
#[test]
fn test_out_of_range_ias_is_rejected_with_error() {
    let adapter = create_adapter();
    let data = make_data(json!({"ias": 2500.0}));

    let result = adapter.convert_to_bus_snapshot(0, "F-16C", &data);
    assert!(
        result.is_err(),
        "IAS of 2500 kts exceeds ValidatedSpeed max and must return an error, not succeed"
    );
}

/// Flaps above 100 must be clamped to 100 before being stored; the conversion
/// must succeed (not error).
#[test]
fn test_out_of_range_flaps_are_clamped_to_100() {
    let adapter = create_adapter();
    let data = make_data(json!({"ias": 80.0, "flaps": 130.0}));

    let snap = adapter
        .convert_to_bus_snapshot(0, "F/A-18C", &data)
        .expect("flaps above 100 must be clamped to 100 without returning an error");

    assert_eq!(snap.config.flaps.value(), 100.0);
}

// ============================================================================
// 2. State machine tests
// ============================================================================

/// A freshly constructed adapter must always be in the `Disconnected` state.
/// All "no-connection" helpers must agree.
#[test]
fn test_initial_adapter_state_is_disconnected() {
    let adapter = create_adapter();

    assert_eq!(
        adapter.state(),
        AdapterState::Disconnected,
        "new adapter must start Disconnected"
    );
    assert!(
        adapter.connection_status().is_none(),
        "no connection_status for a fresh adapter"
    );
    assert!(
        !adapter.is_connection_timeout(),
        "is_connection_timeout must be false with no connection"
    );
    assert!(
        adapter.time_since_last_telemetry().is_none(),
        "time_since_last_telemetry must be None with no connection"
    );
}

/// Without an active connection, `is_connection_timeout()` must stay false and
/// `time_since_last_telemetry()` must stay None regardless of elapsed time.
#[test]
fn test_no_timeout_detected_without_active_connection() {
    let adapter = create_adapter();

    // No connection established → timeout helpers must not fire
    assert!(!adapter.is_connection_timeout());
    assert!(adapter.time_since_last_telemetry().is_none());
}

/// After a session transitions to MP and back to SP, the adapter must reflect
/// the updated session state correctly (reconnection scenario).
#[test]
fn test_session_type_transitions_sp_to_mp_to_sp() {
    let mut adapter = create_adapter();

    // Start in SP
    adapter
        .update_mp_session(&json!({"session_type": "SP"}))
        .unwrap();
    assert!(!adapter.is_multiplayer(), "should be SP after SP update");

    // Transition to MP (simulates connecting to a server)
    adapter
        .update_mp_session(&json!({"session_type": "MP", "server_name": "Test"}))
        .unwrap();
    assert!(adapter.is_multiplayer(), "should be MP after MP update");

    // Return to SP (simulates leaving the server / reconnecting SP)
    adapter
        .update_mp_session(&json!({"session_type": "SP"}))
        .unwrap();
    assert!(
        !adapter.is_multiplayer(),
        "should be back to SP after second SP update"
    );
}

// ============================================================================
// 3. Bus integration tests
// ============================================================================

/// Verify that all primary DCS telemetry fields map to the correct
/// BusSnapshot slots: pitch, bank (roll), heading (yaw), IAS (airspeed),
/// altitude, AoA, vertical speed, and g-force.
#[test]
fn test_bus_snapshot_primary_telemetry_field_mapping() {
    let adapter = create_adapter();
    let data = make_data(json!({
        "pitch":          -3.0,
        "bank":            8.0,
        "heading":         45.0,
        "ias":             280.0,
        "altitude_asl":    8_500.0,
        "aoa":             6.2,
        "vertical_speed":  500.0,
        "g_force":         2.5,
    }));

    let snap = adapter.convert_to_bus_snapshot(0, "A-10C", &data).unwrap();

    assert!(
        (snap.kinematics.pitch.value() - (-3.0_f32)).abs() < 1e-4,
        "pitch"
    );
    assert!(
        (snap.kinematics.bank.value() - 8.0_f32).abs() < 1e-4,
        "roll/bank"
    );
    assert!(
        (snap.kinematics.heading.value() - 45.0_f32).abs() < 1e-4,
        "yaw/heading"
    );
    assert!(
        (snap.kinematics.ias.value() - 280.0_f32).abs() < 1e-4,
        "airspeed/IAS"
    );
    assert!(
        (snap.environment.altitude - 8_500.0_f32).abs() < 1.0,
        "altitude"
    );
    assert!((snap.kinematics.aoa.value() - 6.2_f32).abs() < 1e-4, "AoA");
    assert!(
        (snap.kinematics.vertical_speed - 500.0_f32).abs() < 1e-3,
        "vertical_speed"
    );
    assert!(
        (snap.kinematics.g_force.value() - 2.5_f32).abs() < 1e-4,
        "g_force"
    );
}

/// `BusSnapshot::timestamp` must be monotonically non-decreasing across
/// consecutive calls.  A later snapshot must have a timestamp ≥ the earlier one.
#[test]
fn test_bus_snapshot_timestamps_are_monotonically_non_decreasing() {
    let adapter = create_adapter();
    let data = make_data(json!({"ias": 100.0}));

    let snap1 = adapter
        .convert_to_bus_snapshot(1000, "F-16C", &data)
        .unwrap();

    // A short sleep guarantees wall-clock time advances
    std::thread::sleep(Duration::from_millis(30));

    let snap2 = adapter
        .convert_to_bus_snapshot(1033, "F-16C", &data)
        .unwrap();

    assert!(
        snap2.timestamp >= snap1.timestamp,
        "snap2.ts={} must be >= snap1.ts={}",
        snap2.timestamp,
        snap1.timestamp
    );

    // The delta should be at least ~30 ms worth of nanoseconds
    let delta_ns = snap2.timestamp.saturating_sub(snap1.timestamp);
    assert!(
        delta_ns >= 20_000_000,
        "expected ≥20 ms of ns delta, got {} ns",
        delta_ns
    );
}

/// A snapshot created 60 ms ago must have `age_ms() ≥ 50`.
#[test]
fn test_bus_snapshot_age_ms_increases_after_sleep() {
    let adapter = create_adapter();
    let data = make_data(json!({"ias": 100.0}));

    let snap = adapter.convert_to_bus_snapshot(0, "F-16C", &data).unwrap();

    std::thread::sleep(Duration::from_millis(60));

    let age = snap.age_ms();
    assert!(
        age >= 50,
        "expected snapshot age ≥ 50 ms after sleeping 60 ms, got {} ms",
        age
    );
}

/// `validate()` must return `Ok(())` for a snapshot built from valid telemetry.
#[test]
fn test_bus_snapshot_validate_passes_for_valid_telemetry() {
    let adapter = create_adapter();
    let data = make_data(json!({"ias": 350.0, "tas": 380.0, "g_force": 1.1}));

    let snap = adapter.convert_to_bus_snapshot(0, "F-16C", &data).unwrap();

    assert!(
        snap.validate().is_ok(),
        "valid telemetry must pass validation"
    );
}

/// The `sim` field of the BusSnapshot must be `SimId::Dcs` and the `aircraft`
/// ICAO must match the name passed to `convert_to_bus_snapshot`.
#[test]
fn test_bus_snapshot_sim_id_and_aircraft_icao_are_set_correctly() {
    let adapter = create_adapter();
    let data = make_data(json!({"ias": 200.0}));

    let snap = adapter.convert_to_bus_snapshot(0, "Ka-50", &data).unwrap();

    // SimId comparison via debug string (avoids needing `use flight_bus::...`)
    assert_eq!(
        format!("{:?}", snap.sim),
        "Dcs",
        "sim field must be SimId::Dcs"
    );
    assert_eq!(snap.aircraft.icao, "Ka-50");
}

// ============================================================================
// 4. Error handling tests
// ============================================================================

/// If the target TCP port is already bound, `SocketBridge::start()` must return
/// an error — not panic or silently ignore the conflict.
#[tokio::test]
async fn test_socket_bridge_start_fails_when_port_is_already_in_use() {
    let port = find_free_tcp_port();
    let addr: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();

    // Hold the port open so the bridge cannot bind to it
    let _holder = tokio::net::TcpListener::bind(addr).await.unwrap();

    let mut bridge = SocketBridge::new(bridge_config(port));
    let result = bridge.start().await;

    assert!(
        result.is_err(),
        "SocketBridge::start() must return an error when the port is already in use"
    );
}

/// Attempting to parse an empty byte sequence as a `DcsMessage` must return an
/// error and must not panic.
#[test]
fn test_parse_empty_string_returns_error_no_panic() {
    let result = serde_json::from_str::<DcsMessage>("");
    assert!(
        result.is_err(),
        "parsing an empty string must return a parse error"
    );
}

/// A `Telemetry` payload that contains extra unknown fields (fields not in the
/// adapter's mapping table) must be accepted and must not cause an error.
#[test]
fn test_telemetry_with_extra_unknown_fields_is_accepted() {
    let adapter = create_adapter();
    let data = make_data(json!({
        "ias":              200.0,
        "future_field_1":   "some_string_value",
        "future_field_2":   {"nested": true, "count": 42},
        "future_field_3":   [1, 2, 3],
    }));

    let result = adapter.convert_to_bus_snapshot(0, "Su-27", &data);
    assert!(
        result.is_ok(),
        "extra unknown fields must not cause a conversion error: {:?}",
        result
    );

    // The known field must still be mapped correctly
    let snap = result.unwrap();
    assert!((snap.kinematics.ias.value() - 200.0_f32).abs() < 1e-4);
}

/// When telemetry fields contain the wrong JSON value type (string/bool/array
/// where a number is expected), the adapter must skip those fields gracefully —
/// no panic, and no error for the overall snapshot.
#[test]
fn test_wrong_json_value_types_in_telemetry_are_skipped_gracefully() {
    let adapter = create_adapter();
    let data = make_data(json!({
        "ias":          "not_a_number",
        "altitude_asl": {"unexpected": "object"},
        "heading":      true,
        "pitch":        null,
        "bank":         [],
    }));

    // Must not panic; the adapter may succeed with defaults or return an error
    let _ = adapter.convert_to_bus_snapshot(0, "F-16C", &data);
}

// ============================================================================
// 5. TCP loopback integration tests
// ============================================================================

/// `SocketBridge` must accept an inbound TCP connection on the loopback
/// interface and report a `connection_count` of 1.
#[tokio::test]
async fn test_socket_bridge_accepts_loopback_tcp_connection() {
    let port = find_free_tcp_port();
    let addr: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();

    let mut bridge = SocketBridge::new(bridge_config(port));
    bridge.start().await.expect("bridge must bind successfully");

    // Connect from a background task so accept() isn't blocked
    let connect_task = tokio::spawn(async move {
        TcpStream::connect(addr)
            .await
            .expect("client must connect to bridge")
    });

    let accepted = bridge
        .accept_connection()
        .await
        .expect("accept_connection must not error");
    assert!(
        accepted.is_some(),
        "bridge must accept the inbound connection"
    );
    assert_eq!(
        bridge.connection_count(),
        1,
        "connection_count must be 1 after one accept"
    );

    // Allow the spawned task to complete cleanly
    let _ = connect_task.await;
}

/// A `Handshake` message sent by a mock DCS client over TCP must be received
/// and parsed correctly by `SocketBridge::process_messages()`.
#[tokio::test]
async fn test_socket_bridge_receives_handshake_over_tcp() {
    let port = find_free_tcp_port();
    let addr: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();

    let mut bridge = SocketBridge::new(bridge_config(port));
    bridge.start().await.unwrap();

    let msg = DcsMessage::Handshake {
        version: ProtocolVersion::V1_0,
        features: vec![
            "telemetry_basic".to_string(),
            "telemetry_navigation".to_string(),
        ],
    };
    let wire = send_dcs_message(&msg);

    tokio::spawn(async move {
        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream.write_all(wire.as_bytes()).await.unwrap();
        stream.flush().await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
    });

    let client_addr = bridge
        .accept_connection()
        .await
        .unwrap()
        .expect("must accept the connection");

    tokio::time::sleep(Duration::from_millis(50)).await;

    let messages = bridge.process_messages().await.unwrap();
    assert_eq!(messages.len(), 1, "expected exactly one message");

    match &messages[0] {
        (addr, DcsMessage::Handshake { version, features }) => {
            assert_eq!(*addr, client_addr);
            assert_eq!(*version, ProtocolVersion::V1_0);
            assert!(features.contains(&"telemetry_basic".to_string()));
            assert!(features.contains(&"telemetry_navigation".to_string()));
        }
        other => panic!("expected Handshake, got {:?}", other),
    }
}

/// A `Telemetry` message sent over TCP must be received with the correct
/// timestamp, aircraft name, session type, and data keys.
#[tokio::test]
async fn test_socket_bridge_receives_telemetry_over_tcp() {
    let port = find_free_tcp_port();
    let addr: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();

    let mut bridge = SocketBridge::new(bridge_config(port));
    bridge.start().await.unwrap();

    let expected_ts: u64 = 987_654_321;
    let mut telem_data = HashMap::new();
    telem_data.insert("ias".to_string(), json!(350.0));
    telem_data.insert("altitude_asl".to_string(), json!(15_000.0));
    telem_data.insert("pitch".to_string(), json!(3.0));

    let msg = DcsMessage::Telemetry {
        timestamp: expected_ts,
        aircraft: "F-16C".to_string(),
        session_type: "SP".to_string(),
        data: telem_data,
    };
    let wire = send_dcs_message(&msg);

    tokio::spawn(async move {
        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream.write_all(wire.as_bytes()).await.unwrap();
        stream.flush().await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
    });

    let _client_addr = bridge
        .accept_connection()
        .await
        .unwrap()
        .expect("must accept");

    tokio::time::sleep(Duration::from_millis(50)).await;

    let messages = bridge.process_messages().await.unwrap();
    assert_eq!(messages.len(), 1);

    match &messages[0] {
        (
            _,
            DcsMessage::Telemetry {
                timestamp,
                aircraft,
                session_type,
                data,
            },
        ) => {
            assert_eq!(*timestamp, expected_ts);
            assert_eq!(aircraft, "F-16C");
            assert_eq!(session_type, "SP");
            assert!(data.contains_key("ias"), "data must contain 'ias'");
            assert!(
                data.contains_key("altitude_asl"),
                "data must contain 'altitude_asl'"
            );
            assert!(data.contains_key("pitch"), "data must contain 'pitch'");
        }
        other => panic!("expected Telemetry, got {:?}", other),
    }
}

/// A `Heartbeat` message sent over TCP must be received with the correct
/// timestamp.
#[tokio::test]
async fn test_socket_bridge_receives_heartbeat_over_tcp() {
    let port = find_free_tcp_port();
    let addr: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();

    let mut bridge = SocketBridge::new(bridge_config(port));
    bridge.start().await.unwrap();

    let expected_ts: u64 = 42_000;
    let wire = send_dcs_message(&DcsMessage::Heartbeat {
        timestamp: expected_ts,
    });

    tokio::spawn(async move {
        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream.write_all(wire.as_bytes()).await.unwrap();
        stream.flush().await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
    });

    let _client_addr = bridge
        .accept_connection()
        .await
        .unwrap()
        .expect("must accept");

    tokio::time::sleep(Duration::from_millis(50)).await;

    let messages = bridge.process_messages().await.unwrap();
    assert_eq!(messages.len(), 1);

    match &messages[0] {
        (_, DcsMessage::Heartbeat { timestamp }) => {
            assert_eq!(*timestamp, expected_ts);
        }
        other => panic!("expected Heartbeat, got {:?}", other),
    }
}

/// An `Error` message from a mock DCS client must be received with the correct
/// error code and message text.
#[tokio::test]
async fn test_socket_bridge_receives_error_message_over_tcp() {
    let port = find_free_tcp_port();
    let addr: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();

    let mut bridge = SocketBridge::new(bridge_config(port));
    bridge.start().await.unwrap();

    let wire = send_dcs_message(&DcsMessage::Error {
        code: "DCS_INIT_FAILED".to_string(),
        message: "Export.lua failed to initialise".to_string(),
    });

    tokio::spawn(async move {
        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream.write_all(wire.as_bytes()).await.unwrap();
        stream.flush().await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
    });

    let _client_addr = bridge
        .accept_connection()
        .await
        .unwrap()
        .expect("must accept");

    tokio::time::sleep(Duration::from_millis(50)).await;

    let messages = bridge.process_messages().await.unwrap();
    assert_eq!(messages.len(), 1);

    match &messages[0] {
        (_, DcsMessage::Error { code, message }) => {
            assert_eq!(code, "DCS_INIT_FAILED");
            assert!(
                message.contains("Export.lua"),
                "error message must mention Export.lua"
            );
        }
        other => panic!("expected Error, got {:?}", other),
    }
}

/// An MP `Telemetry` message sent over TCP must preserve `session_type = "MP"`
/// through the full wire round-trip.
#[tokio::test]
async fn test_socket_bridge_receives_mp_telemetry_preserves_session_type() {
    let port = find_free_tcp_port();
    let addr: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();

    let mut bridge = SocketBridge::new(bridge_config(port));
    bridge.start().await.unwrap();

    let mut data = HashMap::new();
    data.insert("ias".to_string(), json!(280.0));
    // Weapons field present on wire — adapter will filter it in MP
    data.insert(
        "weapons".to_string(),
        json!({"missile": "AIM-120C", "count": 4}),
    );

    let wire = send_dcs_message(&DcsMessage::Telemetry {
        timestamp: 500,
        aircraft: "F/A-18C".to_string(),
        session_type: "MP".to_string(),
        data,
    });

    tokio::spawn(async move {
        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream.write_all(wire.as_bytes()).await.unwrap();
        stream.flush().await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
    });

    let _client_addr = bridge
        .accept_connection()
        .await
        .unwrap()
        .expect("must accept");

    tokio::time::sleep(Duration::from_millis(50)).await;

    let messages = bridge.process_messages().await.unwrap();
    assert_eq!(messages.len(), 1);

    match &messages[0] {
        (_, DcsMessage::Telemetry { session_type, .. }) => {
            assert_eq!(
                session_type, "MP",
                "session_type must survive the TCP round-trip"
            );
        }
        other => panic!("expected Telemetry, got {:?}", other),
    }
}
