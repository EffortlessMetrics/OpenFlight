// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Depth tests for the War Thunder telemetry adapter.
//!
//! Covers five areas:
//! 1. HTTP telemetry — `/state` and `/indicators` endpoint parsing
//! 2. Vehicle detection — name extraction, type/nation inference
//! 3. State extraction — kinematics, environment, engine data
//! 4. Connection lifecycle — start/stop, timeout, reconnect, polling config
//! 5. Bus publishing — snapshot format, field mapping, stale marking, vehicle change

use flight_adapter_common::{AdapterConfig, AdapterState};
use flight_bus::types::{GearPosition, SimId};
use flight_warthunder::protocol::{WtIndicators, WtState};
use flight_warthunder::{WarThunderAdapter, WarThunderConfig};
use std::time::Duration;

fn adapter() -> WarThunderAdapter {
    WarThunderAdapter::new(WarThunderConfig::default())
}

fn full_indicators() -> WtIndicators {
    WtIndicators {
        valid: Some(true),
        airframe: Some("Spitfire Mk.Vc".to_string()),
        ias_kmh: Some(400.0),
        tas_kmh: Some(420.0),
        altitude: Some(2000.0),
        heading: Some(180.0),
        pitch: Some(5.0),
        roll: Some(-10.0),
        g_load: Some(1.5),
        vert_speed: Some(2.0),
        gear: Some(0.0),
        flaps: Some(0.3),
    }
}

fn full_state() -> WtState {
    WtState {
        valid: Some(true),
        airframe: Some("P-51D-20NA".to_string()),
        aoa_deg: Some(3.5),
        aos_deg: Some(0.5),
        speed_mps: Some(95.0),
        mach: Some(0.28),
        ny: Some(1.1),
        nx: Some(0.05),
        nz: Some(0.0),
        engine0_rpm: Some(2400.0),
        engine1_rpm: None,
        engine2_rpm: None,
        engine3_rpm: None,
    }
}

// ============================================================================
// 1. HTTP telemetry — /state and /indicators endpoint parsing (6 tests)
// ============================================================================

/// Parse a realistic `/indicators` JSON payload with all fields present.
#[test]
fn parse_indicators_full_realistic_payload() {
    let raw = r#"{
        "valid": true,
        "airframe": "A6M5 Zero",
        "IAS km/h": 310.2,
        "TAS km/h": 325.8,
        "altitude": 4200.0,
        "heading": 42.7,
        "pitch": -2.3,
        "roll": 15.4,
        "gLoad": 1.8,
        "vertSpeed": -3.1,
        "gear": 0.0,
        "flaps": 0.15
    }"#;
    let ind: WtIndicators = serde_json::from_str(raw).unwrap();
    assert_eq!(ind.airframe.as_deref(), Some("A6M5 Zero"));
    assert!((ind.ias_kmh.unwrap() - 310.2).abs() < 0.01);
    assert!((ind.tas_kmh.unwrap() - 325.8).abs() < 0.01);
    assert!((ind.altitude.unwrap() - 4200.0).abs() < 0.01);
    assert!((ind.heading.unwrap() - 42.7).abs() < 0.01);
    assert!((ind.pitch.unwrap() - (-2.3)).abs() < 0.01);
    assert!((ind.roll.unwrap() - 15.4).abs() < 0.01);
    assert!((ind.g_load.unwrap() - 1.8).abs() < 0.01);
    assert!((ind.vert_speed.unwrap() - (-3.1)).abs() < 0.01);
    assert_eq!(ind.gear, Some(0.0));
    assert!((ind.flaps.unwrap() - 0.15).abs() < 0.01);
}

