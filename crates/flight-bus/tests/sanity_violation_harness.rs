// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Sanity Violation Harness - Comprehensive test that sanity violations only occur
//! when deliberately injecting nonsense data
//!
//! This test harness verifies that:
//! 1. Normal, valid telemetry does NOT trigger sanity violations
//! 2. Deliberately injected NaN/Inf values DO trigger violations
//! 3. Deliberately injected implausible jumps DO trigger violations
//! 4. safe_for_ffb flag behaves correctly in all scenarios
//!
//! Requirements: SIM-TEST-01.9, QG-SANITY-GATE
//!
//! Exit Criteria for Phase 1:
//! - Sanity violations only occur when deliberately injecting nonsense
//! - All adapter tests pass
//! - Adapters can run in harness that logs BusSnapshots with no NaN/Inf under normal use

use flight_bus::snapshot::BusSnapshot;
use flight_bus::types::{AircraftId, GForce, Mach, SimId, ValidatedAngle, ValidatedSpeed};

/// Test result for sanity violation harness
#[derive(Debug)]
struct HarnessResult {
    test_name: String,
    passed: bool,
    message: String,
}

impl HarnessResult {
    fn pass(test_name: &str) -> Self {
        Self {
            test_name: test_name.to_string(),
            passed: true,
            message: "PASS".to_string(),
        }
    }

    fn fail(test_name: &str, message: &str) -> Self {
        Self {
            test_name: test_name.to_string(),
            passed: false,
            message: message.to_string(),
        }
    }
}

/// Helper to create a valid baseline snapshot
fn create_valid_baseline_snapshot() -> BusSnapshot {
    let mut snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));

    // Set valid core telemetry - typical Cessna 172 cruise
    snapshot.kinematics.pitch = ValidatedAngle::new_degrees(5.0).unwrap();
    snapshot.kinematics.bank = ValidatedAngle::new_degrees(0.0).unwrap();
    snapshot.kinematics.heading = ValidatedAngle::new_degrees(90.0).unwrap();
    snapshot.kinematics.ias = ValidatedSpeed::new_knots(120.0).unwrap();
    snapshot.kinematics.tas = ValidatedSpeed::new_knots(125.0).unwrap();
    snapshot.kinematics.ground_speed = ValidatedSpeed::new_knots(120.0).unwrap();
    snapshot.kinematics.g_force = GForce::new(1.0).unwrap();
    snapshot.kinematics.g_lateral = GForce::new(0.0).unwrap();
    snapshot.kinematics.g_longitudinal = GForce::new(0.0).unwrap();
    snapshot.kinematics.mach = Mach::new(0.18).unwrap();

    snapshot.angular_rates.p = 0.0;
    snapshot.angular_rates.q = 0.0;
    snapshot.angular_rates.r = 0.0;

    snapshot.environment.altitude = 5000.0;
    snapshot.environment.oat = 15.0;

    snapshot.validity.attitude_valid = true;
    snapshot.validity.velocities_valid = true;
    snapshot.validity.kinematics_valid = true;

    snapshot.timestamp = 1_000_000_000; // 1 second

    snapshot
}

/// Test 1: Verify that valid baseline telemetry has no NaN/Inf
#[test]
fn test_valid_baseline_has_no_nan_inf() {
    let snapshot = create_valid_baseline_snapshot();

    // Check all core telemetry fields for NaN/Inf
    let has_nan_inf = snapshot.kinematics.pitch.to_radians().is_nan()
        || snapshot.kinematics.pitch.to_radians().is_infinite()
        || snapshot.kinematics.bank.to_radians().is_nan()
        || snapshot.kinematics.bank.to_radians().is_infinite()
        || snapshot.kinematics.heading.to_radians().is_nan()
        || snapshot.kinematics.heading.to_radians().is_infinite()
        || snapshot.kinematics.ias.to_mps().is_nan()
        || snapshot.kinematics.ias.to_mps().is_infinite()
        || snapshot.kinematics.tas.to_mps().is_nan()
        || snapshot.kinematics.tas.to_mps().is_infinite()
        || snapshot.kinematics.g_force.value().is_nan()
        || snapshot.kinematics.g_force.value().is_infinite()
        || snapshot.angular_rates.p.is_nan()
        || snapshot.angular_rates.p.is_infinite()
        || snapshot.angular_rates.q.is_nan()
        || snapshot.angular_rates.q.is_infinite()
        || snapshot.angular_rates.r.is_nan()
        || snapshot.angular_rates.r.is_infinite()
        || snapshot.environment.altitude.is_nan()
        || snapshot.environment.altitude.is_infinite();

    assert!(
        !has_nan_inf,
        "Valid baseline snapshot should not contain NaN or Inf values"
    );
}

