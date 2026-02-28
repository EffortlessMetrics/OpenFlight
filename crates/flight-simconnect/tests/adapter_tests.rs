#![cfg(windows)]
// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Unit tests for SimConnect adapter state machine and reconnection behaviour.
//!
//! Coverage areas:
//!  1. Config defaults (SessionConfig + MsfsAdapterConfig)
//!  2. Adapter state transitions (Disconnected → Connecting → Connected)
//!  3. Reconnection backoff via ReconnectionStrategy
//!  4. Variable mapping — SimVar names match MSFS API strings
//!  5. BusSnapshot construction and conversion
//!  6. Disconnect / connection-loss handling

use flight_adapter_common::{AdapterState, ReconnectionStrategy};
use flight_bus::adapters::msfs::MsfsConverter;
use flight_bus::snapshot::BusSnapshot;
use flight_bus::types::{AircraftId, SimId};
use flight_simconnect::{
    MsfsAdapter, MsfsAdapterConfig, MsfsAdapterError, SessionConfig,
    mapping::create_default_mapping,
};
use std::time::Duration;

// ============================================================================
// 1. Config defaults
// ============================================================================

/// `SessionConfig` must specify a positive reconnect delay.
#[test]
fn test_session_config_reconnect_delay_positive() {
    let cfg = SessionConfig::default();
    assert!(
        cfg.reconnect_delay > Duration::ZERO,
        "reconnect_delay must be > 0, got {:?}",
        cfg.reconnect_delay
    );
}

/// `SessionConfig` must allow at least one reconnect attempt.
#[test]
fn test_session_config_max_reconnect_attempts_positive() {
    let cfg = SessionConfig::default();
    assert!(
        cfg.max_reconnect_attempts > 0,
        "max_reconnect_attempts must be > 0"
    );
}

/// Poll interval should target ~60 Hz (≈16 ms).
#[test]
fn test_session_config_poll_interval_is_approximately_60hz() {
    let cfg = SessionConfig::default();
    let ms = cfg.poll_interval.as_millis();
    assert!(
        (15..=17).contains(&ms),
        "poll_interval should be ~16 ms for 60 Hz, got {} ms",
        ms
    );
}

/// Connect timeout must be at least one second (not hair-trigger).
#[test]
fn test_session_config_connect_timeout_at_least_one_second() {
    let cfg = SessionConfig::default();
    assert!(
        cfg.connect_timeout >= Duration::from_secs(1),
        "connect_timeout must be ≥ 1 s, got {:?}",
        cfg.connect_timeout
    );
}

/// `MsfsAdapterConfig` publish rate should match the 60 Hz target.
#[test]
fn test_adapter_config_publish_rate_60hz_target() {
    let cfg = MsfsAdapterConfig::default();
    assert_eq!(cfg.publish_rate, 60.0);
    // Minimum interval between publishes at 60 Hz
    let min_interval = Duration::from_secs_f32(1.0 / cfg.publish_rate);
    assert!(
        (16..=17).contains(&min_interval.as_millis()),
        "60 Hz interval should be ~16 ms, got {} ms",
        min_interval.as_millis()
    );
}

// ============================================================================
// 2. Adapter state transitions
// ============================================================================

/// New adapter always starts in Disconnected state.
#[tokio::test]
async fn test_new_adapter_is_disconnected() {
    let adapter = MsfsAdapter::new(MsfsAdapterConfig::default()).unwrap();
    assert_eq!(adapter.state().await, AdapterState::Disconnected);
}

/// `start()` sets state to Connecting as its very first action, before any
/// SimConnect call.  On machines without MSFS the subsequent `connect()` call
/// fails, but the Connecting transition must still have happened (the state is
/// left as Connecting when `connect()` returns an error).
#[tokio::test]
async fn test_start_transitions_state_to_connecting_first() {
    let mut adapter = MsfsAdapter::new(MsfsAdapterConfig::default()).unwrap();
    assert_eq!(adapter.state().await, AdapterState::Disconnected);

    // start() may fail if MSFS/SimConnect is not available, which is expected.
    let result = adapter.start().await;

    match result {
        Ok(()) => {
            // MSFS is running on this machine — state should have advanced.
            let s = adapter.state().await;
            assert!(
                matches!(
                    s,
                    AdapterState::Connected
                        | AdapterState::DetectingAircraft
                        | AdapterState::Active
                ),
                "unexpected state after successful start: {:?}",
                s
            );
            let _ = adapter.stop().await;
        }
        Err(_) => {
            // Without MSFS the connection fails after state was set to Connecting.
            let s = adapter.state().await;
            assert_ne!(
                s,
                AdapterState::Disconnected,
                "state should have left Disconnected before connect() failed"
            );
        }
    }
}

