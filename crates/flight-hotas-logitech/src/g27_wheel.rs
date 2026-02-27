// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the Logitech G27 Racing Wheel.
//!
//! # Confirmed device identifier
//!
//! VID 0x046D (Logitech), PID 0xC29B — confirmed via the Linux kernel
//! `hid-logitech-hidpp.c` driver source and linux-hardware.org probes.
//!
//! # Device overview
//!
//! The G27 is a force-feedback racing wheel with three pedals (accelerator,
//! brake, clutch) and a H-pattern shifter with shift paddles. Some flight-sim
//! pilots use it as a throttle or control input by mapping pedals and the
//! steering axis to sim axes.
//!
//! # Report layout (estimated, 8 bytes minimum)
//!
//! **Caution:** The exact HID descriptor byte layout has not been independently
//! verified on hardware against raw USB captures. The layout below follows a
//! plausible scheme consistent with community documentation. Validate with
//! `lsusb -d 046d:c29b -v` or a USB sniffer before relying on this in
//! production.
//!
//! | Byte(s) | Field              | Type  | Range    | Notes                            |
//! |---------|--------------------|-------|----------|----------------------------------|
//! | 0-1     | Wheel              | u16BE | 0..65535 | Bipolar; center ≈ 32768          |
//! | 2       | Accelerator        | u8    | 0..255   | Unipolar; 0 = fully released     |
//! | 3       | Brake              | u8    | 0..255   | Unipolar; 0 = fully released     |
//! | 4       | Clutch             | u8    | 0..255   | Unipolar; 0 = fully released     |
//! | 5       | Buttons\[7:0\]     | u8    | bitmask  | Bit 0 = shift up, bit 1 = dn     |
//! | 6       | Buttons\[15:8\]    | u8    | bitmask  | Wheel face buttons               |
//! | 7       | Btns\[19:16\]/hat  | u8    | —        | Lower nibble = buttons; upper = dpad |

use thiserror::Error;

/// USB Vendor ID for Logitech, shared by all devices in this crate.
pub const LOGITECH_VID: u16 = 0x046D;

/// USB Product ID for the Logitech G27 Racing Wheel.
///
/// Confirmed via the Linux kernel `hid-logitech-hidpp.c` source.
pub const G27_PID: u16 = 0xC29B;

/// Minimum HID input report length in bytes.
pub const G27_MIN_REPORT_BYTES: usize = 8;

/// Parsed input state from a G27 HID input report.
#[derive(Debug, Clone)]
pub struct G27State {
    /// Steering wheel axis. −1.0 = full left, 1.0 = full right.
    pub wheel: f32,
    /// Accelerator pedal. 0.0 = released, 1.0 = fully depressed.
    pub accelerator: f32,
    /// Brake pedal. 0.0 = released, 1.0 = fully depressed.
    pub brake: f32,
    /// Clutch pedal. 0.0 = released, 1.0 = fully depressed.
    pub clutch: f32,
    /// Shift-up paddle (bit 0 of byte 5).
    pub shift_up: bool,
    /// Shift-down paddle (bit 1 of byte 5).
    pub shift_down: bool,
    /// Button bitmask; bit 0 = shift up, bit 1 = shift down, bits 2-19 = other buttons.
    /// Upper 12 bits are always 0.
    pub buttons: u32,
    /// D-pad hat switch value (upper nibble of byte 7). 0=N, 1=NE, 2=E, 3=SE,
    /// 4=S, 5=SW, 6=W, 7=NW, 8-15=center.
    pub dpad: u8,
}

impl G27State {
    /// Returns `true` if the specified 1-indexed button (1–20) is pressed.
    pub fn button(&self, n: u8) -> bool {
        matches!(n, 1..=20) && (self.buttons >> (n - 1)) & 1 != 0
    }
}

/// Errors returned by G27 report parsing.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum G27ParseError {
    #[error("G27 report too short: expected at least {expected} bytes, got {actual}")]
    TooShort { expected: usize, actual: usize },
}

/// Normalize a 16-bit big-endian bipolar axis (0..65535) to −1.0..=1.0.
#[inline]
fn normalize_wheel_16bit_be(raw: u16) -> f32 {
    (raw as f32 - 32767.5) / 32767.5
}

/// Normalize an 8-bit unipolar axis (0..255) to 0.0..=1.0.
#[inline]
fn normalize_pedal_8bit(raw: u8) -> f32 {
    raw as f32 / 255.0
}

