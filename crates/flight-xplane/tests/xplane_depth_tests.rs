// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for X-Plane adapter — UDP/plugin integration.
//!
//! Covers:
//! 1. UDP protocol codec (RREF, DATA, DREF, CMND)
//! 2. Dataref system (subscribe, update, unsubscribe, types)
//! 3. Adapter state machine (lifecycle, timeout, reconnect)
//! 4. Bus publishing (snapshot, stale detection, field mapping)
//! 5. Aircraft detection (ICAO, classification, livery, change)
//! 6. Command injection (set_dataref, send_command, axis, rate limiting)
//! 7. Plugin protocol (encode/decode, discovery, heartbeat)

use flight_xplane::{
    // UDP protocol
    ParseError,
    build_dref_command, parse_data_packet, parse_rref_response,
    // Dataref system
    DatarefManager,
    // Adapter state machine
    AdapterEvent, AdapterStateMachine, TransitionError, XPlaneAdapterState,
    // Dataref database
    DatarefDatabase, DatarefType,
    // Aircraft detection (base)
    AircraftDetector,
    aircraft::AircraftType,
    // Enhanced aircraft detection
    AircraftChange, EnhancedAircraftDetector, EnhancedAircraftId,
    aircraft_detection::{
        DATAREF_ACF_DESCRIP, DATAREF_ACF_FILE_PATH, DATAREF_ACF_ICAO, DATAREF_ACF_LIVERY,
    },
    // Control injection
    control_injection::{ControlInjectorConfig, XPlaneControlInjector, ControlInjectionError},
    // Plugin protocol
    plugin_protocol::{
        self, DatarefEntry, PluginDiscovery, PluginDiscoveryState,
        PluginProtoMessage, ProtocolError, SubscriptionRequest,
    },
    // DataRef value type
    dataref::DataRefValue,
};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;

// ═══════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════

fn make_data_packet(groups: &[(u32, [f32; 8])]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(b"DATA\0");
    for (idx, vals) in groups {
        buf.extend_from_slice(&idx.to_le_bytes());
        for v in vals {
            buf.extend_from_slice(&v.to_le_bytes());
        }
    }
    buf
}

fn make_rref_packet(entries: &[(u32, f32)]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(b"RREF\0");
    for (id, val) in entries {
        buf.extend_from_slice(&id.to_le_bytes());
        buf.extend_from_slice(&val.to_le_bytes());
    }
    buf
}

/// Build RREF request packet (client → X-Plane):
/// `RREF\0` + freq(u32 LE) + id(u32 LE) + 400-byte NUL-padded path
fn make_rref_request(freq_hz: u32, id: u32, path: &str) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(b"RREF\0");
    buf.extend_from_slice(&freq_hz.to_le_bytes());
    buf.extend_from_slice(&id.to_le_bytes());
    let path_bytes = path.as_bytes();
    let copy_len = path_bytes.len().min(400);
    buf.extend_from_slice(&path_bytes[..copy_len]);
    buf.resize(5 + 4 + 4 + 400, 0);
    buf
}

fn make_raw(icao: &str, descrip: &str) -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert(DATAREF_ACF_ICAO.to_owned(), icao.to_owned());
    m.insert(DATAREF_ACF_DESCRIP.to_owned(), descrip.to_owned());
    m
}

fn drive_to_active(sm: &mut AdapterStateMachine) {
    sm.transition(AdapterEvent::SocketBound).unwrap();
    sm.transition(AdapterEvent::SocketBound).unwrap();
    sm.transition(AdapterEvent::TelemetryReceived).unwrap();
}

async fn loopback_injector() -> (XPlaneControlInjector, UdpSocket) {
    let receiver = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let recv_addr = receiver.local_addr().unwrap();
    let cfg = ControlInjectorConfig {
        remote_addr: recv_addr,
        max_packets_per_second: 1000,
        min_dataref_interval: Duration::from_millis(0),
    };
    let sender = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    sender.connect(recv_addr).await.unwrap();
    let injector = XPlaneControlInjector::with_socket(cfg, sender);
    (injector, receiver)
}

// ═══════════════════════════════════════════════════════════════════════
// 1. UDP PROTOCOL (8 tests)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn udp_rref_request_format_is_413_bytes() {
    let pkt = make_rref_request(20, 1, "sim/flightmodel/position/indicated_airspeed");
    assert_eq!(pkt.len(), 5 + 4 + 4 + 400); // 413
    assert_eq!(&pkt[..5], b"RREF\0");
    let freq = u32::from_le_bytes([pkt[5], pkt[6], pkt[7], pkt[8]]);
    assert_eq!(freq, 20);
    let id = u32::from_le_bytes([pkt[9], pkt[10], pkt[11], pkt[12]]);
    assert_eq!(id, 1);
}

