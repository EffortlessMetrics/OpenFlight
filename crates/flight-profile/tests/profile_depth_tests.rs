// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the profile system covering schema validation, cascade,
//! merge_with, migration, serialization, and property-based verification.
//!
//! 35 tests total across 6 categories.

use flight_profile::{
    AircraftId, AxisConfig, CurvePoint, DetentZone, FilterConfig, PofOverrides, Profile,
    PROFILE_SCHEMA_VERSION, merge_axis_configs,
    profile_migration::{MigrationError, MigrationRegistry},
};
use proptest::prelude::*;
use serde_json::json;
use std::collections::HashMap;

// ── helpers ──────────────────────────────────────────────────────────────────

fn empty_axis() -> AxisConfig {
    AxisConfig {
        deadzone: None,
        expo: None,
        slew_rate: None,
        detents: vec![],
        curve: None,
        filter: None,
    }
}

fn axis(deadzone: Option<f32>, expo: Option<f32>) -> AxisConfig {
    AxisConfig {
        deadzone,
        expo,
        slew_rate: None,
        detents: vec![],
        curve: None,
        filter: None,
    }
}

fn profile_with_axes(axes: HashMap<String, AxisConfig>) -> Profile {
    Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes,
        pof_overrides: None,
    }
}

fn global_profile() -> Profile {
    let mut axes = HashMap::new();
    axes.insert("pitch".to_string(), axis(Some(0.05), Some(0.2)));
    axes.insert("roll".to_string(), axis(Some(0.05), Some(0.15)));
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
    axes.insert("pitch".to_string(), axis(Some(0.03), None));
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
    axes.insert("pitch".to_string(), axis(None, Some(0.4)));
    axes.insert(
        "rudder".to_string(),
        axis(Some(0.02), Some(0.1)),
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

fn phase_profile() -> Profile {
    let mut axes = HashMap::new();
    axes.insert("pitch".to_string(), axis(Some(0.01), None));
    let mut pof = HashMap::new();
    pof.insert(
        "landing".to_string(),
        PofOverrides {
            axes: Some({
                let mut a = HashMap::new();
                a.insert("pitch".to_string(), axis(Some(0.02), Some(0.5)));
                a
            }),
            hysteresis: None,
        },
    );
    Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: Some("msfs".to_string()),
        aircraft: Some(AircraftId {
            icao: "C172".to_string(),
        }),
        axes,
        pof_overrides: Some(pof),
    }
}

// =============================================================================
// 1. Schema Validation (8 tests)
// =============================================================================

/// Required `schema` field — missing field causes deserialization failure.
#[test]
fn schema_required_field_missing_rejected() {
    let json = r#"{"sim":"msfs","axes":{}}"#;
    assert!(
        serde_json::from_str::<Profile>(json).is_err(),
        "missing 'schema' field must fail deserialization"
    );
}

/// Optional fields can all be absent and still produce a valid profile.
#[test]
fn schema_optional_fields_absent_valid() {
    let profile = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes: HashMap::new(),
        pof_overrides: None,
    };
    assert!(profile.validate().is_ok());
}

/// Type validation: deadzone must be a number, not a string.
#[test]
fn schema_type_validation_deadzone_string_rejected() {
    let json = r#"{
        "schema": "flight.profile/1",
        "axes": { "pitch": { "deadzone": "high", "detents": [] } }
    }"#;
    assert!(
        serde_json::from_str::<Profile>(json).is_err(),
        "string deadzone must fail deserialization"
    );
}

/// Range constraint: deadzone at upper boundary (0.5) is valid.
#[test]
fn schema_range_deadzone_upper_boundary_valid() {
    let mut axes = HashMap::new();
    axes.insert("pitch".to_string(), AxisConfig {
        deadzone: Some(0.5),
        ..empty_axis()
    });
    let p = profile_with_axes(axes);
    assert!(p.validate().is_ok(), "deadzone at MAX_DEADZONE must be valid");
}

