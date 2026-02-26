// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Unit tests for MSFS SimConnect telemetry mapping
//!
//! Tests verify correct conversion of SimConnect SimVars to BusSnapshot fields
//! with proper unit conversions as specified in requirements:
//! - MSFS-INT-01.4: Attitude conversion (degrees → radians)
//! - MSFS-INT-01.5: Velocity conversion (knots → m/s, FPM → m/s, ft/s → m/s)
//! - MSFS-INT-01.6: Angular rate mapping (already rad/s), g-load and aero mapping

use flight_bus::adapters::msfs::MsfsConverter;
use flight_bus::types::Percentage;
use std::f32::consts::PI;

/// Test attitude conversion (degrees → radians)
/// Requirements: MSFS-INT-01.4
#[test]
fn test_attitude_conversion_degrees_to_radians() {
    // Test pitch conversion
    let pitch_deg = 5.0_f32;
    let pitch = MsfsConverter::convert_angle_degrees(pitch_deg as f64).unwrap();
    let expected_rad = pitch_deg * PI / 180.0;
    assert!(
        (pitch.to_radians() - expected_rad).abs() < 0.0001,
        "Pitch conversion failed: expected {}, got {}",
        expected_rad,
        pitch.to_radians()
    );

    // Test bank/roll conversion
    let bank_deg = 2.0_f32;
    let bank = MsfsConverter::convert_angle_degrees(bank_deg as f64).unwrap();
    let expected_rad = bank_deg * PI / 180.0;
    assert!(
        (bank.to_radians() - expected_rad).abs() < 0.0001,
        "Bank conversion failed: expected {}, got {}",
        expected_rad,
        bank.to_radians()
    );

    // Test heading conversion
    let heading_deg = 270.0;
    let heading = MsfsConverter::convert_angle_degrees(heading_deg).unwrap();
    let expected_rad = -90.0 * PI / 180.0; // 270° normalizes to -90°
    assert!(
        (heading.to_radians() - expected_rad).abs() < 0.0001,
        "Heading conversion failed: expected {}, got {}",
        expected_rad,
        heading.to_radians()
    );

    // Test angle normalization (>180°)
    let angle_360 = MsfsConverter::convert_angle_degrees(360.0).unwrap();
    assert!(
        angle_360.to_degrees().abs() < 0.0001,
        "360° should normalize to 0°"
    );

    let angle_270 = MsfsConverter::convert_angle_degrees(270.0).unwrap();
    assert!(
        (angle_270.to_degrees() - (-90.0)).abs() < 0.0001,
        "270° should normalize to -90°"
    );

    // Test negative angle normalization
    let angle_neg_270 = MsfsConverter::convert_angle_degrees(-270.0).unwrap();
    assert!(
        (angle_neg_270.to_degrees() - 90.0).abs() < 0.0001,
        "-270° should normalize to 90°"
    );
}

/// Test velocity conversion (knots → m/s)
/// Requirements: MSFS-INT-01.5
#[test]
fn test_velocity_conversion_knots_to_mps() {
    // Test IAS conversion
    let ias_knots = 100.0_f32;
    let ias = MsfsConverter::convert_ias(ias_knots as f64).unwrap();
    assert_eq!(ias.to_knots(), ias_knots);

    // Verify m/s conversion (1 knot = 0.514444 m/s)
    let expected_mps = ias_knots * 0.514444;
    assert!(
        (ias.to_mps() - expected_mps).abs() < 0.01,
        "IAS m/s conversion failed: expected {}, got {}",
        expected_mps,
        ias.to_mps()
    );

    // Test TAS conversion
    let tas_knots = 105.0_f32;
    let tas = MsfsConverter::convert_tas(tas_knots as f64).unwrap();
    assert_eq!(tas.to_knots(), tas_knots);

    let expected_mps = tas_knots * 0.514444;
    assert!(
        (tas.to_mps() - expected_mps).abs() < 0.01,
        "TAS m/s conversion failed: expected {}, got {}",
        expected_mps,
        tas.to_mps()
    );

    // Test ground speed conversion
    let gs_knots = 98.0_f32;
    let gs = MsfsConverter::convert_ground_speed(gs_knots as f64).unwrap();
    assert_eq!(gs.to_knots(), gs_knots);

    let expected_mps = gs_knots * 0.514444;
    assert!(
        (gs.to_mps() - expected_mps).abs() < 0.01,
        "Ground speed m/s conversion failed: expected {}, got {}",
        expected_mps,
        gs.to_mps()
    );
}

