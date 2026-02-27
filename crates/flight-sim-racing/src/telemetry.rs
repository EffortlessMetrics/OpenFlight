// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Normalised racing telemetry shared across all sim racing adapters.

/// Normalised racing telemetry, populated by any racing telemetry adapter.
#[derive(Debug, Clone, PartialEq)]
pub struct RacingTelemetry {
    /// Vehicle speed in metres per second.
    pub speed_ms: f32,
    /// Lateral G-force (negative = left turn, positive = right turn). Typical range: −5.0 – 5.0.
    pub lateral_g: f32,
    /// Longitudinal G-force (negative = braking, positive = acceleration).
    pub longitudinal_g: f32,
    /// Vertical G-force from bumps and kerbs.
    pub vertical_g: f32,
    /// Throttle pedal position, normalised to `0.0` (released) – `1.0` (fully depressed).
    pub throttle: f32,
    /// Brake pedal position, normalised to `0.0` (released) – `1.0` (fully depressed).
    pub brake: f32,
    /// Steering wheel angle, normalised to `−1.0` (full left) – `1.0` (full right).
    pub steering_angle: f32,
    /// Current gear: `−1` = reverse, `0` = neutral, `1`–`8` = forward gears.
    pub gear: i8,
    /// Engine RPM.
    pub rpm: f32,
    /// Engine redline / maximum RPM.
    pub rpm_max: f32,
    /// `true` when the vehicle is on the racing surface.
    pub is_on_track: bool,
    /// `true` when this telemetry sample contains trustworthy data.
    pub is_valid: bool,
}

impl Default for RacingTelemetry {
    fn default() -> Self {
        Self {
            speed_ms: 0.0,
            lateral_g: 0.0,
            longitudinal_g: 0.0,
            vertical_g: 0.0,
            throttle: 0.0,
            brake: 0.0,
            steering_angle: 0.0,
            gear: 0,
            rpm: 0.0,
            rpm_max: 0.0,
            is_on_track: false,
            is_valid: false,
        }
    }
}

impl RacingTelemetry {
    /// Engine RPM normalised against the redline, in the range `0.0` – `1.0`.
    pub fn rpm_normalized(&self) -> f32 {
        self.rpm / self.rpm_max.max(1.0)
    }

    /// `true` when the brake pedal is depressed beyond the dead-zone threshold.
    pub fn is_braking(&self) -> bool {
        self.brake > 0.05
    }

    /// `true` when the throttle pedal is depressed beyond the dead-zone threshold.
    pub fn is_accelerating(&self) -> bool {
        self.throttle > 0.05
    }
}
