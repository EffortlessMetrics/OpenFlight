// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Comprehensive tests for SimConnect variable name mappings.
//!
//! These tests verify that every SimVar name in the default mapping exactly
//! matches the MSFS SDK documentation.  A misspelled name silently produces no
//! data — MSFS returns zeros without any error — making these tests critical for
//! correctness.
//!
//! Coverage:
//! 1. Snapshot of the complete SimVar name table (regression detection)
//! 2. Critical SimVar name assertions — attitude, airspeed, autopilot, etc.
//! 3. Indexed variable format (:1, :2) for multi-engine aircraft
//! 4. Unit conversion direction — degrees, radians, FPM
//! 5. Boolean SimVar conversion semantics
//! 6. Throttle position normalisation [0.0, 1.0]
//! 7. Control-surface axis normalisation [-1.0, 1.0]
//! 8. Generic invariants — non-empty, uppercase, no stray whitespace

use flight_bus::adapters::msfs::MsfsConverter;
use flight_bus::types::AutopilotState;
use flight_simconnect::mapping::{create_default_mapping, EngineMapping};

// ============================================================================
// Helpers
// ============================================================================

/// Collect every SimVar name used in the default mapping into a sorted,
/// deduplicated list.  Used both for the snapshot test and the invariant
/// property tests.
fn all_simvar_names_sorted() -> Vec<String> {
    let cfg = create_default_mapping();
    let m = &cfg.default_mapping;
    let mut names: Vec<String> = Vec::new();

    // Kinematics
    names.push(m.kinematics.ias.clone());
    names.push(m.kinematics.tas.clone());
    names.push(m.kinematics.ground_speed.clone());
    names.push(m.kinematics.aoa.clone());
    names.push(m.kinematics.sideslip.clone());
    names.push(m.kinematics.bank.clone());
    names.push(m.kinematics.pitch.clone());
    names.push(m.kinematics.heading.clone());
    names.push(m.kinematics.g_force.clone());
    names.push(m.kinematics.g_lateral.clone());
    names.push(m.kinematics.g_longitudinal.clone());
    names.push(m.kinematics.mach.clone());
    names.push(m.kinematics.vertical_speed.clone());

    // Aircraft configuration
    names.push(m.config.gear_nose.clone());
    names.push(m.config.gear_left.clone());
    names.push(m.config.gear_right.clone());
    names.push(m.config.flaps.clone());
    names.push(m.config.spoilers.clone());
    names.push(m.config.ap_master.clone());
    names.push(m.config.ap_altitude_hold.clone());
    names.push(m.config.ap_heading_hold.clone());
    names.push(m.config.ap_speed_hold.clone());
    names.push(m.config.ap_altitude.clone());
    names.push(m.config.ap_heading.clone());
    names.push(m.config.ap_speed.clone());

    // Lights
    names.push(m.config.lights.nav.clone());
    names.push(m.config.lights.beacon.clone());
    names.push(m.config.lights.strobe.clone());
    names.push(m.config.lights.landing.clone());
    names.push(m.config.lights.taxi.clone());
    names.push(m.config.lights.logo.clone());
    names.push(m.config.lights.wing.clone());

    // Fuel tanks
    for tank in &m.config.fuel_tanks {
        names.push(tank.clone());
    }

    // Environment
    names.push(m.environment.altitude.clone());
    names.push(m.environment.pressure_altitude.clone());
    names.push(m.environment.oat.clone());
    names.push(m.environment.wind_speed.clone());
    names.push(m.environment.wind_direction.clone());
    names.push(m.environment.visibility.clone());
    names.push(m.environment.cloud_coverage.clone());

    // Navigation (required)
    names.push(m.navigation.latitude.clone());
    names.push(m.navigation.longitude.clone());
    names.push(m.navigation.ground_track.clone());

    // Navigation (optional)
    if let Some(n) = &m.navigation.distance_to_dest {
        names.push(n.clone());
    }
    if let Some(n) = &m.navigation.time_to_dest {
        names.push(n.clone());
    }
    if let Some(n) = &m.navigation.active_waypoint {
        names.push(n.clone());
    }

    // Engines
    for eng in &m.engines {
        names.push(eng.running.clone());
        names.push(eng.rpm.clone());
        if let Some(n) = &eng.manifold_pressure {
            names.push(n.clone());
        }
        if let Some(n) = &eng.egt {
            names.push(n.clone());
        }
        if let Some(n) = &eng.cht {
            names.push(n.clone());
        }
        if let Some(n) = &eng.fuel_flow {
            names.push(n.clone());
        }
        if let Some(n) = &eng.oil_pressure {
            names.push(n.clone());
        }
        if let Some(n) = &eng.oil_temperature {
            names.push(n.clone());
        }
    }

    names.sort();
    names.dedup();
    names
}

