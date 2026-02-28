// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the VIRPIL VPC Throttle CM3.
//!
//! # Confirmed device identifier
//!
//! VID 0x3344 (VIRPIL Controls), PID 0x0194 — sourced from the open-source
//! Rust LED control library [Buzzec/virpil](https://github.com/Buzzec/virpil).
//!
//! # Input report layout (23 bytes, community-documented)
//!
//! ```text
//! byte  0         : report_id (0x01)
//! bytes  1–12     : axes (6 × u16 LE), axis max = 16384
//!   bytes  1–2   : left throttle
//!   bytes  3–4   : right throttle
//!   bytes  5–6   : flaps lever
//!   bytes  7–8   : SCX (slew control X)
//!   bytes  9–10  : SCY (slew control Y)
//!   bytes 11–12  : slider
//! bytes 13–22    : buttons (78 buttons → 10 bytes, LSB-first)
//! ```
//!
//! Axis normalisation: raw value 0→0.0, raw value 16383→1.0.

use thiserror::Error;

/// Maximum raw axis value for VIRPIL VPC devices.
///
/// From `virpil_device.rs` in Buzzec/virpil: `u16::from_le_bytes([0, 64])` = 16384.
pub const VIRPIL_AXIS_MAX: u16 = crate::VIRPIL_AXIS_MAX;

/// Minimum byte count for a CM3 Throttle report (report_id + 6 axes + 10 button bytes).
pub const VPC_CM3_THROTTLE_MIN_REPORT_BYTES: usize = 23;

/// Number of axes in the CM3 Throttle report.
const CM3_AXIS_COUNT: usize = 6;

/// Number of button bytes in the CM3 Throttle report.
const CM3_BUTTON_BYTES: usize = 10;

/// Parse error for the VPC Throttle CM3.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VpcCm3ParseError {
    #[error("VPC CM3 Throttle report too short: got {0} bytes (need ≥23)")]
    TooShort(usize),
}

/// Normalised axes from the VPC Throttle CM3.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VpcCm3ThrottleAxes {
    /// Left throttle lever. 0.0 = idle, 1.0 = full.
    pub left_throttle: f32,
    /// Right throttle lever. 0.0 = idle, 1.0 = full.
    pub right_throttle: f32,
    /// Flaps lever / detent. 0.0 = retracted, 1.0 = full extension.
    pub flaps: f32,
    /// Slew control X. 0.0 = left, 0.5 = center, 1.0 = right.
    pub scx: f32,
    /// Slew control Y. 0.0 = forward, 0.5 = center, 1.0 = back.
    pub scy: f32,
    /// Miscellaneous slider / scroll wheel. 0.0 = min, 1.0 = max.
    pub slider: f32,
}

/// Button state from the VPC Throttle CM3 (78 buttons).
///
/// Buttons are numbered 1–78, stored as 1-indexed bitmask across 10 bytes.
/// Access with [`VpcCm3ThrottleButtons::is_pressed`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VpcCm3ThrottleButtons {
    /// Raw button bytes, 10 bytes covering buttons 1–78 (LSB-first per byte).
    pub raw: [u8; CM3_BUTTON_BYTES],
}

impl Default for VpcCm3ThrottleButtons {
    fn default() -> Self {
        Self {
            raw: [0u8; CM3_BUTTON_BYTES],
        }
    }
}

impl VpcCm3ThrottleButtons {
    /// Return `true` if button `n` (1-indexed, 1..=78) is pressed.
    pub fn is_pressed(&self, n: u8) -> bool {
        if !(1..=78).contains(&n) {
            return false;
        }
        let idx = (n - 1) as usize;
        let byte = idx / 8;
        let bit = idx % 8;
        (self.raw[byte] >> bit) & 1 == 1
    }

    /// Return a `Vec` of pressed button numbers (1-indexed).
    pub fn pressed(&self) -> Vec<u8> {
        (1u8..=78).filter(|&n| self.is_pressed(n)).collect()
    }
}

/// Full parsed input state from one VPC Throttle CM3 HID report.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VpcCm3ThrottleInputState {
    pub axes: VpcCm3ThrottleAxes,
    pub buttons: VpcCm3ThrottleButtons,
}

