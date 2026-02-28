// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the Honeycomb Alpha Flight Controls XPC (Yoke).
//!
//! # Report layout (estimated)
//!
//! The exact HID descriptor for the Alpha Yoke is not publicly documented.
//! This layout is inferred from the HID joystick specification and community
//! reports. **Hardware validation required before production use.**
//!
//! ```text
//! Byte 0:       Report ID = 0x01
//! Bytes 1–2:    Roll (X)  — u16 LE, logical range 0–4095, centre = 2048
//! Bytes 3–4:    Pitch (Y) — u16 LE, logical range 0–4095, centre = 2048
//! Bytes 5–9:    Button bitmask (40 bits; buttons 1–36 used, bits 37–40 padding)
//! Byte 10:      Hat switch — lower nibble 0–15
//!                 0 = centred, 1 = N, 3 = NE, 5 = E, 7 = SE,
//!                 9 = S, 11 = SW, 13 = W, 15 = NW
//! ```
//!
//! Axis resolution: 12-bit (0–4095), stored in 16-bit LE fields.
//! Alpha Yoke VID: 0x294B  PID: 0x0102 (community-reported, unverified)

/// Expected minimum report length in bytes.
pub const ALPHA_REPORT_LEN: usize = 11;

/// Axis values for the Alpha Yoke, normalised to standard ranges.
#[derive(Debug, Clone, PartialEq)]
pub struct AlphaAxes {
    /// Roll (X axis) — \[−1.0, +1.0\]; left = negative, right = positive.
    pub roll: f32,
    /// Pitch (Y axis) — \[−1.0, +1.0\]; forward = negative, back = positive.
    pub pitch: f32,
}

/// Button state for the Alpha Yoke.
///
/// Buttons 1–36 are packed into the low 36 bits of the mask.
/// The hat switch is stored separately.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AlphaButtons {
    /// 64-bit bitmask; bit (n−1) corresponds to button n (1-indexed).
    pub mask: u64,
    /// Hat switch position: 0 = centred, 1–8 = N/NE/E/SE/S/SW/W/NW.
    /// Raw device value 0–15 is mapped to 0–8 (odd values from device become
    /// cardinal/diagonal directions).
    pub hat: u8,
}

impl AlphaButtons {
    /// Returns `true` if button `n` (1-based) is currently pressed.
    pub fn is_pressed(&self, n: u8) -> bool {
        (1u8..=36).contains(&n) && (self.mask >> (n - 1)) & 1 == 1
    }

    /// Returns the hat position as a compass direction string.
    pub fn hat_direction(&self) -> &'static str {
        match self.hat {
            0 => "center",
            1 => "N",
            2 => "NE",
            3 => "E",
            4 => "SE",
            5 => "S",
            6 => "SW",
            7 => "W",
            8 => "NW",
            _ => "unknown",
        }
    }
}

/// Parsed state from a single Alpha Yoke HID input report.
#[derive(Debug, Clone)]
pub struct AlphaInputState {
    pub axes: AlphaAxes,
    pub buttons: AlphaButtons,
}

/// Parse a raw HID input report from the Alpha Yoke.
///
/// # Errors
///
/// Returns [`AlphaParseError`] if the report is too short or has an unexpected
/// report ID byte.
///
/// # Layout assumption
///
/// See module documentation for the assumed report layout. This parser has not
/// been validated against real hardware. If axis values appear inverted or
/// offset, the `centre` and `max` constants below may need adjustment.
pub fn parse_alpha_report(data: &[u8]) -> Result<AlphaInputState, AlphaParseError> {
    if data.len() < ALPHA_REPORT_LEN {
        return Err(AlphaParseError::TooShort {
            expected: ALPHA_REPORT_LEN,
            got: data.len(),
        });
    }
    if data[0] != 0x01 {
        return Err(AlphaParseError::UnknownReportId { id: data[0] });
    }

    let roll_raw = u16::from_le_bytes([data[1], data[2]]);
    let pitch_raw = u16::from_le_bytes([data[3], data[4]]);

    let roll = norm_12bit_centered(roll_raw);
    let pitch = norm_12bit_centered(pitch_raw);

    // Buttons are in bytes 5–9 (40 bits, buttons 1–36 + 4 padding bits)
    let mask = u64::from(data[5])
        | (u64::from(data[6]) << 8)
        | (u64::from(data[7]) << 16)
        | (u64::from(data[8]) << 24)
        | (u64::from(data[9]) << 32);

    // Hat: lower nibble of byte 10 (raw 0–15, odd = cardinal/diagonal)
    let hat_raw = (data[10] & 0x0F) as u16;
    let hat = hat_raw_to_8way(hat_raw);

    Ok(AlphaInputState {
        axes: AlphaAxes { roll, pitch },
        buttons: AlphaButtons { mask, hat },
    })
}

/// Errors returned by [`parse_alpha_report`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AlphaParseError {
    #[error("report too short: expected ≥{expected} bytes, got {got}")]
    TooShort { expected: usize, got: usize },
    #[error("unknown report ID: 0x{id:02X}")]
    UnknownReportId { id: u8 },
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Normalise a 12-bit unsigned value centred at 2048 to \[−1.0, +1.0\].
fn norm_12bit_centered(raw: u16) -> f32 {
    let raw = raw.min(4095);
    ((raw as f32 - 2048.0) / 2048.0).clamp(-1.0, 1.0)
}

