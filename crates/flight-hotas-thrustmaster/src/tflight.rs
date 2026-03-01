// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Standalone HID input parsing for Thrustmaster T.Flight HOTAS X/4/One.
//!
//! # Confirmed device identifiers
//!
//! - T.Flight HOTAS X: VID 0x044F, PID 0xB108 (combined stick + throttle)
//! - T.Flight HOTAS 4: VID 0x044F, PID 0xB67B (primary PID)
//! - T.Flight HOTAS 4 (legacy): VID 0x044F, PID 0xB67A (older firmware)
//! - T.Flight HOTAS 4 v2: VID 0x044F, PID 0xB67C (newer firmware)
//! - T.Flight HOTAS One: VID 0x044F, PID 0xB68D (interrupt, Xbox/PC)
//! - T.Flight HOTAS One (bulk): VID 0x044F, PID 0xB68B
//! - T.Flight Stick X: VID 0x044F, PID 0xB106 (standalone joystick)
//! - T.Flight Stick X v2: VID 0x044F, PID 0xB107
//!
//! # Report formats
//!
//! The T.Flight family uses two HID report layouts selected by the physical
//! axis-mode switch on the throttle base:
//!
//! ## Merged mode (8-byte payload)
//!
//! Twist and rocker are combined into a single yaw axis.
//!
//! | Bytes | Field         | Type   | Range     | Notes                          |
//! |-------|---------------|--------|-----------|--------------------------------|
//! | 0–1   | X             | u16 LE | 0..=65535 | Stick horizontal; center ~32768|
//! | 2–3   | Y             | u16 LE | 0..=65535 | Stick vertical; center ~32768  |
//! | 4     | Throttle      | u8     | 0..=255   | Throttle lever; 0=idle         |
//! | 5     | Rz (combined) | u8     | 0..=255   | Twist + rocker combined; 128=center |
//! | 6–7   | Buttons + Hat | u16 LE |           | Bits 0–11 → buttons 1–12, upper nibble byte 7 → hat |
//!
//! ## Separate mode (9-byte payload)
//!
//! Twist and rocker are reported on independent axes.
//!
//! | Bytes | Field    | Type   | Range     | Notes                          |
//! |-------|----------|--------|-----------|--------------------------------|
//! | 0–1   | X        | u16 LE | 0..=65535 | Stick horizontal; center ~32768|
//! | 2–3   | Y        | u16 LE | 0..=65535 | Stick vertical; center ~32768  |
//! | 4     | Throttle | u8     | 0..=255   | Throttle lever; 0=idle         |
//! | 5     | Twist Rz | u8     | 0..=255   | Twist yaw; 128=center          |
//! | 6     | Rocker   | u8     | 0..=255   | Rocker yaw; 128=center         |
//! | 7–8   | Buttons + Hat | u16 LE |      | Bits 0–11 → buttons 1–12, upper nibble byte 8 → hat |
//!
//! # Named button constants
//!
//! Button numbering follows the standard HID report bitmask order, matching
//! Thrustmaster's own documentation and TARGET scripting conventions.

use thiserror::Error;

// ─── Button constants ────────────────────────────────────────────────────────

/// Named button constants for the T.Flight HOTAS family.
///
/// Button numbers are 1-indexed, matching the HID bitmask order.
pub mod buttons {
    /// Trigger (main fire button on the stick grip).
    pub const TRIGGER: u8 = 1;
    /// Thumb button (large button on stick head, left side).
    pub const THUMB: u8 = 2;
    /// Bottom-left button (stick base, left cluster).
    pub const BOTTOM_LEFT: u8 = 3;
    /// Bottom-right button (stick base, right cluster).
    pub const BOTTOM_RIGHT: u8 = 4;
    /// Top-left button (stick head, upper-left).
    pub const TOP_LEFT: u8 = 5;
    /// Top-right button (stick head, upper-right).
    pub const TOP_RIGHT: u8 = 6;
    /// Left throttle button 1 (throttle base, left row, upper).
    pub const THROTTLE_L1: u8 = 7;
    /// Left throttle button 2 (throttle base, left row, lower).
    pub const THROTTLE_L2: u8 = 8;
    /// Right throttle button 1 (throttle base, right row, upper).
    pub const THROTTLE_R1: u8 = 9;
    /// Right throttle button 2 (throttle base, right row, lower).
    pub const THROTTLE_R2: u8 = 10;
    /// SE button (Select/Back on HOTAS 4, View on HOTAS One).
    pub const SELECT: u8 = 11;
    /// ST button (Start on HOTAS 4, Menu on HOTAS One).
    pub const START: u8 = 12;

