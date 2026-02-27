// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! `FlightData` structure matching the Falcon BMS shared memory layout.

use std::f32::consts;

/// Primary flight data polled from the `BMS-Data` shared memory segment.
///
/// The layout is a simplified subset of the BMS SDK `FlightData` structure.
/// Fields occupy the first 68 bytes; the remainder is padding to approximate
/// the real 800-byte block size.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FlightData {
    /// 3D position x (ft)
    pub x: f32,
    /// 3D position y (ft)
    pub y: f32,
    /// 3D position z (ft, negative = up)
    pub z: f32,
    /// Velocity x component
    pub x_dot: f32,
    /// Velocity y component
    pub y_dot: f32,
    /// Velocity z component
    pub z_dot: f32,
    /// Angle of attack (radians)
    pub alpha: f32,
    /// Sideslip angle (radians)
    pub beta: f32,
    /// Flight path angle (radians)
    pub gamma: f32,
    /// Aircraft pitch (radians)
    pub pitch: f32,
    /// Aircraft roll (radians)
    pub roll: f32,
    /// Aircraft yaw (radians)
    pub yaw: f32,
    /// Mach number
    pub mach: f32,
    /// Calibrated airspeed (knots)
    pub cas: f32,
    /// MSL altitude (ft)
    pub alt: f32,
    /// Throttle position (0.0–1.0)
    pub throttle: f32,
    /// Engine RPM (0.0–1.0)
    pub rpm: f32,
    /// Padding to approximate the real BMS struct size
    pub _pad: [u8; 700],
}

impl FlightData {
    /// Pitch normalised to \[-1.0, 1.0\] (±π → ±1.0).
    pub fn pitch_normalized(&self) -> f32 {
        (self.pitch / consts::PI).clamp(-1.0, 1.0)
    }

    /// Roll normalised to \[-1.0, 1.0\] (±π → ±1.0).
    pub fn roll_normalized(&self) -> f32 {
        (self.roll / consts::PI).clamp(-1.0, 1.0)
    }

    /// Yaw normalised to \[-1.0, 1.0\] (±π/2 → ±1.0).
    pub fn yaw_normalized(&self) -> f32 {
        (self.yaw / consts::FRAC_PI_2).clamp(-1.0, 1.0)
    }

    /// Throttle clamped to \[0.0, 1.0\].
    pub fn throttle_normalized(&self) -> f32 {
        self.throttle.clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;
    use bytemuck::Zeroable;

    fn with_pitch(pitch: f32) -> FlightData {
        let mut fd = FlightData::zeroed();
        fd.pitch = pitch;
        fd
    }

    fn with_throttle(throttle: f32) -> FlightData {
        let mut fd = FlightData::zeroed();
        fd.throttle = throttle;
        fd
    }

    #[test]
    fn test_pitch_normalization() {
        // pitch = π/2 → normalized = 0.5
        let fd = with_pitch(consts::FRAC_PI_2);
        assert_relative_eq!(fd.pitch_normalized(), 0.5, epsilon = 1e-6);
    }

    #[test]
    fn test_throttle_clamp() {
        // throttle = 1.5 → clamped to 1.0
        let fd = with_throttle(1.5);
        assert_relative_eq!(fd.throttle_normalized(), 1.0, epsilon = 1e-6);
    }

    #[test]
    fn pitch_negative_pi_gives_minus_one() {
        let fd = with_pitch(-consts::PI);
        assert_relative_eq!(fd.pitch_normalized(), -1.0, epsilon = 1e-6);
    }

    #[test]
    fn throttle_negative_clamped_to_zero() {
        let fd = with_throttle(-0.5);
        assert_relative_eq!(fd.throttle_normalized(), 0.0, epsilon = 1e-6);
    }
}
