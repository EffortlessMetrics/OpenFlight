// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the VIRPIL VPC WarBRD and WarBRD-D right-hand joystick bases.
//!
//! # Confirmed device identifiers
//!
//! | Variant  | VID    | PID    | Source |
//! |----------|--------|--------|--------|
//! | WarBRD   | 0x3344 | 0x40CC | fredemmott/cpp-remapper devicedb.h — "RIGHT VPC Stick WarBRD" |
//! | WarBRD-D | 0x3344 | 0x43F5 | LunaBaloona/Virpil_devices_on_Linux — `lsusb` real hardware |
//!
//! # Report format
//!
//! The WarBRD(-D) base uses the **same 15-byte HID report format** as the
//! VPC MongoosT-50CM3 stick (`stick_mongoost.rs`). This module delegates all
//! parsing to [`parse_mongoost_stick_report`] and wraps the result with a
//! [`WarBrdVariant`] field so callers can distinguish the two hardware variants.

use thiserror::Error;

pub use crate::stick_mongoost::{VpcMongoostAxes, VpcMongoostButtons, VpcMongoostHat};
use crate::stick_mongoost::{
    VpcMongoostInputState, VpcMongoostParseError, parse_mongoost_stick_report,
};

/// Minimum byte count for a WarBRD / WarBRD-D HID report.
///
/// Identical to the MongoosT-50CM3 minimum (15 bytes).
pub const VPC_WARBRD_MIN_REPORT_BYTES: usize =
    crate::stick_mongoost::VPC_MONGOOST_STICK_MIN_REPORT_BYTES;

/// Distinguishes the original WarBRD from the revised WarBRD-D base.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarBrdVariant {
    /// VPC WarBRD (original). VID 0x3344, PID 0x40CC.
    Original,
    /// VPC WarBRD-D (revised "D" variant). VID 0x3344, PID 0x43F5.
    D,
}

impl WarBrdVariant {
    /// Human-readable product name for this variant.
    pub fn product_name(self) -> &'static str {
        match self {
            Self::Original => "VPC WarBRD Stick",
            Self::D => "VPC WarBRD-D Stick",
        }
    }
}

/// Full parsed input state from one VPC WarBRD / WarBRD-D HID report.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VpcWarBrdInputState {
    /// Which physical base produced this report.
    pub variant: WarBrdVariant,
    /// Normalised axes and decoded buttons (identical layout to MongoosT-50CM3).
    pub inner: VpcMongoostInputState,
}

/// Parse error for the VPC WarBRD / WarBRD-D.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VpcWarBrdParseError {
    #[error("VPC WarBRD report too short: got {0} bytes (need ≥15)")]
    TooShort(usize),
}

impl From<VpcMongoostParseError> for VpcWarBrdParseError {
    fn from(e: VpcMongoostParseError) -> Self {
        match e {
            VpcMongoostParseError::TooShort(n) => Self::TooShort(n),
        }
    }
}

/// Parse one raw HID report from a VPC WarBRD or WarBRD-D base.
///
/// Both hardware variants produce an identical 15-byte report; the `variant`
/// parameter is caller-supplied and stored verbatim in the returned state.
pub fn parse_warbrd_report(
    data: &[u8],
    variant: WarBrdVariant,
) -> Result<VpcWarBrdInputState, VpcWarBrdParseError> {
    let inner = parse_mongoost_stick_report(data)?;
    Ok(VpcWarBrdInputState { variant, inner })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VIRPIL_AXIS_MAX;

    fn make_warbrd_report(axes: [u16; 5], buttons: [u8; 4]) -> Vec<u8> {
        let mut data = vec![0x01u8];
        for ax in &axes {
            data.extend_from_slice(&ax.to_le_bytes());
        }
        data.extend_from_slice(&buttons);
        data
    }

    #[test]
    fn too_short_returns_error() {
        let err = parse_warbrd_report(&[0x01; 14], WarBrdVariant::D).unwrap_err();
        assert_eq!(err, VpcWarBrdParseError::TooShort(14));
    }

    #[test]
    fn variant_original_preserved() {
        let report = make_warbrd_report([0u16; 5], [0u8; 4]);
        let state = parse_warbrd_report(&report, WarBrdVariant::Original).unwrap();
        assert_eq!(state.variant, WarBrdVariant::Original);
    }

    #[test]
    fn variant_d_preserved() {
        let report = make_warbrd_report([0u16; 5], [0u8; 4]);
        let state = parse_warbrd_report(&report, WarBrdVariant::D).unwrap();
        assert_eq!(state.variant, WarBrdVariant::D);
    }

    #[test]
    fn zero_axes_parse_to_zero() {
        let report = make_warbrd_report([0u16; 5], [0u8; 4]);
        let state = parse_warbrd_report(&report, WarBrdVariant::D).unwrap();
        assert_eq!(state.inner.axes.x, 0.0);
        assert_eq!(state.inner.axes.y, 0.0);
        assert_eq!(state.inner.axes.z, 0.0);
        assert_eq!(state.inner.axes.sz, 0.0);
        assert_eq!(state.inner.axes.sl, 0.0);
    }

    #[test]
    fn max_axes_parse_to_one() {
        let report = make_warbrd_report([VIRPIL_AXIS_MAX; 5], [0u8; 4]);
        let state = parse_warbrd_report(&report, WarBrdVariant::D).unwrap();
        assert!((state.inner.axes.x - 1.0).abs() < 1e-4);
        assert!((state.inner.axes.y - 1.0).abs() < 1e-4);
        assert!((state.inner.axes.z - 1.0).abs() < 1e-4);
    }

    #[test]
    fn button_1_detected() {
        let mut buttons = [0u8; 4];
        buttons[0] = 0x01;
        let report = make_warbrd_report([0u16; 5], buttons);
        let state = parse_warbrd_report(&report, WarBrdVariant::D).unwrap();
        assert!(state.inner.buttons.is_pressed(1));
        assert!(!state.inner.buttons.is_pressed(2));
    }

    #[test]
    fn hat_south_detected() {
        let mut buttons = [0u8; 4];
        // South = 4 → bits 4..7 of byte 3: byte3 |= (4 << 4) = 0x40
        buttons[3] = 0x40;
        let report = make_warbrd_report([0u16; 5], buttons);
        let state = parse_warbrd_report(&report, WarBrdVariant::D).unwrap();
        assert_eq!(state.inner.buttons.hat, VpcMongoostHat::South);
    }

    #[test]
    fn product_name_original() {
        assert_eq!(WarBrdVariant::Original.product_name(), "VPC WarBRD Stick");
    }

    #[test]
    fn product_name_d() {
        assert_eq!(WarBrdVariant::D.product_name(), "VPC WarBRD-D Stick");
    }
}