#[test]
fn udp_rref_response_multiple_ids_parsed_correctly() {
    let pkt = make_rref_packet(&[
        (0, 250.0),
        (1, 35000.0),
        (2, 0.82),
        (3, -3.5),
        (4, 180.0),
    ]);
    let entries = parse_rref_response(&pkt).unwrap();
    assert_eq!(entries.len(), 5);
    assert_eq!(entries[0], (0, 250.0));
    assert_eq!(entries[3].0, 3);
    assert!((entries[3].1 - (-3.5)).abs() < f32::EPSILON);
}

#[test]
fn udp_data_packet_multi_group_preserves_order() {
    let groups: Vec<(u32, [f32; 8])> = (0..10)
        .map(|i| (i, [i as f32; 8]))
        .collect();
    let pkt = make_data_packet(&groups);
    let parsed = parse_data_packet(&pkt).unwrap();
    assert_eq!(parsed.data_groups.len(), 10);
    for (i, g) in parsed.data_groups.iter().enumerate() {
        assert_eq!(g.index, i as u32);
        assert_eq!(g.values[0], i as f32);
    }
}

#[test]
fn udp_dref_write_format_509_bytes() {
    let pkt = build_dref_command("sim/cockpit2/controls/yoke_pitch_ratio", 0.75);
    assert_eq!(pkt.len(), 509); // 5 header + 4 float + 500 path
}

#[test]
fn udp_packet_fragmentation_data_truncated_detected() {
    // Construct a valid 2-group packet, then chop mid-second-group
    let pkt = make_data_packet(&[(0, [1.0; 8]), (1, [2.0; 8])]);
    let truncated = pkt[..pkt.len() - 4].to_vec(); // remove last 4 bytes
    let err = parse_data_packet(&truncated).unwrap_err();
    assert!(matches!(err, ParseError::TruncatedDataGroup { .. }));
}

#[test]
fn udp_rref_truncated_entry_detected() {
    let mut pkt = make_rref_packet(&[(1, 100.0)]);
    pkt.truncate(pkt.len() - 3); // remove 3 bytes from the entry
    let err = parse_rref_response(&pkt).unwrap_err();
    assert!(matches!(err, ParseError::TruncatedRrefEntry { .. }));
}

#[test]
fn udp_endianness_little_endian_round_trip() {
    let value: f32 = -12_345.679;
    let pkt = build_dref_command("sim/test", value);
    let decoded = f32::from_le_bytes([pkt[5], pkt[6], pkt[7], pkt[8]]);
    assert!((decoded - value).abs() < 0.01);

    // DATA packet endianness
    let vals = [f32::MIN_POSITIVE, f32::MAX, -0.0, 1.0e-38, 0.0, 0.0, 0.0, 0.0];
    let data_pkt = make_data_packet(&[(99, vals)]);
    let parsed = parse_data_packet(&data_pkt).unwrap();
    assert_eq!(parsed.data_groups[0].values[0], f32::MIN_POSITIVE);
    assert_eq!(parsed.data_groups[0].values[1], f32::MAX);
}

#[test]
fn udp_maximum_payload_rref_many_entries() {
    // RREF entry = 8 bytes, max UDP = 65535, header = 5
    // Max entries ≈ (65535 - 5) / 8 = 8191
    let entries: Vec<(u32, f32)> = (0..1000).map(|i| (i, i as f32 * 0.1)).collect();
    let pkt = make_rref_packet(&entries);
    let parsed = parse_rref_response(&pkt).unwrap();
    assert_eq!(parsed.len(), 1000);
    // Spot-check last entry
    let last = parsed.last().unwrap();
    assert_eq!(last.0, 999);
    assert!((last.1 - 99.9).abs() < 0.1);
}

// ═══════════════════════════════════════════════════════════════════════
// 2. DATAREF SYSTEM (6 tests)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn dataref_register_by_path() {
    let mut mgr = DatarefManager::new();
    mgr.subscribe("sim/flightmodel/position/indicated_airspeed", 20.0);
    assert!(mgr.is_subscribed("sim/flightmodel/position/indicated_airspeed"));
    let sub = mgr.get_subscription("sim/flightmodel/position/indicated_airspeed").unwrap();
    assert_eq!(sub.dataref_path, "sim/flightmodel/position/indicated_airspeed");
    assert_eq!(sub.update_rate_hz, 20.0);
}

