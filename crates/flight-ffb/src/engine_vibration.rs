// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Engine vibration force synthesis for force feedback (REQ-828)
//!
//! Generates periodic vibration forces based on engine RPM, cylinder count,
//! and a roughness parameter. The vibration frequency is derived from the
//! engine firing rate and the amplitude scales with RPM.

/// Configuration for engine vibration synthesis.
#[derive(Debug, Clone)]
pub struct EngineVibrationConfig {
    /// Number of cylinders in the engine.
    pub cylinders: u32,
    /// RPM at idle.
    pub idle_rpm: f32,
    /// RPM at redline.
    pub redline_rpm: f32,
    /// Minimum vibration amplitude at idle (0.0–1.0).
    pub idle_amplitude: f32,
    /// Maximum vibration amplitude at redline (0.0–1.0).
    pub redline_amplitude: f32,
}

impl Default for EngineVibrationConfig {
    fn default() -> Self {
        Self {
            cylinders: 4,
            idle_rpm: 600.0,
            redline_rpm: 2700.0,
            idle_amplitude: 0.05,
            redline_amplitude: 0.6,
        }
    }
}

/// Output of the engine vibration computation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EngineVibrationOutput {
    /// Vibration frequency in Hz.
    pub frequency_hz: f32,
    /// Vibration amplitude (0.0–1.0).
    pub amplitude: f32,
}

/// Computes engine vibration frequency and amplitude.
///
/// # Arguments
///
/// * `rpm` — Current engine RPM.
/// * `roughness` — Additional roughness factor (`0.0` = smooth, `1.0` = very
///   rough). Models partial-power vibrations and engine wear.
/// * `config` — Engine-specific [`EngineVibrationConfig`].
///
/// # Returns
///
/// An [`EngineVibrationOutput`] with the resulting frequency and amplitude.
/// If RPM is zero or negative the output is zeroed.
pub fn compute_engine_vibration(
    rpm: f32,
    roughness: f32,
    config: &EngineVibrationConfig,
) -> EngineVibrationOutput {
    if rpm <= 0.0 || config.cylinders == 0 {
        return EngineVibrationOutput {
            frequency_hz: 0.0,
            amplitude: 0.0,
        };
    }

    // Firing frequency: RPM * cylinders / 120
    let frequency_hz = rpm * config.cylinders as f32 / 120.0;

    // Normalised RPM position between idle and redline
    let rpm_range = (config.redline_rpm - config.idle_rpm).max(1.0);
    let rpm_norm = ((rpm - config.idle_rpm) / rpm_range).clamp(0.0, 1.0);

    // Linear interpolation of amplitude between idle and redline
    let base_amplitude =
        config.idle_amplitude + (config.redline_amplitude - config.idle_amplitude) * rpm_norm;

    // Roughness adds up to 30% additional amplitude
    let roughness_clamped = roughness.clamp(0.0, 1.0);
    let amplitude = (base_amplitude + roughness_clamped * 0.3).clamp(0.0, 1.0);

    EngineVibrationOutput {
        frequency_hz,
        amplitude,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_cfg() -> EngineVibrationConfig {
        EngineVibrationConfig::default()
    }

    #[test]
    fn zero_rpm_gives_zero_output() {
        let out = compute_engine_vibration(0.0, 0.0, &default_cfg());
        assert_eq!(out.frequency_hz, 0.0);
        assert_eq!(out.amplitude, 0.0);
    }

    #[test]
    fn idle_rpm_gives_idle_amplitude() {
        let cfg = default_cfg();
        let out = compute_engine_vibration(cfg.idle_rpm, 0.0, &cfg);
        assert!(
            (out.amplitude - cfg.idle_amplitude).abs() < 1e-4,
            "idle amplitude mismatch: {}",
            out.amplitude
        );
    }

    #[test]
    fn redline_rpm_gives_redline_amplitude() {
        let cfg = default_cfg();
        let out = compute_engine_vibration(cfg.redline_rpm, 0.0, &cfg);
        assert!(
            (out.amplitude - cfg.redline_amplitude).abs() < 1e-4,
            "redline amplitude mismatch: {}",
            out.amplitude
        );
    }

    #[test]
    fn frequency_formula_correct() {
        let cfg = default_cfg();
        let out = compute_engine_vibration(2400.0, 0.0, &cfg);
        let expected = 2400.0 * 4.0 / 120.0; // 80 Hz
        assert!(
            (out.frequency_hz - expected).abs() < 1e-3,
            "frequency mismatch: expected {expected}, got {}",
            out.frequency_hz
        );
    }

    #[test]
    fn roughness_increases_amplitude() {
        let cfg = default_cfg();
        let smooth = compute_engine_vibration(1500.0, 0.0, &cfg);
        let rough = compute_engine_vibration(1500.0, 1.0, &cfg);
        assert!(
            rough.amplitude > smooth.amplitude,
            "roughness should increase amplitude: smooth={}, rough={}",
            smooth.amplitude,
            rough.amplitude
        );
    }

    #[test]
    fn amplitude_clamped_to_one() {
        let cfg = EngineVibrationConfig {
            redline_amplitude: 0.95,
            ..default_cfg()
        };
        let out = compute_engine_vibration(cfg.redline_rpm, 1.0, &cfg);
        assert!(out.amplitude <= 1.0, "amplitude must not exceed 1.0: {}", out.amplitude);
    }

    #[test]
    fn negative_rpm_gives_zero_output() {
        let out = compute_engine_vibration(-500.0, 0.0, &default_cfg());
        assert_eq!(out.frequency_hz, 0.0);
        assert_eq!(out.amplitude, 0.0);
    }
}
