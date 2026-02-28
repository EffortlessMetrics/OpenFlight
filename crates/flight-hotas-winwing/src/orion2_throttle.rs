// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID report parser for the WinWing Orion 2 F/A-18C Throttle (PID 0xBE62).
//!
//! The Orion 2 Throttle is a split dual-throttle unit with Hall-effect sensors
//! on both throttle levers, a friction slider, a mouse/slew stick, 50 buttons,
//! and 5 rotary encoders.
//!
//! # Report layout (24 bytes, report ID `0x01`)
//!
//! | Offset | Size | Field |
//! |--------|------|-------|
//! | 0      | 1    | Report ID (`0x01`) |
//! | 1      | 2    | Left throttle (u16 LE, 0..65535 → 0.0..1.0) |
//! | 3      | 2    | Right throttle (u16 LE, 0..65535 → 0.0..1.0) |
//! | 5      | 2    | Friction slider (u16 LE, 0..65535 → 0.0..1.0) |
//! | 7      | 2    | Mouse stick X (u16 LE, midpoint 32768 → −1.0..1.0) |
//! | 9      | 2    | Mouse stick Y (u16 LE, midpoint 32768 → −1.0..1.0) |
//! | 11     | 8    | Button bitmask (u64 LE, bits 0–49 = buttons 1–50) |
//! | 19     | 5    | Encoder deltas (5 × i8; positive = CW, negative = CCW) |

use thiserror::Error;

/// USB Product ID for the WinWing Orion 2 F/A-18C Throttle.
pub const ORION2_THROTTLE_PID: u16 = 0xBE62;

/// Minimum bytes required in a valid Orion 2 Throttle HID report.
pub const MIN_REPORT_BYTES: usize = 24;

/// Number of mapped buttons on the Orion 2 Throttle.
pub const BUTTON_COUNT: u8 = 50;

/// Number of rotary encoders on the Orion 2 Throttle.
pub const ENCODER_COUNT: usize = 5;

const REPORT_ID: u8 = 0x01;

// ── Axis snapshot ─────────────────────────────────────────────────────────────

/// Axis snapshot for the WinWing Orion 2 F/A-18C Throttle.
#[derive(Debug, Clone, PartialEq)]
pub struct Orion2ThrottleAxes {
    /// Left throttle lever — \[0.0, 1.0\] (0.0 = idle, 1.0 = full power).
    pub throttle_left: f32,
    /// Right throttle lever — \[0.0, 1.0\] (0.0 = idle, 1.0 = full power).
    pub throttle_right: f32,
    /// Combined throttle (average of left and right) — \[0.0, 1.0\].
    pub throttle_combined: f32,
    /// Friction slider — \[0.0, 1.0\] (0.0 = no friction, 1.0 = full friction).
    pub friction: f32,
    /// Mouse/slew stick X — \[−1.0, 1.0\] (negative = left, positive = right).
    pub mouse_x: f32,
    /// Mouse/slew stick Y — \[−1.0, 1.0\] (negative = forward, positive = aft).
    pub mouse_y: f32,
}

// ── Button snapshot ───────────────────────────────────────────────────────────

/// Button and encoder state for the WinWing Orion 2 Throttle.
#[derive(Debug, Clone, Default)]
pub struct Orion2ThrottleButtons {
    /// Bitmask for up to 50 buttons; bit `n−1` set → button `n` pressed.
    pub mask: u64,
    /// Encoder detent deltas (positive = CW, negative = CCW), 5 encoders.
    pub encoders: [i8; ENCODER_COUNT],
}

impl Orion2ThrottleButtons {
    /// Returns `true` if button `n` (1-indexed, 1–50) is pressed.
    pub fn is_pressed(&self, n: u8) -> bool {
        (1u8..=BUTTON_COUNT).contains(&n) && (self.mask >> (n - 1)) & 1 == 1
    }
}

// ── Input state ───────────────────────────────────────────────────────────────

/// Parsed state from a single Orion 2 Throttle HID report.
#[derive(Debug, Clone)]
pub struct Orion2ThrottleInputState {
    pub axes: Orion2ThrottleAxes,
    pub buttons: Orion2ThrottleButtons,
}

