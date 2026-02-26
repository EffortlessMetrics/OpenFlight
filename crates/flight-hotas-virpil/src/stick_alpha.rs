// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the VIRPIL VPC Constellation Alpha (left grip on CM3 base).
//!
//! # Confirmed device identifier
//!
//! VID 0x3344 (VIRPIL Controls), PID 0x838F — confirmed via linux-hardware.org
//! probe data ("L-VPC Stick MT-50CM3", 1 probe).
//!
//! # Input report layout (15 bytes, same firmware family as MongoosT-50CM3)
//!
//! ```text
//! byte  0        : report_id (0x01)
//! bytes  1–10    : axes (5 × u16 LE), axis max = 16384
//!   bytes  1–2  : X (roll / left-right)
//!   bytes  3–4  : Y (pitch / front-back)
//!   bytes  5–6  : Z (twist / rotary knob)
//!   bytes  7–8  : SZ (secondary rotary)
//!   bytes  9–10 : SL (slew lever)
//! bytes 11–14   : buttons (28 buttons + hat → 4 bytes, LSB-first)
//! ```
//!
//! The hat switch is encoded in the high nibble (bits 4–7) of button byte 4
//! (byte offset 14 in the report). Values 0–7 map to N/NE/E/SE/S/SW/W/NW;
//! all other values mean center.

use crate::VIRPIL_AXIS_MAX;
use thiserror::Error;

/// Minimum byte count for a Constellation Alpha stick HID report.
pub const VPC_ALPHA_MIN_REPORT_BYTES: usize = 15;

const ALPHA_AXIS_COUNT: usize = 5;
const ALPHA_BUTTON_BYTES: usize = 4;

/// Parse error for the VPC Constellation Alpha stick.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VpcAlphaParseError {
    #[error("VPC Constellation Alpha report too short: got {0} bytes (need ≥15)")]
    TooShort(usize),
}

/// 8-way hat switch position for the VPC Constellation Alpha stick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VpcAlphaHat {
    /// Hat is centred / not pressed.
    #[default]
    Center,
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
    NorthWest,
}

impl VpcAlphaHat {
    fn from_raw(raw: u8) -> Self {
        match raw {
            0 => Self::North,
            1 => Self::NorthEast,
            2 => Self::East,
            3 => Self::SouthEast,
            4 => Self::South,
            5 => Self::SouthWest,
            6 => Self::West,
            7 => Self::NorthWest,
            _ => Self::Center,
        }
    }
}

/// Normalised axes from the VPC Constellation Alpha stick.
///
/// All values are in the range `[0.0, 1.0]`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VpcAlphaAxes {
    /// Primary X axis (roll / left-right). 0.0 = left, 0.5 = centre, 1.0 = right.
    pub x: f32,
    /// Primary Y axis (pitch / front-back). 0.0 = forward, 0.5 = centre, 1.0 = back.
    pub y: f32,
    /// Z axis (twist / rotary knob). 0.0 = min, 0.5 = centre, 1.0 = max.
    pub z: f32,
    /// SZ secondary rotary. 0.0 = min, 1.0 = max.
    pub sz: f32,
    /// SL slew lever. 0.0 = min, 1.0 = max.
    pub sl: f32,
}

/// Button state from the VPC Constellation Alpha stick (28 buttons + hat).
///
/// Buttons 1–28 are mapped LSB-first across the first 3.5 bytes. The hat
/// occupies the high nibble (bits 4–7) of the 4th button byte and should be
/// read via the [`hat`][VpcAlphaButtons::hat] field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VpcAlphaButtons {
    /// Raw button bytes covering buttons 1–28 (+ hat in high nibble of byte 4).
    pub raw: [u8; ALPHA_BUTTON_BYTES],
    /// Hat switch position decoded from the report.
    pub hat: VpcAlphaHat,
}

impl VpcAlphaButtons {
    /// Return `true` if button `n` (1-indexed, 1..=28) is pressed.
    ///
    /// Buttons 29–32 encode the hat switch direction and must be read via
    /// [`hat`][VpcAlphaButtons::hat]; this method returns `false` for them.
    pub fn is_pressed(&self, n: u8) -> bool {
        if !(1..=28).contains(&n) {
            return false;
        }
        let idx = (n - 1) as usize;
        let byte = idx / 8;
        let bit = idx % 8;
        (self.raw[byte] >> bit) & 1 == 1
    }

    /// Return a `Vec` of pressed button numbers (1-indexed, 1..=28).
    pub fn pressed(&self) -> Vec<u8> {
        (1u8..=28).filter(|&n| self.is_pressed(n)).collect()
    }
}

/// Full parsed input state from one VPC Constellation Alpha HID report.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VpcAlphaInputState {
    pub axes: VpcAlphaAxes,
    pub buttons: VpcAlphaButtons,
}

