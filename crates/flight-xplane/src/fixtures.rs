// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Test fixtures for X-Plane adapter
//!
//! Provides comprehensive test fixtures for validating X-Plane adapter functionality,
//! including mock data, scenario testing, and integration validation.

use crate::{
    adapter::XPlaneRawData,
    aircraft::{DetectedAircraft, AircraftType, EngineType},
    dataref::DataRefValue,
    latency::LatencyMeasurement,
};
use flight_bus::{
    fixtures::{ScenarioType, ValidationTolerance},
    snapshot::BusSnapshot,
    types::{AircraftId, SimId},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

/// X-Plane specific test fixture
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XPlaneFixture {
    /// Fixture metadata
    pub metadata: FixtureMetadata,
    /// Mock aircraft information
    pub aircraft: DetectedAircraft,
    /// Mock DataRef values
    pub dataref_values: HashMap<String, DataRefValue>,
    /// Expected bus snapshot
    pub expected_snapshot: BusSnapshot,
    /// Latency expectations
    pub latency_expectations: LatencyExpectations,
    /// Validation tolerances
    pub tolerances: ValidationTolerance,
}

/// Fixture metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureMetadata {
    pub name: String,
    pub description: String,
    pub scenario_type: ScenarioType,
    pub aircraft_type: AircraftType,
    pub engine_type: EngineType,
    pub created_at: String,
    pub version: String,
}

/// Latency expectations for testing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyExpectations {
    pub max_telemetry_latency_ms: u64,
    pub max_dataref_latency_ms: u64,
    pub max_aircraft_detection_ms: u64,
    pub expected_jitter_ms: u64,
}

impl Default for LatencyExpectations {
    fn default() -> Self {
        Self {
            max_telemetry_latency_ms: 50,
            max_dataref_latency_ms: 100,
            max_aircraft_detection_ms: 300,
            expected_jitter_ms: 5,
        }
    }
}

/// X-Plane fixture generator
pub struct XPlaneFixtureGenerator;

