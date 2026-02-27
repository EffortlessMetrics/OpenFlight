// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Snapshot tests for `flight-profile` serialization, deserialization, and validation.
//!
//! These integration-level tests exercise the public API and capture stable output
//! shapes via `insta`. Run `cargo insta review` to accept new/changed snapshots.

use flight_profile::{
    AircraftId, AxisConfig, CapabilityContext, CapabilityMode, CurvePoint, DetentZone,
    FilterConfig, PROFILE_SCHEMA_VERSION, PofOverrides, Profile,
};
use std::collections::HashMap;

// ── helpers ──────────────────────────────────────────────────────────────────

fn minimal_profile() -> Profile {
    Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes: HashMap::new(),
        pof_overrides: None,
    }
}

fn profile_with_axis() -> Profile {
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

// ── JSON serialization snapshots ─────────────────────────────────────────────

#[test]
fn snapshot_minimal_profile_json() {
    let profile = minimal_profile();
    insta::assert_json_snapshot!(profile);
}

#[test]
fn snapshot_profile_with_axis_json() {
    let profile = profile_with_axis();
    insta::assert_json_snapshot!(profile);
}

#[test]
fn snapshot_profile_with_detents_json() {
    let mut axes = HashMap::new();
    axes.insert(
        "throttle".to_string(),
        AxisConfig {
            deadzone: Some(0.02),
            expo: None,
            slew_rate: Some(50.0),
            detents: vec![
                DetentZone {
                    position: -1.0,
                    width: 0.05,
                    role: "cutoff".to_string(),
                },
                DetentZone {
                    position: 0.0,
                    width: 0.05,
                    role: "idle".to_string(),
                },
                DetentZone {
                    position: 1.0,
                    width: 0.05,
                    role: "toga".to_string(),
                },
            ],
            curve: None,
            filter: None,
        },
    );
    let profile = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: Some("msfs".to_string()),
        aircraft: Some(AircraftId {
            icao: "A320".to_string(),
        }),
        axes,
        pof_overrides: None,
    };
    insta::assert_json_snapshot!(profile);
}

#[test]
fn snapshot_profile_with_filter_json() {
    let mut axes = HashMap::new();
    axes.insert(
        "rudder".to_string(),
        AxisConfig {
            deadzone: Some(0.05),
            expo: Some(0.1),
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: Some(FilterConfig {
                alpha: 0.15,
                spike_threshold: Some(0.08),
                max_spike_count: Some(5),
            }),
        },
    );
    let profile = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: Some("xplane".to_string()),
        aircraft: None,
        axes,
        pof_overrides: None,
    };
    insta::assert_json_snapshot!(profile);
}

#[test]
fn snapshot_profile_with_pof_overrides_json() {
    let base_axis = AxisConfig {
        deadzone: Some(0.03),
        expo: Some(0.2),
        slew_rate: Some(1.0),
        detents: vec![],
        curve: None,
        filter: None,
    };
    let mut axes = HashMap::new();
    axes.insert("pitch".to_string(), base_axis.clone());

    let mut pof_axes = HashMap::new();
    pof_axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.05),
            expo: Some(0.4),
            slew_rate: Some(2.0),
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    let mut pof_overrides = HashMap::new();
    pof_overrides.insert(
        "approach".to_string(),
        PofOverrides {
            axes: Some(pof_axes),
            hysteresis: None,
        },
    );

    let profile = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: Some("msfs".to_string()),
        aircraft: Some(AircraftId {
            icao: "B738".to_string(),
        }),
        axes,
        pof_overrides: Some(pof_overrides),
    };
    insta::assert_json_snapshot!(profile);
}

// ── YAML serialization snapshots ─────────────────────────────────────────────

#[test]
fn snapshot_minimal_profile_yaml() {
    let profile = minimal_profile();
    insta::assert_yaml_snapshot!(profile);
}

