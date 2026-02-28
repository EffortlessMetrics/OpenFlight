// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the Thrustmaster HOTAS Cougar F-16 HOTAS system.
//!
//! # Confirmed device identifiers
//!
//! - HOTAS Cougar (combined stick + throttle): VID 0x044F, PID 0x0400
//!   Confirmed via linux-hardware.org (USB string "ThrustMaster HOTAS Cougar").
//!
//! The HOTAS Cougar (manufactured 2001–2009) is a faithful replica of the
//! F-16C Block 52 HOTAS. The stick and throttle grip share a single USB
//! connection; there is no separate throttle USB endpoint.
//!
//! # Input report layout (community-documented; HIL validation recommended)
//!
//! ## HOTAS Cougar combined report (10-byte payload, no report ID)
//!
//! | Bytes | Field    | Type   | Range     | Notes                               |
//! |-------|----------|--------|-----------|-------------------------------------|
//! | 0–1   | X        | u16 LE | 0..=65535 | Stick roll; center ~32768           |
//! | 2–3   | Y        | u16 LE | 0..=65535 | Stick pitch; center ~32768          |
//! | 4–5   | Throttle | u16 LE | 0..=65535 | Throttle lever; 0 = idle            |
//! | 6–7   | Buttons  | u16 LE | bitmask   | See [`CougarButtons`] bit table     |
//! | 8     | Hat      | u8     | 0–15      | TMS hat lower nibble; 0xF = center  |
//! | 9     | Switches | u8     | bitmask   | Throttle micro-switches             |
//!
//! **Note:** Byte offsets derived from community analysis of TARGET scripting
//! output and Linux `evtest` captures. Verify with HIL before production use.

use thiserror::Error;

/// USB Product ID for the Thrustmaster HOTAS Cougar (combined stick + throttle).
///
/// Confirmed: VID 0x044F, PID 0x0400 — linux-hardware.org
/// ("ThrustMaster HOTAS Cougar").
pub const COUGAR_STICK_PID: u16 = 0x0400;

/// Minimum HID report payload size for the HOTAS Cougar.
pub const COUGAR_MIN_REPORT_BYTES: usize = 10;

// ─── Hat encoding ────────────────────────────────────────────────────────────

/// F-16 TMS hat switch directions (4-bit encoded).
///
/// Encoding: 0 = N, 1 = NE, 2 = E, 3 = SE, 4 = S, 5 = SW, 6 = W, 7 = NW,
/// ≥ 8 = center (0xF typical). Matches the Warthog hat convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CougarHat {
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

impl CougarHat {
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
            _ => Self::Center,
        }
    }
}

// ─── Axes ────────────────────────────────────────────────────────────────────

/// Parsed axis values from the HOTAS Cougar.
#[derive(Debug, Clone, Default)]
pub struct CougarAxes {
    /// Stick roll (X). −1.0 = full left, 1.0 = full right; center = 0.0.
    pub x: f32,
    /// Stick pitch (Y). −1.0 = full forward/up, 1.0 = full back/down; center = 0.0.
    pub y: f32,
    /// Throttle lever. 0.0 = idle (full aft), 1.0 = full (full forward).
    pub throttle: f32,
}

// ─── Buttons ─────────────────────────────────────────────────────────────────

/// Parsed buttons from the HOTAS Cougar.
///
/// ## Button bit table (bytes 6–7, u16 LE)
///
/// | Bit | F-16 label  | Description                           |
/// |-----|-------------|---------------------------------------|
/// |   0 | TG1         | Trigger first stage                   |
/// |   1 | TG2         | Trigger second stage (fire)           |
/// |   2 | S2 / NWS    | Nose-wheel steering / paddle          |
/// |   3 | S3 / TMS ↓  | Target management switch push (in)    |
/// |   4 | S4 / WPN    | Weapon release                        |
/// |   5 | S5 / Pinky  | Pinky switch (auto-terrain-following) |
/// |   6 | TMS Up      | TMS hat up                            |
/// |   7 | TMS Down    | TMS hat down                          |
/// |   8 | TMS Left    | TMS hat left                          |
/// |   9 | TMS Right   | TMS hat right                         |
/// |  10 | CH Fwd      | China hat forward (mic forward)       |
/// |  11 | CH Back     | China hat back                        |
/// |  12 | BS Fwd      | Boat switch forward                   |
/// |  13 | BS Back     | Boat switch back (aft)                |
/// |  14 | DOGFIGHT    | Dogfight / missile override switch    |
/// |  15 | APD         | Autopilot disconnect                  |
#[derive(Debug, Clone, Default)]
pub struct CougarButtons {
    /// Button bitmask (bits 0-15, see table above).
    pub buttons: u16,
    /// TMS hat direction (from byte 8 lower nibble).
    pub tms_hat: CougarHat,
    /// Throttle micro-switch bitmask (byte 9).
    pub throttle_switches: u8,
}

