// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Depth tests for the AC7 telemetry adapter.
//!
//! These tests exercise conversion logic, validity flags, edge cases,
//! error paths, config behaviour, and async UDP round-trips.

use flight_ac7_protocol::{AC7_TELEMETRY_SCHEMA_V1, Ac7Controls, Ac7State, Ac7TelemetryPacket};
use flight_ac7_telemetry::{Ac7TelemetryAdapter, Ac7TelemetryConfig, Ac7TelemetryError};
use flight_adapter_common::{AdapterConfig, AdapterState};
use flight_bus::types::SimId;
use serde_json::json;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::UdpSocket;

// ── Helpers ──────────────────────────────────────────────────────────────

fn default_adapter() -> Ac7TelemetryAdapter {
    Ac7TelemetryAdapter::new(Ac7TelemetryConfig::default())
}

fn ephemeral_config() -> Ac7TelemetryConfig {
    Ac7TelemetryConfig {
        listen_addr: "127.0.0.1:0".parse().unwrap(),
        connection_timeout: Duration::from_secs(2),
        ..Default::default()
    }
}

fn full_packet() -> Ac7TelemetryPacket {
    Ac7TelemetryPacket::from_json_str(
        &json!({
            "schema": AC7_TELEMETRY_SCHEMA_V1,
            "timestamp_ms": 5000,
            "aircraft": "Su-57",
            "state": {
                "altitude_m": 3000.0,
                "speed_mps": 250.0,
                "ground_speed_mps": 240.0,
                "vertical_speed_mps": 5.0,
                "heading_deg": 45.0,
                "pitch_deg": 10.0,
                "roll_deg": -15.0,
                "g_force": 2.0
            },
            "controls": {
                "pitch": 0.3,
                "roll": -0.4,
                "yaw": 0.1,
                "throttle": 0.9
            }
        })
        .to_string(),
    )
    .expect("full_packet must parse")
}

fn minimal_packet() -> Ac7TelemetryPacket {
    Ac7TelemetryPacket {
        schema: AC7_TELEMETRY_SCHEMA_V1.to_string(),
        ..Default::default()
    }
}

async fn send_packet(target: SocketAddr, packet: &Ac7TelemetryPacket) {
    let sender = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let payload = packet.to_json_vec().unwrap();
    sender.send_to(&payload, target).await.unwrap();
}

// ═══════════════════════════════════════════════════════════════════════
// 1. Config / construction
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn default_config_has_sane_values() {
    let cfg = Ac7TelemetryConfig::default();
    assert_eq!(cfg.listen_addr.port(), 7779);
    assert!(cfg.bus_max_rate_hz > 0.0);
    assert!(cfg.update_rate_hz > 0.0);
    assert!(cfg.connection_timeout.as_secs() > 0);
    assert!(cfg.max_packet_size > 0);
}

#[test]
fn adapter_config_trait_delegates() {
    let cfg = Ac7TelemetryConfig::default();
    assert_eq!(cfg.publish_rate_hz(), cfg.update_rate_hz);
    assert_eq!(cfg.connection_timeout(), cfg.connection_timeout);
    assert_eq!(cfg.max_reconnect_attempts(), 0);
    assert!(!cfg.enable_auto_reconnect());
}

#[test]
fn custom_config_preserved() {
    let cfg = Ac7TelemetryConfig {
        listen_addr: "127.0.0.1:9999".parse().unwrap(),
        bus_max_rate_hz: 30.0,
        update_rate_hz: 30.0,
        connection_timeout: Duration::from_millis(500),
        max_packet_size: 8192,
    };
    let adapter = Ac7TelemetryAdapter::new(cfg);
    assert_eq!(adapter.state(), AdapterState::Disconnected);
}

#[test]
fn config_serialization_round_trip() {
    let cfg = Ac7TelemetryConfig::default();
    let json = serde_json::to_string(&cfg).unwrap();
    let restored: Ac7TelemetryConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.listen_addr, cfg.listen_addr);
    assert_eq!(restored.bus_max_rate_hz, cfg.bus_max_rate_hz);
}

// ═══════════════════════════════════════════════════════════════════════
// 2. Initial adapter state
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn new_adapter_is_disconnected() {
    let a = default_adapter();
    assert_eq!(a.state(), AdapterState::Disconnected);
    assert!(a.local_addr().is_none());
    assert!(a.source_addr().is_none());
}

