// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the Thrustmaster HOTAS Warthog joystick and throttle.
//!
//! # Confirmed device identifiers
//!
//! - HOTAS Warthog Joystick: VID 0x044F, PID 0x0402 (confirmed via linux-hardware.org)
//! - HOTAS Warthog Throttle: VID 0x044F, PID 0x0404 (confirmed via linux-hardware.org)
//!
//! # Input report layouts (community-documented; HIL validation recommended)
//!
//! ## Warthog Joystick (10-byte payload, no report ID)
//!
//! | Bytes | Field   | Type   | Range      | Notes                                   |
//! |-------|---------|--------|------------|----------------------------------------|
//! | 0-1   | X       | u16 LE | 0..=65535  | Roll/stick horizontal; center ~32768    |
//! | 2-3   | Y       | u16 LE | 0..=65535  | Pitch/stick vertical; center ~32768     |
//! | 4-5   | RZ      | u16 LE | 0..=65535  | Twist/rudder; center ~32768             |
//! | 6-7   | Buttons | u16 LE | bitmask    | Bits 0-15 → buttons 1-16               |
//! | 8     | Buttons | u8     | bitmask    | Bits 0-2 → buttons 17-19               |
//! | 9     | Hat     | u8     | 0-15       | Upper nibble: 0xF=center, 0=N, 2=E...  |
//!
//! ## Warthog Throttle (20-byte payload, no report ID)
//!
//! | Bytes | Field         | Type   | Range      | Notes                               |
//! |-------|---------------|--------|------------|-------------------------------------|
//! | 0-1   | SCX           | u16 LE | 0..=65535  | Slew X (mini-stick); center ~32768  |
//! | 2-3   | SCY           | u16 LE | 0..=65535  | Slew Y (mini-stick); center ~32768  |
//! | 4-5   | Left throttle | u16 LE | 0..=65535  | Left throttle lever; 0=idle         |
//! | 6-7   | Right throttle| u16 LE | 0..=65535  | Right throttle lever; 0=idle        |
//! | 8-9   | Combined      | u16 LE | 0..=65535  | Combined/interlock throttle lever   |
//! | 10-11 | Buttons 1-16  | u16 LE | bitmask    | Bits 0-15 → buttons 1-16           |
//! | 12-13 | Buttons 17-32 | u16 LE | bitmask    | Bits 0-15 → buttons 17-32          |
//! | 14    | Buttons 33-40 | u8     | bitmask    | Bits 0-7 → buttons 33-40           |
//! | 15    | Toggle sw     | u8     | bitmask    | Physical toggle switch states       |
//! | 16    | Hat1 (DMS)    | u8     | 0-15       | Lower nibble: 0xF=center, 0=N...    |
//! | 17    | Hat2 (CSL)    | u8     | 0-15       | Lower nibble: 0xF=center, 0=N...    |
//! | 18-19 | Reserved      | u16    | -          | Unused/padding                      |
//!
//! **Note:** The exact byte offsets for the throttle have not been verified on hardware.
//! The mapping above reflects community analysis of TARGET scripting output and
//! Linux `evtest` captures. Verify with HIL before production use.

use thiserror::Error;

/// Warthog hat switch directions (4-bit encoded, used by both stick and throttle hats).
///
/// Encoding: 0=N, 1=NE, 2=E, 3=SE, 4=S, 5=SW, 6=W, 7=NW, ≥8=center (0xF typical).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WarthogHat {
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

