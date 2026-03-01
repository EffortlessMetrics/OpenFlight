// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for `flight-session`.
//!
//! Covers session lifecycle (create → record → end), session ID uniqueness,
//! duration tracking, concurrent sessions, persistence round-trips,
//! history limits, and corrupted-file resilience.

use flight_session::recovery::RecoveryManager;
use flight_session::state_persistence::StatePersistence;
use flight_session::store::{
    CalibrationData, SessionState, SessionStore, ShutdownInfo, ShutdownReason, WindowPosition,
};
use std::collections::HashSet;
use tempfile::TempDir;

// ═══════════════════════════════════════════════════════════════════════════
// 6. Session lifecycle
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn create_session_record_events_end_session() {
    let mut sp = StatePersistence::new("session-1", 10);

    // Create: fresh session
    assert!(!sp.is_dirty());
    assert_eq!(sp.snapshot_count(), 0);

    // Record events
    sp.set_profile("combat");
    sp.set_aircraft("F18");
    sp.set_sim("DCS");
    sp.set_device_config("stick-1", r#"{"deadzone":0.05}"#);
    sp.set_preference("theme", "dark");
    assert!(sp.is_dirty());

    // Snapshot (end of session checkpoint)
    sp.snapshot();
    assert_eq!(sp.snapshot_count(), 1);

    let snap = sp.restore_by_index(0).unwrap();
    assert_eq!(snap.session_id, "session-1");
    assert_eq!(snap.active_profile.as_deref(), Some("combat"));
    assert_eq!(snap.active_aircraft.as_deref(), Some("F18"));
    assert_eq!(snap.active_sim.as_deref(), Some("DCS"));
    assert!(snap.last_save_timestamp > 0);
}

#[test]
fn session_id_uniqueness() {
    let mut ids = HashSet::new();
    for i in 0..100 {
        let sp = StatePersistence::new(&format!("session-{i}"), 5);
        let json = sp.to_json();
        let state = StatePersistence::from_json(&json).unwrap();
        assert!(
            ids.insert(state.session_id.clone()),
            "session ID '{}' must be unique",
            state.session_id
        );
    }
    assert_eq!(ids.len(), 100);
}

#[test]
fn session_duration_tracking_via_timestamps() {
    let mut sp = StatePersistence::new("dur-test", 10);

    sp.set_profile("first");
    sp.snapshot();
    let t1 = sp.restore_by_index(0).unwrap().last_save_timestamp;

    // Ensure a minimal time difference (may be 0 on fast machines)
    std::thread::sleep(std::time::Duration::from_millis(10));

    sp.set_profile("second");
    sp.snapshot();
    let t2 = sp.restore_by_index(1).unwrap().last_save_timestamp;

    assert!(t2 >= t1, "second snapshot timestamp must be >= first");
}

#[test]
fn concurrent_session_instances() {
    let handles: Vec<_> = (0..8)
        .map(|i| {
            std::thread::spawn(move || {
                let mut sp = StatePersistence::new(&format!("thread-{i}"), 5);
                sp.set_profile(&format!("profile-{i}"));
                sp.set_aircraft(&format!("aircraft-{i}"));
                sp.snapshot();

                let json = sp.to_json();
                let state = StatePersistence::from_json(&json).unwrap();
                assert_eq!(state.session_id, format!("thread-{i}"));
                assert_eq!(state.active_profile.as_deref(), Some(format!("profile-{i}").as_str()));
            })
        })
        .collect();

    for h in handles {
        h.join().expect("thread panicked");
    }
}

#[test]
fn session_restore_latest_returns_most_recent() {
    let mut sp = StatePersistence::new("restore-test", 10);
    sp.set_profile("old");
    sp.snapshot();
    sp.set_profile("new");
    sp.snapshot();

    let latest = sp.restore_latest().unwrap();
    assert_eq!(latest.active_profile.as_deref(), Some("new"));
    assert_eq!(sp.snapshot_count(), 1); // one snapshot remains
}

#[test]
fn session_json_round_trip_all_fields() {
    let mut sp = StatePersistence::new("roundtrip", 5);
    sp.set_profile("ga");
    sp.set_aircraft("C172");
    sp.set_sim("MSFS");
    sp.set_device_config("throttle", r#"{"axes":2}"#);
    sp.set_preference("units", "imperial");

    let json = sp.to_json();
    let restored = StatePersistence::from_json(&json).unwrap();

    assert_eq!(restored.session_id, "roundtrip");
    assert_eq!(restored.active_profile.as_deref(), Some("ga"));
    assert_eq!(restored.active_aircraft.as_deref(), Some("C172"));
    assert_eq!(restored.active_sim.as_deref(), Some("MSFS"));
    assert_eq!(restored.device_configs.get("throttle").unwrap(), r#"{"axes":2}"#);
    assert_eq!(restored.preferences.get("units").unwrap(), "imperial");
}

// ═══════════════════════════════════════════════════════════════════════════
// 7. Session persistence (SessionStore)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn save_and_load_session_to_disk() {
    let dir = TempDir::new().unwrap();
    let store = SessionStore::new(dir.path().join("state.json"));

    let mut state = SessionState::default();
    state.active_profile = Some("combat".into());
    state.last_sim = Some("DCS".into());
    state.device_assignments.insert("stick".into(), "roll".into());

    store.save(&state).unwrap();
    let loaded = store.load().unwrap().expect("state should exist");
    assert_eq!(state, loaded);
}

#[test]
fn load_previous_sessions_overwrite() {
    let dir = TempDir::new().unwrap();
    let store = SessionStore::new(dir.path().join("state.json"));

    let mut state1 = SessionState::default();
    state1.active_profile = Some("alpha".into());
    store.save(&state1).unwrap();

    let mut state2 = SessionState::default();
    state2.active_profile = Some("beta".into());
    store.save(&state2).unwrap();

    let loaded = store.load().unwrap().unwrap();
    assert_eq!(
        loaded.active_profile.as_deref(),
        Some("beta"),
        "latest save must win"
    );
}

#[test]
fn session_history_limits_via_state_persistence() {
    let mut sp = StatePersistence::new("history", 3);
    for i in 0..6 {
        sp.set_preference("v", &i.to_string());
        sp.snapshot();
    }

    assert_eq!(sp.snapshot_count(), 3, "max_snapshots should be enforced");

    // Oldest three (0,1,2) were dropped; remaining are 3,4,5
    let oldest = sp.restore_by_index(0).unwrap();
    assert_eq!(oldest.preferences.get("v").unwrap(), "3");
    let newest = sp.restore_by_index(2).unwrap();
    assert_eq!(newest.preferences.get("v").unwrap(), "5");
}

#[test]
fn corrupted_session_file_returns_error() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("state.json");

    // Write garbage
    std::fs::write(&path, "{{{{not valid json!!!!").unwrap();

    let store = SessionStore::new(path);
    assert!(store.load().is_err(), "corrupted file must return Err");
}

#[test]
fn corrupted_session_file_does_not_prevent_new_save() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("state.json");

    std::fs::write(&path, "garbage").unwrap();
    let store = SessionStore::new(&path);
    assert!(store.load().is_err());

    // Saving a new valid state must succeed
    let state = SessionState::default();
    store.save(&state).unwrap();
    let loaded = store.load().unwrap().unwrap();
    assert_eq!(loaded, state);
}