#[test]
fn new_adapter_has_no_last_packet() {
    let a = default_adapter();
    assert!(a.time_since_last_packet().is_none());
}

#[test]
fn new_adapter_is_always_timed_out() {
    let a = default_adapter();
    assert!(a.is_connection_timeout());
}

#[test]
fn metrics_initially_zero() {
    let a = default_adapter();
    let m = a.metrics();
    assert_eq!(m.total_updates, 0);
    assert_eq!(m.aircraft_changes, 0);
}

// ═══════════════════════════════════════════════════════════════════════
// 3. Snapshot conversion — full packet
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn full_packet_sim_id() {
    let snap = default_adapter()
        .convert_packet_to_snapshot(&full_packet())
        .unwrap();
    assert_eq!(snap.sim, SimId::AceCombat7);
}

#[test]
fn full_packet_aircraft_id() {
    let snap = default_adapter()
        .convert_packet_to_snapshot(&full_packet())
        .unwrap();
    assert_eq!(snap.aircraft.icao, "Su-57");
}

#[test]
fn full_packet_heading_degrees() {
    let snap = default_adapter()
        .convert_packet_to_snapshot(&full_packet())
        .unwrap();
    assert!((snap.kinematics.heading.to_degrees() - 45.0).abs() < 1e-5);
}

#[test]
fn full_packet_pitch_degrees() {
    let snap = default_adapter()
        .convert_packet_to_snapshot(&full_packet())
        .unwrap();
    assert!((snap.kinematics.pitch.to_degrees() - 10.0).abs() < 1e-5);
}

#[test]
fn full_packet_bank_degrees() {
    let snap = default_adapter()
        .convert_packet_to_snapshot(&full_packet())
        .unwrap();
    assert!((snap.kinematics.bank.to_degrees() - (-15.0)).abs() < 1e-5);
}

#[test]
fn full_packet_ias_mps() {
    let snap = default_adapter()
        .convert_packet_to_snapshot(&full_packet())
        .unwrap();
    assert!((snap.kinematics.ias.value() - 250.0).abs() < 1e-5);
}

#[test]
fn full_packet_ground_speed_mps() {
    let snap = default_adapter()
        .convert_packet_to_snapshot(&full_packet())
        .unwrap();
    assert!((snap.kinematics.ground_speed.value() - 240.0).abs() < 1e-5);
}

#[test]
fn full_packet_vertical_speed_fpm() {
    let snap = default_adapter()
        .convert_packet_to_snapshot(&full_packet())
        .unwrap();
    // 5 m/s * 196.85 = 984.25 fpm
    let expected_fpm = 5.0 * 196.85;
    assert!((snap.kinematics.vertical_speed - expected_fpm).abs() < 0.1);
}

#[test]
fn full_packet_altitude_feet() {
    let snap = default_adapter()
        .convert_packet_to_snapshot(&full_packet())
        .unwrap();
    // 3000 m / 0.3048 ≈ 9842.52 ft
    let expected_ft = 3000.0 / 0.3048;
    assert!((snap.environment.altitude - expected_ft).abs() < 0.1);
}

#[test]
fn full_packet_g_force() {
    let snap = default_adapter()
        .convert_packet_to_snapshot(&full_packet())
        .unwrap();
    assert!((snap.kinematics.g_force.value() - 2.0).abs() < 1e-5);
}

#[test]
fn full_packet_control_inputs() {
    let snap = default_adapter()
        .convert_packet_to_snapshot(&full_packet())
        .unwrap();
    assert!((snap.control_inputs.pitch - 0.3).abs() < 1e-5);
    assert!((snap.control_inputs.roll - (-0.4)).abs() < 1e-5);
    assert!((snap.control_inputs.yaw - 0.1).abs() < 1e-5);
    assert_eq!(snap.control_inputs.throttle, vec![0.9]);
}

#[test]
fn full_packet_timestamp_nonzero() {
    let snap = default_adapter()
        .convert_packet_to_snapshot(&full_packet())
        .unwrap();
    assert!(snap.timestamp > 0);
}

