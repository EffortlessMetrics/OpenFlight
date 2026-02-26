// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID OUT Report 0x10 — FFB force command from host to device.
//!
//! Layout (8 bytes):
//!
//! | Bytes | Field      | Type  | Range         | Notes                       |
//! |-------|------------|-------|---------------|-----------------------------|
//! | 0     | report_id  | u8    | 0x10          | Always 0x10                 |
//! | 1–2   | force_x    | i16 LE| [-32767,32767]| Force on X axis (right=+)   |
//! | 3–4   | force_y    | i16 LE| [-32767,32767]| Force on Y axis (back=+)    |
//! | 5     | mode       | u8    | see `FfbMode` | 0=off,1=constant,2=spring…  |
//! | 6     | gain       | u8    | [0,255]       | Global gain (255=100%)      |
//! | 7     | _reserved  | u8    |               | Must be zero                |

/// Report ID for the FFB output report.
pub const FFB_REPORT_ID: u8 = 0x10;

/// Length of the FFB output report in bytes.
pub const FFB_REPORT_LEN: usize = 8;

/// FFB operating mode byte.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfbMode {
    Off = 0,
    Constant = 1,
    Spring = 2,
    Damper = 3,
    Friction = 4,
}

/// FFB force command sent from host to firmware.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FfbOutputReport {
    /// X-axis force component. Range: –32767 … +32767.
    pub force_x: i16,
    /// Y-axis force component. Range: –32767 … +32767.
    pub force_y: i16,
    /// FFB mode.
    pub mode: FfbMode,
    /// Global gain (0 = 0%, 255 = 100%).
    pub gain: u8,
}

impl FfbOutputReport {
    /// Serialise to an 8-byte HID output report.
    ///
    /// # Example
    ///
    /// ```
    /// use flight_open_hardware::output_report::{FfbOutputReport, FfbMode, FFB_REPORT_ID};
    ///
    /// let cmd = FfbOutputReport { force_x: 0, force_y: 0, mode: FfbMode::Off, gain: 255 };
    /// let bytes = cmd.to_bytes();
    /// assert_eq!(bytes[0], FFB_REPORT_ID);
    /// ```
    pub fn to_bytes(&self) -> [u8; FFB_REPORT_LEN] {
        let mut buf = [0u8; FFB_REPORT_LEN];
        buf[0] = FFB_REPORT_ID;
        buf[1..3].copy_from_slice(&self.force_x.to_le_bytes());
        buf[3..5].copy_from_slice(&self.force_y.to_le_bytes());
        buf[5] = self.mode as u8;
        buf[6] = self.gain;
        buf
    }

    /// Parse an 8-byte HID output report.
    pub fn parse(buf: &[u8]) -> Option<Self> {
        if buf.len() < FFB_REPORT_LEN || buf[0] != FFB_REPORT_ID {
            return None;
        }
        let mode = match buf[5] {
            0 => FfbMode::Off,
            1 => FfbMode::Constant,
            2 => FfbMode::Spring,
            3 => FfbMode::Damper,
            4 => FfbMode::Friction,
            _ => return None,
        };
        Some(Self {
            force_x: i16::from_le_bytes([buf[1], buf[2]]),
            force_y: i16::from_le_bytes([buf[3], buf[4]]),
            mode,
            gain: buf[6],
        })
    }

    /// Return a zero-force, off-mode report.
    pub fn stop() -> Self {
        Self {
            force_x: 0,
            force_y: 0,
            mode: FfbMode::Off,
            gain: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stop_report() {
        let bytes = FfbOutputReport::stop().to_bytes();
        assert_eq!(bytes[0], FFB_REPORT_ID);
        assert_eq!(bytes[5], 0);
    }

    #[test]
    fn test_roundtrip() {
        let cmd = FfbOutputReport {
            force_x: -16000,
            force_y: 16000,
            mode: FfbMode::Spring,
            gain: 200,
        };
        let bytes = cmd.to_bytes();
        let parsed = FfbOutputReport::parse(&bytes).unwrap();
        assert_eq!(cmd, parsed);
    }

    #[test]
    fn test_wrong_id_returns_none() {
        let mut bytes = FfbOutputReport::stop().to_bytes();
        bytes[0] = 0xFF;
        assert!(FfbOutputReport::parse(&bytes).is_none());
    }

    #[test]
    fn test_invalid_mode_byte_returns_none() {
        let mut bytes = FfbOutputReport::stop().to_bytes();
        bytes[5] = 0xFF; // no such mode
        assert!(FfbOutputReport::parse(&bytes).is_none());
    }

    #[test]
    fn test_all_ffb_modes_roundtrip() {
        for mode in [
            FfbMode::Off,
            FfbMode::Constant,
            FfbMode::Spring,
            FfbMode::Damper,
            FfbMode::Friction,
        ] {
            let cmd = FfbOutputReport {
                force_x: 100,
                force_y: -100,
                mode,
                gain: 128,
            };
            let parsed = FfbOutputReport::parse(&cmd.to_bytes()).unwrap();
            assert_eq!(cmd, parsed);
        }
    }

    #[test]
    fn test_format_byte_positions() {
        let cmd = FfbOutputReport {
            force_x: 0x0102_i16,
            force_y: 0x0304_i16,
            mode: FfbMode::Constant,
            gain: 0xAB,
        };
        let bytes = cmd.to_bytes();
        assert_eq!(bytes[0], FFB_REPORT_ID);
        assert_eq!(bytes[1], 0x02); // LE low byte of force_x
        assert_eq!(bytes[2], 0x01); // LE high byte of force_x
        assert_eq!(bytes[3], 0x04); // LE low byte of force_y
        assert_eq!(bytes[4], 0x03); // LE high byte of force_y
        assert_eq!(bytes[5], FfbMode::Constant as u8);
        assert_eq!(bytes[6], 0xAB);
        assert_eq!(bytes[7], 0); // reserved
    }
}