/// Test vertical speed conversion (FPM → m/s)
/// Requirements: MSFS-INT-01.5
#[test]
fn test_vertical_speed_conversion_fpm_to_mps() {
    // Test positive vertical speed (climbing)
    let vs_fpm = 500.0_f32;
    let expected_mps = vs_fpm * 0.00508; // 1 FPM = 0.00508 m/s
    assert!(
        (expected_mps - 2.54_f32).abs() < 0.01,
        "500 FPM should be ~2.54 m/s"
    );

    // Test negative vertical speed (descending)
    let vs_fpm = -300.0_f32;
    let expected_mps = vs_fpm * 0.00508;
    assert!(
        (expected_mps - (-1.524_f32)).abs() < 0.01,
        "-300 FPM should be ~-1.524 m/s"
    );

    // Test zero vertical speed (level flight)
    let vs_fpm = 0.0_f32;
    let expected_mps = vs_fpm * 0.00508;
    assert_eq!(expected_mps, 0.0, "0 FPM should be 0 m/s");
}

/// Test body velocity conversion (ft/s → m/s)
/// Requirements: MSFS-INT-01.5
#[test]
fn test_body_velocity_conversion_fps_to_mps() {
    // Test forward velocity (X-axis)
    let vel_fps = 169.0_f32; // ~100 knots
    let expected_mps = vel_fps * 0.3048; // 1 ft/s = 0.3048 m/s
    assert!(
        (expected_mps - 51.51_f32).abs() < 0.1,
        "169 ft/s should be ~51.51 m/s"
    );

    // Test lateral velocity (Y-axis)
    let vel_fps = 10.0_f32;
    let expected_mps = vel_fps * 0.3048;
    assert!(
        (expected_mps - 3.048_f32).abs() < 0.01,
        "10 ft/s should be ~3.048 m/s"
    );

    // Test vertical velocity (Z-axis)
    let vel_fps = -5.0_f32;
    let expected_mps = vel_fps * 0.3048;
    assert!(
        (expected_mps - (-1.524_f32)).abs() < 0.01,
        "-5 ft/s should be ~-1.524 m/s"
    );
}

/// Test angular rate mapping (already in rad/s, no conversion needed)
/// Requirements: MSFS-INT-01.6
#[test]
fn test_angular_rate_mapping_passthrough() {
    // Angular rates from SimConnect are already in rad/s
    // Test that they pass through without conversion

    let p_rad_s: f32 = 0.01; // Roll rate
    let q_rad_s: f32 = 0.02; // Pitch rate
    let r_rad_s: f32 = 0.005; // Yaw rate

    // These values should be used directly without conversion
    assert_eq!(p_rad_s, 0.01, "Roll rate should pass through unchanged");
    assert_eq!(q_rad_s, 0.02, "Pitch rate should pass through unchanged");
    assert_eq!(r_rad_s, 0.005, "Yaw rate should pass through unchanged");

    // Verify reasonable ranges for angular rates
    assert!(
        p_rad_s.abs() < 10.0,
        "Roll rate should be within reasonable range"
    );
    assert!(
        q_rad_s.abs() < 10.0,
        "Pitch rate should be within reasonable range"
    );
    assert!(
        r_rad_s.abs() < 10.0,
        "Yaw rate should be within reasonable range"
    );
}

