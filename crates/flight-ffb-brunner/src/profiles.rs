// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Per-aircraft FFB profiles for Brunner CLS-E / CLS-P.
//!
//! Profiles define the default effect parameters for different aircraft
//! categories. The profile system allows pilots to have different force
//! characteristics for general aviation, heavy transport, fighter, and
//! helicopter aircraft.
//!
//! # Profile cascade
//!
//! Per ADR-007, profiles cascade: Global → Simulator → Aircraft → Phase.
//! These Brunner-specific presets provide the aircraft-level defaults.

use serde::{Deserialize, Serialize};

use crate::effects::{
    DamperParams, FrictionParams, PeriodicParams, PeriodicWaveform, SpringParams,
};

/// Aircraft category for profile selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AircraftCategory {
    /// General aviation (Cessna 172, Piper Cherokee, etc.)
    GeneralAviation,
    /// Heavy transport (Boeing 737, A320, etc.)
    Transport,
    /// Military fighter (F-16, F/A-18, etc.)
    Fighter,
    /// Helicopter (Bell 206, H145, etc.)
    Helicopter,
}

impl AircraftCategory {
    /// All defined categories.
    pub const ALL: &'static [Self] = &[
        Self::GeneralAviation,
        Self::Transport,
        Self::Fighter,
        Self::Helicopter,
    ];
}

impl std::fmt::Display for AircraftCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GeneralAviation => f.write_str("General Aviation"),
            Self::Transport => f.write_str("Transport"),
            Self::Fighter => f.write_str("Fighter"),
            Self::Helicopter => f.write_str("Helicopter"),
        }
    }
}

/// Brunner FFB profile — defines effect parameters for a device/aircraft.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrunnerProfile {
    /// Profile name.
    pub name: String,
    /// Aircraft category.
    pub category: AircraftCategory,
    /// Spring centering parameters.
    pub spring: SpringParams,
    /// Damper parameters.
    pub damper: DamperParams,
    /// Friction parameters.
    pub friction: FrictionParams,
    /// Turbulence vibration intensity scaling (0.0 = off, 1.0 = full).
    pub turbulence_scale: f32,
    /// Turbulence vibration frequency in Hz.
    pub turbulence_frequency_hz: f32,
    /// Trim force coefficient (how much force the trim offset produces).
    pub trim_force_coefficient: f32,
    /// Maximum force envelope (normalised 0.0..1.0).
    pub max_force: f32,
}

impl BrunnerProfile {
    /// Create a profile for general aviation aircraft.
    ///
    /// Light controls with moderate centering and low friction.
    pub fn general_aviation() -> Self {
        Self {
            name: "General Aviation".into(),
            category: AircraftCategory::GeneralAviation,
            spring: SpringParams {
                coefficient: 0.4,
                center: 0.0,
                dead_band: 0.02,
                saturation: 0.8,
            },
            damper: DamperParams { coefficient: 0.2 },
            friction: FrictionParams { coefficient: 0.08 },
            turbulence_scale: 0.6,
            turbulence_frequency_hz: 8.0,
            trim_force_coefficient: 0.3,
            max_force: 0.7,
        }
    }

    /// Create a profile for transport / airliner aircraft.
    ///
    /// Heavy controls with strong centering and moderate friction.
    pub fn transport() -> Self {
        Self {
            name: "Transport".into(),
            category: AircraftCategory::Transport,
            spring: SpringParams {
                coefficient: 0.7,
                center: 0.0,
                dead_band: 0.01,
                saturation: 1.0,
            },
            damper: DamperParams { coefficient: 0.4 },
            friction: FrictionParams { coefficient: 0.15 },
            turbulence_scale: 0.3,
            turbulence_frequency_hz: 6.0,
            trim_force_coefficient: 0.5,
            max_force: 0.9,
        }
    }

    /// Create a profile for fighter / military jet aircraft.
    ///
    /// Stiff, responsive controls with minimal friction and low damping.
    pub fn fighter() -> Self {
        Self {
            name: "Fighter".into(),
            category: AircraftCategory::Fighter,
            spring: SpringParams {
                coefficient: 0.9,
                center: 0.0,
                dead_band: 0.0,
                saturation: 1.0,
            },
            damper: DamperParams { coefficient: 0.15 },
            friction: FrictionParams { coefficient: 0.05 },
            turbulence_scale: 0.2,
            turbulence_frequency_hz: 12.0,
            trim_force_coefficient: 0.7,
            max_force: 1.0,
        }
    }

    /// Create a profile for helicopters.
    ///
    /// Light centering with higher damping for cyclic feel.
    pub fn helicopter() -> Self {
        Self {
            name: "Helicopter".into(),
            category: AircraftCategory::Helicopter,
            spring: SpringParams {
                coefficient: 0.25,
                center: 0.0,
                dead_band: 0.03,
                saturation: 0.6,
            },
            damper: DamperParams { coefficient: 0.35 },
            friction: FrictionParams { coefficient: 0.12 },
            turbulence_scale: 0.8,
            turbulence_frequency_hz: 15.0,
            trim_force_coefficient: 0.2,
            max_force: 0.6,
        }
    }

    /// Get a profile for the given aircraft category.
    pub fn for_category(category: AircraftCategory) -> Self {
        match category {
            AircraftCategory::GeneralAviation => Self::general_aviation(),
            AircraftCategory::Transport => Self::transport(),
            AircraftCategory::Fighter => Self::fighter(),
            AircraftCategory::Helicopter => Self::helicopter(),
        }
    }

