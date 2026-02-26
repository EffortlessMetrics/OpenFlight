// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the VIRPIL VPC Control Panel 2 (right panel).
//!
//! # Confirmed device identifier
//!
//! VID 0x3344 (VIRPIL Controls), PID 0x0259 — sourced from the open-source
//! Rust LED control library [Buzzec/virpil](https://github.com/Buzzec/virpil)
//! (`src/right_panel.rs`, `const PID: u16 = 0x0259`).
//!
//! # Input report layout (11 bytes, community-documented)
//!
//! The Control Panel 2 exposes 2 analogue axes (A1/A2, e.g. rotary knobs)
//! and 47 buttons (toggle switches, rotary encoders, push buttons).
//!
//! ```text
//! byte  0        : report_id (0x01)
//! bytes  1–2    : axis A1 (u16 LE, 14-bit, range 0–16384)
//! bytes  3–4    : axis A2 (u16 LE, 14-bit, range 0–16384)
//! bytes  5–10   : buttons (47 buttons → 6 bytes LSB-first; bit 7 of byte 10 unused)
//! ```
//!
//! Report format follows the generic VIRPIL frame documented in `crate::lib`.
//! Axis max (16384) is [`crate::VIRPIL_AXIS_MAX`].
//!
//! # Notes
//!
//! The exact byte mapping was inferred from the VIRPIL generic HID report
//! format (see [`crate`]) and cross-checked against the compat manifest
//! `compat/devices/virpil/control-panel-2.yaml`. HIL validation is pending.

use crate::VIRPIL_AXIS_MAX;
use thiserror::Error;

/// Minimum byte count for a VPC Control Panel 2 report.
pub const VPC_PANEL2_MIN_REPORT_BYTES: usize = 11;

const PANEL2_BUTTON_BYTES: usize = 6;
/// Number of buttons on the VPC Control Panel 2.
pub const PANEL2_BUTTON_COUNT: u8 = 47;

/// Parse error for the VPC Control Panel 2.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VpcPanel2ParseError {
    #[error("VPC Control Panel 2 report too short: got {0} bytes (need ≥11)")]
    TooShort(usize),
}

/// Analogue axis state from the VPC Control Panel 2.
///
/// Both axes are 14-bit unsigned values in the range `0..=16384`
/// ([`VIRPIL_AXIS_MAX`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VpcPanel2Axes {
    /// Axis A1 raw value (rotary knob, 14-bit).
    pub a1_raw: u16,
    /// Axis A2 raw value (rotary knob, 14-bit).
    pub a2_raw: u16,
}

impl VpcPanel2Axes {
    /// Axis A1 normalised to `0.0..=1.0`.
    #[inline]
    pub fn a1_normalised(&self) -> f32 {
        self.a1_raw as f32 / VIRPIL_AXIS_MAX as f32
    }

    /// Axis A2 normalised to `0.0..=1.0`.
    #[inline]
    pub fn a2_normalised(&self) -> f32 {
        self.a2_raw as f32 / VIRPIL_AXIS_MAX as f32
    }
}

/// Button state from the VPC Control Panel 2 (47 buttons).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VpcPanel2Buttons {
    pub raw: [u8; PANEL2_BUTTON_BYTES],
}

impl Default for VpcPanel2Buttons {
    fn default() -> Self {
        Self {
            raw: [0u8; PANEL2_BUTTON_BYTES],
        }
    }
}

impl VpcPanel2Buttons {
    /// Return `true` if button `n` (1-indexed, `1..=47`) is pressed.
    pub fn is_pressed(&self, n: u8) -> bool {
        if !(1..=PANEL2_BUTTON_COUNT).contains(&n) {
            return false;
        }
        let idx = (n - 1) as usize;
        let byte = idx / 8;
        let bit = idx % 8;
        (self.raw[byte] >> bit) & 1 == 1
    }

    /// Return a `Vec` of pressed button numbers (1-indexed).
    pub fn pressed(&self) -> Vec<u8> {
        (1u8..=PANEL2_BUTTON_COUNT)
            .filter(|&n| self.is_pressed(n))
            .collect()
    }
}

/// Full parsed input state from one VPC Control Panel 2 HID report.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VpcPanel2InputState {
    pub axes: VpcPanel2Axes,
    pub buttons: VpcPanel2Buttons,
}

