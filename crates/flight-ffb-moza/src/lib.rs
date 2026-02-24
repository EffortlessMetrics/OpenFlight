// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Moza flight peripheral driver for Flight Hub.
//!
//! Supports the **AB9 FFB Base** and **R3 FFB Base** with joystick modules
//! for flight simulation.
//!
//! # USB Identifiers
//!
//! | Product     | VID    | PID    |
//! |-------------|--------|--------|
//! | AB9 FFB Base | 0x346E | 0x0005 |
//! | R3 FFB Base  | 0x346E | 0x0002 |
//!
//! # Quick start
//!
//! ```no_run
//! use flight_ffb_moza::input::{parse_ab9_report, AB9_REPORT_LEN};
//! use flight_ffb_moza::effects::TorqueCommand;
//!
//! let raw = [0u8; AB9_REPORT_LEN];
//! // raw[0] = 0x01;
//! let state = parse_ab9_report(&raw).unwrap();
//!
//! // Send a centering torque
//! let _report = TorqueCommand { x: -state.axes.roll * 0.3, y: -state.axes.pitch * 0.3 }
//!     .to_report();
//! ```

pub mod effects;
pub mod health;
pub mod input;
pub mod presets;

pub use effects::{FfbMode, TorqueCommand, TORQUE_REPORT_ID, TORQUE_REPORT_LEN};
pub use health::{MozaHealthMonitor, MozaHealthStatus};
pub use input::{
    Ab9Axes, Ab9Buttons, Ab9InputState, MozaParseError, AB9_BASE_PID, AB9_REPORT_LEN,
    MOZA_VENDOR_ID, R3_BASE_PID, parse_ab9_report,
};
pub use presets::ab9_axis_config;

/// All known Moza base PIDs covered by this crate.
pub const MOZA_PIDS: &[u16] = &[AB9_BASE_PID, R3_BASE_PID];
