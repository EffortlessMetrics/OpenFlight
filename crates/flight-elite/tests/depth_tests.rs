// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Depth tests for the `flight-elite` crate (Elite: Dangerous integration).
//!
//! Covers protocol parsing, status conversion, journal reading, adapter
//! lifecycle, and edge-case handling across all public API surfaces.

use flight_elite::journal::JournalReader;
use flight_elite::protocol::{
    EliteFlags, FuelStatus, JournalEvent, StatusJson, parse_journal_line,
};
use flight_elite::{EliteAdapter, EliteConfig, EliteError};

use flight_adapter_common::{AdapterConfig, AdapterState};
use flight_bus::types::{GearPosition, SimId};

use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

fn make_adapter() -> EliteAdapter {
    EliteAdapter::new(EliteConfig::default())
}

fn status_with_flags(bits: u64) -> StatusJson {
    StatusJson {
        flags: bits,
        ..Default::default()
    }
}

// ===========================================================================
// 1. EliteFlags — bitfield logic
// ===========================================================================

#[test]
fn flags_contains_single_bit() {
    let f = EliteFlags::from_bits_truncate(EliteFlags::SHIELDS_UP.bits());
    assert!(f.contains(EliteFlags::SHIELDS_UP));
    assert!(!f.contains(EliteFlags::DOCKED));
}

#[test]
fn flags_contains_multiple_bits() {
    let bits = EliteFlags::GEAR_DOWN.bits() | EliteFlags::LIGHTS_ON.bits();
    let f = EliteFlags::from_bits_truncate(bits);
    assert!(f.contains(EliteFlags::GEAR_DOWN));
    assert!(f.contains(EliteFlags::LIGHTS_ON));
    assert!(!f.contains(EliteFlags::SUPERCRUISE));
}

#[test]
fn flags_zero_contains_nothing() {
    let f = EliteFlags::from_bits_truncate(0);
    assert!(!f.contains(EliteFlags::DOCKED));
    assert!(!f.contains(EliteFlags::LANDED));
    assert!(!f.contains(EliteFlags::GEAR_DOWN));
    assert!(!f.contains(EliteFlags::LIGHTS_ON));
    assert!(!f.contains(EliteFlags::SUPERCRUISE));
    assert!(!f.contains(EliteFlags::IN_SRV));
}

#[test]
fn flags_all_known_bits_round_trip() {
    let all_known = [
        EliteFlags::DOCKED,
        EliteFlags::LANDED,
        EliteFlags::GEAR_DOWN,
        EliteFlags::SHIELDS_UP,
        EliteFlags::SUPERCRUISE,
        EliteFlags::FLIGHT_ASSIST_OFF,
        EliteFlags::HARDPOINTS_DEPLOYED,
        EliteFlags::IN_WING,
        EliteFlags::LIGHTS_ON,
        EliteFlags::CARGO_SCOOP,
        EliteFlags::SILENT_RUNNING,
        EliteFlags::SCOOPING_FUEL,
        EliteFlags::IN_SRV,
        EliteFlags::FSD_JUMP,
    ];
    let combined: u64 = all_known.iter().map(|f| f.bits()).fold(0, |a, b| a | b);
    let f = EliteFlags::from_bits_truncate(combined);
    for flag in &all_known {
        assert!(
            f.contains(*flag),
            "missing flag with bit {:#x}",
            flag.bits()
        );
    }
}

#[test]
fn flags_unknown_bits_are_preserved() {
    let exotic = 1u64 << 50;
    let f = EliteFlags::from_bits_truncate(exotic | EliteFlags::DOCKED.bits());
    assert!(f.contains(EliteFlags::DOCKED));
    assert_eq!(f.bits(), exotic | EliteFlags::DOCKED.bits());
}

// ===========================================================================
// 2. StatusJson — serde round-trip and edge cases
// ===========================================================================

