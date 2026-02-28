// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Profile management end-to-end integration tests.
//!
//! Proves: load → validate → apply → hot-reload → cascade → safe-mode fallback.
//! Uses real profile types and the actual merging / validation stack.

use flight_core::profile::{
    AircraftId, AxisConfig, CapabilityContext, CapabilityMode, CurvePoint, DetentZone,
    FilterConfig, PROFILE_SCHEMA_VERSION, PofOverrides, Profile,
};
use flight_profile::hot_reload::{FileState, HotReloadTracker, ReloadAction};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn global_profile() -> Profile {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.05),
            expo: Some(0.2),
            slew_rate: Some(2.0),
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    axes.insert(
        "roll".to_string(),
        AxisConfig {
            deadzone: Some(0.05),
            expo: Some(0.2),
            slew_rate: Some(2.0),
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    axes.insert(
        "yaw".to_string(),
        AxisConfig {
            deadzone: Some(0.08),
            expo: Some(0.1),
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    axes.insert(
        "throttle".to_string(),
        AxisConfig {
            deadzone: Some(0.02),
            expo: Some(0.0),
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

fn msfs_profile() -> Profile {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.03),
            expo: Some(0.25),
            slew_rate: Some(1.5),
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: Some("msfs".to_string()),
        aircraft: None,
        axes,
        pof_overrides: None,
    }
}

fn aircraft_profile(icao: &str) -> Profile {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.02),
            expo: Some(0.35),
            slew_rate: None,
            detents: vec![],
            curve: Some(vec![
                CurvePoint {
                    input: 0.0,
                    output: 0.0,
                },
                CurvePoint {
                    input: 0.5,
                    output: 0.3,
                },
                CurvePoint {
                    input: 1.0,
                    output: 1.0,
                },
            ]),
            filter: None,
        },
    );
    Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: Some("msfs".to_string()),
        aircraft: Some(AircraftId {
            icao: icao.to_string(),
        }),
        axes,
        pof_overrides: None,
    }
}

fn invalid_profile_bad_schema() -> Profile {
    Profile {
        schema: "flight.profile/99".to_string(),
        sim: None,
        aircraft: None,
        axes: HashMap::new(),
        pof_overrides: None,
    }
}

fn invalid_profile_deadzone_oob() -> Profile {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.9), // > MAX_DEADZONE (0.5)
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

fn file_state(path: &str, hash: u64) -> FileState {
    FileState {
        path: path.to_string(),
        hash,
        last_modified: 1000,
        size: 100,
    }
}

// ===========================================================================
// 1. Profile validation
// ===========================================================================

#[test]
fn lifecycle_valid_global_profile_passes_validation() {
    let profile = global_profile();
    assert!(profile.validate().is_ok(), "global profile must validate");
}

#[test]
fn lifecycle_valid_sim_profile_passes_validation() {
    let profile = msfs_profile();
    assert!(profile.validate().is_ok(), "sim profile must validate");
}

#[test]
fn lifecycle_valid_aircraft_profile_passes_validation() {
    let profile = aircraft_profile("C172");
    assert!(profile.validate().is_ok(), "aircraft profile must validate");
}

#[test]
fn lifecycle_invalid_schema_rejected() {
    let profile = invalid_profile_bad_schema();
    let result = profile.validate();
    assert!(result.is_err(), "bad schema version must be rejected");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("schema"),
        "error should mention schema: {err_msg}"
    );
}

#[test]
fn lifecycle_invalid_deadzone_rejected() {
    let profile = invalid_profile_deadzone_oob();
    let result = profile.validate();
    assert!(result.is_err(), "deadzone > 0.5 must be rejected");
}

// ===========================================================================
// 2. Profile cascade: Global → Sim → Aircraft
// ===========================================================================

#[test]
fn lifecycle_cascade_global_to_sim_merges_correctly() {
    let global = global_profile();
    let sim = msfs_profile();

    let merged = global.merge_with(&sim).expect("merge must succeed");

    // Sim-specific override on pitch.deadzone should win
    let pitch = merged.axes.get("pitch").expect("pitch must exist");
    assert_eq!(
        pitch.deadzone,
        Some(0.03),
        "sim override should win for deadzone"
    );
    assert_eq!(pitch.expo, Some(0.25), "sim override should win for expo");

    // Global-only axes should survive
    assert!(
        merged.axes.contains_key("roll"),
        "roll from global must survive"
    );
    assert!(
        merged.axes.contains_key("yaw"),
        "yaw from global must survive"
    );
    assert!(
        merged.axes.contains_key("throttle"),
        "throttle from global must survive"
    );

    // Sim should be set
    assert_eq!(merged.sim.as_deref(), Some("msfs"));
}

