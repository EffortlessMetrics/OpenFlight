// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for `flight-session`.
//!
//! Covers session creation, state transitions, event recording with timestamps,
//! session persistence (save/load round-trips), concurrent access to the store,
//! cleanup after timeout, and property-based invariants.

#![allow(clippy::field_reassign_with_default)]

use flight_session::store::{
    CalibrationData, SessionState, SessionStore, ShutdownInfo, ShutdownReason, WindowPosition,
};
use flight_session::state_persistence::StatePersistence;
use flight_session::recovery::RecoveryManager;

use tempfile::TempDir;

// ══════════════════════════════════════════════════════════════════════════════
// 1. Session creation tests
// ══════════════════════════════════════════════════════════════════════════════

mod creation_depth {
    use super::*;

    #[test]
    fn create_with_defaults() {
        let sp = StatePersistence::new("session-1", 10);
        assert!(!sp.is_dirty());
        assert_eq!(sp.snapshot_count(), 0);
    }

    #[test]
    fn create_with_zero_max_snapshots() {
        let mut sp = StatePersistence::new("s", 0);
        sp.set_profile("default");
        sp.snapshot();
        // With max_snapshots=0, every snapshot is immediately evicted
        assert_eq!(sp.snapshot_count(), 0);
    }

    #[test]
    fn create_with_large_max_snapshots() {
        let mut sp = StatePersistence::new("s", 10000);
        for i in 0..100 {
            sp.set_preference("iter", &i.to_string());
            sp.snapshot();
        }
        assert_eq!(sp.snapshot_count(), 100);
    }

    #[test]
    fn session_state_default_is_empty() {
        let state = SessionState::default();
        assert!(state.active_profile.is_none());
        assert!(state.device_assignments.is_empty());
        assert!(state.last_sim.is_none());
        assert!(state.window_positions.is_empty());
        assert!(state.calibration_data.is_empty());
        assert!(state.last_shutdown.is_none());
    }

    #[test]
    fn session_store_path_preserved() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("my_state.json");
        let store = SessionStore::new(&path);
        assert_eq!(store.path(), path);
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// 2. Session state transitions
// ══════════════════════════════════════════════════════════════════════════════

mod state_transitions {
    use super::*;

    #[test]
    fn set_profile_marks_dirty() {
        let mut sp = StatePersistence::new("s", 5);
        assert!(!sp.is_dirty());
        sp.set_profile("combat");
        assert!(sp.is_dirty());
    }

    #[test]
    fn set_aircraft_marks_dirty() {
        let mut sp = StatePersistence::new("s", 5);
        sp.set_aircraft("C172");
        assert!(sp.is_dirty());
    }

    #[test]
    fn set_sim_marks_dirty() {
        let mut sp = StatePersistence::new("s", 5);
        sp.set_sim("MSFS");
        assert!(sp.is_dirty());
    }

    #[test]
    fn mark_clean_resets_dirty() {
        let mut sp = StatePersistence::new("s", 5);
        sp.set_profile("x");
        assert!(sp.is_dirty());
        sp.mark_clean();
        assert!(!sp.is_dirty());
    }

    #[test]
    fn snapshot_does_not_reset_dirty() {
        let mut sp = StatePersistence::new("s", 5);
        sp.set_profile("x");
        sp.snapshot();
        // Snapshot does not clear dirty flag
        assert!(sp.is_dirty());
    }

    #[test]
    fn restore_latest_clears_dirty() {
        let mut sp = StatePersistence::new("s", 5);
        sp.set_profile("a");
        sp.snapshot();
        sp.set_profile("b");
        assert!(sp.is_dirty());
        sp.restore_latest();
        assert!(!sp.is_dirty());
    }