impl XPlaneFixtureGenerator {
    /// Generate fixture for Cessna 172
    pub fn cessna_172_cruise() -> XPlaneFixture {
        let aircraft = DetectedAircraft {
            icao: "C172".to_string(),
            title: "Cessna 172SP".to_string(),
            author: "Laminar Research".to_string(),
        };

        let mut dataref_values = HashMap::new();
        
        // Basic flight data
        dataref_values.insert("sim/flightmodel/position/indicated_airspeed".to_string(), DataRefValue::Float(77.17)); // ~150 knots
        dataref_values.insert("sim/flightmodel/position/true_airspeed".to_string(), DataRefValue::Float(80.56)); // ~156 knots
        dataref_values.insert("sim/flightmodel/position/groundspeed".to_string(), DataRefValue::Float(75.0)); // ~145 knots
        
        // Attitude
        dataref_values.insert("sim/flightmodel/position/theta".to_string(), DataRefValue::Float(2.5)); // Slight climb
        dataref_values.insert("sim/flightmodel/position/phi".to_string(), DataRefValue::Float(0.0)); // Level
        dataref_values.insert("sim/flightmodel/position/psi".to_string(), DataRefValue::Float(90.0)); // East
        
        // Angles
        dataref_values.insert("sim/flightmodel/position/alpha".to_string(), DataRefValue::Float(3.0)); // AoA
        dataref_values.insert("sim/flightmodel/position/beta".to_string(), DataRefValue::Float(0.0)); // No sideslip
        
        // G-forces
        dataref_values.insert("sim/flightmodel/forces/g_nrml".to_string(), DataRefValue::Float(1.0));
        dataref_values.insert("sim/flightmodel/forces/g_side".to_string(), DataRefValue::Float(0.0));
        dataref_values.insert("sim/flightmodel/forces/g_axil".to_string(), DataRefValue::Float(0.0));
        
        // Position
        dataref_values.insert("sim/flightmodel/position/latitude".to_string(), DataRefValue::Double(37.7749));
        dataref_values.insert("sim/flightmodel/position/longitude".to_string(), DataRefValue::Double(-122.4194));
        dataref_values.insert("sim/flightmodel/position/elevation".to_string(), DataRefValue::Float(1524.0)); // 5000 ft
        
        // Vertical speed
        dataref_values.insert("sim/flightmodel/position/vh_ind".to_string(), DataRefValue::Float(2.54)); // 500 fpm
        
        // Aircraft configuration
        dataref_values.insert("sim/aircraft/parts/acf_gear_deploy".to_string(), DataRefValue::Float(0.0)); // Gear up
        dataref_values.insert("sim/aircraft/parts/acf_flap_deploy".to_string(), DataRefValue::Float(0.0)); // No flaps
        dataref_values.insert("sim/aircraft/parts/acf_speedbrake_deploy".to_string(), DataRefValue::Float(0.0));
        
        // Engine
        dataref_values.insert("sim/flightmodel/engine/ENGN_running[0]".to_string(), DataRefValue::Int(1));
        dataref_values.insert("sim/flightmodel/engine/ENGN_N1_[0]".to_string(), DataRefValue::Float(75.0));
        dataref_values.insert("sim/flightmodel/engine/ENGN_MPR[0]".to_string(), DataRefValue::Float(23.5));
        
        // Environment
        dataref_values.insert("sim/weather/temperature_ambient_c".to_string(), DataRefValue::Float(15.0));
        dataref_values.insert("sim/weather/wind_speed_kt[0]".to_string(), DataRefValue::Float(5.14)); // 10 knots
        dataref_values.insert("sim/weather/wind_direction_degt[0]".to_string(), DataRefValue::Float(270.0));
        
        // Aircraft identification
        dataref_values.insert("sim/aircraft/view/acf_ICAO".to_string(), DataRefValue::FloatArray(vec![67.0, 49.0, 55.0, 50.0, 0.0])); // "C172"
        dataref_values.insert("sim/aircraft/view/acf_descrip".to_string(), DataRefValue::FloatArray(vec![67.0, 101.0, 115.0, 115.0, 110.0, 97.0, 32.0, 49.0, 55.0, 50.0, 83.0, 80.0, 0.0])); // "Cessna 172SP"

        // Create expected snapshot
        let aircraft_id = AircraftId::new(&aircraft.icao);
        let expected_snapshot = BusSnapshot::new(SimId::XPlane, aircraft_id);

        XPlaneFixture {
            metadata: FixtureMetadata {
                name: "cessna_172_cruise".to_string(),
                description: "Cessna 172 in cruise flight at 5000 feet".to_string(),
                scenario_type: ScenarioType::Cruise,
                aircraft_type: AircraftType::GeneralAviation,
                engine_type: EngineType::Piston,
                created_at: "2024-01-01T00:00:00Z".to_string(),
                version: "1.0.0".to_string(),
            },
            aircraft,
            dataref_values,
            expected_snapshot,
            latency_expectations: LatencyExpectations::default(),
            tolerances: ValidationTolerance::default(),
        }
    }