/// Range constraint: deadzone just above upper boundary is rejected.
#[test]
fn schema_range_deadzone_above_upper_boundary_rejected() {
    let mut axes = HashMap::new();
    axes.insert("pitch".to_string(), AxisConfig {
        deadzone: Some(0.501),
        ..empty_axis()
    });
    let p = profile_with_axes(axes);
    assert!(p.validate().is_err(), "deadzone above MAX_DEADZONE must be rejected");
}

/// Enum-like validation: schema version string must match exactly.
#[test]
fn schema_enum_version_string_wrong_format_rejected() {
    let p = Profile {
        schema: "flight.profile.v1".to_string(), // wrong format
        ..profile_with_axes(HashMap::new())
    };
    assert!(p.validate().is_err());
}

/// Nested object validation: PoF override with invalid axis is rejected.
#[test]
fn schema_nested_pof_invalid_axis_rejected() {
    let mut pof_axes = HashMap::new();
    pof_axes.insert("pitch".to_string(), AxisConfig {
        deadzone: Some(0.9), // exceeds MAX_DEADZONE
        ..empty_axis()
    });
    let mut pof = HashMap::new();
    pof.insert("climb".to_string(), PofOverrides {
        axes: Some(pof_axes),
        hysteresis: None,
    });
    let p = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes: HashMap::new(),
        pof_overrides: Some(pof),
    };
    assert!(p.validate().is_err(), "invalid axis in PoF must fail validation");
}

/// Array validation: detents array with valid entries passes.
#[test]
fn schema_array_valid_detents_accepted() {
    let mut axes = HashMap::new();
    axes.insert("throttle".to_string(), AxisConfig {
        detents: vec![
            DetentZone { position: 0.0, width: 0.1, role: "idle".to_string() },
            DetentZone { position: 0.5, width: 0.05, role: "half".to_string() },
        ],
        ..empty_axis()
    });
    let p = profile_with_axes(axes);
    assert!(p.validate().is_ok());
}

// =============================================================================
// 2. Profile Cascade (6 tests)
// =============================================================================

/// Global → Simulator → Aircraft → Phase merge ordering produces correct
/// override at each level.
#[test]
fn cascade_full_chain_global_sim_aircraft_phase() {
    let g = global_profile();
    let s = sim_profile();
    let a = aircraft_profile();
    let ph = phase_profile();

    let merged = g
        .merge_with(&s)
        .unwrap()
        .merge_with(&a)
        .unwrap()
        .merge_with(&ph)
        .unwrap();

    let pitch = merged.axes.get("pitch").unwrap();
    // Phase overrides aircraft (deadzone=0.01), aircraft overrides sim (expo=0.4 kept since phase has None)
    assert_eq!(pitch.deadzone, Some(0.01), "phase deadzone should win");
    // Expo: aircraft set 0.4, phase has None → aircraft's 0.4 should persist
    // BUT phase also explicitly sets expo=None, so aircraft's expo=0.4 persists via merge
    assert_eq!(pitch.expo, Some(0.4), "aircraft expo persists through phase merge");

    // Roll comes from global only
    let roll = merged.axes.get("roll").unwrap();
    assert_eq!(roll.deadzone, Some(0.05));

    // Rudder comes from aircraft only
    assert!(merged.axes.contains_key("rudder"), "aircraft-added axis survives cascade");

    // PoF overrides from phase profile must be present
    assert!(merged.pof_overrides.is_some());
}

/// More-specific overrides less-specific: sim overrides global.
#[test]
fn cascade_sim_overrides_global() {
    let g = global_profile();
    let s = sim_profile();
    let merged = g.merge_with(&s).unwrap();
    let pitch = merged.axes.get("pitch").unwrap();
    assert_eq!(pitch.deadzone, Some(0.03), "sim deadzone overrides global");
    assert_eq!(pitch.expo, Some(0.2), "global expo preserved when sim doesn't set it");
}

