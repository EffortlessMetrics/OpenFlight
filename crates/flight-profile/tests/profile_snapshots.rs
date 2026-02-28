// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Comprehensive snapshot tests for `flight-profile` structured outputs.
//!
//! Covers default serialization, merge results, validation errors, and
//! schema migration (v1→v2→v3). Run `cargo insta review` to accept changes.

use flight_profile::{
    AircraftId, AxisConfig, CapabilityContext, CapabilityMode, PROFILE_SCHEMA_VERSION, Profile,
    profile_migration::MigrationRegistry,
};
use serde_json::json;
use std::collections::HashMap;

// ── helpers ──────────────────────────────────────────────────────────────────

fn empty_profile() -> Profile {
    Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes: HashMap::new(),
        pof_overrides: None,
    }
}

fn single_axis_profile() -> Profile {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.03),
            expo: Some(0.2),
            slew_rate: Some(1.2),
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

// ── Default profile serialization (JSON) ─────────────────────────────────────

#[test]
fn snapshot_default_empty_profile_json() {
    let profile = empty_profile();
    insta::assert_json_snapshot!("default_empty_profile_json", profile);
}

#[test]
fn snapshot_single_axis_profile_debug() {
    let profile = single_axis_profile();
    insta::with_settings!({sort_maps => true}, {
        insta::assert_json_snapshot!("single_axis_profile_json", profile);
    });
}

// ── Profile merge result structure ───────────────────────────────────────────

#[test]
fn snapshot_merge_adds_new_axis() {
    let base = single_axis_profile();

    let mut override_axes = HashMap::new();
    override_axes.insert(
        "roll".to_string(),
        AxisConfig {
            deadzone: Some(0.05),
            expo: None,
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    let override_profile = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes: override_axes,
        pof_overrides: None,
    };

    let merged = base.merge_with(&override_profile).unwrap();
    insta::with_settings!({sort_maps => true}, {
        insta::assert_json_snapshot!("merge_adds_new_axis", merged);
    });
}

#[test]
fn snapshot_merge_overrides_existing_field() {
    let base = single_axis_profile();

    let mut override_axes = HashMap::new();
    override_axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: None,
            expo: Some(0.5),
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    let override_profile = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes: override_axes,
        pof_overrides: None,
    };

    let merged = base.merge_with(&override_profile).unwrap();
    insta::with_settings!({sort_maps => true}, {
        insta::assert_json_snapshot!("merge_overrides_existing_field", merged);
    });
}

#[test]
fn snapshot_merge_overrides_sim_and_aircraft() {
    let base = single_axis_profile();

    let override_profile = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: Some("xplane".to_string()),
        aircraft: Some(AircraftId {
            icao: "B738".to_string(),
        }),
        axes: HashMap::new(),
        pof_overrides: None,
    };

    let merged = base.merge_with(&override_profile).unwrap();
    insta::with_settings!({sort_maps => true}, {
        insta::assert_json_snapshot!("merge_overrides_sim_and_aircraft", merged);
    });
}

// ── Profile validation error messages ────────────────────────────────────────

#[test]
fn snapshot_validation_error_expo_out_of_range() {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: None,
            expo: Some(1.5),
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
    insta::assert_snapshot!("validation_error_expo_out_of_range", err.to_string());
}

#[test]
fn snapshot_validation_error_slew_rate_negative() {
    let mut axes = HashMap::new();
    axes.insert(
        "rudder".to_string(),
        AxisConfig {
            deadzone: None,
            expo: None,
            slew_rate: Some(-1.0),
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
    insta::assert_snapshot!("validation_error_slew_rate_negative", err.to_string());
}

#[test]
fn snapshot_validation_error_bad_schema_version() {
    let profile = Profile {
        schema: "flight.profile/99".to_string(),
        sim: None,
        aircraft: None,
        axes: HashMap::new(),
        pof_overrides: None,
    };
    let err = profile.validate().unwrap_err();
    insta::assert_snapshot!("validation_error_bad_schema_version", err.to_string());
}

#[test]
fn snapshot_validation_error_kid_mode_expo() {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: None,
            expo: Some(0.8),
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
    let kid_ctx = CapabilityContext::for_mode(CapabilityMode::Kid);
    let err = profile.validate_with_capabilities(&kid_ctx).unwrap_err();
    insta::assert_snapshot!("validation_error_kid_mode_expo", err.to_string());
}

// ── Profile migration (v1→v2→v3) output ─────────────────────────────────────

#[test]
fn snapshot_migration_v1_to_v2() {
    let reg = MigrationRegistry::new();
    let v1 = json!({
        "schema_version": "v1",
        "axes": {
            "pitch": { "deadzone": 0.03, "expo": 0.2 },
            "roll":  { "deadzone": 0.05, "expo": 0.3 }
        }
    });
    let v2 = reg.migrate(v1, "v1", "v2").unwrap();
    insta::with_settings!({sort_maps => true}, {
        insta::assert_json_snapshot!("migration_v1_to_v2", v2);
    });
}

#[test]
fn snapshot_migration_v2_to_v3() {
    let reg = MigrationRegistry::new();
    let v2 = json!({
        "schema_version": "v2",
        "axes": {
            "pitch": { "deadzone": 0.03, "expo": 0.2, "sensitivity": 1.0 },
            "roll":  { "deadzone": 0.05, "expo": 0.3, "sensitivity": 1.0 }
        }
    });
    let v3 = reg.migrate(v2, "v2", "v3").unwrap();
    insta::with_settings!({sort_maps => true}, {
        insta::assert_json_snapshot!("migration_v2_to_v3", v3);
    });
}

#[test]
fn snapshot_migration_v1_to_v3_chained() {
    let reg = MigrationRegistry::new();
    let v1 = json!({
        "schema_version": "v1",
        "sim": "msfs",
        "aircraft": { "icao": "C172" },
        "axes": {
            "pitch": { "deadzone": 0.03, "expo": 0.2 },
            "roll":  { "deadzone": 0.05, "expo": 0.3 },
            "throttle": { "deadzone": 0.01 }
        }
    });
    let v3 = reg.migrate(v1, "v1", "v3").unwrap();
    insta::with_settings!({sort_maps => true}, {
        insta::assert_json_snapshot!("migration_v1_to_v3_chained", v3);
    });
}

#[test]
fn snapshot_migration_error_unsupported_version() {
    let reg = MigrationRegistry::new();
    let v1 = json!({ "schema_version": "v1", "axes": {} });
    let err = reg.migrate(v1, "v0", "v3").unwrap_err();
    insta::assert_snapshot!("migration_error_unsupported_version", err.to_string());
}

#[test]
fn snapshot_migration_error_invalid_schema() {
    let reg = MigrationRegistry::new();
    let bad = json!({ "schema_version": "v1" });
    let err = reg.migrate(bad, "v1", "v2").unwrap_err();
    insta::assert_snapshot!("migration_error_invalid_schema", err.to_string());
}