    /// Generate fixture for Airbus A320 approach
    pub fn airbus_a320_approach() -> XPlaneFixture {
        let aircraft = DetectedAircraft {
            icao: "A320".to_string(),
            title: "Airbus A320-214".to_string(),
            author: "FlightFactor".to_string(),
        };

        let mut dataref_values = HashMap::new();
        
        // Approach speeds
        dataref_values.insert("sim/flightmodel/position/indicated_airspeed".to_string(), DataRefValue::Float(69.44)); // ~135 knots
        dataref_values.insert("sim/flightmodel/position/true_airspeed".to_string(), DataRefValue::Float(72.0)); // ~140 knots
        dataref_values.insert("sim/flightmodel/position/groundspeed".to_string(), DataRefValue::Float(67.0)); // ~130 knots
        
        // Approach attitude
        dataref_values.insert("sim/flightmodel/position/theta".to_string(), DataRefValue::Float(-3.0)); // Descent
        dataref_values.insert("sim/flightmodel/position/phi".to_string(), DataRefValue::Float(0.0)); // Level
        dataref_values.insert("sim/flightmodel/position/psi".to_string(), DataRefValue::Float(270.0)); // West
        
        // Approach configuration
        dataref_values.insert("sim/aircraft/parts/acf_gear_deploy".to_string(), DataRefValue::Float(1.0)); // Gear down
        dataref_values.insert("sim/aircraft/parts/acf_flap_deploy".to_string(), DataRefValue::Float(0.75)); // Approach flaps
        
        // Twin engines
        dataref_values.insert("sim/flightmodel/engine/ENGN_running[0]".to_string(), DataRefValue::Int(1));
        dataref_values.insert("sim/flightmodel/engine/ENGN_running[1]".to_string(), DataRefValue::Int(1));
        dataref_values.insert("sim/flightmodel/engine/ENGN_N1_[0]".to_string(), DataRefValue::Float(65.0));
        dataref_values.insert("sim/flightmodel/engine/ENGN_N1_[1]".to_string(), DataRefValue::Float(65.0));
        
        // Autopilot
        dataref_values.insert("sim/cockpit/autopilot/autopilot_mode".to_string(), DataRefValue::Int(2)); // Engaged
        dataref_values.insert("sim/cockpit/autopilot/altitude".to_string(), DataRefValue::Float(3000.0));
        dataref_values.insert("sim/cockpit/autopilot/heading".to_string(), DataRefValue::Float(270.0));
        
        // Position
        dataref_values.insert("sim/flightmodel/position/latitude".to_string(), DataRefValue::Double(40.6892));
        dataref_values.insert("sim/flightmodel/position/longitude".to_string(), DataRefValue::Double(-74.1745));
        dataref_values.insert("sim/flightmodel/position/elevation".to_string(), DataRefValue::Float(914.4)); // 3000 ft
        dataref_values.insert("sim/flightmodel/position/vh_ind".to_string(), DataRefValue::Float(-3.81)); // -750 fpm
        
        // Angles
        dataref_values.insert("sim/flightmodel/position/alpha".to_string(), DataRefValue::Float(5.0)); // AoA
        dataref_values.insert("sim/flightmodel/position/beta".to_string(), DataRefValue::Float(0.0)); // No sideslip
        
        // G-forces
        dataref_values.insert("sim/flightmodel/forces/g_nrml".to_string(), DataRefValue::Float(1.0));
        dataref_values.insert("sim/flightmodel/forces/g_side".to_string(), DataRefValue::Float(0.0));
        dataref_values.insert("sim/flightmodel/forces/g_axil".to_string(), DataRefValue::Float(0.0));
        
        // Ground track
        dataref_values.insert("sim/flightmodel/position/hpath".to_string(), DataRefValue::Float(270.0));
        
        // Environment
        dataref_values.insert("sim/weather/temperature_ambient_c".to_string(), DataRefValue::Float(10.0));
        dataref_values.insert("sim/weather/wind_speed_kt[0]".to_string(), DataRefValue::Float(10.28)); // 20 knots
        dataref_values.insert("sim/weather/wind_direction_degt[0]".to_string(), DataRefValue::Float(300.0));
        
        // Aircraft identification
        dataref_values.insert("sim/aircraft/view/acf_ICAO".to_string(), DataRefValue::FloatArray(vec![65.0, 51.0, 50.0, 48.0, 0.0])); // "A320"
        dataref_values.insert("sim/aircraft/view/acf_descrip".to_string(), DataRefValue::FloatArray(vec![65.0, 105.0, 114.0, 98.0, 117.0, 115.0, 32.0, 65.0, 51.0, 50.0, 48.0, 0.0])); // "Airbus A320"

        let aircraft_id = AircraftId::new(&aircraft.icao);
        let expected_snapshot = BusSnapshot::new(SimId::XPlane, aircraft_id);

        XPlaneFixture {
            metadata: FixtureMetadata {
                name: "airbus_a320_approach".to_string(),
                description: "Airbus A320 on approach with autopilot engaged".to_string(),
                scenario_type: ScenarioType::Approach,
                aircraft_type: AircraftType::Airliner,
                engine_type: EngineType::Jet,
                created_at: "2024-01-01T00:00:00Z".to_string(),
                version: "1.0.0".to_string(),
            },
            aircraft,
            dataref_values,
            expected_snapshot,
            latency_expectations: LatencyExpectations::default(),
            tolerances: ValidationTolerance::default(),
        }
    }

