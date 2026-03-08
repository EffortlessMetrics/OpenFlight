// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Expanded property-based tests for flight-profile mathematical invariants.
//!
//! Tests beyond existing proptest suites:
//! 1. merge_with associativity: (A.merge(B)).merge(C) == A.merge(B.merge(C)) for scalar fields
//! 2. Merge with empty profile is identity
//! 3. Cascade order matters: A.merge(B) != B.merge(A) when both define same axis
//! 4. Serialization roundtrip preserves equality (JSON)
//! 5. Validation is deterministic: validate() == validate() always

use flight_profile::{AxisConfig, PROFILE_SCHEMA_VERSION, Profile};
use proptest::prelude::*;

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

fn empty_profile() -> Profile {
    Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes: Default::default(),
        pof_overrides: None,
    }
}

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
        axes in prop::collection::hash_map("[a-z]{1,3}", arb_axis(), 0..3),
    ) -> Profile {
        Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None,
            aircraft: None,
            axes,
            pof_overrides: None,
        }
    }
}

proptest! {
    // ── 1. merge_with associativity for scalar axis fields ─────────────────

    /// (A.merge(B)).merge(C) == A.merge(B.merge(C)) when all profiles have
    /// the same axis key and only scalar fields differ.
    #[test]
    fn merge_with_associativity_scalars(
        dz_a in prop::option::of(0.0f32..0.2f32),
        dz_b in prop::option::of(0.2f32..0.4f32),
        dz_c in prop::option::of(0.3f32..0.5f32),
        expo_a in prop::option::of(0.0f32..0.3f32),
        expo_b in prop::option::of(0.3f32..0.6f32),
        expo_c in prop::option::of(0.6f32..1.0f32),
    ) {
        let a = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None, aircraft: None,
            axes: [("pitch".to_string(), simple_axis(dz_a, expo_a))].into(),
            pof_overrides: None,
        };
        let b = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None, aircraft: None,
            axes: [("pitch".to_string(), simple_axis(dz_b, expo_b))].into(),
            pof_overrides: None,
        };
        let c = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None, aircraft: None,
            axes: [("pitch".to_string(), simple_axis(dz_c, expo_c))].into(),
            pof_overrides: None,
        };

        let ab = a.merge_with(&b).unwrap();
        let ab_c = ab.merge_with(&c).unwrap();

        let bc = b.merge_with(&c).unwrap();
        let a_bc = a.merge_with(&bc).unwrap();

        let pitch_abc = ab_c.axes.get("pitch").unwrap();
        let pitch_a_bc = a_bc.axes.get("pitch").unwrap();

        // With last-writer-wins semantics, both orderings should produce C's
        // values for any field that C specifies (since C is applied last in both).
        prop_assert_eq!(pitch_abc.deadzone, pitch_a_bc.deadzone,
            "associativity deadzone: (A.B).C={:?}, A.(B.C)={:?}", pitch_abc.deadzone, pitch_a_bc.deadzone);
        prop_assert_eq!(pitch_abc.expo, pitch_a_bc.expo,
            "associativity expo: (A.B).C={:?}, A.(B.C)={:?}", pitch_abc.expo, pitch_a_bc.expo);
    }

    // ── 2. Merge with empty profile is identity ─────────────────────────────

    /// Merging any profile with an empty profile returns the original unchanged.
    #[test]
    fn merge_with_empty_is_identity(profile in arb_profile()) {
        let empty = empty_profile();
        let merged = profile.merge_with(&empty).unwrap();
        prop_assert_eq!(
            profile.axes, merged.axes,
            "merge with empty should be identity"
        );
    }

    /// Merging an empty profile with any profile returns the overlay.
    #[test]
    fn merge_empty_base_with_overlay(overlay in arb_profile()) {
        let empty = empty_profile();
        let merged = empty.merge_with(&overlay).unwrap();
        prop_assert_eq!(
            overlay.axes, merged.axes,
            "merging empty base with overlay should produce overlay axes"
        );
    }

    // ── 3. Cascade order matters ────────────────────────────────────────────

    /// When A and B both define the same axis with different values,
    /// A.merge(B) and B.merge(A) may differ (last-writer-wins).
    #[test]
    fn cascade_order_matters(
        expo_a in 0.0f32..0.3f32,
        expo_b in 0.7f32..1.0f32,
    ) {
        let a = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None, aircraft: None,
            axes: [("pitch".to_string(), simple_axis(None, Some(expo_a)))].into(),
            pof_overrides: None,
        };
        let b = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None, aircraft: None,
            axes: [("pitch".to_string(), simple_axis(None, Some(expo_b)))].into(),
            pof_overrides: None,
        };

        let ab = a.merge_with(&b).unwrap();
        let ba = b.merge_with(&a).unwrap();

        let ab_expo = ab.axes.get("pitch").unwrap().expo;
        let ba_expo = ba.axes.get("pitch").unwrap().expo;

        // A.merge(B) should have B's expo; B.merge(A) should have A's expo.
        prop_assert_eq!(ab_expo, Some(expo_b), "A.merge(B) should have B's expo");
        prop_assert_eq!(ba_expo, Some(expo_a), "B.merge(A) should have A's expo");
    }

    // ── 4. Serialization roundtrip ──────────────────────────────────────────

    /// JSON serialize→deserialize produces structurally identical profile.
    #[test]
    fn json_roundtrip_equality(profile in arb_profile()) {
        let json = serde_json::to_string(&profile).unwrap();
        let restored: Profile = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(profile, restored, "JSON roundtrip changed profile");
    }

    // ── 5. Validation is deterministic ──────────────────────────────────────

    /// Calling validate() twice on the same profile produces the same result.
    #[test]
    fn validation_deterministic(
        dz in 0.0f32..0.5f32,
        expo in 0.0f32..1.0f32,
    ) {
        let profile = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None, aircraft: None,
            axes: [("pitch".to_string(), simple_axis(Some(dz), Some(expo)))].into(),
            pof_overrides: None,
        };
        let v1 = profile.validate().is_ok();
        let v2 = profile.validate().is_ok();
        prop_assert_eq!(v1, v2, "validation result changed between calls");
    }

    /// Validation result matches after clone.
    #[test]
    fn validation_stable_after_clone(profile in arb_profile()) {
        let cloned = profile.clone();
        let v1 = profile.validate().is_ok();
        let v2 = cloned.validate().is_ok();
        prop_assert_eq!(v1, v2, "validation differs between original and clone");
    }
}
