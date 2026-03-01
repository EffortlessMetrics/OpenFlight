// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Config reload integration tests.
//!
//! Exercises [`ConfigWatcher`] file change detection and the
//! [`HotReloadTracker`] profile hot-reload pipeline, verifying that
//! file modifications trigger the correct reconfiguration actions.

use flight_core::profile::{AxisConfig, PROFILE_SCHEMA_VERSION, Profile};
use flight_profile::hot_reload::{FileState, HotReloadTracker, ReloadAction};
use flight_service::config_watcher::{ChangeType, ConfigWatcher};
use std::collections::HashMap;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn file_state(path: &str, hash: u64) -> FileState {
    FileState {
        path: path.to_string(),
        hash,
        last_modified: 1000,
        size: 100,
    }
}

fn test_profile(dz: f32) -> Profile {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(dz),
            expo: Some(0.2),
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes,
        pof_overrides: None,
    }
}

// ===========================================================================
// 1. ConfigWatcher detects file creation via tempfile
// ===========================================================================

#[test]
fn config_watcher_detects_file_creation() {
    let temp_dir = std::env::temp_dir().join("flight_test_config_create");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();
    let file_path = temp_dir.join("profile.json");

    let mut watcher = ConfigWatcher::new(Duration::from_millis(100));
    watcher.watch(&file_path);

    // File doesn't exist yet — no changes
    let changes = watcher.check_for_changes();
    assert!(changes.is_empty(), "no file yet = no changes");

    // Create the file
    let profile = test_profile(0.05);
    let json = serde_json::to_string_pretty(&profile).unwrap();
    std::fs::write(&file_path, &json).unwrap();

    // Now watcher should detect creation
    let changes = watcher.check_for_changes();
    assert_eq!(changes.len(), 1, "must detect file creation");
    assert_eq!(changes[0].change_type, ChangeType::Created);

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}

// ===========================================================================
// 2. ConfigWatcher detects file modification
// ===========================================================================

#[test]
fn config_watcher_detects_file_modification() {
    let temp_dir = std::env::temp_dir().join("flight_test_config_modify");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();
    let file_path = temp_dir.join("profile.json");

    // Create initial file
    let profile_v1 = test_profile(0.05);
    std::fs::write(&file_path, serde_json::to_string(&profile_v1).unwrap()).unwrap();

    let mut watcher = ConfigWatcher::new(Duration::from_millis(100));
    watcher.watch(&file_path);

    // First check establishes baseline (Creation since watcher had no prior state)
    let _initial = watcher.check_for_changes();

    // Modify the file
    std::thread::sleep(Duration::from_millis(50));
    let profile_v2 = test_profile(0.10);
    std::fs::write(&file_path, serde_json::to_string(&profile_v2).unwrap()).unwrap();

    // Should detect modification
    let changes = watcher.check_for_changes();
    assert_eq!(changes.len(), 1, "must detect modification");
    assert_eq!(changes[0].change_type, ChangeType::Modified);

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}

// ===========================================================================
// 3. ConfigWatcher detects file deletion
// ===========================================================================

#[test]
fn config_watcher_detects_file_deletion() {
    let temp_dir = std::env::temp_dir().join("flight_test_config_delete");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();
    let file_path = temp_dir.join("profile.json");

    // Create file
    std::fs::write(&file_path, "{}").unwrap();

    let mut watcher = ConfigWatcher::new(Duration::from_millis(100));
    watcher.watch(&file_path);
    let _ = watcher.check_for_changes(); // establish baseline

    // Delete the file
    std::fs::remove_file(&file_path).unwrap();

    let changes = watcher.check_for_changes();
    assert_eq!(changes.len(), 1, "must detect deletion");
    assert_eq!(changes[0].change_type, ChangeType::Deleted);

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}

// ===========================================================================
// 4. HotReloadTracker: hash change triggers reload action
// ===========================================================================

#[test]
fn hot_reload_hash_change_triggers_reload() {
    let mut tracker = HotReloadTracker::new(0);
    tracker.track("global.json".to_string(), file_state("global.json", 100));

    let actions = tracker.check_changes(&[file_state("global.json", 200)], 100);
    assert_eq!(
        actions,
        vec![ReloadAction::Reload("global.json".to_string())]
    );
}

// ===========================================================================
// 5. Full cycle: detect change → reload → validate → apply
// ===========================================================================

#[test]
fn config_reload_full_cycle_detect_validate_apply() {
    let mut tracker = HotReloadTracker::new(0);

    // Track initial profile
    tracker.track("combat.json".to_string(), file_state("combat.json", 1));

    // Simulate file edit (hash changes)
    let actions = tracker.check_changes(&[file_state("combat.json", 2)], 100);
    assert_eq!(actions.len(), 1, "must detect hash change");
    assert_eq!(actions[0], ReloadAction::Reload("combat.json".to_string()));

    // Simulate loading and validating the new profile
    let new_profile = test_profile(0.08);
    assert!(
        new_profile.validate().is_ok(),
        "reloaded profile must validate"
    );

    // Verify no further change after reload
    tracker.track("combat.json".to_string(), file_state("combat.json", 2));
    let actions = tracker.check_changes(&[file_state("combat.json", 2)], 200);
    assert!(actions.is_empty(), "no change after reload completes");
}

// ===========================================================================
// 6. ConfigWatcher enable/disable toggle
// ===========================================================================

#[test]
fn config_watcher_disabled_returns_no_changes() {
    let temp_dir = std::env::temp_dir().join("flight_test_config_disable");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();
    let file_path = temp_dir.join("profile.json");

    std::fs::write(&file_path, "{}").unwrap();

    let mut watcher = ConfigWatcher::new(Duration::from_millis(100));
    watcher.watch(&file_path);

    // Disable the watcher
    watcher.disable();
    assert!(!watcher.is_enabled());

    // Even with a file present, disabled watcher returns nothing
    let changes = watcher.check_for_changes();
    assert!(
        changes.is_empty(),
        "disabled watcher must return no changes"
    );

    // Re-enable
    watcher.enable();
    assert!(watcher.is_enabled());
    let changes = watcher.check_for_changes();
    assert!(!changes.is_empty(), "re-enabled watcher detects file");

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}

// ===========================================================================
// 7. Multiple files tracked simultaneously
// ===========================================================================

#[test]
fn hot_reload_multiple_files_tracked() {
    let mut tracker = HotReloadTracker::new(0);
    tracker.track("global.json".to_string(), file_state("global.json", 10));
    tracker.track("msfs.json".to_string(), file_state("msfs.json", 20));
    tracker.track("c172.json".to_string(), file_state("c172.json", 30));

    // Only msfs.json changed
    let actions = tracker.check_changes(
        &[
            file_state("global.json", 10), // unchanged
            file_state("msfs.json", 99),   // changed
            file_state("c172.json", 30),   // unchanged
        ],
        100,
    );

    assert_eq!(actions.len(), 1, "only msfs.json changed");
    assert_eq!(actions[0], ReloadAction::Reload("msfs.json".to_string()));
}
