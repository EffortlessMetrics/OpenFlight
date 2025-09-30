//! Bus snapshot structures for comprehensive telemetry model

use crate::types::{
    AircraftId, AutopilotState, BusTypeError, GForce, GearState, Mach, Percentage, SimId,
    ValidatedAngle, ValidatedSpeed,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Complete telemetry snapshot published on the bus
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BusSnapshot {
    /// Simulator identifier
    pub sim: SimId,
    /// Aircraft identifier
    pub aircraft: AircraftId,
    /// Monotonic timestamp in nanoseconds
    pub timestamp: u64,
    /// Flight kinematics data
    pub kinematics: Kinematics,
    /// Aircraft configuration
    pub config: AircraftConfig,
    /// Helicopter-specific data (if applicable)
    pub helo: Option<HeloData>,
    /// Engine data
    pub engines: Vec<EngineData>,
    /// Environmental data
    pub environment: Environment,
    /// Navigation data
    pub navigation: Navigation,
}

/// Flight kinematics and performance data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Kinematics {
    /// Indicated airspeed
    pub ias: ValidatedSpeed,
    /// True airspeed
    pub tas: ValidatedSpeed,
    /// Ground speed
    pub ground_speed: ValidatedSpeed,
    /// Angle of attack
    pub aoa: ValidatedAngle,
    /// Sideslip angle
    pub sideslip: ValidatedAngle,
    /// Bank angle
    pub bank: ValidatedAngle,
    /// Pitch angle
    pub pitch: ValidatedAngle,
    /// Heading (magnetic)
    pub heading: ValidatedAngle,
    /// G-force (vertical)
    pub g_force: GForce,
    /// G-force lateral
    pub g_lateral: GForce,
    /// G-force longitudinal
    pub g_longitudinal: GForce,
    /// Mach number
    pub mach: Mach,
    /// Vertical speed (feet per minute)
    pub vertical_speed: f32,
}

/// Aircraft configuration and systems
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AircraftConfig {
    /// Landing gear state
    pub gear: GearState,
    /// Flaps position (percentage)
    pub flaps: Percentage,
    /// Spoilers position (percentage)
    pub spoilers: Percentage,
    /// Autopilot state
    pub ap_state: AutopilotState,
    /// Autopilot altitude target (feet)
    pub ap_altitude: Option<f32>,
    /// Autopilot heading target (degrees)
    pub ap_heading: Option<ValidatedAngle>,
    /// Autopilot speed target
    pub ap_speed: Option<ValidatedSpeed>,
    /// Lights configuration
    pub lights: LightsConfig,
    /// Fuel quantity (percentage per tank)
    pub fuel: HashMap<String, Percentage>,
}

/// Helicopter-specific telemetry
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeloData {
    /// Main rotor RPM (percentage of nominal)
    pub nr: Percentage,
    /// Power turbine RPM (percentage of nominal)
    pub np: Percentage,
    /// Torque (percentage of maximum)
    pub torque: Percentage,
    /// Collective position (percentage)
    pub collective: Percentage,
    /// Anti-torque pedal position (percentage, -100 to 100)
    pub pedals: f32,
}

/// Engine telemetry data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngineData {
    /// Engine index
    pub index: u8,
    /// Engine running state
    pub running: bool,
    /// RPM (percentage of redline)
    pub rpm: Percentage,
    /// Manifold pressure (inHg)
    pub manifold_pressure: Option<f32>,
    /// Exhaust gas temperature (Celsius)
    pub egt: Option<f32>,
    /// Cylinder head temperature (Celsius)
    pub cht: Option<f32>,
    /// Fuel flow (gallons per hour)
    pub fuel_flow: Option<f32>,
    /// Oil pressure (PSI)
    pub oil_pressure: Option<f32>,
    /// Oil temperature (Celsius)
    pub oil_temperature: Option<f32>,
}

