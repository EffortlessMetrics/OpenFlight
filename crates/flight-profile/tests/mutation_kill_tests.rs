// SPDX-License-Identifier: MIT OR Apache-2.0
// Targeted tests to improve mutation kill rate in flight-profile.
// Covers validation boundaries, merge correctness, capability limit enforcement,
// and return value mutations.

use flight_profile::{
    merge_axis_configs, AxisConfig, CapabilityContext, CapabilityMode, CurvePoint, DetentZone,
    FilterConfig, Profile, PROFILE_SCHEMA_VERSION,
};
use std::collections::HashMap;

fn minimal_profile() -> Profile {
    Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes: HashMap::new(),
        pof_overrides: None,
    }
}

fn axis_with_expo(expo: f32) -> AxisConfig {
    AxisConfig {
        deadzone: None,
        expo: Some(expo),
        slew_rate: None,
        detents: vec![],
        curve: None,
        filter: None,
    }
}

// ── Schema validation ────────────────────────────────────────────────────

#[test]
fn wrong_schema_version_rejected() {
    let mut p = minimal_profile();
    p.schema = "flight.profile/2".to_string();
    assert!(p.validate().is_err());
}

#[test]
fn correct_schema_version_accepted() {
    let p = minimal_profile();
    assert!(p.validate().is_ok());
}

// ── Deadzone boundary validation ─────────────────────────────────────────

#[test]
fn deadzone_at_exact_max_accepted() {
    let mut p = minimal_profile();
    p.axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.5),
            ..axis_with_expo(0.0)
        },
    );
    assert!(p.validate().is_ok(), "deadzone=0.5 must be accepted");
}

#[test]
fn deadzone_above_max_rejected() {
    let mut p = minimal_profile();
    p.axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.501),
            ..axis_with_expo(0.0)
        },
    );
    assert!(p.validate().is_err(), "deadzone=0.501 must be rejected");
}

#[test]
fn deadzone_negative_rejected() {
    let mut p = minimal_profile();
    p.axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(-0.01),
            ..axis_with_expo(0.0)
        },
    );
    assert!(p.validate().is_err(), "negative deadzone must be rejected");
}

// ── Expo boundary validation ─────────────────────────────────────────────

#[test]
fn expo_at_exact_max_accepted() {
    let mut p = minimal_profile();
    p.axes.insert("pitch".to_string(), axis_with_expo(1.0));
    assert!(p.validate().is_ok(), "expo=1.0 must be accepted in Full mode");
}

#[test]
fn expo_above_max_rejected() {
    let mut p = minimal_profile();
    p.axes.insert("pitch".to_string(), axis_with_expo(1.001));
    assert!(p.validate().is_err(), "expo=1.001 must be rejected");
}

// ── Capability mode boundary tests ───────────────────────────────────────

#[test]
fn kid_mode_expo_at_exact_limit_accepted() {
    // Catches > vs >= mutation on expo limit check
    let mut p = minimal_profile();
    p.axes.insert("pitch".to_string(), axis_with_expo(0.3));
    let ctx = CapabilityContext::for_mode(CapabilityMode::Kid);
    assert!(
        p.validate_with_capabilities(&ctx).is_ok(),
        "kid mode expo=0.3 (at limit) must be accepted"
    );
}

#[test]
fn kid_mode_expo_just_above_limit_rejected() {
    let mut p = minimal_profile();
    p.axes.insert("pitch".to_string(), axis_with_expo(0.301));
    let ctx = CapabilityContext::for_mode(CapabilityMode::Kid);
    assert!(
        p.validate_with_capabilities(&ctx).is_err(),
        "kid mode expo=0.301 must be rejected"
    );
}

#[test]
fn demo_mode_expo_at_exact_limit_accepted() {
    let mut p = minimal_profile();
    p.axes.insert("pitch".to_string(), axis_with_expo(0.6));
    let ctx = CapabilityContext::for_mode(CapabilityMode::Demo);
    assert!(
        p.validate_with_capabilities(&ctx).is_ok(),
        "demo mode expo=0.6 (at limit) must be accepted"
    );
}

#[test]
fn demo_mode_expo_just_above_limit_rejected() {
    let mut p = minimal_profile();
    p.axes.insert("pitch".to_string(), axis_with_expo(0.601));
    let ctx = CapabilityContext::for_mode(CapabilityMode::Demo);
    assert!(
        p.validate_with_capabilities(&ctx).is_err(),
        "demo mode expo=0.601 must be rejected"
    );
}

// ── Detent validation boundaries ─────────────────────────────────────────