    /// Build a turbulence periodic effect from this profile's settings.
    pub fn turbulence_effect(&self) -> PeriodicParams {
        PeriodicParams {
            waveform: PeriodicWaveform::Sine,
            frequency_hz: self.turbulence_frequency_hz.clamp(1.0, 200.0),
            amplitude: self.turbulence_scale.clamp(0.0, 1.0),
            phase: 0.0,
        }
    }
}

/// Return the default CLS-E profile (general aviation).
pub fn default_cls_e_profile() -> BrunnerProfile {
    BrunnerProfile::general_aviation()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_is_ga() {
        let p = default_cls_e_profile();
        assert_eq!(p.category, AircraftCategory::GeneralAviation);
        assert_eq!(p.name, "General Aviation");
    }

    #[test]
    fn all_categories_have_profiles() {
        for &cat in AircraftCategory::ALL {
            let p = BrunnerProfile::for_category(cat);
            assert_eq!(p.category, cat);
        }
    }

    #[test]
    fn ga_spring_lighter_than_transport() {
        let ga = BrunnerProfile::general_aviation();
        let transport = BrunnerProfile::transport();
        assert!(
            ga.spring.coefficient < transport.spring.coefficient,
            "GA spring should be lighter than transport"
        );
    }

    #[test]
    fn fighter_spring_stiffest() {
        let fighter = BrunnerProfile::fighter();
        let transport = BrunnerProfile::transport();
        let ga = BrunnerProfile::general_aviation();
        assert!(fighter.spring.coefficient > transport.spring.coefficient);
        assert!(fighter.spring.coefficient > ga.spring.coefficient);
    }

    #[test]
    fn helicopter_damping_higher_than_fighter() {
        let heli = BrunnerProfile::helicopter();
        let fighter = BrunnerProfile::fighter();
        assert!(
            heli.damper.coefficient > fighter.damper.coefficient,
            "helicopter should have more damping"
        );
    }

    #[test]
    fn fighter_has_no_dead_band() {
        let fighter = BrunnerProfile::fighter();
        assert!((fighter.spring.dead_band).abs() < 1e-6);
    }

    #[test]
    fn helicopter_has_highest_turbulence_scale() {
        let heli = BrunnerProfile::helicopter();
        let ga = BrunnerProfile::general_aviation();
        let transport = BrunnerProfile::transport();
        let fighter = BrunnerProfile::fighter();
        assert!(heli.turbulence_scale > ga.turbulence_scale);
        assert!(heli.turbulence_scale > transport.turbulence_scale);
        assert!(heli.turbulence_scale > fighter.turbulence_scale);
    }

    #[test]
    fn all_profiles_max_force_in_range() {
        for &cat in AircraftCategory::ALL {
            let p = BrunnerProfile::for_category(cat);
            assert!(
                (0.0..=1.0).contains(&p.max_force),
                "{cat}: max_force {} out of range",
                p.max_force
            );
        }
    }

    #[test]
    fn all_profiles_spring_coefficient_in_range() {
        for &cat in AircraftCategory::ALL {
            let p = BrunnerProfile::for_category(cat);
            assert!(
                (0.0..=1.0).contains(&p.spring.coefficient),
                "{cat}: spring coefficient out of range"
            );
        }
    }

    #[test]
    fn all_profiles_damper_coefficient_in_range() {
        for &cat in AircraftCategory::ALL {
            let p = BrunnerProfile::for_category(cat);
            assert!(
                (0.0..=1.0).contains(&p.damper.coefficient),
                "{cat}: damper coefficient out of range"
            );
        }
    }

    #[test]
    fn all_profiles_friction_coefficient_in_range() {
        for &cat in AircraftCategory::ALL {
            let p = BrunnerProfile::for_category(cat);
            assert!(
                (0.0..=1.0).contains(&p.friction.coefficient),
                "{cat}: friction coefficient out of range"
            );
        }
    }

    #[test]
    fn turbulence_effect_matches_profile() {
        let ga = BrunnerProfile::general_aviation();
        let effect = ga.turbulence_effect();
        assert!((effect.amplitude - ga.turbulence_scale).abs() < 1e-6);
        assert!((effect.frequency_hz - ga.turbulence_frequency_hz).abs() < 1e-6);
        assert_eq!(effect.waveform, PeriodicWaveform::Sine);
    }

    #[test]
    fn category_display() {
        assert_eq!(
            AircraftCategory::GeneralAviation.to_string(),
            "General Aviation"
        );
        assert_eq!(AircraftCategory::Transport.to_string(), "Transport");
        assert_eq!(AircraftCategory::Fighter.to_string(), "Fighter");
        assert_eq!(AircraftCategory::Helicopter.to_string(), "Helicopter");
    }

    #[test]
    fn transport_trim_force_higher_than_ga() {
        let transport = BrunnerProfile::transport();
        let ga = BrunnerProfile::general_aviation();
        assert!(
            transport.trim_force_coefficient > ga.trim_force_coefficient,
            "transport should have higher trim force"
        );
    }

    #[test]
    fn all_categories_constant() {
        assert_eq!(AircraftCategory::ALL.len(), 4);
    }

    // ── Serialization round-trip ──────────────────────────────────────────────

    #[test]
    fn profile_serde_roundtrip() {
        let original = BrunnerProfile::transport();
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: BrunnerProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(original, deserialized);
    }
}
