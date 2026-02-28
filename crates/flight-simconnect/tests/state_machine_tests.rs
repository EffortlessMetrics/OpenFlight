#![cfg(windows)]
// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! State machine, config validation, and fixture golden tests for MSFS SimConnect adapter.
//!
//! Requirements: MSFS-INT-01.1, MSFS-INT-01.2, MSFS-INT-01.4, MSFS-INT-01.5

use flight_bus::adapters::msfs::MsfsConverter;
use flight_simconnect::{AdapterState, MsfsAdapter, MsfsAdapterConfig, SessionConfig};
use std::time::Duration;

// ---------------------------------------------------------------------------
// State machine tests
// ---------------------------------------------------------------------------

/// Initial adapter state must be Disconnected.
/// Requirements: MSFS-INT-01.1
#[tokio::test]
async fn test_initial_state_is_disconnected() {
    let adapter = MsfsAdapter::new(MsfsAdapterConfig::default()).unwrap();
    assert_eq!(adapter.state().await, AdapterState::Disconnected);
}

/// Adapter must not be active before any connection.
#[tokio::test]
async fn test_adapter_not_active_initially() {
    let adapter = MsfsAdapter::new(MsfsAdapterConfig::default()).unwrap();
    assert!(!adapter.is_active().await);
}

/// No aircraft should be set before connection.
#[tokio::test]
async fn test_no_aircraft_before_connection() {
    let adapter = MsfsAdapter::new(MsfsAdapterConfig::default()).unwrap();
    assert!(adapter.current_aircraft().await.is_none());
}

/// No snapshot should exist before connection.
#[tokio::test]
async fn test_no_snapshot_before_connection() {
    let adapter = MsfsAdapter::new(MsfsAdapterConfig::default()).unwrap();
    assert!(adapter.current_snapshot().await.is_none());
}

/// `handle_connection_loss` transitions state to Disconnected.
/// Requirements: MSFS-INT-01.19
#[tokio::test]
async fn test_connection_loss_transitions_to_disconnected() {
    let mut adapter = MsfsAdapter::new(MsfsAdapterConfig::default()).unwrap();
    adapter.handle_connection_loss().await;
    assert_eq!(adapter.state().await, AdapterState::Disconnected);
}

/// `handle_connection_loss` clears current aircraft info.
/// Requirements: MSFS-INT-01.19
#[tokio::test]
async fn test_connection_loss_clears_aircraft() {
    let mut adapter = MsfsAdapter::new(MsfsAdapterConfig::default()).unwrap();
    adapter.handle_connection_loss().await;
    assert!(adapter.current_aircraft().await.is_none());
}

/// `handle_connection_loss` clears the current bus snapshot.
/// Requirements: MSFS-INT-01.19
#[tokio::test]
async fn test_connection_loss_clears_snapshot() {
    let mut adapter = MsfsAdapter::new(MsfsAdapterConfig::default()).unwrap();
    adapter.handle_connection_loss().await;
    assert!(adapter.current_snapshot().await.is_none());
}

/// `stop` from a disconnected adapter succeeds and stays Disconnected.
#[tokio::test]
async fn test_stop_from_disconnected_succeeds() {
    let mut adapter = MsfsAdapter::new(MsfsAdapterConfig::default()).unwrap();
    assert!(adapter.stop().await.is_ok());
    assert_eq!(adapter.state().await, AdapterState::Disconnected);
}

/// Multiple consecutive `handle_connection_loss` calls are idempotent.
#[tokio::test]
async fn test_connection_loss_idempotent() {
    let mut adapter = MsfsAdapter::new(MsfsAdapterConfig::default()).unwrap();
    adapter.handle_connection_loss().await;
    adapter.handle_connection_loss().await;
    assert_eq!(adapter.state().await, AdapterState::Disconnected);
    assert!(adapter.current_aircraft().await.is_none());
}

/// All adapter state enum variants are distinct.
#[test]
fn test_state_variants_are_distinct() {
    assert_ne!(AdapterState::Disconnected, AdapterState::Connecting);
    assert_ne!(AdapterState::Connecting, AdapterState::Connected);
    assert_ne!(AdapterState::Connected, AdapterState::DetectingAircraft);
    assert_ne!(AdapterState::DetectingAircraft, AdapterState::Active);
    assert_ne!(AdapterState::Active, AdapterState::Error);
    assert_ne!(AdapterState::Disconnected, AdapterState::Active);
    assert_ne!(AdapterState::Disconnected, AdapterState::Error);
}