// ── Parser ────────────────────────────────────────────────────────────────────

/// Parse a raw HID report from the WinWing Orion 2 F/A-18C Throttle.
///
/// # Errors
///
/// Returns [`Orion2ThrottleParseError::TooShort`] if `data` is shorter than
/// [`MIN_REPORT_BYTES`], or [`Orion2ThrottleParseError::UnknownReportId`] if
/// `data[0]` is not `0x01`.
pub fn parse_orion2_throttle_report(
    data: &[u8],
) -> Result<Orion2ThrottleInputState, Orion2ThrottleParseError> {
    if data.len() < MIN_REPORT_BYTES {
        return Err(Orion2ThrottleParseError::TooShort {
            expected: MIN_REPORT_BYTES,
            got: data.len(),
        });
    }
    if data[0] != REPORT_ID {
        return Err(Orion2ThrottleParseError::UnknownReportId { id: data[0] });
    }

    let tl = read_u16(data, 1) as f32 / 65535.0;
    let tr = read_u16(data, 3) as f32 / 65535.0;
    let friction = read_u16(data, 5) as f32 / 65535.0;
    let mouse_x = norm_u16_bipolar(read_u16(data, 7));
    let mouse_y = norm_u16_bipolar(read_u16(data, 9));
    let mask = u64::from_le_bytes(data[11..19].try_into().unwrap());
    let encoders = [
        data[19] as i8,
        data[20] as i8,
        data[21] as i8,
        data[22] as i8,
        data[23] as i8,
    ];

    Ok(Orion2ThrottleInputState {
        axes: Orion2ThrottleAxes {
            throttle_left: tl,
            throttle_right: tr,
            throttle_combined: (tl + tr) * 0.5,
            friction,
            mouse_x,
            mouse_y,
        },
        buttons: Orion2ThrottleButtons { mask, encoders },
    })
}

// ── Error type ────────────────────────────────────────────────────────────────

/// Errors returned by [`parse_orion2_throttle_report`].
#[derive(Debug, Error, PartialEq)]
pub enum Orion2ThrottleParseError {
    #[error("report too short: expected ≥{expected} bytes, got {got}")]
    TooShort { expected: usize, got: usize },
    #[error("unknown report ID: 0x{id:02X}")]
    UnknownReportId { id: u8 },
}

// ── Simple API ────────────────────────────────────────────────────────────────

/// USB Product ID for the WinWing Orion 2 Throttle Base (estimated).
pub const ORION2_THROTTLE_BASE_PID: u16 = 0xB71E;

/// USB Product ID for the WinWing Orion 2 F16EX Throttle (estimated).
pub const ORION2_F16EX_THROTTLE_PID: u16 = 0xB71F;

/// Minimum byte count for the simple Orion 2 throttle parser.
///
/// report_id (1) + 4 axes × 2 bytes (8) + 5 button bytes (5) = 14.
pub const ORION2_THROTTLE_MIN_REPORT_BYTES: usize = 14;

const ORION2_SIMPLE_AXIS_COUNT: usize = 4;
const ORION2_SIMPLE_BUTTON_BYTES: usize = 5;

/// Simple parsed state from an Orion 2 throttle HID report.
///
/// This is a lightweight alternative to [`Orion2ThrottleInputState`] that
/// exposes raw 16-bit axis values and a button bitmask without normalization.
#[derive(Debug, Clone, PartialEq)]
pub struct Orion2ThrottleState {
    /// Main throttle 0–65535.
    pub throttle_main: u16,
    /// Secondary throttle (differential / right engine), 0–65535.
    pub throttle_secondary: u16,
    /// Axis 3 (e.g., radar range), 0–65535.
    pub axis3: u16,
    /// Axis 4 (e.g., alt), 0–65535.
    pub axis4: u16,
    /// Button bitmask (up to 37 buttons across 5 bytes, LSB = button 1).
    pub buttons: u64,
}