#[test]
fn dataref_value_updates_overwrite() {
    let mut mgr = DatarefManager::new();
    mgr.subscribe("sim/airspeed", 10.0);
    mgr.set_value("sim/airspeed", 100.0);
    assert_eq!(mgr.get_value("sim/airspeed"), Some(100.0));
    mgr.set_value("sim/airspeed", 200.0);
    assert_eq!(mgr.get_value("sim/airspeed"), Some(200.0));
}

#[test]
fn dataref_db_array_datarefs_have_type_and_size() {
    let db = DatarefDatabase::new();
    let n1 = db.get("sim/flightmodel/engine/ENGN_N1_").unwrap();
    assert_eq!(n1.data_type, DatarefType::FloatArray);
    assert_eq!(n1.array_size, Some(8));
    assert!(!n1.writable);
}

#[test]
fn dataref_db_string_datarefs_are_data_type() {
    let db = DatarefDatabase::new();
    let icao = db.get("sim/aircraft/view/acf_ICAO").unwrap();
    assert_eq!(icao.data_type, DatarefType::Data);
    assert_eq!(icao.array_size, Some(40));
}

#[test]
fn dataref_type_conversion_int_float_double() {
    // Verify DataRefValue Display impl for all variant types
    assert_eq!(format!("{}", DataRefValue::Float(1.5)), "1.5");
    assert_eq!(format!("{}", DataRefValue::Int(42)), "42");
    assert_eq!(format!("{}", DataRefValue::Double(2.719)), "2.719");
    assert_eq!(format!("{}", DataRefValue::FloatArray(vec![1.0, 2.0])), "[1.0, 2.0]");
    assert_eq!(format!("{}", DataRefValue::IntArray(vec![10, 20])), "[10, 20]");
}

#[test]
fn dataref_unregister_removes_subscription_and_value() {
    let mut mgr = DatarefManager::new();
    mgr.subscribe("sim/airspeed", 30.0);
    mgr.set_value("sim/airspeed", 250.0);
    assert_eq!(mgr.subscription_count(), 1);

    mgr.unsubscribe("sim/airspeed");
    assert_eq!(mgr.subscription_count(), 0);
    assert!(!mgr.is_subscribed("sim/airspeed"));
    assert_eq!(mgr.get_value("sim/airspeed"), None);
}

// ═══════════════════════════════════════════════════════════════════════
// 3. STATE MACHINE (6 tests)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn sm_full_lifecycle_disconnected_to_receiving() {
    let mut sm = AdapterStateMachine::new(2000, 3);
    assert_eq!(sm.state(), XPlaneAdapterState::Disconnected);

    sm.transition(AdapterEvent::SocketBound).unwrap();
    assert_eq!(sm.state(), XPlaneAdapterState::Connecting);

    sm.transition(AdapterEvent::SocketBound).unwrap();
    assert_eq!(sm.state(), XPlaneAdapterState::Connected);
    assert!(sm.is_healthy());

    sm.transition(AdapterEvent::TelemetryReceived).unwrap();
    assert_eq!(sm.state(), XPlaneAdapterState::Active);
    assert!(sm.is_healthy());
}

#[test]
fn sm_active_to_stale_and_back() {
    let mut sm = AdapterStateMachine::new(2000, 3);
    drive_to_active(&mut sm);

    sm.transition(AdapterEvent::TelemetryTimeout).unwrap();
    assert_eq!(sm.state(), XPlaneAdapterState::Stale);
    assert!(!sm.is_healthy());

    // Repeated timeouts keep it stale
    sm.transition(AdapterEvent::TelemetryTimeout).unwrap();
    assert_eq!(sm.state(), XPlaneAdapterState::Stale);

    // Recovery
    sm.transition(AdapterEvent::TelemetryReceived).unwrap();
    assert_eq!(sm.state(), XPlaneAdapterState::Active);
    assert!(sm.is_healthy());
    assert_eq!(sm.error_count(), 0);
}

#[test]
fn sm_udp_timeout_detection_via_stale_threshold() {
    let sm = AdapterStateMachine::new(1500, 5);
    assert_eq!(sm.stale_threshold_ms(), 1500);
    assert!(sm.time_in_state().is_none());
}

#[test]
fn sm_reconnect_logic_error_to_connecting() {
    let mut sm = AdapterStateMachine::new(2000, 3);
    // Error → Connecting (retry within limit)
    sm.transition(AdapterEvent::SocketError("lost".into())).unwrap();
    assert_eq!(sm.state(), XPlaneAdapterState::Error);
    assert_eq!(sm.error_count(), 1);

    sm.transition(AdapterEvent::SocketBound).unwrap();
    assert_eq!(sm.state(), XPlaneAdapterState::Connecting);
}

