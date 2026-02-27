// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Functional tests for Profile serialization, validation, and merging.
//!
//! These tests exercise the public surface of `flight_core::profile` as
//! integration tests (outside the crate boundary) and complement the
//! property-based fuzz tests in `fuzz_tests.rs`.

use flight_core::profile::{
    AircraftId, AxisConfig, CapabilityContext, CapabilityMode, CurvePoint, DetentZone,
    FilterConfig, PROFILE_SCHEMA_VERSION, PofOverrides, Profile,
};
use std::collections::HashMap;

// ── helpers ──────────────────────────────────────────────────────────────────

fn base_axis() -> AxisConfig {
    AxisConfig {
        deadzone: Some(0.05),
        expo: Some(0.3),
        slew_rate: Some(2.0),
        detents: vec![],
        curve: None,
        filter: None,
    }
}

fn minimal_profile() -> Profile {
    let mut axes = HashMap::new();
    axes.insert("pitch".to_string(), base_axis());
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

fn empty_axes_profile() -> Profile {
    Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes: HashMap::new(),
        pof_overrides: None,
    }
}

// ── serialization: JSON round-trip ───────────────────────────────────────────

#[test]
fn profile_json_roundtrip_preserves_all_fields() {
    let original = minimal_profile();

    let json = serde_json::to_string(&original).expect("serialization must not fail");
    let restored: Profile = serde_json::from_str(&json).expect("deserialization must not fail");

    assert_eq!(original, restored);
}

#[test]
fn profile_json_roundtrip_known_values() {
    let profile = minimal_profile();
    let json = serde_json::to_string(&profile).unwrap();

    // Spot-check that key field names appear in the output
    assert!(json.contains("\"schema\""));
    assert!(json.contains("flight.profile/1"));
    assert!(json.contains("\"pitch\""));
    assert!(json.contains("\"deadzone\""));

    let restored: Profile = serde_json::from_str(&json).unwrap();
    let pitch = restored
        .axes
        .get("pitch")
        .expect("pitch axis must survive round-trip");
    assert_eq!(pitch.deadzone, Some(0.05));
    assert_eq!(pitch.expo, Some(0.3));
    assert_eq!(pitch.slew_rate, Some(2.0));
}

#[test]
fn profile_json_roundtrip_with_curve_and_detents() {
    let axis = AxisConfig {
        deadzone: Some(0.02),
        expo: None,
        slew_rate: None,
        detents: vec![
            DetentZone {
                position: 0.0,
                width: 0.05,
                role: "idle".to_string(),
            },
            DetentZone {
                position: 1.0,
                width: 0.05,
                role: "max_power".to_string(),
            },
        ],
        curve: Some(vec![
            CurvePoint {
                input: 0.0,
                output: 0.0,
            },
            CurvePoint {
                input: 0.5,
                output: 0.4,
            },
            CurvePoint {
                input: 1.0,
                output: 1.0,
            },
        ]),
        filter: Some(FilterConfig {
            alpha: 0.8,
            spike_threshold: Some(0.05),
            max_spike_count: Some(3),
        }),
    };

    let mut axes = HashMap::new();
    axes.insert("throttle".to_string(), axis);
    let profile = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: Some("msfs".to_string()),
        aircraft: None,
        axes,
        pof_overrides: None,
    };

    let json = serde_json::to_string(&profile).unwrap();
    let restored: Profile = serde_json::from_str(&json).unwrap();
    assert_eq!(profile, restored);
}

// ── serialization: YAML round-trip ───────────────────────────────────────────

#[test]
fn profile_yaml_roundtrip_preserves_all_fields() {
    let original = minimal_profile();

    let yaml = serde_yaml::to_string(&original).expect("YAML serialization must not fail");
    let restored: Profile =
        serde_yaml::from_str(&yaml).expect("YAML deserialization must not fail");

    assert_eq!(original, restored);
}

#[test]
fn profile_yaml_roundtrip_empty_axes() {
    let original = empty_axes_profile();
    let yaml = serde_yaml::to_string(&original).unwrap();
    let restored: Profile = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(original, restored);
}

