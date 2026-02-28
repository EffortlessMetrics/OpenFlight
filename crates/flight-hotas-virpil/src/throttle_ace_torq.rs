// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the VIRPIL VPC ACE Torq (single-axis throttle quadrant).
//!
//! # Confirmed device identifier
//!
//! VID 0x3344 (VIRPIL Controls), PID 0x0198 — community-documented via
//! linux-hardware.org probe data. HIL validation pending.
//!
//! # Input report layout (5 bytes, community-documented)
//!
//! ```text
//! byte  0       : report_id (0x01)
//! bytes  1–2   : throttle axis (u16 LE, 14-bit, range 0–16384)
//! bytes  3–4   : buttons (up to 16 buttons → 2 bytes, LSB-first)
//! ```
//!
//! The ACE Torq is a compact, single-axis throttle quadrant. It has one
//! main throttle lever and a small number of momentary buttons.

use crate::VIRPIL_AXIS_MAX;
use thiserror::Error;

/// Minimum byte count for a VPC ACE Torq report.
pub const VPC_ACE_TORQ_MIN_REPORT_BYTES: usize = 5;

const ACE_TORQ_BUTTON_BYTES: usize = 2;
/// Number of discrete buttons on the VPC ACE Torq.
pub const ACE_TORQ_BUTTON_COUNT: u8 = 8;

/// Parse error for the VPC ACE Torq.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VpcAceTorqParseError {
    #[error("VPC ACE Torq report too short: got {0} bytes (need ≥5)")]
    TooShort(usize),
}

/// Normalised axis from the VPC ACE Torq.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VpcAceTorqAxis {
    /// Throttle lever. 0.0 = idle, 1.0 = full power.
    pub throttle: f32,
}

/// Button state from the VPC ACE Torq.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VpcAceTorqButtons {
    /// Raw button bytes (2 bytes, LSB-first, only first 8 bits used).
    pub raw: [u8; ACE_TORQ_BUTTON_BYTES],
}

impl Default for VpcAceTorqButtons {
    fn default() -> Self {
        Self {
            raw: [0u8; ACE_TORQ_BUTTON_BYTES],
        }
    }
}

impl VpcAceTorqButtons {
    /// Return `true` if button `n` (1-indexed, 1..=8) is pressed.
    pub fn is_pressed(&self, n: u8) -> bool {
        if !(1..=ACE_TORQ_BUTTON_COUNT).contains(&n) {
            return false;
        }
        let idx = (n - 1) as usize;
        let byte = idx / 8;
        let bit = idx % 8;
        (self.raw[byte] >> bit) & 1 == 1
    }

    /// Return a `Vec` of pressed button numbers (1-indexed).
    pub fn pressed(&self) -> Vec<u8> {
        (1u8..=ACE_TORQ_BUTTON_COUNT)
            .filter(|&n| self.is_pressed(n))
            .collect()
    }
}

/// Full parsed input state from one VPC ACE Torq HID report.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VpcAceTorqInputState {
    pub axis: VpcAceTorqAxis,
    pub buttons: VpcAceTorqButtons,
}