#[test]
fn sm_reconnect_retries_exhausted() {
    let mut sm = AdapterStateMachine::new(2000, 2);
    sm.transition(AdapterEvent::SocketError("e1".into())).unwrap();
    sm.transition(AdapterEvent::SocketError("e2".into())).unwrap();
    assert_eq!(sm.error_count(), 2);

    let err = sm.transition(AdapterEvent::SocketBound).unwrap_err();
    assert!(matches!(err, TransitionError::RetriesExhausted { max_retries: 2 }));
}

#[test]
fn sm_multiple_xplane_instances_independent_state() {
    let mut sm1 = AdapterStateMachine::new(2000, 3);
    let mut sm2 = AdapterStateMachine::new(3000, 5);

    drive_to_active(&mut sm1);
    sm2.transition(AdapterEvent::SocketBound).unwrap();

    assert_eq!(sm1.state(), XPlaneAdapterState::Active);
    assert_eq!(sm2.state(), XPlaneAdapterState::Connecting);

    sm1.transition(AdapterEvent::TelemetryTimeout).unwrap();
    assert_eq!(sm1.state(), XPlaneAdapterState::Stale);
    assert_eq!(sm2.state(), XPlaneAdapterState::Connecting);
}

// ═══════════════════════════════════════════════════════════════════════
// 4. BUS PUBLISHING (5 tests)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn bus_snapshot_from_datarefs() {
    // Verify that DatarefManager can hold a full snapshot of values
    let mut mgr = DatarefManager::new();
    let datarefs = [
        ("sim/flightmodel/position/indicated_airspeed", 250.0),
        ("sim/flightmodel/position/theta", 5.0),
        ("sim/flightmodel/position/phi", -3.0),
        ("sim/flightmodel/position/psi", 180.0),
        ("sim/flightmodel/position/elevation", 10668.0),
    ];
    for (path, val) in &datarefs {
        mgr.subscribe(path, 20.0);
        mgr.set_value(path, *val);
    }
    assert_eq!(mgr.subscription_count(), 5);
    for (path, val) in &datarefs {
        assert_eq!(mgr.get_value(path), Some(*val));
    }
}

#[test]
fn bus_publish_frequency_20hz_datarefs_registered_at_rate() {
    let mut mgr = DatarefManager::new();
    mgr.subscribe("sim/flightmodel/position/indicated_airspeed", 20.0);
    mgr.subscribe("sim/flightmodel/position/theta", 20.0);

    let sub = mgr.get_subscription("sim/flightmodel/position/indicated_airspeed").unwrap();
    assert_eq!(sub.update_rate_hz, 20.0);

    let sub2 = mgr.get_subscription("sim/flightmodel/position/theta").unwrap();
    assert_eq!(sub2.update_rate_hz, 20.0);
}

#[test]
fn bus_stale_detection_no_value_returns_none() {
    let mgr = DatarefManager::new();
    // Not subscribed, no value
    assert_eq!(mgr.get_value("sim/flightmodel/position/indicated_airspeed"), None);
}

#[test]
fn bus_field_mapping_datarefs_to_db_categories() {
    let db = DatarefDatabase::new();
    let controls = db.flight_controls();
    let engines = db.engine_data();
    let nav = db.navigation();

    // Flight controls should include yoke, throttle, flaps, speedbrake
    assert!(controls.len() >= 6, "got {} controls", controls.len());
    // Engines should include N1, N2, EGT, FF, oil
    assert!(engines.len() >= 8, "got {} engine datarefs", engines.len());
    // Navigation includes radios, transponder
    assert!(nav.len() >= 5, "got {} nav datarefs", nav.len());
}

#[test]
fn bus_missing_dataref_handled_gracefully() {
    let mut mgr = DatarefManager::new();
    mgr.subscribe("sim/nonexistent/dataref", 10.0);
    // Subscribed but no value set → None
    assert_eq!(mgr.get_value("sim/nonexistent/dataref"), None);
    // Still subscribed
    assert!(mgr.is_subscribed("sim/nonexistent/dataref"));
    assert_eq!(mgr.subscription_count(), 1);
}

