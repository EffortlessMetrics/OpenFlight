// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Additional integration and property-based tests for the War Thunder adapter.
//!
//! Supplements `telemetry_property_tests.rs` with:
//! - `proptest`: arbitrary byte sequences never panic when parsed as `WtIndicators`
//! - `proptest`: TAS, pitch, roll, G-load, and flaps range coverage
//! - Aircraft-specific scenario tests (P-51D cruise, Bf 109 G-6 combat turn)
//! - Edge cases: `valid=false` ignored by `convert_indicators`, empty/missing
//!   airframe, sim field, heading 180° normalisation, extreme altitude, pitch ±90°

use flight_bus::types::SimId;
use flight_warthunder::{WarThunderAdapter, WarThunderConfig, protocol::WtIndicators};
use proptest::prelude::*;

fn adapter() -> WarThunderAdapter {
    WarThunderAdapter::new(WarThunderConfig::default())
}

// ============================================================================
// Property-based tests
// ============================================================================

proptest! {
    /// Any arbitrary byte sequence must never cause a panic when parsed as
    /// `WtIndicators` JSON.  Returning a parse error is correct; panicking
    /// is not.
    #[test]
    fn prop_arbitrary_bytes_never_panic(
        bytes in proptest::collection::vec(any::<u8>(), 0usize..1024)
    ) {
        let s = String::from_utf8_lossy(&bytes);
        let _ = serde_json::from_str::<WtIndicators>(&s);
    }

    /// Any printable ASCII string must never panic when parsed as `WtIndicators`.
    #[test]
    fn prop_printable_ascii_never_panics(s in "[ -~]{0,512}") {
        let _ = serde_json::from_str::<WtIndicators>(&s);
    }

    /// TAS in 0..=1799 km/h always maps to a non-negative m/s value.
    #[test]
    fn prop_tas_non_negative(tas_kmh in 0.0f32..=1799.0) {
        let ind = WtIndicators {
            valid: Some(true),
            tas_kmh: Some(tas_kmh),
            ..Default::default()
        };
        let snap = adapter().convert_indicators(&ind).unwrap();
        prop_assert!(
            snap.kinematics.tas.to_mps() >= 0.0,
            "TAS m/s negative for {} km/h",
            tas_kmh
        );
    }

    /// Valid pitch angles (-90..=90 degrees) always produce a valid snapshot
    /// with the pitch stored accurately.
    #[test]
    fn prop_valid_pitch_round_trips(pitch in -90.0f32..=90.0f32) {
        let ind = WtIndicators {
            valid: Some(true),
            pitch: Some(pitch),
            ..Default::default()
        };
        let result = adapter().convert_indicators(&ind);
        prop_assert!(result.is_ok(), "pitch {} should succeed", pitch);
        let snap = result.unwrap();
        prop_assert!(
            (snap.kinematics.pitch.to_degrees() - pitch).abs() < 1e-3,
            "pitch stored incorrectly: expected {}, got {}",
            pitch,
            snap.kinematics.pitch.to_degrees()
        );
    }

    /// Valid roll angles (-180..=180 degrees) always produce a valid snapshot.
    #[test]
    fn prop_valid_roll_always_succeeds(roll in -180.0f32..=180.0f32) {
        let ind = WtIndicators {
            valid: Some(true),
            roll: Some(roll),
            ..Default::default()
        };
        let result = adapter().convert_indicators(&ind);
        prop_assert!(result.is_ok(), "roll {} should succeed", roll);
    }

    /// G-load in the full flight envelope (-20..=20 g) never causes an error.
    #[test]
    fn prop_g_load_in_flight_envelope_succeeds(g in -20.0f32..=20.0) {
        let ind = WtIndicators {
            valid: Some(true),
            g_load: Some(g),
            ..Default::default()
        };
        let result = adapter().convert_indicators(&ind);
        prop_assert!(result.is_ok(), "g_load {} should succeed", g);
    }

    /// Flaps 0..=1 ratio always maps to Percentage 0..=100 and matches.
    #[test]
    fn prop_flaps_pct_matches_ratio(flaps in 0.0f32..=1.0) {
        let ind = WtIndicators {
            valid: Some(true),
            flaps: Some(flaps),
            ..Default::default()
        };
        let snap = adapter().convert_indicators(&ind).unwrap();
        let expected_pct = flaps * 100.0;
        prop_assert!(
            (snap.config.flaps.value() - expected_pct).abs() < 0.1,
            "flaps pct {} for ratio {}",
            snap.config.flaps.value(),
            flaps
        );
    }
}