/// Parse one raw HID report from the VPC ACE Torq.
pub fn parse_ace_torq_report(data: &[u8]) -> Result<VpcAceTorqInputState, VpcAceTorqParseError> {
    if data.len() < VPC_ACE_TORQ_MIN_REPORT_BYTES {
        return Err(VpcAceTorqParseError::TooShort(data.len()));
    }

    let raw_throttle = u16::from_le_bytes([data[1], data[2]]);
    let throttle = (raw_throttle as f32 / VIRPIL_AXIS_MAX as f32).clamp(0.0, 1.0);

    let mut raw_buttons = [0u8; ACE_TORQ_BUTTON_BYTES];
    raw_buttons.copy_from_slice(&data[3..3 + ACE_TORQ_BUTTON_BYTES]);

    Ok(VpcAceTorqInputState {
        axis: VpcAceTorqAxis { throttle },
        buttons: VpcAceTorqButtons { raw: raw_buttons },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn make_ace_torq_report(throttle: u16, buttons: [u8; 2]) -> Vec<u8> {
        let mut data = vec![0x01u8]; // report_id
        data.extend_from_slice(&throttle.to_le_bytes());
        data.extend_from_slice(&buttons);
        data
    }

    #[test]
    fn too_short_is_error() {
        assert!(parse_ace_torq_report(&[0x01; 4]).is_err());
    }

    #[test]
    fn empty_slice_is_error() {
        assert!(parse_ace_torq_report(&[]).is_err());
    }

    #[test]
    fn exactly_min_bytes_parses_ok() {
        let report = make_ace_torq_report(0, [0u8; 2]);
        assert_eq!(report.len(), VPC_ACE_TORQ_MIN_REPORT_BYTES);
        assert!(parse_ace_torq_report(&report).is_ok());
    }

    #[test]
    fn longer_report_does_not_error() {
        let mut report = make_ace_torq_report(0, [0u8; 2]);
        report.extend_from_slice(&[0u8; 8]);
        assert!(parse_ace_torq_report(&report).is_ok());
    }

    #[test]
    fn zero_throttle_parses_to_zero() {
        let report = make_ace_torq_report(0, [0u8; 2]);
        let state = parse_ace_torq_report(&report).unwrap();
        assert_eq!(state.axis.throttle, 0.0);
    }

    #[test]
    fn max_throttle_parses_to_one() {
        let report = make_ace_torq_report(VIRPIL_AXIS_MAX, [0u8; 2]);
        let state = parse_ace_torq_report(&report).unwrap();
        assert!((state.axis.throttle - 1.0).abs() < 1e-4);
    }

    #[test]
    fn half_throttle_is_approximately_half() {
        let report = make_ace_torq_report(VIRPIL_AXIS_MAX / 2, [0u8; 2]);
        let state = parse_ace_torq_report(&report).unwrap();
        assert!((state.axis.throttle - 0.5).abs() < 0.01);
    }

    #[test]
    fn no_buttons_pressed_by_default() {
        let report = make_ace_torq_report(0, [0u8; 2]);
        let state = parse_ace_torq_report(&report).unwrap();
        assert!(state.buttons.pressed().is_empty());
    }

    #[test]
    fn button_1_detected() {
        let report = make_ace_torq_report(0, [0x01, 0x00]);
        let state = parse_ace_torq_report(&report).unwrap();
        assert!(state.buttons.is_pressed(1));
        assert!(!state.buttons.is_pressed(2));
    }

    #[test]
    fn button_8_detected() {
        let report = make_ace_torq_report(0, [0x80, 0x00]);
        let state = parse_ace_torq_report(&report).unwrap();
        assert!(state.buttons.is_pressed(8));
    }

    #[test]
    fn all_buttons_pressed() {
        let report = make_ace_torq_report(0, [0xFF, 0xFF]);
        let state = parse_ace_torq_report(&report).unwrap();
        for i in 1u8..=8 {
            assert!(state.buttons.is_pressed(i), "button {i} not pressed");
        }
    }

    #[test]
    fn out_of_range_button_returns_false() {
        let report = make_ace_torq_report(0, [0xFF, 0xFF]);
        let state = parse_ace_torq_report(&report).unwrap();
        assert!(!state.buttons.is_pressed(0));
        assert!(!state.buttons.is_pressed(9));
    }

    #[test]
    fn error_message_contains_byte_count() {
        let err = parse_ace_torq_report(&[0x01; 3]).unwrap_err();
        assert!(err.to_string().contains('3'));
    }

    proptest! {
        #[test]
        fn throttle_always_in_range(raw in 0u16..=u16::MAX) {
            let report = make_ace_torq_report(raw, [0u8; 2]);
            let state = parse_ace_torq_report(&report).unwrap();
            prop_assert!((0.0..=1.0).contains(&state.axis.throttle));
        }

        #[test]
        fn random_report_does_not_panic(data in proptest::collection::vec(0u8..=255u8, 5..16)) {
            let _ = parse_ace_torq_report(&data);
        }

        #[test]
        fn roundtrip_axis_encode_decode(raw in 0u16..=VIRPIL_AXIS_MAX) {
            let report = make_ace_torq_report(raw, [0u8; 2]);
            let state = parse_ace_torq_report(&report).unwrap();
            let expected = raw as f32 / VIRPIL_AXIS_MAX as f32;
            prop_assert!((state.axis.throttle - expected).abs() < 1e-4);
        }
    }
}
