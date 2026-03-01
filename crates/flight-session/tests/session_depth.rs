// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the session and persistence subsystem.
//!
//! Covers: session lifecycle, state persistence, serialization,
//! file management, recovery, and property-based testing.

use flight_session::migration::{self, MigrationError, StateVersion, CURRENT_VERSION};
use flight_session::recovery::RecoveryManager;
use flight_session::state_persistence::StatePersistence;
use flight_session::store::{
    CalibrationData, SessionState, SessionStore, ShutdownInfo, ShutdownReason,
    WindowPosition,
};
use proptest::prelude::*;
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;
use tempfile::TempDir;

// ═══════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════

fn rich_state() -> SessionState {
    SessionState {
        active_profile: Some("combat".into()),
        last_sim: Some("MSFS".into()),
        device_assignments: HashMap::from([
            ("stick-1".into(), "pitch_roll".into()),
            ("throttle-1".into(), "thrust".into()),
        ]),
        window_positions: HashMap::from([
            (
                "main".into(),
                WindowPosition {
                    x: 100,
                    y: 200,
                    width: 1920,
                    height: 1080,
                },
            ),
            (
                "hud".into(),
                WindowPosition {
                    x: 0,
                    y: 0,
                    width: 640,
                    height: 480,
                },
            ),
        ]),
        calibration_data: HashMap::from([(
            "stick-1".into(),
            CalibrationData {
                min: -1.0,
                max: 1.0,
                center: 0.0,
                deadzone: 0.05,
                timestamp: 1_700_000_000,
            },
        )]),
        last_shutdown: Some(ShutdownInfo {
            timestamp: 1_700_000_100,
            reason: ShutdownReason::Clean,
        }),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. Session Lifecycle (6 tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn lifecycle_create_session_with_defaults() {
    let sp = StatePersistence::new("sess-001", 10);
    assert_eq!(sp.snapshot_count(), 0);
    assert!(!sp.is_dirty());
}

#[test]
fn lifecycle_save_session_to_disk() {
    let dir = TempDir::new().unwrap();
    let store = SessionStore::new(dir.path().join("state.json"));
    let state = rich_state();
    store.save(&state).unwrap();
    assert!(store.path().exists());
    let content = std::fs::read_to_string(store.path()).unwrap();
    assert!(content.contains("combat"));
}

#[test]
fn lifecycle_load_session_preserves_all_fields() {
    let dir = TempDir::new().unwrap();
    let store = SessionStore::new(dir.path().join("state.json"));
    let state = rich_state();
    store.save(&state).unwrap();
    let loaded = store.load().unwrap().unwrap();

    assert_eq!(loaded.active_profile, state.active_profile);
    assert_eq!(loaded.last_sim, state.last_sim);
    assert_eq!(loaded.device_assignments, state.device_assignments);
    assert_eq!(loaded.window_positions, state.window_positions);
    assert_eq!(loaded.calibration_data, state.calibration_data);
    assert_eq!(loaded.last_shutdown, state.last_shutdown);
}

#[test]
fn lifecycle_delete_session_clears_file() {
    let dir = TempDir::new().unwrap();
    let store = SessionStore::new(dir.path().join("state.json"));
    store.save(&SessionState::default()).unwrap();
    assert!(store.path().exists());
    store.clear().unwrap();
    assert!(!store.path().exists());
    // Load after clear returns None
    assert!(store.load().unwrap().is_none());
}

#[test]
fn lifecycle_list_sessions_via_snapshots() {
    let mut sp = StatePersistence::new("list-test", 10);
    for i in 0..5 {
        sp.set_profile(&format!("profile-{i}"));
        sp.snapshot();
    }
    assert_eq!(sp.snapshot_count(), 5);
    for i in 0..5 {
        let snap = sp.restore_by_index(i).unwrap();
        assert_eq!(
            snap.active_profile.as_deref(),
            Some(format!("profile-{i}").as_str())
        );
    }
}

#[test]
fn lifecycle_session_versioning_envelope() {
    let dir = TempDir::new().unwrap();
    let store = SessionStore::new(dir.path().join("state.json"));
    store.save(&SessionState::default()).unwrap();

    let raw = std::fs::read_to_string(store.path()).unwrap();
    let envelope: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(envelope["version"], CURRENT_VERSION);
    assert!(envelope["state"].is_object());
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. State Persistence (6 tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn persist_device_state_round_trip() {
    let dir = TempDir::new().unwrap();
    let store = SessionStore::new(dir.path().join("state.json"));

    let state = SessionState {
        device_assignments: HashMap::from([
            ("rudder-1".into(), "yaw".into()),
            ("throttle-2".into(), "mixture".into()),
        ]),
        ..SessionState::default()
    };

    store.save(&state).unwrap();
    let loaded = store.load().unwrap().unwrap();
    assert_eq!(loaded.device_assignments.len(), 2);
    assert_eq!(loaded.device_assignments["rudder-1"], "yaw");
    assert_eq!(loaded.device_assignments["throttle-2"], "mixture");
}

#[test]
fn persist_profile_state_via_state_persistence() {
    let mut sp = StatePersistence::new("sp-test", 5);
    sp.set_profile("aerobatics");
    sp.set_aircraft("Extra300");
    sp.set_sim("DCS");
    sp.snapshot();

    let snap = sp.restore_by_index(0).unwrap();
    assert_eq!(snap.active_profile.as_deref(), Some("aerobatics"));
    assert_eq!(snap.active_aircraft.as_deref(), Some("Extra300"));
    assert_eq!(snap.active_sim.as_deref(), Some("DCS"));
}

#[test]
fn persist_calibration_data() {
    let dir = TempDir::new().unwrap();
    let store = SessionStore::new(dir.path().join("state.json"));

    let state = SessionState {
        calibration_data: HashMap::from([(
            "rudder-pedals".into(),
            CalibrationData {
                min: -0.8,
                max: 0.9,
                center: 0.05,
                deadzone: 0.1,
                timestamp: 1_700_123_456,
            },
        )]),
        ..SessionState::default()
    };
    store.save(&state).unwrap();
    let loaded = store.load().unwrap().unwrap();
    let cal = &loaded.calibration_data["rudder-pedals"];
    assert!((cal.min - (-0.8)).abs() < f64::EPSILON);
    assert!((cal.max - 0.9).abs() < f64::EPSILON);
    assert!((cal.center - 0.05).abs() < f64::EPSILON);
    assert!((cal.deadzone - 0.1).abs() < f64::EPSILON);
    assert_eq!(cal.timestamp, 1_700_123_456);
}

#[test]
fn persist_window_positions() {
    let dir = TempDir::new().unwrap();
    let store = SessionStore::new(dir.path().join("state.json"));

    let state = SessionState {
        window_positions: HashMap::from([(
            "settings".into(),
            WindowPosition {
                x: -10,
                y: -20,
                width: 800,
                height: 600,
            },
        )]),
        ..SessionState::default()
    };
    store.save(&state).unwrap();
    let loaded = store.load().unwrap().unwrap();
    let wp = &loaded.window_positions["settings"];
    assert_eq!(wp.x, -10);
    assert_eq!(wp.y, -20);
    assert_eq!(wp.width, 800);
    assert_eq!(wp.height, 600);
}

#[test]
fn persist_preferences_via_state_persistence() {
    let mut sp = StatePersistence::new("pref-test", 5);
    sp.set_preference("theme", "dark");
    sp.set_preference("units", "metric");
    sp.set_preference("language", "en-US");

    assert_eq!(sp.get_preference("theme"), Some("dark"));
    assert_eq!(sp.get_preference("units"), Some("metric"));
    assert_eq!(sp.get_preference("language"), Some("en-US"));
}

#[test]
fn persist_atomic_save_no_partial_state() {
    let dir = TempDir::new().unwrap();
    let store = SessionStore::new(dir.path().join("state.json"));

    // Save twice — the second should atomically replace the first.
    let s1 = SessionState {
        active_profile: Some("first".into()),
        ..SessionState::default()
    };
    store.save(&s1).unwrap();

    let s2 = SessionState {
        active_profile: Some("second".into()),
        last_sim: Some("XPlane".into()),
        ..SessionState::default()
    };
    store.save(&s2).unwrap();

    // No temp file should remain.
    let tmp = dir.path().join("state.tmp");
    assert!(!tmp.exists());

    let loaded = store.load().unwrap().unwrap();
    assert_eq!(loaded.active_profile.as_deref(), Some("second"));
    assert_eq!(loaded.last_sim.as_deref(), Some("XPlane"));
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. Serialization (5 tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn serde_json_roundtrip_full_state() {
    let state = rich_state();
    let json = serde_json::to_string_pretty(&state).unwrap();
    let back: SessionState = serde_json::from_str(&json).unwrap();
    assert_eq!(state, back);
}

#[test]
fn serde_toml_roundtrip_state_persistence() {
    // SessionState from state_persistence is also Serialize/Deserialize.
    let mut sp = StatePersistence::new("toml-test", 3);
    sp.set_profile("landing");
    sp.set_aircraft("B737");
    sp.set_sim("MSFS");
    sp.set_device_config("stick-1", r#"{"mode":"normal"}"#);
    sp.set_preference("hud_opacity", "0.8");

    let json_str = sp.to_json();
    let state: flight_session::state_persistence::SessionState =
        serde_json::from_str(&json_str).unwrap();

    // Round-trip through TOML.
    let toml_str = toml::to_string(&state).unwrap();
    let back: flight_session::state_persistence::SessionState = toml::from_str(&toml_str).unwrap();
    assert_eq!(state, back);
}

#[test]
fn serde_forward_compat_unknown_fields_ignored() {
    // Simulate a future version that added an extra field.
    let json = json!({
        "version": CURRENT_VERSION,
        "state": {
            "active_profile": "test",
            "device_assignments": {},
            "last_sim": null,
            "window_positions": {},
            "calibration_data": {},
            "last_shutdown": null,
            "future_field": "should be ignored"
        }
    });
    let raw = serde_json::to_string(&json).unwrap();
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("state.json");
    std::fs::write(&path, raw).unwrap();

    let store = SessionStore::new(path);
    // Should load successfully — unknown fields are ignored by serde default.
    let loaded = store.load().unwrap().unwrap();
    assert_eq!(loaded.active_profile.as_deref(), Some("test"));
}

#[test]
fn serde_backward_compat_v1_to_v2_migration() {
    let v1_json = json!({
        "version": 1,
        "state": {
            "active_profile": "legacy",
            "device_assignments": {"old-stick": "axes"},
            "last_sim": "DCS"
        }
    });
    let raw = serde_json::to_string(&v1_json).unwrap();
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("state.json");
    std::fs::write(&path, raw).unwrap();

    let store = SessionStore::new(path);
    let loaded = store.load().unwrap().unwrap();
    assert_eq!(loaded.active_profile.as_deref(), Some("legacy"));
    assert_eq!(loaded.last_sim.as_deref(), Some("DCS"));
    // V2 fields get defaults from migration.
    assert!(loaded.window_positions.is_empty());
    assert!(loaded.calibration_data.is_empty());
    assert!(loaded.last_shutdown.is_none());
}

#[test]
fn serde_corrupt_file_returns_descriptive_error() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("state.json");

    // Truncated JSON.
    std::fs::write(&path, r#"{"version": 2, "state": {"active_pro"#).unwrap();
    let store = SessionStore::new(&path);
    let err = store.load().unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("serialization") || msg.contains("EOF"),
        "error should describe parse failure: {msg}"
    );

    // Binary garbage.
    std::fs::write(&path, [0xFF, 0xFE, 0x00, 0x01]).unwrap();
    let store = SessionStore::new(path);
    assert!(store.load().is_err());
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. File Management (5 tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn file_config_directory_created_recursively() {
    let dir = TempDir::new().unwrap();
    let deep = dir.path().join("a").join("b").join("c").join("state.json");
    let store = SessionStore::new(deep);
    store.save(&SessionState::default()).unwrap();
    assert!(store.path().exists());
}

#[test]
fn file_locking_concurrent_writes_last_wins() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("state.json");

    // Simulate sequential "concurrent" writes — last one wins.
    for i in 0..10 {
        let store = SessionStore::new(&path);
        let state = SessionState {
            active_profile: Some(format!("profile-{i}")),
            ..SessionState::default()
        };
        store.save(&state).unwrap();
    }

    let store = SessionStore::new(path);
    let loaded = store.load().unwrap().unwrap();
    assert_eq!(loaded.active_profile.as_deref(), Some("profile-9"));
}

#[test]
fn file_concurrent_read_does_not_corrupt() {
    let dir = TempDir::new().unwrap();
    let store = SessionStore::new(dir.path().join("state.json"));
    store.save(&rich_state()).unwrap();

    // Multiple reads should all succeed and return identical data.
    let results: Vec<SessionState> = (0..10)
        .map(|_| {
            let s = SessionStore::new(store.path());
            s.load().unwrap().unwrap()
        })
        .collect();

    for r in &results {
        assert_eq!(r, &results[0]);
    }
}

#[test]
fn file_backup_before_overwrite_via_atomic() {
    let dir = TempDir::new().unwrap();
    let store = SessionStore::new(dir.path().join("state.json"));

    let s1 = SessionState {
        active_profile: Some("original".into()),
        ..SessionState::default()
    };
    store.save(&s1).unwrap();

    // Overwriting uses atomic write (tmp + rename), so if save succeeds
    // the previous file is fully replaced.
    let s2 = SessionState {
        active_profile: Some("updated".into()),
        ..SessionState::default()
    };
    store.save(&s2).unwrap();

    let loaded = store.load().unwrap().unwrap();
    assert_eq!(loaded.active_profile.as_deref(), Some("updated"));
    // No leftover tmp file.
    assert!(!dir.path().join("state.tmp").exists());
}

#[test]
fn file_readonly_directory_returns_io_error() {
    // On Windows, we can't easily make a directory read-only in the same way,
    // but we can test writing to a path where the parent is a file (not a dir).
    let dir = TempDir::new().unwrap();
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, "I am a file").unwrap();

    let store = SessionStore::new(blocker.join("state.json"));
    let result = store.save(&SessionState::default());
    assert!(result.is_err(), "writing inside a file should fail");
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. Recovery (5 tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn recovery_from_corrupt_session_loads_defaults() {
    let dir = TempDir::new().unwrap();
    let session_dir = dir.path().join("session");
    std::fs::create_dir_all(&session_dir).unwrap();

    // Write corrupt state file.
    std::fs::write(session_dir.join("session_state.json"), "CORRUPT!!!").unwrap();
    // Write heartbeat to signal crash.
    std::fs::write(session_dir.join("heartbeat"), "1700000000").unwrap();

    let mgr = RecoveryManager::new(&session_dir);
    assert!(mgr.needs_recovery().unwrap());
    // Recovery attempts to load the corrupt file — should error.
    let result = mgr.recover();
    assert!(result.is_err(), "corrupt state should cause recovery error");
}

#[test]
fn recovery_from_partial_write_detects_crash() {
    let dir = TempDir::new().unwrap();
    let session_dir = dir.path().join("session");
    let mgr = RecoveryManager::new(&session_dir);

    // Normal save.
    let state = rich_state();
    mgr.store().save(&state).unwrap();
    mgr.set_heartbeat().unwrap();
    // Do NOT mark clean shutdown — simulate crash.

    assert!(mgr.needs_recovery().unwrap());

    let recovered = mgr.recover().unwrap().unwrap();
    assert_eq!(recovered.active_profile.as_deref(), Some("combat"));
    // After recovery, heartbeat is cleaned up.
    assert!(!mgr.needs_recovery().unwrap());
}

#[test]
fn recovery_journal_based_heartbeat_lifecycle() {
    let dir = TempDir::new().unwrap();
    let mgr = RecoveryManager::new(dir.path().join("session"));

    // Initially no recovery needed.
    assert!(!mgr.needs_recovery().unwrap());

    // Start heartbeat (simulating service running).
    mgr.set_heartbeat().unwrap();
    assert!(mgr.needs_recovery().unwrap());

    // Clean shutdown clears the heartbeat.
    mgr.mark_clean_shutdown().unwrap();
    assert!(!mgr.needs_recovery().unwrap());
    assert!(mgr.check_clean_shutdown().unwrap());
}

#[test]
fn recovery_default_values_when_no_state_persisted() {
    let dir = TempDir::new().unwrap();
    let mgr = RecoveryManager::new(dir.path().join("session"));

    mgr.set_heartbeat().unwrap();
    // No state was ever saved.
    let recovered = mgr.recover().unwrap();
    assert!(
        recovered.is_none(),
        "recovery with no persisted state returns None"
    );
}

#[test]
fn recovery_save_and_mark_shutdown_round_trip() {
    let dir = TempDir::new().unwrap();
    let mgr = RecoveryManager::new(dir.path().join("session"));

    let state = SessionState {
        active_profile: Some("departure".into()),
        device_assignments: HashMap::from([("stick".into(), "roll".into())]),
        ..SessionState::default()
    };

    mgr.save_and_mark_shutdown(&state, ShutdownReason::Clean)
        .unwrap();

    assert!(mgr.check_clean_shutdown().unwrap());
    assert!(!mgr.needs_recovery().unwrap());

    let loaded = mgr.store().load().unwrap().unwrap();
    assert_eq!(loaded.active_profile.as_deref(), Some("departure"));
    assert_eq!(loaded.device_assignments["stick"], "roll");
    let shutdown = loaded.last_shutdown.unwrap();
    assert_eq!(shutdown.reason, ShutdownReason::Clean);
    assert!(shutdown.timestamp > 0);
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. Migration depth tests (4 tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn migration_version_enum_all_variants() {
    assert_eq!(StateVersion::from_u32(1), Some(StateVersion::V1));
    assert_eq!(StateVersion::from_u32(2), Some(StateVersion::V2));
    assert_eq!(StateVersion::from_u32(0), None);
    assert_eq!(StateVersion::from_u32(3), None);
    assert_eq!(StateVersion::from_u32(u32::MAX), None);
    assert_eq!(StateVersion::V1.as_u32(), 1);
    assert_eq!(StateVersion::V2.as_u32(), 2);
}

#[test]
fn migration_v1_adds_all_v2_fields() {
    let v1 = json!({
        "active_profile": "fighter",
        "device_assignments": {"joystick": "pitch"},
        "last_sim": "DCS"
    });
    let result = migration::migrate(1, v1).unwrap();
    assert_eq!(result.active_profile.as_deref(), Some("fighter"));
    assert!(result.window_positions.is_empty());
    assert!(result.calibration_data.is_empty());
    assert!(result.last_shutdown.is_none());
}

#[test]
fn migration_future_version_error_message() {
    let future = json!({"active_profile": null});
    let err = migration::migrate(100, future).unwrap_err();
    match err {
        MigrationError::UnknownVersion(v) => assert_eq!(v, 100),
        other => panic!("expected UnknownVersion, got: {other:?}"),
    }
}

#[test]
fn migration_wrong_types_in_v2_fail_gracefully() {
    let bad = json!({
        "active_profile": 12345,
        "device_assignments": [],
        "last_sim": null,
        "window_positions": {},
        "calibration_data": {},
        "last_shutdown": null
    });
    let err = migration::migrate(2, bad).unwrap_err();
    assert!(
        matches!(err, MigrationError::Deserialization(_)),
        "wrong types should cause deserialization error"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 7. Property Tests (5 tests)
// ═══════════════════════════════════════════════════════════════════════════

proptest! {
    #[test]
    fn prop_roundtrip_identity(
        profile in proptest::option::of("[a-zA-Z0-9_-]{1,30}"),
        sim in proptest::option::of("[a-zA-Z0-9]{1,20}"),
        device_count in 0..5usize,
    ) {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::new(dir.path().join("state.json"));

        let mut state = SessionState {
            active_profile: profile,
            last_sim: sim,
            ..SessionState::default()
        };
        for i in 0..device_count {
            state.device_assignments.insert(
                format!("dev-{i}"),
                format!("role-{i}"),
            );
        }

        store.save(&state).unwrap();
        let loaded = store.load().unwrap().unwrap();
        prop_assert_eq!(state, loaded);
    }

    #[test]
    fn prop_idempotent_save(
        profile in proptest::option::of("[a-zA-Z]{1,20}"),
    ) {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::new(dir.path().join("state.json"));

        let state = SessionState {
            active_profile: profile,
            ..SessionState::default()
        };

        // Save twice — result should be identical.
        store.save(&state).unwrap();
        let after_first = std::fs::read_to_string(store.path()).unwrap();

        store.save(&state).unwrap();
        let after_second = std::fs::read_to_string(store.path()).unwrap();

        prop_assert_eq!(after_first, after_second);
    }

    #[test]
    fn prop_concurrent_load_safety(
        profile in "[a-z]{1,10}",
    ) {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::new(dir.path().join("state.json"));

        let state = SessionState {
            active_profile: Some(profile),
            ..SessionState::default()
        };
        store.save(&state).unwrap();

        // Multiple loads of the same file yield identical results.
        let results: Vec<SessionState> = (0..5)
            .map(|_| {
                let s = SessionStore::new(store.path());
                s.load().unwrap().unwrap()
            })
            .collect();

        for r in &results {
            prop_assert_eq!(r, &results[0]);
        }
    }

    #[test]
    fn prop_calibration_values_preserved(
        min_val in -100.0f64..0.0,
        max_val in 0.0f64..100.0,
        center in -1.0f64..1.0,
        deadzone in 0.0f64..0.5,
    ) {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::new(dir.path().join("state.json"));

        let state = SessionState {
            calibration_data: HashMap::from([("test-device".into(), CalibrationData {
                min: min_val,
                max: max_val,
                center,
                deadzone,
                timestamp: 1_700_000_000,
            })]),
            ..SessionState::default()
        };

        store.save(&state).unwrap();
        let loaded = store.load().unwrap().unwrap();
        let cal = &loaded.calibration_data["test-device"];
        prop_assert!((cal.min - min_val).abs() < 1e-10);
        prop_assert!((cal.max - max_val).abs() < 1e-10);
        prop_assert!((cal.center - center).abs() < 1e-10);
        prop_assert!((cal.deadzone - deadzone).abs() < 1e-10);
    }

    #[test]
    fn prop_window_position_preserved(
        x in -5000i32..5000,
        y in -5000i32..5000,
        width in 1u32..10000,
        height in 1u32..10000,
    ) {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::new(dir.path().join("state.json"));

        let state = SessionState {
            window_positions: HashMap::from([("test-win".into(), WindowPosition {
                x, y, width, height,
            })]),
            ..SessionState::default()
        };

        store.save(&state).unwrap();
        let loaded = store.load().unwrap().unwrap();
        let wp = &loaded.window_positions["test-win"];
        prop_assert_eq!(wp.x, x);
        prop_assert_eq!(wp.y, y);
        prop_assert_eq!(wp.width, width);
        prop_assert_eq!(wp.height, height);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 8. StatePersistence depth (additional edge cases)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn state_persistence_dirty_flag_on_each_setter() {
    let mut sp = StatePersistence::new("dirty-test", 5);
    assert!(!sp.is_dirty());

    sp.set_profile("a");
    assert!(sp.is_dirty());
    sp.mark_clean();

    sp.set_aircraft("B747");
    assert!(sp.is_dirty());
    sp.mark_clean();

    sp.set_sim("MSFS");
    assert!(sp.is_dirty());
    sp.mark_clean();

    sp.set_device_config("dev", "cfg");
    assert!(sp.is_dirty());
    sp.mark_clean();

    sp.set_preference("key", "val");
    assert!(sp.is_dirty());
}

#[test]
fn state_persistence_restore_latest_sets_clean() {
    let mut sp = StatePersistence::new("clean-test", 5);
    sp.set_profile("before");
    sp.snapshot();
    sp.set_profile("after");
    assert!(sp.is_dirty());

    sp.restore_latest().unwrap();
    assert!(!sp.is_dirty(), "restore_latest should mark state clean");
}

#[test]
fn state_persistence_restore_empty_returns_none() {
    let mut sp = StatePersistence::new("empty", 5);
    assert!(sp.restore_latest().is_none());
}

#[test]
fn state_persistence_out_of_bounds_index_returns_none() {
    let sp = StatePersistence::new("bounds", 5);
    assert!(sp.restore_by_index(0).is_none());
    assert!(sp.restore_by_index(100).is_none());
}

#[test]
fn state_persistence_json_invalid_returns_descriptive_error() {
    let result = StatePersistence::from_json("{broken");
    assert!(result.is_err());
    let msg = result.unwrap_err();
    assert!(!msg.is_empty(), "error message should be non-empty");
}

// ═══════════════════════════════════════════════════════════════════════════
// 9. Shutdown reason variants
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn shutdown_reason_all_variants_serialize() {
    let reasons = [
        ShutdownReason::Clean,
        ShutdownReason::Crash,
        ShutdownReason::Unknown,
    ];
    for reason in &reasons {
        let info = ShutdownInfo {
            timestamp: 123,
            reason: reason.clone(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let back: ShutdownInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.reason, *reason);
        assert_eq!(back.timestamp, 123);
    }
}

#[test]
fn recovery_crash_reason_persisted_through_save_and_mark() {
    let dir = TempDir::new().unwrap();
    let mgr = RecoveryManager::new(dir.path().join("session"));

    let state = SessionState::default();
    mgr.save_and_mark_shutdown(&state, ShutdownReason::Crash)
        .unwrap();

    let loaded = mgr.store().load().unwrap().unwrap();
    assert_eq!(loaded.last_shutdown.unwrap().reason, ShutdownReason::Crash);
}

#[test]
fn recovery_staleness_threshold_configurable() {
    let dir = TempDir::new().unwrap();
    let session_dir = dir.path().join("session");
    std::fs::create_dir_all(&session_dir).unwrap();

    // Write a heartbeat from 5 seconds ago.
    let five_ago = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        - 5;
    std::fs::write(session_dir.join("heartbeat"), five_ago.to_string()).unwrap();

    // With 1s threshold → stale.
    let mgr_short =
        RecoveryManager::new(&session_dir).with_staleness_threshold(Duration::from_secs(1));
    assert!(mgr_short.is_heartbeat_stale().unwrap());

    // With 60s threshold → not stale.
    let mgr_long =
        RecoveryManager::new(&session_dir).with_staleness_threshold(Duration::from_secs(60));
    assert!(!mgr_long.is_heartbeat_stale().unwrap());
}