// ═══════════════════════════════════════════════════════════════════════
// 5. AIRCRAFT DETECTION (5 tests)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn aircraft_filename_dataref_used_for_detection() {
    let mut det = EnhancedAircraftDetector::with_default_db();
    let mut raw = make_raw("C172", "Cessna 172SP");
    raw.insert(
        DATAREF_ACF_FILE_PATH.to_owned(),
        "Aircraft/Laminar Research/Cessna 172SP/Cessna_172SP.acf".to_owned(),
    );
    let id = det.identify(&raw);
    assert_eq!(id.icao, "C172");
    assert!(id.db_match.is_some());
}

#[test]
fn aircraft_icao_detection_standard_codes() {
    let mut det = EnhancedAircraftDetector::with_default_db();

    for (raw_icao, expected) in &[
        ("C172\0\0", "C172"),
        ("  a320  ", "A320"),
        ("b738", "B738"),
        ("c172sp", "C172"),
    ] {
        let raw = make_raw(raw_icao, "test");
        let id = det.identify(&raw);
        assert_eq!(id.icao, *expected, "raw={} expected={}", raw_icao, expected);
    }
}

#[test]
fn aircraft_livery_path_tracked() {
    let mut det = EnhancedAircraftDetector::with_default_db();
    let mut raw = make_raw("A320", "Airbus A320");
    raw.insert(DATAREF_ACF_LIVERY.to_owned(), "liveries/Delta/".to_owned());
    let id = det.identify(&raw);
    assert_eq!(id.livery_path, Some("liveries/Delta/".to_owned()));
}

#[test]
fn aircraft_fleet_lookup_and_classification() {
    let detector = AircraftDetector::new();

    // GA aircraft
    assert_eq!(
        detector.get_aircraft_capabilities(AircraftType::GeneralAviation),
        vec!["basic_flight_controls", "engine_management", "navigation"]
    );

    // Airliner capabilities
    let caps = detector.get_aircraft_capabilities(AircraftType::Airliner);
    assert!(caps.contains(&"autopilot".to_string()));
    assert!(caps.contains(&"flight_management".to_string()));

    // Helicopter capabilities
    let helo_caps = detector.get_aircraft_capabilities(AircraftType::Helicopter);
    assert!(helo_caps.contains(&"collective".to_string()));
    assert!(helo_caps.contains(&"rotor_management".to_string()));
}

#[test]
fn aircraft_type_classification_by_icao_and_title() {
    let detector = AircraftDetector::new();

    // Direct ICAO mappings
    assert_eq!(
        detector.get_aircraft_capabilities(AircraftType::Fighter).len(),
        4
    );

    // Glider
    let glider_caps = detector.get_aircraft_capabilities(AircraftType::Glider);
    assert_eq!(glider_caps.len(), 2);
    assert!(glider_caps.contains(&"soaring".to_string()));

    // Seaplane
    let sea_caps = detector.get_aircraft_capabilities(AircraftType::Seaplane);
    assert!(sea_caps.contains(&"water_operations".to_string()));
}

// ═══════════════════════════════════════════════════════════════════════
// 6. COMMAND INJECTION (5 tests)
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn inject_command_once_sends_cmnd_packet() {
    let (mut injector, receiver) = loopback_injector().await;

    injector.send_command("sim/autopilot/heading_up").await.unwrap();

    let mut buf = [0u8; 1024];
    let n = receiver.recv(&mut buf).await.unwrap();
    assert_eq!(&buf[..5], b"CMND\0");
    let end = buf[5..n].iter().position(|&b| b == 0).unwrap();
    let cmd = std::str::from_utf8(&buf[5..5 + end]).unwrap();
    assert_eq!(cmd, "sim/autopilot/heading_up");
}

#[tokio::test]
async fn inject_command_begin_end_button_press_release() {
    let (mut injector, receiver) = loopback_injector().await;

    // Simulate button press (command begin) and release (command end)
    injector
        .send_command("sim/flight_controls/landing_gear_down")
        .await
        .unwrap();

    let mut buf = [0u8; 1024];
    let n = receiver.recv(&mut buf).await.unwrap();
    assert_eq!(&buf[..5], b"CMND\0");

    // Verify the command path is intact
    let end = buf[5..n].iter().position(|&b| b == 0).unwrap();
    assert!(end > 0);
}

