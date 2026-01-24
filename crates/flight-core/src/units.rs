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
        mps * 1.94384
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
        meters * 3.28084
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
}
