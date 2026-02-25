// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Recommended axis presets for the VPforce Rhino.
//!
//! The Rhino uses Hall-effect sensors for the primary X/Y axes (very low noise)
//! and a resistive pot for throttle (moderate noise).

/// Recommended axis configuration entry.
#[derive(Debug, Clone)]
pub struct RecommendedAxisConfig {
    pub name: &'static str,
    pub deadzone: f32,
    pub filter_alpha: Option<f32>,
    pub slew_rate: Option<f32>,
    pub notes: &'static str,
}

/// Get recommended axis configurations for the VPforce Rhino.
///
/// # Example
///
/// ```
/// use flight_ffb_vpforce::presets::recommended_axis_config;
/// let cfg = recommended_axis_config();
/// assert_eq!(cfg[0].name, "roll");
/// assert!(cfg[0].deadzone < 0.02); // Hall-effect — very small deadzone
/// ```
pub fn recommended_axis_config() -> [RecommendedAxisConfig; 5] {
    [
        RecommendedAxisConfig {
            name: "roll",
            deadzone: 0.01,
            filter_alpha: None, // Hall-effect: no smoothing needed
            slew_rate: None,
            notes: "X axis — Hall-effect sensor; minimal deadzone required",
        },
        RecommendedAxisConfig {
            name: "pitch",
            deadzone: 0.01,
            filter_alpha: None,
            slew_rate: None,
            notes: "Y axis — Hall-effect sensor; minimal deadzone required",
        },
        RecommendedAxisConfig {
            name: "throttle",
            deadzone: 0.03,
            filter_alpha: Some(0.12),
            slew_rate: Some(2.0),
            notes: "Z slider — resistive pot; light filtering recommended",
        },
        RecommendedAxisConfig {
            name: "twist",
            deadzone: 0.04,
            filter_alpha: Some(0.15),
            slew_rate: None,
            notes: "Rz — resistive pot twist; moderate deadzone",
        },
        RecommendedAxisConfig {
            name: "rocker",
            deadzone: 0.04,
            filter_alpha: Some(0.15),
            slew_rate: None,
            notes: "Rx — side rocker; moderate deadzone",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preset_count() {
        assert_eq!(recommended_axis_config().len(), 5);
    }

    #[test]
    fn test_roll_has_minimal_deadzone() {
        let cfg = recommended_axis_config();
        assert!(
            cfg[0].deadzone < 0.02,
            "Hall-effect roll should have small deadzone"
        );
    }

    #[test]
    fn test_throttle_has_filter_alpha() {
        let cfg = recommended_axis_config();
        let throttle = cfg.iter().find(|c| c.name == "throttle").unwrap();
        assert!(throttle.filter_alpha.is_some());
    }
}
