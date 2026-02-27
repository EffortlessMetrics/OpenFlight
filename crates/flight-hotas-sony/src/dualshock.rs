// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for Sony DualShock 3 and DualShock 4 controllers.
//!
//! # DualShock 4 USB HID report layout (report ID 0x01, 64 bytes)
//!
//! | Byte | Field       | Type | Range   | Notes                              |
//! |------|-------------|------|---------|------------------------------------|
//! | 0    | report_id   | u8   | 0x01    | Fixed                              |
//! | 1    | left_x      | u8   | 0..255  | 0=left, 127=center, 255=right      |
//! | 2    | left_y      | u8   | 0..255  | 0=up, 127=center, 255=down         |
//! | 3    | right_x     | u8   | 0..255  | 0=left, 127=center, 255=right      |
//! | 4    | right_y     | u8   | 0..255  | 0=up, 127=center, 255=down         |
//! | 5    | L2          | u8   | 0..255  | 0=released, 255=fully pressed      |
//! | 6    | R2          | u8   | 0..255  | 0=released, 255=fully pressed      |
//! | 7    | buttons_lo  | u8   | bitmask | D-pad in low nibble; face/shoulder |
//! | 8    | buttons_hi  | u8   | bitmask | Shoulder/trigger digital buttons   |
//! | 9    | buttons_ps  | u8   | bitmask | PS button, options, share, touchpad|

use thiserror::Error;

/// Errors returned by Sony controller report parsers.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SonyError {
    #[error("report too short: expected at least {expected} bytes, got {actual}")]
    TooShort { expected: usize, actual: usize },
}

/// Minimum HID input report length for DualShock 4 parsing.
pub const DS4_MIN_REPORT_BYTES: usize = 10;

/// Parsed input state from a DualShock 3 or DualShock 4 HID report.
#[derive(Debug, Clone, Default)]
pub struct DualShockReport {
    /// Left stick horizontal. −1.0 = full left, 1.0 = full right.
    pub left_x: f32,
    /// Left stick vertical. −1.0 = full up (pushed forward), 1.0 = full down.
    ///
    /// Raw byte 0 maps to −1.0; raw byte 255 maps to +1.0. This follows the
    /// OpenFlight convention where negative Y is "nose down" / forward push.
    pub left_y: f32,
    /// Right stick horizontal. −1.0 = full left, 1.0 = full right.
    pub right_x: f32,
    /// Right stick vertical. −1.0 = full up, 1.0 = full down.
    pub right_y: f32,
    /// L2 trigger. 0.0 = released, 1.0 = fully pressed.
    pub l2: f32,
    /// R2 trigger. 0.0 = released, 1.0 = fully pressed.
    pub r2: f32,
    /// Button bitmask from bytes 7–9 of the report.
    pub buttons: u32,
    /// D-pad / hat value from the low nibble of byte 7.
    /// 0=N, 1=NE, 2=E, 3=SE, 4=S, 5=SW, 6=W, 7=NW, 8=released.
    pub dpad: u8,
}

/// Normalize a raw u8 stick axis (0..=255, center ≈ 127.5) to −1.0..=1.0.
#[inline]
fn normalize_stick(raw: u8) -> f32 {
    ((raw as f32 - 127.5) / 127.5).clamp(-1.0, 1.0)
}

/// Normalize a raw u8 trigger axis (0..=255) to 0.0..=1.0.
#[inline]
fn normalize_trigger(raw: u8) -> f32 {
    (raw as f32 / 255.0).clamp(0.0, 1.0)
}

