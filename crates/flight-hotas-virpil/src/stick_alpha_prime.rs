// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the VIRPIL VPC Constellation Alpha Prime grips.
//!
//! # Confirmed device identifiers
//!
//! - **Left**:  VID 0x3344 (VIRPIL Controls), PID 0x0138
//! - **Right**: VID 0x3344 (VIRPIL Controls), PID 0x4139
//!
//! # Report format
//!
//! The Alpha Prime uses the **same 15-byte report format** as the Constellation
//! Alpha (`stick_alpha.rs`). The "Prime" designation refers to improved grip
//! ergonomics, not a change in the USB protocol. Both grips share the same
//! firmware family and HID descriptor layout.
//!
//! This module delegates all parsing to [`parse_alpha_report`] and wraps the
//! result with an [`AlphaPrimeVariant`] field so callers can distinguish left
//! and right grips.

use thiserror::Error;

pub use crate::stick_alpha::{VpcAlphaAxes, VpcAlphaButtons, VpcAlphaHat};
use crate::stick_alpha::{VpcAlphaInputState, VpcAlphaParseError, parse_alpha_report};

/// Minimum byte count for a Constellation Alpha Prime HID report.
///
/// Identical to the non-Prime Alpha minimum (15 bytes).
pub const VPC_ALPHA_PRIME_MIN_REPORT_BYTES: usize = crate::stick_alpha::VPC_ALPHA_MIN_REPORT_BYTES;

/// Distinguishes the left and right Alpha Prime grip variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlphaPrimeVariant {
    /// VPC Constellation Alpha Prime Left grip (PID 0x0138).
    Left,
    /// VPC Constellation Alpha Prime Right grip (PID 0x4139).
    Right,
}

impl AlphaPrimeVariant {
    /// Human-readable product name for this variant.
    pub fn product_name(self) -> &'static str {
        match self {
            Self::Left => "VPC Constellation Alpha Prime Left",
            Self::Right => "VPC Constellation Alpha Prime Right",
        }
    }
}

/// Parse error for the VPC Constellation Alpha Prime stick.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VpcAlphaPrimeParseError {
    /// Report is shorter than the required 15 bytes.
    #[error("VPC Constellation Alpha Prime report too short: got {0} bytes (need ≥15)")]
    TooShort(usize),
}

impl From<VpcAlphaParseError> for VpcAlphaPrimeParseError {
    fn from(err: VpcAlphaParseError) -> Self {
        match err {
            VpcAlphaParseError::TooShort(n) => Self::TooShort(n),
        }
    }
}

/// Full parsed input state from one VPC Constellation Alpha Prime HID report.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VpcAlphaPrimeInputState {
    /// Which grip variant produced this report.
    pub variant: AlphaPrimeVariant,
    /// Normalised axis values (same layout as the non-Prime Alpha).
    pub axes: VpcAlphaAxes,
    /// Button and hat state (same layout as the non-Prime Alpha).
    pub buttons: VpcAlphaButtons,
}

impl VpcAlphaPrimeInputState {
    fn from_alpha(base: VpcAlphaInputState, variant: AlphaPrimeVariant) -> Self {
        Self {
            variant,
            axes: base.axes,
            buttons: base.buttons,
        }
    }
}

