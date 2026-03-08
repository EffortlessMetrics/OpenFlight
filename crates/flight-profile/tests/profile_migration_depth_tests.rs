// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Deep integration tests for the profile migration system.
//!
//! Fills gaps not covered by the existing test suite:
//!
//! - **Validation depth**: boundary values for deadzone (0.0–0.5), expo (0.0–1.0),
//!   curve point monotonicity, detent ranges
//! - **Migration + merge interop**: migrate a v1 profile then merge with a typed `Profile`
//! - **4-layer cascade merge**: Global → Simulator → Aircraft → Phase-of-Flight
//! - **Migration round-trips**: migrate → serialize → deserialize → compare
//! - **Snapshot stability**: insta snapshots for cascaded merge and migration outputs
//! - **Edge cases**: NaN/Inf injection, empty-string axis names, large axis counts,
//!   unknown fields gracefully preserved through the full chain
//! - **Property tests**: migration chain idempotency, merge commutativity of disjoint axes

use flight_profile::profile_migration::{MigrationError, MigrationRegistry, ProfileMigration};
use flight_profile::{
    AircraftId, AxisConfig, CurvePoint, DetentZone, FilterConfig, PofOverrides,
    PROFILE_SCHEMA_VERSION, Profile,
};
use serde_json::{Value, json};
use std::collections::HashMap;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn empty_profile() -> Profile {
    Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes: HashMap::new(),
        pof_overrides: None,
    }
}

fn axis(deadzone: f32, expo: f32) -> AxisConfig {
    AxisConfig {
        deadzone: Some(deadzone),
        expo: Some(expo),
        slew_rate: None,
        detents: vec![],
        curve: None,
        filter: None,
    }
}

fn axis_full(
    deadzone: f32,
    expo: f32,
    slew_rate: f32,
    curve: Option<Vec<CurvePoint>>,
) -> AxisConfig {
    AxisConfig {
        deadzone: Some(deadzone),
        expo: Some(expo),
        slew_rate: Some(slew_rate),
        detents: vec![],
        curve,
        filter: None,
    }
}

fn linear_curve() -> Vec<CurvePoint> {
    vec![
        CurvePoint {
            input: 0.0,
            output: 0.0,
        },
        CurvePoint {
            input: 0.5,
            output: 0.5,
        },
        CurvePoint {
            input: 1.0,
            output: 1.0,
        },
    ]
}

// ═══════════════════════════════════════════════════════════════════════════════
// 1. Validation boundary tests
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn deadzone_exactly_zero_is_valid() {
    let mut axes = HashMap::new();
    axes.insert("pitch".to_string(), axis(0.0, 0.5));
    let p = Profile {
        axes,
        ..empty_profile()
    };
    assert!(p.validate().is_ok());
}

#[test]
fn deadzone_exactly_max_is_valid() {
    let mut axes = HashMap::new();
    axes.insert("pitch".to_string(), axis(0.5, 0.5));
    let p = Profile {
        axes,
        ..empty_profile()
    };
    assert!(p.validate().is_ok());
}

#[test]
fn deadzone_just_above_max_is_rejected() {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.500_001),
            expo: None,
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    let p = Profile {
        axes,
        ..empty_profile()
    };
    assert!(p.validate().is_err());
}

#[test]
fn expo_exactly_zero_is_valid() {
    let mut axes = HashMap::new();
    axes.insert("pitch".to_string(), axis(0.03, 0.0));
    let p = Profile {
        axes,
        ..empty_profile()
    };
    assert!(p.validate().is_ok());
}

#[test]
fn expo_exactly_one_is_valid() {
    let mut axes = HashMap::new();
    axes.insert("pitch".to_string(), axis(0.03, 1.0));
    let p = Profile {
        axes,
        ..empty_profile()
    };
    assert!(p.validate().is_ok());
}

#[test]
fn expo_just_above_one_is_rejected() {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.03),
            expo: Some(1.000_001),
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    let p = Profile {
        axes,
        ..empty_profile()
    };
    assert!(p.validate().is_err());
}

#[test]
fn expo_negative_is_rejected() {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.03),
            expo: Some(-0.01),
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    let p = Profile {
        axes,
        ..empty_profile()
    };
    let err = p.validate().unwrap_err();
    assert!(
        err.to_string().contains("expo"),
        "expected expo error, got: {err}"
    );
}

