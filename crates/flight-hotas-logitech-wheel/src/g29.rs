// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the Logitech G29, G920, and G923 Racing Wheels.
//!
//! # Confirmed device identifiers
//!
//! - VID 0x046D, PID 0xC24F — G29 (PS3/PS4/PC)
//! - VID 0x046D, PID 0xC262 — G920 (Xbox/PC)
//! - VID 0x046D, PID 0xC266 — G923 with TrueForce (PS)
//! - VID 0x046D, PID 0xC267 — G923 with TrueForce (Xbox/PC)
//!
//! # Report layout (12 bytes)
//!
//! **Caution:** The byte layout is based on community documentation. Validate
//! with a USB sniffer before relying on this parser in production.
//!
//! | Byte(s) | Field     | Type   | Range    | Notes                       |
//! |---------|-----------|--------|----------|-----------------------------|
//! | 0       | Report ID | u8     | 0x01     | Must equal `0x01`           |
//! | 1–2     | Wheel     | u16 LE | 0..65535 | Bipolar; center ≈ 32768     |
//! | 3–4     | Gas       | u16 LE | 0..65535 | Unipolar; 0 = released      |
//! | 5–6     | Brake     | u16 LE | 0..65535 | Unipolar; 0 = released      |
//! | 7–8     | Clutch    | u16 LE | 0..65535 | Unipolar; 0 = released      |
//! | 9–10    | Buttons   | u16 LE | bitmask  | Bits 0–15                   |
//! | 11      | Hat       | u8     | 0–8      | 0=N…7=NW, 8=center          |

use crate::WheelError;

/// USB Vendor ID for Logitech, shared across all devices in this crate.
pub const LOGITECH_VID: u16 = 0x046D;

/// USB Product ID for the Logitech G29 Racing Wheel (PS3/PS4/PC).
pub const G29_PID: u16 = 0xC24F;

/// USB Product ID for the Logitech G920 Racing Wheel (Xbox/PC).
///
/// The G920 is functionally identical to the G29 and shares its report format.
pub const G920_PID: u16 = 0xC262;

/// USB Product ID for the Logitech G923 Racing Wheel with TrueForce (PS).
pub const G923_PS_PID: u16 = 0xC266;

/// USB Product ID for the Logitech G923 Racing Wheel with TrueForce (Xbox/PC).
pub const G923_XBOX_PID: u16 = 0xC267;

/// Expected HID report length in bytes (including Report ID at byte 0).
pub const G29_REPORT_LEN: usize = 12;

/// Expected HID Report ID for G29/G920/G923 input reports.
pub const G29_REPORT_ID: u8 = 0x01;

/// Parsed input state from a G29, G920, or G923 HID input report.
///
/// All axis values are raw u16. Use [`crate::normalize_wheel`] and
/// [`crate::normalize_pedal`] to convert to floating-point ranges.
#[derive(Debug, Clone, PartialEq)]
pub struct G29State {
    /// Steering wheel axis. 0 = full left, 65535 = full right, 32768 ≈ center.
    pub wheel: u16,
    /// Gas/accelerator pedal. 0 = released, 65535 = fully depressed.
    pub gas: u16,
    /// Brake pedal. 0 = released, 65535 = fully depressed.
    pub brake: u16,
    /// Clutch pedal. 0 = released, 65535 = fully depressed.
    pub clutch: u16,
    /// Button bitmask (bits 0–15).
    pub buttons: u16,
    /// D-pad/hat position. 0=N, 1=NE, 2=E, 3=SE, 4=S, 5=SW, 6=W, 7=NW, 8=center.
    pub hat: u8,
}

/// Parse a 12-byte HID input report from a Logitech G29, G920, or G923 Racing Wheel.
///
/// The report must include the USB Report ID byte at position 0.
///
/// # Errors
///
/// - [`WheelError::TooShort`] if `report` is shorter than [`G29_REPORT_LEN`].
/// - [`WheelError::InvalidReportId`] if byte 0 is not [`G29_REPORT_ID`].
pub fn parse_g29(report: &[u8]) -> Result<G29State, WheelError> {
    if report.len() < G29_REPORT_LEN {
        return Err(WheelError::TooShort {
            need: G29_REPORT_LEN,
            got: report.len(),
        });
    }
    if report[0] != G29_REPORT_ID {
        return Err(WheelError::InvalidReportId(report[0]));
    }
    Ok(G29State {
        wheel: u16::from_le_bytes([report[1], report[2]]),
        gas: u16::from_le_bytes([report[3], report[4]]),
        brake: u16::from_le_bytes([report[5], report[6]]),
        clutch: u16::from_le_bytes([report[7], report[8]]),
        buttons: u16::from_le_bytes([report[9], report[10]]),
        hat: report[11],
    })
}