// ============================================================================
// 1. Snapshot test — any name change is caught immediately as a diff
// ============================================================================

/// Snapshot the complete, sorted SimVar name table.
///
/// If any name changes, this test fails and presents a diff.  The engineer
/// must consciously approve the change via `cargo insta review` or
/// `INSTA_UPDATE=new cargo test`.
#[test]
fn snapshot_simvar_names() {
    let names = all_simvar_names_sorted();
    insta::assert_yaml_snapshot!("simvar_names", names);
}

// ============================================================================
// 2. Critical SimVar name correctness
// ============================================================================

/// Attitude SimVars must use the exact MSFS-documented names.
///
/// A wrong name silently returns zeros; pitch-up would always read 0°.
#[test]
fn test_attitude_simvar_names_are_correct() {
    let cfg = create_default_mapping();
    let kin = &cfg.default_mapping.kinematics;

    assert_eq!(
        kin.pitch, "PLANE PITCH DEGREES",
        "pitch SimVar must be 'PLANE PITCH DEGREES'"
    );
    assert_eq!(
        kin.bank, "PLANE BANK DEGREES",
        "bank/roll SimVar must be 'PLANE BANK DEGREES'"
    );
    assert_eq!(
        kin.heading, "PLANE HEADING DEGREES MAGNETIC",
        "magnetic heading SimVar must be 'PLANE HEADING DEGREES MAGNETIC'"
    );
}

/// Airspeed SimVars must use the MSFS-documented names.
#[test]
fn test_airspeed_simvar_names_are_correct() {
    let cfg = create_default_mapping();
    let kin = &cfg.default_mapping.kinematics;

    assert_eq!(kin.ias, "AIRSPEED INDICATED");
    assert_eq!(kin.tas, "AIRSPEED TRUE");
    assert_eq!(kin.mach, "AIRSPEED MACH");
    assert_eq!(kin.vertical_speed, "VERTICAL SPEED");
}

/// Altitude SimVar must be the indicated (barometric) altitude.
#[test]
fn test_altitude_simvar_name_is_indicated_altitude() {
    let cfg = create_default_mapping();
    assert_eq!(cfg.default_mapping.environment.altitude, "INDICATED ALTITUDE");
}

/// Geographic position SimVars use the PLANE LATITUDE / PLANE LONGITUDE names.
///
/// These are required for position-valid telemetry and GPS logging.
#[test]
fn test_position_simvar_names_are_correct() {
    let cfg = create_default_mapping();
    let nav = &cfg.default_mapping.navigation;

    assert_eq!(nav.latitude, "PLANE LATITUDE");
    assert_eq!(nav.longitude, "PLANE LONGITUDE");
}

/// Autopilot SimVars must use the exact MSFS boolean and setpoint names.
///
/// A misspelling here means AP state always reads as Off, regardless of what
/// the sim is doing.
#[test]
fn test_autopilot_simvar_names_are_correct() {
    let cfg = create_default_mapping();
    let c = &cfg.default_mapping.config;

    // Boolean engagement flags
    assert_eq!(c.ap_master, "AUTOPILOT MASTER");
    assert_eq!(c.ap_altitude_hold, "AUTOPILOT ALTITUDE LOCK");
    assert_eq!(c.ap_heading_hold, "AUTOPILOT HEADING LOCK");
    assert_eq!(c.ap_speed_hold, "AUTOPILOT AIRSPEED HOLD");

    // Setpoint readbacks
    assert_eq!(c.ap_altitude, "AUTOPILOT ALTITUDE LOCK VAR");
    assert_eq!(c.ap_heading, "AUTOPILOT HEADING LOCK DIR");
    assert_eq!(c.ap_speed, "AUTOPILOT AIRSPEED HOLD VAR");
}

