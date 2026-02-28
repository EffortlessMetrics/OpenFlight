// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Property-based and unit tests for the War Thunder telemetry adapter.

use flight_adapter_common::AdapterState;
use flight_bus::types::GearPosition;
use flight_warthunder::{WarThunderAdapter, WarThunderConfig, protocol::WtIndicators};
use proptest::prelude::*;

fn adapter() -> WarThunderAdapter {
    WarThunderAdapter::new(WarThunderConfig::default())
}

proptest! {
    /// IAS from any non-negative km/h value stays non-negative in m/s.
    /// Capped at 1800 km/h = 500 m/s (ValidatedSpeed::new_mps upper bound).
    #[test]
    fn prop_ias_non_negative(ias_kmh in 0.0f32..=1800.0f32) {
        let ind = WtIndicators {
            valid: Some(true),
            ias_kmh: Some(ias_kmh),
            ..Default::default()
        };
        let snap = adapter().convert_indicators(&ind).unwrap();
        prop_assert!(
            snap.kinematics.ias.to_mps() >= 0.0,
            "IAS m/s negative for {}km/h", ias_kmh
        );
    }

    /// Altitude (metres) always maps to a non-negative feet value.
    #[test]
    fn prop_altitude_non_negative(alt_m in 0.0f32..50_000.0) {
        let ind = WtIndicators {
            valid: Some(true),
            altitude: Some(alt_m),
            ..Default::default()
        };
        let snap = adapter().convert_indicators(&ind).unwrap();
        prop_assert!(
            snap.environment.altitude >= 0.0,
            "altitude_ft negative for {}m", alt_m
        );
    }

    /// Heading conversion never panics for any real-valued input.
    #[test]
    fn prop_heading_normalises(hdg in -720.0f32..720.0) {
        let ind = WtIndicators {
            valid: Some(true),
            heading: Some(hdg),
            ..Default::default()
        };
        let result = adapter().convert_indicators(&ind);
        prop_assert!(result.is_ok(), "convert failed for heading={}", hdg);
    }

    /// Gear value ≥ 0.5 always produces GearPosition::Down for all three legs.
    #[test]
    fn prop_gear_down_when_value_high(gear in 0.5f32..=1.0) {
        let ind = WtIndicators {
            valid: Some(true),
            gear: Some(gear),
            ..Default::default()
        };
        let snap = adapter().convert_indicators(&ind).unwrap();
        prop_assert_eq!(snap.config.gear.nose, GearPosition::Down);
        prop_assert_eq!(snap.config.gear.left, GearPosition::Down);
        prop_assert_eq!(snap.config.gear.right, GearPosition::Down);
    }

    /// Gear value < 0.5 always produces GearPosition::Up.
    #[test]
    fn prop_gear_up_when_value_low(gear in 0.0f32..0.5) {
        let ind = WtIndicators {
            valid: Some(true),
            gear: Some(gear),
            ..Default::default()
        };
        let snap = adapter().convert_indicators(&ind).unwrap();
        prop_assert_eq!(snap.config.gear.nose, GearPosition::Up);
        prop_assert_eq!(snap.config.gear.left, GearPosition::Up);
        prop_assert_eq!(snap.config.gear.right, GearPosition::Up);
    }

    /// Flaps 0..=1 always maps to Percentage 0..=100 without error.
    #[test]
    fn prop_flaps_percentage(flaps in 0.0f32..=1.0) {
        let ind = WtIndicators {
            valid: Some(true),
            flaps: Some(flaps),
            ..Default::default()
        };
        let snap = adapter().convert_indicators(&ind).unwrap();
        let pct = snap.config.flaps.value();
        prop_assert!(
            (0.0..=100.0).contains(&pct),
            "flaps pct out of range: {} for input {}", pct, flaps
        );
    }

    /// safe_for_ffb is true iff pitch, roll, ias, AND altitude are all present.
    #[test]
    fn prop_safe_for_ffb_requires_all_fields(
        has_pitch in any::<bool>(),
        has_roll in any::<bool>(),
        has_ias in any::<bool>(),
        has_altitude in any::<bool>(),
    ) {
        let ind = WtIndicators {
            valid: Some(true),
            pitch: if has_pitch { Some(5.0) } else { None },
            roll: if has_roll { Some(-10.0) } else { None },
            ias_kmh: if has_ias { Some(300.0) } else { None },
            altitude: if has_altitude { Some(1000.0) } else { None },
            ..Default::default()
        };
        let snap = adapter().convert_indicators(&ind).unwrap();
        let expected = has_pitch && has_roll && has_ias && has_altitude;
        prop_assert_eq!(
            snap.validity.safe_for_ffb,
            expected,
            "safe_for_ffb mismatch: pitch={} roll={} ias={} alt={}",
            has_pitch, has_roll, has_ias, has_altitude
        );
    }
}

#[test]
fn empty_indicators_gives_empty_snapshot() {
    let ind = WtIndicators {
        valid: Some(true),
        ..Default::default()
    };
    let snap = adapter().convert_indicators(&ind).unwrap();
    assert!(!snap.validity.safe_for_ffb);
    assert!(!snap.validity.attitude_valid);
    assert!(!snap.validity.velocities_valid);
    assert!(!snap.validity.position_valid);
}

#[test]
fn ias_unit_conversion_400kmh() {
    // 400 km/h → 111.111... m/s
    let ind = WtIndicators {
        valid: Some(true),
        ias_kmh: Some(400.0),
        ..Default::default()
    };
    let snap = adapter().convert_indicators(&ind).unwrap();
    let ias_mps = snap.kinematics.ias.to_mps();
    assert!((ias_mps - 111.111).abs() < 0.01, "got {ias_mps}");
}

#[test]
fn altitude_unit_conversion_1000m() {
    // 1000 m → 3280.84 ft
    let ind = WtIndicators {
        valid: Some(true),
        altitude: Some(1000.0),
        ..Default::default()
    };
    let snap = adapter().convert_indicators(&ind).unwrap();
    let ft = snap.environment.altitude;
    assert!((ft - 3280.84).abs() < 1.0, "got {ft}");
}

#[test]
fn adapter_initial_state_disconnected() {
    let a = adapter();
    assert_eq!(a.state(), AdapterState::Disconnected);
    // Never received a packet → unwrap_or(true) returns true
    assert!(a.is_connection_timeout());
}
