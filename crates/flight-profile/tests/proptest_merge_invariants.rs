// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Property-based tests for profile merge invariants.
//!
//! - merge_with preserves override values
//! - merge_with with empty override is identity
//! - Effective hash is stable after merge
//! - Serialization round-trip preserves merge result
//! - Merged profile with valid inputs passes validation

use flight_profile::{
    AircraftId, AxisConfig, Profile, PROFILE_SCHEMA_VERSION,
};
use proptest::prelude::*;
use std::collections::HashMap;

fn make_profile(axes: HashMap<String, AxisConfig>) -> Profile {
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

fn make_axis(deadzone: Option<f32>, expo: Option<f32>, slew_rate: Option<f32>) -> AxisConfig {
    AxisConfig {
        deadzone,
        expo,
        slew_rate,
        detents: vec![],
        curve: None,
        filter: None,
    }
}

proptest! {
    // ── Override preservation ────────────────────────────────────────────────

    /// When override has expo=X, merged result has expo=X.
    #[test]
    fn merge_preserves_override_expo(
        base_expo in 0.0f32..0.5,
        override_expo in 0.5f32..1.0,
        base_dz in 0.0f32..0.5,
    ) {
        let base = make_profile(
            [("pitch".to_string(), make_axis(Some(base_dz), Some(base_expo), None))].into(),
        );
        let over = make_profile(
            [("pitch".to_string(), make_axis(None, Some(override_expo), None))].into(),
        );
        let merged = base.merge_with(&over).unwrap();
        let pitch = merged.axes.get("pitch").unwrap();
        prop_assert_eq!(
            pitch.expo,
            Some(override_expo),
            "override expo {} should win, got {:?}",
            override_expo,
            pitch.expo
        );
        // Base deadzone should be preserved since override has None
        prop_assert_eq!(pitch.deadzone, Some(base_dz));
    }

    // ── Identity: merge with empty is identity ──────────────────────────────

    /// Merging with an empty-axes profile preserves all base values.
    #[test]
    fn merge_with_empty_is_identity(
        dz in 0.0f32..0.5,
        expo in 0.0f32..1.0,
        slew in 0.0f32..100.0,
    ) {
        let base = make_profile(
            [("pitch".to_string(), make_axis(Some(dz), Some(expo), Some(slew)))].into(),
        );
        let empty = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None,
            aircraft: None,
            axes: HashMap::new(),
            pof_overrides: None,
        };
        let merged = base.merge_with(&empty).unwrap();
        prop_assert_eq!(base, merged, "merge with empty should be identity");
    }

    // ── Hash stability after merge ──────────────────────────────────────────

    /// Effective hash is deterministic after merge.
    #[test]
    fn hash_stable_after_merge(
        dz in 0.0f32..0.5,
        expo in 0.0f32..1.0,
    ) {
        let base = make_profile(
            [("pitch".to_string(), make_axis(Some(dz), Some(expo), None))].into(),
        );
        let over = make_profile(
            [("pitch".to_string(), make_axis(None, Some(0.5), None))].into(),
        );
        let merged = base.merge_with(&over).unwrap();
        let h1 = merged.effective_hash();
        let h2 = merged.effective_hash();
        prop_assert_eq!(h1, h2, "hash should be stable after merge");
    }

    // ── Serialization round-trip after merge ────────────────────────────────

    /// Merged profile survives JSON serialization round-trip.
    #[test]
    fn merge_survives_json_roundtrip(
        dz in 0.0f32..0.5,
        expo in 0.0f32..1.0,
        over_slew in 0.0f32..100.0,
    ) {
        let base = make_profile(
            [("pitch".to_string(), make_axis(Some(dz), Some(expo), None))].into(),
        );
        let over = make_profile(
            [("pitch".to_string(), make_axis(None, None, Some(over_slew)))].into(),
        );
        let merged = base.merge_with(&over).unwrap();
        let json = serde_json::to_string(&merged).unwrap();
        let restored: Profile = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(merged, restored, "merge result changed after JSON round-trip");
    }

    // ── Merged valid profiles pass validation ───────────────────────────────

    /// Merging two valid profiles produces a valid profile.
    #[test]
    fn merge_of_valid_profiles_is_valid(
        dz_a in 0.0f32..0.5,
        expo_a in 0.0f32..1.0,
        dz_b in 0.0f32..0.5,
        expo_b in 0.0f32..1.0,
    ) {
        let a = make_profile(
            [("pitch".to_string(), make_axis(Some(dz_a), Some(expo_a), None))].into(),
        );
        let b = make_profile(
            [("pitch".to_string(), make_axis(Some(dz_b), Some(expo_b), None))].into(),
        );
        prop_assert!(a.validate().is_ok());
        prop_assert!(b.validate().is_ok());
        let merged = a.merge_with(&b).unwrap();
        prop_assert!(
            merged.validate().is_ok(),
            "merge of two valid profiles should be valid"
        );
    }

    // ── Merge adds new axes from override ───────────────────────────────────

    /// Override axes that don't exist in base are added to the merged profile.
    #[test]
    fn merge_adds_new_axes(
        dz in 0.0f32..0.5,
        expo in 0.0f32..1.0,
    ) {
        let base = make_profile(
            [("pitch".to_string(), make_axis(Some(dz), None, None))].into(),
        );
        let over = make_profile(
            [("roll".to_string(), make_axis(None, Some(expo), None))].into(),
        );
        let merged = base.merge_with(&over).unwrap();
        prop_assert!(
            merged.axes.contains_key("pitch"),
            "base axis 'pitch' should be preserved"
        );
        prop_assert!(
            merged.axes.contains_key("roll"),
            "override axis 'roll' should be added"
        );
        prop_assert_eq!(
            merged.axes.len(),
            2,
            "merged should have both axes"
        );
    }
}
