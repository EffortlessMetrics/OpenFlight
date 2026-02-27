// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID report parsing and LED command building for GoFlight panel modules.
//!
//! All GoFlight modules share a common 8-byte HID report format. Encoders
//! report signed delta clicks, buttons are a 16-bit bitmask, and LED state
//! is a single output byte promoted to u16 for the public API.

use thiserror::Error;

/// GoFlight module type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoFlightModule {
    /// GF-46 COM/NAV radio panel.
    Gf46,
    /// GF-45 autopilot panel.
    Gf45,
    /// GF-LGT landing gear / lighting panel.
    GfLgt,
    /// GF-WCP weather / climate panel.
    GfWcp,
}

/// Errors returned by GoFlight report parsing.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GoFlightError {
    /// The input buffer was shorter than the required 8 bytes.
    #[error("report too short: expected at least {expected} bytes, got {actual}")]
    TooShort { expected: usize, actual: usize },
}

/// Minimum HID input report length for any GoFlight module.
pub const GOFLIGHT_MIN_REPORT_BYTES: usize = 8;

/// Parsed input/output state from a GoFlight module HID report.
#[derive(Debug, Clone)]
pub struct GoFlightReport {
    /// Module that produced this report.
    pub module: GoFlightModule,
    /// Signed delta clicks for up to 4 rotary encoders.
    /// Positive = clockwise, negative = counter-clockwise.
    pub encoders: [i8; 4],
    /// Button state bitmask (bit 0 = button 1).
    pub buttons: u16,
    /// Requested LED states from the device (bit per LED).
    pub leds: u16,
}

/// Parse an 8-byte GoFlight HID input report.
///
/// # Report layout
///
/// | Byte  | Field           | Type | Notes                       |
/// |-------|-----------------|------|-----------------------------|
/// | 0     | report_id       | u8   | HID report identifier       |
/// | 1–4   | encoder_deltas  | i8×4 | Signed delta per encoder    |
/// | 5–6   | button_state    | u16  | Little-endian bitmask       |
/// | 7     | led_state       | u8   | LED bitmask from device     |
///
/// # Errors
///
/// Returns [`GoFlightError::TooShort`] if `bytes` has fewer than
/// [`GOFLIGHT_MIN_REPORT_BYTES`] bytes.
pub fn parse_report(bytes: &[u8], module: GoFlightModule) -> Result<GoFlightReport, GoFlightError> {
    if bytes.len() < GOFLIGHT_MIN_REPORT_BYTES {
        tracing::warn!(
            expected = GOFLIGHT_MIN_REPORT_BYTES,
            actual = bytes.len(),
            "GoFlight report too short"
        );
        return Err(GoFlightError::TooShort {
            expected: GOFLIGHT_MIN_REPORT_BYTES,
            actual: bytes.len(),
        });
    }

    let encoders = [
        bytes[1] as i8,
        bytes[2] as i8,
        bytes[3] as i8,
        bytes[4] as i8,
    ];
    let buttons = u16::from_le_bytes([bytes[5], bytes[6]]);
    let leds = bytes[7] as u16;

    Ok(GoFlightReport {
        module,
        encoders,
        buttons,
        leds,
    })
}

