#![cfg(windows)]
// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the SimConnect adapter (MSFS integration layer).
//!
//! Covers: connection state machine, variable reading, event mapping,
//! aircraft detection, bus publishing, and protocol handling — all using
//! [`MockSimConnectBackend`] so no real MSFS is required.

use std::time::Duration;

use flight_simconnect::connection::{
    ConnectionConfig, ConnectionState,
    ExponentialBackoff, HealthMonitor, SimConnectConnection,
};
use flight_simconnect::adapter_state::{
    SimConnectAdapterState, SimConnectEvent, SimConnectStateMachine, SimConnectTransitionError,
};
use flight_simconnect::simconnect_bridge::{
    BackendError, BridgeConfig, DispatchMessage, MockSimConnectBackend, SimConnectBackend,
    SimConnectBridge, VarSnapshot,
};
use flight_simconnect::aircraft_detection::{
    AircraftDetectionEngine, AircraftEntry, MatchConfidence, SimAircraftData,
};
use flight_simconnect::var_registry::{SimVarCategory, SimVarRegistry};
use flight_simconnect::event_mapping::{
    SimEventCategory, SimEventMapper, SIM_EVENT_CATALOG, catalog_by_category, catalog_lookup,
};
use flight_simconnect::control_injection::AxisId;
use flight_simconnect::subscription::CORE_SUBSCRIPTION_VARS;

// ===========================================================================
// Helpers
// ===========================================================================

fn default_bridge() -> SimConnectBridge<MockSimConnectBackend> {
    SimConnectBridge::new(MockSimConnectBackend::new(), BridgeConfig::default())
}

fn connected_bridge() -> SimConnectBridge<MockSimConnectBackend> {
    let mut b = default_bridge();
    b.connect().expect("connect must succeed on mock");
    b
}

// Well-known IDs used by the bridge (mirror of private constants).
const DEF_TELEMETRY: u32 = 1;
const REQ_TELEMETRY: u32 = 1;
const DEF_AIRCRAFT: u32 = 2;
const REQ_AIRCRAFT: u32 = 2;

// ═══════════════════════════════════════════════════════════════════════════
// 1. Connection state machine (8 tests)
// ═══════════════════════════════════════════════════════════════════════════

mod connection_state_machine {
    use super::*;

    /// Full happy-path: Disconnected → Connecting → Connected → Active → Disconnected.
    #[test]
    fn full_lifecycle_transitions() {
        let mut conn = SimConnectConnection::default();
        assert_eq!(conn.state(), ConnectionState::Disconnected);

        assert_eq!(conn.connect().unwrap(), ConnectionState::Connecting);
        assert_eq!(conn.on_connected().unwrap(), ConnectionState::Connected);
        assert!(conn.is_connected());

        assert_eq!(conn.on_data_received().unwrap(), ConnectionState::Active);
        assert!(conn.is_active());

        assert_eq!(conn.disconnect().unwrap(), ConnectionState::Disconnected);
        assert!(!conn.is_connected());
    }

    /// Guards prevent skipping the Connecting step.
    #[test]
    fn connect_succeeded_from_disconnected_rejected() {
        let mut conn = SimConnectConnection::default();
        let err = conn.on_connected().unwrap_err();
        assert_eq!(err.from, ConnectionState::Disconnected);
    }

    /// DataReceived is invalid while Disconnected.
    #[test]
    fn data_received_from_disconnected_rejected() {
        let mut conn = SimConnectConnection::default();
        assert!(conn.on_data_received().is_err());
    }

    /// ConnectAttempted is invalid while already Active.
    #[test]
    fn connect_from_active_rejected() {
        let mut conn = SimConnectConnection::default();
        conn.connect().unwrap();
        conn.on_connected().unwrap();
        conn.on_data_received().unwrap();
        assert!(conn.connect().is_err());
    }

    /// ConnectFailed goes back to Disconnected and records the error.
    #[test]
    fn connect_failed_records_error() {
        let mut conn = SimConnectConnection::default();
        conn.connect().unwrap();
        conn.on_connect_failed("MSFS unavailable").unwrap();
        assert_eq!(conn.state(), ConnectionState::Disconnected);
        assert_eq!(conn.last_error(), Some("MSFS unavailable"));
    }

    /// Reconnect after error: lost → disconnected → reconnect → connecting.
    #[test]
    fn reconnect_after_connection_lost() {
        let mut conn = SimConnectConnection::new(ConnectionConfig {
            max_reconnect_attempts: 5,
            ..Default::default()
        });
        conn.connect().unwrap();
        conn.on_connected().unwrap();
        conn.on_data_received().unwrap();

        conn.on_connection_lost("pipe broken").unwrap();
        assert_eq!(conn.state(), ConnectionState::Disconnected);

        conn.reconnect().unwrap();
        assert_eq!(conn.state(), ConnectionState::Connecting);
        assert_eq!(conn.total_reconnects(), 1);
    }