// ── serialization: TOML round-trip ───────────────────────────────────────────

#[test]
fn profile_toml_roundtrip_preserves_all_fields() {
    let original = minimal_profile();

    let toml_str = toml::to_string(&original).expect("TOML serialization must not fail");
    let restored: Profile = toml::from_str(&toml_str).expect("TOML deserialization must not fail");

    assert_eq!(original, restored);
}

// ── validation: valid profiles ────────────────────────────────────────────────

#[test]
fn validate_profile_with_typical_axis_is_ok() {
    assert!(minimal_profile().validate().is_ok());
}

#[test]
fn validate_profile_with_empty_axes_is_ok() {
    assert!(empty_axes_profile().validate().is_ok());
}

#[test]
fn validate_profile_with_boundary_deadzone_is_ok() {
    let mut p = minimal_profile();
    // MAX_DEADZONE = 0.5 — exactly at the boundary is valid
    p.axes.get_mut("pitch").unwrap().deadzone = Some(0.5);
    assert!(p.validate().is_ok());
}

#[test]
fn validate_profile_with_zero_deadzone_is_ok() {
    let mut p = minimal_profile();
    p.axes.get_mut("pitch").unwrap().deadzone = Some(0.0);
    assert!(p.validate().is_ok());
}

#[test]
fn validate_profile_with_boundary_expo_is_ok() {
    let mut p = minimal_profile();
    // MAX_EXPO = 1.0 — exactly at the boundary is valid
    p.axes.get_mut("pitch").unwrap().expo = Some(1.0);
    assert!(p.validate().is_ok());
}

#[test]
fn validate_profile_with_zero_expo_is_ok() {
    let mut p = minimal_profile();
    p.axes.get_mut("pitch").unwrap().expo = Some(0.0);
    assert!(p.validate().is_ok());
}

#[test]
fn validate_profile_with_all_none_axis_fields_is_ok() {
    // An AxisConfig with every optional field absent is still valid
    let mut axes = HashMap::new();
    axes.insert(
        "roll".to_string(),
        AxisConfig {
            deadzone: None,
            expo: None,
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    let p = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes,
        pof_overrides: None,
    };
    assert!(p.validate().is_ok());
}

// ── validation: invalid profiles ─────────────────────────────────────────────

#[test]
fn validate_rejects_unknown_schema_version() {
    let mut p = minimal_profile();
    p.schema = "flight.profile/999".to_string();
    let err = p.validate().unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("schema") || msg.contains("Unsupported"),
        "got: {msg}"
    );
}

#[test]
fn validate_rejects_deadzone_above_max() {
    let mut p = minimal_profile();
    // MAX_DEADZONE is 0.5; anything above is invalid
    p.axes.get_mut("pitch").unwrap().deadzone = Some(0.51);
    let err = p.validate().unwrap_err();
    assert!(
        err.to_string().contains("deadzone"),
        "error should mention deadzone, got: {err}"
    );
}

#[test]
fn validate_rejects_negative_deadzone() {
    let mut p = minimal_profile();
    p.axes.get_mut("pitch").unwrap().deadzone = Some(-0.01);
    assert!(p.validate().is_err());
}

#[test]
fn validate_rejects_expo_above_max() {
    let mut p = minimal_profile();
    // MAX_EXPO is 1.0
    p.axes.get_mut("pitch").unwrap().expo = Some(1.01);
    let err = p.validate().unwrap_err();
    assert!(
        err.to_string().contains("expo"),
        "error should mention expo, got: {err}"
    );
}

#[test]
fn validate_rejects_negative_expo() {
    let mut p = minimal_profile();
    p.axes.get_mut("pitch").unwrap().expo = Some(-0.01);
    let err = p.validate().unwrap_err();
    assert!(err.to_string().contains("expo"), "got: {err}");
}

#[test]
fn validate_rejects_negative_slew_rate() {
    let mut p = minimal_profile();
    p.axes.get_mut("pitch").unwrap().slew_rate = Some(-1.0);
    assert!(p.validate().is_err());
}