/// Parse an 8-byte HID input report from the Logitech G27 Racing Wheel.
///
/// The report must not include a USB report ID prefix. Strip it before calling
/// this function if the host OS prepends one.
///
/// **Note:** The byte layout is approximate and based on community documentation.
/// Validate against hardware before relying on this parser in production.
///
/// # Errors
///
/// Returns [`G27ParseError::TooShort`] if `data` is shorter than
/// [`G27_MIN_REPORT_BYTES`].
pub fn parse_g27(data: &[u8]) -> Result<G27State, G27ParseError> {
    if data.len() < G27_MIN_REPORT_BYTES {
        return Err(G27ParseError::TooShort {
            expected: G27_MIN_REPORT_BYTES,
            actual: data.len(),
        });
    }

    let wheel_raw = u16::from_be_bytes([data[0], data[1]]);
    // Lower nibble of byte 7 extends the button field to 20 bits; upper nibble is d-pad.
    let buttons = (data[5] as u32) | ((data[6] as u32) << 8) | (((data[7] as u32) & 0x0F) << 16);

    Ok(G27State {
        wheel: normalize_wheel_16bit_be(wheel_raw),
        accelerator: normalize_pedal_8bit(data[2]),
        brake: normalize_pedal_8bit(data[3]),
        clutch: normalize_pedal_8bit(data[4]),
        shift_up: (data[5] & 0x01) != 0,
        shift_down: (data[5] & 0x02) != 0,
        buttons,
        dpad: (data[7] >> 4) & 0x0F,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build an 8-byte G27 report from logical field values.
    ///
    /// `dpad`: 0=N, 1=NE, …, 7=NW, 8-15=center.
    fn build_report(
        wheel: u16,
        accelerator: u8,
        brake: u8,
        clutch: u8,
        buttons: u32,
        dpad: u8,
    ) -> [u8; 8] {
        let buttons = buttons & 0x000F_FFFF; // 20 bits
        let dpad = dpad & 0x0F;
        let mut d = [0u8; 8];
        let [hi, lo] = wheel.to_be_bytes();
        d[0] = hi;
        d[1] = lo;
        d[2] = accelerator;
        d[3] = brake;
        d[4] = clutch;
        d[5] = buttons as u8;
        d[6] = (buttons >> 8) as u8;
        d[7] = ((buttons >> 16) as u8 & 0x0F) | (dpad << 4);
        d
    }

    #[test]
    fn test_too_short() {
        assert!(parse_g27(&[]).is_err());
        assert!(parse_g27(&[0u8; 7]).is_err());
        let err = parse_g27(&[0u8; 3]).unwrap_err();
        assert_eq!(
            err,
            G27ParseError::TooShort {
                expected: 8,
                actual: 3
            }
        );
    }

    #[test]
    fn test_wheel_center() {
        // 32768 is one step above center; result should be very close to 0.
        let data = build_report(32768, 0, 0, 0, 0, 8);
        let state = parse_g27(&data).unwrap();
        assert!(state.wheel.abs() < 0.01, "wheel near 0: {}", state.wheel);
    }

    #[test]
    fn test_wheel_full_right() {
        let data = build_report(65535, 0, 0, 0, 0, 8);
        let state = parse_g27(&data).unwrap();
        assert!(state.wheel > 0.999, "wheel should be ~1.0: {}", state.wheel);
    }

    #[test]
    fn test_wheel_full_left() {
        let data = build_report(0, 0, 0, 0, 0, 8);
        let state = parse_g27(&data).unwrap();
        assert!(
            state.wheel < -0.999,
            "wheel should be ~-1.0: {}",
            state.wheel
        );
    }

    #[test]
    fn test_accelerator_range() {
        let data = build_report(32768, 255, 0, 0, 0, 8);
        let state = parse_g27(&data).unwrap();
        assert!(
            state.accelerator > 0.999,
            "accelerator full: {}",
            state.accelerator
        );

        let data = build_report(32768, 0, 0, 0, 0, 8);
        let state = parse_g27(&data).unwrap();
        assert!(
            state.accelerator < 0.001,
            "accelerator idle: {}",
            state.accelerator
        );
    }

    #[test]
    fn test_brake_and_clutch_range() {
        let data = build_report(32768, 0, 255, 255, 0, 8);
        let state = parse_g27(&data).unwrap();
        assert!(state.brake > 0.999, "brake full: {}", state.brake);
        assert!(state.clutch > 0.999, "clutch full: {}", state.clutch);

        let data = build_report(32768, 0, 0, 0, 0, 8);
        let state = parse_g27(&data).unwrap();
        assert!(state.brake < 0.001, "brake idle: {}", state.brake);
        assert!(state.clutch < 0.001, "clutch idle: {}", state.clutch);
    }

    #[test]
    fn test_shift_paddles() {
        // shift_up = bit 0 of byte 5 = button bit 0
        let data = build_report(32768, 0, 0, 0, 0x01, 8);
        let state = parse_g27(&data).unwrap();
        assert!(state.shift_up, "shift_up should be pressed");
        assert!(!state.shift_down, "shift_down should not be pressed");

        // shift_down = bit 1 of byte 5 = button bit 1
        let data = build_report(32768, 0, 0, 0, 0x02, 8);
        let state = parse_g27(&data).unwrap();
        assert!(!state.shift_up, "shift_up should not be pressed");
        assert!(state.shift_down, "shift_down should be pressed");
    }

    #[test]
    fn test_buttons_individual() {
        for b in 1u8..=20 {
            let mask = 1u32 << (b - 1);
            let data = build_report(32768, 0, 0, 0, mask, 8);
            let state = parse_g27(&data).unwrap();
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
    fn test_dpad_values() {
        for raw in 0u8..=8 {
            let data = build_report(32768, 0, 0, 0, 0, raw);
            let state = parse_g27(&data).unwrap();
            assert_eq!(state.dpad, raw, "dpad should be {}", raw);
        }
        // Values 9-15 are still valid nibbles (treated as center by consumers)
        let data = build_report(32768, 0, 0, 0, 0, 15);
        let state = parse_g27(&data).unwrap();
        assert_eq!(state.dpad, 15);
    }

    #[test]
    fn test_out_of_range_buttons_false() {
        let data = build_report(32768, 0, 0, 0, 0x000F_FFFF, 8);
        let state = parse_g27(&data).unwrap();
        assert!(!state.button(0), "button 0 out of range");
        for b in 21u8..=32 {
            assert!(!state.button(b), "button {} out of range", b);
        }
        assert_eq!(
            state.buttons & 0xFFF0_0000,
            0,
            "upper 12 bits of button word must be 0"
        );
    }

    #[test]
    fn test_minimum_length_accepted() {
        let data = build_report(32768, 128, 64, 32, 0, 8);
        assert!(parse_g27(&data).is_ok());
        // Longer reports are also accepted
        let mut longer = [0u8; 16];
        longer[..8].copy_from_slice(&data);
        assert!(parse_g27(&longer).is_ok());
    }

    #[cfg(test)]
    mod proptest_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn wheel_always_in_range(wheel in 0u16..=65535) {
                let data = build_report(wheel, 0, 0, 0, 0, 8);
                let state = parse_g27(&data).unwrap();
                prop_assert!((-1.0f32..=1.0).contains(&state.wheel));
            }

            #[test]
            fn accelerator_always_unipolar(acc in 0u8..=255) {
                let data = build_report(32768, acc, 0, 0, 0, 8);
                let state = parse_g27(&data).unwrap();
                prop_assert!((0.0f32..=1.0).contains(&state.accelerator));
            }

            #[test]
            fn brake_always_unipolar(brake in 0u8..=255) {
                let data = build_report(32768, 0, brake, 0, 0, 8);
                let state = parse_g27(&data).unwrap();
                prop_assert!((0.0f32..=1.0).contains(&state.brake));
            }

            #[test]
            fn clutch_always_unipolar(clutch in 0u8..=255) {
                let data = build_report(32768, 0, 0, clutch, 0, 8);
                let state = parse_g27(&data).unwrap();
                prop_assert!((0.0f32..=1.0).contains(&state.clutch));
            }

            #[test]
            fn buttons_roundtrip(buttons in 0u32..=0x000F_FFFFu32) {
                let data = build_report(32768, 0, 0, 0, buttons, 8);
                let state = parse_g27(&data).unwrap();
                prop_assert_eq!(state.buttons, buttons);
            }

            #[test]
            fn any_8byte_report_no_panic(
                data in proptest::collection::vec(any::<u8>(), 8..16usize),
            ) {
                let result = parse_g27(&data);
                prop_assert!(result.is_ok());
            }

            #[test]
            fn arbitrary_bytes_axes_in_range(
                data in proptest::collection::vec(any::<u8>(), 8..16usize),
            ) {
                let state = parse_g27(&data).unwrap();
                prop_assert!((-1.0f32..=1.0).contains(&state.wheel), "wheel: {}", state.wheel);
                prop_assert!((0.0f32..=1.0).contains(&state.accelerator), "acc: {}", state.accelerator);
                prop_assert!((0.0f32..=1.0).contains(&state.brake), "brake: {}", state.brake);
                prop_assert!((0.0f32..=1.0).contains(&state.clutch), "clutch: {}", state.clutch);
            }

            #[test]
            fn out_of_range_buttons_always_false(
                data in proptest::collection::vec(any::<u8>(), 8..16usize),
            ) {
                let state = parse_g27(&data).unwrap();
                prop_assert!(!state.button(0));
                for b in 21u8..=32 {
                    prop_assert!(!state.button(b), "button {} out of range should be false", b);
                }
                prop_assert_eq!(state.buttons & 0xFFF0_0000, 0, "upper 12 bits must be 0");
            }
        }
    }
}
