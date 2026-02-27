// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the Logitech G27 and G25 Racing Wheels.
//!
//! # Confirmed device identifiers
//!
//! - VID 0x046D, PID 0xC29B — G27 (with H-pattern shifter and paddle shifters)
//! - VID 0x046D, PID 0xC299 — G25 (paddle shifters only, fewer buttons)
//!
//! # Report layout (11 bytes)
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
//! | 9–10    | Buttons   | u16 LE | bitmask  | G27: full button set        |

use crate::WheelError;

/// USB Product ID for the Logitech G27 Racing Wheel.
pub const G27_PID: u16 = 0xC29B;

/// USB Product ID for the Logitech G25 Racing Wheel.
pub const G25_PID: u16 = 0xC299;

/// Expected HID report length in bytes (including Report ID at byte 0).
pub const G27_REPORT_LEN: usize = 11;

/// Expected HID Report ID for G27/G25 input reports.
pub const G27_REPORT_ID: u8 = 0x01;

/// Parsed input state from a G27 or G25 HID input report.
///
/// All axis values are raw u16. Use [`crate::normalize_wheel`] and
/// [`crate::normalize_pedal`] to convert to floating-point ranges.
///
/// `buttons` is stored as `u32` for forward compatibility; only bits 0–15 are
/// populated from the 2-byte button field in the report.
#[derive(Debug, Clone, PartialEq)]
pub struct G27State {
    /// Steering wheel axis. 0 = full left, 65535 = full right, 32768 ≈ center.
    pub wheel: u16,
    /// Gas/accelerator pedal. 0 = released, 65535 = fully depressed.
    pub gas: u16,
    /// Brake pedal. 0 = released, 65535 = fully depressed.
    pub brake: u16,
    /// Clutch pedal. 0 = released, 65535 = fully depressed.
    pub clutch: u16,
    /// Button bitmask. G27 has more buttons than G25 (paddle shifters only).
    pub buttons: u32,
}

/// Parse an 11-byte HID input report from a Logitech G27 or G25 Racing Wheel.
///
/// The report must include the USB Report ID byte at position 0.
///
/// # Errors
///
/// - [`WheelError::TooShort`] if `report` is shorter than [`G27_REPORT_LEN`].
/// - [`WheelError::InvalidReportId`] if byte 0 is not [`G27_REPORT_ID`].
pub fn parse_g27(report: &[u8]) -> Result<G27State, WheelError> {
    if report.len() < G27_REPORT_LEN {
        return Err(WheelError::TooShort {
            need: G27_REPORT_LEN,
            got: report.len(),
        });
    }
    if report[0] != G27_REPORT_ID {
        return Err(WheelError::InvalidReportId(report[0]));
    }
    Ok(G27State {
        wheel: u16::from_le_bytes([report[1], report[2]]),
        gas: u16::from_le_bytes([report[3], report[4]]),
        brake: u16::from_le_bytes([report[5], report[6]]),
        clutch: u16::from_le_bytes([report[7], report[8]]),
        buttons: u16::from_le_bytes([report[9], report[10]]) as u32,
    })
}
