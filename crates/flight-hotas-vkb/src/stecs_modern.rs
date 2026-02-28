// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! HID input parsing for the VKB S-TECS Modern Throttle (Mini and Max variants).
//!
//! # Confirmed device identifiers
//!
//! - **Mini**: VID 0x231D (VKB), PID 0x012B — confirmed via linux-hardware.org (1 probe)
//! - **Max**: VID 0x231D (VKB), PID 0x012E — confirmed via linux-hardware.org (1 probe)
//!
//! Both are 2023-generation standalone throttle units, distinct from the earlier
//! S-TECS Space series (which uses a multi-virtual-controller HID model).
//!
//! # Report layout (ASSUMED — not captured from hardware)
//!
//! The exact USB HID descriptor for the Modern Throttle has not been confirmed
//! from a live device. The layout below is inferred from VKB firmware family
//! conventions and similarity to the STECS Space series. Fields marked **ASSUMED**
//! should be verified with a hardware USB capture before relying on them.
//!
//! ```text
//! byte   0      : report_id (0x01) — always stripped before parsing
//! bytes  1–2   : throttle / main lever    (u16 LE, 0..65535 → 0.0..=1.0)  ASSUMED
//! bytes  3–4   : left mini-throttle       (u16 LE, 0..65535 → 0.0..=1.0)  ASSUMED
//! bytes  5–6   : right mini-throttle      (u16 LE, 0..65535 → 0.0..=1.0)  ASSUMED
//! bytes  7–8   : rotary knob              (u16 LE, 0..65535 → 0.0..=1.0)  ASSUMED
//! bytes  9–12  : buttons 1–32             (u32 LE, bit 0 = button 1)       ASSUMED
//! bytes 13–16  : buttons 33–64            (u32 LE, bit 0 = button 33)      ASSUMED
//! ```
//!
//! Minimum report length (including the report_id byte): 17 bytes.
//! Extra bytes beyond offset 16 are silently ignored.

use thiserror::Error;

/// Minimum byte count for a STECS Modern Throttle HID report (including report_id).
pub const VKC_STECS_MT_MIN_REPORT_BYTES: usize = 17;

/// Maximum number of buttons tracked per report across both button words.
pub const VKC_STECS_MT_MAX_BUTTONS: usize = 64;

/// Identifies which Modern Throttle variant produced a given report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StecsMtVariant {
    /// VKB S-TECS Modern Throttle Mini (PID 0x012B).
    Mini,
    /// VKB S-TECS Modern Throttle Max (PID 0x012E).
    Max,
}

impl StecsMtVariant {
    /// Human-readable product name for this variant.
    pub fn product_name(self) -> &'static str {
        match self {
            Self::Mini => "VKB S-TECS Modern Throttle Mini",
            Self::Max => "VKB S-TECS Modern Throttle Max",
        }
    }
}

/// Parse error for the VKB S-TECS Modern Throttle.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StecsMtParseError {
    /// Report is shorter than [`VKC_STECS_MT_MIN_REPORT_BYTES`].
    #[error("VKB STECS Modern Throttle report too short: got {0} bytes (need ≥17)")]
    TooShort(usize),
}

/// Normalised axes from the VKB S-TECS Modern Throttle.
///
/// All values are in `[0.0, 1.0]`. Layout is **ASSUMED** from firmware family
/// conventions; verify against a hardware capture before shipping.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VkcStecsMtAxes {
    /// Main throttle lever. 0.0 = fully aft, 1.0 = fully forward. ASSUMED.
    pub throttle: f32,
    /// Left mini-throttle / secondary lever. 0.0 = min, 1.0 = max. ASSUMED.
    pub mini_left: f32,
    /// Right mini-throttle / SpeedBrake-style lever. 0.0 = min, 1.0 = max. ASSUMED.
    pub mini_right: f32,
    /// Rotary knob. 0.0 = CCW end-stop, 1.0 = CW end-stop. ASSUMED.
    pub rotary: f32,
}

/// Button state from the VKB S-TECS Modern Throttle.
///
/// Up to 64 buttons are tracked across two 32-bit words. The Max variant
/// likely exposes more physical buttons than the Mini. Layout is **ASSUMED**.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VkcStecsMtButtons {
    /// Buttons 1–32 packed LSB-first (bit 0 = button 1). ASSUMED.
    pub word0: u32,
    /// Buttons 33–64 packed LSB-first (bit 0 = button 33). ASSUMED.
    pub word1: u32,
}

