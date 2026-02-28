// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for Turtle Beach VelocityOne devices.
//!
//! # Confirmed device identifiers
//!
//! | Model                         | VID    | PID    | Tier | Notes                   |
//! |-------------------------------|--------|--------|------|-------------------------|
//! | VelocityOne Flightdeck (yoke) | 0x1432 | 0xB300 | 1    | Confirmed via usb.ids   |
//! | VelocityOne Stick             | 0x1432 | 0xB301 | 3    | PID estimated           |
//! | VelocityOne Rudder            | 0x1432 | 0xB302 | 3    | PID estimated           |

use thiserror::Error;

/// Turtle Beach VendorID.
pub const TURTLEBEACH_VID: u16 = 0x1432;

/// PID for the VelocityOne Flightdeck yoke (confirmed via usb.ids).
pub const VELOCITYONE_FLIGHTDECK_PID: u16 = 0xB300;

/// PID for the VelocityOne Stick (estimated, tier:3 — not USB-capture verified).
pub const VELOCITYONE_STICK_PID: u16 = 0xB301;

/// PID for the VelocityOne Rudder (estimated, tier:3 — not USB-capture verified).
pub const VELOCITYONE_RUDDER_PID: u16 = 0xB302;

/// Minimum report length for a Flightdeck (yoke) HID input report.
pub const FLIGHTDECK_MIN_REPORT_BYTES: usize = 16;

/// Minimum report length for a Rudder HID input report.
pub const RUDDER_MIN_REPORT_BYTES: usize = 8;

/// Turtle Beach VelocityOne device model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VelocityOneModel {
    /// VelocityOne Flightdeck yoke — VID 0x1432, PID 0xB300 (confirmed).
    Flightdeck,
    /// VelocityOne Stick — VID 0x1432, PID 0xB301 (estimated, tier:3).
    Stick,
    /// VelocityOne Rudder pedals — VID 0x1432, PID 0xB302 (estimated, tier:3).
    Rudder,
}

/// Errors returned by VelocityOne HID report parsing.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TurtleBeachError {
    /// The input buffer was shorter than required for this report type.
    #[error("report too short: expected at least {expected} bytes, got {actual}")]
    TooShort { expected: usize, actual: usize },
}

/// Parsed input state from a VelocityOne Flightdeck (yoke) HID report.
///
/// # Axis conventions
///
/// Bipolar axes are normalised to −1.0..=1.0; unipolar axes to 0.0..=1.0.
#[derive(Debug, Clone)]
pub struct VelocityOneFlightdeckReport {
    /// Roll axis. −1.0 = full left, 1.0 = full right.
    pub roll: f32,
    /// Pitch axis. −1.0 = full forward, 1.0 = full back.
    pub pitch: f32,
    /// Left throttle lever. 0.0 = idle, 1.0 = full.
    pub throttle_left: f32,
    /// Right throttle lever. 0.0 = idle, 1.0 = full.
    pub throttle_right: f32,
    /// Button bitmask (bit 0 = button 1, little-endian).
    pub buttons: u32,
}

/// Parsed input state from a VelocityOne Rudder HID report.
#[derive(Debug, Clone)]
pub struct VelocityOneRudderReport {
    /// Rudder (yaw) axis. −1.0 = full left, 1.0 = full right.
    pub rudder: f32,
    /// Left toe brake. 0.0 = released, 1.0 = fully pressed.
    pub brake_left: f32,
    /// Right toe brake. 0.0 = released, 1.0 = fully pressed.
    pub brake_right: f32,
}

