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

// ---------------------------------------------------------------------------
// 8. convert_raw_to_snapshot — autopilot state extraction
// ---------------------------------------------------------------------------

/// Helper: critical datarefs plus the supplied extras, converted to a snapshot.
fn snapshot_from(
    extra: impl IntoIterator<Item = (String, DataRefValue)>,
) -> flight_bus::snapshot::BusSnapshot {
    let mut datarefs = make_critical_datarefs();
    datarefs.extend(extra);
    let raw = make_raw_data(datarefs);
    XPlaneAdapter::convert_raw_to_snapshot(raw, std::time::Instant::now())
        .expect("snapshot conversion must succeed")
}

/// Autopilot mode 0 → `AutopilotState::Off`.
#[test]
fn convert_snapshot_autopilot_mode_off() {
    use flight_bus::types::AutopilotState;
    let snapshot = snapshot_from([(
        "sim/cockpit/autopilot/autopilot_mode".to_string(),
        DataRefValue::Int(0),
    )]);
    assert_eq!(snapshot.config.ap_state, AutopilotState::Off);
}

/// Autopilot mode 1 → `AutopilotState::Armed`.
#[test]
fn convert_snapshot_autopilot_mode_armed() {
    use flight_bus::types::AutopilotState;
    let snapshot = snapshot_from([(
        "sim/cockpit/autopilot/autopilot_mode".to_string(),
        DataRefValue::Int(1),
    )]);
    assert_eq!(snapshot.config.ap_state, AutopilotState::Armed);
}

/// Autopilot mode ≥ 2 → `AutopilotState::Engaged`.
#[test]
fn convert_snapshot_autopilot_mode_engaged() {
    use flight_bus::types::AutopilotState;
    let snapshot = snapshot_from([(
        "sim/cockpit/autopilot/autopilot_mode".to_string(),
        DataRefValue::Int(2),
    )]);
    assert_eq!(snapshot.config.ap_state, AutopilotState::Engaged);
}

// ---------------------------------------------------------------------------
// 9. convert_raw_to_snapshot — flaps position
// ---------------------------------------------------------------------------

/// Flap deploy ratio 0.5 → 50 % flaps.
#[test]
fn convert_snapshot_flaps_half_deployed() {
    let snapshot = snapshot_from([(
        "sim/aircraft/parts/acf_flap_deploy".to_string(),
        DataRefValue::Float(0.5),
    )]);
    let pct = snapshot.config.flaps.value();
    assert!((pct - 50.0).abs() < 0.1, "expected 50 % flaps, got {pct}");
}

/// Flap deploy ratio 1.0 → 100 % flaps.
#[test]
fn convert_snapshot_flaps_fully_deployed() {
    let snapshot = snapshot_from([(
        "sim/aircraft/parts/acf_flap_deploy".to_string(),
        DataRefValue::Float(1.0),
    )]);
    let pct = snapshot.config.flaps.value();
    assert!((pct - 100.0).abs() < 0.1, "expected 100 % flaps, got {pct}");
}

// ---------------------------------------------------------------------------
// 10. convert_raw_to_snapshot — landing gear state
// ---------------------------------------------------------------------------

/// Gear deploy value > 0.9 → `GearPosition::Down` on all three legs.
#[test]
fn convert_snapshot_gear_down() {
    use flight_bus::types::GearPosition;
    let snapshot = snapshot_from([(
        "sim/aircraft/parts/acf_gear_deploy".to_string(),
        DataRefValue::Float(1.0),
    )]);
    assert_eq!(snapshot.config.gear.nose, GearPosition::Down);
    assert_eq!(snapshot.config.gear.left, GearPosition::Down);
    assert_eq!(snapshot.config.gear.right, GearPosition::Down);
}

/// Gear deploy value < 0.1 → `GearPosition::Up` on all three legs.
#[test]
fn convert_snapshot_gear_up() {
    use flight_bus::types::GearPosition;
    let snapshot = snapshot_from([(
        "sim/aircraft/parts/acf_gear_deploy".to_string(),
        DataRefValue::Float(0.0),
    )]);
    assert_eq!(snapshot.config.gear.nose, GearPosition::Up);
    assert_eq!(snapshot.config.gear.left, GearPosition::Up);
    assert_eq!(snapshot.config.gear.right, GearPosition::Up);
}

/// Gear deploy value between 0.1 and 0.9 → `GearPosition::Transitioning`.
#[test]
fn convert_snapshot_gear_transitioning() {
    use flight_bus::types::GearPosition;
    let snapshot = snapshot_from([(
        "sim/aircraft/parts/acf_gear_deploy".to_string(),
        DataRefValue::Float(0.5),
    )]);
    assert_eq!(snapshot.config.gear.nose, GearPosition::Transitioning);
}