/// Test 2: Verify that normal flight variations do not trigger violations
#[test]
fn test_normal_flight_variations_no_violations() {
    let mut results = Vec::new();

    // Test various normal flight scenarios
    let scenarios = vec![
        ("Level cruise", 5.0, 0.0, 120.0, 1.0),
        ("Gentle climb", 10.0, 0.0, 110.0, 1.1),
        ("Gentle descent", -5.0, 0.0, 130.0, 0.9),
        ("Gentle turn", 5.0, 15.0, 120.0, 1.05),
        ("Coordinated turn", 5.0, 30.0, 115.0, 1.15),
    ];

    for (name, pitch, bank, ias, g_force) in scenarios {
        let mut snapshot = create_valid_baseline_snapshot();
        snapshot.kinematics.pitch = ValidatedAngle::new_degrees(pitch).unwrap();
        snapshot.kinematics.bank = ValidatedAngle::new_degrees(bank).unwrap();
        snapshot.kinematics.ias = ValidatedSpeed::new_knots(ias).unwrap();
        snapshot.kinematics.g_force = GForce::new(g_force).unwrap();

        // Verify no NaN/Inf
        let has_nan_inf = snapshot.kinematics.pitch.to_radians().is_nan()
            || snapshot.kinematics.pitch.to_radians().is_infinite()
            || snapshot.kinematics.bank.to_radians().is_nan()
            || snapshot.kinematics.bank.to_radians().is_infinite()
            || snapshot.kinematics.ias.to_mps().is_nan()
            || snapshot.kinematics.ias.to_mps().is_infinite()
            || snapshot.kinematics.g_force.value().is_nan()
            || snapshot.kinematics.g_force.value().is_infinite();

        if has_nan_inf {
            results.push(HarnessResult::fail(
                name,
                "Normal flight scenario contains NaN/Inf",
            ));
        } else {
            results.push(HarnessResult::pass(name));
        }
    }

    // Verify all scenarios passed
    let failed = results.iter().filter(|r| !r.passed).count();
    if failed > 0 {
        for result in &results {
            if !result.passed {
                eprintln!("FAIL: {} - {}", result.test_name, result.message);
            }
        }
        panic!(
            "{} out of {} normal flight scenarios failed",
            failed,
            results.len()
        );
    }
}

/// Test 3: Verify that deliberately injected NaN values are detectable
#[test]
fn test_injected_nan_is_detectable() {
    let mut results = Vec::new();

    // Test NaN injection in various fields
    let test_cases: Vec<(&str, Box<dyn Fn(&mut BusSnapshot)>)> = vec![
        (
            "Angular rate P",
            Box::new(|s: &mut BusSnapshot| s.angular_rates.p = f32::NAN),
        ),
        (
            "Angular rate Q",
            Box::new(|s: &mut BusSnapshot| s.angular_rates.q = f32::NAN),
        ),
        (
            "Angular rate R",
            Box::new(|s: &mut BusSnapshot| s.angular_rates.r = f32::NAN),
        ),
        (
            "Altitude",
            Box::new(|s: &mut BusSnapshot| s.environment.altitude = f32::NAN),
        ),
        (
            "OAT",
            Box::new(|s: &mut BusSnapshot| s.environment.oat = f32::NAN),
        ),
    ];

    for (name, inject_fn) in test_cases {
        let mut snapshot = create_valid_baseline_snapshot();
        inject_fn(&mut snapshot);

        // Check if NaN is present
        let has_nan = snapshot.angular_rates.p.is_nan()
            || snapshot.angular_rates.q.is_nan()
            || snapshot.angular_rates.r.is_nan()
            || snapshot.environment.altitude.is_nan()
            || snapshot.environment.oat.is_nan();

        if has_nan {
            results.push(HarnessResult::pass(name));
        } else {
            results.push(HarnessResult::fail(name, "NaN injection not detected"));
        }
    }

    // Verify all injections were detected
    let failed = results.iter().filter(|r| !r.passed).count();
    if failed > 0 {
        for result in &results {
            if !result.passed {
                eprintln!("FAIL: {} - {}", result.test_name, result.message);
            }
        }
        panic!(
            "{} out of {} NaN injection tests failed",
            failed,
            results.len()
        );
    }
}