    /// Connect timeout: max reconnect attempts exhausted.
    #[test]
    fn max_reconnect_attempts_exhausted() {
        let mut conn = SimConnectConnection::new(ConnectionConfig {
            max_reconnect_attempts: 1,
            ..Default::default()
        });
        // Attempt 1
        conn.connect().unwrap();
        conn.on_connect_failed("timeout").unwrap();

        // Attempt 2 — should fail because max_reconnect_attempts=1 and
        // connect_attempts is already 1.
        assert!(conn.reconnect().is_err());
    }

    /// Disconnect clears all internal state (error, attempt counter, health).
    #[test]
    fn disconnect_clears_all_internal_state() {
        let mut conn = SimConnectConnection::default();
        conn.connect().unwrap();
        conn.on_connect_failed("err").unwrap();
        conn.connect().unwrap();
        conn.on_connected().unwrap();
        conn.on_data_received().unwrap();

        conn.disconnect().unwrap();
        assert_eq!(conn.connect_attempts(), 0);
        assert!(conn.last_error().is_none());
        assert!(!conn.health().has_received());
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. Variable reading (6 tests)
// ═══════════════════════════════════════════════════════════════════════════

mod variable_reading {
    use super::*;

    /// Connecting to the mock backend registers SimVars on the backend.
    #[test]
    fn register_simvar_on_connect() {
        let b = connected_bridge();
        assert!(!b.registered_vars().is_empty());
        let defs = b.backend().definitions();
        assert!(defs.contains_key(&DEF_TELEMETRY));
    }

    /// Telemetry data dispatch populates the snapshot with correct values.
    #[test]
    fn read_simvar_update_into_snapshot() {
        let mut b = connected_bridge();
        let n = b.registered_vars().len();
        let values: Vec<f64> = (0..n).map(|i| (i as f64) * 10.0).collect();

        b.backend_mut()
            .push_dispatch(DispatchMessage::SimObjectData {
                define_id: DEF_TELEMETRY,
                request_id: REQ_TELEMETRY,
                values: values.clone(),
            });
        b.poll().unwrap();

        let snap = b.latest_snapshot.as_ref().expect("snapshot must exist");
        assert_eq!(snap.values.len(), n);
        // Each registered var should have the corresponding value.
        for (i, name) in b.registered_vars().iter().enumerate() {
            let expected = i as f64 * 10.0;
            assert!(
                (snap.values[name] - expected).abs() < f64::EPSILON,
                "var {name}: expected {expected}, got {}",
                snap.values[name]
            );
        }
    }

    /// Batch variable read: multiple dispatches are processed in one poll.
    #[test]
    fn batch_variable_read() {
        let mut b = connected_bridge();
        let n = b.registered_vars().len();

        // Push two telemetry updates.
        for val in [1.0, 2.0] {
            b.backend_mut()
                .push_dispatch(DispatchMessage::SimObjectData {
                    define_id: DEF_TELEMETRY,
                    request_id: REQ_TELEMETRY,
                    values: vec![val; n],
                });
        }

        let count = b.poll().unwrap();
        assert_eq!(count, 2);
        // Latest snapshot should reflect the second (most recent) update.
        let snap = b.latest_snapshot.as_ref().unwrap();
        let first_var = &b.registered_vars()[0];
        assert!((snap.values[first_var] - 2.0).abs() < f64::EPSILON);
    }

    /// Variable type mapping: SimVarRegistry distinguishes categories and units.
    #[test]
    fn variable_type_categories_and_units() {
        let reg = SimVarRegistry::new();

        // Flight controls use "position".
        let aileron = reg.get("AILERON POSITION").expect("aileron must exist");
        assert_eq!(aileron.category, SimVarCategory::FlightControls);
        assert_eq!(aileron.unit, "position");
        assert!(aileron.writable);

        // Navigation vars like altitude use "feet".
        let alt = reg.get("INDICATED ALTITUDE").expect("altitude must exist");
        assert_eq!(alt.category, SimVarCategory::Navigation);
        assert_eq!(alt.unit, "feet");
        assert!(!alt.writable);

        // Electrical vars use "bool".
        let battery = reg
            .get("ELECTRICAL MASTER BATTERY")
            .expect("battery must exist");
        assert_eq!(battery.category, SimVarCategory::Electrical);
        assert_eq!(battery.unit, "bool");
    }

    /// Requesting an unknown variable name returns None.
    #[test]
    fn invalid_variable_not_found() {
        let reg = SimVarRegistry::new();
        assert!(reg.get("THIS_VARIABLE_DOES_NOT_EXIST").is_none());
    }

    /// Unit strings are consistent between registry and subscription.
    #[test]
    fn unit_conversion_consistency() {
        let reg = SimVarRegistry::new();
        for sv in CORE_SUBSCRIPTION_VARS {
            if let Some(var) = reg.get(sv.name) {
                // Subscription units match registry for navigation vars.
                if sv.name == "AIRSPEED INDICATED" {
                    assert_eq!(var.unit, "knots");
                    assert_eq!(sv.units, "knots");
                }
            }
        }
        // Verify core subscription vars have expected units.
        let ias = CORE_SUBSCRIPTION_VARS
            .iter()
            .find(|v| v.name == "AIRSPEED INDICATED")
            .unwrap();
        assert_eq!(ias.units, "knots");

        let alt = CORE_SUBSCRIPTION_VARS
            .iter()
            .find(|v| v.name == "INDICATED ALTITUDE")
            .unwrap();
        assert_eq!(alt.units, "feet");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. Event mapping (6 tests)
// ═══════════════════════════════════════════════════════════════════════════

mod event_mapping {
    use super::*;

    /// System event subscription: AircraftLoaded is registered during connect.
    #[test]
    fn system_event_subscription_on_connect() {
        let b = connected_bridge();
        let sys = b.backend().system_events();
        assert!(
            sys.values().any(|v| v == "AircraftLoaded"),
            "AircraftLoaded must be subscribed after connect"
        );
    }

    /// Key event send: inject_key_event maps and transmits the event.
    #[test]
    fn key_event_send() {
        let mut b = connected_bridge();
        b.inject_key_event("GEAR_TOGGLE", 0).unwrap();

        let events = b.backend().transmitted_events();
        assert!(!events.is_empty(), "key event must be transmitted");
        // Data should be 0.
        assert_eq!(events.last().unwrap().1, 0);
    }

    /// Custom event registration: event mapper tracks button bindings.
    #[test]
    fn custom_event_registration_and_lookup() {
        let mut mapper = SimEventMapper::new();
        mapper.map_button("btn_trigger", "GEAR_TOGGLE");
        mapper.map_button("btn_trigger", "TOGGLE_NAV_LIGHTS");
        mapper.map_button("btn_hat_up", "AP_MASTER");

        let events = mapper.get_events("btn_trigger").unwrap();
        assert_eq!(events.len(), 2);
        assert!(events.contains(&"GEAR_TOGGLE"));
        assert!(events.contains(&"TOGGLE_NAV_LIGHTS"));

        assert_eq!(mapper.mapped_button_count(), 2);
    }

    /// Event parameter passing: axis events transmit clamped data values.
    #[test]
    fn event_parameter_passing() {
        let mut b = connected_bridge();
        b.inject_axis(AxisId::Elevator, 8000).unwrap();
        b.inject_axis(AxisId::Rudder, -5000).unwrap();

        let events = b.backend().transmitted_events();
        assert_eq!(events.len(), 2);
        // Elevator: 8000 (within range, passed through).
        assert_eq!(events[0].1, 8000u32);
        // Rudder: -5000 as u32.
        assert_eq!(events[1].1, (-5000i32) as u32);
    }

    /// Event acknowledgment: sent counter increments per injection.
    #[test]
    fn event_acknowledgment_counter() {
        let mut b = connected_bridge();
        b.inject_axis(AxisId::Ailerons, 0).unwrap();
        b.inject_key_event("AP_MASTER", 1).unwrap();
        assert_eq!(b.injector().commands_sent(), 2);
    }

    /// Event overflow: axis value clamped to 16-bit SimConnect range.
    #[test]
    fn event_overflow_clamping() {
        let mut b = connected_bridge();
        // Positive overflow.
        b.inject_axis(AxisId::Throttle, 99999).unwrap();
        let events = b.backend().transmitted_events();
        assert_eq!(events[0].1, 16383u32, "must clamp to +16383");

        // Negative overflow.
        b.inject_axis(AxisId::Throttle, -99999).unwrap();
        let events = b.backend().transmitted_events();
        assert_eq!(events[1].1, (-16384i32) as u32, "must clamp to -16384");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. Aircraft detection (5 tests)
// ═══════════════════════════════════════════════════════════════════════════

mod aircraft_detection {
    use super::*;

    /// Aircraft title extraction: exact model match by title string.
    #[test]
    fn aircraft_title_extraction() {
        let engine = AircraftDetectionEngine::default_msfs();
        let result = engine.detect(&SimAircraftData {
            title: "Cessna 172 Skyhawk G1000 NXi".into(),
            atc_type: String::new(),
            atc_model: String::new(),
        });
        assert!(result.confidence >= MatchConfidence::Low);
        assert_eq!(result.icao, Some("C172".into()));
    }

    /// ICAO code detection via ATC_MODEL (fast-path exact match).
    #[test]
    fn icao_code_detection_via_atc_model() {
        let engine = AircraftDetectionEngine::default_msfs();
        let result = engine.detect(&SimAircraftData {
            title: String::new(),
            atc_type: String::new(),
            atc_model: "A20N".into(),
        });
        assert_eq!(result.confidence, MatchConfidence::Exact);
        assert_eq!(result.icao, Some("A320".into()));
        assert_eq!(
            result.display_name,
            Some("Airbus A320neo".into())
        );
    }

    /// Livery / community mod identification via title keywords.
    #[test]
    fn livery_and_community_mod_identification() {
        let engine = AircraftDetectionEngine::default_msfs();

        let fbw = engine.detect(&SimAircraftData {
            title: "FlyByWire A320neo Custom Livery".into(),
            atc_type: "AIRBUS".into(),
            atc_model: "A320".into(),
        });
        assert!(fbw.is_community_mod);
        assert_eq!(fbw.icao, Some("A320".into()));

        let pmdg = engine.detect(&SimAircraftData {
            title: "PMDG 737-800 NGXu".into(),
            atc_type: "BOEING".into(),
            atc_model: "B738".into(),
        });
        assert!(pmdg.is_community_mod);

        // Standard aircraft should NOT be flagged as community mod.
        let standard = engine.detect(&SimAircraftData {
            title: "Cessna 172 Skyhawk".into(),
            atc_type: "CESSNA".into(),
            atc_model: "C172".into(),
        });
        assert!(!standard.is_community_mod);
    }

    /// Aircraft type classification: GA, airliner, helicopter tags.
    #[test]
    fn aircraft_type_classification() {
        let entries = vec![
            AircraftEntry {
                icao: "C172".into(),
                display_name: "Cessna 172".into(),
                known_titles: vec!["Cessna 172".into()],
                known_atc_types: vec!["CESSNA".into()],
                known_atc_models: vec!["C172".into()],
                tags: vec!["ga".into(), "single-engine".into()],
            },
            AircraftEntry {
                icao: "A320".into(),
                display_name: "Airbus A320".into(),
                known_titles: vec!["Airbus A320".into()],
                known_atc_types: vec!["AIRBUS".into()],
                known_atc_models: vec!["A320".into()],
                tags: vec!["airliner".into(), "jet".into()],
            },
            AircraftEntry {
                icao: "R22".into(),
                display_name: "Robinson R22".into(),
                known_titles: vec!["Robinson R22".into()],
                known_atc_types: vec!["ROBINSON".into()],
                known_atc_models: vec!["R22".into()],
                tags: vec!["helicopter".into()],
            },
        ];
        let engine = AircraftDetectionEngine::new(entries.clone());

        // Verify tags are accessible on the original entries.
        assert!(entries[0].tags.contains(&"ga".into()));
        assert!(entries[1].tags.contains(&"airliner".into()));
        assert!(entries[2].tags.contains(&"helicopter".into()));

        // Detection works for each.
        let r = engine.detect(&SimAircraftData {
            title: String::new(),
            atc_type: String::new(),
            atc_model: "R22".into(),
        });
        assert_eq!(r.icao, Some("R22".into()));
    }

    /// Detection confidence scoring: multi-indicator vs single-indicator.
    #[test]
    fn detection_confidence_scoring() {
        let engine = AircraftDetectionEngine::default_msfs();

        // Exact ATC_MODEL → highest confidence.
        let exact = engine.detect(&SimAircraftData {
            title: String::new(),
            atc_type: String::new(),
            atc_model: "C172".into(),
        });
        assert_eq!(exact.confidence, MatchConfidence::Exact);
        assert!((exact.indicator_scores.atc_model_score - 1.0).abs() < f32::EPSILON);

        // Multi-indicator (title + atc_type, no model) → medium or higher.
        let multi = engine.detect(&SimAircraftData {
            title: "Airbus A320neo".into(),
            atc_type: "AIRBUS".into(),
            atc_model: String::new(),
        });
        assert!(multi.confidence >= MatchConfidence::Medium);

        // No data at all → None.
        let none = engine.detect(&SimAircraftData::default());
        assert_eq!(none.confidence, MatchConfidence::None);

        // Confidence ordering holds.
        assert!(MatchConfidence::Exact > MatchConfidence::High);
        assert!(MatchConfidence::High > MatchConfidence::Medium);
        assert!(MatchConfidence::Medium > MatchConfidence::Low);
        assert!(MatchConfidence::Low > MatchConfidence::None);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. Bus publishing (5 tests)
// ═══════════════════════════════════════════════════════════════════════════

mod bus_publishing {
    use super::*;

    /// Snapshot format: VarSnapshot stores named f64 values.
    #[test]
    fn snapshot_format() {
        let mut snap = VarSnapshot::default();
        snap.values.insert("AIRSPEED INDICATED".into(), 120.0);
        snap.values.insert("INDICATED ALTITUDE".into(), 5000.0);
        snap.timestamp_ns = 1_000_000;

        assert_eq!(snap.values.len(), 2);
        assert!((snap.values["AIRSPEED INDICATED"] - 120.0).abs() < f64::EPSILON);
        assert_eq!(snap.timestamp_ns, 1_000_000);
    }

    /// Snapshot frequency: each telemetry dispatch produces a new snapshot.
    #[test]
    fn snapshot_frequency_per_dispatch() {
        let mut b = connected_bridge();
        let n = b.registered_vars().len();

        // No snapshot before data.
        assert!(b.latest_snapshot.is_none());

        // First dispatch.
        b.backend_mut()
            .push_dispatch(DispatchMessage::SimObjectData {
                define_id: DEF_TELEMETRY,
                request_id: REQ_TELEMETRY,
                values: vec![1.0; n],
            });
        b.poll().unwrap();
        assert!(b.latest_snapshot.is_some());
        let first_snap = b.latest_snapshot.clone().unwrap();

        // Second dispatch overwrites.
        b.backend_mut()
            .push_dispatch(DispatchMessage::SimObjectData {
                define_id: DEF_TELEMETRY,
                request_id: REQ_TELEMETRY,
                values: vec![2.0; n],
            });
        b.poll().unwrap();
        let second_snap = b.latest_snapshot.as_ref().unwrap();
        assert_ne!(&first_snap, second_snap);
    }

    /// Stale snapshot: QUIT clears the latest snapshot.
    #[test]
    fn stale_snapshot_on_quit() {
        let mut b = connected_bridge();
        let n = b.registered_vars().len();

        // Receive telemetry.
        b.backend_mut()
            .push_dispatch(DispatchMessage::SimObjectData {
                define_id: DEF_TELEMETRY,
                request_id: REQ_TELEMETRY,
                values: vec![1.0; n],
            });
        b.poll().unwrap();
        assert!(b.latest_snapshot.is_some());

        // Quit clears snapshot.
        b.backend_mut().push_dispatch(DispatchMessage::Quit);
        b.poll().unwrap();
        assert!(b.latest_snapshot.is_none());
    }

    /// Bus disconnect handling: disconnect clears snapshot and aircraft.
    #[test]
    fn disconnect_clears_snapshot_and_aircraft() {
        let mut b = connected_bridge();

        // Bring to active with aircraft data.
        b.backend_mut()
            .push_dispatch(DispatchMessage::SimObjectData {
                define_id: DEF_AIRCRAFT,
                request_id: REQ_AIRCRAFT,
                values: vec![1.0],
            });
        b.poll().unwrap();
        assert!(b.latest_snapshot.is_some());

        // Detect aircraft.
        b.detect_aircraft(&SimAircraftData {
            title: "Cessna 172".into(),
            atc_type: "CESSNA".into(),
            atc_model: "C172".into(),
        });
        assert!(b.latest_aircraft.is_some());

        // Disconnect clears both.
        b.disconnect().unwrap();
        assert!(b.latest_snapshot.is_none());
        assert!(b.latest_aircraft.is_none());
    }

    /// Telemetry field mapping: registered vars map to snapshot keys.
    #[test]
    fn telemetry_field_mapping() {
        let mut b = connected_bridge();
        let vars = b.registered_vars().to_vec();
        let n = vars.len();
        assert!(n > 0);

        let values: Vec<f64> = (0..n).map(|i| (i + 1) as f64).collect();
        b.backend_mut()
            .push_dispatch(DispatchMessage::SimObjectData {
                define_id: DEF_TELEMETRY,
                request_id: REQ_TELEMETRY,
                values,
            });
        b.poll().unwrap();

        let snap = b.latest_snapshot.as_ref().unwrap();
        // Every registered var should appear as a key in the snapshot.
        for (i, name) in vars.iter().enumerate() {
            assert!(
                snap.values.contains_key(name),
                "snapshot missing key: {name}"
            );
            let expected = (i + 1) as f64;
            assert!(
                (snap.values[name] - expected).abs() < f64::EPSILON,
                "var {name}: expected {expected}, got {}",
                snap.values[name]
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. Protocol (5 tests)
// ═══════════════════════════════════════════════════════════════════════════

mod protocol {
    use super::*;

    /// SimConnect message framing: Open, Quit, SimObjectData, Event, Exception.
    #[test]
    fn dispatch_message_framing() {
        let mut mock = MockSimConnectBackend::new();
        mock.open("test").unwrap();

        // Push all message types.
        mock.push_dispatch(DispatchMessage::Open);
        mock.push_dispatch(DispatchMessage::Quit);
        mock.push_dispatch(DispatchMessage::SimObjectData {
            define_id: 1,
            request_id: 1,
            values: vec![42.0],
        });
        mock.push_dispatch(DispatchMessage::Event {
            event_id: 100,
            data: 0,
        });
        mock.push_dispatch(DispatchMessage::Exception { code: 7 });

        // Drain all messages.
        let mut messages = Vec::new();
        while let Ok(Some(msg)) = mock.get_next_dispatch() {
            messages.push(msg);
        }
        assert_eq!(messages.len(), 5);
        assert_eq!(messages[0], DispatchMessage::Open);
        assert_eq!(messages[1], DispatchMessage::Quit);
        assert!(matches!(
            messages[2],
            DispatchMessage::SimObjectData { define_id: 1, .. }
        ));
        assert!(matches!(
            messages[3],
            DispatchMessage::Event { event_id: 100, .. }
        ));
        assert!(matches!(
            messages[4],
            DispatchMessage::Exception { code: 7 }
        ));
    }

    /// Message sequence numbers: request IDs are unique per definition.
    #[test]
    fn request_id_management() {
        let b = connected_bridge();
        let reqs = b.backend().active_requests();

        // Telemetry and aircraft requests have distinct IDs.
        assert!(reqs.contains_key(&REQ_TELEMETRY));
        assert!(reqs.contains_key(&REQ_AIRCRAFT));
        assert_ne!(REQ_TELEMETRY, REQ_AIRCRAFT);

        // Each request maps to its corresponding definition.
        assert_eq!(reqs[&REQ_TELEMETRY], DEF_TELEMETRY);
        assert_eq!(reqs[&REQ_AIRCRAFT], DEF_AIRCRAFT);
    }

    /// Definition IDs separate telemetry from aircraft identification.
    #[test]
    fn definition_id_separation() {
        let b = connected_bridge();
        let defs = b.backend().definitions();

        assert!(defs.contains_key(&DEF_TELEMETRY));
        assert!(defs.contains_key(&DEF_AIRCRAFT));
        assert_ne!(DEF_TELEMETRY, DEF_AIRCRAFT);

        // Telemetry definition has multiple vars.
        assert!(defs[&DEF_TELEMETRY].len() > 1);

        // Aircraft definition has TITLE.
        assert!(defs[&DEF_AIRCRAFT]
            .iter()
            .any(|(name, _)| name == "TITLE"));
    }

    /// Error response handling: backend errors propagate correctly.
    #[test]
    fn error_response_handling() {
        // Connection failure.
        let mut mock = MockSimConnectBackend::new();
        mock.fail_next_open = true;
        let err = mock.open("test").unwrap_err();
        assert!(matches!(err, BackendError::ConnectionFailed(_)));

        // Transmit failure.
        let mut mock2 = MockSimConnectBackend::new();
        mock2.open("test").unwrap();
        mock2.fail_next_transmit = true;
        let err = mock2.transmit_event(1, 0).unwrap_err();
        assert!(matches!(err, BackendError::EventFailed(_)));

        // Operations on closed backend.
        let mut mock3 = MockSimConnectBackend::new();
        let err = mock3.add_to_data_definition(1, "VAR", "units").unwrap_err();
        assert!(matches!(err, BackendError::ConnectionLost(_)));

        // Display impls.
        assert_eq!(
            BackendError::ConnectionFailed("t".into()).to_string(),
            "connection failed: t"
        );
        assert_eq!(
            BackendError::InvalidRequest("bad".into()).to_string(),
            "invalid request: bad"
        );
    }

    /// Version negotiation: bridge handles OPEN dispatch during lifecycle.
    #[test]
    fn open_dispatch_during_active_session() {
        let mut b = connected_bridge();

        // Bring to Active.
        b.backend_mut()
            .push_dispatch(DispatchMessage::SimObjectData {
                define_id: DEF_AIRCRAFT,
                request_id: REQ_AIRCRAFT,
                values: vec![1.0],
            });
        b.poll().unwrap();
        assert_eq!(b.state(), SimConnectAdapterState::Active);

        // Receiving a duplicate Open should not crash or change state.
        b.backend_mut().push_dispatch(DispatchMessage::Open);
        let count = b.poll().unwrap();
        assert_eq!(count, 1);
        assert_eq!(b.state(), SimConnectAdapterState::Active);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Additional depth tests (supplementary)
// ═══════════════════════════════════════════════════════════════════════════

mod adapter_state_machine_depth {
    use super::*;

    /// Adapter state machine: full lifecycle with stale recovery.
    #[test]
    fn adapter_sm_stale_recovery() {
        let mut sm = SimConnectStateMachine::new(5000, 5);

        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        sm.transition(SimConnectEvent::AircraftDetected).unwrap();
        assert_eq!(sm.state(), SimConnectAdapterState::Active);

        // Go stale.
        sm.transition(SimConnectEvent::TelemetryTimeout).unwrap();
        assert_eq!(sm.state(), SimConnectAdapterState::Stale);
        assert!(!sm.is_healthy());

        // Recover.
        sm.transition(SimConnectEvent::TelemetryReceived).unwrap();
        assert_eq!(sm.state(), SimConnectAdapterState::Active);
        assert!(sm.is_healthy());
        assert_eq!(sm.error_count(), 0);
    }

    /// Adapter state machine: error retry with exhaustion.
    #[test]
    fn adapter_sm_error_retries_exhausted() {
        let mut sm = SimConnectStateMachine::new(5000, 2);

        // Two errors.
        sm.transition(SimConnectEvent::ConnectionLost("e1".into()))
            .unwrap();
        assert_eq!(sm.error_count(), 1);
        sm.transition(SimConnectEvent::OpenReceived).unwrap(); // recover

        sm.transition(SimConnectEvent::ConnectionLost("e2".into()))
            .unwrap();
        assert_eq!(sm.error_count(), 2);

        // Third attempt → exhausted.
        let res = sm.transition(SimConnectEvent::OpenReceived);
        assert!(matches!(
            res,
            Err(SimConnectTransitionError::RetriesExhausted { max_retries: 2 })
        ));
    }

    /// Shutdown resets error count from any state.
    #[test]
    fn adapter_sm_shutdown_resets_errors() {
        let mut sm = SimConnectStateMachine::new(5000, 10);
        sm.transition(SimConnectEvent::ConnectionLost("x".into()))
            .unwrap();
        sm.transition(SimConnectEvent::ConnectionLost("y".into()))
            .unwrap();
        assert!(sm.error_count() >= 2);

        sm.transition(SimConnectEvent::Shutdown).unwrap();
        assert_eq!(sm.error_count(), 0);
        assert_eq!(sm.state(), SimConnectAdapterState::Disconnected);
    }

    /// Invalid transitions produce descriptive errors.
    #[test]
    fn adapter_sm_invalid_transition_error_info() {
        let mut sm = SimConnectStateMachine::new(5000, 3);
        let err = sm
            .transition(SimConnectEvent::AircraftDetected)
            .unwrap_err();
        match err {
            SimConnectTransitionError::InvalidTransition { from, event } => {
                assert_eq!(from, SimConnectAdapterState::Disconnected);
                assert!(event.contains("AircraftDetected"));
            }
            _ => panic!("expected InvalidTransition"),
        }
    }
}

mod bridge_reconnection_depth {
    use super::*;

    /// Bridge reconnection with mock backend failure/recovery cycle.
    #[test]
    fn bridge_reconnect_after_failure() {
        let mut b = default_bridge();
        b.backend_mut().fail_next_open = true;
        let _ = b.connect();
        assert_eq!(b.state(), SimConnectAdapterState::Error);

        // Reconnect should work now.
        let ok = b.try_reconnect().unwrap();
        assert!(ok);
        assert_eq!(b.state(), SimConnectAdapterState::Connected);
    }

    /// Backoff delay resets after successful reconnect.
    #[test]
    fn backoff_resets_on_successful_reconnect() {
        let config = BridgeConfig {
            backoff_base: Duration::from_millis(100),
            backoff_max: Duration::from_secs(10),
            ..Default::default()
        };
        let mut b = SimConnectBridge::new(MockSimConnectBackend::new(), config);

        // Advance backoff.
        let d1 = b.next_reconnect_delay();
        let d2 = b.next_reconnect_delay();
        assert!(d2 > d1);

        // Connect resets backoff.
        b.connect().unwrap();
        let d_after = b.next_reconnect_delay();
        assert_eq!(d_after, Duration::from_millis(100));
    }

    /// AircraftLoaded event resets detection state.
    #[test]
    fn aircraft_loaded_resets_detection() {
        let mut b = connected_bridge();

        // Bring to Active.
        b.backend_mut()
            .push_dispatch(DispatchMessage::SimObjectData {
                define_id: DEF_AIRCRAFT,
                request_id: REQ_AIRCRAFT,
                values: vec![1.0],
            });
        b.poll().unwrap();

        // Detect an aircraft.
        b.detect_aircraft(&SimAircraftData {
            title: "Cessna 172".into(),
            atc_type: "CESSNA".into(),
            atc_model: "C172".into(),
        });
        assert!(b.latest_aircraft.is_some());

        // AircraftLoaded event → clears aircraft and snapshot.
        b.backend_mut().push_dispatch(DispatchMessage::Event {
            event_id: 100, // EVT_AIRCRAFT_LOADED
            data: 0,
        });
        b.poll().unwrap();
        assert!(b.latest_aircraft.is_none());
        assert!(b.latest_snapshot.is_none());
    }
}

mod registry_depth {
    use super::*;

    /// Registry has entries for all 10 categories.
    #[test]
    fn registry_covers_all_categories() {
        let reg = SimVarRegistry::new();
        let categories = [
            SimVarCategory::FlightControls,
            SimVarCategory::Engine,
            SimVarCategory::Navigation,
            SimVarCategory::Electrical,
            SimVarCategory::Fuel,
            SimVarCategory::Landing,
            SimVarCategory::Environment,
            SimVarCategory::Instruments,
            SimVarCategory::Autopilot,
            SimVarCategory::Communication,
        ];
        for cat in categories {
            let vars = reg.by_category(cat);
            assert!(
                !vars.is_empty(),
                "category {cat:?} must have at least one var"
            );
        }
    }

    /// Registry writable flag is correct for known vars.
    #[test]
    fn registry_writable_flags() {
        let reg = SimVarRegistry::new();

        // Control surfaces are writable.
        assert!(reg.get("AILERON POSITION").unwrap().writable);
        assert!(reg.get("ELEVATOR POSITION").unwrap().writable);

        // Navigation readouts are not writable.
        assert!(!reg.get("AIRSPEED INDICATED").unwrap().writable);
        assert!(!reg.get("PLANE ALTITUDE").unwrap().writable);
    }

    /// Registry total entry count is substantial.
    #[test]
    fn registry_has_many_entries() {
        let reg = SimVarRegistry::new();
        assert!(
            reg.len() >= 50,
            "registry must have ≥50 vars, got {}",
            reg.len()
        );
    }
}

mod event_catalog_depth {
    use super::*;

    /// Catalog covers every SimEventCategory.
    #[test]
    fn catalog_covers_all_categories() {
        let categories = [
            SimEventCategory::FlightControls,
            SimEventCategory::Engine,
            SimEventCategory::Autopilot,
            SimEventCategory::Electrical,
            SimEventCategory::Radios,
            SimEventCategory::Views,
            SimEventCategory::Misc,
        ];
        for cat in categories {
            let events = catalog_by_category(cat);
            assert!(
                !events.is_empty(),
                "category {cat:?} must have at least one event"
            );
            for e in &events {
                assert_eq!(e.category, cat);
            }
        }
    }

    /// Catalog has ≥50 events total.
    #[test]
    fn catalog_has_minimum_events() {
        assert!(
            SIM_EVENT_CATALOG.len() >= 50,
            "got {}",
            SIM_EVENT_CATALOG.len()
        );
    }

    /// Autopilot events are all toggles; axis events are not.
    #[test]
    fn toggle_flag_correctness() {
        for ev in catalog_by_category(SimEventCategory::Autopilot) {
            assert!(ev.toggle, "{} should be toggle", ev.name);
        }
        assert!(!catalog_lookup("AXIS_ELEVATOR_SET").unwrap().toggle);
        assert!(!catalog_lookup("FLAPS_INCR").unwrap().toggle);
    }
}

mod health_monitor_depth {
    use super::*;

    /// Health monitor stale detection with short timeout.
    #[test]
    fn health_monitor_stale_detection() {
        let mut hm = HealthMonitor::new(Duration::from_millis(1));
        assert!(!hm.is_stale()); // No message yet ≠ stale.
        assert!(!hm.has_received());

        hm.record_message();
        assert!(hm.has_received());
        assert!(!hm.is_stale()); // Just recorded.

        std::thread::sleep(Duration::from_millis(5));
        assert!(hm.is_stale());

        // Reset clears everything.
        hm.reset();
        assert!(!hm.has_received());
        assert!(!hm.is_stale());
    }

    /// Backoff doubles and caps correctly.
    #[test]
    fn backoff_doubling_and_cap() {
        let mut b = ExponentialBackoff::new(Duration::from_millis(100), Duration::from_millis(500));
        assert_eq!(b.next_delay(), Duration::from_millis(100));
        assert_eq!(b.next_delay(), Duration::from_millis(200));
        assert_eq!(b.next_delay(), Duration::from_millis(400));
        assert_eq!(b.next_delay(), Duration::from_millis(500)); // capped
        assert_eq!(b.next_delay(), Duration::from_millis(500)); // still capped

        b.reset();
        assert_eq!(b.attempt(), 0);
        assert_eq!(b.next_delay(), Duration::from_millis(100));
    }
}

mod mock_backend_depth {
    use super::*;

    /// Mock backend tracks all state correctly.
    #[test]
    fn mock_backend_state_tracking() {
        let mut mock = MockSimConnectBackend::new();
        assert!(!mock.is_open());

        mock.open("test-app").unwrap();
        assert!(mock.is_open());

        // Add definitions.
        mock.add_to_data_definition(1, "AILERON POSITION", "position")
            .unwrap();
        mock.add_to_data_definition(1, "ELEVATOR POSITION", "position")
            .unwrap();
        assert_eq!(mock.definitions()[&1].len(), 2);

        // Map events.
        mock.map_client_event(10, "GEAR_TOGGLE").unwrap();
        assert_eq!(mock.mapped_events()[&10], "GEAR_TOGGLE");

        // Subscribe system events.
        mock.subscribe_system_event(20, "SimStart").unwrap();
        assert_eq!(mock.system_events()[&20], "SimStart");

        // Request data.
        mock.request_data(1, 1).unwrap();
        assert_eq!(mock.active_requests()[&1], 1);

        // Transmit events.
        mock.transmit_event(10, 42).unwrap();
        assert_eq!(mock.transmitted_events(), &[(10, 42)]);

        // Close clears everything.
        mock.close().unwrap();
        assert!(!mock.is_open());
        assert!(mock.definitions().is_empty());
        assert!(mock.mapped_events().is_empty());
    }

    /// Mock backend operations fail when not open.
    #[test]
    fn mock_backend_fails_when_closed() {
        let mut mock = MockSimConnectBackend::new();
        assert!(mock.add_to_data_definition(1, "V", "u").is_err());
        assert!(mock.request_data(1, 1).is_err());
        assert!(mock.map_client_event(1, "E").is_err());
        assert!(mock.transmit_event(1, 0).is_err());
        assert!(mock.subscribe_system_event(1, "E").is_err());
        assert!(mock.get_next_dispatch().is_err());
    }
}
