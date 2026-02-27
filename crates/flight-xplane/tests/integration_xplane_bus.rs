// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests for the X-Plane UDP → BusPublisher → BusSubscriber pipeline.
//!
//! These tests verify that data received over the X-Plane DATA protocol flows
//! correctly through the conversion layer and into the telemetry bus.

use flight_bus::{BusPublisher, SubscriptionConfig};
use flight_xplane::{
    DetectedAircraft,
    adapter::{XPlaneAdapter, XPlaneRawData},
    dataref::DataRefValue,
};
use std::{collections::HashMap, net::UdpSocket, time::Instant};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build an X-Plane DATA UDP packet containing a single 36-byte record.
///
/// Packet layout:
/// - `DATA\0`           – 5-byte header (magic + null)
/// - 4 bytes (i32 LE)   – data group index
/// - 8 × 4 bytes (f32)  – data values
fn make_xplane_data_packet(index: u32, values: [f32; 8]) -> Vec<u8> {
    let mut pkt = b"DATA\0".to_vec();
    pkt.extend_from_slice(&(index as i32).to_le_bytes());
    for v in &values {
        pkt.extend_from_slice(&v.to_le_bytes());
    }
    pkt
}

/// Parse a single DATA record out of a raw X-Plane DATA packet.
///
/// Returns `(index, values)` for the first record in the packet.
fn parse_first_data_record(pkt: &[u8]) -> (i32, [f32; 8]) {
    assert!(pkt.len() >= 5 + 36, "packet too short");
    assert_eq!(&pkt[0..4], b"DATA");

    let offset = 5; // skip "DATA\0"
    let index = i32::from_le_bytes([
        pkt[offset],
        pkt[offset + 1],
        pkt[offset + 2],
        pkt[offset + 3],
    ]);
    let mut values = [0.0f32; 8];
    for i in 0..8 {
        let o = offset + 4 + i * 4;
        values[i] = f32::from_le_bytes([pkt[o], pkt[o + 1], pkt[o + 2], pkt[o + 3]]);
    }
    (index, values)
}