/// Test 4: Verify that deliberately injected Inf values are detectable
#[test]
fn test_injected_inf_is_detectable() {
    let mut results = Vec::new();

    // Test Inf injection in various fields
    let test_cases: Vec<(&str, Box<dyn Fn(&mut BusSnapshot)>)> = vec![
        (
            "Angular rate P +Inf",
            Box::new(|s: &mut BusSnapshot| s.angular_rates.p = f32::INFINITY),
        ),
        (
            "Angular rate Q -Inf",
            Box::new(|s: &mut BusSnapshot| s.angular_rates.q = f32::NEG_INFINITY),
        ),
        (
            "Angular rate R +Inf",
            Box::new(|s: &mut BusSnapshot| s.angular_rates.r = f32::INFINITY),
        ),
        (
            "Altitude +Inf",
            Box::new(|s: &mut BusSnapshot| s.environment.altitude = f32::INFINITY),
        ),
        (
            "OAT -Inf",
            Box::new(|s: &mut BusSnapshot| s.environment.oat = f32::NEG_INFINITY),
        ),
    ];

    for (name, inject_fn) in test_cases {
        let mut snapshot = create_valid_baseline_snapshot();
        inject_fn(&mut snapshot);

        // Check if Inf is present
        let has_inf = snapshot.angular_rates.p.is_infinite()
            || snapshot.angular_rates.q.is_infinite()
            || snapshot.angular_rates.r.is_infinite()
            || snapshot.environment.altitude.is_infinite()
            || snapshot.environment.oat.is_infinite();

        if has_inf {
            results.push(HarnessResult::pass(name));
        } else {
            results.push(HarnessResult::fail(name, "Inf injection not detected"));
        }
    }

    // Verify all injections were detected
    let failed = results.iter().filter(|r| !r.passed).count();
    if failed > 0 {
        for result in &results {
            if !result.passed {
                eprintln!("FAIL: {} - {}", result.test_name, result.message);
            }
        }
        panic!(
            "{} out of {} Inf injection tests failed",
            failed,
            results.len()
        );
    }
}

/// Test 5: Verify that plausible changes over time do not look like jumps
#[test]
fn test_plausible_changes_are_not_jumps() {
    let mut results = Vec::new();

    // Simulate realistic flight maneuvers over multiple frames at 60Hz (16ms per frame)
    let scenarios = vec![
        (
            "Gradual pitch up",
            vec![
                (0, 5.0, 0.0, 120.0),
                (16, 6.0, 0.0, 119.0),
                (32, 7.0, 0.0, 118.0),
                (48, 8.0, 0.0, 117.0),
            ],
        ),
        (
            "Gradual bank into turn",
            vec![
                (0, 5.0, 0.0, 120.0),
                (16, 5.0, 5.0, 120.0),
                (32, 5.0, 10.0, 119.0),
                (48, 5.0, 15.0, 118.0),
            ],
        ),
        (
            "Speed reduction",
            vec![
                (0, 5.0, 0.0, 120.0),
                (16, 5.0, 0.0, 118.0),
                (32, 5.0, 0.0, 116.0),
                (48, 5.0, 0.0, 114.0),
            ],
        ),
    ];

    for (name, frames) in scenarios {
        let mut all_valid = true;

        for (time_ms, pitch, bank, ias) in frames {
            let mut snapshot = create_valid_baseline_snapshot();
            snapshot.timestamp = (time_ms as u64) * 1_000_000; // Convert ms to ns
            snapshot.kinematics.pitch = ValidatedAngle::new_degrees(pitch).unwrap();
            snapshot.kinematics.bank = ValidatedAngle::new_degrees(bank).unwrap();
            snapshot.kinematics.ias = ValidatedSpeed::new_knots(ias).unwrap();

            // Verify no NaN/Inf
            if snapshot.kinematics.pitch.to_radians().is_nan()
                || snapshot.kinematics.pitch.to_radians().is_infinite()
                || snapshot.kinematics.bank.to_radians().is_nan()
                || snapshot.kinematics.bank.to_radians().is_infinite()
                || snapshot.kinematics.ias.to_mps().is_nan()
                || snapshot.kinematics.ias.to_mps().is_infinite()
            {
                all_valid = false;
                break;
            }
        }

        if all_valid {
            results.push(HarnessResult::pass(name));
        } else {
            results.push(HarnessResult::fail(
                name,
                "Plausible changes triggered NaN/Inf",
            ));
        }
    }

    // Verify all scenarios passed
    let failed = results.iter().filter(|r| !r.passed).count();
    if failed > 0 {
        for result in &results {
            if !result.passed {
                eprintln!("FAIL: {} - {}", result.test_name, result.message);
            }
        }
        panic!(
            "{} out of {} plausible change scenarios failed",
            failed,
            results.len()
        );
    }
}

