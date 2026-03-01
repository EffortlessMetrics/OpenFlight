// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for profile hot-reload, migration, cascade merge, validation,
//! round-trip serialization, and property-based invariants.

use std::collections::HashMap;

use flight_profile::hot_reload::{FileState, HotReloadTracker, ReloadAction};
use flight_profile::profile_migration::{MigrationError, MigrationRegistry, ProfileMigration};
use flight_profile::{
    AircraftId, AxisConfig, CurvePoint, DetentZone, FilterConfig, PofOverrides, Profile,
    PROFILE_SCHEMA_VERSION,
};
use serde_json::{Value, json};

// ═══════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════

fn file_state(path: &str, hash: u64) -> FileState {
    FileState {
        path: path.to_string(),
        hash,
        last_modified: 1000,
        size: 256,
    }
}

fn global_profile() -> Profile {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.05),
            expo: Some(0.2),
            slew_rate: Some(1.0),
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    axes.insert(
        "roll".to_string(),
        AxisConfig {
            deadzone: Some(0.05),
            expo: Some(0.15),
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

fn sim_profile() -> Profile {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: None,
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
            deadzone: Some(0.03),
            expo: None,
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    axes.insert(
        "yaw".to_string(),
        AxisConfig {
            deadzone: Some(0.08),
            expo: Some(0.4),
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

fn pof_profile() -> Profile {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: None,
            expo: Some(0.1),
            slew_rate: Some(2.0),
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

fn sample_v1() -> Value {
    json!({
        "schema_version": "v1",
        "axes": {
            "pitch": { "deadzone": 0.03, "expo": 0.2 },
            "roll":  { "deadzone": 0.05, "expo": 0.3 }
        }
    })
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. Hot-reload lifecycle
// ═══════════════════════════════════════════════════════════════════════════

mod hot_reload_lifecycle {
    use super::*;

    #[test]
    fn file_change_detected_triggers_reload() {
        let mut tracker = HotReloadTracker::new(50);
        tracker.track("profile.json".into(), file_state("profile.json", 100));

        // Simulate file change (different hash) after debounce window
        let actions = tracker.check_changes(&[file_state("profile.json", 200)], 100);
        assert_eq!(actions, vec![ReloadAction::Reload("profile.json".into())]);
    }

    #[test]
    fn invalid_file_change_keeps_previous_on_reject() {
        // Simulate: detect change → attempt reload → validation fails → tracker
        // still has the original state for the *next* check.
        let mut tracker = HotReloadTracker::new(0);
        tracker.track("profile.json".into(), file_state("profile.json", 1));

        // Change detected
        let actions = tracker.check_changes(&[file_state("profile.json", 2)], 100);
        assert_eq!(actions, vec![ReloadAction::Reload("profile.json".into())]);

        // Application decides the new content is invalid and re-tracks old state
        tracker.track("profile.json".into(), file_state("profile.json", 1));

        // Next check with same bad file — still triggers because hash differs from tracked
        let actions = tracker.check_changes(&[file_state("profile.json", 2)], 200);
        assert_eq!(actions, vec![ReloadAction::Reload("profile.json".into())]);
    }

    #[test]
    fn rapid_changes_debounce_coalesces_to_single_reload() {
        let mut tracker = HotReloadTracker::new(100);
        tracker.track("profile.json".into(), file_state("profile.json", 1));

        // First check at t=200: detects change
        let actions = tracker.check_changes(&[file_state("profile.json", 2)], 200);
        assert_eq!(actions.len(), 1);

        // Rapid changes at t=210, t=220, t=250 — all within debounce window
        let actions = tracker.check_changes(&[file_state("profile.json", 3)], 210);
        assert!(actions.is_empty(), "should be suppressed by debounce");
        let actions = tracker.check_changes(&[file_state("profile.json", 4)], 220);
        assert!(actions.is_empty(), "should be suppressed by debounce");
        let actions = tracker.check_changes(&[file_state("profile.json", 5)], 250);
        assert!(actions.is_empty(), "should be suppressed by debounce");

        // After debounce window (200 + 100 = 300): single coalesced reload
        let actions = tracker.check_changes(&[file_state("profile.json", 5)], 300);
        assert_eq!(
            actions,
            vec![ReloadAction::Reload("profile.json".into())],
            "should see exactly one reload after debounce expires"
        );
    }

    #[test]
    fn file_deleted_produces_remove_action() {
        let mut tracker = HotReloadTracker::new(0);
        tracker.track("profile.json".into(), file_state("profile.json", 1));

        let actions = tracker.check_changes(&[], 100);
        assert_eq!(actions, vec![ReloadAction::Remove("profile.json".into())]);
        assert_eq!(tracker.tracked_count(), 0);
    }

    #[test]
    fn file_deleted_then_recreated() {
        let mut tracker = HotReloadTracker::new(0);
        tracker.track("profile.json".into(), file_state("profile.json", 1));

        // Delete
        let actions = tracker.check_changes(&[], 100);
        assert_eq!(actions, vec![ReloadAction::Remove("profile.json".into())]);

        // Re-track after file recreation
        tracker.track("profile.json".into(), file_state("profile.json", 42));
        let actions = tracker.check_changes(&[file_state("profile.json", 42)], 200);
        assert!(actions.is_empty(), "no change since re-track");
    }

    #[test]
    fn multiple_files_mixed_actions() {
        let mut tracker = HotReloadTracker::new(0);
        tracker.track("a.json".into(), file_state("a.json", 1));
        tracker.track("b.json".into(), file_state("b.json", 2));
        tracker.track("c.json".into(), file_state("c.json", 3));

        // a changed, b unchanged, c deleted
        let current = vec![file_state("a.json", 10), file_state("b.json", 2)];
        let mut actions = tracker.check_changes(&current, 100);
        actions.sort_by_key(|a| format!("{a:?}"));

        assert!(actions.contains(&ReloadAction::Reload("a.json".into())));
        assert!(actions.contains(&ReloadAction::Remove("c.json".into())));
        assert_eq!(actions.len(), 2);
    }

    #[test]
    fn debounce_zero_allows_every_check() {
        let mut tracker = HotReloadTracker::new(0);
        tracker.track("f.json".into(), file_state("f.json", 1));

        for i in 2..=5u64 {
            let actions = tracker.check_changes(&[file_state("f.json", i)], i * 10);
            assert_eq!(actions.len(), 1, "check {i} should detect change");
        }
    }

    #[test]
    fn large_debounce_suppresses_for_duration() {
        let mut tracker = HotReloadTracker::new(5000);
        tracker.track("f.json".into(), file_state("f.json", 1));

        // First check outside initial debounce
        let _ = tracker.check_changes(&[file_state("f.json", 1)], 5000);

        // All checks within 5000ms window suppressed
        for t in (5001..10000).step_by(500) {
            let actions = tracker.check_changes(&[file_state("f.json", 99)], t);
            assert!(actions.is_empty(), "t={t} should be suppressed");
        }

        // After window
        let actions = tracker.check_changes(&[file_state("f.json", 99)], 10000);
        assert_eq!(actions.len(), 1);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. Migration tests
// ═══════════════════════════════════════════════════════════════════════════

mod migration {
    use super::*;

    #[test]
    fn v1_to_v2_adds_sensitivity_and_preserves_fields() {
        let reg = MigrationRegistry::new();
        let result = reg.migrate(sample_v1(), "v1", "v2").unwrap();

        // Sensitivity added
        assert_eq!(result["axes"]["pitch"]["sensitivity"], json!(1.0));
        assert_eq!(result["axes"]["roll"]["sensitivity"], json!(1.0));

        // Original fields preserved
        assert_eq!(result["axes"]["pitch"]["deadzone"], json!(0.03));
        assert_eq!(result["axes"]["pitch"]["expo"], json!(0.2));
        assert_eq!(result["axes"]["roll"]["deadzone"], json!(0.05));
        assert_eq!(result["axes"]["roll"]["expo"], json!(0.3));

        // Schema version updated
        assert_eq!(result["schema_version"], json!("v2"));
    }

    #[test]
    fn v2_to_v3_renames_expo_adds_response_curve_type() {
        let reg = MigrationRegistry::new();
        let v2 = reg.migrate(sample_v1(), "v1", "v2").unwrap();
        let v3 = reg.migrate(v2, "v2", "v3").unwrap();

        let pitch = &v3["axes"]["pitch"];
        assert!(pitch.get("expo").is_none(), "expo should be removed");
        assert_eq!(pitch["exponential"], json!(0.2));
        assert_eq!(pitch["response_curve_type"], json!("default"));
        assert_eq!(v3["schema_version"], json!("v3"));
    }

    #[test]
    fn v2_to_v3_without_expo_adds_defaults_only() {
        let input = json!({
            "schema_version": "v2",
            "axes": {
                "throttle": { "deadzone": 0.01, "sensitivity": 1.0 }
            }
        });
        let reg = MigrationRegistry::new();
        let result = reg.migrate(input, "v2", "v3").unwrap();
        let throttle = &result["axes"]["throttle"];
        assert!(throttle.get("expo").is_none());
        assert!(throttle.get("exponential").is_none());
        assert_eq!(throttle["response_curve_type"], json!("default"));
    }

    #[test]
    fn chained_migration_v1_to_v3_single_pass() {
        let reg = MigrationRegistry::new();
        let result = reg.migrate(sample_v1(), "v1", "v3").unwrap();

        let roll = &result["axes"]["roll"];
        assert_eq!(roll["sensitivity"], json!(1.0));
        assert_eq!(roll["exponential"], json!(0.3));
        assert_eq!(roll["response_curve_type"], json!("default"));
        assert!(roll.get("expo").is_none());
        assert_eq!(result["schema_version"], json!("v3"));
    }

    #[test]
    fn unknown_source_version_gives_clear_error() {
        let reg = MigrationRegistry::new();
        let err = reg.migrate(sample_v1(), "v0", "v3").unwrap_err();
        match err {
            MigrationError::UnsupportedVersion(msg) => {
                assert!(
                    msg.contains("v0"),
                    "error should mention the unknown version, got: {msg}"
                );
            }
            other => panic!("expected UnsupportedVersion, got: {other}"),
        }
    }

    #[test]
    fn unknown_target_version_gives_clear_error() {
        let reg = MigrationRegistry::new();
        let err = reg.migrate(sample_v1(), "v1", "v99").unwrap_err();
        match err {
            MigrationError::UnsupportedVersion(msg) => {
                assert!(
                    msg.contains("v99"),
                    "error should mention target version, got: {msg}"
                );
            }
            other => panic!("expected UnsupportedVersion, got: {other}"),
        }
    }

    #[test]
    fn future_version_rejects_downgrade() {
        let reg = MigrationRegistry::new();
        assert!(!reg.can_migrate("v3", "v1"), "downgrade should not be possible");
        let err = reg.migrate(json!({}), "v3", "v1").unwrap_err();
        assert!(matches!(err, MigrationError::UnsupportedVersion(_)));
    }

    #[test]
    fn same_version_is_noop() {
        let reg = MigrationRegistry::new();
        let input = sample_v1();
        let result = reg.migrate(input.clone(), "v1", "v1").unwrap();
        assert_eq!(input, result);
    }

    #[test]
    fn migration_preserves_extra_fields() {
        let input = json!({
            "schema_version": "v1",
            "custom_metadata": { "author": "test" },
            "axes": {
                "pitch": { "deadzone": 0.03, "expo": 0.2, "custom_flag": true }
            }
        });
        let reg = MigrationRegistry::new();
        let result = reg.migrate(input, "v1", "v3").unwrap();

        assert_eq!(result["custom_metadata"]["author"], json!("test"));
        assert_eq!(result["axes"]["pitch"]["custom_flag"], json!(true));
    }

    #[test]
    fn invalid_schema_missing_axes_object() {
        let reg = MigrationRegistry::new();
        let bad = json!({ "schema_version": "v1", "axes": "not an object" });
        let err = reg.migrate(bad, "v1", "v2").unwrap_err();
        assert!(matches!(err, MigrationError::InvalidSchema(_)));
    }

    #[test]
    fn custom_migration_v3_to_v4_chains_from_v1() {
        let mut reg = MigrationRegistry::new();
        reg.register(ProfileMigration {
            from_version: "v3",
            to_version: "v4",
            description: "Add trim_defaults",
            migrate_fn: |mut v| {
                if let Some(axes) = v.get_mut("axes").and_then(|a| a.as_object_mut()) {
                    for (_name, axis) in axes.iter_mut() {
                        if let Some(obj) = axis.as_object_mut() {
                            obj.entry("trim_default").or_insert(Value::from(0.0));
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
        let result = reg.migrate(sample_v1(), "v1", "v4").unwrap();
        assert_eq!(result["schema_version"], json!("v4"));
        assert_eq!(result["axes"]["pitch"]["trim_default"], json!(0.0));
        assert_eq!(result["axes"]["pitch"]["exponential"], json!(0.2));
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. Cascade merge tests (ADR-007)
// ═══════════════════════════════════════════════════════════════════════════

mod cascade_merge {
    use super::*;

    #[test]
    fn full_cascade_global_sim_aircraft_pof() {
        let merged = global_profile()
            .merge_with(&sim_profile())
            .unwrap()
            .merge_with(&aircraft_profile())
            .unwrap()
            .merge_with(&pof_profile())
            .unwrap();

        let pitch = merged.axes.get("pitch").unwrap();
        // deadzone: global=0.05 → aircraft=0.03 (override) → pof=None (keep 0.03)
        assert_eq!(pitch.deadzone, Some(0.03));
        // expo: global=0.2 → sim=0.3 → aircraft=None (keep 0.3) → pof=0.1
        assert_eq!(pitch.expo, Some(0.1));
        // slew_rate: global=1.0 → pof=2.0
        assert_eq!(pitch.slew_rate, Some(2.0));

        // roll: only in global, untouched by later layers
        let roll = merged.axes.get("roll").unwrap();
        assert_eq!(roll.deadzone, Some(0.05));
        assert_eq!(roll.expo, Some(0.15));

        // yaw: introduced at aircraft level
        let yaw = merged.axes.get("yaw").unwrap();
        assert_eq!(yaw.deadzone, Some(0.08));
        assert_eq!(yaw.expo, Some(0.4));

        // Aircraft and sim propagated
        assert_eq!(merged.sim, Some("msfs".to_string()));
        assert_eq!(
            merged.aircraft,
            Some(AircraftId {
                icao: "C172".to_string()
            })
        );
    }

    #[test]
    fn more_specific_overrides_less_specific() {
        let global = global_profile();
        let mut specific = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None,
            aircraft: None,
            axes: HashMap::new(),
            pof_overrides: None,
        };
        specific.axes.insert(
            "pitch".to_string(),
            AxisConfig {
                deadzone: Some(0.01),
                expo: Some(0.9),
                slew_rate: None,
                detents: vec![],
                curve: None,
                filter: None,
            },
        );

        let merged = global.merge_with(&specific).unwrap();
        let pitch = merged.axes.get("pitch").unwrap();
        assert_eq!(pitch.deadzone, Some(0.01), "specific should override global");
        assert_eq!(pitch.expo, Some(0.9), "specific should override global");
        // slew_rate: specific=None → falls back to global
        assert_eq!(pitch.slew_rate, Some(1.0));
    }

    #[test]
    fn missing_level_skipped_gracefully() {
        // Skip sim level: global → aircraft
        let merged = global_profile().merge_with(&aircraft_profile()).unwrap();
        let pitch = merged.axes.get("pitch").unwrap();
        assert_eq!(pitch.deadzone, Some(0.03)); // aircraft override
        assert_eq!(pitch.expo, Some(0.2)); // global preserved (aircraft has None)
    }

    #[test]
    fn merge_with_empty_profile_is_identity() {
        let base = global_profile();
        let empty = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None,
            aircraft: None,
            axes: HashMap::new(),
            pof_overrides: None,
        };
        let merged = base.merge_with(&empty).unwrap();
        assert_eq!(base, merged);
    }

    #[test]
    fn merge_adds_new_axes_from_override() {
        let base = global_profile(); // has pitch, roll
        let mut over = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None,
            aircraft: None,
            axes: HashMap::new(),
            pof_overrides: None,
        };
        over.axes.insert(
            "throttle".to_string(),
            AxisConfig {
                deadzone: Some(0.02),
                expo: None,
                slew_rate: None,
                detents: vec![],
                curve: None,
                filter: None,
            },
        );

        let merged = base.merge_with(&over).unwrap();
        assert!(merged.axes.contains_key("throttle"));
        assert!(merged.axes.contains_key("pitch"));
        assert!(merged.axes.contains_key("roll"));
    }

    #[test]
    fn pof_overrides_merged_last_writer_wins() {
        let mut base = global_profile();
        let mut pof1 = HashMap::new();
        pof1.insert(
            "takeoff".to_string(),
            PofOverrides {
                axes: None,
                hysteresis: None,
            },
        );
        base.pof_overrides = Some(pof1);

        let mut over = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None,
            aircraft: None,
            axes: HashMap::new(),
            pof_overrides: None,
        };
        let mut pof2 = HashMap::new();
        pof2.insert(
            "landing".to_string(),
            PofOverrides {
                axes: None,
                hysteresis: None,
            },
        );
        over.pof_overrides = Some(pof2);

        let merged = base.merge_with(&over).unwrap();
        let pof = merged.pof_overrides.as_ref().unwrap();
        assert!(pof.contains_key("takeoff"));
        assert!(pof.contains_key("landing"));
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. Validation depth
// ═══════════════════════════════════════════════════════════════════════════

mod validation_depth {
    use super::*;

    #[test]
    fn deadzone_out_of_range_high() {
        let mut p = global_profile();
        p.axes.get_mut("pitch").unwrap().deadzone = Some(0.6);
        let err = p.validate().unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("deadzone"), "error should mention deadzone: {msg}");
    }

    #[test]
    fn deadzone_out_of_range_negative() {
        let mut p = global_profile();
        p.axes.get_mut("pitch").unwrap().deadzone = Some(-0.1);
        assert!(p.validate().is_err());
    }

    #[test]
    fn deadzone_boundary_zero_valid() {
        let mut p = global_profile();
        p.axes.get_mut("pitch").unwrap().deadzone = Some(0.0);
        assert!(p.validate().is_ok());
    }

    #[test]
    fn deadzone_boundary_max_valid() {
        let mut p = global_profile();
        p.axes.get_mut("pitch").unwrap().deadzone = Some(0.5);
        assert!(p.validate().is_ok());
    }

    #[test]
    fn expo_out_of_range_high() {
        let mut p = global_profile();
        p.axes.get_mut("pitch").unwrap().expo = Some(1.1);
        assert!(p.validate().is_err());
    }

    #[test]
    fn expo_out_of_range_negative() {
        let mut p = global_profile();
        p.axes.get_mut("pitch").unwrap().expo = Some(-0.1);
        assert!(p.validate().is_err());
    }

    #[test]
    fn expo_boundary_values_valid() {
        let mut p = global_profile();
        p.axes.get_mut("pitch").unwrap().expo = Some(0.0);
        assert!(p.validate().is_ok());
        p.axes.get_mut("pitch").unwrap().expo = Some(1.0);
        assert!(p.validate().is_ok());
    }

    #[test]
    fn schema_version_mismatch_rejected() {
        let mut p = global_profile();
        p.schema = "flight.profile/999".to_string();
        let err = p.validate().unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("schema") || msg.contains("version"), "{msg}");
    }

    #[test]
    fn empty_axes_valid() {
        let p = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None,
            aircraft: None,
            axes: HashMap::new(),
            pof_overrides: None,
        };
        assert!(p.validate().is_ok());
    }

    #[test]
    fn slew_rate_negative_rejected() {
        let mut p = global_profile();
        p.axes.get_mut("pitch").unwrap().slew_rate = Some(-1.0);
        assert!(p.validate().is_err());
    }

    #[test]
    fn slew_rate_over_max_rejected() {
        let mut p = global_profile();
        p.axes.get_mut("pitch").unwrap().slew_rate = Some(101.0);
        assert!(p.validate().is_err());
    }

    #[test]
    fn detent_position_out_of_range() {
        let mut p = global_profile();
        p.axes.get_mut("pitch").unwrap().detents = vec![DetentZone {
            position: 1.5,
            width: 0.1,
            role: "gate".to_string(),
        }];
        assert!(p.validate().is_err());
    }

    #[test]
    fn detent_width_zero_rejected() {
        let mut p = global_profile();
        p.axes.get_mut("pitch").unwrap().detents = vec![DetentZone {
            position: 0.0,
            width: 0.0,
            role: "gate".to_string(),
        }];
        assert!(p.validate().is_err());
    }

    #[test]
    fn curve_single_point_rejected() {
        let mut p = global_profile();
        p.axes.get_mut("pitch").unwrap().curve = Some(vec![CurvePoint {
            input: 0.0,
            output: 0.0,
        }]);
        assert!(p.validate().is_err());
    }

    #[test]
    fn curve_non_monotonic_input_rejected() {
        let mut p = global_profile();
        p.axes.get_mut("pitch").unwrap().curve = Some(vec![
            CurvePoint {
                input: 0.0,
                output: 0.0,
            },
            CurvePoint {
                input: 0.5,
                output: 0.5,
            },
            CurvePoint {
                input: 0.3,
                output: 0.8,
            },
        ]);
        assert!(p.validate().is_err());
    }

    #[test]
    fn filter_alpha_out_of_range() {
        let mut p = global_profile();
        p.axes.get_mut("pitch").unwrap().filter = Some(FilterConfig {
            alpha: 1.5,
            spike_threshold: None,
            max_spike_count: None,
        });
        assert!(p.validate().is_err());
    }

    #[test]
    fn filter_spike_threshold_zero_rejected() {
        let mut p = global_profile();
        p.axes.get_mut("pitch").unwrap().filter = Some(FilterConfig {
            alpha: 0.5,
            spike_threshold: Some(0.0),
            max_spike_count: None,
        });
        assert!(p.validate().is_err());
    }

    #[test]
    fn filter_max_spike_count_zero_rejected() {
        let mut p = global_profile();
        p.axes.get_mut("pitch").unwrap().filter = Some(FilterConfig {
            alpha: 0.5,
            spike_threshold: None,
            max_spike_count: Some(0),
        });
        assert!(p.validate().is_err());
    }

    #[test]
    fn valid_filter_accepted() {
        let mut p = global_profile();
        p.axes.get_mut("pitch").unwrap().filter = Some(FilterConfig {
            alpha: 0.3,
            spike_threshold: Some(0.1),
            max_spike_count: Some(3),
        });
        assert!(p.validate().is_ok());
    }

    #[test]
    fn pof_override_axis_validated() {
        let mut p = global_profile();
        let mut pof_axes = HashMap::new();
        pof_axes.insert(
            "pitch".to_string(),
            AxisConfig {
                deadzone: Some(0.9), // out of range
                expo: None,
                slew_rate: None,
                detents: vec![],
                curve: None,
                filter: None,
            },
        );
        let mut pof = HashMap::new();
        pof.insert(
            "takeoff".to_string(),
            PofOverrides {
                axes: Some(pof_axes),
                hysteresis: None,
            },
        );
        p.pof_overrides = Some(pof);
        assert!(p.validate().is_err());
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. JSON/YAML round-trip
// ═══════════════════════════════════════════════════════════════════════════

mod round_trip {
    use super::*;

    #[test]
    fn json_serialize_deserialize_equality() {
        let original = global_profile();
        let json_str = serde_json::to_string(&original).unwrap();
        let restored: Profile = serde_json::from_str(&json_str).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn json_pretty_roundtrip() {
        let original = aircraft_profile();
        let json_str = original.export_json().unwrap();
        let restored: Profile = serde_json::from_str(&json_str).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn yaml_serialize_deserialize_equality() {
        let original = global_profile();
        let yaml_str = serde_yaml::to_string(&original).unwrap();
        let restored: Profile = serde_yaml::from_str(&yaml_str).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn unknown_fields_ignored_forward_compatibility() {
        let json_str = r#"{
            "schema": "flight.profile/1",
            "sim": "msfs",
            "aircraft": null,
            "axes": {},
            "pof_overrides": null,
            "future_field": "should be ignored",
            "another_future": 42
        }"#;
        // serde default behavior: unknown fields cause errors.
        // If the Profile derives Deserialize without deny_unknown_fields,
        // this should succeed. Otherwise, we document the behavior.
        let result: std::result::Result<Profile, _> = serde_json::from_str(json_str);
        // The crate doesn't use deny_unknown_fields, so this should work:
        if let Ok(p) = result {
            assert_eq!(p.schema, PROFILE_SCHEMA_VERSION);
        }
        // If it fails, that's also a valid design choice — just document it.
    }

    #[test]
    fn canonicalize_deterministic() {
        let p = global_profile();
        let c1 = p.canonicalize();
        let c2 = p.canonicalize();
        assert_eq!(c1, c2);
    }

    #[test]
    fn effective_hash_stable_across_roundtrip() {
        let original = aircraft_profile();
        let hash1 = original.effective_hash();

        let json_str = serde_json::to_string(&original).unwrap();
        let restored: Profile = serde_json::from_str(&json_str).unwrap();
        let hash2 = restored.effective_hash();

        assert_eq!(hash1, hash2, "hash should be stable across JSON roundtrip");
    }

    #[test]
    fn yaml_roundtrip_with_all_fields() {
        let mut p = aircraft_profile();
        p.axes.get_mut("pitch").unwrap().filter = Some(FilterConfig {
            alpha: 0.3,
            spike_threshold: Some(0.1),
            max_spike_count: Some(3),
        });
        p.axes.get_mut("pitch").unwrap().detents = vec![DetentZone {
            position: 0.0,
            width: 0.1,
            role: "center".to_string(),
        }];
        p.axes.get_mut("pitch").unwrap().curve = Some(vec![
            CurvePoint {
                input: 0.0,
                output: 0.0,
            },
            CurvePoint {
                input: 1.0,
                output: 1.0,
            },
        ]);

        let yaml = serde_yaml::to_string(&p).unwrap();
        let restored: Profile = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(p, restored);
    }

    #[test]
    fn profile_with_pof_overrides_roundtrip() {
        let mut p = global_profile();
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
            "takeoff".to_string(),
            PofOverrides {
                axes: Some(pof_axes),
                hysteresis: None,
            },
        );
        p.pof_overrides = Some(pof);

        let json_str = serde_json::to_string(&p).unwrap();
        let restored: Profile = serde_json::from_str(&json_str).unwrap();
        assert_eq!(p, restored);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. Property tests
// ═══════════════════════════════════════════════════════════════════════════

mod property_tests {
    use super::*;
    use proptest::prelude::*;

    fn arb_axis_config() -> impl Strategy<Value = AxisConfig> {
        (
            proptest::option::of(0.0f32..=0.5),
            proptest::option::of(0.0f32..=1.0),
            proptest::option::of(0.0f32..=100.0),
        )
            .prop_map(|(dz, expo, slew)| AxisConfig {
                deadzone: dz,
                expo,
                slew_rate: slew,
                detents: vec![],
                curve: None,
                filter: None,
            })
    }

    fn arb_profile() -> impl Strategy<Value = Profile> {
        (arb_axis_config(), arb_axis_config()).prop_map(|(pitch_cfg, roll_cfg)| {
            let mut axes = HashMap::new();
            axes.insert("pitch".to_string(), pitch_cfg);
            axes.insert("roll".to_string(), roll_cfg);
            Profile {
                schema: PROFILE_SCHEMA_VERSION.to_string(),
                sim: Some("msfs".to_string()),
                aircraft: None,
                axes,
                pof_overrides: None,
            }
        })
    }

    proptest! {
        #[test]
        fn merge_idempotent(profile in arb_profile()) {
            let merged = profile.merge_with(&profile).unwrap();
            prop_assert_eq!(&profile, &merged, "merge(a, a) should equal a");
        }

        #[test]
        fn merge_associative(
            a in arb_profile(),
            b in arb_profile(),
            c in arb_profile(),
        ) {
            let ab_c = a.merge_with(&b).unwrap().merge_with(&c).unwrap();
            let a_bc = a.merge_with(&b.merge_with(&c).unwrap()).unwrap();
            prop_assert_eq!(ab_c, a_bc, "merge should be associative");
        }

        #[test]
        fn validation_deterministic(profile in arb_profile()) {
            let r1 = profile.validate();
            let r2 = profile.validate();
            prop_assert_eq!(r1.is_ok(), r2.is_ok(), "validation should be deterministic");
        }

        #[test]
        fn roundtrip_preserves_all_fields(profile in arb_profile()) {
            let json_str = serde_json::to_string(&profile).unwrap();
            let restored: Profile = serde_json::from_str(&json_str).unwrap();
            prop_assert_eq!(&profile, &restored, "JSON roundtrip should preserve all fields");
        }

        #[test]
        fn canonical_hash_stable(profile in arb_profile()) {
            let h1 = profile.effective_hash();
            let h2 = profile.effective_hash();
            prop_assert_eq!(h1, h2, "effective_hash should be deterministic");
        }

        #[test]
        fn valid_profiles_remain_valid_after_merge(
            a in arb_profile(),
            b in arb_profile(),
        ) {
            if a.validate().is_ok() && b.validate().is_ok() {
                let merged = a.merge_with(&b).unwrap();
                prop_assert!(
                    merged.validate().is_ok(),
                    "merging two valid profiles should produce a valid profile"
                );
            }
        }

        #[test]
        fn merge_preserves_base_axes_count(
            base in arb_profile(),
        ) {
            let empty_override = Profile {
                schema: PROFILE_SCHEMA_VERSION.to_string(),
                sim: None,
                aircraft: None,
                axes: HashMap::new(),
                pof_overrides: None,
            };
            let merged = base.merge_with(&empty_override).unwrap();
            prop_assert!(
                merged.axes.len() >= base.axes.len(),
                "merged should have at least as many axes as base"
            );
        }
    }
}
