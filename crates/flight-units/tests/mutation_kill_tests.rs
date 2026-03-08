// SPDX-License-Identifier: MIT OR Apache-2.0
// Targeted tests to improve mutation kill rate in flight-units.
// Covers conversion coefficient accuracy, angle normalization boundaries,
// zero/negative handling, and formula correctness.

use flight_units::angles;
use flight_units::conversions;

// ── Conversion coefficient accuracy ──────────────────────────────────────

#[test]
fn knots_to_mps_coefficient_correct() {
    // 1 knot = 0.514444 m/s — catches wrong coefficient
    let mps = conversions::knots_to_mps(1.0);
    assert!(
        (mps - 0.514444).abs() < 0.0001,
        "1 knot must be ~0.514444 mps, got {mps}"
    );
    // 100 knots
    let mps100 = conversions::knots_to_mps(100.0);
    assert!(
        (mps100 - 51.4444).abs() < 0.01,
        "100 knots must be ~51.4444 mps, got {mps100}"
    );
}

#[test]
fn mps_to_knots_coefficient_correct() {
    let knots = conversions::mps_to_knots(0.514444);
    assert!(
        (knots - 1.0).abs() < 0.001,
        "0.514444 mps must be ~1 knot, got {knots}"
    );
}

#[test]
fn kph_to_mps_coefficient_correct() {
    // 3.6 kph = 1 mps
    let mps = conversions::kph_to_mps(3.6);
    assert!(
        (mps - 1.0).abs() < 0.01,
        "3.6 kph must be ~1.0 mps, got {mps}"
    );
}

#[test]
fn mps_to_kph_coefficient_correct() {
    // 1 mps = 3.6 kph
    let kph = conversions::mps_to_kph(1.0);
    assert!(
        (kph - 3.6).abs() < 0.001,
        "1 mps must be 3.6 kph, got {kph}"
    );
}

#[test]
fn feet_to_meters_coefficient_correct() {
    // 1 foot = 0.3048 meters
    let m = conversions::feet_to_meters(1.0);
    assert!(
        (m - 0.3048).abs() < 0.0001,
        "1 foot must be 0.3048 m, got {m}"
    );
}

#[test]
fn meters_to_feet_coefficient_correct() {
    let ft = conversions::meters_to_feet(0.3048);
    assert!(
        (ft - 1.0).abs() < 0.001,
        "0.3048 m must be ~1 foot, got {ft}"
    );
}

#[test]
fn fpm_to_mps_coefficient_correct() {
    // 1 mps ≈ 196.85 fpm, so 196.85 fpm ≈ 1 mps
    let mps = conversions::fpm_to_mps(196.85);
    assert!(
        (mps - 1.0).abs() < 0.01,
        "196.85 fpm must be ~1.0 mps, got {mps}"
    );
}

#[test]
fn mps_to_fpm_coefficient_correct() {
    let fpm = conversions::mps_to_fpm(1.0);
    assert!(
        (fpm - 196.85).abs() < 0.1,
        "1 mps must be ~196.85 fpm, got {fpm}"
    );
}

// ── Zero value handling ──────────────────────────────────────────────────

#[test]
fn all_conversions_preserve_zero() {
    // Catches mutation that adds constant offset
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
}

// ── Negative values ──────────────────────────────────────────────────────

#[test]
fn fpm_to_mps_preserves_sign() {
    // Catches abs() mutation or sign flip
    let mps = conversions::fpm_to_mps(-500.0);
    assert!(mps < 0.0, "negative fpm must give negative mps, got {mps}");
}

#[test]
fn mps_to_fpm_preserves_sign() {
    let fpm = conversions::mps_to_fpm(-2.0);
    assert!(fpm < 0.0, "negative mps must give negative fpm, got {fpm}");
}

// ── Degrees/Radians known values ─────────────────────────────────────────