/// `stop()` from Disconnected state succeeds and keeps state as Disconnected.
#[tokio::test]
async fn test_stop_from_disconnected_is_noop() {
    let mut adapter = MsfsAdapter::new(MsfsAdapterConfig::default()).unwrap();
    assert!(adapter.stop().await.is_ok());
    assert_eq!(adapter.state().await, AdapterState::Disconnected);
}

/// Calling `stop()` twice is idempotent.
#[tokio::test]
async fn test_stop_twice_is_idempotent() {
    let mut adapter = MsfsAdapter::new(MsfsAdapterConfig::default()).unwrap();
    assert!(adapter.stop().await.is_ok());
    assert!(adapter.stop().await.is_ok());
    assert_eq!(adapter.state().await, AdapterState::Disconnected);
}

/// All `AdapterState` variants are distinct — the state machine has no
/// ambiguous nodes.
#[test]
fn test_all_adapter_state_variants_are_distinct() {
    use AdapterState::*;
    let all = [
        Disconnected,
        Connecting,
        Connected,
        DetectingAircraft,
        Active,
        Error,
    ];
    for i in 0..all.len() {
        for j in (i + 1)..all.len() {
            assert_ne!(all[i], all[j], "{:?} == {:?}", all[i], all[j]);
        }
    }
}

/// `AdapterState` implements `Copy` — cloning a state value costs nothing.
#[test]
fn test_adapter_state_is_copy() {
    let s = AdapterState::Connecting;
    let t = s; // copy, not move
    assert_eq!(s, t);
}

// ============================================================================
// 3. Reconnection backoff
// ============================================================================

/// `ReconnectionStrategy` with the adapter's default parameters (1 s initial,
/// 30 s cap, 5 max attempts) should double on each step up to the cap.
#[test]
fn test_reconnection_strategy_simconnect_defaults_exponential_doubling() {
    let s = ReconnectionStrategy::new(5, Duration::from_secs(1), Duration::from_secs(30));

    assert_eq!(s.next_backoff(1), Duration::from_secs(1), "attempt 1");
    assert_eq!(s.next_backoff(2), Duration::from_secs(2), "attempt 2");
    assert_eq!(s.next_backoff(3), Duration::from_secs(4), "attempt 3");
    assert_eq!(s.next_backoff(4), Duration::from_secs(8), "attempt 4");
    assert_eq!(s.next_backoff(5), Duration::from_secs(16), "attempt 5");
    // 32 s > max 30 s → should cap
    assert_eq!(
        s.next_backoff(6),
        Duration::from_secs(30),
        "attempt 6 capped"
    );
}

/// The adapter's in-process backoff calculation (`delay = min(delay*2, 30.0)`)
/// must produce the same sequence as `ReconnectionStrategy` for the same
/// parameters.
#[test]
fn test_adapter_inline_backoff_matches_strategy() {
    let s = ReconnectionStrategy::new(5, Duration::from_secs(1), Duration::from_secs(30));
    let mut delay = 1.0_f64;

    for attempt in 1..=6u32 {
        let strategy_delay = s.next_backoff(attempt).as_secs_f64();
        assert!(
            (delay - strategy_delay).abs() < 0.001,
            "attempt {}: adapter delay={} strategy={}",
            attempt,
            delay,
            strategy_delay
        );
        delay = (delay * 2.0).min(30.0);
    }
}

/// `should_retry` must return `true` for attempts ≤ max and `false` beyond.
#[test]
fn test_reconnection_should_retry_boundary() {
    let s = ReconnectionStrategy::new(5, Duration::from_secs(1), Duration::from_secs(30));
    assert!(s.should_retry(1));
    assert!(s.should_retry(5));
    assert!(!s.should_retry(6));
}

/// Zero-attempt strategy never allows a retry.
#[test]
fn test_reconnection_strategy_zero_max_attempts_never_retries() {
    let s = ReconnectionStrategy::new(0, Duration::from_secs(1), Duration::from_secs(30));
    assert!(!s.should_retry(1));
}

/// Backoff with a very large attempt number must not overflow and must cap.
#[test]
fn test_reconnection_backoff_overflow_safe() {
    let s = ReconnectionStrategy::new(100, Duration::from_millis(100), Duration::from_secs(30));
    // u64 overflow path — must return max_backoff, not panic
    let result = s.next_backoff(100);
    assert_eq!(result, Duration::from_secs(30));
}

// ============================================================================
// 4. Variable mapping — SimVar names must match MSFS API strings exactly
// ============================================================================