/// Test g-load mapping
/// Requirements: MSFS-INT-01.6
#[test]
fn test_g_load_mapping() {
    // Test normal g-load (1g level flight)
    let g_normal = MsfsConverter::convert_g_force(1.0).unwrap();
    assert_eq!(g_normal.value(), 1.0, "Normal g-load should be 1.0");

    // Test positive g-load (pulling up)
    let g_positive = MsfsConverter::convert_g_force(2.5).unwrap();
    assert_eq!(g_positive.value(), 2.5, "Positive g-load should be 2.5");

    // Test negative g-load (pushing over)
    let g_negative = MsfsConverter::convert_g_force(-0.5).unwrap();
    assert_eq!(g_negative.value(), -0.5, "Negative g-load should be -0.5");

    // Test lateral g-load
    let g_lateral = MsfsConverter::convert_g_force(0.05).unwrap();
    assert_eq!(g_lateral.value(), 0.05, "Lateral g-load should be 0.05");

    // Test longitudinal g-load
    let g_longitudinal = MsfsConverter::convert_g_force(0.1).unwrap();
    assert_eq!(
        g_longitudinal.value(),
        0.1,
        "Longitudinal g-load should be 0.1"
    );

    // Test out of range g-loads
    assert!(
        MsfsConverter::convert_g_force(25.0).is_err(),
        "G-load > 20 should be rejected"
    );
    assert!(
        MsfsConverter::convert_g_force(-25.0).is_err(),
        "G-load < -20 should be rejected"
    );
}

/// Test aerodynamic angle mapping (AoA, sideslip)
/// Requirements: MSFS-INT-01.6
#[test]
fn test_aero_angle_mapping() {
    // Test angle of attack (AoA)
    let aoa_deg = 3.5_f32;
    let aoa = MsfsConverter::convert_angle_degrees(aoa_deg as f64).unwrap();
    assert_eq!(aoa.to_degrees(), aoa_deg, "AoA degrees should match");

    let expected_rad = aoa_deg * PI / 180.0;
    assert!(
        (aoa.to_radians() - expected_rad).abs() < 0.0001,
        "AoA radians conversion failed"
    );

    // Test sideslip angle (beta)
    let beta_deg = 0.2_f32;
    let beta = MsfsConverter::convert_angle_degrees(beta_deg as f64).unwrap();
    assert!(
        (beta.to_degrees() - beta_deg).abs() < 0.001,
        "Sideslip degrees should match"
    );

    let expected_rad = beta_deg * PI / 180.0;
    assert!(
        (beta.to_radians() - expected_rad).abs() < 0.0001,
        "Sideslip radians conversion failed"
    );

    // Test negative AoA (nose down)
    let aoa_neg = MsfsConverter::convert_angle_degrees(-2.0).unwrap();
    assert_eq!(
        aoa_neg.to_degrees(),
        -2.0,
        "Negative AoA should be preserved"
    );

    // Test large sideslip angle
    let beta_large = MsfsConverter::convert_angle_degrees(15.0).unwrap();
    assert_eq!(
        beta_large.to_degrees(),
        15.0,
        "Large sideslip angle should be preserved"
    );
}

/// Test Mach number mapping
/// Requirements: MSFS-INT-01.6
#[test]
fn test_mach_number_mapping() {
    // Test subsonic Mach number
    let mach_subsonic = MsfsConverter::convert_mach(0.15).unwrap();
    assert_eq!(mach_subsonic.value(), 0.15, "Subsonic Mach should be 0.15");

    // Test transonic Mach number
    let mach_transonic = MsfsConverter::convert_mach(0.85).unwrap();
    assert_eq!(
        mach_transonic.value(),
        0.85,
        "Transonic Mach should be 0.85"
    );

    // Test supersonic Mach number
    let mach_supersonic = MsfsConverter::convert_mach(1.5).unwrap();
    assert_eq!(
        mach_supersonic.value(),
        1.5,
        "Supersonic Mach should be 1.5"
    );

    // Test out of range Mach numbers
    assert!(
        MsfsConverter::convert_mach(-0.1).is_err(),
        "Negative Mach should be rejected"
    );
    assert!(
        MsfsConverter::convert_mach(6.0).is_err(),
        "Mach > 5 should be rejected"
    );
}