    /// Total number of buttons on T.Flight HOTAS devices.
    pub const BUTTON_COUNT: u8 = 12;

    /// Returns the name of a button (1-indexed), or `None` for out-of-range.
    pub const fn name(n: u8) -> Option<&'static str> {
        match n {
            1 => Some("Trigger"),
            2 => Some("Thumb"),
            3 => Some("Bottom Left"),
            4 => Some("Bottom Right"),
            5 => Some("Top Left"),
            6 => Some("Top Right"),
            7 => Some("Throttle L1"),
            8 => Some("Throttle L2"),
            9 => Some("Throttle R1"),
            10 => Some("Throttle R2"),
            11 => Some("Select"),
            12 => Some("Start"),
            _ => None,
        }
    }
}

// ─── Hat encoding ────────────────────────────────────────────────────────────

/// T.Flight hat switch directions.
///
/// Encoding: upper nibble of the hat byte, 0=center, 1=N, 2=NE, ... 8=NW.
/// Values > 8 are treated as center.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TFlightHat {
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

impl TFlightHat {
    /// Decode a 4-bit nibble into a hat direction.
    pub fn from_nibble(nibble: u8) -> Self {
        match nibble & 0x0F {
            1 => Self::North,
            2 => Self::NorthEast,
            3 => Self::East,
            4 => Self::SouthEast,
            5 => Self::South,
            6 => Self::SouthWest,
            7 => Self::West,
            8 => Self::NorthWest,
            _ => Self::Center,
        }
    }

    /// Returns the hat direction as a numeric value (0=center, 1–8=directions).
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Center => 0,
            Self::North => 1,
            Self::NorthEast => 2,
            Self::East => 3,
            Self::SouthEast => 4,
            Self::South => 5,
            Self::SouthWest => 6,
            Self::West => 7,
            Self::NorthWest => 8,
        }
    }
}

// ─── Axis mode ───────────────────────────────────────────────────────────────

/// T.Flight axis report mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TFlightAxisMode {
    /// 8-byte report: twist and rocker combined into one yaw axis.
    Merged,
    /// 9-byte report: twist and rocker on separate axes.
    Separate,
}

// ─── Parsed types ────────────────────────────────────────────────────────────

/// Parsed axis values from a T.Flight HOTAS report.
#[derive(Debug, Clone, Default)]
pub struct TFlightStickAxes {
    /// Stick horizontal (X / roll). −1.0 = full left, 1.0 = full right.
    pub x: f32,
    /// Stick vertical (Y / pitch). −1.0 = full forward, 1.0 = full back.
    pub y: f32,
    /// Throttle lever. 0.0 = idle, 1.0 = full.
    pub throttle: f32,
    /// Twist yaw axis. −1.0 = full left, 1.0 = full right.
    /// In merged mode this is the combined twist+rocker value.
    pub twist: f32,
    /// Rocker axis (separate mode only). −1.0 = full left, 1.0 = full right.
    /// `None` in merged mode.
    pub rocker: Option<f32>,
}

/// Parsed buttons from a T.Flight HOTAS report.
#[derive(Debug, Clone, Default)]
pub struct TFlightStickButtons {
    /// Button bitmask; bits 0–11 → buttons 1–12.
    pub buttons: u16,
    /// Hat switch direction.
    pub hat: TFlightHat,
}

impl TFlightStickButtons {
    /// Returns `true` if button `n` (1-indexed, 1–12) is pressed.
    pub fn button(&self, n: u8) -> bool {
        match n {
            1..=12 => (self.buttons >> (n - 1)) & 1 != 0,
            _ => false,
        }
    }
}