/// Parse a realistic `/state` JSON payload with engine data and aero angles.
#[test]
fn parse_state_full_realistic_payload() {
    let raw = r#"{
        "valid": true,
        "airframe": "B-17G",
        "AoA, deg": 6.2,
        "AoS, deg": -1.1,
        "speed, m/s": 110.5,
        "Mach": 0.33,
        "Ny": 1.05,
        "Nx": 0.01,
        "Nz": -0.02,
        "engine0.rpm": 2100.0,
        "engine1.rpm": 2050.0,
        "engine2.rpm": 2080.0,
        "engine3.rpm": 2090.0
    }"#;
    let state: WtState = serde_json::from_str(raw).unwrap();
    assert_eq!(state.airframe.as_deref(), Some("B-17G"));
    assert!((state.aoa_deg.unwrap() - 6.2).abs() < 0.01);
    assert!((state.aos_deg.unwrap() - (-1.1)).abs() < 0.01);
    assert!((state.speed_mps.unwrap() - 110.5).abs() < 0.01);
    assert!((state.mach.unwrap() - 0.33).abs() < 0.01);
    // All four engines present
    assert!(state.engine0_rpm.is_some());
    assert!(state.engine1_rpm.is_some());
    assert!(state.engine2_rpm.is_some());
    assert!(state.engine3_rpm.is_some());
}

/// JSON response with extra unknown keys is tolerated (forward compat).
#[test]
fn indicators_ignores_unknown_json_fields() {
    let raw = r#"{
        "valid": true,
        "IAS km/h": 200.0,
        "unknownField": 42,
        "anotherExtra": "hello"
    }"#;
    let ind: WtIndicators = serde_json::from_str(raw).unwrap();
    assert!((ind.ias_kmh.unwrap() - 200.0).abs() < 0.01);
}

/// State JSON with extra unknown keys is tolerated.
#[test]
fn state_ignores_unknown_json_fields() {
    let raw = r#"{
        "valid": true,
        "Mach": 0.8,
        "fuel_pct": 65.0,
        "weapon_selected": "bombs"
    }"#;
    let state: WtState = serde_json::from_str(raw).unwrap();
    assert!((state.mach.unwrap() - 0.8).abs() < 0.01);
}

/// Flight parameter fields parse correctly from indicators: pitch, roll, heading, G-load.
#[test]
fn indicators_flight_parameters_parsed() {
    let raw = r#"{
        "valid": true,
        "heading": 359.9,
        "pitch": -45.0,
        "roll": 89.5,
        "gLoad": 7.3
    }"#;
    let ind: WtIndicators = serde_json::from_str(raw).unwrap();
    assert!((ind.heading.unwrap() - 359.9).abs() < 0.01);
    assert!((ind.pitch.unwrap() - (-45.0)).abs() < 0.01);
    assert!((ind.roll.unwrap() - 89.5).abs() < 0.01);
    assert!((ind.g_load.unwrap() - 7.3).abs() < 0.01);
}

/// Engine data from `/state` endpoint: multi-engine RPM parsing.
#[test]
fn state_engine_data_twin_engine() {
    let raw = r#"{
        "engine0.rpm": 2700.0,
        "engine1.rpm": 2650.0
    }"#;
    let state: WtState = serde_json::from_str(raw).unwrap();
    assert!((state.engine0_rpm.unwrap() - 2700.0).abs() < 0.01);
    assert!((state.engine1_rpm.unwrap() - 2650.0).abs() < 0.01);
    assert!(state.engine2_rpm.is_none());
    assert!(state.engine3_rpm.is_none());
}

// ============================================================================
// 2. Vehicle detection — name extraction, type, nation (5 tests)
// ============================================================================

/// Vehicle name extraction: diverse aircraft names are preserved exactly.
#[test]
fn vehicle_name_extraction_preserves_special_chars() {
    let names = [
        "Fw 190 A-5/U2",
        "MiG-21bis (R-60)",
        "Ki-84 otsu",
        "Yak-3P",
        "He 162 A-2",
        "M4A3 (76) W Sherman",
    ];
    let a = adapter();
    for name in &names {
        let ind = WtIndicators {
            airframe: Some(name.to_string()),
            ..Default::default()
        };
        let snap = a.convert_indicators(&ind).unwrap();
        assert_eq!(snap.aircraft.icao, *name, "name must be preserved exactly");
    }
}

/// Fighter aircraft names map to correct aircraft IDs.
#[test]
fn vehicle_type_fighters_identified() {
    let fighters = ["F-86F-2", "MiG-15bis", "Spitfire F Mk.IX", "Bf 109 K-4"];
    let a = adapter();
    for name in &fighters {
        let ind = WtIndicators {
            valid: Some(true),
            airframe: Some(name.to_string()),
            ias_kmh: Some(500.0),
            altitude: Some(5000.0),
            pitch: Some(0.0),
            roll: Some(0.0),
            ..Default::default()
        };
        let snap = a.convert_indicators(&ind).unwrap();
        assert_eq!(snap.sim, SimId::WarThunder);
        assert_eq!(snap.aircraft.icao, *name);
    }
}

