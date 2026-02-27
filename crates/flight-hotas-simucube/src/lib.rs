// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Simucube 2 direct-drive wheel/stick driver for OpenFlight.
//!
//! Supports **Sport**, **Pro**, and **Ultimate** variants of the Simucube 2
//! family via USB HID.
//!
//! # USB Identifiers
//!
//! | Model     | VID    | PID    |
//! |-----------|--------|--------|
//! | Sport     | 0x16D0 | 0x0D5A |
//! | Pro       | 0x16D0 | 0x0D61 |
//! | Ultimate  | 0x16D0 | 0x0D60 |
//!
//! # Encoder
//!
//! The Simucube 2 uses a **22-bit** absolute encoder (0 … 4 194 303).
//! Centre position is **2 097 151** (midpoint of the range).
//! [`normalize_angle`] converts encoder counts to −1.0 … +1.0 for any
//! resolution.
//!
//! # Torque
//!
//! Output torque commands are **i16** values where ±32 767 = ±100 % rated
//! torque.  See [`TorqueCommand`] for the builder API.

use thiserror::Error;

// ── Constants ─────────────────────────────────────────────────────────────────

/// Simcraft / Granite Devices USB Vendor ID (shared across SC2 family).
pub const SIMUCUBE_VENDOR_ID: u16 = 0x16D0;

/// PID for Simucube 2 Sport.
pub const SC2_SPORT_PID: u16 = 0x0D5A;

/// PID for Simucube 2 Pro.
pub const SC2_PRO_PID: u16 = 0x0D61;

/// PID for Simucube 2 Ultimate.
pub const SC2_ULTIMATE_PID: u16 = 0x0D60;

/// Minimum HID report length expected from the Simucube 2.
pub const SC2_REPORT_MIN_LEN: usize = 7;

/// Full 22-bit encoder range (0 … 4 194 303).
pub const ENCODER_MAX: u32 = (1u32 << 22) - 1;

/// Centre position of the 22-bit encoder.
pub const ENCODER_CENTER: u32 = ENCODER_MAX / 2;

// ── Error ─────────────────────────────────────────────────────────────────────

/// Errors produced by the Simucube driver.
#[derive(Debug, Error, PartialEq)]
pub enum SimucubeError {
    /// Report buffer shorter than [`SC2_REPORT_MIN_LEN`].
    #[error("report too short: expected ≥{SC2_REPORT_MIN_LEN} bytes, got {got}")]
    TooShort { got: usize },

    /// Unknown or unsupported HID report ID.
    #[error("unknown report ID: 0x{id:02X}")]
    UnknownReportId { id: u8 },
}

// ── Model detection ───────────────────────────────────────────────────────────

/// Simucube 2 product variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimucubeModel {
    /// Simucube 2 Sport (PID 0x0D5A) — 17 Nm peak.
    Sport,
    /// Simucube 2 Pro (PID 0x0D61) — 25 Nm peak.
    Pro,
    /// Simucube 2 Ultimate (PID 0x0D60) — 32 Nm peak.
    Ultimate,
}

impl SimucubeModel {
    /// Identify the model from a USB **Product ID**.
    ///
    /// Returns `None` if the PID is not a known Simucube 2.
    pub fn from_pid(pid: u16) -> Option<Self> {
        match pid {
            SC2_SPORT_PID => Some(Self::Sport),
            SC2_PRO_PID => Some(Self::Pro),
            SC2_ULTIMATE_PID => Some(Self::Ultimate),
            _ => None,
        }
    }

    /// USB Product ID for this model.
    pub fn pid(self) -> u16 {
        match self {
            Self::Sport => SC2_SPORT_PID,
            Self::Pro => SC2_PRO_PID,
            Self::Ultimate => SC2_ULTIMATE_PID,
        }
    }
}

// ── Report ────────────────────────────────────────────────────────────────────