// ═══════════════════════════════════════════════════════════════════════
// 4. Validity flags
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn full_packet_all_validity_flags_set() {
    let snap = default_adapter()
        .convert_packet_to_snapshot(&full_packet())
        .unwrap();
    assert!(snap.validity.attitude_valid);
    assert!(snap.validity.velocities_valid);
    assert!(snap.validity.position_valid);
    assert!(snap.validity.kinematics_valid);
    assert!(snap.validity.aero_valid);
    assert!(snap.validity.safe_for_ffb);
}

#[test]
fn minimal_packet_no_validity() {
    let snap = default_adapter()
        .convert_packet_to_snapshot(&minimal_packet())
        .unwrap();
    assert!(!snap.validity.attitude_valid);
    assert!(!snap.validity.velocities_valid);
    assert!(!snap.validity.position_valid);
    assert!(!snap.validity.kinematics_valid);
    assert!(!snap.validity.aero_valid);
    assert!(!snap.validity.safe_for_ffb);
}

#[test]
fn attitude_requires_both_pitch_and_roll() {
    let only_pitch = Ac7TelemetryPacket {
        state: Ac7State {
            pitch_deg: Some(5.0),
            ..Default::default()
        },
        ..Default::default()
    };
    let snap = default_adapter()
        .convert_packet_to_snapshot(&only_pitch)
        .unwrap();
    assert!(!snap.validity.attitude_valid);

    let only_roll = Ac7TelemetryPacket {
        state: Ac7State {
            roll_deg: Some(5.0),
            ..Default::default()
        },
        ..Default::default()
    };
    let snap = default_adapter()
        .convert_packet_to_snapshot(&only_roll)
        .unwrap();
    assert!(!snap.validity.attitude_valid);
}

#[test]
fn velocities_valid_with_only_speed() {
    let packet = Ac7TelemetryPacket {
        state: Ac7State {
            speed_mps: Some(100.0),
            ..Default::default()
        },
        ..Default::default()
    };
    let snap = default_adapter()
        .convert_packet_to_snapshot(&packet)
        .unwrap();
    assert!(snap.validity.velocities_valid);
}

#[test]
fn velocities_valid_with_only_ground_speed() {
    let packet = Ac7TelemetryPacket {
        state: Ac7State {
            ground_speed_mps: Some(100.0),
            ..Default::default()
        },
        ..Default::default()
    };
    let snap = default_adapter()
        .convert_packet_to_snapshot(&packet)
        .unwrap();
    assert!(snap.validity.velocities_valid);
}

#[test]
fn safe_for_ffb_requires_attitude_velocity_position() {
    // attitude + velocity but no position → not safe
    let packet = Ac7TelemetryPacket {
        state: Ac7State {
            pitch_deg: Some(0.0),
            roll_deg: Some(0.0),
            speed_mps: Some(100.0),
            ..Default::default()
        },
        ..Default::default()
    };
    let snap = default_adapter()
        .convert_packet_to_snapshot(&packet)
        .unwrap();
    assert!(!snap.validity.safe_for_ffb);
}

#[test]
fn safe_for_ffb_when_all_three_present() {
    let packet = Ac7TelemetryPacket {
        state: Ac7State {
            pitch_deg: Some(0.0),
            roll_deg: Some(0.0),
            speed_mps: Some(100.0),
            altitude_m: Some(500.0),
            ..Default::default()
        },
        ..Default::default()
    };
    let snap = default_adapter()
        .convert_packet_to_snapshot(&packet)
        .unwrap();
    assert!(snap.validity.safe_for_ffb);
}

#[test]
fn aero_valid_matches_attitude_valid() {
    let packet = Ac7TelemetryPacket {
        state: Ac7State {
            pitch_deg: Some(3.0),
            roll_deg: Some(-2.0),
            ..Default::default()
        },
        ..Default::default()
    };
    let snap = default_adapter()
        .convert_packet_to_snapshot(&packet)
        .unwrap();
    assert!(snap.validity.aero_valid);
    assert_eq!(snap.validity.aero_valid, snap.validity.attitude_valid);
}

// ═══════════════════════════════════════════════════════════════════════
// 5. Speed fallback: ground_speed inherits from speed when absent
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn ground_speed_falls_back_to_airspeed() {
    let packet = Ac7TelemetryPacket {
        state: Ac7State {
            speed_mps: Some(200.0),
            ..Default::default()
        },
        ..Default::default()
    };
    let snap = default_adapter()
        .convert_packet_to_snapshot(&packet)
        .unwrap();
    assert!((snap.kinematics.ground_speed.value() - 200.0).abs() < 1e-5);
}

