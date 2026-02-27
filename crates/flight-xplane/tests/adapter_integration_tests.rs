// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests for [`XPlaneAdapter`] state machine, timeout detection,
//! bus publishing, and raw-data validation.
//!
//! None of these tests require a live X-Plane instance.

use flight_adapter_common::AdapterState;
use flight_bus::{BusPublisher, SubscriptionConfig, adapters::SimAdapter};
use flight_xplane::{
    DetectedAircraft,
    adapter::{XPlaneAdapter, XPlaneAdapterConfig, XPlaneRawData},
    dataref::DataRefValue,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Instant,
};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn make_publisher() -> Arc<Mutex<BusPublisher>> {
    Arc::new(Mutex::new(BusPublisher::new(60.0)))
}

fn make_adapter() -> XPlaneAdapter {
    XPlaneAdapter::new(XPlaneAdapterConfig::default(), make_publisher())
        .expect("adapter creation must succeed")
}

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

fn make_critical_datarefs() -> HashMap<String, DataRefValue> {
    let mut map = HashMap::new();
    map.insert(
        "sim/flightmodel/position/indicated_airspeed".to_string(),
        DataRefValue::Float(50.0),
    );
    map.insert(
        "sim/flightmodel/position/latitude".to_string(),
        DataRefValue::Double(37.0),
    );
    map.insert(
        "sim/flightmodel/position/longitude".to_string(),
        DataRefValue::Double(-122.0),
    );
    map
}

// ---------------------------------------------------------------------------
// 1. State machine — initial state
// ---------------------------------------------------------------------------

/// A freshly created adapter starts in the `Disconnected` state.
#[tokio::test]
async fn new_adapter_starts_in_disconnected_state() {
    let adapter = make_adapter();
    assert_eq!(adapter.state(), AdapterState::Disconnected);
}

/// A freshly created adapter is not running.
#[tokio::test]
async fn new_adapter_is_not_running() {
    let adapter = make_adapter();
    assert!(!adapter.is_running());
}

/// A freshly created adapter has no current aircraft.
#[tokio::test]
async fn new_adapter_has_no_current_aircraft() {
    let adapter = make_adapter();
    assert!(adapter.get_current_aircraft().is_none());
}

/// The adapter reports itself as `XPlane` via the `SimAdapter` trait.
#[tokio::test]
async fn adapter_reports_xplane_sim_id() {
    use flight_bus::types::SimId;
    let adapter = make_adapter();
    assert_eq!(adapter.sim_id(), SimId::XPlane);
}

// ---------------------------------------------------------------------------
// 2. State machine — stop
// ---------------------------------------------------------------------------

/// `stop()` from `Disconnected` is a safe no-op and keeps the state as
/// `Disconnected`.
#[tokio::test]
async fn stop_from_disconnected_is_noop() {
    let adapter = make_adapter();
    assert!(adapter.stop().await.is_ok());
    assert_eq!(adapter.state(), AdapterState::Disconnected);
    assert!(!adapter.is_running());
}

/// Calling `stop()` twice is idempotent.
#[tokio::test]
async fn stop_twice_is_idempotent() {
    let adapter = make_adapter();
    assert!(adapter.stop().await.is_ok());
    assert!(adapter.stop().await.is_ok());
    assert_eq!(adapter.state(), AdapterState::Disconnected);
}

// ---------------------------------------------------------------------------
// 3. Timeout / stale detection
// ---------------------------------------------------------------------------

/// `is_connection_timeout()` returns `true` when no packet has ever been
/// received (the `last_packet_time` is `None`).
///
/// Requirement: XPLANE-INT-01.13 — no packets for 2 s → mark snapshot invalid.
#[tokio::test]
async fn timeout_is_true_when_no_packet_ever_received() {
    let adapter = make_adapter();
    assert!(
        adapter.is_connection_timeout(),
        "adapter with no packets received must report a timeout"
    );
}

