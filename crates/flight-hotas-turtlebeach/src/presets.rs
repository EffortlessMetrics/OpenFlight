// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Default axis configuration presets for Turtle Beach VelocityOne devices.
//!
//! Each device has tuned deadzone, expo, and filter parameters based on its
//! hardware characteristics. The VelocityOne Flight and Flightstick use
//! 12-bit Hall-effect sensors, which are relatively low-noise, so filtering
//! is light.

/// Recommended axis configuration with tuned parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct RecommendedAxisConfig {
    /// Axis name (e.g., "roll", "pitch", "throttle_left").
    pub name: &'static str,
    /// Recommended deadzone value.
    pub deadzone: f32,
    /// Recommended EMA filter alpha (`None` = no filtering).
    pub filter_alpha: Option<f32>,
    /// Recommended slew rate limit (`None` = no limit).
    pub slew_rate: Option<f32>,
    /// Notes about this axis configuration.
    pub notes: &'static str,
}

// ── VelocityOne Flight presets ───────────────────────────────────────────────

/// Recommended axis configurations for the VelocityOne Flight (Flightdeck).
///
/// The Flight has 12-bit Hall-effect sensors on the yoke axes and 8-bit
/// potentiometers on the throttle quadrant.
pub fn flight_axis_config() -> [RecommendedAxisConfig; 6] {
    [
        RecommendedAxisConfig {
            name: "roll",
            deadzone: 0.02,
            filter_alpha: Some(0.12),
            slew_rate: None,
            notes: "Yoke roll — Hall-effect, low noise, light filtering",
        },
        RecommendedAxisConfig {
            name: "pitch",
            deadzone: 0.02,
            filter_alpha: Some(0.12),
            slew_rate: None,
            notes: "Yoke pitch — Hall-effect, low noise, light filtering",
        },
        RecommendedAxisConfig {
            name: "rudder_twist",
            deadzone: 0.04,
            filter_alpha: Some(0.15),
            slew_rate: None,
            notes: "Twist rudder — slightly larger deadzone for centering",
        },
        RecommendedAxisConfig {
            name: "throttle_left",
            deadzone: 0.01,
            filter_alpha: Some(0.10),
            slew_rate: Some(2.0),
            notes: "Left throttle lever — 8-bit pot, slew-limited for smoothness",
        },
        RecommendedAxisConfig {
            name: "throttle_right",
            deadzone: 0.01,
            filter_alpha: Some(0.10),
            slew_rate: Some(2.0),
            notes: "Right throttle lever — 8-bit pot, slew-limited for smoothness",
        },
        RecommendedAxisConfig {
            name: "trim_wheel",
            deadzone: 0.0,
            filter_alpha: None,
            slew_rate: None,
            notes: "Trim wheel — delta-encoded, no deadzone or filtering needed",
        },
    ]
}

// ── VelocityOne Flightstick presets ──────────────────────────────────────────

/// Recommended axis configurations for the VelocityOne Flightstick.
///
/// The Flightstick has 12-bit Hall-effect sensors on all stick axes and an
/// 8-bit potentiometer on the throttle slider.
pub fn flightstick_axis_config() -> [RecommendedAxisConfig; 4] {
    [
        RecommendedAxisConfig {
            name: "x",
            deadzone: 0.03,
            filter_alpha: Some(0.12),
            slew_rate: None,
            notes: "Stick X — Hall-effect, moderate deadzone for centering",
        },
        RecommendedAxisConfig {
            name: "y",
            deadzone: 0.03,
            filter_alpha: Some(0.12),
            slew_rate: None,
            notes: "Stick Y — Hall-effect, moderate deadzone for centering",
        },
        RecommendedAxisConfig {
            name: "twist",
            deadzone: 0.05,
            filter_alpha: Some(0.15),
            slew_rate: None,
            notes: "Twist rudder — larger deadzone to avoid unintended yaw",
        },
        RecommendedAxisConfig {
            name: "throttle",
            deadzone: 0.02,
            filter_alpha: Some(0.10),
            slew_rate: Some(2.0),
            notes: "Throttle slider — 8-bit pot, slew-limited for smoothness",
        },
    ]
}

