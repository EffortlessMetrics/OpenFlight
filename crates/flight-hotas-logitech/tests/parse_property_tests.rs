// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Property-based integration tests for Logitech Extreme 3D Pro HID report parsing.

use flight_hotas_logitech::{
    EXTREME_3D_PRO_MIN_REPORT_BYTES, Extreme3DProHat, parse_extreme_3d_pro,
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
