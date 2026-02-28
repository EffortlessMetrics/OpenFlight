// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the VIRPIL VPC Rotor TCS Plus (helicopter collective).
//!
//! # Confirmed device identifier
//!
//! VID 0x3344 (VIRPIL Controls), PID 0x01A0 — community-documented via
//! linux-hardware.org probe data. HIL validation pending.
//!
//! # Input report layout (11 bytes, community-documented)
//!
//! ```text
//! byte  0       : report_id (0x01)
//! bytes  1–2   : collective axis (u16 LE, 14-bit, range 0–16384)
//! bytes  3–4   : throttle/idle cutoff axis (u16 LE)
//! bytes  5–6   : rotary axis (u16 LE)
//! bytes  7–10  : buttons (up to 32 buttons → 4 bytes, LSB-first)
//! ```
//!
//! The Rotor TCS Plus features a main collective lever with a friction
//! clutch, a secondary throttle/idle cutoff axis, a rotary encoder knob,
//! and approximately 24 buttons including momentary and latching switches.

use crate::VIRPIL_AXIS_MAX;
use thiserror::Error;

/// Minimum byte count for a VPC Rotor TCS Plus report.
pub const VPC_ROTOR_TCS_MIN_REPORT_BYTES: usize = 11;

const ROTOR_TCS_AXIS_COUNT: usize = 3;
const ROTOR_TCS_BUTTON_BYTES: usize = 4;
/// Number of discrete buttons on the VPC Rotor TCS Plus.
pub const ROTOR_TCS_BUTTON_COUNT: u8 = 24;

/// Parse error for the VPC Rotor TCS Plus.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VpcRotorTcsParseError {
    #[error("VPC Rotor TCS Plus report too short: got {0} bytes (need ≥11)")]
    TooShort(usize),
}

/// Normalised axes from the VPC Rotor TCS Plus.
///
/// All values are in the range `[0.0, 1.0]`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VpcRotorTcsAxes {
    /// Collective lever axis. 0.0 = full down, 1.0 = full up.
    pub collective: f32,
    /// Throttle / idle cutoff axis. 0.0 = idle cutoff, 1.0 = full.
    pub throttle_idle: f32,
    /// Rotary axis (e.g. friction knob or rotary encoder). 0.0 = min, 1.0 = max.
    pub rotary: f32,
}

/// Button state from the VPC Rotor TCS Plus (24 buttons).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VpcRotorTcsButtons {
    /// Raw button bytes (4 bytes covering up to 32 bits, only 24 used, LSB-first).
    pub raw: [u8; ROTOR_TCS_BUTTON_BYTES],
}

impl Default for VpcRotorTcsButtons {
    fn default() -> Self {
        Self {
            raw: [0u8; ROTOR_TCS_BUTTON_BYTES],
        }
    }
}

impl VpcRotorTcsButtons {
    /// Return `true` if button `n` (1-indexed, 1..=24) is pressed.
    pub fn is_pressed(&self, n: u8) -> bool {
        if !(1..=ROTOR_TCS_BUTTON_COUNT).contains(&n) {
            return false;
        }
        let idx = (n - 1) as usize;
        let byte = idx / 8;
        let bit = idx % 8;
        (self.raw[byte] >> bit) & 1 == 1
    }

    /// Return a `Vec` of pressed button numbers (1-indexed).
    pub fn pressed(&self) -> Vec<u8> {
        (1u8..=ROTOR_TCS_BUTTON_COUNT)
            .filter(|&n| self.is_pressed(n))
            .collect()
    }
}

/// Full parsed input state from one VPC Rotor TCS Plus HID report.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VpcRotorTcsInputState {
    pub axes: VpcRotorTcsAxes,
    pub buttons: VpcRotorTcsButtons,
}

