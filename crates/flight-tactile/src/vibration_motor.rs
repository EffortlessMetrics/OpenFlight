// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Vibration motor driver abstraction
//!
//! Provides [`VibrationMotor`] which translates a desired intensity
//! (0.0–1.0) into a concrete [`MotorCommand`] respecting the motor's
//! physical dead-zone, frequency range, and response curve.

use serde::{Deserialize, Serialize};

// ── Motor type ───────────────────────────────────────────────────────

/// Physical motor technology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MotorType {
    /// Eccentric rotating mass (ERM) motor.
    Rotary,
    /// Linear resonant actuator (LRA).
    Linear,
    /// Piezoelectric actuator.
    Piezo,
}

// ── Response curve ───────────────────────────────────────────────────

/// Maps logical intensity to physical duty-cycle / amplitude.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ResponseCurve {
    /// Output = input.
    Linear,
    /// Output = input² — softer low end, sharper high end.
    Quadratic,
    /// Output = √input — more responsive at low intensities.
    SquareRoot,
}

impl ResponseCurve {
    /// Apply the curve to a normalised input (0.0–1.0).
    #[inline]
    pub fn apply(self, input: f64) -> f64 {
        let v = input.clamp(0.0, 1.0);
        match self {
            Self::Linear => v,
            Self::Quadratic => v * v,
            Self::SquareRoot => v.sqrt(),
        }
    }
}

// ── Motor config ─────────────────────────────────────────────────────

/// Configuration describing a vibration motor's physical capabilities.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MotorConfig {
    /// Physical actuator type.
    pub motor_type: MotorType,
    /// Minimum operating frequency (Hz).
    pub min_frequency_hz: f64,
    /// Maximum operating frequency (Hz).
    pub max_frequency_hz: f64,
    /// Intensity-to-output mapping.
    pub response_curve: ResponseCurve,
    /// Dead-zone threshold (0.0–1.0). Inputs below this yield no output.
    pub dead_zone: f64,
}

impl Default for MotorConfig {
    fn default() -> Self {
        Self {
            motor_type: MotorType::Linear,
            min_frequency_hz: 20.0,
            max_frequency_hz: 200.0,
            response_curve: ResponseCurve::Linear,
            dead_zone: 0.05,
        }
    }
}

// ── Motor command ────────────────────────────────────────────────────

/// Rotation / oscillation direction for bi-directional motors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MotorDirection {
    Forward,
    Reverse,
}

/// Concrete command sent to a motor driver.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MotorCommand {
    /// PWM duty cycle 0.0–1.0.
    pub duty_cycle: f64,
    /// Drive frequency in Hz.
    pub frequency: f64,
    /// Oscillation direction.
    pub direction: MotorDirection,
}

impl MotorCommand {
    /// A stopped-motor command.
    pub fn off() -> Self {
        Self {
            duty_cycle: 0.0,
            frequency: 0.0,
            direction: MotorDirection::Forward,
        }
    }
}

// ── VibrationMotor ───────────────────────────────────────────────────

/// Translates a desired intensity into a hardware [`MotorCommand`].
pub struct VibrationMotor {
    config: MotorConfig,
}

impl VibrationMotor {
    /// Create a motor with the given configuration.
    pub fn new(config: MotorConfig) -> Self {
        Self { config }
    }

    /// Get the motor configuration.
    pub fn config(&self) -> &MotorConfig {
        &self.config
    }

    /// Convert a desired `intensity` (0.0–1.0) into a [`MotorCommand`].
    ///
    /// * Intensities below the dead-zone return [`MotorCommand::off`].
    /// * The response curve shapes the duty cycle.
    /// * Frequency is linearly interpolated across the motor's range.
    pub fn apply(&self, intensity: f64) -> MotorCommand {
        let intensity = intensity.clamp(0.0, 1.0);

        if intensity <= self.config.dead_zone {
            return MotorCommand::off();
        }

        // Re-scale intensity so that the dead-zone boundary maps to 0.0
        let usable_range = 1.0 - self.config.dead_zone;
        let scaled = if usable_range > 0.0 {
            (intensity - self.config.dead_zone) / usable_range
        } else {
            1.0
        };

        let duty_cycle = self.config.response_curve.apply(scaled);

        let freq_range = self.config.max_frequency_hz - self.config.min_frequency_hz;
        let frequency = self.config.min_frequency_hz + freq_range * scaled;
        let frequency = frequency.clamp(self.config.min_frequency_hz, self.config.max_frequency_hz);

        MotorCommand {
            duty_cycle,
            frequency,
            direction: MotorDirection::Forward,
        }
    }