    /// Test a full lifecycle: configure → snapshot → modify → restore → verify
    #[test]
    fn full_lifecycle() {
        let mut sp = StatePersistence::new("session-1", 5);

        // Active state
        sp.set_profile("combat");
        sp.set_aircraft("F-18C");
        sp.set_sim("DCS");
        sp.set_device_config("stick-1", r#"{"type":"joystick"}"#);
        sp.set_preference("theme", "dark");
        sp.snapshot();

        // Modify (Paused-like)
        sp.set_profile("ga-default");
        sp.set_aircraft("C172");
        sp.set_sim("MSFS");
        sp.snapshot();

        assert_eq!(sp.snapshot_count(), 2);

        // Resume by restoring latest
        let restored = sp.restore_latest().unwrap();
        assert_eq!(restored.active_profile.as_deref(), Some("ga-default"));
        assert_eq!(restored.active_aircraft.as_deref(), Some("C172"));
        assert_eq!(restored.active_sim.as_deref(), Some("MSFS"));

        // Can still restore the earlier snapshot
        let earlier = sp.restore_latest().unwrap();
        assert_eq!(earlier.active_profile.as_deref(), Some("combat"));
        assert_eq!(earlier.active_aircraft.as_deref(), Some("F-18C"));
    }

    #[test]
    fn restore_from_empty_returns_none() {
        let mut sp = StatePersistence::new("s", 5);
        assert!(sp.restore_latest().is_none());
    }

    #[test]
    fn max_snapshots_evicts_oldest() {
        let mut sp = StatePersistence::new("s", 3);
        for i in 0..5 {
            sp.set_preference("val", &i.to_string());
            sp.snapshot();
        }
        assert_eq!(sp.snapshot_count(), 3);
        // Oldest (0 and 1) evicted; remaining are 2, 3, 4
        let oldest = sp.restore_by_index(0).unwrap();
        assert_eq!(oldest.preferences.get("val").unwrap(), "2");
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// 3. Event recording tests
// ══════════════════════════════════════════════════════════════════════════════

mod event_recording {
    use super::*;

    #[test]
    fn snapshot_records_timestamp() {
        let mut sp = StatePersistence::new("s", 5);
        sp.set_profile("test");
        sp.snapshot();
        let snap = sp.restore_by_index(0).unwrap();
        assert!(snap.last_save_timestamp > 0, "timestamp should be set by snapshot()");
    }

    #[test]
    fn multiple_snapshots_have_nondecreasing_timestamps() {
        let mut sp = StatePersistence::new("s", 10);
        for i in 0..5 {
            sp.set_preference("i", &i.to_string());
            sp.snapshot();
        }
        let mut prev_ts = 0u64;
        for i in 0..5 {
            let ts = sp.restore_by_index(i).unwrap().last_save_timestamp;
            assert!(ts >= prev_ts, "timestamps must be non-decreasing: {ts} < {prev_ts}");
            prev_ts = ts;
        }
    }

    #[test]
    fn device_config_overwrites() {
        let mut sp = StatePersistence::new("s", 5);
        sp.set_device_config("dev-1", "config-a");
        sp.set_device_config("dev-1", "config-b");
        sp.snapshot();
        let snap = sp.restore_by_index(0).unwrap();
        assert_eq!(snap.device_configs.get("dev-1").unwrap(), "config-b");
    }

    #[test]
    fn preference_stored_and_retrieved() {
        let mut sp = StatePersistence::new("s", 5);
        sp.set_preference("units", "metric");
        assert_eq!(sp.get_preference("units"), Some("metric"));
        assert_eq!(sp.get_preference("missing"), None);
    }

    #[test]
    fn multiple_preferences() {
        let mut sp = StatePersistence::new("s", 5);
        sp.set_preference("theme", "dark");
        sp.set_preference("units", "imperial");
        sp.set_preference("language", "en");
        assert_eq!(sp.get_preference("theme"), Some("dark"));
        assert_eq!(sp.get_preference("units"), Some("imperial"));
        assert_eq!(sp.get_preference("language"), Some("en"));
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// 4. Session persistence tests
// ══════════════════════════════════════════════════════════════════════════════

mod persistence_depth {
    use super::*;

    #[test]
    fn save_load_round_trip_full_state() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::new(dir.path().join("state.json"));

        let mut state = SessionState::default();
        state.active_profile = Some("combat".into());
        state.last_sim = Some("DCS".into());
        state.device_assignments.insert("stick-1".into(), "pitch_roll".into());
        state.window_positions.insert("main".into(), WindowPosition {
            x: 100, y: 200, width: 1920, height: 1080,
        });
        state.calibration_data.insert("stick-1".into(), CalibrationData {
            min: -1.0, max: 1.0, center: 0.0, deadzone: 0.05, timestamp: 1_700_000_000,
        });
        state.last_shutdown = Some(ShutdownInfo {
            timestamp: 1_700_000_100,
            reason: ShutdownReason::Clean,
        });

        store.save(&state).unwrap();
        let loaded = store.load().unwrap().expect("state should exist");
        assert_eq!(state, loaded);
    }

    #[test]
    fn save_load_default_state() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::new(dir.path().join("state.json"));
        let state = SessionState::default();
        store.save(&state).unwrap();
        let loaded = store.load().unwrap().unwrap();
        assert_eq!(state, loaded);
    }

    #[test]
    fn load_missing_returns_none() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::new(dir.path().join("nonexistent.json"));
        assert!(store.load().unwrap().is_none());
    }

    #[test]
    fn overwrite_preserves_latest() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::new(dir.path().join("state.json"));

        let mut state1 = SessionState::default();
        state1.active_profile = Some("alpha".into());
        store.save(&state1).unwrap();

        let mut state2 = SessionState::default();
        state2.active_profile = Some("beta".into());
        store.save(&state2).unwrap();

        let loaded = store.load().unwrap().unwrap();
        assert_eq!(loaded.active_profile.as_deref(), Some("beta"));
    }

    #[test]
    fn clear_removes_file() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::new(dir.path().join("state.json"));
        store.save(&SessionState::default()).unwrap();
        assert!(store.path().exists());
        store.clear().unwrap();
        assert!(!store.path().exists());
    }