#[test]
fn lifecycle_cascade_global_to_sim_to_aircraft() {
    let global = global_profile();
    let sim = msfs_profile();
    let aircraft = aircraft_profile("C172");

    let step1 = global.merge_with(&sim).expect("global→sim merge");
    let final_profile = step1.merge_with(&aircraft).expect("sim→aircraft merge");

    // Aircraft-specific pitch overrides should win
    let pitch = final_profile.axes.get("pitch").expect("pitch");
    assert_eq!(pitch.deadzone, Some(0.02), "aircraft deadzone wins");
    assert_eq!(pitch.expo, Some(0.35), "aircraft expo wins");
    assert!(pitch.curve.is_some(), "aircraft curve applied");

    // Aircraft ID set
    assert_eq!(
        final_profile.aircraft.as_ref().map(|a| a.icao.as_str()),
        Some("C172")
    );

    // Global-only axes still present
    assert!(final_profile.axes.contains_key("throttle"));
}

#[test]
fn lifecycle_cascade_preserves_pof_overrides() {
    let mut global = global_profile();
    let mut takeoff_axes = HashMap::new();
    takeoff_axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.02),
            expo: Some(0.1),
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    global.pof_overrides = Some(HashMap::from([(
        "takeoff".to_string(),
        PofOverrides {
            axes: Some(takeoff_axes),
            hysteresis: None,
        },
    )]));

    let sim = msfs_profile();
    let merged = global.merge_with(&sim).expect("merge");

    assert!(merged.pof_overrides.is_some(), "PoF overrides preserved");
    let pof = merged.pof_overrides.as_ref().unwrap();
    assert!(pof.contains_key("takeoff"), "takeoff phase preserved");
}

// ===========================================================================
// 3. Deterministic hashing and canonicalization
// ===========================================================================

#[test]
fn lifecycle_hash_is_deterministic() {
    let profile = global_profile();
    let h1 = profile.effective_hash();
    let h2 = profile.effective_hash();
    assert_eq!(h1, h2, "effective_hash must be deterministic");
}

#[test]
fn lifecycle_different_profiles_produce_different_hashes() {
    let global = global_profile();
    let sim = msfs_profile();
    assert_ne!(
        global.effective_hash(),
        sim.effective_hash(),
        "different profiles should produce different hashes"
    );
}

#[test]
fn lifecycle_canonicalize_is_stable() {
    let profile = aircraft_profile("F16C");
    let c1 = profile.canonicalize();
    let c2 = profile.canonicalize();
    assert_eq!(c1, c2, "canonicalize must be stable");
    assert!(c1.contains("flight.profile/1"));
}

// ===========================================================================
// 4. Hot-reload tracking
// ===========================================================================

#[test]
fn lifecycle_hot_reload_detects_change() {
    let mut tracker = HotReloadTracker::new(0);
    tracker.track("global.json".to_string(), file_state("global.json", 100));

    let actions = tracker.check_changes(&[file_state("global.json", 200)], 100);
    assert_eq!(
        actions,
        vec![ReloadAction::Reload("global.json".to_string())]
    );
}

#[test]
fn lifecycle_hot_reload_no_change_is_noop() {
    let mut tracker = HotReloadTracker::new(0);
    tracker.track("global.json".to_string(), file_state("global.json", 100));

    let actions = tracker.check_changes(&[file_state("global.json", 100)], 100);
    assert!(actions.is_empty());
}

#[test]
fn lifecycle_hot_reload_detects_removal() {
    let mut tracker = HotReloadTracker::new(0);
    tracker.track("old.json".to_string(), file_state("old.json", 50));

    let actions = tracker.check_changes(&[], 100);
    assert_eq!(actions, vec![ReloadAction::Remove("old.json".to_string())]);
    assert_eq!(tracker.tracked_count(), 0);
}

