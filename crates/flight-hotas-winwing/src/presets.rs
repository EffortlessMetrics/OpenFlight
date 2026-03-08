// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Recommended axis presets for WinWing HOTAS devices.
//!
//! WinWing uses precision Hall-effect sensors on all primary axes.
//! Very small deadzones and no filtering is required under normal conditions.

/// Recommended axis configuration entry.
#[derive(Debug, Clone)]
pub struct RecommendedAxisConfig {
    pub name: &'static str,
    pub deadzone: f32,
    pub filter_alpha: Option<f32>,
    pub slew_rate: Option<f32>,
    pub notes: &'static str,
}

/// Get recommended axis configurations for the Orion 2 Throttle.
///
/// # Example
///
/// ```
/// use flight_hotas_winwing::presets::orion2_throttle_config;
/// let cfg = orion2_throttle_config();
/// // Left and right throttle use Hall-effect sensors — tiny deadzones
/// assert!(cfg[0].deadzone < 0.02);
/// ```
pub fn orion2_throttle_config() -> [RecommendedAxisConfig; 4] {
    [
        RecommendedAxisConfig {
            name: "throttle_left",
            deadzone: 0.01,
            filter_alpha: None,
            slew_rate: None,
            notes: "Left throttle lever — Hall-effect, very low noise",
        },
        RecommendedAxisConfig {
            name: "throttle_right",
            deadzone: 0.01,
            filter_alpha: None,
            slew_rate: None,
            notes: "Right throttle lever — Hall-effect, very low noise",
        },
        RecommendedAxisConfig {
            name: "friction",
            deadzone: 0.03,
            filter_alpha: Some(0.10),
            slew_rate: None,
            notes: "Friction slider — resistive; light filter",
        },
        RecommendedAxisConfig {
            name: "mouse_stick",
            deadzone: 0.08,
            filter_alpha: Some(0.20),
            slew_rate: None,
            notes: "Slew/mouse stick — spring return; larger deadzone",
        },
    ]
}

/// Get recommended axis configurations for the Orion 2 Stick.
pub fn orion2_stick_config() -> [RecommendedAxisConfig; 2] {
    [
        RecommendedAxisConfig {
            name: "roll",
            deadzone: 0.01,
            filter_alpha: None,
            slew_rate: None,
            notes: "X axis — Hall-effect stick",
        },
        RecommendedAxisConfig {
            name: "pitch",
            deadzone: 0.01,
            filter_alpha: None,
            slew_rate: None,
            notes: "Y axis — Hall-effect stick",
        },
    ]
}

/// Get recommended axis configurations for the TFRP Rudder Pedals.
pub fn tfrp_rudder_config() -> [RecommendedAxisConfig; 3] {
    [
        RecommendedAxisConfig {
            name: "rudder",
            deadzone: 0.04,
            filter_alpha: Some(0.12),
            slew_rate: None,
            notes: "Rudder axis — linear pot; moderate deadzone",
        },
        RecommendedAxisConfig {
            name: "brake_left",
            deadzone: 0.04,
            filter_alpha: Some(0.12),
            slew_rate: None,
            notes: "Left toe brake — linear pot",
        },
        RecommendedAxisConfig {
            name: "brake_right",
            deadzone: 0.04,
            filter_alpha: Some(0.12),
            slew_rate: None,
            notes: "Right toe brake — linear pot",
        },
    ]
}

/// Get recommended axis configurations for the Super Taurus F-15EX Throttle.
///
/// The Super Taurus uses the same Hall-effect sensors as the Orion 2 with
/// dual throttle levers plus friction and slew axes.
pub fn super_taurus_config() -> [RecommendedAxisConfig; 3] {
    [
        RecommendedAxisConfig {
            name: "throttle_left",
            deadzone: 0.01,
            filter_alpha: None,
            slew_rate: None,
            notes: "Left throttle lever — Hall-effect, very low noise",
        },
        RecommendedAxisConfig {
            name: "throttle_right",
            deadzone: 0.01,
            filter_alpha: None,
            slew_rate: None,
            notes: "Right throttle lever — Hall-effect, very low noise",
        },
        RecommendedAxisConfig {
            name: "trim",
            deadzone: 0.03,
            filter_alpha: Some(0.10),
            slew_rate: None,
            notes: "Trim wheel — signed axis",
        },
    ]
}

/// Get recommended axis configurations for the Super Libra Joystick Base.
///
/// The Super Libra uses high-precision Hall-effect sensors on both axes.
pub fn super_libra_config() -> [RecommendedAxisConfig; 2] {
    [
        RecommendedAxisConfig {
            name: "roll",
            deadzone: 0.01,
            filter_alpha: None,
            slew_rate: None,
            notes: "Roll axis — Hall-effect, centre-mount gimbal",
        },
        RecommendedAxisConfig {
            name: "pitch",
            deadzone: 0.01,
            filter_alpha: None,
            slew_rate: None,
            notes: "Pitch axis — Hall-effect, centre-mount gimbal",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_throttle_preset_count() {
        assert_eq!(orion2_throttle_config().len(), 4);
    }

    #[test]
    fn test_stick_preset_count() {
        assert_eq!(orion2_stick_config().len(), 2);
    }

    #[test]
    fn test_rudder_preset_count() {
        assert_eq!(tfrp_rudder_config().len(), 3);
    }

    #[test]
    fn test_super_taurus_preset_count() {
        assert_eq!(super_taurus_config().len(), 3);
    }

    #[test]
    fn test_super_libra_preset_count() {
        assert_eq!(super_libra_config().len(), 2);
    }

    #[test]
    fn test_hall_effect_axes_have_small_deadzone() {
        for cfg in &orion2_throttle_config()[..2] {
            assert!(
                cfg.deadzone < 0.02,
                "{} should have small deadzone",
                cfg.name
            );
        }
    }
}
