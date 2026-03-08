// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Standard safe-mode axis defaults and per-aircraft-category profiles.
//!
//! These are intentionally conservative: no custom curves, no detents, no
//! slew-rate limiters.  The goal is a profile that *always* compiles and
//! gives the pilot predictable, monotonic control authority.

use crate::{AircraftId, AxisConfig, Profile, PROFILE_SCHEMA_VERSION};
use std::collections::HashMap;

// ── Axis-level constants ────────────────────────────────────────────────────

/// Standard deadzone for flight axes (pitch, roll, yaw).
pub const FLIGHT_AXIS_DEADZONE: f32 = 0.03;

/// Standard expo for flight axes.
pub const FLIGHT_AXIS_EXPO: f32 = 0.2;

/// Standard deadzone for throttle axes.
pub const THROTTLE_DEADZONE: f32 = 0.01;

/// Helicopter collective deadzone — slightly larger to avoid inadvertent input.
pub const COLLECTIVE_DEADZONE: f32 = 0.02;

// ── Axis config constructors ────────────────────────────────────────────────

/// Safe axis config for pitch / roll / yaw.
pub fn flight_axis() -> AxisConfig {
    AxisConfig {
        deadzone: Some(FLIGHT_AXIS_DEADZONE),
        expo: Some(FLIGHT_AXIS_EXPO),
        slew_rate: None,
        detents: vec![],
        curve: None,
        filter: None,
    }
}

/// Safe axis config for a throttle / power lever.
pub fn throttle_axis() -> AxisConfig {
    AxisConfig {
        deadzone: Some(THROTTLE_DEADZONE),
        expo: None,
        slew_rate: None,
        detents: vec![],
        curve: None,
        filter: None,
    }
}

/// Safe axis config for helicopter collective — linear, slightly wider deadzone.
pub fn collective_axis() -> AxisConfig {
    AxisConfig {
        deadzone: Some(COLLECTIVE_DEADZONE),
        expo: None,
        slew_rate: None,
        detents: vec![],
        curve: None,
        filter: None,
    }
}

// ── Aircraft category ───────────────────────────────────────────────────────

/// Broad aircraft category used to select a default profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AircraftCategory {
    /// General aviation (Cessna 172, Piper Cherokee, etc.)
    GeneralAviation,
    /// Jet / turboprop (A320, 737, King Air, etc.)
    Jet,
    /// Helicopter / rotorcraft
    Helicopter,
}

// ── Per-category default profiles ───────────────────────────────────────────

/// Build the default safe-mode profile for an aircraft category.
pub fn default_profile_for(category: AircraftCategory) -> Profile {
    match category {
        AircraftCategory::GeneralAviation => ga_profile(),
        AircraftCategory::Jet => jet_profile(),
        AircraftCategory::Helicopter => helicopter_profile(),
    }
}

/// Default safe profile for general aviation aircraft.
///
/// Four axes: pitch, roll, yaw, throttle.
pub fn ga_profile() -> Profile {
    let mut axes = HashMap::new();
    for name in ["pitch", "roll", "yaw"] {
        axes.insert(name.to_string(), flight_axis());
    }
    axes.insert("throttle".to_string(), throttle_axis());

    Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: Some(AircraftId {
            icao: "C172".to_string(),
        }),
        axes,
        pof_overrides: None,
    }
}

/// Default safe profile for jet / turboprop aircraft.
///
/// Adds a second throttle axis for twin-engine jets.
pub fn jet_profile() -> Profile {
    let mut axes = HashMap::new();
    for name in ["pitch", "roll", "yaw"] {
        axes.insert(name.to_string(), flight_axis());
    }
    axes.insert("throttle_1".to_string(), throttle_axis());
    axes.insert("throttle_2".to_string(), throttle_axis());

    Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: Some(AircraftId {
            icao: "B738".to_string(),
        }),
        axes,
        pof_overrides: None,
    }
}

/// Default safe profile for helicopters.
///
/// Replaces throttle with a collective axis.
pub fn helicopter_profile() -> Profile {
    let mut axes = HashMap::new();
    for name in ["pitch", "roll", "yaw"] {
        axes.insert(name.to_string(), flight_axis());
    }
    axes.insert("collective".to_string(), collective_axis());

    Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: Some(AircraftId {
            icao: "R22".to_string(),
        }),
        axes,
        pof_overrides: None,
    }
}

