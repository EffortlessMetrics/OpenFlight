// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! DCS adapter unit tests
//!
//! Tests JSON parsing, field mapping, nil handling, MP status annotation,
//! and connection timeout detection per requirements DCS-INT-01.7, DCS-INT-01.8,
//! DCS-INT-01.11, DCS-INT-01.13, DCS-INT-01.15, SIM-TEST-01.4

use flight_dcs_export::{AdapterState, DcsAdapter, DcsAdapterConfig, DcsMessage, ProtocolVersion};
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;

/// Helper to create test adapter
fn create_test_adapter() -> DcsAdapter {
    let config = DcsAdapterConfig::default();
    DcsAdapter::new(config)
}

/// Helper to create telemetry data map
fn create_telemetry_data(values: serde_json::Value) -> HashMap<String, serde_json::Value> {
    values.as_object().unwrap().clone().into_iter().collect()
}

#[test]
fn test_json_parsing_and_field_mapping() {
    let adapter = create_test_adapter();

    // Create telemetry data with all core fields
    let data = create_telemetry_data(json!({
        "ias": 350.0,
        "tas": 380.0,
        "altitude_asl": 15000.0,
        "heading": 90.0,
        "pitch": 3.0,
        "bank": -5.0,
        "vertical_speed": 0.0,
        "g_force": 1.1,
        "g_lateral": -0.05,
        "g_longitudinal": 0.15,
        "latitude": 45.5,
        "longitude": -122.8
    }));

    let snapshot = adapter
        .convert_to_bus_snapshot(1000, "F-16C", &data)
        .expect("Should convert telemetry to snapshot");

    // Verify field mapping
    assert_eq!(snapshot.kinematics.ias.value(), 350.0);
    assert_eq!(snapshot.kinematics.tas.value(), 380.0);
    assert_eq!(snapshot.environment.altitude, 15000.0);
    assert_eq!(snapshot.kinematics.heading.value(), 90.0);
    assert_eq!(snapshot.kinematics.pitch.value(), 3.0);
    assert_eq!(snapshot.kinematics.bank.value(), -5.0);
    assert_eq!(snapshot.kinematics.vertical_speed, 0.0);
    assert_eq!(snapshot.kinematics.g_force.value(), 1.1);
    assert_eq!(snapshot.kinematics.g_lateral.value(), -0.05);
    assert_eq!(snapshot.kinematics.g_longitudinal.value(), 0.15);
    assert_eq!(snapshot.navigation.latitude, 45.5);
    assert_eq!(snapshot.navigation.longitude, -122.8);
}

#[test]
fn test_nil_handling_graceful_degradation() {
    let adapter = create_test_adapter();

    // Create telemetry data with some nil (missing) fields
    let data = create_telemetry_data(json!({
        "ias": 350.0,
        "tas": 380.0,
        // Missing: altitude_asl, heading, pitch, bank
        "g_force": 1.1,
        "latitude": 45.5,
        "longitude": -122.8
    }));

    let snapshot = adapter
        .convert_to_bus_snapshot(1000, "F-16C", &data)
        .expect("Should handle missing fields gracefully");

    // Verify present fields are mapped
    assert_eq!(snapshot.kinematics.ias.value(), 350.0);
    assert_eq!(snapshot.kinematics.tas.value(), 380.0);
    assert_eq!(snapshot.kinematics.g_force.value(), 1.1);
    assert_eq!(snapshot.navigation.latitude, 45.5);
    assert_eq!(snapshot.navigation.longitude, -122.8);

    // Verify missing fields have default values (0.0 for validated types)
    assert_eq!(snapshot.environment.altitude, 0.0);
    assert_eq!(snapshot.kinematics.heading.value(), 0.0);
    assert_eq!(snapshot.kinematics.pitch.value(), 0.0);
    assert_eq!(snapshot.kinematics.bank.value(), 0.0);
}