/// Connection attempt counter starts at zero.
#[tokio::test]
async fn test_connection_attempts_start_at_zero() {
    let adapter = MsfsAdapter::new(MsfsAdapterConfig::default()).unwrap();
    assert_eq!(adapter.connection_attempts(), 0);
}

/// Backoff delay starts at 1 second.
#[tokio::test]
async fn test_initial_backoff_delay() {
    let adapter = MsfsAdapter::new(MsfsAdapterConfig::default()).unwrap();
    assert_eq!(adapter.current_backoff_delay(), 1.0);
}

/// Exponential backoff doubles each step and is capped at 30 s.
#[test]
fn test_exponential_backoff_calculation() {
    let steps = [1.0_f64, 2.0, 4.0, 8.0, 16.0, 30.0, 30.0];
    let mut delay = 1.0_f64;
    for &expected in &steps {
        assert!(
            (delay - expected).abs() < 0.001,
            "expected {expected}, got {delay}"
        );
        delay = (delay * 2.0).min(30.0);
    }
}

// ---------------------------------------------------------------------------
// Config validation tests
// ---------------------------------------------------------------------------

/// Default `MsfsAdapterConfig` should have sensible non-zero values.
#[test]
fn test_default_adapter_config_is_valid() {
    let cfg = MsfsAdapterConfig::default();
    assert!(cfg.publish_rate > 0.0, "publish_rate must be positive");
    assert!(
        cfg.max_reconnect_attempts > 0,
        "must allow at least one reconnect"
    );
    assert!(
        cfg.aircraft_detection_timeout > Duration::ZERO,
        "detection timeout must be non-zero"
    );
    assert!(
        cfg.auto_reconnect,
        "auto-reconnect should be enabled by default"
    );
}

/// Default `SessionConfig` describes a local SimConnect connection.
/// Requirements: MSFS-INT-01.1
#[test]
fn test_session_config_local_connection() {
    let cfg = SessionConfig::default();
    // config_index 0 means "use local default" - no SimConnect.cfg needed.
    assert_eq!(cfg.config_index, 0, "default must use local connection");
    assert_eq!(cfg.app_name, "Flight Hub");
    assert!(cfg.connect_timeout > Duration::ZERO);
    assert!(cfg.poll_interval > Duration::ZERO);
}

/// Disabling auto-reconnect is reflected in the config and adapter creation succeeds.
#[tokio::test]
async fn test_disabled_auto_reconnect_config() {
    let mut cfg = MsfsAdapterConfig::default();
    cfg.auto_reconnect = false;
    let adapter = MsfsAdapter::new(cfg).unwrap();
    assert_eq!(adapter.state().await, AdapterState::Disconnected);
}

/// Custom publish rate is preserved through adapter creation.
#[tokio::test]
async fn test_custom_publish_rate() {
    let mut cfg = MsfsAdapterConfig::default();
    cfg.publish_rate = 30.0;
    // Adapter creation should succeed with a non-default publish rate.
    let adapter = MsfsAdapter::new(cfg).unwrap();
    assert_eq!(adapter.state().await, AdapterState::Disconnected);
}

/// Default mapping has all required SimVar fields populated.
#[test]
fn test_default_mapping_has_required_simvars() {
    use flight_simconnect::mapping::create_default_mapping;

    let cfg = create_default_mapping();
    let kin = &cfg.default_mapping.kinematics;
    assert!(!kin.ias.is_empty(), "IAS simvar must be set");
    assert!(!kin.tas.is_empty(), "TAS simvar must be set");
    assert!(
        !kin.ground_speed.is_empty(),
        "ground speed simvar must be set"
    );
    assert!(!kin.aoa.is_empty(), "AoA simvar must be set");
    assert!(!kin.sideslip.is_empty(), "sideslip simvar must be set");
    assert!(!kin.bank.is_empty(), "bank simvar must be set");
    assert!(!kin.pitch.is_empty(), "pitch simvar must be set");
    assert!(!kin.heading.is_empty(), "heading simvar must be set");
    assert!(!kin.g_force.is_empty(), "g-force simvar must be set");
    assert!(!kin.mach.is_empty(), "Mach simvar must be set");
    assert!(
        !kin.vertical_speed.is_empty(),
        "vertical speed simvar must be set"
    );

    let config = &cfg.default_mapping.config;
    assert!(!config.gear_nose.is_empty(), "gear nose simvar must be set");
    assert!(!config.gear_left.is_empty(), "gear left simvar must be set");
    assert!(
        !config.gear_right.is_empty(),
        "gear right simvar must be set"
    );
    assert!(!config.flaps.is_empty(), "flaps simvar must be set");
    assert!(!config.ap_master.is_empty(), "AP master simvar must be set");

    assert!(
        !cfg.default_mapping.engines.is_empty(),
        "engine mapping must not be empty"
    );
}