    /// Generate fixture for UH-1H helicopter hover
    pub fn uh1h_hover() -> XPlaneFixture {
        let aircraft = DetectedAircraft {
            icao: "UH1H".to_string(),
            title: "Bell UH-1H Huey".to_string(),
            author: "X-Plane".to_string(),
        };

        let mut dataref_values = HashMap::new();
        
        // Hover speeds (near zero)
        dataref_values.insert("sim/flightmodel/position/indicated_airspeed".to_string(), DataRefValue::Float(2.57)); // ~5 knots
        dataref_values.insert("sim/flightmodel/position/true_airspeed".to_string(), DataRefValue::Float(2.57));
        dataref_values.insert("sim/flightmodel/position/groundspeed".to_string(), DataRefValue::Float(0.0));
        
        // Hover attitude
        dataref_values.insert("sim/flightmodel/position/theta".to_string(), DataRefValue::Float(5.0)); // Slight nose up
        dataref_values.insert("sim/flightmodel/position/phi".to_string(), DataRefValue::Float(-2.0)); // Slight left bank
        dataref_values.insert("sim/flightmodel/position/psi".to_string(), DataRefValue::Float(180.0)); // South
        
        // Helicopter-specific
        dataref_values.insert("sim/flightmodel/engine/ENGN_Nrotor".to_string(), DataRefValue::Float(100.0)); // Main rotor
        dataref_values.insert("sim/flightmodel/engine/ENGN_Nturb".to_string(), DataRefValue::Float(95.0)); // Turbine
        dataref_values.insert("sim/flightmodel/engine/ENGN_torq".to_string(), DataRefValue::Float(65.0)); // Torque
        dataref_values.insert("sim/joystick/yoke_pitch_ratio".to_string(), DataRefValue::Float(0.6)); // Collective
        dataref_values.insert("sim/joystick/yoke_heading_ratio".to_string(), DataRefValue::Float(0.1)); // Pedals
        
        // Position
        dataref_values.insert("sim/flightmodel/position/latitude".to_string(), DataRefValue::Double(32.7767));
        dataref_values.insert("sim/flightmodel/position/longitude".to_string(), DataRefValue::Double(-96.7970));
        dataref_values.insert("sim/flightmodel/position/elevation".to_string(), DataRefValue::Float(30.48)); // 100 ft
        dataref_values.insert("sim/flightmodel/position/vh_ind".to_string(), DataRefValue::Float(0.0)); // Stable hover
        
        // G-forces
        dataref_values.insert("sim/flightmodel/forces/g_nrml".to_string(), DataRefValue::Float(1.0));
        dataref_values.insert("sim/flightmodel/forces/g_side".to_string(), DataRefValue::Float(0.0));
        dataref_values.insert("sim/flightmodel/forces/g_axil".to_string(), DataRefValue::Float(0.0));
        
        // Angles
        dataref_values.insert("sim/flightmodel/position/alpha".to_string(), DataRefValue::Float(0.0)); // AoA
        dataref_values.insert("sim/flightmodel/position/beta".to_string(), DataRefValue::Float(0.0)); // No sideslip
        
        // Ground track
        dataref_values.insert("sim/flightmodel/position/hpath".to_string(), DataRefValue::Float(180.0));
        
        // Environment
        dataref_values.insert("sim/weather/temperature_ambient_c".to_string(), DataRefValue::Float(25.0));
        dataref_values.insert("sim/weather/wind_speed_kt[0]".to_string(), DataRefValue::Float(2.57)); // 5 knots
        dataref_values.insert("sim/weather/wind_direction_degt[0]".to_string(), DataRefValue::Float(90.0));
        
        // Aircraft identification
        dataref_values.insert("sim/aircraft/view/acf_ICAO".to_string(), DataRefValue::FloatArray(vec![85.0, 72.0, 49.0, 72.0, 0.0])); // "UH1H"
        dataref_values.insert("sim/aircraft/view/acf_descrip".to_string(), DataRefValue::FloatArray(vec![66.0, 101.0, 108.0, 108.0, 32.0, 85.0, 72.0, 45.0, 49.0, 72.0, 0.0])); // "Bell UH-1H"

        let aircraft_id = AircraftId::new(&aircraft.icao);
        let expected_snapshot = BusSnapshot::new(SimId::XPlane, aircraft_id);

        XPlaneFixture {
            metadata: FixtureMetadata {
                name: "uh1h_hover".to_string(),
                description: "UH-1H helicopter in stable hover at 100 feet".to_string(),
                scenario_type: ScenarioType::Hover,
                aircraft_type: AircraftType::Helicopter,
                engine_type: EngineType::Turboshaft,
                created_at: "2024-01-01T00:00:00Z".to_string(),
                version: "1.0.0".to_string(),
            },
            aircraft,
            dataref_values,
            expected_snapshot,
            latency_expectations: LatencyExpectations::default(),
            tolerances: ValidationTolerance::default(),
        }
    }