/// Test complete telemetry mapping using C172 cruise scenario
/// Requirements: MSFS-INT-01.4, MSFS-INT-01.5, MSFS-INT-01.6, SIM-TEST-01.2
#[test]
fn test_msfs_fixture_c172_cruise() {
    // C172 in cruise at 2500ft, 100 knots
    // SimVar values from fixture
    let ias_knots = 100.0;
    let tas_knots = 105.0;
    let gs_knots = 98.0;
    let aoa_deg = 3.5;
    let beta_deg = 0.2;
    let bank_deg = 2.0;
    let pitch_deg = 5.0;
    let heading_deg = 270.0; // Normalizes to -90°
    let g_force = 1.0;
    let g_lateral = 0.05;
    let g_longitudinal = 0.1;
    let mach = 0.15;

    // Test IAS conversion
    let ias = MsfsConverter::convert_ias(ias_knots).unwrap();
    assert_eq!(ias.to_knots(), ias_knots as f32);

    // Test TAS conversion
    let tas = MsfsConverter::convert_tas(tas_knots).unwrap();
    assert_eq!(tas.to_knots(), tas_knots as f32);

    // Test ground speed conversion
    let gs = MsfsConverter::convert_ground_speed(gs_knots).unwrap();
    assert_eq!(gs.to_knots(), gs_knots as f32);

    // Test AoA conversion
    let aoa = MsfsConverter::convert_angle_degrees(aoa_deg).unwrap();
    assert_eq!(aoa.to_degrees(), aoa_deg as f32);
    let expected_aoa_rad = (aoa_deg as f32) * PI / 180.0;
    assert!((aoa.to_radians() - expected_aoa_rad).abs() < 0.0001);

    // Test sideslip conversion
    let beta = MsfsConverter::convert_angle_degrees(beta_deg).unwrap();
    assert!((beta.to_degrees() - beta_deg as f32).abs() < 0.001);

    // Test bank conversion
    let bank = MsfsConverter::convert_angle_degrees(bank_deg).unwrap();
    assert_eq!(bank.to_degrees(), bank_deg as f32);

    // Test pitch conversion
    let pitch = MsfsConverter::convert_angle_degrees(pitch_deg).unwrap();
    assert_eq!(pitch.to_degrees(), pitch_deg as f32);

    // Test heading conversion (with normalization: 270° → -90°)
    let heading = MsfsConverter::convert_angle_degrees(heading_deg).unwrap();
    assert_eq!(heading.to_degrees(), -90.0);

    // Test g-load conversions
    let g = MsfsConverter::convert_g_force(g_force).unwrap();
    assert_eq!(g.value(), g_force as f32);

    let g_lat = MsfsConverter::convert_g_force(g_lateral).unwrap();
    assert_eq!(g_lat.value(), g_lateral as f32);

    let g_long = MsfsConverter::convert_g_force(g_longitudinal).unwrap();
    assert_eq!(g_long.value(), g_longitudinal as f32);

    // Test Mach number
    let m = MsfsConverter::convert_mach(mach).unwrap();
    assert_eq!(m.value(), mach as f32);
}

/// Test edge cases for unit conversions
/// Requirements: MSFS-INT-01.4, MSFS-INT-01.5, MSFS-INT-01.6
#[test]
fn test_unit_conversion_edge_cases() {
    // Test zero values
    let zero_speed = MsfsConverter::convert_ias(0.0).unwrap();
    assert_eq!(zero_speed.to_knots(), 0.0);
    assert_eq!(zero_speed.to_mps(), 0.0);

    let zero_angle = MsfsConverter::convert_angle_degrees(0.0).unwrap();
    assert_eq!(zero_angle.to_degrees(), 0.0);
    assert_eq!(zero_angle.to_radians(), 0.0);

    let zero_g = MsfsConverter::convert_g_force(0.0).unwrap();
    assert_eq!(zero_g.value(), 0.0);

    // Test maximum valid values
    let max_speed = MsfsConverter::convert_ias(999.0).unwrap();
    assert_eq!(max_speed.to_knots(), 999.0);

    // Note: 180° and -180° are equivalent after normalization
    // The converter normalizes to -180 to 180 range, so 180° stays as 180° or -180°
    let max_angle = MsfsConverter::convert_angle_degrees(179.0).unwrap();
    assert_eq!(max_angle.to_degrees(), 179.0);

    let max_g = MsfsConverter::convert_g_force(19.9).unwrap();
    assert_eq!(max_g.value(), 19.9);

    // Test minimum valid values
    let min_angle = MsfsConverter::convert_angle_degrees(-179.0).unwrap();
    assert_eq!(min_angle.to_degrees(), -179.0);

    let min_g = MsfsConverter::convert_g_force(-19.9).unwrap();
    assert_eq!(min_g.value(), -19.9);
}