#[test]
fn explicit_ground_speed_overrides_fallback() {
    let packet = Ac7TelemetryPacket {
        state: Ac7State {
            speed_mps: Some(200.0),
            ground_speed_mps: Some(190.0),
            ..Default::default()
        },
        ..Default::default()
    };
    let snap = default_adapter()
        .convert_packet_to_snapshot(&packet)
        .unwrap();
    assert!((snap.kinematics.ground_speed.value() - 190.0).abs() < 1e-5);
}

#[test]
fn ias_and_tas_same_from_speed_mps() {
    let packet = Ac7TelemetryPacket {
        state: Ac7State {
            speed_mps: Some(150.0),
            ..Default::default()
        },
        ..Default::default()
    };
    let snap = default_adapter()
        .convert_packet_to_snapshot(&packet)
        .unwrap();
    assert!((snap.kinematics.ias.value() - snap.kinematics.tas.value()).abs() < 1e-5);
}

// ═══════════════════════════════════════════════════════════════════════
// 6. Aircraft label edge cases
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn empty_aircraft_defaults_to_ac7() {
    let packet = Ac7TelemetryPacket {
        aircraft: String::new(),
        ..Default::default()
    };
    let snap = default_adapter()
        .convert_packet_to_snapshot(&packet)
        .unwrap();
    assert_eq!(snap.aircraft.icao, "AC7");
}

#[test]
fn whitespace_aircraft_defaults_to_ac7() {
    let packet = Ac7TelemetryPacket {
        aircraft: "   ".to_string(),
        ..Default::default()
    };
    let snap = default_adapter()
        .convert_packet_to_snapshot(&packet)
        .unwrap();
    assert_eq!(snap.aircraft.icao, "AC7");
}

#[test]
fn trimmed_aircraft_label_used() {
    let packet = Ac7TelemetryPacket {
        aircraft: "  F-22A  ".to_string(),
        ..Default::default()
    };
    let snap = default_adapter()
        .convert_packet_to_snapshot(&packet)
        .unwrap();
    assert_eq!(snap.aircraft.icao, "F-22A");
}

// ═══════════════════════════════════════════════════════════════════════
// 7. Heading normalization
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn heading_350_normalized() {
    let packet = Ac7TelemetryPacket {
        state: Ac7State {
            heading_deg: Some(350.0),
            ..Default::default()
        },
        ..Default::default()
    };
    let snap = default_adapter()
        .convert_packet_to_snapshot(&packet)
        .unwrap();
    // normalize_degrees_signed(350) = -10
    assert!((snap.kinematics.heading.to_degrees() - (-10.0)).abs() < 0.01);
}

#[test]
fn heading_negative_normalized() {
    let packet = Ac7TelemetryPacket {
        state: Ac7State {
            heading_deg: Some(-90.0),
            ..Default::default()
        },
        ..Default::default()
    };
    let snap = default_adapter()
        .convert_packet_to_snapshot(&packet)
        .unwrap();
    assert!((snap.kinematics.heading.to_degrees() - (-90.0)).abs() < 0.01);
}

#[test]
fn heading_zero_unchanged() {
    let packet = Ac7TelemetryPacket {
        state: Ac7State {
            heading_deg: Some(0.0),
            ..Default::default()
        },
        ..Default::default()
    };
    let snap = default_adapter()
        .convert_packet_to_snapshot(&packet)
        .unwrap();
    assert!((snap.kinematics.heading.to_degrees()).abs() < 1e-5);
}

#[test]
fn heading_180_normalizes_to_negative_180() {
    // normalize_degrees_signed(180) = -180
    let packet = Ac7TelemetryPacket {
        state: Ac7State {
            heading_deg: Some(180.0),
            ..Default::default()
        },
        ..Default::default()
    };
    let snap = default_adapter()
        .convert_packet_to_snapshot(&packet)
        .unwrap();
    assert!((snap.kinematics.heading.to_degrees() - (-180.0)).abs() < 0.01);
}

// ═══════════════════════════════════════════════════════════════════════
// 8. Partial control inputs
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn no_controls_leaves_defaults() {
    let snap = default_adapter()
        .convert_packet_to_snapshot(&minimal_packet())
        .unwrap();
    assert!((snap.control_inputs.pitch).abs() < 1e-5);
    assert!((snap.control_inputs.roll).abs() < 1e-5);
    assert!((snap.control_inputs.yaw).abs() < 1e-5);
    assert!(snap.control_inputs.throttle.is_empty());
}

