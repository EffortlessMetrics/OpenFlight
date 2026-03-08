// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Property-based integration tests for Logitech HID report parsing.

use flight_hotas_logitech::{
    EXTREME_3D_PRO_MIN_REPORT_BYTES, Extreme3DProHat, G27_MIN_REPORT_BYTES, G29_MIN_REPORT_BYTES,
    RUDDER_PEDALS_MIN_REPORT_BYTES, X56_STICK_MIN_REPORT_BYTES, X56_THROTTLE_MIN_REPORT_BYTES,
    parse_extreme_3d_pro, parse_g27, parse_g29, parse_rudder_pedals, parse_x56_stick,
    parse_x56_throttle,
};
use proptest::prelude::*;

proptest! {
    /// X, Y, twist axes are always in [-1, 1]; throttle is always in [0, 1].
    #[test]
    fn extreme3dpro_axes_always_bounded(
        data in prop::collection::vec(any::<u8>(), 7..=12),
    ) {
        let s = parse_extreme_3d_pro(&data).unwrap();
        prop_assert!(s.axes.x >= -1.0 && s.axes.x <= 1.0,
            "x out of bounds: {}", s.axes.x);
        prop_assert!(s.axes.y >= -1.0 && s.axes.y <= 1.0,
            "y out of bounds: {}", s.axes.y);
        prop_assert!(s.axes.twist >= -1.0 && s.axes.twist <= 1.0,
            "twist out of bounds: {}", s.axes.twist);
        prop_assert!(s.axes.throttle >= 0.0 && s.axes.throttle <= 1.0,
            "throttle out of bounds: {}", s.axes.throttle);
    }

    /// All axes are finite (never NaN or infinity).
    #[test]
    fn extreme3dpro_axes_never_nan_or_inf(
        data in prop::collection::vec(any::<u8>(), 7..=12),
    ) {
        let s = parse_extreme_3d_pro(&data).unwrap();
        prop_assert!(s.axes.x.is_finite(), "x is not finite: {}", s.axes.x);
        prop_assert!(s.axes.y.is_finite(), "y is not finite: {}", s.axes.y);
        prop_assert!(s.axes.twist.is_finite(), "twist is not finite: {}", s.axes.twist);
        prop_assert!(s.axes.throttle.is_finite(), "throttle is not finite: {}", s.axes.throttle);
    }

    /// The raw button bitmask is always within the 12-bit mask (≤ 0xFFF).
    #[test]
    fn extreme3dpro_buttons_within_12bit_mask(
        data in prop::collection::vec(any::<u8>(), 7..=12),
    ) {
        let s = parse_extreme_3d_pro(&data).unwrap();
        prop_assert!(
            s.buttons.buttons <= 0x0FFF,
            "button mask 0x{:04X} exceeds 12-bit range",
            s.buttons.buttons
        );
    }

    /// Hat always decodes to one of the valid enum variants for every possible nibble value.
    #[test]
    fn extreme3dpro_hat_is_valid(nibble in 0u8..=15) {
        // Build a minimal report with the hat nibble in the correct bit positions:
        // Hat[0] is bit 7 of byte 5; Hat[3:1] are bits 2:0 of byte 6.
        let mut data = [0u8; 7];
        data[5] = (nibble & 0x01) << 7;
        data[6] = (nibble >> 1) & 0x07;
        let s = parse_extreme_3d_pro(&data).unwrap();
        // Ensure the hat is one of the valid variants (any valid Extreme3DProHat).
        let valid = matches!(
            s.buttons.hat,
            Extreme3DProHat::Center
                | Extreme3DProHat::North
                | Extreme3DProHat::NorthEast
                | Extreme3DProHat::East
                | Extreme3DProHat::SouthEast
                | Extreme3DProHat::South
                | Extreme3DProHat::SouthWest
                | Extreme3DProHat::West
                | Extreme3DProHat::NorthWest
        );
        prop_assert!(valid, "unexpected hat value: {:?}", s.buttons.hat);
    }

    /// Reports shorter than EXTREME_3D_PRO_MIN_REPORT_BYTES always return Err.
    #[test]
    fn extreme3dpro_short_report_returns_error(
        len in 0usize..EXTREME_3D_PRO_MIN_REPORT_BYTES,
    ) {
        let data = vec![0u8; len];
        prop_assert!(
            parse_extreme_3d_pro(&data).is_err(),
            "expected Err for {} bytes, got Ok",
            len
        );
    }
}

