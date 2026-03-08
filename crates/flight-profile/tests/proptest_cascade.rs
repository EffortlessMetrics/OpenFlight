// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Property-based tests for profile cascade invariants.
//!
//! Validates:
//! - Identity: merging with an empty profile is identity
//! - Idempotence: merge(A, A) == A for all set axes
//! - More-specific always wins for any field that is `Some`

use flight_profile::{AxisConfig, PROFILE_SCHEMA_VERSION, Profile};
use proptest::prelude::*;
use std::collections::HashMap;

// ── Strategies ──────────────────────────────────────────────────────────────

prop_compose! {
    fn arb_axis_config()(
        deadzone in prop::option::of(0.0f32..0.5f32),
        expo in prop::option::of(0.0f32..1.0f32),
        slew_rate in prop::option::of(0.0f32..100.0f32),
    ) -> AxisConfig {
        AxisConfig {
            deadzone,
            expo,
            slew_rate,
            detents: vec![],
            curve: None,
            filter: None,
        }
    }
}

prop_compose! {
    fn arb_profile()(
        sim in prop::option::of(prop::sample::select(vec![
            "msfs".to_string(),
            "xplane".to_string(),
            "dcs".to_string(),
        ])),
        pitch in arb_axis_config(),
        roll in arb_axis_config(),
    ) -> Profile {
        let mut axes = HashMap::new();
        axes.insert("pitch".to_string(), pitch);
        axes.insert("roll".to_string(), roll);
        Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim,
            aircraft: None,
            axes,
            pof_overrides: None,
        }
    }
}

fn empty_profile() -> Profile {
    Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes: HashMap::new(),
        pof_overrides: None,
    }
}

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