#[test]
fn validate_rejects_slew_rate_above_max() {
    let mut p = minimal_profile();
    // MAX_SLEW_RATE is 100.0
    p.axes.get_mut("pitch").unwrap().slew_rate = Some(100.01);
    assert!(p.validate().is_err());
}

#[test]
fn validate_rejects_non_monotonic_curve() {
    let mut p = minimal_profile();
    p.axes.get_mut("pitch").unwrap().curve = Some(vec![
        CurvePoint {
            input: 0.0,
            output: 0.0,
        },
        CurvePoint {
            input: 0.8,
            output: 0.7,
        },
        // Duplicate input — not strictly increasing
        CurvePoint {
            input: 0.8,
            output: 0.9,
        },
        CurvePoint {
            input: 1.0,
            output: 1.0,
        },
    ]);
    assert!(p.validate().is_err());
}

#[test]
fn validate_rejects_single_point_curve() {
    let mut p = minimal_profile();
    p.axes.get_mut("pitch").unwrap().curve = Some(vec![CurvePoint {
        input: 0.5,
        output: 0.5,
    }]);
    assert!(p.validate().is_err());
}

#[test]
fn validate_rejects_detent_position_out_of_range() {
    let mut p = minimal_profile();
    p.axes.get_mut("pitch").unwrap().detents = vec![DetentZone {
        position: 1.5, // > 1.0
        width: 0.05,
        role: "bad".to_string(),
    }];
    assert!(p.validate().is_err());
}

#[test]
fn validate_rejects_invalid_filter_alpha() {
    let mut p = minimal_profile();
    p.axes.get_mut("pitch").unwrap().filter = Some(FilterConfig {
        alpha: 1.5, // > 1.0
        spike_threshold: None,
        max_spike_count: None,
    });
    assert!(p.validate().is_err());
}

// ── validation: capability mode enforcement ──────────────────────────────────

#[test]
fn validate_kid_mode_rejects_high_expo() {
    let mut p = minimal_profile();
    // Kid mode max_expo = 0.3; value above that should be rejected
    p.axes.get_mut("pitch").unwrap().expo = Some(0.4);

    let kid_ctx = CapabilityContext::for_mode(CapabilityMode::Kid);
    assert!(p.validate_with_capabilities(&kid_ctx).is_err());

    // Same value is fine in Full mode
    let full_ctx = CapabilityContext::for_mode(CapabilityMode::Full);
    assert!(p.validate_with_capabilities(&full_ctx).is_ok());
}

#[test]
fn validate_demo_mode_rejects_high_slew_rate() {
    let mut p = minimal_profile();
    // Demo mode max_slew_rate = 50.0
    p.axes.get_mut("pitch").unwrap().slew_rate = Some(60.0);

    let demo_ctx = CapabilityContext::for_mode(CapabilityMode::Demo);
    assert!(p.validate_with_capabilities(&demo_ctx).is_err());

    let full_ctx = CapabilityContext::for_mode(CapabilityMode::Full);
    assert!(p.validate_with_capabilities(&full_ctx).is_ok());
}

// ── merge_with ────────────────────────────────────────────────────────────────

#[test]
fn merge_with_override_replaces_expo() {
    let base = minimal_profile(); // pitch.expo = 0.3

    let override_profile = {
        let mut axes = HashMap::new();
        axes.insert(
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
        Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None,
            aircraft: None,
            axes,
            pof_overrides: None,
        }
    };

    let merged = base.merge_with(&override_profile).unwrap();
    let pitch = merged.axes.get("pitch").unwrap();

    // override wins for expo
    assert_eq!(pitch.expo, Some(0.5));
    // base value preserved when override has None
    assert_eq!(pitch.deadzone, Some(0.05));
    assert_eq!(pitch.slew_rate, Some(2.0));
}

