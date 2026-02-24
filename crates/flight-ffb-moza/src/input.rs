// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID report parsing for Moza flight peripherals.
//!
//! Moza uses VID **0x346E** across all products.  The AB9 FFB base (PID 0x0005)
//! with joystick module is the primary flight-sim product.
//!
//! Reports use 16-bit signed values for X/Y/Rz; torque feedback is sent as
//! HID output reports using an FFB protocol similar to DirectInput PID.

/// USB Vendor ID for all Moza products.
pub const MOZA_VENDOR_ID: u16 = 0x346E;

/// USB Product ID for the Moza AB9 FFB Base (joystick/flight config).
pub const AB9_BASE_PID: u16 = 0x0005;

/// USB Product ID for the Moza R3 FFB Base.
pub const R3_BASE_PID: u16 = 0x0002;

/// Expected HID input report size for the AB9 (bytes including report ID).
pub const AB9_REPORT_LEN: usize = 16;

/// Normalised axis snapshot for the AB9 + joystick module.
#[derive(Debug, Clone, PartialEq)]
pub struct Ab9Axes {
    /// Roll (stick X) — \[-1.0, 1.0\].
    pub roll: f32,
    /// Pitch (stick Y) — \[-1.0, 1.0\].
    pub pitch: f32,
    /// Throttle lever (Z) — \[0.0, 1.0\].
    pub throttle: f32,
    /// Twist (Rz) — \[-1.0, 1.0\].
    pub twist: f32,
}

/// Button state for the AB9 joystick module (16 buttons).
#[derive(Debug, Clone, Default)]
pub struct Ab9Buttons {
    pub mask: u16,
    pub hat: u8,
}

impl Ab9Buttons {
    pub fn is_pressed(&self, n: u8) -> bool {
        n >= 1 && n <= 16 && (self.mask >> (n - 1)) & 1 == 1
    }
}

/// Parsed state from a single AB9 HID report.
#[derive(Debug, Clone)]
pub struct Ab9InputState {
    pub axes: Ab9Axes,
    pub buttons: Ab9Buttons,
}

/// Parse error type.
#[derive(Debug, thiserror::Error)]
pub enum MozaParseError {
    #[error("report too short: expected ≥{expected} bytes, got {got}")]
    TooShort { expected: usize, got: usize },
    #[error("unknown report ID: 0x{id:02X}")]
    UnknownReportId { id: u8 },
}

/// Parse a raw HID input report from the Moza AB9.
///
/// # Example
///
/// ```
/// use flight_ffb_moza::input::{parse_ab9_report, AB9_REPORT_LEN};
///
/// let mut report = [0u8; AB9_REPORT_LEN];
/// report[0] = 0x01;
/// let state = parse_ab9_report(&report).unwrap();
/// assert!(state.axes.roll.abs() < 1e-3);
/// ```
pub fn parse_ab9_report(data: &[u8]) -> Result<Ab9InputState, MozaParseError> {
    if data.len() < AB9_REPORT_LEN {
        return Err(MozaParseError::TooShort {
            expected: AB9_REPORT_LEN,
            got: data.len(),
        });
    }
    if data[0] != 0x01 {
        return Err(MozaParseError::UnknownReportId { id: data[0] });
    }

    let roll = norm(i16::from_le_bytes([data[1], data[2]]));
    let pitch = norm(i16::from_le_bytes([data[3], data[4]]));
    let throttle_raw = norm(i16::from_le_bytes([data[5], data[6]]));
    let throttle = (throttle_raw + 1.0) * 0.5;
    let twist = norm(i16::from_le_bytes([data[7], data[8]]));
    let mask = u16::from_le_bytes([data[9], data[10]]);
    let hat = data[11];

    Ok(Ab9InputState {
        axes: Ab9Axes {
            roll,
            pitch,
            throttle,
            twist,
        },
        buttons: Ab9Buttons { mask, hat },
    })
}

fn norm(v: i16) -> f32 {
    v as f32 / 32767.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn centred() -> [u8; AB9_REPORT_LEN] {
        let mut r = [0u8; AB9_REPORT_LEN];
        r[0] = 0x01;
        r
    }

    #[test]
    fn test_centred_axes_are_zero() {
        let s = parse_ab9_report(&centred()).unwrap();
        assert!(s.axes.roll.abs() < 1e-4);
        assert!(s.axes.pitch.abs() < 1e-4);
        assert!((s.axes.throttle - 0.5).abs() < 1e-3);
    }

    #[test]
    fn test_full_roll_deflection() {
        let mut r = centred();
        r[1] = 0xFF;
        r[2] = 0x7F; // +32767
        let s = parse_ab9_report(&r).unwrap();
        assert!((s.axes.roll - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_report_too_short() {
        assert!(parse_ab9_report(&[0u8; 4]).is_err());
    }

    #[test]
    fn test_wrong_report_id() {
        let mut r = centred();
        r[0] = 0x05;
        assert!(matches!(
            parse_ab9_report(&r),
            Err(MozaParseError::UnknownReportId { .. })
        ));
    }

    #[test]
    fn test_button_pressed() {
        let mut r = centred();
        r[9] = 0b0000_0110; // buttons 2 and 3
        let s = parse_ab9_report(&r).unwrap();
        assert!(!s.buttons.is_pressed(1));
        assert!(s.buttons.is_pressed(2));
        assert!(s.buttons.is_pressed(3));
    }

    #[test]
    fn test_throttle_full_range() {
        let mut r = centred();
        // Min throttle: i16 = -32768
        r[5] = 0x00;
        r[6] = 0x80;
        let s = parse_ab9_report(&r).unwrap();
        assert!(s.axes.throttle < 0.01);

        // Max throttle: i16 = +32767
        r[5] = 0xFF;
        r[6] = 0x7F;
        let s = parse_ab9_report(&r).unwrap();
        assert!(s.axes.throttle > 0.99);
    }
}
