// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID IN Report 0x01 — axis and button state from device to host.
//!
//! Layout (16 bytes):
//!
//! | Bytes  | Field           | Type  | Range    | Notes                  |
//! |--------|-----------------|-------|----------|------------------------|
//! | 0      | report_id       | u8    | 0x01     | Always 0x01            |
//! | 1–2    | x               | i16 LE| [-32767,32767] | Roll (right=+) |
//! | 3–4    | y               | i16 LE| [-32767,32767] | Pitch (back=+) |
//! | 5–6    | twist           | i16 LE| [-32767,32767] | Yaw twist      |
//! | 7      | throttle        | u8    | [0,255]  | 0=full aft, 255=full fwd |
//! | 8–9    | buttons_lo      | u16 LE| bitmask  | Buttons 1–16           |
//! | 10     | hat             | u8    | 0–8      | 0=centred, 1–8=CW from N |
//! | 11     | ffb_fault       | u8    | 0/1      | 1 = overcurrent/overtemp |
//! | 12–15  | _reserved       | [u8;4]|          | Must be zero           |

/// Report ID for the input report.
pub const INPUT_REPORT_ID: u8 = 0x01;

/// Length of the input report in bytes (including the report ID byte).
pub const INPUT_REPORT_LEN: usize = 16;

/// Parsed representation of an input report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputReport {
    /// Roll axis (right = positive). Range: –32767 … +32767.
    pub x: i16,
    /// Pitch axis (back = positive). Range: –32767 … +32767.
    pub y: i16,
    /// Twist / yaw. Range: –32767 … +32767.
    pub twist: i16,
    /// Throttle. 0 = full aft, 255 = full forward.
    pub throttle: u8,
    /// Button bitmask (bit 0 = button 1).
    pub buttons: u16,
    /// 8-way hat switch. 0 = centred; 1–8 clockwise from North.
    pub hat: u8,
    /// True if the FFB motor reported a fault (overcurrent/overtemp).
    pub ffb_fault: bool,
}

impl InputReport {
    /// Parse a 16-byte raw HID report.
    ///
    /// Returns `None` if the slice is too short or the report ID is wrong.
    ///
    /// # Example
    ///
    /// ```
    /// use flight_open_hardware::input_report::{InputReport, INPUT_REPORT_LEN};
    ///
    /// let mut raw = [0u8; INPUT_REPORT_LEN];
    /// raw[0] = 0x01;
    /// let report = InputReport::parse(&raw).unwrap();
    /// assert_eq!(report.x, 0);
    /// ```
    pub fn parse(buf: &[u8]) -> Option<Self> {
        if buf.len() < INPUT_REPORT_LEN || buf[0] != INPUT_REPORT_ID {
            return None;
        }
        Some(Self {
            x: i16::from_le_bytes([buf[1], buf[2]]),
            y: i16::from_le_bytes([buf[3], buf[4]]),
            twist: i16::from_le_bytes([buf[5], buf[6]]),
            throttle: buf[7],
            buttons: u16::from_le_bytes([buf[8], buf[9]]),
            hat: buf[10],
            ffb_fault: buf[11] != 0,
        })
    }

    /// Serialise to a 16-byte buffer.
    pub fn to_bytes(&self) -> [u8; INPUT_REPORT_LEN] {
        let mut buf = [0u8; INPUT_REPORT_LEN];
        buf[0] = INPUT_REPORT_ID;
        buf[1..3].copy_from_slice(&self.x.to_le_bytes());
        buf[3..5].copy_from_slice(&self.y.to_le_bytes());
        buf[5..7].copy_from_slice(&self.twist.to_le_bytes());
        buf[7] = self.throttle;
        buf[8..10].copy_from_slice(&self.buttons.to_le_bytes());
        buf[10] = self.hat;
        buf[11] = self.ffb_fault as u8;
        buf
    }

    /// Normalise the `x` axis to `[-1.0, 1.0]`.
    pub fn x_norm(&self) -> f32 {
        self.x as f32 / 32767.0
    }

    /// Normalise the `y` axis to `[-1.0, 1.0]`.
    pub fn y_norm(&self) -> f32 {
        self.y as f32 / 32767.0
    }

    /// Normalise the throttle to `[0.0, 1.0]`.
    pub fn throttle_norm(&self) -> f32 {
        self.throttle as f32 / 255.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zero_report() -> [u8; INPUT_REPORT_LEN] {
        let mut r = [0u8; INPUT_REPORT_LEN];
        r[0] = INPUT_REPORT_ID;
        r
    }

    #[test]
    fn test_parse_zero_report() {
        let r = InputReport::parse(&zero_report()).unwrap();
        assert_eq!(r.x, 0);
        assert_eq!(r.y, 0);
        assert_eq!(r.throttle, 0);
        assert!(!r.ffb_fault);
    }

    #[test]
    fn test_roundtrip() {
        let report = InputReport {
            x: 16383,
            y: -16383,
            twist: 0,
            throttle: 128,
            buttons: 0b1010_0101,
            hat: 3,
            ffb_fault: false,
        };
        let bytes = report.to_bytes();
        let parsed = InputReport::parse(&bytes).unwrap();
        assert_eq!(report, parsed);
    }

    #[test]
    fn test_wrong_report_id_returns_none() {
        let mut r = zero_report();
        r[0] = 0x99;
        assert!(InputReport::parse(&r).is_none());
    }

    #[test]
    fn test_too_short_returns_none() {
        let r = [0x01u8; 4];
        assert!(InputReport::parse(&r).is_none());
    }

    #[test]
    fn test_normalise_full_deflection() {
        let report = InputReport {
            x: 32767,
            y: -32767,
            twist: 0,
            throttle: 255,
            buttons: 0,
            hat: 0,
            ffb_fault: false,
        };
        assert!((report.x_norm() - 1.0).abs() < 1e-4);
        assert!((report.y_norm() + 1.0).abs() < 1e-4);
        assert!((report.throttle_norm() - 1.0).abs() < 1e-3);
    }
}