// ---------------------------------------------------------------------------
// 11. convert_raw_to_snapshot — multi-engine N1 extraction
// ---------------------------------------------------------------------------

/// Two engines with distinct N1 values are both present in the snapshot.
///
/// X-Plane DataRef layout: `sim/flightmodel/engine/ENGN_running[i]` (Int) and
/// `sim/flightmodel/engine/ENGN_N1_[i]` (Float, percent).
#[test]
fn convert_snapshot_multi_engine_distinct_n1() {
    let extras = [
        (
            "sim/flightmodel/engine/ENGN_running[0]".to_string(),
            DataRefValue::Int(1),
        ),
        (
            "sim/flightmodel/engine/ENGN_N1_[0]".to_string(),
            DataRefValue::Float(75.0),
        ),
        (
            "sim/flightmodel/engine/ENGN_running[1]".to_string(),
            DataRefValue::Int(1),
        ),
        (
            "sim/flightmodel/engine/ENGN_N1_[1]".to_string(),
            DataRefValue::Float(80.0),
        ),
    ];
    let snapshot = snapshot_from(extras);

    assert_eq!(
        snapshot.engines.len(),
        2,
        "expected two engines in snapshot"
    );

    let eng0 = snapshot
        .engines
        .iter()
        .find(|e| e.index == 0)
        .expect("engine 0 missing");
    let eng1 = snapshot
        .engines
        .iter()
        .find(|e| e.index == 1)
        .expect("engine 1 missing");

    assert!(
        (eng0.rpm.value() - 75.0).abs() < 0.5,
        "engine 0 N1 should be ~75 %, got {}",
        eng0.rpm.value()
    );
    assert!(
        (eng1.rpm.value() - 80.0).abs() < 0.5,
        "engine 1 N1 should be ~80 %, got {}",
        eng1.rpm.value()
    );
    assert!(eng0.running, "engine 0 must be marked running");
    assert!(eng1.running, "engine 1 must be marked running");
}

// ---------------------------------------------------------------------------
// 12. convert_raw_to_snapshot — AoA / alpha dataref (XP11 + XP12)
// ---------------------------------------------------------------------------

/// `sim/flightmodel/position/alpha` (XP11 primary path, also valid in XP12) is
/// correctly extracted as the angle-of-attack in the kinematics sub-struct.
#[test]
fn convert_snapshot_aoa_from_alpha_dataref() {
    let snapshot = snapshot_from([(
        "sim/flightmodel/position/alpha".to_string(),
        DataRefValue::Float(8.0), // 8 degrees AoA
    )]);
    let aoa_deg = snapshot.kinematics.aoa.to_degrees();
    assert!(
        (aoa_deg - 8.0).abs() < 0.1,
        "expected AoA 8°, got {aoa_deg}°"
    );
}

// ---------------------------------------------------------------------------
// 13. State machine — disconnect / reconnect lifecycle
// ---------------------------------------------------------------------------

/// Full happy-path lifecycle:
///   Disconnected → Connecting → Connected → Active → Stale → Active (recovery)
#[tokio::test]
async fn state_machine_full_lifecycle_happy_path() {
    use flight_xplane::XPlaneAdapterState;

    let adapter = make_adapter();
    assert_eq!(adapter.xplane_state(), XPlaneAdapterState::Disconnected);

    // Socket bound → Connecting
    adapter.handle_socket_bound();
    assert_eq!(adapter.xplane_state(), XPlaneAdapterState::Connecting);

    // Socket bound again → Connected
    adapter.handle_socket_bound();
    assert_eq!(adapter.xplane_state(), XPlaneAdapterState::Connected);
    assert_eq!(adapter.state(), AdapterState::Connected);

    // Telemetry received → Active
    let snapshot = flight_bus::snapshot::BusSnapshot::new(
        flight_bus::types::SimId::XPlane,
        flight_bus::types::AircraftId::new("C172"),
    );
    adapter.process_telemetry(snapshot).unwrap();
    assert_eq!(adapter.xplane_state(), XPlaneAdapterState::Active);
    assert_eq!(adapter.state(), AdapterState::Active);

    // Timeout → Stale (maps to AdapterState::Disconnected)
    adapter.handle_telemetry_timeout().unwrap();
    assert_eq!(adapter.xplane_state(), XPlaneAdapterState::Stale);
    assert_eq!(adapter.state(), AdapterState::Disconnected);

    // Recovery: telemetry resumes → Active
    let snapshot2 = flight_bus::snapshot::BusSnapshot::new(
        flight_bus::types::SimId::XPlane,
        flight_bus::types::AircraftId::new("C172"),
    );
    adapter.process_telemetry(snapshot2).unwrap();
    assert_eq!(adapter.xplane_state(), XPlaneAdapterState::Active);
    assert_eq!(adapter.state(), AdapterState::Active);
}

