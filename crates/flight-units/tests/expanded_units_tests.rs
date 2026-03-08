// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Expanded tests for flight-units: edge cases, boundary conditions,
//! known-value verifications, and additional property-based tests.

use flight_units::angles;
use flight_units::conversions;
use flight_units::{Angle, AngleUnit, Force, ForceUnit, Speed, SpeedUnit, UnitValue};
use proptest::prelude::*;

// ── UnitValue struct tests ──────────────────────────────────────────────────

#[test]
fn unit_value_speed_clone_eq() {
    let s = Speed {
        value: 120.0,
        unit: SpeedUnit::Knots,
    };
    let cloned = s;
    assert_eq!(s, cloned);
}

#[test]
fn unit_value_angle_variants() {
    let deg = Angle {
        value: 90.0,
        unit: AngleUnit::Degrees,
    };
    let rad = Angle {
        value: std::f32::consts::FRAC_PI_2,
        unit: AngleUnit::Radians,
    };
    assert_ne!(deg, rad);
}

#[test]
fn unit_value_force_variants() {
    let f1 = Force {
        value: 10.0,
        unit: ForceUnit::Newtons,
    };
    let f2 = Force {
        value: 10.0,
        unit: ForceUnit::NewtonMeters,
    };
    assert_ne!(f1, f2);
}

#[test]
fn unit_value_serde_roundtrip() {
    let speed = Speed {
        value: 250.5,
        unit: SpeedUnit::Kph,
    };
    let json = serde_json::to_string(&speed).unwrap();
    let restored: Speed = serde_json::from_str(&json).unwrap();
    assert_eq!(speed, restored);
}

#[test]
fn unit_value_debug_format() {
    let v = UnitValue {
        value: 42.0f32,
        unit: SpeedUnit::Mps,
    };
    let dbg = format!("{:?}", v);
    assert!(dbg.contains("42.0"));
    assert!(dbg.contains("Mps"));
}

// ── Angle normalization: edge cases ─────────────────────────────────────────

#[test]
fn normalize_signed_zero() {
    assert!((angles::normalize_degrees_signed(0.0)).abs() < 0.001);
}

#[test]
fn normalize_signed_180() {
    let n = angles::normalize_degrees_signed(180.0);
    // 180 could normalize to 180 or -180, both are valid
    assert!((n.abs() - 180.0).abs() < 0.001);
}

#[test]
fn normalize_signed_minus_180() {
    let n = angles::normalize_degrees_signed(-180.0);
    assert!((n.abs() - 180.0).abs() < 0.001);
}

#[test]
fn normalize_signed_720() {
    assert!((angles::normalize_degrees_signed(720.0)).abs() < 0.001);
}

#[test]
fn normalize_signed_negative_720() {
    assert!((angles::normalize_degrees_signed(-720.0)).abs() < 0.001);
}

#[test]
fn normalize_unsigned_zero() {
    assert!((angles::normalize_degrees_unsigned(0.0)).abs() < 0.001);
}

#[test]
fn normalize_unsigned_359() {
    let n = angles::normalize_degrees_unsigned(359.0);
    assert!((n - 359.0).abs() < 0.001);
}

#[test]
fn normalize_unsigned_negative_1() {
    let n = angles::normalize_degrees_unsigned(-1.0);
    assert!((n - 359.0).abs() < 0.001);
}

#[test]
fn normalize_unsigned_720() {
    assert!((angles::normalize_degrees_unsigned(720.0)).abs() < 0.001);
}

// ── Conversion: known values ────────────────────────────────────────────────

#[test]
fn knots_to_mps_known_value() {
    // 1 knot = 0.514444 m/s
    let mps = conversions::knots_to_mps(1.0);
    assert!((mps - 0.514444).abs() < 0.001);
}

#[test]
fn mps_to_knots_known_value() {
    let knots = conversions::mps_to_knots(0.514444);
    assert!((knots - 1.0).abs() < 0.001);
}

#[test]
fn feet_to_meters_known_value() {
    // 1 foot = 0.3048 m
    let m = conversions::feet_to_meters(1.0);
    assert!((m - 0.3048).abs() < 0.0001);
}

#[test]
fn meters_to_feet_known_value() {
    let ft = conversions::meters_to_feet(0.3048);
    assert!((ft - 1.0).abs() < 0.001);
}

#[test]
fn feet_to_meters_fl350() {
    // FL350 = 35,000 ft ≈ 10,668 m
    let m = conversions::feet_to_meters(35000.0);
    assert!((m - 10668.0).abs() < 1.0);
}

#[test]
fn kph_to_mps_known_value() {
    // 3.6 kph = 1 m/s
    let mps = conversions::kph_to_mps(3.6);
    assert!((mps - 1.0).abs() < 0.01);
}

#[test]
fn mps_to_kph_known_value() {
    let kph = conversions::mps_to_kph(1.0);
    assert!((kph - 3.6).abs() < 0.01);
}

#[test]
fn degrees_to_radians_90() {
    let rad = conversions::degrees_to_radians(90.0);
    assert!((rad - std::f32::consts::FRAC_PI_2).abs() < 0.0001);
}