// ============================================================================
// Aircraft-specific scenario tests
// ============================================================================

/// P-51D Mustang cruise: ~350 km/h at 3 000 m, level flight, gear up.
#[test]
fn test_p51d_cruise_telemetry() {
    let ind = WtIndicators {
        valid: Some(true),
        airframe: Some("P-51D-20NA".to_string()),
        ias_kmh: Some(350.0),
        tas_kmh: Some(370.0),
        altitude: Some(3_000.0),
        heading: Some(90.0),
        pitch: Some(1.5),
        roll: Some(0.0),
        g_load: Some(1.0),
        vert_speed: Some(0.0),
        gear: Some(0.0),
        flaps: Some(0.0),
    };

    let snap = adapter()
        .convert_indicators(&ind)
        .expect("P-51D cruise must succeed");

    assert_eq!(snap.aircraft.icao, "P-51D-20NA");
    // 350 km/h → ~97.22 m/s
    let ias_mps = snap.kinematics.ias.to_mps();
    assert!((ias_mps - 97.22).abs() < 0.5, "IAS {ias_mps} m/s");
    assert!(snap.environment.altitude > 0.0, "altitude must be positive");
    assert!(snap.validity.attitude_valid, "attitude must be valid");
    assert!(snap.validity.safe_for_ffb, "must be safe for FFB");
}

/// Bf 109 G-6 combat turn: banked 60°, 500 km/h, 4 500 m, G ~4.5.
#[test]
fn test_bf109g_combat_turn_telemetry() {
    let ind = WtIndicators {
        valid: Some(true),
        airframe: Some("Bf 109 G-6".to_string()),
        ias_kmh: Some(500.0),
        tas_kmh: Some(520.0),
        altitude: Some(4_500.0),
        heading: Some(270.0),
        pitch: Some(20.0),
        roll: Some(60.0),
        g_load: Some(4.5),
        vert_speed: Some(5.0),
        gear: Some(0.0),
        flaps: Some(0.0),
    };

    let snap = adapter()
        .convert_indicators(&ind)
        .expect("Bf 109 G-6 combat turn must succeed");

    assert_eq!(snap.aircraft.icao, "Bf 109 G-6");
    assert!(
        (snap.kinematics.bank.to_degrees() - 60.0_f32).abs() < 1e-3,
        "roll/bank"
    );
    assert!(snap.kinematics.vertical_speed > 0.0, "climbing in turn");
    assert!(snap.validity.safe_for_ffb);
}

// ============================================================================
// Additional edge-case and unit-conversion tests
// ============================================================================

/// TAS unit conversion: 360 km/h → ~100 m/s.
#[test]
fn tas_unit_conversion_360kmh() {
    let ind = WtIndicators {
        valid: Some(true),
        tas_kmh: Some(360.0),
        ..Default::default()
    };
    let snap = adapter().convert_indicators(&ind).unwrap();
    let tas_mps = snap.kinematics.tas.to_mps();
    // 360 km/h × 0.277778 ≈ 100.0 m/s
    assert!(
        (tas_mps - 100.0).abs() < 0.1,
        "360 km/h should be ~100 m/s, got {tas_mps}"
    );
}

/// Heading 0 degrees stays at 0 after normalisation.
#[test]
fn heading_zero_stays_zero() {
    let ind = WtIndicators {
        valid: Some(true),
        heading: Some(0.0),
        ..Default::default()
    };
    let snap = adapter().convert_indicators(&ind).unwrap();
    assert!(
        snap.kinematics.heading.to_degrees().abs() < 0.01,
        "heading 0° should stay 0°"
    );
}

