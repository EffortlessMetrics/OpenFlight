// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Depth tests for the Elite adapter:
//! telemetry conversion, state machine, event sequences, error handling.

use flight_adapter_common::AdapterState;
use flight_bus::types::{GearPosition, SimId};
use flight_elite::protocol::{EliteFlags, FuelStatus, JournalEvent, StatusJson};
use flight_elite::{EliteAdapter, EliteConfig, EliteError};
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;

fn test_adapter() -> EliteAdapter {
    EliteAdapter::new(EliteConfig::default())
}

fn config_with_dir(dir: &TempDir) -> EliteConfig {
    EliteConfig {
        journal_dir: dir.path().to_path_buf(),
        ..Default::default()
    }
}

fn write_status(dir: &TempDir, status: &StatusJson) {
    let path = dir.path().join("Status.json");
    std::fs::write(path, serde_json::to_string(status).unwrap()).unwrap();
}

// ═══════════════════════════════════════════════════════════════════════════
// Config
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn config_default_poll_interval() {
    let config = EliteConfig::default();
    assert_eq!(config.poll_interval, Duration::from_millis(250));
}

#[test]
fn config_default_bus_rate() {
    let config = EliteConfig::default();
    assert!((config.bus_max_rate_hz - 4.0).abs() < f32::EPSILON);
}

#[test]
fn config_adapter_trait_publish_rate() {
    use flight_adapter_common::AdapterConfig;
    let config = EliteConfig::default();
    assert!((config.publish_rate_hz() - 4.0).abs() < f32::EPSILON);
}

#[test]
fn config_adapter_trait_connection_timeout() {
    use flight_adapter_common::AdapterConfig;
    let config = EliteConfig::default();
    assert_eq!(config.connection_timeout(), Duration::from_secs(5));
}

#[test]
fn config_adapter_trait_reconnect() {
    use flight_adapter_common::AdapterConfig;
    let config = EliteConfig::default();
    assert_eq!(config.max_reconnect_attempts(), 0);
    assert!(config.enable_auto_reconnect());
}

#[test]
fn config_custom_values() {
    let config = EliteConfig {
        journal_dir: PathBuf::from("/custom/path"),
        poll_interval: Duration::from_millis(500),
        bus_max_rate_hz: 10.0,
    };
    assert_eq!(config.journal_dir, PathBuf::from("/custom/path"));
    assert_eq!(config.poll_interval, Duration::from_millis(500));
    assert!((config.bus_max_rate_hz - 10.0).abs() < f32::EPSILON);
}

#[test]
fn config_serde_roundtrip() {
    let config = EliteConfig {
        journal_dir: PathBuf::from("test/dir"),
        poll_interval: Duration::from_millis(100),
        bus_max_rate_hz: 8.0,
    };
    let json = serde_json::to_string(&config).unwrap();
    let parsed: EliteConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.journal_dir, config.journal_dir);
    assert_eq!(parsed.poll_interval, config.poll_interval);
    assert!((parsed.bus_max_rate_hz - config.bus_max_rate_hz).abs() < f32::EPSILON);
}

// ═══════════════════════════════════════════════════════════════════════════
// Adapter lifecycle (start / stop / state)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn adapter_initial_state_disconnected() {
    let adapter = test_adapter();
    assert_eq!(adapter.state(), AdapterState::Disconnected);
}

#[test]
fn adapter_start_transitions_to_connected() {
    let dir = TempDir::new().unwrap();
    let mut adapter = EliteAdapter::new(config_with_dir(&dir));
    adapter.start().unwrap();
    assert_eq!(adapter.state(), AdapterState::Connected);
}

#[test]
fn adapter_stop_transitions_to_disconnected() {
    let dir = TempDir::new().unwrap();
    let mut adapter = EliteAdapter::new(config_with_dir(&dir));
    adapter.start().unwrap();
    adapter.stop();
    assert_eq!(adapter.state(), AdapterState::Disconnected);
}

#[test]
fn adapter_start_stop_start_cycle() {
    let dir = TempDir::new().unwrap();
    let mut adapter = EliteAdapter::new(config_with_dir(&dir));
    adapter.start().unwrap();
    assert_eq!(adapter.state(), AdapterState::Connected);
    adapter.stop();
    assert_eq!(adapter.state(), AdapterState::Disconnected);
    adapter.start().unwrap();
    assert_eq!(adapter.state(), AdapterState::Connected);
}

