// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! FFB torque output for the Moza AB9 base.
//!
//! Moza uses a proprietary USB HID output report format for torque commands.
//! The AB9 accepts X and Y torque values as 16-bit signed integers.
//!
//! **Safety**: all torque magnitudes are clamped to \[-1.0, 1.0\] before
//! serialisation.

/// HID output report ID for torque commands.
pub const TORQUE_REPORT_ID: u8 = 0x20;

/// Output report length for a torque command.
pub const TORQUE_REPORT_LEN: usize = 6;

/// Torque command for the Moza AB9 FFB base.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TorqueCommand {
    /// Torque on the roll (X) axis — \[-1.0, 1.0\].
    pub x: f32,
    /// Torque on the pitch (Y) axis — \[-1.0, 1.0\].
    pub y: f32,
}

impl TorqueCommand {
    /// Zero torque (neutral / passive state).
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    /// Serialise into an HID output report.
    ///
    /// # Safety: magnitudes are clamped to [-1.0, 1.0].
    ///
    /// # Example
    ///
    /// ```
    /// use flight_ffb_moza::effects::TorqueCommand;
    /// let cmd = TorqueCommand { x: 0.5, y: -0.25 };
    /// let report = cmd.to_report();
    /// assert_eq!(report[0], 0x20);
    /// ```
    pub fn to_report(self) -> [u8; TORQUE_REPORT_LEN] {
        let mut buf = [0u8; TORQUE_REPORT_LEN];
        buf[0] = TORQUE_REPORT_ID;
        let x_raw = (self.x.clamp(-1.0, 1.0) * 32767.0) as i16;
        let y_raw = (self.y.clamp(-1.0, 1.0) * 32767.0) as i16;
        buf[1..3].copy_from_slice(&x_raw.to_le_bytes());
        buf[3..5].copy_from_slice(&y_raw.to_le_bytes());
        buf
    }

    /// Returns `true` if both torque values are within safe range.
    pub fn is_safe(&self) -> bool {
        self.x.abs() <= 1.0 && self.y.abs() <= 1.0
    }
}

/// FFB mode for the AB9 base.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfbMode {
    /// Forces disabled; base in passive/friction mode.
    Passive,
    /// Spring centering — resist displacement from centre.
    Spring,
    /// Damper — absorb rapid movements.
    Damper,
    /// Direct torque command via [`TorqueCommand`].
    Direct,
}

impl std::fmt::Display for FfbMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Passive => f.write_str("Passive"),
            Self::Spring => f.write_str("Spring"),
            Self::Damper => f.write_str("Damper"),
            Self::Direct => f.write_str("Direct"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_torque_report() {
        let r = TorqueCommand::ZERO.to_report();
        assert_eq!(r[0], TORQUE_REPORT_ID);
        assert_eq!(&r[1..3], &[0, 0]);
        assert_eq!(&r[3..5], &[0, 0]);
    }

    #[test]
    fn test_full_positive_x_torque() {
        let cmd = TorqueCommand { x: 1.0, y: 0.0 };
        let r = cmd.to_report();
        let x = i16::from_le_bytes([r[1], r[2]]);
        assert_eq!(x, 32767);
    }

    #[test]
    fn test_full_negative_y_torque() {
        let cmd = TorqueCommand { x: 0.0, y: -1.0 };
        let r = cmd.to_report();
        let y = i16::from_le_bytes([r[3], r[4]]);
        assert_eq!(y, -32767);
    }

    #[test]
    fn test_torque_clamps_over_range() {
        let cmd = TorqueCommand { x: 2.5, y: -3.0 };
        let r = cmd.to_report();
        let x = i16::from_le_bytes([r[1], r[2]]);
        let y = i16::from_le_bytes([r[3], r[4]]);
        assert_eq!(x, 32767);
        assert_eq!(y, -32767);
    }

    #[test]
    fn test_is_safe() {
        assert!(TorqueCommand::ZERO.is_safe());
        assert!(TorqueCommand { x: 0.5, y: -0.5 }.is_safe());
        assert!(!TorqueCommand { x: 1.1, y: 0.0 }.is_safe());
    }

    #[test]
    fn test_ffb_mode_display() {
        assert_eq!(FfbMode::Passive.to_string(), "Passive");
        assert_eq!(FfbMode::Spring.to_string(), "Spring");
    }
}