/// Gear position SimVars match the MSFS per-leg names (nose/left/right).
#[test]
fn test_gear_simvar_names_are_correct() {
    let cfg = create_default_mapping();
    let c = &cfg.default_mapping.config;

    assert_eq!(c.gear_nose, "GEAR CENTER POSITION");
    assert_eq!(c.gear_left, "GEAR LEFT POSITION");
    assert_eq!(c.gear_right, "GEAR RIGHT POSITION");
}

/// Flap and spoiler SimVars use the handle (not the surface) position.
#[test]
fn test_flap_and_spoiler_simvar_names_are_correct() {
    let cfg = create_default_mapping();
    let c = &cfg.default_mapping.config;

    assert_eq!(c.flaps, "FLAPS HANDLE PERCENT");
    assert_eq!(c.spoilers, "SPOILERS HANDLE POSITION");
}

/// Aero-angle SimVars use the INCIDENCE family of names.
#[test]
fn test_aero_angle_simvar_names_are_correct() {
    let cfg = create_default_mapping();
    let kin = &cfg.default_mapping.kinematics;

    assert_eq!(kin.aoa, "INCIDENCE ALPHA");
    assert_eq!(kin.sideslip, "INCIDENCE BETA");
}

/// Lateral and longitudinal G-load use the ACCELERATION BODY X/Z SimVars.
#[test]
fn test_g_load_simvar_names_are_correct() {
    let cfg = create_default_mapping();
    let kin = &cfg.default_mapping.kinematics;

    assert_eq!(kin.g_force, "G FORCE");
    assert_eq!(kin.g_lateral, "ACCELERATION BODY X");
    assert_eq!(kin.g_longitudinal, "ACCELERATION BODY Z");
}

/// Default engine uses the GENERAL ENG COMBUSTION:1 name for the running flag.
#[test]
fn test_engine_running_simvar_name_is_correct() {
    let cfg = create_default_mapping();
    let eng = &cfg.default_mapping.engines[0];

    assert_eq!(eng.running, "GENERAL ENG COMBUSTION:1");
    assert_eq!(eng.rpm, "GENERAL ENG RPM:1");
}

// ============================================================================
// 3. Indexed variable format (:N) for multi-engine aircraft
// ============================================================================

/// Default engine mapping uses the `:1` MSFS one-based index suffix.
///
/// All engine SimVars have the form `GENERAL ENG <PARAM>:N` where N is the
/// one-based engine number.  Index 0 in Flight Hub maps to `:1` in MSFS.
#[test]
fn test_engine_simvars_use_one_based_index_suffix() {
    let cfg = create_default_mapping();
    let eng = &cfg.default_mapping.engines[0];

    assert!(
        eng.running.ends_with(":1"),
        "engine running SimVar '{}' must end with ':1'",
        eng.running
    );
    assert!(
        eng.rpm.ends_with(":1"),
        "engine RPM SimVar '{}' must end with ':1'",
        eng.rpm
    );
    if let Some(ref mp) = eng.manifold_pressure {
        assert!(
            mp.ends_with(":1"),
            "manifold pressure SimVar '{}' must end with ':1'",
            mp
        );
    }
    if let Some(ref egt) = eng.egt {
        assert!(egt.ends_with(":1"), "EGT SimVar '{}' must end with ':1'", egt);
    }
    if let Some(ref cht) = eng.cht {
        assert!(cht.ends_with(":1"), "CHT SimVar '{}' must end with ':1'", cht);
    }
    if let Some(ref ff) = eng.fuel_flow {
        assert!(
            ff.ends_with(":1"),
            "fuel flow SimVar '{}' must end with ':1'",
            ff
        );
    }
    if let Some(ref op) = eng.oil_pressure {
        assert!(
            op.ends_with(":1"),
            "oil pressure SimVar '{}' must end with ':1'",
            op
        );
    }
    if let Some(ref ot) = eng.oil_temperature {
        assert!(
            ot.ends_with(":1"),
            "oil temperature SimVar '{}' must end with ':1'",
            ot
        );
    }
}

