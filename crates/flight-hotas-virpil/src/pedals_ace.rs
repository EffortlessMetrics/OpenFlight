// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the VIRPIL VPC ACE Collection Pedals.
//!
//! # Confirmed device identifier
//!
//! VID 0x3344 (VIRPIL Controls), PID 0x019C — community-documented via
//! linux-hardware.org probe data. HIL validation pending.
//!
//! # Input report layout (9 bytes, community-documented)
//!
//! ```text
//! byte  0       : report_id (0x01)
//! bytes  1–2   : rudder axis (u16 LE, 14-bit, range 0–16384)
//! bytes  3–4   : left toe brake axis (u16 LE)
//! bytes  5–6   : right toe brake axis (u16 LE)
//! bytes  7–8   : buttons (up to 16 buttons → 2 bytes, LSB-first)
//! ```
//!
//! Axis normalisation: raw value 0→0.0, raw value 16384→1.0.

use crate::VIRPIL_AXIS_MAX;
use thiserror::Error;

/// Minimum byte count for a VPC ACE Pedals report.
pub const VPC_ACE_PEDALS_MIN_REPORT_BYTES: usize = 9;

const ACE_PEDALS_AXIS_COUNT: usize = 3;
const ACE_PEDALS_BUTTON_BYTES: usize = 2;
/// Number of discrete buttons on the VPC ACE Collection Pedals.
pub const ACE_PEDALS_BUTTON_COUNT: u8 = 16;

/// Parse error for the VPC ACE Collection Pedals.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VpcAcePedalsParseError {
    #[error("VPC ACE Pedals report too short: got {0} bytes (need ≥9)")]
    TooShort(usize),
}

/// Normalised axes from the VPC ACE Collection Pedals.
///
/// All values are in the range `[0.0, 1.0]`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VpcAcePedalsAxes {
    /// Rudder axis. 0.0 = full left, 0.5 = centre, 1.0 = full right.
    pub rudder: f32,
    /// Left toe brake. 0.0 = released, 1.0 = fully pressed.
    pub left_toe_brake: f32,
    /// Right toe brake. 0.0 = released, 1.0 = fully pressed.
    pub right_toe_brake: f32,
}

/// Button state from the VPC ACE Collection Pedals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VpcAcePedalsButtons {
    /// Raw button bytes (2 bytes covering up to 16 buttons, LSB-first).
    pub raw: [u8; ACE_PEDALS_BUTTON_BYTES],
}

impl Default for VpcAcePedalsButtons {
    fn default() -> Self {
        Self {
            raw: [0u8; ACE_PEDALS_BUTTON_BYTES],
        }
    }
}

impl VpcAcePedalsButtons {
    /// Return `true` if button `n` (1-indexed, 1..=16) is pressed.
    pub fn is_pressed(&self, n: u8) -> bool {
        if !(1..=ACE_PEDALS_BUTTON_COUNT).contains(&n) {
            return false;
        }
        let idx = (n - 1) as usize;
        let byte = idx / 8;
        let bit = idx % 8;
        (self.raw[byte] >> bit) & 1 == 1
    }

    /// Return a `Vec` of pressed button numbers (1-indexed).
    pub fn pressed(&self) -> Vec<u8> {
        (1u8..=ACE_PEDALS_BUTTON_COUNT)
            .filter(|&n| self.is_pressed(n))
            .collect()
    }
}

/// Full parsed input state from one VPC ACE Collection Pedals HID report.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VpcAcePedalsInputState {
    pub axes: VpcAcePedalsAxes,
    pub buttons: VpcAcePedalsButtons,
}

