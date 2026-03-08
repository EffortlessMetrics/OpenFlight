// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for flight-units: known-value conversions, boundary conditions,
//! round-trip identity, angle normalization, NaN/Inf handling, monotonicity,
//! and property-based tests via proptest.

use flight_units::conversions;
use flight_units::{angles, Angle, AngleUnit, Force, ForceUnit, Speed, SpeedUnit, UnitValue};
use proptest::prelude::*;

// ── Helpers ──────────────────────────────────────────────────────────────────

const EPS: f32 = 1e-3;
const TIGHT_EPS: f32 = 1e-4;

fn approx(a: f32, b: f32, eps: f32) -> bool {
    (a - b).abs() <= eps
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Section 1 – UnitValue / type-alias smoke tests
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn unit_value_speed_knots_construction() {
    let s: Speed = UnitValue {
        value: 250.0,
        unit: SpeedUnit::Knots,
    };
    assert_eq!(s.value, 250.0);
    assert_eq!(s.unit, SpeedUnit::Knots);
}

#[test]
fn unit_value_speed_mps_construction() {
    let s: Speed = UnitValue {
        value: 100.0,
        unit: SpeedUnit::Mps,
    };
    assert_eq!(s.value, 100.0);
    assert_eq!(s.unit, SpeedUnit::Mps);
}

#[test]
fn unit_value_speed_kph_construction() {
    let s: Speed = UnitValue {
        value: 900.0,
        unit: SpeedUnit::Kph,
    };
    assert_eq!(s.value, 900.0);
    assert_eq!(s.unit, SpeedUnit::Kph);
}

#[test]
fn unit_value_angle_degrees_construction() {
    let a: Angle = UnitValue {
        value: 45.0,
        unit: AngleUnit::Degrees,
    };
    assert_eq!(a.value, 45.0);
    assert_eq!(a.unit, AngleUnit::Degrees);
}

#[test]
fn unit_value_angle_radians_construction() {
    let a: Angle = UnitValue {
        value: std::f32::consts::FRAC_PI_4,
        unit: AngleUnit::Radians,
    };
    assert!(approx(a.value, std::f32::consts::FRAC_PI_4, TIGHT_EPS));
    assert_eq!(a.unit, AngleUnit::Radians);
}

#[test]
fn unit_value_force_newtons_construction() {
    let f: Force = UnitValue {
        value: 9.81,
        unit: ForceUnit::Newtons,
    };
    assert!(approx(f.value, 9.81, TIGHT_EPS));
    assert_eq!(f.unit, ForceUnit::Newtons);
}

#[test]
fn unit_value_force_newton_meters_construction() {
    let f: Force = UnitValue {
        value: 50.0,
        unit: ForceUnit::NewtonMeters,
    };
    assert_eq!(f.value, 50.0);
    assert_eq!(f.unit, ForceUnit::NewtonMeters);
}

#[test]
fn unit_value_clone_and_copy() {
    let original: Speed = UnitValue {
        value: 42.0,
        unit: SpeedUnit::Knots,
    };
    let copied = original;
    let cloned = original.clone();
    assert_eq!(original, copied);
    assert_eq!(original, cloned);
}

#[test]
fn unit_value_debug_format() {
    let s: Speed = UnitValue {
        value: 1.0,
        unit: SpeedUnit::Knots,
    };
    let dbg = format!("{s:?}");
    assert!(dbg.contains("Knots"));
    assert!(dbg.contains("value"));
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Section 2 – Degrees ↔ Radians known values
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn degrees_to_radians_zero() {
    assert!(approx(conversions::degrees_to_radians(0.0), 0.0, TIGHT_EPS));
}

#[test]
fn degrees_to_radians_90() {
    assert!(approx(
        conversions::degrees_to_radians(90.0),
        std::f32::consts::FRAC_PI_2,
        TIGHT_EPS
    ));
}

#[test]
fn degrees_to_radians_180() {
    assert!(approx(
        conversions::degrees_to_radians(180.0),
        std::f32::consts::PI,
        TIGHT_EPS
    ));
}

#[test]
fn degrees_to_radians_360() {
    assert!(approx(
        conversions::degrees_to_radians(360.0),
        std::f32::consts::TAU,
        TIGHT_EPS
    ));
}

#[test]
fn degrees_to_radians_negative_90() {
    assert!(approx(
        conversions::degrees_to_radians(-90.0),
        -std::f32::consts::FRAC_PI_2,
        TIGHT_EPS
    ));
}

#[test]
fn radians_to_degrees_zero() {
    assert!(approx(conversions::radians_to_degrees(0.0), 0.0, TIGHT_EPS));
}

#[test]
fn radians_to_degrees_pi() {
    assert!(approx(
        conversions::radians_to_degrees(std::f32::consts::PI),
        180.0,
        TIGHT_EPS
    ));
}

#[test]
fn radians_to_degrees_two_pi() {
    assert!(approx(
        conversions::radians_to_degrees(std::f32::consts::TAU),
        360.0,
        TIGHT_EPS
    ));
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Section 3 – Speed conversions: known reference values
// ═══════════════════════════════════════════════════════════════════════════════

// 1 knot = 0.514444 m/s (exact definition)
#[test]
fn knots_to_mps_one_knot() {
    assert!(approx(conversions::knots_to_mps(1.0), 0.514444, TIGHT_EPS));
}

#[test]
fn knots_to_mps_100_knots() {
    assert!(approx(conversions::knots_to_mps(100.0), 51.4444, 0.01));
}

#[test]
fn mps_to_knots_one_mps() {
    assert!(approx(conversions::mps_to_knots(1.0), 1.94384, EPS));
}

#[test]
fn knots_to_mps_zero() {
    assert!(approx(conversions::knots_to_mps(0.0), 0.0, TIGHT_EPS));
}

#[test]
fn mps_to_knots_zero() {
    assert!(approx(conversions::mps_to_knots(0.0), 0.0, TIGHT_EPS));
}

// 1 knot ≈ 1.852 km/h
#[test]
fn knots_to_kph_one_knot() {
    assert!(approx(conversions::knots_to_kph(1.0), 1.852, 0.01));
}

#[test]
fn knots_to_kph_100_knots() {
    assert!(approx(conversions::knots_to_kph(100.0), 185.2, 0.2));
}

#[test]
fn kph_to_knots_one_kph() {
    assert!(approx(conversions::kph_to_knots(1.852), 1.0, 0.01));
}

// 36 km/h = 10 m/s
#[test]
fn kph_to_mps_36_kph() {
    assert!(approx(conversions::kph_to_mps(36.0), 10.0, 0.01));
}

#[test]
fn mps_to_kph_10_mps() {
    assert!(approx(conversions::mps_to_kph(10.0), 36.0, 0.01));
}

#[test]
fn kph_to_mps_zero() {
    assert!(approx(conversions::kph_to_mps(0.0), 0.0, TIGHT_EPS));
}

#[test]
fn mps_to_kph_zero() {
    assert!(approx(conversions::mps_to_kph(0.0), 0.0, TIGHT_EPS));
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Section 4 – Distance conversions: known reference values
// ═══════════════════════════════════════════════════════════════════════════════

// 1 foot = 0.3048 m (exact definition)
#[test]
fn feet_to_meters_one_foot() {
    assert!(approx(conversions::feet_to_meters(1.0), 0.3048, TIGHT_EPS));
}

#[test]
fn meters_to_feet_one_meter() {
    assert!(approx(conversions::meters_to_feet(1.0), 3.28084, EPS));
}

#[test]
fn feet_to_meters_zero() {
    assert!(approx(conversions::feet_to_meters(0.0), 0.0, TIGHT_EPS));
}

#[test]
fn meters_to_feet_zero() {
    assert!(approx(conversions::meters_to_feet(0.0), 0.0, TIGHT_EPS));
}

// FL350 = 35,000 ft ≈ 10,668 m
#[test]
fn feet_to_meters_fl350() {
    assert!(approx(conversions::feet_to_meters(35000.0), 10668.0, 1.0));
}

// Negative altitude (Dead Sea depression ~−1412 ft)
#[test]
fn feet_to_meters_negative() {
    assert!(approx(conversions::feet_to_meters(-1412.0), -430.4, 0.1));
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Section 5 – Vertical speed conversions: known reference values
// ═══════════════════════════════════════════════════════════════════════════════

// 196.85 ft/min ≈ 1 m/s
#[test]
fn fpm_to_mps_standard() {
    assert!(approx(conversions::fpm_to_mps(196.85), 1.0, 0.01));
}

#[test]
fn mps_to_fpm_standard() {
    assert!(approx(conversions::mps_to_fpm(1.0), 196.85, 0.1));
}

#[test]
fn fpm_to_mps_zero() {
    assert!(approx(conversions::fpm_to_mps(0.0), 0.0, TIGHT_EPS));
}

#[test]
fn mps_to_fpm_zero() {
    assert!(approx(conversions::mps_to_fpm(0.0), 0.0, TIGHT_EPS));
}

// Negative vertical speed (descent)
#[test]
fn fpm_to_mps_descent() {
    let mps = conversions::fpm_to_mps(-1000.0);
    assert!(mps < 0.0);
    assert!(approx(mps, -5.08, 0.01));
}

#[test]
fn mps_to_fpm_descent() {
    let fpm = conversions::mps_to_fpm(-5.08);
    assert!(fpm < 0.0);
    assert!(approx(fpm, -1000.0, 0.5));
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Section 6 – Angle normalization: exhaustive known values
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn normalize_signed_zero() {
    assert!(approx(angles::normalize_degrees_signed(0.0), 0.0, TIGHT_EPS));
}

#[test]
fn normalize_signed_90() {
    assert!(approx(angles::normalize_degrees_signed(90.0), 90.0, TIGHT_EPS));
}

#[test]
fn normalize_signed_180() {
    // Both +180 and −180 are valid representations of the boundary
    let result = angles::normalize_degrees_signed(180.0);
    assert!(
        approx(result, 180.0, TIGHT_EPS) || approx(result, -180.0, TIGHT_EPS),
        "expected ±180.0, got {result}"
    );
}

#[test]
fn normalize_signed_270() {
    assert!(approx(
        angles::normalize_degrees_signed(270.0),
        -90.0,
        TIGHT_EPS
    ));
}

#[test]
fn normalize_signed_360() {
    assert!(approx(angles::normalize_degrees_signed(360.0), 0.0, TIGHT_EPS));
}

#[test]
fn normalize_signed_neg90() {
    assert!(approx(
        angles::normalize_degrees_signed(-90.0),
        -90.0,
        TIGHT_EPS
    ));
}

#[test]
fn normalize_signed_neg270() {
    assert!(approx(angles::normalize_degrees_signed(-270.0), 90.0, TIGHT_EPS));
}

#[test]
fn normalize_signed_720() {
    assert!(approx(angles::normalize_degrees_signed(720.0), 0.0, TIGHT_EPS));
}

#[test]
fn normalize_signed_neg720() {
    assert!(approx(
        angles::normalize_degrees_signed(-720.0),
        0.0,
        TIGHT_EPS
    ));
}

#[test]
fn normalize_unsigned_zero() {
    assert!(approx(
        angles::normalize_degrees_unsigned(0.0),
        0.0,
        TIGHT_EPS
    ));
}

#[test]
fn normalize_unsigned_90() {
    assert!(approx(
        angles::normalize_degrees_unsigned(90.0),
        90.0,
        TIGHT_EPS
    ));
}

#[test]
fn normalize_unsigned_180() {
    assert!(approx(
        angles::normalize_degrees_unsigned(180.0),
        180.0,
        TIGHT_EPS
    ));
}

#[test]
fn normalize_unsigned_270() {
    assert!(approx(
        angles::normalize_degrees_unsigned(270.0),
        270.0,
        TIGHT_EPS
    ));
}

#[test]
fn normalize_unsigned_360() {
    assert!(approx(
        angles::normalize_degrees_unsigned(360.0),
        0.0,
        TIGHT_EPS
    ));
}

#[test]
fn normalize_unsigned_neg90() {
    assert!(approx(
        angles::normalize_degrees_unsigned(-90.0),
        270.0,
        TIGHT_EPS
    ));
}

#[test]
fn normalize_unsigned_450() {
    assert!(approx(
        angles::normalize_degrees_unsigned(450.0),
        90.0,
        TIGHT_EPS
    ));
}

#[test]
fn normalize_unsigned_neg360() {
    assert!(approx(
        angles::normalize_degrees_unsigned(-360.0),
        0.0,
        TIGHT_EPS
    ));
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Section 7 – Boundary / special float values
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn knots_to_mps_nan_returns_nan() {
    assert!(conversions::knots_to_mps(f32::NAN).is_nan());
}

#[test]
fn mps_to_knots_nan_returns_nan() {
    assert!(conversions::mps_to_knots(f32::NAN).is_nan());
}

#[test]
fn feet_to_meters_nan_returns_nan() {
    assert!(conversions::feet_to_meters(f32::NAN).is_nan());
}

#[test]
fn meters_to_feet_nan_returns_nan() {
    assert!(conversions::meters_to_feet(f32::NAN).is_nan());
}

#[test]
fn degrees_to_radians_nan_returns_nan() {
    assert!(conversions::degrees_to_radians(f32::NAN).is_nan());
}

#[test]
fn fpm_to_mps_nan_returns_nan() {
    assert!(conversions::fpm_to_mps(f32::NAN).is_nan());
}

#[test]
fn kph_to_mps_nan_returns_nan() {
    assert!(conversions::kph_to_mps(f32::NAN).is_nan());
}

#[test]
fn knots_to_mps_pos_inf() {
    assert_eq!(conversions::knots_to_mps(f32::INFINITY), f32::INFINITY);
}

#[test]
fn knots_to_mps_neg_inf() {
    assert_eq!(conversions::knots_to_mps(f32::NEG_INFINITY), f32::NEG_INFINITY);
}

#[test]
fn feet_to_meters_pos_inf() {
    assert_eq!(conversions::feet_to_meters(f32::INFINITY), f32::INFINITY);
}

#[test]
fn feet_to_meters_neg_inf() {
    assert_eq!(
        conversions::feet_to_meters(f32::NEG_INFINITY),
        f32::NEG_INFINITY
    );
}

#[test]
fn degrees_to_radians_pos_inf() {
    assert_eq!(
        conversions::degrees_to_radians(f32::INFINITY),
        f32::INFINITY
    );
}

#[test]
fn mps_to_kph_pos_inf() {
    assert_eq!(conversions::mps_to_kph(f32::INFINITY), f32::INFINITY);
}

#[test]
fn fpm_to_mps_pos_inf() {
    assert_eq!(conversions::fpm_to_mps(f32::INFINITY), f32::INFINITY);
}

#[test]
fn normalize_signed_nan_returns_nan() {
    assert!(angles::normalize_degrees_signed(f32::NAN).is_nan());
}

#[test]
fn normalize_unsigned_nan_returns_nan() {
    assert!(angles::normalize_degrees_unsigned(f32::NAN).is_nan());
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Section 8 – Round-trip identity (deterministic)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn roundtrip_knots_mps_250kt() {
    let kt = 250.0_f32;
    let back = conversions::mps_to_knots(conversions::knots_to_mps(kt));
    assert!(approx(back, kt, 0.01));
}

#[test]
fn roundtrip_kph_mps_900kph() {
    let kph = 900.0_f32;
    let back = conversions::mps_to_kph(conversions::kph_to_mps(kph));
    assert!(approx(back, kph, 0.01));
}

#[test]
fn roundtrip_knots_kph_500kt() {
    let kt = 500.0_f32;
    let back = conversions::kph_to_knots(conversions::knots_to_kph(kt));
    assert!(approx(back, kt, 0.1));
}

#[test]
fn roundtrip_feet_meters_40000ft() {
    let ft = 40000.0_f32;
    let back = conversions::meters_to_feet(conversions::feet_to_meters(ft));
    assert!(approx(back, ft, 0.1));
}

#[test]
fn roundtrip_fpm_mps_2000fpm() {
    let fpm = 2000.0_f32;
    let back = conversions::mps_to_fpm(conversions::fpm_to_mps(fpm));
    assert!(approx(back, fpm, 0.5));
}

#[test]
fn roundtrip_degrees_radians_45deg() {
    let deg = 45.0_f32;
    let back = conversions::radians_to_degrees(conversions::degrees_to_radians(deg));
    assert!(approx(back, deg, TIGHT_EPS));
}

#[test]
fn roundtrip_degrees_radians_negative_135deg() {
    let deg = -135.0_f32;
    let back = conversions::radians_to_degrees(conversions::degrees_to_radians(deg));
    assert!(approx(back, deg, TIGHT_EPS));
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Section 9 – Aviation-specific reference values
// ═══════════════════════════════════════════════════════════════════════════════

// Typical approach speed: 140 kt ≈ 72.02 m/s
#[test]
fn approach_speed_140kt() {
    assert!(approx(conversions::knots_to_mps(140.0), 72.02, 0.1));
}

// Mach 1 at sea level ≈ 661.5 kt ≈ 340.3 m/s
#[test]
fn mach1_sea_level_knots_to_mps() {
    assert!(approx(conversions::knots_to_mps(661.5), 340.3, 0.5));
}

// Standard pressure altitude: 29,029 ft (Everest) ≈ 8,848 m
#[test]
fn everest_altitude_ft_to_m() {
    assert!(approx(conversions::feet_to_meters(29029.0), 8848.0, 1.0));
}

// Standard climb rate: 1500 fpm ≈ 7.62 m/s
#[test]
fn standard_climb_1500fpm() {
    assert!(approx(conversions::fpm_to_mps(1500.0), 7.62, 0.01));
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Section 10 – Serde round-trip (JSON)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn serde_speed_roundtrip_json() {
    let s: Speed = UnitValue {
        value: 250.0,
        unit: SpeedUnit::Knots,
    };
    let json = serde_json::to_string(&s).unwrap();
    let back: Speed = serde_json::from_str(&json).unwrap();
    assert_eq!(s, back);
}

#[test]
fn serde_angle_roundtrip_json() {
    let a: Angle = UnitValue {
        value: 3.14,
        unit: AngleUnit::Radians,
    };
    let json = serde_json::to_string(&a).unwrap();
    let back: Angle = serde_json::from_str(&json).unwrap();
    assert_eq!(a, back);
}

#[test]
fn serde_force_roundtrip_json() {
    let f: Force = UnitValue {
        value: 9.81,
        unit: ForceUnit::Newtons,
    };
    let json = serde_json::to_string(&f).unwrap();
    let back: Force = serde_json::from_str(&json).unwrap();
    assert_eq!(f, back);
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Section 11 – Property-based tests (proptest)
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    // ── Round-trip identity ──────────────────────────────────────────────────

    #[test]
    fn prop_knots_mps_roundtrip(v in 0.001f32..50000.0) {
        let back = conversions::mps_to_knots(conversions::knots_to_mps(v));
        prop_assert!((v - back).abs() < v * 0.001 + TIGHT_EPS);
    }

    #[test]
    fn prop_kph_mps_roundtrip(v in 0.001f32..50000.0) {
        let back = conversions::mps_to_kph(conversions::kph_to_mps(v));
        prop_assert!((v - back).abs() < v * 0.001 + TIGHT_EPS);
    }

    #[test]
    fn prop_knots_kph_roundtrip(v in 0.001f32..50000.0) {
        let back = conversions::kph_to_knots(conversions::knots_to_kph(v));
        prop_assert!((v - back).abs() < v * 0.002 + EPS);
    }

    #[test]
    fn prop_feet_meters_roundtrip(v in -100000.0f32..100000.0) {
        let back = conversions::meters_to_feet(conversions::feet_to_meters(v));
        prop_assert!((v - back).abs() < v.abs() * 0.001 + TIGHT_EPS);
    }

    #[test]
    fn prop_fpm_mps_roundtrip(v in -50000.0f32..50000.0) {
        let back = conversions::mps_to_fpm(conversions::fpm_to_mps(v));
        prop_assert!((v - back).abs() < v.abs() * 0.002 + 0.01);
    }

    #[test]
    fn prop_degrees_radians_roundtrip(v in -3600.0f32..3600.0) {
        let back = conversions::radians_to_degrees(conversions::degrees_to_radians(v));
        prop_assert!((v - back).abs() < v.abs() * 0.001 + TIGHT_EPS);
    }

    // ── Monotonicity: more input → more output ──────────────────────────────

    #[test]
    fn prop_knots_to_mps_monotonic(a in 0.0f32..10000.0, b in 0.0f32..10000.0) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        prop_assert!(conversions::knots_to_mps(lo) <= conversions::knots_to_mps(hi));
    }

    #[test]
    fn prop_feet_to_meters_monotonic(a in -50000.0f32..50000.0, b in -50000.0f32..50000.0) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        prop_assert!(conversions::feet_to_meters(lo) <= conversions::feet_to_meters(hi));
    }

    #[test]
    fn prop_kph_to_mps_monotonic(a in 0.0f32..10000.0, b in 0.0f32..10000.0) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        prop_assert!(conversions::kph_to_mps(lo) <= conversions::kph_to_mps(hi));
    }

    #[test]
    fn prop_fpm_to_mps_monotonic(a in -50000.0f32..50000.0, b in -50000.0f32..50000.0) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        prop_assert!(conversions::fpm_to_mps(lo) <= conversions::fpm_to_mps(hi));
    }

    #[test]
    fn prop_degrees_to_radians_monotonic(a in -1000.0f32..1000.0, b in -1000.0f32..1000.0) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        prop_assert!(conversions::degrees_to_radians(lo) <= conversions::degrees_to_radians(hi));
    }

    // ── Normalization bounds ─────────────────────────────────────────────────

    #[test]
    fn prop_normalize_signed_in_range(v in -100000.0f32..100000.0) {
        let n = angles::normalize_degrees_signed(v);
        prop_assert!(n >= -180.0 - TIGHT_EPS);
        prop_assert!(n <= 180.0 + TIGHT_EPS);
    }

    #[test]
    fn prop_normalize_unsigned_in_range(v in -100000.0f32..100000.0) {
        let n = angles::normalize_degrees_unsigned(v);
        prop_assert!(n >= -TIGHT_EPS);
        prop_assert!(n < 360.0 + TIGHT_EPS);
    }

    // ── Normalization idempotence: normalizing twice gives same result ───────

    #[test]
    fn prop_normalize_signed_idempotent(v in -100000.0f32..100000.0) {
        let once = angles::normalize_degrees_signed(v);
        let twice = angles::normalize_degrees_signed(once);
        prop_assert!((once - twice).abs() < TIGHT_EPS);
    }

    #[test]
    fn prop_normalize_unsigned_idempotent(v in -100000.0f32..100000.0) {
        let once = angles::normalize_degrees_unsigned(v);
        let twice = angles::normalize_degrees_unsigned(once);
        prop_assert!((once - twice).abs() < TIGHT_EPS);
    }

    // ── Normalization periodicity: v and v+360 normalize identically ────────

    #[test]
    fn prop_normalize_signed_periodic(v in -10000.0f32..10000.0) {
        let n1 = angles::normalize_degrees_signed(v);
        let n2 = angles::normalize_degrees_signed(v + 360.0);
        prop_assert!((n1 - n2).abs() < EPS);
    }

    #[test]
    fn prop_normalize_unsigned_periodic(v in -10000.0f32..10000.0) {
        let n1 = angles::normalize_degrees_unsigned(v);
        let n2 = angles::normalize_degrees_unsigned(v + 360.0);
        prop_assert!((n1 - n2).abs() < EPS);
    }

    // ── Conversion sign preservation ────────────────────────────────────────

    #[test]
    fn prop_knots_to_mps_sign_preserving(v in -10000.0f32..10000.0) {
        // Skip near-zero values where underflow can produce ±0.0
        if v.abs() < 1e-6 { return Ok(()); }
        let out = conversions::knots_to_mps(v);
        prop_assert_eq!(v.signum(), out.signum());
    }

    #[test]
    fn prop_feet_to_meters_sign_preserving(v in -100000.0f32..100000.0) {
        if v.abs() < 1e-6 { return Ok(()); }
        let out = conversions::feet_to_meters(v);
        prop_assert_eq!(v.signum(), out.signum());
    }

    // ── Cross-path consistency: knots→kph via two routes should match ───────

    #[test]
    fn prop_knots_kph_consistency(kt in 0.0f32..5000.0) {
        let direct = conversions::knots_to_kph(kt);
        let via_mps = conversions::mps_to_kph(conversions::knots_to_mps(kt));
        let tol = TIGHT_EPS + direct.abs() * 1e-5;
        prop_assert!((direct - via_mps).abs() < tol);
    }
}