/// Parsed Simucube 2 HID input report.
///
/// Report layout (bytes, all little-endian):
///
/// | Offset | Size | Field              |
/// |--------|------|--------------------|
/// | 0      | 1    | report ID (0x01)   |
/// | 1      | 4    | encoder position   |
/// | 5      | 2    | velocity (i16)     |
/// | 7 *    | 2    | torque feedback (i16) |
///
/// \* Optional — only present when `torque_feedback` byte is provided.
#[derive(Debug, Clone, PartialEq)]
pub struct SimucubeReport {
    /// Absolute encoder position (22-bit, 0 … 4 194 303).
    pub encoder_position: u32,
    /// Wheel velocity in encoder counts per millisecond (i16).
    pub velocity: i16,
    /// Torque feedback from the device (i16, ±32 767 = ±100 % rated).
    pub torque_feedback: i16,
}

/// Parse a raw HID input report from a Simucube 2 device.
///
/// # Errors
///
/// - [`SimucubeError::TooShort`] — fewer than [`SC2_REPORT_MIN_LEN`] bytes.
/// - [`SimucubeError::UnknownReportId`] — byte 0 is not `0x01`.
pub fn parse_report(bytes: &[u8]) -> Result<SimucubeReport, SimucubeError> {
    if bytes.len() < SC2_REPORT_MIN_LEN {
        return Err(SimucubeError::TooShort { got: bytes.len() });
    }
    if bytes[0] != 0x01 {
        return Err(SimucubeError::UnknownReportId { id: bytes[0] });
    }

    let encoder_position =
        u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) & ENCODER_MAX;
    let velocity = i16::from_le_bytes([bytes[5], bytes[6]]);
    let torque_feedback = if bytes.len() >= 9 {
        i16::from_le_bytes([bytes[7], bytes[8]])
    } else {
        0
    };

    tracing::trace!(
        encoder_position,
        velocity,
        torque_feedback,
        "parsed Simucube report"
    );

    Ok(SimucubeReport {
        encoder_position,
        velocity,
        torque_feedback,
    })
}

// ── Angle normalisation ───────────────────────────────────────────────────────

/// Normalise an absolute encoder position to **−1.0 … +1.0**.
///
/// `resolution_bits` controls the encoder range:
/// - 22 bits → 0 … 4 194 303, centre = 2 097 151
/// - Other values are supported; `pos` is clamped to the valid range.
///
/// Centre maps to **0.0**; full CCW to **−1.0**; full CW to **+1.0**.
pub fn normalize_angle(pos: u32, resolution_bits: u8) -> f32 {
    let max = if resolution_bits >= 32 {
        u32::MAX
    } else {
        (1u32 << resolution_bits).saturating_sub(1)
    };
    let center = max / 2;
    let clamped = pos.min(max) as f32;
    let half = center as f32;
    if half == 0.0 {
        return 0.0;
    }
    ((clamped - half) / half).clamp(-1.0, 1.0)
}

// ── Torque command ────────────────────────────────────────────────────────────

/// Torque output command for the Simucube 2.
///
/// `value` is in the range **−1.0 … +1.0** (normalised), which this struct
/// converts to the ±32 767 i16 wire format expected by the device.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TorqueCommand {
    /// Normalised torque request, clamped to −1.0 … +1.0.
    pub value: f32,
}

impl TorqueCommand {
    /// Create a new torque command, clamping `value` to [−1.0, 1.0].
    pub fn new(value: f32) -> Self {
        Self {
            value: value.clamp(-1.0, 1.0),
        }
    }

