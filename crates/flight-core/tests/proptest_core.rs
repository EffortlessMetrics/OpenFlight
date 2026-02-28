// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Property-based tests for flight-core types and invariants.
//!
//! Tests verify:
//! - Profile merge associativity
//! - Serialization round-trips
//! - Error catalog code uniqueness
//! - Validation consistency

use flight_core::error_catalog::{ErrorCatalog, ErrorCategory};
use flight_core::profile::{AircraftId, AxisConfig, PROFILE_SCHEMA_VERSION, Profile};
use proptest::prelude::*;
use std::collections::HashMap;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn make_profile(
    axes: HashMap<String, AxisConfig>,
    sim: Option<String>,
    aircraft: Option<AircraftId>,
) -> Profile {
    Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim,
        aircraft,
        axes,
        pof_overrides: None,
    }
}

fn simple_axis(deadzone: Option<f32>, expo: Option<f32>, slew_rate: Option<f32>) -> AxisConfig {
    AxisConfig {
        deadzone,
        expo,
        slew_rate,
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
        simple_axis(deadzone, expo, slew_rate)
    }
}

prop_compose! {
    fn arb_profile()(
        sim in prop::option::of("[a-z]+"),
        axes in prop::collection::hash_map("[a-z]{1,5}", arb_axis_config(), 0..3),
    ) -> Profile {
        make_profile(axes, sim, None)
    }
}

proptest! {
    // ── Profile merge associativity ─────────────────────────────────────────

    /// Property 1: merge(merge(a, b), c) == merge(a, merge(b, c))
    /// Profile merge is associative when profiles use non-overlapping axes.
    #[test]
    fn profile_merge_associative_disjoint_axes(
        dz_a in 0.0f32..0.5,
        dz_b in 0.0f32..0.5,
        dz_c in 0.0f32..0.5,
    ) {
        let a = make_profile(
            [("pitch".to_string(), simple_axis(Some(dz_a), None, None))].into(),
            None, None,
        );
        let b = make_profile(
            [("roll".to_string(), simple_axis(Some(dz_b), None, None))].into(),
            None, None,
        );
        let c = make_profile(
            [("yaw".to_string(), simple_axis(Some(dz_c), None, None))].into(),
            None, None,
        );

        let ab_c = a.merge_with(&b).unwrap().merge_with(&c).unwrap();
        let a_bc = a.merge_with(&b.merge_with(&c).unwrap()).unwrap();

        prop_assert_eq!(ab_c.axes.len(), a_bc.axes.len());
        for (key, val) in &ab_c.axes {
            let other = a_bc.axes.get(key).expect("key should exist in both");
            prop_assert_eq!(val, other, "merge associativity violated for axis '{}'", key);
        }
    }

    /// Property 2: Merging a profile with an empty profile is identity.
    #[test]
    fn profile_merge_with_empty_is_identity(profile in arb_profile()) {
        let empty = make_profile(HashMap::new(), None, None);
        let merged = profile.merge_with(&empty).unwrap();
        prop_assert_eq!(&profile, &merged, "merging with empty changed the profile");
    }

    // ── Serialization round-trip ────────────────────────────────────────────

    /// Property 3: deserialize(serialize(profile)) == profile
    #[test]
    fn profile_json_roundtrip(profile in arb_profile()) {
        let json = serde_json::to_string(&profile).unwrap();
        let restored: Profile = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(&profile, &restored, "JSON round-trip changed profile");
    }

    /// Property 4: Canonical hash is stable across serialize/deserialize.
    #[test]
    fn profile_hash_stable_across_roundtrip(profile in arb_profile()) {
        let json = serde_json::to_string(&profile).unwrap();
        let restored: Profile = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(
            profile.effective_hash(),
            restored.effective_hash(),
            "effective_hash changed after JSON round-trip"
        );
    }

    /// Property 5: canonicalize() is deterministic — same profile always yields same string.
    #[test]
    fn profile_canonicalize_deterministic(profile in arb_profile()) {
        let c1 = profile.canonicalize();
        let c2 = profile.canonicalize();
        prop_assert_eq!(c1, c2, "canonicalize produced different results");
    }

    // ── Validation consistency ──────────────────────────────────────────────

    /// Property 6: A valid profile re-validates successfully.
    #[test]
    fn valid_profile_passes_revalidation(
        dz in 0.0f32..0.5,
        expo in 0.0f32..1.0,
        slew in 0.0f32..100.0,
    ) {
        let profile = make_profile(
            [("pitch".to_string(), simple_axis(Some(dz), Some(expo), Some(slew)))].into(),
            None, None,
        );
        let first = profile.validate();
        prop_assert!(first.is_ok(), "first validation failed: {:?}", first.err());
        let second = profile.validate();
        prop_assert!(second.is_ok(), "re-validation failed: {:?}", second.err());
    }

    /// Property 7: Merged valid profiles produce a valid profile.
    #[test]
    fn merged_valid_profiles_are_valid(
        dz_a in 0.0f32..0.5,
        expo_a in 0.0f32..1.0,
        dz_b in 0.0f32..0.5,
        expo_b in 0.0f32..1.0,
    ) {
        let a = make_profile(
            [("pitch".to_string(), simple_axis(Some(dz_a), Some(expo_a), None))].into(),
            None, None,
        );
        let b = make_profile(
            [("roll".to_string(), simple_axis(Some(dz_b), Some(expo_b), None))].into(),
            None, None,
        );
        prop_assert!(a.validate().is_ok());
        prop_assert!(b.validate().is_ok());
        let merged = a.merge_with(&b).unwrap();
        prop_assert!(merged.validate().is_ok(), "merged profile failed validation");
    }
}

// ── Error catalog tests (non-proptest, verifying structural invariants) ──────

#[test]
fn error_codes_are_unique() {
    let all = ErrorCatalog::all();
    let mut seen = std::collections::HashSet::new();
    for info in all {
        assert!(
            seen.insert(info.code),
            "Duplicate error code: {}",
            info.code
        );
    }
}

#[test]
fn all_error_categories_have_entries() {
    let categories = [
        ErrorCategory::Device,
        ErrorCategory::Sim,
        ErrorCategory::Profile,
        ErrorCategory::Service,
        ErrorCategory::Plugin,
        ErrorCategory::Network,
        ErrorCategory::Config,
        ErrorCategory::Internal,
    ];
    for cat in categories {
        let entries = ErrorCatalog::by_category(cat);
        assert!(
            !entries.is_empty(),
            "Category {:?} has no entries in catalog",
            cat
        );
    }
}

#[test]
fn error_code_format_is_consistent() {
    for info in ErrorCatalog::all() {
        // Each code should match pattern: 3 uppercase letters, dash, 3 digits
        let parts: Vec<&str> = info.code.split('-').collect();
        assert_eq!(
            parts.len(),
            2,
            "Error code '{}' should have format XXX-NNN",
            info.code
        );
        assert_eq!(
            parts[0].len(),
            3,
            "Error code prefix '{}' should be 3 chars",
            parts[0]
        );
        assert_eq!(
            parts[1].len(),
            3,
            "Error code number '{}' should be 3 digits",
            parts[1]
        );
        assert!(
            parts[0].chars().all(|c| c.is_ascii_uppercase()),
            "Error code prefix '{}' should be uppercase",
            parts[0]
        );
        assert!(
            parts[1].chars().all(|c| c.is_ascii_digit()),
            "Error code number '{}' should be digits",
            parts[1]
        );
    }
}