impl CougarButtons {
    /// Returns `true` if button `n` (1-indexed, 1–16) is pressed.
    #[inline]
    pub fn button(&self, n: u8) -> bool {
        match n {
            1..=16 => (self.buttons >> (n - 1)) & 1 != 0,
            _ => false,
        }
    }
}

// ─── Input state ─────────────────────────────────────────────────────────────

/// Full parsed input state from one HOTAS Cougar HID report.
#[derive(Debug, Clone, Default)]
pub struct CougarInputState {
    pub axes: CougarAxes,
    pub buttons: CougarButtons,
}

// ─── Errors ──────────────────────────────────────────────────────────────────

/// Errors returned by HOTAS Cougar report parsing.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CougarParseError {
    #[error("HOTAS Cougar report too short: expected at least {expected} bytes, got {actual}")]
    TooShort { expected: usize, actual: usize },
}

// ─── Normalization helpers ────────────────────────────────────────────────────

/// Normalize a centred u16 axis (0..65535) to −1.0..=1.0.
#[inline]
fn normalize_bipolar(raw: u16) -> f32 {
    ((raw as f32 - 32767.5) / 32767.5).clamp(-1.0, 1.0)
}

/// Normalize a unipolar u16 axis (0..65535) to 0.0..=1.0.
#[inline]
fn normalize_unipolar(raw: u16) -> f32 {
    (raw as f32 / 65535.0).clamp(0.0, 1.0)
}

// ─── Parser ──────────────────────────────────────────────────────────────────