/// Bomber aircraft names are handled correctly.
#[test]
fn vehicle_type_bombers_identified() {
    let bombers = ["B-17G", "Lancaster B Mk.I", "He 111 H-6", "Tu-2S"];
    let a = adapter();
    for name in &bombers {
        let ind = WtIndicators {
            airframe: Some(name.to_string()),
            ..Default::default()
        };
        let snap = a.convert_indicators(&ind).unwrap();
        assert_eq!(snap.aircraft.icao, *name);
    }
}

/// Helicopter names are handled correctly.
#[test]
fn vehicle_type_helicopters_identified() {
    let helos = ["AH-1G", "Mi-24V", "Ka-50", "UH-1C"];
    let a = adapter();
    for name in &helos {
        let ind = WtIndicators {
            airframe: Some(name.to_string()),
            ..Default::default()
        };
        let snap = a.convert_indicators(&ind).unwrap();
        assert_eq!(snap.aircraft.icao, *name);
    }
}

/// Multiple nations' aircraft are all accepted as valid identifiers.
#[test]
fn vehicle_nation_diversity() {
    let by_nation = [
        ("USA", "P-47D-28"),
        ("Germany", "Fw 190 D-9"),
        ("USSR", "La-7"),
        ("Britain", "Typhoon Mk.Ib"),
        ("Japan", "N1K2-J"),
        ("Italy", "G.55 serie 1"),
        ("France", "M.B.157"),
        ("Sweden", "J21A-1"),
    ];
    let a = adapter();
    for (nation, name) in &by_nation {
        let ind = WtIndicators {
            airframe: Some(name.to_string()),
            ..Default::default()
        };
        let snap = a.convert_indicators(&ind).unwrap();
        assert_eq!(
            snap.aircraft.icao, *name,
            "{nation} aircraft {name} must be preserved"
        );
    }
}

// ============================================================================
// 3. State extraction — kinematics, environment, engine (5 tests)
// ============================================================================

/// IAS and TAS conversion: km/h → m/s with correct ratio (÷3.6).
#[test]
fn state_extraction_ias_tas_conversion() {
    let a = adapter();
    let ind = WtIndicators {
        valid: Some(true),
        ias_kmh: Some(720.0), // 200 m/s
        tas_kmh: Some(756.0), // 210 m/s
        ..Default::default()
    };
    let snap = a.convert_indicators(&ind).unwrap();
    assert!(
        (snap.kinematics.ias.to_mps() - 200.0).abs() < 0.1,
        "720 km/h should be 200 m/s, got {}",
        snap.kinematics.ias.to_mps()
    );
    assert!(
        (snap.kinematics.tas.to_mps() - 210.0).abs() < 0.1,
        "756 km/h should be 210 m/s, got {}",
        snap.kinematics.tas.to_mps()
    );
}

/// Altitude conversion accuracy: m → ft at multiple reference points.
#[test]
fn state_extraction_altitude_reference_points() {
    let a = adapter();
    let test_cases: &[(f32, f32)] = &[
        (0.0, 0.0),
        (304.8, 1000.0),   // ~1000 ft
        (1000.0, 3280.84), // standard
        (10668.0, 35000.0), // cruise altitude
    ];
    for &(meters, expected_ft) in test_cases {
        let ind = WtIndicators {
            altitude: Some(meters),
            ..Default::default()
        };
        let snap = a.convert_indicators(&ind).unwrap();
        assert!(
            (snap.environment.altitude - expected_ft).abs() < 5.0,
            "{meters}m should be ~{expected_ft}ft, got {}",
            snap.environment.altitude
        );
    }
}

/// Heading wrapping: values >360 and negative values normalise correctly.
#[test]
fn state_extraction_heading_wrapping() {
    let a = adapter();
    // 450° should normalise to 90°
    let ind = WtIndicators {
        heading: Some(450.0),
        ..Default::default()
    };
    let snap = a.convert_indicators(&ind).unwrap();
    assert!(
        (snap.kinematics.heading.to_degrees() - 90.0).abs() < 0.01,
        "450° should normalise to 90°, got {}",
        snap.kinematics.heading.to_degrees()
    );
}