#[test]
fn test_nil_handling_with_null_values() {
    let adapter = create_test_adapter();

    // Create telemetry data with explicit null values
    let data = create_telemetry_data(json!({
        "ias": 350.0,
        "tas": null,
        "altitude_asl": 15000.0,
        "heading": null,
        "pitch": 3.0,
        "bank": null,
        "g_force": 1.1
    }));

    let snapshot = adapter
        .convert_to_bus_snapshot(1000, "F-16C", &data)
        .expect("Should handle null values gracefully");

    // Verify non-null fields are mapped
    assert_eq!(snapshot.kinematics.ias.value(), 350.0);
    assert_eq!(snapshot.environment.altitude, 15000.0);
    assert_eq!(snapshot.kinematics.pitch.value(), 3.0);
    assert_eq!(snapshot.kinematics.g_force.value(), 1.1);

    // Verify null fields have default values
    assert_eq!(snapshot.kinematics.tas.value(), 0.0);
    assert_eq!(snapshot.kinematics.heading.value(), 0.0);
    assert_eq!(snapshot.kinematics.bank.value(), 0.0);
}

#[test]
fn test_mp_status_annotation_single_player() {
    let mut adapter = create_test_adapter();

    // Set up single player session
    let session_data = json!({
        "session_type": "SP",
        "mission_name": "Test Mission"
    });

    adapter
        .update_mp_session(&session_data)
        .expect("Should update session");

    // Verify SP session allows all features
    assert!(adapter.check_feature_blocked("telemetry_weapons").is_none());
    assert!(
        adapter
            .check_feature_blocked("telemetry_countermeasures")
            .is_none()
    );
    assert!(!adapter.is_multiplayer());
}

#[test]
fn test_mp_status_annotation_multiplayer() {
    let mut adapter = create_test_adapter();

    // Set up multiplayer session
    let session_data = json!({
        "session_type": "MP",
        "server_name": "Test Server",
        "player_count": 5
    });

    adapter
        .update_mp_session(&session_data)
        .expect("Should update session");

    // Verify MP session blocks restricted features
    let weapons_msg = adapter.check_feature_blocked("telemetry_weapons");
    assert!(weapons_msg.is_some());
    assert!(weapons_msg.unwrap().contains("multiplayer integrity"));

    let cm_msg = adapter.check_feature_blocked("telemetry_countermeasures");
    assert!(cm_msg.is_some());

    // Verify MP session allows basic features
    assert!(adapter.check_feature_blocked("telemetry_basic").is_none());

    // Verify MP detection
    assert!(adapter.is_multiplayer());
}

#[test]
fn test_mp_status_no_invalidation_of_self_aircraft_data() {
    let mut adapter = create_test_adapter();

    // Set up multiplayer session
    let session_data = json!({
        "session_type": "MP",
        "server_name": "Test Server"
    });

    adapter
        .update_mp_session(&session_data)
        .expect("Should update session");

    // Create telemetry data with self-aircraft data (should be allowed in MP)
    let data = create_telemetry_data(json!({
        "ias": 350.0,
        "tas": 380.0,
        "altitude_asl": 15000.0,
        "heading": 90.0,
        "pitch": 3.0,
        "bank": -5.0,
        "g_force": 1.1
    }));

    // Should successfully convert even in MP session
    let snapshot = adapter
        .convert_to_bus_snapshot(1000, "F-16C", &data)
        .expect("Should convert self-aircraft data in MP session");

    // Verify all self-aircraft fields are populated
    assert_eq!(snapshot.kinematics.ias.value(), 350.0);
    assert_eq!(snapshot.kinematics.tas.value(), 380.0);
    assert_eq!(snapshot.environment.altitude, 15000.0);
    assert_eq!(snapshot.kinematics.heading.value(), 90.0);
    assert_eq!(snapshot.kinematics.pitch.value(), 3.0);
    assert_eq!(snapshot.kinematics.bank.value(), -5.0);
    assert_eq!(snapshot.kinematics.g_force.value(), 1.1);
}

#[test]
fn test_connection_timeout_detection() {
    let adapter = create_test_adapter();

    // No connection - should not timeout
    assert!(!adapter.is_connection_timeout());
    assert!(adapter.time_since_last_telemetry().is_none());
}