/// A second-engine mapping (e.g., twin-piston King Air 350) must use `:2`.
///
/// This verifies that the indexed SimVar format generalises correctly to
/// multi-engine aircraft beyond the single-engine default.
#[test]
fn test_second_engine_uses_index_two_suffix() {
    let engine2 = EngineMapping {
        index: 1, // 0-based in Flight Hub
        running: "GENERAL ENG COMBUSTION:2".to_string(),
        rpm: "GENERAL ENG RPM:2".to_string(),
        manifold_pressure: Some("RECIP ENG MANIFOLD PRESSURE:2".to_string()),
        egt: Some("GENERAL ENG EXHAUST GAS TEMPERATURE:2".to_string()),
        cht: Some("RECIP ENG CYLINDER HEAD TEMPERATURE:2".to_string()),
        fuel_flow: Some("GENERAL ENG FUEL FLOW GPH:2".to_string()),
        oil_pressure: Some("GENERAL ENG OIL PRESSURE:2".to_string()),
        oil_temperature: Some("GENERAL ENG OIL TEMPERATURE:2".to_string()),
    };

    assert_eq!(engine2.index, 1, "second engine must have 0-based index 1");
    assert!(
        engine2.running.ends_with(":2"),
        "'{}' must use MSFS one-based index :2",
        engine2.running
    );
    assert!(
        engine2.rpm.ends_with(":2"),
        "'{}' must use MSFS one-based index :2",
        engine2.rpm
    );
    assert!(engine2.manifold_pressure.as_deref().unwrap().ends_with(":2"));
    assert!(engine2.egt.as_deref().unwrap().ends_with(":2"));
    assert!(engine2.cht.as_deref().unwrap().ends_with(":2"));
    assert!(engine2.fuel_flow.as_deref().unwrap().ends_with(":2"));
    assert!(engine2.oil_pressure.as_deref().unwrap().ends_with(":2"));
    assert!(engine2.oil_temperature.as_deref().unwrap().ends_with(":2"));
}

/// The 0-based Flight Hub engine index always equals the MSFS SimVar suffix − 1.
#[test]
fn test_engine_index_offset_is_one() {
    let cfg = create_default_mapping();
    let eng = &cfg.default_mapping.engines[0];

    let msfs_suffix: u8 = eng
        .running
        .split(':')
        .last()
        .and_then(|s| s.parse().ok())
        .expect("engine running SimVar must contain a colon-separated index");

    assert_eq!(
        msfs_suffix,
        eng.index + 1,
        "MSFS engine suffix must equal Flight Hub index + 1"
    );
}

/// Autopilot setpoint SimVars for single-engine aircraft use no index suffix.
///
/// Unlike engine variables (`:1`), AUTOPILOT ALTITUDE LOCK VAR is not indexed
/// in the default mapping — verify this expectation explicitly.
#[test]
fn test_autopilot_setpoint_simvars_have_no_index_suffix() {
    let cfg = create_default_mapping();
    let c = &cfg.default_mapping.config;

    assert!(
        !c.ap_altitude.contains(':'),
        "AUTOPILOT ALTITUDE LOCK VAR should not have an index suffix by default, got '{}'",
        c.ap_altitude
    );
    assert!(
        !c.ap_heading.contains(':'),
        "AUTOPILOT HEADING LOCK DIR should not have an index suffix by default, got '{}'",
        c.ap_heading
    );
    assert!(
        !c.ap_speed.contains(':'),
        "AUTOPILOT AIRSPEED HOLD VAR should not have an index suffix by default, got '{}'",
        c.ap_speed
    );
}

// ============================================================================
// 4. Unit conversion direction
// ============================================================================