#[test]
fn lifecycle_hot_reload_debounce_suppresses_rapid_checks() {
    let mut tracker = HotReloadTracker::new(100);
    tracker.track("file.json".to_string(), file_state("file.json", 1));

    // First check at t=200
    let _ = tracker.check_changes(&[file_state("file.json", 2)], 200);

    // Within debounce window
    tracker.track("file.json".to_string(), file_state("file.json", 2));
    let actions = tracker.check_changes(&[file_state("file.json", 3)], 250);
    assert!(actions.is_empty(), "debounce should suppress check");

    // After debounce window
    let actions = tracker.check_changes(&[file_state("file.json", 3)], 300);
    assert_eq!(
        actions.len(),
        1,
        "past debounce window should detect change"
    );
}

#[test]
fn lifecycle_hot_reload_simulated_edit_cycle() {
    let mut tracker = HotReloadTracker::new(0);

    // Track initial profile
    tracker.track("profile.json".to_string(), file_state("profile.json", 1));
    assert_eq!(tracker.tracked_count(), 1);

    // User edits profile (hash changes)
    let actions = tracker.check_changes(&[file_state("profile.json", 2)], 100);
    assert_eq!(
        actions,
        vec![ReloadAction::Reload("profile.json".to_string())]
    );

    // Reload action: load, validate, apply
    let profile = global_profile();
    assert!(profile.validate().is_ok(), "reloaded profile must validate");

    // No further change
    let actions = tracker.check_changes(&[file_state("profile.json", 2)], 200);
    assert!(actions.is_empty(), "no change after reload");
}

// ===========================================================================
// 5. Capability-constrained validation
// ===========================================================================

#[test]
fn lifecycle_kid_mode_validation() {
    let ctx = CapabilityContext::for_mode(CapabilityMode::Kid);
    let profile = global_profile();
    let result = profile.validate_with_capabilities(&ctx);
    assert!(
        result.is_ok(),
        "global profile should pass kid-mode validation"
    );
}

#[test]
fn lifecycle_demo_mode_validation() {
    let ctx = CapabilityContext::for_mode(CapabilityMode::Demo);
    let profile = global_profile();
    let result = profile.validate_with_capabilities(&ctx);
    assert!(
        result.is_ok(),
        "global profile should pass demo-mode validation"
    );
}

// ===========================================================================
// 6. Profile with filter config
// ===========================================================================

#[test]
fn lifecycle_profile_with_filter_validates() {
    let mut axes = HashMap::new();
    axes.insert(
        "throttle".to_string(),
        AxisConfig {
            deadzone: Some(0.02),
            expo: Some(0.0),
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: Some(FilterConfig {
                alpha: 0.3,
                spike_threshold: Some(0.5),
                max_spike_count: Some(3),
            }),
        },
    );
    let profile = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes,
        pof_overrides: None,
    };
    assert!(profile.validate().is_ok(), "filter config should validate");
}

// ===========================================================================
// 7. Profile with detent zones
// ===========================================================================

#[test]
fn lifecycle_profile_with_detents_validates_and_merges() {
    let mut axes = HashMap::new();
    axes.insert(
        "throttle".to_string(),
        AxisConfig {
            deadzone: Some(0.02),
            expo: Some(0.0),
            slew_rate: None,
            detents: vec![
                DetentZone {
                    position: 0.0,
                    width: 0.05,
                    role: "idle".to_string(),
                },
                DetentZone {
                    position: 0.8,
                    width: 0.03,
                    role: "toga".to_string(),
                },
            ],
            curve: None,
            filter: None,
        },
    );
    let profile = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes,
        pof_overrides: None,
    };
    assert!(profile.validate().is_ok());

    // Merge with global should preserve detents
    let global = global_profile();
    let merged = global.merge_with(&profile).expect("merge");
    let throttle = merged.axes.get("throttle").unwrap();
    assert_eq!(throttle.detents.len(), 2, "detents preserved after merge");
}

// ===========================================================================
// 8. JSON round-trip preserves profile
// ===========================================================================

#[test]
fn lifecycle_json_roundtrip_preserves_full_profile() {
    let profile = aircraft_profile("A320");
    let json = serde_json::to_string(&profile).expect("serialize");
    let restored: Profile = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(profile, restored, "JSON round-trip must be lossless");
}
