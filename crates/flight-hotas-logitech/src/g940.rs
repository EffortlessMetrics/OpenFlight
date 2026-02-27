// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the Logitech Flight System G940 FFB HOTAS.
//!
//! # Confirmed device identifier
//!
//! VID 0x046D (Logitech), PID 0xC287 — confirmed via linux-hardware.org (1 probe,
//! "Flight System G940"). The throttle enumerates as a separate USB device via the
//! G940's internal hub; PID 0xC288 is inferred but unconfirmed on hardware.
//!
//! # Device overview
//!
//! The G940 is one of the few consumer-grade FFB HOTAS sets ever produced (~2009–2013).
//! It includes:
//! - Force-feedback joystick (VID 0x046D, PID 0xC287): X/Y/Rz axes with DirectInput FFB
//! - Throttle unit (PID 0xC288, unconfirmed): dual-lever throttle with switches
//! - Optional rudder pedals — all connected via an internal USB 2.0 hub
//!
//! # Axis resolution
//!
//! Per `compat/devices/logitech/g940.yaml`: `resolution_bits: 12` (range 0–4095).
//!
//! # Report layout
//!
//! **Caution:** The exact HID descriptor byte layout for the G940 has not been
//! independently verified on hardware. The parsing below follows a plausible
//! 12-bit LSB-first packing scheme consistent with similar Logitech devices.
//! Validate against actual hardware before relying on this parser in production.
//!
//! ## Joystick report (estimated 11 bytes, 12-bit LSB-first packing)
//!
//! | Bit range | Field     | Type | Range   | Notes                         |
//! |-----------|-----------|------|---------|-------------------------------|
//! | 0-11      | X         | u12  | 0..4095 | Roll; center ~2047            |
//! | 12-23     | Y         | u12  | 0..4095 | Pitch; center ~2047           |
//! | 24-35     | Z         | u12  | 0..4095 | Stick throttle (unipolar)     |
//! | 36-47     | Rz        | u12  | 0..4095 | Twist/yaw; center ~2047       |
//! | 48-67     | Buttons   | u20  | bitmask | 20 buttons, LSB-first         |
//! | 68-71     | Hat 1     | u4   | 0-15    | Main hat; 0=N, …, 7=NW        |
//! | 72-75     | Hat 2     | u4   | 0-15    | Secondary hat                 |
//! | 76-79     | Hat 3     | u4   | 0-15    | Tertiary hat                  |
//! | 80-87     | Padding   | —    | —       | Reserved                      |
//!
//! ## Throttle report (estimated 5 bytes, 12-bit LSB-first packing)
//!
//! | Bit range | Field          | Type | Range   | Notes                        |
//! |-----------|----------------|------|---------|------------------------------|
//! | 0-11      | Left throttle  | u12  | 0..4095 | Unipolar; 0=idle, 4095=full  |
//! | 12-23     | Right throttle | u12  | 0..4095 | Unipolar; 0=idle, 4095=full  |
//! | 24-34     | Buttons        | u11  | bitmask | 11 buttons, LSB-first        |

use thiserror::Error;

/// USB Product ID for the G940 joystick/FFB stick interface.
///
/// Identical to [`flight_hid_support::device_support::G940_FLIGHT_SYSTEM_PID`];
/// provided here as a local convenience constant.
pub const G940_JOYSTICK_PID: u16 = 0xC287;

/// USB Product ID for the G940 throttle interface.
///
/// **Unconfirmed:** Inferred from the sequential Logitech flight-controller
/// numbering (Force 3D Pro = 0xC286, G940 joystick = 0xC287 → throttle = 0xC288).
/// No independent hardware probe has confirmed this PID.
pub const G940_THROTTLE_PID: u16 = 0xC288;

/// Axis resolution in bits (12-bit, per `compat/devices/logitech/g940.yaml`).
pub const G940_AXIS_BITS: u8 = 12;

/// Maximum raw axis value for 12-bit axes (4095).
pub const G940_AXIS_MAX: u16 = (1 << 12) - 1; // 4095

/// Nominal raw center value for bipolar 12-bit axes (2047).
pub const G940_AXIS_CENTER: u16 = 2047;

/// Minimum joystick HID input report length in bytes.
pub const G940_JOYSTICK_MIN_REPORT_BYTES: usize = 11;