/// Full parsed input state from a T.Flight HOTAS HID report.
#[derive(Debug, Clone)]
pub struct TFlightStickState {
    pub axes: TFlightStickAxes,
    pub buttons: TFlightStickButtons,
    /// Which axis mode was used to parse this report.
    pub mode: TFlightAxisMode,
}

// ─── Errors ──────────────────────────────────────────────────────────────────

/// Errors returned by T.Flight report parsing.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TFlightStickParseError {
    #[error(
        "T.Flight report too short: expected at least {expected} bytes, got {actual}"
    )]
    TooShort { expected: usize, actual: usize },

    #[error(
        "T.Flight report length {actual} is ambiguous: expected exactly 8 (merged) or 9 (separate) bytes; strip any Report ID prefix and pass the exact payload"
    )]
    AmbiguousLength { actual: usize },
}

// ─── Constants ───────────────────────────────────────────────────────────────

/// Minimum report payload for merged mode (8 bytes).
pub const TFLIGHT_MERGED_MIN_BYTES: usize = 8;

/// Minimum report payload for separate mode (9 bytes).
pub const TFLIGHT_SEPARATE_MIN_BYTES: usize = 9;

// ─── Normalization helpers ───────────────────────────────────────────────────

/// Normalize a 16-bit centered axis (0..65535) to −1.0..=1.0.
#[inline]
fn normalize_16bit_bipolar(raw: u16) -> f32 {
    ((raw as f32 - 32767.5) / 32767.5).clamp(-1.0, 1.0)
}

/// Normalize an 8-bit centered axis (0..255, center=128) to −1.0..=1.0.
#[inline]
fn normalize_8bit_bipolar(raw: u8) -> f32 {
    ((raw as f32 - 127.5) / 127.5).clamp(-1.0, 1.0)
}

/// Normalize an 8-bit unipolar axis (0..255) to 0.0..=1.0.
#[inline]
fn normalize_8bit_unipolar(raw: u8) -> f32 {
    (raw as f32 / 255.0).clamp(0.0, 1.0)
}

// ─── Parsers ─────────────────────────────────────────────────────────────────

/// Parse a T.Flight HOTAS HID report in **merged** mode (8-byte payload).
///
/// In merged mode, twist and rocker are combined into a single yaw axis.
/// Strip any leading report ID byte before calling.
///
/// # Errors
/// Returns [`TFlightStickParseError::TooShort`] if `data.len() < 8`.
pub fn parse_tflight_merged(data: &[u8]) -> Result<TFlightStickState, TFlightStickParseError> {
    if data.len() < TFLIGHT_MERGED_MIN_BYTES {
        return Err(TFlightStickParseError::TooShort {
            expected: TFLIGHT_MERGED_MIN_BYTES,
            actual: data.len(),
        });
    }

    let x = u16::from_le_bytes([data[0], data[1]]);
    let y = u16::from_le_bytes([data[2], data[3]]);
    let throttle = data[4];
    let rz_combined = data[5];
    let (btn_bits, hat_nibble) = decode_buttons_hat(&data[6..8]);

    Ok(TFlightStickState {
        axes: TFlightStickAxes {
            x: normalize_16bit_bipolar(x),
            y: normalize_16bit_bipolar(y),
            throttle: normalize_8bit_unipolar(throttle),
            twist: normalize_8bit_bipolar(rz_combined),
            rocker: None,
        },
        buttons: TFlightStickButtons {
            buttons: btn_bits,
            hat: TFlightHat::from_nibble(hat_nibble),
        },
        mode: TFlightAxisMode::Merged,
    })
}