#[test]
fn status_json_default_has_zero_flags() {
    let s = StatusJson::default();
    assert_eq!(s.flags, 0);
    assert!(s.fuel.is_none());
    assert!(s.pips.is_none());
    assert!(s.cargo.is_none());
    assert!(s.legal_state.is_none());
}

#[test]
fn status_json_serde_round_trip() {
    let original = StatusJson {
        schema_version: Some(4),
        event: Some("Status".to_string()),
        flags: EliteFlags::GEAR_DOWN.bits() | EliteFlags::SHIELDS_UP.bits(),
        pips: Some([4, 4, 4]),
        fire_group: Some(1),
        gui_focus: Some(0),
        fuel: Some(FuelStatus {
            fuel_main: 20.0,
            fuel_reservoir: 0.67,
        }),
        cargo: Some(12.0),
        legal_state: Some("Clean".to_string()),
    };
    let json = serde_json::to_string(&original).unwrap();
    let parsed: StatusJson = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.flags, original.flags);
    assert_eq!(parsed.pips, original.pips);
    assert_eq!(parsed.fire_group, original.fire_group);
    assert_eq!(parsed.fuel.as_ref().map(|f| f.fuel_main), Some(20.0));
    assert_eq!(parsed.legal_state.as_deref(), Some("Clean"));
}

#[test]
fn status_json_extra_fields_ignored() {
    let raw = r#"{"Flags":4,"ExtraField":"should be ignored","AnotherOne":999}"#;
    let s: StatusJson = serde_json::from_str(raw).unwrap();
    assert_eq!(s.flags, 4);
}

#[test]
fn status_json_empty_object_uses_defaults() {
    let s: StatusJson = serde_json::from_str("{}").unwrap();
    assert_eq!(s.flags, 0);
    assert!(s.fuel.is_none());
}

// ===========================================================================
// 3. JournalEvent — parsing coverage
// ===========================================================================

#[test]
fn parse_docked_event() {
    let line = r#"{"timestamp":"2025-01-01T00:00:00Z","event":"Docked","StationName":"Coriolis Hub","StarSystem":"Alpha Centauri"}"#;
    match parse_journal_line(line) {
        Some(JournalEvent::Docked {
            station_name,
            star_system,
        }) => {
            assert_eq!(station_name, "Coriolis Hub");
            assert_eq!(star_system, "Alpha Centauri");
        }
        other => panic!("expected Docked, got {other:?}"),
    }
}

#[test]
fn parse_undocked_event() {
    let line =
        r#"{"timestamp":"2025-01-01T00:00:00Z","event":"Undocked","StationName":"Coriolis Hub"}"#;
    match parse_journal_line(line) {
        Some(JournalEvent::Undocked { station_name }) => {
            assert_eq!(station_name, "Coriolis Hub");
        }
        other => panic!("expected Undocked, got {other:?}"),
    }
}

#[test]
fn parse_liftoff_with_coordinates() {
    let line = r#"{"timestamp":"2025-01-01T00:00:00Z","event":"Liftoff","Latitude":23.5,"Longitude":-45.2}"#;
    match parse_journal_line(line) {
        Some(JournalEvent::Liftoff {
            latitude,
            longitude,
        }) => {
            assert!((latitude.unwrap() - 23.5).abs() < 0.01);
            assert!((longitude.unwrap() - (-45.2)).abs() < 0.01);
        }
        other => panic!("expected Liftoff, got {other:?}"),
    }
}

#[test]
fn parse_touchdown_with_coordinates() {
    let line = r#"{"timestamp":"2025-01-01T00:00:00Z","event":"Touchdown","Latitude":-10.0,"Longitude":120.0}"#;
    match parse_journal_line(line) {
        Some(JournalEvent::Touchdown {
            latitude,
            longitude,
        }) => {
            assert!((latitude.unwrap() - (-10.0)).abs() < 0.01);
            assert!((longitude.unwrap() - 120.0).abs() < 0.01);
        }
        other => panic!("expected Touchdown, got {other:?}"),
    }
}