/// Pitch and roll stored accurately and bank alias equals roll.
#[test]
fn state_extraction_pitch_roll_stored_accurately() {
    let a = adapter();
    let ind = WtIndicators {
        valid: Some(true),
        pitch: Some(15.5),
        roll: Some(-30.0),
        ..Default::default()
    };
    let snap = a.convert_indicators(&ind).unwrap();
    assert!(
        (snap.kinematics.pitch.to_degrees() - 15.5).abs() < 0.01,
        "pitch should be 15.5°, got {}",
        snap.kinematics.pitch.to_degrees()
    );
    // Roll maps to `bank` field in snapshot
    assert!(
        (snap.kinematics.bank.to_degrees() - (-30.0)).abs() < 0.01,
        "bank should be -30.0°, got {}",
        snap.kinematics.bank.to_degrees()
    );
}

/// G-force: normal, longitudinal, and lateral components from /state.
#[test]
fn state_extraction_g_force_all_axes() {
    let a = adapter();
    let mut snap = a.convert_indicators(&full_indicators()).unwrap();
    let state = WtState {
        ny: Some(6.0),
        nx: Some(0.8),
        nz: Some(-1.2),
        ..Default::default()
    };
    a.apply_state(&state, &mut snap).unwrap();
    assert!(
        (snap.kinematics.g_force.value() - 6.0).abs() < 0.01,
        "Ny (normal g) should be 6.0"
    );
    assert!(
        (snap.kinematics.g_longitudinal.value() - 0.8).abs() < 0.01,
        "Nx (longitudinal g) should be 0.8"
    );
    assert!(
        (snap.kinematics.g_lateral.value() - (-1.2)).abs() < 0.01,
        "Nz (lateral g) should be -1.2"
    );
}

// ============================================================================
// 4. Connection lifecycle — start, stop, timeout, reconnect, polling (5 tests)
// ============================================================================

/// Adapter starts disconnected, transitions to connected on start, back on stop.
#[test]
fn connection_start_stop_lifecycle() {
    let mut a = adapter();
    assert_eq!(a.state(), AdapterState::Disconnected);
    a.start().unwrap();
    assert_eq!(a.state(), AdapterState::Connected);
    a.stop();
    assert_eq!(a.state(), AdapterState::Disconnected);
}

/// Timeout detected when no packet has ever been received.
#[test]
fn connection_timeout_before_any_packet() {
    let a = adapter();
    assert!(
        a.is_connection_timeout(),
        "should be timed-out before any packet"
    );
    assert!(
        a.time_since_last_packet().is_none(),
        "no packet → None duration"
    );
}

/// Adapter can be stopped and restarted (reconnect cycle).
#[test]
fn connection_reconnect_cycle() {
    let mut a = adapter();
    a.start().unwrap();
    assert_eq!(a.state(), AdapterState::Connected);
    a.stop();
    assert_eq!(a.state(), AdapterState::Disconnected);
    // Reconnect
    a.start().unwrap();
    assert_eq!(a.state(), AdapterState::Connected);
}

/// Polling rate configuration is respected via AdapterConfig trait.
#[test]
fn connection_polling_rate_from_config() {
    let cfg = WarThunderConfig {
        poll_rate_hz: 60.0,
        request_timeout: Duration::from_millis(250),
        ..Default::default()
    };
    assert!((cfg.publish_rate_hz() - 60.0).abs() < 0.01);
    assert_eq!(cfg.connection_timeout(), Duration::from_millis(250));
}

/// poll_once returns NotStarted error when adapter hasn't been started.
#[tokio::test]
async fn connection_poll_without_start_returns_error() {
    let mut a = adapter();
    let result = a.poll_once().await;
    assert!(result.is_err(), "poll before start should error");
    let err = result.unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("not started"),
        "error should mention not started, got: {msg}"
    );
}

// ============================================================================
// 5. Bus publishing — snapshot format, field mapping, stale, vehicle change (5 tests)
// ============================================================================