/// Parse a VelocityOne Flightdeck HID input report.
///
/// # Report layout (bytes, little-endian)
///
/// | Bytes | Field          | Type | Raw range | Notes                    |
/// |-------|----------------|------|-----------|--------------------------|
/// | 0–1   | roll           | u16  | 0–65535   | Center ≈ 32767           |
/// | 2–3   | pitch          | u16  | 0–65535   | Center ≈ 32767           |
/// | 4     | throttle_left  | u8   | 0–255     | 0 = idle, 255 = full     |
/// | 5     | throttle_right | u8   | 0–255     | 0 = idle, 255 = full     |
/// | 6–9   | buttons        | u32  | bitmask   | Little-endian bitfield   |
/// | 10–15 | reserved       | —    | —         | Ignored                  |
///
/// # Errors
///
/// Returns [`TurtleBeachError::TooShort`] if `bytes` has fewer than
/// [`FLIGHTDECK_MIN_REPORT_BYTES`] bytes.
pub fn parse_flightdeck_report(
    bytes: &[u8],
) -> Result<VelocityOneFlightdeckReport, TurtleBeachError> {
    if bytes.len() < FLIGHTDECK_MIN_REPORT_BYTES {
        tracing::warn!(
            expected = FLIGHTDECK_MIN_REPORT_BYTES,
            actual = bytes.len(),
            "VelocityOne Flightdeck report too short"
        );
        return Err(TurtleBeachError::TooShort {
            expected: FLIGHTDECK_MIN_REPORT_BYTES,
            actual: bytes.len(),
        });
    }

    let roll_raw = u16::from_le_bytes([bytes[0], bytes[1]]);
    let pitch_raw = u16::from_le_bytes([bytes[2], bytes[3]]);
    let throttle_left_raw = bytes[4];
    let throttle_right_raw = bytes[5];
    let buttons = u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]);

    Ok(VelocityOneFlightdeckReport {
        roll: normalize_u16_bipolar(roll_raw),
        pitch: normalize_u16_bipolar(pitch_raw),
        throttle_left: normalize_u8_unipolar(throttle_left_raw),
        throttle_right: normalize_u8_unipolar(throttle_right_raw),
        buttons,
    })
}

/// Parse a VelocityOne Rudder HID input report.
///
/// # Report layout (bytes, little-endian)
///
/// | Bytes | Field       | Type | Raw range | Notes                    |
/// |-------|-------------|------|-----------|--------------------------|
/// | 0–1   | rudder      | u16  | 0–65535   | Center ≈ 32767           |
/// | 2     | brake_left  | u8   | 0–255     | 0 = released             |
/// | 3     | brake_right | u8   | 0–255     | 0 = released             |
/// | 4–7   | reserved    | —    | —         | Ignored                  |
///
/// # Errors
///
/// Returns [`TurtleBeachError::TooShort`] if `bytes` has fewer than
/// [`RUDDER_MIN_REPORT_BYTES`] bytes.
pub fn parse_rudder_report(bytes: &[u8]) -> Result<VelocityOneRudderReport, TurtleBeachError> {
    if bytes.len() < RUDDER_MIN_REPORT_BYTES {
        tracing::warn!(
            expected = RUDDER_MIN_REPORT_BYTES,
            actual = bytes.len(),
            "VelocityOne Rudder report too short"
        );
        return Err(TurtleBeachError::TooShort {
            expected: RUDDER_MIN_REPORT_BYTES,
            actual: bytes.len(),
        });
    }

    let rudder_raw = u16::from_le_bytes([bytes[0], bytes[1]]);
    let brake_left_raw = bytes[2];
    let brake_right_raw = bytes[3];

    Ok(VelocityOneRudderReport {
        rudder: normalize_u16_bipolar(rudder_raw),
        brake_left: normalize_u8_unipolar(brake_left_raw),
        brake_right: normalize_u8_unipolar(brake_right_raw),
    })
}

/// Normalise a 16-bit unsigned axis value to −1.0..=1.0.
///
/// Maps raw 0 → −1.0, 32767 ≈ 0.0, 65535 ≈ 1.0.
#[inline]
fn normalize_u16_bipolar(raw: u16) -> f32 {
    ((raw as f32 - 32767.5) / 32767.5).clamp(-1.0, 1.0)
}