/// Return the universal safe-mode profile (no aircraft, no sim).
///
/// This is the absolute fallback used by [`SafeModeManager::create_basic_profile`].
pub fn safe_mode_profile() -> Profile {
    let mut axes = HashMap::new();
    for name in ["pitch", "roll", "yaw"] {
        axes.insert(name.to_string(), flight_axis());
    }
    axes.insert("throttle".to_string(), throttle_axis());

    Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes,
        pof_overrides: None,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Every category profile must pass validation.
    #[test]
    fn all_category_profiles_validate() {
        for category in [
            AircraftCategory::GeneralAviation,
            AircraftCategory::Jet,
            AircraftCategory::Helicopter,
        ] {
            let profile = default_profile_for(category);
            profile
                .validate()
                .unwrap_or_else(|e| panic!("{category:?} profile failed validation: {e}"));
        }
    }

    /// The universal safe-mode profile validates.
    #[test]
    fn safe_mode_profile_validates() {
        safe_mode_profile()
            .validate()
            .expect("safe-mode profile must validate");
    }

    /// GA profile has the expected four axes.
    #[test]
    fn ga_profile_axes() {
        let p = ga_profile();
        assert!(p.axes.contains_key("pitch"));
        assert!(p.axes.contains_key("roll"));
        assert!(p.axes.contains_key("yaw"));
        assert!(p.axes.contains_key("throttle"));
        assert_eq!(p.axes.len(), 4);
    }

    /// Jet profile has twin throttles.
    #[test]
    fn jet_profile_axes() {
        let p = jet_profile();
        assert!(p.axes.contains_key("pitch"));
        assert!(p.axes.contains_key("roll"));
        assert!(p.axes.contains_key("yaw"));
        assert!(p.axes.contains_key("throttle_1"));
        assert!(p.axes.contains_key("throttle_2"));
        assert_eq!(p.axes.len(), 5);
    }

    /// Helicopter profile has collective instead of throttle.
    #[test]
    fn helicopter_profile_axes() {
        let p = helicopter_profile();
        assert!(p.axes.contains_key("pitch"));
        assert!(p.axes.contains_key("roll"));
        assert!(p.axes.contains_key("yaw"));
        assert!(p.axes.contains_key("collective"));
        assert!(!p.axes.contains_key("throttle"));
        assert_eq!(p.axes.len(), 4);
    }

    /// Flight axes have the documented defaults.
    #[test]
    fn flight_axis_defaults() {
        let ax = flight_axis();
        assert_eq!(ax.deadzone, Some(FLIGHT_AXIS_DEADZONE));
        assert_eq!(ax.expo, Some(FLIGHT_AXIS_EXPO));
        assert!(ax.slew_rate.is_none());
        assert!(ax.detents.is_empty());
        assert!(ax.curve.is_none());
        assert!(ax.filter.is_none());
    }

    /// Throttle axis has the documented defaults.
    #[test]
    fn throttle_axis_defaults() {
        let ax = throttle_axis();
        assert_eq!(ax.deadzone, Some(THROTTLE_DEADZONE));
        assert!(ax.expo.is_none());
    }

    /// Collective axis has slightly wider deadzone than throttle.
    #[test]
    fn collective_axis_defaults() {
        let ax = collective_axis();
        assert_eq!(ax.deadzone, Some(COLLECTIVE_DEADZONE));
        assert!(ax.expo.is_none());
    }

    /// All default profiles use monotonic, no-curve, no-detent axes.
    #[test]
    fn all_defaults_are_safe() {
        let profiles = [
            ga_profile(),
            jet_profile(),
            helicopter_profile(),
            safe_mode_profile(),
        ];
        for p in &profiles {
            for (name, ax) in &p.axes {
                assert!(
                    ax.curve.is_none(),
                    "axis '{name}' must not have a custom curve"
                );
                assert!(ax.detents.is_empty(), "axis '{name}' must have no detents");
                assert!(
                    ax.slew_rate.is_none(),
                    "axis '{name}' must have no slew limit"
                );
            }
        }
    }

    /// Effective hash is stable for each category profile.
    #[test]
    fn category_profile_hashes_are_stable() {
        for category in [
            AircraftCategory::GeneralAviation,
            AircraftCategory::Jet,
            AircraftCategory::Helicopter,
        ] {
            let p = default_profile_for(category);
            let h1 = p.effective_hash();
            let h2 = p.effective_hash();
            assert_eq!(h1, h2, "hash must be deterministic for {category:?}");
        }
    }
}