/// Minimum throttle HID input report length in bytes.
pub const G940_THROTTLE_MIN_REPORT_BYTES: usize = 5;

/// Hat switch positions for the G940 8-way hat switches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum G940Hat {
    #[default]
    Center,
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
    NorthWest,
}

impl G940Hat {
    fn from_nibble(nibble: u8) -> Self {
        match nibble & 0x0F {
            0 => Self::North,
            1 => Self::NorthEast,
            2 => Self::East,
            3 => Self::SouthEast,
            4 => Self::South,
            5 => Self::SouthWest,
            6 => Self::West,
            7 => Self::NorthWest,
            _ => Self::Center, // 8-15
        }
    }
}

/// Parsed input state from a G940 joystick HID report.
#[derive(Debug, Clone, Default)]
pub struct G940InputState {
    /// Roll axis (X). −1.0 = full left, 1.0 = full right.
    pub x: f32,
    /// Pitch axis (Y). −1.0 = full forward, 1.0 = full back.
    pub y: f32,
    /// Twist/yaw axis (Rz). −1.0 = full left twist, 1.0 = full right twist.
    pub rz: f32,
    /// Stick body throttle (Z axis). 0.0 = idle, 1.0 = full travel.
    pub z: f32,
    /// Button bitmask; bit 0 = button 1, bit 19 = button 20. Upper 12 bits unused.
    pub buttons: u32,
    /// Primary hat switch position.
    pub hat: G940Hat,
}

impl G940InputState {
    /// Returns `true` if the specified button (1-indexed, 1–20) is pressed.
    pub fn button(&self, n: u8) -> bool {
        matches!(n, 1..=20) && (self.buttons >> (n - 1)) & 1 != 0
    }
}

/// Parsed input state from a G940 throttle HID report.
#[derive(Debug, Clone, Default)]
pub struct G940ThrottleState {
    /// Left throttle lever. 0.0 = idle, 1.0 = full forward.
    pub left_throttle: f32,
    /// Right throttle lever. 0.0 = idle, 1.0 = full forward.
    pub right_throttle: f32,
    /// Button bitmask; bit 0 = button 1, bit 10 = button 11. Upper 5 bits unused.
    pub buttons: u16,
}

impl G940ThrottleState {
    /// Returns `true` if the specified button (1-indexed, 1–11) is pressed.
    pub fn button(&self, n: u8) -> bool {
        matches!(n, 1..=11) && (self.buttons >> (n - 1)) & 1 != 0
    }
}

/// Errors returned by G940 report parsing.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum G940ParseError {
    #[error("G940 joystick report too short: expected at least {expected} bytes, got {actual}")]
    JoystickTooShort { expected: usize, actual: usize },
    #[error("G940 throttle report too short: expected at least {expected} bytes, got {actual}")]
    ThrottleTooShort { expected: usize, actual: usize },
}

/// Normalize a 12-bit bipolar axis (0..4095) to −1.0..=1.0.
#[inline]
fn normalize_12bit_bipolar(raw: u16) -> f32 {
    (raw as f32 - 2047.5) / 2047.5
}

/// Normalize a 12-bit unipolar axis (0..4095) to 0.0..=1.0.
#[inline]
fn normalize_12bit_unipolar(raw: u16) -> f32 {
    raw as f32 / 4095.0
}

/// Extract the lower 12-bit value from a 3-byte, two-value 12-bit pack.
///
/// Two consecutive 12-bit values share 3 bytes (LSB-first bit order):
/// - `data[0]` = bits 7:0 of value A
/// - `data[1]` bits 3:0 = bits 11:8 of value A; bits 7:4 = bits 3:0 of value B
/// - `data[2]` = bits 11:4 of value B
#[inline]
fn extract_12bit_lo(data: &[u8]) -> u16 {
    (data[0] as u16) | ((data[1] as u16 & 0x0F) << 8)
}

/// Extract the upper 12-bit value from a 3-byte, two-value 12-bit pack.
///
/// See [`extract_12bit_lo`] for the byte layout.
#[inline]
fn extract_12bit_hi(data: &[u8]) -> u16 {
    ((data[1] as u16) >> 4) | ((data[2] as u16) << 4)
}