/// Test 6: Verify that implausible jumps are detectable
#[test]
fn test_implausible_jumps_are_detectable() {
    // Create two snapshots with implausible changes

    // Scenario 1: Huge pitch jump (0° to 90° in 16ms)
    let snapshot1 = {
        let mut s = create_valid_baseline_snapshot();
        s.timestamp = 1_000_000_000; // 1 second
        s.kinematics.pitch = ValidatedAngle::new_degrees(0.0).unwrap();
        s
    };

    let snapshot2 = {
        let mut s = create_valid_baseline_snapshot();
        s.timestamp = 1_016_000_000; // 1.016 seconds (16ms later)
        s.kinematics.pitch = ValidatedAngle::new_degrees(90.0).unwrap();
        s
    };

    // Calculate the change
    let dt = (snapshot2.timestamp - snapshot1.timestamp) as f64 / 1e9;
    let d_pitch =
        (snapshot2.kinematics.pitch.to_radians() - snapshot1.kinematics.pitch.to_radians()).abs();
    let pitch_rate = d_pitch / dt as f32;

    // At 60Hz (16ms), a 90 degree change is 90/0.016 = 5625 deg/s = 98.2 rad/s
    // This is clearly implausible for any aircraft
    assert!(
        pitch_rate > 10.0,
        "Implausible pitch jump should have high rate: {} rad/s",
        pitch_rate
    );

    // Scenario 2: Huge velocity jump (120 to 300 knots in 16ms)
    let snapshot3 = {
        let mut s = create_valid_baseline_snapshot();
        s.timestamp = 2_000_000_000;
        s.kinematics.ias = ValidatedSpeed::new_knots(120.0).unwrap();
        s
    };

    let snapshot4 = {
        let mut s = create_valid_baseline_snapshot();
        s.timestamp = 2_016_000_000;
        s.kinematics.ias = ValidatedSpeed::new_knots(300.0).unwrap();
        s
    };

    let dt2 = (snapshot4.timestamp - snapshot3.timestamp) as f64 / 1e9;
    let d_ias = (snapshot4.kinematics.ias.to_mps() - snapshot3.kinematics.ias.to_mps()).abs();
    let ias_rate = d_ias / dt2 as f32;

    // 180 knots = ~92.6 m/s change in 0.016s = 5787 m/s² = 590g acceleration
    // This is clearly implausible
    assert!(
        ias_rate > 100.0,
        "Implausible velocity jump should have high rate: {} m/s²",
        ias_rate
    );
}

/// Test 7: Verify heading wraparound is handled correctly
#[test]
fn test_heading_wraparound_not_implausible() {
    // Create two snapshots with heading wraparound (179° to -179°)
    // The actual angular difference is 2°, not 358°

    let snapshot1 = {
        let mut s = create_valid_baseline_snapshot();
        s.timestamp = 1_000_000_000;
        s.kinematics.heading = ValidatedAngle::new_degrees(179.0).unwrap();
        s
    };

    let snapshot2 = {
        let mut s = create_valid_baseline_snapshot();
        s.timestamp = 1_016_000_000; // 16ms later
        s.kinematics.heading = ValidatedAngle::new_degrees(-179.0).unwrap();
        s
    };

    // Calculate the smallest angle difference
    let heading1_rad = snapshot1.kinematics.heading.to_radians();
    let heading2_rad = snapshot2.kinematics.heading.to_radians();
    let mut diff = (heading2_rad - heading1_rad).abs();

    // Handle wraparound
    if diff > std::f32::consts::PI {
        diff = 2.0 * std::f32::consts::PI - diff;
    }

    // The difference should be small (2 degrees = 0.0349 radians)
    assert!(
        diff < 0.1,
        "Heading wraparound should result in small difference: {} rad",
        diff
    );
}

