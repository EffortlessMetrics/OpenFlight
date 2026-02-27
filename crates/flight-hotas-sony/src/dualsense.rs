// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the Sony DualSense (PS5) controller.
//!
//! # DualSense USB HID report layout (report ID 0x01)
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
//! | 7    | buttons_0   | u8   | bitmask | Face + shoulder buttons            |
//! | 8    | buttons_1   | u8   | bitmask | Shoulder/trigger digital           |
//! | 9    | buttons_2   | u8   | bitmask | PS/create/mute/touchpad            |
//! | 10   | hat         | u8   | 0..8    | D-pad; 0=N, 4=S, 8=released        |

use crate::dualshock::SonyError;
use tracing;

/// Minimum HID input report length for DualSense parsing.
pub const DUALSENSE_MIN_REPORT_BYTES: usize = 11;

/// Parsed input state from a DualSense (PS5) HID report.
#[derive(Debug, Clone, Default)]
pub struct DualSenseReport {
    /// Left stick horizontal. −1.0 = full left, 1.0 = full right.
    pub left_x: f32,
    /// Left stick vertical. −1.0 = full up (pushed forward), 1.0 = full down.
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
    /// D-pad / hat value from byte 10.
    /// 0=N, 1=NE, 2=E, 3=SE, 4=S, 5=SW, 6=W, 7=NW, 8=released.
    pub dpad: u8,
    /// Touchpad X position, normalized −1.0..=1.0 (placeholder; populated from
    /// the extended report when available).
    pub touchpad_x: f32,
    /// Touchpad Y position, normalized −1.0..=1.0 (placeholder; populated from
    /// the extended report when available).
    pub touchpad_y: f32,
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

/// Parse a DualSense USB HID input report.
///
/// The report must be at least [`DUALSENSE_MIN_REPORT_BYTES`] (11) bytes.
/// Touchpad axes default to 0.0 in this minimal report path; extended touchpad
/// data requires a longer report and is not yet parsed.
///
/// # Errors
/// Returns [`SonyError::TooShort`] if `bytes` is shorter than
/// [`DUALSENSE_MIN_REPORT_BYTES`].
pub fn parse_dualsense_report(bytes: &[u8]) -> Result<DualSenseReport, SonyError> {
    if bytes.len() < DUALSENSE_MIN_REPORT_BYTES {
        return Err(SonyError::TooShort {
            expected: DUALSENSE_MIN_REPORT_BYTES,
            actual: bytes.len(),
        });
    }

    let left_x = normalize_stick(bytes[1]);
    let left_y = normalize_stick(bytes[2]);
    let right_x = normalize_stick(bytes[3]);
    let right_y = normalize_stick(bytes[4]);
    let l2 = normalize_trigger(bytes[5]);
    let r2 = normalize_trigger(bytes[6]);

    let buttons = (bytes[7] as u32) | ((bytes[8] as u32) << 8) | ((bytes[9] as u32) << 16);
    let dpad = bytes[10] & 0x0F;

    tracing::trace!(
        left_x,
        left_y,
        right_x,
        right_y,
        l2,
        r2,
        dpad,
        buttons,
        "DualSense report parsed"
    );

    Ok(DualSenseReport {
        left_x,
        left_y,
        right_x,
        right_y,
        l2,
        r2,
        buttons,
        dpad,
        touchpad_x: 0.0,
        touchpad_y: 0.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal 11-byte DualSense report from logical field values.
    fn build_ds(
        left_x: u8,
        left_y: u8,
        right_x: u8,
        right_y: u8,
        l2: u8,
        r2: u8,
        buttons_0: u8,
        buttons_1: u8,
        buttons_2: u8,
        hat: u8,
    ) -> [u8; 11] {
        [
            0x01, left_x, left_y, right_x, right_y, l2, r2, buttons_0, buttons_1, buttons_2, hat,
        ]
    }

    #[test]
    fn test_dualsense_center() {
        let data = build_ds(127, 127, 127, 127, 0, 0, 0, 0, 0, 8);
        let state = parse_dualsense_report(&data).unwrap();
        assert!(state.left_x.abs() < 0.01, "left_x: {}", state.left_x);
        assert!(state.left_y.abs() < 0.01, "left_y: {}", state.left_y);
        assert!(state.right_x.abs() < 0.01, "right_x: {}", state.right_x);
        assert!(state.right_y.abs() < 0.01, "right_y: {}", state.right_y);
        assert!(state.l2 < 0.01, "l2: {}", state.l2);
        assert!(state.r2 < 0.01, "r2: {}", state.r2);
    }

    #[test]
    fn test_dualsense_l2_r2() {
        let data = build_ds(127, 127, 127, 127, 255, 128, 0, 0, 0, 8);
        let state = parse_dualsense_report(&data).unwrap();
        assert!(state.l2 > 0.99, "l2 full pressed: {}", state.l2);
        assert!(
            (0.49..=0.51).contains(&state.r2),
            "r2 half pressed: {}",
            state.r2
        );
    }

    #[test]
    fn test_dualsense_too_short() {
        let result = parse_dualsense_report(&[0x01, 127, 127, 127, 127, 0, 0, 0, 0, 0]);
        assert!(matches!(
            result,
            Err(SonyError::TooShort {
                expected: 11,
                actual: 10
            })
        ));

        let result = parse_dualsense_report(&[]);
        assert!(matches!(
            result,
            Err(SonyError::TooShort {
                expected: 11,
                actual: 0
            })
        ));
    }

    #[test]
    fn test_dualsense_full_left_stick() {
        let data = build_ds(0, 127, 127, 127, 0, 0, 0, 0, 0, 8);
        let state = parse_dualsense_report(&data).unwrap();
        assert!(
            state.left_x < -0.99,
            "left_x at 0 should be ~-1.0: {}",
            state.left_x
        );
    }

    #[test]
    fn test_dualsense_y_axis_inverted() {
        let data = build_ds(127, 0, 127, 127, 0, 0, 0, 0, 0, 8);
        let state = parse_dualsense_report(&data).unwrap();
        assert!(
            state.left_y < -0.99,
            "left_y at 0 should be ~-1.0 (up): {}",
            state.left_y
        );
    }

    #[test]
    fn test_dualsense_dpad() {
        for hat_val in 0u8..=8 {
            let data = build_ds(127, 127, 127, 127, 0, 0, 0, 0, 0, hat_val);
            let state = parse_dualsense_report(&data).unwrap();
            assert_eq!(state.dpad, hat_val, "dpad mismatch for {hat_val}");
        }
    }

    #[cfg(test)]
    mod proptest_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn dualsense_axes_always_in_range(
                lx in 0u8..=255,
                ly in 0u8..=255,
                rx in 0u8..=255,
                ry in 0u8..=255,
                l2 in 0u8..=255,
                r2 in 0u8..=255,
            ) {
                let data = [0x01u8, lx, ly, rx, ry, l2, r2, 0, 0, 0, 8];
                let state = parse_dualsense_report(&data).unwrap();
                prop_assert!((-1.0f32..=1.0).contains(&state.left_x));
                prop_assert!((-1.0f32..=1.0).contains(&state.left_y));
                prop_assert!((-1.0f32..=1.0).contains(&state.right_x));
                prop_assert!((-1.0f32..=1.0).contains(&state.right_y));
                prop_assert!((0.0f32..=1.0).contains(&state.l2));
                prop_assert!((0.0f32..=1.0).contains(&state.r2));
            }

            #[test]
            fn dualsense_any_report_parses(
                data in proptest::collection::vec(any::<u8>(), 11..64usize),
            ) {
                let result = parse_dualsense_report(&data);
                prop_assert!(result.is_ok());
            }
        }
    }
}