/// Partial profiles: override with only some fields set preserves base fields.
#[test]
fn cascade_partial_profile_preserves_base() {
    let base = global_profile();
    let partial = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: Some("xplane".to_string()),
        aircraft: None,
        axes: HashMap::new(), // no axis overrides
        pof_overrides: None,
    };
    let merged = base.merge_with(&partial).unwrap();
    assert_eq!(merged.sim, Some("xplane".to_string()), "sim updated");
    assert_eq!(merged.axes.len(), 2, "all base axes preserved");
}

/// Empty override is effectively a passthrough.
#[test]
fn cascade_empty_level_passthrough() {
    let base = global_profile();
    let empty = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes: HashMap::new(),
        pof_overrides: None,
    };
    let merged = base.merge_with(&empty).unwrap();
    assert_eq!(merged.axes, base.axes, "empty override must not change axes");
    assert_eq!(merged.sim, base.sim, "empty override must not change sim");
}

/// Cascade with missing middle level (no sim profile) still works.
#[test]
fn cascade_missing_sim_level() {
    let g = global_profile();
    let a = aircraft_profile();
    let merged = g.merge_with(&a).unwrap();
    let pitch = merged.axes.get("pitch").unwrap();
    // Aircraft sets expo=0.4, global set deadzone=0.05
    assert_eq!(pitch.expo, Some(0.4), "aircraft expo applied directly over global");
    assert_eq!(pitch.deadzone, Some(0.05), "global deadzone preserved");
    assert!(merged.axes.contains_key("rudder"), "aircraft rudder axis added");
}

/// Cascade order matters: A then B ≠ B then A when both set the same field.
#[test]
fn cascade_order_matters() {
    let mut axes_a = HashMap::new();
    axes_a.insert("pitch".to_string(), axis(Some(0.01), None));
    let a = profile_with_axes(axes_a);

    let mut axes_b = HashMap::new();
    axes_b.insert("pitch".to_string(), axis(Some(0.09), None));
    let b = profile_with_axes(axes_b);

    let ab = a.merge_with(&b).unwrap();
    let ba = b.merge_with(&a).unwrap();

    assert_eq!(ab.axes["pitch"].deadzone, Some(0.09), "B wins when applied second");
    assert_eq!(ba.axes["pitch"].deadzone, Some(0.01), "A wins when applied second");
    assert_ne!(
        ab.axes["pitch"].deadzone,
        ba.axes["pitch"].deadzone,
        "order must matter"
    );
}

// =============================================================================
// 3. merge_with (6 tests)
// =============================================================================

/// Axis merge: override's non-None fields replace base, None fields keep base.
#[test]
fn merge_with_axis_field_level_override() {
    let base_axis = AxisConfig {
        deadzone: Some(0.03),
        expo: Some(0.2),
        slew_rate: Some(1.5),
        detents: vec![DetentZone {
            position: 0.0,
            width: 0.1,
            role: "idle".to_string(),
        }],
        curve: Some(vec![
            CurvePoint { input: 0.0, output: 0.0 },
            CurvePoint { input: 1.0, output: 1.0 },
        ]),
        filter: Some(FilterConfig {
            alpha: 0.3,
            spike_threshold: None,
            max_spike_count: None,
        }),
    };
    let override_axis = AxisConfig {
        deadzone: Some(0.05), // override
        expo: None,           // keep base
        slew_rate: None,      // keep base
        detents: vec![],      // empty → keep base detents
        curve: None,          // keep base
        filter: None,         // keep base
    };

    let merged = merge_axis_configs(&base_axis, &override_axis);
    assert_eq!(merged.deadzone, Some(0.05), "overridden deadzone");
    assert_eq!(merged.expo, Some(0.2), "base expo preserved");
    assert_eq!(merged.slew_rate, Some(1.5), "base slew_rate preserved");
    assert_eq!(merged.detents.len(), 1, "base detents preserved when override is empty");
    assert!(merged.curve.is_some(), "base curve preserved");
    assert!(merged.filter.is_some(), "base filter preserved");
}

