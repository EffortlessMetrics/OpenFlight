// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! FFB effect output for the VPforce Rhino.
//!
//! The Rhino implements the USB HID force-feedback specification (PID page).
//! Effects are sent as HID output reports targeting report ID 0x10–0x1F.
//!
//! **Safety**: all force magnitudes are clamped to \[0.0, 1.0\] before
//! serialisation to prevent hardware over-drive.

/// Maximum output report payload for an effect command.
pub const FFB_REPORT_LEN: usize = 8;

/// Report ID for the constant-force effect command.
pub const REPORT_CONSTANT_FORCE: u8 = 0x10;

/// Report ID for the spring/damper effect command.
pub const REPORT_SPRING: u8 = 0x11;

/// Report ID for the periodic (sine) effect command.
pub const REPORT_PERIODIC: u8 = 0x12;

/// Supported FFB effect types.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FfbEffect {
    /// Constant lateral force (trim / stick shake).
    ///
    /// `direction_deg` is the force angle (0 = right, 90 = up).
    /// `magnitude` is clamped to \[0.0, 1.0\].
    ConstantForce { direction_deg: f32, magnitude: f32 },

    /// Spring / centering effect.
    ///
    /// `coefficient` controls stiffness; clamped to \[0.0, 1.0\].
    Spring { coefficient: f32 },

    /// Damping effect (absorbs rapid movements).
    ///
    /// `coefficient` controls damping strength; clamped to \[0.0, 1.0\].
    Damper { coefficient: f32 },

    /// Sinusoidal vibration.
    ///
    /// `frequency_hz` limited to 1–200 Hz; `magnitude` clamped to \[0.0, 1.0\].
    Sine { frequency_hz: f32, magnitude: f32 },

    /// Stop all active effects and return to passive state.
    StopAll,
}

/// Serialise an [`FfbEffect`] into an HID output report payload.
///
/// Returns the report bytes (including report ID as byte 0).
///
/// # Example
///
/// ```
/// use flight_ffb_vpforce::effects::{FfbEffect, serialize_effect};
/// let bytes = serialize_effect(FfbEffect::StopAll);
/// assert_eq!(bytes[0], 0xFF);
/// ```
pub fn serialize_effect(effect: FfbEffect) -> [u8; FFB_REPORT_LEN] {
    let mut buf = [0u8; FFB_REPORT_LEN];
    match effect {
        FfbEffect::ConstantForce {
            direction_deg,
            magnitude,
        } => {
            let m = magnitude.clamp(0.0, 1.0);
            let angle = (direction_deg % 360.0 + 360.0) % 360.0;
            let angle_raw = (angle / 360.0 * 65535.0) as u16;
            let mag_raw = (m * 10000.0) as u16;
            buf[0] = REPORT_CONSTANT_FORCE;
            buf[1..3].copy_from_slice(&angle_raw.to_le_bytes());
            buf[3..5].copy_from_slice(&mag_raw.to_le_bytes());
        }
        FfbEffect::Spring { coefficient } => {
            let c = coefficient.clamp(0.0, 1.0);
            let raw = (c * 10000.0) as u16;
            buf[0] = REPORT_SPRING;
            buf[1] = 0x01; // spring mode
            buf[2..4].copy_from_slice(&raw.to_le_bytes());
        }
        FfbEffect::Damper { coefficient } => {
            let c = coefficient.clamp(0.0, 1.0);
            let raw = (c * 10000.0) as u16;
            buf[0] = REPORT_SPRING;
            buf[1] = 0x02; // damper mode
            buf[2..4].copy_from_slice(&raw.to_le_bytes());
        }
        FfbEffect::Sine {
            frequency_hz,
            magnitude,
        } => {
            let freq = frequency_hz.clamp(1.0, 200.0) as u16;
            let m = (magnitude.clamp(0.0, 1.0) * 10000.0) as u16;
            buf[0] = REPORT_PERIODIC;
            buf[1..3].copy_from_slice(&freq.to_le_bytes());
            buf[3..5].copy_from_slice(&m.to_le_bytes());
        }
        FfbEffect::StopAll => {
            buf[0] = 0xFF;
        }
    }
    buf
}

/// Returns `true` if the effect magnitude is within a safe operating range.
pub fn is_magnitude_safe(magnitude: f32) -> bool {
    magnitude >= 0.0 && magnitude <= 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stop_all_report_id() {
        let b = serialize_effect(FfbEffect::StopAll);
        assert_eq!(b[0], 0xFF);
    }

    #[test]
    fn test_constant_force_clamps_magnitude() {
        let b = serialize_effect(FfbEffect::ConstantForce {
            direction_deg: 0.0,
            magnitude: 2.0,
        });
        // mag_raw = clamp(2.0, 0,1) * 10000 = 10000 = 0x2710
        let mag = u16::from_le_bytes([b[3], b[4]]);
        assert_eq!(mag, 10000);
    }

    #[test]
    fn test_constant_force_zero_magnitude() {
        let b = serialize_effect(FfbEffect::ConstantForce {
            direction_deg: 90.0,
            magnitude: 0.0,
        });
        let mag = u16::from_le_bytes([b[3], b[4]]);
        assert_eq!(mag, 0);
    }

    #[test]
    fn test_spring_coefficient_serialised() {
        let b = serialize_effect(FfbEffect::Spring { coefficient: 0.5 });
        assert_eq!(b[0], REPORT_SPRING);
        assert_eq!(b[1], 0x01);
        let raw = u16::from_le_bytes([b[2], b[3]]);
        assert_eq!(raw, 5000);
    }

    #[test]
    fn test_damper_uses_spring_report_with_mode_2() {
        let b = serialize_effect(FfbEffect::Damper { coefficient: 1.0 });
        assert_eq!(b[0], REPORT_SPRING);
        assert_eq!(b[1], 0x02);
    }

    #[test]
    fn test_sine_frequency_clamped() {
        let b = serialize_effect(FfbEffect::Sine {
            frequency_hz: 999.0,
            magnitude: 0.5,
        });
        let freq = u16::from_le_bytes([b[1], b[2]]);
        assert_eq!(freq, 200);
    }

    #[test]
    fn test_sine_minimum_frequency() {
        let b = serialize_effect(FfbEffect::Sine {
            frequency_hz: 0.0,
            magnitude: 1.0,
        });
        let freq = u16::from_le_bytes([b[1], b[2]]);
        assert_eq!(freq, 1);
    }

    #[test]
    fn test_is_magnitude_safe() {
        assert!(is_magnitude_safe(0.0));
        assert!(is_magnitude_safe(0.5));
        assert!(is_magnitude_safe(1.0));
        assert!(!is_magnitude_safe(1.01));
        assert!(!is_magnitude_safe(-0.01));
    }
}
