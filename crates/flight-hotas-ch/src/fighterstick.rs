// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! HID input parsing for the CH Products Fighterstick.
//!
//! # Device identifier
//!
//! VID 0x068E (CH Products), PID 0x00F0 (estimated).
//!
//! # Report format (minimum 9 bytes)
//!
//! ```text
//! byte  0      : report_id (0x01)
//! bytes 1–2    : X axis LE u16 (aileron)
//! bytes 3–4    : Y axis LE u16 (elevator)
//! bytes 5–6    : Z axis LE u16 (twist)
//! byte  7      : buttons[7:0]
//! byte  8      : high nibble = hat[0] (0=center,1=N,2=E,3=S,4=W)
//!                low  nibble = buttons[11:8]
//! ```
//!
//! `throttle`, `hats[1..3]`, and `buttons[31:12]` are zero for 9-byte reports.

use crate::ChError;

/// VID for all CH Products devices.
pub const CH_VID: u16 = 0x068E;

/// PID for the CH Products Fighterstick (estimated).
pub const FIGHTERSTICK_PID: u16 = 0x00F0;

/// Minimum byte count for a Fighterstick HID report.
pub const FIGHTERSTICK_MIN_REPORT_BYTES: usize = 9;

/// Parsed input state from one CH Fighterstick HID report.
#[derive(Debug, Clone, PartialEq)]
pub struct FighterstickState {
    /// Aileron (left-right), 0–65535.
    pub x: u16,
    /// Elevator (front-back), 0–65535.
    pub y: u16,
    /// Twist, 0–65535.
    pub z: u16,
    /// Optional physical throttle, 0–65535. Zero if not present in report.
    pub throttle: u16,
    /// Four 4-way hats. 0 = center, 1 = N, 2 = E, 3 = S, 4 = W.
    pub hats: [u8; 4],
    /// 32 buttons as bitmask (bit 0 = button 1).
    pub buttons: u32,
}

/// Parse one raw HID report from the CH Fighterstick.
///
/// Returns [`ChError::TooShort`] if `report.len() < 9`, or
/// [`ChError::InvalidReportId`] if `report[0] != 0x01`.
pub fn parse_fighterstick(report: &[u8]) -> Result<FighterstickState, ChError> {
    if report.len() < FIGHTERSTICK_MIN_REPORT_BYTES {
        return Err(ChError::TooShort {
            need: FIGHTERSTICK_MIN_REPORT_BYTES,
            got: report.len(),
        });
    }
    if report[0] != 0x01 {
        return Err(ChError::InvalidReportId(report[0]));
    }

    let x = u16::from_le_bytes([report[1], report[2]]);
    let y = u16::from_le_bytes([report[3], report[4]]);
    let z = u16::from_le_bytes([report[5], report[6]]);

    // Byte 7: buttons[7:0].  Byte 8: high nibble = hat[0], low nibble = buttons[11:8].
    let buttons_lo = u32::from(report[7]);
    let hat0 = (report[8] >> 4) & 0x0F;
    let buttons_hi = u32::from(report[8] & 0x0F) << 8;
    let buttons = buttons_lo | buttons_hi;

    Ok(FighterstickState {
        x,
        y,
        z,
        throttle: 0,
        hats: [hat0, 0, 0, 0],
        buttons,
    })
}

/// Normalise a raw axis value to `[-1.0, 1.0]`.
///
/// Maps `0` → `-1.0`, `32768` ≈ `0.0`, `65535` → `1.0`.
pub fn normalize_axis(raw: u16) -> f32 {
    (raw as f32 / 65535.0 * 2.0 - 1.0).clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

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
        assert!(parse_fighterstick(&[0x01; 8]).is_err());
    }

    #[test]
    fn empty_returns_error() {
        assert!(parse_fighterstick(&[]).is_err());
    }

    #[test]
    fn invalid_report_id_returns_error() {
        let mut r = make_report(0, 0, 0, 0, 0);
        r[0] = 0x02;
        assert!(matches!(
            parse_fighterstick(&r).unwrap_err(),
            ChError::InvalidReportId(0x02)
        ));
    }

    #[test]
    fn all_zero_axes_parse_to_zero() {
        let r = make_report(0, 0, 0, 0, 0);
        let s = parse_fighterstick(&r).unwrap();
        assert_eq!((s.x, s.y, s.z), (0, 0, 0));
        assert_eq!(s.buttons, 0);
        assert_eq!(s.hats, [0, 0, 0, 0]);
    }

    #[test]
    fn max_axes_parse_correctly() {
        let r = make_report(65535, 65535, 65535, 0, 0);
        let s = parse_fighterstick(&r).unwrap();
        assert_eq!(s.x, 65535);
        assert_eq!(s.y, 65535);
        assert_eq!(s.z, 65535);
    }

    #[test]
    fn hat_north_detected() {
        let r = make_report(0, 0, 0, 0, 0x10); // high nibble = 1 = North
        let s = parse_fighterstick(&r).unwrap();
        assert_eq!(s.hats[0], 1); // North
        assert_eq!(s.buttons, 0);
    }

    #[test]
    fn buttons_packed_correctly() {
        let r = make_report(0, 0, 0, 0xFF, 0x0F); // buttons[7:0]=0xFF, buttons[11:8]=0xF
        let s = parse_fighterstick(&r).unwrap();
        assert_eq!(s.buttons, 0x0FFF);
        assert_eq!(s.hats[0], 0); // center
    }

    #[test]
    fn normalize_axis_min_is_neg_one() {
        assert!((normalize_axis(0) + 1.0).abs() < 1e-4);
    }

    #[test]
    fn normalize_axis_max_is_pos_one() {
        assert!((normalize_axis(65535) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn normalize_axis_midpoint_near_zero() {
        assert!(normalize_axis(32768).abs() < 0.001);
    }

    proptest! {
        #[test]
        fn axes_always_in_range(x in 0u16..=u16::MAX, y in 0u16..=u16::MAX, z in 0u16..=u16::MAX) {
            let r = make_report(x, y, z, 0, 0);
            let s = parse_fighterstick(&r).unwrap();
            prop_assert!(normalize_axis(s.x) >= -1.0 && normalize_axis(s.x) <= 1.0);
            prop_assert!(normalize_axis(s.y) >= -1.0 && normalize_axis(s.y) <= 1.0);
            prop_assert!(normalize_axis(s.z) >= -1.0 && normalize_axis(s.z) <= 1.0);
        }

        #[test]
        fn random_report_does_not_panic(data in proptest::collection::vec(0u8..=255u8, 9..32)) {
            let _ = parse_fighterstick(&data);
        }
    }
}