#[test]
fn parse_refuel_all_without_amount() {
    let line = r#"{"timestamp":"2025-01-01T00:00:00Z","event":"RefuelAll","Cost":500}"#;
    match parse_journal_line(line) {
        Some(JournalEvent::RefuelAll { amount }) => {
            assert!(amount.is_none());
        }
        other => panic!("expected RefuelAll, got {other:?}"),
    }
}

#[test]
fn parse_location_without_star_pos() {
    let line = r#"{"timestamp":"2025-01-01T00:00:00Z","event":"Location","StarSystem":"Achenar"}"#;
    match parse_journal_line(line) {
        Some(JournalEvent::Location {
            star_system,
            star_pos,
        }) => {
            assert_eq!(star_system, "Achenar");
            assert!(star_pos.is_none());
        }
        other => panic!("expected Location, got {other:?}"),
    }
}

#[test]
fn empty_line_returns_none() {
    assert!(parse_journal_line("").is_none());
}

#[test]
fn garbage_json_returns_none() {
    assert!(parse_journal_line("{invalid json!!!!").is_none());
}

#[test]
fn array_json_returns_none() {
    assert!(parse_journal_line("[1,2,3]").is_none());
}

// ===========================================================================
// 4. convert_status — snapshot mapping
// ===========================================================================

#[test]
fn snapshot_sim_id_always_elite() {
    let adapter = make_adapter();
    let snap = adapter.convert_status(&StatusJson::default());
    assert_eq!(snap.sim, SimId::EliteDangerous);
}

#[test]
fn snapshot_gear_down_sets_all_three() {
    let adapter = make_adapter();
    let snap = adapter.convert_status(&status_with_flags(EliteFlags::GEAR_DOWN.bits()));
    assert_eq!(snap.config.gear.nose, GearPosition::Down);
    assert_eq!(snap.config.gear.left, GearPosition::Down);
    assert_eq!(snap.config.gear.right, GearPosition::Down);
}

#[test]
fn snapshot_gear_up_sets_all_three() {
    let adapter = make_adapter();
    let snap = adapter.convert_status(&status_with_flags(0));
    assert_eq!(snap.config.gear.nose, GearPosition::Up);
    assert_eq!(snap.config.gear.left, GearPosition::Up);
    assert_eq!(snap.config.gear.right, GearPosition::Up);
}

#[test]
fn snapshot_lights_off_by_default() {
    let adapter = make_adapter();
    let snap = adapter.convert_status(&status_with_flags(0));
    assert!(!snap.config.lights.nav);
    assert!(!snap.config.lights.landing);
    assert!(!snap.config.lights.beacon);
    assert!(!snap.config.lights.strobe);
}

#[test]
fn snapshot_lights_on_sets_nav_and_landing() {
    let adapter = make_adapter();
    let snap = adapter.convert_status(&status_with_flags(EliteFlags::LIGHTS_ON.bits()));
    assert!(snap.config.lights.nav);
    assert!(snap.config.lights.landing);
    // beacon and strobe are always off for Elite
    assert!(!snap.config.lights.beacon);
    assert!(!snap.config.lights.strobe);
}

#[test]
fn snapshot_fuel_percentage_calculation() {
    let adapter = make_adapter();
    let status = StatusJson {
        fuel: Some(FuelStatus {
            fuel_main: 30.0,
            fuel_reservoir: 10.0,
        }),
        ..Default::default()
    };
    let snap = adapter.convert_status(&status);
    let pct = snap.config.fuel["main"].value();
    // 30 / (30 + 10) = 75%
    assert!((pct - 75.0).abs() < 0.01, "expected 75%, got {pct}");
}

#[test]
fn snapshot_fuel_zero_main_yields_zero_pct() {
    let adapter = make_adapter();
    let status = StatusJson {
        fuel: Some(FuelStatus {
            fuel_main: 0.0,
            fuel_reservoir: 1.0,
        }),
        ..Default::default()
    };
    let snap = adapter.convert_status(&status);
    let pct = snap.config.fuel["main"].value();
    assert!((pct - 0.0).abs() < 0.01, "expected 0%, got {pct}");
}

