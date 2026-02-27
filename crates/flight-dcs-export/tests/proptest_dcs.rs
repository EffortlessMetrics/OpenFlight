// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Property-based and additional integration tests for the DCS Export adapter.
//!
//! Supplements `adapter_tests.rs` and `integration_dcs_bus.rs` with:
//! - `proptest`: arbitrary byte sequences never panic when parsed as `DcsMessage`
//! - `proptest`: valid telemetry field ranges always produce correct snapshots
//! - F/A-18C specific scenarios (carrier approach, combat climb)
//! - Edge cases: zero IAS, negative altitude, long/empty aircraft names,
//!   IAS boundary values

use flight_dcs_export::{DcsAdapter, DcsAdapterConfig, DcsMessage};
use proptest::prelude::*;
use serde_json::json;
use std::collections::HashMap;

fn adapter() -> DcsAdapter {
    DcsAdapter::new(DcsAdapterConfig::default())
}

fn data_from(v: serde_json::Value) -> HashMap<String, serde_json::Value> {
    v.as_object().unwrap().clone().into_iter().collect()
}

// ============================================================================
// Property-based tests
// ============================================================================

proptest! {
    /// Any arbitrary byte sequence must never cause a panic when fed to the
    /// `DcsMessage` JSON parser.  Returning a parse error is correct; panicking
    /// is not.
    #[test]
    fn prop_arbitrary_bytes_never_panic(
        bytes in proptest::collection::vec(any::<u8>(), 0usize..1024)
    ) {
        let s = String::from_utf8_lossy(&bytes);
        let _ = serde_json::from_str::<DcsMessage>(&s);
    }

    /// Any printable ASCII string must not panic when parsed as a `DcsMessage`.
    #[test]
    fn prop_printable_ascii_string_never_panics(s in "[ -~]{0,512}") {
        let _ = serde_json::from_str::<DcsMessage>(&s);
    }

    /// Valid pitch angles (-180..=180 degrees) always produce a valid snapshot
    /// and are stored accurately.
    #[test]
    fn prop_valid_pitch_round_trips(pitch in -180.0f32..=180.0f32) {
        let data = data_from(json!({"ias": 200.0, "pitch": pitch}));
        let result = adapter().convert_to_bus_snapshot(0, "F-16C", &data);
        prop_assert!(result.is_ok(), "pitch {} should succeed", pitch);
        let snap = result.unwrap();
        prop_assert!(
            (snap.kinematics.pitch.value() - pitch).abs() < 1e-3,
            "pitch stored incorrectly: expected {}, got {}",
            pitch,
            snap.kinematics.pitch.value()
        );
    }

    /// Valid heading values (-180..=180 degrees) always produce a valid snapshot
    /// with the heading stored accurately.
    #[test]
    fn prop_valid_heading_round_trips(heading in -180.0f32..=180.0f32) {
        let data = data_from(json!({"ias": 200.0, "heading": heading}));
        let result = adapter().convert_to_bus_snapshot(0, "F-16C", &data);
        prop_assert!(result.is_ok(), "heading {} should succeed", heading);
        let snap = result.unwrap();
        prop_assert!(
            (snap.kinematics.heading.value() - heading).abs() < 1e-3,
            "heading stored incorrectly"
        );
    }

    /// Valid IAS in 0..=999 kts always round-trips correctly through the snapshot.
    #[test]
    fn prop_valid_ias_round_trips(ias in 0.0f32..=999.0f32) {
        let data = data_from(json!({"ias": ias}));
        let result = adapter().convert_to_bus_snapshot(0, "F-16C", &data);
        prop_assert!(result.is_ok(), "ias {} should succeed", ias);
        let snap = result.unwrap();
        prop_assert!(
            (snap.kinematics.ias.value() - ias).abs() < 0.1,
            "IAS stored incorrectly: expected {}, got {}",
            ias,
            snap.kinematics.ias.value()
        );
    }

    /// Valid bank (roll) angles in -180..=180 degrees must be stored correctly.
    #[test]
    fn prop_valid_bank_round_trips(bank in -180.0f32..=180.0f32) {
        let data = data_from(json!({"ias": 200.0, "bank": bank}));
        let result = adapter().convert_to_bus_snapshot(0, "F-16C", &data);
        prop_assert!(result.is_ok(), "bank {} should succeed", bank);
        let snap = result.unwrap();
        prop_assert!(
            (snap.kinematics.bank.value() - bank).abs() < 1e-3,
            "bank stored incorrectly: expected {}, got {}",
            bank,
            snap.kinematics.bank.value()
        );
    }
}