/// Parse one raw HID report from the VPC Rotor TCS Plus.
pub fn parse_rotor_tcs_report(
    data: &[u8],
) -> Result<VpcRotorTcsInputState, VpcRotorTcsParseError> {
    if data.len() < VPC_ROTOR_TCS_MIN_REPORT_BYTES {
        return Err(VpcRotorTcsParseError::TooShort(data.len()));
    }

    let payload = &data[1..]; // skip report_id
    let mut raw_axes = [0u16; ROTOR_TCS_AXIS_COUNT];
    for (i, v) in raw_axes.iter_mut().enumerate() {
        *v = u16::from_le_bytes([payload[i * 2], payload[i * 2 + 1]]);
    }

    let normalize = |v: u16| (v as f32 / VIRPIL_AXIS_MAX as f32).clamp(0.0, 1.0);

    let axes = VpcRotorTcsAxes {
        collective: normalize(raw_axes[0]),
        throttle_idle: normalize(raw_axes[1]),
        rotary: normalize(raw_axes[2]),
    };

    let btn_start = 1 + ROTOR_TCS_AXIS_COUNT * 2;
    let mut raw_buttons = [0u8; ROTOR_TCS_BUTTON_BYTES];
    raw_buttons.copy_from_slice(&data[btn_start..btn_start + ROTOR_TCS_BUTTON_BYTES]);

    Ok(VpcRotorTcsInputState {
        axes,
        buttons: VpcRotorTcsButtons { raw: raw_buttons },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn make_rotor_tcs_report(axes: [u16; 3], buttons: [u8; 4]) -> Vec<u8> {
        let mut data = vec![0x01u8]; // report_id
        for ax in &axes {
            data.extend_from_slice(&ax.to_le_bytes());
        }
        data.extend_from_slice(&buttons);
        data
    }

    #[test]
    fn too_short_is_error() {
        assert!(parse_rotor_tcs_report(&[0x01; 10]).is_err());
    }

    #[test]
    fn empty_slice_is_error() {
        assert!(parse_rotor_tcs_report(&[]).is_err());
    }

    #[test]
    fn exactly_min_bytes_parses_ok() {
        let report = make_rotor_tcs_report([0u16; 3], [0u8; 4]);
        assert_eq!(report.len(), VPC_ROTOR_TCS_MIN_REPORT_BYTES);
        assert!(parse_rotor_tcs_report(&report).is_ok());
    }

    #[test]
    fn longer_report_does_not_error() {
        let mut report = make_rotor_tcs_report([0u16; 3], [0u8; 4]);
        report.extend_from_slice(&[0u8; 8]);
        assert!(parse_rotor_tcs_report(&report).is_ok());
    }

    #[test]
    fn zero_axes_parse_to_zero() {
        let report = make_rotor_tcs_report([0u16; 3], [0u8; 4]);
        let state = parse_rotor_tcs_report(&report).unwrap();
        assert_eq!(state.axes.collective, 0.0);
        assert_eq!(state.axes.throttle_idle, 0.0);
        assert_eq!(state.axes.rotary, 0.0);
    }

    #[test]
    fn max_axes_parse_to_one() {
        let report = make_rotor_tcs_report([VIRPIL_AXIS_MAX; 3], [0u8; 4]);
        let state = parse_rotor_tcs_report(&report).unwrap();
        assert!((state.axes.collective - 1.0).abs() < 1e-4);
        assert!((state.axes.throttle_idle - 1.0).abs() < 1e-4);
        assert!((state.axes.rotary - 1.0).abs() < 1e-4);
    }

    #[test]
    fn midpoint_collective_is_half() {
        let report = make_rotor_tcs_report([VIRPIL_AXIS_MAX / 2, 0, 0], [0u8; 4]);
        let state = parse_rotor_tcs_report(&report).unwrap();
        assert!((state.axes.collective - 0.5).abs() < 0.01);
    }

    #[test]
    fn no_buttons_pressed_by_default() {
        let report = make_rotor_tcs_report([0u16; 3], [0u8; 4]);
        let state = parse_rotor_tcs_report(&report).unwrap();
        assert!(state.buttons.pressed().is_empty());
    }

    #[test]
    fn button_1_detected() {
        let report = make_rotor_tcs_report([0u16; 3], [0x01, 0x00, 0x00, 0x00]);
        let state = parse_rotor_tcs_report(&report).unwrap();
        assert!(state.buttons.is_pressed(1));
        assert!(!state.buttons.is_pressed(2));
    }

    #[test]
    fn button_24_detected() {
        // button 24 = index 23 → byte 2 (23/8=2), bit 7 (23%8=7)
        let report = make_rotor_tcs_report([0u16; 3], [0x00, 0x00, 0x80, 0x00]);
        let state = parse_rotor_tcs_report(&report).unwrap();
        assert!(state.buttons.is_pressed(24));
    }

    #[test]
    fn all_buttons_pressed() {
        let report = make_rotor_tcs_report([0u16; 3], [0xFF, 0xFF, 0xFF, 0xFF]);
        let state = parse_rotor_tcs_report(&report).unwrap();
        for i in 1u8..=24 {
            assert!(state.buttons.is_pressed(i), "button {i} not pressed");
        }
    }

    #[test]
    fn out_of_range_button_returns_false() {
        let report = make_rotor_tcs_report([0u16; 3], [0xFF, 0xFF, 0xFF, 0xFF]);
        let state = parse_rotor_tcs_report(&report).unwrap();
        assert!(!state.buttons.is_pressed(0));
        assert!(!state.buttons.is_pressed(25));
    }

    #[test]
    fn error_message_contains_byte_count() {
        let err = parse_rotor_tcs_report(&[0x01; 5]).unwrap_err();
        assert!(err.to_string().contains('5'));
    }

    proptest! {
        #[test]
        fn axes_always_in_range(
            raw0 in 0u16..=u16::MAX,
            raw1 in 0u16..=u16::MAX,
            raw2 in 0u16..=u16::MAX,
        ) {
            let report = make_rotor_tcs_report([raw0, raw1, raw2], [0u8; 4]);
            let state = parse_rotor_tcs_report(&report).unwrap();
            prop_assert!((0.0..=1.0).contains(&state.axes.collective));
            prop_assert!((0.0..=1.0).contains(&state.axes.throttle_idle));
            prop_assert!((0.0..=1.0).contains(&state.axes.rotary));
        }

        #[test]
        fn random_report_does_not_panic(data in proptest::collection::vec(0u8..=255u8, 11..24)) {
            let _ = parse_rotor_tcs_report(&data);
        }

        #[test]
        fn roundtrip_axis_encode_decode(raw in 0u16..=VIRPIL_AXIS_MAX) {
            let report = make_rotor_tcs_report([raw, raw, raw], [0u8; 4]);
            let state = parse_rotor_tcs_report(&report).unwrap();
            let expected = raw as f32 / VIRPIL_AXIS_MAX as f32;
            prop_assert!((state.axes.collective - expected).abs() < 1e-4);
        }
    }
}