/// Kinematics SimVar names must match the official MSFS SimVar identifiers.
#[test]
fn test_kinematics_simvar_names_match_msfs_api() {
    let cfg = create_default_mapping();
    let kin = &cfg.default_mapping.kinematics;

    assert_eq!(kin.ias, "AIRSPEED INDICATED");
    assert_eq!(kin.tas, "AIRSPEED TRUE");
    assert_eq!(kin.ground_speed, "GROUND VELOCITY");
    assert_eq!(kin.aoa, "INCIDENCE ALPHA");
    assert_eq!(kin.sideslip, "INCIDENCE BETA");
    assert_eq!(kin.bank, "PLANE BANK DEGREES");
    assert_eq!(kin.pitch, "PLANE PITCH DEGREES");
    assert_eq!(kin.heading, "PLANE HEADING DEGREES MAGNETIC");
    assert_eq!(kin.g_force, "G FORCE");
    assert_eq!(kin.g_lateral, "ACCELERATION BODY X");
    assert_eq!(kin.g_longitudinal, "ACCELERATION BODY Z");
    assert_eq!(kin.mach, "AIRSPEED MACH");
    assert_eq!(kin.vertical_speed, "VERTICAL SPEED");
}

/// Aircraft configuration SimVar names must match the MSFS API.
#[test]
fn test_config_simvar_names_match_msfs_api() {
    let cfg = create_default_mapping();
    let c = &cfg.default_mapping.config;

    assert_eq!(c.gear_nose, "GEAR CENTER POSITION");
    assert_eq!(c.gear_left, "GEAR LEFT POSITION");
    assert_eq!(c.gear_right, "GEAR RIGHT POSITION");
    assert_eq!(c.flaps, "FLAPS HANDLE PERCENT");
    assert_eq!(c.spoilers, "SPOILERS HANDLE POSITION");
    assert_eq!(c.ap_master, "AUTOPILOT MASTER");
    assert_eq!(c.ap_altitude_hold, "AUTOPILOT ALTITUDE LOCK");
    assert_eq!(c.ap_heading_hold, "AUTOPILOT HEADING LOCK");
    assert_eq!(c.ap_speed_hold, "AUTOPILOT AIRSPEED HOLD");
    assert_eq!(c.ap_altitude, "AUTOPILOT ALTITUDE LOCK VAR");
    assert_eq!(c.ap_heading, "AUTOPILOT HEADING LOCK DIR");
    assert_eq!(c.ap_speed, "AUTOPILOT AIRSPEED HOLD VAR");
}

/// Environment SimVar names must match the MSFS API.
#[test]
fn test_environment_simvar_names_match_msfs_api() {
    let cfg = create_default_mapping();
    let e = &cfg.default_mapping.environment;

    assert_eq!(e.altitude, "INDICATED ALTITUDE");
    assert_eq!(e.pressure_altitude, "PRESSURE ALTITUDE");
    assert_eq!(e.oat, "AMBIENT TEMPERATURE");
    assert_eq!(e.wind_speed, "AMBIENT WIND VELOCITY");
    assert_eq!(e.wind_direction, "AMBIENT WIND DIRECTION");
}

/// Navigation SimVar names must match the MSFS API.
#[test]
fn test_navigation_simvar_names_match_msfs_api() {
    let cfg = create_default_mapping();
    let n = &cfg.default_mapping.navigation;

    assert_eq!(n.latitude, "PLANE LATITUDE");
    assert_eq!(n.longitude, "PLANE LONGITUDE");
    assert_eq!(n.ground_track, "GPS GROUND TRUE TRACK");
}

/// Default engine mapping uses indexed SimVar syntax with engine 1.
#[test]
fn test_engine_simvar_names_use_index_one() {
    let cfg = create_default_mapping();
    let eng = &cfg.default_mapping.engines[0];

    assert_eq!(eng.running, "GENERAL ENG COMBUSTION:1");
    assert_eq!(eng.rpm, "GENERAL ENG RPM:1");
    assert_eq!(eng.index, 0, "first engine mapping has index 0 (0-based)");
}

// ============================================================================
// 5. Snapshot construction and conversion
// ============================================================================

/// A freshly created `BusSnapshot` must have all validity flags false.
#[test]
fn test_new_snapshot_has_all_validity_flags_false() {
    let snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    assert!(!snap.validity.attitude_valid);
    assert!(!snap.validity.velocities_valid);
    assert!(!snap.validity.kinematics_valid);
    assert!(!snap.validity.position_valid);
    assert!(!snap.validity.aero_valid);
}