    /// Apply with an explicit direction.
    pub fn apply_directed(&self, intensity: f64, direction: MotorDirection) -> MotorCommand {
        let mut cmd = self.apply(intensity);
        cmd.direction = direction;
        cmd
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn default_motor() -> VibrationMotor {
        VibrationMotor::new(MotorConfig::default())
    }

    // ── basic apply ─────────────────────────────────────────────────

    #[test]
    fn test_zero_intensity_returns_off() {
        let motor = default_motor();
        let cmd = motor.apply(0.0);
        assert_eq!(cmd.duty_cycle, 0.0);
        assert_eq!(cmd.frequency, 0.0);
    }

    #[test]
    fn test_full_intensity() {
        let motor = default_motor();
        let cmd = motor.apply(1.0);
        assert!(cmd.duty_cycle > 0.9, "full intensity ≈ full duty cycle");
        assert!(
            (cmd.frequency - 200.0).abs() < 1e-6,
            "should reach max freq"
        );
    }

    #[test]
    fn test_mid_intensity_produces_output() {
        let motor = default_motor();
        let cmd = motor.apply(0.5);
        assert!(cmd.duty_cycle > 0.0);
        assert!(cmd.frequency >= 20.0);
        assert!(cmd.frequency <= 200.0);
    }

    // ── dead-zone ───────────────────────────────────────────────────

    #[test]
    fn test_below_dead_zone_is_off() {
        let motor = VibrationMotor::new(MotorConfig {
            dead_zone: 0.1,
            ..Default::default()
        });
        let cmd = motor.apply(0.09);
        assert_eq!(cmd.duty_cycle, 0.0);
        assert_eq!(cmd.frequency, 0.0);
    }

    #[test]
    fn test_at_dead_zone_boundary() {
        let motor = VibrationMotor::new(MotorConfig {
            dead_zone: 0.1,
            ..Default::default()
        });
        let cmd = motor.apply(0.1);
        // Exactly at dead-zone → off (below threshold)
        assert_eq!(cmd.duty_cycle, 0.0);
    }

    #[test]
    fn test_just_above_dead_zone() {
        let motor = VibrationMotor::new(MotorConfig {
            dead_zone: 0.1,
            ..Default::default()
        });
        let cmd = motor.apply(0.15);
        assert!(
            cmd.duty_cycle > 0.0,
            "above dead-zone should produce output"
        );
    }

    // ── response curves ─────────────────────────────────────────────

    #[test]
    fn test_linear_curve() {
        assert!((ResponseCurve::Linear.apply(0.5) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_quadratic_curve() {
        assert!((ResponseCurve::Quadratic.apply(0.5) - 0.25).abs() < 1e-9);
    }

    #[test]
    fn test_sqrt_curve() {
        let val = ResponseCurve::SquareRoot.apply(0.25);
        assert!((val - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_curve_clamping() {
        assert_eq!(ResponseCurve::Linear.apply(-1.0), 0.0);
        assert_eq!(ResponseCurve::Linear.apply(2.0), 1.0);
    }

    #[test]
    fn test_quadratic_motor_low_intensity() {
        let motor = VibrationMotor::new(MotorConfig {
            response_curve: ResponseCurve::Quadratic,
            dead_zone: 0.0,
            ..Default::default()
        });
        let cmd = motor.apply(0.5);
        assert!(
            cmd.duty_cycle < 0.3,
            "quadratic should produce low duty at mid input"
        );
    }

    // ── frequency clamping ──────────────────────────────────────────

    #[test]
    fn test_frequency_within_range() {
        let motor = default_motor();
        for i in 0..=100 {
            let cmd = motor.apply(i as f64 / 100.0);
            if cmd.duty_cycle > 0.0 {
                assert!(cmd.frequency >= 20.0);
                assert!(cmd.frequency <= 200.0);
            }
        }
    }

    // ── direction ───────────────────────────────────────────────────

    #[test]
    fn test_apply_directed() {
        let motor = default_motor();
        let cmd = motor.apply_directed(0.8, MotorDirection::Reverse);
        assert_eq!(cmd.direction, MotorDirection::Reverse);
        assert!(cmd.duty_cycle > 0.0);
    }

    // ── motor types ─────────────────────────────────────────────────

    #[test]
    fn test_piezo_config() {
        let motor = VibrationMotor::new(MotorConfig {
            motor_type: MotorType::Piezo,
            min_frequency_hz: 100.0,
            max_frequency_hz: 400.0,
            response_curve: ResponseCurve::SquareRoot,
            dead_zone: 0.02,
        });
        let cmd = motor.apply(1.0);
        assert!((cmd.frequency - 400.0).abs() < 1e-6);
        assert!((cmd.duty_cycle - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_rotary_config() {
        let motor = VibrationMotor::new(MotorConfig {
            motor_type: MotorType::Rotary,
            min_frequency_hz: 10.0,
            max_frequency_hz: 100.0,
            response_curve: ResponseCurve::Linear,
            dead_zone: 0.15,
        });
        let cmd = motor.apply(0.1);
        assert_eq!(cmd.duty_cycle, 0.0, "below dead-zone");
    }

    // ── MotorCommand::off ───────────────────────────────────────────

    #[test]
    fn test_motor_command_off() {
        let cmd = MotorCommand::off();
        assert_eq!(cmd.duty_cycle, 0.0);
        assert_eq!(cmd.frequency, 0.0);
        assert_eq!(cmd.direction, MotorDirection::Forward);
    }

    // ── intensity clamping ──────────────────────────────────────────

    #[test]
    fn test_negative_intensity_clamped() {
        let motor = default_motor();
        let cmd = motor.apply(-1.0);
        assert_eq!(cmd.duty_cycle, 0.0);
    }

    #[test]
    fn test_over_one_intensity_clamped() {
        let motor = default_motor();
        let cmd = motor.apply(5.0);
        assert!(cmd.duty_cycle <= 1.0);
        assert!(cmd.frequency <= 200.0);
    }
}