/// Normalise an 8-bit unsigned value to 0.0..=1.0.
#[inline]
fn normalize_u8_unipolar(raw: u8) -> f32 {
    (raw as f32 / 255.0).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn make_flightdeck(roll: u16, pitch: u16, tl: u8, tr: u8, buttons: u32) -> [u8; 16] {
        let mut b = [0u8; 16];
        b[0..2].copy_from_slice(&roll.to_le_bytes());
        b[2..4].copy_from_slice(&pitch.to_le_bytes());
        b[4] = tl;
        b[5] = tr;
        b[6..10].copy_from_slice(&buttons.to_le_bytes());
        b
    }

    fn make_rudder(rudder: u16, bl: u8, br: u8) -> [u8; 8] {
        let mut b = [0u8; 8];
        b[0..2].copy_from_slice(&rudder.to_le_bytes());
        b[2] = bl;
        b[3] = br;
        b
    }

    #[test]
    fn test_parse_flightdeck_center_position() {
        let data = make_flightdeck(32767, 32767, 0, 0, 0);
        let r = parse_flightdeck_report(&data).unwrap();
        assert!(
            r.roll.abs() < 0.001,
            "roll at center should be ~0, got {}",
            r.roll
        );
        assert!(
            r.pitch.abs() < 0.001,
            "pitch at center should be ~0, got {}",
            r.pitch
        );
        assert!(r.throttle_left < 0.001);
        assert!(r.throttle_right < 0.001);
        assert_eq!(r.buttons, 0);
    }

    #[test]
    fn test_parse_flightdeck_full_left_roll() {
        let data = make_flightdeck(0, 32767, 0, 0, 0);
        let r = parse_flightdeck_report(&data).unwrap();
        assert!(
            r.roll < -0.99,
            "full left roll should be near -1.0, got {}",
            r.roll
        );
    }

    #[test]
    fn test_parse_flightdeck_full_right_roll() {
        let data = make_flightdeck(65535, 32767, 0, 0, 0);
        let r = parse_flightdeck_report(&data).unwrap();
        assert!(
            r.roll > 0.99,
            "full right roll should be near 1.0, got {}",
            r.roll
        );
    }

    #[test]
    fn test_parse_flightdeck_throttle_full() {
        let data = make_flightdeck(32767, 32767, 255, 255, 0);
        let r = parse_flightdeck_report(&data).unwrap();
        assert!(
            r.throttle_left > 0.999,
            "full throttle_left should be 1.0, got {}",
            r.throttle_left
        );
        assert!(
            r.throttle_right > 0.999,
            "full throttle_right should be 1.0, got {}",
            r.throttle_right
        );
    }

    #[test]
    fn test_parse_rudder_center() {
        let data = make_rudder(32767, 0, 0);
        let r = parse_rudder_report(&data).unwrap();
        assert!(
            r.rudder.abs() < 0.001,
            "rudder at center should be ~0, got {}",
            r.rudder
        );
        assert!(r.brake_left < 0.001);
        assert!(r.brake_right < 0.001);
    }

    #[test]
    fn test_parse_rudder_full_left() {
        let data = make_rudder(0, 0, 0);
        let r = parse_rudder_report(&data).unwrap();
        assert!(
            r.rudder < -0.99,
            "full left rudder should be near -1.0, got {}",
            r.rudder
        );
    }

    #[test]
    fn test_parse_rudder_brake_full() {
        let data = make_rudder(32767, 255, 255);
        let r = parse_rudder_report(&data).unwrap();
        assert!(
            r.brake_left > 0.999,
            "full brake_left should be 1.0, got {}",
            r.brake_left
        );
        assert!(
            r.brake_right > 0.999,
            "full brake_right should be 1.0, got {}",
            r.brake_right
        );
    }

    #[test]
    fn test_too_short_returns_error() {
        assert!(parse_flightdeck_report(&[0u8; 15]).is_err());
        assert!(parse_flightdeck_report(&[]).is_err());
        assert!(parse_rudder_report(&[0u8; 7]).is_err());
        assert!(parse_rudder_report(&[]).is_err());
    }

    proptest! {
        #[test]
        fn test_flightdeck_output_always_bounded(
            bytes in proptest::collection::vec(any::<u8>(), 16..=64)
        ) {
            let r = parse_flightdeck_report(&bytes).unwrap();
            prop_assert!((-1.0..=1.0).contains(&r.roll),
                "roll out of bounds: {}", r.roll);
            prop_assert!((-1.0..=1.0).contains(&r.pitch),
                "pitch out of bounds: {}", r.pitch);
            prop_assert!((0.0..=1.0).contains(&r.throttle_left),
                "throttle_left out of bounds: {}", r.throttle_left);
            prop_assert!((0.0..=1.0).contains(&r.throttle_right),
                "throttle_right out of bounds: {}", r.throttle_right);
        }

        #[test]
        fn test_rudder_output_always_bounded(
            bytes in proptest::collection::vec(any::<u8>(), 8..=32)
        ) {
            let r = parse_rudder_report(&bytes).unwrap();
            prop_assert!((-1.0..=1.0).contains(&r.rudder),
                "rudder out of bounds: {}", r.rudder);
            prop_assert!((0.0..=1.0).contains(&r.brake_left),
                "brake_left out of bounds: {}", r.brake_left);
            prop_assert!((0.0..=1.0).contains(&r.brake_right),
                "brake_right out of bounds: {}", r.brake_right);
        }
    }
}
