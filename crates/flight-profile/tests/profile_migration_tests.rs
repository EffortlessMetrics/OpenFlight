// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Comprehensive tests for the profile migration system.
//!
//! Covers:
//! - Schema version detection
//! - Migration chains (v1→v2, v2→v3, v1→v3)
//! - Backward compatibility
//! - Data preservation
//! - Idempotency
//! - Invalid / corrupt input handling
//! - Property-based (proptest) arbitrary-profile migration
//! - Golden-file regression tests

use flight_profile::profile_migration::{MigrationError, MigrationRegistry, ProfileMigration};
use serde_json::{Value, json};

// ── Helpers ──────────────────────────────────────────────────────────────────

fn sample_v1_minimal() -> Value {
    json!({
        "schema_version": "v1",
        "axes": {
            "pitch": { "deadzone": 0.03, "expo": 0.2 }
        }
    })
}

fn sample_v1_rich() -> Value {
    json!({
        "schema_version": "v1",
        "sim": "msfs",
        "aircraft": { "icao": "C172" },
        "description": "Default C172 profile",
        "author": "test",
        "axes": {
            "pitch":    { "deadzone": 0.03, "expo": 0.2, "slew_rate": 1.5 },
            "roll":     { "deadzone": 0.05, "expo": 0.3, "slew_rate": 2.0 },
            "yaw":      { "deadzone": 0.08, "expo": 0.1 },
            "throttle": { "deadzone": 0.01 }
        }
    })
}

fn sample_v2() -> Value {
    json!({
        "schema_version": "v2",
        "axes": {
            "pitch": { "deadzone": 0.03, "expo": 0.2, "sensitivity": 1.0 },
            "roll":  { "deadzone": 0.05, "expo": 0.3, "sensitivity": 0.8 }
        }
    })
}

fn sample_v3() -> Value {
    json!({
        "schema_version": "v3",
        "axes": {
            "pitch": {
                "deadzone": 0.03,
                "exponential": 0.2,
                "sensitivity": 1.0,
                "response_curve_type": "default"
            }
        }
    })
}

// ── 1. Schema version detection ─────────────────────────────────────────────

#[test]
fn sample_v1_contains_version() {
    let v1 = sample_v1_minimal();
    assert_eq!(v1["schema_version"], "v1");
}

#[test]
fn sample_v2_contains_version() {
    let v2 = sample_v2();
    assert_eq!(v2["schema_version"], "v2");
}

#[test]
fn sample_v3_contains_version() {
    let v3 = sample_v3();
    assert_eq!(v3["schema_version"], "v3");
}

#[test]
fn registry_recognises_all_builtin_versions() {
    let reg = MigrationRegistry::new();
    let versions = reg.available_versions();
    assert!(versions.contains(&"v1"));
    assert!(versions.contains(&"v2"));
    assert!(versions.contains(&"v3"));
    assert_eq!(versions.len(), 3);
}

// ── 2. Migration chain tests ────────────────────────────────────────────────

#[test]
fn migrate_v1_to_v2_adds_sensitivity() {
    let reg = MigrationRegistry::new();
    let result = reg.migrate(sample_v1_minimal(), "v1", "v2").unwrap();

    assert_eq!(result["schema_version"], "v2");
    assert_eq!(result["axes"]["pitch"]["sensitivity"], json!(1.0));
    // Original fields preserved
    assert_eq!(result["axes"]["pitch"]["deadzone"], json!(0.03));
    assert_eq!(result["axes"]["pitch"]["expo"], json!(0.2));
}

#[test]
fn migrate_v2_to_v3_renames_expo_and_adds_curve_type() {
    let reg = MigrationRegistry::new();
    let result = reg.migrate(sample_v2(), "v2", "v3").unwrap();

    assert_eq!(result["schema_version"], "v3");
    let pitch = &result["axes"]["pitch"];
    // expo → exponential
    assert!(pitch.get("expo").is_none());
    assert_eq!(pitch["exponential"], json!(0.2));
    assert_eq!(pitch["response_curve_type"], json!("default"));
    // sensitivity preserved
    assert_eq!(pitch["sensitivity"], json!(1.0));
}