/// Button/axis addition: override adds new axis while base axes persist.
#[test]
fn merge_with_adds_new_axis() {
    let mut base_axes = HashMap::new();
    base_axes.insert("pitch".to_string(), axis(Some(0.03), Some(0.2)));
    let base = profile_with_axes(base_axes);

    let mut over_axes = HashMap::new();
    over_axes.insert("yaw".to_string(), axis(Some(0.04), Some(0.1)));
    let over = profile_with_axes(over_axes);

    let merged = base.merge_with(&over).unwrap();
    assert!(merged.axes.contains_key("pitch"), "base axis preserved");
    assert!(merged.axes.contains_key("yaw"), "new axis added");
}

/// Deadzone override replaces base deadzone.
#[test]
fn merge_with_deadzone_override() {
    let base = AxisConfig {
        deadzone: Some(0.03),
        ..empty_axis()
    };
    let over = AxisConfig {
        deadzone: Some(0.08),
        ..empty_axis()
    };
    let merged = merge_axis_configs(&base, &over);
    assert_eq!(merged.deadzone, Some(0.08));
}

/// Curve override replaces entire base curve.
#[test]
fn merge_with_curve_override() {
    let base = AxisConfig {
        curve: Some(vec![
            CurvePoint { input: 0.0, output: 0.0 },
            CurvePoint { input: 1.0, output: 1.0 },
        ]),
        ..empty_axis()
    };
    let over = AxisConfig {
        curve: Some(vec![
            CurvePoint { input: 0.0, output: 0.0 },
            CurvePoint { input: 0.5, output: 0.3 },
            CurvePoint { input: 1.0, output: 1.0 },
        ]),
        ..empty_axis()
    };
    let merged = merge_axis_configs(&base, &over);
    assert_eq!(merged.curve.as_ref().unwrap().len(), 3, "override curve replaces base");
}

/// Conflicting scalar values: override always wins.
#[test]
fn merge_with_conflicting_values_override_wins() {
    let mut base_axes = HashMap::new();
    base_axes.insert("pitch".to_string(), AxisConfig {
        deadzone: Some(0.03),
        expo: Some(0.2),
        slew_rate: Some(1.0),
        ..empty_axis()
    });
    let base = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: Some("msfs".to_string()),
        aircraft: Some(AircraftId { icao: "C172".to_string() }),
        axes: base_axes,
        pof_overrides: None,
    };

    let mut over_axes = HashMap::new();
    over_axes.insert("pitch".to_string(), AxisConfig {
        deadzone: Some(0.09),
        expo: Some(0.8),
        slew_rate: Some(50.0),
        ..empty_axis()
    });
    let over = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: Some("xplane".to_string()),
        aircraft: Some(AircraftId { icao: "A320".to_string() }),
        axes: over_axes,
        pof_overrides: None,
    };

    let merged = base.merge_with(&over).unwrap();
    assert_eq!(merged.sim, Some("xplane".to_string()), "sim overridden");
    assert_eq!(merged.aircraft.as_ref().unwrap().icao, "A320", "aircraft overridden");
    let pitch = &merged.axes["pitch"];
    assert_eq!(pitch.deadzone, Some(0.09));
    assert_eq!(pitch.expo, Some(0.8));
    assert_eq!(pitch.slew_rate, Some(50.0));
}

/// Merge with defaults: base has defaults, override has None → defaults preserved.
#[test]
fn merge_with_defaults_preserved() {
    let base = AxisConfig {
        deadzone: Some(0.05),
        expo: Some(0.0),
        slew_rate: Some(1.0),
        detents: vec![],
        curve: None,
        filter: None,
    };
    let over = empty_axis();
    let merged = merge_axis_configs(&base, &over);
    assert_eq!(merged.deadzone, Some(0.05), "default deadzone kept");
    assert_eq!(merged.expo, Some(0.0), "default expo kept");
    assert_eq!(merged.slew_rate, Some(1.0), "default slew_rate kept");
}

