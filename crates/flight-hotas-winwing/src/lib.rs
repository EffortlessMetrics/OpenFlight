// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! WinWing HOTAS driver for Flight Hub.
//!
//! Supports the **Orion 2 Throttle**, **Orion 2 F/A-18C Stick**, and
//! **TFRP Rudder Pedals** via USB HID.
//!
//! # USB Identifiers
//!
//! | Product | VID    | PID    |
//! |---------|--------|--------|
//! | Orion 2 Throttle  | 0x4098 | 0xBE62 |
//! | Orion 2 F/A-18C Stick | 0x4098 | 0xBE63 |
//! | TFRP Rudder Pedals | 0x4098 | 0xBE64 |
//!
//! # Quick start
//!
//! ```no_run
//! use flight_hotas_winwing::input::{parse_throttle_report, THROTTLE_REPORT_LEN};
//!
//! let raw = [0u8; THROTTLE_REPORT_LEN];
//! // raw[0] = 0x01;
//! let state = parse_throttle_report(&raw).unwrap();
//! let combined = state.axes.throttle_combined;
//! ```

pub mod health;
pub mod input;
pub mod presets;

pub use health::{WinWingDevice, WinWingHealthMonitor, WinWingHealthStatus};
pub use input::{
    RudderAxes, StickAxes, StickButtons, StickInputState, ThrottleAxes, ThrottleButtons,
    ThrottleInputState, WinWingParseError, ORION2_F18_STICK_PID, ORION2_THROTTLE_PID,
    RUDDER_REPORT_LEN, STICK_REPORT_LEN, TFRP_RUDDER_PID, THROTTLE_REPORT_LEN, WINWING_VENDOR_ID,
    parse_rudder_report, parse_stick_report, parse_throttle_report,
};
pub use presets::{orion2_stick_config, orion2_throttle_config, tfrp_rudder_config};

/// All known WinWing PIDs covered by this crate.
pub const WINWING_PIDS: &[u16] = &[ORION2_THROTTLE_PID, ORION2_F18_STICK_PID, TFRP_RUDDER_PID];
