// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Property-based tests for flight-profile invariants.
//!
//! Tests verify:
//! - Profile cascade order matters (more-specific wins)
//! - Validated profile always passes re-validation
//! - Profile diff symmetry
//! - Template-generated profiles pass validation

use flight_profile::{
    AircraftId, AxisConfig, PROFILE_SCHEMA_VERSION, Profile,
    profile_compare::{compare_profiles, flatten_profile},
    templates::Template,
};
use proptest::prelude::*;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn simple_axis(deadzone: Option<f32>, expo: Option<f32>) -> AxisConfig {
    AxisConfig {
        deadzone,
        expo,
        slew_rate: None,
        detents: vec![],
        curve: None,
        filter: None,
    }
}

// ── Strategies ───────────────────────────────────────────────────────────────

prop_compose! {
    fn arb_axis_config()(
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
        axes in prop::collection::hash_map("[a-z]{1,4}", arb_axis_config(), 0..3),
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
    // ── Cascade order ───────────────────────────────────────────────────────

    /// Property 1: More-specific profile override wins over less-specific.
    /// When base has expo=A and override has expo=B, merged should have expo=B.
    #[test]
    fn cascade_override_wins(
        base_expo in 0.0f32..0.5,
        override_expo in 0.5f32..1.0,
    ) {
        let base = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: Some("msfs".to_string()),
            aircraft: None,
            axes: [("pitch".to_string(), simple_axis(Some(0.03), Some(base_expo)))].into(),
            pof_overrides: None,
        };
        let specific = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: Some("msfs".to_string()),
            aircraft: Some(AircraftId { icao: "C172".to_string() }),
            axes: [("pitch".to_string(), simple_axis(None, Some(override_expo)))].into(),
            pof_overrides: None,
        };
        let merged = base.merge_with(&specific).unwrap();
        let pitch = merged.axes.get("pitch").expect("pitch axis should exist");
        prop_assert_eq!(
            pitch.expo, Some(override_expo),
            "override expo {} should win over base expo {}", override_expo, base_expo
        );
        // Base deadzone should be preserved since override has None
        prop_assert_eq!(pitch.deadzone, Some(0.03));
    }

    /// Property 2: Cascade preserves base values when override is None.
    #[test]
    fn cascade_preserves_base_when_no_override(
        base_dz in 0.0f32..0.5,
        base_expo in 0.0f32..1.0,
        base_slew in 0.0f32..100.0,
    ) {
        let base = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None,
            aircraft: None,
            axes: [("pitch".to_string(), AxisConfig {
                deadzone: Some(base_dz),
                expo: Some(base_expo),
                slew_rate: Some(base_slew),
                detents: vec![],
                curve: None,
                filter: None,
            })].into(),
            pof_overrides: None,
        };
        let empty_override = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None,
            aircraft: None,
            axes: [("pitch".to_string(), simple_axis(None, None))].into(),
            pof_overrides: None,
        };
        let merged = base.merge_with(&empty_override).unwrap();
        let pitch = merged.axes.get("pitch").unwrap();
        prop_assert_eq!(pitch.deadzone, Some(base_dz));
        prop_assert_eq!(pitch.expo, Some(base_expo));
        prop_assert_eq!(pitch.slew_rate, Some(base_slew));
    }

    // ── Validated profile re-validation ─────────────────────────────────────

    /// Property 3: A profile that passes validation always passes re-validation.
    #[test]
    fn validated_profile_passes_revalidation(
        dz in 0.0f32..0.5,
        expo in 0.0f32..1.0,
    ) {
        let profile = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None,
            aircraft: None,
            axes: [("test".to_string(), simple_axis(Some(dz), Some(expo)))].into(),
            pof_overrides: None,
        };
        let v1 = profile.validate();
        prop_assert!(v1.is_ok(), "first validation failed: {:?}", v1.err());
        let v2 = profile.validate();
        prop_assert!(v2.is_ok(), "re-validation failed: {:?}", v2.err());
    }

    /// Property 4: Validation is consistent after serialization round-trip.
    #[test]
    fn validation_stable_after_roundtrip(
        dz in 0.0f32..0.5,
        expo in 0.0f32..1.0,
        slew in 0.0f32..100.0,
    ) {
        let profile = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None,
            aircraft: None,
            axes: [("test".to_string(), AxisConfig {
                deadzone: Some(dz),
                expo: Some(expo),
                slew_rate: Some(slew),
                detents: vec![],
                curve: None,
                filter: None,
            })].into(),
            pof_overrides: None,
        };
        let json = serde_json::to_string(&profile).unwrap();
        let restored: Profile = serde_json::from_str(&json).unwrap();
        let original_valid = profile.validate().is_ok();
        let restored_valid = restored.validate().is_ok();
        prop_assert_eq!(
            original_valid, restored_valid,
            "validation result changed after round-trip"
        );
    }

    // ── Profile diff ────────────────────────────────────────────────────────

    /// Property 5: Diff of a profile with itself produces zero differences.
    #[test]
    fn diff_self_is_empty(profile in arb_profile()) {
        let json = serde_json::to_value(&profile).unwrap();
        let flat = flatten_profile(&json, "");
        let diff = compare_profiles(&flat, &flat, "a", "b");
        prop_assert!(
            diff.is_empty(),
            "diff of profile with itself should be empty, got {} changes",
            diff.total_changes()
        );
    }

    /// Property 6: Diff(a, b) added_count == Diff(b, a) removed_count.
    #[test]
    fn diff_added_equals_reverse_removed(
        a in arb_profile(),
        b in arb_profile(),
    ) {
        let ja = serde_json::to_value(&a).unwrap();
        let jb = serde_json::to_value(&b).unwrap();
        let fa = flatten_profile(&ja, "");
        let fb = flatten_profile(&jb, "");
        let diff_ab = compare_profiles(&fa, &fb, "a", "b");
        let diff_ba = compare_profiles(&fb, &fa, "b", "a");
        prop_assert_eq!(
            diff_ab.added_count(), diff_ba.removed_count(),
            "added in (a→b) should equal removed in (b→a)"
        );
        prop_assert_eq!(
            diff_ab.removed_count(), diff_ba.added_count(),
            "removed in (a→b) should equal added in (b→a)"
        );
    }
}