    /// Generate all standard fixtures
    pub fn generate_all_fixtures() -> Vec<XPlaneFixture> {
        vec![
            Self::cessna_172_cruise(),
            Self::airbus_a320_approach(),
            Self::uh1h_hover(),
        ]
    }
}

/// X-Plane fixture validator
pub struct XPlaneFixtureValidator;

impl XPlaneFixtureValidator {
    /// Validate raw data against fixture
    pub fn validate_raw_data(fixture: &XPlaneFixture, raw_data: &XPlaneRawData) -> Result<(), String> {
        // Validate aircraft matches
        if raw_data.aircraft_info.icao != fixture.aircraft.icao {
            return Err(format!(
                "Aircraft ICAO mismatch: expected {}, got {}",
                fixture.aircraft.icao, raw_data.aircraft_info.icao
            ));
        }

        // Validate critical DataRefs are present
        let critical_datarefs = [
            "sim/flightmodel/position/indicated_airspeed",
            "sim/flightmodel/position/latitude",
            "sim/flightmodel/position/longitude",
        ];

        for dataref in &critical_datarefs {
            if !raw_data.dataref_values.contains_key(*dataref) {
                return Err(format!("Missing critical DataRef: {}", dataref));
            }
        }

        // Validate DataRef values are within reasonable ranges
        for (name, expected_value) in &fixture.dataref_values {
            if let Some(actual_value) = raw_data.dataref_values.get(name) {
                if let Err(e) = Self::validate_dataref_value(name, expected_value, actual_value, &fixture.tolerances) {
                    return Err(format!("DataRef {} validation failed: {}", name, e));
                }
            }
        }

        Ok(())
    }

    /// Validate converted snapshot against fixture
    pub fn validate_snapshot(fixture: &XPlaneFixture, snapshot: &BusSnapshot) -> Result<(), String> {
        // Validate basic properties
        if snapshot.sim != SimId::XPlane {
            return Err(format!("Wrong simulator ID: expected XPlane, got {:?}", snapshot.sim));
        }

        if snapshot.aircraft.icao != fixture.aircraft.icao {
            return Err(format!(
                "Aircraft ICAO mismatch: expected {}, got {}",
                fixture.aircraft.icao, snapshot.aircraft.icao
            ));
        }

        // Validate snapshot is recent
        if snapshot.age_ms() > 1000 {
            return Err(format!("Snapshot too old: {} ms", snapshot.age_ms()));
        }

        // Validate kinematics are reasonable
        Self::validate_kinematics(&snapshot.kinematics, &fixture.tolerances)?;

        // Validate aircraft configuration
        Self::validate_aircraft_config(&snapshot.config, &fixture.tolerances)?;

        Ok(())
    }

    /// Validate latency measurements
    pub fn validate_latency(fixture: &XPlaneFixture, measurements: &[LatencyMeasurement]) -> Result<(), String> {
        if measurements.is_empty() {
            return Err("No latency measurements provided".to_string());
        }

        for measurement in measurements {
            let max_latency = match measurement.operation.as_str() {
                "telemetry" => fixture.latency_expectations.max_telemetry_latency_ms,
                "dataref" => fixture.latency_expectations.max_dataref_latency_ms,
                "aircraft_detection" => fixture.latency_expectations.max_aircraft_detection_ms,
                _ => fixture.latency_expectations.max_telemetry_latency_ms, // Default
            };

            if measurement.latency.as_millis() as u64 > max_latency {
                return Err(format!(
                    "Latency budget exceeded for {}: {}ms > {}ms",
                    measurement.operation,
                    measurement.latency.as_millis(),
                    max_latency
                ));
            }
        }

        Ok(())
    }

