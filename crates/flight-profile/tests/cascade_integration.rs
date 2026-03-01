// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests for the full profile cascade pipeline.
//!
//! Validates the Global → Simulator → Aircraft → Phase-of-Flight merge chain
//! including conflict resolution, hash stability, and schema version checks.

use flight_profile::{
    AircraftId, AxisConfig, CurvePoint, DetentZone, FilterConfig, PROFILE_SCHEMA_VERSION,
    PofOverrides, Profile,
};
use std::collections::HashMap;

// ── Profile factory helpers ─────────────────────────────────────────────────

/// Global profile: base deadzones and linear curves for all axes.
fn global_profile() -> Profile {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.05),
            expo: Some(0.0),
            slew_rate: Some(2.0),
            detents: vec![],
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
            filter: None,
        },
    );
    axes.insert(
        "roll".to_string(),
        AxisConfig {
            deadzone: Some(0.05),
            expo: Some(0.0),
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
            slew_rate: Some(1.5),
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    axes.insert(
        "throttle".to_string(),
        AxisConfig {
            deadzone: Some(0.02),
            expo: None,
            slew_rate: Some(50.0),
            detents: vec![DetentZone {
                position: -1.0,
                width: 0.05,
                role: "cutoff".to_string(),
            }],
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

/// MSFS simulator overlay: different trim/slew behaviour.
fn msfs_sim_profile() -> Profile {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: None,
            expo: Some(0.15),
            slew_rate: Some(3.0),
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    axes.insert(
        "roll".to_string(),
        AxisConfig {
            deadzone: None,
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
        aircraft: None,
        axes,
        pof_overrides: None,
    }
}

/// F/A-18C aircraft overlay: tighter deadzones and S-curve response.
fn fa18c_aircraft_profile() -> Profile {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.03),
            expo: Some(0.3),
            slew_rate: None,
            detents: vec![],
            curve: Some(vec![
                CurvePoint {
                    input: 0.0,
                    output: 0.0,
                },
                CurvePoint {
                    input: 0.25,
                    output: 0.1,
                },
                CurvePoint {
                    input: 0.5,
                    output: 0.4,
                },
                CurvePoint {
                    input: 0.75,
                    output: 0.8,
                },
                CurvePoint {
                    input: 1.0,
                    output: 1.0,
                },
            ]),
            filter: None,
        },
    );
    axes.insert(
        "roll".to_string(),
        AxisConfig {
            deadzone: Some(0.02),
            expo: None,
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
            icao: "F18".to_string(),
        }),
        axes,
        pof_overrides: None,
    }
}

/// Takeoff phase-of-flight overlay: reduced sensitivity for ground handling.
fn takeoff_pof_profile() -> Profile {
    let mut pof_axes = HashMap::new();
    pof_axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: None,
            expo: Some(0.15),
            slew_rate: Some(1.5),
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    pof_axes.insert(
        "yaw".to_string(),
        AxisConfig {
            deadzone: Some(0.12),
            expo: Some(0.2),
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        },
    );

    let mut pof_overrides = HashMap::new();
    pof_overrides.insert(
        "takeoff".to_string(),
        PofOverrides {
            axes: Some(pof_axes),
            hysteresis: None,
        },
    );

    Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes: HashMap::new(),
        pof_overrides: Some(pof_overrides),
    }
}

/// Cruise phase-of-flight overlay.
fn cruise_pof_profile() -> Profile {
    let mut pof_axes = HashMap::new();
    pof_axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: None,
            expo: Some(0.4),
            slew_rate: Some(0.8),
            detents: vec![],
            curve: None,
            filter: None,
        },
    );

    let mut pof_overrides = HashMap::new();
    pof_overrides.insert(
        "cruise".to_string(),
        PofOverrides {
            axes: Some(pof_axes),
            hysteresis: None,
        },
    );

    Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes: HashMap::new(),
        pof_overrides: Some(pof_overrides),
    }
}

/// Empty overlay with matching schema but no axes or overrides.
fn empty_overlay() -> Profile {
    Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes: HashMap::new(),
        pof_overrides: None,
    }
}

// ── Test: Full Cascade ──────────────────────────────────────────────────────