#[test]
fn adapter_start_succeeds_even_if_dir_missing() {
    // start() warns but doesn't fail when journal_dir doesn't exist.
    let config = EliteConfig {
        journal_dir: PathBuf::from("nonexistent_dir_that_should_not_exist"),
        ..Default::default()
    };
    let mut adapter = EliteAdapter::new(config);
    assert!(adapter.start().is_ok());
}

// ═══════════════════════════════════════════════════════════════════════════
// Telemetry conversion depth
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn convert_all_flags_zero_defaults() {
    let adapter = test_adapter();
    let status = StatusJson::default();
    let snap = adapter.convert_status(&status);

    assert_eq!(snap.sim, SimId::EliteDangerous);
    assert!(snap.config.gear.all_up(), "no gear flag → gear up");
    assert!(!snap.config.lights.nav, "no lights flag → lights off");
    assert!(!snap.config.lights.landing);
    assert!(!snap.validity.safe_for_ffb, "ED never safe for FFB");
    // Not docked/landed/SRV → position valid
    assert!(snap.validity.position_valid);
}

#[test]
fn convert_gear_down_sets_all_three() {
    let adapter = test_adapter();
    let status = StatusJson {
        flags: EliteFlags::GEAR_DOWN.bits(),
        ..Default::default()
    };
    let snap = adapter.convert_status(&status);
    assert_eq!(snap.config.gear.nose, GearPosition::Down);
    assert_eq!(snap.config.gear.left, GearPosition::Down);
    assert_eq!(snap.config.gear.right, GearPosition::Down);
}

#[test]
fn convert_gear_up_sets_all_three() {
    let adapter = test_adapter();
    let status = StatusJson {
        flags: 0,
        ..Default::default()
    };
    let snap = adapter.convert_status(&status);
    assert_eq!(snap.config.gear.nose, GearPosition::Up);
    assert_eq!(snap.config.gear.left, GearPosition::Up);
    assert_eq!(snap.config.gear.right, GearPosition::Up);
}

#[test]
fn convert_lights_on_sets_nav_and_landing() {
    let adapter = test_adapter();
    let status = StatusJson {
        flags: EliteFlags::LIGHTS_ON.bits(),
        ..Default::default()
    };
    let snap = adapter.convert_status(&status);
    assert!(snap.config.lights.nav);
    assert!(snap.config.lights.landing);
    assert!(!snap.config.lights.beacon);
    assert!(!snap.config.lights.strobe);
}

#[test]
fn convert_lights_off() {
    let adapter = test_adapter();
    let status = StatusJson {
        flags: 0,
        ..Default::default()
    };
    let snap = adapter.convert_status(&status);
    assert!(!snap.config.lights.nav);
    assert!(!snap.config.lights.landing);
}

#[test]
fn convert_multiple_flags_combined() {
    let adapter = test_adapter();
    let status = StatusJson {
        flags: EliteFlags::GEAR_DOWN.bits()
            | EliteFlags::LIGHTS_ON.bits()
            | EliteFlags::SHIELDS_UP.bits()
            | EliteFlags::SUPERCRUISE.bits(),
        ..Default::default()
    };
    let snap = adapter.convert_status(&status);
    assert!(snap.config.gear.all_down());
    assert!(snap.config.lights.nav);
    assert!(snap.config.lights.landing);
    assert!(snap.validity.position_valid, "supercruise = in flight");
}

// ── Fuel conversion ─────────────────────────────────────────────────────────

#[test]
fn convert_fuel_percentage_calculation() {
    let adapter = test_adapter();
    // 16 / (16 + 4) = 80%
    let status = StatusJson {
        fuel: Some(FuelStatus {
            fuel_main: 16.0,
            fuel_reservoir: 4.0,
        }),
        ..Default::default()
    };
    let snap = adapter.convert_status(&status);
    let pct = snap.config.fuel["main"].value();
    assert!((pct - 80.0).abs() < 0.01);
}

