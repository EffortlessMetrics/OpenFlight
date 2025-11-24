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