#[test]
fn snapshot_no_fuel_field_means_no_fuel_key() {
    let adapter = make_adapter();
    let snap = adapter.convert_status(&StatusJson::default());
    assert!(
        !snap.config.fuel.contains_key("main"),
        "fuel should be absent when StatusJson has no fuel"
    );
}

#[test]
fn snapshot_ffb_always_unsafe() {
    // ED provides no attitude data, so FFB is never safe.
    let adapter = make_adapter();
    for bits in [0, EliteFlags::SUPERCRUISE.bits(), EliteFlags::DOCKED.bits()] {
        let snap = adapter.convert_status(&status_with_flags(bits));
        assert!(
            !snap.validity.safe_for_ffb,
            "FFB should never be safe for ED"
        );
    }
}

#[test]
fn snapshot_position_valid_in_flight() {
    let adapter = make_adapter();
    // No DOCKED / LANDED / IN_SRV → in flight
    let snap = adapter.convert_status(&status_with_flags(EliteFlags::SUPERCRUISE.bits()));
    assert!(snap.validity.position_valid);
}

#[test]
fn snapshot_position_invalid_when_docked() {
    let adapter = make_adapter();
    let snap = adapter.convert_status(&status_with_flags(EliteFlags::DOCKED.bits()));
    assert!(!snap.validity.position_valid);
}

#[test]
fn snapshot_position_invalid_when_landed() {
    let adapter = make_adapter();
    let snap = adapter.convert_status(&status_with_flags(EliteFlags::LANDED.bits()));
    assert!(!snap.validity.position_valid);
}

#[test]
fn snapshot_position_invalid_when_in_srv() {
    let adapter = make_adapter();
    let snap = adapter.convert_status(&status_with_flags(EliteFlags::IN_SRV.bits()));
    assert!(!snap.validity.position_valid);
}

#[test]
fn snapshot_waypoint_from_current_system() {
    let mut adapter = make_adapter();
    adapter.apply_journal_event(&JournalEvent::FsdJump {
        star_system: "Beagle Point".to_string(),
        star_pos: None,
    });
    let snap = adapter.convert_status(&StatusJson::default());
    assert_eq!(
        snap.navigation.active_waypoint.as_deref(),
        Some("Beagle Point")
    );
}

#[test]
fn snapshot_no_waypoint_when_system_unknown() {
    let adapter = make_adapter();
    let snap = adapter.convert_status(&StatusJson::default());
    assert!(snap.navigation.active_waypoint.is_none());
}

#[test]
fn snapshot_aircraft_id_reflects_current_ship() {
    let mut adapter = make_adapter();
    adapter.set_ship("Anaconda");
    let snap = adapter.convert_status(&StatusJson::default());
    assert_eq!(snap.aircraft.icao, "Anaconda");
}

#[test]
fn snapshot_timestamp_is_nonzero() {
    let adapter = make_adapter();
    let snap = adapter.convert_status(&StatusJson::default());
    // Timestamp is nanoseconds since adapter creation — should be >= 0.
    assert!(snap.timestamp >= 0);
}

// ===========================================================================
// 5. apply_journal_event — state transitions
// ===========================================================================

#[test]
fn load_game_sets_ship_and_clears_station() {
    let mut adapter = make_adapter();
    // Pre-dock so station is set
    adapter.apply_journal_event(&JournalEvent::Docked {
        station_name: "Station A".to_string(),
        star_system: "System A".to_string(),
    });
    assert!(adapter.docked_station().is_some());

    adapter.apply_journal_event(&JournalEvent::LoadGame {
        ship: "Cobra_MkIII".to_string(),
        commander: Some("CMDR Test".to_string()),
    });
    assert_eq!(
        adapter.metrics().last_aircraft_title.as_deref(),
        Some("Cobra_MkIII")
    );
    assert!(adapter.docked_station().is_none());
}