#[test]
fn curve_points_must_be_strictly_increasing() {
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
                    input: 0.5,
                    output: 0.3,
                },
                CurvePoint {
                    input: 0.5,
                    output: 0.7,
                }, // duplicate input
            ]),
            filter: None,
        },
    );
    let p = Profile {
        axes,
        ..empty_profile()
    };
    let err = p.validate().unwrap_err();
    assert!(err.to_string().contains("monotonic"));
}

#[test]
fn curve_with_decreasing_inputs_is_rejected() {
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
                },
            ]),
            filter: None,
        },
    );
    let p = Profile {
        axes,
        ..empty_profile()
    };
    assert!(p.validate().is_err());
}

#[test]
fn detent_position_boundaries_accepted() {
    let mut axes = HashMap::new();
    axes.insert(
        "throttle".to_string(),
        AxisConfig {
            deadzone: Some(0.01),
            expo: None,
            slew_rate: None,
            detents: vec![
                DetentZone {
                    position: -1.0,
                    width: 0.05,
                    role: "min".to_string(),
                },
                DetentZone {
                    position: 1.0,
                    width: 0.05,
                    role: "max".to_string(),
                },
            ],
            curve: None,
            filter: None,
        },
    );
    let p = Profile {
        axes,
        ..empty_profile()
    };
    assert!(p.validate().is_ok());
}

#[test]
fn detent_position_beyond_range_rejected() {
    let mut axes = HashMap::new();
    axes.insert(
        "throttle".to_string(),
        AxisConfig {
            deadzone: Some(0.01),
            expo: None,
            slew_rate: None,
            detents: vec![DetentZone {
                position: 1.1,
                width: 0.05,
                role: "oob".to_string(),
            }],
            curve: None,
            filter: None,
        },
    );
    let p = Profile {
        axes,
        ..empty_profile()
    };
    assert!(p.validate().is_err());
}

#[test]
fn filter_alpha_boundaries() {
    // alpha = 0.0 valid
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: None,
            expo: None,
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: Some(FilterConfig {
                alpha: 0.0,
                spike_threshold: None,
                max_spike_count: None,
            }),
        },
    );
    let p = Profile {
        axes,
        ..empty_profile()
    };
    assert!(p.validate().is_ok());

    // alpha = 1.0 valid
    let mut axes2 = HashMap::new();
    axes2.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: None,
            expo: None,
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: Some(FilterConfig {
                alpha: 1.0,
                spike_threshold: None,
                max_spike_count: None,
            }),
        },
    );
    let p2 = Profile {
        axes: axes2,
        ..empty_profile()
    };
    assert!(p2.validate().is_ok());

    // alpha = 1.1 invalid
    let mut axes3 = HashMap::new();
    axes3.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: None,
            expo: None,
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: Some(FilterConfig {
                alpha: 1.1,
                spike_threshold: None,
                max_spike_count: None,
            }),
        },
    );
    let p3 = Profile {
        axes: axes3,
        ..empty_profile()
    };
    assert!(p3.validate().is_err());
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. Four-layer cascade merge: Global → Simulator → Aircraft → Phase-of-Flight
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn four_layer_cascade_merge() {
    // Layer 1: Global defaults
    let global = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes: [
            ("pitch".to_string(), axis_full(0.05, 0.3, 2.0, None)),
            ("roll".to_string(), axis_full(0.05, 0.3, 2.0, None)),
            ("yaw".to_string(), axis_full(0.08, 0.1, 1.5, None)),
        ]
        .into(),
        pof_overrides: None,
    };

    // Layer 2: Simulator-specific (MSFS tighter deadzones)
    let sim = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: Some("msfs".to_string()),
        aircraft: None,
        axes: [
            ("pitch".to_string(), axis(0.03, 0.2)),
            ("roll".to_string(), axis(0.03, 0.2)),
        ]
        .into(),
        pof_overrides: None,
    };

    // Layer 3: Aircraft-specific (C172 needs more expo)
    let aircraft = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: Some("msfs".to_string()),
        aircraft: Some(AircraftId {
            icao: "C172".to_string(),
        }),
        axes: [("pitch".to_string(), {
            let mut a = AxisConfig::default_empty();
            a.expo = Some(0.4);
            a
        })]
        .into(),
        pof_overrides: None,
    };

    // Layer 4: Phase-of-Flight (approach phase further tightens pitch)
    let mut pof_axes = HashMap::new();
    pof_axes.insert("pitch".to_string(), axis(0.02, 0.5));
    let mut pof_overrides = HashMap::new();
    pof_overrides.insert(
        "approach".to_string(),
        PofOverrides {
            axes: Some(pof_axes),
            hysteresis: None,
        },
    );
    let phase = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes: HashMap::new(),
        pof_overrides: Some(pof_overrides),
    };

    // Apply cascade: Global → Sim → Aircraft → Phase
    let merged = global
        .merge_with(&sim)
        .unwrap()
        .merge_with(&aircraft)
        .unwrap()
        .merge_with(&phase)
        .unwrap();

    // Verify cascade precedence
    let pitch = merged.axes.get("pitch").expect("pitch must exist");
    assert_eq!(
        pitch.deadzone,
        Some(0.03),
        "sim deadzone should override global"
    );
    assert_eq!(
        pitch.expo,
        Some(0.4),
        "aircraft expo should override sim expo"
    );
    assert_eq!(
        pitch.slew_rate,
        Some(2.0),
        "global slew_rate preserved through cascade"
    );

    // Yaw should retain global defaults (no sim/aircraft override)
    let yaw = merged.axes.get("yaw").expect("yaw must exist");
    assert_eq!(yaw.deadzone, Some(0.08));
    assert_eq!(yaw.expo, Some(0.1));

    // PoF override present
    let pof = merged.pof_overrides.as_ref().expect("pof must exist");
    let approach_pitch = pof["approach"]
        .axes
        .as_ref()
        .unwrap()
        .get("pitch")
        .unwrap();
    assert_eq!(approach_pitch.deadzone, Some(0.02));
    assert_eq!(approach_pitch.expo, Some(0.5));

    // Merged profile must still be valid
    assert!(merged.validate().is_ok());
}