// ── VelocityOne Rudder presets ───────────────────────────────────────────────

/// Recommended axis configurations for the VelocityOne Rudder pedals.
///
/// The Rudder pedals use 12-bit Hall-effect sensors on all three axes.
pub fn rudder_axis_config() -> [RecommendedAxisConfig; 3] {
    [
        RecommendedAxisConfig {
            name: "rudder",
            deadzone: 0.03,
            filter_alpha: Some(0.12),
            slew_rate: None,
            notes: "Rudder axis — Hall-effect, centred bipolar",
        },
        RecommendedAxisConfig {
            name: "left_toe_brake",
            deadzone: 0.02,
            filter_alpha: Some(0.10),
            slew_rate: None,
            notes: "Left toe brake — Hall-effect, unipolar",
        },
        RecommendedAxisConfig {
            name: "right_toe_brake",
            deadzone: 0.02,
            filter_alpha: Some(0.10),
            slew_rate: None,
            notes: "Right toe brake — Hall-effect, unipolar",
        },
    ]
}

// ── Axis name constants ──────────────────────────────────────────────────────

/// Axis name constants for the VelocityOne Flight.
pub mod flight_axes {
    pub const ROLL: &str = "roll";
    pub const PITCH: &str = "pitch";
    pub const RUDDER_TWIST: &str = "rudder_twist";
    pub const THROTTLE_LEFT: &str = "throttle_left";
    pub const THROTTLE_RIGHT: &str = "throttle_right";
    pub const TRIM_WHEEL: &str = "trim_wheel";
}

/// All axis names for the VelocityOne Flight, in report order.
pub const FLIGHT_AXIS_NAMES: &[&str] = &[
    flight_axes::ROLL,
    flight_axes::PITCH,
    flight_axes::RUDDER_TWIST,
    flight_axes::THROTTLE_LEFT,
    flight_axes::THROTTLE_RIGHT,
    flight_axes::TRIM_WHEEL,
];

/// Axis name constants for the VelocityOne Flightstick.
pub mod flightstick_axes {
    pub const X: &str = "x";
    pub const Y: &str = "y";
    pub const TWIST: &str = "twist";
    pub const THROTTLE: &str = "throttle";
}

/// All axis names for the VelocityOne Flightstick, in report order.
pub const FLIGHTSTICK_AXIS_NAMES: &[&str] = &[
    flightstick_axes::X,
    flightstick_axes::Y,
    flightstick_axes::TWIST,
    flightstick_axes::THROTTLE,
];

/// Axis name constants for the VelocityOne Rudder.
pub mod rudder_axes {
    pub const RUDDER: &str = "rudder";
    pub const LEFT_TOE_BRAKE: &str = "left_toe_brake";
    pub const RIGHT_TOE_BRAKE: &str = "right_toe_brake";
}

/// All axis names for the VelocityOne Rudder, in report order.
pub const RUDDER_AXIS_NAMES: &[&str] = &[
    rudder_axes::RUDDER,
    rudder_axes::LEFT_TOE_BRAKE,
    rudder_axes::RIGHT_TOE_BRAKE,
];

// ── Preset lookup ────────────────────────────────────────────────────────────