#[test]
fn only_throttle_set() {
    let packet = Ac7TelemetryPacket {
        controls: Ac7Controls {
            throttle: Some(0.5),
            ..Default::default()
        },
        ..Default::default()
    };
    let snap = default_adapter()
        .convert_packet_to_snapshot(&packet)
        .unwrap();
    assert_eq!(snap.control_inputs.throttle, vec![0.5]);
    assert!((snap.control_inputs.pitch).abs() < 1e-5);
}

// ═══════════════════════════════════════════════════════════════════════
// 9. Boundary values for validated bus types
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn speed_at_upper_bus_limit() {
    // ValidatedSpeed::new_mps accepts 0..=500
    let packet = Ac7TelemetryPacket {
        state: Ac7State {
            speed_mps: Some(500.0),
            ..Default::default()
        },
        ..Default::default()
    };
    let snap = default_adapter()
        .convert_packet_to_snapshot(&packet)
        .unwrap();
    assert!((snap.kinematics.ias.value() - 500.0).abs() < 1e-5);
}

#[test]
fn speed_zero_is_valid() {
    let packet = Ac7TelemetryPacket {
        state: Ac7State {
            speed_mps: Some(0.0),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(
        default_adapter()
            .convert_packet_to_snapshot(&packet)
            .is_ok()
    );
}

#[test]
fn g_force_at_boundaries() {
    for g in [-20.0, 0.0, 1.0, 9.0, 20.0] {
        let packet = Ac7TelemetryPacket {
            state: Ac7State {
                g_force: Some(g),
                ..Default::default()
            },
            ..Default::default()
        };
        let snap = default_adapter()
            .convert_packet_to_snapshot(&packet)
            .unwrap();
        assert!((snap.kinematics.g_force.value() - g).abs() < 1e-5);
    }
}

#[test]
fn angle_at_boundaries() {
    for deg in [-180.0, -90.0, 0.0, 90.0, 180.0] {
        let packet = Ac7TelemetryPacket {
            state: Ac7State {
                pitch_deg: Some(deg),
                roll_deg: Some(deg),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(
            default_adapter()
                .convert_packet_to_snapshot(&packet)
                .is_ok()
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 10. Error display / variants
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn error_not_started_display() {
    let err = Ac7TelemetryError::NotStarted;
    assert_eq!(err.to_string(), "adapter is not started");
}

#[test]
fn error_invalid_field_display() {
    let err = Ac7TelemetryError::InvalidField {
        field: "state.speed_mps",
    };
    assert!(err.to_string().contains("state.speed_mps"));
}

// ═══════════════════════════════════════════════════════════════════════
// 11. Async lifecycle
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn start_transitions_to_connected() {
    let mut adapter = Ac7TelemetryAdapter::new(ephemeral_config());
    adapter.start().await.unwrap();
    assert_eq!(adapter.state(), AdapterState::Connected);
    assert!(adapter.local_addr().is_some());
}

#[tokio::test]
async fn stop_transitions_to_disconnected() {
    let mut adapter = Ac7TelemetryAdapter::new(ephemeral_config());
    adapter.start().await.unwrap();
    adapter.stop();
    assert_eq!(adapter.state(), AdapterState::Disconnected);
    assert!(adapter.local_addr().is_none());
}

#[tokio::test]
async fn start_stop_start_cycle() {
    let mut adapter = Ac7TelemetryAdapter::new(ephemeral_config());
    adapter.start().await.unwrap();
    let addr1 = adapter.local_addr().unwrap();
    adapter.stop();
    adapter.start().await.unwrap();
    let addr2 = adapter.local_addr().unwrap();
    // Different ephemeral port expected
    assert_ne!(addr1.port(), addr2.port());
}

#[tokio::test]
async fn poll_without_start_returns_not_started() {
    let mut adapter = Ac7TelemetryAdapter::new(ephemeral_config());
    let err = adapter.poll_once().await.unwrap_err();
    assert!(matches!(err, Ac7TelemetryError::NotStarted));
}

// ═══════════════════════════════════════════════════════════════════════
// 12. UDP round-trip tests
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn poll_receives_full_packet() {
    let mut adapter = Ac7TelemetryAdapter::new(ephemeral_config());
    adapter.start().await.unwrap();
    let target = adapter.local_addr().unwrap();

    send_packet(target, &full_packet()).await;

    let snap = adapter.poll_once().await.unwrap().unwrap();
    assert_eq!(snap.sim, SimId::AceCombat7);
    assert_eq!(snap.aircraft.icao, "Su-57");
}

#[tokio::test]
async fn poll_transitions_to_active() {
    let mut adapter = Ac7TelemetryAdapter::new(ephemeral_config());
    adapter.start().await.unwrap();
    let target = adapter.local_addr().unwrap();

    send_packet(target, &full_packet()).await;
    adapter.poll_once().await.unwrap();

    assert_eq!(adapter.state(), AdapterState::Active);
}

#[tokio::test]
async fn poll_records_source_addr() {
    let mut adapter = Ac7TelemetryAdapter::new(ephemeral_config());
    adapter.start().await.unwrap();
    let target = adapter.local_addr().unwrap();

    send_packet(target, &full_packet()).await;
    adapter.poll_once().await.unwrap();

    assert!(adapter.source_addr().is_some());
}

#[tokio::test]
async fn poll_updates_last_packet_time() {
    let mut adapter = Ac7TelemetryAdapter::new(ephemeral_config());
    adapter.start().await.unwrap();
    let target = adapter.local_addr().unwrap();

    send_packet(target, &full_packet()).await;
    adapter.poll_once().await.unwrap();

    assert!(adapter.time_since_last_packet().is_some());
    assert!(!adapter.is_connection_timeout());
}

#[tokio::test]
async fn poll_increments_metrics() {
    let mut adapter = Ac7TelemetryAdapter::new(ephemeral_config());
    adapter.start().await.unwrap();
    let target = adapter.local_addr().unwrap();

    send_packet(target, &full_packet()).await;
    adapter.poll_once().await.unwrap();

    let m = adapter.metrics();
    assert_eq!(m.total_updates, 1);
    assert_eq!(m.aircraft_changes, 1);
}

#[tokio::test]
async fn consecutive_polls_increment_metrics() {
    let mut adapter = Ac7TelemetryAdapter::new(ephemeral_config());
    adapter.start().await.unwrap();
    let target = adapter.local_addr().unwrap();

    for _ in 0..3 {
        send_packet(target, &full_packet()).await;
        adapter.poll_once().await.unwrap();
    }

    let m = adapter.metrics();
    assert_eq!(m.total_updates, 3);
    // Same aircraft each time, so changes stays at 1
    assert_eq!(m.aircraft_changes, 1);
}

#[tokio::test]
async fn aircraft_change_detected() {
    let mut adapter = Ac7TelemetryAdapter::new(ephemeral_config());
    adapter.start().await.unwrap();
    let target = adapter.local_addr().unwrap();

    send_packet(target, &full_packet()).await;
    adapter.poll_once().await.unwrap();

    let other = Ac7TelemetryPacket {
        aircraft: "F-15C".to_string(),
        ..full_packet()
    };
    // Re-validate to ensure consistent packet
    let other = Ac7TelemetryPacket::from_json_slice(&other.to_json_vec().unwrap()).unwrap();
    send_packet(target, &other).await;
    adapter.poll_once().await.unwrap();

    assert_eq!(adapter.metrics().aircraft_changes, 2);
}

#[tokio::test]
async fn poll_timeout_returns_adapter_error() {
    let mut adapter = Ac7TelemetryAdapter::new(Ac7TelemetryConfig {
        listen_addr: "127.0.0.1:0".parse().unwrap(),
        connection_timeout: Duration::from_millis(50),
        ..Default::default()
    });
    adapter.start().await.unwrap();
    // Don't send anything → timeout
    let err = adapter.poll_once().await.unwrap_err();
    assert!(matches!(err, Ac7TelemetryError::Adapter(_)));
}

// ═══════════════════════════════════════════════════════════════════════
// 13. Bus publisher access
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn bus_publisher_mut_accessible() {
    let mut adapter = default_adapter();
    let _pub = adapter.bus_publisher_mut();
}

#[test]
fn metrics_registry_accessible() {
    let adapter = default_adapter();
    let _reg = adapter.metrics_registry();
}