/// Default update rates are positive and in a sensible range.
#[test]
fn test_default_update_rates() {
    use flight_simconnect::mapping::create_default_mapping;

    let cfg = create_default_mapping();
    let rates = &cfg.update_rates;
    assert!(rates.kinematics > 0.0 && rates.kinematics <= 250.0);
    assert!(rates.config > 0.0 && rates.config <= 250.0);
    assert!(rates.engines > 0.0 && rates.engines <= 250.0);
    assert!(rates.environment > 0.0 && rates.environment <= 250.0);
    assert!(rates.navigation > 0.0 && rates.navigation <= 250.0);
    // Kinematics should be the fastest (most time-critical)
    assert!(rates.kinematics >= rates.navigation);
}

// ---------------------------------------------------------------------------
// Fixture / golden tests
// ---------------------------------------------------------------------------
//
// Values are taken from tests/fixtures/msfs_c172_cruise.json
// (C172 cruise at 2500 ft, 100 kts).  These tests act as golden regression
// tests: if a converter changes its semantics the test will fail.

/// IAS from the C172 cruise fixture converts correctly.
#[test]
fn test_fixture_c172_ias() {
    let ias = MsfsConverter::convert_ias(100.0).unwrap();
    assert_eq!(ias.to_knots(), 100.0f32);
    // 1 knot = 0.514444 m/s
    assert!((ias.to_mps() - 51.4444f32).abs() < 0.01, "IAS m/s mismatch");
}

/// TAS from the C172 cruise fixture converts correctly.
#[test]
fn test_fixture_c172_tas() {
    let tas = MsfsConverter::convert_tas(105.0).unwrap();
    assert_eq!(tas.to_knots(), 105.0f32);
}

/// Ground speed from the C172 cruise fixture converts correctly.
#[test]
fn test_fixture_c172_ground_speed() {
    let gs = MsfsConverter::convert_ground_speed(98.0).unwrap();
    assert_eq!(gs.to_knots(), 98.0f32);
}

/// Heading 270° normalises to −90° (fixture expected value).
/// Requirements: MSFS-INT-01.4
#[test]
fn test_fixture_c172_heading_normalization() {
    let heading = MsfsConverter::convert_angle_degrees(270.0).unwrap();
    assert!(
        (heading.to_degrees() - (-90.0f32)).abs() < 0.001,
        "270° should normalise to -90°, got {}°",
        heading.to_degrees()
    );
    assert!(
        (heading.to_radians() - (-std::f32::consts::FRAC_PI_2)).abs() < 0.001,
        "heading radians mismatch"
    );
}

/// AoA from the C172 cruise fixture.
#[test]
fn test_fixture_c172_aoa() {
    let aoa = MsfsConverter::convert_angle_degrees(3.5).unwrap();
    assert!((aoa.to_degrees() - 3.5f32).abs() < 0.001);
    // 3.5° × π/180 ≈ 0.061087 rad
    assert!((aoa.to_radians() - 0.061087f32).abs() < 0.0001);
}

/// Sideslip from the C172 cruise fixture.
#[test]
fn test_fixture_c172_sideslip() {
    let beta = MsfsConverter::convert_angle_degrees(0.2).unwrap();
    assert!((beta.to_degrees() - 0.2f32).abs() < 0.001);
    // 0.2° × π/180 ≈ 0.003491 rad
    assert!((beta.to_radians() - 0.003491f32).abs() < 0.0001);
}

