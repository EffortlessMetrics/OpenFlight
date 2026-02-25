// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID report parsing for the VPforce Rhino FFB joystick.
//!
//! The Rhino exposes a standard USB HID joystick descriptor with 6 axes
//! (X/Y/Z for roll/pitch/throttle; Rx/Ry/Rz for twist/rocker/extra) and
//! up to 32 buttons.  Reports are 20 bytes (report ID 0x01).
//!
//! All axis values are 16-bit signed integers (-32768..32767), normalised
//! to \[-1.0, 1.0\] by this parser.

/// USB Vendor ID assigned to the Rhino hardware (STMicroelectronics MCU).
pub const VPFORCE_VENDOR_ID: u16 = 0x0483;

/// USB Product ID for the VPforce Rhino (revision 2).
pub const RHINO_PID_V2: u16 = 0xA1C0;

/// USB Product ID for the VPforce Rhino (revision 3 / Mk II).
pub const RHINO_PID_V3: u16 = 0xA1C1;

/// Expected HID input report size (bytes, including report ID).
pub const RHINO_REPORT_LEN: usize = 20;

/// Normalised axis snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct RhinoAxes {
    /// Roll (stick X) — \[-1.0, 1.0\].
    pub roll: f32,
    /// Pitch (stick Y) — \[-1.0, 1.0\].
    pub pitch: f32,
    /// Throttle (stick Z or separate slider) — \[0.0, 1.0\].
    pub throttle: f32,
    /// Twist (Rz) — \[-1.0, 1.0\].
    pub twist: f32,
    /// Side rocker / toe brake axis (Rx) — \[-1.0, 1.0\].
    pub rocker: f32,
}

/// Button and hat-switch state.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RhinoButtons {
    /// 32-bit bitmask of pressed buttons (bit 0 = button 1).
    pub mask: u32,
    /// 8-direction POV hat value (0=N, 1=NE, … 7=NW, 0xFF=centred).
    pub hat: u8,
}

impl RhinoButtons {
    /// Returns `true` if button `n` (1-based) is pressed.
    pub fn is_pressed(&self, n: u8) -> bool {
        (1u8..=32).contains(&n) && (self.mask >> (n - 1)) & 1 == 1
    }
}

/// Parsed state from a single Rhino HID report.
#[derive(Debug, Clone)]
pub struct RhinoInputState {
    pub axes: RhinoAxes,
    pub buttons: RhinoButtons,
}

/// Error type for Rhino report parsing.
#[derive(Debug, thiserror::Error)]
pub enum RhinoParseError {
    #[error("report too short: expected ≥{expected} bytes, got {got}")]
    TooShort { expected: usize, got: usize },
    #[error("unknown report ID: 0x{id:02X}")]
    UnknownReportId { id: u8 },
}

/// Parse a raw HID input report from the Rhino.
///
/// # Example
///
/// ```
/// use flight_ffb_vpforce::input::{parse_report, RHINO_REPORT_LEN};
///
/// let mut report = [0u8; RHINO_REPORT_LEN];
/// report[0] = 0x01; // report ID
/// let state = parse_report(&report).unwrap();
/// assert!((state.axes.roll).abs() < 1e-3);
/// ```
pub fn parse_report(data: &[u8]) -> Result<RhinoInputState, RhinoParseError> {
    if data.len() < RHINO_REPORT_LEN {
        return Err(RhinoParseError::TooShort {
            expected: RHINO_REPORT_LEN,
            got: data.len(),
        });
    }
    if data[0] != 0x01 {
        return Err(RhinoParseError::UnknownReportId { id: data[0] });
    }

    // Bytes 1-2: X (roll), 3-4: Y (pitch), 5-6: Z (throttle/slider),
    // 7-8: Rx (rocker), 9-10: Ry (unused), 11-12: Rz (twist)
    let roll = norm_i16(read_i16(data, 1));
    let pitch = norm_i16(read_i16(data, 3));
    let throttle_raw = norm_i16(read_i16(data, 5));
    let throttle = (throttle_raw + 1.0) * 0.5; // remap [-1,1] → [0,1]
    let rocker = norm_i16(read_i16(data, 7));
    let twist = norm_i16(read_i16(data, 11));

    // Bytes 13-16: button mask (LE u32), byte 17: hat
    let button_mask = u32::from_le_bytes([data[13], data[14], data[15], data[16]]);
    let hat = data[17];

    Ok(RhinoInputState {
        axes: RhinoAxes {
            roll,
            pitch,
            throttle,
            twist,
            rocker,
        },
        buttons: RhinoButtons {
            mask: button_mask,
            hat,
        },
    })
}

fn read_i16(data: &[u8], offset: usize) -> i16 {
    i16::from_le_bytes([data[offset], data[offset + 1]])
}

fn norm_i16(v: i16) -> f32 {
    (v as f32 / 32767.0).clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn centred_report() -> [u8; RHINO_REPORT_LEN] {
        let mut r = [0u8; RHINO_REPORT_LEN];
        r[0] = 0x01;
        // All axes at zero (0x0000 = centred for signed i16)
        r
    }

    fn full_deflection_report() -> [u8; RHINO_REPORT_LEN] {
        let mut r = [0u8; RHINO_REPORT_LEN];
        r[0] = 0x01;
        // Roll +full: 0x7FFF = 32767
        r[1] = 0xFF;
        r[2] = 0x7F;
        r
    }

    #[test]
    fn test_parse_centred_report_axes_zero() {
        let state = parse_report(&centred_report()).unwrap();
        assert!(state.axes.roll.abs() < 1e-4);
        assert!(state.axes.pitch.abs() < 1e-4);
        assert!((state.axes.throttle - 0.5).abs() < 1e-3);
        assert!(state.axes.twist.abs() < 1e-4);
    }

    #[test]
    fn test_parse_full_roll_deflection() {
        let state = parse_report(&full_deflection_report()).unwrap();
        assert!((state.axes.roll - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_parse_report_too_short_returns_error() {
        assert!(parse_report(&[0u8; 5]).is_err());
    }

    #[test]
    fn test_parse_wrong_report_id_returns_error() {
        let mut r = [0u8; RHINO_REPORT_LEN];
        r[0] = 0x02;
        assert!(matches!(
            parse_report(&r),
            Err(RhinoParseError::UnknownReportId { .. })
        ));
    }

    #[test]
    fn test_button_pressed_detection() {
        let mut r = centred_report();
        r[13] = 0b0000_0101; // buttons 1 and 3 pressed
        let state = parse_report(&r).unwrap();
        assert!(state.buttons.is_pressed(1));
        assert!(!state.buttons.is_pressed(2));
        assert!(state.buttons.is_pressed(3));
    }

    #[test]
    fn test_hat_parsed() {
        let mut r = centred_report();
        r[17] = 0x02; // East
        let state = parse_report(&r).unwrap();
        assert_eq!(state.buttons.hat, 0x02);
    }

    #[test]
    fn test_throttle_remapped_to_0_1() {
        // Throttle at minimum i16 (-32768) should give ~0.0
        let mut r = centred_report();
        r[5] = 0x00;
        r[6] = 0x80; // -32768 in LE
        let state = parse_report(&r).unwrap();
        assert!(state.axes.throttle < 0.01);

        // Throttle at +32767 should give ~1.0
        r[5] = 0xFF;
        r[6] = 0x7F;
        let state = parse_report(&r).unwrap();
        assert!(state.axes.throttle > 0.99);
    }
}