/// Parse a DualShock 4 USB HID input report.
///
/// The report must be at least [`DS4_MIN_REPORT_BYTES`] (10) bytes long.
/// The first byte must be report ID 0x01 (not validated — callers are
/// responsible for routing by report ID if needed).
///
/// # Errors
/// Returns [`SonyError::TooShort`] if `bytes` is shorter than
/// [`DS4_MIN_REPORT_BYTES`].
pub fn parse_ds4_report(bytes: &[u8]) -> Result<DualShockReport, SonyError> {
    if bytes.len() < DS4_MIN_REPORT_BYTES {
        return Err(SonyError::TooShort {
            expected: DS4_MIN_REPORT_BYTES,
            actual: bytes.len(),
        });
    }

    let left_x = normalize_stick(bytes[1]);
    let left_y = normalize_stick(bytes[2]);
    let right_x = normalize_stick(bytes[3]);
    let right_y = normalize_stick(bytes[4]);
    let l2 = normalize_trigger(bytes[5]);
    let r2 = normalize_trigger(bytes[6]);

    let dpad = bytes[7] & 0x0F;
    let buttons = (bytes[7] as u32) | ((bytes[8] as u32) << 8) | ((bytes[9] as u32) << 16);

    tracing::trace!(
        left_x,
        left_y,
        right_x,
        right_y,
        l2,
        r2,
        dpad,
        buttons,
        "DS4 report parsed"
    );

    Ok(DualShockReport {
        left_x,
        left_y,
        right_x,
        right_y,
        l2,
        r2,
        buttons,
        dpad,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal 10-byte DS4 report from logical field values.
    fn build_ds4(
        left_x: u8,
        left_y: u8,
        right_x: u8,
        right_y: u8,
        l2: u8,
        r2: u8,
        buttons_lo: u8,
        buttons_hi: u8,
        buttons_ps: u8,
    ) -> [u8; 10] {
        [
            0x01, left_x, left_y, right_x, right_y, l2, r2, buttons_lo, buttons_hi, buttons_ps,
        ]
    }

    #[test]
    fn test_ds4_center_position() {
        let data = build_ds4(127, 127, 127, 127, 0, 0, 0x08, 0, 0);
        let state = parse_ds4_report(&data).unwrap();
        assert!(state.left_x.abs() < 0.01, "left_x: {}", state.left_x);
        assert!(state.left_y.abs() < 0.01, "left_y: {}", state.left_y);
        assert!(state.right_x.abs() < 0.01, "right_x: {}", state.right_x);
        assert!(state.right_y.abs() < 0.01, "right_y: {}", state.right_y);
        assert!(state.l2 < 0.01, "l2: {}", state.l2);
        assert!(state.r2 < 0.01, "r2: {}", state.r2);
    }

    #[test]
    fn test_ds4_full_left_stick() {
        let data = build_ds4(0, 127, 127, 127, 0, 0, 0x08, 0, 0);
        let state = parse_ds4_report(&data).unwrap();
        assert!(
            state.left_x < -0.99,
            "left_x at 0 should be ~-1.0: {}",
            state.left_x
        );
    }

    #[test]
    fn test_ds4_full_right_stick() {
        let data = build_ds4(255, 127, 127, 127, 0, 0, 0x08, 0, 0);
        let state = parse_ds4_report(&data).unwrap();
        assert!(
            state.left_x > 0.99,
            "left_x at 255 should be ~+1.0: {}",
            state.left_x
        );
    }

    #[test]
    fn test_ds4_y_axis_inverted() {
        // left_y raw=0 → −1.0 (stick pushed fully up/forward)
        let data = build_ds4(127, 0, 127, 127, 0, 0, 0x08, 0, 0);
        let state = parse_ds4_report(&data).unwrap();
        assert!(
            state.left_y < -0.99,
            "left_y at 0 should be ~-1.0 (up): {}",
            state.left_y
        );
    }

    #[test]
    fn test_ds4_l2_full_pressed() {
        let data = build_ds4(127, 127, 127, 127, 255, 0, 0x08, 0, 0);
        let state = parse_ds4_report(&data).unwrap();
        assert!(state.l2 > 0.99, "l2 at 255 should be ~1.0: {}", state.l2);
    }

    #[test]
    fn test_ds4_buttons_cross() {
        // Cross button is bit 5 of byte 7 (bit index 5 in the bitmask = value 0x20)
        let data = build_ds4(127, 127, 127, 127, 0, 0, 0x28, 0, 0); // 0x28 = dpad released (0x8) | cross (0x20)
        let state = parse_ds4_report(&data).unwrap();
        assert_ne!(state.buttons & 0x20, 0, "cross button bit should be set");
        assert_eq!(state.dpad, 8, "dpad should be released (8)");
    }

    #[test]
    fn test_ds4_too_short_error() {
        let result = parse_ds4_report(&[0x01, 127, 127, 127, 127, 0, 0, 0, 0]);
        assert!(matches!(
            result,
            Err(SonyError::TooShort {
                expected: 10,
                actual: 9
            })
        ));

        let result = parse_ds4_report(&[]);
        assert!(matches!(
            result,
            Err(SonyError::TooShort {
                expected: 10,
                actual: 0
            })
        ));
    }

    #[test]
    fn test_ds4_dpad_values() {
        for dpad_val in 0u8..=8 {
            let data = build_ds4(127, 127, 127, 127, 0, 0, dpad_val, 0, 0);
            let state = parse_ds4_report(&data).unwrap();
            assert_eq!(state.dpad, dpad_val, "dpad mismatch for value {dpad_val}");
        }
    }

    #[cfg(test)]
    mod proptest_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn ds4_axes_always_in_range(
                lx in 0u8..=255,
                ly in 0u8..=255,
                rx in 0u8..=255,
                ry in 0u8..=255,
                l2 in 0u8..=255,
                r2 in 0u8..=255,
            ) {
                let data = [0x01, lx, ly, rx, ry, l2, r2, 0x08, 0, 0];
                let state = parse_ds4_report(&data).unwrap();
                prop_assert!((-1.0f32..=1.0).contains(&state.left_x));
                prop_assert!((-1.0f32..=1.0).contains(&state.left_y));
                prop_assert!((-1.0f32..=1.0).contains(&state.right_x));
                prop_assert!((-1.0f32..=1.0).contains(&state.right_y));
                prop_assert!((0.0f32..=1.0).contains(&state.l2));
                prop_assert!((0.0f32..=1.0).contains(&state.r2));
            }

            #[test]
            fn ds4_any_report_parses(
                data in proptest::collection::vec(any::<u8>(), 10..64usize),
            ) {
                let result = parse_ds4_report(&data);
                prop_assert!(result.is_ok());
            }
        }
    }
}
