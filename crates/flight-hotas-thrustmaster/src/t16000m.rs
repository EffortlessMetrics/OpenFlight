// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the Thrustmaster T.16000M FCS joystick and
//! TWCS Throttle.
//!
//! # Confirmed device identifiers
//!
//! - T.16000M FCS joystick: VID 0x044F, PID 0xB10A (confirmed via linux-hardware.org)
//! - TWCS Throttle: VID 0x044F, PID 0xB687 (confirmed via linux-hardware.org)
//!
//! # Input report layouts (community-documented; hardware validation recommended)
//!
//! ## T.16000M FCS Joystick (11-byte payload + optional 1-byte report ID)
//!
//! | Bytes | Field   | Type   | Range      | Notes                          |
//! |-------|---------|--------|------------|-------------------------------|
//! | 0-1   | X       | u16 LE | 0..=16383  | Stick horizontal; center ~8191 |
//! | 2-3   | Y       | u16 LE | 0..=16383  | Stick vertical; center ~8191   |
//! | 4-5   | Rz      | u16 LE | 0..=16383  | Twist handle; center ~8191     |
//! | 6-7   | Slider  | u16 LE | 0..=65535  | Throttle lever; 0=idle         |
//! | 8-9   | Buttons | u16 LE | bitmask    | Bits 0-15 → buttons 1-16       |
//! | 10    | Hat     | u8     | 0-15       | 0x0F=center, 0=N, 2=E, 4=S, 6=W |
//!
//! ## TWCS Throttle (10-byte payload + optional 1-byte report ID)
//!
//! | Bytes | Field    | Type   | Range      | Notes                              |
//! |-------|----------|--------|------------|------------------------------------|
//! | 0-1   | Throttle | u16 LE | 0..=65535  | Main lever; 0=idle, 65535=full     |
//! | 2-3   | Rx       | u16 LE | 0..=65535  | Mini-stick X; center ~32767        |
//! | 4-5   | Ry       | u16 LE | 0..=65535  | Mini-stick Y; center ~32767        |
//! | 6-7   | Rz       | u16 LE | 0..=65535  | Rocker; center ~32767              |
//! | 8-9   | Buttons  | u16 LE | bitmask    | Bits 0-13 → buttons 1-14           |

use thiserror::Error;

/// Parsed axes from the T.16000M FCS joystick.
#[derive(Debug, Clone, Default)]
pub struct T16000mAxes {
    /// Stick horizontal (X / roll-yaw). Range −1.0 (left) to 1.0 (right).
    pub x: f32,
    /// Stick vertical (Y / pitch). Range −1.0 (forward/up) to 1.0 (back/down).
    pub y: f32,
    /// Twist handle (Rz / rudder). Range −1.0 to 1.0; center = 0.0.
    pub twist: f32,
    /// Throttle lever (slider). Range 0.0 (idle) to 1.0 (full).
    pub throttle: f32,
}

/// Parsed buttons from the T.16000M FCS joystick.
#[derive(Debug, Clone, Default)]
pub struct T16000mButtons {
    /// Button bitmask; bit 0 = button 1, bit 15 = button 16.
    pub buttons: u16,
    /// Hat position. 0 = center; 1 = N, 2 = NE, 3 = E, 4 = SE, 5 = S, 6 = SW,
    /// 7 = W, 8 = NW. 0x0F (15) is also treated as center.
    pub hat: u8,
}

/// Full parsed input state from a T.16000M FCS joystick HID report.
#[derive(Debug, Clone, Default)]
pub struct T16000mInputState {
    pub axes: T16000mAxes,
    pub buttons: T16000mButtons,
}

/// Parsed axes from the TWCS Throttle.
#[derive(Debug, Clone, Default)]
pub struct TwcsAxes {
    /// Main throttle lever. Range 0.0 (idle) to 1.0 (full).
    pub throttle: f32,
    /// Mini-stick X. Range −1.0 to 1.0; center = 0.0.
    pub mini_stick_x: f32,
    /// Mini-stick Y. Range −1.0 to 1.0; center = 0.0.
    pub mini_stick_y: f32,
    /// Rocker. Range −1.0 to 1.0; center = 0.0.
    pub rocker: f32,
}