/// Parse a raw HID report from the HOTAS Cougar.
///
/// `data` must be at least [`COUGAR_MIN_REPORT_BYTES`] bytes long. Strip any
/// 1-byte report ID prefix before calling.
///
/// # Errors
///
/// Returns [`CougarParseError::TooShort`] when `data.len() < COUGAR_MIN_REPORT_BYTES`.
pub fn parse_cougar(data: &[u8]) -> Result<CougarInputState, CougarParseError> {
    if data.len() < COUGAR_MIN_REPORT_BYTES {
        return Err(CougarParseError::TooShort {
            expected: COUGAR_MIN_REPORT_BYTES,
            actual: data.len(),
        });
    }

    let x = u16::from_le_bytes([data[0], data[1]]);
    let y = u16::from_le_bytes([data[2], data[3]]);
    let throttle = u16::from_le_bytes([data[4], data[5]]);
    let buttons = u16::from_le_bytes([data[6], data[7]]);
    let tms_hat = CougarHat::from_nibble(data[8]);
    let throttle_switches = data[9];

    Ok(CougarInputState {
        axes: CougarAxes {
            x: normalize_bipolar(x),
            y: normalize_bipolar(y),
            throttle: normalize_unipolar(throttle),
        },
        buttons: CougarButtons {
            buttons,
            tms_hat,
            throttle_switches,
        },
    })
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_report(
        x: u16,
        y: u16,
        throttle: u16,
        buttons: u16,
        tms_hat: u8,
        switches: u8,
    ) -> Vec<u8> {
        let mut buf = vec![0u8; 10];
        buf[0..2].copy_from_slice(&x.to_le_bytes());
        buf[2..4].copy_from_slice(&y.to_le_bytes());
        buf[4..6].copy_from_slice(&throttle.to_le_bytes());
        buf[6..8].copy_from_slice(&buttons.to_le_bytes());
        buf[8] = tms_hat;
        buf[9] = switches;
        buf
    }

    #[test]
    fn test_too_short_returns_error() {
        assert!(parse_cougar(&[0u8; 9]).is_err());
    }

    #[test]
    fn test_empty_returns_error() {
        let err = parse_cougar(&[]).unwrap_err();
        assert_eq!(
            err,
            CougarParseError::TooShort {
                expected: COUGAR_MIN_REPORT_BYTES,
                actual: 0,
            }
        );
    }

    #[test]
    fn test_centered_axes_near_zero() {
        let data = make_report(32768, 32768, 0, 0, 0xFF, 0);
        let state = parse_cougar(&data).unwrap();
        assert!(state.axes.x.abs() < 0.01, "x near 0: {}", state.axes.x);
        assert!(state.axes.y.abs() < 0.01, "y near 0: {}", state.axes.y);
    }

    #[test]
    fn test_throttle_idle() {
        let data = make_report(32768, 32768, 0, 0, 0xFF, 0);
        let state = parse_cougar(&data).unwrap();
        assert!(
            state.axes.throttle < 0.001,
            "throttle idle: {}",
            state.axes.throttle
        );
    }

    #[test]
    fn test_throttle_full() {
        let data = make_report(32768, 32768, 65535, 0, 0xFF, 0);
        let state = parse_cougar(&data).unwrap();
        assert!(
            state.axes.throttle > 0.999,
            "throttle full: {}",
            state.axes.throttle
        );
    }

    #[test]
    fn test_stick_full_right() {
        let data = make_report(65535, 32768, 0, 0, 0xFF, 0);
        let state = parse_cougar(&data).unwrap();
        assert!(state.axes.x > 0.99, "x should be ~1.0: {}", state.axes.x);
    }

    #[test]
    fn test_stick_full_left() {
        let data = make_report(0, 32768, 0, 0, 0xFF, 0);
        let state = parse_cougar(&data).unwrap();
        assert!(state.axes.x < -0.99, "x should be ~-1.0: {}", state.axes.x);
    }

    #[test]
    fn test_stick_full_back() {
        let data = make_report(32768, 65535, 0, 0, 0xFF, 0);
        let state = parse_cougar(&data).unwrap();
        assert!(state.axes.y > 0.99, "y should be ~1.0: {}", state.axes.y);
    }

    #[test]
    fn test_stick_full_forward() {
        let data = make_report(32768, 0, 0, 0, 0xFF, 0);
        let state = parse_cougar(&data).unwrap();
        assert!(state.axes.y < -0.99, "y should be ~-1.0: {}", state.axes.y);
    }

    #[test]
    fn test_tms_hat_center() {
        let data = make_report(32768, 32768, 0, 0, 0x0F, 0);
        let state = parse_cougar(&data).unwrap();
        assert_eq!(state.buttons.tms_hat, CougarHat::Center);
    }

    #[test]
    fn test_tms_hat_north() {
        let data = make_report(32768, 32768, 0, 0, 0x00, 0);
        let state = parse_cougar(&data).unwrap();
        assert_eq!(state.buttons.tms_hat, CougarHat::North);
    }

    #[test]
    fn test_tms_hat_east() {
        let data = make_report(32768, 32768, 0, 0, 0x02, 0);
        let state = parse_cougar(&data).unwrap();
        assert_eq!(state.buttons.tms_hat, CougarHat::East);
    }

    #[test]
    fn test_tms_hat_south() {
        let data = make_report(32768, 32768, 0, 0, 0x04, 0);
        let state = parse_cougar(&data).unwrap();
        assert_eq!(state.buttons.tms_hat, CougarHat::South);
    }

    #[test]
    fn test_tms_hat_west() {
        let data = make_report(32768, 32768, 0, 0, 0x06, 0);
        let state = parse_cougar(&data).unwrap();
        assert_eq!(state.buttons.tms_hat, CougarHat::West);
    }

    #[test]
    fn test_trigger_tg1_pressed() {
        let data = make_report(32768, 32768, 0, 0x0001, 0xFF, 0);
        let state = parse_cougar(&data).unwrap();
        assert!(state.buttons.button(1), "TG1 should be pressed");
        assert!(!state.buttons.button(2), "TG2 should not be pressed");
    }

    #[test]
    fn test_trigger_tg2_pressed() {
        let data = make_report(32768, 32768, 0, 0x0002, 0xFF, 0);
        let state = parse_cougar(&data).unwrap();
        assert!(!state.buttons.button(1), "TG1 should not be pressed");
        assert!(state.buttons.button(2), "TG2 should be pressed");
    }

    #[test]
    fn test_pinky_switch() {
        // Bit 5 = S5/Pinky
        let data = make_report(32768, 32768, 0, 1 << 5, 0xFF, 0);
        let state = parse_cougar(&data).unwrap();
        assert!(state.buttons.button(6), "pinky (bit 5 = button 6)");
    }

    #[test]
    fn test_dogfight_switch() {
        // Bit 14 = DOGFIGHT
        let data = make_report(32768, 32768, 0, 1 << 14, 0xFF, 0);
        let state = parse_cougar(&data).unwrap();
        assert!(state.buttons.button(15), "DOGFIGHT (bit 14 = button 15)");
    }

    #[test]
    fn test_apd_button() {
        // Bit 15 = APD
        let data = make_report(32768, 32768, 0, 1 << 15, 0xFF, 0);
        let state = parse_cougar(&data).unwrap();
        assert!(state.buttons.button(16), "APD (bit 15 = button 16)");
    }

    #[test]
    fn test_button_out_of_range_returns_false() {
        let data = make_report(32768, 32768, 0, 0xFFFF, 0xFF, 0xFF);
        let state = parse_cougar(&data).unwrap();
        assert!(!state.buttons.button(0), "button(0) is out of range");
        assert!(!state.buttons.button(17), "button(17) is out of range");
    }

    #[test]
    fn test_button_decode_consistency() {
        let raw_buttons: u16 = 0b1010_1010_0101_0101;
        let data = make_report(32768, 32768, 0, raw_buttons, 0xFF, 0);
        let state = parse_cougar(&data).unwrap();
        for n in 1u8..=16 {
            let expected = (raw_buttons >> (n - 1)) & 1 != 0;
            assert_eq!(state.buttons.button(n), expected, "button({n}) mismatch");
        }
    }

    #[test]
    fn test_throttle_switches_preserved() {
        let data = make_report(32768, 32768, 0, 0, 0xFF, 0b1100_0011);
        let state = parse_cougar(&data).unwrap();
        assert_eq!(state.buttons.throttle_switches, 0b1100_0011);
    }

    #[test]
    fn test_axes_within_range_for_various_inputs() {
        for raw in [0u16, 1, 16383, 32767, 32768, 49151, 65534, 65535] {
            let data = make_report(raw, raw, raw, 0, 0xFF, 0);
            let state = parse_cougar(&data).unwrap();
            assert!(
                (-1.0..=1.0).contains(&state.axes.x),
                "x out of range for raw={raw}: {}",
                state.axes.x
            );
            assert!(
                (-1.0..=1.0).contains(&state.axes.y),
                "y out of range for raw={raw}: {}",
                state.axes.y
            );
            assert!(
                (0.0..=1.0).contains(&state.axes.throttle),
                "throttle out of range for raw={raw}: {}",
                state.axes.throttle
            );
        }
    }

    #[test]
    fn test_oversized_report_parses_ok() {
        let mut data = make_report(32768, 32768, 32767, 0, 0xFF, 0);
        data.extend_from_slice(&[0xFF; 10]); // extra bytes
        assert!(parse_cougar(&data).is_ok());
    }

    #[cfg(test)]
    mod proptest_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn stick_axes_always_in_range(x in 0u16..=65535, y in 0u16..=65535) {
                let data = make_report(x, y, 0, 0, 0xFF, 0);
                let state = parse_cougar(&data).unwrap();
                prop_assert!((-1.0..=1.0).contains(&state.axes.x));
                prop_assert!((-1.0..=1.0).contains(&state.axes.y));
            }

            #[test]
            fn throttle_unipolar_in_range(thr in 0u16..=65535) {
                let data = make_report(32768, 32768, thr, 0, 0xFF, 0);
                let state = parse_cougar(&data).unwrap();
                prop_assert!((0.0..=1.0).contains(&state.axes.throttle));
            }

            #[test]
            fn short_report_always_errors(
                data in proptest::collection::vec(any::<u8>(), 0..COUGAR_MIN_REPORT_BYTES)
            ) {
                prop_assert!(parse_cougar(&data).is_err());
            }

            #[test]
            fn valid_report_always_parses(
                data in proptest::collection::vec(any::<u8>(), COUGAR_MIN_REPORT_BYTES..64)
            ) {
                prop_assert!(parse_cougar(&data).is_ok());
            }

            #[test]
            fn buttons_decode_consistently(raw in any::<u16>()) {
                let data = make_report(32768, 32768, 0, raw, 0xFF, 0);
                let state = parse_cougar(&data).unwrap();
                for n in 1u8..=16 {
                    let expected = (raw >> (n - 1)) & 1 != 0;
                    prop_assert_eq!(
                        state.buttons.button(n), expected,
                        "button({}) mismatch for bitmask {:018b}", n, raw
                    );
                }
            }
        }
    }
}