#[test]
fn test_aircraft_change_detection() {
    let adapter = create_test_adapter();

    // Create telemetry for F-16C
    let data_f16 = create_telemetry_data(json!({
        "ias": 350.0,
        "tas": 380.0
    }));

    let snapshot_f16 = adapter
        .convert_to_bus_snapshot(1000, "F-16C", &data_f16)
        .expect("Should convert F-16C telemetry");

    assert_eq!(snapshot_f16.aircraft.icao, "F-16C");

    // Create telemetry for Ka-50
    let data_ka50 = create_telemetry_data(json!({
        "ias": 5.0,
        "tas": 5.0
    }));

    let snapshot_ka50 = adapter
        .convert_to_bus_snapshot(2000, "Ka-50", &data_ka50)
        .expect("Should convert Ka-50 telemetry");

    assert_eq!(snapshot_ka50.aircraft.icao, "Ka-50");

    // Verify aircraft identifiers are different
    assert_ne!(snapshot_f16.aircraft.icao, snapshot_ka50.aircraft.icao);
}

#[test]
fn test_engine_data_parsing() {
    let adapter = create_test_adapter();

    // Create telemetry with engine data
    let data = create_telemetry_data(json!({
        "ias": 350.0,
        "engines": {
            "0": {
                "rpm": 85.0,
                "temperature": 650.0,
                "fuel_flow": 1200.0
            },
            "1": {
                "rpm": 87.0,
                "temperature": 655.0,
                "fuel_flow": 1250.0
            }
        }
    }));

    let snapshot = adapter
        .convert_to_bus_snapshot(1000, "F-15C", &data)
        .expect("Should convert engine data");

    // Verify engine data is parsed
    assert_eq!(snapshot.engines.len(), 2);

    let engine_0 = snapshot.engines.iter().find(|e| e.index == 0).unwrap();
    assert_eq!(engine_0.rpm.value(), 85.0);
    assert_eq!(engine_0.egt, Some(650.0));
    assert_eq!(engine_0.fuel_flow, Some(1200.0));

    let engine_1 = snapshot.engines.iter().find(|e| e.index == 1).unwrap();
    assert_eq!(engine_1.rpm.value(), 87.0);
    assert_eq!(engine_1.egt, Some(655.0));
    assert_eq!(engine_1.fuel_flow, Some(1250.0));
}

#[test]
fn test_engine_data_with_missing_fields() {
    let adapter = create_test_adapter();

    // Create telemetry with partial engine data
    let data = create_telemetry_data(json!({
        "ias": 350.0,
        "engines": {
            "0": {
                "rpm": 85.0
                // Missing: temperature, fuel_flow
            }
        }
    }));

    let snapshot = adapter
        .convert_to_bus_snapshot(1000, "F-16C", &data)
        .expect("Should handle missing engine fields");

    // Verify engine data is parsed with defaults
    assert_eq!(snapshot.engines.len(), 1);

    let engine_0 = snapshot.engines.iter().find(|e| e.index == 0).unwrap();
    assert_eq!(engine_0.rpm.value(), 85.0);
    assert_eq!(engine_0.egt, None);
    assert_eq!(engine_0.fuel_flow, None);
}

#[test]
fn test_unit_conversions() {
    let adapter = create_test_adapter();

    // DCS uses degrees for angles, knots for speeds
    let data = create_telemetry_data(json!({
        "ias": 350.0,        // knots
        "tas": 380.0,        // knots
        "heading": 90.0,     // degrees
        "pitch": 3.0,        // degrees
        "bank": -5.0         // degrees
    }));

    let snapshot = adapter
        .convert_to_bus_snapshot(1000, "F-16C", &data)
        .expect("Should convert units");

    // Verify speeds are in knots (ValidatedSpeed stores in knots)
    assert_eq!(snapshot.kinematics.ias.value(), 350.0);
    assert_eq!(snapshot.kinematics.tas.value(), 380.0);

    // Verify angles are in degrees (ValidatedAngle stores in degrees)
    assert_eq!(snapshot.kinematics.heading.value(), 90.0);
    assert_eq!(snapshot.kinematics.pitch.value(), 3.0);
    assert_eq!(snapshot.kinematics.bank.value(), -5.0);
}