impl WarthogHat {
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

/// Parsed axis values from the Warthog Joystick, normalized to −1.0..=1.0.
#[derive(Debug, Clone, Default)]
pub struct WarthogStickAxes {
    /// Stick horizontal (X / roll). −1.0 = full left, 1.0 = full right.
    pub x: f32,
    /// Stick vertical (Y / pitch). −1.0 = full forward/up, 1.0 = full back/down.
    pub y: f32,
    /// Twist handle (RZ / rudder). −1.0 = full left, 1.0 = full right; center = 0.0.
    pub rz: f32,
}

/// Parsed buttons from the Warthog Joystick.
#[derive(Debug, Clone, Default)]
pub struct WarthogStickButtons {
    /// Button bitmask, bits 0-15 → buttons 1-16.
    pub buttons_low: u16,
    /// Button bitmask, bits 0-2 → buttons 17-19 (upper bits unused).
    pub buttons_high: u8,
    /// TDC/DMS hat direction.
    pub hat: WarthogHat,
}

impl WarthogStickButtons {
    /// Returns `true` if the specified button (1-indexed, 1-19) is pressed.
    pub fn button(&self, n: u8) -> bool {
        match n {
            1..=16 => (self.buttons_low >> (n - 1)) & 1 != 0,
            17..=19 => (self.buttons_high >> (n - 17)) & 1 != 0,
            _ => false,
        }
    }
}

/// Full parsed input state from a Warthog Joystick HID report.
#[derive(Debug, Clone, Default)]
pub struct WarthogStickInputState {
    pub axes: WarthogStickAxes,
    pub buttons: WarthogStickButtons,
}

/// Parsed axis values from the Warthog Throttle, normalized.
#[derive(Debug, Clone, Default)]
pub struct WarthogThrottleAxes {
    /// Slew control X (SCX mini-stick). −1.0..=1.0; center = 0.0.
    pub slew_x: f32,
    /// Slew control Y (SCY mini-stick). −1.0..=1.0; center = 0.0.
    pub slew_y: f32,
    /// Left throttle lever. 0.0 = idle, 1.0 = full.
    pub throttle_left: f32,
    /// Right throttle lever. 0.0 = idle, 1.0 = full.
    pub throttle_right: f32,
    /// Combined / interlock position (reported when throttles are locked together).
    /// 0.0 = idle, 1.0 = full.
    pub throttle_combined: f32,
}

/// Parsed buttons from the Warthog Throttle (40 buttons + 2 hats + toggle switches).
#[derive(Debug, Clone, Default)]
pub struct WarthogThrottleButtons {
    /// Buttons 1-16 bitmask (bits 0-15 → buttons 1-16).
    pub buttons_low: u16,
    /// Buttons 17-32 bitmask (bits 0-15 → buttons 17-32).
    pub buttons_mid: u16,
    /// Buttons 33-40 bitmask (bits 0-7 → buttons 33-40).
    pub buttons_high: u8,
    /// Physical toggle switch state bitmask (FLNORM, FLAUT, SPNORM, etc.).
    pub toggles: u8,
    /// DMS hat direction.
    pub hat_dms: WarthogHat,
    /// CSL hat direction.
    pub hat_csl: WarthogHat,
}

impl WarthogThrottleButtons {
    /// Returns `true` if the specified button (1-indexed, 1-40) is pressed.
    pub fn button(&self, n: u8) -> bool {
        match n {
            1..=16 => (self.buttons_low >> (n - 1)) & 1 != 0,
            17..=32 => (self.buttons_mid >> (n - 17)) & 1 != 0,
            33..=40 => (self.buttons_high >> (n - 33)) & 1 != 0,
            _ => false,
        }
    }
}

/// Full parsed input state from a Warthog Throttle HID report.
#[derive(Debug, Clone, Default)]
pub struct WarthogThrottleInputState {
    pub axes: WarthogThrottleAxes,
    pub buttons: WarthogThrottleButtons,
}

/// Errors returned by Warthog report parsing.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WarthogParseError {
    #[error("Warthog {device} report too short: expected at least {expected} bytes, got {actual}")]
    TooShort {
        device: &'static str,
        expected: usize,
        actual: usize,
    },
}

/// Minimum report payload length for the Warthog Joystick.
pub const WARTHOG_STICK_MIN_REPORT_BYTES: usize = 10;

/// Minimum report payload length for the Warthog Throttle.
pub const WARTHOG_THROTTLE_MIN_REPORT_BYTES: usize = 18;

/// Normalize a centered u16 axis (0..65535) to −1.0..=1.0.
#[inline]
fn normalize_bipolar(raw: u16) -> f32 {
    ((raw as f32 - 32767.5) / 32767.5).clamp(-1.0, 1.0)
}

/// Normalize a unipolar u16 axis (0..65535) to 0.0..=1.0.
#[inline]
fn normalize_unipolar(raw: u16) -> f32 {
    (raw as f32 / 65535.0).clamp(0.0, 1.0)
}