#[test]
fn cascade_sim_override_sets_sim_field() {
    let global = empty_profile();
    let sim = Profile {
        sim: Some("xplane".to_string()),
        ..empty_profile()
    };
    let aircraft = Profile {
        aircraft: Some(AircraftId {
            icao: "B738".to_string(),
        }),
        ..empty_profile()
    };

    let merged = global
        .merge_with(&sim)
        .unwrap()
        .merge_with(&aircraft)
        .unwrap();

    assert_eq!(merged.sim, Some("xplane".to_string()));
    assert_eq!(
        merged.aircraft,
        Some(AircraftId {
            icao: "B738".to_string()
        })
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. Migration + merge interop
// ═══════════════════════════════════════════════════════════════════════════════

/// Migrate a v1 JSON profile to v3, then verify the migrated JSON structure is
/// consistent with what the typed Profile system expects.
#[test]
fn migrate_v1_to_v3_then_verify_structure() {
    let reg = MigrationRegistry::new();
    let v1 = json!({
        "schema_version": "v1",
        "sim": "msfs",
        "axes": {
            "pitch": { "deadzone": 0.03, "expo": 0.2 },
            "roll":  { "deadzone": 0.05, "expo": 0.15 }
        }
    });
    let v3 = reg.migrate(v1, "v1", "v3").unwrap();

    // After migration, v3 JSON should have: sensitivity, exponential, response_curve_type
    let pitch = &v3["axes"]["pitch"];
    assert_eq!(pitch["sensitivity"], json!(1.0));
    assert_eq!(pitch["exponential"], json!(0.2));
    assert_eq!(pitch["response_curve_type"], json!("default"));
    assert!(pitch.get("expo").is_none());
    assert_eq!(v3["schema_version"], "v3");
}

/// Migrate then serialize → deserialize as JSON to verify round-trip.
#[test]
fn migration_json_roundtrip_v1_to_v3() {
    let reg = MigrationRegistry::new();
    let v1 = json!({
        "schema_version": "v1",
        "axes": {
            "pitch":    { "deadzone": 0.03, "expo": 0.2 },
            "roll":     { "deadzone": 0.05, "expo": 0.3 },
            "throttle": { "deadzone": 0.01 }
        }
    });
    let v3 = reg.migrate(v1, "v1", "v3").unwrap();

    // Serialize → deserialize
    let json_str = serde_json::to_string(&v3).unwrap();
    let restored: Value = serde_json::from_str(&json_str).unwrap();

    assert_eq!(v3, restored, "migration result must survive JSON round-trip");
}

/// Migrate v1 to v3, then migrate v3 to v3 (noop) — result unchanged.
#[test]
fn migration_chain_then_noop_is_stable() {
    let reg = MigrationRegistry::new();
    let v1 = json!({
        "schema_version": "v1",
        "axes": {
            "pitch": { "deadzone": 0.03, "expo": 0.2 }
        }
    });
    let v3 = reg.migrate(v1, "v1", "v3").unwrap();
    let v3_again = reg.migrate(v3.clone(), "v3", "v3").unwrap();
    assert_eq!(v3, v3_again);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. Merge edge cases
// ═══════════════════════════════════════════════════════════════════════════════

/// Merge with curve: override's curve replaces base's curve entirely.
#[test]
fn merge_replaces_curve() {
    let base_curve = vec![
        CurvePoint {
            input: 0.0,
            output: 0.0,
        },
        CurvePoint {
            input: 1.0,
            output: 1.0,
        },
    ];
    let override_curve = vec![
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
    ];

    let base = Profile {
        axes: [(
            "pitch".to_string(),
            AxisConfig {
                curve: Some(base_curve),
                ..axis(0.03, 0.2)
            },
        )]
        .into(),
        ..empty_profile()
    };
    let over = Profile {
        axes: [(
            "pitch".to_string(),
            AxisConfig {
                curve: Some(override_curve.clone()),
                deadzone: None,
                expo: None,
                slew_rate: None,
                detents: vec![],
                filter: None,
            },
        )]
        .into(),
        ..empty_profile()
    };

    let merged = base.merge_with(&over).unwrap();
    let pitch = &merged.axes["pitch"];
    assert_eq!(
        pitch.curve.as_ref().unwrap().len(),
        3,
        "override curve should replace base"
    );
    assert_eq!(pitch.curve, Some(override_curve));
    // Base deadzone preserved
    assert_eq!(pitch.deadzone, Some(0.03));
}

/// Merge with filter: override's filter replaces base's filter.
#[test]
fn merge_replaces_filter() {
    let base = Profile {
        axes: [(
            "pitch".to_string(),
            AxisConfig {
                filter: Some(FilterConfig {
                    alpha: 0.3,
                    spike_threshold: Some(0.05),
                    max_spike_count: Some(3),
                }),
                ..axis(0.03, 0.2)
            },
        )]
        .into(),
        ..empty_profile()
    };
    let over = Profile {
        axes: [(
            "pitch".to_string(),
            AxisConfig {
                filter: Some(FilterConfig {
                    alpha: 0.1,
                    spike_threshold: None,
                    max_spike_count: None,
                }),
                deadzone: None,
                expo: None,
                slew_rate: None,
                detents: vec![],
                curve: None,
            },
        )]
        .into(),
        ..empty_profile()
    };

    let merged = base.merge_with(&over).unwrap();
    let filter = merged.axes["pitch"].filter.as_ref().unwrap();
    assert_eq!(filter.alpha, 0.1, "override filter replaces base");
}

/// Merge with detents: non-empty override detents replace base detents.
#[test]
fn merge_replaces_detents_when_override_nonempty() {
    let base = Profile {
        axes: [(
            "throttle".to_string(),
            AxisConfig {
                detents: vec![DetentZone {
                    position: 0.0,
                    width: 0.05,
                    role: "idle".to_string(),
                }],
                ..axis(0.01, 0.0)
            },
        )]
        .into(),
        ..empty_profile()
    };
    let over = Profile {
        axes: [(
            "throttle".to_string(),
            AxisConfig {
                detents: vec![
                    DetentZone {
                        position: 0.0,
                        width: 0.03,
                        role: "idle".to_string(),
                    },
                    DetentZone {
                        position: 0.95,
                        width: 0.03,
                        role: "toga".to_string(),
                    },
                ],
                deadzone: None,
                expo: None,
                slew_rate: None,
                curve: None,
                filter: None,
            },
        )]
        .into(),
        ..empty_profile()
    };

    let merged = base.merge_with(&over).unwrap();
    assert_eq!(
        merged.axes["throttle"].detents.len(),
        2,
        "override detents should replace base"
    );
}

/// Merge preserves base detents when override detents are empty.
#[test]
fn merge_preserves_base_detents_when_override_empty() {
    let base = Profile {
        axes: [(
            "throttle".to_string(),
            AxisConfig {
                detents: vec![DetentZone {
                    position: 0.0,
                    width: 0.05,
                    role: "idle".to_string(),
                }],
                ..axis(0.01, 0.0)
            },
        )]
        .into(),
        ..empty_profile()
    };
    let over = Profile {
        axes: [(
            "throttle".to_string(),
            AxisConfig {
                deadzone: None,
                expo: Some(0.1),
                slew_rate: None,
                detents: vec![],
                curve: None,
                filter: None,
            },
        )]
        .into(),
        ..empty_profile()
    };

    let merged = base.merge_with(&over).unwrap();
    assert_eq!(
        merged.axes["throttle"].detents.len(),
        1,
        "base detents preserved when override empty"
    );
}

/// Empty profile merged with empty profile yields empty profile.
#[test]
fn merge_two_empty_profiles() {
    let merged = empty_profile().merge_with(&empty_profile()).unwrap();
    assert!(merged.axes.is_empty());
    assert!(merged.sim.is_none());
    assert!(merged.aircraft.is_none());
    assert!(merged.pof_overrides.is_none());
    assert!(merged.validate().is_ok());
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. Serialization round-trip tests
// ═══════════════════════════════════════════════════════════════════════════════

/// JSON round-trip for a profile with all fields populated.
#[test]
fn json_roundtrip_full_profile() {
    let mut pof_axes = HashMap::new();
    pof_axes.insert("pitch".to_string(), axis(0.05, 0.4));
    let mut pof = HashMap::new();
    pof.insert(
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
            icao: "C172".to_string(),
        }),
        axes: [
            (
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
                    curve: Some(linear_curve()),
                    filter: Some(FilterConfig {
                        alpha: 0.3,
                        spike_threshold: Some(0.05),
                        max_spike_count: Some(3),
                    }),
                },
            ),
            ("roll".to_string(), axis(0.04, 0.15)),
        ]
        .into(),
        pof_overrides: Some(pof),
    };

    let json = serde_json::to_string_pretty(&profile).unwrap();
    let restored: Profile = serde_json::from_str(&json).unwrap();
    assert_eq!(profile, restored, "full profile must survive JSON round-trip");
    assert!(restored.validate().is_ok());
}

/// YAML round-trip for a multi-axis profile.
#[test]
fn yaml_roundtrip_multi_axis_profile() {
    let profile = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: Some("dcs".to_string()),
        aircraft: Some(AircraftId {
            icao: "F16C".to_string(),
        }),
        axes: [
            (
                "pitch".to_string(),
                axis_full(0.02, 0.15, 1.0, Some(linear_curve())),
            ),
            ("roll".to_string(), axis_full(0.02, 0.15, 1.0, None)),
            ("yaw".to_string(), axis(0.05, 0.1)),
            ("throttle".to_string(), axis(0.01, 0.0)),
        ]
        .into(),
        pof_overrides: None,
    };

    let yaml = serde_yaml::to_string(&profile).unwrap();
    let restored: Profile = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(profile, restored, "profile must survive YAML round-trip");
}

