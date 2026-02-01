// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Recommended axis configurations for T.Flight HOTAS devices.
//!
//! These presets are tuned for the B104 (100kΩ linear) potentiometers
//! used in T.Flight HOTAS 4, which are known to exhibit jitter.

/// Recommended axis configuration with tuned parameters.
#[derive(Debug, Clone)]
pub struct RecommendedAxisConfig {
    /// Axis name (e.g., "roll", "pitch", "yaw", "throttle")
    pub name: &'static str,
    /// Recommended deadzone value
    pub deadzone: f32,
    /// Recommended EMA filter alpha (None = no filtering)
    pub filter_alpha: Option<f32>,
    /// Recommended slew rate limit (None = no limit)
    pub slew_rate: Option<f32>,
    /// Notes about this axis configuration
    pub notes: &'static str,
}

/// Get recommended axis configurations for T.Flight HOTAS 4.
///
/// These configurations are tuned for the B104 potentiometer noise
/// characteristics and provide a good balance between responsiveness
/// and smoothness.
///
/// # Example
///
/// ```
/// use flight_hotas_thrustmaster::recommended_axis_config;
///
/// for config in recommended_axis_config() {
///     println!("{}: deadzone={}, filter_alpha={:?}",
///         config.name, config.deadzone, config.filter_alpha);
/// }
/// ```
pub fn recommended_axis_config() -> [RecommendedAxisConfig; 4] {
    [
        RecommendedAxisConfig {
            name: "roll",
            deadzone: 0.05,
            filter_alpha: Some(0.15),
            slew_rate: None,
            notes: "X axis - primary flight control, moderate filtering",
        },
        RecommendedAxisConfig {
            name: "pitch",
            deadzone: 0.05,
            filter_alpha: Some(0.15),
            slew_rate: None,
            notes: "Y axis - primary flight control, moderate filtering",
        },
        RecommendedAxisConfig {
            name: "yaw",
            deadzone: 0.08,
            filter_alpha: Some(0.15),
            slew_rate: None,
            notes: "Twist/Rz axis - larger deadzone due to spring centering",
        },
        RecommendedAxisConfig {
            name: "throttle",
            deadzone: 0.02,
            filter_alpha: Some(0.10),
            slew_rate: Some(2.0),
            notes: "Throttle - minimal deadzone, heavier filtering, slew limit for smoothness",
        },
    ]
}

/// Get the B104 potentiometer filter preset parameters.
///
/// These values are optimized for the B104 (100kΩ linear) potentiometers
/// used in T.Flight HOTAS 4:
///
/// - **alpha**: 0.15 - Provides moderate smoothing without excessive latency
/// - **spike_threshold**: 0.4 - Rejects large transient noise spikes
/// - **max_spike_count**: 5 - Allows sustained real changes to pass through
pub fn b104_filter_params() -> (f32, f32, u8) {
    (0.15, 0.4, 5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recommended_configs() {
        let configs = recommended_axis_config();

        assert_eq!(configs.len(), 4);

        // Check that all axes are covered
        let names: Vec<_> = configs.iter().map(|c| c.name).collect();
        assert!(names.contains(&"roll"));
        assert!(names.contains(&"pitch"));
        assert!(names.contains(&"yaw"));
        assert!(names.contains(&"throttle"));
    }

    #[test]
    fn test_deadzone_values_reasonable() {
        let configs = recommended_axis_config();

        for config in &configs {
            assert!(
                config.deadzone >= 0.0,
                "{} deadzone should be non-negative",
                config.name
            );
            assert!(
                config.deadzone <= 0.2,
                "{} deadzone should not be too large",
                config.name
            );
        }
    }

    #[test]
    fn test_filter_alpha_values_reasonable() {
        let configs = recommended_axis_config();

        for config in &configs {
            if let Some(alpha) = config.filter_alpha {
                assert!(
                    alpha > 0.0 && alpha <= 1.0,
                    "{} filter alpha should be in (0, 1]",
                    config.name
                );
            }
        }
    }

    #[test]
    fn test_b104_params() {
        let (alpha, spike_threshold, max_spike_count) = b104_filter_params();

        assert!(alpha > 0.0 && alpha <= 1.0);
        assert!(spike_threshold > 0.0 && spike_threshold <= 1.0);
        assert!(max_spike_count > 0);
    }
}