// ============================================================================
// F/A-18C specific integration tests
// ============================================================================

/// Parse an F/A-18C carrier-approach sample.
///
/// Typical on-speed approach: pitch ~8°, IAS 140 kts, AoA ~8°, gear down,
/// full flaps, descending at ~700 ft/min.
#[test]
fn test_fa18c_carrier_approach_telemetry() {
    let data = data_from(json!({
        "pitch":          8.1,
        "bank":           0.5,
        "heading":        12.0,
        "ias":            140.0,
        "tas":            143.0,
        "altitude_asl":   600.0,
        "aoa":            8.1,
        "vertical_speed": -700.0,
        "g_force":        1.0,
        "gear_down":      1.0,
        "flaps":          100.0,
    }));

    let snap = adapter()
        .convert_to_bus_snapshot(0, "F/A-18C", &data)
        .expect("F/A-18C carrier approach must parse");

    assert_eq!(snap.aircraft.icao, "F/A-18C");
    assert!(
        (snap.kinematics.pitch.value() - 8.1_f32).abs() < 1e-3,
        "pitch"
    );
    assert!(
        (snap.kinematics.ias.value() - 140.0_f32).abs() < 1e-3,
        "IAS"
    );
    assert!((snap.kinematics.aoa.value() - 8.1_f32).abs() < 1e-3, "AoA");
    assert!(
        snap.kinematics.vertical_speed < 0.0,
        "descending on approach"
    );
    assert!(snap.config.gear.all_down(), "gear down on approach");
    assert_eq!(snap.config.flaps.value(), 100.0, "full flaps");
}

/// Parse an F/A-18C combat-climb sample.
///
/// Steep climb: pitch 35°, IAS 400 kts, G ~3.5, gear retracted, flaps
/// retracted.
#[test]
fn test_fa18c_combat_climb_telemetry() {
    let data = data_from(json!({
        "pitch":          35.0,
        "bank":           0.0,
        "heading":        -90.0,   // 270° expressed in DCS signed range (-180..=180)
        "ias":            400.0,
        "tas":            420.0,
        "altitude_asl":   20_000.0,
        "aoa":            4.0,
        "vertical_speed": 15_000.0,
        "g_force":        3.5,
        "gear_down":      0.0,
        "flaps":          0.0,
    }));

    let snap = adapter()
        .convert_to_bus_snapshot(0, "F/A-18C", &data)
        .expect("F/A-18C combat climb must parse");

    assert!(
        (snap.kinematics.pitch.value() - 35.0_f32).abs() < 1e-3,
        "pitch"
    );
    assert!(
        (snap.kinematics.ias.value() - 400.0_f32).abs() < 1e-3,
        "IAS"
    );
    assert!(snap.kinematics.vertical_speed > 0.0, "climbing");
    assert!(snap.config.gear.all_up(), "gear retracted");
    assert!(snap.config.flaps.value() < 1.0, "flaps retracted");
}

// ============================================================================
// Edge-case integration tests
// ============================================================================

/// Zero IAS (aircraft stationary on the ground) must be accepted.
#[test]
fn test_zero_ias_ground_state_is_valid() {
    let data = data_from(json!({"ias": 0.0, "heading": 180.0, "gear_down": 1.0}));

    let snap = adapter()
        .convert_to_bus_snapshot(0, "F-16C", &data)
        .expect("zero IAS on ground must be valid");

    assert_eq!(snap.kinematics.ias.value(), 0.0);
    assert!(snap.config.gear.all_down());
}

