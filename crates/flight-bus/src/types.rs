// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Bus telemetry types with comprehensive unit safety and validation

use flight_core::units::{Angle, AngleUnit, Speed, SpeedUnit, conversions};
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Bus type validation errors
#[derive(Error, Debug, Clone, PartialEq)]
pub enum BusTypeError {
    #[error("Value {value} out of range for {field}: expected {min} to {max}")]
    OutOfRange {
        field: String,
        value: f32,
        min: f32,
        max: f32,
    },
    #[error("Invalid unit for {field}: expected {expected}, got {actual}")]
    InvalidUnit {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("Invalid value for {field}: {reason}")]
    InvalidValue { field: String, reason: String },
}

/// Percentage value (0.0 to 100.0)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Percentage(f32);

impl Percentage {
    pub fn new(value: f32) -> Result<Self, BusTypeError> {
        if !(0.0..=100.0).contains(&value) {
            return Err(BusTypeError::OutOfRange {
                field: "percentage".to_string(),
                value,
                min: 0.0,
                max: 100.0,
            });
        }
        Ok(Percentage(value))
    }

    pub fn value(&self) -> f32 {
        self.0
    }

    /// Create from normalized value (0.0 to 1.0)
    pub fn from_normalized(value: f32) -> Result<Self, BusTypeError> {
        if !(0.0..=1.0).contains(&value) {
            return Err(BusTypeError::OutOfRange {
                field: "normalized_percentage".to_string(),
                value,
                min: 0.0,
                max: 1.0,
            });
        }
        Ok(Percentage(value * 100.0))
    }

    /// Get as normalized value (0.0 to 1.0)
    pub fn normalized(&self) -> f32 {
        self.0 / 100.0
    }
}

/// G-force value (-20.0 to 20.0 typical range)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GForce(f32);

impl GForce {
    pub fn new(value: f32) -> Result<Self, BusTypeError> {
        if !(-20.0..=20.0).contains(&value) {
            return Err(BusTypeError::OutOfRange {
                field: "g_force".to_string(),
                value,
                min: -20.0,
                max: 20.0,
            });
        }
        Ok(GForce(value))
    }

    pub fn value(&self) -> f32 {
        self.0
    }
}

/// Mach number (0.0 to 5.0 typical range)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Mach(f32);

impl Mach {
    pub fn new(value: f32) -> Result<Self, BusTypeError> {
        if !(0.0..=5.0).contains(&value) {
            return Err(BusTypeError::OutOfRange {
                field: "mach".to_string(),
                value,
                min: 0.0,
                max: 5.0,
            });
        }
        Ok(Mach(value))
    }

    pub fn value(&self) -> f32 {
        self.0
    }
}

/// Validated speed with unit checking
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ValidatedSpeed {
    pub speed: Speed,
}

impl ValidatedSpeed {
    pub fn new_knots(value: f32) -> Result<Self, BusTypeError> {
        if !(0.0..=1000.0).contains(&value) {
            return Err(BusTypeError::OutOfRange {
                field: "speed_knots".to_string(),
                value,
                min: 0.0,
                max: 1000.0,
            });
        }
        Ok(ValidatedSpeed {
            speed: Speed {
                value,
                unit: SpeedUnit::Knots,
            },
        })
    }

    pub fn new_mps(value: f32) -> Result<Self, BusTypeError> {
        if !(0.0..=500.0).contains(&value) {
            return Err(BusTypeError::OutOfRange {
                field: "speed_mps".to_string(),
                value,
                min: 0.0,
                max: 500.0,
            });
        }
        Ok(ValidatedSpeed {
            speed: Speed {
                value,
                unit: SpeedUnit::Mps,
            },
        })
    }

    pub fn value(&self) -> f32 {
        self.speed.value
    }

    pub fn unit(&self) -> SpeedUnit {
        self.speed.unit
    }