    #[test]
    fn clear_missing_file_is_ok() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::new(dir.path().join("no.json"));
        store.clear().unwrap(); // must not panic
    }

    #[test]
    fn corrupt_file_returns_error() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("state.json");
        std::fs::write(&path, "not valid json {{{").unwrap();
        let store = SessionStore::new(path);
        assert!(store.load().is_err());
    }

    #[test]
    fn save_creates_parent_dirs() {
        let dir = TempDir::new().unwrap();
        let nested = dir.path().join("a").join("b").join("c").join("state.json");
        let store = SessionStore::new(nested);
        store.save(&SessionState::default()).unwrap();
        assert!(store.path().exists());
    }

    #[test]
    fn atomic_write_no_tmp_leftover() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::new(dir.path().join("state.json"));
        store.save(&SessionState::default()).unwrap();
        let entries = std::fs::read_dir(dir.path()).unwrap();
        for entry in entries {
            let path = entry.unwrap().path();
            let ext = path.extension().and_then(|e| e.to_str());
            assert_ne!(ext, Some("tmp"), "temporary files should be cleaned up after save");
        }
    }

    // StatePersistence JSON round-trip
    #[test]
    fn state_persistence_json_round_trip() {
        let mut sp = StatePersistence::new("session-42", 5);
        sp.set_profile("ga");
        sp.set_aircraft("A320");
        sp.set_sim("XPlane");
        sp.set_device_config("dev1", "{}");
        sp.set_preference("units", "metric");

        let json = sp.to_json();
        let restored = StatePersistence::from_json(&json).unwrap();
        assert_eq!(restored.session_id, "session-42");
        assert_eq!(restored.active_profile.as_deref(), Some("ga"));
        assert_eq!(restored.active_aircraft.as_deref(), Some("A320"));
        assert_eq!(restored.active_sim.as_deref(), Some("XPlane"));
        assert_eq!(restored.device_configs.get("dev1").unwrap(), "{}");
        assert_eq!(restored.preferences.get("units").unwrap(), "metric");
    }

    #[test]
    fn state_persistence_invalid_json_returns_error() {
        assert!(StatePersistence::from_json("not json").is_err());
    }

    // Shutdown reason variants
    #[test]
    fn shutdown_reason_variants_round_trip() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::new(dir.path().join("state.json"));

        for reason in [ShutdownReason::Clean, ShutdownReason::Crash, ShutdownReason::Unknown] {
            let mut state = SessionState::default();
            state.last_shutdown = Some(ShutdownInfo {
                timestamp: 12345,
                reason: reason.clone(),
            });
            store.save(&state).unwrap();
            let loaded = store.load().unwrap().unwrap();
            assert_eq!(loaded.last_shutdown.unwrap().reason, reason);
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// 5. Concurrent access tests
// ══════════════════════════════════════════════════════════════════════════════

mod concurrent_access {
    use super::*;

    #[test]
    fn concurrent_store_save_load() {
        let dir = TempDir::new().unwrap();

        // Each thread saves to its own path to avoid temp file races
        let handles: Vec<_> = (0..4)
            .map(|i| {
                let p = dir.path().join(format!("state_{i}.json"));
                std::thread::spawn(move || {
                    let store = SessionStore::new(&p);
                    let mut state = SessionState::default();
                    state.active_profile = Some(format!("profile_{i}"));
                    store.save(&state).unwrap();
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        // Verify each file independently contains a valid state
        for i in 0..4 {
            let p = dir.path().join(format!("state_{i}.json"));
            let store = SessionStore::new(&p);
            let loaded = store.load().unwrap().expect("state should exist");
            assert!(loaded.active_profile.is_some());
            let profile = loaded.active_profile.unwrap();
            assert_eq!(
                profile,
                format!("profile_{i}"),
                "profile should match the written value"
            );
        }
    }

    #[test]
    fn concurrent_save_then_sequential_load() {
        let dir = TempDir::new().unwrap();
        // Each thread writes to its own store file for isolation
        let handles: Vec<_> = (0..4)
            .map(|i| {
                let p = dir.path().join(format!("state_{i}.json"));
                std::thread::spawn(move || {
                    let store = SessionStore::new(&p);
                    let mut state = SessionState::default();
                    state.active_profile = Some(format!("profile_{i}"));
                    state.last_sim = Some(format!("sim_{i}"));
                    store.save(&state).unwrap();
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        // Verify each file independently
        for i in 0..4 {
            let p = dir.path().join(format!("state_{i}.json"));
            let store = SessionStore::new(&p);
            let loaded = store.load().unwrap().unwrap();
            let expected_profile = format!("profile_{i}");
            let expected_sim = format!("sim_{i}");
            assert_eq!(loaded.active_profile.as_deref(), Some(expected_profile.as_str()));
            assert_eq!(loaded.last_sim.as_deref(), Some(expected_sim.as_str()));
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// 6. Cleanup / recovery tests
// ══════════════════════════════════════════════════════════════════════════════

mod cleanup_depth {
    use super::*;
    use std::time::Duration;

    #[test]
    fn fresh_dir_no_recovery_needed() {
        let dir = TempDir::new().unwrap();
        let mgr = RecoveryManager::new(dir.path().join("session"));
        assert!(!mgr.needs_recovery().unwrap());
    }

    #[test]
    fn heartbeat_without_shutdown_needs_recovery() {
        let dir = TempDir::new().unwrap();
        let mgr = RecoveryManager::new(dir.path().join("session"));
        mgr.set_heartbeat().unwrap();
        assert!(mgr.needs_recovery().unwrap());
    }

    #[test]
    fn clean_shutdown_clears_heartbeat() {
        let dir = TempDir::new().unwrap();
        let mgr = RecoveryManager::new(dir.path().join("session"));
        mgr.set_heartbeat().unwrap();
        mgr.mark_clean_shutdown().unwrap();
        assert!(!mgr.needs_recovery().unwrap());
        assert!(mgr.check_clean_shutdown().unwrap());
    }

    #[test]
    fn recover_loads_persisted_state() {
        let dir = TempDir::new().unwrap();
        let mgr = RecoveryManager::new(dir.path().join("session"));

        let mut state = SessionState::default();
        state.active_profile = Some("combat".into());
        mgr.store().save(&state).unwrap();
        mgr.set_heartbeat().unwrap();

        assert!(mgr.needs_recovery().unwrap());

        let recovered = mgr.recover().unwrap().expect("state should exist");
        assert_eq!(recovered.active_profile.as_deref(), Some("combat"));
    }

    #[test]
    fn recover_no_state_returns_none() {
        let dir = TempDir::new().unwrap();
        let mgr = RecoveryManager::new(dir.path().join("session"));
        mgr.set_heartbeat().unwrap();
        assert!(mgr.recover().unwrap().is_none());
    }

    #[test]
    fn heartbeat_staleness_fresh_not_stale() {
        let dir = TempDir::new().unwrap();
        let mgr = RecoveryManager::new(dir.path().join("session"))
            .with_staleness_threshold(Duration::from_secs(60));
        mgr.set_heartbeat().unwrap();
        assert!(!mgr.is_heartbeat_stale().unwrap());
    }

    #[test]
    fn heartbeat_staleness_old_is_stale() {
        let dir = TempDir::new().unwrap();
        let session_dir = dir.path().join("session");
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::write(session_dir.join("heartbeat"), "1000000000").unwrap();

        let mgr = RecoveryManager::new(&session_dir)
            .with_staleness_threshold(Duration::from_secs(10));
        assert!(mgr.is_heartbeat_stale().unwrap());
    }

    #[test]
    fn no_heartbeat_not_stale() {
        let dir = TempDir::new().unwrap();
        let mgr = RecoveryManager::new(dir.path().join("session"));
        assert!(!mgr.is_heartbeat_stale().unwrap());
    }

    #[test]
    fn save_and_mark_shutdown_persists_clean() {
        let dir = TempDir::new().unwrap();
        let mgr = RecoveryManager::new(dir.path().join("session"));

        let state = SessionState::default();
        mgr.save_and_mark_shutdown(&state, ShutdownReason::Clean).unwrap();

        assert!(mgr.check_clean_shutdown().unwrap());
        let loaded = mgr.store().load().unwrap().unwrap();
        assert_eq!(loaded.last_shutdown.unwrap().reason, ShutdownReason::Clean);
    }

    #[test]
    fn save_and_mark_shutdown_crash_reason() {
        let dir = TempDir::new().unwrap();
        let mgr = RecoveryManager::new(dir.path().join("session"));

        let state = SessionState::default();
        mgr.save_and_mark_shutdown(&state, ShutdownReason::Crash).unwrap();

        let loaded = mgr.store().load().unwrap().unwrap();
        assert_eq!(loaded.last_shutdown.unwrap().reason, ShutdownReason::Crash);
    }

    #[test]
    fn recovery_then_clean_shutdown_cycle() {
        let dir = TempDir::new().unwrap();
        let mgr = RecoveryManager::new(dir.path().join("session"));

        // Simulate crash: heartbeat but no clean shutdown
        let mut state = SessionState::default();
        state.active_profile = Some("crashed_session".into());
        mgr.store().save(&state).unwrap();
        mgr.set_heartbeat().unwrap();
        assert!(mgr.needs_recovery().unwrap());

        // Recover
        let recovered = mgr.recover().unwrap().unwrap();
        assert_eq!(recovered.active_profile.as_deref(), Some("crashed_session"));

        // Now run cleanly
        mgr.set_heartbeat().unwrap();
        let mut new_state = SessionState::default();
        new_state.active_profile = Some("clean_session".into());
        mgr.save_and_mark_shutdown(&new_state, ShutdownReason::Clean).unwrap();

        assert!(!mgr.needs_recovery().unwrap());
        let loaded = mgr.store().load().unwrap().unwrap();
        assert_eq!(loaded.active_profile.as_deref(), Some("clean_session"));
    }

    #[test]
    fn store_clear_then_recover_returns_none() {
        let dir = TempDir::new().unwrap();
        let mgr = RecoveryManager::new(dir.path().join("session"));

        let state = SessionState::default();
        mgr.store().save(&state).unwrap();
        mgr.set_heartbeat().unwrap();

        // Clear the state file
        mgr.store().clear().unwrap();

        // Recovery should return None since state was cleared
        let recovered = mgr.recover().unwrap();
        assert!(recovered.is_none());
    }

    #[test]
    fn multiple_heartbeats_overwrite() {
        let dir = TempDir::new().unwrap();
        let mgr = RecoveryManager::new(dir.path().join("session"));
        mgr.set_heartbeat().unwrap();
        mgr.set_heartbeat().unwrap();
        // Should still need recovery (heartbeat present, no clean shutdown)
        assert!(mgr.needs_recovery().unwrap());
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// 7. Property tests
// ══════════════════════════════════════════════════════════════════════════════

mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// Session state JSON round-trips preserve all fields.
        #[test]
        fn state_persistence_json_round_trip(
            session_id in "[a-z0-9-]{1,20}",
            profile in proptest::option::of("[a-zA-Z0-9_-]{1,20}"),
            aircraft in proptest::option::of("[A-Z0-9]{3,5}"),
            sim in proptest::option::of("[a-zA-Z]{3,10}"),
        ) {
            let mut sp = StatePersistence::new(&session_id, 5);
            if let Some(p) = &profile {
                sp.set_profile(p);
            }
            if let Some(a) = &aircraft {
                sp.set_aircraft(a);
            }
            if let Some(s) = &sim {
                sp.set_sim(s);
            }

            let json = sp.to_json();
            let restored = StatePersistence::from_json(&json).unwrap();
            prop_assert_eq!(restored.session_id, session_id);
            prop_assert_eq!(restored.active_profile.as_deref(), profile.as_deref());
            prop_assert_eq!(restored.active_aircraft.as_deref(), aircraft.as_deref());
            prop_assert_eq!(restored.active_sim.as_deref(), sim.as_deref());
        }

        /// Snapshot count never exceeds max_snapshots.
        #[test]
        fn snapshot_count_bounded(
            max_snaps in 1usize..20,
            n_ops in 0usize..50,
        ) {
            let mut sp = StatePersistence::new("s", max_snaps);
            for i in 0..n_ops {
                sp.set_preference("i", &i.to_string());
                sp.snapshot();
            }
            prop_assert!(sp.snapshot_count() <= max_snaps);
        }

        /// Store save/load round-trip preserves device assignments.
        #[test]
        fn store_round_trip_device_assignments(
            assignments in proptest::collection::hash_map(
                "[a-z]{2,8}",
                "[a-z_]{2,10}",
                0..5
            )
        ) {
            let dir = TempDir::new().unwrap();
            let store = SessionStore::new(dir.path().join("state.json"));
            let mut state = SessionState::default();
            state.device_assignments = assignments.clone();
            store.save(&state).unwrap();
            let loaded = store.load().unwrap().unwrap();
            prop_assert_eq!(loaded.device_assignments, assignments);
        }

        /// Window positions round-trip correctly.
        #[test]
        fn store_round_trip_window_positions(
            x in -10000i32..10000,
            y in -10000i32..10000,
            w in 1u32..5000,
            h in 1u32..5000,
        ) {
            let dir = TempDir::new().unwrap();
            let store = SessionStore::new(dir.path().join("state.json"));
            let mut state = SessionState::default();
            state.window_positions.insert("main".into(), WindowPosition {
                x, y, width: w, height: h,
            });
            store.save(&state).unwrap();
            let loaded = store.load().unwrap().unwrap();
            let pos = loaded.window_positions.get("main").unwrap();
            prop_assert_eq!(pos.x, x);
            prop_assert_eq!(pos.y, y);
            prop_assert_eq!(pos.width, w);
            prop_assert_eq!(pos.height, h);
        }

        /// Calibration data round-trips correctly.
        #[test]
        fn store_round_trip_calibration(
            min_val in -10.0f64..0.0,
            max_val in 0.0f64..10.0,
            center in -1.0f64..1.0,
            deadzone in 0.0f64..0.5,
            ts in 0u64..u64::MAX,
        ) {
            let dir = TempDir::new().unwrap();
            let store = SessionStore::new(dir.path().join("state.json"));
            let mut state = SessionState::default();
            state.calibration_data.insert("stick-1".into(), CalibrationData {
                min: min_val, max: max_val, center, deadzone, timestamp: ts,
            });
            store.save(&state).unwrap();
            let loaded = store.load().unwrap().unwrap();
            let cal = loaded.calibration_data.get("stick-1").unwrap();
            let tol = 1e-10;
            prop_assert!((cal.min - min_val).abs() < tol);
            prop_assert!((cal.max - max_val).abs() < tol);
            prop_assert!((cal.center - center).abs() < tol);
            prop_assert!((cal.deadzone - deadzone).abs() < tol);
            prop_assert_eq!(cal.timestamp, ts);
        }
    }
}