#[test]
fn merge_with_none_in_override_does_not_clobber_base() {
    let base = minimal_profile(); // pitch.deadzone = 0.05, expo = 0.3

    let override_profile = {
        let mut axes = HashMap::new();
        axes.insert(
            "pitch".to_string(),
            AxisConfig {
                deadzone: None, // None — must NOT overwrite base's 0.05
                expo: None,     // None — must NOT overwrite base's 0.3
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
    };

    let merged = base.merge_with(&override_profile).unwrap();
    let pitch = merged.axes.get("pitch").unwrap();
    assert_eq!(pitch.deadzone, Some(0.05), "base deadzone must survive");
    assert_eq!(pitch.expo, Some(0.3), "base expo must survive");
    assert_eq!(pitch.slew_rate, Some(2.0), "base slew_rate must survive");
}

#[test]
fn merge_with_adds_new_axis_from_override() {
    let base = minimal_profile(); // only has "pitch"

    let override_profile = {
        let mut axes = HashMap::new();
        axes.insert("roll".to_string(), base_axis());
        Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None,
            aircraft: None,
            axes,
            pof_overrides: None,
        }
    };

    let merged = base.merge_with(&override_profile).unwrap();
    assert!(
        merged.axes.contains_key("pitch"),
        "base axis must be present"
    );
    assert!(
        merged.axes.contains_key("roll"),
        "new axis from override must appear"
    );
}

#[test]
fn merge_with_override_sets_sim_and_aircraft() {
    // base has sim + aircraft; override provides different values
    let base = minimal_profile();

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
    assert_eq!(merged.sim.as_deref(), Some("xplane"));
    assert_eq!(
        merged.aircraft.as_ref().map(|a| a.icao.as_str()),
        Some("B738")
    );
}

#[test]
fn merge_with_none_sim_aircraft_preserves_base() {
    let base = minimal_profile(); // sim="msfs", aircraft=C172

    let override_profile = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,      // None — must NOT clobber base
        aircraft: None, // None — must NOT clobber base
        axes: HashMap::new(),
        pof_overrides: None,
    };

    let merged = base.merge_with(&override_profile).unwrap();
    assert_eq!(merged.sim.as_deref(), Some("msfs"));
    assert_eq!(
        merged.aircraft.as_ref().map(|a| a.icao.as_str()),
        Some("C172")
    );
}

#[test]
fn merge_with_self_is_idempotent() {
    let profile = minimal_profile();
    let merged = profile.merge_with(&profile).unwrap();
    assert_eq!(profile, merged);
}

#[test]
fn merge_with_empty_base_adopts_all_override_axes() {
    let base = empty_axes_profile();

    let override_profile = {
        let mut axes = HashMap::new();
        axes.insert("pitch".to_string(), base_axis());
        axes.insert("roll".to_string(), base_axis());
        Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: Some("dcs".to_string()),
            aircraft: None,
            axes,
            pof_overrides: None,
        }
    };

    let merged = base.merge_with(&override_profile).unwrap();
    assert_eq!(merged.axes.len(), 2);
    assert_eq!(merged.sim.as_deref(), Some("dcs"));
}

#[test]
fn merge_with_preserves_schema_from_base() {
    let base = minimal_profile();
    let override_profile = empty_axes_profile();
    let merged = base.merge_with(&override_profile).unwrap();
    assert_eq!(merged.schema, PROFILE_SCHEMA_VERSION);
}

// ── AxisConfig — default / all-None semantics ────────────────────────────────

#[test]
fn axis_config_all_none_serializes_and_deserializes() {
    let axis = AxisConfig {
        deadzone: None,
        expo: None,
        slew_rate: None,
        detents: vec![],
        curve: None,
        filter: None,
    };
    let json = serde_json::to_string(&axis).unwrap();
    let restored: AxisConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(axis, restored);
}

#[test]
fn axis_config_with_all_fields_set_roundtrips() {
    let axis = AxisConfig {
        deadzone: Some(0.1),
        expo: Some(0.5),
        slew_rate: Some(10.0),
        detents: vec![DetentZone {
            position: 0.0,
            width: 0.05,
            role: "center".to_string(),
        }],
        curve: Some(vec![
            CurvePoint {
                input: 0.0,
                output: 0.0,
            },
            CurvePoint {
                input: 1.0,
                output: 1.0,
            },
        ]),
        filter: Some(FilterConfig {
            alpha: 0.9,
            spike_threshold: Some(0.02),
            max_spike_count: Some(5),
        }),
    };

    let json = serde_json::to_string(&axis).unwrap();
    let restored: AxisConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(axis, restored);
}

