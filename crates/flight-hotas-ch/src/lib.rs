// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! CH Products HOTAS device parsers for OpenFlight.
//!
//! CH Products devices (Fighterstick, Combat Stick, Pro Throttle, Pro Pedals,
//! Eclipse Yoke, Flight Yoke) use VID `0x068E`.
//!
//! This crate provides:
//! - Raw HID report parsers: [`fighterstick`], [`pro_throttle`], [`pro_pedals`],
//!   [`combatstick`], [`eclipse_yoke`], [`flight_yoke`]
//! - Device identification database ([`devices`])
//! - HID protocol constants and helpers ([`protocol`])
//! - Per-device input profiles with axis/button definitions ([`profiles`])
//! - Recommended axis presets per device model ([`presets`])
//! - Device health/connectivity status types ([`health`])

use thiserror::Error;

pub mod combatstick;
pub mod devices;
pub mod eclipse_yoke;
pub mod fighterstick;
pub mod flight_yoke;
pub mod health;
pub mod presets;
pub mod pro_pedals;
pub mod pro_throttle;
pub mod profiles;
pub mod protocol;

/// Parse error shared by all CH Products HID report parsers.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ChError {
    /// The raw report slice is shorter than the device minimum.
    #[error("Report too short: need {need}, got {got}")]
    TooShort { need: usize, got: usize },
    /// The first byte of the report is not the expected report ID (`0x01`).
    #[error("Invalid report ID: {0:#04x}")]
    InvalidReportId(u8),
}

pub use flight_hid_support::device_support::{
    CH_COMBAT_STICK_PID, CH_ECLIPSE_YOKE_PID, CH_FIGHTERSTICK_PID, CH_FLIGHT_YOKE_PID,
    CH_PRO_PEDALS_PID, CH_PRO_THROTTLE_PID, CH_VENDOR_ID, ChModel, ch_model, is_ch_device,
};

pub use health::{ChHealthMonitor, ChHealthStatus};
pub use presets::{ChAxisPreset, recommended_preset};

pub use fighterstick::{
    CH_VID, FIGHTERSTICK_MIN_REPORT_BYTES, FIGHTERSTICK_PID, FighterstickState, normalize_axis,
    parse_fighterstick,
};

pub use pro_throttle::{
    PRO_THROTTLE_MIN_REPORT_BYTES, PRO_THROTTLE_PID, ProThrottleState, normalize_throttle,
    parse_pro_throttle,
};

pub use pro_pedals::{
    PRO_PEDALS_MIN_REPORT_BYTES, PRO_PEDALS_PID, ProPedalsState, normalize_pedal, parse_pro_pedals,
};

pub use combatstick::{
    COMBATSTICK_MIN_REPORT_BYTES, COMBATSTICK_PID, CombatstickState, parse_combatstick,
};

pub use eclipse_yoke::{
    ECLIPSE_YOKE_MIN_REPORT_BYTES, ECLIPSE_YOKE_PID, EclipseYokeState, parse_eclipse_yoke,
};

pub use flight_yoke::{
    FLIGHT_YOKE_MIN_REPORT_BYTES, FLIGHT_YOKE_PID, FlightYokeState, parse_flight_yoke,
};

pub use devices::{ChDevice, DEVICE_TABLE, DeviceEntry, identify_device};
pub use profiles::{AxisDescriptor, AxisNormalization, DeviceProfile, device_profile};
pub use protocol::{FourWayHat, PovDirection};
