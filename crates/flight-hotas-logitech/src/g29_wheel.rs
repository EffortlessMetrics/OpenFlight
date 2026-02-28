// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the Logitech G29 Racing Wheel (and G920).
//!
//! # Confirmed device identifiers
//!
//! - VID 0x046D (Logitech), PID 0xC24F — G29 (PS3/PS4/PC), confirmed via the
//!   Linux kernel `hid-logitech-hidpp.c` driver source.
//! - VID 0x046D, PID 0xC262 — G920 (Xbox/PC), confirmed via the same source.
//!   The G920 is functionally identical to the G29 but targets the Xbox
//!   ecosystem. Both share the same HID report structure.
//!
//! # Device overview
//!
//! The G29/G920 is a force-feedback racing wheel with three pedals (accelerator,
//! brake, clutch). Some flight-sim pilots use it as a throttle or control input
//! by mapping pedals and the steering axis to sim axes.
//!
//! # Report layout (estimated, 8 bytes minimum)
//!
//! **Caution:** The exact HID descriptor byte layout has not been independently
//! verified on hardware against raw USB captures. The layout below follows a
//! plausible scheme consistent with community documentation and differs from the
//! G27 in wheel endianness and field byte positions. Validate with a USB sniffer
//! before relying on this parser in production.
//!
//! | Byte(s) | Field              | Type  | Range    | Notes                             |
//! |---------|--------------------|-------|----------|-----------------------------------|
//! | 0-1     | Wheel              | u16LE | 0..65535 | Bipolar; center ≈ 32768           |
//! | 2       | D-pad/hat          | u8    | see note | Lower nibble = hat position       |
//! | 3       | Accelerator        | u8    | 0..255   | Unipolar; 0 = fully released      |
//! | 4       | Brake              | u8    | 0..255   | Unipolar; 0 = fully released      |
//! | 5       | Clutch             | u8    | 0..255   | Unipolar; 0 = fully released      |
//! | 6       | Buttons\[7:0\]     | u8    | bitmask  | Bit 4 = shift up, bit 5 = dn      |
//! | 7       | Buttons\[15:8\]    | u8    | bitmask  | Upper wheel face buttons          |
//!
//! Hat positions (lower nibble of byte 2): 0=N, 1=NE, 2=E, 3=SE, 4=S, 5=SW,
//! 6=W, 7=NW, 8-15=center.

use thiserror::Error;

/// USB Product ID for the Logitech G29 Racing Wheel (PS3/PS4/PC).
///
/// Confirmed via the Linux kernel `hid-logitech-hidpp.c` source.
pub const G29_PID: u16 = 0xC24F;

/// USB Product ID for the Logitech G920 Racing Wheel (Xbox/PC).
///
/// The G920 is the Xbox-platform variant of the G29 and shares its HID report
/// structure. Confirmed via the Linux kernel `hid-logitech-hidpp.c` source.
pub const G920_PID: u16 = 0xC262;

/// Minimum HID input report length in bytes.
pub const G29_MIN_REPORT_BYTES: usize = 8;

/// Parsed input state from a G29 or G920 HID input report.
#[derive(Debug, Clone)]
pub struct G29State {
    /// Steering wheel axis. −1.0 = full left, 1.0 = full right.
    pub wheel: f32,
    /// Accelerator pedal. 0.0 = released, 1.0 = fully depressed.
    pub accelerator: f32,
    /// Brake pedal. 0.0 = released, 1.0 = fully depressed.
    pub brake: f32,
    /// Clutch pedal. 0.0 = released, 1.0 = fully depressed.
    pub clutch: f32,
    /// Shift-up paddle (bit 4 of byte 6).
    pub shift_up: bool,
    /// Shift-down paddle (bit 5 of byte 6).
    pub shift_down: bool,
    /// Button bitmask across bytes 6-7 (16 bits). Bit 0 = button 1.
    pub buttons: u16,
    /// D-pad hat switch value (lower nibble of byte 2). 0=N, 1=NE, 2=E, 3=SE,
    /// 4=S, 5=SW, 6=W, 7=NW, 8-15=center.
    pub dpad: u8,
}

impl G29State {
    /// Returns `true` if the specified 1-indexed button (1–16) is pressed.
    pub fn button(&self, n: u8) -> bool {
        matches!(n, 1..=16) && (self.buttons >> (n - 1)) & 1 != 0
    }
}

/// Errors returned by G29/G920 report parsing.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum G29ParseError {
    #[error("G29 report too short: expected at least {expected} bytes, got {actual}")]
    TooShort { expected: usize, actual: usize },
}

/// Normalize a 16-bit little-endian bipolar axis (0..65535) to −1.0..=1.0.
#[inline]
fn normalize_wheel_16bit_le(raw: u16) -> f32 {
    ((raw as f32 - 32767.5) / 32767.5).clamp(-1.0, 1.0)
}