/// Socket error from Active drives to Error, then SocketBound recovers.
#[tokio::test]
async fn state_machine_error_and_reconnect() {
    use flight_xplane::XPlaneAdapterState;

    let publisher = make_publisher();
    let adapter = XPlaneAdapter::new(XPlaneAdapterConfig::default(), publisher).unwrap();

    // Advance to Active
    adapter.handle_socket_bound();
    adapter.handle_socket_bound();
    let snapshot = flight_bus::snapshot::BusSnapshot::new(
        flight_bus::types::SimId::XPlane,
        flight_bus::types::AircraftId::new("C172"),
    );
    adapter.process_telemetry(snapshot).unwrap();
    assert_eq!(adapter.xplane_state(), XPlaneAdapterState::Active);

    // Socket error → Error
    adapter.handle_socket_error("connection reset");
    assert_eq!(adapter.xplane_state(), XPlaneAdapterState::Error);
    assert_eq!(adapter.state(), AdapterState::Error);

    // Retry → Connecting
    adapter.handle_socket_bound();
    assert_eq!(adapter.xplane_state(), XPlaneAdapterState::Connecting);

    // → Connected
    adapter.handle_socket_bound();
    assert_eq!(adapter.xplane_state(), XPlaneAdapterState::Connected);

    // → Active again with new telemetry
    let snapshot2 = flight_bus::snapshot::BusSnapshot::new(
        flight_bus::types::SimId::XPlane,
        flight_bus::types::AircraftId::new("C172"),
    );
    adapter.process_telemetry(snapshot2).unwrap();
    assert_eq!(adapter.xplane_state(), XPlaneAdapterState::Active);
}

/// Shutdown from Active returns to Disconnected and clears error state.
#[tokio::test]
async fn shutdown_from_active_returns_to_disconnected() {
    use flight_xplane::XPlaneAdapterState;

    let adapter = make_adapter();

    // Advance to Active
    adapter.handle_socket_bound();
    adapter.handle_socket_bound();
    let snapshot = flight_bus::snapshot::BusSnapshot::new(
        flight_bus::types::SimId::XPlane,
        flight_bus::types::AircraftId::new("C172"),
    );
    adapter.process_telemetry(snapshot).unwrap();
    assert_eq!(adapter.xplane_state(), XPlaneAdapterState::Active);

    // Shutdown
    adapter.handle_shutdown();
    assert_eq!(adapter.xplane_state(), XPlaneAdapterState::Disconnected);
    assert_eq!(adapter.state(), AdapterState::Disconnected);
}

// ---------------------------------------------------------------------------
// 14. Bus subscriber receives snapshots through state transitions
// ---------------------------------------------------------------------------

/// Subscriber receives valid snapshot on Active transition and stale on timeout.
#[tokio::test]
async fn subscriber_receives_snapshots_across_state_transitions() {
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

    let adapter = XPlaneAdapter::new(XPlaneAdapterConfig::default(), Arc::clone(&publisher)).unwrap();

    // Drive to Connected (no bus publish yet)
    adapter.handle_socket_bound();
    adapter.handle_socket_bound();

    // Process telemetry → Active, publishes to bus
    let snapshot = BusSnapshot::new(SimId::XPlane, AircraftId::new("A320"));
    adapter.process_telemetry(snapshot).unwrap();

    let received = subscriber.try_recv().unwrap().expect("should receive valid snapshot");
    assert_eq!(received.sim, SimId::XPlane);
    assert_eq!(received.aircraft.icao, "A320");

    // Wait for rate limiter to allow next publish
    std::thread::sleep(std::time::Duration::from_millis(20));

    // Timeout → Stale, publishes stale snapshot
    adapter.handle_telemetry_timeout().unwrap();

    let stale = subscriber.try_recv().unwrap().expect("should receive stale snapshot");
    assert!(!stale.validity.safe_for_ffb, "stale must not be safe for FFB");
    assert_eq!(stale.aircraft.icao, "unknown", "stale uses 'unknown' aircraft");

    // Wait for rate limiter again
    std::thread::sleep(std::time::Duration::from_millis(20));

    // Recovery → Active again
    let recovery = BusSnapshot::new(SimId::XPlane, AircraftId::new("A320"));
    adapter.process_telemetry(recovery).unwrap();

    let recovered = subscriber.try_recv().unwrap().expect("should receive recovery snapshot");
    assert_eq!(recovered.aircraft.icao, "A320");
}