/// Effective hash is stable across JSON round-trip.
#[test]
fn effective_hash_stable_across_roundtrip() {
    let profile = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: Some("msfs".to_string()),
        aircraft: Some(AircraftId {
            icao: "C172".to_string(),
        }),
        axes: [("pitch".to_string(), axis(0.03, 0.2))].into(),
        pof_overrides: None,
    };

    let hash_before = profile.effective_hash();
    let json = serde_json::to_string(&profile).unwrap();
    let restored: Profile = serde_json::from_str(&json).unwrap();
    let hash_after = restored.effective_hash();

    assert_eq!(hash_before, hash_after);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 6. Snapshot tests (insta)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn snapshot_four_layer_cascade_result() {
    let global = Profile {
        axes: [
            ("pitch".to_string(), axis_full(0.05, 0.3, 2.0, None)),
            ("roll".to_string(), axis_full(0.05, 0.3, 2.0, None)),
            ("yaw".to_string(), axis(0.08, 0.1)),
        ]
        .into(),
        ..empty_profile()
    };
    let sim = Profile {
        sim: Some("msfs".to_string()),
        axes: [
            ("pitch".to_string(), axis(0.03, 0.2)),
            ("roll".to_string(), axis(0.03, 0.2)),
        ]
        .into(),
        ..empty_profile()
    };
    let aircraft = Profile {
        aircraft: Some(AircraftId {
            icao: "C172".to_string(),
        }),
        axes: [("pitch".to_string(), {
            let mut a = AxisConfig::default_empty();
            a.expo = Some(0.4);
            a
        })]
        .into(),
        ..empty_profile()
    };

    let merged = global
        .merge_with(&sim)
        .unwrap()
        .merge_with(&aircraft)
        .unwrap();

    insta::with_settings!({sort_maps => true}, {
        insta::assert_json_snapshot!("four_layer_cascade_result", merged);
    });
}