#[test]
fn session_store_clear_then_load_returns_none() {
    let dir = TempDir::new().unwrap();
    let store = SessionStore::new(dir.path().join("state.json"));

    store.save(&SessionState::default()).unwrap();
    store.clear().unwrap();
    assert!(store.load().unwrap().is_none());
}

#[test]
fn session_state_with_all_fields_round_trips() {
    let dir = TempDir::new().unwrap();
    let store = SessionStore::new(dir.path().join("state.json"));

    let mut state = SessionState::default();
    state.active_profile = Some("aerobatics".into());
    state.last_sim = Some("XPlane".into());
    state
        .device_assignments
        .insert("rudder".into(), "yaw".into());
    state.window_positions.insert(
        "settings".into(),
        WindowPosition {
            x: 50,
            y: 100,
            width: 640,
            height: 480,
        },
    );
    state.calibration_data.insert(
        "stick-2".into(),
        CalibrationData {
            min: -1.0,
            max: 1.0,
            center: 0.02,
            deadzone: 0.03,
            timestamp: 1_700_000_500,
        },
    );
    state.last_shutdown = Some(ShutdownInfo {
        timestamp: 1_700_001_000,
        reason: ShutdownReason::Crash,
    });

    store.save(&state).unwrap();
    let loaded = store.load().unwrap().unwrap();
    assert_eq!(state, loaded);
}