/// Build an 8-byte HID output report to set LED states on a GoFlight module.
///
/// # Report layout
///
/// | Byte  | Value              |
/// |-------|--------------------|
/// | 0     | `0x01` (report ID) |
/// | 1–2   | `leds` as LE u16   |
/// | 3–7   | `0x00` (padding)   |
pub fn build_led_command(leds: u16) -> [u8; 8] {
    let [lo, hi] = leds.to_le_bytes();
    [0x01, lo, hi, 0x00, 0x00, 0x00, 0x00, 0x00]
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn make_report(report_id: u8, enc: [i8; 4], buttons: u16, leds: u8) -> [u8; 8] {
        let [bl, bh] = buttons.to_le_bytes();
        [
            report_id,
            enc[0] as u8,
            enc[1] as u8,
            enc[2] as u8,
            enc[3] as u8,
            bl,
            bh,
            leds,
        ]
    }

    #[test]
    fn test_parse_gf46_no_events() {
        let data = make_report(0x01, [0, 0, 0, 0], 0, 0);
        let r = parse_report(&data, GoFlightModule::Gf46).unwrap();
        assert_eq!(r.module, GoFlightModule::Gf46);
        assert_eq!(r.encoders, [0, 0, 0, 0]);
        assert_eq!(r.buttons, 0);
        assert_eq!(r.leds, 0);
    }

    #[test]
    fn test_parse_encoder_increment() {
        let data = make_report(0x01, [1, 0, 0, 0], 0, 0);
        let r = parse_report(&data, GoFlightModule::Gf45).unwrap();
        assert_eq!(r.encoders[0], 1);
        assert_eq!(r.encoders[1], 0);
    }

    #[test]
    fn test_parse_encoder_decrement() {
        let data = make_report(0x01, [0, -3, 0, 0], 0, 0);
        let r = parse_report(&data, GoFlightModule::Gf46).unwrap();
        assert_eq!(r.encoders[1], -3);
    }

    #[test]
    fn test_parse_button_press() {
        let data = make_report(0x01, [0, 0, 0, 0], 0b0000_0000_0000_0101, 0);
        let r = parse_report(&data, GoFlightModule::GfLgt).unwrap();
        assert_eq!(r.buttons, 0b0000_0000_0000_0101);
        assert!(r.buttons & 0x01 != 0, "button 1 should be pressed");
        assert!(r.buttons & 0x04 != 0, "button 3 should be pressed");
    }

    #[test]
    fn test_build_led_command_all_on() {
        let cmd = build_led_command(0xFFFF);
        assert_eq!(cmd[0], 0x01, "report ID must be 0x01");
        assert_eq!(cmd[1], 0xFF);
        assert_eq!(cmd[2], 0xFF);
        assert_eq!(&cmd[3..], &[0, 0, 0, 0, 0]);
    }

    #[test]
    fn test_build_led_command_all_off() {
        let cmd = build_led_command(0x0000);
        assert_eq!(cmd[0], 0x01);
        assert_eq!(cmd[1], 0x00);
        assert_eq!(cmd[2], 0x00);
        assert_eq!(&cmd[3..], &[0, 0, 0, 0, 0]);
    }

    #[test]
    fn test_parse_too_short_error() {
        assert!(parse_report(&[0u8; 7], GoFlightModule::Gf46).is_err());
        assert!(parse_report(&[], GoFlightModule::Gf45).is_err());
        let err = parse_report(&[0u8; 3], GoFlightModule::GfWcp).unwrap_err();
        assert!(matches!(
            err,
            GoFlightError::TooShort {
                expected: 8,
                actual: 3
            }
        ));
    }

    #[test]
    fn test_encoder_overflow_not_panics() {
        // i8 wraps on overflow — cast from u8 0xFF = -1i8; must not panic
        let data = make_report(0x01, [-1, -1, -1, -1], 0xFFFF, 0xFF);
        let r = parse_report(&data, GoFlightModule::GfWcp).unwrap();
        assert_eq!(r.encoders, [-1i8, -1, -1, -1]);
        assert_eq!(r.buttons, 0xFFFF);
        assert_eq!(r.leds, 0xFF);
    }

    proptest! {
        #[test]
        fn test_parse_report_fields_round_trip(
            bytes in proptest::collection::vec(any::<u8>(), 8..=16)
        ) {
            let r = parse_report(&bytes, GoFlightModule::Gf46).unwrap();
            // Encoder deltas must match raw bytes reinterpreted as i8
            prop_assert_eq!(r.encoders[0], bytes[1] as i8);
            prop_assert_eq!(r.encoders[1], bytes[2] as i8);
            prop_assert_eq!(r.encoders[2], bytes[3] as i8);
            prop_assert_eq!(r.encoders[3], bytes[4] as i8);
            // Button state must match little-endian bytes[5..7]
            let expected_buttons = u16::from_le_bytes([bytes[5], bytes[6]]);
            prop_assert_eq!(r.buttons, expected_buttons);
        }

        #[test]
        fn test_build_led_command_round_trip(leds: u16) {
            let cmd = build_led_command(leds);
            prop_assert_eq!(cmd[0], 0x01);
            let parsed = u16::from_le_bytes([cmd[1], cmd[2]]);
            prop_assert_eq!(parsed, leds);
            prop_assert_eq!(&cmd[3..], &[0u8, 0, 0, 0, 0]);
        }
    }
}