#[test]
fn test_timestamp_conversion() {
    let adapter = create_test_adapter();

    let data = create_telemetry_data(json!({
        "ias": 350.0
    }));

    // DCS timestamp is in milliseconds
    let dcs_timestamp_ms = 1000u64;

    let snapshot1 = adapter
        .convert_to_bus_snapshot(dcs_timestamp_ms, "F-16C", &data)
        .expect("Should convert timestamp");

    // Sleep a tiny bit to ensure time passes
    std::thread::sleep(std::time::Duration::from_millis(1));

    let snapshot2 = adapter
        .convert_to_bus_snapshot(dcs_timestamp_ms + 100, "F-16C", &data)
        .expect("Should convert timestamp");

    // Verify timestamp is monotonic
    assert!(snapshot1.timestamp > 0);
    assert!(snapshot2.timestamp >= snapshot1.timestamp);
}

#[test]
fn test_invalid_field_values() {
    let adapter = create_test_adapter();

    // Create telemetry with out-of-range values
    let data = create_telemetry_data(json!({
        "ias": 2000.0,  // Exceeds ValidatedSpeed max (1000 knots)
        "tas": 380.0,
        "g_force": 1.1
    }));

    // Should fail validation
    let result = adapter.convert_to_bus_snapshot(1000, "F-16C", &data);
    assert!(result.is_err());
}

#[test]
fn test_blocked_features_list() {
    let mut adapter = create_test_adapter();

    // Set up multiplayer session
    let session_data = json!({
        "session_type": "MP"
    });

    adapter
        .update_mp_session(&session_data)
        .expect("Should update session");

    // Get blocked features
    let blocked = adapter.blocked_features();

    // Verify restricted features are blocked
    assert!(blocked.contains(&"telemetry_weapons".to_string()));
    assert!(blocked.contains(&"telemetry_countermeasures".to_string()));
    assert!(blocked.contains(&"telemetry_rwr".to_string()));
}

#[test]
fn test_mp_banner_message() {
    let mut adapter = create_test_adapter();

    // Single player - no banner
    let sp_data = json!({
        "session_type": "SP"
    });

    adapter
        .update_mp_session(&sp_data)
        .expect("Should update session");

    assert!(adapter.mp_session_info().is_none());

    // Multiplayer - show banner
    let mp_data = json!({
        "session_type": "MP",
        "server_name": "Test Server"
    });

    adapter
        .update_mp_session(&mp_data)
        .expect("Should update session");

    let banner = adapter.mp_session_info();
    assert!(banner.is_some());
    let banner_text = banner.unwrap();
    assert!(banner_text.contains("Test Server"));
    assert!(banner_text.contains("Multiplayer"));
}

#[test]
fn test_connection_timeout_value() {
    let config = DcsAdapterConfig::default();

    // Verify connection timeout is 2 seconds per requirements
    assert_eq!(config.connection_timeout, Duration::from_secs(2));
}

#[test]
fn test_snapshot_validation() {
    let adapter = create_test_adapter();

    // Create valid telemetry
    let data = create_telemetry_data(json!({
        "ias": 350.0,
        "tas": 380.0,
        "g_force": 1.1
    }));

    let snapshot = adapter
        .convert_to_bus_snapshot(1000, "F-16C", &data)
        .expect("Should convert telemetry");

    // Verify snapshot validates successfully
    assert!(snapshot.validate().is_ok());
}

#[test]
fn test_empty_telemetry_data() {
    let adapter = create_test_adapter();

    // Create empty telemetry data
    let data = create_telemetry_data(json!({}));

    let snapshot = adapter
        .convert_to_bus_snapshot(1000, "F-16C", &data)
        .expect("Should handle empty telemetry");

    // Verify snapshot has default values
    assert_eq!(snapshot.kinematics.ias.value(), 0.0);
    assert_eq!(snapshot.kinematics.tas.value(), 0.0);
    assert_eq!(snapshot.environment.altitude, 0.0);
}