/// Parse a HID input report from the Warthog Joystick.
///
/// The report must be exactly 10 bytes (no report ID prefix). If the device
/// is known to prepend a 1-byte report ID, strip it before calling this
/// function.
///
/// # Errors
/// Returns [`WarthogParseError::TooShort`] if `data` is shorter than
/// [`WARTHOG_STICK_MIN_REPORT_BYTES`].
pub fn parse_warthog_stick(data: &[u8]) -> Result<WarthogStickInputState, WarthogParseError> {
    if data.len() < WARTHOG_STICK_MIN_REPORT_BYTES {
        return Err(WarthogParseError::TooShort {
            device: "Joystick",
            expected: WARTHOG_STICK_MIN_REPORT_BYTES,
            actual: data.len(),
        });
    }

    let x = u16::from_le_bytes([data[0], data[1]]);
    let y = u16::from_le_bytes([data[2], data[3]]);
    let rz = u16::from_le_bytes([data[4], data[5]]);
    let buttons_low = u16::from_le_bytes([data[6], data[7]]);
    let buttons_high = data[8] & 0x07; // bits 0-2 only
    let hat = WarthogHat::from_nibble(data[9] >> 4); // upper nibble

    Ok(WarthogStickInputState {
        axes: WarthogStickAxes {
            x: normalize_bipolar(x),
            y: normalize_bipolar(y),
            rz: normalize_bipolar(rz),
        },
        buttons: WarthogStickButtons {
            buttons_low,
            buttons_high,
            hat,
        },
    })
}