/// Normalize an 8-bit unipolar axis (0..255) to 0.0..=1.0.
#[inline]
fn normalize_pedal_8bit(raw: u8) -> f32 {
    (raw as f32 / 255.0).clamp(0.0, 1.0)
}

/// Parse an 8-byte HID input report from the Logitech G29 or G920 Racing Wheel.
///
/// The report must not include a USB report ID prefix. Strip it before calling
/// this function if the host OS prepends one.
///
/// **Note:** The byte layout is approximate and based on community documentation.
/// Validate against hardware before relying on this parser in production.
///
/// # Errors
///
/// Returns [`G29ParseError::TooShort`] if `data` is shorter than
/// [`G29_MIN_REPORT_BYTES`].
pub fn parse_g29(data: &[u8]) -> Result<G29State, G29ParseError> {
    if data.len() < G29_MIN_REPORT_BYTES {
        return Err(G29ParseError::TooShort {
            expected: G29_MIN_REPORT_BYTES,
            actual: data.len(),
        });
    }

    let wheel_raw = u16::from_le_bytes([data[0], data[1]]);
    let buttons = (data[6] as u16) | ((data[7] as u16) << 8);

    Ok(G29State {
        wheel: normalize_wheel_16bit_le(wheel_raw),
        accelerator: normalize_pedal_8bit(data[3]),
        brake: normalize_pedal_8bit(data[4]),
        clutch: normalize_pedal_8bit(data[5]),
        shift_up: (data[6] & 0x10) != 0,
        shift_down: (data[6] & 0x20) != 0,
        buttons,
        dpad: data[2] & 0x0F,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build an 8-byte G29 report from logical field values.
    ///
    /// `dpad`: 0=N, 1=NE, …, 7=NW, 8-15=center.
    fn build_report(
        wheel: u16,
        dpad: u8,
        accelerator: u8,
        brake: u8,
        clutch: u8,
        buttons: u16,
    ) -> [u8; 8] {
        let [lo, hi] = wheel.to_le_bytes();
        let mut d = [0u8; 8];
        d[0] = lo;
        d[1] = hi;
        d[2] = dpad & 0x0F;
        d[3] = accelerator;
        d[4] = brake;
        d[5] = clutch;
        d[6] = buttons as u8;
        d[7] = (buttons >> 8) as u8;
        d
    }

    #[test]
    fn test_too_short() {
        assert!(parse_g29(&[]).is_err());
        assert!(parse_g29(&[0u8; 7]).is_err());
        let err = parse_g29(&[0u8; 2]).unwrap_err();
        assert_eq!(
            err,
            G29ParseError::TooShort {
                expected: 8,
                actual: 2
            }
        );
    }

    #[test]
    fn test_wheel_center() {
        let data = build_report(32768, 8, 0, 0, 0, 0);
        let state = parse_g29(&data).unwrap();
        assert!(state.wheel.abs() < 0.01, "wheel near 0: {}", state.wheel);
    }

    #[test]
    fn test_wheel_full_right() {
        let data = build_report(65535, 8, 0, 0, 0, 0);
        let state = parse_g29(&data).unwrap();
        assert!(state.wheel > 0.999, "wheel should be ~1.0: {}", state.wheel);
    }

    #[test]
    fn test_wheel_full_left() {
        let data = build_report(0, 8, 0, 0, 0, 0);
        let state = parse_g29(&data).unwrap();
        assert!(
            state.wheel < -0.999,
            "wheel should be ~-1.0: {}",
            state.wheel
        );
    }

    #[test]
    fn test_accelerator_range() {
        let data = build_report(32768, 8, 255, 0, 0, 0);
        let state = parse_g29(&data).unwrap();
        assert!(
            state.accelerator > 0.999,
            "accelerator full: {}",
            state.accelerator
        );

        let data = build_report(32768, 8, 0, 0, 0, 0);
        let state = parse_g29(&data).unwrap();
        assert!(
            state.accelerator < 0.001,
            "accelerator idle: {}",
            state.accelerator
        );
    }

    #[test]
    fn test_brake_and_clutch_range() {
        let data = build_report(32768, 8, 0, 255, 255, 0);
        let state = parse_g29(&data).unwrap();
        assert!(state.brake > 0.999, "brake full: {}", state.brake);
        assert!(state.clutch > 0.999, "clutch full: {}", state.clutch);

        let data = build_report(32768, 8, 0, 0, 0, 0);
        let state = parse_g29(&data).unwrap();
        assert!(state.brake < 0.001, "brake idle: {}", state.brake);
        assert!(state.clutch < 0.001, "clutch idle: {}", state.clutch);
    }

    #[test]
    fn test_shift_paddles() {
        // shift_up = bit 4 of byte 6
        let data = build_report(32768, 8, 0, 0, 0, 0x0010);
        let state = parse_g29(&data).unwrap();
        assert!(state.shift_up, "shift_up should be pressed");
        assert!(!state.shift_down, "shift_down should not be pressed");

        // shift_down = bit 5 of byte 6
        let data = build_report(32768, 8, 0, 0, 0, 0x0020);
        let state = parse_g29(&data).unwrap();
        assert!(!state.shift_up, "shift_up should not be pressed");
        assert!(state.shift_down, "shift_down should be pressed");
    }

    #[test]
    fn test_buttons_individual() {
        for b in 1u8..=16 {
            let mask = 1u16 << (b - 1);
            let data = build_report(32768, 8, 0, 0, 0, mask);
            let state = parse_g29(&data).unwrap();
            assert!(state.button(b), "button {} should be pressed", b);
            for other in 1u8..=16 {
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
    fn test_dpad_values() {
        for raw in 0u8..=8 {
            let data = build_report(32768, raw, 0, 0, 0, 0);
            let state = parse_g29(&data).unwrap();
            assert_eq!(state.dpad, raw, "dpad should be {}", raw);
        }
    }

    #[test]
    fn test_out_of_range_buttons_false() {
        let data = build_report(32768, 8, 0, 0, 0, 0xFFFF);
        let state = parse_g29(&data).unwrap();
        assert!(!state.button(0), "button 0 out of range");
        for b in 17u8..=32 {
            assert!(!state.button(b), "button {} out of range", b);
        }
    }

    #[test]
    fn test_minimum_length_accepted() {
        let data = build_report(32768, 8, 128, 64, 32, 0);
        assert!(parse_g29(&data).is_ok());
        // Longer reports are also accepted
        let mut longer = [0u8; 16];
        longer[..8].copy_from_slice(&data);
        assert!(parse_g29(&longer).is_ok());
    }

    #[cfg(test)]
    mod proptest_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn wheel_always_in_range(wheel in 0u16..=65535) {
                let data = build_report(wheel, 8, 0, 0, 0, 0);
                let state = parse_g29(&data).unwrap();
                prop_assert!((-1.0f32..=1.0).contains(&state.wheel));
            }

            #[test]
            fn accelerator_always_unipolar(acc in 0u8..=255) {
                let data = build_report(32768, 8, acc, 0, 0, 0);
                let state = parse_g29(&data).unwrap();
                prop_assert!((0.0f32..=1.0).contains(&state.accelerator));
            }

            #[test]
            fn brake_always_unipolar(brake in 0u8..=255) {
                let data = build_report(32768, 8, 0, brake, 0, 0);
                let state = parse_g29(&data).unwrap();
                prop_assert!((0.0f32..=1.0).contains(&state.brake));
            }

            #[test]
            fn clutch_always_unipolar(clutch in 0u8..=255) {
                let data = build_report(32768, 8, 0, 0, clutch, 0);
                let state = parse_g29(&data).unwrap();
                prop_assert!((0.0f32..=1.0).contains(&state.clutch));
            }

            #[test]
            fn buttons_roundtrip(buttons in 0u16..=0xFFFFu16) {
                let data = build_report(32768, 8, 0, 0, 0, buttons);
                let state = parse_g29(&data).unwrap();
                prop_assert_eq!(state.buttons, buttons);
            }

            #[test]
            fn any_8byte_report_no_panic(
                data in proptest::collection::vec(any::<u8>(), 8..16usize),
            ) {
                let result = parse_g29(&data);
                prop_assert!(result.is_ok());
            }

            #[test]
            fn arbitrary_bytes_axes_in_range(
                data in proptest::collection::vec(any::<u8>(), 8..16usize),
            ) {
                let state = parse_g29(&data).unwrap();
                prop_assert!((-1.0f32..=1.0).contains(&state.wheel), "wheel: {}", state.wheel);
                prop_assert!((0.0f32..=1.0).contains(&state.accelerator), "acc: {}", state.accelerator);
                prop_assert!((0.0f32..=1.0).contains(&state.brake), "brake: {}", state.brake);
                prop_assert!((0.0f32..=1.0).contains(&state.clutch), "clutch: {}", state.clutch);
            }

            #[test]
            fn out_of_range_buttons_always_false(
                data in proptest::collection::vec(any::<u8>(), 8..16usize),
            ) {
                let state = parse_g29(&data).unwrap();
                prop_assert!(!state.button(0));
                for b in 17u8..=32 {
                    prop_assert!(!state.button(b), "button {} out of range should be false", b);
                }
            }
        }
    }
}