#[test]
fn test_fixture_f16_cruise() {
    let adapter = create_test_adapter();

    // Load fixture
    let fixture_json = include_str!("fixtures/dcs_f16_cruise.json");
    let fixture: serde_json::Value = serde_json::from_str(fixture_json).unwrap();

    let lua_values = fixture.get("lua_values").unwrap();
    let data = create_telemetry_data(lua_values.clone());

    let snapshot = adapter
        .convert_to_bus_snapshot(1000, "F-16C", &data)
        .expect("Should convert F-16 cruise fixture");

    // Verify expected values from fixture
    let expected = fixture.get("expected_bus_values").unwrap();
    assert_eq!(
        snapshot.kinematics.ias.value(),
        expected.get("ias_knots").unwrap().as_f64().unwrap() as f32
    );
    assert_eq!(
        snapshot.kinematics.tas.value(),
        expected.get("tas_knots").unwrap().as_f64().unwrap() as f32
    );
    assert_eq!(
        snapshot.environment.altitude,
        expected.get("altitude_ft").unwrap().as_f64().unwrap() as f32
    );
}

#[test]
fn test_fixture_ka50_hover() {
    let adapter = create_test_adapter();

    // Load fixture
    let fixture_json = include_str!("fixtures/dcs_ka50_hover.json");
    let fixture: serde_json::Value = serde_json::from_str(fixture_json).unwrap();

    let lua_values = fixture.get("lua_values").unwrap();
    let data = create_telemetry_data(lua_values.clone());

    let snapshot = adapter
        .convert_to_bus_snapshot(1000, "Ka-50", &data)
        .expect("Should convert Ka-50 hover fixture");

    // Verify expected values from fixture
    let expected = fixture.get("expected_bus_values").unwrap();
    assert_eq!(
        snapshot.kinematics.ias.value(),
        expected.get("ias_knots").unwrap().as_f64().unwrap() as f32
    );

    // Verify helicopter has multiple engines
    assert_eq!(snapshot.engines.len(), 2);
}

#[test]
fn test_restricted_fields_filtered_in_mp() {
    let mut adapter = create_test_adapter();

    let session_data = json!({"session_type": "MP", "server_name": "Test Server"});
    adapter
        .update_mp_session(&session_data)
        .expect("Should update session");

    let mut data = HashMap::new();
    data.insert("ias".to_string(), json!(350.0));
    data.insert("weapons".to_string(), json!({"missile": "AIM-120C"}));
    data.insert("countermeasures".to_string(), json!({"chaff": 30}));
    data.insert("rwr_contacts".to_string(), json!([]));

    let (filtered, blocked) = adapter.filter_restricted_fields(data);

    // Restricted fields must be stripped
    assert!(!filtered.contains_key("weapons"));
    assert!(!filtered.contains_key("countermeasures"));
    assert!(!filtered.contains_key("rwr_contacts"));
    // Safe field preserved
    assert!(filtered.contains_key("ias"));
    // All three restricted fields reported as blocked
    assert!(blocked.contains(&"weapons".to_string()));
    assert!(blocked.contains(&"countermeasures".to_string()));
    assert!(blocked.contains(&"rwr_contacts".to_string()));
}

#[test]
fn test_restricted_fields_allowed_in_sp() {
    let mut adapter = create_test_adapter();

    let session_data = json!({"session_type": "SP"});
    adapter
        .update_mp_session(&session_data)
        .expect("Should update session");

    let mut data = HashMap::new();
    data.insert("ias".to_string(), json!(350.0));
    data.insert("weapons".to_string(), json!({"missile": "AIM-120C"}));

    let (filtered, blocked) = adapter.filter_restricted_fields(data);

    // All fields preserved in SP
    assert!(filtered.contains_key("weapons"));
    assert!(filtered.contains_key("ias"));
    assert!(blocked.is_empty());
}

#[test]
fn test_aoa_mapping() {
    let adapter = create_test_adapter();

    let data = create_telemetry_data(json!({
        "ias": 350.0,
        "aoa": 4.5
    }));

    let snapshot = adapter
        .convert_to_bus_snapshot(1000, "F-16C", &data)
        .expect("Should map AoA");

    assert_eq!(snapshot.kinematics.aoa.value(), 4.5);
}

