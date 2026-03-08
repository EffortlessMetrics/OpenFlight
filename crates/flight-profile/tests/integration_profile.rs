// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests for the profile pipeline.
//!
//! Exercises: create → validate → merge → hash, hot-reload detection,
//! template materialization, and error reporting.

use flight_profile::{
    AircraftId, AxisConfig, CapabilityContext, CapabilityMode, PROFILE_SCHEMA_VERSION,
    PofOverrides, Profile,
    editor::ProfileEditor,
    hot_reload::{FileState, HotReloadTracker, ReloadAction},
    templates::Template,
};
use std::collections::HashMap;

// ── helpers ────────────────────────────────────────────────────────────────

fn base_profile() -> Profile {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.03),
            expo: Some(0.2),
            slew_rate: Some(1.0),
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

fn sim_profile() -> Profile {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.05),
            expo: Some(0.3),
            slew_rate: None,
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

fn aircraft_profile() -> Profile {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: None,
            expo: Some(0.4),
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    axes.insert(
        "roll".to_string(),
        AxisConfig {
            deadzone: Some(0.02),
            expo: Some(0.1),
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: Some("msfs".to_string()),
        aircraft: Some(AircraftId {
            icao: "C172".to_string(),
        }),
        axes,
        pof_overrides: None,
    }
}

// ── 1. Create profile → validate → confirm valid ─────────────────────────

#[test]
fn integration_create_and_validate_profile() {
    let profile = base_profile();
    profile.validate().expect("base profile should be valid");
}

// ── 2. Profile merge cascade: global → sim → aircraft ────────────────────

#[test]
fn integration_merge_cascade_global_sim_aircraft() {
    let global = base_profile();
    let sim = sim_profile();
    let aircraft = aircraft_profile();

    // Global → Sim
    let merged1 = global.merge_with(&sim).expect("merge global+sim");
    assert_eq!(merged1.sim.as_deref(), Some("msfs"));
    // Sim overrides deadzone
    assert_eq!(merged1.axes["pitch"].deadzone, Some(0.05));

    // (Global → Sim) → Aircraft
    let merged2 = merged1.merge_with(&aircraft).expect("merge +aircraft");
    assert_eq!(merged2.aircraft.as_ref().unwrap().icao, "C172");
    // Aircraft overrides expo
    assert_eq!(merged2.axes["pitch"].expo, Some(0.4));
    // Roll axis added by aircraft profile
    assert!(merged2.axes.contains_key("roll"));
}

// ── 3. Merged profile validates ──────────────────────────────────────────

#[test]
fn integration_merged_profile_validates() {
    let merged = base_profile()
        .merge_with(&sim_profile())
        .unwrap()
        .merge_with(&aircraft_profile())
        .unwrap();
    merged.validate().expect("merged profile should be valid");
}

// ── 4. Invalid profile rejected with clear error ─────────────────────────

#[test]
fn integration_invalid_deadzone_rejected() {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.9), // > MAX_DEADZONE (0.5)
            expo: None,
            slew_rate: None,
            detents: vec![],
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
    let err = profile.validate().unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("deadzone") || msg.contains("Deadzone"),
        "error should mention deadzone: {msg}"
    );
}

// ── 5. Invalid schema version rejected ───────────────────────────────────

#[test]
fn integration_invalid_schema_rejected() {
    let profile = Profile {
        schema: "flight.profile/999".to_string(),
        sim: None,
        aircraft: None,
        axes: HashMap::new(),
        pof_overrides: None,
    };
    assert!(profile.validate().is_err());
}

// ── 6. Hot-reload detects file change ────────────────────────────────────

#[test]
fn integration_hot_reload_detects_change() {
    let mut tracker = HotReloadTracker::new(100); // 100ms debounce
    let initial = FileState {
        path: "profile.toml".to_string(),
        hash: 12345,
        last_modified: 1000,
        size: 256,
    };
    tracker.track("profile.toml".to_string(), initial);

    // Simulate file change after debounce window.
    let updated = FileState {
        path: "profile.toml".to_string(),
        hash: 99999, // changed hash
        last_modified: 2000,
        size: 300,
    };
    let actions = tracker.check_changes(&[updated], 1200);
    let has_reload = actions.iter().any(|a| matches!(a, ReloadAction::Reload(_)));
    assert!(has_reload, "should detect change: {actions:?}");
}

// ── 7. Hot-reload detects file removal ───────────────────────────────────

#[test]
fn integration_hot_reload_detects_removal() {
    let mut tracker = HotReloadTracker::new(100);
    let initial = FileState {
        path: "old.toml".to_string(),
        hash: 111,
        last_modified: 1000,
        size: 100,
    };
    tracker.track("old.toml".to_string(), initial);

    // Check with empty current states (file removed).
    let actions = tracker.check_changes(&[], 1200);
    let has_remove = actions.iter().any(|a| matches!(a, ReloadAction::Remove(_)));
    assert!(has_remove, "should detect removal: {actions:?}");
}

// ── 8. Effective hash is deterministic ───────────────────────────────────

#[test]
fn integration_effective_hash_deterministic() {
    let profile = base_profile();
    let h1 = profile.effective_hash();
    let h2 = profile.effective_hash();
    assert_eq!(h1, h2, "hashes should be identical");
    assert!(!h1.is_empty());
}

// ── 9. Template builds valid profile ─────────────────────────────────────

#[test]
fn integration_template_builds_valid_profile() {
    let profile = Template::default_flight();
    profile
        .validate()
        .expect("template profile should be valid");
    assert!(!profile.axes.is_empty(), "template should have axes");
}

// ── 10. Profile editor round-trip ────────────────────────────────────────

#[test]
fn integration_profile_editor_round_trip() {
    let original = base_profile();
    let mut editor = ProfileEditor::new(original.clone());
    editor.set_deadzone("pitch", 0.08);
    let modified = editor.into_profile();

    assert_eq!(modified.axes["pitch"].deadzone, Some(0.08));
    modified.validate().expect("edited profile should be valid");
}

// ── 11. Capability mode restricts expo ───────────────────────────────────

#[test]
fn integration_kid_mode_rejects_high_expo() {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.03),
            expo: Some(0.5), // exceeds Kid mode limit (0.3)
            slew_rate: None,
            detents: vec![],
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
    let ctx = CapabilityContext::for_mode(CapabilityMode::Kid);
    let result = profile.validate_with_capabilities(&ctx);
    assert!(result.is_err(), "Kid mode should reject expo 0.5");
}

// ── 12. Merge preserves PoF overrides ────────────────────────────────────

#[test]
fn integration_merge_preserves_pof_overrides() {
    let mut pof = HashMap::new();
    pof.insert(
        "takeoff".to_string(),
        PofOverrides {
            axes: Some(HashMap::new()),
            hysteresis: None,
        },
    );
    let override_profile = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes: HashMap::new(),
        pof_overrides: Some(pof),
    };
    let merged = base_profile().merge_with(&override_profile).unwrap();
    assert!(merged.pof_overrides.is_some());
    assert!(merged.pof_overrides.unwrap().contains_key("takeoff"));
}