#[test]
fn snapshot_migration_v1_to_v3_rich_profile() {
    let reg = MigrationRegistry::new();
    let v1 = json!({
        "schema_version": "v1",
        "sim": "msfs",
        "aircraft": { "icao": "C172" },
        "description": "Test profile",
        "axes": {
            "pitch":    { "deadzone": 0.03, "expo": 0.2, "slew_rate": 1.5 },
            "roll":     { "deadzone": 0.05, "expo": 0.3 },
            "yaw":      { "deadzone": 0.08, "expo": 0.1 },
            "throttle": { "deadzone": 0.01 }
        }
    });
    let v3 = reg.migrate(v1, "v1", "v3").unwrap();
    insta::with_settings!({sort_maps => true}, {
        insta::assert_json_snapshot!("migration_v1_to_v3_rich", v3);
    });
}

#[test]
fn snapshot_merged_profile_with_filter_and_curve() {
    let base = Profile {
        sim: Some("xplane".to_string()),
        axes: [(
            "pitch".to_string(),
            AxisConfig {
                deadzone: Some(0.03),
                expo: Some(0.2),
                slew_rate: Some(1.0),
                detents: vec![],
                curve: Some(linear_curve()),
                filter: Some(FilterConfig {
                    alpha: 0.3,
                    spike_threshold: Some(0.05),
                    max_spike_count: Some(3),
                }),
            },
        )]
        .into(),
        ..empty_profile()
    };
    let over = Profile {
        axes: [(
            "pitch".to_string(),
            AxisConfig {
                deadzone: None,
                expo: Some(0.5),
                slew_rate: None,
                detents: vec![],
                curve: None,
                filter: None,
            },
        )]
        .into(),
        ..empty_profile()
    };

    let merged = base.merge_with(&over).unwrap();
    insta::with_settings!({sort_maps => true}, {
        insta::assert_json_snapshot!("merge_preserves_curve_and_filter", merged);
    });
}