    /// Validate individual DataRef value
    fn validate_dataref_value(
        name: &str,
        expected: &DataRefValue,
        actual: &DataRefValue,
        tolerances: &ValidationTolerance,
    ) -> Result<(), String> {
        match (expected, actual) {
            (DataRefValue::Float(exp), DataRefValue::Float(act)) => {
                let tolerance = Self::get_tolerance_for_dataref(name, tolerances);
                if (exp - act).abs() > tolerance {
                    return Err(format!("Float value out of tolerance: expected {}, got {}, tolerance {}", exp, act, tolerance));
                }
            }
            (DataRefValue::Double(exp), DataRefValue::Double(act)) => {
                let tolerance = Self::get_tolerance_for_dataref(name, tolerances) as f64;
                if (exp - act).abs() > tolerance {
                    return Err(format!("Double value out of tolerance: expected {}, got {}, tolerance {}", exp, act, tolerance));
                }
            }
            (DataRefValue::Int(exp), DataRefValue::Int(act)) => {
                if exp != act {
                    return Err(format!("Integer value mismatch: expected {}, got {}", exp, act));
                }
            }
            (DataRefValue::FloatArray(exp), DataRefValue::FloatArray(act)) => {
                if exp.len() != act.len() {
                    return Err(format!("Array length mismatch: expected {}, got {}", exp.len(), act.len()));
                }
                let tolerance = Self::get_tolerance_for_dataref(name, tolerances);
                for (i, (e, a)) in exp.iter().zip(act.iter()).enumerate() {
                    if (e - a).abs() > tolerance {
                        return Err(format!("Array element {} out of tolerance: expected {}, got {}, tolerance {}", i, e, a, tolerance));
                    }
                }
            }
            (DataRefValue::IntArray(exp), DataRefValue::IntArray(act)) => {
                if exp != act {
                    return Err(format!("Integer array mismatch: expected {:?}, got {:?}", exp, act));
                }
            }
            _ => {
                return Err(format!("Type mismatch: expected {:?}, got {:?}", expected, actual));
            }
        }

        Ok(())
    }

    /// Get tolerance for specific DataRef
    fn get_tolerance_for_dataref(name: &str, tolerances: &ValidationTolerance) -> f32 {
        match name {
            name if name.contains("airspeed") => tolerances.speed_knots,
            name if name.contains("position/theta") || name.contains("position/phi") || name.contains("position/psi") => tolerances.angle_degrees,
            name if name.contains("position/alpha") || name.contains("position/beta") => tolerances.angle_degrees,
            name if name.contains("forces/g_") => tolerances.g_force,
            name if name.contains("position/latitude") || name.contains("position/longitude") => 0.0001, // ~10m
            name if name.contains("position/elevation") => tolerances.altitude_feet,
            name if name.contains("engine") => tolerances.percentage,
            _ => 1.0, // Default tolerance
        }
    }

    /// Validate kinematics data
    fn validate_kinematics(kinematics: &flight_bus::snapshot::Kinematics, _tolerances: &ValidationTolerance) -> Result<(), String> {
        // Check for reasonable values
        let ias_knots = kinematics.ias.to_knots();
        if ias_knots < 0.0 || ias_knots > 500.0 {
            return Err(format!("Unreasonable IAS: {} knots", ias_knots));
        }

        let g_force = kinematics.g_force.value();
        if g_force < -10.0 || g_force > 10.0 {
            return Err(format!("Unreasonable G-force: {} G", g_force));
        }

        let mach = kinematics.mach.value();
        if mach < 0.0 || mach > 2.0 {
            return Err(format!("Unreasonable Mach: {}", mach));
        }

        Ok(())
    }

    /// Validate aircraft configuration
    fn validate_aircraft_config(config: &flight_bus::snapshot::AircraftConfig, _tolerances: &ValidationTolerance) -> Result<(), String> {
        // Validate percentages are in range
        if config.flaps.value() < 0.0 || config.flaps.value() > 100.0 {
            return Err(format!("Flaps out of range: {}%", config.flaps.value()));
        }

        if config.spoilers.value() < 0.0 || config.spoilers.value() > 100.0 {
            return Err(format!("Spoilers out of range: {}%", config.spoilers.value()));
        }

        Ok(())
    }
}

