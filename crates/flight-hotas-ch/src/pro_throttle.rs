// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! HID input parsing for the CH Products Pro Throttle.
//!
//! # Device identifier
//!
//! VID 0x068E (CH Products), PID 0x00F1 (from Linux kernel hid-ids.h).
//!
//! # Report format (minimum 9 bytes)
//!
//! ```text
//! byte  0      : report_id (0x01)
//! bytes 1–2    : throttle_main LE u16
//! bytes 3–4    : axis2 LE u16
//! bytes 5–6    : axis3 LE u16
//! byte  7      : buttons[7:0]
//! byte  8      : high nibble = hat (0=center, 1–8 for 8 directions)
//!                low  nibble = buttons[13:8]
//! ```
//!
//! `axis4` is zero for 9-byte reports.

use std::fmt;

use crate::ChError;

/// PID for the CH Products Pro Throttle (confirmed from Linux kernel hid-ids.h).
pub const PRO_THROTTLE_PID: u16 = 0x00F1;

/// Minimum byte count for a Pro Throttle HID report.
pub const PRO_THROTTLE_MIN_REPORT_BYTES: usize = 9;

/// Parsed input state from one CH Pro Throttle HID report.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProThrottleState {
    /// Main throttle axis, 0–65535.
    pub throttle_main: u16,
    /// Second axis, 0–65535.
    pub axis2: u16,
    /// Third axis, 0–65535.
    pub axis3: u16,
    /// Fourth axis, 0–65535. Zero if not present in report.
    pub axis4: u16,
    /// Hat position: 0 = center, 1–8 for 8 directions.
    pub hat: u8,
    /// Button bitmask (bits 0–13).
    pub buttons: u16,
}

/// Parse one raw HID report from the CH Pro Throttle.
///
/// Returns [`ChError::TooShort`] if `report.len() < 9`, or
/// [`ChError::InvalidReportId`] if `report[0] != 0x01`.
pub fn parse_pro_throttle(report: &[u8]) -> Result<ProThrottleState, ChError> {
    if report.len() < PRO_THROTTLE_MIN_REPORT_BYTES {
        return Err(ChError::TooShort {
            need: PRO_THROTTLE_MIN_REPORT_BYTES,
            got: report.len(),
        });
    }
    if report[0] != 0x01 {
        return Err(ChError::InvalidReportId(report[0]));
    }

    let throttle_main = u16::from_le_bytes([report[1], report[2]]);
    let axis2 = u16::from_le_bytes([report[3], report[4]]);
    let axis3 = u16::from_le_bytes([report[5], report[6]]);

    // Byte 7: buttons[7:0].  Byte 8: high nibble = hat, low nibble = buttons[13:8].
    let buttons_lo = u16::from(report[7]);
    let hat = (report[8] >> 4) & 0x0F;
    let buttons_hi = u16::from(report[8] & 0x0F) << 8;
    let buttons = buttons_lo | buttons_hi;

    Ok(ProThrottleState {
        throttle_main,
        axis2,
        axis3,
        axis4: 0,
        hat,
        buttons,
    })
}

/// Normalise a throttle axis value to `[0.0, 1.0]`.
///
/// Maps `0` → `0.0`, `65535` → `1.0`.
pub fn normalize_throttle(raw: u16) -> f32 {
    (raw as f32 / 65535.0).clamp(0.0, 1.0)
}

impl fmt::Display for ProThrottleState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ProThrottle main={} axis2={} axis3={} axis4={} hat={} buttons={:#06x}",
            self.throttle_main, self.axis2, self.axis3, self.axis4, self.hat, self.buttons
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn make_report(throttle: u16, a2: u16, a3: u16, buttons: u8, extra: u8) -> [u8; 9] {
        let mut r = [0u8; 9];
        r[0] = 0x01;
        r[1..3].copy_from_slice(&throttle.to_le_bytes());
        r[3..5].copy_from_slice(&a2.to_le_bytes());
        r[5..7].copy_from_slice(&a3.to_le_bytes());
        r[7] = buttons;
        r[8] = extra;
        r
    }

    #[test]
    fn too_short_returns_error() {
        assert!(parse_pro_throttle(&[0x01; 8]).is_err());
    }

    #[test]
    fn all_zero_parses_ok() {
        let r = make_report(0, 0, 0, 0, 0);
        let s = parse_pro_throttle(&r).unwrap();
        assert_eq!(s.throttle_main, 0);
        assert_eq!(s.buttons, 0);
        assert_eq!(s.hat, 0);
    }

    #[test]
    fn max_throttle_parses_correctly() {
        let r = make_report(65535, 0, 0, 0, 0);
        let s = parse_pro_throttle(&r).unwrap();
        assert_eq!(s.throttle_main, 65535);
    }

    #[test]
    fn hat_encoding() {
        let r = make_report(0, 0, 0, 0, 0x10); // hat = 1
        let s = parse_pro_throttle(&r).unwrap();
        assert_eq!(s.hat, 1);
    }

    #[test]
    fn normalize_throttle_bounds() {
        assert!((normalize_throttle(0) - 0.0).abs() < 1e-4);
        assert!((normalize_throttle(65535) - 1.0).abs() < 1e-4);
    }

    proptest! {
        #[test]
        fn throttle_always_in_range(raw in 0u16..=u16::MAX) {
            let n = normalize_throttle(raw);
            prop_assert!((0.0..=1.0).contains(&n));
        }

        #[test]
        fn random_report_does_not_panic(data in proptest::collection::vec(0u8..=255u8, 9..32)) {
            let _ = parse_pro_throttle(&data);
        }
    }
}