proptest! {
    // ── Identity ────────────────────────────────────────────────────────────

    /// Merging any profile with an empty profile preserves the original.
    #[test]
    fn merge_with_empty_is_identity(profile in arb_profile()) {
        let empty = empty_profile();
        let merged = profile.merge_with(&empty).unwrap();

        for (name, config) in &profile.axes {
            let merged_config = merged.axes.get(name).expect("axis should survive merge");
            prop_assert_eq!(merged_config.deadzone, config.deadzone,
                "axis {}: deadzone changed after merge with empty", name);
            prop_assert_eq!(merged_config.expo, config.expo,
                "axis {}: expo changed after merge with empty", name);
            prop_assert_eq!(merged_config.slew_rate, config.slew_rate,
                "axis {}: slew_rate changed after merge with empty", name);
        }

        prop_assert_eq!(merged.axes.len(), profile.axes.len(),
            "axis count should not change when merging with empty");
    }

    /// Merging an empty profile with any profile yields the overlay.
    #[test]
    fn empty_merge_with_overlay_yields_overlay(profile in arb_profile()) {
        let empty = empty_profile();
        let merged = empty.merge_with(&profile).unwrap();

        for (name, config) in &profile.axes {
            let merged_config = merged.axes.get(name).expect("overlay axis should appear");
            prop_assert_eq!(merged_config.deadzone, config.deadzone);
            prop_assert_eq!(merged_config.expo, config.expo);
            prop_assert_eq!(merged_config.slew_rate, config.slew_rate);
        }
    }

    // ── Idempotence ─────────────────────────────────────────────────────────

    /// Merging a profile with itself is idempotent: merge(A, A) == A.
    #[test]
    fn merge_self_is_idempotent(profile in arb_profile()) {
        let merged = profile.merge_with(&profile).unwrap();
        prop_assert_eq!(&merged, &profile,
            "merge(A, A) should equal A");
    }

    // ── More-Specific Wins ──────────────────────────────────────────────────

    /// When the override has a `Some` deadzone, it always wins.
    #[test]
    fn override_deadzone_wins(
        base_dz in 0.0f32..0.5,
        override_dz in 0.0f32..0.5,
    ) {
        let base = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None,
            aircraft: None,
            axes: [("pitch".to_string(), AxisConfig {
                deadzone: Some(base_dz),
                ..empty_axis()
            })].into(),
            pof_overrides: None,
        };
        let overlay = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None,
            aircraft: None,
            axes: [("pitch".to_string(), AxisConfig {
                deadzone: Some(override_dz),
                ..empty_axis()
            })].into(),
            pof_overrides: None,
        };

        let merged = base.merge_with(&overlay).unwrap();
        prop_assert_eq!(
            merged.axes.get("pitch").unwrap().deadzone,
            Some(override_dz),
            "override deadzone {} should win over base {}",
            override_dz, base_dz
        );
    }

    /// When the override has a `Some` expo, it always wins.
    #[test]
    fn override_expo_wins(
        base_expo in 0.0f32..1.0,
        override_expo in 0.0f32..1.0,
    ) {
        let base = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None,
            aircraft: None,
            axes: [("pitch".to_string(), AxisConfig {
                expo: Some(base_expo),
                ..empty_axis()
            })].into(),
            pof_overrides: None,
        };
        let overlay = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None,
            aircraft: None,
            axes: [("pitch".to_string(), AxisConfig {
                expo: Some(override_expo),
                ..empty_axis()
            })].into(),
            pof_overrides: None,
        };

        let merged = base.merge_with(&overlay).unwrap();
        prop_assert_eq!(
            merged.axes.get("pitch").unwrap().expo,
            Some(override_expo),
        );
    }

    /// When the override has `None` for a field, the base value is preserved.
    #[test]
    fn none_override_preserves_base(
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
        let overlay = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None,
            aircraft: None,
            axes: [("pitch".to_string(), empty_axis())].into(),
            pof_overrides: None,
        };

        let merged = base.merge_with(&overlay).unwrap();
        let pitch = merged.axes.get("pitch").unwrap();
        prop_assert_eq!(pitch.deadzone, Some(base_dz));
        prop_assert_eq!(pitch.expo, Some(base_expo));
        prop_assert_eq!(pitch.slew_rate, Some(base_slew));
    }

    // ── Hash Stability ──────────────────────────────────────────────────────

    /// The effective_hash is deterministic for any profile.
    #[test]
    fn hash_is_deterministic(profile in arb_profile()) {
        let h1 = profile.effective_hash();
        let h2 = profile.effective_hash();
        prop_assert_eq!(h1, h2,
            "effective_hash must be deterministic");
    }

    // ── Merge Preserves Axis Count ──────────────────────────────────────────

    /// Merged profile has at least as many axes as either input.
    #[test]
    fn merge_axis_count_is_union(
        a in arb_profile(),
        b in arb_profile(),
    ) {
        let merged = a.merge_with(&b).unwrap();
        prop_assert!(
            merged.axes.len() >= a.axes.len(),
            "merged should have >= base axes"
        );
        prop_assert!(
            merged.axes.len() >= b.axes.len(),
            "merged should have >= overlay axes"
        );
    }

    // ── Sim/Aircraft Metadata Propagation ───────────────────────────────────

    /// When the overlay sets sim, it wins. When None, base is preserved.
    #[test]
    fn sim_metadata_propagation(
        base_sim in prop::option::of("msfs|xplane|dcs"),
        overlay_sim in prop::option::of("msfs|xplane|dcs"),
    ) {
        let base = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: base_sim.clone(),
            aircraft: None,
            axes: HashMap::new(),
            pof_overrides: None,
        };
        let overlay = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: overlay_sim.clone(),
            aircraft: None,
            axes: HashMap::new(),
            pof_overrides: None,
        };

        let merged = base.merge_with(&overlay).unwrap();

        if overlay_sim.is_some() {
            prop_assert_eq!(merged.sim, overlay_sim);
        } else {
            prop_assert_eq!(merged.sim, base_sim);
        }
    }
}