/// Parse one raw HID report from the VPC Throttle CM3.
///
/// The first byte is the report ID (0x01), followed by 6 × 2-byte axes and
/// 10 button bytes, for a total of 23 bytes minimum.
pub fn parse_cm3_throttle_report(
    data: &[u8],
) -> Result<VpcCm3ThrottleInputState, VpcCm3ParseError> {
    if data.len() < VPC_CM3_THROTTLE_MIN_REPORT_BYTES {
        return Err(VpcCm3ParseError::TooShort(data.len()));
    }

    // Skip report_id byte, read 6 axes
    let payload = &data[1..];
    let mut raw_axes = [0u16; CM3_AXIS_COUNT];
    for (i, v) in raw_axes.iter_mut().enumerate() {
        *v = u16::from_le_bytes([payload[i * 2], payload[i * 2 + 1]]);
    }

    let normalize_unipolar = |v: u16| (v as f32 / VIRPIL_AXIS_MAX as f32).clamp(0.0, 1.0);

    let axes = VpcCm3ThrottleAxes {
        left_throttle: normalize_unipolar(raw_axes[0]),
        right_throttle: normalize_unipolar(raw_axes[1]),
        flaps: normalize_unipolar(raw_axes[2]),
        scx: normalize_unipolar(raw_axes[3]),
        scy: normalize_unipolar(raw_axes[4]),
        slider: normalize_unipolar(raw_axes[5]),
    };

    let btn_start = 1 + CM3_AXIS_COUNT * 2;
    let mut raw_buttons = [0u8; CM3_BUTTON_BYTES];
    raw_buttons.copy_from_slice(&data[btn_start..btn_start + CM3_BUTTON_BYTES]);

    Ok(VpcCm3ThrottleInputState {
        axes,
        buttons: VpcCm3ThrottleButtons { raw: raw_buttons },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn make_cm3_report(axes: [u16; 6], buttons: [u8; 10]) -> Vec<u8> {
        let mut data = vec![0x01u8]; // report_id
        for ax in &axes {
            data.extend_from_slice(&ax.to_le_bytes());
        }
        data.extend_from_slice(&buttons);
        data
    }

    #[test]
    fn too_short_is_error() {
        assert!(parse_cm3_throttle_report(&[0x01; 22]).is_err());
    }

    #[test]
    fn all_zero_axes_parse_to_zero() {
        let report = make_cm3_report([0u16; 6], [0u8; 10]);
        let state = parse_cm3_throttle_report(&report).unwrap();
        assert_eq!(state.axes.left_throttle, 0.0);
        assert_eq!(state.axes.right_throttle, 0.0);
        assert_eq!(state.axes.flaps, 0.0);
        assert_eq!(state.axes.scx, 0.0);
        assert_eq!(state.axes.scy, 0.0);
        assert_eq!(state.axes.slider, 0.0);
    }

    #[test]
    fn max_axes_parse_to_one() {
        let report = make_cm3_report([VIRPIL_AXIS_MAX; 6], [0u8; 10]);
        let state = parse_cm3_throttle_report(&report).unwrap();
        assert!((state.axes.left_throttle - 1.0).abs() < 1e-4);
        assert!((state.axes.right_throttle - 1.0).abs() < 1e-4);
        assert!((state.axes.flaps - 1.0).abs() < 1e-4);
    }

    #[test]
    fn half_throttle_is_approximately_half() {
        let report = make_cm3_report([VIRPIL_AXIS_MAX / 2; 6], [0u8; 10]);
        let state = parse_cm3_throttle_report(&report).unwrap();
        assert!((state.axes.left_throttle - 0.5).abs() < 0.01);
    }

    #[test]
    fn no_buttons_pressed_by_default() {
        let report = make_cm3_report([0u16; 6], [0u8; 10]);
        let state = parse_cm3_throttle_report(&report).unwrap();
        assert!(state.buttons.pressed().is_empty());
    }

    #[test]
    fn button_1_detected() {
        let mut buttons = [0u8; 10];
        buttons[0] = 0x01; // button 1
        let report = make_cm3_report([0u16; 6], buttons);
        let state = parse_cm3_throttle_report(&report).unwrap();
        assert!(state.buttons.is_pressed(1));
        assert!(!state.buttons.is_pressed(2));
        assert_eq!(state.buttons.pressed(), vec![1]);
    }

    #[test]
    fn button_78_detected() {
        let mut buttons = [0u8; 10];
        // button 78 = index 77 → byte 9, bit 5
        buttons[9] = 1 << 5;
        let report = make_cm3_report([0u16; 6], buttons);
        let state = parse_cm3_throttle_report(&report).unwrap();
        assert!(state.buttons.is_pressed(78));
    }

    #[test]
    fn all_buttons_pressed() {
        let buttons = [0xFFu8; 10];
        let report = make_cm3_report([0u16; 6], buttons);
        let state = parse_cm3_throttle_report(&report).unwrap();
        // All 78 bits are set in 10 bytes (80 bits), but only 78 are valid
        for i in 1u8..=78 {
            assert!(state.buttons.is_pressed(i), "button {} not pressed", i);
        }
    }

    #[test]
    fn out_of_range_button_index_returns_false() {
        let report = make_cm3_report([0u16; 6], [0xFFu8; 10]);
        let state = parse_cm3_throttle_report(&report).unwrap();
        assert!(!state.buttons.is_pressed(0));
        assert!(!state.buttons.is_pressed(79));
    }

    #[test]
    fn extra_bytes_ignored() {
        let mut report = make_cm3_report([VIRPIL_AXIS_MAX; 6], [0u8; 10]);
        report.extend_from_slice(&[0xFF; 20]);
        let state = parse_cm3_throttle_report(&report).unwrap();
        assert!((state.axes.left_throttle - 1.0).abs() < 1e-4);
    }

    proptest! {
        #[test]
        fn axes_always_in_range(
            raw0 in 0u16..=u16::MAX,
            raw1 in 0u16..=u16::MAX,
            raw2 in 0u16..=u16::MAX,
            raw3 in 0u16..=u16::MAX,
            raw4 in 0u16..=u16::MAX,
            raw5 in 0u16..=u16::MAX,
        ) {
            let report = make_cm3_report([raw0, raw1, raw2, raw3, raw4, raw5], [0u8; 10]);
            let state = parse_cm3_throttle_report(&report).unwrap();
            assert!((0.0..=1.0).contains(&state.axes.left_throttle));
            assert!((0.0..=1.0).contains(&state.axes.right_throttle));
            assert!((0.0..=1.0).contains(&state.axes.flaps));
            assert!((0.0..=1.0).contains(&state.axes.scx));
            assert!((0.0..=1.0).contains(&state.axes.scy));
            assert!((0.0..=1.0).contains(&state.axes.slider));
        }

        #[test]
        fn random_report_does_not_panic(data in proptest::collection::vec(0u8..=255u8, 23..64)) {
            let _ = parse_cm3_throttle_report(&data);
        }
    }
}
