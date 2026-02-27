// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Additional tests for `flight-profile` covering gaps not addressed by the
//! existing test suite:
//!
//! - Schema-version gate (migration boundary)
//! - Negative expo rejection
//! - Empty-profile validation
//! - Full-fields-present validation
//! - Phase-of-Flight cascade accessibility
//! - Disjoint-axis merge completeness

use flight_profile::{
    AircraftId, AxisConfig, CapabilityContext, CapabilityMode, CurvePoint, DetentZone,
    FilterConfig, PROFILE_SCHEMA_VERSION, PofOverrides, Profile,
};
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

// ── 1. Schema-version gate (migration boundary) ──────────────────────────────

/// The current schema string `PROFILE_SCHEMA_VERSION` must be accepted by
/// `validate()`.  This is the complement of the "old schema is rejected" test
/// already in `profile_validation_tests.rs` and together they define the
/// migration boundary.
#[test]
fn current_schema_version_is_accepted() {
    let profile = empty_profile();
    assert!(
        profile.validate().is_ok(),
        "current schema version must pass validation"
    );
}

/// A schema version string that looks like a plausible future version must also
/// be rejected, ensuring the gate is a strict equality check not a prefix check.
#[test]
fn future_schema_version_is_rejected() {
    let profile = Profile {
        schema: "flight.profile/2".to_string(),
        ..empty_profile()
    };
    let err = profile.validate().unwrap_err();
    assert!(
        err.to_string().contains("Unsupported schema version"),
        "future schema version must be rejected, got: {err}"
    );
}

// ── 2. Negative expo is rejected ─────────────────────────────────────────────

/// `expo < 0.0` must be rejected even in Full mode.
/// The existing suite covers `expo > MAX_EXPO`; this test closes the lower-
/// bound gap.
#[test]
fn negative_expo_is_rejected_in_full_mode() {
    let mut axes = HashMap::new();
    axes.insert("pitch".to_string(), axis(0.03, -0.1));
    let profile = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes,
        pof_overrides: None,
    };
    let err = profile.validate().unwrap_err();
    assert!(
        err.to_string().contains("expo"),
        "validation error must mention the expo field, got: {err}"
    );
}

/// `expo = 0.0` is the boundary and must be accepted.
#[test]
fn zero_expo_is_valid() {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            expo: Some(0.0),
            ..axis(0.03, 0.0)
        },
    );
    let profile = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes,
        pof_overrides: None,
    };
    assert!(
        profile.validate().is_ok(),
        "expo = 0.0 is at the lower boundary and must be valid"
    );
}

// ── 3 & 10. Empty profile is valid / all-fields profile is valid ─────────────

/// An empty profile (no axes, no PoF overrides) with the correct schema
/// version must pass `validate()`.
#[test]
fn empty_profile_is_valid() {
    assert!(
        empty_profile().validate().is_ok(),
        "an empty profile must pass validation"
    );
}

/// A profile with every optional field populated must also pass `validate()`.
/// This exercises all validation code-paths simultaneously.
#[test]
fn profile_with_all_optional_fields_is_valid() {
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

    let mut axes = HashMap::new();
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
                CurvePoint {
                    input: 0.0,
                    output: 0.0,
                },
                CurvePoint {
                    input: 0.5,
                    output: 0.35,
                },
                CurvePoint {
                    input: 1.0,
                    output: 1.0,
                },
            ]),
            filter: Some(FilterConfig {
                alpha: 0.3,
                spike_threshold: Some(0.05),
                max_spike_count: Some(3),
            }),
        },
    );
    axes.insert(
        "roll".to_string(),
        AxisConfig {
            deadzone: Some(0.04),
            expo: Some(0.15),
            slew_rate: Some(0.8),
            detents: vec![],
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
        pof_overrides: Some(pof_overrides),
    };

    assert!(
        profile.validate().is_ok(),
        "a fully-populated profile must pass validation"
    );
}

/// Same profile must also pass the Kid capability context, provided the values
/// are within Kid limits.
#[test]
fn full_profile_with_kid_safe_values_passes_kid_mode() {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.02),
            expo: Some(0.2),       // ≤ Kid max_expo (0.3)
            slew_rate: Some(10.0), // ≤ Kid max_slew_rate (20.0)
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
    assert!(
        profile.validate_with_capabilities(&kid_ctx).is_ok(),
        "profile within Kid limits must pass Kid validation"
    );
}

// ── 6. Phase-of-flight cascade: approach overrides cruise ───────────────────

