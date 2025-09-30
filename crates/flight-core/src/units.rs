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