/// Parse one raw HID report from the VPC ACE Collection Pedals.
pub fn parse_ace_pedals_report(
    data: &[u8],
) -> Result<VpcAcePedalsInputState, VpcAcePedalsParseError> {
    if data.len() < VPC_ACE_PEDALS_MIN_REPORT_BYTES {
        return Err(VpcAcePedalsParseError::TooShort(data.len()));
    }

    let payload = &data[1..]; // skip report_id
    let mut raw_axes = [0u16; ACE_PEDALS_AXIS_COUNT];
    for (i, v) in raw_axes.iter_mut().enumerate() {
        *v = u16::from_le_bytes([payload[i * 2], payload[i * 2 + 1]]);
    }

    let normalize = |v: u16| (v as f32 / VIRPIL_AXIS_MAX as f32).clamp(0.0, 1.0);

    let axes = VpcAcePedalsAxes {
        rudder: normalize(raw_axes[0]),
        left_toe_brake: normalize(raw_axes[1]),
        right_toe_brake: normalize(raw_axes[2]),
    };

    let btn_start = 1 + ACE_PEDALS_AXIS_COUNT * 2;
    let mut raw_buttons = [0u8; ACE_PEDALS_BUTTON_BYTES];
    raw_buttons.copy_from_slice(&data[btn_start..btn_start + ACE_PEDALS_BUTTON_BYTES]);

    Ok(VpcAcePedalsInputState {
        axes,
        buttons: VpcAcePedalsButtons { raw: raw_buttons },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn make_ace_pedals_report(axes: [u16; 3], buttons: [u8; 2]) -> Vec<u8> {
        let mut data = vec![0x01u8]; // report_id
        for ax in &axes {
            data.extend_from_slice(&ax.to_le_bytes());
        }
        data.extend_from_slice(&buttons);
        data
    }

    #[test]
    fn too_short_is_error() {
        assert!(parse_ace_pedals_report(&[0x01; 8]).is_err());
    }

    #[test]
    fn empty_slice_is_error() {
        assert!(parse_ace_pedals_report(&[]).is_err());
    }

    #[test]
    fn exactly_min_bytes_parses_ok() {
        let report = make_ace_pedals_report([0u16; 3], [0u8; 2]);
        assert_eq!(report.len(), VPC_ACE_PEDALS_MIN_REPORT_BYTES);
        assert!(parse_ace_pedals_report(&report).is_ok());
    }

    #[test]
    fn longer_report_does_not_error() {
        let mut report = make_ace_pedals_report([0u16; 3], [0u8; 2]);
        report.extend_from_slice(&[0u8; 8]);
        assert!(parse_ace_pedals_report(&report).is_ok());
    }

    #[test]
    fn zero_axes_parse_to_zero() {
        let report = make_ace_pedals_report([0u16; 3], [0u8; 2]);
        let state = parse_ace_pedals_report(&report).unwrap();
        assert_eq!(state.axes.rudder, 0.0);
        assert_eq!(state.axes.left_toe_brake, 0.0);
        assert_eq!(state.axes.right_toe_brake, 0.0);
    }

    #[test]
    fn max_axes_parse_to_one() {
        let report = make_ace_pedals_report([VIRPIL_AXIS_MAX; 3], [0u8; 2]);
        let state = parse_ace_pedals_report(&report).unwrap();
        assert!((state.axes.rudder - 1.0).abs() < 1e-4);
        assert!((state.axes.left_toe_brake - 1.0).abs() < 1e-4);
        assert!((state.axes.right_toe_brake - 1.0).abs() < 1e-4);
    }

    #[test]
    fn midpoint_rudder_is_half() {
        let report = make_ace_pedals_report([VIRPIL_AXIS_MAX / 2, 0, 0], [0u8; 2]);
        let state = parse_ace_pedals_report(&report).unwrap();
        assert!((state.axes.rudder - 0.5).abs() < 0.01);
    }

    #[test]
    fn no_buttons_pressed_by_default() {
        let report = make_ace_pedals_report([0u16; 3], [0u8; 2]);
        let state = parse_ace_pedals_report(&report).unwrap();
        assert!(state.buttons.pressed().is_empty());
    }

    #[test]
    fn button_1_detected() {
        let report = make_ace_pedals_report([0u16; 3], [0x01, 0x00]);
        let state = parse_ace_pedals_report(&report).unwrap();
        assert!(state.buttons.is_pressed(1));
        assert!(!state.buttons.is_pressed(2));
    }

    #[test]
    fn button_16_detected() {
        let report = make_ace_pedals_report([0u16; 3], [0x00, 0x80]);
        let state = parse_ace_pedals_report(&report).unwrap();
        assert!(state.buttons.is_pressed(16));
    }

    #[test]
    fn out_of_range_button_returns_false() {
        let report = make_ace_pedals_report([0u16; 3], [0xFF, 0xFF]);
        let state = parse_ace_pedals_report(&report).unwrap();
        assert!(!state.buttons.is_pressed(0));
        assert!(!state.buttons.is_pressed(17));
    }

    #[test]
    fn error_message_contains_byte_count() {
        let err = parse_ace_pedals_report(&[0x01; 5]).unwrap_err();
        assert!(err.to_string().contains('5'));
    }

    proptest! {
        #[test]
        fn axes_always_in_range(
            raw0 in 0u16..=u16::MAX,
            raw1 in 0u16..=u16::MAX,
            raw2 in 0u16..=u16::MAX,
        ) {
            let report = make_ace_pedals_report([raw0, raw1, raw2], [0u8; 2]);
            let state = parse_ace_pedals_report(&report).unwrap();
            prop_assert!((0.0..=1.0).contains(&state.axes.rudder));
            prop_assert!((0.0..=1.0).contains(&state.axes.left_toe_brake));
            prop_assert!((0.0..=1.0).contains(&state.axes.right_toe_brake));
        }

        #[test]
        fn random_report_does_not_panic(data in proptest::collection::vec(0u8..=255u8, 9..24)) {
            let _ = parse_ace_pedals_report(&data);
        }

        #[test]
        fn roundtrip_axis_encode_decode(raw in 0u16..=VIRPIL_AXIS_MAX) {
            let report = make_ace_pedals_report([raw, raw, raw], [0u8; 2]);
            let state = parse_ace_pedals_report(&report).unwrap();
            let expected = raw as f32 / VIRPIL_AXIS_MAX as f32;
            prop_assert!((state.axes.rudder - expected).abs() < 1e-4);
            prop_assert!((state.axes.left_toe_brake - expected).abs() < 1e-4);
            prop_assert!((state.axes.right_toe_brake - expected).abs() < 1e-4);
        }
    }
}