#[test]
fn migrate_v1_to_v3_full_chain() {
    let reg = MigrationRegistry::new();
    let result = reg.migrate(sample_v1_rich(), "v1", "v3").unwrap();

    assert_eq!(result["schema_version"], "v3");

    // All axes should have sensitivity (from v1→v2) and exponential (from v2→v3)
    for axis_name in &["pitch", "roll", "yaw", "throttle"] {
        let axis = &result["axes"][axis_name];
        assert_eq!(
            axis["sensitivity"],
            json!(1.0),
            "{axis_name} missing sensitivity"
        );
        assert_eq!(
            axis["response_curve_type"],
            json!("default"),
            "{axis_name} missing response_curve_type"
        );
        assert!(
            axis.get("expo").is_none(),
            "{axis_name} still has 'expo' field"
        );
    }

    // expo values moved to exponential
    assert_eq!(result["axes"]["pitch"]["exponential"], json!(0.2));
    assert_eq!(result["axes"]["roll"]["exponential"], json!(0.3));
    assert_eq!(result["axes"]["yaw"]["exponential"], json!(0.1));
    // throttle had no expo, so exponential should be absent
    assert!(result["axes"]["throttle"].get("exponential").is_none());
}

#[test]
fn chain_path_exists_for_all_forward_pairs() {
    let reg = MigrationRegistry::new();
    assert!(reg.can_migrate("v1", "v2"));
    assert!(reg.can_migrate("v2", "v3"));
    assert!(reg.can_migrate("v1", "v3"));
}

// ── 3. Backward compatibility tests ────────────────────────────────────────

#[test]
fn no_downgrade_path_v3_to_v1() {
    let reg = MigrationRegistry::new();
    assert!(!reg.can_migrate("v3", "v1"));
}

#[test]
fn no_downgrade_path_v3_to_v2() {
    let reg = MigrationRegistry::new();
    assert!(!reg.can_migrate("v3", "v2"));
}

#[test]
fn no_downgrade_path_v2_to_v1() {
    let reg = MigrationRegistry::new();
    assert!(!reg.can_migrate("v2", "v1"));
}

#[test]
fn downgrade_attempt_returns_unsupported_error() {
    let reg = MigrationRegistry::new();
    let result = reg.migrate(sample_v3(), "v3", "v1");
    assert!(matches!(result, Err(MigrationError::UnsupportedVersion(_))));
}

// ── 4. Data preservation tests ──────────────────────────────────────────────

#[test]
fn v1_to_v2_preserves_all_original_fields() {
    let reg = MigrationRegistry::new();
    let input = sample_v1_rich();
    let result = reg.migrate(input, "v1", "v2").unwrap();

    // Top-level metadata preserved
    assert_eq!(result["sim"], json!("msfs"));
    assert_eq!(result["aircraft"]["icao"], json!("C172"));
    assert_eq!(result["description"], json!("Default C172 profile"));
    assert_eq!(result["author"], json!("test"));

    // Axis fields preserved
    assert_eq!(result["axes"]["pitch"]["deadzone"], json!(0.03));
    assert_eq!(result["axes"]["pitch"]["expo"], json!(0.2));
    assert_eq!(result["axes"]["pitch"]["slew_rate"], json!(1.5));
    assert_eq!(result["axes"]["roll"]["deadzone"], json!(0.05));
    assert_eq!(result["axes"]["throttle"]["deadzone"], json!(0.01));
}

#[test]
fn v1_to_v3_preserves_non_axis_metadata() {
    let reg = MigrationRegistry::new();
    let input = json!({
        "schema_version": "v1",
        "sim": "xplane",
        "aircraft": { "icao": "B738" },
        "user_notes": "My custom profile",
        "tags": ["airliner", "twin-engine"],
        "axes": {
            "pitch": { "deadzone": 0.02, "expo": 0.15 }
        }
    });
    let result = reg.migrate(input, "v1", "v3").unwrap();

    assert_eq!(result["sim"], json!("xplane"));
    assert_eq!(result["aircraft"]["icao"], json!("B738"));
    assert_eq!(result["user_notes"], json!("My custom profile"));
    assert_eq!(result["tags"], json!(["airliner", "twin-engine"]));
}

