// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Unit definitions and conversions

use serde::{Deserialize, Serialize};

/// Unit-safe value wrapper
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct UnitValue<T> {
    pub value: f32,
    pub unit: T,
}

/// Speed units
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SpeedUnit {
    Knots,
    Mps,
    Kph,
}

/// Angle units  
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum AngleUnit {
    Degrees,
    Radians,
}

/// Force units
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ForceUnit {
    Newtons,
    NewtonMeters,
}

pub type Speed = UnitValue<SpeedUnit>;
pub type Angle = UnitValue<AngleUnit>;
pub type Force = UnitValue<ForceUnit>;

/// Angle normalization helpers
pub mod angles {
    /// Normalize degrees to the [-180, 180] range.
    ///
    /// Example: 270 -> -90, -270 -> 90, 360 -> 0
    pub fn normalize_degrees_signed(degrees: f32) -> f32 {
        ((degrees % 360.0) + 540.0) % 360.0 - 180.0
    }

    /// Normalize degrees to the [0, 360) range.
    ///
    /// Example: -90 -> 270, 360 -> 0
    pub fn normalize_degrees_unsigned(degrees: f32) -> f32 {
        ((degrees % 360.0) + 360.0) % 360.0
    }
}

/// Unit conversion utilities
pub mod conversions {
    /// Convert degrees to radians
    pub fn degrees_to_radians(degrees: f32) -> f32 {
        degrees.to_radians()
    }

    /// Convert radians to degrees
    pub fn radians_to_degrees(radians: f32) -> f32 {
        radians.to_degrees()
    }

    /// Convert knots to meters per second
    pub fn knots_to_mps(knots: f32) -> f32 {
        knots * 0.514444
    }

    /// Convert meters per second to knots
    pub fn mps_to_knots(mps: f32) -> f32 {
        mps / 0.514444
    }

    /// Convert kilometers per hour to meters per second
    pub fn kph_to_mps(kph: f32) -> f32 {
        kph * 0.277778
    }

    /// Convert meters per second to kilometers per hour
    pub fn mps_to_kph(mps: f32) -> f32 {
        mps * 3.6
    }

    /// Convert knots to kilometers per hour
    pub fn knots_to_kph(knots: f32) -> f32 {
        mps_to_kph(knots_to_mps(knots))
    }

    /// Convert kilometers per hour to knots
    pub fn kph_to_knots(kph: f32) -> f32 {
        mps_to_knots(kph_to_mps(kph))
    }

    /// Convert feet to meters
    pub fn feet_to_meters(feet: f32) -> f32 {
        feet * 0.3048
    }

    /// Convert meters to feet
    pub fn meters_to_feet(meters: f32) -> f32 {
        meters / 0.3048
    }

    /// Convert feet per minute to meters per second
    pub fn fpm_to_mps(fpm: f32) -> f32 {
        fpm * 0.00508
    }

    /// Convert meters per second to feet per minute
    pub fn mps_to_fpm(mps: f32) -> f32 {
        mps * 196.85
    }
}

#[cfg(test)]
mod tests {
    use super::{angles, conversions};
    use proptest::prelude::*;

    #[test]
    fn test_normalize_degrees_signed() {
        assert!((angles::normalize_degrees_signed(270.0) - (-90.0)).abs() < 0.001);
        assert!((angles::normalize_degrees_signed(-270.0) - 90.0).abs() < 0.001);
        assert!((angles::normalize_degrees_signed(360.0) - 0.0).abs() < 0.001);
        assert!((angles::normalize_degrees_signed(-180.0) - (-180.0)).abs() < 0.001);
    }

    #[test]
    fn test_normalize_degrees_unsigned() {
        assert!((angles::normalize_degrees_unsigned(-90.0) - 270.0).abs() < 0.001);
        assert!((angles::normalize_degrees_unsigned(360.0) - 0.0).abs() < 0.001);
        assert!((angles::normalize_degrees_unsigned(450.0) - 90.0).abs() < 0.001);
    }