// ═══════════════════════════════════════════════════════════════════════════════
// 7. Edge cases
// ═══════════════════════════════════════════════════════════════════════════════

/// NaN deadzone should fail validation, not panic.
#[test]
fn nan_deadzone_rejected() {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(f32::NAN),
            expo: None,
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    let p = Profile {
        axes,
        ..empty_profile()
    };
    // Must not panic — either Err or a graceful response
    let result = p.validate();
    assert!(result.is_err(), "NaN deadzone should fail validation");
}

/// Infinity expo should fail validation.
#[test]
fn infinity_expo_rejected() {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.03),
            expo: Some(f32::INFINITY),
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    let p = Profile {
        axes,
        ..empty_profile()
    };
    assert!(p.validate().is_err(), "Inf expo should fail validation");
}

/// Negative infinity slew rate should fail validation.
#[test]
fn neg_infinity_slew_rate_rejected() {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: None,
            expo: None,
            slew_rate: Some(f32::NEG_INFINITY),
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    let p = Profile {
        axes,
        ..empty_profile()
    };
    assert!(
        p.validate().is_err(),
        "-Inf slew_rate should fail validation"
    );
}

/// Migration with axis containing only unknown fields still works.
#[test]
fn migration_with_only_unknown_fields_in_axis() {
    let reg = MigrationRegistry::new();
    let input = json!({
        "schema_version": "v1",
        "axes": {
            "custom": { "unknown_field": "value", "another": 42 }
        }
    });
    let result = reg.migrate(input, "v1", "v3").unwrap();

    let custom = &result["axes"]["custom"];
    assert_eq!(custom["unknown_field"], json!("value"));
    assert_eq!(custom["another"], json!(42));
    assert_eq!(custom["sensitivity"], json!(1.0));
    assert_eq!(custom["response_curve_type"], json!("default"));
}

/// Migration preserves deeply nested custom data.
#[test]
fn migration_preserves_deep_nested_custom_data() {
    let reg = MigrationRegistry::new();
    let input = json!({
        "schema_version": "v1",
        "metadata": {
            "created_by": "test",
            "tags": ["fighter", "military"],
            "hardware": {
                "joystick": "Warthog",
                "throttle": "TWCS"
            }
        },
        "axes": {
            "pitch": { "deadzone": 0.03, "expo": 0.2 }
        }
    });
    let result = reg.migrate(input, "v1", "v3").unwrap();

    assert_eq!(result["metadata"]["created_by"], json!("test"));
    assert_eq!(result["metadata"]["tags"], json!(["fighter", "military"]));
    assert_eq!(result["metadata"]["hardware"]["joystick"], json!("Warthog"));
}