#[test]
fn v2_to_v3_preserves_custom_sensitivity_values() {
    let reg = MigrationRegistry::new();
    let input = json!({
        "schema_version": "v2",
        "axes": {
            "pitch": { "deadzone": 0.03, "expo": 0.2, "sensitivity": 0.75 },
            "roll":  { "deadzone": 0.05, "expo": 0.3, "sensitivity": 1.25 }
        }
    });
    let result = reg.migrate(input, "v2", "v3").unwrap();

    assert_eq!(result["axes"]["pitch"]["sensitivity"], json!(0.75));
    assert_eq!(result["axes"]["roll"]["sensitivity"], json!(1.25));
}

#[test]
fn migration_preserves_axis_count() {
    let reg = MigrationRegistry::new();
    let input = sample_v1_rich();
    let axis_count = input["axes"].as_object().unwrap().len();

    let result = reg.migrate(input, "v1", "v3").unwrap();
    let result_count = result["axes"].as_object().unwrap().len();

    assert_eq!(
        axis_count, result_count,
        "axis count changed during migration"
    );
}

#[test]
fn v1_to_v3_preserves_extra_unknown_axis_fields() {
    let reg = MigrationRegistry::new();
    let input = json!({
        "schema_version": "v1",
        "axes": {
            "pitch": {
                "deadzone": 0.03,
                "expo": 0.2,
                "custom_field": "preserved",
                "nested": { "a": 1, "b": 2 }
            }
        }
    });
    let result = reg.migrate(input, "v1", "v3").unwrap();

    assert_eq!(result["axes"]["pitch"]["custom_field"], json!("preserved"));
    assert_eq!(result["axes"]["pitch"]["nested"]["a"], json!(1));
    assert_eq!(result["axes"]["pitch"]["nested"]["b"], json!(2));
}

// ── 5. Idempotency tests ────────────────────────────────────────────────────

#[test]
fn same_version_migration_is_noop() {
    let reg = MigrationRegistry::new();
    let input = sample_v1_minimal();
    let result = reg.migrate(input.clone(), "v1", "v1").unwrap();
    assert_eq!(input, result);
}

#[test]
fn same_version_v2_is_noop() {
    let reg = MigrationRegistry::new();
    let input = sample_v2();
    let result = reg.migrate(input.clone(), "v2", "v2").unwrap();
    assert_eq!(input, result);
}

#[test]
fn same_version_v3_is_noop() {
    let reg = MigrationRegistry::new();
    let input = sample_v3();
    let result = reg.migrate(input.clone(), "v3", "v3").unwrap();
    assert_eq!(input, result);
}

#[test]
fn double_migrate_v1_to_v2_is_stable() {
    let reg = MigrationRegistry::new();
    let first = reg.migrate(sample_v1_minimal(), "v1", "v2").unwrap();
    let second = reg.migrate(first.clone(), "v2", "v2").unwrap();
    assert_eq!(first, second, "second migration should be a no-op");
}

#[test]
fn full_chain_then_noop_is_stable() {
    let reg = MigrationRegistry::new();
    let migrated = reg.migrate(sample_v1_rich(), "v1", "v3").unwrap();
    let again = reg.migrate(migrated.clone(), "v3", "v3").unwrap();
    assert_eq!(migrated, again);
}

// ── 6. Invalid / corrupt input handling ─────────────────────────────────────

#[test]
fn missing_axes_object_returns_invalid_schema() {
    let reg = MigrationRegistry::new();
    let bad = json!({ "schema_version": "v1" });
    let result = reg.migrate(bad, "v1", "v2");
    assert!(matches!(result, Err(MigrationError::InvalidSchema(_))));
}