#[test]
fn full_cascade_global_sim_aircraft_pof() {
    let global = global_profile();
    let sim = msfs_sim_profile();
    let aircraft = fa18c_aircraft_profile();
    let pof = takeoff_pof_profile();

    let merged = global
        .merge_with(&sim)
        .unwrap()
        .merge_with(&aircraft)
        .unwrap()
        .merge_with(&pof)
        .unwrap();

    // Pitch: aircraft deadzone 0.03 wins over global 0.05
    let pitch = merged.axes.get("pitch").unwrap();
    assert_eq!(pitch.deadzone, Some(0.03));
    // Pitch: aircraft expo 0.3 wins over sim 0.15 (aircraft is more specific)
    assert_eq!(pitch.expo, Some(0.3));
    // Pitch: sim slew_rate 3.0 wins over global 2.0 (aircraft didn't set it)
    assert_eq!(pitch.slew_rate, Some(3.0));
    // Pitch: aircraft S-curve wins over global linear
    assert_eq!(pitch.curve.as_ref().unwrap().len(), 5);

    // Roll: aircraft deadzone 0.02 wins over global 0.05
    let roll = merged.axes.get("roll").unwrap();
    assert_eq!(roll.deadzone, Some(0.02));
    // Roll: sim expo 0.1 preserved (aircraft didn't set expo)
    assert_eq!(roll.expo, Some(0.1));
    // Roll: global slew_rate 2.0 preserved through chain
    assert_eq!(roll.slew_rate, Some(2.0));

    // Yaw: untouched by sim or aircraft → global values preserved
    let yaw = merged.axes.get("yaw").unwrap();
    assert_eq!(yaw.deadzone, Some(0.08));
    assert_eq!(yaw.expo, Some(0.1));
    assert_eq!(yaw.slew_rate, Some(1.5));

    // Throttle: untouched → global detent preserved
    let throttle = merged.axes.get("throttle").unwrap();
    assert_eq!(throttle.detents.len(), 1);
    assert_eq!(throttle.detents[0].role, "cutoff");

    // PoF overrides from takeoff_pof are present
    let pof_map = merged.pof_overrides.as_ref().unwrap();
    assert!(pof_map.contains_key("takeoff"));
    let takeoff = pof_map.get("takeoff").unwrap();
    let takeoff_pitch = takeoff.axes.as_ref().unwrap().get("pitch").unwrap();
    assert_eq!(takeoff_pitch.expo, Some(0.15));
    assert_eq!(takeoff_pitch.slew_rate, Some(1.5));

    // Sim and aircraft metadata propagated
    assert_eq!(merged.sim, Some("msfs".to_string()));
    assert_eq!(
        merged.aircraft,
        Some(AircraftId {
            icao: "F18".to_string()
        })
    );

    // Merged profile should pass validation
    merged.validate().unwrap();
}

// ── Test: Partial Cascade ───────────────────────────────────────────────────

#[test]
fn partial_cascade_global_then_aircraft_no_sim() {
    let global = global_profile();
    let aircraft = fa18c_aircraft_profile();

    let merged = global.merge_with(&aircraft).unwrap();

    // Pitch: aircraft deadzone 0.03 wins
    let pitch = merged.axes.get("pitch").unwrap();
    assert_eq!(pitch.deadzone, Some(0.03));
    // Pitch: aircraft expo 0.3 wins over global 0.0
    assert_eq!(pitch.expo, Some(0.3));
    // Pitch: global slew_rate preserved (aircraft didn't set it)
    assert_eq!(pitch.slew_rate, Some(2.0));

    // Roll: aircraft deadzone 0.02 wins
    let roll = merged.axes.get("roll").unwrap();
    assert_eq!(roll.deadzone, Some(0.02));
    // Roll: global expo preserved (aircraft didn't set it for roll)
    assert_eq!(roll.expo, Some(0.0));

    // Yaw: global preserved (aircraft doesn't touch yaw)
    assert!(merged.axes.contains_key("yaw"));

    merged.validate().unwrap();
}

// ── Test: Conflict Resolution ───────────────────────────────────────────────

#[test]
fn conflict_resolution_more_specific_wins() {
    let mut global_axes = HashMap::new();
    global_axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.05),
            expo: Some(0.0),
            slew_rate: Some(2.0),
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    let global = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes: global_axes,
        pof_overrides: None,
    };

    let mut sim_axes = HashMap::new();
    sim_axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.04),
            expo: Some(0.2),
            slew_rate: Some(3.0),
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    let sim = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: Some("msfs".to_string()),
        aircraft: None,
        axes: sim_axes,
        pof_overrides: None,
    };

    let mut aircraft_axes = HashMap::new();
    aircraft_axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.02),
            expo: Some(0.5),
            slew_rate: Some(4.0),
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    let aircraft = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: Some("msfs".to_string()),
        aircraft: Some(AircraftId {
            icao: "F18".to_string(),
        }),
        axes: aircraft_axes,
        pof_overrides: None,
    };

    let merged = global
        .merge_with(&sim)
        .unwrap()
        .merge_with(&aircraft)
        .unwrap();
    let pitch = merged.axes.get("pitch").unwrap();

    // Aircraft (most specific) wins for all fields
    assert_eq!(pitch.deadzone, Some(0.02));
    assert_eq!(pitch.expo, Some(0.5));
    assert_eq!(pitch.slew_rate, Some(4.0));
}