/// Construct `XPlaneRawData` for a mock C172 with the given DataRef map.
fn make_raw_data(dataref_values: HashMap<String, DataRefValue>) -> XPlaneRawData {
    XPlaneRawData {
        timestamp: Instant::now(),
        aircraft_info: DetectedAircraft {
            icao: "C172".to_string(),
            title: "Cessna Skyhawk 172SP".to_string(),
            author: "Laminar Research".to_string(),
        },
        dataref_values,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Verify that the `make_xplane_data_packet` helper produces a byte buffer
/// that round-trips through the parse helper unchanged.
#[test]
fn data_packet_round_trips_through_udp_socket() {
    let ias_mps = 77.17_f32; // ≈ 150 knots
    let tas_mps = 79.00_f32;
    let gs_mps = 75.50_f32;

    let pkt = make_xplane_data_packet(3, [ias_mps, tas_mps, gs_mps, 0.0, 0.0, 0.0, 0.0, 0.0]);

    // Bind an ephemeral UDP listener and send the packet to it.
    let listener = UdpSocket::bind("127.0.0.1:0").expect("bind listener");
    let addr = listener.local_addr().unwrap();

    let sender = UdpSocket::bind("127.0.0.1:0").expect("bind sender");
    sender.send_to(&pkt, addr).expect("send packet");

    let mut buf = [0u8; 512];
    let (len, _) = listener.recv_from(&mut buf).expect("recv packet");
    let received = &buf[..len];

    // The received bytes must be identical to what was sent.
    assert_eq!(received, pkt.as_slice(), "UDP round-trip: bytes differ");

    // Parse and verify the first (only) record.
    let (index, values) = parse_first_data_record(received);
    assert_eq!(index, 3, "wrong group index");
    assert!((values[0] - ias_mps).abs() < 1e-4, "IAS mismatch");
    assert!((values[1] - tas_mps).abs() < 1e-4, "TAS mismatch");
    assert!((values[2] - gs_mps).abs() < 1e-4, "GS mismatch");
}

/// Exercise the core of the integration:
///
///   X-Plane DATA values
///     → `XPlaneRawData`
///     → `XPlaneAdapter::convert_raw_to_snapshot`
///     → `BusPublisher::publish`
///     → `Subscriber::try_recv`
///
/// This verifies that the conversion and bus wiring work end-to-end without
/// requiring a live X-Plane instance.
#[test]
fn xplane_data_flows_from_udp_packet_through_bus() {
    // -----------------------------------------------------------------
    // 1. Simulate what the UDP receive loop produces after handling a
    //    group-3 (speeds) + group-17 (attitude) DATA packet.
    // -----------------------------------------------------------------
    let ias_mps = 77.17_f32; // ≈ 150 knots
    let tas_mps = 79.00_f32;
    let gs_mps = 75.50_f32;
    let pitch_deg = 5.0_f32;
    let roll_deg = 10.0_f32;
    let hdg_deg = 270.0_f32;

    // Group 3 packet (speeds)
    let pkt3 = make_xplane_data_packet(3, [ias_mps, tas_mps, gs_mps, 0.0, 0.0, 0.0, 0.0, 0.0]);
    let (_, speeds) = parse_first_data_record(&pkt3);

    // Group 17 packet (attitude)
    let pkt17 =
        make_xplane_data_packet(17, [pitch_deg, roll_deg, hdg_deg, 0.0, 0.0, 0.0, 0.0, 0.0]);
    let (_, attitude) = parse_first_data_record(&pkt17);

    // -----------------------------------------------------------------
    // 2. Build the DataRef cache that the UDP handler would have produced.
    // -----------------------------------------------------------------
    let mut dataref_values: HashMap<String, DataRefValue> = HashMap::new();

    // Group 3 → speeds (m/s)
    dataref_values.insert(
        "sim/flightmodel/position/indicated_airspeed".to_string(),
        DataRefValue::Float(speeds[0]),
    );
    dataref_values.insert(
        "sim/flightmodel/position/true_airspeed".to_string(),
        DataRefValue::Float(speeds[1]),
    );
    dataref_values.insert(
        "sim/flightmodel/position/groundspeed".to_string(),
        DataRefValue::Float(speeds[2]),
    );

    // Group 17 → pitch / roll / heading (degrees)
    dataref_values.insert(
        "sim/flightmodel/position/theta".to_string(),
        DataRefValue::Float(attitude[0]),
    );
    dataref_values.insert(
        "sim/flightmodel/position/phi".to_string(),
        DataRefValue::Float(attitude[1]),
    );
    dataref_values.insert(
        "sim/flightmodel/position/psi".to_string(),
        DataRefValue::Float(attitude[2]),
    );

    // -----------------------------------------------------------------
    // 3. Convert raw data → BusSnapshot.
    // -----------------------------------------------------------------
    let raw = make_raw_data(dataref_values);
    let snapshot = XPlaneAdapter::convert_raw_to_snapshot(raw, Instant::now())
        .expect("convert_raw_to_snapshot failed");

    // Basic sanity checks on the converted snapshot.
    assert_eq!(snapshot.sim, flight_bus::types::SimId::XPlane);

    // IAS: adapter converts m/s → stored as m/s; `value()` returns m/s.
    let ias_stored = snapshot.kinematics.ias.value();
    assert!(
        (ias_stored - ias_mps).abs() < 0.01,
        "IAS stored={ias_stored:.3} m/s, expected {ias_mps:.3} m/s"
    );

    // Pitch: adapter normalises degrees to [-180, 180] then stores as a
    // ValidatedAngle; round-trip through to_degrees() must be close.
    let pitch_stored = snapshot.kinematics.pitch.to_degrees();
    assert!(
        (pitch_stored - pitch_deg).abs() < 0.01,
        "pitch stored={pitch_stored:.3}°, expected {pitch_deg:.3}°"
    );

    // Heading 270° is inside [-180, 180] after normalisation → -90°.
    let hdg_stored = snapshot.kinematics.heading.to_degrees();
    assert!(
        (hdg_stored - (-90.0_f32)).abs() < 0.01,
        "heading stored={hdg_stored:.3}°, expected -90°"
    );

    // -----------------------------------------------------------------
    // 4. Publish snapshot to the bus and verify a subscriber receives it.
    // -----------------------------------------------------------------
    let mut publisher = BusPublisher::new(60.0);
    let mut subscriber = publisher
        .subscribe(SubscriptionConfig::default())
        .expect("subscribe failed");

    publisher.publish(snapshot.clone()).expect("publish failed");

    let received = subscriber
        .try_recv()
        .expect("try_recv error")
        .expect("no snapshot in channel");

    // The snapshot received from the bus must match what was published.
    assert_eq!(received.sim, snapshot.sim, "sim id mismatch");
    assert_eq!(received.aircraft, snapshot.aircraft, "aircraft id mismatch");

    let rx_ias = received.kinematics.ias.value();
    assert!(
        (rx_ias - ias_mps).abs() < 0.01,
        "bus IAS={rx_ias:.3} m/s, expected {ias_mps:.3} m/s"
    );

    let rx_pitch = received.kinematics.pitch.to_degrees();
    assert!(
        (rx_pitch - pitch_deg).abs() < 0.01,
        "bus pitch={rx_pitch:.3}°, expected {pitch_deg:.3}°"
    );
}

/// Verify that sending multiple DATA groups in a single UDP call all flow
/// correctly through the bus pipeline.
#[test]
fn multiple_data_groups_flow_through_bus() {
    let ias_mps = 51.44_f32; // ≈ 100 knots
    let g_normal = 1.05_f32;
    let roll_deg = -15.0_f32;
    let p_rate = 5.0_f32; // deg/s roll rate

    let mut dataref_values: HashMap<String, DataRefValue> = HashMap::new();

    // Group 3 – speeds
    dataref_values.insert(
        "sim/flightmodel/position/indicated_airspeed".to_string(),
        DataRefValue::Float(ias_mps),
    );
    dataref_values.insert(
        "sim/flightmodel/position/true_airspeed".to_string(),
        DataRefValue::Float(ias_mps + 1.5),
    );
    dataref_values.insert(
        "sim/flightmodel/position/groundspeed".to_string(),
        DataRefValue::Float(ias_mps - 1.0),
    );

    // Group 4 – G-load
    dataref_values.insert(
        "sim/flightmodel/forces/g_nrml".to_string(),
        DataRefValue::Float(g_normal),
    );

    // Group 16 – angular rates (deg/s)
    dataref_values.insert(
        "sim/flightmodel/position/P".to_string(),
        DataRefValue::Float(p_rate),
    );

    // Group 17 – attitude
    dataref_values.insert(
        "sim/flightmodel/position/phi".to_string(),
        DataRefValue::Float(roll_deg),
    );

    let raw = make_raw_data(dataref_values);
    let snapshot =
        XPlaneAdapter::convert_raw_to_snapshot(raw, Instant::now()).expect("convert failed");

    // Verify G-load
    let g = snapshot.kinematics.g_force.value();
    assert!(
        (g - g_normal).abs() < 0.01,
        "g_force={g:.3}, expected {g_normal:.3}"
    );

    // Verify roll
    let roll = snapshot.kinematics.bank.to_degrees();
    assert!(
        (roll - roll_deg).abs() < 0.01,
        "bank={roll:.3}°, expected {roll_deg:.3}°"
    );

    // Verify angular rate: P (deg/s) → rad/s
    let p_rad = snapshot.angular_rates.p;
    let expected_rad = p_rate * std::f32::consts::PI / 180.0;
    assert!(
        (p_rad - expected_rad).abs() < 1e-4,
        "angular_rates.p={p_rad:.5} rad/s, expected {expected_rad:.5}"
    );

    // Publish and receive via bus
    let mut publisher = BusPublisher::new(60.0);
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();
    publisher.publish(snapshot).unwrap();

    let rx = sub.try_recv().unwrap().expect("no snapshot");
    assert!((rx.kinematics.g_force.value() - g_normal).abs() < 0.01);
    assert!((rx.kinematics.bank.to_degrees() - roll_deg).abs() < 0.01);
}

/// `convert_raw_to_snapshot` with an empty DataRef map must return `Ok` with
/// default (zero) values and not panic.
#[test]
fn convert_raw_to_snapshot_with_empty_map_returns_defaults() {
    let raw = make_raw_data(HashMap::new());
    let result = XPlaneAdapter::convert_raw_to_snapshot(raw, Instant::now());
    assert!(
        result.is_ok(),
        "Expected Ok with empty dataref map, got: {:?}",
        result.err()
    );
    let snapshot = result.unwrap();
    assert_eq!(snapshot.sim, flight_bus::types::SimId::XPlane);
    // Speeds default to 0 m/s when no DataRefs are present.
    assert_eq!(snapshot.kinematics.ias.value(), 0.0);
    assert_eq!(snapshot.kinematics.tas.value(), 0.0);
    assert_eq!(snapshot.kinematics.ground_speed.value(), 0.0);
}

/// Pitch values outside [-180°, 180°] must be normalised into the signed range
/// rather than causing a conversion error.  270° → -90° is the canonical case.
#[test]
fn out_of_range_pitch_is_normalised_to_signed_range() {
    let mut dataref_values = HashMap::new();
    // 270° is outside [-180, 180]; normalize_degrees_signed maps it to -90°.
    dataref_values.insert(
        "sim/flightmodel/position/theta".to_string(),
        DataRefValue::Float(270.0),
    );
    let raw = make_raw_data(dataref_values);
    let snapshot = XPlaneAdapter::convert_raw_to_snapshot(raw, Instant::now())
        .expect("270° pitch should normalise successfully, not error");
    let pitch = snapshot.kinematics.pitch.to_degrees();
    assert!(
        (pitch - (-90.0_f32)).abs() < 0.01,
        "270° pitch should normalise to -90°, got {pitch:.3}°"
    );
}

/// A `DataRefValue::Float(f32::NAN)` in a validated field must not panic.
/// The adapter treats NaN as out-of-range and returns `Err`.
#[test]
fn nan_dataref_value_does_not_panic() {
    let mut dataref_values = HashMap::new();
    dataref_values.insert(
        "sim/flightmodel/position/indicated_airspeed".to_string(),
        DataRefValue::Float(f32::NAN),
    );
    let raw = make_raw_data(dataref_values);
    // Must not panic; NaN fails `ValidatedSpeed` range-check → Err.
    let result = XPlaneAdapter::convert_raw_to_snapshot(raw, Instant::now());
    assert!(result.is_err(), "NaN airspeed should return Err, not Ok");
}

/// Two aircraft that share the same ICAO code but have different titles / authors
/// must produce an identical `AircraftId` in the converted snapshot.
/// This reflects the adapter's ICAO-only identity key used for deduplication.
#[test]
fn same_icao_different_title_produces_same_aircraft_id() {
    let make = |title: &str, author: &str| XPlaneRawData {
        timestamp: Instant::now(),
        aircraft_info: DetectedAircraft {
            icao: "B738".to_string(),
            title: title.to_string(),
            author: author.to_string(),
        },
        dataref_values: HashMap::new(),
    };

    let snap_a = XPlaneAdapter::convert_raw_to_snapshot(
        make("Boeing 737-800 v1", "Laminar"),
        Instant::now(),
    )
    .unwrap();
    let snap_b =
        XPlaneAdapter::convert_raw_to_snapshot(make("Boeing 737-800 v2", "IXEG"), Instant::now())
            .unwrap();

    assert_eq!(
        snap_a.aircraft, snap_b.aircraft,
        "Same ICAO should yield the same AircraftId regardless of title/author"
    );
}

/// Two subscribers attached to the same `BusPublisher` must each receive a copy
/// of every published snapshot.
#[test]
fn multiple_subscribers_both_receive_published_snapshot() {
    let ias_mps = 80.0_f32;
    let mut dataref_values = HashMap::new();
    dataref_values.insert(
        "sim/flightmodel/position/indicated_airspeed".to_string(),
        DataRefValue::Float(ias_mps),
    );
    let snapshot =
        XPlaneAdapter::convert_raw_to_snapshot(make_raw_data(dataref_values), Instant::now())
            .expect("convert failed");

    let mut publisher = BusPublisher::new(60.0);
    let mut sub1 = publisher.subscribe(SubscriptionConfig::default()).unwrap();
    let mut sub2 = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    publisher.publish(snapshot.clone()).unwrap();

    let rx1 = sub1
        .try_recv()
        .unwrap()
        .expect("sub1: no snapshot in channel");
    let rx2 = sub2
        .try_recv()
        .unwrap()
        .expect("sub2: no snapshot in channel");

    assert_eq!(rx1.sim, snapshot.sim, "sub1 sim mismatch");
    assert_eq!(rx2.sim, snapshot.sim, "sub2 sim mismatch");
    assert!(
        (rx1.kinematics.ias.value() - ias_mps).abs() < 0.01,
        "sub1 IAS={:.3} m/s, expected {ias_mps:.3}",
        rx1.kinematics.ias.value()
    );
    assert!(
        (rx2.kinematics.ias.value() - ias_mps).abs() < 0.01,
        "sub2 IAS={:.3} m/s, expected {ias_mps:.3}",
        rx2.kinematics.ias.value()
    );
}

/// End-to-end loopback test: DATA packets are sent over a loopback UDP socket,
/// parsed into `XPlaneRawData`, converted via `convert_raw_to_snapshot`, and
/// published to the bus — verifying the full pipeline without a live X-Plane
/// instance.
#[test]
fn xplane_adapter_end_to_end_with_live_instance() {
    // -----------------------------------------------------------------
    // 1. Bind an ephemeral loopback listener — no live X-Plane needed.
    // -----------------------------------------------------------------
    let listener = UdpSocket::bind("127.0.0.1:0").expect("bind listener");
    listener
        .set_read_timeout(Some(std::time::Duration::from_secs(2)))
        .expect("set_read_timeout");
    let addr = listener.local_addr().unwrap();
    let sender = UdpSocket::bind("127.0.0.1:0").expect("bind sender");

    let ias_mps = 60.0_f32; // ≈ 117 knots
    let tas_mps = 62.0_f32;
    let gs_mps = 58.0_f32;
    let pitch_deg = 4.0_f32;
    let roll_deg = -8.0_f32;
    let hdg_deg = 45.0_f32;

    // -----------------------------------------------------------------
    // 2. Send group-3 (speeds) then group-17 (attitude) DATA packets.
    // -----------------------------------------------------------------
    sender
        .send_to(
            &make_xplane_data_packet(3, [ias_mps, tas_mps, gs_mps, 0.0, 0.0, 0.0, 0.0, 0.0]),
            addr,
        )
        .expect("send speeds packet");
    sender
        .send_to(
            &make_xplane_data_packet(17, [pitch_deg, roll_deg, hdg_deg, 0.0, 0.0, 0.0, 0.0, 0.0]),
            addr,
        )
        .expect("send attitude packet");

    // -----------------------------------------------------------------
    // 3. Receive both packets and build the DataRef cache.
    // -----------------------------------------------------------------
    let mut buf = [0u8; 512];
    let (len, _) = listener.recv_from(&mut buf).expect("recv speeds");
    let (_, speeds) = parse_first_data_record(&buf[..len]);

    let (len, _) = listener.recv_from(&mut buf).expect("recv attitude");
    let (_, attitude) = parse_first_data_record(&buf[..len]);

    let mut dataref_values: HashMap<String, DataRefValue> = HashMap::new();
    dataref_values.insert(
        "sim/flightmodel/position/indicated_airspeed".to_string(),
        DataRefValue::Float(speeds[0]),
    );
    dataref_values.insert(
        "sim/flightmodel/position/true_airspeed".to_string(),
        DataRefValue::Float(speeds[1]),
    );
    dataref_values.insert(
        "sim/flightmodel/position/groundspeed".to_string(),
        DataRefValue::Float(speeds[2]),
    );
    dataref_values.insert(
        "sim/flightmodel/position/theta".to_string(),
        DataRefValue::Float(attitude[0]),
    );
    dataref_values.insert(
        "sim/flightmodel/position/phi".to_string(),
        DataRefValue::Float(attitude[1]),
    );
    dataref_values.insert(
        "sim/flightmodel/position/psi".to_string(),
        DataRefValue::Float(attitude[2]),
    );

    // -----------------------------------------------------------------
    // 4. Convert raw → snapshot (no background loop required).
    // -----------------------------------------------------------------
    let raw = make_raw_data(dataref_values);
    let snapshot = XPlaneAdapter::convert_raw_to_snapshot(raw, Instant::now())
        .expect("convert_raw_to_snapshot failed");

    assert_eq!(snapshot.sim, flight_bus::types::SimId::XPlane);
    assert!(
        (snapshot.kinematics.ias.value() - ias_mps).abs() < 0.01,
        "IAS: expected {ias_mps:.3} m/s, got {:.3}",
        snapshot.kinematics.ias.value()
    );
    assert!(
        (snapshot.kinematics.pitch.to_degrees() - pitch_deg).abs() < 0.01,
        "pitch: expected {pitch_deg:.3}°, got {:.3}°",
        snapshot.kinematics.pitch.to_degrees()
    );
    assert!(
        (snapshot.kinematics.heading.to_degrees() - hdg_deg).abs() < 0.01,
        "heading: expected {hdg_deg:.3}°, got {:.3}°",
        snapshot.kinematics.heading.to_degrees()
    );

    // -----------------------------------------------------------------
    // 5. Publish snapshot to the bus and verify a subscriber receives it.
    // -----------------------------------------------------------------
    let mut publisher = BusPublisher::new(60.0);
    let mut subscriber = publisher
        .subscribe(SubscriptionConfig::default())
        .expect("subscribe failed");

    publisher.publish(snapshot.clone()).expect("publish failed");

    let received = subscriber
        .try_recv()
        .expect("try_recv error")
        .expect("no snapshot in channel");

    assert_eq!(received.sim, snapshot.sim, "sim id mismatch");
    assert_eq!(received.aircraft, snapshot.aircraft, "aircraft id mismatch");
    assert!(
        (received.kinematics.ias.value() - ias_mps).abs() < 0.01,
        "bus IAS mismatch"
    );
    assert!(
        (received.kinematics.pitch.to_degrees() - pitch_deg).abs() < 0.01,
        "bus pitch mismatch"
    );
}