// =============================================================================
// 4. Migration (5 tests)
// =============================================================================

fn sample_v1() -> serde_json::Value {
    json!({
        "schema_version": "v1",
        "axes": {
            "pitch": { "deadzone": 0.03, "expo": 0.2 },
            "roll":  { "deadzone": 0.05, "expo": 0.3 }
        }
    })
}

/// v1→v2: sensitivity field added with default 1.0.
#[test]
fn migration_v1_to_v2_adds_sensitivity() {
    let reg = MigrationRegistry::new();
    let result = reg.migrate(sample_v1(), "v1", "v2").unwrap();
    assert_eq!(result["axes"]["pitch"]["sensitivity"], json!(1.0));
    assert_eq!(result["axes"]["roll"]["sensitivity"], json!(1.0));
    assert_eq!(result["schema_version"], json!("v2"));
    // Original fields preserved
    assert_eq!(result["axes"]["pitch"]["deadzone"], json!(0.03));
    assert_eq!(result["axes"]["pitch"]["expo"], json!(0.2));
}

/// v2→v3: expo renamed to exponential, response_curve_type added.
#[test]
fn migration_v2_to_v3_renames_and_adds_fields() {
    let reg = MigrationRegistry::new();
    let v2 = reg.migrate(sample_v1(), "v1", "v2").unwrap();
    let v3 = reg.migrate(v2, "v2", "v3").unwrap();

    let pitch = &v3["axes"]["pitch"];
    assert!(pitch.get("expo").is_none(), "expo must be removed");
    assert_eq!(pitch["exponential"], json!(0.2), "expo renamed to exponential");
    assert_eq!(pitch["response_curve_type"], json!("default"));
    assert_eq!(v3["schema_version"], json!("v3"));
}

/// Skip version: v1→v3 chains through v2 automatically.
#[test]
fn migration_v1_to_v3_chains_correctly() {
    let reg = MigrationRegistry::new();
    let result = reg.migrate(sample_v1(), "v1", "v3").unwrap();

    let roll = &result["axes"]["roll"];
    assert_eq!(roll["sensitivity"], json!(1.0), "v1→v2 step applied");
    assert_eq!(roll["exponential"], json!(0.3), "v2→v3 step applied");
    assert!(roll.get("expo").is_none(), "expo removed in chain");
    assert_eq!(result["schema_version"], json!("v3"));
}

/// Unknown version rejection: trying to migrate from/to unknown versions fails.
#[test]
fn migration_unknown_version_rejected() {
    let reg = MigrationRegistry::new();

    let result_from = reg.migrate(sample_v1(), "v0", "v2");
    assert!(matches!(result_from, Err(MigrationError::UnsupportedVersion(_))));

    let result_to = reg.migrate(sample_v1(), "v1", "v99");
    assert!(matches!(result_to, Err(MigrationError::UnsupportedVersion(_))));

    // Reverse direction (downgrade) also fails
    let result_reverse = reg.migrate(sample_v1(), "v3", "v1");
    assert!(matches!(result_reverse, Err(MigrationError::UnsupportedVersion(_))));
}

/// Backward compatibility: extra fields in the source are preserved through migration.
#[test]
fn migration_preserves_extra_fields() {
    let input = json!({
        "schema_version": "v1",
        "sim": "msfs",
        "aircraft": { "icao": "C172" },
        "custom_metadata": { "author": "test" },
        "axes": {
            "pitch": { "deadzone": 0.03, "expo": 0.2 }
        }
    });
    let reg = MigrationRegistry::new();
    let result = reg.migrate(input, "v1", "v3").unwrap();
    assert_eq!(result["sim"], json!("msfs"), "sim preserved");
    assert_eq!(result["aircraft"]["icao"], json!("C172"), "aircraft preserved");
    assert_eq!(result["custom_metadata"]["author"], json!("test"), "extra fields preserved");
}