#[test]
fn test_angular_rates_mapping() {
    let adapter = create_test_adapter();

    let data = create_telemetry_data(json!({
        "ias": 350.0,
        "angular_velocity_x": 0.05,
        "angular_velocity_y": -0.02,
        "angular_velocity_z": 0.01
    }));

    let snapshot = adapter
        .convert_to_bus_snapshot(1000, "F-16C", &data)
        .expect("Should map angular rates");

    assert!((snapshot.angular_rates.p - 0.05_f32).abs() < 1e-5);
    assert!((snapshot.angular_rates.q - (-0.02_f32)).abs() < 1e-5);
    assert!((snapshot.angular_rates.r - 0.01_f32).abs() < 1e-5);
}

#[test]
fn test_navigation_ground_track_and_distance() {
    let adapter = create_test_adapter();

    // Lua pre-converts distance to nautical miles; adapter passes through
    let data = create_telemetry_data(json!({
        "ias": 350.0,
        "course": 95.0,
        "waypoint_distance": 12.387  // NM (post-Lua conversion from meters)
    }));

    let snapshot = adapter
        .convert_to_bus_snapshot(1000, "F-16C", &data)
        .expect("Should map navigation");

    assert_eq!(snapshot.navigation.ground_track.value(), 95.0);
    assert!((snapshot.navigation.distance_to_dest.unwrap() - 12.387_f32).abs() < 0.001);
}

#[test]
fn test_gear_state_down() {
    let adapter = create_test_adapter();

    let data = create_telemetry_data(json!({
        "ias": 5.0,
        "gear_down": 1.0
    }));

    let snapshot = adapter
        .convert_to_bus_snapshot(1000, "F-16C", &data)
        .expect("Should map gear state");

    assert!(snapshot.config.gear.all_down());
}

#[test]
fn test_gear_state_up() {
    let adapter = create_test_adapter();

    let data = create_telemetry_data(json!({
        "ias": 350.0,
        "gear_down": 0.0
    }));

    let snapshot = adapter
        .convert_to_bus_snapshot(1000, "F-16C", &data)
        .expect("Should map gear state");

    assert!(snapshot.config.gear.all_up());
}

#[test]
fn test_gear_state_transitioning() {
    let adapter = create_test_adapter();

    let data = create_telemetry_data(json!({
        "ias": 180.0,
        "gear_down": 0.5
    }));

    let snapshot = adapter
        .convert_to_bus_snapshot(1000, "F-16C", &data)
        .expect("Should map gear state");

    assert!(snapshot.config.gear.transitioning());
}

#[test]
fn test_flaps_mapping() {
    let adapter = create_test_adapter();

    let data = create_telemetry_data(json!({
        "ias": 150.0,
        "flaps": 30.0
    }));

    let snapshot = adapter
        .convert_to_bus_snapshot(1000, "F-16C", &data)
        .expect("Should map flaps");

    assert_eq!(snapshot.config.flaps.value(), 30.0);
}

#[test]
fn test_flaps_clamped_at_bounds() {
    let adapter = create_test_adapter();

    // Flaps values outside 0-100 should be clamped before Percentage::new
    let data = create_telemetry_data(json!({
        "ias": 100.0,
        "flaps": 110.0
    }));

    let snapshot = adapter
        .convert_to_bus_snapshot(1000, "F-16C", &data)
        .expect("Flaps out-of-range should be clamped");

    assert_eq!(snapshot.config.flaps.value(), 100.0);
}

// ---------------------------------------------------------------------------
// State machine tests
// ---------------------------------------------------------------------------

#[test]
fn test_state_machine_initial_state_is_disconnected() {
    let adapter = create_test_adapter();
    assert_eq!(adapter.state(), AdapterState::Disconnected);
}

// ---------------------------------------------------------------------------
// Multiple consecutive telemetry packets
// ---------------------------------------------------------------------------