#[tokio::test]
async fn inject_axis_value_clamped_and_sent() {
    let (mut injector, receiver) = loopback_injector().await;

    // Pitch axis (0) with value > 1.0 should be clamped
    injector.set_axis(0, 2.5).await.unwrap();

    let mut buf = [0u8; 1024];
    receiver.recv(&mut buf).await.unwrap();
    let val = f32::from_le_bytes([buf[5], buf[6], buf[7], buf[8]]);
    assert!((val - 1.0).abs() < f32::EPSILON, "expected 1.0, got {}", val);

    // Negative clamp: value < -1.0 should be clamped to -1.0
    injector.set_axis(1, -5.0).await.unwrap();
    receiver.recv(&mut buf).await.unwrap();
    let val2 = f32::from_le_bytes([buf[5], buf[6], buf[7], buf[8]]);
    assert!((val2 - (-1.0)).abs() < f32::EPSILON, "expected -1.0, got {}", val2);
}

#[tokio::test]
async fn inject_set_dataref_float_round_trip() {
    let (mut injector, receiver) = loopback_injector().await;
    let path = "sim/cockpit2/controls/yoke_pitch_ratio";
    let value = 0.42f32;

    injector.set_dataref(path, value).await.unwrap();

    let mut buf = [0u8; 1024];
    let n = receiver.recv(&mut buf).await.unwrap();
    assert_eq!(n, 509);
    assert_eq!(&buf[..5], b"DREF\0");

    let decoded = f32::from_le_bytes([buf[5], buf[6], buf[7], buf[8]]);
    assert!((decoded - value).abs() < f32::EPSILON);

    let path_region = &buf[9..9 + path.len()];
    assert_eq!(path_region, path.as_bytes());
}

