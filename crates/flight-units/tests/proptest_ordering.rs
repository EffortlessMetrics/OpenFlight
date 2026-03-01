// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Property-based tests for unit conversion ordering and sign preservation.
//!
//! - Speed conversions preserve ordering: a > b ⟹ convert(a) > convert(b)
//! - Positive inputs yield positive outputs for all distance/speed conversions
//! - Double normalization is idempotent
//! - Angle conversion preserves ordering within a half-circle

use flight_units::{angles, conversions};
use proptest::prelude::*;

proptest! {
    // ── Speed ordering ──────────────────────────────────────────────────────

    /// Knots-to-MPS preserves ordering: if a > b then knots_to_mps(a) > knots_to_mps(b).
    #[test]
    fn knots_to_mps_preserves_order(
        a in 0.0f32..10000.0,
        b in 0.0f32..10000.0,
    ) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        let mps_lo = conversions::knots_to_mps(lo);
        let mps_hi = conversions::knots_to_mps(hi);
        prop_assert!(
            mps_lo <= mps_hi,
            "ordering violated: knots_to_mps({})={} > knots_to_mps({})={}",
            lo, mps_lo, hi, mps_hi
        );
    }

    /// KPH-to-MPS preserves ordering.
    #[test]
    fn kph_to_mps_preserves_order(
        a in 0.0f32..10000.0,
        b in 0.0f32..10000.0,
    ) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        let mps_lo = conversions::kph_to_mps(lo);
        let mps_hi = conversions::kph_to_mps(hi);
        prop_assert!(
            mps_lo <= mps_hi,
            "ordering violated: kph_to_mps({})={} > kph_to_mps({})={}",
            lo, mps_lo, hi, mps_hi
        );
    }

    /// Feet-to-meters preserves ordering.
    #[test]
    fn feet_to_meters_preserves_order(
        a in -50000.0f32..50000.0,
        b in -50000.0f32..50000.0,
    ) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        let m_lo = conversions::feet_to_meters(lo);
        let m_hi = conversions::feet_to_meters(hi);
        prop_assert!(
            m_lo <= m_hi,
            "ordering violated: feet_to_meters({})={} > feet_to_meters({})={}",
            lo, m_lo, hi, m_hi
        );
    }

    // ── Positive input ⟹ positive output ────────────────────────────────────

    /// Positive knots produce positive MPS.
    #[test]
    fn positive_knots_positive_mps(knots in 0.001f32..10000.0) {
        let mps = conversions::knots_to_mps(knots);
        prop_assert!(mps > 0.0, "knots_to_mps({})={} should be positive", knots, mps);
    }

    /// Positive feet produce positive meters.
    #[test]
    fn positive_feet_positive_meters(feet in 0.001f32..100000.0) {
        let m = conversions::feet_to_meters(feet);
        prop_assert!(m > 0.0, "feet_to_meters({})={} should be positive", feet, m);
    }

    /// Positive KPH produces positive MPS.
    #[test]
    fn positive_kph_positive_mps(kph in 0.001f32..10000.0) {
        let mps = conversions::kph_to_mps(kph);
        prop_assert!(mps > 0.0, "kph_to_mps({})={} should be positive", kph, mps);
    }

    /// Positive FPM produces positive MPS.
    #[test]
    fn positive_fpm_positive_mps(fpm in 0.001f32..100000.0) {
        let mps = conversions::fpm_to_mps(fpm);
        prop_assert!(mps > 0.0, "fpm_to_mps({})={} should be positive", fpm, mps);
    }

    // ── Angle normalization idempotency ─────────────────────────────────────

    /// Normalizing signed degrees twice gives the same result.
    #[test]
    fn normalize_signed_idempotent(degrees in -10000.0f32..10000.0) {
        let once = angles::normalize_degrees_signed(degrees);
        let twice = angles::normalize_degrees_signed(once);
        prop_assert!(
            (once - twice).abs() < 1e-3,
            "normalize_signed not idempotent: once={}, twice={} for input={}",
            once, twice, degrees
        );
    }

    /// Normalizing unsigned degrees twice gives the same result.
    #[test]
    fn normalize_unsigned_idempotent(degrees in -10000.0f32..10000.0) {
        let once = angles::normalize_degrees_unsigned(degrees);
        let twice = angles::normalize_degrees_unsigned(once);
        prop_assert!(
            (once - twice).abs() < 1e-3,
            "normalize_unsigned not idempotent: once={}, twice={} for input={}",
            once, twice, degrees
        );
    }

    // ── Degree-radian ordering (full circle) ───────────────────────────────

    /// Degrees-to-radians preserves ordering for inputs in [0, 360].
    #[test]
    fn degrees_to_radians_preserves_order(
        a in 0.0f32..360.0,
        b in 0.0f32..360.0,
    ) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        let rad_lo = conversions::degrees_to_radians(lo);
        let rad_hi = conversions::degrees_to_radians(hi);
        prop_assert!(
            rad_lo <= rad_hi,
            "ordering violated: deg_to_rad({})={} > deg_to_rad({})={}",
            lo, rad_lo, hi, rad_hi
        );
    }

    // ── Zero maps to zero ───────────────────────────────────────────────────

    /// Zero input produces zero output for all conversions.
    #[test]
    fn zero_input_zero_output(_dummy in 0u8..1u8) {
        prop_assert_eq!(conversions::knots_to_mps(0.0), 0.0);
        prop_assert_eq!(conversions::mps_to_knots(0.0), 0.0);
        prop_assert_eq!(conversions::kph_to_mps(0.0), 0.0);
        prop_assert_eq!(conversions::mps_to_kph(0.0), 0.0);
        prop_assert_eq!(conversions::feet_to_meters(0.0), 0.0);
        prop_assert_eq!(conversions::meters_to_feet(0.0), 0.0);
        prop_assert_eq!(conversions::fpm_to_mps(0.0), 0.0);
        prop_assert_eq!(conversions::mps_to_fpm(0.0), 0.0);
        prop_assert_eq!(conversions::degrees_to_radians(0.0), 0.0);
        prop_assert_eq!(conversions::radians_to_degrees(0.0), 0.0);
    }
}
