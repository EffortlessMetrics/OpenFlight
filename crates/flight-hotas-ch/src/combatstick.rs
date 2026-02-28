// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! HID input parsing for the CH Products Combat Stick.
//!
//! # Device identifier
//!
//! VID 0x068E (CH Products), PID 0x00F4 (from Linux kernel hid-ids.h).
//!
//! # Report format (minimum 9 bytes)
//!
//! ```text
//! byte  0      : report_id (0x01)
//! bytes 1–2    : X axis LE u16 (aileron)
//! bytes 3–4    : Y axis LE u16 (elevator)
//! bytes 5–6    : Z axis LE u16 (twist/rudder)
//! byte  7      : buttons[7:0]
//! byte  8      : high nibble = hat (8-way, 0=center, 1–8)
//!                low  nibble = buttons[11:8]
//! ```
//!
//! The Combat Stick is similar to the Fighterstick but with a different
//! grip shape and button layout optimized for combat simulation.

use crate::ChError;

pub use flight_hid_support::device_support::CH_COMBAT_STICK_PID as COMBATSTICK_PID;

/// Minimum byte count for a Combat Stick HID report.
pub const COMBATSTICK_MIN_REPORT_BYTES: usize = 9;

/// Parsed input state from one CH Combat Stick HID report.
#[derive(Debug, Clone, PartialEq)]
pub struct CombatstickState {
    /// Aileron (left-right), 0–65535.
    pub x: u16,
    /// Elevator (front-back), 0–65535.
    pub y: u16,
    /// Twist/rudder, 0–65535.
    pub z: u16,
    /// 8-way hat position: 0 = center, 1–8 for directions.
    pub hat: u8,
    /// 24 buttons as bitmask (bit 0 = button 1).
    pub buttons: u32,
}

/// Parse one raw HID report from the CH Combat Stick.
///
/// Returns [`ChError::TooShort`] if `report.len() < 9`, or
/// [`ChError::InvalidReportId`] if `report[0] != 0x01`.
pub fn parse_combatstick(report: &[u8]) -> Result<CombatstickState, ChError> {
    if report.len() < COMBATSTICK_MIN_REPORT_BYTES {
        return Err(ChError::TooShort {
            need: COMBATSTICK_MIN_REPORT_BYTES,
            got: report.len(),
        });
    }
    if report[0] != 0x01 {
        return Err(ChError::InvalidReportId(report[0]));
    }

    let x = u16::from_le_bytes([report[1], report[2]]);
    let y = u16::from_le_bytes([report[3], report[4]]);
    let z = u16::from_le_bytes([report[5], report[6]]);

    let buttons_lo = u32::from(report[7]);
    let hat = (report[8] >> 4) & 0x0F;
    let buttons_hi = u32::from(report[8] & 0x0F) << 8;
    let buttons = buttons_lo | buttons_hi;

    Ok(CombatstickState {
        x,
        y,
        z,
        hat,
        buttons,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fighterstick::normalize_axis;

    fn make_report(x: u16, y: u16, z: u16, buttons: u8, extra: u8) -> [u8; 9] {
        let mut r = [0u8; 9];
        r[0] = 0x01;
        r[1..3].copy_from_slice(&x.to_le_bytes());
        r[3..5].copy_from_slice(&y.to_le_bytes());
        r[5..7].copy_from_slice(&z.to_le_bytes());
        r[7] = buttons;
        r[8] = extra;
        r
    }

    #[test]
    fn too_short_returns_error() {
        assert!(parse_combatstick(&[0x01; 8]).is_err());
    }

    #[test]
    fn empty_returns_error() {
        assert!(parse_combatstick(&[]).is_err());
    }

    #[test]
    fn invalid_report_id() {
        let mut r = make_report(0, 0, 0, 0, 0);
        r[0] = 0x02;
        assert!(matches!(
            parse_combatstick(&r).unwrap_err(),
            ChError::InvalidReportId(0x02)
        ));
    }

    #[test]
    fn center_position() {
        let r = make_report(32768, 32768, 32768, 0, 0);
        let s = parse_combatstick(&r).unwrap();
        assert_eq!(s.x, 32768);
        assert_eq!(s.y, 32768);
        assert_eq!(s.z, 32768);
        assert!(normalize_axis(s.x).abs() < 0.001);
    }

    #[test]
    fn hat_directions() {
        for dir in 0..=8u8 {
            let r = make_report(0, 0, 0, 0, dir << 4);
            let s = parse_combatstick(&r).unwrap();
            assert_eq!(s.hat, dir);
        }
    }

    #[test]
    fn buttons_packed() {
        let r = make_report(0, 0, 0, 0xFF, 0x0F);
        let s = parse_combatstick(&r).unwrap();
        assert_eq!(s.buttons, 0x0FFF);
    }
}