    /// Encode to i16 wire format (±32 767).
    pub fn to_i16(self) -> i16 {
        (self.value * 32767.0).clamp(-32767.0, 32767.0) as i16
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn build_report(encoder: u32, velocity: i16, torque: i16) -> Vec<u8> {
        let enc = encoder.to_le_bytes();
        let vel = velocity.to_le_bytes();
        let tq = torque.to_le_bytes();
        vec![
            0x01, enc[0], enc[1], enc[2], enc[3], vel[0], vel[1], tq[0], tq[1],
        ]
    }

    // ── parse_report ─────────────────────────────────────────────────────────

    #[test]
    fn test_parse_valid_report() {
        let data = build_report(2_097_151, 100, -500);
        let r = parse_report(&data).unwrap();
        assert_eq!(r.encoder_position, 2_097_151);
        assert_eq!(r.velocity, 100);
        assert_eq!(r.torque_feedback, -500);
    }

    #[test]
    fn test_parse_too_short() {
        assert_eq!(
            parse_report(&[0x01, 0x00, 0x00]),
            Err(SimucubeError::TooShort { got: 3 })
        );
    }

    #[test]
    fn test_parse_empty() {
        assert_eq!(parse_report(&[]), Err(SimucubeError::TooShort { got: 0 }));
    }

    #[test]
    fn test_parse_unknown_report_id() {
        let mut data = build_report(0, 0, 0);
        data[0] = 0x02;
        assert_eq!(
            parse_report(&data),
            Err(SimucubeError::UnknownReportId { id: 0x02 })
        );
    }

    #[test]
    fn test_parse_encoder_masked_to_22bit() {
        // All bits set in 4 bytes = 0xFFFF_FFFF; masked to 22 bits = ENCODER_MAX.
        let data = build_report(0xFFFF_FFFF, 0, 0);
        let r = parse_report(&data).unwrap();
        assert_eq!(r.encoder_position, ENCODER_MAX);
    }

    #[test]
    fn test_parse_min_length_no_torque() {
        // 7 bytes: no torque field → defaults to 0.
        let enc = 0u32.to_le_bytes();
        let vel = 0i16.to_le_bytes();
        let data = vec![0x01, enc[0], enc[1], enc[2], enc[3], vel[0], vel[1]];
        let r = parse_report(&data).unwrap();
        assert_eq!(r.torque_feedback, 0);
    }

    // ── model detection ──────────────────────────────────────────────────────

    #[test]
    fn test_model_from_pid() {
        assert_eq!(
            SimucubeModel::from_pid(SC2_SPORT_PID),
            Some(SimucubeModel::Sport)
        );
        assert_eq!(
            SimucubeModel::from_pid(SC2_PRO_PID),
            Some(SimucubeModel::Pro)
        );
        assert_eq!(
            SimucubeModel::from_pid(SC2_ULTIMATE_PID),
            Some(SimucubeModel::Ultimate)
        );
        assert_eq!(SimucubeModel::from_pid(0xDEAD), None);
    }

    #[test]
    fn test_model_pid_round_trip() {
        for model in [
            SimucubeModel::Sport,
            SimucubeModel::Pro,
            SimucubeModel::Ultimate,
        ] {
            assert_eq!(SimucubeModel::from_pid(model.pid()), Some(model));
        }
    }

    // ── normalize_angle ──────────────────────────────────────────────────────

    #[test]
    fn test_normalize_angle_center_is_zero() {
        assert!((normalize_angle(ENCODER_CENTER, 22)).abs() < 1e-5);
    }

    #[test]
    fn test_normalize_angle_min_is_negative_one() {
        let v = normalize_angle(0, 22);
        assert!((v - (-1.0)).abs() < 1e-4);
    }

    #[test]
    fn test_normalize_angle_max_is_positive_one() {
        let v = normalize_angle(ENCODER_MAX, 22);
        assert!((v - 1.0).abs() < 1e-4);
    }

    // ── TorqueCommand ─────────────────────────────────────────────────────────

    #[test]
    fn test_torque_command_max() {
        assert_eq!(TorqueCommand::new(1.0).to_i16(), 32767);
    }

    #[test]
    fn test_torque_command_min() {
        assert_eq!(TorqueCommand::new(-1.0).to_i16(), -32767);
    }

    #[test]
    fn test_torque_command_zero() {
        assert_eq!(TorqueCommand::new(0.0).to_i16(), 0);
    }

    #[test]
    fn test_torque_command_clamped() {
        assert_eq!(TorqueCommand::new(999.0).to_i16(), 32767);
        assert_eq!(TorqueCommand::new(-999.0).to_i16(), -32767);
    }

    // ── proptest ─────────────────────────────────────────────────────────────

    proptest! {
        #[test]
        fn test_normalize_angle_always_in_range(
            pos in 0u32..=ENCODER_MAX,
        ) {
            let v = normalize_angle(pos, 22);
            prop_assert!(v >= -1.0 && v <= 1.0, "got {}", v);
        }
    }
}
