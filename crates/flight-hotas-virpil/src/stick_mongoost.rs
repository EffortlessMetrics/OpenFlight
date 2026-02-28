// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the VIRPIL VPC MongoosT-50CM3 (right stick).
//!
//! # Confirmed device identifier
//!
//! VID 0x3344 (VIRPIL Controls), PID 0x4130 — sourced from the open-source
//! Rust LED control library [Buzzec/virpil](https://github.com/Buzzec/virpil).
//!
//! # Input report layout (15 bytes, community-documented)
//!
//! ```text
//! byte  0        : report_id (0x01)
//! bytes  1–10    : axes (5 × u16 LE), axis max = 16384
//!   bytes  1–2  : X (roll/left-right)
//!   bytes  3–4  : Y (pitch/front-back)
//!   bytes  5–6  : Z / twist axis
//!   bytes  7–8  : SZ (secondary Z / trim)
//!   bytes  9–10 : SL (slew lever)
//! bytes 11–14   : buttons (32 buttons → 4 bytes, LSB-first)
//! ```

use crate::VIRPIL_AXIS_MAX;
use thiserror::Error;

/// Minimum byte count for a MongoosT-50CM3 stick report.
pub const VPC_MONGOOST_STICK_MIN_REPORT_BYTES: usize = 15;

const MONGOOST_AXIS_COUNT: usize = 5;
const MONGOOST_BUTTON_BYTES: usize = 4;

/// Parse error for the VPC MongoosT-50CM3 stick.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VpcMongoostParseError {
    #[error("VPC MongoosT stick report too short: got {0} bytes (need ≥15)")]
    TooShort(usize),
}

/// 8-way hat switch position for the VPC MongoosT-50CM3 stick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VpcMongoostHat {
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

impl VpcMongoostHat {
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

/// Normalised axes from the VPC MongoosT-50CM3 stick.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VpcMongoostAxes {
    /// Primary X axis (roll / left-right). 0.0=left, 0.5=center, 1.0=right.
    pub x: f32,
    /// Primary Y axis (pitch / front-back). 0.0=forward, 0.5=center, 1.0=back.
    pub y: f32,
    /// Z/twist axis. 0.0=left, 0.5=center, 1.0=right.
    pub z: f32,
    /// SZ secondary axis (trim wheel). 0.0=min, 1.0=max.
    pub sz: f32,
    /// SL slew lever. 0.0=min, 1.0=max.
    pub sl: f32,
}

/// Button state from the VPC MongoosT-50CM3 stick (32 buttons).
///
/// Buttons 1–32 are mapped LSB-first across 4 button bytes.
/// The 4-way hat switch is encoded in the highest nibble of button byte 4
/// (bits 28–31) per the VIRPIL HID descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VpcMongoostButtons {
    /// Raw button bytes covering buttons 1–32.
    pub raw: [u8; MONGOOST_BUTTON_BYTES],
    /// Hat switch position decoded from the hat bits in the report.
    pub hat: VpcMongoostHat,
}

impl VpcMongoostButtons {
    /// Return `true` if button `n` (1-indexed, 1..=28) is pressed.
    ///
    /// Buttons 29–32 are reserved for hat switch directions and should be
    /// read via the [`hat`][VpcMongoostButtons::hat] field instead.
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

/// Full parsed input state from one VPC MongoosT-50CM3 stick HID report.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VpcMongoostInputState {
    pub axes: VpcMongoostAxes,
    pub buttons: VpcMongoostButtons,
}