/// Parsed buttons from the TWCS Throttle.
#[derive(Debug, Clone, Default)]
pub struct TwcsButtons {
    /// Button bitmask; bits 0-13 = buttons 1-14.
    pub buttons: u16,
}

/// Full parsed input state from a TWCS Throttle HID report.
#[derive(Debug, Clone, Default)]
pub struct TwcsInputState {
    pub axes: TwcsAxes,
    pub buttons: TwcsButtons,
}

/// Errors returned by T.16000M report parsing.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum T16000mParseError {
    /// The report is too short to contain a complete joystick payload.
    #[error("T.16000M joystick report too short: expected at least {expected} bytes, got {actual}")]
    JoystickReportTooShort { expected: usize, actual: usize },

    /// The report is too short to contain a complete TWCS payload.
    #[error("TWCS throttle report too short: expected at least {expected} bytes, got {actual}")]
    TwcsReportTooShort { expected: usize, actual: usize },

    /// The report has an unrecognised report ID.
    #[error("Unrecognised report ID 0x{id:02X}")]
    UnknownReportId { id: u8 },
}

// ──────────────────────────────────────────────────────────────────────────────
// T.16000M joystick axis scaling helpers
// ──────────────────────────────────────────────────────────────────────────────

/// Maximum raw value for the 14-bit centered axes (X, Y, twist).
const AXIS_14BIT_MAX: f32 = 16383.0;
const AXIS_14BIT_CENTER: f32 = AXIS_14BIT_MAX / 2.0; // 8191.5

/// Normalise a 14-bit raw axis value to −1.0 … 1.0 (centered).
#[inline]
fn norm_14bit_centered(raw: u16) -> f32 {
    let v = (raw & 0x3FFF) as f32;
    ((v - AXIS_14BIT_CENTER) / AXIS_14BIT_CENTER).clamp(-1.0, 1.0)
}

/// Normalise a 16-bit raw axis value to 0.0 … 1.0 (unipolar).
#[inline]
fn norm_16bit_unipolar(raw: u16) -> f32 {
    raw as f32 / u16::MAX as f32
}

/// Normalise a 16-bit raw axis value to −1.0 … 1.0 (centered, bipolar).
#[inline]
fn norm_16bit_centered(raw: u16) -> f32 {
    let center: f32 = u16::MAX as f32 / 2.0; // 32767.5
    ((raw as f32 - center) / center).clamp(-1.0, 1.0)
}

