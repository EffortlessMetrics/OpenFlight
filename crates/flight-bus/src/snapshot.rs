// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Bus snapshot structures for comprehensive telemetry model

use crate::types::{
    AircraftId, AutopilotState, BusTypeError, GForce, GearState, Mach, Percentage, SimId,
    ValidatedAngle, ValidatedSpeed,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Angular rates in body frame (rad/s)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AngularRates {
    /// Roll rate (rad/s)
    pub p: f32,
    /// Pitch rate (rad/s)
    pub q: f32,
    /// Yaw rate (rad/s)
    pub r: f32,
}

/// Control inputs (normalized -1.0 to 1.0 or 0.0 to 1.0)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ControlInputs {
    /// Pitch control (-1.0 to 1.0, negative = nose down)
    pub pitch: f32,
    /// Roll control (-1.0 to 1.0, negative = left roll)
    pub roll: f32,
    /// Yaw control (-1.0 to 1.0, negative = left yaw)
    pub yaw: f32,
    /// Throttle (0.0 to 1.0 per engine)
    pub throttle: Vec<f32>,
}

/// Trim state (normalized -1.0 to 1.0)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TrimState {
    /// Elevator trim (-1.0 to 1.0)
    pub elevator: f32,
    /// Aileron trim (-1.0 to 1.0)
    pub aileron: f32,
    /// Rudder trim (-1.0 to 1.0)
    pub rudder: f32,
}

/// Validity flags for telemetry data
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct ValidityFlags {
    /// Safe for force feedback output
    pub safe_for_ffb: bool,
    /// Attitude data is valid
    pub attitude_valid: bool,
    /// Angular rates are valid
    pub angular_rates_valid: bool,
    /// Velocities are valid
    pub velocities_valid: bool,
    /// Kinematics (g-loads) are valid
    pub kinematics_valid: bool,
    /// Aerodynamics (AoA, sideslip) are valid
    pub aero_valid: bool,
    /// Position data is valid
    pub position_valid: bool,
}

