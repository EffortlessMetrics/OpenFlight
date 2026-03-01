// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Expanded property-based tests for flight-units mathematical invariants.
//!
//! Tests beyond existing roundtrip/range coverage:
//! 1. Conversion roundtrip identity (meters↔feet, verified within tolerance)
//! 2. Monotonicity preserved: if a < b then convert(a) < convert(b)
//! 3. Transitivity: knots→mps→kph == knots→kph (within tolerance)
//! 4. Zero maps to zero for all conversion functions
//! 5. Negative handling: sign preserved through conversions

use flight_units::{angles, conversions};
use proptest::prelude::*;

proptest! {
    // ── 1. Conversion roundtrip identity ────────────────────────────────────

    /// meters↔feet roundtrip preserves value within tolerance.
    #[test]
    fn meters_feet_roundtrip(meters in -10000.0f32..10000.0f32) {
        let feet = conversions::meters_to_feet(meters);
        let back = conversions::feet_to_meters(feet);
        prop_assert!(
            (meters - back).abs() < 1e-2,
            "meters→feet→meters roundtrip: {} → {} → {}", meters, feet, back
        );
    }

    /// fpm↔mps roundtrip preserves value within tolerance.
    #[test]
    fn fpm_mps_roundtrip(fpm in -5000.0f32..5000.0f32) {
        let mps = conversions::fpm_to_mps(fpm);
        let back = conversions::mps_to_fpm(mps);
        prop_assert!(
            (fpm - back).abs() < 0.5,
            "fpm→mps→fpm roundtrip: {} → {} → {}", fpm, mps, back
        );
    }

    // ── 2. Monotonicity preserved ───────────────────────────────────────────

    /// Knots→MPS is strictly monotone: if a < b then knots_to_mps(a) < knots_to_mps(b).
    #[test]
    fn knots_to_mps_monotone(
        a in 0.0f32..500.0f32,
        b in 0.0f32..500.0f32,
    ) {
        prop_assume!(a != b);
        let (lo, hi) = if a < b { (a, b) } else { (b, a) };
        let mps_lo = conversions::knots_to_mps(lo);
        let mps_hi = conversions::knots_to_mps(hi);
        prop_assert!(
            mps_lo < mps_hi,
            "monotonicity violated: knots_to_mps({})={} >= knots_to_mps({})={}",
            lo, mps_lo, hi, mps_hi
        );
    }

    /// Feet→meters is strictly monotone.
    #[test]
    fn feet_to_meters_monotone(
        a in -10000.0f32..50000.0f32,
        b in -10000.0f32..50000.0f32,
    ) {
        prop_assume!((a - b).abs() > 1e-3);
        let (lo, hi) = if a < b { (a, b) } else { (b, a) };
        let m_lo = conversions::feet_to_meters(lo);
        let m_hi = conversions::feet_to_meters(hi);
        prop_assert!(
            m_lo < m_hi,
            "monotonicity violated: feet_to_meters({})={} >= feet_to_meters({})={}",
            lo, m_lo, hi, m_hi
        );
    }

    /// degrees→radians is strictly monotone.
    #[test]
    fn degrees_to_radians_monotone(
        a in -360.0f32..360.0f32,
        b in -360.0f32..360.0f32,
    ) {
        prop_assume!((a - b).abs() > 1e-3);
        let (lo, hi) = if a < b { (a, b) } else { (b, a) };
        let r_lo = conversions::degrees_to_radians(lo);
        let r_hi = conversions::degrees_to_radians(hi);
        prop_assert!(
            r_lo < r_hi,
            "monotonicity violated: deg_to_rad({})={} >= deg_to_rad({})={}",
            lo, r_lo, hi, r_hi
        );
    }

    // ── 3. Transitivity ────────────────────────────────────────────────────

    /// Transitivity: knots→mps→kph matches knots→kph (indirect vs direct).
    #[test]
    fn knots_kph_transitivity(knots in 0.0f32..1000.0f32) {
        let direct = conversions::knots_to_kph(knots);
        let indirect = conversions::mps_to_kph(conversions::knots_to_mps(knots));
        prop_assert!(
            (direct - indirect).abs() < 1e-3,
            "transitivity: knots_to_kph({})={} vs via mps={}", knots, direct, indirect
        );
    }

    /// Transitivity: kph→knots direct matches kph→mps→knots indirect.
    #[test]
    fn kph_knots_transitivity(kph in 0.0f32..1000.0f32) {
        let direct = conversions::kph_to_knots(kph);
        let indirect = conversions::mps_to_knots(conversions::kph_to_mps(kph));
        prop_assert!(
            (direct - indirect).abs() < 1e-3,
            "transitivity: kph_to_knots({})={} vs via mps={}", kph, direct, indirect
        );
    }

    // ── 4. Zero maps to zero ────────────────────────────────────────────────

    /// All conversion functions map 0.0 to 0.0 (or very close).
    #[test]
    fn zero_maps_to_zero(_dummy in 0u8..1u8) {
        let conversions_at_zero = [
            ("knots_to_mps", conversions::knots_to_mps(0.0)),
            ("mps_to_knots", conversions::mps_to_knots(0.0)),
            ("kph_to_mps", conversions::kph_to_mps(0.0)),
            ("mps_to_kph", conversions::mps_to_kph(0.0)),
            ("knots_to_kph", conversions::knots_to_kph(0.0)),
            ("kph_to_knots", conversions::kph_to_knots(0.0)),
            ("feet_to_meters", conversions::feet_to_meters(0.0)),
            ("meters_to_feet", conversions::meters_to_feet(0.0)),
            ("fpm_to_mps", conversions::fpm_to_mps(0.0)),
            ("mps_to_fpm", conversions::mps_to_fpm(0.0)),
            ("deg_to_rad", conversions::degrees_to_radians(0.0)),
            ("rad_to_deg", conversions::radians_to_degrees(0.0)),
        ];
        for (name, val) in &conversions_at_zero {
            prop_assert!(
                val.abs() < 1e-6,
                "{} at 0.0 produced {}, expected ~0.0", name, val
            );
        }
    }

    // ── 5. Negative handling: sign preserved ────────────────────────────────

    /// Negative values preserve sign through knots↔mps conversion.
    #[test]
    fn negative_sign_preserved_knots_mps(val in -1000.0f32..-0.001f32) {
        let mps = conversions::knots_to_mps(val);
        prop_assert!(
            mps < 0.0,
            "negative knots {} should produce negative mps, got {}", val, mps
        );
        let back = conversions::mps_to_knots(mps);
        prop_assert!(
            back < 0.0,
            "negative mps {} should produce negative knots, got {}", mps, back
        );
    }

    /// Negative values preserve sign through feet↔meters conversion.
    #[test]
    fn negative_sign_preserved_feet_meters(val in -10000.0f32..-0.001f32) {
        let meters = conversions::feet_to_meters(val);
        prop_assert!(
            meters < 0.0,
            "negative feet {} should produce negative meters, got {}", val, meters
        );
        let back = conversions::meters_to_feet(meters);
        prop_assert!(
            back < 0.0,
            "negative meters {} should produce negative feet, got {}", meters, back
        );
    }

    /// Angle normalization: signed normalization of negative angles stays in [-180, 180].
    #[test]
    fn normalize_signed_negative_input(degrees in -10000.0f32..0.0f32) {
        let n = angles::normalize_degrees_signed(degrees);
        prop_assert!(
            (-180.0..=180.0 + 1e-5).contains(&n),
            "normalize_degrees_signed({}) = {} out of [-180, 180]", degrees, n
        );
    }
}