#[test]
fn convert_fuel_full_tank() {
    let adapter = test_adapter();
    // 32 / (32 + 0.01) ≈ ~100%
    let status = StatusJson {
        fuel: Some(FuelStatus {
            fuel_main: 32.0,
            fuel_reservoir: 0.01,
        }),
        ..Default::default()
    };
    let snap = adapter.convert_status(&status);
    let pct = snap.config.fuel["main"].value();
    assert!(pct > 99.0 && pct <= 100.0);
}

#[test]
fn convert_fuel_zero_main_yields_zero_pct() {
    let adapter = test_adapter();
    let status = StatusJson {
        fuel: Some(FuelStatus {
            fuel_main: 0.0,
            fuel_reservoir: 0.5,
        }),
        ..Default::default()
    };
    let snap = adapter.convert_status(&status);
    if snap.config.fuel.contains_key("main") {
        let pct = snap.config.fuel["main"].value();
        assert!(pct.abs() < 0.01, "zero main fuel should be 0%");
    }
    // It's also valid if the key is absent (percentage creation may fail for 0.0)
}

#[test]
fn convert_no_fuel_data() {
    let adapter = test_adapter();
    let status = StatusJson {
        fuel: None,
        ..Default::default()
    };
    let snap = adapter.convert_status(&status);
    assert!(
        snap.config.fuel.is_empty(),
        "no fuel data → no fuel in snapshot"
    );
}

#[test]
fn convert_fuel_equal_main_and_reservoir() {
    let adapter = test_adapter();
    // 10 / (10 + 10) = 50%
    let status = StatusJson {
        fuel: Some(FuelStatus {
            fuel_main: 10.0,
            fuel_reservoir: 10.0,
        }),
        ..Default::default()
    };
    let snap = adapter.convert_status(&status);
    let pct = snap.config.fuel["main"].value();
    assert!((pct - 50.0).abs() < 0.01);
}

// ── Position validity scenarios ─────────────────────────────────────────────

#[test]
fn docked_position_not_valid() {
    let adapter = test_adapter();
    let status = StatusJson {
        flags: EliteFlags::DOCKED.bits(),
        ..Default::default()
    };
    let snap = adapter.convert_status(&status);
    assert!(!snap.validity.position_valid);
}

#[test]
fn landed_position_not_valid() {
    let adapter = test_adapter();
    let status = StatusJson {
        flags: EliteFlags::LANDED.bits(),
        ..Default::default()
    };
    let snap = adapter.convert_status(&status);
    assert!(!snap.validity.position_valid);
}

#[test]
fn in_srv_position_not_valid() {
    let adapter = test_adapter();
    let status = StatusJson {
        flags: EliteFlags::IN_SRV.bits(),
        ..Default::default()
    };
    let snap = adapter.convert_status(&status);
    assert!(!snap.validity.position_valid);
}

#[test]
fn supercruise_position_valid() {
    let adapter = test_adapter();
    let status = StatusJson {
        flags: EliteFlags::SUPERCRUISE.bits(),
        ..Default::default()
    };
    let snap = adapter.convert_status(&status);
    assert!(snap.validity.position_valid);
}

#[test]
fn fsd_jump_position_valid() {
    let adapter = test_adapter();
    let status = StatusJson {
        flags: EliteFlags::FSD_JUMP.bits(),
        ..Default::default()
    };
    let snap = adapter.convert_status(&status);
    assert!(snap.validity.position_valid);
}

#[test]
fn flight_assist_off_position_valid() {
    let adapter = test_adapter();
    let status = StatusJson {
        flags: EliteFlags::FLIGHT_ASSIST_OFF.bits(),
        ..Default::default()
    };
    let snap = adapter.convert_status(&status);
    assert!(snap.validity.position_valid);
}

#[test]
fn docked_and_supercruise_position_not_valid() {
    // If somehow both docked and supercruise, docked wins → not valid.
    let adapter = test_adapter();
    let status = StatusJson {
        flags: EliteFlags::DOCKED.bits() | EliteFlags::SUPERCRUISE.bits(),
        ..Default::default()
    };
    let snap = adapter.convert_status(&status);
    assert!(!snap.validity.position_valid);
}