/// Parse an estimated 11-byte HID input report from the G940 joystick.
///
/// **Note:** The byte layout is approximate and based on a plausible 12-bit
/// LSB-first packing scheme. Validate against hardware before use.
///
/// # Bit layout
///
/// ```text
/// Bytes 0-2:  X[11:0] (lo) and Y[11:0] (hi), packed as two 12-bit values
/// Bytes 3-5:  Z[11:0] (lo) and Rz[11:0] (hi), packed as two 12-bit values
/// Byte  6:    Buttons[7:0]
/// Byte  7:    Buttons[15:8]
/// Byte  8:    Buttons[19:16] in bits 3:0; Hat1[3:0] in bits 7:4
/// Byte  9:    Hat2[3:0] in bits 3:0; Hat3[3:0] in bits 7:4
/// Byte  10:   Padding
/// ```
///
/// # Errors
///
/// Returns [`G940ParseError::JoystickTooShort`] if `data` is shorter than
/// [`G940_JOYSTICK_MIN_REPORT_BYTES`].
pub fn parse_g940_joystick(data: &[u8]) -> Result<G940InputState, G940ParseError> {
    if data.len() < G940_JOYSTICK_MIN_REPORT_BYTES {
        return Err(G940ParseError::JoystickTooShort {
            expected: G940_JOYSTICK_MIN_REPORT_BYTES,
            actual: data.len(),
        });
    }

    // Bytes 0-2: X (lo) and Y (hi)
    let x = extract_12bit_lo(&data[0..]);
    let y = extract_12bit_hi(&data[0..]);

    // Bytes 3-5: Z (lo) and Rz (hi)
    let z = extract_12bit_lo(&data[3..]);
    let rz = extract_12bit_hi(&data[3..]);

    // Bytes 6-8: 20 buttons (lower nibble of byte 8) + Hat 1 (upper nibble of byte 8)
    let buttons = (data[6] as u32) | ((data[7] as u32) << 8) | (((data[8] as u32) & 0x0F) << 16);
    let hat1_raw = (data[8] >> 4) & 0x0F;

    Ok(G940InputState {
        x: normalize_12bit_bipolar(x),
        y: normalize_12bit_bipolar(y),
        rz: normalize_12bit_bipolar(rz),
        z: normalize_12bit_unipolar(z),
        buttons,
        hat: G940Hat::from_nibble(hat1_raw),
    })
}