#[test]
fn snapshot_profile_with_curve_yaml() {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.02),
            expo: None,
            slew_rate: None,
            detents: vec![],
            curve: Some(vec![
                CurvePoint {
                    input: 0.0,
                    output: 0.0,
                },
                CurvePoint {
                    input: 0.25,
                    output: 0.15,
                },
                CurvePoint {
                    input: 0.5,
                    output: 0.4,
                },
                CurvePoint {
                    input: 0.75,
                    output: 0.7,
                },
                CurvePoint {
                    input: 1.0,
                    output: 1.0,
                },
            ]),
            filter: None,
        },
    );
    let profile = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: Some("dcs".to_string()),
        aircraft: Some(AircraftId {
            icao: "F16C".to_string(),
        }),
        axes,
        pof_overrides: None,
    };
    insta::assert_yaml_snapshot!(profile);
}

// ── JSON deserialization snapshot ─────────────────────────────────────────────

#[test]
fn snapshot_profile_deserialized_from_json() {
    let json = r#"{
        "schema": "flight.profile/1",
        "sim": "msfs",
        "aircraft": { "icao": "C172" },
        "axes": {
            "pitch": {
                "deadzone": 0.03,
                "expo": 0.2,
                "slew_rate": 1.2,
                "detents": [],
                "curve": null,
                "filter": null
            }
        },
        "pof_overrides": null
    }"#;
    let profile: Profile = serde_json::from_str(json).expect("JSON should deserialize");
    insta::assert_yaml_snapshot!(profile);
}

// ── Merge result snapshots ────────────────────────────────────────────────────

#[test]
fn snapshot_profile_merge_result_yaml() {
    let base = profile_with_axis();
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
    insta::assert_yaml_snapshot!(merged);
}

#[test]
fn snapshot_multi_axis_merge_yaml() {
    let mut base_axes = HashMap::new();
    base_axes.insert(
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
    base_axes.insert(
        "roll".to_string(),
        AxisConfig {
            deadzone: Some(0.04),
            expo: Some(0.3),
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    let base = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: Some("msfs".to_string()),
        aircraft: Some(AircraftId {
            icao: "C172".to_string(),
        }),
        axes: base_axes,
        pof_overrides: None,
    };

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
    // Add a new axis in the override
    override_axes.insert(
        "rudder".to_string(),
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
        aircraft: Some(AircraftId {
            icao: "C172SP".to_string(),
        }),
        axes: override_axes,
        pof_overrides: None,
    };
    let merged = base.merge_with(&override_profile).unwrap();
    // sort_maps ensures stable key ordering despite HashMap non-determinism
    insta::with_settings!({sort_maps => true}, {
        insta::assert_json_snapshot!(merged);
    });
}

// ── Capability context snapshots ──────────────────────────────────────────────

#[test]
fn snapshot_capability_context_audit_enabled_yaml() {
    let mut ctx = CapabilityContext::for_mode(CapabilityMode::Full);
    ctx.audit_enabled = true;
    insta::assert_yaml_snapshot!(ctx);
}

// ── Validation error message snapshots ───────────────────────────────────────

#[test]
fn snapshot_validation_error_deadzone_out_of_range() {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.9),
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
    insta::assert_snapshot!(err.to_string());
}

#[test]
fn snapshot_validation_error_curve_non_monotonic() {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: None,
            expo: None,
            slew_rate: None,
            detents: vec![],
            curve: Some(vec![
                CurvePoint {
                    input: 0.0,
                    output: 0.0,
                },
                CurvePoint {
                    input: 0.8,
                    output: 0.5,
                },
                CurvePoint {
                    input: 0.3,
                    output: 1.0,
                }, // non-monotonic
            ]),
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
    insta::assert_snapshot!(err.to_string());
}

#[test]
fn snapshot_validation_error_kid_mode_expo() {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: None,
            expo: Some(0.9),
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
    insta::assert_snapshot!(err.to_string());
}