#[test]
fn fsd_jump_clears_station() {
    let mut adapter = make_adapter();
    adapter.apply_journal_event(&JournalEvent::Docked {
        station_name: "Station X".to_string(),
        star_system: "System X".to_string(),
    });
    assert!(adapter.docked_station().is_some());
    adapter.apply_journal_event(&JournalEvent::FsdJump {
        star_system: "System Y".to_string(),
        star_pos: Some([1.0, 2.0, 3.0]),
    });
    assert_eq!(adapter.current_system(), "System Y");
    assert!(adapter.docked_station().is_none());
}

#[test]
fn duplicate_system_no_redundant_update() {
    let mut adapter = make_adapter();
    adapter.apply_journal_event(&JournalEvent::Location {
        star_system: "Sol".to_string(),
        star_pos: None,
    });
    // Same system again — should not panic or change state
    adapter.apply_journal_event(&JournalEvent::Location {
        star_system: "Sol".to_string(),
        star_pos: None,
    });
    assert_eq!(adapter.current_system(), "Sol");
}

#[test]
fn docked_updates_system_if_different() {
    let mut adapter = make_adapter();
    adapter.apply_journal_event(&JournalEvent::Location {
        star_system: "Old System".to_string(),
        star_pos: None,
    });
    adapter.apply_journal_event(&JournalEvent::Docked {
        station_name: "Port".to_string(),
        star_system: "New System".to_string(),
    });
    assert_eq!(adapter.current_system(), "New System");
    assert_eq!(adapter.docked_station(), Some("Port"));
}

#[test]
fn touchdown_and_liftoff_are_no_ops() {
    let mut adapter = make_adapter();
    adapter.apply_journal_event(&JournalEvent::Touchdown {
        latitude: Some(10.0),
        longitude: Some(20.0),
    });
    adapter.apply_journal_event(&JournalEvent::Liftoff {
        latitude: Some(10.0),
        longitude: Some(20.0),
    });
    // These events don't change tracked state
    assert_eq!(adapter.current_system(), "");
    assert!(adapter.docked_station().is_none());
}

#[test]
fn refuel_all_is_no_op() {
    let mut adapter = make_adapter();
    adapter.apply_journal_event(&JournalEvent::RefuelAll { amount: Some(32.0) });
    assert_eq!(adapter.current_system(), "");
    assert!(adapter.docked_station().is_none());
}

// ===========================================================================
// 6. EliteConfig & AdapterConfig trait
// ===========================================================================

#[test]
fn default_config_poll_interval() {
    let cfg = EliteConfig::default();
    assert_eq!(cfg.poll_interval, Duration::from_millis(250));
}

#[test]
fn default_config_bus_rate() {
    let cfg = EliteConfig::default();
    assert!((cfg.bus_max_rate_hz - 4.0).abs() < f32::EPSILON);
}

#[test]
fn adapter_config_publish_rate() {
    let cfg = EliteConfig::default();
    assert!((cfg.publish_rate_hz() - 4.0).abs() < f32::EPSILON);
}

#[test]
fn adapter_config_connection_timeout() {
    let cfg = EliteConfig::default();
    assert_eq!(cfg.connection_timeout(), Duration::from_secs(5));
}

#[test]
fn adapter_config_auto_reconnect_enabled() {
    let cfg = EliteConfig::default();
    assert!(cfg.enable_auto_reconnect());
}

#[test]
fn adapter_config_max_reconnect_zero() {
    let cfg = EliteConfig::default();
    assert_eq!(cfg.max_reconnect_attempts(), 0);
}

// ===========================================================================
// 7. Adapter lifecycle — start / stop / state
// ===========================================================================