/// `time_since_last_packet()` returns `None` before any packet arrives.
#[tokio::test]
async fn time_since_last_packet_is_none_initially() {
    let adapter = make_adapter();
    assert!(
        adapter.time_since_last_packet().is_none(),
        "no packet received yet → time_since_last_packet must be None"
    );
}

// ---------------------------------------------------------------------------
// 4. Bus publishing — publish_snapshot
// ---------------------------------------------------------------------------

/// `publish_snapshot` delivers a snapshot to every subscriber.
#[tokio::test]
async fn publish_snapshot_delivers_to_subscriber() {
    use flight_bus::{
        snapshot::BusSnapshot,
        types::{AircraftId, SimId},
    };

    let publisher = make_publisher();
    let mut subscriber = publisher
        .lock()
        .unwrap()
        .subscribe(SubscriptionConfig::default())
        .expect("subscribe");

    let adapter =
        XPlaneAdapter::new(XPlaneAdapterConfig::default(), publisher).expect("create adapter");

    let snapshot = BusSnapshot::new(SimId::XPlane, AircraftId::new("C172"));
    adapter.publish_snapshot(snapshot.clone()).expect("publish");

    let received = subscriber
        .try_recv()
        .expect("try_recv error")
        .expect("no snapshot in channel");

    assert_eq!(received.sim, SimId::XPlane);
    assert_eq!(received.aircraft.icao, "C172");
}

/// `publish_snapshot` delivers to multiple subscribers independently.
#[tokio::test]
async fn publish_snapshot_delivers_to_multiple_subscribers() {
    use flight_bus::{
        snapshot::BusSnapshot,
        types::{AircraftId, SimId},
    };

    let publisher = make_publisher();
    let mut sub1 = publisher
        .lock()
        .unwrap()
        .subscribe(SubscriptionConfig::default())
        .expect("sub1");
    let mut sub2 = publisher
        .lock()
        .unwrap()
        .subscribe(SubscriptionConfig::default())
        .expect("sub2");

    let adapter =
        XPlaneAdapter::new(XPlaneAdapterConfig::default(), publisher).expect("create adapter");

    let snapshot = BusSnapshot::new(SimId::XPlane, AircraftId::new("B738"));
    adapter.publish_snapshot(snapshot).expect("publish");

    let rx1 = sub1.try_recv().unwrap().expect("sub1: no snapshot");
    let rx2 = sub2.try_recv().unwrap().expect("sub2: no snapshot");

    assert_eq!(rx1.aircraft.icao, "B738");
    assert_eq!(rx2.aircraft.icao, "B738");
}

// ---------------------------------------------------------------------------
// 5. Bus publishing — publish_stale_snapshot
// ---------------------------------------------------------------------------

/// `publish_stale_snapshot` sends a snapshot with **all** validity flags set
/// to `false`, signalling that data is no longer safe for FFB or display.
///
/// Requirement: XPLANE-INT-01.13 — timeout must produce an invalid snapshot.
#[tokio::test]
async fn publish_stale_snapshot_clears_all_validity_flags() {
    let publisher = make_publisher();
    let mut subscriber = publisher
        .lock()
        .unwrap()
        .subscribe(SubscriptionConfig::default())
        .expect("subscribe");

    let adapter =
        XPlaneAdapter::new(XPlaneAdapterConfig::default(), publisher).expect("create adapter");
    adapter.publish_stale_snapshot().expect("publish stale");

    let received = subscriber
        .try_recv()
        .expect("try_recv error")
        .expect("no stale snapshot");

    assert!(
        !received.validity.attitude_valid,
        "stale: attitude_valid must be false"
    );
    assert!(
        !received.validity.velocities_valid,
        "stale: velocities_valid must be false"
    );
    assert!(
        !received.validity.kinematics_valid,
        "stale: kinematics_valid must be false"
    );
    assert!(
        !received.validity.position_valid,
        "stale: position_valid must be false"
    );
    assert!(
        !received.validity.aero_valid,
        "stale: aero_valid must be false"
    );
}