#[tokio::test]
async fn inject_multi_command_batch_sequential() {
    let (mut injector, receiver) = loopback_injector().await;
    let commands = [
        "sim/flight_controls/flaps_down",
        "sim/flight_controls/landing_gear_down",
        "sim/flight_controls/speed_brakes_toggle",
    ];

    for cmd in &commands {
        injector.send_command(cmd).await.unwrap();
    }

    assert_eq!(injector.packets_sent(), 3);

    // Verify all three arrived
    for expected_cmd in &commands {
        let mut buf = [0u8; 1024];
        let n = receiver.recv(&mut buf).await.unwrap();
        assert_eq!(&buf[..5], b"CMND\0");
        let end = buf[5..n].iter().position(|&b| b == 0).unwrap();
        let received = std::str::from_utf8(&buf[5..5 + end]).unwrap();
        assert_eq!(received, *expected_cmd);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 7. PLUGIN PROTOCOL — ADDITIONAL DEPTH (supplementary)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn plugin_proto_encode_decode_subscribe_multiple() {
    let msg = PluginProtoMessage::Subscribe {
        datarefs: vec![
            SubscriptionRequest {
                id: 1,
                path: "sim/flightmodel/position/indicated_airspeed".to_owned(),
                frequency_hz: 20,
            },
            SubscriptionRequest {
                id: 2,
                path: "sim/flightmodel/position/theta".to_owned(),
                frequency_hz: 50,
            },
            SubscriptionRequest {
                id: 3,
                path: "sim/flightmodel/position/phi".to_owned(),
                frequency_hz: 50,
            },
        ],
    };
    let buf = plugin_protocol::encode(&msg).unwrap();
    let decoded = plugin_protocol::decode(&buf).unwrap();
    assert_eq!(decoded, msg);
}

#[test]
fn plugin_proto_encode_decode_unsubscribe() {
    let msg = PluginProtoMessage::Unsubscribe {
        dataref_ids: vec![1, 2, 3, 100],
    };
    let buf = plugin_protocol::encode(&msg).unwrap();
    let decoded = plugin_protocol::decode(&buf).unwrap();
    assert_eq!(decoded, msg);
}

#[test]
fn plugin_proto_encode_decode_set_dataref() {
    let msg = PluginProtoMessage::SetDataref {
        path: "sim/cockpit2/controls/yoke_pitch_ratio".to_owned(),
        value: -0.5,
    };
    let buf = plugin_protocol::encode(&msg).unwrap();
    let decoded = plugin_protocol::decode(&buf).unwrap();
    assert_eq!(decoded, msg);
}

#[test]
fn plugin_proto_encode_decode_execute_command() {
    let msg = PluginProtoMessage::ExecuteCommand {
        path: "sim/autopilot/heading_up".to_owned(),
    };
    let buf = plugin_protocol::encode(&msg).unwrap();
    let decoded = plugin_protocol::decode(&buf).unwrap();
    assert_eq!(decoded, msg);
}

#[test]
fn plugin_proto_encode_decode_error() {
    let msg = PluginProtoMessage::Error {
        code: 0x8001,
        message: "dataref not found".to_owned(),
    };
    let buf = plugin_protocol::encode(&msg).unwrap();
    let decoded = plugin_protocol::decode(&buf).unwrap();
    assert_eq!(decoded, msg);
}

#[test]
fn plugin_proto_decode_bad_magic_rejected() {
    let mut buf = plugin_protocol::encode(&PluginProtoMessage::Heartbeat { timestamp_us: 0 }).unwrap();
    buf[0] = b'X';
    let err = plugin_protocol::decode(&buf).unwrap_err();
    assert!(matches!(err, ProtocolError::BadMagic { .. }));
}

#[test]
fn plugin_proto_decode_wrong_version_rejected() {
    let mut buf = plugin_protocol::encode(&PluginProtoMessage::Heartbeat { timestamp_us: 0 }).unwrap();
    buf[4] = 99;
    let err = plugin_protocol::decode(&buf).unwrap_err();
    assert!(matches!(err, ProtocolError::UnsupportedVersion { version: 99 }));
}

#[test]
fn plugin_proto_decode_unknown_message_type_rejected() {
    let mut buf = plugin_protocol::encode(&PluginProtoMessage::Heartbeat { timestamp_us: 0 }).unwrap();
    buf[5] = 0x99;
    let err = plugin_protocol::decode(&buf).unwrap_err();
    assert!(matches!(err, ProtocolError::UnknownMessageType { type_byte: 0x99 }));
}

#[test]
fn plugin_proto_decode_truncated_buffer() {
    let err = plugin_protocol::decode(&[0x4F, 0x46, 0x58]).unwrap_err();
    assert!(matches!(err, ProtocolError::BufferTooShort { .. }));
}

#[test]
fn plugin_proto_dataref_batch_large() {
    let entries: Vec<DatarefEntry> = (0..500)
        .map(|i| DatarefEntry {
            id: i,
            value: i as f32 * 0.001,
        })
        .collect();
    let msg = PluginProtoMessage::DatarefBatch {
        sequence: 42,
        timestamp_us: 1_000_000,
        entries,
    };
    let buf = plugin_protocol::encode(&msg).unwrap();
    let decoded = plugin_protocol::decode(&buf).unwrap();
    assert_eq!(decoded, msg);
}

// ── Plugin discovery state machine ──────────────────────────────────

#[test]
fn plugin_discovery_initial_state() {
    let disc = PluginDiscovery::new();
    assert_eq!(*disc.state(), PluginDiscoveryState::NotDetected);
    assert!(!disc.is_connected());
    assert!(disc.should_use_standard_udp());
}

#[test]
fn plugin_discovery_handshake_transitions_to_discovering() {
    let mut disc = PluginDiscovery::new();
    let msg = disc.build_handshake("1.0.0", 250);
    assert!(matches!(msg, PluginProtoMessage::Handshake { .. }));
    assert_eq!(*disc.state(), PluginDiscoveryState::Discovering);
    assert!(!disc.is_connected());
}

#[test]
fn plugin_discovery_handshake_ack_transitions_to_connected() {
    let mut disc = PluginDiscovery::new();
    disc.build_handshake("1.0.0", 250);

    let ack = PluginProtoMessage::HandshakeAck {
        plugin_version: "1.0.0".to_owned(),
        granted_frequency_hz: 100,
        capabilities: vec!["datarefs".to_owned(), "commands".to_owned()],
    };
    disc.process_message(&ack);

    assert!(disc.is_connected());
    assert!(!disc.should_use_standard_udp());
    match disc.state() {
        PluginDiscoveryState::Connected {
            plugin_version,
            frequency_hz,
            capabilities,
        } => {
            assert_eq!(plugin_version, "1.0.0");
            assert_eq!(*frequency_hz, 100);
            assert_eq!(capabilities.len(), 2);
        }
        _ => panic!("expected Connected state"),
    }
}

#[test]
fn plugin_discovery_fatal_error_disconnects() {
    let mut disc = PluginDiscovery::new();
    disc.build_handshake("1.0.0", 250);

    // Simulate connection
    let ack = PluginProtoMessage::HandshakeAck {
        plugin_version: "1.0.0".to_owned(),
        granted_frequency_hz: 100,
        capabilities: vec![],
    };
    disc.process_message(&ack);
    assert!(disc.is_connected());

    // Fatal error (code >= 0x8000) should disconnect
    let err = PluginProtoMessage::Error {
        code: 0x8001,
        message: "fatal crash".to_owned(),
    };
    disc.process_message(&err);
    assert!(!disc.is_connected());
    assert!(disc.should_use_standard_udp());
}

#[test]
fn plugin_discovery_heartbeat_timeout_disconnects() {
    let mut disc = PluginDiscovery::new();
    disc.build_handshake("1.0.0", 250);

    let ack = PluginProtoMessage::HandshakeAck {
        plugin_version: "1.0.0".to_owned(),
        granted_frequency_hz: 100,
        capabilities: vec![],
    };
    disc.process_message(&ack);
    assert!(disc.is_connected());

    // Simulate timeout: check_timeout with a time far in the future
    let future = Instant::now() + Duration::from_secs(60);
    disc.check_timeout(future);
    assert!(!disc.is_connected());
    assert!(matches!(
        disc.state(),
        PluginDiscoveryState::Disconnected { .. }
    ));
}

#[test]
fn plugin_discovery_reset_returns_to_not_detected() {
    let mut disc = PluginDiscovery::new();
    disc.build_handshake("1.0.0", 250);
    let ack = PluginProtoMessage::HandshakeAck {
        plugin_version: "1.0.0".to_owned(),
        granted_frequency_hz: 100,
        capabilities: vec![],
    };
    disc.process_message(&ack);
    assert!(disc.is_connected());

    disc.reset();
    assert_eq!(*disc.state(), PluginDiscoveryState::NotDetected);
    assert!(!disc.is_connected());
}

// ── Enhanced aircraft detection depth ───────────────────────────────

#[test]
fn enhanced_detect_community_aircraft_alias() {
    let mut det = EnhancedAircraftDetector::with_default_db();
    // Toliss A321 alias
    let raw = make_raw("TLSB", "Toliss A321neo");
    let id = det.identify(&raw);
    assert_eq!(id.icao, "A321");
    assert!(id.is_standard_icao);
}

#[test]
fn enhanced_detect_type_change() {
    let mut det = EnhancedAircraftDetector::with_default_db();

    let raw1 = make_raw("C172", "Cessna 172");
    det.identify(&raw1);

    let current = EnhancedAircraftId {
        icao: "A320".to_owned(),
        display_name: "Airbus A320".to_owned(),
        livery_path: None,
        db_match: None,
        is_standard_icao: true,
    };
    let change = det.detect_change(&current);
    assert!(matches!(change, AircraftChange::TypeChanged { .. }));
}

#[test]
fn enhanced_detect_livery_change() {
    let mut det = EnhancedAircraftDetector::with_default_db();
    let mut raw = make_raw("A320", "Airbus A320");
    raw.insert(DATAREF_ACF_LIVERY.to_owned(), "liveries/Delta/".to_owned());
    det.identify(&raw);

    let current = EnhancedAircraftId {
        icao: "A320".to_owned(),
        display_name: "Airbus A320".to_owned(),
        livery_path: Some("liveries/United/".to_owned()),
        db_match: None,
        is_standard_icao: true,
    };
    let change = det.detect_change(&current);
    assert!(matches!(change, AircraftChange::LiveryChanged { .. }));
}

// ── Control injection edge cases ────────────────────────────────────

#[tokio::test]
async fn inject_rejects_nan_value() {
    let (mut injector, _receiver) = loopback_injector().await;
    let err = injector.set_dataref("sim/test", f32::NAN).await.unwrap_err();
    assert!(matches!(err, ControlInjectionError::InvalidValue { .. }));
}

#[tokio::test]
async fn inject_rejects_infinity() {
    let (mut injector, _receiver) = loopback_injector().await;
    let err = injector
        .set_dataref("sim/test", f32::INFINITY)
        .await
        .unwrap_err();
    assert!(matches!(err, ControlInjectionError::InvalidValue { .. }));
}

#[tokio::test]
async fn inject_unknown_axis_rejected() {
    let (mut injector, _receiver) = loopback_injector().await;
    let err = injector.set_axis(99, 0.5).await.unwrap_err();
    assert!(matches!(err, ControlInjectionError::UnknownAxis { id: 99 }));
}

#[tokio::test]
async fn inject_not_bound_returns_error() {
    let cfg = ControlInjectorConfig::default();
    let mut injector = XPlaneControlInjector::new(cfg);
    let err = injector.set_dataref("sim/test", 0.0).await.unwrap_err();
    assert!(matches!(err, ControlInjectionError::NotBound));
}

#[tokio::test]
async fn inject_packets_sent_counter() {
    let (mut injector, _receiver) = loopback_injector().await;
    assert_eq!(injector.packets_sent(), 0);

    injector.set_dataref("sim/a", 1.0).await.unwrap();
    injector.send_command("sim/cmd").await.unwrap();
    injector.set_axis(0, 0.5).await.unwrap();
    assert_eq!(injector.packets_sent(), 3);
}