/// Decode a HID hat-switch nibble into a 0–8 direction value.
///
/// Standard HID encoding: 0=N, 1=NE, 2=E, 3=SE, 4=S, 5=SW, 6=W, 7=NW, ≥8=center.
/// Returns `0` for all centre/invalid encodings.
#[inline]
fn decode_hat(raw: u8) -> u8 {
    let nibble = raw & 0x0F;
    match nibble {
        0 => 1, // N
        1 => 2, // NE
        2 => 3, // E
        3 => 4, // SE
        4 => 5, // S
        5 => 6, // SW
        6 => 7, // W
        7 => 8, // NW
        _ => 0, // center / off
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// T.16000M joystick parser
// ──────────────────────────────────────────────────────────────────────────────

/// Minimum payload bytes required for a T.16000M joystick report (excluding
/// a potential leading report ID byte).
pub const T16000M_MIN_REPORT_BYTES: usize = 11;

/// Parse a T.16000M FCS joystick HID input report.
///
/// Accepts reports with or without a leading Report ID byte; if the first byte
/// is `0x01`, it is stripped before parsing.
///
/// # Errors
/// Returns [`T16000mParseError::JoystickReportTooShort`] if the payload is < 11 bytes.
/// Returns [`T16000mParseError::UnknownReportId`] if a report ID byte is present but
/// is not `0x01`.
pub fn parse_t16000m_report(data: &[u8]) -> Result<T16000mInputState, T16000mParseError> {
    let payload = strip_report_id_by_length(data, T16000M_MIN_REPORT_BYTES, 0x01)?;

    if payload.len() < T16000M_MIN_REPORT_BYTES {
        return Err(T16000mParseError::JoystickReportTooShort {
            expected: T16000M_MIN_REPORT_BYTES,
            actual: payload.len(),
        });
    }

    let x_raw = u16::from_le_bytes([payload[0], payload[1]]);
    let y_raw = u16::from_le_bytes([payload[2], payload[3]]);
    let rz_raw = u16::from_le_bytes([payload[4], payload[5]]);
    let slider_raw = u16::from_le_bytes([payload[6], payload[7]]);
    let buttons_raw = u16::from_le_bytes([payload[8], payload[9]]);
    let hat_raw = payload[10];

    Ok(T16000mInputState {
        axes: T16000mAxes {
            x: norm_14bit_centered(x_raw),
            y: norm_14bit_centered(y_raw),
            twist: norm_14bit_centered(rz_raw),
            throttle: norm_16bit_unipolar(slider_raw),
        },
        buttons: T16000mButtons {
            buttons: buttons_raw,
            hat: decode_hat(hat_raw),
        },
    })
}

// ──────────────────────────────────────────────────────────────────────────────
// TWCS Throttle parser
// ──────────────────────────────────────────────────────────────────────────────

/// Minimum payload bytes required for a TWCS throttle report (excluding a
/// potential leading report ID byte).
pub const TWCS_MIN_REPORT_BYTES: usize = 10;

/// Parse a TWCS Throttle HID input report.
///
/// Accepts reports with or without a leading Report ID byte; if the first byte
/// is `0x01`, it is stripped before parsing.
///
/// # Errors
/// Returns [`T16000mParseError::TwcsReportTooShort`] if the payload is < 10 bytes.
/// Returns [`T16000mParseError::UnknownReportId`] if a report ID byte is present but
/// is not `0x01`.
pub fn parse_twcs_report(data: &[u8]) -> Result<TwcsInputState, T16000mParseError> {
    let payload = strip_report_id_by_length(data, TWCS_MIN_REPORT_BYTES, 0x01)?;

    if payload.len() < TWCS_MIN_REPORT_BYTES {
        return Err(T16000mParseError::TwcsReportTooShort {
            expected: TWCS_MIN_REPORT_BYTES,
            actual: payload.len(),
        });
    }

    let throttle_raw = u16::from_le_bytes([payload[0], payload[1]]);
    let rx_raw = u16::from_le_bytes([payload[2], payload[3]]);
    let ry_raw = u16::from_le_bytes([payload[4], payload[5]]);
    let rz_raw = u16::from_le_bytes([payload[6], payload[7]]);
    let buttons_raw = u16::from_le_bytes([payload[8], payload[9]]);

    Ok(TwcsInputState {
        axes: TwcsAxes {
            throttle: norm_16bit_unipolar(throttle_raw),
            mini_stick_x: norm_16bit_centered(rx_raw),
            mini_stick_y: norm_16bit_centered(ry_raw),
            rocker: norm_16bit_centered(rz_raw),
        },
        buttons: TwcsButtons {
            buttons: buttons_raw & 0x3FFF, // only 14 buttons
        },
    })
}

// ──────────────────────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────────────────────

/// Strip a leading report ID byte, using length-based detection.
///
/// - If `data.len() == min_len`: no report ID prefix, return `data` as-is.
/// - If `data.len() == min_len + 1`:
///   - `data[0] == expected_id` → strip leading byte and return `&data[1..]`.
///   - `data[0] != expected_id` → return `UnknownReportId` error.
/// - If `data.len() > min_len + 1`: return `data` as-is (cannot reliably determine).
fn strip_report_id_by_length(
    data: &[u8],
    min_len: usize,
    expected_id: u8,
) -> Result<&[u8], T16000mParseError> {
    match data.len().cmp(&(min_len + 1)) {
        std::cmp::Ordering::Equal => {
            // Exactly one extra byte — check if it's the expected report ID.
            if data[0] == expected_id {
                Ok(&data[1..])
            } else {
                Err(T16000mParseError::UnknownReportId { id: data[0] })
            }
        }
        // Exact minimum or longer-than-expected: pass through unchanged.
        _ => Ok(data),
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── T.16000M Joystick ────────────────────────────────────────────────────

    fn make_joystick_report(
        x: u16,
        y: u16,
        rz: u16,
        slider: u16,
        buttons: u16,
        hat: u8,
    ) -> Vec<u8> {
        let mut r = vec![0u8; 11];
        r[0..2].copy_from_slice(&x.to_le_bytes());
        r[2..4].copy_from_slice(&y.to_le_bytes());
        r[4..6].copy_from_slice(&rz.to_le_bytes());
        r[6..8].copy_from_slice(&slider.to_le_bytes());
        r[8..10].copy_from_slice(&buttons.to_le_bytes());
        r[10] = hat;
        r
    }

    fn make_joystick_report_with_id(
        x: u16,
        y: u16,
        rz: u16,
        slider: u16,
        buttons: u16,
        hat: u8,
    ) -> Vec<u8> {
        let mut r = vec![0x01u8];
        r.extend_from_slice(&make_joystick_report(x, y, rz, slider, buttons, hat));
        r
    }

    #[test]
    fn test_joystick_centered() {
        let center: u16 = 8192;
        let report = make_joystick_report(center, center, center, 0, 0, 0x0F);
        let state = parse_t16000m_report(&report).unwrap();
        assert!(state.axes.x.abs() < 0.01, "x={}", state.axes.x);
        assert!(state.axes.y.abs() < 0.01, "y={}", state.axes.y);
        assert!(state.axes.twist.abs() < 0.01, "twist={}", state.axes.twist);
        assert!(state.axes.throttle.abs() < 0.01);
        assert_eq!(state.buttons.hat, 0, "hat should be center");
    }

    #[test]
    fn test_joystick_full_right() {
        let report = make_joystick_report(16383, 8192, 8192, 0, 0, 0x0F);
        let state = parse_t16000m_report(&report).unwrap();
        assert!(state.axes.x > 0.99, "x={}", state.axes.x);
    }

    #[test]
    fn test_joystick_full_left() {
        let report = make_joystick_report(0, 8192, 8192, 0, 0, 0x0F);
        let state = parse_t16000m_report(&report).unwrap();
        assert!(state.axes.x < -0.99, "x={}", state.axes.x);
    }

    #[test]
    fn test_joystick_full_throttle() {
        let report = make_joystick_report(8192, 8192, 8192, u16::MAX, 0, 0x0F);
        let state = parse_t16000m_report(&report).unwrap();
        assert!(
            state.axes.throttle > 0.999,
            "throttle={}",
            state.axes.throttle
        );
    }

    #[test]
    fn test_joystick_hat_north() {
        let report = make_joystick_report(8192, 8192, 8192, 0, 0, 0x00);
        let state = parse_t16000m_report(&report).unwrap();
        assert_eq!(state.buttons.hat, 1, "N=1");
    }

    #[test]
    fn test_joystick_hat_east() {
        let report = make_joystick_report(8192, 8192, 8192, 0, 0, 0x02);
        let state = parse_t16000m_report(&report).unwrap();
        assert_eq!(state.buttons.hat, 3, "E=3");
    }

    #[test]
    fn test_joystick_buttons() {
        let report = make_joystick_report(8192, 8192, 8192, 0, 0b0000_0000_0000_0101, 0x0F);
        let state = parse_t16000m_report(&report).unwrap();
        assert_eq!(state.buttons.buttons & 1, 1, "button 1");
        assert_eq!((state.buttons.buttons >> 2) & 1, 1, "button 3");
    }

    #[test]
    fn test_joystick_with_report_id() {
        let report = make_joystick_report_with_id(8192, 8192, 8192, 0, 0, 0x0F);
        let state = parse_t16000m_report(&report).unwrap();
        assert!(state.axes.x.abs() < 0.01);
    }

    #[test]
    fn test_joystick_too_short() {
        let err = parse_t16000m_report(&[0u8; 5]).unwrap_err();
        assert!(
            matches!(err, T16000mParseError::JoystickReportTooShort { .. }),
            "{err:?}"
        );
    }

    #[test]
    fn test_joystick_unknown_report_id() {
        // 0x02 is not a valid report ID for T.16000M joystick
        let mut r = vec![0x02u8];
        r.extend_from_slice(&make_joystick_report(8192, 8192, 8192, 0, 0, 0x0F));
        let err = parse_t16000m_report(&r).unwrap_err();
        assert!(
            matches!(err, T16000mParseError::UnknownReportId { id: 0x02 }),
            "{err:?}"
        );
    }

    // ── TWCS Throttle ────────────────────────────────────────────────────────

    fn make_twcs_report(throttle: u16, rx: u16, ry: u16, rz: u16, buttons: u16) -> Vec<u8> {
        let mut r = vec![0u8; 10];
        r[0..2].copy_from_slice(&throttle.to_le_bytes());
        r[2..4].copy_from_slice(&rx.to_le_bytes());
        r[4..6].copy_from_slice(&ry.to_le_bytes());
        r[6..8].copy_from_slice(&rz.to_le_bytes());
        r[8..10].copy_from_slice(&buttons.to_le_bytes());
        r
    }

    #[test]
    fn test_twcs_idle_centered() {
        let center: u16 = 32768;
        let report = make_twcs_report(0, center, center, center, 0);
        let state = parse_twcs_report(&report).unwrap();
        assert!(state.axes.throttle < 0.001);
        assert!(state.axes.mini_stick_x.abs() < 0.01);
        assert!(state.axes.mini_stick_y.abs() < 0.01);
        assert!(state.axes.rocker.abs() < 0.01);
    }

    #[test]
    fn test_twcs_full_throttle() {
        let center: u16 = 32768;
        let report = make_twcs_report(u16::MAX, center, center, center, 0);
        let state = parse_twcs_report(&report).unwrap();
        assert!(state.axes.throttle > 0.999);
    }

    #[test]
    fn test_twcs_button_mask() {
        let report = make_twcs_report(0, 32768, 32768, 32768, 0xFFFF);
        let state = parse_twcs_report(&report).unwrap();
        assert_eq!(state.buttons.buttons, 0x3FFF, "only 14 buttons are valid");
    }

    #[test]
    fn test_twcs_too_short() {
        let err = parse_twcs_report(&[0u8; 4]).unwrap_err();
        assert!(
            matches!(err, T16000mParseError::TwcsReportTooShort { .. }),
            "{err:?}"
        );
    }

    // ── Property tests ───────────────────────────────────────────────────────

    mod proptest_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn joystick_axes_within_bounds(
                x in 0u16..=16383,
                y in 0u16..=16383,
                rz in 0u16..=16383,
                slider in 0u16..=u16::MAX,
            ) {
                let report = make_joystick_report(x, y, rz, slider, 0, 0x0F);
                let state = parse_t16000m_report(&report).unwrap();
                prop_assert!(state.axes.x >= -1.0 && state.axes.x <= 1.0);
                prop_assert!(state.axes.y >= -1.0 && state.axes.y <= 1.0);
                prop_assert!(state.axes.twist >= -1.0 && state.axes.twist <= 1.0);
                prop_assert!(state.axes.throttle >= 0.0 && state.axes.throttle <= 1.0);
            }

            #[test]
            fn twcs_axes_within_bounds(
                throttle in 0u16..=u16::MAX,
                rx in 0u16..=u16::MAX,
                ry in 0u16..=u16::MAX,
                rz in 0u16..=u16::MAX,
            ) {
                let report = make_twcs_report(throttle, rx, ry, rz, 0);
                let state = parse_twcs_report(&report).unwrap();
                prop_assert!(state.axes.throttle >= 0.0 && state.axes.throttle <= 1.0);
                prop_assert!(state.axes.mini_stick_x >= -1.0 && state.axes.mini_stick_x <= 1.0);
                prop_assert!(state.axes.mini_stick_y >= -1.0 && state.axes.mini_stick_y <= 1.0);
                prop_assert!(state.axes.rocker >= -1.0 && state.axes.rocker <= 1.0);
            }

            #[test]
            fn any_valid_joystick_report_parses(
                data in proptest::collection::vec(0u8..=255, 11..=20),
            ) {
                // Ensure no panic for arbitrary data of sufficient length.
                let _ = parse_t16000m_report(&data);
            }

            #[test]
            fn any_valid_twcs_report_parses(
                data in proptest::collection::vec(0u8..=255, 10..=20),
            ) {
                let _ = parse_twcs_report(&data);
            }
        }
    }
}