/// Parse a T.Flight HOTAS HID report in **separate** mode (9-byte payload).
///
/// In separate mode, twist and rocker are on independent axes.
/// Strip any leading report ID byte before calling.
///
/// # Errors
/// Returns [`TFlightStickParseError::TooShort`] if `data.len() < 9`.
pub fn parse_tflight_separate(
    data: &[u8],
) -> Result<TFlightStickState, TFlightStickParseError> {
    if data.len() < TFLIGHT_SEPARATE_MIN_BYTES {
        return Err(TFlightStickParseError::TooShort {
            expected: TFLIGHT_SEPARATE_MIN_BYTES,
            actual: data.len(),
        });
    }

    let x = u16::from_le_bytes([data[0], data[1]]);
    let y = u16::from_le_bytes([data[2], data[3]]);
    let throttle = data[4];
    let twist = data[5];
    let rocker = data[6];
    let (btn_bits, hat_nibble) = decode_buttons_hat(&data[7..9]);

    Ok(TFlightStickState {
        axes: TFlightStickAxes {
            x: normalize_16bit_bipolar(x),
            y: normalize_16bit_bipolar(y),
            throttle: normalize_8bit_unipolar(throttle),
            twist: normalize_8bit_bipolar(twist),
            rocker: Some(normalize_8bit_bipolar(rocker)),
        },
        buttons: TFlightStickButtons {
            buttons: btn_bits,
            hat: TFlightHat::from_nibble(hat_nibble),
        },
        mode: TFlightAxisMode::Separate,
    })
}

/// Auto-detect report mode from payload length and parse accordingly.
///
/// - 8 bytes → merged mode
/// - 9 bytes → separate mode
/// - Other lengths → [`TFlightStickParseError::AmbiguousLength`]
///
/// Callers must strip any leading Report ID byte and pass the exact payload
/// (8 or 9 bytes) for unambiguous parsing.
///
/// # Errors
/// Returns [`TFlightStickParseError::TooShort`] if `data.len() < 8`.
/// Returns [`TFlightStickParseError::AmbiguousLength`] if `data.len() > 9`.
pub fn parse_tflight_auto(data: &[u8]) -> Result<TFlightStickState, TFlightStickParseError> {
    if data.len() < TFLIGHT_MERGED_MIN_BYTES {
        return Err(TFlightStickParseError::TooShort {
            expected: TFLIGHT_MERGED_MIN_BYTES,
            actual: data.len(),
        });
    }
    if data.len() > TFLIGHT_SEPARATE_MIN_BYTES {
        return Err(TFlightStickParseError::AmbiguousLength {
            actual: data.len(),
        });
    }
    if data.len() == TFLIGHT_MERGED_MIN_BYTES {
        parse_tflight_merged(data)
    } else {
        parse_tflight_separate(data)
    }
}

// ─── Internal helpers ────────────────────────────────────────────────────────

