// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the WinWing Orion joystick.
//!
//! # Input report layout (≥12 bytes)
//!
//! ```text
//! byte  0      : report_id
//! bytes 1–2    : X axis (u16 LE, 0–65535)
//! bytes 3–4    : Y axis (u16 LE, 0–65535)
//! bytes 5–6    : twist / Z axis (u16 LE, 0–65535)
//! bytes 7–10   : buttons (4 bytes, LSB-first, up to 28 buttons)
//! byte  11     : hat (0=N, 1=NE, 2=E, 3=SE, 4=S, 5=SW, 6=W, 7=NW, 8=center)
//! ```

use crate::WinWingError;

/// USB Product ID for the WinWing Orion joystick (estimated).
pub const ORION_JOYSTICK_PID: u16 = 0xB350;

/// USB Product ID for the WinWing URSA MINOR L Throttle (estimated).
pub const URSA_MINOR_L_PID: u16 = 0xB400;

/// Minimum byte count for an Orion joystick report.
///
/// report_id (1) + 3 axes × 2 bytes (6) + 4 button bytes (4) + hat byte (1) = 12.
pub const ORION_JOYSTICK_MIN_REPORT_BYTES: usize = 12;

const ORION_JOY_AXIS_COUNT: usize = 3;
const ORION_JOY_BUTTON_BYTES: usize = 4;

/// Parsed state from one Orion joystick HID report.
#[derive(Debug, Clone, PartialEq)]
pub struct OrionJoystickState {
    /// X axis (roll / left-right), 0–65535.
    pub x: u16,
    /// Y axis (pitch / fore-aft), 0–65535.
    pub y: u16,
    /// Twist / Z axis, 0–65535.
    pub twist: u16,
    /// Button bitmask (up to 28 buttons, LSB = button 1).
    pub buttons: u32,
    /// Hat switch position: 0=N, 1=NE, 2=E, 3=SE, 4=S, 5=SW, 6=W, 7=NW, 8=center.
    pub hat: u8,
}

/// Parse a raw HID report (≥12 bytes) into [`OrionJoystickState`].
///
/// # Errors
///
/// Returns [`WinWingError::ReportTooShort`] if `report` has fewer than
/// [`ORION_JOYSTICK_MIN_REPORT_BYTES`] bytes.
pub fn parse_orion_joystick(report: &[u8]) -> Result<OrionJoystickState, WinWingError> {
    if report.len() < ORION_JOYSTICK_MIN_REPORT_BYTES {
        return Err(WinWingError::ReportTooShort {
            need: ORION_JOYSTICK_MIN_REPORT_BYTES,
            got: report.len(),
        });
    }
    let payload = &report[1..]; // skip report_id
    let x = u16::from_le_bytes([payload[0], payload[1]]);
    let y = u16::from_le_bytes([payload[2], payload[3]]);
    let twist = u16::from_le_bytes([payload[4], payload[5]]);

    let btn_start = 1 + ORION_JOY_AXIS_COUNT * 2;
    let mut buttons: u32 = 0;
    for i in 0..ORION_JOY_BUTTON_BYTES {
        buttons |= (report[btn_start + i] as u32) << (i * 8);
    }

    let hat_byte = report[btn_start + ORION_JOY_BUTTON_BYTES];
    // Clamp unknown hat values to "center" (8)
    let hat = if hat_byte > 8 { 8 } else { hat_byte };

    Ok(OrionJoystickState {
        x,
        y,
        twist,
        buttons,
        hat,
    })
}
