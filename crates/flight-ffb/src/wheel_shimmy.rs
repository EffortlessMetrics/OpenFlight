// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Nose-wheel shimmy force simulation for force feedback (REQ-842)
//!
//! Generates an oscillating rudder-axis force that simulates nose-wheel
//! shimmy during ground roll. The effect activates when on the ground
//! with speed above 10 kts and the nose wheel deflected, and damps out
//! above approximately 60 kts as the nose wheel unloads.

/// Configuration for wheel shimmy simulation.
#[derive(Debug, Clone)]
pub struct WheelShimmyConfig {
    /// Minimum ground speed (knots) at which shimmy begins.
    pub min_speed_kts: f32,
    /// Ground speed (knots) above which shimmy damps out.
    pub damp_speed_kts: f32,
    /// Maximum shimmy amplitude (0.0–1.0).
    pub max_amplitude: f32,
    /// Base shimmy frequency in Hz at `min_speed_kts`.
    pub base_frequency_hz: f32,
    /// Frequency increase per knot of ground speed.
    pub freq_per_knot: f32,
}

impl Default for WheelShimmyConfig {
    fn default() -> Self {
        Self {
            min_speed_kts: 10.0,
            damp_speed_kts: 60.0,
            max_amplitude: 0.4,
            base_frequency_hz: 8.0,
            freq_per_knot: 0.15,
        }
    }
}

/// Output of the wheel shimmy computation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WheelShimmyOutput {
    /// Shimmy oscillation frequency in Hz.
    pub frequency_hz: f32,
    /// Shimmy force amplitude (0.0–1.0) for the rudder axis.
    pub amplitude: f32,
}

/// Computes nose-wheel shimmy force parameters.
///
/// # Arguments
///
/// * `ground_speed_kts` — Aircraft ground speed in **knots**.
/// * `nose_wheel_angle_deg` — Nose-wheel steering angle in **degrees**
///   from centre. Positive or negative; only the magnitude matters.
/// * `on_ground` — `true` when the aircraft weight is on the wheels.
/// * `config` — Shimmy-specific [`WheelShimmyConfig`].
///
/// # Returns
///
/// A [`WheelShimmyOutput`] with frequency and amplitude. Both are zero
/// when shimmy conditions are not met.
pub fn compute_wheel_shimmy(
    ground_speed_kts: f32,
    nose_wheel_angle_deg: f32,
    on_ground: bool,
    config: &WheelShimmyConfig,
) -> WheelShimmyOutput {
    let zero = WheelShimmyOutput {
        frequency_hz: 0.0,
        amplitude: 0.0,
    };

    // Must be on ground, above minimum speed, and nose wheel deflected
    if !on_ground || ground_speed_kts < config.min_speed_kts || nose_wheel_angle_deg.abs() < 0.5 {
        return zero;
    }

    // Frequency increases with ground speed
    let speed_above_min = ground_speed_kts - config.min_speed_kts;
    let frequency_hz = config.base_frequency_hz + speed_above_min * config.freq_per_knot;

    // Amplitude builds with speed but damps above damp_speed_kts
    let speed_factor = if ground_speed_kts >= config.damp_speed_kts {
        // Above damp speed: rapid falloff
        let over = ground_speed_kts - config.damp_speed_kts;
        (1.0 - over / 20.0).max(0.0)
    } else {
        // Build up: linear ramp from min to damp
        let range = (config.damp_speed_kts - config.min_speed_kts).max(1.0);
        ((ground_speed_kts - config.min_speed_kts) / range).clamp(0.0, 1.0)
    };

    // Nose-wheel angle factor (more deflection = more shimmy, capped at 15°)
    let angle_factor = (nose_wheel_angle_deg.abs() / 15.0).clamp(0.0, 1.0);

    let amplitude = (config.max_amplitude * speed_factor * angle_factor).clamp(0.0, 1.0);

    WheelShimmyOutput {
        frequency_hz,
        amplitude,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_cfg() -> WheelShimmyConfig {
        WheelShimmyConfig::default()
    }

    #[test]
    fn no_shimmy_in_air() {
        let out = compute_wheel_shimmy(40.0, 5.0, false, &default_cfg());
        assert_eq!(out.amplitude, 0.0);
    }

    #[test]
    fn no_shimmy_below_min_speed() {
        let out = compute_wheel_shimmy(5.0, 10.0, true, &default_cfg());
        assert_eq!(out.amplitude, 0.0);
    }

    #[test]
    fn no_shimmy_with_centred_wheel() {
        let out = compute_wheel_shimmy(30.0, 0.0, true, &default_cfg());
        assert_eq!(out.amplitude, 0.0);
    }

    #[test]
    fn shimmy_active_at_moderate_speed_with_deflection() {
        let out = compute_wheel_shimmy(35.0, 8.0, true, &default_cfg());
        assert!(out.amplitude > 0.0, "shimmy should be active");
        assert!(out.frequency_hz > 0.0, "frequency should be positive");
    }

    #[test]
    fn shimmy_damps_above_damp_speed() {
        let cfg = default_cfg();
        let at_damp = compute_wheel_shimmy(cfg.damp_speed_kts, 10.0, true, &cfg);
        let well_above = compute_wheel_shimmy(cfg.damp_speed_kts + 25.0, 10.0, true, &cfg);
        assert!(
            well_above.amplitude < at_damp.amplitude,
            "shimmy should damp above {} kts: at_damp={}, well_above={}",
            cfg.damp_speed_kts,
            at_damp.amplitude,
            well_above.amplitude
        );
    }

    #[test]
    fn amplitude_increases_with_speed_below_damp() {
        let cfg = default_cfg();
        let slow = compute_wheel_shimmy(15.0, 10.0, true, &cfg);
        let faster = compute_wheel_shimmy(40.0, 10.0, true, &cfg);
        assert!(
            faster.amplitude > slow.amplitude,
            "amplitude should increase with speed: slow={}, faster={}",
            slow.amplitude,
            faster.amplitude
        );
    }

    #[test]
    fn frequency_increases_with_speed() {
        let cfg = default_cfg();
        let slow = compute_wheel_shimmy(15.0, 5.0, true, &cfg);
        let fast = compute_wheel_shimmy(50.0, 5.0, true, &cfg);
        assert!(
            fast.frequency_hz > slow.frequency_hz,
            "frequency should increase with speed"
        );
    }
}