#[test]
fn detent_position_boundary_accepted() {
    // Catches < vs <= on detent.position checks
    let mut p = minimal_profile();
    p.axes.insert(
        "throttle".to_string(),
        AxisConfig {
            detents: vec![
                DetentZone {
                    position: -1.0,
                    width: 0.1,
                    role: "idle".to_string(),
                },
                DetentZone {
                    position: 1.0,
                    width: 0.1,
                    role: "toga".to_string(),
                },
            ],
            ..axis_with_expo(0.0)
        },
    );
    assert!(p.validate().is_ok(), "position at ±1.0 must be valid");
}

#[test]
fn detent_position_out_of_range_rejected() {
    let mut p = minimal_profile();
    p.axes.insert(
        "throttle".to_string(),
        AxisConfig {
            detents: vec![DetentZone {
                position: 1.01,
                width: 0.1,
                role: "test".to_string(),
            }],
            ..axis_with_expo(0.0)
        },
    );
    assert!(p.validate().is_err(), "position=1.01 must be rejected");
}

#[test]
fn detent_width_boundary() {
    // width must be > 0.0 and <= 0.5
    let mut p = minimal_profile();

    // width = 0.5 valid
    p.axes.insert(
        "throttle".to_string(),
        AxisConfig {
            detents: vec![DetentZone {
                position: 0.0,
                width: 0.5,
                role: "test".to_string(),
            }],
            ..axis_with_expo(0.0)
        },
    );
    assert!(p.validate().is_ok(), "width=0.5 must be accepted");

    // width = 0.0 invalid (must be > 0)
    p.axes.insert(
        "throttle".to_string(),
        AxisConfig {
            detents: vec![DetentZone {
                position: 0.0,
                width: 0.0,
                role: "test".to_string(),
            }],
            ..axis_with_expo(0.0)
        },
    );
    assert!(p.validate().is_err(), "width=0.0 must be rejected");

    // width = 0.501 invalid
    p.axes.insert(
        "throttle".to_string(),
        AxisConfig {
            detents: vec![DetentZone {
                position: 0.0,
                width: 0.501,
                role: "test".to_string(),
            }],
            ..axis_with_expo(0.0)
        },
    );
    assert!(p.validate().is_err(), "width=0.501 must be rejected");
}

// ── Filter config validation boundaries ──────────────────────────────────

#[test]
fn filter_alpha_boundaries() {
    let mut p = minimal_profile();

    // alpha = 0.0 valid
    p.axes.insert(
        "pitch".to_string(),
        AxisConfig {
            filter: Some(FilterConfig {
                alpha: 0.0,
                spike_threshold: None,
                max_spike_count: None,
            }),
            ..axis_with_expo(0.0)
        },
    );
    assert!(p.validate().is_ok(), "alpha=0.0 must be accepted");

    // alpha = 1.0 valid
    p.axes.get_mut("pitch").unwrap().filter.as_mut().unwrap().alpha = 1.0;
    assert!(p.validate().is_ok(), "alpha=1.0 must be accepted");

    // alpha = 1.01 invalid
    p.axes.get_mut("pitch").unwrap().filter.as_mut().unwrap().alpha = 1.01;
    assert!(p.validate().is_err(), "alpha=1.01 must be rejected");
}

#[test]
fn filter_spike_threshold_boundaries() {
    let mut p = minimal_profile();
    p.axes.insert(
        "pitch".to_string(),
        AxisConfig {
            filter: Some(FilterConfig {
                alpha: 0.5,
                spike_threshold: Some(0.0),
                max_spike_count: None,
            }),
            ..axis_with_expo(0.0)
        },
    );
    assert!(
        p.validate().is_err(),
        "spike_threshold=0.0 must be rejected (must be > 0)"
    );

    p.axes.get_mut("pitch").unwrap().filter.as_mut().unwrap().spike_threshold = Some(1.0);
    assert!(p.validate().is_ok(), "spike_threshold=1.0 must be accepted");

    p.axes.get_mut("pitch").unwrap().filter.as_mut().unwrap().spike_threshold = Some(1.01);
    assert!(
        p.validate().is_err(),
        "spike_threshold=1.01 must be rejected"
    );
}

#[test]
fn filter_max_spike_count_boundaries() {
    let mut p = minimal_profile();
    p.axes.insert(
        "pitch".to_string(),
        AxisConfig {
            filter: Some(FilterConfig {
                alpha: 0.5,
                spike_threshold: None,
                max_spike_count: Some(0),
            }),
            ..axis_with_expo(0.0)
        },
    );
    assert!(
        p.validate().is_err(),
        "max_spike_count=0 must be rejected"
    );

    p.axes.get_mut("pitch").unwrap().filter.as_mut().unwrap().max_spike_count = Some(1);
    assert!(p.validate().is_ok(), "max_spike_count=1 must be accepted");

    p.axes.get_mut("pitch").unwrap().filter.as_mut().unwrap().max_spike_count = Some(10);
    assert!(p.validate().is_ok(), "max_spike_count=10 must be accepted");

    p.axes.get_mut("pitch").unwrap().filter.as_mut().unwrap().max_spike_count = Some(11);
    assert!(
        p.validate().is_err(),
        "max_spike_count=11 must be rejected"
    );
}