/// Parse a HID input report from the Warthog Throttle.
///
/// The report must be at least 18 bytes (no report ID prefix). If the device
/// is known to prepend a 1-byte report ID, strip it before calling this
/// function.
///
/// # Errors
/// Returns [`WarthogParseError::TooShort`] if `data` is shorter than
/// [`WARTHOG_THROTTLE_MIN_REPORT_BYTES`].
pub fn parse_warthog_throttle(data: &[u8]) -> Result<WarthogThrottleInputState, WarthogParseError> {
    if data.len() < WARTHOG_THROTTLE_MIN_REPORT_BYTES {
        return Err(WarthogParseError::TooShort {
            device: "Throttle",
            expected: WARTHOG_THROTTLE_MIN_REPORT_BYTES,
            actual: data.len(),
        });
    }

    let scx = u16::from_le_bytes([data[0], data[1]]);
    let scy = u16::from_le_bytes([data[2], data[3]]);
    let throttle_left = u16::from_le_bytes([data[4], data[5]]);
    let throttle_right = u16::from_le_bytes([data[6], data[7]]);
    let throttle_combined = u16::from_le_bytes([data[8], data[9]]);
    let buttons_low = u16::from_le_bytes([data[10], data[11]]);
    let buttons_mid = u16::from_le_bytes([data[12], data[13]]);
    let buttons_high = data[14];
    let toggles = data[15];
    let hat_dms = WarthogHat::from_nibble(data[16] & 0x0F);
    let hat_csl = WarthogHat::from_nibble(data[17] & 0x0F);

    Ok(WarthogThrottleInputState {
        axes: WarthogThrottleAxes {
            slew_x: normalize_bipolar(scx),
            slew_y: normalize_bipolar(scy),
            throttle_left: normalize_unipolar(throttle_left),
            throttle_right: normalize_unipolar(throttle_right),
            throttle_combined: normalize_unipolar(throttle_combined),
        },
        buttons: WarthogThrottleButtons {
            buttons_low,
            buttons_mid,
            buttons_high,
            toggles,
            hat_dms,
            hat_csl,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stick_report(x: u16, y: u16, rz: u16, btn_low: u16, btn_high: u8, hat: u8) -> Vec<u8> {
        let mut buf = vec![0u8; 10];
        buf[0..2].copy_from_slice(&x.to_le_bytes());
        buf[2..4].copy_from_slice(&y.to_le_bytes());
        buf[4..6].copy_from_slice(&rz.to_le_bytes());
        buf[6..8].copy_from_slice(&btn_low.to_le_bytes());
        buf[8] = btn_high;
        buf[9] = hat;
        buf
    }

    #[allow(clippy::too_many_arguments)]
    fn throttle_report(
        scx: u16,
        scy: u16,
        tl: u16,
        tr: u16,
        tc: u16,
        btn_low: u16,
        btn_mid: u16,
        btn_high: u8,
        toggles: u8,
        hat_dms: u8,
        hat_csl: u8,
    ) -> Vec<u8> {
        let mut buf = vec![0u8; 20];
        buf[0..2].copy_from_slice(&scx.to_le_bytes());
        buf[2..4].copy_from_slice(&scy.to_le_bytes());
        buf[4..6].copy_from_slice(&tl.to_le_bytes());
        buf[6..8].copy_from_slice(&tr.to_le_bytes());
        buf[8..10].copy_from_slice(&tc.to_le_bytes());
        buf[10..12].copy_from_slice(&btn_low.to_le_bytes());
        buf[12..14].copy_from_slice(&btn_mid.to_le_bytes());
        buf[14] = btn_high;
        buf[15] = toggles;
        buf[16] = hat_dms;
        buf[17] = hat_csl;
        buf
    }

    // ─── Stick tests ────────────────────────────────────────────────────────

    #[test]
    fn test_stick_too_short() {
        assert!(parse_warthog_stick(&[0u8; 9]).is_err());
        assert!(parse_warthog_stick(&[]).is_err());
    }

    #[test]
    fn test_stick_centered() {
        let data = stick_report(32768, 32768, 32768, 0, 0, 0xFF);
        let state = parse_warthog_stick(&data).unwrap();
        assert!(state.axes.x.abs() < 0.01, "x near 0: {}", state.axes.x);
        assert!(state.axes.y.abs() < 0.01, "y near 0: {}", state.axes.y);
        assert!(state.axes.rz.abs() < 0.01, "rz near 0: {}", state.axes.rz);
        assert_eq!(state.buttons.hat, WarthogHat::Center);
        assert_eq!(state.buttons.buttons_low, 0);
    }

    #[test]
    fn test_stick_full_right() {
        let data = stick_report(65535, 32768, 32768, 0, 0, 0xFF);
        let state = parse_warthog_stick(&data).unwrap();
        assert!(state.axes.x > 0.99, "x should be ~1.0: {}", state.axes.x);
    }

    #[test]
    fn test_stick_full_left() {
        let data = stick_report(0, 32768, 32768, 0, 0, 0xFF);
        let state = parse_warthog_stick(&data).unwrap();
        assert!(state.axes.x < -0.99, "x should be ~-1.0: {}", state.axes.x);
    }

    #[test]
    fn test_stick_hat_north() {
        let data = stick_report(32768, 32768, 32768, 0, 0, 0x00); // upper nibble 0 = North
        let state = parse_warthog_stick(&data).unwrap();
        assert_eq!(state.buttons.hat, WarthogHat::North);
    }

    #[test]
    fn test_stick_hat_east() {
        let data = stick_report(32768, 32768, 32768, 0, 0, 0x20); // upper nibble 2 = East
        let state = parse_warthog_stick(&data).unwrap();
        assert_eq!(state.buttons.hat, WarthogHat::East);
    }

    #[test]
    fn test_stick_buttons() {
        let data = stick_report(32768, 32768, 32768, 0x0003, 0x04, 0xFF);
        let state = parse_warthog_stick(&data).unwrap();
        assert!(state.buttons.button(1), "button 1");
        assert!(state.buttons.button(2), "button 2");
        assert!(!state.buttons.button(3), "button 3 not pressed");
        assert!(state.buttons.button(19), "button 19");
        assert!(!state.buttons.button(20), "button 20 out of range");
    }

    #[test]
    fn test_stick_axes_within_range() {
        for raw in [0u16, 1, 16383, 32767, 32768, 49151, 65534, 65535] {
            let data = stick_report(raw, raw, raw, 0, 0, 0xFF);
            let state = parse_warthog_stick(&data).unwrap();
            assert!(
                (-1.0..=1.0).contains(&state.axes.x),
                "x out of range: {}",
                state.axes.x
            );
            assert!(
                (-1.0..=1.0).contains(&state.axes.y),
                "y out of range: {}",
                state.axes.y
            );
            assert!(
                (-1.0..=1.0).contains(&state.axes.rz),
                "rz out of range: {}",
                state.axes.rz
            );
        }
    }

    // ─── Throttle tests ─────────────────────────────────────────────────────

    #[test]
    fn test_throttle_too_short() {
        assert!(parse_warthog_throttle(&[0u8; 17]).is_err());
        assert!(parse_warthog_throttle(&[]).is_err());
    }

    #[test]
    fn test_throttle_idle() {
        let data = throttle_report(32768, 32768, 0, 0, 0, 0, 0, 0, 0, 0xFF, 0xFF);
        let state = parse_warthog_throttle(&data).unwrap();
        assert!(
            state.axes.throttle_left < 0.001,
            "left idle: {}",
            state.axes.throttle_left
        );
        assert!(
            state.axes.throttle_right < 0.001,
            "right idle: {}",
            state.axes.throttle_right
        );
        assert_eq!(state.buttons.hat_dms, WarthogHat::Center);
        assert_eq!(state.buttons.hat_csl, WarthogHat::Center);
    }

    #[test]
    fn test_throttle_full() {
        let data = throttle_report(32768, 32768, 65535, 65535, 65535, 0, 0, 0, 0, 0xFF, 0xFF);
        let state = parse_warthog_throttle(&data).unwrap();
        assert!(
            state.axes.throttle_left > 0.999,
            "left full: {}",
            state.axes.throttle_left
        );
        assert!(
            state.axes.throttle_right > 0.999,
            "right full: {}",
            state.axes.throttle_right
        );
    }

    #[test]
    fn test_throttle_slew_centered() {
        let data = throttle_report(32768, 32768, 0, 0, 0, 0, 0, 0, 0, 0xFF, 0xFF);
        let state = parse_warthog_throttle(&data).unwrap();
        assert!(
            state.axes.slew_x.abs() < 0.01,
            "slew_x near 0: {}",
            state.axes.slew_x
        );
        assert!(
            state.axes.slew_y.abs() < 0.01,
            "slew_y near 0: {}",
            state.axes.slew_y
        );
    }

    #[test]
    fn test_throttle_buttons() {
        let data = throttle_report(32768, 32768, 0, 0, 0, 0x8001, 0x0001, 0x01, 0, 0xFF, 0xFF);
        let state = parse_warthog_throttle(&data).unwrap();
        assert!(state.buttons.button(1), "button 1");
        assert!(state.buttons.button(16), "button 16");
        assert!(state.buttons.button(17), "button 17");
        assert!(state.buttons.button(33), "button 33");
        assert!(!state.buttons.button(34), "button 34 not pressed");
    }

    #[test]
    fn test_throttle_hats() {
        let data = throttle_report(32768, 32768, 0, 0, 0, 0, 0, 0, 0, 0x00, 0x04); // DMS=N, CSL=S
        let state = parse_warthog_throttle(&data).unwrap();
        assert_eq!(state.buttons.hat_dms, WarthogHat::North);
        assert_eq!(state.buttons.hat_csl, WarthogHat::South);
    }

    #[test]
    fn test_throttle_axes_within_range() {
        for raw in [0u16, 1, 32767, 32768, 65534, 65535] {
            let data = throttle_report(raw, raw, raw, raw, raw, 0, 0, 0, 0, 0xFF, 0xFF);
            let state = parse_warthog_throttle(&data).unwrap();
            assert!(
                (0.0..=1.0).contains(&state.axes.throttle_left),
                "left out of range: {}",
                state.axes.throttle_left
            );
            assert!(
                (-1.0..=1.0).contains(&state.axes.slew_x),
                "slew_x out of range: {}",
                state.axes.slew_x
            );
        }
    }

    // ─── Property tests ─────────────────────────────────────────────────────

    #[cfg(test)]
    mod proptest_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn stick_axes_always_in_range(x in 0u16..=65535, y in 0u16..=65535, rz in 0u16..=65535) {
                let data = stick_report(x, y, rz, 0, 0, 0xFF);
                let state = parse_warthog_stick(&data).unwrap();
                prop_assert!((-1.0..=1.0).contains(&state.axes.x));
                prop_assert!((-1.0..=1.0).contains(&state.axes.y));
                prop_assert!((-1.0..=1.0).contains(&state.axes.rz));
            }

            #[test]
            fn throttle_unipolar_axes_in_range(tl in 0u16..=65535, tr in 0u16..=65535) {
                let data = throttle_report(32768, 32768, tl, tr, tl, 0, 0, 0, 0, 0xFF, 0xFF);
                let state = parse_warthog_throttle(&data).unwrap();
                prop_assert!((0.0..=1.0).contains(&state.axes.throttle_left));
                prop_assert!((0.0..=1.0).contains(&state.axes.throttle_right));
            }

            #[test]
            fn any_valid_stick_report_parses(data in proptest::collection::vec(any::<u8>(), 10..32usize)) {
                let result = parse_warthog_stick(&data);
                prop_assert!(result.is_ok());
            }

            #[test]
            fn any_valid_throttle_report_parses(data in proptest::collection::vec(any::<u8>(), 18..40usize)) {
                let result = parse_warthog_throttle(&data);
                prop_assert!(result.is_ok());
            }

            // ── New invariants ────────────────────────────────────────────────

            /// Any report shorter than the minimum must return an error.
            #[test]
            fn stick_short_report_is_error(
                data in proptest::collection::vec(any::<u8>(), 0..WARTHOG_STICK_MIN_REPORT_BYTES)
            ) {
                prop_assert!(parse_warthog_stick(&data).is_err());
            }

            /// Any throttle report shorter than the minimum must return an error.
            #[test]
            fn throttle_short_report_is_error(
                data in proptest::collection::vec(any::<u8>(), 0..WARTHOG_THROTTLE_MIN_REPORT_BYTES)
            ) {
                prop_assert!(parse_warthog_throttle(&data).is_err());
            }

            /// Oversized stick reports must not panic and must parse successfully.
            #[test]
            fn stick_oversized_report_no_panic(
                data in proptest::collection::vec(any::<u8>(), WARTHOG_STICK_MIN_REPORT_BYTES..256)
            ) {
                let _ = parse_warthog_stick(&data);
            }

            /// Oversized throttle reports must not panic and must parse successfully.
            #[test]
            fn throttle_oversized_report_no_panic(
                data in proptest::collection::vec(any::<u8>(), WARTHOG_THROTTLE_MIN_REPORT_BYTES..256)
            ) {
                let _ = parse_warthog_throttle(&data);
            }

            /// Button decoding must be consistent with the raw bitmask (stick).
            #[test]
            fn stick_buttons_decode_consistently(
                buttons_low in any::<u16>(),
                buttons_high in any::<u8>(),
            ) {
                let data = stick_report(32768, 32768, 32768, buttons_low, buttons_high, 0xFF);
                let state = parse_warthog_stick(&data).unwrap();
                for n in 1u8..=16 {
                    let expected = (buttons_low >> (n - 1)) & 1 != 0;
                    prop_assert_eq!(state.buttons.button(n), expected, "stick button {}", n);
                }
                for n in 17u8..=19 {
                    let expected = (buttons_high >> (n - 17)) & 1 != 0;
                    prop_assert_eq!(state.buttons.button(n), expected, "stick button {}", n);
                }
                // Out-of-range buttons always false.
                prop_assert!(!state.buttons.button(0));
                prop_assert!(!state.buttons.button(20));
            }

            /// Button decoding must be consistent with the raw bitmask (throttle).
            #[test]
            fn throttle_buttons_decode_consistently(
                btn_low in any::<u16>(),
                btn_mid in any::<u16>(),
                btn_high in any::<u8>(),
            ) {
                let data = throttle_report(32768, 32768, 0, 0, 0, btn_low, btn_mid, btn_high, 0, 0xFF, 0xFF);
                let state = parse_warthog_throttle(&data).unwrap();
                for n in 1u8..=16 {
                    let expected = (btn_low >> (n - 1)) & 1 != 0;
                    prop_assert_eq!(state.buttons.button(n), expected, "throttle button {}", n);
                }
                for n in 17u8..=32 {
                    let expected = (btn_mid >> (n - 17)) & 1 != 0;
                    prop_assert_eq!(state.buttons.button(n), expected, "throttle button {}", n);
                }
                for n in 33u8..=40 {
                    let expected = (btn_high >> (n - 33)) & 1 != 0;
                    prop_assert_eq!(state.buttons.button(n), expected, "throttle button {}", n);
                }
                // Out-of-range buttons always false.
                prop_assert!(!state.buttons.button(0));
                prop_assert!(!state.buttons.button(41));
            }
        }
    }
}