// ── Test: Empty Overlay Preserves Parent ────────────────────────────────────

#[test]
fn empty_overlay_preserves_parent_values() {
    let global = global_profile();
    let empty = empty_overlay();

    let merged = global.merge_with(&empty).unwrap();

    // All global axes should be preserved exactly
    assert_eq!(merged.axes.len(), global.axes.len());
    for (name, config) in &global.axes {
        let merged_config = merged.axes.get(name).unwrap();
        assert_eq!(merged_config.deadzone, config.deadzone, "axis {name}");
        assert_eq!(merged_config.expo, config.expo, "axis {name}");
        assert_eq!(merged_config.slew_rate, config.slew_rate, "axis {name}");
    }

    // Metadata unchanged
    assert_eq!(merged.sim, global.sim);
    assert_eq!(merged.aircraft, global.aircraft);

    merged.validate().unwrap();
}

// ── Test: Deadzone Cascade ──────────────────────────────────────────────────

#[test]
fn deadzone_cascade_aircraft_overrides_global() {
    let global = global_profile(); // pitch deadzone 0.05

    let mut aircraft_axes = HashMap::new();
    aircraft_axes.insert(
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
    let aircraft = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: Some(AircraftId {
            icao: "C172".to_string(),
        }),
        axes: aircraft_axes,
        pof_overrides: None,
    };

    let merged = global.merge_with(&aircraft).unwrap();
    let pitch = merged.axes.get("pitch").unwrap();

    assert_eq!(pitch.deadzone, Some(0.03), "aircraft deadzone should win");
    // Global expo preserved since aircraft didn't set it
    assert_eq!(pitch.expo, Some(0.0));
}

// ── Test: Curve Cascade ─────────────────────────────────────────────────────

#[test]
fn curve_cascade_aircraft_scurve_overrides_global_linear() {
    let global = global_profile(); // linear curve on pitch

    let s_curve = vec![
        CurvePoint {
            input: 0.0,
            output: 0.0,
        },
        CurvePoint {
            input: 0.25,
            output: 0.1,
        },
        CurvePoint {
            input: 0.5,
            output: 0.4,
        },
        CurvePoint {
            input: 0.75,
            output: 0.8,
        },
        CurvePoint {
            input: 1.0,
            output: 1.0,
        },
    ];

    let mut aircraft_axes = HashMap::new();
    aircraft_axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: None,
            expo: None,
            slew_rate: None,
            detents: vec![],
            curve: Some(s_curve.clone()),
            filter: None,
        },
    );
    let aircraft = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: Some(AircraftId {
            icao: "F18".to_string(),
        }),
        axes: aircraft_axes,
        pof_overrides: None,
    };

    let merged = global.merge_with(&aircraft).unwrap();
    let pitch = merged.axes.get("pitch").unwrap();

    // Aircraft S-curve replaces global linear
    let curve = pitch.curve.as_ref().unwrap();
    assert_eq!(curve.len(), 5, "S-curve should have 5 points");
    assert_eq!(
        curve[2].output, 0.4,
        "midpoint output should be S-curve value"
    );

    // Global deadzone preserved
    assert_eq!(pitch.deadzone, Some(0.05));

    merged.validate().unwrap();
}

// ── Test: Button Mapping Cascade (via detents as proxy) ─────────────────────

#[test]
fn detent_mapping_cascade_aircraft_overrides_global() {
    let global = global_profile(); // throttle has cutoff detent

    let mut aircraft_axes = HashMap::new();
    aircraft_axes.insert(
        "throttle".to_string(),
        AxisConfig {
            deadzone: None,
            expo: None,
            slew_rate: None,
            detents: vec![
                DetentZone {
                    position: -1.0,
                    width: 0.05,
                    role: "reverse".to_string(),
                },
                DetentZone {
                    position: 0.0,
                    width: 0.03,
                    role: "idle".to_string(),
                },
            ],
            curve: None,
            filter: None,
        },
    );
    let aircraft = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: Some(AircraftId {
            icao: "A320".to_string(),
        }),
        axes: aircraft_axes,
        pof_overrides: None,
    };

    let merged = global.merge_with(&aircraft).unwrap();
    let throttle = merged.axes.get("throttle").unwrap();

    // Aircraft detents replace global detents entirely
    assert_eq!(throttle.detents.len(), 2);
    assert_eq!(throttle.detents[0].role, "reverse");
    assert_eq!(throttle.detents[1].role, "idle");

    // Global deadzone preserved since aircraft didn't set it
    assert_eq!(throttle.deadzone, Some(0.02));

    merged.validate().unwrap();
}