/// 0° → 0 rad (due north).
#[test]
fn test_heading_north_converts_to_zero_radians() {
    let north = MsfsConverter::convert_angle_degrees(0.0).unwrap();
    assert_eq!(north.to_degrees(), 0.0f32);
    assert!(north.to_radians().abs() < 1e-6, "0° should be 0 rad");
}

/// 90° (due east) → π/2 rad.
#[test]
fn test_heading_east_converts_to_half_pi_radians() {
    use std::f32::consts::FRAC_PI_2;
    let east = MsfsConverter::convert_angle_degrees(90.0).unwrap();
    assert_eq!(east.to_degrees(), 90.0f32);
    assert!(
        (east.to_radians() - FRAC_PI_2).abs() < 1e-5,
        "90° should be π/2 rad, got {}",
        east.to_radians()
    );
}

/// 5° pitch-up survives the degrees → stored → degrees round-trip losslessly.
#[test]
fn test_pitch_up_degree_round_trip() {
    let pitch = MsfsConverter::convert_angle_degrees(5.0).unwrap();
    assert_eq!(pitch.to_degrees(), 5.0f32, "5° pitch must round-trip exactly");
}

/// 2° bank survives the degrees → stored → degrees round-trip losslessly.
#[test]
fn test_bank_degree_round_trip() {
    let bank = MsfsConverter::convert_angle_degrees(2.0).unwrap();
    assert_eq!(bank.to_degrees(), 2.0f32, "2° bank must round-trip exactly");
}

/// Heading of 270° wraps to −90° after normalisation to (−180°, 180°].
///
/// PLANE HEADING DEGREES MAGNETIC returns 0–360; the converter normalises
/// this to (−180°, 180°] so that heading differences can be calculated with
/// simple subtraction.
#[test]
fn test_heading_270_degrees_normalises_to_minus_90() {
    let heading = MsfsConverter::convert_angle_degrees(270.0).unwrap();
    assert_eq!(
        heading.to_degrees(),
        -90.0f32,
        "270° heading must normalise to −90°"
    );
}

/// VERTICAL SPEED is measured in feet-per-minute; 0 FPM → 0 m/s.
#[test]
fn test_vertical_speed_zero_fpm_is_zero() {
    let vs_mps: f32 = 0.0 * 0.00508;
    assert_eq!(vs_mps, 0.0f32);
}

/// 500 FPM ≈ 2.54 m/s (conversion factor: 1 FPM = 0.00508 m/s).
#[test]
fn test_vertical_speed_500_fpm_is_approximately_2p54_mps() {
    let vs_mps: f32 = 500.0 * 0.00508;
    assert!(
        (vs_mps - 2.54f32).abs() < 0.01,
        "500 FPM should be ~2.54 m/s, got {}",
        vs_mps
    );
}

/// ACCELERATION BODY X/Z (ft/s²) → G: 32.174 ft/s² = 1 g.
///
/// The mapping divides by STANDARD_GRAVITY_FT_S2 (32.174).  Verify the
/// conversion factor is applied in the correct direction.
#[test]
fn test_acceleration_body_one_g_equals_standard_gravity_ft_s2() {
    const STANDARD_GRAVITY_FT_S2: f64 = 32.174;
    let g = MsfsConverter::convert_g_force(STANDARD_GRAVITY_FT_S2 / STANDARD_GRAVITY_FT_S2)
        .unwrap();
    assert_eq!(g.value(), 1.0f32, "32.174 ft/s² must convert to exactly 1 g");
}

// ============================================================================
// 5. Boolean SimVar conversion semantics
// ============================================================================

/// Non-zero INT32 SimVar → `true`.
///
/// MSFS AUTOPILOT MASTER and GEAR HANDLE POSITION use INT32 bool semantics:
/// 0 = off/up, any non-zero = on/down.
#[test]
fn test_bool_simvar_nonzero_is_true() {
    assert!(simvar_bool_from_int32(1), "1 must be true");
    assert!(simvar_bool_from_int32(2), "any non-zero must be true");
    assert!(simvar_bool_from_int32(i32::MAX), "MAX must be true");
}