// ── Template tests (non-proptest, exhaustive over all templates) ─────────────

#[test]
fn all_templates_produce_valid_profiles() {
    for template in Template::all() {
        let profile = template.build();
        let result = profile.validate();
        assert!(
            result.is_ok(),
            "Template {:?} ({}) produced invalid profile: {:?}",
            template,
            template.name(),
            result.err()
        );
    }
}

#[test]
fn all_templates_have_at_least_one_axis() {
    for template in Template::all() {
        let profile = template.build();
        assert!(
            !profile.axes.is_empty(),
            "Template {:?} produced profile with no axes",
            template
        );
    }
}

#[test]
fn all_templates_use_correct_schema_version() {
    for template in Template::all() {
        let profile = template.build();
        assert_eq!(
            profile.schema, PROFILE_SCHEMA_VERSION,
            "Template {:?} uses wrong schema version",
            template
        );
    }
}

#[test]
fn template_profiles_survive_json_roundtrip() {
    for template in Template::all() {
        let profile = template.build();
        let json = serde_json::to_string(&profile).unwrap();
        let restored: Profile = serde_json::from_str(&json).unwrap();
        assert_eq!(
            profile, restored,
            "Template {:?} profile changed after JSON round-trip",
            template
        );
    }
}

#[test]
fn template_profiles_have_stable_hash() {
    for template in Template::all() {
        let profile = template.build();
        let h1 = profile.effective_hash();
        let h2 = profile.effective_hash();
        assert_eq!(
            h1, h2,
            "Template {:?} effective_hash is not stable",
            template
        );
    }
}