    #[test]
    fn test_kph_conversions() {
        let mps = conversions::kph_to_mps(36.0);
        assert!((mps - 10.0).abs() < 0.01);

        let kph = conversions::mps_to_kph(10.0);
        assert!((kph - 36.0).abs() < 0.01);

        let knots = conversions::kph_to_knots(18.52); // ~10 knots
        assert!((knots - 10.0).abs() < 0.05);
    }

    proptest! {
        // Test angle normalization properties
        #[test]
        fn prop_normalize_degrees_signed_range(val in -10000.0f32..10000.0) {
            let normalized = angles::normalize_degrees_signed(val);
            prop_assert!(normalized >= -180.0);
            prop_assert!(normalized <= 180.0);

            // Should be congruent modulo 360
            let diff = (normalized - val) % 360.0;
            let diff = if diff.abs() > 0.001 { diff.abs() } else { 0.0 };
            prop_assert!(diff < 0.001 || (diff - 360.0).abs() < 0.001);
        }

        #[test]
        fn prop_normalize_degrees_unsigned_range(val in -10000.0f32..10000.0) {
            let normalized = angles::normalize_degrees_unsigned(val);
            prop_assert!(normalized >= 0.0);
            prop_assert!(normalized < 360.0);
        }

        // Test round-trip conversion properties
        #[test]
        fn prop_knots_mps_roundtrip(val in 0.0f32..10000.0) {
            let mps = conversions::knots_to_mps(val);
            let back = conversions::mps_to_knots(mps);
            // 0.1% error tolerance for float math
            prop_assert!((val - back).abs() < val * 0.001 + 0.0001);
        }

        #[test]
        fn prop_kph_mps_roundtrip(val in 0.0f32..10000.0) {
            let mps = conversions::kph_to_mps(val);
            let back = conversions::mps_to_kph(mps);
            prop_assert!((val - back).abs() < val * 0.001 + 0.0001);
        }

        #[test]
        fn prop_feet_meters_roundtrip(val in 0.0f32..100000.0) {
            let meters = conversions::feet_to_meters(val);
            let back = conversions::meters_to_feet(meters);
            prop_assert!((val - back).abs() < val * 0.001 + 0.0001);
        }

        #[test]
        fn prop_fpm_mps_roundtrip(val in -100000.0f32..100000.0) {
            let mps = conversions::fpm_to_mps(val);
            let back = conversions::mps_to_fpm(mps);
            prop_assert!((val - back).abs() < val.abs() * 0.001 + 0.0001);
        }

        #[test]
        fn prop_degrees_radians_roundtrip(val in -1000.0f32..1000.0) {
            let rads = conversions::degrees_to_radians(val);
            let back = conversions::radians_to_degrees(rads);
            prop_assert!((val - back).abs() < val.abs() * 0.001 + 0.0001);
        }

        #[test]
        fn prop_knots_kph_roundtrip(val in 0.0f32..10000.0) {
            let kph = conversions::knots_to_kph(val);
            let back = conversions::kph_to_knots(kph);
            prop_assert!((val - back).abs() < val * 0.001 + 0.0001);
        }
    }

    #[test]
    fn test_degrees_radians_known_values() {
        let pi = std::f32::consts::PI;
        assert!((conversions::degrees_to_radians(180.0) - pi).abs() < 0.0001);
        assert!((conversions::radians_to_degrees(pi) - 180.0).abs() < 0.0001);
        assert!((conversions::degrees_to_radians(0.0)).abs() < 0.0001);
    }

    #[test]
    fn test_fpm_mps_known_values() {
        // 196.85 ft/min = 1 m/s (approximately)
        let mps = conversions::fpm_to_mps(196.85);
        assert!((mps - 1.0).abs() < 0.01, "fpm→mps: {mps}");
        let fpm = conversions::mps_to_fpm(1.0);
        assert!((fpm - 196.85).abs() < 0.1, "mps→fpm: {fpm}");
    }

    #[test]
    fn test_knots_to_kph_known_value() {
        // 1 knot ≈ 1.852 km/h
        let kph = conversions::knots_to_kph(1.0);
        assert!((kph - 1.852).abs() < 0.01, "1 kt should be ~1.852 kph, got {kph}");
    }
}