#[test]
fn radians_to_degrees_pi_over_4() {
    let deg = conversions::radians_to_degrees(std::f32::consts::FRAC_PI_4);
    assert!((deg - 45.0).abs() < 0.001);
}

// ── Conversion: zero values ─────────────────────────────────────────────────

#[test]
fn conversions_zero_values() {
    assert_eq!(conversions::knots_to_mps(0.0), 0.0);
    assert_eq!(conversions::mps_to_knots(0.0), 0.0);
    assert_eq!(conversions::kph_to_mps(0.0), 0.0);
    assert_eq!(conversions::mps_to_kph(0.0), 0.0);
    assert_eq!(conversions::feet_to_meters(0.0), 0.0);
    assert_eq!(conversions::meters_to_feet(0.0), 0.0);
    assert_eq!(conversions::fpm_to_mps(0.0), 0.0);
    assert_eq!(conversions::mps_to_fpm(0.0), 0.0);
    assert_eq!(conversions::degrees_to_radians(0.0), 0.0);
    assert_eq!(conversions::radians_to_degrees(0.0), 0.0);
    assert_eq!(conversions::knots_to_kph(0.0), 0.0);
    assert_eq!(conversions::kph_to_knots(0.0), 0.0);
}

// ── Conversion: negative values ─────────────────────────────────────────────

#[test]
fn fpm_to_mps_negative() {
    // Descending: -500 fpm
    let mps = conversions::fpm_to_mps(-500.0);
    assert!(mps < 0.0);
    assert!((mps - (-2.54)).abs() < 0.01);
}

#[test]
fn feet_to_meters_negative() {
    let m = conversions::feet_to_meters(-100.0);
    assert!((m - (-30.48)).abs() < 0.01);
}

// ── Property-based: additional properties ───────────────────────────────────

proptest! {
    /// Zero converts to zero for all speed conversions
    #[test]
    fn prop_speed_conversion_preserves_sign(val in -1000.0f32..1000.0) {
        let mps = conversions::knots_to_mps(val);
        if val > 0.0 {
            prop_assert!(mps > 0.0);
        } else if val < 0.0 {
            prop_assert!(mps < 0.0);
        } else {
            prop_assert!((mps).abs() < f32::EPSILON);
        }
    }

    /// feet_to_meters is monotonically increasing
    #[test]
    fn prop_feet_to_meters_monotonic(a in 0.0f32..50000.0, b in 0.0f32..50000.0) {
        let ma = conversions::feet_to_meters(a);
        let mb = conversions::feet_to_meters(b);
        if a < b {
            prop_assert!(ma < mb, "f({}) = {} >= f({}) = {}", a, ma, b, mb);
        } else if a > b {
            prop_assert!(ma > mb);
        }
    }

    /// knots_to_mps is monotonically increasing
    #[test]
    fn prop_knots_to_mps_monotonic(a in 0.0f32..10000.0, b in 0.0f32..10000.0) {
        let ma = conversions::knots_to_mps(a);
        let mb = conversions::knots_to_mps(b);
        if a < b {
            prop_assert!(ma < mb);
        } else if a > b {
            prop_assert!(ma > mb);
        }
    }

    /// normalize_degrees_signed is idempotent
    #[test]
    fn prop_normalize_signed_idempotent(val in -10000.0f32..10000.0) {
        let once = angles::normalize_degrees_signed(val);
        let twice = angles::normalize_degrees_signed(once);
        prop_assert!(
            (once - twice).abs() < 0.001,
            "normalize_signed not idempotent: {} -> {} -> {}",
            val, once, twice
        );
    }

    /// normalize_degrees_unsigned is idempotent
    #[test]
    fn prop_normalize_unsigned_idempotent(val in -10000.0f32..10000.0) {
        let once = angles::normalize_degrees_unsigned(val);
        let twice = angles::normalize_degrees_unsigned(once);
        prop_assert!(
            (once - twice).abs() < 0.001,
            "normalize_unsigned not idempotent: {} -> {} -> {}",
            val, once, twice
        );
    }

    /// fpm_to_mps → mps_to_fpm roundtrip preserves sign
    #[test]
    fn prop_fpm_roundtrip_preserves_sign(val in -10000.0f32..10000.0) {
        let mps = conversions::fpm_to_mps(val);
        let back = conversions::mps_to_fpm(mps);
        if val > 0.0 {
            prop_assert!(back > 0.0);
        } else if val < 0.0 {
            prop_assert!(back < 0.0);
        }
    }

    /// UnitValue<SpeedUnit> serde roundtrip
    #[test]
    fn prop_speed_serde_roundtrip(val in -10000.0f32..10000.0, unit_idx in 0u8..3) {
        let unit = match unit_idx {
            0 => SpeedUnit::Knots,
            1 => SpeedUnit::Mps,
            _ => SpeedUnit::Kph,
        };
        let speed = Speed { value: val, unit };
        let json = serde_json::to_string(&speed).unwrap();
        let restored: Speed = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(speed, restored);
    }
}