/// Parse a raw HID report (≥14 bytes) into [`Orion2ThrottleState`].
///
/// # Errors
///
/// Returns [`WinWingError::ReportTooShort`] if `report` has fewer than
/// [`ORION2_THROTTLE_MIN_REPORT_BYTES`] bytes.
pub fn parse_orion2_throttle(report: &[u8]) -> Result<Orion2ThrottleState, crate::WinWingError> {
    if report.len() < ORION2_THROTTLE_MIN_REPORT_BYTES {
        return Err(crate::WinWingError::ReportTooShort {
            need: ORION2_THROTTLE_MIN_REPORT_BYTES,
            got: report.len(),
        });
    }
    let payload = &report[1..]; // skip report_id
    let throttle_main = u16::from_le_bytes([payload[0], payload[1]]);
    let throttle_secondary = u16::from_le_bytes([payload[2], payload[3]]);
    let axis3 = u16::from_le_bytes([payload[4], payload[5]]);
    let axis4 = u16::from_le_bytes([payload[6], payload[7]]);

    let btn_start = 1 + ORION2_SIMPLE_AXIS_COUNT * 2;
    let mut buttons: u64 = 0;
    for i in 0..ORION2_SIMPLE_BUTTON_BYTES {
        buttons |= (report[btn_start + i] as u64) << (i * 8);
    }

    Ok(Orion2ThrottleState {
        throttle_main,
        throttle_secondary,
        axis3,
        axis4,
        buttons,
    })
}

/// Normalize a 16-bit bipolar axis (0–65535) to −1.0 … +1.0.
///
/// Center (32767/32768) maps to approximately 0.0.
pub fn normalize_axis_16bit(raw: u16) -> f32 {
    ((raw as f32 / 32767.5) - 1.0).clamp(-1.0, 1.0)
}