// ── G27 integration property tests ───────────────────────────────────────────

proptest! {
    /// Wheel is always in [-1, 1]; pedals are always in [0, 1].
    #[test]
    fn g27_axes_always_bounded(
        data in prop::collection::vec(any::<u8>(), 8..=14),
    ) {
        let s = parse_g27(&data).unwrap();
        prop_assert!(s.wheel >= -1.0 && s.wheel <= 1.0, "wheel out of bounds: {}", s.wheel);
        prop_assert!(s.accelerator >= 0.0 && s.accelerator <= 1.0, "acc out of bounds: {}", s.accelerator);
        prop_assert!(s.brake >= 0.0 && s.brake <= 1.0, "brake out of bounds: {}", s.brake);
        prop_assert!(s.clutch >= 0.0 && s.clutch <= 1.0, "clutch out of bounds: {}", s.clutch);
    }

    /// All G27 axes are finite (never NaN or infinity).
    #[test]
    fn g27_axes_never_nan_or_inf(
        data in prop::collection::vec(any::<u8>(), 8..=14),
    ) {
        let s = parse_g27(&data).unwrap();
        prop_assert!(s.wheel.is_finite(), "wheel is not finite: {}", s.wheel);
        prop_assert!(s.accelerator.is_finite(), "acc is not finite: {}", s.accelerator);
        prop_assert!(s.brake.is_finite(), "brake is not finite: {}", s.brake);
        prop_assert!(s.clutch.is_finite(), "clutch is not finite: {}", s.clutch);
    }

    /// The G27 button mask upper 12 bits are always 0.
    #[test]
    fn g27_buttons_within_20bit_mask(
        data in prop::collection::vec(any::<u8>(), 8..=14),
    ) {
        let s = parse_g27(&data).unwrap();
        prop_assert!(
            s.buttons & 0xFFF0_0000 == 0,
            "button mask 0x{:08X} has bits set above bit 19",
            s.buttons
        );
    }

    /// Reports shorter than G27_MIN_REPORT_BYTES always return Err.
    #[test]
    fn g27_short_report_returns_error(len in 0usize..G27_MIN_REPORT_BYTES) {
        let data = vec![0u8; len];
        prop_assert!(
            parse_g27(&data).is_err(),
            "expected Err for {} bytes, got Ok",
            len
        );
    }
}

// ── G29 integration property tests ───────────────────────────────────────────

proptest! {
    /// Wheel is always in [-1, 1]; pedals are always in [0, 1].
    #[test]
    fn g29_axes_always_bounded(
        data in prop::collection::vec(any::<u8>(), 8..=14),
    ) {
        let s = parse_g29(&data).unwrap();
        prop_assert!(s.wheel >= -1.0 && s.wheel <= 1.0, "wheel out of bounds: {}", s.wheel);
        prop_assert!(s.accelerator >= 0.0 && s.accelerator <= 1.0, "acc out of bounds: {}", s.accelerator);
        prop_assert!(s.brake >= 0.0 && s.brake <= 1.0, "brake out of bounds: {}", s.brake);
        prop_assert!(s.clutch >= 0.0 && s.clutch <= 1.0, "clutch out of bounds: {}", s.clutch);
    }

    /// All G29 axes are finite (never NaN or infinity).
    #[test]
    fn g29_axes_never_nan_or_inf(
        data in prop::collection::vec(any::<u8>(), 8..=14),
    ) {
        let s = parse_g29(&data).unwrap();
        prop_assert!(s.wheel.is_finite(), "wheel is not finite: {}", s.wheel);
        prop_assert!(s.accelerator.is_finite(), "acc is not finite: {}", s.accelerator);
        prop_assert!(s.brake.is_finite(), "brake is not finite: {}", s.brake);
        prop_assert!(s.clutch.is_finite(), "clutch is not finite: {}", s.clutch);
    }

    /// Reports shorter than G29_MIN_REPORT_BYTES always return Err.
    #[test]
    fn g29_short_report_returns_error(len in 0usize..G29_MIN_REPORT_BYTES) {
        let data = vec![0u8; len];
        prop_assert!(
            parse_g29(&data).is_err(),
            "expected Err for {} bytes, got Ok",
            len
        );
    }
}

// ── X56 stick integration property tests ─────────────────────────────────────

