// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Built-in profile templates.
//!
//! Each [`Template`] variant produces a [`Profile`] pre-populated with sensible
//! defaults for a particular aircraft category (GA, helicopter, space-sim, etc.).

use crate::{AxisConfig, CurvePoint, DetentZone, PROFILE_SCHEMA_VERSION, Profile};
use std::collections::HashMap;

/// Built-in template categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Template {
    /// Basic GA flight — stick/yoke pitch, roll, rudder, throttle.
    DefaultFlight,
    /// Helicopter — collective, cyclic pitch/roll, pedals.
    Helicopter,
    /// Space sim — 6-DOF (pitch, yaw, roll, strafe-x, strafe-y, strafe-z).
    SpaceSim,
    /// Airliner — yoke pitch/roll, rudder, throttle quadrant, speedbrake.
    Airliner,
    /// Warbird — stick pitch/roll, rudder, throttle, prop pitch, mixture.
    Warbird,
}

impl Template {
    /// Shortcut: `Template::DefaultFlight.build()`.
    pub fn default_flight() -> Profile {
        Template::DefaultFlight.build()
    }

    /// Shortcut: `Template::Helicopter.build()`.
    pub fn helicopter() -> Profile {
        Template::Helicopter.build()
    }

    /// Shortcut: `Template::SpaceSim.build()`.
    pub fn space_sim() -> Profile {
        Template::SpaceSim.build()
    }

    /// Shortcut: `Template::Airliner.build()`.
    pub fn airliner() -> Profile {
        Template::Airliner.build()
    }

    /// Shortcut: `Template::Warbird.build()`.
    pub fn warbird() -> Profile {
        Template::Warbird.build()
    }

    /// Materialise the template into a fully-populated [`Profile`].
    pub fn build(self) -> Profile {
        match self {
            Template::DefaultFlight => build_default_flight(),
            Template::Helicopter => build_helicopter(),
            Template::SpaceSim => build_space_sim(),
            Template::Airliner => build_airliner(),
            Template::Warbird => build_warbird(),
        }
    }

    /// Human-readable name.
    pub fn name(self) -> &'static str {
        match self {
            Template::DefaultFlight => "Default Flight",
            Template::Helicopter => "Helicopter",
            Template::SpaceSim => "Space Sim (6-DOF)",
            Template::Airliner => "Airliner",
            Template::Warbird => "Warbird",
        }
    }

    /// All built-in templates.
    pub fn all() -> &'static [Template] {
        &[
            Template::DefaultFlight,
            Template::Helicopter,
            Template::SpaceSim,
            Template::Airliner,
            Template::Warbird,
        ]
    }
}

// ── builders ─────────────────────────────────────────────────────────────────

fn linear_curve() -> Vec<CurvePoint> {
    vec![
        CurvePoint {
            input: 0.0,
            output: 0.0,
        },
        CurvePoint {
            input: 1.0,
            output: 1.0,
        },
    ]
}

fn gentle_expo_curve() -> Vec<CurvePoint> {
    vec![
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
            output: 0.3,
        },
        CurvePoint {
            input: 0.75,
            output: 0.6,
        },
        CurvePoint {
            input: 1.0,
            output: 1.0,
        },
    ]
}

fn flight_axis(deadzone: f32, expo: Option<f32>, curve: Option<Vec<CurvePoint>>) -> AxisConfig {
    AxisConfig {
        deadzone: Some(deadzone),
        expo,
        slew_rate: None,
        detents: vec![],
        curve,
        filter: None,
    }
}

fn throttle_axis(deadzone: f32) -> AxisConfig {
    AxisConfig {
        deadzone: Some(deadzone),
        expo: None,
        slew_rate: Some(5.0),
        detents: vec![DetentZone {
            position: 0.0,
            width: 0.03,
            role: "idle".to_string(),
        }],
        curve: Some(linear_curve()),
        filter: None,
    }
}

fn make_profile(axes: HashMap<String, AxisConfig>) -> Profile {
    Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes,
        pof_overrides: None,
    }
}

// ── template builders ────────────────────────────────────────────────────────

