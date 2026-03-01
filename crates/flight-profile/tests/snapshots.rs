// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Snapshot tests for built-in profile templates and profile diff output.
//!
//! Run `cargo insta review` to accept new or changed snapshots.

use flight_profile::profile_compare::{compare_profiles, flatten_profile};
use flight_profile::templates::Template;

// ── Built-in template snapshots (JSON) ───────────────────────────────────────

#[test]
fn snapshot_template_default_flight_json() {
    let profile = Template::default_flight();
    insta::with_settings!({sort_maps => true}, {
        insta::assert_json_snapshot!("template_default_flight", profile);
    });
}

#[test]
fn snapshot_template_helicopter_json() {
    let profile = Template::helicopter();
    insta::with_settings!({sort_maps => true}, {
        insta::assert_json_snapshot!("template_helicopter", profile);
    });
}

#[test]
fn snapshot_template_space_sim_json() {
    let profile = Template::space_sim();
    insta::with_settings!({sort_maps => true}, {
        insta::assert_json_snapshot!("template_space_sim", profile);
    });
}

#[test]
fn snapshot_template_airliner_json() {
    let profile = Template::airliner();
    insta::with_settings!({sort_maps => true}, {
        insta::assert_json_snapshot!("template_airliner", profile);
    });
}

#[test]
fn snapshot_template_warbird_json() {
    let profile = Template::warbird();
    insta::with_settings!({sort_maps => true}, {
        insta::assert_json_snapshot!("template_warbird", profile);
    });
}

// ── Profile diff output snapshots ────────────────────────────────────────────

#[test]
fn snapshot_profile_diff_text_output() {
    let base = Template::default_flight();
    let modified = Template::warbird();

    let base_json = serde_json::to_value(&base).unwrap();
    let mod_json = serde_json::to_value(&modified).unwrap();

    let left = flatten_profile(&base_json, "");
    let right = flatten_profile(&mod_json, "");

    let diff = compare_profiles(&left, &right, "default_flight", "warbird");
    insta::assert_snapshot!("profile_diff_default_vs_warbird", diff.to_text());
}

#[test]
fn snapshot_profile_diff_axes_filtered() {
    let base = Template::default_flight();
    let modified = Template::airliner();

    let base_json = serde_json::to_value(&base).unwrap();
    let mod_json = serde_json::to_value(&modified).unwrap();

    let left = flatten_profile(&base_json, "");
    let right = flatten_profile(&mod_json, "");

    let diff = compare_profiles(&left, &right, "default_flight", "airliner");
    let axes_only = diff.filter_by_prefix("axes");
    insta::assert_snapshot!(
        "profile_diff_axes_only_default_vs_airliner",
        axes_only.to_text()
    );
}

// ── Default profile schema snapshot ──────────────────────────────────────────

use flight_profile::{
    AircraftId, AxisConfig, CapabilityContext, CapabilityMode, CurvePoint, DetentZone,
    FilterConfig, PofOverrides, Profile, PROFILE_SCHEMA_VERSION,
};

#[test]
fn snapshot_default_profile_schema() {
    let profile = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes: std::collections::HashMap::new(),
        pof_overrides: None,
    };
    insta::assert_yaml_snapshot!("default_profile_schema", profile);
}

// ── Fully-populated profile snapshot ─────────────────────────────────────────

#[test]
fn snapshot_fully_populated_profile() {
    let mut axes = std::collections::HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.03),
            expo: Some(0.2),
            slew_rate: Some(1.2),
            detents: vec![DetentZone {
                position: 0.0,
                width: 0.05,
                role: "center".to_string(),
            }],
            curve: Some(vec![
                CurvePoint { input: 0.0, output: 0.0 },
                CurvePoint { input: 0.5, output: 0.4 },
                CurvePoint { input: 1.0, output: 1.0 },
            ]),
            filter: Some(FilterConfig {
                alpha: 0.3,
                spike_threshold: Some(0.1),
                max_spike_count: Some(3),
            }),
        },
    );
    axes.insert(
        "roll".to_string(),
        AxisConfig {
            deadzone: Some(0.02),
            expo: Some(0.15),
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        },
    );

    let mut pof_axes = std::collections::HashMap::new();
    pof_axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.05),
            expo: Some(0.1),
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    let mut pof_overrides = std::collections::HashMap::new();
    pof_overrides.insert(
        "landing".to_string(),
        PofOverrides {
            axes: Some(pof_axes),
            hysteresis: None,
        },
    );

    let profile = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: Some("msfs".to_string()),
        aircraft: Some(AircraftId { icao: "C172".to_string() }),
        axes,
        pof_overrides: Some(pof_overrides),
    };

    profile.validate().expect("profile should be valid");
    insta::with_settings!({sort_maps => true}, {
        insta::assert_yaml_snapshot!("fully_populated_profile", profile);
    });
}

// ── Validation error message snapshots ───────────────────────────────────────

#[test]
fn snapshot_validation_error_bad_schema_version() {
    let profile = Profile {
        schema: "unknown/99".to_string(),
        sim: None,
        aircraft: None,
        axes: std::collections::HashMap::new(),
        pof_overrides: None,
    };
    let err = profile.validate().unwrap_err();
    insta::assert_snapshot!("validation_error_bad_schema_version", err.to_string());
}

#[test]
fn snapshot_validation_error_deadzone_out_of_range() {
    let mut axes = std::collections::HashMap::new();
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
    insta::assert_snapshot!("validation_error_deadzone_out_of_range", err.to_string());
}

#[test]
fn snapshot_validation_error_kid_mode_expo() {
    let mut axes = std::collections::HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.03),
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
    let ctx = CapabilityContext::for_mode(CapabilityMode::Kid);
    let err = profile.validate_with_capabilities(&ctx).unwrap_err();
    insta::assert_snapshot!("validation_error_kid_mode_expo", err.to_string());
}

#[test]
fn snapshot_validation_error_non_monotonic_curve() {
    let mut axes = std::collections::HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: None,
            expo: None,
            slew_rate: None,
            detents: vec![],
            curve: Some(vec![
                CurvePoint { input: 0.0, output: 0.0 },
                CurvePoint { input: 0.5, output: 0.6 },
                CurvePoint { input: 0.3, output: 0.8 },
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
    insta::assert_snapshot!("validation_error_non_monotonic_curve", err.to_string());
}

#[test]
fn snapshot_capability_limits_all_modes() {
    let full = CapabilityContext::for_mode(CapabilityMode::Full);
    let demo = CapabilityContext::for_mode(CapabilityMode::Demo);
    let kid = CapabilityContext::for_mode(CapabilityMode::Kid);
    insta::assert_yaml_snapshot!("capability_limits_full", full);
    insta::assert_yaml_snapshot!("capability_limits_demo", demo);
    insta::assert_yaml_snapshot!("capability_limits_kid", kid);
}
