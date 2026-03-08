// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2025 Flight Hub Team

//! Profile management step definitions.
//!
//! Connects Gherkin steps to `flight_profile` types:
//! - Profile creation with axes
//! - Aircraft overlay profiles
//! - Profile merging via `merge_with`
//! - Merged-profile assertions

use crate::step_registry::{StepOutcome, StepRegistry};
use flight_profile::{AxisConfig, PROFILE_SCHEMA_VERSION, Profile};
use std::collections::HashMap;

/// Helper to build a default profile with the given number of axes.
fn make_profile(num_axes: usize) -> Profile {
    let mut axes = HashMap::new();
    for i in 0..num_axes {
        let name = default_axis_name(i);
        axes.insert(
            name,
            AxisConfig {
                deadzone: Some(0.03),
                expo: None,
                slew_rate: None,
                detents: vec![],
                curve: None,
                filter: None,
            },
        );
    }
    Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes,
        pof_overrides: None,
    }
}

fn default_axis_name(index: usize) -> String {
    match index {
        0 => "pitch".to_string(),
        1 => "roll".to_string(),
        2 => "yaw".to_string(),
        3 => "throttle".to_string(),
        _ => format!("axis_{index}"),
    }
}

/// Register all profile-related step definitions.
pub fn register(registry: &mut StepRegistry) {
    // -- Given ----------------------------------------------------------

    registry.given(
        r"^a global profile with (\d+) axes$",
        |ctx, caps| {
            let n: usize = match caps[1].parse() {
                Ok(v) => v,
                Err(e) => return StepOutcome::Failed(format!("bad int: {e}")),
            };
            let profile = make_profile(n);
            ctx.set("base_profile", profile);
            StepOutcome::Passed
        },
    );

    registry.given(
        r#"^an aircraft overlay for "([^"]+)"$"#,
        |ctx, caps| {
            let name = &caps[1];
            let mut overlay = Profile {
                schema: PROFILE_SCHEMA_VERSION.to_string(),
                sim: Some("msfs".to_string()),
                aircraft: Some(flight_profile::AircraftId {
                    icao: name.to_string(),
                }),
                axes: HashMap::new(),
                pof_overrides: None,
            };
            // Overlay overrides pitch deadzone
            overlay.axes.insert(
                "pitch".to_string(),
                AxisConfig {
                    deadzone: Some(0.08),
                    expo: Some(0.3),
                    slew_rate: None,
                    detents: vec![],
                    curve: None,
                    filter: None,
                },
            );
            ctx.set("overlay_profile", overlay);
            StepOutcome::Passed
        },
    );

    // -- When -----------------------------------------------------------

    registry.when(r"^profiles are merged$", |ctx, _caps| {
        let base = match ctx.get::<Profile>("base_profile") {
            Some(p) => p,
            None => return StepOutcome::Failed("no base_profile in context".to_string()),
        };
        let overlay = match ctx.get::<Profile>("overlay_profile") {
            Some(p) => p,
            None => return StepOutcome::Failed("no overlay_profile in context".to_string()),
        };
        match base.merge_with(&overlay) {
            Ok(merged) => {
                ctx.set("merged_profile", merged);
                StepOutcome::Passed
            }
            Err(e) => StepOutcome::Failed(format!("merge failed: {e}")),
        }
    });

    // -- Then -----------------------------------------------------------

    registry.then(
        r"^the merged profile should have (\d+) axes$",
        |ctx, caps| {
            let expected: usize = match caps[1].parse() {
                Ok(v) => v,
                Err(e) => return StepOutcome::Failed(format!("bad int: {e}")),
            };
            match ctx.get::<Profile>("merged_profile") {
                Some(p) => {
                    if p.axes.len() == expected {
                        StepOutcome::Passed
                    } else {
                        StepOutcome::Failed(format!(
                            "expected {} axes, got {}",
                            expected,
                            p.axes.len()
                        ))
                    }
                }
                None => StepOutcome::Failed("no merged_profile in context".to_string()),
            }
        },
    );

    registry.then(
        r#"^axis "([^"]+)" should have deadzone (-?\d+\.?\d*)$"#,
        |ctx, caps| {
            let axis_name = &caps[1];
            let expected_dz: f32 = match caps[2].parse() {
                Ok(v) => v,
                Err(e) => return StepOutcome::Failed(format!("bad float: {e}")),
            };
            let profile = match ctx.get::<Profile>("merged_profile") {
                Some(p) => p,
                None => return StepOutcome::Failed("no merged_profile in context".to_string()),
            };
            match profile.axes.get(axis_name) {
                Some(cfg) => match cfg.deadzone {
                    Some(dz) if (dz - expected_dz).abs() < 1e-6 => StepOutcome::Passed,
                    Some(dz) => StepOutcome::Failed(format!(
                        "axis '{axis_name}' deadzone: expected {expected_dz}, got {dz}"
                    )),
                    None => StepOutcome::Failed(format!(
                        "axis '{axis_name}' has no deadzone configured"
                    )),
                },
                None => StepOutcome::Failed(format!("axis '{axis_name}' not found in profile")),
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::{parse_scenario, run_scenario};

    fn registry() -> StepRegistry {
        let mut r = StepRegistry::new();
        register(&mut r);
        r
    }

    #[test]
    fn create_global_profile() {
        let reg = registry();
        let s = parse_scenario("create_profile", "Given a global profile with 3 axes").unwrap();
        let result = run_scenario(&s, &reg);
        assert!(result.is_passed(), "{:?}", result.step_results);
    }

    #[test]
    fn merge_profiles_preserves_axis_count() {
        let reg = registry();
        let text = r#"
            Given a global profile with 3 axes
            And an aircraft overlay for "C172"
            When profiles are merged
            Then the merged profile should have 3 axes
        "#;
        let s = parse_scenario("merge", text).unwrap();
        let result = run_scenario(&s, &reg);
        assert!(result.is_passed(), "{:?}", result.step_results);
    }

    #[test]
    fn merged_overlay_overrides_deadzone() {
        let reg = registry();
        let text = r#"
            Given a global profile with 3 axes
            And an aircraft overlay for "C172"
            When profiles are merged
            Then axis "pitch" should have deadzone 0.08
        "#;
        let s = parse_scenario("dz_override", text).unwrap();
        let result = run_scenario(&s, &reg);
        assert!(result.is_passed(), "{:?}", result.step_results);
    }

    #[test]
    fn base_axis_deadzone_unchanged_after_merge() {
        let reg = registry();
        let text = r#"
            Given a global profile with 3 axes
            And an aircraft overlay for "C172"
            When profiles are merged
            Then axis "roll" should have deadzone 0.03
        "#;
        let s = parse_scenario("dz_base", text).unwrap();
        let result = run_scenario(&s, &reg);
        assert!(result.is_passed(), "{:?}", result.step_results);
    }
}