fn build_default_flight() -> Profile {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        flight_axis(0.03, Some(0.2), Some(gentle_expo_curve())),
    );
    axes.insert(
        "roll".to_string(),
        flight_axis(0.03, Some(0.15), Some(gentle_expo_curve())),
    );
    axes.insert("yaw".to_string(), flight_axis(0.05, Some(0.1), None));
    axes.insert("throttle".to_string(), throttle_axis(0.02));
    make_profile(axes)
}

fn build_helicopter() -> Profile {
    let mut axes = HashMap::new();
    // Cyclic — very sensitive, low deadzone.
    axes.insert(
        "cyclic_pitch".to_string(),
        flight_axis(0.02, Some(0.15), Some(gentle_expo_curve())),
    );
    axes.insert(
        "cyclic_roll".to_string(),
        flight_axis(0.02, Some(0.15), Some(gentle_expo_curve())),
    );
    // Pedals — moderate deadzone.
    axes.insert("pedals".to_string(), flight_axis(0.05, Some(0.1), None));
    // Collective — linear, with idle detent at bottom.
    axes.insert(
        "collective".to_string(),
        AxisConfig {
            deadzone: Some(0.01),
            expo: None,
            slew_rate: Some(3.0),
            detents: vec![DetentZone {
                position: -1.0,
                width: 0.05,
                role: "flat_pitch".to_string(),
            }],
            curve: Some(linear_curve()),
            filter: None,
        },
    );
    make_profile(axes)
}

fn build_space_sim() -> Profile {
    let mut axes = HashMap::new();
    let default_dz = 0.04;
    for axis_name in &["pitch", "yaw", "roll", "strafe_x", "strafe_y", "strafe_z"] {
        axes.insert(
            axis_name.to_string(),
            flight_axis(default_dz, Some(0.2), Some(gentle_expo_curve())),
        );
    }
    // Throttle for forward/back thrust.
    axes.insert(
        "throttle".to_string(),
        AxisConfig {
            deadzone: Some(0.02),
            expo: None,
            slew_rate: Some(8.0),
            detents: vec![DetentZone {
                position: 0.0,
                width: 0.04,
                role: "zero_thrust".to_string(),
            }],
            curve: Some(linear_curve()),
            filter: None,
        },
    );
    make_profile(axes)
}

fn build_airliner() -> Profile {
    let mut axes = HashMap::new();
    // Yoke.
    axes.insert(
        "pitch".to_string(),
        flight_axis(0.03, Some(0.25), Some(gentle_expo_curve())),
    );
    axes.insert(
        "roll".to_string(),
        flight_axis(0.03, Some(0.2), Some(gentle_expo_curve())),
    );
    // Rudder pedals.
    axes.insert("yaw".to_string(), flight_axis(0.05, Some(0.1), None));
    // Throttle quadrant — two engines.
    for engine in &["throttle_1", "throttle_2"] {
        axes.insert(
            engine.to_string(),
            AxisConfig {
                deadzone: Some(0.01),
                expo: None,
                slew_rate: Some(4.0),
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
                curve: Some(linear_curve()),
                filter: None,
            },
        );
    }
    // Speedbrake.
    axes.insert(
        "speedbrake".to_string(),
        AxisConfig {
            deadzone: Some(0.02),
            expo: None,
            slew_rate: Some(3.0),
            detents: vec![DetentZone {
                position: 0.0,
                width: 0.04,
                role: "retracted".to_string(),
            }],
            curve: Some(linear_curve()),
            filter: None,
        },
    );
    make_profile(axes)
}

