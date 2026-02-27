// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Sony PlayStation controller adapter for OpenFlight.
//!
//! Supports:
//! - DualShock 3 (VID 0x054C, PID 0x0268)
//! - DualShock 4 v1 (VID 0x054C, PID 0x05C4)
//! - DualShock 4 v2 (VID 0x054C, PID 0x09CC)
//! - DualSense PS5 (VID 0x054C, PID 0x0CE6)
//! - DualSense Edge (VID 0x054C, PID 0x0DF2)
//!
//! # Architecture
//!
//! Input reports are parsed from raw USB HID data. All stick axis values are
//! normalised to −1.0..=1.0; trigger axes to 0.0..=1.0.

pub mod dualsense;
pub mod dualshock;

pub use dualsense::{DUALSENSE_MIN_REPORT_BYTES, DualSenseReport, parse_dualsense_report};
pub use dualshock::{DS4_MIN_REPORT_BYTES, DualShockReport, SonyError, parse_ds4_report};

/// Sony Interactive Entertainment USB vendor ID.
pub const SONY_VENDOR_ID: u16 = 0x054C;

/// DualShock 3 product ID.
pub const DUALSHOCK_3_PID: u16 = 0x0268;
/// DualShock 4 v1 product ID.
pub const DUALSHOCK_4_V1_PID: u16 = 0x05C4;
/// DualShock 4 v2 product ID.
pub const DUALSHOCK_4_V2_PID: u16 = 0x09CC;
/// DualSense (PS5) product ID.
pub const DUALSENSE_PID: u16 = 0x0CE6;
/// DualSense Edge product ID.
pub const DUALSENSE_EDGE_PID: u16 = 0x0DF2;

/// Returns `true` if the given VID/PID pair is a DualShock 3 or 4.
pub fn is_dualshock(vid: u16, pid: u16) -> bool {
    vid == SONY_VENDOR_ID
        && matches!(
            pid,
            DUALSHOCK_3_PID | DUALSHOCK_4_V1_PID | DUALSHOCK_4_V2_PID
        )
}

/// Returns `true` if the given VID/PID pair is a DualSense or DualSense Edge.
pub fn is_dualsense(vid: u16, pid: u16) -> bool {
    vid == SONY_VENDOR_ID && matches!(pid, DUALSENSE_PID | DUALSENSE_EDGE_PID)
}