proptest! {
    /// X56 stick: all axes always in [-1, 1].
    #[test]
    fn x56_stick_axes_always_bounded(
        data in prop::collection::vec(any::<u8>(), 13..=18),
    ) {
        let s = parse_x56_stick(&data).unwrap();
        prop_assert!(s.axes.x >= -1.0 && s.axes.x <= 1.0);
        prop_assert!(s.axes.y >= -1.0 && s.axes.y <= 1.0);
        prop_assert!(s.axes.rz >= -1.0 && s.axes.rz <= 1.0);
        prop_assert!(s.axes.rx >= -1.0 && s.axes.rx <= 1.0);
        prop_assert!(s.axes.ry >= -1.0 && s.axes.ry <= 1.0);
    }

    /// X56 stick: all axes are finite.
    #[test]
    fn x56_stick_axes_never_nan_or_inf(
        data in prop::collection::vec(any::<u8>(), 13..=18),
    ) {
        let s = parse_x56_stick(&data).unwrap();
        prop_assert!(s.axes.x.is_finite());
        prop_assert!(s.axes.y.is_finite());
        prop_assert!(s.axes.rz.is_finite());
        prop_assert!(s.axes.rx.is_finite());
        prop_assert!(s.axes.ry.is_finite());
    }

    /// Reports shorter than X56_STICK_MIN_REPORT_BYTES always return Err.
    #[test]
    fn x56_stick_short_report_returns_error(len in 0usize..X56_STICK_MIN_REPORT_BYTES) {
        let data = vec![0u8; len];
        prop_assert!(parse_x56_stick(&data).is_err());
    }
}

// ── X56 throttle integration property tests ──────────────────────────────────

proptest! {
    /// X56 throttle: all axes always in [0, 1].
    #[test]
    fn x56_throttle_axes_always_bounded(
        data in prop::collection::vec(any::<u8>(), 14..=20),
    ) {
        let s = parse_x56_throttle(&data).unwrap();
        prop_assert!(s.axes.throttle_left >= 0.0 && s.axes.throttle_left <= 1.0);
        prop_assert!(s.axes.throttle_right >= 0.0 && s.axes.throttle_right <= 1.0);
        prop_assert!(s.axes.rotary_left >= 0.0 && s.axes.rotary_left <= 1.0);
        prop_assert!(s.axes.rotary_right >= 0.0 && s.axes.rotary_right <= 1.0);
        prop_assert!(s.axes.slider_left >= 0.0 && s.axes.slider_left <= 1.0);
        prop_assert!(s.axes.slider_right >= 0.0 && s.axes.slider_right <= 1.0);
    }

    /// Reports shorter than X56_THROTTLE_MIN_REPORT_BYTES always return Err.
    #[test]
    fn x56_throttle_short_report_returns_error(len in 0usize..X56_THROTTLE_MIN_REPORT_BYTES) {
        let data = vec![0u8; len];
        prop_assert!(parse_x56_throttle(&data).is_err());
    }
}

// ── Rudder pedals integration property tests ─────────────────────────────────

proptest! {
    /// Rudder is always in [-1, 1]; brakes are always in [0, 1].
    #[test]
    fn rudder_pedals_axes_always_bounded(
        data in prop::collection::vec(any::<u8>(), 5..=10),
    ) {
        let s = parse_rudder_pedals(&data).unwrap();
        prop_assert!(s.axes.rudder >= -1.0 && s.axes.rudder <= 1.0);
        prop_assert!(s.axes.left_brake >= 0.0 && s.axes.left_brake <= 1.0);
        prop_assert!(s.axes.right_brake >= 0.0 && s.axes.right_brake <= 1.0);
    }

    /// All rudder pedal axes are finite.
    #[test]
    fn rudder_pedals_axes_never_nan_or_inf(
        data in prop::collection::vec(any::<u8>(), 5..=10),
    ) {
        let s = parse_rudder_pedals(&data).unwrap();
        prop_assert!(s.axes.rudder.is_finite());
        prop_assert!(s.axes.left_brake.is_finite());
        prop_assert!(s.axes.right_brake.is_finite());
    }

    /// Reports shorter than RUDDER_PEDALS_MIN_REPORT_BYTES always return Err.
    #[test]
    fn rudder_pedals_short_report_returns_error(len in 0usize..RUDDER_PEDALS_MIN_REPORT_BYTES) {
        let data = vec![0u8; len];
        prop_assert!(parse_rudder_pedals(&data).is_err());
    }
}