fn build_warbird() -> Profile {
    let mut axes = HashMap::new();
    // Stick.
    axes.insert(
        "pitch".to_string(),
        flight_axis(0.03, Some(0.2), Some(gentle_expo_curve())),
    );
    axes.insert(
        "roll".to_string(),
        flight_axis(0.03, Some(0.15), Some(gentle_expo_curve())),
    );
    // Rudder.
    axes.insert("yaw".to_string(), flight_axis(0.05, Some(0.1), None));
    // Throttle.
    axes.insert("throttle".to_string(), throttle_axis(0.02));
    // Prop pitch.
    axes.insert(
        "prop_pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.01),
            expo: None,
            slew_rate: Some(3.0),
            detents: vec![],
            curve: Some(linear_curve()),
            filter: None,
        },
    );
    // Mixture.
    axes.insert(
        "mixture".to_string(),
        AxisConfig {
            deadzone: Some(0.01),
            expo: None,
            slew_rate: Some(3.0),
            detents: vec![
                DetentZone {
                    position: 0.0,
                    width: 0.03,
                    role: "cutoff".to_string(),
                },
                DetentZone {
                    position: 1.0,
                    width: 0.03,
                    role: "full_rich".to_string(),
                },
            ],
            curve: Some(linear_curve()),
            filter: None,
        },
    );
    make_profile(axes)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::deep_validate;

    /// Every template must produce a profile that passes standard validation.
    #[test]
    fn all_templates_validate() {
        for tmpl in Template::all() {
            let p = tmpl.build();
            p.validate()
                .unwrap_or_else(|e| panic!("{} template failed validation: {e}", tmpl.name()));
        }
    }

    /// Every template must pass deep validation with no errors.
    #[test]
    fn all_templates_deep_validate() {
        for tmpl in Template::all() {
            let p = tmpl.build();
            let r = deep_validate(&p);
            assert!(
                r.is_ok(),
                "{} template has deep-validation errors: {:?}",
                tmpl.name(),
                r.errors
            );
        }
    }

    /// Default flight must have pitch, roll, yaw, throttle.
    #[test]
    fn default_flight_has_expected_axes() {
        let p = Template::default_flight();
        for ax in &["pitch", "roll", "yaw", "throttle"] {
            assert!(
                p.axes.contains_key(*ax),
                "default_flight missing axis '{ax}'"
            );
        }
    }

    /// Helicopter must have cyclic, collective, pedals.
    #[test]
    fn helicopter_has_expected_axes() {
        let p = Template::helicopter();
        for ax in &["cyclic_pitch", "cyclic_roll", "collective", "pedals"] {
            assert!(p.axes.contains_key(*ax), "helicopter missing axis '{ax}'");
        }
    }

    /// Space sim must have 6-DOF + throttle.
    #[test]
    fn space_sim_has_6dof() {
        let p = Template::space_sim();
        for ax in &[
            "pitch", "yaw", "roll", "strafe_x", "strafe_y", "strafe_z", "throttle",
        ] {
            assert!(p.axes.contains_key(*ax), "space_sim missing axis '{ax}'");
        }
    }

    /// Airliner must have dual throttles, speedbrake, and flight axes.
    #[test]
    fn airliner_has_expected_axes() {
        let p = Template::airliner();
        for ax in &[
            "pitch",
            "roll",
            "yaw",
            "throttle_1",
            "throttle_2",
            "speedbrake",
        ] {
            assert!(p.axes.contains_key(*ax), "airliner missing axis '{ax}'");
        }
    }

    /// Warbird must have stick, throttle, prop, mixture.
    #[test]
    fn warbird_has_expected_axes() {
        let p = Template::warbird();
        for ax in &["pitch", "roll", "yaw", "throttle", "prop_pitch", "mixture"] {
            assert!(p.axes.contains_key(*ax), "warbird missing axis '{ax}'");
        }
    }

    /// All templates use the current schema version.
    #[test]
    fn templates_use_current_schema() {
        for tmpl in Template::all() {
            let p = tmpl.build();
            assert_eq!(
                p.schema,
                PROFILE_SCHEMA_VERSION,
                "{} has wrong schema",
                tmpl.name()
            );
        }
    }

    /// All templates have at least one axis with a deadzone set.
    #[test]
    fn templates_have_deadzones() {
        for tmpl in Template::all() {
            let p = tmpl.build();
            let has_dz = p.axes.values().any(|c| c.deadzone.is_some());
            assert!(has_dz, "{} has no axes with deadzones", tmpl.name());
        }
    }

    /// Template → Profile → validate round-trip.
    #[test]
    fn template_round_trip_via_json() {
        for tmpl in Template::all() {
            let p = tmpl.build();
            let json = p.export_json().unwrap();
            let restored: Profile = serde_json::from_str(&json).expect("JSON round-trip must work");
            assert_eq!(p, restored, "{} round-trip mismatch", tmpl.name());
            restored
                .validate()
                .unwrap_or_else(|e| panic!("{} restored profile invalid: {e}", tmpl.name()));
        }
    }

    /// Template names are non-empty.
    #[test]
    fn template_names_non_empty() {
        for tmpl in Template::all() {
            assert!(!tmpl.name().is_empty());
        }
    }
}