#[test]
fn axes_is_array_not_object_returns_invalid_schema() {
    let reg = MigrationRegistry::new();
    let bad = json!({
        "schema_version": "v1",
        "axes": [1, 2, 3]
    });
    let result = reg.migrate(bad, "v1", "v2");
    assert!(matches!(result, Err(MigrationError::InvalidSchema(_))));
}

#[test]
fn axis_entry_is_string_returns_invalid_schema() {
    let reg = MigrationRegistry::new();
    let bad = json!({
        "schema_version": "v1",
        "axes": { "pitch": "not_an_object" }
    });
    let result = reg.migrate(bad, "v1", "v2");
    assert!(matches!(result, Err(MigrationError::InvalidSchema(_))));
}

#[test]
fn axis_entry_is_null_returns_invalid_schema() {
    let reg = MigrationRegistry::new();
    let bad = json!({
        "schema_version": "v1",
        "axes": { "pitch": null }
    });
    let result = reg.migrate(bad, "v1", "v2");
    assert!(matches!(result, Err(MigrationError::InvalidSchema(_))));
}

#[test]
fn unknown_source_version_returns_error() {
    let reg = MigrationRegistry::new();
    let result = reg.migrate(sample_v1_minimal(), "v0", "v2");
    assert!(matches!(result, Err(MigrationError::UnsupportedVersion(_))));
}

#[test]
fn unknown_target_version_returns_error() {
    let reg = MigrationRegistry::new();
    let result = reg.migrate(sample_v1_minimal(), "v1", "v99");
    assert!(matches!(result, Err(MigrationError::UnsupportedVersion(_))));
}

#[test]
fn axes_is_null_returns_invalid_schema() {
    let reg = MigrationRegistry::new();
    let bad = json!({
        "schema_version": "v2",
        "axes": null
    });
    let result = reg.migrate(bad, "v2", "v3");
    assert!(matches!(result, Err(MigrationError::InvalidSchema(_))));
}

#[test]
fn completely_empty_object_returns_invalid_schema() {
    let reg = MigrationRegistry::new();
    let bad = json!({});
    let result = reg.migrate(bad, "v1", "v2");
    assert!(matches!(result, Err(MigrationError::InvalidSchema(_))));
}

#[test]
fn error_display_contains_useful_info() {
    let err = MigrationError::UnsupportedVersion("no path from v0 to v5".into());
    let msg = err.to_string();
    assert!(msg.contains("v0"), "error should mention version");
    assert!(msg.contains("v5"), "error should mention version");

    let err2 = MigrationError::InvalidSchema("missing 'axes' object".into());
    assert!(err2.to_string().contains("axes"));

    let err3 = MigrationError::DataLoss("field removed".into());
    assert!(err3.to_string().contains("field removed"));
}

// ── 7. Custom migration registration ───────────────────────────────────────

#[test]
fn custom_v3_to_v4_migration() {
    let mut reg = MigrationRegistry::new();
    reg.register(ProfileMigration {
        from_version: "v3",
        to_version: "v4",
        description: "Add force_feedback_enabled flag",
        migrate_fn: |mut v| {
            if let Some(axes) = v.get_mut("axes").and_then(|a| a.as_object_mut()) {
                for (_name, axis) in axes.iter_mut() {
                    if let Some(obj) = axis.as_object_mut() {
                        obj.entry("force_feedback_enabled")
                            .or_insert(Value::from(false));
                    }
                }
            }
            if let Some(obj) = v.as_object_mut() {
                obj.insert("schema_version".to_string(), Value::from("v4"));
            }
            Ok(v)
        },
    });

    assert!(reg.can_migrate("v1", "v4"));
    let result = reg.migrate(sample_v1_minimal(), "v1", "v4").unwrap();
    assert_eq!(result["schema_version"], "v4");
    assert_eq!(
        result["axes"]["pitch"]["force_feedback_enabled"],
        json!(false)
    );
}

#[test]
fn custom_migration_extends_available_versions() {
    let mut reg = MigrationRegistry::new();
    reg.register(ProfileMigration {
        from_version: "v3",
        to_version: "v4",
        description: "stub",
        migrate_fn: |v| Ok(v),
    });
    let versions = reg.available_versions();
    assert!(versions.contains(&"v4"));
}