// =============================================================================
// 5. Serialization (5 tests)
// =============================================================================

/// TOML roundtrip: serialize → deserialize produces equal profile.
#[test]
fn serialization_toml_roundtrip() {
    let mut axes = HashMap::new();
    axes.insert("pitch".to_string(), AxisConfig {
        deadzone: Some(0.03),
        expo: Some(0.2),
        slew_rate: Some(1.5),
        detents: vec![],
        curve: None,
        filter: None,
    });
    let profile = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: Some("msfs".to_string()),
        aircraft: Some(AircraftId { icao: "C172".to_string() }),
        axes,
        pof_overrides: None,
    };

    let toml_str = toml::to_string(&profile).expect("TOML serialize");
    let restored: Profile = toml::from_str(&toml_str).expect("TOML deserialize");
    assert_eq!(profile, restored, "TOML roundtrip must preserve equality");
}

/// JSON roundtrip: serialize → deserialize produces equal profile.
#[test]
fn serialization_json_roundtrip() {
    let profile = global_profile();
    let json = serde_json::to_string(&profile).unwrap();
    let restored: Profile = serde_json::from_str(&json).unwrap();
    assert_eq!(profile, restored, "JSON roundtrip must preserve equality");
}

/// YAML roundtrip: serialize → deserialize produces equal profile.
#[test]
fn serialization_yaml_roundtrip() {
    let profile = global_profile();
    let yaml = serde_yaml::to_string(&profile).unwrap();
    let restored: Profile = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(profile, restored, "YAML roundtrip must preserve equality");
}

/// Format detection: can deserialize from JSON, YAML, and TOML by trying each.
#[test]
fn serialization_format_detection() {
    let profile = global_profile();

    // Serialize to all three formats
    let json_str = serde_json::to_string(&profile).unwrap();
    let yaml_str = serde_yaml::to_string(&profile).unwrap();
    let toml_str = toml::to_string(&profile).unwrap();

    // Simple format detection: try JSON first, then TOML, then YAML
    fn detect_and_parse(input: &str) -> Option<Profile> {
        serde_json::from_str(input)
            .ok()
            .or_else(|| toml::from_str(input).ok())
            .or_else(|| serde_yaml::from_str(input).ok())
    }

    assert_eq!(detect_and_parse(&json_str), Some(profile.clone()), "JSON detected");
    assert_eq!(detect_and_parse(&toml_str), Some(profile.clone()), "TOML detected");
    assert_eq!(detect_and_parse(&yaml_str), Some(profile), "YAML detected");
}

/// Pretty-printed JSON is valid and roundtrips.
#[test]
fn serialization_pretty_print_roundtrip() {
    let profile = aircraft_profile();
    let pretty = profile.export_json().unwrap();

    // Must contain newlines (pretty-printed)
    assert!(pretty.contains('\n'), "export_json must be pretty-printed");

    // Roundtrip
    let restored: Profile = serde_json::from_str(&pretty).unwrap();
    assert_eq!(profile, restored);
}

// =============================================================================
// 6. Property Tests (5 tests)
// =============================================================================

prop_compose! {
    fn arb_axis()(
        deadzone in prop::option::of(0.0f32..0.5f32),
        expo in prop::option::of(0.0f32..1.0f32),
        slew_rate in prop::option::of(0.0f32..100.0f32),
    ) -> AxisConfig {
        AxisConfig { deadzone, expo, slew_rate, detents: vec![], curve: None, filter: None }
    }
}

prop_compose! {
    fn arb_profile()(
        sim in prop::option::of("[a-z]+"),
        axes in prop::collection::hash_map("[a-z]{1,5}", arb_axis(), 1..4),
    ) -> Profile {
        Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim,
            aircraft: None,
            axes,
            pof_overrides: None,
        }
    }
}

