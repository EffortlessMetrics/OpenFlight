// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! DCS adapter unit tests
//!
//! Tests JSON parsing, field mapping, nil handling, MP status annotation,
//! and connection timeout detection per requirements DCS-INT-01.7, DCS-INT-01.8,
//! DCS-INT-01.11, DCS-INT-01.13, DCS-INT-01.15, SIM-TEST-01.4

use flight_dcs_export::{DcsAdapter, DcsAdapterConfig};
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