/// Parse one raw HID report from the VPC Constellation Alpha stick.
pub fn parse_alpha_report(data: &[u8]) -> Result<VpcAlphaInputState, VpcAlphaParseError> {
    if data.len() < VPC_ALPHA_MIN_REPORT_BYTES {
        return Err(VpcAlphaParseError::TooShort(data.len()));
    }

    let payload = &data[1..]; // skip report_id byte
    let mut raw_axes = [0u16; ALPHA_AXIS_COUNT];
    for (i, v) in raw_axes.iter_mut().enumerate() {
        *v = u16::from_le_bytes([payload[i * 2], payload[i * 2 + 1]]);
    }

    let normalize = |v: u16| (v as f32 / VIRPIL_AXIS_MAX as f32).clamp(0.0, 1.0);

    let axes = VpcAlphaAxes {
        x: normalize(raw_axes[0]),
        y: normalize(raw_axes[1]),
        z: normalize(raw_axes[2]),
        sz: normalize(raw_axes[3]),
        sl: normalize(raw_axes[4]),
    };

    let btn_start = 1 + ALPHA_AXIS_COUNT * 2;
    let mut raw_buttons = [0u8; ALPHA_BUTTON_BYTES];
    raw_buttons.copy_from_slice(&data[btn_start..btn_start + ALPHA_BUTTON_BYTES]);

    // Hat is encoded in the high nibble of the last button byte (bits 4..7 of byte 14).
    let hat_nibble = (raw_buttons[3] >> 4) & 0x0F;
    let hat = VpcAlphaHat::from_raw(hat_nibble);

    Ok(VpcAlphaInputState {
        axes,
        buttons: VpcAlphaButtons {
            raw: raw_buttons,
            hat,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn make_alpha_report(axes: [u16; 5], buttons: [u8; 4]) -> Vec<u8> {
        let mut data = vec![0x01u8]; // report_id
        for ax in &axes {
            data.extend_from_slice(&ax.to_le_bytes());
        }
        data.extend_from_slice(&buttons);
        data
    }

    #[test]
    fn too_short_returns_error() {
        assert!(parse_alpha_report(&[0x01; 14]).is_err());
    }

    #[test]
    fn empty_slice_returns_error() {
        assert!(parse_alpha_report(&[]).is_err());
    }

    #[test]
    fn exactly_min_bytes_parses_ok() {
        let report = make_alpha_report([0u16; 5], [0u8; 4]);
        assert_eq!(report.len(), VPC_ALPHA_MIN_REPORT_BYTES);
        assert!(parse_alpha_report(&report).is_ok());
    }

    #[test]
    fn longer_report_does_not_error() {
        let mut report = make_alpha_report([0u16; 5], [0u8; 4]);
        report.extend_from_slice(&[0u8; 8]); // extra padding
        assert!(parse_alpha_report(&report).is_ok());
    }

    #[test]
    fn all_zero_axes_parse_to_zero() {
        let report = make_alpha_report([0u16; 5], [0u8; 4]);
        let state = parse_alpha_report(&report).unwrap();
        assert_eq!(state.axes.x, 0.0);
        assert_eq!(state.axes.y, 0.0);
        assert_eq!(state.axes.z, 0.0);
        assert_eq!(state.axes.sz, 0.0);
        assert_eq!(state.axes.sl, 0.0);
    }

    #[test]
    fn max_axes_parse_to_one() {
        let report = make_alpha_report([VIRPIL_AXIS_MAX; 5], [0u8; 4]);
        let state = parse_alpha_report(&report).unwrap();
        assert!((state.axes.x - 1.0).abs() < 1e-4);
        assert!((state.axes.y - 1.0).abs() < 1e-4);
        assert!((state.axes.z - 1.0).abs() < 1e-4);
        assert!((state.axes.sz - 1.0).abs() < 1e-4);
        assert!((state.axes.sl - 1.0).abs() < 1e-4);
    }

    #[test]
    fn no_buttons_pressed_by_default() {
        let report = make_alpha_report([0u16; 5], [0u8; 4]);
        let state = parse_alpha_report(&report).unwrap();
        assert!(state.buttons.pressed().is_empty());
    }

    #[test]
    fn button_1_detected() {
        let mut buttons = [0u8; 4];
        buttons[0] = 0x01; // bit 0 of byte 0 → button 1
        let report = make_alpha_report([0u16; 5], buttons);
        let state = parse_alpha_report(&report).unwrap();
        assert!(state.buttons.is_pressed(1));
        assert!(!state.buttons.is_pressed(2));
    }

    #[test]
    fn button_8_detected() {
        let mut buttons = [0u8; 4];
        buttons[0] = 0x80; // bit 7 of byte 0 → button 8
        let report = make_alpha_report([0u16; 5], buttons);
        let state = parse_alpha_report(&report).unwrap();
        assert!(state.buttons.is_pressed(8));
        assert!(!state.buttons.is_pressed(7));
        assert!(!state.buttons.is_pressed(9));
    }

    #[test]
    fn button_28_detected() {
        let mut buttons = [0u8; 4];
        // button 28 = index 27 → byte 3 (27/8=3), bit 3 (27%8=3)
        buttons[3] = 1 << 3;
        let report = make_alpha_report([0u16; 5], buttons);
        let state = parse_alpha_report(&report).unwrap();
        assert!(state.buttons.is_pressed(28));
        assert!(!state.buttons.is_pressed(27));
    }

    #[test]
    fn out_of_range_button_returns_false() {
        let report = make_alpha_report([0u16; 5], [0xFFu8; 4]);
        let state = parse_alpha_report(&report).unwrap();
        assert!(!state.buttons.is_pressed(0));
        assert!(!state.buttons.is_pressed(29));
        assert!(!state.buttons.is_pressed(255));
    }

    #[test]
    fn hat_north_detected() {
        let mut buttons = [0u8; 4];
        // North = 0 → high nibble of byte 3 = 0b0000
        buttons[3] = 0x00;
        let report = make_alpha_report([0u16; 5], buttons);
        let state = parse_alpha_report(&report).unwrap();
        assert_eq!(state.buttons.hat, VpcAlphaHat::North);
    }

    #[test]
    fn hat_south_detected() {
        let mut buttons = [0u8; 4];
        // South = 4 → high nibble of byte 3 = 0b0100 → byte3 = (4 << 4) = 0x40
        buttons[3] = 0x40;
        let report = make_alpha_report([0u16; 5], buttons);
        let state = parse_alpha_report(&report).unwrap();
        assert_eq!(state.buttons.hat, VpcAlphaHat::South);
    }

    #[test]
    fn hat_northeast_detected() {
        let mut buttons = [0u8; 4];
        // NorthEast = 1 → high nibble = 0b0001 → byte3 = (1 << 4) = 0x10
        buttons[3] = 0x10;
        let report = make_alpha_report([0u16; 5], buttons);
        let state = parse_alpha_report(&report).unwrap();
        assert_eq!(state.buttons.hat, VpcAlphaHat::NorthEast);
    }

    #[test]
    fn hat_center_for_unknown_value() {
        let mut buttons = [0u8; 4];
        // Value 0xF in high nibble → Center
        buttons[3] = 0xF0;
        let report = make_alpha_report([0u16; 5], buttons);
        let state = parse_alpha_report(&report).unwrap();
        assert_eq!(state.buttons.hat, VpcAlphaHat::Center);
    }

    #[test]
    fn hat_and_button_coexist() {
        let mut buttons = [0u8; 4];
        // Button 1 pressed AND hat = NorthWest (7 << 4 = 0x70)
        buttons[0] = 0x01;
        buttons[3] = 0x70;
        let report = make_alpha_report([0u16; 5], buttons);
        let state = parse_alpha_report(&report).unwrap();
        assert!(state.buttons.is_pressed(1));
        assert_eq!(state.buttons.hat, VpcAlphaHat::NorthWest);
    }

    #[test]
    fn error_message_contains_byte_count() {
        let err = parse_alpha_report(&[0x01; 5]).unwrap_err();
        assert!(err.to_string().contains('5'));
    }

    proptest! {
        #[test]
        fn axes_always_in_range(
            raw0 in 0u16..=u16::MAX,
            raw1 in 0u16..=u16::MAX,
            raw2 in 0u16..=u16::MAX,
            raw3 in 0u16..=u16::MAX,
            raw4 in 0u16..=u16::MAX,
        ) {
            let report = make_alpha_report([raw0, raw1, raw2, raw3, raw4], [0u8; 4]);
            let state = parse_alpha_report(&report).unwrap();
            prop_assert!(state.axes.x >= 0.0 && state.axes.x <= 1.0);
            prop_assert!(state.axes.y >= 0.0 && state.axes.y <= 1.0);
            prop_assert!(state.axes.z >= 0.0 && state.axes.z <= 1.0);
            prop_assert!(state.axes.sz >= 0.0 && state.axes.sz <= 1.0);
            prop_assert!(state.axes.sl >= 0.0 && state.axes.sl <= 1.0);
        }

        #[test]
        fn random_report_does_not_panic(data in proptest::collection::vec(0u8..=255u8, 15..32)) {
            let _ = parse_alpha_report(&data);
        }

        #[test]
        fn roundtrip_axis_encode_decode(raw in 0u16..=VIRPIL_AXIS_MAX) {
            let report = make_alpha_report([raw, raw, raw, raw, raw], [0u8; 4]);
            let state = parse_alpha_report(&report).unwrap();
            let expected = raw as f32 / VIRPIL_AXIS_MAX as f32;
            prop_assert!((state.axes.x - expected).abs() < 1e-4);
            prop_assert!((state.axes.y - expected).abs() < 1e-4);
        }
    }
}