/// Decode button bitmask and hat nibble from the 2-byte button+hat field.
///
/// Layout: `[byte0, byte1]`
/// - Buttons: lower 12 bits across byte0 (all 8 bits) and byte1 lower nibble (bits 0–3)
/// - Hat: upper nibble of byte1 (bits 4–7)
fn decode_buttons_hat(bytes: &[u8]) -> (u16, u8) {
    let raw = u16::from_le_bytes([bytes[0], bytes[1] & 0x0F]);
    let hat = (bytes[1] >> 4) & 0x0F;
    (raw & 0x0FFF, hat)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Report builders ──────────────────────────────────────────────────

    fn make_merged(x: u16, y: u16, throttle: u8, rz: u8, buttons: u16, hat: u8) -> Vec<u8> {
        let mut r = vec![0u8; 8];
        r[0..2].copy_from_slice(&x.to_le_bytes());
        r[2..4].copy_from_slice(&y.to_le_bytes());
        r[4] = throttle;
        r[5] = rz;
        r[6] = (buttons & 0xFF) as u8;
        r[7] = ((buttons >> 8) & 0x0F) as u8 | (hat << 4);
        r
    }

    fn make_separate(
        x: u16,
        y: u16,
        throttle: u8,
        twist: u8,
        rocker: u8,
        buttons: u16,
        hat: u8,
    ) -> Vec<u8> {
        let mut r = vec![0u8; 9];
        r[0..2].copy_from_slice(&x.to_le_bytes());
        r[2..4].copy_from_slice(&y.to_le_bytes());
        r[4] = throttle;
        r[5] = twist;
        r[6] = rocker;
        r[7] = (buttons & 0xFF) as u8;
        r[8] = ((buttons >> 8) & 0x0F) as u8 | (hat << 4);
        r
    }

    // ── Merged mode tests ────────────────────────────────────────────────

    #[test]
    fn merged_too_short() {
        assert!(parse_tflight_merged(&[0u8; 7]).is_err());
        assert!(parse_tflight_merged(&[]).is_err());
    }

    #[test]
    fn merged_centered() {
        let r = make_merged(32768, 32768, 128, 128, 0, 0);
        let s = parse_tflight_merged(&r).unwrap();
        assert!(s.axes.x.abs() < 0.01, "x={}", s.axes.x);
        assert!(s.axes.y.abs() < 0.01, "y={}", s.axes.y);
        assert!((s.axes.throttle - 0.502).abs() < 0.01, "thr={}", s.axes.throttle);
        assert!(s.axes.twist.abs() < 0.01, "twist={}", s.axes.twist);
        assert!(s.axes.rocker.is_none());
        assert_eq!(s.buttons.hat, TFlightHat::Center);
        assert_eq!(s.mode, TFlightAxisMode::Merged);
    }

    #[test]
    fn merged_full_right() {
        let r = make_merged(65535, 32768, 0, 128, 0, 0);
        let s = parse_tflight_merged(&r).unwrap();
        assert!(s.axes.x > 0.99, "x={}", s.axes.x);
    }

    #[test]
    fn merged_full_left() {
        let r = make_merged(0, 32768, 0, 128, 0, 0);
        let s = parse_tflight_merged(&r).unwrap();
        assert!(s.axes.x < -0.99, "x={}", s.axes.x);
    }

    #[test]
    fn merged_full_throttle() {
        let r = make_merged(32768, 32768, 255, 128, 0, 0);
        let s = parse_tflight_merged(&r).unwrap();
        assert!(s.axes.throttle > 0.999, "thr={}", s.axes.throttle);
    }

    #[test]
    fn merged_idle_throttle() {
        let r = make_merged(32768, 32768, 0, 128, 0, 0);
        let s = parse_tflight_merged(&r).unwrap();
        assert!(s.axes.throttle < 0.001, "thr={}", s.axes.throttle);
    }

    #[test]
    fn merged_twist_full_right() {
        let r = make_merged(32768, 32768, 0, 255, 0, 0);
        let s = parse_tflight_merged(&r).unwrap();
        assert!(s.axes.twist > 0.99, "twist={}", s.axes.twist);
    }

    #[test]
    fn merged_twist_full_left() {
        let r = make_merged(32768, 32768, 0, 0, 0, 0);
        let s = parse_tflight_merged(&r).unwrap();
        assert!(s.axes.twist < -0.99, "twist={}", s.axes.twist);
    }

    #[test]
    fn merged_hat_north() {
        let r = make_merged(32768, 32768, 0, 128, 0, 1);
        let s = parse_tflight_merged(&r).unwrap();
        assert_eq!(s.buttons.hat, TFlightHat::North);
    }

    #[test]
    fn merged_hat_all_directions() {
        for (nibble, expected) in [
            (0, TFlightHat::Center),
            (1, TFlightHat::North),
            (2, TFlightHat::NorthEast),
            (3, TFlightHat::East),
            (4, TFlightHat::SouthEast),
            (5, TFlightHat::South),
            (6, TFlightHat::SouthWest),
            (7, TFlightHat::West),
            (8, TFlightHat::NorthWest),
            (9, TFlightHat::Center),
            (15, TFlightHat::Center),
        ] {
            let r = make_merged(32768, 32768, 0, 128, 0, nibble);
            let s = parse_tflight_merged(&r).unwrap();
            assert_eq!(s.buttons.hat, expected, "hat nibble {nibble}");
        }
    }

    #[test]
    fn merged_trigger_pressed() {
        let r = make_merged(32768, 32768, 0, 128, 0x0001, 0);
        let s = parse_tflight_merged(&r).unwrap();
        assert!(s.buttons.button(buttons::TRIGGER));
        assert!(!s.buttons.button(buttons::THUMB));
    }

    #[test]
    fn merged_all_buttons_pressed() {
        let r = make_merged(32768, 32768, 0, 128, 0x0FFF, 0);
        let s = parse_tflight_merged(&r).unwrap();
        for n in 1..=12u8 {
            assert!(s.buttons.button(n), "button {n} should be pressed");
        }
    }

    #[test]
    fn merged_no_buttons_pressed() {
        let r = make_merged(32768, 32768, 0, 128, 0, 0);
        let s = parse_tflight_merged(&r).unwrap();
        for n in 1..=12u8 {
            assert!(!s.buttons.button(n), "button {n} should not be pressed");
        }
    }

    #[test]
    fn merged_button_out_of_range() {
        let r = make_merged(32768, 32768, 0, 128, 0x0FFF, 0);
        let s = parse_tflight_merged(&r).unwrap();
        assert!(!s.buttons.button(0));
        assert!(!s.buttons.button(13));
        assert!(!s.buttons.button(255));
    }

    #[test]
    fn merged_individual_buttons() {
        for n in 1..=12u8 {
            let mask = 1u16 << (n - 1);
            let r = make_merged(32768, 32768, 0, 128, mask, 0);
            let s = parse_tflight_merged(&r).unwrap();
            assert!(s.buttons.button(n), "button {n} with mask 0x{mask:04X}");
            for other in 1..=12u8 {
                if other != n {
                    assert!(!s.buttons.button(other), "button {other} should not be set when only {n} is");
                }
            }
        }
    }

    #[test]
    fn merged_oversized_report() {
        let mut r = make_merged(32768, 32768, 128, 128, 0, 0);
        r.extend_from_slice(&[0xFF; 10]);
        assert!(parse_tflight_merged(&r).is_ok());
    }

    // ── Separate mode tests ──────────────────────────────────────────────

    #[test]
    fn separate_too_short() {
        assert!(parse_tflight_separate(&[0u8; 8]).is_err());
        assert!(parse_tflight_separate(&[]).is_err());
    }

    #[test]
    fn separate_centered() {
        let r = make_separate(32768, 32768, 128, 128, 128, 0, 0);
        let s = parse_tflight_separate(&r).unwrap();
        assert!(s.axes.x.abs() < 0.01);
        assert!(s.axes.y.abs() < 0.01);
        assert!(s.axes.twist.abs() < 0.01);
        assert!(s.axes.rocker.unwrap().abs() < 0.01);
        assert_eq!(s.mode, TFlightAxisMode::Separate);
    }

    #[test]
    fn separate_has_rocker() {
        let r = make_separate(32768, 32768, 0, 128, 255, 0, 0);
        let s = parse_tflight_separate(&r).unwrap();
        assert!(s.axes.rocker.is_some());
        assert!(s.axes.rocker.unwrap() > 0.99, "rocker={:?}", s.axes.rocker);
    }

    #[test]
    fn separate_rocker_full_left() {
        let r = make_separate(32768, 32768, 0, 128, 0, 0, 0);
        let s = parse_tflight_separate(&r).unwrap();
        assert!(s.axes.rocker.unwrap() < -0.99);
    }

    #[test]
    fn separate_twist_independent_of_rocker() {
        let r = make_separate(32768, 32768, 0, 0, 255, 0, 0);
        let s = parse_tflight_separate(&r).unwrap();
        assert!(s.axes.twist < -0.99, "twist should be full-left");
        assert!(s.axes.rocker.unwrap() > 0.99, "rocker should be full-right");
    }

    #[test]
    fn separate_full_deflection() {
        let r = make_separate(65535, 0, 255, 255, 255, 0x0FFF, 5);
        let s = parse_tflight_separate(&r).unwrap();
        assert!(s.axes.x > 0.99);
        assert!(s.axes.y < -0.99);
        assert!(s.axes.throttle > 0.999);
        assert!(s.axes.twist > 0.99);
        assert!(s.axes.rocker.unwrap() > 0.99);
        assert_eq!(s.buttons.hat, TFlightHat::South);
        for n in 1..=12u8 {
            assert!(s.buttons.button(n));
        }
    }

    #[test]
    fn separate_buttons() {
        let r = make_separate(32768, 32768, 0, 128, 128, 0x0005, 0);
        let s = parse_tflight_separate(&r).unwrap();
        assert!(s.buttons.button(1));
        assert!(!s.buttons.button(2));
        assert!(s.buttons.button(3));
    }

    #[test]
    fn separate_hat_east() {
        let r = make_separate(32768, 32768, 0, 128, 128, 0, 3);
        let s = parse_tflight_separate(&r).unwrap();
        assert_eq!(s.buttons.hat, TFlightHat::East);
    }

    #[test]
    fn separate_oversized_report() {
        let mut r = make_separate(32768, 32768, 128, 128, 128, 0, 0);
        r.extend_from_slice(&[0xFF; 10]);
        assert!(parse_tflight_separate(&r).is_ok());
    }

    // ── Auto-detect tests ────────────────────────────────────────────────

    #[test]
    fn auto_detect_merged_8_bytes() {
        let r = make_merged(32768, 32768, 128, 128, 0, 0);
        let s = parse_tflight_auto(&r).unwrap();
        assert_eq!(s.mode, TFlightAxisMode::Merged);
        assert!(s.axes.rocker.is_none());
    }

    #[test]
    fn auto_detect_separate_9_bytes() {
        let r = make_separate(32768, 32768, 128, 128, 128, 0, 0);
        let s = parse_tflight_auto(&r).unwrap();
        assert_eq!(s.mode, TFlightAxisMode::Separate);
        assert!(s.axes.rocker.is_some());
    }

    #[test]
    fn auto_detect_too_short() {
        assert!(parse_tflight_auto(&[0u8; 7]).is_err());
    }

    #[test]
    fn auto_detect_padded_report_is_ambiguous() {
        let mut r = make_separate(32768, 32768, 128, 128, 128, 0, 0);
        r.extend_from_slice(&[0; 10]);
        let err = parse_tflight_auto(&r).unwrap_err();
        assert!(
            matches!(err, TFlightStickParseError::AmbiguousLength { .. }),
            "padded report should be rejected as ambiguous, got: {err}"
        );
    }

    // ── Button name tests ────────────────────────────────────────────────

    #[test]
    fn button_names_valid_range() {
        for n in 1..=12u8 {
            assert!(buttons::name(n).is_some(), "button {n} should have a name");
        }
    }

    #[test]
    fn button_names_out_of_range() {
        assert!(buttons::name(0).is_none());
        assert!(buttons::name(13).is_none());
    }

    #[test]
    fn button_constants_match_expected() {
        assert_eq!(buttons::TRIGGER, 1);
        assert_eq!(buttons::THUMB, 2);
        assert_eq!(buttons::SELECT, 11);
        assert_eq!(buttons::START, 12);
        assert_eq!(buttons::BUTTON_COUNT, 12);
    }

    // ── Hat direction tests ──────────────────────────────────────────────

    #[test]
    fn hat_from_nibble_roundtrip() {
        for dir in [
            TFlightHat::Center,
            TFlightHat::North,
            TFlightHat::NorthEast,
            TFlightHat::East,
            TFlightHat::SouthEast,
            TFlightHat::South,
            TFlightHat::SouthWest,
            TFlightHat::West,
            TFlightHat::NorthWest,
        ] {
            let numeric = dir.as_u8();
            let decoded = TFlightHat::from_nibble(numeric);
            assert_eq!(decoded, dir, "roundtrip failed for {dir:?}");
        }
    }

    #[test]
    fn hat_as_u8_values() {
        assert_eq!(TFlightHat::Center.as_u8(), 0);
        assert_eq!(TFlightHat::North.as_u8(), 1);
        assert_eq!(TFlightHat::NorthWest.as_u8(), 8);
    }

    // ── Normalization boundary tests ─────────────────────────────────────

    #[test]
    fn normalize_16bit_boundaries() {
        assert!(normalize_16bit_bipolar(0) < -0.99);
        assert!(normalize_16bit_bipolar(32768).abs() < 0.01);
        assert!(normalize_16bit_bipolar(65535) > 0.99);
    }

    #[test]
    fn normalize_8bit_bipolar_boundaries() {
        assert!(normalize_8bit_bipolar(0) < -0.99);
        assert!(normalize_8bit_bipolar(128).abs() < 0.01);
        assert!(normalize_8bit_bipolar(255) > 0.99);
    }

    #[test]
    fn normalize_8bit_unipolar_boundaries() {
        assert!(normalize_8bit_unipolar(0) < 0.001);
        assert!((normalize_8bit_unipolar(128) - 0.502).abs() < 0.01);
        assert!(normalize_8bit_unipolar(255) > 0.999);
    }

    // ── Error display test ───────────────────────────────────────────────

    #[test]
    fn error_display() {
        let err = TFlightStickParseError::TooShort {
            expected: 8,
            actual: 3,
        };
        let msg = format!("{err}");
        assert!(msg.contains("8"));
        assert!(msg.contains("3"));
    }

    // ── Property tests ───────────────────────────────────────────────────

    mod proptest_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn merged_axes_always_in_range(
                x in 0u16..=u16::MAX,
                y in 0u16..=u16::MAX,
                throttle in 0u8..=255u8,
                rz in 0u8..=255u8,
            ) {
                let r = make_merged(x, y, throttle, rz, 0, 0);
                let s = parse_tflight_merged(&r).unwrap();
                prop_assert!((-1.0..=1.0).contains(&s.axes.x));
                prop_assert!((-1.0..=1.0).contains(&s.axes.y));
                prop_assert!((0.0..=1.0).contains(&s.axes.throttle));
                prop_assert!((-1.0..=1.0).contains(&s.axes.twist));
            }

            #[test]
            fn separate_axes_always_in_range(
                x in 0u16..=u16::MAX,
                y in 0u16..=u16::MAX,
                throttle in 0u8..=255u8,
                twist in 0u8..=255u8,
                rocker in 0u8..=255u8,
            ) {
                let r = make_separate(x, y, throttle, twist, rocker, 0, 0);
                let s = parse_tflight_separate(&r).unwrap();
                prop_assert!((-1.0..=1.0).contains(&s.axes.x));
                prop_assert!((-1.0..=1.0).contains(&s.axes.y));
                prop_assert!((0.0..=1.0).contains(&s.axes.throttle));
                prop_assert!((-1.0..=1.0).contains(&s.axes.twist));
                let rocker_val = s.axes.rocker.unwrap();
                prop_assert!((-1.0..=1.0).contains(&rocker_val));
            }

            #[test]
            fn merged_axes_always_finite(
                x in 0u16..=u16::MAX,
                y in 0u16..=u16::MAX,
                throttle in 0u8..=255u8,
                rz in 0u8..=255u8,
            ) {
                let r = make_merged(x, y, throttle, rz, 0, 0);
                let s = parse_tflight_merged(&r).unwrap();
                prop_assert!(s.axes.x.is_finite());
                prop_assert!(s.axes.y.is_finite());
                prop_assert!(s.axes.throttle.is_finite());
                prop_assert!(s.axes.twist.is_finite());
            }

            #[test]
            fn merged_short_always_errors(
                data in proptest::collection::vec(any::<u8>(), 0..TFLIGHT_MERGED_MIN_BYTES),
            ) {
                prop_assert!(parse_tflight_merged(&data).is_err());
            }

            #[test]
            fn separate_short_always_errors(
                data in proptest::collection::vec(any::<u8>(), 0..TFLIGHT_SEPARATE_MIN_BYTES),
            ) {
                prop_assert!(parse_tflight_separate(&data).is_err());
            }

            #[test]
            fn merged_buttons_decode_consistently(buttons in 0u16..=0x0FFF) {
                let r = make_merged(32768, 32768, 128, 128, buttons, 0);
                let s = parse_tflight_merged(&r).unwrap();
                for n in 1u8..=12 {
                    let expected = (buttons >> (n - 1)) & 1 != 0;
                    prop_assert_eq!(s.buttons.button(n), expected, "button {}", n);
                }
            }

            #[test]
            fn no_panic_on_arbitrary_merged(
                data in proptest::collection::vec(any::<u8>(), TFLIGHT_MERGED_MIN_BYTES..64),
            ) {
                let _ = parse_tflight_merged(&data);
            }

            #[test]
            fn no_panic_on_arbitrary_separate(
                data in proptest::collection::vec(any::<u8>(), TFLIGHT_SEPARATE_MIN_BYTES..64),
            ) {
                let _ = parse_tflight_separate(&data);
            }
        }
    }
}