// ── Test: Schema Version Mismatch ───────────────────────────────────────────

#[test]
fn schema_version_mismatch_in_cascade() {
    let global = global_profile();

    let bad_schema = Profile {
        schema: "flight.profile/999".to_string(),
        sim: None,
        aircraft: None,
        axes: HashMap::new(),
        pof_overrides: None,
    };

    // merge_with itself doesn't check schema — but validation should catch it
    let merged = global.merge_with(&bad_schema).unwrap();
    // The merged profile inherits global's schema (merge doesn't override schema)
    // But the bad_schema profile itself should fail validation
    assert!(bad_schema.validate().is_err());
    // The merged result keeps global's schema so should be valid
    merged.validate().unwrap();
}

#[test]
fn invalid_schema_fails_validation_standalone() {
    let profile = Profile {
        schema: "flight.profile/2".to_string(),
        sim: None,
        aircraft: None,
        axes: HashMap::new(),
        pof_overrides: None,
    };
    let result = profile.validate();
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("schema version"),
        "error should mention schema version: {err_msg}"
    );
}

// ── Test: Hash Stability ────────────────────────────────────────────────────

#[test]
fn hash_stability_same_inputs_same_hash() {
    let global = global_profile();
    let sim = msfs_sim_profile();
    let aircraft = fa18c_aircraft_profile();

    let merged1 = global
        .merge_with(&sim)
        .unwrap()
        .merge_with(&aircraft)
        .unwrap();

    let merged2 = global
        .merge_with(&sim)
        .unwrap()
        .merge_with(&aircraft)
        .unwrap();

    assert_eq!(
        merged1.effective_hash(),
        merged2.effective_hash(),
        "identical cascade inputs must produce identical hashes"
    );
}

#[test]
fn hash_changes_when_cascade_differs() {
    let global = global_profile();
    let sim = msfs_sim_profile();
    let aircraft = fa18c_aircraft_profile();

    let with_aircraft = global
        .merge_with(&sim)
        .unwrap()
        .merge_with(&aircraft)
        .unwrap();
    let without_aircraft = global.merge_with(&sim).unwrap();

    assert_ne!(
        with_aircraft.effective_hash(),
        without_aircraft.effective_hash(),
        "different cascade depths should produce different hashes"
    );
}

#[test]
fn hash_stable_across_serialization_roundtrip() {
    let global = global_profile();
    let sim = msfs_sim_profile();
    let merged = global.merge_with(&sim).unwrap();

    let hash_before = merged.effective_hash();
    let json = serde_json::to_string(&merged).unwrap();
    let restored: Profile = serde_json::from_str(&json).unwrap();
    let hash_after = restored.effective_hash();

    assert_eq!(
        hash_before, hash_after,
        "hash must survive serialization round-trip"
    );
}

// ── Test: Merge Ordering / Associativity ────────────────────────────────────

#[test]
fn merge_ordering_left_associative() {
    let global = global_profile();
    let sim = msfs_sim_profile();
    let aircraft = fa18c_aircraft_profile();

    // Left-associative: (Global.merge(Sim)).merge(Aircraft)
    let left = global
        .merge_with(&sim)
        .unwrap()
        .merge_with(&aircraft)
        .unwrap();

    // Right-associative: Global.merge(Sim.merge(Aircraft))
    let right = global
        .merge_with(&sim.merge_with(&aircraft).unwrap())
        .unwrap();

    // With all-Some fields, merge_with is last-writer-wins so both should equal
    // because aircraft values dominate in both orderings for the same axis
    let left_pitch = left.axes.get("pitch").unwrap();
    let right_pitch = right.axes.get("pitch").unwrap();

    assert_eq!(left_pitch.deadzone, right_pitch.deadzone);
    assert_eq!(left_pitch.expo, right_pitch.expo);
    assert_eq!(left_pitch.curve, right_pitch.curve);
}

// ── Test: Filter Cascade ────────────────────────────────────────────────────

