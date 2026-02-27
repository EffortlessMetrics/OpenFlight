// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Logitech joystick driver for OpenFlight.
//!
//! This crate provides support for:
//! - Logitech Extreme 3D Pro (VID 0x046D, PID 0xC215)
//! - Logitech G Flight Yoke System (VID 0x046D, PID 0xC259)
//! - Logitech G Flight Throttle Quadrant (VID 0x046D, PID 0xC25A)
//!
//! # Architecture
//!
//! Input reports are parsed from raw HID data. All axis values are normalized
//! to −1.0..=1.0 (bipolar) or 0.0..=1.0 (unipolar).

pub mod extreme3dpro;
pub mod g_flight_throttle;
pub mod g_flight_yoke;

pub use extreme3dpro::{
    EXTREME_3D_PRO_MIN_REPORT_BYTES, Extreme3DProAxes, Extreme3DProButtons, Extreme3DProHat,
    Extreme3DProInputState, Extreme3DProParseError, parse_extreme_3d_pro,
};
pub use flight_hid_support::device_support::{
    EXTREME_3D_PRO_PID, G_FLIGHT_THROTTLE_QUADRANT_PID, G_FLIGHT_YOKE_PID, LOGITECH_VENDOR_ID,
    is_extreme_3d_pro, is_g_flight_throttle_quadrant, is_g_flight_yoke,
};
pub use g_flight_throttle::{
    G_FLIGHT_THROTTLE_MIN_REPORT_BYTES, GFlightThrottleAxes, GFlightThrottleButtons,
    GFlightThrottleInputState, GFlightThrottleParseError, parse_g_flight_throttle,
};
pub use g_flight_yoke::{
    G_FLIGHT_YOKE_AXIS_CENTER, G_FLIGHT_YOKE_AXIS_MAX, G_FLIGHT_YOKE_MIN_REPORT_BYTES,
    GFlightYokeAxes, GFlightYokeButtons, GFlightYokeHat, GFlightYokeInputState,
    GFlightYokeParseError, parse_g_flight_yoke,
};