    /// Convert to knots
    pub fn to_knots(&self) -> f32 {
        match self.speed.unit {
            SpeedUnit::Knots => self.speed.value,
            SpeedUnit::Mps => conversions::mps_to_knots(self.speed.value),
            SpeedUnit::Kph => conversions::kph_to_knots(self.speed.value),
        }
    }

    /// Convert to meters per second
    pub fn to_mps(&self) -> f32 {
        match self.speed.unit {
            SpeedUnit::Knots => conversions::knots_to_mps(self.speed.value),
            SpeedUnit::Mps => self.speed.value,
            SpeedUnit::Kph => conversions::kph_to_mps(self.speed.value),
        }
    }
}

/// Validated angle with unit checking
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ValidatedAngle {
    pub angle: Angle,
}

impl ValidatedAngle {
    pub fn new_degrees(value: f32) -> Result<Self, BusTypeError> {
        if !(-180.0..=180.0).contains(&value) {
            return Err(BusTypeError::OutOfRange {
                field: "angle_degrees".to_string(),
                value,
                min: -180.0,
                max: 180.0,
            });
        }
        Ok(ValidatedAngle {
            angle: Angle {
                value,
                unit: AngleUnit::Degrees,
            },
        })
    }

    pub fn new_radians(value: f32) -> Result<Self, BusTypeError> {
        if !(-std::f32::consts::PI..=std::f32::consts::PI).contains(&value) {
            return Err(BusTypeError::OutOfRange {
                field: "angle_radians".to_string(),
                value,
                min: -std::f32::consts::PI,
                max: std::f32::consts::PI,
            });
        }
        Ok(ValidatedAngle {
            angle: Angle {
                value,
                unit: AngleUnit::Radians,
            },
        })
    }

    pub fn value(&self) -> f32 {
        self.angle.value
    }

    pub fn unit(&self) -> AngleUnit {
        self.angle.unit
    }

    /// Convert to degrees
    pub fn to_degrees(&self) -> f32 {
        match self.angle.unit {
            AngleUnit::Degrees => self.angle.value,
            AngleUnit::Radians => self.angle.value.to_degrees(),
        }
    }

    /// Convert to radians
    pub fn to_radians(&self) -> f32 {
        match self.angle.unit {
            AngleUnit::Degrees => self.angle.value.to_radians(),
            AngleUnit::Radians => self.angle.value,
        }
    }
}

/// Simulator identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SimId {
    Msfs,
    Msfs2024,
    XPlane,
    Dcs,
    AceCombat7,
    WarThunder,
    EliteDangerous,
    Ksp,
    Wingman,
    Unknown,
}

impl fmt::Display for SimId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SimId::Msfs => write!(f, "MSFS"),
            SimId::Msfs2024 => write!(f, "MSFS 2024"),
            SimId::XPlane => write!(f, "X-Plane"),
            SimId::Dcs => write!(f, "DCS"),
            SimId::AceCombat7 => write!(f, "Ace Combat 7"),
            SimId::WarThunder => write!(f, "War Thunder"),
            SimId::EliteDangerous => write!(f, "Elite: Dangerous"),
            SimId::Ksp => write!(f, "Kerbal Space Program"),
            SimId::Wingman => write!(f, "Project Wingman"),
            SimId::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Aircraft identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AircraftId {
    pub icao: String,
    pub variant: Option<String>,
}

impl AircraftId {
    pub fn new(icao: impl Into<String>) -> Self {
        Self {
            icao: icao.into(),
            variant: None,
        }
    }

    pub fn with_variant(icao: impl Into<String>, variant: impl Into<String>) -> Self {
        Self {
            icao: icao.into(),
            variant: Some(variant.into()),
        }
    }
}

impl fmt::Display for AircraftId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.variant {
            Some(variant) => write!(f, "{}-{}", self.icao, variant),
            None => write!(f, "{}", self.icao),
        }
    }
}

/// Autopilot state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AutopilotState {
    Off,
    Armed,
    Engaged,
    Failed,
}

