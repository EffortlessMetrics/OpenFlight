// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Logitech joystick driver for OpenFlight.
//!
//! This crate provides support for:
//! - Logitech Extreme 3D Pro (VID 0x046D, PID 0xC215)
//! - Logitech G Flight Yoke System (VID 0x046D, PID 0xC259)
//! - Logitech G Flight Throttle Quadrant (VID 0x046D, PID 0xC25A)
//! - Logitech Flight System G940 FFB HOTAS (VID 0x046D, PID 0xC287)
//! - Logitech G27 Racing Wheel (VID 0x046D, PID 0xC29B)
//! - Logitech G29 Racing Wheel (VID 0x046D, PID 0xC24F)
//! - Logitech G920 Racing Wheel (VID 0x046D, PID 0xC262)
//! - X56 Rhino HOTAS — stick (VID 0x0738, PID 0x2221)
//! - X56 Rhino HOTAS — throttle (VID 0x0738, PID 0xA221)
//! - Logitech X56 RGB HOTAS — stick (VID 0x06A3, PID 0x0C59)
//! - Logitech X56 RGB HOTAS — throttle (VID 0x06A3, PID 0x0C5B)
//! - Saitek Pro Flight Rudder Pedals (VID 0x06A3, PID 0x0763)
//! - Logitech Flight Rudder Pedals (VID 0x046D, PID 0xC264)
//!
//! # Architecture
//!
//! Input reports are parsed from raw HID data. All axis values are normalized
//! to −1.0..=1.0 (bipolar) or 0.0..=1.0 (unipolar).

pub mod extreme3dpro;
pub mod g27_wheel;
pub mod g29_wheel;
pub mod g940;
pub mod g_flight_throttle;
pub mod g_flight_yoke;
pub mod profiles;
pub mod protocol;
pub mod rudder_pedals;
pub mod x56_stick;
pub mod x56_throttle;

pub use extreme3dpro::{
    EXTREME_3D_PRO_MIN_REPORT_BYTES, Extreme3DProAxes, Extreme3DProButtons, Extreme3DProHat,
    Extreme3DProInputState, Extreme3DProParseError, parse_extreme_3d_pro,
};
pub use flight_hid_support::device_support::{
    EXTREME_3D_PRO_PID, G_FLIGHT_THROTTLE_QUADRANT_PID, G_FLIGHT_YOKE_PID, LOGITECH_VENDOR_ID,
    is_extreme_3d_pro, is_g_flight_throttle_quadrant, is_g_flight_yoke, is_g940_joystick,
    is_g940_throttle,
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
pub use g27_wheel::{
    G27_MIN_REPORT_BYTES, G27_PID, G27ParseError, G27State, LOGITECH_VID, parse_g27,
};
pub use g29_wheel::{G29_MIN_REPORT_BYTES, G29_PID, G29ParseError, G29State, G920_PID, parse_g29};
pub use g940::{
    G940_AXIS_BITS, G940_AXIS_CENTER, G940_AXIS_MAX, G940_JOYSTICK_MIN_REPORT_BYTES,
    G940_JOYSTICK_PID, G940_THROTTLE_MIN_REPORT_BYTES, G940_THROTTLE_PID, G940Hat, G940InputState,
    G940ParseError, G940ThrottleState, parse_g940_joystick, parse_g940_throttle,
};
pub use rudder_pedals::{
    RUDDER_PEDALS_MIN_REPORT_BYTES, RudderPedalsAxes, RudderPedalsInputState,
    RudderPedalsParseError, parse_rudder_pedals,
};
pub use x56_stick::{
    X56_STICK_MIN_REPORT_BYTES, X56Hat, X56StickAxes, X56StickButtons, X56StickInputState,
    X56StickParseError, parse_x56_stick,
};
pub use x56_throttle::{
    X56_THROTTLE_MIN_REPORT_BYTES, X56ThrottleAxes, X56ThrottleButtons, X56ThrottleInputState,
    X56ThrottleParseError, parse_x56_throttle,
};