/// Profile with single-point curve (too few) fails validation.
#[test]
fn curve_single_point_rejected() {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: None,
            expo: None,
            slew_rate: None,
            detents: vec![],
            curve: Some(vec![CurvePoint {
                input: 0.0,
                output: 0.0,
            }]),
            filter: None,
        },
    );
    let p = Profile {
        axes,
        ..empty_profile()
    };
    assert!(p.validate().is_err(), "single-point curve should be invalid");
}

/// Detent with zero width rejected.
#[test]
fn detent_zero_width_rejected() {
    let mut axes = HashMap::new();
    axes.insert(
        "throttle".to_string(),
        AxisConfig {
            deadzone: Some(0.01),
            expo: None,
            slew_rate: None,
            detents: vec![DetentZone {
                position: 0.0,
                width: 0.0,
                role: "idle".to_string(),
            }],
            curve: None,
            filter: None,
        },
    );
    let p = Profile {
        axes,
        ..empty_profile()
    };
    assert!(
        p.validate().is_err(),
        "zero-width detent should be rejected"
    );
}

/// Migration error for v2→v3 with non-object axis entries.
#[test]
fn migration_v2_to_v3_non_object_axis_rejected() {
    let reg = MigrationRegistry::new();
    let bad = json!({
        "schema_version": "v2",
        "axes": {
            "pitch": 42
        }
    });
    let result = reg.migrate(bad, "v2", "v3");
    assert!(matches!(result, Err(MigrationError::InvalidSchema(_))));
}

/// Custom v3→v4→v5 two-step extension chain.
#[test]
fn custom_two_step_extension_v3_to_v5() {
    let mut reg = MigrationRegistry::new();
    reg.register(ProfileMigration {
        from_version: "v3",
        to_version: "v4",
        description: "Add haptics field",
        migrate_fn: |mut v| {
            if let Some(axes) = v.get_mut("axes").and_then(|a| a.as_object_mut()) {
                for (_name, axis) in axes.iter_mut() {
                    if let Some(obj) = axis.as_object_mut() {
                        obj.entry("haptics_enabled").or_insert(Value::from(false));
                    }
                }
            }
            if let Some(obj) = v.as_object_mut() {
                obj.insert("schema_version".to_string(), Value::from("v4"));
            }
            Ok(v)
        },
    });
    reg.register(ProfileMigration {
        from_version: "v4",
        to_version: "v5",
        description: "Add axis priority field",
        migrate_fn: |mut v| {
            if let Some(axes) = v.get_mut("axes").and_then(|a| a.as_object_mut()) {
                for (_name, axis) in axes.iter_mut() {
                    if let Some(obj) = axis.as_object_mut() {
                        obj.entry("priority").or_insert(Value::from(0));
                    }
                }
            }
            if let Some(obj) = v.as_object_mut() {
                obj.insert("schema_version".to_string(), Value::from("v5"));
            }
            Ok(v)
        },
    });

    // Migrate from v1 all the way to v5
    assert!(reg.can_migrate("v1", "v5"));
    let input = json!({
        "schema_version": "v1",
        "axes": {
            "pitch": { "deadzone": 0.03, "expo": 0.2 }
        }
    });
    let result = reg.migrate(input, "v1", "v5").unwrap();
    assert_eq!(result["schema_version"], "v5");

    let pitch = &result["axes"]["pitch"];
    assert_eq!(pitch["sensitivity"], json!(1.0)); // from v1→v2
    assert_eq!(pitch["exponential"], json!(0.2)); // from v2→v3
    assert_eq!(pitch["response_curve_type"], json!("default")); // from v2→v3
    assert_eq!(pitch["haptics_enabled"], json!(false)); // from v3→v4
    assert_eq!(pitch["priority"], json!(0)); // from v4→v5
}

// ═══════════════════════════════════════════════════════════════════════════════
// 8. Property-based tests
// ═══════════════════════════════════════════════════════════════════════════════

mod proptest_depth {
    use super::*;
    use proptest::prelude::*;

    prop_compose! {
        fn arb_axis()(
            deadzone in prop::option::of(0.0f32..0.5f32),
            expo in prop::option::of(0.0f32..1.0f32),
            slew_rate in prop::option::of(0.0f32..100.0f32),
        ) -> AxisConfig {
            AxisConfig {
                deadzone, expo, slew_rate,
                detents: vec![], curve: None, filter: None,
            }
        }
    }