#[test]
fn degrees_to_radians_known_values() {
    let pi = std::f32::consts::PI;
    assert!(
        (conversions::degrees_to_radians(90.0) - pi / 2.0).abs() < 0.0001,
        "90° must be π/2"
    );
    assert!(
        (conversions::degrees_to_radians(180.0) - pi).abs() < 0.0001,
        "180° must be π"
    );
    assert!(
        (conversions::degrees_to_radians(360.0) - 2.0 * pi).abs() < 0.0001,
        "360° must be 2π"
    );
    assert!(
        (conversions::degrees_to_radians(-90.0) + pi / 2.0).abs() < 0.0001,
        "-90° must be -π/2"
    );
}

// ── Angle normalization boundary cases ───────────────────────────────────

#[test]
fn normalize_signed_boundary_values() {
    // Catches off-by-one in modular arithmetic
    assert!(
        (angles::normalize_degrees_signed(0.0)).abs() < 0.001,
        "0 must stay 0"
    );
    assert!(
        (angles::normalize_degrees_signed(180.0) - 180.0).abs() < 0.001
            || (angles::normalize_degrees_signed(180.0) + 180.0).abs() < 0.001,
        "180 must be ±180"
    );
    assert!(
        (angles::normalize_degrees_signed(-180.0) + 180.0).abs() < 0.001,
        "-180 must stay -180"
    );
    assert!(
        (angles::normalize_degrees_signed(360.0)).abs() < 0.001,
        "360 must normalize to 0"
    );
    assert!(
        (angles::normalize_degrees_signed(-360.0)).abs() < 0.001,
        "-360 must normalize to 0"
    );
}

#[test]
fn normalize_unsigned_boundary_values() {
    assert!(
        (angles::normalize_degrees_unsigned(0.0)).abs() < 0.001,
        "0 must stay 0"
    );
    assert!(
        (angles::normalize_degrees_unsigned(360.0)).abs() < 0.001,
        "360 must normalize to 0"
    );
    assert!(
        (angles::normalize_degrees_unsigned(-360.0)).abs() < 0.001,
        "-360 must normalize to 0"
    );
    assert!(
        (angles::normalize_degrees_unsigned(90.0) - 90.0).abs() < 0.001,
        "90 must stay 90"
    );
    assert!(
        (angles::normalize_degrees_unsigned(-90.0) - 270.0).abs() < 0.001,
        "-90 must normalize to 270"
    );
}

#[test]
fn normalize_signed_range_always_in_bounds() {
    // Catches formula mutation in ((deg % 360) + 540) % 360 - 180
    for deg in [-720, -360, -180, -90, 0, 90, 180, 270, 360, 720] {
        let n = angles::normalize_degrees_signed(deg as f32);
        assert!(
            (-180.0..=180.0).contains(&n),
            "normalize_signed({deg}) = {n} out of [-180, 180]"
        );
    }
}

#[test]
fn normalize_unsigned_range_always_in_bounds() {
    for deg in [-720, -360, -180, -90, 0, 90, 180, 270, 360, 720] {
        let n = angles::normalize_degrees_unsigned(deg as f32);
        assert!(
            (0.0..360.0).contains(&n),
            "normalize_unsigned({deg}) = {n} out of [0, 360)"
        );
    }
}

// ── Chained conversion consistency ───────────────────────────────────────

#[test]
fn knots_kph_consistency() {
    // knots → kph directly vs knots → mps → kph must agree
    let direct = conversions::knots_to_kph(100.0);
    let indirect = conversions::mps_to_kph(conversions::knots_to_mps(100.0));
    assert!(
        (direct - indirect).abs() < 0.01,
        "direct={direct}, indirect={indirect}"
    );
}

#[test]
fn kph_knots_consistency() {
    let direct = conversions::kph_to_knots(185.2);
    let indirect = conversions::mps_to_knots(conversions::kph_to_mps(185.2));
    assert!(
        (direct - indirect).abs() < 0.01,
        "direct={direct}, indirect={indirect}"
    );
}