/// Environmental conditions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Environment {
    /// Altitude above sea level (feet)
    pub altitude: f32,
    /// Pressure altitude (feet)
    pub pressure_altitude: f32,
    /// Outside air temperature (Celsius)
    pub oat: f32,
    /// Wind speed
    pub wind_speed: ValidatedSpeed,
    /// Wind direction (degrees)
    pub wind_direction: ValidatedAngle,
    /// Visibility (statute miles)
    pub visibility: f32,
    /// Cloud coverage (percentage)
    pub cloud_coverage: Percentage,
}

/// Navigation and position data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Navigation {
    /// Latitude (degrees)
    pub latitude: f64,
    /// Longitude (degrees)
    pub longitude: f64,
    /// GPS ground track (degrees)
    pub ground_track: ValidatedAngle,
    /// Distance to destination (nautical miles)
    pub distance_to_dest: Option<f32>,
    /// Time to destination (minutes)
    pub time_to_dest: Option<f32>,
    /// Active waypoint identifier
    pub active_waypoint: Option<String>,
}

/// Aircraft lights configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LightsConfig {
    /// Navigation lights
    pub nav: bool,
    /// Beacon light
    pub beacon: bool,
    /// Strobe lights
    pub strobe: bool,
    /// Landing lights
    pub landing: bool,
    /// Taxi lights
    pub taxi: bool,
    /// Logo lights
    pub logo: bool,
    /// Wing lights
    pub wing: bool,
}

impl BusSnapshot {
    /// Create a new snapshot with current timestamp
    pub fn new(sim: SimId, aircraft: AircraftId) -> Self {
        Self {
            sim,
            aircraft,
            timestamp: current_timestamp_ns(),
            kinematics: Kinematics::default(),
            config: AircraftConfig::default(),
            helo: None,
            engines: Vec::new(),
            environment: Environment::default(),
            navigation: Navigation::default(),
        }
    }

    /// Validate all fields in the snapshot
    pub fn validate(&self) -> Result<(), BusTypeError> {
        // Kinematics validation is handled by the typed fields themselves
        // Additional cross-field validation can be added here
        
        // Validate engine indices are unique
        let mut engine_indices = std::collections::HashSet::new();
        for engine in &self.engines {
            if !engine_indices.insert(engine.index) {
                return Err(BusTypeError::InvalidValue {
                    field: "engines".to_string(),
                    reason: format!("Duplicate engine index: {}", engine.index),
                });
            }
        }

        // Validate helicopter data consistency
        if let Some(helo) = &self.helo {
            if helo.pedals < -100.0 || helo.pedals > 100.0 {
                return Err(BusTypeError::OutOfRange {
                    field: "helo.pedals".to_string(),
                    value: helo.pedals,
                    min: -100.0,
                    max: 100.0,
                });
            }
        }

        Ok(())
    }

    /// Get age of snapshot in milliseconds
    pub fn age_ms(&self) -> u64 {
        let now = current_timestamp_ns();
        if now > self.timestamp {
            (now - self.timestamp) / 1_000_000
        } else {
            0
        }
    }
}

impl Default for Kinematics {
    fn default() -> Self {
        Self {
            ias: ValidatedSpeed::new_knots(0.0).unwrap(),
            tas: ValidatedSpeed::new_knots(0.0).unwrap(),
            ground_speed: ValidatedSpeed::new_knots(0.0).unwrap(),
            aoa: ValidatedAngle::new_degrees(0.0).unwrap(),
            sideslip: ValidatedAngle::new_degrees(0.0).unwrap(),
            bank: ValidatedAngle::new_degrees(0.0).unwrap(),
            pitch: ValidatedAngle::new_degrees(0.0).unwrap(),
            heading: ValidatedAngle::new_degrees(0.0).unwrap(),
            g_force: GForce::new(1.0).unwrap(),
            g_lateral: GForce::new(0.0).unwrap(),
            g_longitudinal: GForce::new(0.0).unwrap(),
            mach: Mach::new(0.0).unwrap(),
            vertical_speed: 0.0,
        }
    }
}