#[test]
fn adapter_starts_disconnected() {
    let adapter = make_adapter();
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
fn adapter_initial_ship_is_unknown() {
    let adapter = make_adapter();
    let snap = adapter.convert_status(&StatusJson::default());
    assert_eq!(snap.aircraft.icao, "Unknown");
}

#[test]
fn set_ship_updates_metrics_title() {
    let mut adapter = make_adapter();
    adapter.set_ship("Python");
    let m = adapter.metrics();
    assert_eq!(m.last_aircraft_title.as_deref(), Some("Python"));
    assert_eq!(m.aircraft_changes, 1);
}

#[test]
fn set_ship_twice_increments_aircraft_changes() {
    let mut adapter = make_adapter();
    adapter.set_ship("Sidewinder");
    adapter.set_ship("Vulture");
    let m = adapter.metrics();
    assert_eq!(m.aircraft_changes, 2);
    assert_eq!(m.last_aircraft_title.as_deref(), Some("Vulture"));
}

// ===========================================================================
// 8. poll_once — async file-based tests
// ===========================================================================

#[tokio::test]
async fn poll_errors_when_not_started() {
    let dir = TempDir::new().unwrap();
    let mut adapter = EliteAdapter::new(config_with_dir(&dir));
    let result = adapter.poll_once().await;
    assert!(result.is_err(), "should fail when adapter not started");
}

#[tokio::test]
async fn poll_returns_snapshot_then_none_on_same_flags() {
    let dir = TempDir::new().unwrap();
    write_status(
        &dir,
        &status_with_flags(EliteFlags::GEAR_DOWN.bits() | EliteFlags::SHIELDS_UP.bits()),
    );

    let mut adapter = EliteAdapter::new(config_with_dir(&dir));
    adapter.start().unwrap();

    let first = adapter.poll_once().await.unwrap();
    assert!(first.is_some());
    let second = adapter.poll_once().await.unwrap();
    assert!(second.is_none(), "unchanged flags should return None");
}

#[tokio::test]
async fn poll_detects_flag_change() {
    let dir = TempDir::new().unwrap();
    write_status(&dir, &status_with_flags(0));

    let mut adapter = EliteAdapter::new(config_with_dir(&dir));
    adapter.start().unwrap();

    // First poll
    let _ = adapter.poll_once().await.unwrap();

    // Change flags
    write_status(&dir, &status_with_flags(EliteFlags::GEAR_DOWN.bits()));
    let snap = adapter.poll_once().await.unwrap();
    assert!(snap.is_some(), "changed flags should produce new snapshot");
    assert!(snap.unwrap().config.gear.all_down());
}

#[tokio::test]
async fn poll_transitions_to_active() {
    let dir = TempDir::new().unwrap();
    write_status(&dir, &StatusJson::default());

    let mut adapter = EliteAdapter::new(config_with_dir(&dir));
    adapter.start().unwrap();
    assert_eq!(adapter.state(), AdapterState::Connected);

    let _ = adapter.poll_once().await.unwrap();
    assert_eq!(adapter.state(), AdapterState::Active);
}

#[tokio::test]
async fn poll_returns_none_for_missing_status_file() {
    let dir = TempDir::new().unwrap();
    let mut adapter = EliteAdapter::new(config_with_dir(&dir));
    adapter.start().unwrap();
    let result = adapter.poll_once().await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn poll_with_malformed_json_returns_error() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("Status.json"), "NOT VALID JSON").unwrap();

    let mut adapter = EliteAdapter::new(config_with_dir(&dir));
    adapter.start().unwrap();
    let result = adapter.poll_once().await;
    assert!(result.is_err(), "malformed JSON should produce an error");
}

// ===========================================================================
// 9. JournalReader — file-based tailing
// ===========================================================================

fn write_journal_file(dir: &TempDir, name: &str, content: &str) -> PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, content).unwrap();
    path
}

#[test]
fn journal_reader_empty_dir_returns_empty() {
    let dir = TempDir::new().unwrap();
    let mut reader = JournalReader::new(dir.path());
    let events = reader.read_new_events().unwrap();
    assert!(events.is_empty());
}

#[test]
fn journal_reader_current_file_is_none_initially() {
    let dir = TempDir::new().unwrap();
    let reader = JournalReader::new(dir.path());
    assert!(reader.current_file().is_none());
}

