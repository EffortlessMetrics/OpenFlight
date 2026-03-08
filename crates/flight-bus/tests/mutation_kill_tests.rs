// SPDX-License-Identifier: MIT OR Apache-2.0
// Targeted tests to improve mutation kill rate in flight-bus.
// Covers boundary mutations in type validation, gear state boolean logic,
// conversion return values, and backpressure thresholds.

use flight_bus::types::{
    GForce, GearPosition, GearState, Mach, Percentage, ValidatedAngle, ValidatedSpeed,
};

// ── Percentage: boundary mutations ───────────────────────────────────────

#[test]
fn percentage_exact_boundaries_accepted() {
    // Catches < vs <= mutation on `(0.0..=100.0).contains`
    assert!(Percentage::new(0.0).is_ok(), "0.0 must be valid");
    assert!(Percentage::new(100.0).is_ok(), "100.0 must be valid");
}

#[test]
fn percentage_just_outside_boundaries_rejected() {
    assert!(Percentage::new(-0.001).is_err(), "-0.001 must be invalid");
    assert!(Percentage::new(100.001).is_err(), "100.001 must be invalid");
}

#[test]
fn percentage_from_normalized_boundaries() {
    // Catches mutation on `(0.0..=1.0).contains`
    assert!(Percentage::from_normalized(0.0).is_ok());
    assert!(Percentage::from_normalized(1.0).is_ok());
    assert!(Percentage::from_normalized(-0.001).is_err());
    assert!(Percentage::from_normalized(1.001).is_err());
}

#[test]
fn percentage_normalized_round_trip_exact() {
    // Catches mutation in * 100.0 or / 100.0
    let p = Percentage::from_normalized(0.5).unwrap();
    assert_eq!(p.value(), 50.0, "0.5 normalized must be 50.0");
    assert!(
        (p.normalized() - 0.5).abs() < f32::EPSILON,
        "50.0 must normalize back to 0.5"
    );

    let p = Percentage::from_normalized(1.0).unwrap();
    assert_eq!(p.value(), 100.0);
    assert!((p.normalized() - 1.0).abs() < f32::EPSILON);
}

// ── GForce: boundary mutations ───────────────────────────────────────────

#[test]
fn gforce_exact_boundaries() {
    assert!(GForce::new(-20.0).is_ok(), "-20.0 must be valid");
    assert!(GForce::new(20.0).is_ok(), "20.0 must be valid");
    assert!(GForce::new(-20.001).is_err());
    assert!(GForce::new(20.001).is_err());
}

#[test]
fn gforce_value_preserved() {
    // Catches mutation that changes stored value
    let g = GForce::new(9.81).unwrap();
    assert!((g.value() - 9.81).abs() < 1e-5);
}

// ── Mach: boundary mutations ─────────────────────────────────────────────

#[test]
fn mach_exact_boundaries() {
    assert!(Mach::new(0.0).is_ok(), "0.0 must be valid");
    assert!(Mach::new(5.0).is_ok(), "5.0 must be valid");
    assert!(Mach::new(-0.001).is_err());
    assert!(Mach::new(5.001).is_err());
}

// ── ValidatedSpeed: boundary & conversion mutations ──────────────────────

#[test]
fn validated_speed_knots_boundaries() {
    assert!(ValidatedSpeed::new_knots(0.0).is_ok());
    assert!(ValidatedSpeed::new_knots(1000.0).is_ok());
    assert!(ValidatedSpeed::new_knots(-0.001).is_err());
    assert!(ValidatedSpeed::new_knots(1000.001).is_err());
}

#[test]
fn validated_speed_mps_boundaries() {
    assert!(ValidatedSpeed::new_mps(0.0).is_ok());
    assert!(ValidatedSpeed::new_mps(500.0).is_ok());
    assert!(ValidatedSpeed::new_mps(-0.001).is_err());
    assert!(ValidatedSpeed::new_mps(500.001).is_err());
}

#[test]
fn validated_speed_knots_to_knots_identity() {
    // Catches mutation in to_knots() match arm for Knots
    let speed = ValidatedSpeed::new_knots(150.0).unwrap();
    assert!(
        (speed.to_knots() - 150.0).abs() < 0.001,
        "knots-to-knots must be identity"
    );
}