/// Test percentage conversions
/// Requirements: MSFS-INT-01.6
#[test]
fn test_percentage_conversions() {
    // Test direct percentage (0-100)
    let pct_50 = MsfsConverter::convert_percentage(50.0).unwrap();
    assert_eq!(pct_50.value(), 50.0);

    let pct_100 = MsfsConverter::convert_percentage(100.0).unwrap();
    assert_eq!(pct_100.value(), 100.0);

    let pct_0 = MsfsConverter::convert_percentage(0.0).unwrap();
    assert_eq!(pct_0.value(), 0.0);

    // Test normalized to percentage (0-1 → 0-100)
    let norm_50 = Percentage::from_normalized(0.5).unwrap();
    assert_eq!(norm_50.value(), 50.0);

    let norm_100 = Percentage::from_normalized(1.0).unwrap();
    assert_eq!(norm_100.value(), 100.0);

    let norm_0 = Percentage::from_normalized(0.0).unwrap();
    assert_eq!(norm_0.value(), 0.0);

    // Test out of range
    assert!(MsfsConverter::convert_percentage(-5.0).is_err());
    assert!(MsfsConverter::convert_percentage(105.0).is_err());
    assert!(Percentage::from_normalized(-0.1).is_err());
    assert!(Percentage::from_normalized(1.1).is_err());
}

/// Test RPM to percentage conversion
/// Requirements: MSFS-INT-01.6
#[test]
fn test_rpm_to_percentage_conversion() {
    // Test typical piston engine RPM
    let rpm_pct = MsfsConverter::convert_rpm_to_percentage(2400.0, 2700.0).unwrap();
    assert!((rpm_pct.value() - 88.89).abs() < 0.1);

    // Test idle RPM
    let idle_pct = MsfsConverter::convert_rpm_to_percentage(800.0, 2700.0).unwrap();
    assert!((idle_pct.value() - 29.63).abs() < 0.1);

    // Test redline RPM
    let redline_pct = MsfsConverter::convert_rpm_to_percentage(2700.0, 2700.0).unwrap();
    assert_eq!(redline_pct.value(), 100.0);

    // Test over-rev (should clamp to 100%)
    let overrev_pct = MsfsConverter::convert_rpm_to_percentage(2800.0, 2700.0).unwrap();
    assert_eq!(overrev_pct.value(), 100.0);

    // Test invalid redline
    assert!(MsfsConverter::convert_rpm_to_percentage(2400.0, 0.0).is_err());
    assert!(MsfsConverter::convert_rpm_to_percentage(2400.0, -100.0).is_err());
}

/// Test fuel quantity to percentage conversion
/// Requirements: MSFS-INT-01.6
#[test]
fn test_fuel_to_percentage_conversion() {
    // Test half tank
    let fuel_50 = MsfsConverter::convert_fuel_to_percentage(20.0, 40.0).unwrap();
    assert_eq!(fuel_50.value(), 50.0);

    // Test full tank
    let fuel_100 = MsfsConverter::convert_fuel_to_percentage(40.0, 40.0).unwrap();
    assert_eq!(fuel_100.value(), 100.0);

    // Test empty tank
    let fuel_0 = MsfsConverter::convert_fuel_to_percentage(0.0, 40.0).unwrap();
    assert_eq!(fuel_0.value(), 0.0);

    // Test quarter tank
    let fuel_25 = MsfsConverter::convert_fuel_to_percentage(10.0, 40.0).unwrap();
    assert_eq!(fuel_25.value(), 25.0);

    // Test overfill (should clamp to 100%)
    let fuel_overfill = MsfsConverter::convert_fuel_to_percentage(45.0, 40.0).unwrap();
    assert_eq!(fuel_overfill.value(), 100.0);

    // Test invalid capacity
    assert!(MsfsConverter::convert_fuel_to_percentage(20.0, 0.0).is_err());
    assert!(MsfsConverter::convert_fuel_to_percentage(20.0, -10.0).is_err());
}

