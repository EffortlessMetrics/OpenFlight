// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Profile sanitization before community upload.
//!
//! Strips any data that could identify a specific user or local hardware
//! configuration before a profile is published to the cloud repository.
//!
//! **What is removed:**
//! - Any axis comments or notes fields (not currently in the schema but
//!   guarded for future additions)
//! - The `schema` field is normalized to the canonical version string
//!
//! **What is preserved:**
//! - All axis configuration (deadzones, expo, curves, filters, detents)
//! - `sim` and `aircraft` identifiers — these are needed for browsing
//! - Phase-of-flight overrides
//!
//! # Example
//!
//! ```
//! use flight_profile::{Profile, AircraftId};
//! use flight_cloud_profiles::sanitize_for_upload;
//! use std::collections::HashMap;
//!
//! let profile = Profile {
//!     schema: "flight.profile/1".to_string(),
//!     sim: Some("msfs".to_string()),
//!     aircraft: Some(AircraftId { icao: "C172".to_string() }),
//!     axes: HashMap::new(),
//!     pof_overrides: None,
//! };
//!
//! let sanitized = sanitize_for_upload(&profile);
//! assert_eq!(sanitized.schema, "flight.profile/1");
//! assert_eq!(sanitized.sim.as_deref(), Some("msfs"));
//! ```

use flight_profile::{AxisConfig, Profile, PROFILE_SCHEMA_VERSION};

/// Return a sanitized copy of `profile` suitable for community upload.
///
/// The original profile is not modified.
pub fn sanitize_for_upload(profile: &Profile) -> Profile {
    Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: profile.sim.as_deref().map(|s| s.to_ascii_lowercase()),
        aircraft: profile.aircraft.clone(),
        axes: profile
            .axes
            .iter()
            .map(|(name, cfg)| (name.clone(), sanitize_axis(cfg)))
            .collect(),
        pof_overrides: profile.pof_overrides.clone(),
    }
}

/// Sanitize a single axis configuration (currently a no-op but kept as
/// an extension point for future fields like inline comments).
fn sanitize_axis(cfg: &AxisConfig) -> AxisConfig {
    cfg.clone()
}

/// Validate that a profile is suitable for publishing.
///
/// Returns `Ok(())` if the profile passes all checks, or an error message
/// describing the first violation found.
pub fn validate_for_publish(profile: &Profile, title: &str) -> Result<(), String> {
    if title.trim().is_empty() {
        return Err("title must not be blank".to_string());
    }
    if title.len() > 100 {
        return Err(format!("title too long ({} chars, max 100)", title.len()));
    }
    if profile.axes.is_empty() && profile.pof_overrides.is_none() {
        return Err("profile has no axis configuration".to_string());
    }
    // Validate each axis config
    for (name, cfg) in &profile.axes {
        if let Some(dz) = cfg.deadzone {
            if !(0.0..=0.5).contains(&dz) {
                return Err(format!(
                    "axis '{}': deadzone {dz} out of range [0.0, 0.5]",
                    name
                ));
            }
        }
        if let Some(expo) = cfg.expo {
            if !(0.0..=1.0).contains(&expo) {
                return Err(format!("axis '{}': expo {expo} out of range [0.0, 1.0]", name));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flight_profile::{AircraftId, AxisConfig};
    use std::collections::HashMap;

    fn make_profile(sim: Option<&str>, axes: HashMap<String, AxisConfig>) -> Profile {
        Profile {
            schema: "flight.profile/1".to_string(),
            sim: sim.map(|s| s.to_string()),
            aircraft: Some(AircraftId { icao: "C172".to_string() }),
            axes,
            pof_overrides: None,
        }
    }

    fn axis_with_deadzone(dz: f32) -> AxisConfig {
        AxisConfig {
            deadzone: Some(dz),
            expo: None,
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        }
    }

    // ── sanitize_for_upload ─────────────────────────────────────────────────

    #[test]
    fn test_sanitize_normalizes_schema_version() {
        let mut profile = make_profile(None, HashMap::new());
        profile.schema = "flight.profile/0".to_string(); // old version
        let sanitized = sanitize_for_upload(&profile);
        assert_eq!(sanitized.schema, PROFILE_SCHEMA_VERSION);
    }

    #[test]
    fn test_sanitize_lowercases_sim() {
        let profile = make_profile(Some("MSFS"), HashMap::new());
        let sanitized = sanitize_for_upload(&profile);
        assert_eq!(sanitized.sim.as_deref(), Some("msfs"));
    }

    #[test]
    fn test_sanitize_preserves_axes() {
        let mut axes = HashMap::new();
        axes.insert("pitch".to_string(), axis_with_deadzone(0.05));
        let profile = make_profile(Some("msfs"), axes);
        let sanitized = sanitize_for_upload(&profile);
        assert!(sanitized.axes.contains_key("pitch"));
        assert_eq!(sanitized.axes["pitch"].deadzone, Some(0.05));
    }

    #[test]
    fn test_sanitize_preserves_aircraft() {
        let profile = make_profile(Some("msfs"), HashMap::new());
        let sanitized = sanitize_for_upload(&profile);
        assert_eq!(sanitized.aircraft.as_ref().map(|a| &a.icao).unwrap(), "C172");
    }

    #[test]
    fn test_sanitize_does_not_modify_original() {
        let profile = make_profile(Some("MSFS"), HashMap::new());
        let original_sim = profile.sim.clone();
        let _ = sanitize_for_upload(&profile);
        assert_eq!(profile.sim, original_sim); // unchanged
    }

    // ── validate_for_publish ────────────────────────────────────────────────

    #[test]
    fn test_validate_empty_title_rejected() {
        let profile = make_profile(Some("msfs"), HashMap::new());
        assert!(validate_for_publish(&profile, "").is_err());
    }

    #[test]
    fn test_validate_whitespace_title_rejected() {
        let profile = make_profile(Some("msfs"), HashMap::new());
        assert!(validate_for_publish(&profile, "   ").is_err());
    }

    #[test]
    fn test_validate_title_too_long_rejected() {
        let profile = make_profile(Some("msfs"), HashMap::new());
        let title: String = "x".repeat(101);
        assert!(validate_for_publish(&profile, &title).is_err());
    }

    #[test]
    fn test_validate_empty_axes_rejected() {
        let profile = make_profile(Some("msfs"), HashMap::new());
        let err = validate_for_publish(&profile, "My Profile").unwrap_err();
        assert!(err.contains("no axis"));
    }

    #[test]
    fn test_validate_invalid_deadzone_rejected() {
        let mut axes = HashMap::new();
        axes.insert("pitch".to_string(), axis_with_deadzone(0.9)); // >0.5
        let profile = make_profile(Some("msfs"), axes);
        let err = validate_for_publish(&profile, "Bad Profile").unwrap_err();
        assert!(err.contains("deadzone"));
    }

    #[test]
    fn test_validate_valid_profile_accepted() {
        let mut axes = HashMap::new();
        axes.insert("pitch".to_string(), axis_with_deadzone(0.05));
        let profile = make_profile(Some("msfs"), axes);
        assert!(validate_for_publish(&profile, "Good Profile").is_ok());
    }
}