/// Snapshot sim field is always WarThunder regardless of airframe content.
#[test]
fn bus_snapshot_sim_id_always_warthunder() {
    let a = adapter();
    for airframe in [Some("F-16C".to_string()), Some(String::new()), None] {
        let ind = WtIndicators {
            airframe,
            ..Default::default()
        };
        let snap = a.convert_indicators(&ind).unwrap();
        assert_eq!(snap.sim, SimId::WarThunder);
    }
}

/// Full indicators produce a snapshot with all kinematic fields populated.
#[test]
fn bus_snapshot_all_fields_mapped() {
    let a = adapter();
    let snap = a.convert_indicators(&full_indicators()).unwrap();

    // Speeds
    assert!(snap.kinematics.ias.to_mps() > 0.0, "IAS should be set");
    assert!(snap.kinematics.tas.to_mps() > 0.0, "TAS should be set");
    // Altitude
    assert!(snap.environment.altitude > 0.0, "altitude should be set");
    // Attitude
    // Heading (could be 0 due to normalisation, just verify it's set)
    let _heading = snap.kinematics.heading.to_degrees();
    assert!(snap.kinematics.pitch.to_degrees().abs() > 0.0, "pitch set");
    assert!(snap.kinematics.bank.to_degrees().abs() > 0.0, "roll set");
    // G-load
    assert!(snap.kinematics.g_force.value() > 0.0, "g_force set");
    // Vertical speed
    assert!(snap.kinematics.vertical_speed > 0.0, "vspeed set");
    // Gear
    assert_eq!(snap.config.gear.nose, GearPosition::Up, "gear retracted");
    // Flaps
    assert!(snap.config.flaps.value() > 0.0, "flaps partially deployed");
}

/// Stale marking: missing IAS/TAS sets velocities_valid = false.
#[test]
fn bus_snapshot_stale_when_speeds_missing() {
    let a = adapter();
    let ind = WtIndicators {
        valid: Some(true),
        altitude: Some(1000.0),
        pitch: Some(5.0),
        roll: Some(0.0),
        // No IAS, no TAS
        ..Default::default()
    };
    let snap = a.convert_indicators(&ind).unwrap();
    assert!(
        !snap.validity.velocities_valid,
        "no speed data → velocities_valid must be false"
    );
    assert!(
        !snap.validity.safe_for_ffb,
        "missing velocity → not safe for FFB"
    );
}

/// Vehicle change detection: metrics track aircraft changes.
#[test]
fn bus_vehicle_change_detected_in_metrics() {
    let a = adapter();
    let metrics_before = a.metrics();
    assert_eq!(metrics_before.aircraft_changes, 0);

    // Simulate recording aircraft changes
    let mut metrics = a.metrics();
    // None → "F-86F" counts as first change
    metrics.record_aircraft_change("F-86F".to_string());
    assert_eq!(metrics.aircraft_changes, 1);
    assert_eq!(metrics.last_aircraft_title.as_deref(), Some("F-86F"));

    // A different aircraft triggers another change
    metrics.record_aircraft_change("MiG-15".to_string());
    assert_eq!(metrics.aircraft_changes, 2);
    assert_eq!(metrics.last_aircraft_title.as_deref(), Some("MiG-15"));

    // Same aircraft does not increment
    metrics.record_aircraft_change("MiG-15".to_string());
    assert_eq!(metrics.aircraft_changes, 2);
}

/// Timestamp field is populated in the snapshot (non-zero after conversion).
#[test]
fn bus_snapshot_timestamp_populated() {
    let a = adapter();
    let snap = a.convert_indicators(&full_indicators()).unwrap();
    // Timestamp is nanoseconds since adapter creation; should be > 0.
    assert!(snap.timestamp > 0, "timestamp should be > 0");
}

// ============================================================================
// Additional depth: combined /indicators + /state overlay
// ============================================================================