// ── 8. Empty axes edge cases ────────────────────────────────────────────────

#[test]
fn v1_to_v2_with_empty_axes_succeeds() {
    let reg = MigrationRegistry::new();
    let input = json!({
        "schema_version": "v1",
        "axes": {}
    });
    let result = reg.migrate(input, "v1", "v2").unwrap();
    assert_eq!(result["schema_version"], "v2");
    assert!(result["axes"].as_object().unwrap().is_empty());
}

#[test]
fn v2_to_v3_with_empty_axes_succeeds() {
    let reg = MigrationRegistry::new();
    let input = json!({
        "schema_version": "v2",
        "axes": {}
    });
    let result = reg.migrate(input, "v2", "v3").unwrap();
    assert_eq!(result["schema_version"], "v3");
    assert!(result["axes"].as_object().unwrap().is_empty());
}

#[test]
fn v1_to_v3_with_empty_axes_succeeds() {
    let reg = MigrationRegistry::new();
    let input = json!({
        "schema_version": "v1",
        "axes": {}
    });
    let result = reg.migrate(input, "v1", "v3").unwrap();
    assert_eq!(result["schema_version"], "v3");
}

// ── 9. Many-axis stress test ────────────────────────────────────────────────

#[test]
fn migrate_profile_with_many_axes() {
    let mut axes = serde_json::Map::new();
    for i in 0..50 {
        let mut axis = serde_json::Map::new();
        axis.insert("deadzone".to_string(), json!(0.01 + (i as f64) * 0.001));
        axis.insert("expo".to_string(), json!(0.1 + (i as f64) * 0.01));
        axes.insert(format!("axis_{i}"), Value::Object(axis));
    }
    let input = json!({
        "schema_version": "v1",
        "axes": Value::Object(axes)
    });

    let reg = MigrationRegistry::new();
    let result = reg.migrate(input, "v1", "v3").unwrap();

    assert_eq!(result["schema_version"], "v3");
    let result_axes = result["axes"].as_object().unwrap();
    assert_eq!(result_axes.len(), 50);

    // Spot-check a few axes
    for i in [0, 25, 49] {
        let axis = &result["axes"][&format!("axis_{i}")];
        assert_eq!(axis["sensitivity"], json!(1.0));
        assert_eq!(axis["response_curve_type"], json!("default"));
        assert!(axis.get("expo").is_none());
        assert!(axis.get("exponential").is_some());
    }
}

// ── 10. v2→v3 without expo field (axis that never had expo) ─────────────────

#[test]
fn v2_to_v3_axis_without_expo_gets_no_exponential() {
    let reg = MigrationRegistry::new();
    let input = json!({
        "schema_version": "v2",
        "axes": {
            "throttle": { "deadzone": 0.01, "sensitivity": 1.0 }
        }
    });
    let result = reg.migrate(input, "v2", "v3").unwrap();
    let throttle = &result["axes"]["throttle"];
    assert!(throttle.get("expo").is_none());
    assert!(throttle.get("exponential").is_none());
    assert_eq!(throttle["response_curve_type"], json!("default"));
}

// ── 11. v1→v2 preserves pre-existing sensitivity ────────────────────────────

#[test]
fn v1_to_v2_preserves_existing_sensitivity() {
    let reg = MigrationRegistry::new();
    let input = json!({
        "schema_version": "v1",
        "axes": {
            "pitch": { "deadzone": 0.03, "expo": 0.2, "sensitivity": 0.5 }
        }
    });
    let result = reg.migrate(input, "v1", "v2").unwrap();
    // or_insert should keep existing value
    assert_eq!(result["axes"]["pitch"]["sensitivity"], json!(0.5));
}

// ── 12. Golden file tests ───────────────────────────────────────────────────