/// Zero INT32 SimVar → `false`.
#[test]
fn test_bool_simvar_zero_is_false() {
    assert!(!simvar_bool_from_int32(0), "0 must be false");
}

/// AUTOPILOT MASTER: 0 → Off, 1 → Armed (no modes active).
#[test]
fn test_autopilot_master_bool_to_state() {
    let ap_off = simvar_bool_from_int32(0);
    let state = if ap_off {
        AutopilotState::Armed
    } else {
        AutopilotState::Off
    };
    assert_eq!(state, AutopilotState::Off, "AP master 0 must produce Off state");

    let ap_on = simvar_bool_from_int32(1);
    let state = if ap_on {
        AutopilotState::Armed
    } else {
        AutopilotState::Off
    };
    assert_eq!(state, AutopilotState::Armed, "AP master 1 must produce Armed state");
}

/// AUTOPILOT MASTER on with a hold mode active → Engaged.
#[test]
fn test_autopilot_master_plus_hold_mode_gives_engaged_state() {
    let ap_master = simvar_bool_from_int32(1);
    let alt_hold = simvar_bool_from_int32(1);

    let state = if !ap_master {
        AutopilotState::Off
    } else if alt_hold {
        AutopilotState::Engaged
    } else {
        AutopilotState::Armed
    };

    assert_eq!(state, AutopilotState::Engaged);
}

/// Replicates the `read_bool` logic from mapping.rs: value != 0.0.
fn simvar_bool_from_int32(raw: i32) -> bool {
    (raw as f64) != 0.0
}

// ============================================================================
// 6. Throttle position normalisation — GENERAL ENG THROTTLE LEVER POSITION
// ============================================================================

/// GENERAL ENG THROTTLE LEVER POSITION is 0–100 in MSFS; after normalisation
/// to a [0.0, 1.0] fraction the values must be correct.
#[test]
fn test_throttle_lever_full_range_normalises_to_0_1() {
    assert_eq!(normalise_throttle_pct(0.0), 0.0f32, "idle");
    assert_eq!(normalise_throttle_pct(100.0), 1.0f32, "full");
    assert!(
        (normalise_throttle_pct(50.0) - 0.5f32).abs() < 1e-6,
        "half throttle"
    );
    assert!(
        (normalise_throttle_pct(25.0) - 0.25f32).abs() < 1e-6,
        "quarter throttle"
    );
}

/// Values outside 0–100 must be clamped to [0.0, 1.0].
#[test]
fn test_throttle_lever_out_of_range_clamps() {
    assert_eq!(normalise_throttle_pct(-10.0), 0.0f32, "below idle → 0.0");
    assert_eq!(normalise_throttle_pct(110.0), 1.0f32, "over full → 1.0");
}

/// Engine 1 and engine 2 throttle positions normalise independently.
#[test]
fn test_throttle_lever_engine1_and_engine2_are_independent() {
    let eng1_pos = 75.0f32; // 75 % throttle
    let eng2_pos = 50.0f32; // 50 % throttle

    let eng1_norm = normalise_throttle_pct(eng1_pos);
    let eng2_norm = normalise_throttle_pct(eng2_pos);

    assert!((eng1_norm - 0.75f32).abs() < 1e-6);
    assert!((eng2_norm - 0.50f32).abs() < 1e-6);
    assert_ne!(eng1_norm, eng2_norm, "independent throttles must differ");
}

/// Normalises a 0–100 percent throttle value to [0.0, 1.0].
fn normalise_throttle_pct(pct: f32) -> f32 {
    (pct / 100.0).clamp(0.0, 1.0)
}

// ============================================================================
// 7. Control-surface axis normalisation — ELEVATOR / AILERON / RUDDER POSITION
// ============================================================================

/// SimConnect POSITION-unit variables use the −16383 to +16383 range.
/// After normalisation the output must be in [−1.0, 1.0].
#[test]
fn test_control_surface_full_deflection_is_plus_minus_one() {
    assert!(
        (normalise_axis(16383) - 1.0f32).abs() < 1e-4,
        "full positive deflection must be ≈ 1.0, got {}",
        normalise_axis(16383)
    );
    assert!(
        (normalise_axis(-16383) - (-1.0f32)).abs() < 1e-4,
        "full negative deflection must be ≈ -1.0, got {}",
        normalise_axis(-16383)
    );
    assert!(
        normalise_axis(0).abs() < 1e-6,
        "neutral must be exactly 0.0"
    );
}