/// Complete telemetry snapshot published on the bus
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BusSnapshot {
    /// Simulator identifier
    pub sim: SimId,
    /// Aircraft identifier
    pub aircraft: AircraftId,
    /// Monotonic timestamp in nanoseconds since process start
    pub timestamp: u64,
    /// Flight kinematics data
    pub kinematics: Kinematics,
    /// Angular rates (body frame)
    pub angular_rates: AngularRates,
    /// Aircraft configuration
    pub config: AircraftConfig,
    /// Control inputs
    pub control_inputs: ControlInputs,
    /// Trim state
    pub trim_state: TrimState,
    /// Helicopter-specific data (if applicable)
    pub helo: Option<HeloData>,
    /// Engine data
    pub engines: Vec<EngineData>,
    /// Environmental data
    pub environment: Environment,
    /// Navigation data
    pub navigation: Navigation,
    /// Validity flags
    pub validity: ValidityFlags,
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
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

impl Default for BusSnapshot {
    fn default() -> Self {
        Self {
            sim: SimId::Unknown,
            aircraft: AircraftId::new("unknown"),
            timestamp: 0,
            kinematics: Kinematics::default(),
            angular_rates: AngularRates::default(),
            config: AircraftConfig::default(),
            control_inputs: ControlInputs::default(),
            trim_state: TrimState::default(),
            helo: None,
            engines: Vec::new(),
            environment: Environment::default(),
            navigation: Navigation::default(),
            validity: ValidityFlags::default(),
        }
    }
}

impl BusSnapshot {
    /// Create a new snapshot with current timestamp
    pub fn new(sim: SimId, aircraft: AircraftId) -> Self {
        Self {
            sim,
            aircraft,
            timestamp: current_timestamp_ns(),
            kinematics: Kinematics::default(),
            angular_rates: AngularRates::default(),
            config: AircraftConfig::default(),
            control_inputs: ControlInputs::default(),
            trim_state: TrimState::default(),
            helo: None,
            engines: Vec::new(),
            environment: Environment::default(),
            navigation: Navigation::default(),
            validity: ValidityFlags::default(),
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
        if let Some(helo) = &self.helo
            && (helo.pedals < -100.0 || helo.pedals > 100.0)
        {
            return Err(BusTypeError::OutOfRange {
                field: "helo.pedals".to_string(),
                value: helo.pedals,
                min: -100.0,
                max: 100.0,
            });
        }

        // Validate control inputs are in valid ranges
        if self.control_inputs.pitch < -1.0 || self.control_inputs.pitch > 1.0 {
            return Err(BusTypeError::OutOfRange {
                field: "control_inputs.pitch".to_string(),
                value: self.control_inputs.pitch,
                min: -1.0,
                max: 1.0,
            });
        }
        if self.control_inputs.roll < -1.0 || self.control_inputs.roll > 1.0 {
            return Err(BusTypeError::OutOfRange {
                field: "control_inputs.roll".to_string(),
                value: self.control_inputs.roll,
                min: -1.0,
                max: 1.0,
            });
        }
        if self.control_inputs.yaw < -1.0 || self.control_inputs.yaw > 1.0 {
            return Err(BusTypeError::OutOfRange {
                field: "control_inputs.yaw".to_string(),
                value: self.control_inputs.yaw,
                min: -1.0,
                max: 1.0,
            });
        }
        for (idx, throttle) in self.control_inputs.throttle.iter().enumerate() {
            if *throttle < 0.0 || *throttle > 1.0 {
                return Err(BusTypeError::OutOfRange {
                    field: format!("control_inputs.throttle[{}]", idx),
                    value: *throttle,
                    min: 0.0,
                    max: 1.0,
                });
            }
        }

        // Validate trim state is in valid ranges
        if self.trim_state.elevator < -1.0 || self.trim_state.elevator > 1.0 {
            return Err(BusTypeError::OutOfRange {
                field: "trim_state.elevator".to_string(),
                value: self.trim_state.elevator,
                min: -1.0,
                max: 1.0,
            });
        }
        if self.trim_state.aileron < -1.0 || self.trim_state.aileron > 1.0 {
            return Err(BusTypeError::OutOfRange {
                field: "trim_state.aileron".to_string(),
                value: self.trim_state.aileron,
                min: -1.0,
                max: 1.0,
            });
        }
        if self.trim_state.rudder < -1.0 || self.trim_state.rudder > 1.0 {
            return Err(BusTypeError::OutOfRange {
                field: "trim_state.rudder".to_string(),
                value: self.trim_state.rudder,
                min: -1.0,
                max: 1.0,
            });
        }

        // Validate angular rates are reasonable (not NaN or Inf)
        if !self.angular_rates.p.is_finite() {
            return Err(BusTypeError::InvalidValue {
                field: "angular_rates.p".to_string(),
                reason: "Value is not finite".to_string(),
            });
        }
        if !self.angular_rates.q.is_finite() {
            return Err(BusTypeError::InvalidValue {
                field: "angular_rates.q".to_string(),
                reason: "Value is not finite".to_string(),
            });
        }
        if !self.angular_rates.r.is_finite() {
            return Err(BusTypeError::InvalidValue {
                field: "angular_rates.r".to_string(),
                reason: "Value is not finite".to_string(),
            });
        }

        // Validate environment fields are reasonable
        if !self.environment.altitude.is_finite() {
            return Err(BusTypeError::InvalidValue {
                field: "environment.altitude".to_string(),
                reason: "Value is not finite".to_string(),
            });
        }
        if !self.environment.oat.is_finite() {
            return Err(BusTypeError::InvalidValue {
                field: "environment.oat".to_string(),
                reason: "Value is not finite".to_string(),
            });
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

impl Default for AngularRates {
    fn default() -> Self {
        Self {
            p: 0.0,
            q: 0.0,
            r: 0.0,
        }
    }
}

impl Default for ControlInputs {
    fn default() -> Self {
        Self {
            pitch: 0.0,
            roll: 0.0,
            yaw: 0.0,
            throttle: Vec::new(),
        }
    }
}

impl Default for TrimState {
    fn default() -> Self {
        Self {
            elevator: 0.0,
            aileron: 0.0,
            rudder: 0.0,
        }
    }
}

/// Get current monotonic timestamp in nanoseconds since process start
fn current_timestamp_ns() -> u64 {
    // Standard library doesn't expose raw monotonic ticks easily, so we use Instant
    // This is relative to an arbitrary point, but consistent within a run
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let start = START.get_or_init(std::time::Instant::now);
    std::time::Instant::now().duration_since(*start).as_nanos() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use flight_core::units::{SpeedUnit, conversions};

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

    // Core field validation tests
    #[test]
    fn test_validated_speed_construction() {
        // Valid speeds
        assert!(ValidatedSpeed::new_knots(150.0).is_ok());
        assert!(ValidatedSpeed::new_mps(77.2).is_ok());

        // Out of range speeds
        assert!(ValidatedSpeed::new_knots(-1.0).is_err());
        assert!(ValidatedSpeed::new_knots(1001.0).is_err());
        assert!(ValidatedSpeed::new_mps(-1.0).is_err());
        assert!(ValidatedSpeed::new_mps(501.0).is_err());
    }

    #[test]
    fn test_validated_angle_construction() {
        // Valid angles
        assert!(ValidatedAngle::new_degrees(45.0).is_ok());
        assert!(ValidatedAngle::new_radians(0.785).is_ok());

        // Out of range angles
        assert!(ValidatedAngle::new_degrees(-181.0).is_err());
        assert!(ValidatedAngle::new_degrees(181.0).is_err());
        assert!(ValidatedAngle::new_radians(-3.15).is_err());
        assert!(ValidatedAngle::new_radians(3.15).is_err());
    }

    #[test]
    fn test_g_force_validation() {
        // Valid g-forces
        assert!(GForce::new(1.0).is_ok());
        assert!(GForce::new(-5.0).is_ok());
        assert!(GForce::new(10.0).is_ok());

        // Out of range g-forces
        assert!(GForce::new(-21.0).is_err());
        assert!(GForce::new(21.0).is_err());
    }

    #[test]
    fn test_mach_validation() {
        // Valid Mach numbers
        assert!(Mach::new(0.0).is_ok());
        assert!(Mach::new(0.85).is_ok());
        assert!(Mach::new(2.5).is_ok());

        // Out of range Mach numbers
        assert!(Mach::new(-0.1).is_err());
        assert!(Mach::new(5.1).is_err());
    }

    // Unit conversion tests
    #[test]
    fn test_degrees_to_radians_conversion() {
        let angle = ValidatedAngle::new_degrees(180.0).unwrap();
        let radians = angle.to_radians();
        assert!((radians - std::f32::consts::PI).abs() < 0.001);

        let angle = ValidatedAngle::new_degrees(90.0).unwrap();
        let radians = angle.to_radians();
        assert!((radians - std::f32::consts::FRAC_PI_2).abs() < 0.001);
    }

    #[test]
    fn test_radians_to_degrees_conversion() {
        let angle = ValidatedAngle::new_radians(std::f32::consts::PI).unwrap();
        let degrees = angle.to_degrees();
        assert!((degrees - 180.0).abs() < 0.001);

        let angle = ValidatedAngle::new_radians(std::f32::consts::FRAC_PI_2).unwrap();
        let degrees = angle.to_degrees();
        assert!((degrees - 90.0).abs() < 0.001);
    }

    #[test]
    fn test_knots_to_mps_conversion() {
        let speed = ValidatedSpeed::new_knots(100.0).unwrap();
        let mps = speed.to_mps();
        assert!((mps - 51.4444).abs() < 0.001);

        // Test conversion utility
        let mps = conversions::knots_to_mps(100.0);
        assert!((mps - 51.4444).abs() < 0.001);
    }

    #[test]
    fn test_mps_to_knots_conversion() {
        let speed = ValidatedSpeed::new_mps(50.0).unwrap();
        let knots = speed.to_knots();
        assert!((knots - 97.192).abs() < 0.01);

        // Test conversion utility
        let knots = conversions::mps_to_knots(50.0);
        assert!((knots - 97.192).abs() < 0.01);
    }

    #[test]
    fn test_feet_to_meters_conversion() {
        let meters = conversions::feet_to_meters(1000.0);
        assert!((meters - 304.8).abs() < 0.1);
    }

    #[test]
    fn test_meters_to_feet_conversion() {
        let feet = conversions::meters_to_feet(304.8);
        assert!((feet - 1000.0).abs() < 0.1);
    }

    #[test]
    fn test_fpm_to_mps_conversion() {
        let mps = conversions::fpm_to_mps(1000.0);
        assert!((mps - 5.08).abs() < 0.01);
    }

    #[test]
    fn test_mps_to_fpm_conversion() {
        let fpm = conversions::mps_to_fpm(5.08);
        assert!((fpm - 1000.0).abs() < 1.0);
    }

    // Snapshot age calculation tests
    #[test]
    fn test_snapshot_age_calculation() {
        let snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));

        // Age should be very small immediately after creation
        assert!(snapshot.age_ms() < 10);

        // Wait and check age increases
        std::thread::sleep(std::time::Duration::from_millis(50));
        let age = snapshot.age_ms();
        assert!((50..100).contains(&age));
    }

    // Core field range validation tests
    #[test]
    fn test_attitude_field_validation() {
        let mut snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));

        // Valid attitude values
        snapshot.kinematics.pitch = ValidatedAngle::new_degrees(10.0).unwrap();
        snapshot.kinematics.bank = ValidatedAngle::new_degrees(-15.0).unwrap();
        snapshot.kinematics.heading = ValidatedAngle::new_degrees(90.0).unwrap();
        assert!(snapshot.validate().is_ok());
    }

    #[test]
    fn test_velocity_field_validation() {
        let mut snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));

        // Valid velocity values
        snapshot.kinematics.ias = ValidatedSpeed::new_knots(120.0).unwrap();
        snapshot.kinematics.tas = ValidatedSpeed::new_knots(130.0).unwrap();
        snapshot.kinematics.ground_speed = ValidatedSpeed::new_knots(125.0).unwrap();
        assert!(snapshot.validate().is_ok());
    }

    #[test]
    fn test_g_load_field_validation() {
        let mut snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));

        // Valid g-load values
        snapshot.kinematics.g_force = GForce::new(2.5).unwrap();
        snapshot.kinematics.g_lateral = GForce::new(-0.5).unwrap();
        snapshot.kinematics.g_longitudinal = GForce::new(0.2).unwrap();
        assert!(snapshot.validate().is_ok());
    }

    #[test]
    fn test_angular_rates_defaults() {
        let rates = AngularRates::default();
        assert_eq!(rates.p, 0.0);
        assert_eq!(rates.q, 0.0);
        assert_eq!(rates.r, 0.0);
    }

    #[test]
    fn test_control_inputs_defaults() {
        let controls = ControlInputs::default();
        assert_eq!(controls.pitch, 0.0);
        assert_eq!(controls.roll, 0.0);
        assert_eq!(controls.yaw, 0.0);
        assert!(controls.throttle.is_empty());
    }

    #[test]
    fn test_trim_state_defaults() {
        let trim = TrimState::default();
        assert_eq!(trim.elevator, 0.0);
        assert_eq!(trim.aileron, 0.0);
        assert_eq!(trim.rudder, 0.0);
    }

    #[test]
    fn test_validity_flags_defaults() {
        let validity = ValidityFlags::default();
        assert!(!validity.safe_for_ffb);
        assert!(!validity.attitude_valid);
        assert!(!validity.angular_rates_valid);
        assert!(!validity.velocities_valid);
        assert!(!validity.kinematics_valid);
        assert!(!validity.aero_valid);
        assert!(!validity.position_valid);
    }

    // Extended field validation tests
    #[test]
    fn test_unique_engine_indices_validation() {
        let mut snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("B737"));

        // Add engines with unique indices
        snapshot.engines.push(EngineData {
            index: 0,
            running: true,
            rpm: Percentage::new(75.0).unwrap(),
            manifold_pressure: Some(29.92),
            egt: Some(650.0),
            cht: Some(380.0),
            fuel_flow: Some(12.5),
            oil_pressure: Some(55.0),
            oil_temperature: Some(85.0),
        });
        snapshot.engines.push(EngineData {
            index: 1,
            running: true,
            rpm: Percentage::new(75.0).unwrap(),
            manifold_pressure: Some(29.92),
            egt: Some(650.0),
            cht: Some(380.0),
            fuel_flow: Some(12.5),
            oil_pressure: Some(55.0),
            oil_temperature: Some(85.0),
        });

        // Should pass with unique indices
        assert!(snapshot.validate().is_ok());

        // Add duplicate engine index
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

        // Should fail with duplicate indices
        assert!(snapshot.validate().is_err());
    }

    #[test]
    fn test_helicopter_pedal_range_validation() {
        let mut snapshot = BusSnapshot::new(SimId::Dcs, AircraftId::new("UH1H"));

        // Valid pedal positions
        snapshot.helo = Some(HeloData {
            nr: Percentage::new(100.0).unwrap(),
            np: Percentage::new(100.0).unwrap(),
            torque: Percentage::new(75.0).unwrap(),
            collective: Percentage::new(50.0).unwrap(),
            pedals: -100.0,
        });
        assert!(snapshot.validate().is_ok());

        snapshot.helo.as_mut().unwrap().pedals = 100.0;
        assert!(snapshot.validate().is_ok());

        snapshot.helo.as_mut().unwrap().pedals = 0.0;
        assert!(snapshot.validate().is_ok());

        // Invalid pedal positions
        snapshot.helo.as_mut().unwrap().pedals = -100.1;
        assert!(snapshot.validate().is_err());

        snapshot.helo.as_mut().unwrap().pedals = 100.1;
        assert!(snapshot.validate().is_err());
    }

    #[test]
    fn test_control_inputs_range_validation() {
        let mut snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));

        // Valid control inputs
        snapshot.control_inputs.pitch = 0.5;
        snapshot.control_inputs.roll = -0.3;
        snapshot.control_inputs.yaw = 0.1;
        snapshot.control_inputs.throttle = vec![0.75];
        assert!(snapshot.validate().is_ok());

        // Invalid pitch
        snapshot.control_inputs.pitch = 1.1;
        assert!(snapshot.validate().is_err());
        snapshot.control_inputs.pitch = 0.0;

        // Invalid roll
        snapshot.control_inputs.roll = -1.1;
        assert!(snapshot.validate().is_err());
        snapshot.control_inputs.roll = 0.0;

        // Invalid yaw
        snapshot.control_inputs.yaw = 1.5;
        assert!(snapshot.validate().is_err());
        snapshot.control_inputs.yaw = 0.0;

        // Invalid throttle
        snapshot.control_inputs.throttle = vec![1.1];
        assert!(snapshot.validate().is_err());
        snapshot.control_inputs.throttle = vec![-0.1];
        assert!(snapshot.validate().is_err());
    }

    #[test]
    fn test_trim_state_range_validation() {
        let mut snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));

        // Valid trim state
        snapshot.trim_state.elevator = 0.2;
        snapshot.trim_state.aileron = -0.1;
        snapshot.trim_state.rudder = 0.05;
        assert!(snapshot.validate().is_ok());

        // Invalid elevator trim
        snapshot.trim_state.elevator = 1.1;
        assert!(snapshot.validate().is_err());
        snapshot.trim_state.elevator = 0.0;

        // Invalid aileron trim
        snapshot.trim_state.aileron = -1.1;
        assert!(snapshot.validate().is_err());
        snapshot.trim_state.aileron = 0.0;

        // Invalid rudder trim
        snapshot.trim_state.rudder = 1.5;
        assert!(snapshot.validate().is_err());
    }

    #[test]
    fn test_angular_rates_finite_validation() {
        let mut snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));

        // Valid angular rates
        snapshot.angular_rates.p = 0.1;
        snapshot.angular_rates.q = -0.05;
        snapshot.angular_rates.r = 0.02;
        assert!(snapshot.validate().is_ok());

        // Invalid angular rates (NaN)
        snapshot.angular_rates.p = f32::NAN;
        assert!(snapshot.validate().is_err());
        snapshot.angular_rates.p = 0.0;

        // Invalid angular rates (Inf)
        snapshot.angular_rates.q = f32::INFINITY;
        assert!(snapshot.validate().is_err());
        snapshot.angular_rates.q = 0.0;

        snapshot.angular_rates.r = f32::NEG_INFINITY;
        assert!(snapshot.validate().is_err());
    }

    #[test]
    fn test_environment_finite_validation() {
        let mut snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));

        // Valid environment
        snapshot.environment.altitude = 5000.0;
        snapshot.environment.oat = 15.0;
        assert!(snapshot.validate().is_ok());

        // Invalid altitude (NaN)
        snapshot.environment.altitude = f32::NAN;
        assert!(snapshot.validate().is_err());
        snapshot.environment.altitude = 5000.0;

        // Invalid OAT (Inf)
        snapshot.environment.oat = f32::INFINITY;
        assert!(snapshot.validate().is_err());
    }

    #[test]
    fn test_extended_fields_present() {
        let snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));

        // Verify all extended fields are present
        assert!(snapshot.engines.is_empty()); // Empty but present
        assert!(snapshot.config.fuel.is_empty()); // Empty but present
        assert!(snapshot.helo.is_none()); // Optional
        assert_eq!(snapshot.environment.altitude, 0.0);
        assert_eq!(snapshot.navigation.latitude, 0.0);
        assert_eq!(snapshot.config.ap_state, AutopilotState::Off);
        assert!(!snapshot.config.lights.nav);
    }

    #[test]
    fn test_engine_data_fields() {
        let engine = EngineData {
            index: 0,
            running: true,
            rpm: Percentage::new(75.0).unwrap(),
            manifold_pressure: Some(29.92),
            egt: Some(650.0),
            cht: Some(380.0),
            fuel_flow: Some(12.5),
            oil_pressure: Some(55.0),
            oil_temperature: Some(85.0),
        };

        assert_eq!(engine.index, 0);
        assert!(engine.running);
        assert_eq!(engine.rpm.value(), 75.0);
        assert_eq!(engine.manifold_pressure, Some(29.92));
        assert_eq!(engine.egt, Some(650.0));
        assert_eq!(engine.cht, Some(380.0));
        assert_eq!(engine.fuel_flow, Some(12.5));
        assert_eq!(engine.oil_pressure, Some(55.0));
        assert_eq!(engine.oil_temperature, Some(85.0));
    }

    #[test]
    fn test_helicopter_data_fields() {
        let helo = HeloData {
            nr: Percentage::new(100.0).unwrap(),
            np: Percentage::new(95.0).unwrap(),
            torque: Percentage::new(75.0).unwrap(),
            collective: Percentage::new(50.0).unwrap(),
            pedals: 25.0,
        };

        assert_eq!(helo.nr.value(), 100.0);
        assert_eq!(helo.np.value(), 95.0);
        assert_eq!(helo.torque.value(), 75.0);
        assert_eq!(helo.collective.value(), 50.0);
        assert_eq!(helo.pedals, 25.0);
    }

    #[test]
    fn test_environment_fields() {
        let env = Environment {
            altitude: 5000.0,
            pressure_altitude: 5200.0,
            oat: 10.0,
            wind_speed: ValidatedSpeed::new_knots(15.0).unwrap(),
            wind_direction: ValidatedAngle::new_degrees(90.0).unwrap(),
            visibility: 10.0,
            cloud_coverage: Percentage::new(25.0).unwrap(),
        };

        assert_eq!(env.altitude, 5000.0);
        assert_eq!(env.pressure_altitude, 5200.0);
        assert_eq!(env.oat, 10.0);
        assert_eq!(env.wind_speed.to_knots(), 15.0);
        assert_eq!(env.wind_direction.to_degrees(), 90.0);
        assert_eq!(env.visibility, 10.0);
        assert_eq!(env.cloud_coverage.value(), 25.0);
    }

    #[test]
    fn test_navigation_fields() {
        let nav = Navigation {
            latitude: 47.6062,
            longitude: -122.3321,
            ground_track: ValidatedAngle::new_degrees(90.0).unwrap(),
            distance_to_dest: Some(125.5),
            time_to_dest: Some(45.0),
            active_waypoint: Some("KSEA".to_string()),
        };

        assert_eq!(nav.latitude, 47.6062);
        assert_eq!(nav.longitude, -122.3321);
        assert_eq!(nav.ground_track.to_degrees(), 90.0);
        assert_eq!(nav.distance_to_dest, Some(125.5));
        assert_eq!(nav.time_to_dest, Some(45.0));
        assert_eq!(nav.active_waypoint, Some("KSEA".to_string()));
    }

    #[test]
    fn test_lights_config() {
        let mut lights = LightsConfig::default();
        assert!(!lights.nav);
        assert!(!lights.beacon);
        assert!(!lights.strobe);
        assert!(!lights.landing);
        assert!(!lights.taxi);

        lights.nav = true;
        lights.beacon = true;
        assert!(lights.nav);
        assert!(lights.beacon);
    }

    // Validity-flag unit tests

    /// A freshly created BusSnapshot has all ValidityFlags false.
    #[test]
    fn test_new_snapshot_all_validity_flags_false() {
        let snapshot = BusSnapshot::new(SimId::XPlane, AircraftId::new("A320"));
        assert!(!snapshot.validity.safe_for_ffb);
        assert!(!snapshot.validity.attitude_valid);
        assert!(!snapshot.validity.angular_rates_valid);
        assert!(!snapshot.validity.velocities_valid);
        assert!(!snapshot.validity.kinematics_valid);
        assert!(!snapshot.validity.aero_valid);
        assert!(!snapshot.validity.position_valid);
    }

    /// Setting all validity flags to true on a valid snapshot still passes validate().
    #[test]
    fn test_snapshot_with_all_flags_set_passes_validate() {
        let mut snapshot = BusSnapshot::new(SimId::XPlane, AircraftId::new("A320"));
        snapshot.kinematics.ias = ValidatedSpeed::new_knots(250.0).unwrap();
        snapshot.kinematics.pitch = ValidatedAngle::new_degrees(3.0).unwrap();
        snapshot.validity = ValidityFlags {
            safe_for_ffb: true,
            attitude_valid: true,
            angular_rates_valid: true,
            velocities_valid: true,
            kinematics_valid: true,
            aero_valid: true,
            position_valid: true,
        };
        assert!(snapshot.validate().is_ok());
        assert!(snapshot.validity.safe_for_ffb);
    }

    /// A snapshot whose age_ms() exceeds a staleness threshold has safe_for_ffb = false
    /// by default; consumers must not assume stale data is safe.
    #[test]
    fn test_stale_snapshot_safe_for_ffb_false_by_default() {
        let snapshot = BusSnapshot::new(SimId::XPlane, AircraftId::new("A320"));
        // Wait long enough that the snapshot is unambiguously stale (>100 ms)
        std::thread::sleep(std::time::Duration::from_millis(120));
        let age = snapshot.age_ms();
        assert!(age >= 100, "expected age >= 100 ms, got {age} ms");
        // The flag must remain false because no code set it true
        assert!(
            !snapshot.validity.safe_for_ffb,
            "stale snapshot must have safe_for_ffb = false"
        );
    }
}