/// Parse one raw HID report from the VPC Control Panel 2.
///
/// # Report layout
///
/// ```text
/// byte 0       : report_id (ignored)
/// bytes 1–2   : axis A1 (u16 LE)
/// bytes 3–4   : axis A2 (u16 LE)
/// bytes 5–10  : 47 button bits (LSB-first)
/// ```
pub fn parse_panel2_report(data: &[u8]) -> Result<VpcPanel2InputState, VpcPanel2ParseError> {
    if data.len() < VPC_PANEL2_MIN_REPORT_BYTES {
        return Err(VpcPanel2ParseError::TooShort(data.len()));
    }
    let a1_raw = u16::from_le_bytes([data[1], data[2]]);
    let a2_raw = u16::from_le_bytes([data[3], data[4]]);
    let mut raw = [0u8; PANEL2_BUTTON_BYTES];
    raw.copy_from_slice(&data[5..5 + PANEL2_BUTTON_BYTES]);
    Ok(VpcPanel2InputState {
        axes: VpcPanel2Axes { a1_raw, a2_raw },
        buttons: VpcPanel2Buttons { raw },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn make_panel2_report(a1: u16, a2: u16, buttons: [u8; 6]) -> Vec<u8> {
        let mut data = vec![0x01u8];
        data.extend_from_slice(&a1.to_le_bytes());
        data.extend_from_slice(&a2.to_le_bytes());
        data.extend_from_slice(&buttons);
        data
    }

    #[test]
    fn too_short_is_error() {
        assert!(parse_panel2_report(&[0x01; 10]).is_err());
    }

    #[test]
    fn exact_length_is_ok() {
        let report = make_panel2_report(0, 0, [0u8; 6]);
        assert!(parse_panel2_report(&report).is_ok());
    }

    #[test]
    fn no_buttons_by_default() {
        let report = make_panel2_report(0, 0, [0u8; 6]);
        let state = parse_panel2_report(&report).unwrap();
        assert!(state.buttons.pressed().is_empty());
    }

    #[test]
    fn axes_round_trip() {
        let report = make_panel2_report(8192, 16384, [0u8; 6]);
        let state = parse_panel2_report(&report).unwrap();
        assert_eq!(state.axes.a1_raw, 8192);
        assert_eq!(state.axes.a2_raw, 16384);
    }

    #[test]
    fn axis_normalised_midpoint() {
        let report = make_panel2_report(8192, 0, [0u8; 6]);
        let state = parse_panel2_report(&report).unwrap();
        let norm = state.axes.a1_normalised();
        assert!((norm - 0.5).abs() < 0.001, "expected ~0.5, got {norm}");
    }

    #[test]
    fn axis_normalised_full() {
        let report = make_panel2_report(16384, 16384, [0u8; 6]);
        let state = parse_panel2_report(&report).unwrap();
        assert!((state.axes.a1_normalised() - 1.0).abs() < 0.001);
        assert!((state.axes.a2_normalised() - 1.0).abs() < 0.001);
    }

    #[test]
    fn button_1_detected() {
        let mut buttons = [0u8; 6];
        buttons[0] = 0x01;
        let report = make_panel2_report(0, 0, buttons);
        let state = parse_panel2_report(&report).unwrap();
        assert!(state.buttons.is_pressed(1));
        assert!(!state.buttons.is_pressed(2));
    }

    #[test]
    fn button_47_detected() {
        let mut buttons = [0u8; 6];
        // button 47 = index 46 → byte 5, bit 6
        buttons[5] = 0x40;
        let report = make_panel2_report(0, 0, buttons);
        let state = parse_panel2_report(&report).unwrap();
        assert!(state.buttons.is_pressed(47));
        // bit 7 of last byte is unused; must not be reported
        assert!(!state.buttons.is_pressed(48));
    }

    #[test]
    fn all_buttons_pressed() {
        let report = make_panel2_report(0, 0, [0xFFu8; 6]);
        let state = parse_panel2_report(&report).unwrap();
        for i in 1u8..=47 {
            assert!(state.buttons.is_pressed(i), "button {i} not pressed");
        }
    }

    #[test]
    fn out_of_range_button_returns_false() {
        let report = make_panel2_report(0, 0, [0xFFu8; 6]);
        let state = parse_panel2_report(&report).unwrap();
        assert!(!state.buttons.is_pressed(0));
        assert!(!state.buttons.is_pressed(48));
    }

    proptest! {
        #[test]
        fn random_report_does_not_panic(data in proptest::collection::vec(0u8..=255u8, 11..16)) {
            let _ = parse_panel2_report(&data);
        }
    }
}