proptest! {
    /// merge_with is associative: merge(merge(a,b), c) == merge(a, merge(b,c))
    /// when all profiles use the same axis name.
    #[test]
    fn prop_merge_associative(
        dz_a in 0.0f32..0.5,
        dz_b in 0.0f32..0.5,
        dz_c in 0.0f32..0.5,
        expo_a in 0.0f32..1.0,
        expo_b in 0.0f32..1.0,
        expo_c in 0.0f32..1.0,
    ) {
        let mk = |dz: f32, expo: f32| -> Profile {
            let mut axes = HashMap::new();
            axes.insert("pitch".to_string(), AxisConfig {
                deadzone: Some(dz), expo: Some(expo), slew_rate: None,
                detents: vec![], curve: None, filter: None,
            });
            Profile {
                schema: PROFILE_SCHEMA_VERSION.to_string(),
                sim: Some("msfs".to_string()),
                aircraft: None,
                axes,
                pof_overrides: None,
            }
        };
        let a = mk(dz_a, expo_a);
        let b = mk(dz_b, expo_b);
        let c = mk(dz_c, expo_c);

        let ab_c = a.merge_with(&b).unwrap().merge_with(&c).unwrap();
        let a_bc = a.merge_with(&b.merge_with(&c).unwrap()).unwrap();
        prop_assert_eq!(ab_c, a_bc);
    }

    /// Merging with an empty profile is the identity operation.
    #[test]
    fn prop_merge_empty_identity(profile in arb_profile()) {
        let empty = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None,
            aircraft: None,
            axes: HashMap::new(),
            pof_overrides: None,
        };
        let merged = profile.merge_with(&empty).unwrap();
        prop_assert_eq!(&profile, &merged, "merge with empty must be identity");
    }

    /// Cascade order matters: a.merge(b) ≠ b.merge(a) when both set the same field
    /// to different values.
    #[test]
    fn prop_cascade_order_matters(
        dz_a in 0.0f32..0.25f32,
        dz_b in 0.25f32..0.5f32,
    ) {
        let mk = |dz: f32| -> Profile {
            let mut axes = HashMap::new();
            axes.insert("pitch".to_string(), axis(Some(dz), None));
            profile_with_axes(axes)
        };
        let a = mk(dz_a);
        let b = mk(dz_b);

        let ab = a.merge_with(&b).unwrap();
        let ba = b.merge_with(&a).unwrap();

        prop_assert_eq!(ab.axes["pitch"].deadzone, Some(dz_b));
        prop_assert_eq!(ba.axes["pitch"].deadzone, Some(dz_a));
        prop_assert_ne!(
            ab.axes["pitch"].deadzone,
            ba.axes["pitch"].deadzone,
            "order must matter for different values"
        );
    }

    /// JSON roundtrip is identity: serialize → deserialize → serialize produces
    /// structurally identical JSON.
    #[test]
    fn prop_roundtrip_identity(profile in arb_profile()) {
        let json1 = serde_json::to_string(&profile).unwrap();
        let restored: Profile = serde_json::from_str(&json1).unwrap();
        let json2 = serde_json::to_string(&restored).unwrap();

        let val1: serde_json::Value = serde_json::from_str(&json1).unwrap();
        let val2: serde_json::Value = serde_json::from_str(&json2).unwrap();
        prop_assert_eq!(val1, val2, "double roundtrip must be stable");
    }

    /// Effective hash is stable across JSON roundtrip.
    #[test]
    fn prop_hash_stable_across_roundtrip(profile in arb_profile()) {
        let json = serde_json::to_string(&profile).unwrap();
        let restored: Profile = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(
            profile.effective_hash(),
            restored.effective_hash(),
            "hash must survive roundtrip"
        );
    }
}
