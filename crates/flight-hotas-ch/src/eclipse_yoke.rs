// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! HID input parsing for the CH Products Flight Sim Eclipse Yoke.
//!
//! # Device identifier
//!
//! VID 0x068E (CH Products), PID 0x0051 (from Linux kernel hid-ids.h).
//!
//! # Report format (minimum 11 bytes)
//!
//! ```text
//! byte   0      : report_id (0x01)
//! bytes  1–2    : X axis LE u16 (roll / aileron)
//! bytes  3–4    : Y axis LE u16 (pitch / elevator)
//! bytes  5–6    : Z axis LE u16 (throttle knob)
//! byte   7      : buttons[7:0]
//! byte   8      : buttons[15:8]
//! byte   9      : buttons[23:16]
//! byte  10      : high nibble = hat (8-way), low nibble = buttons[27:24]
//! ```
//!
//! The Eclipse Yoke is a yoke form factor with X/Y for roll/pitch and a
//! throttle knob on the base. It has 32 buttons and 1 hat.

use crate::ChError;

pub use flight_hid_support::device_support::CH_ECLIPSE_YOKE_PID as ECLIPSE_YOKE_PID;

/// Minimum byte count for an Eclipse Yoke HID report.
pub const ECLIPSE_YOKE_MIN_REPORT_BYTES: usize = 11;

/// Parsed input state from one CH Eclipse Yoke HID report.
#[derive(Debug, Clone, PartialEq)]
pub struct EclipseYokeState {
    /// Roll axis (left-right yoke), 0–65535.
    pub roll: u16,
    /// Pitch axis (forward-back yoke), 0–65535.
    pub pitch: u16,
    /// Throttle knob on the yoke base, 0–65535.
    pub throttle: u16,
    /// 8-way hat position: 0 = center, 1–8 for directions.
    pub hat: u8,
    /// 32 buttons as bitmask.
    pub buttons: u32,
}

/// Parse one raw HID report from the CH Eclipse Yoke.
///
/// Returns [`ChError::TooShort`] if `report.len() < 11`, or
/// [`ChError::InvalidReportId`] if `report[0] != 0x01`.
pub fn parse_eclipse_yoke(report: &[u8]) -> Result<EclipseYokeState, ChError> {
    if report.len() < ECLIPSE_YOKE_MIN_REPORT_BYTES {
        return Err(ChError::TooShort {
            need: ECLIPSE_YOKE_MIN_REPORT_BYTES,
            got: report.len(),
        });
    }
    if report[0] != 0x01 {
        return Err(ChError::InvalidReportId(report[0]));
    }

    let roll = u16::from_le_bytes([report[1], report[2]]);
    let pitch = u16::from_le_bytes([report[3], report[4]]);
    let throttle = u16::from_le_bytes([report[5], report[6]]);

    let buttons_0 = u32::from(report[7]);
    let buttons_1 = u32::from(report[8]) << 8;
    let buttons_2 = u32::from(report[9]) << 16;
    let hat = (report[10] >> 4) & 0x0F;
    let buttons_3 = u32::from(report[10] & 0x0F) << 24;
    let buttons = buttons_0 | buttons_1 | buttons_2 | buttons_3;

    Ok(EclipseYokeState {
        roll,
        pitch,
        throttle,
        hat,
        buttons,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_report(roll: u16, pitch: u16, throttle: u16, btn: [u8; 3], hat_extra: u8) -> [u8; 11] {
        let mut r = [0u8; 11];
        r[0] = 0x01;
        r[1..3].copy_from_slice(&roll.to_le_bytes());
        r[3..5].copy_from_slice(&pitch.to_le_bytes());
        r[5..7].copy_from_slice(&throttle.to_le_bytes());
        r[7] = btn[0];
        r[8] = btn[1];
        r[9] = btn[2];
        r[10] = hat_extra;
        r
    }

    #[test]
    fn too_short_returns_error() {
        assert!(parse_eclipse_yoke(&[0x01; 10]).is_err());
    }

    #[test]
    fn invalid_report_id() {
        let mut r = make_report(0, 0, 0, [0; 3], 0);
        r[0] = 0x03;
        assert!(matches!(
            parse_eclipse_yoke(&r).unwrap_err(),
            ChError::InvalidReportId(0x03)
        ));
    }

    #[test]
    fn center_position() {
        let r = make_report(32768, 32768, 0, [0; 3], 0);
        let s = parse_eclipse_yoke(&r).unwrap();
        assert_eq!(s.roll, 32768);
        assert_eq!(s.pitch, 32768);
        assert_eq!(s.throttle, 0);
        assert_eq!(s.hat, 0);
        assert_eq!(s.buttons, 0);
    }

    #[test]
    fn full_throttle() {
        let r = make_report(0, 0, 65535, [0; 3], 0);
        let s = parse_eclipse_yoke(&r).unwrap();
        assert_eq!(s.throttle, 65535);
    }

    #[test]
    fn buttons_across_bytes() {
        let r = make_report(0, 0, 0, [0xFF, 0xFF, 0xFF], 0x0F);
        let s = parse_eclipse_yoke(&r).unwrap();
        assert_eq!(s.buttons, 0x0FFF_FFFF);
    }

    #[test]
    fn hat_encoding() {
        let r = make_report(0, 0, 0, [0; 3], 0x50); // hat = 5 = South
        let s = parse_eclipse_yoke(&r).unwrap();
        assert_eq!(s.hat, 5);
    }
}
