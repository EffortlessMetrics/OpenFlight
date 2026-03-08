// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! HID input parsing for the CH Products Flight Sim Yoke.
//!
//! # Device identifier
//!
//! VID 0x068E (CH Products), PID 0x00FF (from Linux kernel hid-ids.h).
//!
//! # Report format (minimum 11 bytes)
//!
//! ```text
//! byte   0      : report_id (0x01)
//! bytes  1–2    : X axis LE u16 (roll / aileron)
//! bytes  3–4    : Y axis LE u16 (pitch / elevator)
//! bytes  5–6    : throttle LE u16
//! byte   7      : buttons[7:0]
//! byte   8      : buttons[15:8]
//! byte   9      : high nibble = hat (8-way), low nibble = buttons[19:16]
//! byte  10      : reserved (zero)
//! ```
//!
//! The Flight Sim Yoke is the classic CH Products yoke (circa 1997–2008),
//! predecessor to the Eclipse Yoke. It has 20 buttons and 1 hat.

use std::fmt;

use crate::ChError;

pub use flight_hid_support::device_support::CH_FLIGHT_YOKE_PID as FLIGHT_YOKE_PID;

/// Minimum byte count for a Flight Sim Yoke HID report.
pub const FLIGHT_YOKE_MIN_REPORT_BYTES: usize = 10;

/// Parsed input state from one CH Flight Sim Yoke HID report.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FlightYokeState {
    /// Roll axis (left-right yoke), 0–65535.
    pub roll: u16,
    /// Pitch axis (forward-back yoke), 0–65535.
    pub pitch: u16,
    /// Throttle lever, 0–65535.
    pub throttle: u16,
    /// 8-way hat position: 0 = center, 1–8 for directions.
    pub hat: u8,
    /// 20 buttons as bitmask.
    pub buttons: u32,
}

/// Parse one raw HID report from the CH Flight Sim Yoke.
///
/// Returns [`ChError::TooShort`] if `report.len() < 10`, or
/// [`ChError::InvalidReportId`] if `report[0] != 0x01`.
pub fn parse_flight_yoke(report: &[u8]) -> Result<FlightYokeState, ChError> {
    if report.len() < FLIGHT_YOKE_MIN_REPORT_BYTES {
        return Err(ChError::TooShort {
            need: FLIGHT_YOKE_MIN_REPORT_BYTES,
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
    let hat = (report[9] >> 4) & 0x0F;
    let buttons_2 = u32::from(report[9] & 0x0F) << 16;
    let buttons = buttons_0 | buttons_1 | buttons_2;

    Ok(FlightYokeState {
        roll,
        pitch,
        throttle,
        hat,
        buttons,
    })
}

impl fmt::Display for FlightYokeState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "FlightYoke roll={} pitch={} throttle={} hat={} buttons={:#07x}",
            self.roll, self.pitch, self.throttle, self.hat, self.buttons
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_report(
        roll: u16,
        pitch: u16,
        throttle: u16,
        btn0: u8,
        btn1: u8,
        hat_extra: u8,
    ) -> [u8; 10] {
        let mut r = [0u8; 10];
        r[0] = 0x01;
        r[1..3].copy_from_slice(&roll.to_le_bytes());
        r[3..5].copy_from_slice(&pitch.to_le_bytes());
        r[5..7].copy_from_slice(&throttle.to_le_bytes());
        r[7] = btn0;
        r[8] = btn1;
        r[9] = hat_extra;
        r
    }

    #[test]
    fn too_short_returns_error() {
        assert!(parse_flight_yoke(&[0x01; 9]).is_err());
    }

    #[test]
    fn invalid_report_id() {
        let mut r = make_report(0, 0, 0, 0, 0, 0);
        r[0] = 0x05;
        assert!(matches!(
            parse_flight_yoke(&r).unwrap_err(),
            ChError::InvalidReportId(0x05)
        ));
    }

    #[test]
    fn center_position() {
        let r = make_report(32768, 32768, 0, 0, 0, 0);
        let s = parse_flight_yoke(&r).unwrap();
        assert_eq!(s.roll, 32768);
        assert_eq!(s.pitch, 32768);
        assert_eq!(s.throttle, 0);
        assert_eq!(s.hat, 0);
        assert_eq!(s.buttons, 0);
    }

    #[test]
    fn full_throttle() {
        let r = make_report(0, 0, 65535, 0, 0, 0);
        let s = parse_flight_yoke(&r).unwrap();
        assert_eq!(s.throttle, 65535);
    }

    #[test]
    fn buttons_across_bytes() {
        let r = make_report(0, 0, 0, 0xFF, 0xFF, 0x0F);
        let s = parse_flight_yoke(&r).unwrap();
        assert_eq!(s.buttons, 0x000F_FFFF);
    }

    #[test]
    fn hat_encoding() {
        let r = make_report(0, 0, 0, 0, 0, 0x30); // hat = 3 = East
        let s = parse_flight_yoke(&r).unwrap();
        assert_eq!(s.hat, 3);
    }
}