#[test]
fn filter_cascade_aircraft_overrides_global() {
    let mut global_axes = HashMap::new();
    global_axes.insert(
        "throttle".to_string(),
        AxisConfig {
            deadzone: Some(0.02),
            expo: None,
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: Some(FilterConfig {
                alpha: 0.3,
                spike_threshold: Some(0.1),
                max_spike_count: Some(3),
            }),
        },
    );
    let global = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes: global_axes,
        pof_overrides: None,
    };

    let mut aircraft_axes = HashMap::new();
    aircraft_axes.insert(
        "throttle".to_string(),
        AxisConfig {
            deadzone: None,
            expo: None,
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: Some(FilterConfig {
                alpha: 0.5,
                spike_threshold: Some(0.2),
                max_spike_count: Some(5),
            }),
        },
    );
    let aircraft = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: Some(AircraftId {
            icao: "C172".to_string(),
        }),
        axes: aircraft_axes,
        pof_overrides: None,
    };

    let merged = global.merge_with(&aircraft).unwrap();
    let throttle = merged.axes.get("throttle").unwrap();
    let filter = throttle.filter.as_ref().unwrap();

    assert_eq!(filter.alpha, 0.5, "aircraft filter alpha should win");
    assert_eq!(filter.spike_threshold, Some(0.2));
    assert_eq!(filter.max_spike_count, Some(5));
    // Global deadzone preserved
    assert_eq!(throttle.deadzone, Some(0.02));

    merged.validate().unwrap();
}

// ── Test: PoF Merge Across Multiple Phases ──────────────────────────────────

#[test]
fn pof_merge_collects_all_phases() {
    let global = global_profile();
    let takeoff = takeoff_pof_profile();
    let cruise = cruise_pof_profile();

    let merged = global
        .merge_with(&takeoff)
        .unwrap()
        .merge_with(&cruise)
        .unwrap();
    let pof_map = merged.pof_overrides.as_ref().unwrap();

    assert!(pof_map.contains_key("takeoff"), "takeoff phase present");
    assert!(pof_map.contains_key("cruise"), "cruise phase present");
    assert_eq!(pof_map.len(), 2);
}

// ── Test: New Axis Introduced by Overlay ────────────────────────────────────

#[test]
fn overlay_introduces_new_axis() {
    let global = global_profile(); // has pitch, roll, yaw, throttle

    let mut sim_axes = HashMap::new();
    sim_axes.insert(
        "brake_left".to_string(),
        AxisConfig {
            deadzone: Some(0.01),
            expo: None,
            slew_rate: Some(10.0),
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    let sim = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: Some("dcs".to_string()),
        aircraft: None,
        axes: sim_axes,
        pof_overrides: None,
    };

    let merged = global.merge_with(&sim).unwrap();

    // Global axes preserved
    assert!(merged.axes.contains_key("pitch"));
    assert!(merged.axes.contains_key("roll"));
    assert!(merged.axes.contains_key("yaw"));
    assert!(merged.axes.contains_key("throttle"));
    // New axis introduced by sim
    assert!(merged.axes.contains_key("brake_left"));
    assert_eq!(merged.axes.get("brake_left").unwrap().deadzone, Some(0.01));
    assert_eq!(merged.axes.len(), 5);
}

// ── Test: Deep Four-Level Cascade Validates ─────────────────────────────────

#[test]
fn four_level_cascade_produces_valid_profile() {
    let merged = global_profile()
        .merge_with(&msfs_sim_profile())
        .unwrap()
        .merge_with(&fa18c_aircraft_profile())
        .unwrap()
        .merge_with(&takeoff_pof_profile())
        .unwrap();

    merged
        .validate()
        .expect("four-level cascade should produce a valid profile");
}

// ── Snapshot: Canonical F/A-18C Cascade ─────────────────────────────────────

#[test]
fn snapshot_fa18c_cascade_merged() {
    let merged = global_profile()
        .merge_with(&msfs_sim_profile())
        .unwrap()
        .merge_with(&fa18c_aircraft_profile())
        .unwrap()
        .merge_with(&takeoff_pof_profile())
        .unwrap();

    insta::with_settings!({sort_maps => true}, {
        insta::assert_json_snapshot!("fa18c_full_cascade", &merged);
    });
}

#[test]
fn snapshot_fa18c_partial_cascade_no_pof() {
    let merged = global_profile()
        .merge_with(&msfs_sim_profile())
        .unwrap()
        .merge_with(&fa18c_aircraft_profile())
        .unwrap();

    insta::with_settings!({sort_maps => true}, {
        insta::assert_json_snapshot!("fa18c_cascade_no_pof", &merged);
    });
}

#[test]
fn snapshot_global_only() {
    let global = global_profile();
    insta::with_settings!({sort_maps => true}, {
        insta::assert_json_snapshot!("global_base_profile", &global);
    });
}