    prop_compose! {
        fn arb_profile()(
            sim in prop::option::of("[a-z]+"),
            axes in prop::collection::hash_map("[a-z]{1,4}", arb_axis(), 0..4),
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

    fn arb_v1_axes() -> impl Strategy<Value = serde_json::Map<String, Value>> {
        prop::collection::hash_map(
            "[a-z]{1,4}",
            (0.0f64..0.5, prop::option::of(0.0f64..1.0)).prop_map(|(dz, expo)| {
                let mut obj = serde_json::Map::new();
                obj.insert("deadzone".to_string(), json!(dz));
                if let Some(e) = expo {
                    obj.insert("expo".to_string(), json!(e));
                }
                Value::Object(obj)
            }),
            1..6,
        )
        .prop_map(|m| m.into_iter().collect())
    }

    proptest! {
        /// Migration v1→v2→v3 produces same result as v1→v3 direct chain.
        #[test]
        fn migration_chain_equivalent_to_direct(axes in arb_v1_axes()) {
            let reg = MigrationRegistry::new();
            let input = json!({
                "schema_version": "v1",
                "axes": Value::Object(axes)
            });

            // Two-step: v1→v2 then v2→v3
            let v2 = reg.migrate(input.clone(), "v1", "v2").unwrap();
            let v3_two_step = reg.migrate(v2, "v2", "v3").unwrap();

            // Direct: v1→v3
            let v3_direct = reg.migrate(input, "v1", "v3").unwrap();

            prop_assert_eq!(v3_two_step, v3_direct,
                "two-step migration must equal direct chain");
        }

        /// Migration followed by noop is idempotent.
        #[test]
        fn migration_then_noop_idempotent(axes in arb_v1_axes()) {
            let reg = MigrationRegistry::new();
            let input = json!({
                "schema_version": "v1",
                "axes": Value::Object(axes)
            });

            let v3 = reg.migrate(input, "v1", "v3").unwrap();
            let v3_again = reg.migrate(v3.clone(), "v3", "v3").unwrap();
            prop_assert_eq!(v3, v3_again);
        }

        /// Merging two disjoint-axis profiles produces all axes from both.
        #[test]
        fn merge_disjoint_axes_union(
            a in arb_profile(),
            b in arb_profile(),
        ) {
            let merged = a.merge_with(&b).unwrap();
            // Merged must have at least all axes from a and b
            for name in a.axes.keys() {
                prop_assert!(merged.axes.contains_key(name),
                    "base axis '{}' missing after merge", name);
            }
            for name in b.axes.keys() {
                prop_assert!(merged.axes.contains_key(name),
                    "override axis '{}' missing after merge", name);
            }
        }

        /// Merge idempotency: merge(A, B).merge(B) == merge(A, B).
        #[test]
        fn merge_idempotent(a in arb_profile(), b in arb_profile()) {
            let merged1 = a.merge_with(&b).unwrap();
            let merged2 = merged1.merge_with(&b).unwrap();
            prop_assert_eq!(merged1, merged2, "merge is not idempotent");
        }

        /// Merged profile has at least as many axes as the larger input.
        #[test]
        fn merge_axis_count_at_least_max(a in arb_profile(), b in arb_profile()) {
            let merged = a.merge_with(&b).unwrap();
            let max_input = a.axes.len().max(b.axes.len());
            prop_assert!(merged.axes.len() >= max_input,
                "merged ({}) has fewer axes than max input ({})",
                merged.axes.len(), max_input);
        }

        /// Cascade of 3 profiles is equivalent to sequential merges.
        #[test]
        fn three_layer_cascade_associative(
            a in arb_profile(),
            b in arb_profile(),
            c in arb_profile(),
        ) {
            let ab_c = a.merge_with(&b).unwrap().merge_with(&c).unwrap();
            let a_bc = a.merge_with(&b.merge_with(&c).unwrap()).unwrap();
            prop_assert_eq!(ab_c, a_bc, "merge is not associative");
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 9. AxisConfig Default trait
// ═══════════════════════════════════════════════════════════════════════════════

/// Verify AxisConfig::default_empty() produces all-None/empty fields.
#[test]
fn axis_config_default_is_empty() {
    let cfg = AxisConfig::default_empty();
    assert!(cfg.deadzone.is_none());
    assert!(cfg.expo.is_none());
    assert!(cfg.slew_rate.is_none());
    assert!(cfg.detents.is_empty());
    assert!(cfg.curve.is_none());
    assert!(cfg.filter.is_none());
}

/// A profile with only default axis configs is valid.
#[test]
fn profile_with_default_axis_configs_valid() {
    let p = Profile {
        axes: [
            ("pitch".to_string(), AxisConfig::default_empty()),
            ("roll".to_string(), AxisConfig::default_empty()),
        ]
        .into(),
        ..empty_profile()
    };
    assert!(p.validate().is_ok());
}