/// Negative altitude (below sea level, e.g. terrain in the Caucasus map)
/// must be stored as-is without producing an error.
#[test]
fn test_negative_altitude_stored_correctly() {
    let data = data_from(json!({"ias": 80.0, "altitude_asl": -100.0}));

    let snap = adapter()
        .convert_to_bus_snapshot(0, "F-16C", &data)
        .expect("negative altitude must not cause an error");

    assert!(
        (snap.environment.altitude - (-100.0_f32)).abs() < 1.0,
        "altitude should be -100, got {}",
        snap.environment.altitude
    );
}

/// A whitespace-only aircraft name must not panic and must be stored verbatim.
#[test]
fn test_whitespace_only_aircraft_name_does_not_panic() {
    let data = data_from(json!({"ias": 100.0}));
    let result = adapter().convert_to_bus_snapshot(0, "   ", &data);
    assert!(result.is_ok(), "whitespace aircraft name must not panic");
    assert_eq!(result.unwrap().aircraft.icao, "   ");
}

/// An empty aircraft name must not panic.
#[test]
fn test_empty_aircraft_name_does_not_panic() {
    let data = data_from(json!({"ias": 100.0}));
    let result = adapter().convert_to_bus_snapshot(0, "", &data);
    assert!(result.is_ok(), "empty aircraft name must not panic");
}

/// A very long aircraft name (10 000 chars) must not panic or cause allocation
/// issues.
#[test]
fn test_very_long_aircraft_name_does_not_panic() {
    let long_name = "X".repeat(10_000);
    let data = data_from(json!({"ias": 100.0}));
    assert!(
        adapter()
            .convert_to_bus_snapshot(0, &long_name, &data)
            .is_ok(),
        "long aircraft name must not panic"
    );
}

/// The `sim` field of every snapshot must always be `SimId::Dcs` regardless
/// of aircraft name.
#[test]
fn test_sim_id_is_always_dcs() {
    for name in &["F-16C", "F/A-18C", "Ka-50", "A-10C", "Su-27", "MiG-29"] {
        let data = data_from(json!({"ias": 100.0}));
        let snap = adapter()
            .convert_to_bus_snapshot(0, name, &data)
            .unwrap_or_else(|_| panic!("should produce a snapshot for aircraft {name}"));
        assert_eq!(
            format!("{:?}", snap.sim),
            "Dcs",
            "sim must be Dcs for aircraft {name}"
        );
    }
}

/// IAS at the inclusive maximum (1000 kts) must be accepted.
#[test]
fn test_boundary_ias_1000_kts_is_accepted() {
    let data = data_from(json!({"ias": 1000.0}));
    let snap = adapter()
        .convert_to_bus_snapshot(0, "F-16C", &data)
        .expect("IAS 1000 kts (inclusive boundary) must be accepted");
    assert!((snap.kinematics.ias.value() - 1000.0_f32).abs() < 0.5);
}

/// IAS just above the maximum (1000.1 kts) must be rejected.
#[test]
fn test_ias_above_1000_kts_is_rejected() {
    let data = data_from(json!({"ias": 1000.1}));
    assert!(
        adapter()
            .convert_to_bus_snapshot(0, "F-16C", &data)
            .is_err(),
        "IAS above 1000 kts must be rejected"
    );
}

/// High stratospheric altitude (~FL590, 18 000 m) must be stored correctly.
#[test]
fn test_high_altitude_fl590_stored_correctly() {
    let data = data_from(json!({"ias": 450.0, "altitude_asl": 18_000.0}));
    let snap = adapter()
        .convert_to_bus_snapshot(0, "F-16C", &data)
        .expect("high altitude must succeed");
    assert!(
        (snap.environment.altitude - 18_000.0_f32).abs() < 1.0,
        "altitude should be 18 000 m, got {}",
        snap.environment.altitude
    );
}

/// Extreme heading values at the -180 and +180 boundaries must be accepted.
#[test]
fn test_heading_at_negative_180_and_positive_180_boundaries() {
    let adapter = adapter();
    for &hdg in &[-180.0_f32, 0.0, 90.0, 180.0] {
        let data = data_from(json!({"ias": 200.0, "heading": hdg}));
        let result = adapter.convert_to_bus_snapshot(0, "F-16C", &data);
        assert!(
            result.is_ok(),
            "heading {hdg}° must be accepted, got {:?}",
            result.err()
        );
    }
}
