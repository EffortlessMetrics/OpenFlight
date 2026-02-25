// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Recommended axis presets for the Moza AB9 FFB base.

/// Recommended axis configuration entry.
#[derive(Debug, Clone)]
pub struct RecommendedAxisConfig {
    pub name: &'static str,
    pub deadzone: f32,
    pub filter_alpha: Option<f32>,
    pub notes: &'static str,
}

/// Get recommended axis configurations for the Moza AB9 + joystick module.
///
/// # Example
///
/// ```
/// use flight_ffb_moza::presets::ab9_axis_config;
/// let cfg = ab9_axis_config();
/// assert_eq!(cfg[0].name, "roll");
/// ```
pub fn ab9_axis_config() -> [RecommendedAxisConfig; 4] {
    [
        RecommendedAxisConfig {
            name: "roll",
            deadzone: 0.01,
            filter_alpha: None,
            notes: "X axis — servo-based; Hall-effect feedback",
        },
        RecommendedAxisConfig {
            name: "pitch",
            deadzone: 0.01,
            filter_alpha: None,
            notes: "Y axis — servo-based; Hall-effect feedback",
        },
        RecommendedAxisConfig {
            name: "throttle",
            deadzone: 0.03,
            filter_alpha: Some(0.12),
            notes: "Z slider — resistive; light filter",
        },
        RecommendedAxisConfig {
            name: "twist",
            deadzone: 0.04,
            filter_alpha: Some(0.15),
            notes: "Rz twist — resistive pot",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preset_count() {
        assert_eq!(ab9_axis_config().len(), 4);
    }

    #[test]
    fn test_servo_axes_minimal_deadzone() {
        let cfg = ab9_axis_config();
        assert!(cfg[0].deadzone < 0.02);
        assert!(cfg[1].deadzone < 0.02);
    }
}