/// Test scenario runner for X-Plane fixtures
pub struct XPlaneScenarioRunner;

impl XPlaneScenarioRunner {
    /// Run a complete test scenario
    pub async fn run_scenario(fixture: &XPlaneFixture) -> Result<ScenarioResult, String> {
        let start_time = Instant::now();
        
        // Create raw data from fixture
        let raw_data = XPlaneRawData {
            timestamp: Instant::now(),
            aircraft_info: fixture.aircraft.clone(),
            dataref_values: fixture.dataref_values.clone(),
        };

        // Validate raw data
        XPlaneFixtureValidator::validate_raw_data(fixture, &raw_data)?;

        // Convert to snapshot (this would normally be done by the adapter)
        let snapshot = Self::simulate_conversion(&raw_data)?;

        // Validate snapshot
        XPlaneFixtureValidator::validate_snapshot(fixture, &snapshot)?;

        // Create mock latency measurements
        let latency_measurements = vec![
            LatencyMeasurement::new("telemetry".to_string(), Duration::from_millis(25)),
            LatencyMeasurement::new("dataref".to_string(), Duration::from_millis(50)),
            LatencyMeasurement::new("aircraft_detection".to_string(), Duration::from_millis(150)),
        ];

        // Validate latency
        XPlaneFixtureValidator::validate_latency(fixture, &latency_measurements)?;

        let total_duration = start_time.elapsed();

        Ok(ScenarioResult {
            fixture_name: fixture.metadata.name.clone(),
            success: true,
            duration: total_duration,
            measurements: latency_measurements,
            snapshot,
            errors: Vec::new(),
        })
    }

    /// Simulate conversion from raw data to snapshot
    fn simulate_conversion(raw_data: &XPlaneRawData) -> Result<BusSnapshot, String> {
        let aircraft_id = AircraftId::new(&raw_data.aircraft_info.icao);
        let mut snapshot = BusSnapshot::new(SimId::XPlane, aircraft_id);

        // This is a simplified conversion for testing
        // In the real adapter, this would use the full conversion logic
        
        // Set timestamp to current time (in nanoseconds since epoch)
        snapshot.timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        Ok(snapshot)
    }

    /// Run all standard scenarios
    pub async fn run_all_scenarios() -> Vec<ScenarioResult> {
        let fixtures = XPlaneFixtureGenerator::generate_all_fixtures();
        let mut results = Vec::new();

        for fixture in fixtures {
            match Self::run_scenario(&fixture).await {
                Ok(result) => results.push(result),
                Err(error) => {
                    results.push(ScenarioResult {
                        fixture_name: fixture.metadata.name.clone(),
                        success: false,
                        duration: Duration::ZERO,
                        measurements: Vec::new(),
                        snapshot: BusSnapshot::new(SimId::XPlane, AircraftId::new("ERROR")),
                        errors: vec![error],
                    });
                }
            }
        }

        results
    }
}

/// Test scenario result
#[derive(Debug, Clone)]
pub struct ScenarioResult {
    pub fixture_name: String,
    pub success: bool,
    pub duration: Duration,
    pub measurements: Vec<LatencyMeasurement>,
    pub snapshot: BusSnapshot,
    pub errors: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixture_generation() {
        let fixture = XPlaneFixtureGenerator::cessna_172_cruise();
        
        assert_eq!(fixture.metadata.name, "cessna_172_cruise");
        assert_eq!(fixture.aircraft.icao, "C172");
        assert_eq!(fixture.metadata.aircraft_type, AircraftType::GeneralAviation);
        assert_eq!(fixture.metadata.engine_type, EngineType::Piston);
        
        // Should have critical DataRefs
        assert!(fixture.dataref_values.contains_key("sim/flightmodel/position/indicated_airspeed"));
        assert!(fixture.dataref_values.contains_key("sim/flightmodel/position/latitude"));
        assert!(fixture.dataref_values.contains_key("sim/flightmodel/position/longitude"));
    }