#[test]
fn test_multiple_consecutive_telemetry_packets() {
    let adapter = create_test_adapter();

    let frames: Vec<(u64, &str, serde_json::Value)> = vec![
        (1000, "F-16C", json!({"ias": 200.0, "tas": 210.0, "altitude_asl": 5000.0})),
        (1033, "F-16C", json!({"ias": 210.0, "tas": 221.0, "altitude_asl": 5100.0})),
        (1066, "F-16C", json!({"ias": 220.0, "tas": 232.0, "altitude_asl": 5200.0})),
        (1099, "F-16C", json!({"ias": 230.0, "tas": 243.0, "altitude_asl": 5300.0})),
    ];

    let mut last_ts: Option<u64> = None;
    for (ts, aircraft, frame) in &frames {
        let data = create_telemetry_data(frame.clone());
        let snapshot = adapter
            .convert_to_bus_snapshot(*ts, aircraft, &data)
            .expect("Should convert consecutive telemetry");
        assert_eq!(snapshot.aircraft.icao, *aircraft);
        // Bus timestamps must be monotonically non-decreasing
        if let Some(prev) = last_ts {
            assert!(snapshot.timestamp >= prev);
        }
        last_ts = Some(snapshot.timestamp);
    }
}

#[test]
fn test_consecutive_telemetry_different_aircraft() {
    let adapter = create_test_adapter();

    let aircraft_frames = [
        ("F-16C", json!({"ias": 350.0})),
        ("Ka-50", json!({"ias": 5.0})),
        ("A-10C", json!({"ias": 200.0})),
    ];

    for (aircraft, frame) in &aircraft_frames {
        let data = create_telemetry_data(frame.clone());
        let snapshot = adapter
            .convert_to_bus_snapshot(1000, aircraft, &data)
            .expect("Should convert telemetry for each aircraft");
        assert_eq!(snapshot.aircraft.icao, *aircraft);
    }
}

// ---------------------------------------------------------------------------
// Malformed packet handling — must not panic
// ---------------------------------------------------------------------------

#[test]
fn test_malformed_dcs_message_does_not_panic() {
    // Malformed inputs must never cause a panic; returning a parse error is fine.
    let malformed_inputs: &[&str] = &[
        "",
        "not json at all",
        "{}",
        r#"{"type": "UnknownVariant", "data": {}}"#,
        r#"{"type": "Telemetry"}"#, // missing 'data' key
        "null",
        "[]",
        r#"{"type": "Telemetry", "data": {"timestamp": "not_a_number"}}"#,
        &"x".repeat(1_000), // long invalid input (value created below)
    ];

    for input in malformed_inputs {
        let _ = serde_json::from_str::<DcsMessage>(input);
    }
}

#[test]
fn test_malformed_dcs_message_long_input_does_not_panic() {
    // Very long invalid JSON input — must not panic or cause OOM
    let long_input = "x".repeat(65_536);
    let _ = serde_json::from_str::<DcsMessage>(&long_input);
}

#[test]
fn test_malformed_telemetry_field_types_do_not_panic() {
    let adapter = create_test_adapter();

    // Fields with wrong JSON types — adapter must skip them gracefully
    let bad_data = create_telemetry_data(json!({
        "ias": "not_a_number",
        "tas": true,
        "altitude_asl": {"nested": "object"},
        "heading": [],
        "g_force": null
    }));

    // Must not panic; may succeed with defaults or return an error
    let _ = adapter.convert_to_bus_snapshot(1000, "F-16C", &bad_data);
}

// ---------------------------------------------------------------------------
// DCS wire protocol (Export.lua → Flight Hub) packet format tests
// ---------------------------------------------------------------------------

#[test]
fn test_wire_protocol_handshake_roundtrip() {
    let msg = DcsMessage::Handshake {
        version: ProtocolVersion::V1_0,
        features: vec![
            "telemetry_basic".to_string(),
            "telemetry_navigation".to_string(),
            "session_detection".to_string(),
        ],
    };

    // Wire protocol: newline-delimited JSON
    let wire = format!("{}\n", serde_json::to_string(&msg).unwrap());
    let parsed: DcsMessage = serde_json::from_str(wire.trim()).unwrap();

    match parsed {
        DcsMessage::Handshake { version, features } => {
            assert_eq!(version, ProtocolVersion::V1_0);
            assert!(features.contains(&"telemetry_basic".to_string()));
            assert!(features.contains(&"telemetry_navigation".to_string()));
        }
        _ => panic!("Expected Handshake"),
    }
}