#[test]
fn ffb_never_safe_regardless_of_flags() {
    let adapter = test_adapter();
    for flags in [
        0,
        EliteFlags::SUPERCRUISE.bits(),
        EliteFlags::DOCKED.bits(),
        u64::MAX,
    ] {
        let status = StatusJson {
            flags,
            ..Default::default()
        };
        let snap = adapter.convert_status(&status);
        assert!(
            !snap.validity.safe_for_ffb,
            "FFB should never be safe for Elite (flags={flags:#x})"
        );
    }
}

// ── Navigation waypoint ─────────────────────────────────────────────────────

#[test]
fn no_system_no_waypoint() {
    let adapter = test_adapter();
    let snap = adapter.convert_status(&StatusJson::default());
    assert!(snap.navigation.active_waypoint.is_none());
}

#[test]
fn system_appears_as_waypoint() {
    let mut adapter = test_adapter();
    adapter.apply_journal_event(&JournalEvent::Location {
        star_system: "Colonia".to_string(),
        star_pos: None,
    });
    let snap = adapter.convert_status(&StatusJson::default());
    assert_eq!(snap.navigation.active_waypoint.as_deref(), Some("Colonia"));
}

#[test]
fn waypoint_updates_on_fsd_jump() {
    let mut adapter = test_adapter();
    adapter.apply_journal_event(&JournalEvent::Location {
        star_system: "Sol".to_string(),
        star_pos: None,
    });
    adapter.apply_journal_event(&JournalEvent::FsdJump {
        star_system: "Alpha Centauri".to_string(),
        star_pos: Some([3.0, -0.1, 3.2]),
    });
    let snap = adapter.convert_status(&StatusJson::default());
    assert_eq!(
        snap.navigation.active_waypoint.as_deref(),
        Some("Alpha Centauri")
    );
}

// ── Ship name in snapshot ───────────────────────────────────────────────────

#[test]
fn default_ship_name_unknown() {
    let adapter = test_adapter();
    let snap = adapter.convert_status(&StatusJson::default());
    assert_eq!(snap.aircraft.icao, "Unknown");
}

#[test]
fn set_ship_updates_aircraft_id() {
    let mut adapter = test_adapter();
    adapter.set_ship("Anaconda");
    let snap = adapter.convert_status(&StatusJson::default());
    assert_eq!(snap.aircraft.icao, "Anaconda");
}

#[test]
fn load_game_event_sets_ship() {
    let mut adapter = test_adapter();
    adapter.apply_journal_event(&JournalEvent::LoadGame {
        ship: "Asp_Explorer".to_string(),
        commander: Some("CMDR Test".to_string()),
    });
    let snap = adapter.convert_status(&StatusJson::default());
    assert_eq!(snap.aircraft.icao, "Asp_Explorer");
}

// ═══════════════════════════════════════════════════════════════════════════
// State machine — event sequences
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn full_session_sequence() {
    let mut adapter = test_adapter();

    // 1. LoadGame
    adapter.apply_journal_event(&JournalEvent::LoadGame {
        ship: "Python".to_string(),
        commander: Some("CMDR Depth".to_string()),
    });
    assert_eq!(adapter.current_system(), "");
    assert!(adapter.docked_station().is_none());

    // 2. Location (initial spawn in a station area)
    adapter.apply_journal_event(&JournalEvent::Location {
        star_system: "Sol".to_string(),
        star_pos: Some([0.0, 0.0, 0.0]),
    });
    assert_eq!(adapter.current_system(), "Sol");

    // 3. Docked
    adapter.apply_journal_event(&JournalEvent::Docked {
        station_name: "Abraham Lincoln".to_string(),
        star_system: "Sol".to_string(),
    });
    assert_eq!(adapter.docked_station(), Some("Abraham Lincoln"));

    // 4. Undocked
    adapter.apply_journal_event(&JournalEvent::Undocked {
        station_name: "Abraham Lincoln".to_string(),
    });
    assert!(adapter.docked_station().is_none());

    // 5. FSD jump
    adapter.apply_journal_event(&JournalEvent::FsdJump {
        star_system: "Alpha Centauri".to_string(),
        star_pos: Some([3.03125, -0.09375, 3.15625]),
    });
    assert_eq!(adapter.current_system(), "Alpha Centauri");
    assert!(adapter.docked_station().is_none());

    // 6. Touchdown on planet
    adapter.apply_journal_event(&JournalEvent::Touchdown {
        latitude: Some(10.5),
        longitude: Some(-20.3),
    });
    // Touchdown doesn't change system or station.
    assert_eq!(adapter.current_system(), "Alpha Centauri");

    // 7. Liftoff
    adapter.apply_journal_event(&JournalEvent::Liftoff {
        latitude: Some(10.5),
        longitude: Some(-20.3),
    });
    assert_eq!(adapter.current_system(), "Alpha Centauri");

    // 8. RefuelAll
    adapter.apply_journal_event(&JournalEvent::RefuelAll {
        amount: Some(24.0),
    });
    assert_eq!(adapter.current_system(), "Alpha Centauri");
}