/// Parse an estimated 5-byte HID input report from the G940 throttle.
///
/// **Note:** The byte layout is approximate. Validate against hardware before use.
///
/// # Bit layout
///
/// ```text
/// Bytes 0-2: LeftThrottle[11:0] (lo) and RightThrottle[11:0] (hi)
/// Byte  3:   Buttons[7:0]
/// Byte  4:   Buttons[10:8] in bits 2:0; padding in bits 7:3
/// ```
///
/// # Errors
///
/// Returns [`G940ParseError::ThrottleTooShort`] if `data` is shorter than
/// [`G940_THROTTLE_MIN_REPORT_BYTES`].
pub fn parse_g940_throttle(data: &[u8]) -> Result<G940ThrottleState, G940ParseError> {
    if data.len() < G940_THROTTLE_MIN_REPORT_BYTES {
        return Err(G940ParseError::ThrottleTooShort {
            expected: G940_THROTTLE_MIN_REPORT_BYTES,
            actual: data.len(),
        });
    }

    // Bytes 0-2: left throttle (lo) and right throttle (hi)
    let left = extract_12bit_lo(&data[0..]);
    let right = extract_12bit_hi(&data[0..]);

    // Bytes 3-4: 11 buttons
    let buttons = (data[3] as u16) | (((data[4] as u16) & 0x07) << 8);

    Ok(G940ThrottleState {
        left_throttle: normalize_12bit_unipolar(left),
        right_throttle: normalize_12bit_unipolar(right),
        buttons,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build an 11-byte G940 joystick report from logical field values.
    ///
    /// `hat` uses the same encoding as [`G940Hat::from_nibble`]: 0=N, …, 7=NW, 8-15=Center.
    fn build_joystick_report(x: u16, y: u16, z: u16, rz: u16, buttons: u32, hat: u8) -> [u8; 11] {
        let x = x & 0x0FFF;
        let y = y & 0x0FFF;
        let z = z & 0x0FFF;
        let rz = rz & 0x0FFF;
        let buttons = buttons & 0x000F_FFFF; // 20 bits
        let hat = hat & 0x0F;

        let mut data = [0u8; 11];
        // Bytes 0-2: X and Y
        data[0] = x as u8;
        data[1] = ((x >> 8) as u8 & 0x0F) | (((y & 0x0F) as u8) << 4);
        data[2] = (y >> 4) as u8;
        // Bytes 3-5: Z and Rz
        data[3] = z as u8;
        data[4] = ((z >> 8) as u8 & 0x0F) | (((rz & 0x0F) as u8) << 4);
        data[5] = (rz >> 4) as u8;
        // Bytes 6-8: buttons + hat 1
        data[6] = buttons as u8;
        data[7] = (buttons >> 8) as u8;
        data[8] = ((buttons >> 16) as u8 & 0x0F) | (hat << 4);
        // Byte 9: hats 2 and 3 centered (value 8 = center)
        data[9] = 8 | (8 << 4);
        // Byte 10: padding
        data[10] = 0;
        data
    }

    /// Build a 5-byte G940 throttle report from logical field values.
    fn build_throttle_report(left: u16, right: u16, buttons: u16) -> [u8; 5] {
        let left = left & 0x0FFF;
        let right = right & 0x0FFF;
        let buttons = buttons & 0x07FF; // 11 bits

        let mut data = [0u8; 5];
        data[0] = left as u8;
        data[1] = ((left >> 8) as u8 & 0x0F) | (((right & 0x0F) as u8) << 4);
        data[2] = (right >> 4) as u8;
        data[3] = buttons as u8;
        data[4] = ((buttons >> 8) as u8) & 0x07;
        data
    }

    // ── Joystick error-path tests ──────────────────────────────────────────────

    #[test]
    fn test_joystick_too_short() {
        assert!(parse_g940_joystick(&[0u8; 10]).is_err());
        assert!(parse_g940_joystick(&[]).is_err());
        let err = parse_g940_joystick(&[0u8; 5]).unwrap_err();
        assert_eq!(
            err,
            G940ParseError::JoystickTooShort {
                expected: 11,
                actual: 5
            }
        );
    }

    #[test]
    fn test_throttle_too_short() {
        assert!(parse_g940_throttle(&[0u8; 4]).is_err());
        assert!(parse_g940_throttle(&[]).is_err());
        let err = parse_g940_throttle(&[0u8; 2]).unwrap_err();
        assert_eq!(
            err,
            G940ParseError::ThrottleTooShort {
                expected: 5,
                actual: 2
            }
        );
    }

    // ── Joystick axis tests ────────────────────────────────────────────────────

    #[test]
    fn test_joystick_centered_axes() {
        // Center raw value (2048) maps to a value very close to 0.0
        let data = build_joystick_report(2048, 2048, 0, 2048, 0, 8);
        let state = parse_g940_joystick(&data).unwrap();
        assert!(state.x.abs() < 0.01, "x near 0: {}", state.x);
        assert!(state.y.abs() < 0.01, "y near 0: {}", state.y);
        assert!(state.rz.abs() < 0.01, "rz near 0: {}", state.rz);
    }

    #[test]
    fn test_x_full_right() {
        let data = build_joystick_report(4095, 2048, 0, 2048, 0, 8);
        let state = parse_g940_joystick(&data).unwrap();
        assert!(state.x > 0.999, "x should be ~1.0: {}", state.x);
    }

    #[test]
    fn test_x_full_left() {
        let data = build_joystick_report(0, 2048, 0, 2048, 0, 8);
        let state = parse_g940_joystick(&data).unwrap();
        assert!(state.x < -0.999, "x should be ~-1.0: {}", state.x);
    }

    #[test]
    fn test_y_full_forward() {
        let data = build_joystick_report(2048, 0, 0, 2048, 0, 8);
        let state = parse_g940_joystick(&data).unwrap();
        assert!(
            state.y < -0.999,
            "y full forward should be ~-1.0: {}",
            state.y
        );
    }

    #[test]
    fn test_y_full_back() {
        let data = build_joystick_report(2048, 4095, 0, 2048, 0, 8);
        let state = parse_g940_joystick(&data).unwrap();
        assert!(state.y > 0.999, "y full back should be ~1.0: {}", state.y);
    }

    #[test]
    fn test_z_throttle_min() {
        let data = build_joystick_report(2048, 2048, 0, 2048, 0, 8);
        let state = parse_g940_joystick(&data).unwrap();
        assert!(
            state.z < 0.001,
            "z throttle min should be ~0.0: {}",
            state.z
        );
    }

    #[test]
    fn test_z_throttle_max() {
        let data = build_joystick_report(2048, 2048, 4095, 2048, 0, 8);
        let state = parse_g940_joystick(&data).unwrap();
        assert!(
            state.z > 0.999,
            "z throttle max should be ~1.0: {}",
            state.z
        );
    }

    #[test]
    fn test_rz_twist_center() {
        let data = build_joystick_report(2048, 2048, 0, 2048, 0, 8);
        let state = parse_g940_joystick(&data).unwrap();
        assert!(state.rz.abs() < 0.01, "rz near 0: {}", state.rz);
    }

    #[test]
    fn test_rz_twist_full_right() {
        let data = build_joystick_report(2048, 2048, 0, 4095, 0, 8);
        let state = parse_g940_joystick(&data).unwrap();
        assert!(
            state.rz > 0.999,
            "rz full right should be ~1.0: {}",
            state.rz
        );
    }

    #[test]
    fn test_rz_twist_full_left() {
        let data = build_joystick_report(2048, 2048, 0, 0, 0, 8);
        let state = parse_g940_joystick(&data).unwrap();
        assert!(
            state.rz < -0.999,
            "rz full left should be ~-1.0: {}",
            state.rz
        );
    }

    // ── Hat switch tests ───────────────────────────────────────────────────────

    #[test]
    fn test_hat_center() {
        let data = build_joystick_report(2048, 2048, 0, 2048, 0, 8);
        let state = parse_g940_joystick(&data).unwrap();
        assert_eq!(state.hat, G940Hat::Center);
    }

    #[test]
    fn test_hat_north() {
        let data = build_joystick_report(2048, 2048, 0, 2048, 0, 0);
        let state = parse_g940_joystick(&data).unwrap();
        assert_eq!(state.hat, G940Hat::North);
    }

    #[test]
    fn test_hat_south() {
        let data = build_joystick_report(2048, 2048, 0, 2048, 0, 4);
        let state = parse_g940_joystick(&data).unwrap();
        assert_eq!(state.hat, G940Hat::South);
    }

    #[test]
    fn test_hat_northeast() {
        let data = build_joystick_report(2048, 2048, 0, 2048, 0, 1);
        let state = parse_g940_joystick(&data).unwrap();
        assert_eq!(state.hat, G940Hat::NorthEast);
    }

    #[test]
    fn test_hat_northwest() {
        let data = build_joystick_report(2048, 2048, 0, 2048, 0, 7);
        let state = parse_g940_joystick(&data).unwrap();
        assert_eq!(state.hat, G940Hat::NorthWest);
    }

    // ── Button tests ───────────────────────────────────────────────────────────

    #[test]
    fn test_joystick_buttons_individual() {
        for b in 1u8..=20 {
            let mask = 1u32 << (b - 1);
            let data = build_joystick_report(2048, 2048, 0, 2048, mask, 8);
            let state = parse_g940_joystick(&data).unwrap();
            assert!(state.button(b), "button {} should be pressed", b);
            for other in 1u8..=20 {
                if other != b {
                    assert!(
                        !state.button(other),
                        "button {} should NOT be pressed when {} is",
                        other,
                        b
                    );
                }
            }
        }
    }

    #[test]
    fn test_joystick_all_buttons_pressed() {
        let data = build_joystick_report(2048, 2048, 0, 2048, 0x000F_FFFF, 8);
        let state = parse_g940_joystick(&data).unwrap();
        for b in 1u8..=20 {
            assert!(state.button(b), "button {} should be pressed", b);
        }
    }

    #[test]
    fn test_joystick_out_of_range_buttons_false() {
        let data = build_joystick_report(2048, 2048, 0, 2048, 0x000F_FFFF, 8);
        let state = parse_g940_joystick(&data).unwrap();
        assert!(!state.button(0), "button 0 out of range");
        for b in 21u8..=30 {
            assert!(!state.button(b), "button {} out of range", b);
        }
    }

    // ── Throttle tests ─────────────────────────────────────────────────────────

    #[test]
    fn test_left_throttle_min() {
        let data = build_throttle_report(0, 2048, 0);
        let state = parse_g940_throttle(&data).unwrap();
        assert!(
            state.left_throttle < 0.001,
            "left throttle min should be ~0.0: {}",
            state.left_throttle
        );
    }

    #[test]
    fn test_left_throttle_max() {
        let data = build_throttle_report(4095, 0, 0);
        let state = parse_g940_throttle(&data).unwrap();
        assert!(
            state.left_throttle > 0.999,
            "left throttle max should be ~1.0: {}",
            state.left_throttle
        );
    }

    #[test]
    fn test_right_throttle_min() {
        let data = build_throttle_report(2048, 0, 0);
        let state = parse_g940_throttle(&data).unwrap();
        assert!(
            state.right_throttle < 0.001,
            "right throttle min should be ~0.0: {}",
            state.right_throttle
        );
    }

    #[test]
    fn test_right_throttle_max() {
        let data = build_throttle_report(0, 4095, 0);
        let state = parse_g940_throttle(&data).unwrap();
        assert!(
            state.right_throttle > 0.999,
            "right throttle max should be ~1.0: {}",
            state.right_throttle
        );
    }

    #[test]
    fn test_throttle_buttons_individual() {
        for b in 1u8..=11 {
            let mask = 1u16 << (b - 1);
            let data = build_throttle_report(0, 0, mask);
            let state = parse_g940_throttle(&data).unwrap();
            assert!(state.button(b), "throttle button {} should be pressed", b);
            for other in 1u8..=11 {
                if other != b {
                    assert!(
                        !state.button(other),
                        "throttle button {} should NOT be pressed when {} is",
                        other,
                        b
                    );
                }
            }
        }
    }

    #[test]
    fn test_throttle_all_buttons_pressed() {
        let data = build_throttle_report(0, 0, 0x07FF);
        let state = parse_g940_throttle(&data).unwrap();
        for b in 1u8..=11 {
            assert!(state.button(b), "throttle button {} should be pressed", b);
        }
    }

    #[test]
    fn test_throttle_out_of_range_buttons_false() {
        let data = build_throttle_report(0, 0, 0x07FF);
        let state = parse_g940_throttle(&data).unwrap();
        assert!(!state.button(0), "button 0 out of range");
        for b in 12u8..=20 {
            assert!(!state.button(b), "throttle button {} out of range", b);
        }
    }

    // ── Constants ─────────────────────────────────────────────────────────────

    #[test]
    fn test_axis_constants() {
        assert_eq!(G940_JOYSTICK_PID, 0xC287);
        assert_eq!(G940_THROTTLE_PID, 0xC288);
        assert_eq!(G940_AXIS_BITS, 12);
        assert_eq!(G940_AXIS_MAX, 4095);
        assert_eq!(G940_AXIS_CENTER, 2047);
    }

    #[test]
    fn test_axis_range_samples() {
        for x_raw in [0u16, 1024, 2047, 2048, 3071, 4095] {
            let data = build_joystick_report(x_raw, 2048, 0, 2048, 0, 8);
            let state = parse_g940_joystick(&data).unwrap();
            assert!(
                (-1.0..=1.0).contains(&state.x),
                "x out of range at raw {}: {}",
                x_raw,
                state.x
            );
        }
    }

    #[test]
    fn test_joystick_minimum_length_parses() {
        let data = build_joystick_report(2048, 2048, 0, 2048, 0, 8);
        assert!(parse_g940_joystick(&data).is_ok());
    }

    #[test]
    fn test_throttle_minimum_length_parses() {
        let data = build_throttle_report(2048, 2048, 0);
        assert!(parse_g940_throttle(&data).is_ok());
    }

    #[cfg(test)]
    mod proptest_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn x_axis_always_in_range(x in 0u16..=4095) {
                let data = build_joystick_report(x, 2048, 0, 2048, 0, 8);
                let state = parse_g940_joystick(&data).unwrap();
                prop_assert!((-1.0f32..=1.0).contains(&state.x));
            }

            #[test]
            fn y_axis_always_in_range(y in 0u16..=4095) {
                let data = build_joystick_report(2048, y, 0, 2048, 0, 8);
                let state = parse_g940_joystick(&data).unwrap();
                prop_assert!((-1.0f32..=1.0).contains(&state.y));
            }

            #[test]
            fn z_axis_always_unipolar(z in 0u16..=4095) {
                let data = build_joystick_report(2048, 2048, z, 2048, 0, 8);
                let state = parse_g940_joystick(&data).unwrap();
                prop_assert!((0.0f32..=1.0).contains(&state.z));
            }

            #[test]
            fn rz_axis_always_in_range(rz in 0u16..=4095) {
                let data = build_joystick_report(2048, 2048, 0, rz, 0, 8);
                let state = parse_g940_joystick(&data).unwrap();
                prop_assert!((-1.0f32..=1.0).contains(&state.rz));
            }

            #[test]
            fn joystick_buttons_roundtrip(buttons in 0u32..=0x000F_FFFFu32) {
                let data = build_joystick_report(2048, 2048, 0, 2048, buttons, 8);
                let state = parse_g940_joystick(&data).unwrap();
                prop_assert_eq!(state.buttons, buttons);
            }

            #[test]
            fn left_throttle_always_unipolar(left in 0u16..=4095) {
                let data = build_throttle_report(left, 0, 0);
                let state = parse_g940_throttle(&data).unwrap();
                prop_assert!((0.0f32..=1.0).contains(&state.left_throttle));
            }

            #[test]
            fn right_throttle_always_unipolar(right in 0u16..=4095) {
                let data = build_throttle_report(0, right, 0);
                let state = parse_g940_throttle(&data).unwrap();
                prop_assert!((0.0f32..=1.0).contains(&state.right_throttle));
            }

            #[test]
            fn throttle_buttons_roundtrip(buttons in 0u16..=0x07FFu16) {
                let data = build_throttle_report(0, 0, buttons);
                let state = parse_g940_throttle(&data).unwrap();
                prop_assert_eq!(state.buttons, buttons);
            }

            #[test]
            fn any_joystick_report_parses(data in proptest::collection::vec(any::<u8>(), 11..20usize)) {
                let result = parse_g940_joystick(&data);
                prop_assert!(result.is_ok());
            }

            #[test]
            fn any_throttle_report_parses(data in proptest::collection::vec(any::<u8>(), 5..10usize)) {
                let result = parse_g940_throttle(&data);
                prop_assert!(result.is_ok());
            }

            /// Arbitrary byte patterns must always produce axis values within normalised ranges.
            #[test]
            fn arbitrary_joystick_bytes_all_axes_in_range(
                data in proptest::collection::vec(any::<u8>(), 11..20usize),
            ) {
                let state = parse_g940_joystick(&data).unwrap();
                prop_assert!(
                    (-1.0f32..=1.0).contains(&state.x),
                    "x out of range: {}",
                    state.x
                );
                prop_assert!(
                    (-1.0f32..=1.0).contains(&state.y),
                    "y out of range: {}",
                    state.y
                );
                prop_assert!(
                    (0.0f32..=1.0).contains(&state.z),
                    "z out of range: {}",
                    state.z
                );
                prop_assert!(
                    (-1.0f32..=1.0).contains(&state.rz),
                    "rz out of range: {}",
                    state.rz
                );
            }

            /// Arbitrary byte patterns must always produce throttle values within unipolar range.
            #[test]
            fn arbitrary_throttle_bytes_in_range(
                data in proptest::collection::vec(any::<u8>(), 5..10usize),
            ) {
                let state = parse_g940_throttle(&data).unwrap();
                prop_assert!(
                    (0.0f32..=1.0).contains(&state.left_throttle),
                    "left throttle out of range: {}",
                    state.left_throttle
                );
                prop_assert!(
                    (0.0f32..=1.0).contains(&state.right_throttle),
                    "right throttle out of range: {}",
                    state.right_throttle
                );
            }

            /// Button numbers outside valid ranges must always return false.
            #[test]
            fn out_of_range_joystick_buttons_always_false(
                data in proptest::collection::vec(any::<u8>(), 11..20usize),
            ) {
                let state = parse_g940_joystick(&data).unwrap();
                prop_assert!(!state.button(0));
                for b in 21u8..=32 {
                    prop_assert!(
                        !state.button(b),
                        "joystick button {} out of range should be false",
                        b
                    );
                }
                prop_assert_eq!(
                    state.buttons & 0xFFF0_0000,
                    0,
                    "upper 12 bits of joystick button word must be 0"
                );
            }
        }
    }
}
