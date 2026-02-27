// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID OUT Report 0x20 — LED and mode control from host to device.
//!
//! Layout (4 bytes):
//!
//! | Byte | Field     | Type | Notes                              |
//! |------|-----------|------|------------------------------------|
//! | 0    | report_id | u8   | 0x20                               |
//! | 1    | leds      | u8   | bitmask: bit0=power,bit1=pc_mode   |
//! | 2    | brightness| u8   | 0=off … 255=full                   |
//! | 3    | _reserved | u8   | Must be zero                       |

/// Report ID for the LED/mode control report.
pub const LED_REPORT_ID: u8 = 0x20;

/// Length of the LED report in bytes.
pub const LED_REPORT_LEN: usize = 4;

/// LED bitmask flags.
pub mod led_flags {
    pub const POWER: u8 = 0b0000_0001;
    pub const PC_MODE: u8 = 0b0000_0010;
}

/// LED / mode control command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedReport {
    pub leds: u8,
    pub brightness: u8,
}

impl LedReport {
    /// Serialise to a 4-byte HID output report.
    ///
    /// # Example
    ///
    /// ```
    /// use flight_open_hardware::led_report::{LedReport, led_flags, LED_REPORT_ID};
    ///
    /// let cmd = LedReport { leds: led_flags::POWER | led_flags::PC_MODE, brightness: 128 };
    /// let bytes = cmd.to_bytes();
    /// assert_eq!(bytes[0], LED_REPORT_ID);
    /// ```
    pub fn to_bytes(&self) -> [u8; LED_REPORT_LEN] {
        [LED_REPORT_ID, self.leds, self.brightness, 0]
    }

    /// Parse a 4-byte report.
    pub fn parse(buf: &[u8]) -> Option<Self> {
        if buf.len() < LED_REPORT_LEN || buf[0] != LED_REPORT_ID {
            return None;
        }
        Some(Self {
            leds: buf[1],
            brightness: buf[2],
        })
    }

    /// Return a report with all LEDs off.
    pub fn all_off() -> Self {
        Self {
            leds: 0,
            brightness: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_off() {
        let bytes = LedReport::all_off().to_bytes();
        assert_eq!(bytes[0], LED_REPORT_ID);
        assert_eq!(bytes[1], 0);
    }

    #[test]
    fn test_roundtrip() {
        let cmd = LedReport {
            leds: led_flags::POWER | led_flags::PC_MODE,
            brightness: 200,
        };
        let bytes = cmd.to_bytes();
        let parsed = LedReport::parse(&bytes).unwrap();
        assert_eq!(cmd, parsed);
    }

    #[test]
    fn test_wrong_report_id_returns_none() {
        let mut bytes = LedReport::all_off().to_bytes();
        bytes[0] = 0xFF;
        assert!(LedReport::parse(&bytes).is_none());
    }

    #[test]
    fn test_too_short_returns_none() {
        assert!(LedReport::parse(&[LED_REPORT_ID, 0]).is_none());
    }

    #[test]
    fn test_reserved_byte_is_zero() {
        let bytes = LedReport {
            leds: 0xFF,
            brightness: 0xFF,
        }
        .to_bytes();
        assert_eq!(bytes[3], 0, "reserved byte must be zero");
    }

    #[test]
    fn test_led_flags_constants() {
        assert_eq!(led_flags::POWER, 0b0000_0001);
        assert_eq!(led_flags::PC_MODE, 0b0000_0010);
    }
}