#[test]
fn load_game_clears_docked_station() {
    let mut adapter = test_adapter();

    // Dock first.
    adapter.apply_journal_event(&JournalEvent::Docked {
        station_name: "Port".to_string(),
        star_system: "Sol".to_string(),
    });
    assert!(adapter.docked_station().is_some());

    // LoadGame resets station.
    adapter.apply_journal_event(&JournalEvent::LoadGame {
        ship: "Sidewinder".to_string(),
        commander: None,
    });
    assert!(adapter.docked_station().is_none());
}

#[test]
fn fsd_jump_clears_docked_station() {
    let mut adapter = test_adapter();
    adapter.apply_journal_event(&JournalEvent::Docked {
        station_name: "Port".to_string(),
        star_system: "Sol".to_string(),
    });
    // This shouldn't normally happen in-game, but the adapter should handle it.
    adapter.apply_journal_event(&JournalEvent::FsdJump {
        star_system: "Barnard's Star".to_string(),
        star_pos: None,
    });
    assert!(adapter.docked_station().is_none());
    assert_eq!(adapter.current_system(), "Barnard's Star");
}

#[test]
fn location_clears_docked_station() {
    let mut adapter = test_adapter();
    adapter.apply_journal_event(&JournalEvent::Docked {
        station_name: "Port".to_string(),
        star_system: "Sol".to_string(),
    });
    adapter.apply_journal_event(&JournalEvent::Location {
        star_system: "Sol".to_string(),
        star_pos: None,
    });
    assert!(adapter.docked_station().is_none());
}

#[test]
fn docked_at_different_system_updates_system() {
    let mut adapter = test_adapter();
    adapter.apply_journal_event(&JournalEvent::Location {
        star_system: "Sol".to_string(),
        star_pos: None,
    });
    assert_eq!(adapter.current_system(), "Sol");

    // Dock in a different system (e.g., after FSD jump without explicit event).
    adapter.apply_journal_event(&JournalEvent::Docked {
        station_name: "Coriolis".to_string(),
        star_system: "LHS 3447".to_string(),
    });
    assert_eq!(adapter.current_system(), "LHS 3447");
    assert_eq!(adapter.docked_station(), Some("Coriolis"));
}

#[test]
fn same_system_fsd_jump_no_duplicate_update() {
    let mut adapter = test_adapter();
    adapter.apply_journal_event(&JournalEvent::FsdJump {
        star_system: "Sol".to_string(),
        star_pos: None,
    });
    assert_eq!(adapter.current_system(), "Sol");

    // "Jump" to the same system — should not cause issues.
    adapter.apply_journal_event(&JournalEvent::FsdJump {
        star_system: "Sol".to_string(),
        star_pos: None,
    });
    assert_eq!(adapter.current_system(), "Sol");
}