/// Convert a 4-bit hat raw value (0–15) to an 8-way direction.
///
/// Standard HID hat encoding: 0 = N, 2 = E, 4 = S, 6 = W, 1/3/5/7 = diagonals.
/// 0xF (15) = centred (mapped to 0). Values 8–14 are also treated as centred.
fn hat_raw_to_8way(raw: u16) -> u8 {
    match raw {
        0 => 1, // N
        1 => 2, // NE
        2 => 3, // E
        3 => 4, // SE
        4 => 5, // S
        5 => 6, // SW
        6 => 7, // W
        7 => 8, // NW
        _ => 0, // centred (includes 8–15)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha_report(roll: u16, pitch: u16, buttons: u64, hat: u8) -> [u8; ALPHA_REPORT_LEN] {
        let mut r = [0u8; ALPHA_REPORT_LEN];
        r[0] = 0x01;
        r[1..3].copy_from_slice(&roll.to_le_bytes());
        r[3..5].copy_from_slice(&pitch.to_le_bytes());
        r[5] = (buttons & 0xFF) as u8;
        r[6] = ((buttons >> 8) & 0xFF) as u8;
        r[7] = ((buttons >> 16) & 0xFF) as u8;
        r[8] = ((buttons >> 24) & 0xFF) as u8;
        r[9] = ((buttons >> 32) & 0xFF) as u8;
        r[10] = hat & 0x0F;
        r
    }

    #[test]
    fn test_neutral_position() {
        let state = parse_alpha_report(&alpha_report(2048, 2048, 0, 15)).unwrap();
        // Centre: (2048 - 2048) / 2047 = 0.0
        assert!(state.axes.roll.abs() < 1e-4, "roll should be near 0");
        assert!(state.axes.pitch.abs() < 1e-4, "pitch should be near 0");
        assert_eq!(state.buttons.hat, 0, "hat should be centred");
    }

    #[test]
    fn test_full_roll_right() {
        let state = parse_alpha_report(&alpha_report(4095, 2048, 0, 15)).unwrap();
        // (4095 - 2048) / 2047 ≈ 0.9995
        assert!(state.axes.roll > 0.99, "full right roll should be ~1.0");
    }

    #[test]
    fn test_full_roll_left() {
        let state = parse_alpha_report(&alpha_report(0, 2048, 0, 15)).unwrap();
        // (0 - 2048) / 2047 ≈ -1.0004 → clamped representation
        assert!(state.axes.roll < -0.99, "full left roll should be ~-1.0");
    }

    #[test]
    fn test_full_pitch_forward() {
        let state = parse_alpha_report(&alpha_report(2048, 0, 0, 15)).unwrap();
        assert!(
            state.axes.pitch < -0.99,
            "full forward pitch should be ~-1.0"
        );
    }

    #[test]
    fn test_button_detection() {
        // Button 1 = bit 0, button 36 = bit 35
        let mask: u64 = (1u64 << 0) | (1u64 << 35);
        let state = parse_alpha_report(&alpha_report(2048, 2048, mask, 15)).unwrap();
        assert!(state.buttons.is_pressed(1), "button 1 should be pressed");
        assert!(
            !state.buttons.is_pressed(2),
            "button 2 should not be pressed"
        );
        assert!(state.buttons.is_pressed(36), "button 36 should be pressed");
    }

    #[test]
    fn test_hat_north() {
        let state = parse_alpha_report(&alpha_report(2048, 2048, 0, 0)).unwrap();
        assert_eq!(state.buttons.hat, 1); // N
        assert_eq!(state.buttons.hat_direction(), "N");
    }

    #[test]
    fn test_hat_centred() {
        let state = parse_alpha_report(&alpha_report(2048, 2048, 0, 15)).unwrap();
        assert_eq!(state.buttons.hat, 0); // centred
        assert_eq!(state.buttons.hat_direction(), "center");
    }

    #[test]
    fn test_report_too_short() {
        let err = parse_alpha_report(&[0x01, 0x00, 0x00]).unwrap_err();
        assert!(matches!(err, AlphaParseError::TooShort { .. }));
    }

    #[test]
    fn test_unknown_report_id() {
        let mut r = [0u8; ALPHA_REPORT_LEN];
        r[0] = 0x02;
        assert!(matches!(
            parse_alpha_report(&r),
            Err(AlphaParseError::UnknownReportId { id: 0x02 })
        ));
    }

    #[cfg(test)]
    mod proptest_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn roll_within_bounds(raw in 0u16..=4095u16) {
                let state = parse_alpha_report(&super::alpha_report(raw, 2048, 0, 15)).unwrap();
                prop_assert!(state.axes.roll >= -1.001 && state.axes.roll <= 1.001,
                    "roll out of range: {}", state.axes.roll);
            }

            #[test]
            fn pitch_within_bounds(raw in 0u16..=4095u16) {
                let state = parse_alpha_report(&super::alpha_report(2048, raw, 0, 15)).unwrap();
                prop_assert!(state.axes.pitch >= -1.001 && state.axes.pitch <= 1.001,
                    "pitch out of range: {}", state.axes.pitch);
            }

            #[test]
            fn any_valid_report_parses(
                roll in 0u16..=4095u16,
                pitch in 0u16..=4095u16,
                buttons in 0u64..u64::MAX,
                hat in 0u8..16u8,
            ) {
                let r = super::alpha_report(roll, pitch, buttons, hat);
                let result = parse_alpha_report(&r);
                prop_assert!(result.is_ok());
            }
        }
    }
}