/// After an error and reconnect, the subscriber still receives new data.
#[tokio::test]
async fn subscriber_receives_after_reconnect() {
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

    let adapter = XPlaneAdapter::new(XPlaneAdapterConfig::default(), Arc::clone(&publisher)).unwrap();

    // Drive to Active and send first snapshot
    adapter.handle_socket_bound();
    adapter.handle_socket_bound();
    let snap1 = BusSnapshot::new(SimId::XPlane, AircraftId::new("C172"));
    adapter.process_telemetry(snap1).unwrap();
    let _ = subscriber.try_recv().unwrap().expect("first snapshot");

    // Error → disconnect
    adapter.handle_socket_error("link dropped");

    // Reconnect cycle
    adapter.handle_socket_bound(); // → Connecting
    adapter.handle_socket_bound(); // → Connected

    // Wait for rate limiter
    std::thread::sleep(std::time::Duration::from_millis(20));

    // New telemetry after reconnect
    let snap2 = BusSnapshot::new(SimId::XPlane, AircraftId::new("B738"));
    adapter.process_telemetry(snap2).unwrap();

    let received = subscriber.try_recv().unwrap().expect("snapshot after reconnect");
    assert_eq!(received.aircraft.icao, "B738", "should receive data for new aircraft after reconnect");
}

// ---------------------------------------------------------------------------
// 15. Repeated timeout stays Stale
// ---------------------------------------------------------------------------

/// Multiple consecutive timeouts keep the adapter in Stale state and continue
/// publishing stale snapshots (subscribers keep getting notified).
#[tokio::test]
async fn repeated_timeout_stays_stale_and_keeps_publishing() {
    use flight_bus::{
        snapshot::BusSnapshot,
        types::{AircraftId, SimId},
    };
    use flight_xplane::XPlaneAdapterState;

    let publisher = make_publisher();
    let mut subscriber = publisher
        .lock()
        .unwrap()
        .subscribe(SubscriptionConfig::default())
        .expect("subscribe");

    let adapter = XPlaneAdapter::new(XPlaneAdapterConfig::default(), Arc::clone(&publisher)).unwrap();

    // Advance to Active
    adapter.handle_socket_bound();
    adapter.handle_socket_bound();
    let snap = BusSnapshot::new(SimId::XPlane, AircraftId::new("C172"));
    adapter.process_telemetry(snap).unwrap();
    let _ = subscriber.try_recv().unwrap(); // drain

    // First timeout
    std::thread::sleep(std::time::Duration::from_millis(20));
    adapter.handle_telemetry_timeout().unwrap();
    assert_eq!(adapter.xplane_state(), XPlaneAdapterState::Stale);
    let stale1 = subscriber.try_recv().unwrap().expect("first stale");
    assert!(!stale1.validity.safe_for_ffb);

    // Second timeout (should remain Stale)
    std::thread::sleep(std::time::Duration::from_millis(20));
    adapter.handle_telemetry_timeout().unwrap();
    assert_eq!(adapter.xplane_state(), XPlaneAdapterState::Stale);
    let stale2 = subscriber.try_recv().unwrap().expect("second stale");
    assert!(!stale2.validity.safe_for_ffb);
}

// ---------------------------------------------------------------------------
// 16. Validity flags through full pipeline
// ---------------------------------------------------------------------------

/// A snapshot converted from complete datarefs has safe_for_ffb=true and flows
/// through the bus with those flags intact.
#[tokio::test]
async fn validity_flags_preserved_through_bus_publish() {
    let mut datarefs = make_critical_datarefs();
    // Add attitude
    datarefs.insert("sim/flightmodel/position/theta".to_string(), DataRefValue::Float(5.0));
    datarefs.insert("sim/flightmodel/position/phi".to_string(), DataRefValue::Float(10.0));
    datarefs.insert("sim/flightmodel/position/psi".to_string(), DataRefValue::Float(90.0));
    // Add angular rates
    datarefs.insert("sim/flightmodel/position/P".to_string(), DataRefValue::Float(1.0));
    datarefs.insert("sim/flightmodel/position/Q".to_string(), DataRefValue::Float(0.5));
    datarefs.insert("sim/flightmodel/position/R".to_string(), DataRefValue::Float(0.2));

    let raw = make_raw_data(datarefs);
    let snapshot = XPlaneAdapter::convert_raw_to_snapshot(raw, Instant::now()).unwrap();
    assert!(snapshot.validity.safe_for_ffb, "source snapshot should be FFB-safe");

    // Publish through bus and verify flags are preserved
    let publisher = make_publisher();
    let mut subscriber = publisher
        .lock()
        .unwrap()
        .subscribe(SubscriptionConfig::default())
        .expect("subscribe");

    let adapter = XPlaneAdapter::new(XPlaneAdapterConfig::default(), Arc::clone(&publisher)).unwrap();
    adapter.handle_socket_bound();
    adapter.handle_socket_bound();
    adapter.process_telemetry(snapshot).unwrap();

    let received = subscriber.try_recv().unwrap().expect("should receive snapshot");
    assert!(received.validity.safe_for_ffb, "FFB-safe flag must survive bus round-trip");
    assert!(received.validity.attitude_valid);
    assert!(received.validity.angular_rates_valid);
    assert!(received.validity.velocities_valid);
}