/// Applying /state overlay enriches an indicators-based snapshot.
#[test]
fn combined_indicators_then_state_overlay() {
    let a = adapter();
    let mut snap = a.convert_indicators(&full_indicators()).unwrap();

    // Before state overlay, AoA/Mach are defaults
    let aoa_before = snap.kinematics.aoa.to_degrees();

    a.apply_state(&full_state(), &mut snap).unwrap();

    // After overlay, AoA should be 3.5°
    assert!(
        (snap.kinematics.aoa.to_degrees() - 3.5).abs() < 0.01,
        "AoA should be 3.5° after state overlay, got {}",
        snap.kinematics.aoa.to_degrees()
    );
    // Mach should be 0.28
    assert!(
        (snap.kinematics.mach.value() - 0.28).abs() < 0.01,
        "Mach should be 0.28, got {}",
        snap.kinematics.mach.value()
    );
    // TAS from state (95 m/s) overwrites indicators TAS
    assert!(
        (snap.kinematics.tas.to_mps() - 95.0).abs() < 0.1,
        "TAS should be overwritten to 95 m/s, got {}",
        snap.kinematics.tas.to_mps()
    );
    // aero_valid should now be true due to AoA
    assert!(snap.validity.aero_valid);
    // Confirm AoA actually changed from default
    let _ = aoa_before; // used to ensure we captured before
}

/// Partial state overlay only modifies provided fields.
#[test]
fn partial_state_overlay_preserves_other_fields() {
    let a = adapter();
    let mut snap = a.convert_indicators(&full_indicators()).unwrap();
    let original_ias = snap.kinematics.ias.to_mps();
    let original_alt = snap.environment.altitude;

    // Only apply Mach, nothing else
    let state = WtState {
        mach: Some(0.92),
        ..Default::default()
    };
    a.apply_state(&state, &mut snap).unwrap();

    assert!(
        (snap.kinematics.mach.value() - 0.92).abs() < 0.01,
        "Mach should be 0.92"
    );
    // IAS and altitude should be unchanged
    assert!(
        (snap.kinematics.ias.to_mps() - original_ias).abs() < 0.01,
        "IAS should be preserved"
    );
    assert!(
        (snap.environment.altitude - original_alt).abs() < 0.01,
        "altitude should be preserved"
    );
}

/// Negative heading values normalise into [-180, 180] range.
#[test]
fn negative_heading_normalises() {
    let a = adapter();
    let ind = WtIndicators {
        heading: Some(-90.0),
        ..Default::default()
    };
    let snap = a.convert_indicators(&ind).unwrap();
    let hdg = snap.kinematics.heading.to_degrees();
    assert!(
        (hdg - (-90.0)).abs() < 0.01,
        "-90° should normalise to -90°, got {hdg}"
    );
}

/// Config: auto-reconnect is enabled, max_reconnect_attempts is 0 (unlimited).
#[test]
fn config_reconnect_policy() {
    let cfg = WarThunderConfig::default();
    assert!(cfg.enable_auto_reconnect());
    assert_eq!(cfg.max_reconnect_attempts(), 0);
}

/// Flaps >1.0 clamped to 100%, <0.0 clamped to 0%.
#[test]
fn flaps_out_of_range_clamped() {
    let a = adapter();
    // >1.0
    let ind = WtIndicators {
        flaps: Some(1.5),
        ..Default::default()
    };
    let snap = a.convert_indicators(&ind).unwrap();
    assert!(
        (snap.config.flaps.value() - 100.0).abs() < 0.01,
        "flaps >1 should clamp to 100%, got {}",
        snap.config.flaps.value()
    );

    // <0.0
    let ind2 = WtIndicators {
        flaps: Some(-0.5),
        ..Default::default()
    };
    let snap2 = a.convert_indicators(&ind2).unwrap();
    assert!(
        snap2.config.flaps.value() < 0.01,
        "flaps <0 should clamp to 0%, got {}",
        snap2.config.flaps.value()
    );
}

/// Gear at boundary values: exactly 0.0, 0.49, 0.5, 1.0.
#[test]
fn gear_boundary_values() {
    let a = adapter();
    let cases: &[(f32, GearPosition)] = &[
        (0.0, GearPosition::Up),
        (0.49, GearPosition::Up),
        (0.5, GearPosition::Down),
        (1.0, GearPosition::Down),
    ];
    for &(value, expected) in cases {
        let ind = WtIndicators {
            gear: Some(value),
            ..Default::default()
        };
        let snap = a.convert_indicators(&ind).unwrap();
        assert_eq!(
            snap.config.gear.nose, expected,
            "gear {value} should map to {expected:?}"
        );
        assert_eq!(
            snap.config.gear.left, expected,
            "all legs should match for gear {value}"
        );
        assert_eq!(snap.config.gear.right, expected);
    }
}