/// Gear state with individual gear positions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GearState {
    pub nose: GearPosition,
    pub left: GearPosition,
    pub right: GearPosition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GearPosition {
    Up,
    Down,
    Transitioning,
    Unknown,
}

impl GearState {
    pub fn all_down(&self) -> bool {
        matches!(
            (self.nose, self.left, self.right),
            (GearPosition::Down, GearPosition::Down, GearPosition::Down)
        )
    }

    pub fn all_up(&self) -> bool {
        matches!(
            (self.nose, self.left, self.right),
            (GearPosition::Up, GearPosition::Up, GearPosition::Up)
        )
    }

    pub fn transitioning(&self) -> bool {
        matches!(self.nose, GearPosition::Transitioning)
            || matches!(self.left, GearPosition::Transitioning)
            || matches!(self.right, GearPosition::Transitioning)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_percentage_validation() {
        assert!(Percentage::new(50.0).is_ok());
        assert!(Percentage::new(0.0).is_ok());
        assert!(Percentage::new(100.0).is_ok());
        assert!(Percentage::new(-1.0).is_err());
        assert!(Percentage::new(101.0).is_err());
    }

    #[test]
    fn test_percentage_normalized() {
        let p = Percentage::from_normalized(0.5).unwrap();
        assert_eq!(p.value(), 50.0);
        assert_eq!(p.normalized(), 0.5);
    }

    #[test]
    fn test_g_force_validation() {
        assert!(GForce::new(1.0).is_ok());
        assert!(GForce::new(-5.0).is_ok());
        assert!(GForce::new(20.0).is_ok());
        assert!(GForce::new(-20.0).is_ok());
        assert!(GForce::new(21.0).is_err());
        assert!(GForce::new(-21.0).is_err());
    }

    #[test]
    fn test_mach_validation() {
        assert!(Mach::new(0.8).is_ok());
        assert!(Mach::new(0.0).is_ok());
        assert!(Mach::new(5.0).is_ok());
        assert!(Mach::new(-0.1).is_err());
        assert!(Mach::new(5.1).is_err());
    }

    #[test]
    fn test_validated_speed() {
        let speed = ValidatedSpeed::new_knots(150.0).unwrap();
        assert_eq!(speed.value(), 150.0);
        assert_eq!(speed.unit(), SpeedUnit::Knots);
        assert!((speed.to_knots() - 150.0).abs() < 0.001);

        assert!(ValidatedSpeed::new_knots(-1.0).is_err());
        assert!(ValidatedSpeed::new_knots(1001.0).is_err());
    }

    #[test]
    fn test_validated_angle() {
        let angle = ValidatedAngle::new_degrees(45.0).unwrap();
        assert_eq!(angle.value(), 45.0);
        assert_eq!(angle.unit(), AngleUnit::Degrees);
        assert!((angle.to_degrees() - 45.0).abs() < 0.001);

        assert!(ValidatedAngle::new_degrees(-181.0).is_err());
        assert!(ValidatedAngle::new_degrees(181.0).is_err());
    }

    #[test]
    fn test_gear_state() {
        let gear = GearState {
            nose: GearPosition::Down,
            left: GearPosition::Down,
            right: GearPosition::Down,
        };
        assert!(gear.all_down());
        assert!(!gear.all_up());
        assert!(!gear.transitioning());

        let gear = GearState {
            nose: GearPosition::Up,
            left: GearPosition::Transitioning,
            right: GearPosition::Up,
        };
        assert!(!gear.all_down());
        assert!(!gear.all_up());
        assert!(gear.transitioning());
    }

    #[test]
    fn test_aircraft_id_display() {
        let aircraft = AircraftId::new("C172");
        assert_eq!(aircraft.to_string(), "C172");

        let aircraft = AircraftId::with_variant("A320", "NEO");
        assert_eq!(aircraft.to_string(), "A320-NEO");
    }
}