#[test]
fn journal_reader_reads_load_game() {
    let dir = TempDir::new().unwrap();
    write_journal_file(
        &dir,
        "Journal.20250601120000.01.log",
        r#"{"timestamp":"2025-06-01T12:00:00Z","event":"LoadGame","Ship":"Anaconda","Commander":"CMDR Deep"}"#,
    );
    let mut reader = JournalReader::new(dir.path());
    let events = reader.read_new_events().unwrap();
    assert_eq!(events.len(), 1);
    assert!(matches!(&events[0], JournalEvent::LoadGame { ship, .. } if ship == "Anaconda"));
}

#[test]
fn journal_reader_tailing_only_returns_new_lines() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("Journal.20250601120000.01.log");
    {
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"timestamp":"T1","event":"LoadGame","Ship":"Eagle"}}"#
        )
        .unwrap();
    }

    let mut reader = JournalReader::new(dir.path());
    let first = reader.read_new_events().unwrap();
    assert_eq!(first.len(), 1);

    // Append another event
    {
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        writeln!(
            f,
            r#"{{"timestamp":"T2","event":"FsdJump","StarSystem":"Lave","StarPos":[0.0,0.0,0.0]}}"#
        )
        .unwrap();
    }

    let second = reader.read_new_events().unwrap();
    assert_eq!(second.len(), 1);
    assert!(
        matches!(&second[0], JournalEvent::FsdJump { star_system, .. } if star_system == "Lave")
    );
}

#[test]
fn journal_reader_switches_to_newer_file() {
    let dir = TempDir::new().unwrap();
    write_journal_file(
        &dir,
        "Journal.20250601120000.01.log",
        r#"{"timestamp":"T1","event":"LoadGame","Ship":"Eagle"}"#,
    );

    let mut reader = JournalReader::new(dir.path());
    let _ = reader.read_new_events().unwrap();
    let old_file = reader.current_file().unwrap().to_path_buf();

    // Ensure different modification time — sleep long enough for filesystem
    // granularity on all platforms (FAT32 has 2-second resolution).
    std::thread::sleep(Duration::from_millis(1100));
    write_journal_file(
        &dir,
        "Journal.20250602120000.01.log",
        r#"{"timestamp":"T2","event":"LoadGame","Ship":"Python"}"#,
    );

    let events = reader.read_new_events().unwrap();
    assert_eq!(events.len(), 1);
    assert!(matches!(&events[0], JournalEvent::LoadGame { ship, .. } if ship == "Python"));
    assert_ne!(reader.current_file().unwrap(), old_file);
}

#[test]
fn journal_reader_reset_re_reads_from_start() {
    let dir = TempDir::new().unwrap();
    write_journal_file(
        &dir,
        "Journal.20250601120000.01.log",
        "{\"timestamp\":\"T1\",\"event\":\"LoadGame\",\"Ship\":\"Eagle\"}\n\
         {\"timestamp\":\"T2\",\"event\":\"FsdJump\",\"StarSystem\":\"Sol\",\"StarPos\":[0.0,0.0,0.0]}\n",
    );

    let mut reader = JournalReader::new(dir.path());
    let first = reader.read_new_events().unwrap();
    assert_eq!(first.len(), 2);

    // No new events
    let empty = reader.read_new_events().unwrap();
    assert!(empty.is_empty());

    // Reset and re-read
    reader.reset();
    let replayed = reader.read_new_events().unwrap();
    assert_eq!(replayed.len(), 2);
}