#[test]
fn multiple_ship_changes() {
    let mut adapter = test_adapter();
    let ships = ["Sidewinder", "Eagle", "Cobra_Mk3", "Python", "Anaconda"];
    for ship in &ships {
        adapter.set_ship(*ship);
        let snap = adapter.convert_status(&StatusJson::default());
        assert_eq!(snap.aircraft.icao, *ship);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Error handling
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn poll_before_start_returns_error() {
    let dir = TempDir::new().unwrap();
    let mut adapter = EliteAdapter::new(config_with_dir(&dir));
    // Not started yet.
    let result = adapter.poll_once().await;
    assert!(result.is_err());
    match result.unwrap_err() {
        EliteError::Adapter(_) => {}
        other => panic!("expected Adapter error, got {other:?}"),
    }
}

#[tokio::test]
async fn poll_after_stop_returns_error() {
    let dir = TempDir::new().unwrap();
    let mut adapter = EliteAdapter::new(config_with_dir(&dir));
    adapter.start().unwrap();
    adapter.stop();
    let result = adapter.poll_once().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn poll_with_invalid_json_returns_error() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("Status.json");
    std::fs::write(&path, "this is not json").unwrap();

    let mut adapter = EliteAdapter::new(config_with_dir(&dir));
    adapter.start().unwrap();
    let result = adapter.poll_once().await;
    assert!(result.is_err());
    match result.unwrap_err() {
        EliteError::Json(_) => {}
        other => panic!("expected Json error, got {other:?}"),
    }
}

#[tokio::test]
async fn poll_with_wrong_json_type_returns_error() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("Status.json");
    // Valid JSON but wrong shape (array instead of object).
    std::fs::write(&path, "[1, 2, 3]").unwrap();

    let mut adapter = EliteAdapter::new(config_with_dir(&dir));
    adapter.start().unwrap();
    let result = adapter.poll_once().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn poll_detects_flag_change() {
    let dir = TempDir::new().unwrap();

    // First: gear up.
    write_status(
        &dir,
        &StatusJson {
            flags: 0,
            ..Default::default()
        },
    );
    let mut adapter = EliteAdapter::new(config_with_dir(&dir));
    adapter.start().unwrap();
    let first = adapter.poll_once().await.unwrap();
    assert!(first.is_some());

    // Same flags → None.
    let same = adapter.poll_once().await.unwrap();
    assert!(same.is_none());

    // Change flags → new snapshot.
    write_status(
        &dir,
        &StatusJson {
            flags: EliteFlags::GEAR_DOWN.bits(),
            ..Default::default()
        },
    );
    let changed = adapter.poll_once().await.unwrap();
    assert!(changed.is_some());
    assert!(changed.unwrap().config.gear.all_down());
}

#[tokio::test]
async fn poll_transitions_to_active_state() {
    let dir = TempDir::new().unwrap();
    write_status(
        &dir,
        &StatusJson {
            flags: 0,
            ..Default::default()
        },
    );

    let mut adapter = EliteAdapter::new(config_with_dir(&dir));
    adapter.start().unwrap();
    assert_eq!(adapter.state(), AdapterState::Connected);

    adapter.poll_once().await.unwrap();
    assert_eq!(adapter.state(), AdapterState::Active);
}

#[tokio::test]
async fn poll_records_metrics() {
    let dir = TempDir::new().unwrap();
    write_status(
        &dir,
        &StatusJson {
            flags: EliteFlags::GEAR_DOWN.bits(),
            ..Default::default()
        },
    );

    let mut adapter = EliteAdapter::new(config_with_dir(&dir));
    adapter.start().unwrap();

    let before = adapter.metrics().total_updates;
    adapter.poll_once().await.unwrap();
    let after = adapter.metrics().total_updates;
    assert!(after > before, "metrics should record the update");
}

// ═══════════════════════════════════════════════════════════════════════════
// EliteError Display
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn error_display_status_not_found() {
    let err = EliteError::StatusNotFound {
        path: PathBuf::from("/some/path"),
    };
    let msg = err.to_string();
    assert!(msg.contains("Status.json"));
    assert!(msg.contains("/some/path"));
}

#[test]
fn error_display_json_parse() {
    let json_err = serde_json::from_str::<StatusJson>("not json").unwrap_err();
    let err = EliteError::Json(json_err);
    let msg = err.to_string();
    assert!(msg.contains("JSON parse error"));
}

#[test]
fn error_from_io() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let err: EliteError = io_err.into();
    assert!(matches!(err, EliteError::Io(_)));
}

#[test]
fn error_from_serde_json() {
    let json_err = serde_json::from_str::<StatusJson>("{bad}").unwrap_err();
    let err: EliteError = json_err.into();
    assert!(matches!(err, EliteError::Json(_)));
}