/// Known v1 profile produces exact expected v2 output.
#[test]
fn golden_v1_to_v2() {
    let input = json!({
        "schema_version": "v1",
        "sim": "msfs",
        "aircraft": { "icao": "C172" },
        "axes": {
            "pitch": { "deadzone": 0.03, "expo": 0.2 },
            "roll":  { "deadzone": 0.05, "expo": 0.3 }
        }
    });

    let reg = MigrationRegistry::new();
    let result = reg.migrate(input, "v1", "v2").unwrap();

    // Verify exact structure
    let pitch = result["axes"]["pitch"].as_object().unwrap();
    assert_eq!(pitch.get("deadzone").unwrap(), &json!(0.03));
    assert_eq!(pitch.get("expo").unwrap(), &json!(0.2));
    assert_eq!(pitch.get("sensitivity").unwrap(), &json!(1.0));

    let roll = result["axes"]["roll"].as_object().unwrap();
    assert_eq!(roll.get("deadzone").unwrap(), &json!(0.05));
    assert_eq!(roll.get("expo").unwrap(), &json!(0.3));
    assert_eq!(roll.get("sensitivity").unwrap(), &json!(1.0));

    assert_eq!(result["schema_version"], "v2");
    assert_eq!(result["sim"], "msfs");
    assert_eq!(result["aircraft"]["icao"], "C172");
}

/// Known v1 profile produces exact expected v3 output after full chain.
#[test]
fn golden_v1_to_v3() {
    let input = json!({
        "schema_version": "v1",
        "sim": "dcs",
        "aircraft": { "icao": "F18" },
        "axes": {
            "pitch":   { "deadzone": 0.02, "expo": 0.15 },
            "roll":    { "deadzone": 0.02, "expo": 0.15 },
            "throttle": { "deadzone": 0.01 }
        }
    });

    let reg = MigrationRegistry::new();
    let result = reg.migrate(input, "v1", "v3").unwrap();

    // Verify exact v3 output for pitch
    let pitch = result["axes"]["pitch"].as_object().unwrap();
    assert_eq!(pitch.get("deadzone").unwrap(), &json!(0.02));
    assert_eq!(pitch.get("exponential").unwrap(), &json!(0.15));
    assert_eq!(pitch.get("sensitivity").unwrap(), &json!(1.0));
    assert_eq!(pitch.get("response_curve_type").unwrap(), &json!("default"));
    assert!(pitch.get("expo").is_none());

    // Verify throttle (no expo → no exponential)
    let throttle = result["axes"]["throttle"].as_object().unwrap();
    assert_eq!(throttle.get("deadzone").unwrap(), &json!(0.01));
    assert_eq!(throttle.get("sensitivity").unwrap(), &json!(1.0));
    assert_eq!(
        throttle.get("response_curve_type").unwrap(),
        &json!("default")
    );
    assert!(throttle.get("expo").is_none());
    assert!(throttle.get("exponential").is_none());

    assert_eq!(result["schema_version"], "v3");
    assert_eq!(result["sim"], "dcs");
}

/// Known v2 profile produces exact expected v3 output.
#[test]
fn golden_v2_to_v3() {
    let input = json!({
        "schema_version": "v2",
        "sim": "xplane",
        "axes": {
            "pitch": { "deadzone": 0.03, "expo": 0.2, "sensitivity": 0.9 },
            "yaw":   { "deadzone": 0.1, "sensitivity": 1.2 }
        }
    });

    let reg = MigrationRegistry::new();
    let result = reg.migrate(input, "v2", "v3").unwrap();

    let pitch = result["axes"]["pitch"].as_object().unwrap();
    assert_eq!(pitch.get("deadzone").unwrap(), &json!(0.03));
    assert_eq!(pitch.get("exponential").unwrap(), &json!(0.2));
    assert_eq!(pitch.get("sensitivity").unwrap(), &json!(0.9));
    assert_eq!(pitch.get("response_curve_type").unwrap(), &json!("default"));
    assert!(pitch.get("expo").is_none());

    let yaw = result["axes"]["yaw"].as_object().unwrap();
    assert_eq!(yaw.get("deadzone").unwrap(), &json!(0.1));
    assert_eq!(yaw.get("sensitivity").unwrap(), &json!(1.2));
    assert!(yaw.get("expo").is_none());
    assert!(yaw.get("exponential").is_none());
}