#[test]
fn journal_reader_skips_non_journal_files() {
    let dir = TempDir::new().unwrap();
    write_journal_file(&dir, "Status.json", r#"{"Flags":0}"#);
    write_journal_file(&dir, "ModulesInfo.json", "{}");
    assert!(JournalReader::find_latest_journal(dir.path()).is_none());
}

// ===========================================================================
// 10. Combined workflow — journal → adapter → snapshot
// ===========================================================================

#[tokio::test]
async fn full_workflow_load_game_then_poll() {
    let dir = TempDir::new().unwrap();

    // Write a journal with LoadGame
    write_journal_file(
        &dir,
        "Journal.20250601120000.01.log",
        r#"{"timestamp":"T1","event":"LoadGame","Ship":"Type_9_Heavy","Commander":"CMDR Cargo"}"#,
    );

    // Write Status.json with gear down and lights on
    write_status(
        &dir,
        &StatusJson {
            flags: EliteFlags::GEAR_DOWN.bits() | EliteFlags::LIGHTS_ON.bits(),
            fuel: Some(FuelStatus {
                fuel_main: 64.0,
                fuel_reservoir: 16.0,
            }),
            ..Default::default()
        },
    );

    let mut adapter = EliteAdapter::new(config_with_dir(&dir));
    adapter.start().unwrap();

    // Process journal events
    let events = adapter.journal_reader.read_new_events().unwrap();
    for e in &events {
        adapter.apply_journal_event(e);
    }

    // Poll status
    let snap = adapter.poll_once().await.unwrap().unwrap();

    // Verify combined state
    assert_eq!(snap.sim, SimId::EliteDangerous);
    assert_eq!(snap.aircraft.icao, "Type_9_Heavy");
    assert!(snap.config.gear.all_down());
    assert!(snap.config.lights.nav);
    assert!(snap.config.lights.landing);
    let fuel = snap.config.fuel["main"].value();
    // 64 / (64 + 16) = 80%
    assert!((fuel - 80.0).abs() < 0.01);
}

// ===========================================================================
// 11. Error types
// ===========================================================================

#[test]
fn elite_error_display_status_not_found() {
    let err = EliteError::StatusNotFound {
        path: PathBuf::from_iter(["some", "path"]),
    };
    let msg = err.to_string();
    assert!(
        msg.contains("some") && msg.contains("path"),
        "error should contain path components: {msg}"
    );
}

#[test]
fn elite_error_display_json() {
    let json_err = serde_json::from_str::<StatusJson>("!!!").unwrap_err();
    let err = EliteError::Json(json_err);
    let msg = err.to_string();
    assert!(
        msg.contains("JSON") || msg.contains("parse"),
        "error should mention JSON: {msg}"
    );
}

// ===========================================================================
// 12. Edge cases — combined flags
// ===========================================================================

#[test]
fn combined_docked_and_gear_down() {
    let adapter = make_adapter();
    let bits = EliteFlags::DOCKED.bits() | EliteFlags::GEAR_DOWN.bits();
    let snap = adapter.convert_status(&status_with_flags(bits));
    assert!(snap.config.gear.all_down());
    assert!(!snap.validity.position_valid, "docked → position not valid");
}

#[test]
fn all_flags_set_at_once() {
    let adapter = make_adapter();
    let bits = EliteFlags::DOCKED.bits()
        | EliteFlags::LANDED.bits()
        | EliteFlags::GEAR_DOWN.bits()
        | EliteFlags::SHIELDS_UP.bits()
        | EliteFlags::SUPERCRUISE.bits()
        | EliteFlags::LIGHTS_ON.bits()
        | EliteFlags::IN_SRV.bits()
        | EliteFlags::FSD_JUMP.bits();
    let snap = adapter.convert_status(&status_with_flags(bits));
    // Should not panic; verify a few fields
    assert!(snap.config.gear.all_down());
    assert!(snap.config.lights.nav);
    assert!(!snap.validity.position_valid, "DOCKED flag present");
    assert!(!snap.validity.safe_for_ffb);
}

#[test]
fn hardpoints_and_silent_running_do_not_affect_snapshot_validity() {
    let adapter = make_adapter();
    let bits = EliteFlags::HARDPOINTS_DEPLOYED.bits() | EliteFlags::SILENT_RUNNING.bits();
    let snap = adapter.convert_status(&status_with_flags(bits));
    // These flags don't map to any snapshot field, but shouldn't crash
    assert!(snap.validity.position_valid, "in-flight with hardpoints");
    assert!(!snap.config.gear.all_down());
}
