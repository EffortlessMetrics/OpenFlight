// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! VIRPIL Controls VPC device support for OpenFlight.
//!
//! # VID/PID verification
//!
//! - **VID 0x3344** (VIRPIL Controls, UAB): confirmed from
//!   [the-sz.com USB ID DB](https://www.the-sz.com/products/usbid/index.php?v=0x3344)
//! - Device PIDs sourced from the open-source Rust LED control library
//!   [Buzzec/virpil](https://github.com/Buzzec/virpil).
//!
//! # Report format (all VIRPIL devices)
//!
//! Every VIRPIL HID input report uses the same generic frame:
//!
//! ```text
//! byte  0         : report_id (always 0x01 for usage=4 interface)
//! bytes 1..=2n    : axis values (u16 LE, one per axis)
//! bytes 2n+1..end : button bytes (1 byte per 8 buttons)
//! ```
//!
//! Axis range: 0–16383 (14-bit resolution, max confirmed `u16::from_le_bytes([0, 64])`).
//! Full power is 0x4000 (16384). Normalised range 0.0–1.0.

pub mod panel_1;
pub mod panel_2;
pub mod stick_alpha;
pub mod stick_alpha_prime;
pub mod stick_mongoost;
pub mod stick_warbrd;
pub mod throttle_cm3;

/// Maximum raw axis value for all VIRPIL VPC devices (14-bit resolution).
///
/// From Buzzec/virpil source: `u16::from_le_bytes([0, 64])` = 16384.
pub const VIRPIL_AXIS_MAX: u16 = 16384;

pub use flight_hid_support::device_support::{
    VIRPIL_CM3_THROTTLE_PID, VIRPIL_CONSTELLATION_ALPHA_LEFT_PID,
    VIRPIL_CONSTELLATION_ALPHA_PRIME_LEFT_PID, VIRPIL_CONSTELLATION_ALPHA_PRIME_RIGHT_PID,
    VIRPIL_MONGOOST_STICK_PID, VIRPIL_PANEL1_PID, VIRPIL_PANEL2_PID, VIRPIL_VENDOR_ID,
    VIRPIL_WARBRD_D_PID, VIRPIL_WARBRD_PID, VirpilModel, is_virpil_device, virpil_model,
};

pub use throttle_cm3::{
    VPC_CM3_THROTTLE_MIN_REPORT_BYTES, VpcCm3ParseError, VpcCm3ThrottleAxes, VpcCm3ThrottleButtons,
    VpcCm3ThrottleInputState, parse_cm3_throttle_report,
};

pub use stick_mongoost::{
    VPC_MONGOOST_STICK_MIN_REPORT_BYTES, VpcMongoostAxes, VpcMongoostButtons, VpcMongoostHat,
    VpcMongoostInputState, VpcMongoostParseError, parse_mongoost_stick_report,
};

pub use stick_alpha::{
    VPC_ALPHA_MIN_REPORT_BYTES, VpcAlphaAxes, VpcAlphaButtons, VpcAlphaHat, VpcAlphaInputState,
    VpcAlphaParseError, parse_alpha_report,
};

pub use panel_1::{
    VPC_PANEL1_MIN_REPORT_BYTES, VpcPanel1Buttons, VpcPanel1InputState, VpcPanel1ParseError,
    parse_panel1_report,
};

pub use panel_2::{
    PANEL2_BUTTON_COUNT, VPC_PANEL2_MIN_REPORT_BYTES, VpcPanel2Axes, VpcPanel2Buttons,
    VpcPanel2InputState, VpcPanel2ParseError, parse_panel2_report,
};

pub use stick_alpha_prime::{
    AlphaPrimeVariant, VPC_ALPHA_PRIME_MIN_REPORT_BYTES, VpcAlphaPrimeInputState,
    VpcAlphaPrimeParseError, parse_alpha_prime_report,
};

pub use stick_warbrd::{
    VPC_WARBRD_MIN_REPORT_BYTES, VpcWarBrdInputState, VpcWarBrdParseError, WarBrdVariant,
    parse_warbrd_report,
};