/// Look up recommended axis configurations by device model.
///
/// Returns a `Vec` because different devices have different axis counts.
/// For devices without dedicated presets (Flight Pro, Flight Universal,
/// Flight Yoke), the Flight presets are used as a baseline.
pub fn preset_for_device(device: crate::devices::VelocityOneDevice) -> Vec<RecommendedAxisConfig> {
    use crate::devices::VelocityOneDevice;
    match device {
        VelocityOneDevice::Flight
        | VelocityOneDevice::FlightPro
        | VelocityOneDevice::FlightUniversal
        | VelocityOneDevice::FlightYoke => flight_axis_config().to_vec(),
        VelocityOneDevice::Flightstick => flightstick_axis_config().to_vec(),
        VelocityOneDevice::Rudder => rudder_axis_config().to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::VelocityOneDevice;

    #[test]
    fn test_flight_preset_count() {
        let config = flight_axis_config();
        assert_eq!(config.len(), 6);
    }

    #[test]
    fn test_flightstick_preset_count() {
        let config = flightstick_axis_config();
        assert_eq!(config.len(), 4);
    }

    #[test]
    fn test_rudder_preset_count() {
        let config = rudder_axis_config();
        assert_eq!(config.len(), 3);
    }

    #[test]
    fn test_flight_axis_names_match_preset_names() {
        let config = flight_axis_config();
        let names: Vec<&str> = config.iter().map(|c| c.name).collect();
        assert_eq!(names.as_slice(), FLIGHT_AXIS_NAMES);
    }

    #[test]
    fn test_flightstick_axis_names_match_preset_names() {
        let config = flightstick_axis_config();
        let names: Vec<&str> = config.iter().map(|c| c.name).collect();
        assert_eq!(names.as_slice(), FLIGHTSTICK_AXIS_NAMES);
    }

    #[test]
    fn test_rudder_axis_names_match_preset_names() {
        let config = rudder_axis_config();
        let names: Vec<&str> = config.iter().map(|c| c.name).collect();
        assert_eq!(names.as_slice(), RUDDER_AXIS_NAMES);
    }

    #[test]
    fn test_all_deadzones_in_range() {
        for device in VelocityOneDevice::all() {
            for axis in preset_for_device(*device) {
                assert!(
                    axis.deadzone >= 0.0 && axis.deadzone <= 0.2,
                    "{}: deadzone {} out of range for {}",
                    device.name(),
                    axis.deadzone,
                    axis.name
                );
            }
        }
    }

    #[test]
    fn test_all_filter_alphas_in_range() {
        for device in VelocityOneDevice::all() {
            for axis in preset_for_device(*device) {
                if let Some(alpha) = axis.filter_alpha {
                    assert!(
                        alpha > 0.0 && alpha <= 1.0,
                        "{}: filter alpha {} out of range for {}",
                        device.name(),
                        alpha,
                        axis.name
                    );
                }
            }
        }
    }

    #[test]
    fn test_all_slew_rates_positive() {
        for device in VelocityOneDevice::all() {
            for axis in preset_for_device(*device) {
                if let Some(slew) = axis.slew_rate {
                    assert!(
                        slew > 0.0,
                        "{}: slew rate must be positive for {}",
                        device.name(),
                        axis.name
                    );
                }
            }
        }
    }

    #[test]
    fn test_all_presets_have_notes() {
        for device in VelocityOneDevice::all() {
            for axis in preset_for_device(*device) {
                assert!(
                    !axis.notes.is_empty(),
                    "{}: notes must not be empty for {}",
                    device.name(),
                    axis.name
                );
            }
        }
    }

    #[test]
    fn test_preset_for_device_flight_pro_uses_flight() {
        let flight = preset_for_device(VelocityOneDevice::Flight);
        let pro = preset_for_device(VelocityOneDevice::FlightPro);
        assert_eq!(flight, pro);
    }

    #[test]
    fn test_preset_for_device_universal_uses_flight() {
        let flight = preset_for_device(VelocityOneDevice::Flight);
        let universal = preset_for_device(VelocityOneDevice::FlightUniversal);
        assert_eq!(flight, universal);
    }

    #[test]
    fn test_trim_wheel_has_no_deadzone_or_filter() {
        let config = flight_axis_config();
        let trim = config.iter().find(|c| c.name == "trim_wheel").unwrap();
        assert_eq!(trim.deadzone, 0.0);
        assert!(trim.filter_alpha.is_none());
        assert!(trim.slew_rate.is_none());
    }

    #[test]
    fn test_throttle_axes_have_slew_rate() {
        let config = flight_axis_config();
        for axis in &config {
            if axis.name.contains("throttle") {
                assert!(
                    axis.slew_rate.is_some(),
                    "throttle axis {} should have a slew rate",
                    axis.name
                );
            }
        }
    }
}
