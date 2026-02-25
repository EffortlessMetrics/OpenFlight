// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! VPforce Rhino FFB joystick driver for Flight Hub.
//!
//! Supports HID report parsing, force-feedback effect output, and health
//! monitoring for the VPforce Rhino (revisions 2 and 3).
//!
//! # USB Identifiers
//!
//! | Product | VID    | PID    |
//! |---------|--------|--------|
//! | Rhino v2 | 0x0483 | 0xA1C0 |
//! | Rhino v3 | 0x0483 | 0xA1C1 |
//!
//! # Quick start
//!
//! ```no_run
//! use flight_ffb_vpforce::input::{parse_report, RHINO_REPORT_LEN};
//! use flight_ffb_vpforce::effects::{FfbEffect, serialize_effect};
//!
//! // Parse an incoming HID report
//! let raw = [0u8; RHINO_REPORT_LEN]; // replace with real data
//! // raw[0] = 0x01; // report ID set by firmware
//! let state = parse_report(&raw).unwrap();
//!
//! // Send a spring centering effect
//! let _report = serialize_effect(FfbEffect::Spring { coefficient: 0.4 });
//! ```

pub mod effects;
pub mod health;
pub mod input;
pub mod presets;

pub use effects::{FfbEffect, serialize_effect};
pub use health::{RhinoHealthMonitor, RhinoHealthStatus};
pub use input::{
    RHINO_PID_V2, RHINO_PID_V3, RHINO_REPORT_LEN, RhinoAxes, RhinoButtons, RhinoInputState,
    VPFORCE_VENDOR_ID, parse_report,
};
pub use presets::recommended_axis_config;

/// All known VPforce Rhino PIDs.
pub const RHINO_PIDS: &[u16] = &[RHINO_PID_V2, RHINO_PID_V3];