// ── pof_overrides round-trip and validation ──────────────────────────────────

#[test]
fn profile_with_pof_overrides_roundtrips_json() {
    let mut pof_axes = HashMap::new();
    pof_axes.insert(
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
    let mut pof = HashMap::new();
    pof.insert(
        "landing".to_string(),
        PofOverrides {
            axes: Some(pof_axes),
            hysteresis: None,
        },
    );

    let profile = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: Some("msfs".to_string()),
        aircraft: None,
        axes: HashMap::new(),
        pof_overrides: Some(pof),
    };

    let json = serde_json::to_string(&profile).unwrap();
    let restored: Profile = serde_json::from_str(&json).unwrap();
    assert_eq!(profile, restored);
    assert!(profile.validate().is_ok());
}

#[test]
fn profile_pof_override_with_invalid_expo_fails_validation() {
    let mut pof_axes = HashMap::new();
    pof_axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: None,
            expo: Some(-0.1), // invalid
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    let mut pof = HashMap::new();
    pof.insert(
        "cruise".to_string(),
        PofOverrides {
            axes: Some(pof_axes),
            hysteresis: None,
        },
    );

    let profile = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes: HashMap::new(),
        pof_overrides: Some(pof),
    };

    assert!(profile.validate().is_err());
}

// ── deterministic hashing ─────────────────────────────────────────────────────

#[test]
fn effective_hash_is_stable_across_calls() {
    let profile = minimal_profile();
    assert_eq!(profile.effective_hash(), profile.effective_hash());
}

#[test]
fn effective_hash_differs_for_different_profiles() {
    let p1 = minimal_profile();
    let mut p2 = minimal_profile();
    p2.axes.get_mut("pitch").unwrap().expo = Some(0.9);
    assert_ne!(p1.effective_hash(), p2.effective_hash());
}

// ── proptest: valid AxisConfig JSON round-trip ────────────────────────────────

use proptest::prelude::*;

proptest! {
    #[test]
    fn prop_valid_axis_config_json_roundtrip(
        deadzone in 0.0f32..=0.5,
        expo     in 0.0f32..=1.0,
        slew     in 0.0f32..=100.0,
    ) {
        let axis = AxisConfig {
            deadzone: Some(deadzone),
            expo:     Some(expo),
            slew_rate: Some(slew),
            detents: vec![],
            curve:   None,
            filter:  None,
        };

        let json = serde_json::to_string(&axis).unwrap();
        let restored: AxisConfig = serde_json::from_str(&json).unwrap();

        // Compare via their JSON representation to avoid f32 NaN/denorm edge cases
        let json2 = serde_json::to_string(&restored).unwrap();
        prop_assert_eq!(json, json2);
    }

    #[test]
    fn prop_valid_profile_validates_and_roundtrips(
        deadzone in 0.0f32..=0.5,
        expo     in 0.0f32..=1.0,
        slew     in 0.0f32..=100.0,
        axis_name in "[a-z]{3,8}",
    ) {
        let mut axes = HashMap::new();
        axes.insert(
            axis_name,
            AxisConfig {
                deadzone: Some(deadzone),
                expo:     Some(expo),
                slew_rate: Some(slew),
                detents: vec![],
                curve:   None,
                filter:  None,
            },
        );
        let profile = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim:      Some("msfs".to_string()),
            aircraft: None,
            axes,
            pof_overrides: None,
        };

        // Must be valid
        prop_assert!(profile.validate().is_ok());

        // JSON round-trip must preserve effective hash
        let json = serde_json::to_string(&profile).unwrap();
        let restored: Profile = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(profile.effective_hash(), restored.effective_hash());
    }
}