// ── 13. Property-based tests ────────────────────────────────────────────────

mod proptest_migration {
    use super::*;
    use proptest::prelude::*;

    fn arb_axis_v1() -> impl Strategy<Value = Value> {
        (
            0.0f64..0.5,                     // deadzone
            prop::option::of(0.0f64..1.0),   // expo
            prop::option::of(0.0f64..100.0), // slew_rate
        )
            .prop_map(|(dz, expo, slew)| {
                let mut obj = serde_json::Map::new();
                obj.insert("deadzone".to_string(), json!(dz));
                if let Some(e) = expo {
                    obj.insert("expo".to_string(), json!(e));
                }
                if let Some(s) = slew {
                    obj.insert("slew_rate".to_string(), json!(s));
                }
                Value::Object(obj)
            })
    }

    fn arb_v1_profile() -> impl Strategy<Value = Value> {
        prop::collection::hash_map("[a-z]{1,6}", arb_axis_v1(), 1..8).prop_map(|axes| {
            let axes_obj: serde_json::Map<String, Value> = axes.into_iter().collect();
            json!({
                "schema_version": "v1",
                "axes": Value::Object(axes_obj)
            })
        })
    }

    proptest! {
        /// Any valid v1 profile can be migrated to v2 without error.
        #[test]
        fn arbitrary_v1_migrates_to_v2(profile in arb_v1_profile()) {
            let reg = MigrationRegistry::new();
            let result = reg.migrate(profile, "v1", "v2");
            prop_assert!(result.is_ok(), "v1→v2 migration failed: {:?}", result.err());
            let v2 = result.unwrap();
            prop_assert!(v2["schema_version"] == "v2");
        }

        /// Any valid v1 profile can be migrated to v3 without error.
        #[test]
        fn arbitrary_v1_migrates_to_v3(profile in arb_v1_profile()) {
            let reg = MigrationRegistry::new();
            let result = reg.migrate(profile, "v1", "v3");
            prop_assert!(result.is_ok(), "v1→v3 migration failed: {:?}", result.err());
            let v3 = result.unwrap();
            prop_assert!(v3["schema_version"] == "v3");
        }

        /// v1→v3 always produces axes with sensitivity and response_curve_type.
        #[test]
        fn arbitrary_v1_to_v3_has_required_fields(profile in arb_v1_profile()) {
            let reg = MigrationRegistry::new();
            let result = reg.migrate(profile, "v1", "v3").unwrap();
            let axes = result["axes"].as_object().unwrap();
            for (name, axis) in axes {
                let obj = axis.as_object().unwrap();
                prop_assert!(
                    obj.contains_key("sensitivity"),
                    "axis {name} missing sensitivity"
                );
                prop_assert!(
                    obj.contains_key("response_curve_type"),
                    "axis {name} missing response_curve_type"
                );
                prop_assert!(
                    !obj.contains_key("expo"),
                    "axis {name} still has 'expo' after v3 migration"
                );
            }
        }

        /// Migration preserves axis count.
        #[test]
        fn arbitrary_migration_preserves_axis_count(profile in arb_v1_profile()) {
            let reg = MigrationRegistry::new();
            let input_count = profile["axes"].as_object().unwrap().len();
            let result = reg.migrate(profile, "v1", "v3").unwrap();
            let output_count = result["axes"].as_object().unwrap().len();
            prop_assert_eq!(input_count, output_count, "axis count changed");
        }

        /// Migration preserves deadzone values.
        #[test]
        fn arbitrary_migration_preserves_deadzones(profile in arb_v1_profile()) {
            let reg = MigrationRegistry::new();
            let input_axes = profile["axes"].as_object().unwrap().clone();
            let result = reg.migrate(profile, "v1", "v3").unwrap();
            for (name, orig) in &input_axes {
                let migrated = &result["axes"][name];
                prop_assert!(
                    orig["deadzone"] == migrated["deadzone"],
                    "deadzone changed for axis {}", name
                );
            }
        }
    }
}