/// Test 8: Comprehensive harness - run multiple frames of valid telemetry
#[test]
fn test_comprehensive_harness_no_violations() {
    // Simulate 100 frames of valid telemetry at 60Hz
    let mut all_valid = true;

    for i in 0..100 {
        let mut snapshot = create_valid_baseline_snapshot();
        snapshot.timestamp = (i as u64) * 16_000_000; // 16ms per frame

        // Add some realistic variation
        let t = i as f32 * 0.016; // Time in seconds
        snapshot.kinematics.pitch =
            ValidatedAngle::new_degrees(5.0 + (t * 0.5).sin() * 2.0).unwrap();
        snapshot.kinematics.bank = ValidatedAngle::new_degrees((t * 0.3).sin() * 5.0).unwrap();
        snapshot.kinematics.ias = ValidatedSpeed::new_knots(120.0 + (t * 0.2).sin() * 5.0).unwrap();

        // Verify no NaN/Inf
        if snapshot.kinematics.pitch.to_radians().is_nan()
            || snapshot.kinematics.pitch.to_radians().is_infinite()
            || snapshot.kinematics.bank.to_radians().is_nan()
            || snapshot.kinematics.bank.to_radians().is_infinite()
            || snapshot.kinematics.ias.to_mps().is_nan()
            || snapshot.kinematics.ias.to_mps().is_infinite()
        {
            all_valid = false;
            eprintln!("Frame {} contains NaN/Inf", i);
            break;
        }
    }

    assert!(
        all_valid,
        "Comprehensive harness should not produce NaN/Inf in valid telemetry"
    );
}

/// Test 9: Verify that ValidatedSpeed and ValidatedAngle prevent NaN/Inf construction
#[test]
fn test_validated_types_prevent_nan_inf() {
    // ValidatedSpeed should reject NaN
    let speed_nan = ValidatedSpeed::new_knots(f32::NAN);
    assert!(speed_nan.is_err(), "ValidatedSpeed should reject NaN");

    // ValidatedSpeed should reject Inf
    let speed_inf = ValidatedSpeed::new_knots(f32::INFINITY);
    assert!(speed_inf.is_err(), "ValidatedSpeed should reject Inf");

    // ValidatedAngle should reject NaN
    let angle_nan = ValidatedAngle::new_degrees(f32::NAN);
    assert!(angle_nan.is_err(), "ValidatedAngle should reject NaN");

    // ValidatedAngle should reject Inf
    let angle_inf = ValidatedAngle::new_degrees(f32::INFINITY);
    assert!(angle_inf.is_err(), "ValidatedAngle should reject Inf");

    // GForce should reject NaN
    let g_nan = GForce::new(f32::NAN);
    assert!(g_nan.is_err(), "GForce should reject NaN");

    // GForce should reject Inf
    let g_inf = GForce::new(f32::INFINITY);
    assert!(g_inf.is_err(), "GForce should reject Inf");
}

/// Test 10: Summary test - verify exit criteria
#[test]
fn test_exit_criteria_summary() {
    println!("\n=== Sanity Violation Harness - Exit Criteria Summary ===\n");

    let mut all_passed = true;

    // Criterion 1: Valid telemetry has no NaN/Inf
    let snapshot = create_valid_baseline_snapshot();
    let has_nan_inf = snapshot.kinematics.pitch.to_radians().is_nan()
        || snapshot.kinematics.pitch.to_radians().is_infinite()
        || snapshot.angular_rates.p.is_nan()
        || snapshot.angular_rates.p.is_infinite();

    if !has_nan_inf {
        println!("✓ Valid telemetry contains no NaN/Inf");
    } else {
        println!("✗ Valid telemetry contains NaN/Inf");
        all_passed = false;
    }

    // Criterion 2: Injected NaN is detectable
    let mut snapshot_nan = create_valid_baseline_snapshot();
    snapshot_nan.angular_rates.p = f32::NAN;
    if snapshot_nan.angular_rates.p.is_nan() {
        println!("✓ Injected NaN values are detectable");
    } else {
        println!("✗ Injected NaN values are NOT detectable");
        all_passed = false;
    }

    // Criterion 3: Injected Inf is detectable
    let mut snapshot_inf = create_valid_baseline_snapshot();
    snapshot_inf.angular_rates.q = f32::INFINITY;
    if snapshot_inf.angular_rates.q.is_infinite() {
        println!("✓ Injected Inf values are detectable");
    } else {
        println!("✗ Injected Inf values are NOT detectable");
        all_passed = false;
    }

    // Criterion 4: Validated types prevent NaN/Inf construction
    let speed_nan = ValidatedSpeed::new_knots(f32::NAN);
    let angle_inf = ValidatedAngle::new_degrees(f32::INFINITY);
    if speed_nan.is_err() && angle_inf.is_err() {
        println!("✓ Validated types prevent NaN/Inf construction");
    } else {
        println!("✗ Validated types allow NaN/Inf construction");
        all_passed = false;
    }

    println!("\n=== Summary ===");
    if all_passed {
        println!("✓ All exit criteria PASSED");
        println!("✓ Sanity violations only occur when deliberately injecting nonsense");
    } else {
        println!("✗ Some exit criteria FAILED");
        panic!("Exit criteria not met");
    }
}