impl VkcStecsMtButtons {
    /// Return `true` if button `n` (1-indexed, 1..=64) is pressed.
    pub fn is_pressed(&self, n: u8) -> bool {
        if n == 0 || n > 64 {
            return false;
        }
        let idx = (n - 1) as usize;
        if idx < 32 {
            (self.word0 >> idx) & 1 == 1
        } else {
            (self.word1 >> (idx - 32)) & 1 == 1
        }
    }

    /// Return a `Vec` of all pressed button numbers (1-indexed, 1..=64).
    pub fn pressed(&self) -> Vec<u8> {
        (1u8..=64).filter(|&n| self.is_pressed(n)).collect()
    }
}

/// Full parsed input state from one VKB S-TECS Modern Throttle HID report.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VkcStecsMtInputState {
    /// Device variant that produced this report.
    pub variant: StecsMtVariant,
    /// Normalised axis values. Layout is **ASSUMED**.
    pub axes: VkcStecsMtAxes,
    /// Button state. Layout is **ASSUMED**.
    pub buttons: VkcStecsMtButtons,
}

/// Parse one raw HID report from the VKB S-TECS Modern Throttle.
///
/// The first byte is treated as the HID report ID and is always skipped.
/// Reports shorter than [`VKC_STECS_MT_MIN_REPORT_BYTES`] return an error;
/// bytes beyond offset 16 are silently ignored.
///
/// **Note:** The report layout is **ASSUMED** from VKB firmware family
/// conventions. Verify against a hardware USB capture before shipping.
pub fn parse_stecs_mt_report(
    data: &[u8],
    variant: StecsMtVariant,
) -> Result<VkcStecsMtInputState, StecsMtParseError> {
    if data.len() < VKC_STECS_MT_MIN_REPORT_BYTES {
        return Err(StecsMtParseError::TooShort(data.len()));
    }

    // Skip the report_id byte at offset 0.
    let p = &data[1..];

    let normalize = |raw: u16| raw as f32 / u16::MAX as f32;

    let axes = VkcStecsMtAxes {
        throttle: normalize(u16::from_le_bytes([p[0], p[1]])),
        mini_left: normalize(u16::from_le_bytes([p[2], p[3]])),
        mini_right: normalize(u16::from_le_bytes([p[4], p[5]])),
        rotary: normalize(u16::from_le_bytes([p[6], p[7]])),
    };

    let word0 = u32::from_le_bytes([p[8], p[9], p[10], p[11]]);
    let word1 = u32::from_le_bytes([p[12], p[13], p[14], p[15]]);

    Ok(VkcStecsMtInputState {
        variant,
        axes,
        buttons: VkcStecsMtButtons { word0, word1 },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn make_report(
        throttle: u16,
        mini_left: u16,
        mini_right: u16,
        rotary: u16,
        word0: u32,
        word1: u32,
    ) -> Vec<u8> {
        let mut data = vec![0x01u8]; // report_id
        data.extend_from_slice(&throttle.to_le_bytes());
        data.extend_from_slice(&mini_left.to_le_bytes());
        data.extend_from_slice(&mini_right.to_le_bytes());
        data.extend_from_slice(&rotary.to_le_bytes());
        data.extend_from_slice(&word0.to_le_bytes());
        data.extend_from_slice(&word1.to_le_bytes());
        data
    }

    #[test]
    fn too_short_returns_error() {
        let result = parse_stecs_mt_report(&[0x01; 16], StecsMtVariant::Mini);
        assert!(matches!(result, Err(StecsMtParseError::TooShort(16))));
    }

    #[test]
    fn empty_report_returns_error() {
        let result = parse_stecs_mt_report(&[], StecsMtVariant::Max);
        assert!(matches!(result, Err(StecsMtParseError::TooShort(0))));
    }

    #[test]
    fn exactly_min_bytes_parses_ok() {
        let report = make_report(0, 0, 0, 0, 0, 0);
        assert_eq!(report.len(), VKC_STECS_MT_MIN_REPORT_BYTES);
        assert!(parse_stecs_mt_report(&report, StecsMtVariant::Mini).is_ok());
    }

    #[test]
    fn all_zero_axes_map_to_zero() {
        let report = make_report(0, 0, 0, 0, 0, 0);
        let state = parse_stecs_mt_report(&report, StecsMtVariant::Mini).unwrap();
        assert_eq!(state.axes.throttle, 0.0);
        assert_eq!(state.axes.mini_left, 0.0);
        assert_eq!(state.axes.mini_right, 0.0);
        assert_eq!(state.axes.rotary, 0.0);
    }

    #[test]
    fn max_axes_map_to_one() {
        let report = make_report(u16::MAX, u16::MAX, u16::MAX, u16::MAX, 0, 0);
        let state = parse_stecs_mt_report(&report, StecsMtVariant::Max).unwrap();
        assert!((state.axes.throttle - 1.0).abs() < 1e-4);
        assert!((state.axes.mini_left - 1.0).abs() < 1e-4);
        assert!((state.axes.mini_right - 1.0).abs() < 1e-4);
        assert!((state.axes.rotary - 1.0).abs() < 1e-4);
    }

    #[test]
    fn button_1_detected_in_word0() {
        let report = make_report(0, 0, 0, 0, 0x0000_0001, 0);
        let state = parse_stecs_mt_report(&report, StecsMtVariant::Mini).unwrap();
        assert!(state.buttons.is_pressed(1));
        assert!(!state.buttons.is_pressed(2));
    }

    #[test]
    fn button_32_detected_at_word0_msb() {
        let report = make_report(0, 0, 0, 0, 0x8000_0000, 0);
        let state = parse_stecs_mt_report(&report, StecsMtVariant::Max).unwrap();
        assert!(state.buttons.is_pressed(32));
        assert!(!state.buttons.is_pressed(31));
        assert!(!state.buttons.is_pressed(33));
    }

    #[test]
    fn button_33_detected_at_word1_lsb() {
        let report = make_report(0, 0, 0, 0, 0, 0x0000_0001);
        let state = parse_stecs_mt_report(&report, StecsMtVariant::Mini).unwrap();
        assert!(state.buttons.is_pressed(33));
        assert!(!state.buttons.is_pressed(32));
        assert!(!state.buttons.is_pressed(34));
    }

    #[test]
    fn button_64_detected_at_word1_msb() {
        let report = make_report(0, 0, 0, 0, 0, 0x8000_0000);
        let state = parse_stecs_mt_report(&report, StecsMtVariant::Max).unwrap();
        assert!(state.buttons.is_pressed(64));
        assert!(!state.buttons.is_pressed(63));
    }

    #[test]
    fn out_of_range_buttons_return_false() {
        let report = make_report(0, 0, 0, 0, u32::MAX, u32::MAX);
        let state = parse_stecs_mt_report(&report, StecsMtVariant::Mini).unwrap();
        assert!(!state.buttons.is_pressed(0));
        assert!(!state.buttons.is_pressed(65));
        assert!(!state.buttons.is_pressed(255));
    }

    #[test]
    fn variant_is_preserved_in_state() {
        let report = make_report(0, 0, 0, 0, 0, 0);
        let mini = parse_stecs_mt_report(&report, StecsMtVariant::Mini).unwrap();
        let max = parse_stecs_mt_report(&report, StecsMtVariant::Max).unwrap();
        assert_eq!(mini.variant, StecsMtVariant::Mini);
        assert_eq!(max.variant, StecsMtVariant::Max);
    }

    #[test]
    fn longer_report_does_not_error() {
        let mut report = make_report(0x1234, 0x5678, 0xABCD, 0xEF01, 0xDEAD_BEEF, 0xCAFE_BABE);
        report.extend_from_slice(&[0xFFu8; 8]);
        assert!(parse_stecs_mt_report(&report, StecsMtVariant::Mini).is_ok());
    }

    #[test]
    fn no_buttons_pressed_by_default() {
        let report = make_report(0, 0, 0, 0, 0, 0);
        let state = parse_stecs_mt_report(&report, StecsMtVariant::Mini).unwrap();
        assert!(state.buttons.pressed().is_empty());
    }

    proptest! {
        #[test]
        fn axes_always_in_range(
            throttle   in 0u16..=u16::MAX,
            mini_left  in 0u16..=u16::MAX,
            mini_right in 0u16..=u16::MAX,
            rotary     in 0u16..=u16::MAX,
        ) {
            let report = make_report(throttle, mini_left, mini_right, rotary, 0, 0);
            let state = parse_stecs_mt_report(&report, StecsMtVariant::Mini).unwrap();
            prop_assert!((0.0..=1.0).contains(&state.axes.throttle));
            prop_assert!((0.0..=1.0).contains(&state.axes.mini_left));
            prop_assert!((0.0..=1.0).contains(&state.axes.mini_right));
            prop_assert!((0.0..=1.0).contains(&state.axes.rotary));
        }

        #[test]
        fn random_report_does_not_panic(
            data in proptest::collection::vec(0u8..=255u8, 17..=32usize),
        ) {
            let _ = parse_stecs_mt_report(&data, StecsMtVariant::Max);
        }
    }
}