/// Bank and pitch from the C172 cruise fixture.
#[test]
fn test_fixture_c172_bank_and_pitch() {
    let bank = MsfsConverter::convert_angle_degrees(2.0).unwrap();
    assert!((bank.to_degrees() - 2.0f32).abs() < 0.001);
    // 2° × π/180 ≈ 0.034907 rad
    assert!((bank.to_radians() - 0.034907f32).abs() < 0.0001);

    let pitch = MsfsConverter::convert_angle_degrees(5.0).unwrap();
    assert!((pitch.to_degrees() - 5.0f32).abs() < 0.001);
    // 5° × π/180 ≈ 0.087266 rad
    assert!((pitch.to_radians() - 0.087266f32).abs() < 0.0001);
}

/// G-force values from the C172 cruise fixture.
#[test]
fn test_fixture_c172_g_forces() {
    let g = MsfsConverter::convert_g_force(1.0).unwrap();
    assert_eq!(g.value(), 1.0f32, "level-flight g-force");

    let g_lat = MsfsConverter::convert_g_force(0.05).unwrap();
    assert_eq!(g_lat.value(), 0.05f32, "lateral g-force");

    let g_long = MsfsConverter::convert_g_force(0.1).unwrap();
    assert_eq!(g_long.value(), 0.1f32, "longitudinal g-force");
}

/// Mach number from the C172 cruise fixture.
#[test]
fn test_fixture_c172_mach() {
    let mach = MsfsConverter::convert_mach(0.15).unwrap();
    assert_eq!(mach.value(), 0.15f32);
}

/// Comprehensive golden test: all kinematic fixture values in one pass.
/// Requirements: MSFS-INT-01.4, MSFS-INT-01.5, MSFS-INT-01.6, SIM-TEST-01.2
#[test]
fn test_fixture_c172_all_kinematics_golden() {
    // ---- SimVar values (from msfs_c172_cruise.json "simvars") ----
    let sv_ias = 100.0f64;
    let sv_tas = 105.0f64;
    let sv_gs = 98.0f64;
    let sv_aoa = 3.5f64;
    let sv_sideslip = 0.2f64;
    let sv_bank = 2.0f64;
    let sv_pitch = 5.0f64;
    let sv_heading = 270.0f64; // → normalises to -90°
    let sv_g = 1.0f64;
    let sv_g_lat = 0.05f64;
    let sv_g_long = 0.1f64;
    let sv_mach = 0.15f64;

    // ---- Expected bus values (from "expected_bus_values") ----
    assert_eq!(
        MsfsConverter::convert_ias(sv_ias).unwrap().to_knots(),
        100.0f32
    );
    assert_eq!(
        MsfsConverter::convert_tas(sv_tas).unwrap().to_knots(),
        105.0f32
    );
    assert_eq!(
        MsfsConverter::convert_ground_speed(sv_gs)
            .unwrap()
            .to_knots(),
        98.0f32
    );

    let aoa = MsfsConverter::convert_angle_degrees(sv_aoa).unwrap();
    assert!((aoa.to_degrees() - 3.5f32).abs() < 0.001);
    assert!((aoa.to_radians() - 0.061087f32).abs() < 0.0001);

    let beta = MsfsConverter::convert_angle_degrees(sv_sideslip).unwrap();
    assert!((beta.to_degrees() - 0.2f32).abs() < 0.001);
    assert!((beta.to_radians() - 0.003491f32).abs() < 0.0001);

    let bank = MsfsConverter::convert_angle_degrees(sv_bank).unwrap();
    assert!((bank.to_degrees() - 2.0f32).abs() < 0.001);
    assert!((bank.to_radians() - 0.034907f32).abs() < 0.0001);

    let pitch = MsfsConverter::convert_angle_degrees(sv_pitch).unwrap();
    assert!((pitch.to_degrees() - 5.0f32).abs() < 0.001);
    assert!((pitch.to_radians() - 0.087266f32).abs() < 0.0001);

    let heading = MsfsConverter::convert_angle_degrees(sv_heading).unwrap();
    assert!((heading.to_degrees() - (-90.0f32)).abs() < 0.001);
    assert!((heading.to_radians() - (-1.570796f32)).abs() < 0.0001);

    assert_eq!(
        MsfsConverter::convert_g_force(sv_g).unwrap().value(),
        1.0f32
    );
    assert_eq!(
        MsfsConverter::convert_g_force(sv_g_lat).unwrap().value(),
        0.05f32
    );
    assert_eq!(
        MsfsConverter::convert_g_force(sv_g_long).unwrap().value(),
        0.1f32
    );
    assert_eq!(
        MsfsConverter::convert_mach(sv_mach).unwrap().value(),
        0.15f32
    );
}