impl Default for AircraftConfig {
    fn default() -> Self {
        Self {
            gear: GearState {
                nose: crate::types::GearPosition::Down,
                left: crate::types::GearPosition::Down,
                right: crate::types::GearPosition::Down,
            },
            flaps: Percentage::new(0.0).unwrap(),
            spoilers: Percentage::new(0.0).unwrap(),
            ap_state: AutopilotState::Off,
            ap_altitude: None,
            ap_heading: None,
            ap_speed: None,
            lights: LightsConfig::default(),
            fuel: HashMap::new(),
        }
    }
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            altitude: 0.0,
            pressure_altitude: 0.0,
            oat: 15.0, // Standard temperature
            wind_speed: ValidatedSpeed::new_knots(0.0).unwrap(),
            wind_direction: ValidatedAngle::new_degrees(0.0).unwrap(),
            visibility: 10.0, // Clear visibility
            cloud_coverage: Percentage::new(0.0).unwrap(),
        }
    }
}

impl Default for Navigation {
    fn default() -> Self {
        Self {
            latitude: 0.0,
            longitude: 0.0,
            ground_track: ValidatedAngle::new_degrees(0.0).unwrap(),
            distance_to_dest: None,
            time_to_dest: None,
            active_waypoint: None,
        }
    }
}

impl Default for LightsConfig {
    fn default() -> Self {
        Self {
            nav: false,
            beacon: false,
            strobe: false,
            landing: false,
            taxi: false,
            logo: false,
            wing: false,
        }
    }
}

/// Get current timestamp in nanoseconds since Unix epoch
fn current_timestamp_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use flight_core::units::SpeedUnit;

    #[test]
    fn test_bus_snapshot_creation() {
        let snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
        assert_eq!(snapshot.sim, SimId::Msfs);
        assert_eq!(snapshot.aircraft.icao, "C172");
        assert!(snapshot.timestamp > 0);
    }

    #[test]
    fn test_snapshot_validation() {
        let mut snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
        
        // Valid snapshot should pass
        assert!(snapshot.validate().is_ok());
        
        // Add duplicate engine indices
        snapshot.engines.push(EngineData {
            index: 0,
            running: true,
            rpm: Percentage::new(75.0).unwrap(),
            manifold_pressure: None,
            egt: None,
            cht: None,
            fuel_flow: None,
            oil_pressure: None,
            oil_temperature: None,
        });
        snapshot.engines.push(EngineData {
            index: 0, // Duplicate
            running: false,
            rpm: Percentage::new(0.0).unwrap(),
            manifold_pressure: None,
            egt: None,
            cht: None,
            fuel_flow: None,
            oil_pressure: None,
            oil_temperature: None,
        });
        
        assert!(snapshot.validate().is_err());
    }

    #[test]
    fn test_helo_data_validation() {
        let mut snapshot = BusSnapshot::new(SimId::Dcs, AircraftId::new("UH1H"));
        
        // Valid helicopter data
        snapshot.helo = Some(HeloData {
            nr: Percentage::new(100.0).unwrap(),
            np: Percentage::new(100.0).unwrap(),
            torque: Percentage::new(75.0).unwrap(),
            collective: Percentage::new(50.0).unwrap(),
            pedals: 25.0,
        });
        
        assert!(snapshot.validate().is_ok());
        
        // Invalid pedal position
        snapshot.helo.as_mut().unwrap().pedals = 150.0;
        assert!(snapshot.validate().is_err());
    }

    #[test]
    fn test_snapshot_age() {
        let snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(snapshot.age_ms() >= 10);
    }

    #[test]
    fn test_kinematics_defaults() {
        let kinematics = Kinematics::default();
        assert_eq!(kinematics.ias.value(), 0.0);
        assert_eq!(kinematics.g_force.value(), 1.0);
        assert_eq!(kinematics.mach.value(), 0.0);
    }

    #[test]
    fn test_aircraft_config_defaults() {
        let config = AircraftConfig::default();
        assert!(config.gear.all_down());
        assert_eq!(config.flaps.value(), 0.0);
        assert_eq!(config.ap_state, AutopilotState::Off);
    }
}