// ═══════════════════════════════════════════════════════════════════════════
// Recovery manager integration
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn recovery_lifecycle_heartbeat_crash_recover() {
    let dir = TempDir::new().unwrap();
    let mgr = RecoveryManager::new(dir.path().join("session"));

    // Save state and set heartbeat (simulating running service)
    let mut state = SessionState::default();
    state.active_profile = Some("saved-profile".into());
    mgr.store().save(&state).unwrap();
    mgr.set_heartbeat().unwrap();

    // Simulate crash: no clean shutdown marker
    assert!(mgr.needs_recovery().unwrap());

    // Recover
    let recovered = mgr.recover().unwrap().expect("state should exist");
    assert_eq!(recovered.active_profile.as_deref(), Some("saved-profile"));
}

#[test]
fn clean_shutdown_prevents_recovery() {
    let dir = TempDir::new().unwrap();
    let mgr = RecoveryManager::new(dir.path().join("session"));

    mgr.set_heartbeat().unwrap();
    mgr.mark_clean_shutdown().unwrap();

    assert!(!mgr.needs_recovery().unwrap());
    assert!(mgr.check_clean_shutdown().unwrap());
}

#[test]
fn save_and_mark_shutdown_persists_reason() {
    let dir = TempDir::new().unwrap();
    let mgr = RecoveryManager::new(dir.path().join("session"));

    let state = SessionState::default();
    mgr.save_and_mark_shutdown(&state, ShutdownReason::Clean)
        .unwrap();

    let loaded = mgr.store().load().unwrap().unwrap();
    assert_eq!(loaded.last_shutdown.unwrap().reason, ShutdownReason::Clean);
}

// ═══════════════════════════════════════════════════════════════════════════
// Property-based tests
// ═══════════════════════════════════════════════════════════════════════════

use proptest::prelude::*;

proptest! {
    #[test]
    fn state_persistence_json_round_trip(
        profile in "[a-z]{3,8}",
        aircraft in "[A-Z][0-9]{2,4}",
        sim in "(MSFS|XPlane|DCS)",
    ) {
        let mut sp = StatePersistence::new("prop-test", 5);
        sp.set_profile(&profile);
        sp.set_aircraft(&aircraft);
        sp.set_sim(&sim);

        let json = sp.to_json();
        let restored = StatePersistence::from_json(&json).unwrap();
        prop_assert_eq!(restored.active_profile.as_deref(), Some(profile.as_str()));
        prop_assert_eq!(restored.active_aircraft.as_deref(), Some(aircraft.as_str()));
        prop_assert_eq!(restored.active_sim.as_deref(), Some(sim.as_str()));
    }

    #[test]
    fn snapshot_count_never_exceeds_max(max in 1usize..20, writes in 1usize..50) {
        let mut sp = StatePersistence::new("limit-test", max);
        for i in 0..writes {
            sp.set_preference("i", &i.to_string());
            sp.snapshot();
        }
        prop_assert!(sp.snapshot_count() <= max);
    }
}
