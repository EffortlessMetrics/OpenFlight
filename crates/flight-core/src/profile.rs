//! Profile management and validation

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Flight profile schema version 1
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub schema: String,
    pub sim: Option<String>,
    pub aircraft: Option<AircraftId>,
    pub axes: HashMap<String, AxisConfig>,
    pub pof_overrides: Option<HashMap<String, PofOverride>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AircraftId {
    pub icao: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxisConfig {
    pub deadzone: Option<f32>,
    pub expo: Option<f32>,
    pub slew_rate: Option<f32>,
    pub detents: Vec<DetentZone>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetentZone {
    pub position: f32,
    pub width: f32,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PofOverride {
    pub axes: Option<HashMap<String, AxisConfig>>,
    pub hysteresis: Option<HashMap<String, HysteresisConfig>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HysteresisConfig {
    pub enter: HashMap<String, f32>,
    pub exit: HashMap<String, f32>,
}

impl Profile {
    /// Validate profile schema and constraints
    pub fn validate(&self) -> crate::Result<()> {
        if self.schema != "flight.profile/1" {
            return Err(crate::FlightError::ProfileValidation(format!(
                "Unsupported schema version: {}",
                self.schema
            )));
        }

        // Validate axis configurations
        for (axis_name, config) in &self.axes {
            self.validate_axis_config(axis_name, config)?;
        }

        Ok(())
    }

    fn validate_axis_config(&self, _name: &str, config: &AxisConfig) -> crate::Result<()> {
        // Validate deadzone range
        if let Some(deadzone) = config.deadzone {
            if !(0.0..=1.0).contains(&deadzone) {
                return Err(crate::FlightError::ProfileValidation(
                    "Deadzone must be between 0.0 and 1.0".to_string(),
                ));
            }
        }

        // Validate expo range
        if let Some(expo) = config.expo {
            if !(-1.0..=1.0).contains(&expo) {
                return Err(crate::FlightError::ProfileValidation(
                    "Expo must be between -1.0 and 1.0".to_string(),
                ));
            }
        }

        Ok(())
    }
}