/// Parse one raw HID report from the VPC MongoosT-50CM3 stick.
pub fn parse_mongoost_stick_report(
    data: &[u8],
) -> Result<VpcMongoostInputState, VpcMongoostParseError> {
    if data.len() < VPC_MONGOOST_STICK_MIN_REPORT_BYTES {
        return Err(VpcMongoostParseError::TooShort(data.len()));
    }

    let payload = &data[1..]; // skip report_id
    let mut raw_axes = [0u16; MONGOOST_AXIS_COUNT];
    for (i, v) in raw_axes.iter_mut().enumerate() {
        *v = u16::from_le_bytes([payload[i * 2], payload[i * 2 + 1]]);
    }

    let normalize = |v: u16| (v as f32 / VIRPIL_AXIS_MAX as f32).clamp(0.0, 1.0);

    let axes = VpcMongoostAxes {
        x: normalize(raw_axes[0]),
        y: normalize(raw_axes[1]),
        z: normalize(raw_axes[2]),
        sz: normalize(raw_axes[3]),
        sl: normalize(raw_axes[4]),
    };

    let btn_start = 1 + MONGOOST_AXIS_COUNT * 2;
    let mut raw_buttons = [0u8; MONGOOST_BUTTON_BYTES];
    raw_buttons.copy_from_slice(&data[btn_start..btn_start + MONGOOST_BUTTON_BYTES]);

    // Hat is encoded in the high nibble of the last button byte (bits 28..31)
    let hat_nibble = (raw_buttons[3] >> 4) & 0x0F;
    let hat = VpcMongoostHat::from_raw(hat_nibble);

    Ok(VpcMongoostInputState {
        axes,
        buttons: VpcMongoostButtons {
            raw: raw_buttons,
            hat,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn make_mongoost_report(axes: [u16; 5], buttons: [u8; 4]) -> Vec<u8> {
        let mut data = vec![0x01u8];
        for ax in &axes {
            data.extend_from_slice(&ax.to_le_bytes());
        }
        data.extend_from_slice(&buttons);
        data
    }

    #[test]
    fn too_short_is_error() {
        assert!(parse_mongoost_stick_report(&[0x01; 14]).is_err());
    }

    #[test]
    fn all_zero_axes_parse_to_zero() {
        let report = make_mongoost_report([0u16; 5], [0u8; 4]);
        let state = parse_mongoost_stick_report(&report).unwrap();
        assert_eq!(state.axes.x, 0.0);
        assert_eq!(state.axes.y, 0.0);
        assert_eq!(state.axes.z, 0.0);
    }

    #[test]
    fn max_axes_parse_to_one() {
        let report = make_mongoost_report([VIRPIL_AXIS_MAX; 5], [0u8; 4]);
        let state = parse_mongoost_stick_report(&report).unwrap();
        assert!((state.axes.x - 1.0).abs() < 1e-4);
        assert!((state.axes.y - 1.0).abs() < 1e-4);
        assert!((state.axes.z - 1.0).abs() < 1e-4);
        assert!((state.axes.sz - 1.0).abs() < 1e-4);
        assert!((state.axes.sl - 1.0).abs() < 1e-4);
    }

    #[test]
    fn no_buttons_pressed_by_default() {
        let report = make_mongoost_report([0u16; 5], [0u8; 4]);
        let state = parse_mongoost_stick_report(&report).unwrap();
        assert!(state.buttons.pressed().is_empty());
    }

    #[test]
    fn button_1_detected() {
        let mut buttons = [0u8; 4];
        buttons[0] = 0x01;
        let report = make_mongoost_report([0u16; 5], buttons);
        let state = parse_mongoost_stick_report(&report).unwrap();
        assert!(state.buttons.is_pressed(1));
        assert!(!state.buttons.is_pressed(2));
    }

    #[test]
    fn button_28_detected() {
        let mut buttons = [0u8; 4];
        // button 28 = index 27 → byte 3, bit 3
        buttons[3] = 1 << 3;
        let report = make_mongoost_report([0u16; 5], buttons);
        let state = parse_mongoost_stick_report(&report).unwrap();
        assert!(state.buttons.is_pressed(28));
    }

    #[test]
    fn hat_north_detected() {
        let mut buttons = [0u8; 4];
        // Hat nibble in high bits of byte 3: North = 0 → bits 4..7 = 0b0000
        buttons[3] = 0x00;
        let report = make_mongoost_report([0u16; 5], buttons);
        let state = parse_mongoost_stick_report(&report).unwrap();
        assert_eq!(state.buttons.hat, VpcMongoostHat::North);
    }

    #[test]
    fn hat_south_detected() {
        let mut buttons = [0u8; 4];
        // South = 4 → bits 4..7 = 0b0100 → byte3 |= (4 << 4) = 0x40
        buttons[3] = 0x40;
        let report = make_mongoost_report([0u16; 5], buttons);
        let state = parse_mongoost_stick_report(&report).unwrap();
        assert_eq!(state.buttons.hat, VpcMongoostHat::South);
    }

    #[test]
    fn hat_center_for_unknown_value() {
        let mut buttons = [0u8; 4];
        // Value 0xF in high nibble → Center
        buttons[3] = 0xF0;
        let report = make_mongoost_report([0u16; 5], buttons);
        let state = parse_mongoost_stick_report(&report).unwrap();
        assert_eq!(state.buttons.hat, VpcMongoostHat::Center);
    }

    #[test]
    fn out_of_range_button_returns_false() {
        let report = make_mongoost_report([0u16; 5], [0xFFu8; 4]);
        let state = parse_mongoost_stick_report(&report).unwrap();
        assert!(!state.buttons.is_pressed(0));
        assert!(!state.buttons.is_pressed(29));
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
            let report = make_mongoost_report([raw0, raw1, raw2, raw3, raw4], [0u8; 4]);
            let state = parse_mongoost_stick_report(&report).unwrap();
            assert!((0.0..=1.0).contains(&state.axes.x));
            assert!((0.0..=1.0).contains(&state.axes.y));
            assert!((0.0..=1.0).contains(&state.axes.z));
            assert!((0.0..=1.0).contains(&state.axes.sz));
            assert!((0.0..=1.0).contains(&state.axes.sl));
        }

        #[test]
        fn random_report_does_not_panic(data in proptest::collection::vec(0u8..=255u8, 15..32)) {
            let _ = parse_mongoost_stick_report(&data);
        }
    }
}