    #[test]
    fn test_all_fixtures_generation() {
        let fixtures = XPlaneFixtureGenerator::generate_all_fixtures();
        
        assert_eq!(fixtures.len(), 3);
        
        let names: Vec<&String> = fixtures.iter().map(|f| &f.metadata.name).collect();
        assert!(names.contains(&&"cessna_172_cruise".to_string()));
        assert!(names.contains(&&"airbus_a320_approach".to_string()));
        assert!(names.contains(&&"uh1h_hover".to_string()));
    }

    #[test]
    fn test_dataref_value_validation() {
        let tolerances = ValidationTolerance::default();
        
        // Float validation within tolerance
        let result = XPlaneFixtureValidator::validate_dataref_value(
            "test_dataref",
            &DataRefValue::Float(100.0),
            &DataRefValue::Float(100.1),
            &tolerances,
        );
        assert!(result.is_ok());
        
        // Float validation outside tolerance
        let result = XPlaneFixtureValidator::validate_dataref_value(
            "test_dataref",
            &DataRefValue::Float(100.0),
            &DataRefValue::Float(110.0),
            &tolerances,
        );
        assert!(result.is_err());
        
        // Integer validation
        let result = XPlaneFixtureValidator::validate_dataref_value(
            "test_dataref",
            &DataRefValue::Int(42),
            &DataRefValue::Int(42),
            &tolerances,
        );
        assert!(result.is_ok());
        
        let result = XPlaneFixtureValidator::validate_dataref_value(
            "test_dataref",
            &DataRefValue::Int(42),
            &DataRefValue::Int(43),
            &tolerances,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_latency_validation() {
        let fixture = XPlaneFixtureGenerator::cessna_172_cruise();
        
        // Good latency measurements
        let good_measurements = vec![
            LatencyMeasurement::new("telemetry".to_string(), Duration::from_millis(25)),
            LatencyMeasurement::new("dataref".to_string(), Duration::from_millis(50)),
        ];
        
        let result = XPlaneFixtureValidator::validate_latency(&fixture, &good_measurements);
        assert!(result.is_ok());
        
        // Bad latency measurements
        let bad_measurements = vec![
            LatencyMeasurement::new("telemetry".to_string(), Duration::from_millis(100)), // Exceeds 50ms budget
        ];
        
        let result = XPlaneFixtureValidator::validate_latency(&fixture, &bad_measurements);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_scenario_runner() {
        let fixture = XPlaneFixtureGenerator::cessna_172_cruise();
        let result = XPlaneScenarioRunner::run_scenario(&fixture).await;
        
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.success);
        assert_eq!(result.fixture_name, "cessna_172_cruise");
        assert!(!result.measurements.is_empty());
    }

    #[tokio::test]
    async fn test_all_scenarios() {
        let results = XPlaneScenarioRunner::run_all_scenarios().await;
        
        assert_eq!(results.len(), 3);
        
        // All scenarios should pass
        for result in &results {
            assert!(result.success, "Scenario {} failed: {:?}", result.fixture_name, result.errors);
        }
    }

    #[test]
    fn test_helicopter_fixture() {
        let fixture = XPlaneFixtureGenerator::uh1h_hover();
        
        assert_eq!(fixture.metadata.aircraft_type, AircraftType::Helicopter);
        assert_eq!(fixture.metadata.engine_type, EngineType::Turboshaft);
        
        // Should have helicopter-specific DataRefs
        assert!(fixture.dataref_values.contains_key("sim/flightmodel/engine/ENGN_Nrotor"));
        assert!(fixture.dataref_values.contains_key("sim/flightmodel/engine/ENGN_torq"));
        assert!(fixture.dataref_values.contains_key("sim/joystick/yoke_pitch_ratio")); // Collective
    }

    #[test]
    fn test_airliner_fixture() {
        let fixture = XPlaneFixtureGenerator::airbus_a320_approach();
        
        assert_eq!(fixture.metadata.aircraft_type, AircraftType::Airliner);
        assert_eq!(fixture.metadata.engine_type, EngineType::Jet);
        
        // Should have twin engines
        assert!(fixture.dataref_values.contains_key("sim/flightmodel/engine/ENGN_running[0]"));
        assert!(fixture.dataref_values.contains_key("sim/flightmodel/engine/ENGN_running[1]"));
        
        // Should have autopilot
        assert!(fixture.dataref_values.contains_key("sim/cockpit/autopilot/autopilot_mode"));
    }
}