#[test]
fn test_wire_protocol_telemetry_roundtrip() {
    let mut data = HashMap::new();
    data.insert("ias".to_string(), serde_json::json!(350.0));
    data.insert("altitude_asl".to_string(), serde_json::json!(15000.0));
    data.insert("heading".to_string(), serde_json::json!(90.0));

    let msg = DcsMessage::Telemetry {
        timestamp: 123_456_789,
        aircraft: "F-16C".to_string(),
        session_type: "SP".to_string(),
        data,
    };

    let wire = format!("{}\n", serde_json::to_string(&msg).unwrap());
    let parsed: DcsMessage = serde_json::from_str(wire.trim()).unwrap();

    match parsed {
        DcsMessage::Telemetry {
            timestamp,
            aircraft,
            session_type,
            data,
        } => {
            assert_eq!(timestamp, 123_456_789);
            assert_eq!(aircraft, "F-16C");
            assert_eq!(session_type, "SP");
            assert!(data.contains_key("ias"));
            assert!(data.contains_key("altitude_asl"));
        }
        _ => panic!("Expected Telemetry"),
    }
}

#[test]
fn test_wire_protocol_heartbeat_roundtrip() {
    let msg = DcsMessage::Heartbeat { timestamp: 999 };
    let wire = format!("{}\n", serde_json::to_string(&msg).unwrap());
    let parsed: DcsMessage = serde_json::from_str(wire.trim()).unwrap();

    match parsed {
        DcsMessage::Heartbeat { timestamp } => assert_eq!(timestamp, 999),
        _ => panic!("Expected Heartbeat"),
    }
}

#[test]
fn test_wire_protocol_error_roundtrip() {
    let msg = DcsMessage::Error {
        code: "DCS_INIT_FAILED".to_string(),
        message: "DCS failed to initialise Export.lua".to_string(),
    };

    let wire = format!("{}\n", serde_json::to_string(&msg).unwrap());
    let parsed: DcsMessage = serde_json::from_str(wire.trim()).unwrap();

    match parsed {
        DcsMessage::Error { code, message } => {
            assert_eq!(code, "DCS_INIT_FAILED");
            assert!(message.contains("Export.lua"));
        }
        _ => panic!("Expected Error"),
    }
}

#[test]
fn test_wire_protocol_mp_telemetry_roundtrip() {
    // Verify MP session type is preserved across the wire
    let mut data = HashMap::new();
    data.insert("ias".to_string(), serde_json::json!(280.0));
    data.insert("weapons".to_string(), serde_json::json!({"missile": "AIM-120C"}));

    let msg = DcsMessage::Telemetry {
        timestamp: 500,
        aircraft: "F/A-18C".to_string(),
        session_type: "MP".to_string(),
        data,
    };

    let wire = format!("{}\n", serde_json::to_string(&msg).unwrap());
    let parsed: DcsMessage = serde_json::from_str(wire.trim()).unwrap();

    match parsed {
        DcsMessage::Telemetry { session_type, .. } => {
            assert_eq!(session_type, "MP");
        }
        _ => panic!("Expected Telemetry"),
    }
}

#[test]
fn test_wire_protocol_handshake_ack_roundtrip() {
    let msg = DcsMessage::HandshakeAck {
        version: ProtocolVersion::V1_0,
        accepted_features: vec!["telemetry_basic".to_string()],
    };

    let wire = format!("{}\n", serde_json::to_string(&msg).unwrap());
    let parsed: DcsMessage = serde_json::from_str(wire.trim()).unwrap();

    match parsed {
        DcsMessage::HandshakeAck {
            version,
            accepted_features,
        } => {
            assert_eq!(version, ProtocolVersion::V1_0);
            assert_eq!(accepted_features, vec!["telemetry_basic"]);
        }
        _ => panic!("Expected HandshakeAck"),
    }
}