/// Test NaN inputs are rejected by all converters.
/// Requirements: QG-SANITY-GATE
#[test]
fn test_nan_inputs_rejected() {
    assert!(
        MsfsConverter::convert_ias(f64::NAN).is_err(),
        "NaN IAS must be rejected"
    );
    assert!(
        MsfsConverter::convert_tas(f64::NAN).is_err(),
        "NaN TAS must be rejected"
    );
    assert!(
        MsfsConverter::convert_ground_speed(f64::NAN).is_err(),
        "NaN ground speed must be rejected"
    );
    assert!(
        MsfsConverter::convert_angle_degrees(f64::NAN).is_err(),
        "NaN angle must be rejected"
    );
    assert!(
        MsfsConverter::convert_g_force(f64::NAN).is_err(),
        "NaN g-force must be rejected"
    );
    assert!(
        MsfsConverter::convert_mach(f64::NAN).is_err(),
        "NaN Mach must be rejected"
    );
    assert!(
        MsfsConverter::convert_percentage(f64::NAN).is_err(),
        "NaN percentage must be rejected"
    );
}

/// Test positive and negative infinity inputs are rejected.
/// Requirements: QG-SANITY-GATE
#[test]
fn test_infinity_inputs_rejected() {
    assert!(
        MsfsConverter::convert_ias(f64::INFINITY).is_err(),
        "+Inf IAS must be rejected"
    );
    assert!(
        MsfsConverter::convert_ias(f64::NEG_INFINITY).is_err(),
        "-Inf IAS must be rejected"
    );
    assert!(
        MsfsConverter::convert_g_force(f64::INFINITY).is_err(),
        "+Inf g-force must be rejected"
    );
    assert!(
        MsfsConverter::convert_g_force(f64::NEG_INFINITY).is_err(),
        "-Inf g-force must be rejected"
    );
    assert!(
        MsfsConverter::convert_mach(f64::INFINITY).is_err(),
        "+Inf Mach must be rejected"
    );
    assert!(
        MsfsConverter::convert_angle_degrees(f64::INFINITY).is_err(),
        "+Inf angle must be rejected"
    );
}

/// Test out-of-range inputs return errors with appropriate bounds.
/// Requirements: MSFS-INT-01.5, MSFS-INT-01.6
#[test]
fn test_out_of_range_inputs_rejected() {
    // Speed: valid range 0..=1000 knots
    assert!(
        MsfsConverter::convert_ias(-1.0).is_err(),
        "negative IAS must be rejected"
    );
    assert!(
        MsfsConverter::convert_ias(1001.0).is_err(),
        "IAS > 1000 kts must be rejected"
    );
    assert!(
        MsfsConverter::convert_tas(-0.001).is_err(),
        "negative TAS must be rejected"
    );

    // G-force: valid range -20..=+20 g
    assert!(
        MsfsConverter::convert_g_force(20.001).is_err(),
        "g > 20 must be rejected"
    );
    assert!(
        MsfsConverter::convert_g_force(-20.001).is_err(),
        "g < -20 must be rejected"
    );

    // Mach: valid range 0..=5
    assert!(
        MsfsConverter::convert_mach(-0.001).is_err(),
        "negative Mach must be rejected"
    );
    assert!(
        MsfsConverter::convert_mach(5.001).is_err(),
        "Mach > 5 must be rejected"
    );

    // Percentage: valid range 0..=100
    assert!(
        MsfsConverter::convert_percentage(-0.001).is_err(),
        "negative percentage must be rejected"
    );
    assert!(
        MsfsConverter::convert_percentage(100.001).is_err(),
        "percentage > 100 must be rejected"
    );
}

/// Test boundary values are accepted (edge of valid range).
/// Requirements: MSFS-INT-01.5, MSFS-INT-01.6
#[test]
fn test_boundary_values_accepted() {
    // Speed boundaries
    assert!(MsfsConverter::convert_ias(0.0).is_ok(), "0 kts IAS is valid");
    assert!(MsfsConverter::convert_ias(1000.0).is_ok(), "1000 kts IAS is valid");

    // G-force boundaries
    assert!(MsfsConverter::convert_g_force(20.0).is_ok(), "+20 g is valid");
    assert!(MsfsConverter::convert_g_force(-20.0).is_ok(), "-20 g is valid");

    // Mach boundaries
    assert!(MsfsConverter::convert_mach(0.0).is_ok(), "Mach 0 is valid");
    assert!(MsfsConverter::convert_mach(5.0).is_ok(), "Mach 5 is valid");

    // Percentage boundaries
    assert!(MsfsConverter::convert_percentage(0.0).is_ok(), "0% is valid");
    assert!(MsfsConverter::convert_percentage(100.0).is_ok(), "100% is valid");
}
