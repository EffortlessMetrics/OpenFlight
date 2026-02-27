// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Microsoft SideWinder joystick drivers for OpenFlight.
//!
//! This crate provides HID input report parsers for:
//!
//! - **SideWinder Force Feedback Pro** (VID 0x045E, PID 0x001B) — 3-axis FFB joystick
//! - **SideWinder Force Feedback 2** (VID 0x045E, PID 0x001C) — FFB2 (identical report layout)
//! - **SideWinder Precision 2** (VID 0x045E, PID 0x002B) — non-FFB budget joystick
//!
//! # Architecture
//!
//! Input reports are parsed from raw HID data (report ID stripped). All axis values
//! are normalised to −1.0..=1.0 (bipolar) or 0.0..=1.0 (unipolar).

pub mod sidewinder_ffb;
pub mod sidewinder_precision;

pub use flight_hid_support::device_support::{
    MICROSOFT_VENDOR_ID, SIDEWINDER_FFB2_PID, SIDEWINDER_FFB_PRO_PID, SIDEWINDER_PRECISION_2_PID,
    SidewinderModel, is_sidewinder_device, sidewinder_model,
};
pub use sidewinder_ffb::{
    SIDEWINDER_FFB_MIN_REPORT_BYTES, SidewinderFfbAxes, SidewinderFfbButtons, SidewinderFfbHat,
    SidewinderFfbInputState, SidewinderFfbParseError, parse_sidewinder_ffb2,
    parse_sidewinder_ffb_pro,
};
pub use sidewinder_precision::{
    SIDEWINDER_P2_MIN_REPORT_BYTES, SidewinderP2Axes, SidewinderP2Buttons, SidewinderP2Hat,
    SidewinderP2InputState, SidewinderP2ParseError, parse_sidewinder_precision2,
};