/// Normalize a 16-bit throttle (0–65535) to 0.0 … 1.0.
pub fn normalize_throttle_16bit(raw: u16) -> f32 {
    raw as f32 / 65535.0
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn read_u16(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

/// Treat an unsigned u16 as bipolar relative to midpoint 32768 → \[−1.0, 1.0\].
fn norm_u16_bipolar(v: u16) -> f32 {
    let signed = v.wrapping_sub(32768) as i16;
    (signed as f32 / 32767.0).clamp(-1.0, 1.0)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Build a minimal valid report with the given throttle raw values.
    fn make_report(tl: u16, tr: u16) -> [u8; MIN_REPORT_BYTES] {
        let mut r = [0u8; MIN_REPORT_BYTES];
        r[0] = REPORT_ID;
        r[1..3].copy_from_slice(&tl.to_le_bytes());
        r[3..5].copy_from_slice(&tr.to_le_bytes());
        r
    }

    #[test]
    fn test_throttle_min_position() {
        let s = parse_orion2_throttle_report(&make_report(0, 0)).unwrap();
        assert!(s.axes.throttle_left < 0.001);
        assert!(s.axes.throttle_right < 0.001);
        assert!(s.axes.throttle_combined < 0.001);
    }

    #[test]
    fn test_throttle_max_position() {
        let s = parse_orion2_throttle_report(&make_report(0xFFFF, 0xFFFF)).unwrap();
        assert!((s.axes.throttle_left - 1.0).abs() < 1e-4);
        assert!((s.axes.throttle_right - 1.0).abs() < 1e-4);
        assert!((s.axes.throttle_combined - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_throttle_combined_is_average() {
        let s = parse_orion2_throttle_report(&make_report(0xFFFF, 0)).unwrap();
        assert!((s.axes.throttle_combined - 0.5).abs() < 1e-3);
    }

    #[test]
    fn test_friction_slider_range() {
        let mut r = make_report(0, 0);
        r[5..7].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let s = parse_orion2_throttle_report(&r).unwrap();
        assert!((s.axes.friction - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_mouse_stick_centred() {
        let mut r = make_report(0, 0);
        // Midpoint 32768 → bipolar 0.0
        r[7..9].copy_from_slice(&32768u16.to_le_bytes());
        r[9..11].copy_from_slice(&32768u16.to_le_bytes());
        let s = parse_orion2_throttle_report(&r).unwrap();
        assert!(s.axes.mouse_x.abs() < 1e-4, "mouse_x centre should be ~0");
        assert!(s.axes.mouse_y.abs() < 1e-4, "mouse_y centre should be ~0");
    }

    #[test]
    fn test_button_detection() {
        let mut r = make_report(0, 0);
        r[11] = 0b0000_1001; // buttons 1 and 4
        let s = parse_orion2_throttle_report(&r).unwrap();
        assert!(s.buttons.is_pressed(1));
        assert!(!s.buttons.is_pressed(2));
        assert!(s.buttons.is_pressed(4));
    }

    #[test]
    fn test_button_out_of_bounds() {
        let s = parse_orion2_throttle_report(&make_report(0xFFFF, 0xFFFF)).unwrap();
        assert!(!s.buttons.is_pressed(0), "button 0 is out of range");
        assert!(!s.buttons.is_pressed(51), "button 51 is out of range");
    }

    #[test]
    fn test_encoder_delta() {
        let mut r = make_report(0, 0);
        r[19] = 1u8; // encoder 0: +1 CW
        r[20] = (-3i8) as u8; // encoder 1: -3 CCW
        r[23] = 2u8; // encoder 4: +2 CW
        let s = parse_orion2_throttle_report(&r).unwrap();
        assert_eq!(s.buttons.encoders[0], 1);
        assert_eq!(s.buttons.encoders[1], -3);
        assert_eq!(s.buttons.encoders[4], 2);
    }

    #[test]
    fn test_report_too_short() {
        let err = parse_orion2_throttle_report(&[0u8; 10]).unwrap_err();
        assert_eq!(
            err,
            Orion2ThrottleParseError::TooShort {
                expected: MIN_REPORT_BYTES,
                got: 10
            }
        );
    }

    #[test]
    fn test_empty_report() {
        let err = parse_orion2_throttle_report(&[]).unwrap_err();
        assert_eq!(
            err,
            Orion2ThrottleParseError::TooShort {
                expected: MIN_REPORT_BYTES,
                got: 0
            }
        );
    }

    #[test]
    fn test_wrong_report_id() {
        let mut r = make_report(0, 0);
        r[0] = 0xFF;
        let err = parse_orion2_throttle_report(&r).unwrap_err();
        assert_eq!(err, Orion2ThrottleParseError::UnknownReportId { id: 0xFF });
    }

    proptest! {
        #[test]
        fn prop_throttle_axes_always_in_range(tl: u16, tr: u16, fr: u16) {
            let mut r = make_report(tl, tr);
            r[5..7].copy_from_slice(&fr.to_le_bytes());
            let s = parse_orion2_throttle_report(&r).unwrap();
            prop_assert!(
                (0.0..=1.0).contains(&s.axes.throttle_left),
                "throttle_left out of [0,1]: {}",
                s.axes.throttle_left
            );
            prop_assert!(
                (0.0..=1.0).contains(&s.axes.throttle_right),
                "throttle_right out of [0,1]: {}",
                s.axes.throttle_right
            );
            prop_assert!(
                (0.0..=1.0).contains(&s.axes.throttle_combined),
                "throttle_combined out of [0,1]: {}",
                s.axes.throttle_combined
            );
            prop_assert!(
                (0.0..=1.0).contains(&s.axes.friction),
                "friction out of [0,1]: {}",
                s.axes.friction
            );
        }

        #[test]
        fn prop_mouse_axes_always_in_range(mx: u16, my: u16) {
            let mut r = make_report(0, 0);
            r[7..9].copy_from_slice(&mx.to_le_bytes());
            r[9..11].copy_from_slice(&my.to_le_bytes());
            let s = parse_orion2_throttle_report(&r).unwrap();
            prop_assert!(
                (-1.001..=1.001).contains(&s.axes.mouse_x),
                "mouse_x out of [-1,1]: {}",
                s.axes.mouse_x
            );
            prop_assert!(
                (-1.001..=1.001).contains(&s.axes.mouse_y),
                "mouse_y out of [-1,1]: {}",
                s.axes.mouse_y
            );
        }
    }
}