/// Heading 180° normalises to −180° (the signed-normalisation formula maps
/// 180 → −180, both being equivalent at the anti-meridian).
#[test]
fn heading_180_normalises_to_negative_180() {
    let ind = WtIndicators {
        valid: Some(true),
        heading: Some(180.0),
        ..Default::default()
    };
    let snap = adapter().convert_indicators(&ind).unwrap();
    let hdg = snap.kinematics.heading.to_degrees();
    // normalize_degrees_signed(180) = ((180 % 360) + 540) % 360 - 180
    //   = (180 + 540) % 360 - 180 = 0 - 180 = -180
    assert!(
        (hdg - (-180.0_f32)).abs() < 0.01,
        "heading 180° should normalise to -180°, got {}",
        hdg
    );
}

/// Extreme altitude (40 000 m, stratosphere) must not error.
#[test]
fn extreme_altitude_40000m_handled() {
    let ind = WtIndicators {
        valid: Some(true),
        altitude: Some(40_000.0),
        ..Default::default()
    };
    let snap = adapter().convert_indicators(&ind).unwrap();
    // 40 000 m ≈ 131 233 ft
    assert!(
        snap.environment.altitude > 130_000.0,
        "40 000 m should be >130 000 ft, got {}",
        snap.environment.altitude
    );
}

/// Pitch −90° (straight down) must succeed and be stored correctly.
#[test]
fn pitch_negative_90_degrees_accepted() {
    let ind = WtIndicators {
        valid: Some(true),
        pitch: Some(-90.0),
        ..Default::default()
    };
    let result = adapter().convert_indicators(&ind);
    assert!(result.is_ok(), "pitch -90° should be accepted");
    let snap = result.unwrap();
    assert!(
        (snap.kinematics.pitch.to_degrees() - (-90.0_f32)).abs() < 0.01,
        "pitch should be -90°"
    );
}

/// `convert_indicators` must succeed even when `valid` is explicitly `false`
/// (validity gating is the responsibility of `poll_once`, not
/// `convert_indicators`).
#[test]
fn convert_indicators_ignores_valid_false_flag() {
    let ind = WtIndicators {
        valid: Some(false),
        ias_kmh: Some(200.0),
        altitude: Some(1_000.0),
        pitch: Some(5.0),
        roll: Some(0.0),
        ..Default::default()
    };
    // Must not return an error — convert_indicators does not inspect `valid`
    let result = adapter().convert_indicators(&ind);
    assert!(
        result.is_ok(),
        "convert_indicators must not gate on the valid flag"
    );
}

/// The `sim` field must always be `SimId::WarThunder` regardless of aircraft.
#[test]
fn sim_id_is_always_warthunder_for_all_aircraft() {
    let names = [
        "P-51D-20NA",
        "Spitfire Mk.Vc",
        "F-86F Sabre",
        "Su-27",
        "F-14A",
    ];
    for name in &names {
        let ind = WtIndicators {
            airframe: Some(name.to_string()),
            ..Default::default()
        };
        let snap = adapter().convert_indicators(&ind).unwrap();
        assert_eq!(
            snap.sim,
            SimId::WarThunder,
            "sim must be WarThunder for aircraft {name}"
        );
    }
}

/// An empty airframe string produces a snapshot with an empty ICAO.
#[test]
fn empty_airframe_name_gives_empty_icao() {
    let ind = WtIndicators {
        airframe: Some(String::new()),
        ..Default::default()
    };
    let snap = adapter().convert_indicators(&ind).unwrap();
    assert_eq!(snap.aircraft.icao, "");
}

/// A missing airframe (`None`) produces a snapshot with an empty ICAO.
#[test]
fn missing_airframe_gives_empty_icao() {
    let ind = WtIndicators {
        valid: Some(true),
        ..Default::default()
    };
    let snap = adapter().convert_indicators(&ind).unwrap();
    assert_eq!(snap.aircraft.icao, "");
}

/// Vertical speed of −10 m/s (steep descent) produces a large negative ft/min.
#[test]
fn steep_descent_produces_large_negative_fpm() {
    let ind = WtIndicators {
        vert_speed: Some(-10.0),
        ..Default::default()
    };
    let snap = adapter().convert_indicators(&ind).unwrap();
    // −10 m/s × 196.85 ≈ −1968.5 ft/min
    assert!(
        snap.kinematics.vertical_speed < -1900.0,
        "−10 m/s should give <−1900 ft/min, got {}",
        snap.kinematics.vertical_speed
    );
}