/// SimId and aircraft ICAO survive a round-trip through `BusSnapshot::new`.
#[test]
fn test_snapshot_carries_correct_sim_and_aircraft_ids() {
    let snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("A320"));
    assert_eq!(snap.sim, SimId::Msfs);
    assert_eq!(snap.aircraft.icao, "A320");
}

/// Populating kinematics fields from `MsfsConverter` produces a snapshot that
/// passes `validate()`.
#[test]
fn test_snapshot_validates_after_kinematics_populated() {
    let mut snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));

    snap.kinematics.ias = MsfsConverter::convert_ias(100.0).unwrap();
    snap.kinematics.tas = MsfsConverter::convert_tas(105.0).unwrap();
    snap.kinematics.ground_speed = MsfsConverter::convert_ground_speed(98.0).unwrap();
    snap.kinematics.pitch = MsfsConverter::convert_angle_degrees(5.0).unwrap();
    snap.kinematics.bank = MsfsConverter::convert_angle_degrees(2.0).unwrap();
    snap.kinematics.heading = MsfsConverter::convert_angle_degrees(90.0).unwrap();
    snap.kinematics.g_force = MsfsConverter::convert_g_force(1.0).unwrap();
    snap.kinematics.g_lateral = MsfsConverter::convert_g_force(0.0).unwrap();
    snap.kinematics.g_longitudinal = MsfsConverter::convert_g_force(0.0).unwrap();
    snap.kinematics.mach = MsfsConverter::convert_mach(0.15).unwrap();

    snap.validity.attitude_valid = true;
    snap.validity.velocities_valid = true;
    snap.validity.kinematics_valid = true;
    snap.validity.aero_valid = true;

    // Verify field values match converter output
    assert_eq!(snap.kinematics.ias.to_knots(), 100.0f32);
    assert_eq!(snap.kinematics.tas.to_knots(), 105.0f32);
    assert_eq!(snap.kinematics.pitch.to_degrees(), 5.0f32);
    assert_eq!(snap.kinematics.mach.value(), 0.15f32);

    assert!(snap.validate().is_ok(), "populated snapshot must validate");
}

/// The `position_valid` flag is controlled separately from kinematics.
#[test]
fn test_snapshot_position_valid_flag_is_independent() {
    let mut snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    assert!(!snap.validity.position_valid);
    snap.validity.position_valid = true;
    assert!(snap.validity.position_valid);
    // Toggling position_valid must not affect kinematics_valid
    assert!(!snap.validity.kinematics_valid);
}

// ============================================================================
// 6. Disconnect / connection-loss handling
// ============================================================================

/// `send_event` from a disconnected adapter must return a `NotConnected` error.
#[tokio::test]
async fn test_send_event_while_disconnected_returns_not_connected() {
    let mut adapter = MsfsAdapter::new(MsfsAdapterConfig::default()).unwrap();
    let result = adapter.send_event("AXIS_ELEVATOR_SET", Some(0)).await;
    assert!(
        result.is_err(),
        "send_event from Disconnected state must fail"
    );
    assert!(
        matches!(result.unwrap_err(), MsfsAdapterError::NotConnected),
        "error must be NotConnected"
    );
}

/// `handle_connection_loss` clears aircraft info, snapshot, and transitions to
/// Disconnected.
#[tokio::test]
async fn test_handle_connection_loss_clears_all_state() {
    let mut adapter = MsfsAdapter::new(MsfsAdapterConfig::default()).unwrap();
    adapter.handle_connection_loss().await;

    assert_eq!(adapter.state().await, AdapterState::Disconnected);
    assert!(adapter.current_aircraft().await.is_none());
    assert!(adapter.current_snapshot().await.is_none());
    assert!(!adapter.is_active().await);
}

/// Calling `handle_connection_loss` multiple times is idempotent.
#[tokio::test]
async fn test_handle_connection_loss_idempotent() {
    let mut adapter = MsfsAdapter::new(MsfsAdapterConfig::default()).unwrap();
    adapter.handle_connection_loss().await;
    adapter.handle_connection_loss().await;
    adapter.handle_connection_loss().await;

    assert_eq!(adapter.state().await, AdapterState::Disconnected);
    assert!(adapter.current_aircraft().await.is_none());
}

/// `handle_connection_loss` resets the session field so that no zombie session
/// persists (indirect: is_active should be false and no snapshot).
#[tokio::test]
async fn test_handle_connection_loss_makes_adapter_inactive() {
    let mut adapter = MsfsAdapter::new(MsfsAdapterConfig::default()).unwrap();
    adapter.handle_connection_loss().await;
    assert!(!adapter.is_active().await);
    assert!(adapter.current_snapshot().await.is_none());
}