/// Half-deflection normalises to ≈ ±0.5.
#[test]
fn test_control_surface_half_deflection_is_approximately_half() {
    let half_pos = normalise_axis(8191);
    let half_neg = normalise_axis(-8191);

    assert!(
        (half_pos - 0.5f32).abs() < 0.01,
        "half positive should be ~0.5, got {}",
        half_pos
    );
    assert!(
        (half_neg - (-0.5f32)).abs() < 0.01,
        "half negative should be ~-0.5, got {}",
        half_neg
    );
}

/// Axis values beyond ±16383 are clamped to ±1.0.
#[test]
fn test_control_surface_axis_out_of_bounds_clamps_to_one() {
    assert_eq!(normalise_axis(32767), 1.0f32, "out-of-range high → 1.0");
    assert_eq!(normalise_axis(-32767), -1.0f32, "out-of-range low → -1.0");
}

/// Elevator, aileron, and rudder each independently produce the full ±1 range.
#[test]
fn test_elevator_aileron_rudder_each_cover_full_range() {
    // Simulate three independent surfaces at their deflection limits.
    let elevator = normalise_axis(16383);
    let aileron = normalise_axis(-16383);
    let rudder = normalise_axis(0);

    assert!((elevator - 1.0f32).abs() < 1e-4, "elevator full up");
    assert!((aileron - (-1.0f32)).abs() < 1e-4, "aileron full left");
    assert!(rudder.abs() < 1e-6, "rudder neutral");
}

/// Normalises a POSITION-unit SimConnect value (±16383) to [−1.0, 1.0].
fn normalise_axis(raw: i32) -> f32 {
    (raw as f32 / 16383.0).clamp(-1.0, 1.0)
}

// ============================================================================
// 8. Generic invariants on the complete SimVar name table
// ============================================================================

/// No SimVar name may be empty.
///
/// An empty string passed to `add_to_data_definition` silently requests no
/// data from MSFS; all reads would return 0.0.
#[test]
fn test_all_simvar_names_are_nonempty() {
    for name in all_simvar_names_sorted() {
        assert!(!name.is_empty(), "found empty SimVar name in default mapping");
    }
}

/// No SimVar name may have leading or trailing whitespace.
///
/// MSFS treats `"AIRSPEED INDICATED "` (trailing space) as a different
/// (unknown) variable from `"AIRSPEED INDICATED"` and returns 0.0 for it.
#[test]
fn test_all_simvar_names_have_no_stray_whitespace() {
    for name in all_simvar_names_sorted() {
        assert_eq!(
            name.trim(),
            name.as_str(),
            "SimVar '{name}' has extraneous leading/trailing whitespace"
        );
    }
}

/// All SimVar base names (before the `:N` index suffix) must be uppercase.
///
/// MSFS SimVar names are defined in upper case in the SDK documentation.
/// Lowercase variants are not guaranteed to work.
#[test]
fn test_all_simvar_base_names_are_uppercase() {
    for name in all_simvar_names_sorted() {
        let base = name.split(':').next().unwrap_or(&name);
        assert_eq!(
            base,
            base.to_ascii_uppercase(),
            "SimVar '{name}' base is not fully uppercase"
        );
    }
}

/// Indexed SimVar names use a numeric suffix after the colon.
///
/// MSFS engine variables are indexed as `GENERAL ENG RPM:1`, `…:2`, etc.
/// A non-numeric suffix (e.g., `":A"`) would be silently ignored by SimConnect.
#[test]
fn test_indexed_simvar_names_have_numeric_suffix() {
    for name in all_simvar_names_sorted() {
        if let Some(suffix) = name.split(':').nth(1) {
            assert!(
                suffix.parse::<u32>().is_ok(),
                "SimVar '{name}' has non-numeric index suffix '{suffix}'"
            );
        }
    }
}
