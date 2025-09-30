//! Unit definitions and conversions

/// Unit-safe value wrapper
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnitValue<T> {
    pub value: f32,
    pub unit: T,
}

/// Speed units
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpeedUnit {
    Knots,
    Mps,
    Kph,
}

/// Angle units  
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AngleUnit {
    Degrees,
    Radians,
}

/// Force units
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ForceUnit {
    Newtons,
    NewtonMeters,
}

pub type Speed = UnitValue<SpeedUnit>;
pub type Angle = UnitValue<AngleUnit>;
pub type Force = UnitValue<ForceUnit>;