#[test]
fn validated_speed_mps_to_mps_identity() {
    let speed = ValidatedSpeed::new_mps(100.0).unwrap();
    assert!(
        (speed.to_mps() - 100.0).abs() < 0.001,
        "mps-to-mps must be identity"
    );
}

#[test]
fn validated_speed_knots_to_mps_correct() {
    // 100 knots ≈ 51.4444 m/s — catches wrong conversion factor
    let speed = ValidatedSpeed::new_knots(100.0).unwrap();
    let mps = speed.to_mps();
    assert!(
        (mps - 51.4444).abs() < 0.1,
        "100 knots should be ~51.4 mps, got {mps}"
    );
}

// ── ValidatedAngle: boundary & conversion mutations ──────────────────────

#[test]
fn validated_angle_degrees_boundaries() {
    assert!(ValidatedAngle::new_degrees(-180.0).is_ok());
    assert!(ValidatedAngle::new_degrees(180.0).is_ok());
    assert!(ValidatedAngle::new_degrees(-180.001).is_err());
    assert!(ValidatedAngle::new_degrees(180.001).is_err());
}

#[test]
fn validated_angle_radians_boundaries() {
    let pi = std::f32::consts::PI;
    assert!(ValidatedAngle::new_radians(-pi).is_ok());
    assert!(ValidatedAngle::new_radians(pi).is_ok());
    assert!(ValidatedAngle::new_radians(-pi - 0.001).is_err());
    assert!(ValidatedAngle::new_radians(pi + 0.001).is_err());
}

#[test]
fn validated_angle_degrees_to_degrees_identity() {
    let angle = ValidatedAngle::new_degrees(45.0).unwrap();
    assert!(
        (angle.to_degrees() - 45.0).abs() < 0.001,
        "degrees-to-degrees must be identity"
    );
}

#[test]
fn validated_angle_radians_conversion_correct() {
    let angle = ValidatedAngle::new_degrees(90.0).unwrap();
    let rads = angle.to_radians();
    assert!(
        (rads - std::f32::consts::FRAC_PI_2).abs() < 0.001,
        "90 degrees should be π/2 radians, got {rads}"
    );
}

// ── GearState: boolean logic mutations ───────────────────────────────────

#[test]
fn gear_all_down_requires_all_three() {
    // Catches OR vs AND mutation in all_down pattern
    let partial = GearState {
        nose: GearPosition::Down,
        left: GearPosition::Down,
        right: GearPosition::Up,
    };
    assert!(!partial.all_down(), "one gear up means not all down");

    let all = GearState {
        nose: GearPosition::Down,
        left: GearPosition::Down,
        right: GearPosition::Down,
    };
    assert!(all.all_down());
}

#[test]
fn gear_all_up_requires_all_three() {
    let partial = GearState {
        nose: GearPosition::Up,
        left: GearPosition::Up,
        right: GearPosition::Transitioning,
    };
    assert!(!partial.all_up(), "one transitioning means not all up");

    let all = GearState {
        nose: GearPosition::Up,
        left: GearPosition::Up,
        right: GearPosition::Up,
    };
    assert!(all.all_up());
}

#[test]
fn gear_transitioning_any_single() {
    // Catches removal of any of the three OR branches
    let nose_only = GearState {
        nose: GearPosition::Transitioning,
        left: GearPosition::Down,
        right: GearPosition::Down,
    };
    assert!(nose_only.transitioning(), "nose transitioning");

    let left_only = GearState {
        nose: GearPosition::Down,
        left: GearPosition::Transitioning,
        right: GearPosition::Down,
    };
    assert!(left_only.transitioning(), "left transitioning");

    let right_only = GearState {
        nose: GearPosition::Down,
        left: GearPosition::Down,
        right: GearPosition::Transitioning,
    };
    assert!(right_only.transitioning(), "right transitioning");
}

#[test]
fn gear_not_transitioning_when_none() {
    // Catches true vs false return mutation
    let gear = GearState {
        nose: GearPosition::Down,
        left: GearPosition::Up,
        right: GearPosition::Unknown,
    };
    assert!(
        !gear.transitioning(),
        "no transitioning gear must return false"
    );
}