/// Parse one raw HID report from the VPC Constellation Alpha Prime stick.
///
/// The report format is identical to the non-Prime Constellation Alpha, so
/// this function delegates to [`parse_alpha_report`] and attaches `variant`
/// to the result. The `variant` parameter distinguishes left (PID 0x0138) and
/// right (PID 0x4139) grips at the call site.
pub fn parse_alpha_prime_report(
    data: &[u8],
    variant: AlphaPrimeVariant,
) -> Result<VpcAlphaPrimeInputState, VpcAlphaPrimeParseError> {
    let base = parse_alpha_report(data)?;
    Ok(VpcAlphaPrimeInputState::from_alpha(base, variant))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VIRPIL_AXIS_MAX;
    use proptest::prelude::*;

    fn make_prime_report(axes: [u16; 5], buttons: [u8; 4]) -> Vec<u8> {
        let mut data = vec![0x01u8]; // report_id
        for ax in &axes {
            data.extend_from_slice(&ax.to_le_bytes());
        }
        data.extend_from_slice(&buttons);
        data
    }

    #[test]
    fn too_short_returns_error() {
        let result = parse_alpha_prime_report(&[0x01; 14], AlphaPrimeVariant::Left);
        assert!(matches!(result, Err(VpcAlphaPrimeParseError::TooShort(14))));
    }

    #[test]
    fn empty_slice_returns_error() {
        let result = parse_alpha_prime_report(&[], AlphaPrimeVariant::Right);
        assert!(matches!(result, Err(VpcAlphaPrimeParseError::TooShort(0))));
    }

    #[test]
    fn exactly_min_bytes_parses_ok() {
        let report = make_prime_report([0u16; 5], [0u8; 4]);
        assert_eq!(report.len(), VPC_ALPHA_PRIME_MIN_REPORT_BYTES);
        assert!(parse_alpha_prime_report(&report, AlphaPrimeVariant::Left).is_ok());
    }

    #[test]
    fn variant_left_preserved_in_state() {
        let report = make_prime_report([0u16; 5], [0u8; 4]);
        let state = parse_alpha_prime_report(&report, AlphaPrimeVariant::Left).unwrap();
        assert_eq!(state.variant, AlphaPrimeVariant::Left);
    }

    #[test]
    fn variant_right_preserved_in_state() {
        let report = make_prime_report([0u16; 5], [0u8; 4]);
        let state = parse_alpha_prime_report(&report, AlphaPrimeVariant::Right).unwrap();
        assert_eq!(state.variant, AlphaPrimeVariant::Right);
    }

    #[test]
    fn max_axes_parse_to_one() {
        let report = make_prime_report([VIRPIL_AXIS_MAX; 5], [0u8; 4]);
        let state = parse_alpha_prime_report(&report, AlphaPrimeVariant::Left).unwrap();
        assert!((state.axes.x - 1.0).abs() < 1e-4);
        assert!((state.axes.y - 1.0).abs() < 1e-4);
        assert!((state.axes.z - 1.0).abs() < 1e-4);
        assert!((state.axes.sz - 1.0).abs() < 1e-4);
        assert!((state.axes.sl - 1.0).abs() < 1e-4);
    }

    #[test]
    fn zero_axes_parse_to_zero() {
        let report = make_prime_report([0u16; 5], [0u8; 4]);
        let state = parse_alpha_prime_report(&report, AlphaPrimeVariant::Right).unwrap();
        assert_eq!(state.axes.x, 0.0);
        assert_eq!(state.axes.y, 0.0);
        assert_eq!(state.axes.z, 0.0);
        assert_eq!(state.axes.sz, 0.0);
        assert_eq!(state.axes.sl, 0.0);
    }

    #[test]
    fn button_1_detected() {
        let mut buttons = [0u8; 4];
        buttons[0] = 0x01;
        let report = make_prime_report([0u16; 5], buttons);
        let state = parse_alpha_prime_report(&report, AlphaPrimeVariant::Right).unwrap();
        assert!(state.buttons.is_pressed(1));
        assert!(!state.buttons.is_pressed(2));
    }

    #[test]
    fn no_buttons_pressed_by_default() {
        let report = make_prime_report([0u16; 5], [0u8; 4]);
        let state = parse_alpha_prime_report(&report, AlphaPrimeVariant::Left).unwrap();
        assert!(state.buttons.pressed().is_empty());
    }

    #[test]
    fn hat_north_detected() {
        let mut buttons = [0u8; 4];
        // North = 0 in high nibble of byte 3 → byte 3 = 0x00
        buttons[3] = 0x00;
        let report = make_prime_report([0u16; 5], buttons);
        let state = parse_alpha_prime_report(&report, AlphaPrimeVariant::Left).unwrap();
        assert_eq!(state.buttons.hat, VpcAlphaHat::North);
    }

    #[test]
    fn hat_center_for_high_nibble_f() {
        let mut buttons = [0u8; 4];
        buttons[3] = 0xF0; // high nibble = 0xF → Center
        let report = make_prime_report([0u16; 5], buttons);
        let state = parse_alpha_prime_report(&report, AlphaPrimeVariant::Right).unwrap();
        assert_eq!(state.buttons.hat, VpcAlphaHat::Center);
    }

    #[test]
    fn longer_report_does_not_error() {
        let mut report = make_prime_report([VIRPIL_AXIS_MAX; 5], [0xFFu8; 4]);
        report.extend_from_slice(&[0u8; 8]);
        assert!(parse_alpha_prime_report(&report, AlphaPrimeVariant::Left).is_ok());
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
            let report = make_prime_report([raw0, raw1, raw2, raw3, raw4], [0u8; 4]);
            let state = parse_alpha_prime_report(&report, AlphaPrimeVariant::Left).unwrap();
            prop_assert!(state.axes.x  >= 0.0 && state.axes.x  <= 1.0);
            prop_assert!(state.axes.y  >= 0.0 && state.axes.y  <= 1.0);
            prop_assert!(state.axes.z  >= 0.0 && state.axes.z  <= 1.0);
            prop_assert!(state.axes.sz >= 0.0 && state.axes.sz <= 1.0);
            prop_assert!(state.axes.sl >= 0.0 && state.axes.sl <= 1.0);
        }

        #[test]
        fn random_report_does_not_panic(
            data in proptest::collection::vec(0u8..=255u8, 15..=32usize),
        ) {
            let _ = parse_alpha_prime_report(&data, AlphaPrimeVariant::Right);
        }
    }
}