/// When a profile has both base-axis settings and PoF overrides for "approach",
/// the PoF axes must be accessible and carry the expected override values —
/// simulating how a consuming system would apply the approach-phase cascade
/// over the cruise defaults.
#[test]
fn pof_approach_overrides_are_accessible_and_correct() {
    let cruise_deadzone = 0.03_f32;
    let approach_deadzone = 0.07_f32;

    // Base axes represent "cruise" settings
    let mut base_axes = HashMap::new();
    base_axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(cruise_deadzone),
            expo: Some(0.2),
            slew_rate: Some(1.0),
            detents: vec![],
            curve: None,
            filter: None,
        },
    );

    // PoF override for the "approach" phase tightens the deadzone
    let mut approach_axes = HashMap::new();
    approach_axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(approach_deadzone),
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
            axes: Some(approach_axes),
            hysteresis: None,
        },
    );

    let profile = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: Some("msfs".to_string()),
        aircraft: None,
        axes: base_axes,
        pof_overrides: Some(pof_overrides),
    };

    assert!(
        profile.validate().is_ok(),
        "profile with PoF overrides must be valid"
    );

    // The PoF override for "approach" must be accessible and hold the right value
    let pof = profile
        .pof_overrides
        .as_ref()
        .expect("pof_overrides must be Some");
    let approach = pof.get("approach").expect("'approach' phase must exist");
    let approach_pitch = approach
        .axes
        .as_ref()
        .expect("approach axes must be Some")
        .get("pitch")
        .expect("pitch axis must be present in approach override");

    assert_eq!(
        approach_pitch.deadzone,
        Some(approach_deadzone),
        "approach deadzone must differ from cruise deadzone"
    );
    assert_ne!(
        approach_pitch.deadzone,
        Some(cruise_deadzone),
        "approach override must override the base (cruise) deadzone"
    );
}

/// Applying the "approach" PoF override via `merge_with` to a base profile
/// produces an overriding axis config with the approach values.
#[test]
fn pof_approach_merged_into_base_produces_expected_values() {
    let mut base_axes = HashMap::new();
    base_axes.insert("pitch".to_string(), axis(0.03, 0.2));

    let base = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes: base_axes,
        pof_overrides: None,
    };

    // Override profile carries the approach PoF data
    let mut approach_axes = HashMap::new();
    approach_axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.07),
            expo: Some(0.4),
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    let mut pof = HashMap::new();
    pof.insert(
        "approach".to_string(),
        PofOverrides {
            axes: Some(approach_axes),
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

    let merged = base.merge_with(&override_profile).unwrap();

    let pof_map = merged
        .pof_overrides
        .as_ref()
        .expect("merged profile must have pof_overrides");
    assert!(
        pof_map.contains_key("approach"),
        "merged profile must contain the approach phase"
    );

    let approach_pitch = pof_map["approach"]
        .axes
        .as_ref()
        .unwrap()
        .get("pitch")
        .unwrap();
    assert_eq!(
        approach_pitch.deadzone,
        Some(0.07_f32),
        "approach deadzone from override must be present after merge"
    );
}

// ── 7. Merge: two profiles with disjoint axes merge cleanly ─────────────────

/// When the base and override profiles have completely different axis names,
/// `merge_with` must produce a profile containing **all** axes from both.
#[test]
fn merge_disjoint_axes_result_contains_all_axes() {
    let mut base_axes = HashMap::new();
    base_axes.insert("pitch".to_string(), axis(0.03, 0.2));
    base_axes.insert("roll".to_string(), axis(0.04, 0.15));

    let base = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: Some("msfs".to_string()),
        aircraft: None,
        axes: base_axes,
        pof_overrides: None,
    };

    let mut override_axes = HashMap::new();
    override_axes.insert("rudder".to_string(), axis(0.05, 0.1));
    override_axes.insert("throttle".to_string(), axis(0.0, 0.0));

    let override_profile = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes: override_axes,
        pof_overrides: None,
    };

    let merged = base.merge_with(&override_profile).unwrap();

    assert_eq!(
        merged.axes.len(),
        4,
        "merged profile must contain all 4 axes (pitch, roll, rudder, throttle)"
    );
    assert!(merged.axes.contains_key("pitch"), "pitch must be present");
    assert!(merged.axes.contains_key("roll"), "roll must be present");
    assert!(merged.axes.contains_key("rudder"), "rudder must be present");
    assert!(
        merged.axes.contains_key("throttle"),
        "throttle must be present"
    );
}

/// Base-only axes must preserve their original values unchanged after the merge.
#[test]
fn merge_disjoint_axes_base_values_unchanged() {
    let mut base_axes = HashMap::new();
    base_axes.insert("pitch".to_string(), axis(0.03, 0.2));

    let base = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes: base_axes,
        pof_overrides: None,
    };

    let mut override_axes = HashMap::new();
    override_axes.insert("rudder".to_string(), axis(0.05, 0.1));

    let override_profile = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes: override_axes,
        pof_overrides: None,
    };

    let merged = base.merge_with(&override_profile).unwrap();

    let pitch = merged
        .axes
        .get("pitch")
        .expect("pitch must survive the merge");
    assert_eq!(
        pitch.deadzone,
        Some(0.03_f32),
        "base deadzone must be unchanged"
    );
    assert_eq!(pitch.expo, Some(0.2_f32), "base expo must be unchanged");
}