// ── Curve monotonicity validation ────────────────────────────────────────

#[test]
fn curve_with_duplicate_input_rejected() {
    // Catches <= vs < mutation on curve[i].input <= curve[i-1].input
    let mut p = minimal_profile();
    p.axes.insert(
        "pitch".to_string(),
        AxisConfig {
            curve: Some(vec![
                CurvePoint { input: 0.0, output: 0.0 },
                CurvePoint { input: 0.5, output: 0.5 },
                CurvePoint { input: 0.5, output: 0.7 },
                CurvePoint { input: 1.0, output: 1.0 },
            ]),
            ..axis_with_expo(0.0)
        },
    );
    assert!(p.validate().is_err(), "duplicate input must be rejected");
}

#[test]
fn curve_with_single_point_rejected() {
    let mut p = minimal_profile();
    p.axes.insert(
        "pitch".to_string(),
        AxisConfig {
            curve: Some(vec![CurvePoint {
                input: 0.5,
                output: 0.5,
            }]),
            ..axis_with_expo(0.0)
        },
    );
    assert!(p.validate().is_err(), "single point curve must be rejected");
}

// ── Merge correctness ────────────────────────────────────────────────────

#[test]
fn merge_override_values_applied() {
    // Catches mutation where merge returns self.clone() instead of merged
    let mut base = minimal_profile();
    base.axes
        .insert("pitch".to_string(), axis_with_expo(0.2));

    let mut override_p = minimal_profile();
    override_p
        .axes
        .insert("pitch".to_string(), axis_with_expo(0.8));

    let merged = base.merge_with(&override_p).unwrap();
    let pitch = merged.axes.get("pitch").unwrap();
    assert_eq!(
        pitch.expo,
        Some(0.8),
        "override expo must win in merge"
    );
}

#[test]
fn merge_base_preserved_when_override_absent() {
    // Catches mutation where merge drops base values
    let mut base = minimal_profile();
    base.axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.05),
            expo: Some(0.3),
            ..axis_with_expo(0.0)
        },
    );

    let override_p = minimal_profile(); // no axes

    let merged = base.merge_with(&override_p).unwrap();
    let pitch = merged.axes.get("pitch").unwrap();
    assert_eq!(pitch.deadzone, Some(0.05), "base deadzone must be preserved");
    assert_eq!(pitch.expo, Some(0.3), "base expo must be preserved");
}

#[test]
fn merge_axis_empty_detents_uses_base() {
    // Catches mutation of `if override_config.detents.is_empty()` logic
    let base = AxisConfig {
        deadzone: None,
        expo: None,
        slew_rate: None,
        detents: vec![DetentZone {
            position: 0.0,
            width: 0.1,
            role: "idle".to_string(),
        }],
        curve: None,
        filter: None,
    };
    let override_cfg = AxisConfig {
        deadzone: Some(0.05),
        expo: None,
        slew_rate: None,
        detents: vec![], // empty: should fall back to base
        curve: None,
        filter: None,
    };

    let merged = merge_axis_configs(&base, &override_cfg);
    assert_eq!(
        merged.detents.len(),
        1,
        "empty override detents must use base"
    );
    assert_eq!(merged.detents[0].role, "idle");
    assert_eq!(
        merged.deadzone,
        Some(0.05),
        "override deadzone must be applied"
    );
}

#[test]
fn merge_axis_nonempty_detents_uses_override() {
    let base = AxisConfig {
        deadzone: None,
        expo: None,
        slew_rate: None,
        detents: vec![DetentZone {
            position: 0.0,
            width: 0.1,
            role: "base".to_string(),
        }],
        curve: None,
        filter: None,
    };
    let override_cfg = AxisConfig {
        deadzone: None,
        expo: None,
        slew_rate: None,
        detents: vec![DetentZone {
            position: 0.5,
            width: 0.2,
            role: "override".to_string(),
        }],
        curve: None,
        filter: None,
    };

    let merged = merge_axis_configs(&base, &override_cfg);
    assert_eq!(merged.detents.len(), 1);
    assert_eq!(merged.detents[0].role, "override");
}

#[test]
fn merge_sim_and_aircraft_override() {
    // Catches mutation that skips sim/aircraft override
    let mut base = minimal_profile();
    base.sim = Some("msfs".to_string());

    let mut override_p = minimal_profile();
    override_p.sim = Some("xplane".to_string());

    let merged = base.merge_with(&override_p).unwrap();
    assert_eq!(merged.sim, Some("xplane".to_string()), "override sim must win");
}
