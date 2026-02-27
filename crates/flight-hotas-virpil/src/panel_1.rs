// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the VIRPIL VPC Control Panel 1 (panel device).
//!
//! # Confirmed device identifier
//!
//! VID 0x3344 (VIRPIL Controls), PID 0x025B — sourced from the open-source
//! Rust LED control library [Buzzec/virpil](https://github.com/Buzzec/virpil).
//!
//! # Input report layout (7 bytes, community-documented)
//!
//! The Control Panel 1 has no analog axes; it is a pure button panel.
//!
//! ```text
//! byte 0   : report_id (0x01)
//! bytes 1–6: buttons (48 buttons → 6 bytes, LSB-first)
//! ```

use thiserror::Error;

/// Minimum byte count for a VPC Control Panel 1 report.
pub const VPC_PANEL1_MIN_REPORT_BYTES: usize = 7;

const PANEL1_BUTTON_BYTES: usize = 6;

/// Parse error for the VPC Control Panel 1.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VpcPanel1ParseError {
    #[error("VPC Control Panel 1 report too short: got {0} bytes (need ≥7)")]
    TooShort(usize),
}

/// Button state from the VPC Control Panel 1 (48 buttons).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VpcPanel1Buttons {
    pub raw: [u8; PANEL1_BUTTON_BYTES],
}

impl Default for VpcPanel1Buttons {
    fn default() -> Self {
        Self {
            raw: [0u8; PANEL1_BUTTON_BYTES],
        }
    }
}

impl VpcPanel1Buttons {
    /// Return `true` if button `n` (1-indexed, 1..=48) is pressed.
    pub fn is_pressed(&self, n: u8) -> bool {
        if !(1..=48).contains(&n) {
            return false;
        }
        let idx = (n - 1) as usize;
        let byte = idx / 8;
        let bit = idx % 8;
        (self.raw[byte] >> bit) & 1 == 1
    }

    /// Return a `Vec` of pressed button numbers (1-indexed).
    pub fn pressed(&self) -> Vec<u8> {
        (1u8..=48).filter(|&n| self.is_pressed(n)).collect()
    }
}

/// Full parsed input state from one VPC Control Panel 1 HID report.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VpcPanel1InputState {
    pub buttons: VpcPanel1Buttons,
}

/// Parse one raw HID report from the VPC Control Panel 1.
pub fn parse_panel1_report(data: &[u8]) -> Result<VpcPanel1InputState, VpcPanel1ParseError> {
    if data.len() < VPC_PANEL1_MIN_REPORT_BYTES {
        return Err(VpcPanel1ParseError::TooShort(data.len()));
    }
    let mut raw = [0u8; PANEL1_BUTTON_BYTES];
    raw.copy_from_slice(&data[1..1 + PANEL1_BUTTON_BYTES]);
    Ok(VpcPanel1InputState {
        buttons: VpcPanel1Buttons { raw },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn make_panel1_report(buttons: [u8; 6]) -> Vec<u8> {
        let mut data = vec![0x01u8];
        data.extend_from_slice(&buttons);
        data
    }

    #[test]
    fn too_short_is_error() {
        assert!(parse_panel1_report(&[0x01; 6]).is_err());
    }

    #[test]
    fn no_buttons_by_default() {
        let report = make_panel1_report([0u8; 6]);
        let state = parse_panel1_report(&report).unwrap();
        assert!(state.buttons.pressed().is_empty());
    }

    #[test]
    fn button_1_detected() {
        let mut buttons = [0u8; 6];
        buttons[0] = 0x01;
        let report = make_panel1_report(buttons);
        let state = parse_panel1_report(&report).unwrap();
        assert!(state.buttons.is_pressed(1));
    }

    #[test]
    fn button_48_detected() {
        let mut buttons = [0u8; 6];
        // button 48 = index 47 → byte 5, bit 7
        buttons[5] = 0x80;
        let report = make_panel1_report(buttons);
        let state = parse_panel1_report(&report).unwrap();
        assert!(state.buttons.is_pressed(48));
    }

    #[test]
    fn all_buttons_pressed() {
        let report = make_panel1_report([0xFFu8; 6]);
        let state = parse_panel1_report(&report).unwrap();
        for i in 1u8..=48 {
            assert!(state.buttons.is_pressed(i), "button {i} not pressed");
        }
    }

    #[test]
    fn out_of_range_button_returns_false() {
        let report = make_panel1_report([0xFFu8; 6]);
        let state = parse_panel1_report(&report).unwrap();
        assert!(!state.buttons.is_pressed(0));
        assert!(!state.buttons.is_pressed(49));
    }

    proptest! {
        #[test]
        fn random_report_does_not_panic(data in proptest::collection::vec(0u8..=255u8, 7..16)) {
            let _ = parse_panel1_report(&data);
        }
    }
}