// ---------------------------------------------------------------------------
// 6. validate_raw_data — error cases
// ---------------------------------------------------------------------------

/// `validate_raw_data` rejects a completely empty DataRef map.
#[tokio::test]
async fn validate_raw_data_rejects_empty_map() {
    let adapter = make_adapter();
    let raw = make_raw_data(HashMap::new());
    assert!(
        adapter.validate_raw_data(&raw).is_err(),
        "empty DataRef map must be rejected"
    );
}

/// `validate_raw_data` rejects a map that is missing the IAS critical DataRef.
#[tokio::test]
async fn validate_raw_data_rejects_missing_ias_critical_dataref() {
    let adapter = make_adapter();
    // Has lat/lon but no IAS → must fail.
    let mut datarefs = HashMap::new();
    datarefs.insert(
        "sim/flightmodel/position/latitude".to_string(),
        DataRefValue::Double(37.0),
    );
    datarefs.insert(
        "sim/flightmodel/position/longitude".to_string(),
        DataRefValue::Double(-122.0),
    );
    let raw = make_raw_data(datarefs);
    assert!(
        adapter.validate_raw_data(&raw).is_err(),
        "missing IAS critical DataRef must be rejected"
    );
}

/// `validate_raw_data` rejects a map that is missing the lat/lon critical DataRefs.
#[tokio::test]
async fn validate_raw_data_rejects_missing_position_critical_datarefs() {
    let adapter = make_adapter();
    // Has IAS but no position → must fail.
    let mut datarefs = HashMap::new();
    datarefs.insert(
        "sim/flightmodel/position/indicated_airspeed".to_string(),
        DataRefValue::Float(50.0),
    );
    let raw = make_raw_data(datarefs);
    assert!(
        adapter.validate_raw_data(&raw).is_err(),
        "missing lat/lon critical DataRefs must be rejected"
    );
}

/// `validate_raw_data` accepts a map that contains all three critical DataRefs.
#[tokio::test]
async fn validate_raw_data_accepts_all_critical_datarefs_present() {
    let adapter = make_adapter();
    let raw = make_raw_data(make_critical_datarefs());
    assert!(
        adapter.validate_raw_data(&raw).is_ok(),
        "map with all critical DataRefs must pass validation"
    );
}

/// `validate_raw_data` accepts a map that adds extra non-critical DataRefs.
#[tokio::test]
async fn validate_raw_data_accepts_extra_datarefs() {
    let adapter = make_adapter();
    let mut datarefs = make_critical_datarefs();
    datarefs.insert(
        "sim/flightmodel/position/theta".to_string(),
        DataRefValue::Float(5.0),
    );
    datarefs.insert(
        "sim/flightmodel/position/phi".to_string(),
        DataRefValue::Float(-10.0),
    );
    let raw = make_raw_data(datarefs);
    assert!(
        adapter.validate_raw_data(&raw).is_ok(),
        "extra non-critical DataRefs must not cause validation failure"
    );
}

// ---------------------------------------------------------------------------
// 7. Config defaults
// ---------------------------------------------------------------------------

/// The default publish rate is 30 Hz.
#[test]
fn default_config_publish_rate_is_30hz() {
    let cfg = XPlaneAdapterConfig::default();
    assert_eq!(cfg.publish_rate_hz, 30);
}

/// The default latency budget is 50 ms.
#[test]
fn default_config_latency_budget_is_50ms() {
    let cfg = XPlaneAdapterConfig::default();
    assert_eq!(cfg.latency_budget_ms, 50);
}

/// The plugin interface is disabled by default.
#[test]
fn default_config_plugin_disabled() {
    let cfg = XPlaneAdapterConfig::default();
    assert!(!cfg.enable_plugin);
}
