// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Logitech joystick driver for OpenFlight.
//!
//! This crate provides support for:
//! - Logitech Extreme 3D Pro (VID 0x046D, PID 0xC215)
//!
//! # Architecture
//!
//! Input reports are parsed from raw HID data. All axis values are normalized
//! to −1.0..=1.0 (bipolar) or 0.0..=1.0 (unipolar).

pub mod extreme3dpro;

pub use extreme3dpro::{
    EXTREME_3D_PRO_MIN_REPORT_BYTES, Extreme3DProAxes, Extreme3DProButtons, Extreme3DProHat,
    Extreme3DProInputState, Extreme3DProParseError, parse_extreme_3d_pro,
};
pub use flight_hid_support::device_support::{
    EXTREME_3D_PRO_PID, LOGITECH_VENDOR_ID, is_extreme_3d_pro,
};
