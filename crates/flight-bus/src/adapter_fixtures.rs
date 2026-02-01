// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Adapter-specific test fixtures for integration testing.
//!
//! This module provides JSON-based fixtures for each simulator adapter:
//! - MSFS SimConnect fixtures
//! - X-Plane UDP fixtures
//! - DCS Export.lua fixtures
//!
//! Requirements: 14.1 from release-readiness spec

use crate::integration_test::AdapterType;
use crate::snapshot::BusSnapshot;
use crate::types::{AircraftId, GForce, Mach, SimId, ValidatedAngle, ValidatedSpeed};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

/// Fixture loading errors
#[derive(Debug, Error)]
pub enum FixtureError {
    #[error("Failed to read fixture file: {path}")]
    ReadError { path: String },
    #[error("Failed to parse fixture JSON: {reason}")]
    ParseError { reason: String },
    #[error("Invalid fixture data: {field}")]
    InvalidData { field: String },
    #[error("Fixture not found: {name}")]
    NotFound { name: String },
}

/// MSFS SimConnect fixture data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsfsFixture {
    /// Description of the fixture scenario
    pub description: String,
    /// Aircraft ICAO code
    pub aircraft: String,
    /// SimVar values (name -> value)
    pub simvars: HashMap<String, f64>,
    /// Expected bus snapshot values for validation
    pub expected_bus_values: HashMap<String, f64>,
}

/// X-Plane UDP fixture data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XPlaneFixture {
    /// Description of the fixture scenario
    pub description: String,
    /// Aircraft ICAO code
    pub aircraft: String,
    /// DataRef values (name -> value)
    pub datarefs: HashMap<String, f64>,
    /// Expected bus snapshot values for validation
    pub expected_bus_values: HashMap<String, f64>,
}

/// DCS Export.lua fixture data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DcsFixture {
    /// Description of the fixture scenario
    pub description: String,
    /// Aircraft name
    pub aircraft: String,
    /// Session type (SP, MP, etc.)
    pub session_type: String,
    /// Lua export values (name -> value)
    pub lua_values: HashMap<String, serde_json::Value>,
    /// Expected bus snapshot values for validation
    pub expected_bus_values: HashMap<String, f64>,
}

/// Generic adapter fixture that can hold any adapter type's data
#[derive(Debug, Clone)]
pub enum AdapterFixture {
    Msfs(MsfsFixture),
    XPlane(XPlaneFixture),
    Dcs(DcsFixture),
}

impl AdapterFixture {
    /// Get the adapter type for this fixture
    pub fn adapter_type(&self) -> AdapterType {
        match self {
            AdapterFixture::Msfs(_) => AdapterType::Msfs,
            AdapterFixture::XPlane(_) => AdapterType::XPlane,
            AdapterFixture::Dcs(_) => AdapterType::Dcs,
        }
    }

    /// Get the aircraft ID from the fixture
    pub fn aircraft_id(&self) -> AircraftId {
        match self {
            AdapterFixture::Msfs(f) => AircraftId::new(&f.aircraft),
            AdapterFixture::XPlane(f) => AircraftId::new(&f.aircraft),
            AdapterFixture::Dcs(f) => AircraftId::new(&f.aircraft),
        }
    }

    /// Get the description of the fixture
    pub fn description(&self) -> &str {
        match self {
            AdapterFixture::Msfs(f) => &f.description,
            AdapterFixture::XPlane(f) => &f.description,
            AdapterFixture::Dcs(f) => &f.description,
        }
    }
}

/// Fixture loader for adapter integration tests
pub struct FixtureLoader;

impl FixtureLoader {
    /// Load a fixture from a JSON file
    pub fn load_from_file(
        path: &Path,
        adapter_type: AdapterType,
    ) -> Result<AdapterFixture, FixtureError> {
        let content = std::fs::read_to_string(path).map_err(|_| FixtureError::ReadError {
            path: path.display().to_string(),
        })?;

        Self::load_from_json(&content, adapter_type)
    }

    /// Load a fixture from a JSON string
    pub fn load_from_json(
        json: &str,
        adapter_type: AdapterType,
    ) -> Result<AdapterFixture, FixtureError> {
        match adapter_type {
            AdapterType::Msfs => {
                let fixture: MsfsFixture =
                    serde_json::from_str(json).map_err(|e| FixtureError::ParseError {
                        reason: e.to_string(),
                    })?;
                Ok(AdapterFixture::Msfs(fixture))
            }
            AdapterType::XPlane => {
                let fixture: XPlaneFixture =
                    serde_json::from_str(json).map_err(|e| FixtureError::ParseError {
                        reason: e.to_string(),
                    })?;
                Ok(AdapterFixture::XPlane(fixture))
            }
            AdapterType::Dcs => {
                let fixture: DcsFixture =
                    serde_json::from_str(json).map_err(|e| FixtureError::ParseError {
                        reason: e.to_string(),
                    })?;
                Ok(AdapterFixture::Dcs(fixture))
            }
        }
    }
}

/// Built-in fixtures for each adapter type
pub struct BuiltinFixtures;

impl BuiltinFixtures {
    /// Get the MSFS C172 cruise fixture
    pub fn msfs_c172_cruise() -> MsfsFixture {
        MsfsFixture {
            description: "C172 in cruise at 2500ft, 100 knots".to_string(),
            aircraft: "C172".to_string(),
            simvars: [
                ("AIRSPEED INDICATED".to_string(), 100.0),
                ("AIRSPEED TRUE".to_string(), 105.0),
                ("GROUND VELOCITY".to_string(), 98.0),
                ("INCIDENCE ALPHA".to_string(), 3.5),
                ("INCIDENCE BETA".to_string(), 0.2),
                ("ATTITUDE BANK DEGREES".to_string(), 2.0),
                ("ATTITUDE PITCH DEGREES".to_string(), 5.0),
                ("ATTITUDE HEADING DEGREES".to_string(), 270.0),
                ("G FORCE".to_string(), 1.0),
                ("G FORCE LATERAL".to_string(), 0.05),
                ("G FORCE LONGITUDINAL".to_string(), 0.1),
                ("AIRSPEED MACH".to_string(), 0.15),
                ("VERTICAL SPEED".to_string(), 0.0),
                ("PLANE ALTITUDE".to_string(), 2500.0),
            ]
            .into_iter()
            .collect(),
            expected_bus_values: [
                ("ias_knots".to_string(), 100.0),
                ("tas_knots".to_string(), 105.0),
                ("ground_speed_knots".to_string(), 98.0),
                ("aoa_degrees".to_string(), 3.5),
                ("bank_degrees".to_string(), 2.0),
                ("pitch_degrees".to_string(), 5.0),
                ("g_force".to_string(), 1.0),
                ("mach".to_string(), 0.15),
                ("altitude_ft".to_string(), 2500.0),
            ]
            .into_iter()
            .collect(),
        }
    }

    /// Get the MSFS A320 approach fixture
    pub fn msfs_a320_approach() -> MsfsFixture {
        MsfsFixture {
            description: "A320 on ILS approach at 3000ft, 140 knots".to_string(),
            aircraft: "A320".to_string(),
            simvars: [
                ("AIRSPEED INDICATED".to_string(), 140.0),
                ("AIRSPEED TRUE".to_string(), 145.0),
                ("GROUND VELOCITY".to_string(), 135.0),
                ("INCIDENCE ALPHA".to_string(), 5.0),
                ("INCIDENCE BETA".to_string(), 0.5),
                ("ATTITUDE BANK DEGREES".to_string(), -3.0),
                ("ATTITUDE PITCH DEGREES".to_string(), 3.0),
                ("ATTITUDE HEADING DEGREES".to_string(), 90.0),
                ("G FORCE".to_string(), 1.05),
                ("G FORCE LATERAL".to_string(), -0.02),
                ("G FORCE LONGITUDINAL".to_string(), 0.05),
                ("AIRSPEED MACH".to_string(), 0.21),
                ("VERTICAL SPEED".to_string(), -700.0),
                ("PLANE ALTITUDE".to_string(), 3000.0),
            ]
            .into_iter()
            .collect(),
            expected_bus_values: [
                ("ias_knots".to_string(), 140.0),
                ("tas_knots".to_string(), 145.0),
                ("ground_speed_knots".to_string(), 135.0),
                ("aoa_degrees".to_string(), 5.0),
                ("bank_degrees".to_string(), -3.0),
                ("pitch_degrees".to_string(), 3.0),
                ("g_force".to_string(), 1.05),
                ("mach".to_string(), 0.21),
                ("altitude_ft".to_string(), 3000.0),
            ]
            .into_iter()
            .collect(),
        }
    }
}

impl BuiltinFixtures {
    /// Get the X-Plane C172 cruise fixture
    pub fn xplane_c172_cruise() -> XPlaneFixture {
        XPlaneFixture {
            description: "C172 in cruise at 5500ft, 110 knots".to_string(),
            aircraft: "C172".to_string(),
            datarefs: [
                (
                    "sim/flightmodel/position/indicated_airspeed".to_string(),
                    56.58,
                ), // m/s
                ("sim/flightmodel/position/true_airspeed".to_string(), 59.16), // m/s
                ("sim/flightmodel/position/groundspeed".to_string(), 55.0),    // m/s
                ("sim/flightmodel/position/alpha".to_string(), 4.0),           // degrees
                ("sim/flightmodel/position/beta".to_string(), 0.3),            // degrees
                ("sim/flightmodel/position/phi".to_string(), 1.5),             // degrees (bank)
                ("sim/flightmodel/position/theta".to_string(), 3.0),           // degrees (pitch)
                ("sim/flightmodel/position/psi".to_string(), 180.0),           // degrees (heading)
                ("sim/flightmodel/forces/g_nrml".to_string(), 1.0),
                ("sim/flightmodel/forces/g_side".to_string(), 0.02),
                ("sim/flightmodel/forces/g_axil".to_string(), 0.05),
                ("sim/flightmodel/position/vh_ind".to_string(), 0.0), // m/s vertical
                ("sim/flightmodel/position/elevation".to_string(), 1676.4), // meters
            ]
            .into_iter()
            .collect(),
            expected_bus_values: [
                ("ias_mps".to_string(), 56.58),
                ("tas_mps".to_string(), 59.16),
                ("ground_speed_mps".to_string(), 55.0),
                ("aoa_degrees".to_string(), 4.0),
                ("bank_degrees".to_string(), 1.5),
                ("pitch_degrees".to_string(), 3.0),
                ("heading_degrees".to_string(), 180.0),
                ("g_force".to_string(), 1.0),
                ("altitude_m".to_string(), 1676.4),
            ]
            .into_iter()
            .collect(),
        }
    }

    /// Get the X-Plane B738 takeoff fixture
    pub fn xplane_b738_takeoff() -> XPlaneFixture {
        XPlaneFixture {
            description: "B738 during takeoff roll at 80 knots".to_string(),
            aircraft: "B738".to_string(),
            datarefs: [
                (
                    "sim/flightmodel/position/indicated_airspeed".to_string(),
                    41.15,
                ), // m/s (~80kt)
                ("sim/flightmodel/position/true_airspeed".to_string(), 41.15),
                ("sim/flightmodel/position/groundspeed".to_string(), 40.0),
                ("sim/flightmodel/position/alpha".to_string(), 2.0),
                ("sim/flightmodel/position/beta".to_string(), 0.0),
                ("sim/flightmodel/position/phi".to_string(), 0.0),
                ("sim/flightmodel/position/theta".to_string(), 0.0),
                ("sim/flightmodel/position/psi".to_string(), 90.0),
                ("sim/flightmodel/forces/g_nrml".to_string(), 1.0),
                ("sim/flightmodel/forces/g_side".to_string(), 0.0),
                ("sim/flightmodel/forces/g_axil".to_string(), 0.3),
                ("sim/flightmodel/position/vh_ind".to_string(), 0.0),
                ("sim/flightmodel/position/elevation".to_string(), 10.0),
            ]
            .into_iter()
            .collect(),
            expected_bus_values: [
                ("ias_mps".to_string(), 41.15),
                ("aoa_degrees".to_string(), 2.0),
                ("heading_degrees".to_string(), 90.0),
                ("g_force".to_string(), 1.0),
                ("g_longitudinal".to_string(), 0.3),
            ]
            .into_iter()
            .collect(),
        }
    }
}

impl BuiltinFixtures {
    /// Get the DCS F-16C cruise fixture
    pub fn dcs_f16c_cruise() -> DcsFixture {
        DcsFixture {
            description: "F-16C in cruise at 15000ft, 350 knots".to_string(),
            aircraft: "F-16C".to_string(),
            session_type: "SP".to_string(),
            lua_values: [
                ("ias".to_string(), serde_json::json!(350.0)),
                ("tas".to_string(), serde_json::json!(380.0)),
                ("altitude_asl".to_string(), serde_json::json!(15000.0)),
                ("heading".to_string(), serde_json::json!(90.0)),
                ("pitch".to_string(), serde_json::json!(3.0)),
                ("bank".to_string(), serde_json::json!(-5.0)),
                ("vertical_speed".to_string(), serde_json::json!(0.0)),
                ("g_force".to_string(), serde_json::json!(1.1)),
                ("g_lateral".to_string(), serde_json::json!(-0.05)),
                ("g_longitudinal".to_string(), serde_json::json!(0.15)),
                ("latitude".to_string(), serde_json::json!(45.5)),
                ("longitude".to_string(), serde_json::json!(-122.8)),
                (
                    "engines".to_string(),
                    serde_json::json!({
                        "0": {
                            "rpm": 85.0,
                            "temperature": 650.0,
                            "fuel_flow": 1200.0
                        }
                    }),
                ),
            ]
            .into_iter()
            .collect(),
            expected_bus_values: [
                ("ias_knots".to_string(), 350.0),
                ("tas_knots".to_string(), 380.0),
                ("altitude_ft".to_string(), 15000.0),
                ("heading_degrees".to_string(), 90.0),
                ("pitch_degrees".to_string(), 3.0),
                ("bank_degrees".to_string(), -5.0),
                ("g_force".to_string(), 1.1),
                ("latitude".to_string(), 45.5),
                ("longitude".to_string(), -122.8),
            ]
            .into_iter()
            .collect(),
        }
    }

    /// Get the DCS Ka-50 hover fixture
    pub fn dcs_ka50_hover() -> DcsFixture {
        DcsFixture {
            description: "Ka-50 helicopter in hover at 100ft AGL".to_string(),
            aircraft: "Ka-50".to_string(),
            session_type: "SP".to_string(),
            lua_values: [
                ("ias".to_string(), serde_json::json!(5.0)),
                ("tas".to_string(), serde_json::json!(5.0)),
                ("altitude_asl".to_string(), serde_json::json!(100.0)),
                ("heading".to_string(), serde_json::json!(180.0)),
                ("pitch".to_string(), serde_json::json!(2.0)),
                ("bank".to_string(), serde_json::json!(1.0)),
                ("vertical_speed".to_string(), serde_json::json!(0.0)),
                ("g_force".to_string(), serde_json::json!(1.0)),
                ("g_lateral".to_string(), serde_json::json!(0.02)),
                ("g_longitudinal".to_string(), serde_json::json!(0.05)),
                ("latitude".to_string(), serde_json::json!(43.2)),
                ("longitude".to_string(), serde_json::json!(41.8)),
                (
                    "engines".to_string(),
                    serde_json::json!({
                        "0": { "rpm": 95.0, "temperature": 580.0, "fuel_flow": 800.0 },
                        "1": { "rpm": 95.0, "temperature": 575.0, "fuel_flow": 790.0 }
                    }),
                ),
            ]
            .into_iter()
            .collect(),
            expected_bus_values: [
                ("ias_knots".to_string(), 5.0),
                ("altitude_ft".to_string(), 100.0),
                ("heading_degrees".to_string(), 180.0),
                ("g_force".to_string(), 1.0),
                ("latitude".to_string(), 43.2),
                ("longitude".to_string(), 41.8),
            ]
            .into_iter()
            .collect(),
        }
    }

    /// Get the DCS A-10C ground attack fixture
    pub fn dcs_a10c_attack() -> DcsFixture {
        DcsFixture {
            description: "A-10C in ground attack run at 5000ft, 250 knots".to_string(),
            aircraft: "A-10C".to_string(),
            session_type: "SP".to_string(),
            lua_values: [
                ("ias".to_string(), serde_json::json!(250.0)),
                ("tas".to_string(), serde_json::json!(265.0)),
                ("altitude_asl".to_string(), serde_json::json!(5000.0)),
                ("heading".to_string(), serde_json::json!(45.0)),
                ("pitch".to_string(), serde_json::json!(-15.0)),
                ("bank".to_string(), serde_json::json!(0.0)),
                ("vertical_speed".to_string(), serde_json::json!(-2000.0)),
                ("g_force".to_string(), serde_json::json!(1.5)),
                ("g_lateral".to_string(), serde_json::json!(0.0)),
                ("g_longitudinal".to_string(), serde_json::json!(0.3)),
                ("latitude".to_string(), serde_json::json!(42.0)),
                ("longitude".to_string(), serde_json::json!(43.5)),
            ]
            .into_iter()
            .collect(),
            expected_bus_values: [
                ("ias_knots".to_string(), 250.0),
                ("altitude_ft".to_string(), 5000.0),
                ("heading_degrees".to_string(), 45.0),
                ("pitch_degrees".to_string(), -15.0),
                ("g_force".to_string(), 1.5),
            ]
            .into_iter()
            .collect(),
        }
    }
}

/// Fixture converter for creating BusSnapshots from fixture data
pub struct FixtureConverter;

impl FixtureConverter {
    /// Convert an MSFS fixture to a BusSnapshot
    pub fn msfs_to_snapshot(fixture: &MsfsFixture) -> Result<BusSnapshot, FixtureError> {
        let aircraft_id = AircraftId::new(&fixture.aircraft);
        let mut snapshot = BusSnapshot::new(SimId::Msfs, aircraft_id);

        // Convert SimVars to snapshot fields
        if let Some(&ias) = fixture.simvars.get("AIRSPEED INDICATED") {
            snapshot.kinematics.ias =
                ValidatedSpeed::new_knots(ias as f32).map_err(|_| FixtureError::InvalidData {
                    field: "ias".to_string(),
                })?;
        }
        if let Some(&tas) = fixture.simvars.get("AIRSPEED TRUE") {
            snapshot.kinematics.tas =
                ValidatedSpeed::new_knots(tas as f32).map_err(|_| FixtureError::InvalidData {
                    field: "tas".to_string(),
                })?;
        }
        if let Some(&gs) = fixture.simvars.get("GROUND VELOCITY") {
            snapshot.kinematics.ground_speed =
                ValidatedSpeed::new_knots(gs as f32).map_err(|_| FixtureError::InvalidData {
                    field: "ground_speed".to_string(),
                })?;
        }
        if let Some(&aoa) = fixture.simvars.get("INCIDENCE ALPHA") {
            snapshot.kinematics.aoa =
                ValidatedAngle::new_degrees(aoa as f32).map_err(|_| FixtureError::InvalidData {
                    field: "aoa".to_string(),
                })?;
        }
        if let Some(&bank) = fixture.simvars.get("ATTITUDE BANK DEGREES") {
            snapshot.kinematics.bank = ValidatedAngle::new_degrees(bank as f32).map_err(|_| {
                FixtureError::InvalidData {
                    field: "bank".to_string(),
                }
            })?;
        }
        if let Some(&pitch) = fixture.simvars.get("ATTITUDE PITCH DEGREES") {
            snapshot.kinematics.pitch =
                ValidatedAngle::new_degrees(pitch as f32).map_err(|_| {
                    FixtureError::InvalidData {
                        field: "pitch".to_string(),
                    }
                })?;
        }
        if let Some(&heading) = fixture.simvars.get("ATTITUDE HEADING DEGREES") {
            // Normalize heading to -180 to 180
            let normalized = if heading > 180.0 {
                heading - 360.0
            } else {
                heading
            };
            snapshot.kinematics.heading =
                ValidatedAngle::new_degrees(normalized as f32).map_err(|_| {
                    FixtureError::InvalidData {
                        field: "heading".to_string(),
                    }
                })?;
        }
        if let Some(&g) = fixture.simvars.get("G FORCE") {
            snapshot.kinematics.g_force =
                GForce::new(g as f32).map_err(|_| FixtureError::InvalidData {
                    field: "g_force".to_string(),
                })?;
        }
        if let Some(&mach) = fixture.simvars.get("AIRSPEED MACH") {
            snapshot.kinematics.mach =
                Mach::new(mach as f32).map_err(|_| FixtureError::InvalidData {
                    field: "mach".to_string(),
                })?;
        }
        if let Some(&alt) = fixture.simvars.get("PLANE ALTITUDE") {
            snapshot.environment.altitude = alt as f32;
        }

        Ok(snapshot)
    }

    /// Convert an X-Plane fixture to a BusSnapshot
    pub fn xplane_to_snapshot(fixture: &XPlaneFixture) -> Result<BusSnapshot, FixtureError> {
        let aircraft_id = AircraftId::new(&fixture.aircraft);
        let mut snapshot = BusSnapshot::new(SimId::XPlane, aircraft_id);

        // Convert DataRefs to snapshot fields (X-Plane uses m/s for speeds)
        if let Some(&ias) = fixture
            .datarefs
            .get("sim/flightmodel/position/indicated_airspeed")
        {
            snapshot.kinematics.ias =
                ValidatedSpeed::new_mps(ias as f32).map_err(|_| FixtureError::InvalidData {
                    field: "ias".to_string(),
                })?;
        }
        if let Some(&tas) = fixture
            .datarefs
            .get("sim/flightmodel/position/true_airspeed")
        {
            snapshot.kinematics.tas =
                ValidatedSpeed::new_mps(tas as f32).map_err(|_| FixtureError::InvalidData {
                    field: "tas".to_string(),
                })?;
        }
        if let Some(&gs) = fixture.datarefs.get("sim/flightmodel/position/groundspeed") {
            snapshot.kinematics.ground_speed =
                ValidatedSpeed::new_mps(gs as f32).map_err(|_| FixtureError::InvalidData {
                    field: "ground_speed".to_string(),
                })?;
        }
        if let Some(&aoa) = fixture.datarefs.get("sim/flightmodel/position/alpha") {
            snapshot.kinematics.aoa =
                ValidatedAngle::new_degrees(aoa as f32).map_err(|_| FixtureError::InvalidData {
                    field: "aoa".to_string(),
                })?;
        }
        if let Some(&bank) = fixture.datarefs.get("sim/flightmodel/position/phi") {
            snapshot.kinematics.bank = ValidatedAngle::new_degrees(bank as f32).map_err(|_| {
                FixtureError::InvalidData {
                    field: "bank".to_string(),
                }
            })?;
        }
        if let Some(&pitch) = fixture.datarefs.get("sim/flightmodel/position/theta") {
            snapshot.kinematics.pitch =
                ValidatedAngle::new_degrees(pitch as f32).map_err(|_| {
                    FixtureError::InvalidData {
                        field: "pitch".to_string(),
                    }
                })?;
        }
        if let Some(&heading) = fixture.datarefs.get("sim/flightmodel/position/psi") {
            let normalized = if heading > 180.0 {
                heading - 360.0
            } else {
                heading
            };
            snapshot.kinematics.heading =
                ValidatedAngle::new_degrees(normalized as f32).map_err(|_| {
                    FixtureError::InvalidData {
                        field: "heading".to_string(),
                    }
                })?;
        }
        if let Some(&g) = fixture.datarefs.get("sim/flightmodel/forces/g_nrml") {
            snapshot.kinematics.g_force =
                GForce::new(g as f32).map_err(|_| FixtureError::InvalidData {
                    field: "g_force".to_string(),
                })?;
        }
        if let Some(&alt) = fixture.datarefs.get("sim/flightmodel/position/elevation") {
            // X-Plane uses meters, convert to feet
            snapshot.environment.altitude = (alt * 3.28084) as f32;
        }

        Ok(snapshot)
    }
}

impl FixtureConverter {
    /// Convert a DCS fixture to a BusSnapshot
    pub fn dcs_to_snapshot(fixture: &DcsFixture) -> Result<BusSnapshot, FixtureError> {
        let aircraft_id = AircraftId::new(&fixture.aircraft);
        let mut snapshot = BusSnapshot::new(SimId::Dcs, aircraft_id);

        // Convert Lua values to snapshot fields
        if let Some(ias) = fixture.lua_values.get("ias").and_then(|v| v.as_f64()) {
            snapshot.kinematics.ias =
                ValidatedSpeed::new_knots(ias as f32).map_err(|_| FixtureError::InvalidData {
                    field: "ias".to_string(),
                })?;
        }
        if let Some(tas) = fixture.lua_values.get("tas").and_then(|v| v.as_f64()) {
            snapshot.kinematics.tas =
                ValidatedSpeed::new_knots(tas as f32).map_err(|_| FixtureError::InvalidData {
                    field: "tas".to_string(),
                })?;
        }
        if let Some(alt) = fixture
            .lua_values
            .get("altitude_asl")
            .and_then(|v| v.as_f64())
        {
            snapshot.environment.altitude = alt as f32;
        }
        if let Some(heading) = fixture.lua_values.get("heading").and_then(|v| v.as_f64()) {
            snapshot.kinematics.heading =
                ValidatedAngle::new_degrees(heading as f32).map_err(|_| {
                    FixtureError::InvalidData {
                        field: "heading".to_string(),
                    }
                })?;
        }
        if let Some(pitch) = fixture.lua_values.get("pitch").and_then(|v| v.as_f64()) {
            snapshot.kinematics.pitch =
                ValidatedAngle::new_degrees(pitch as f32).map_err(|_| {
                    FixtureError::InvalidData {
                        field: "pitch".to_string(),
                    }
                })?;
        }
        if let Some(bank) = fixture.lua_values.get("bank").and_then(|v| v.as_f64()) {
            snapshot.kinematics.bank = ValidatedAngle::new_degrees(bank as f32).map_err(|_| {
                FixtureError::InvalidData {
                    field: "bank".to_string(),
                }
            })?;
        }
        if let Some(g) = fixture.lua_values.get("g_force").and_then(|v| v.as_f64()) {
            snapshot.kinematics.g_force =
                GForce::new(g as f32).map_err(|_| FixtureError::InvalidData {
                    field: "g_force".to_string(),
                })?;
        }
        if let Some(g_lat) = fixture.lua_values.get("g_lateral").and_then(|v| v.as_f64()) {
            snapshot.kinematics.g_lateral =
                GForce::new(g_lat as f32).map_err(|_| FixtureError::InvalidData {
                    field: "g_lateral".to_string(),
                })?;
        }
        if let Some(g_long) = fixture
            .lua_values
            .get("g_longitudinal")
            .and_then(|v| v.as_f64())
        {
            snapshot.kinematics.g_longitudinal =
                GForce::new(g_long as f32).map_err(|_| FixtureError::InvalidData {
                    field: "g_longitudinal".to_string(),
                })?;
        }
        if let Some(lat) = fixture.lua_values.get("latitude").and_then(|v| v.as_f64()) {
            snapshot.navigation.latitude = lat;
        }
        if let Some(lon) = fixture.lua_values.get("longitude").and_then(|v| v.as_f64()) {
            snapshot.navigation.longitude = lon;
        }

        Ok(snapshot)
    }

    /// Convert any adapter fixture to a BusSnapshot
    pub fn to_snapshot(fixture: &AdapterFixture) -> Result<BusSnapshot, FixtureError> {
        match fixture {
            AdapterFixture::Msfs(f) => Self::msfs_to_snapshot(f),
            AdapterFixture::XPlane(f) => Self::xplane_to_snapshot(f),
            AdapterFixture::Dcs(f) => Self::dcs_to_snapshot(f),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_msfs_c172_fixture() {
        let fixture = BuiltinFixtures::msfs_c172_cruise();
        assert_eq!(fixture.aircraft, "C172");
        assert!(fixture.simvars.contains_key("AIRSPEED INDICATED"));

        let snapshot = FixtureConverter::msfs_to_snapshot(&fixture).unwrap();
        assert_eq!(snapshot.sim, SimId::Msfs);
        assert_eq!(snapshot.aircraft.icao, "C172");
        assert_eq!(snapshot.kinematics.ias.to_knots(), 100.0);
    }

    #[test]
    fn test_msfs_a320_fixture() {
        let fixture = BuiltinFixtures::msfs_a320_approach();
        assert_eq!(fixture.aircraft, "A320");

        let snapshot = FixtureConverter::msfs_to_snapshot(&fixture).unwrap();
        assert_eq!(snapshot.kinematics.ias.to_knots(), 140.0);
        assert_eq!(snapshot.kinematics.g_force.value(), 1.05);
    }

    #[test]
    fn test_xplane_c172_fixture() {
        let fixture = BuiltinFixtures::xplane_c172_cruise();
        assert_eq!(fixture.aircraft, "C172");
        assert!(
            fixture
                .datarefs
                .contains_key("sim/flightmodel/position/indicated_airspeed")
        );

        let snapshot = FixtureConverter::xplane_to_snapshot(&fixture).unwrap();
        assert_eq!(snapshot.sim, SimId::XPlane);
        assert_eq!(snapshot.aircraft.icao, "C172");
        // X-Plane uses m/s, so IAS should be stored in m/s
        assert!((snapshot.kinematics.ias.value() - 56.58).abs() < 0.01);
    }

    #[test]
    fn test_xplane_b738_fixture() {
        let fixture = BuiltinFixtures::xplane_b738_takeoff();
        assert_eq!(fixture.aircraft, "B738");

        let snapshot = FixtureConverter::xplane_to_snapshot(&fixture).unwrap();
        assert_eq!(snapshot.kinematics.heading.to_degrees(), 90.0);
    }

    #[test]
    fn test_dcs_f16c_fixture() {
        let fixture = BuiltinFixtures::dcs_f16c_cruise();
        assert_eq!(fixture.aircraft, "F-16C");
        assert_eq!(fixture.session_type, "SP");

        let snapshot = FixtureConverter::dcs_to_snapshot(&fixture).unwrap();
        assert_eq!(snapshot.sim, SimId::Dcs);
        assert_eq!(snapshot.kinematics.ias.to_knots(), 350.0);
        assert_eq!(snapshot.navigation.latitude, 45.5);
    }

    #[test]
    fn test_dcs_ka50_fixture() {
        let fixture = BuiltinFixtures::dcs_ka50_hover();
        assert_eq!(fixture.aircraft, "Ka-50");

        let snapshot = FixtureConverter::dcs_to_snapshot(&fixture).unwrap();
        assert_eq!(snapshot.kinematics.ias.to_knots(), 5.0);
        assert_eq!(snapshot.kinematics.heading.to_degrees(), 180.0);
    }

    #[test]
    fn test_dcs_a10c_fixture() {
        let fixture = BuiltinFixtures::dcs_a10c_attack();
        assert_eq!(fixture.aircraft, "A-10C");

        let snapshot = FixtureConverter::dcs_to_snapshot(&fixture).unwrap();
        assert_eq!(snapshot.kinematics.pitch.to_degrees(), -15.0);
        assert_eq!(snapshot.kinematics.g_force.value(), 1.5);
    }

    #[test]
    fn test_adapter_fixture_enum() {
        let msfs = AdapterFixture::Msfs(BuiltinFixtures::msfs_c172_cruise());
        assert_eq!(msfs.adapter_type(), AdapterType::Msfs);
        assert_eq!(msfs.aircraft_id().icao, "C172");

        let xplane = AdapterFixture::XPlane(BuiltinFixtures::xplane_c172_cruise());
        assert_eq!(xplane.adapter_type(), AdapterType::XPlane);

        let dcs = AdapterFixture::Dcs(BuiltinFixtures::dcs_f16c_cruise());
        assert_eq!(dcs.adapter_type(), AdapterType::Dcs);
    }

    #[test]
    fn test_fixture_converter_generic() {
        let fixtures = vec![
            AdapterFixture::Msfs(BuiltinFixtures::msfs_c172_cruise()),
            AdapterFixture::XPlane(BuiltinFixtures::xplane_c172_cruise()),
            AdapterFixture::Dcs(BuiltinFixtures::dcs_f16c_cruise()),
        ];

        for fixture in fixtures {
            let snapshot = FixtureConverter::to_snapshot(&fixture).unwrap();
            assert_eq!(snapshot.sim, fixture.adapter_type().sim_id());
        }
    }

    #[test]
    fn test_no_nan_inf_in_fixtures() {
        let fixtures = vec![
            AdapterFixture::Msfs(BuiltinFixtures::msfs_c172_cruise()),
            AdapterFixture::Msfs(BuiltinFixtures::msfs_a320_approach()),
            AdapterFixture::XPlane(BuiltinFixtures::xplane_c172_cruise()),
            AdapterFixture::XPlane(BuiltinFixtures::xplane_b738_takeoff()),
            AdapterFixture::Dcs(BuiltinFixtures::dcs_f16c_cruise()),
            AdapterFixture::Dcs(BuiltinFixtures::dcs_ka50_hover()),
            AdapterFixture::Dcs(BuiltinFixtures::dcs_a10c_attack()),
        ];

        for fixture in fixtures {
            let snapshot = FixtureConverter::to_snapshot(&fixture).unwrap();

            // Check all numeric fields are finite
            assert!(
                snapshot.kinematics.ias.value().is_finite(),
                "IAS should be finite for {:?}",
                fixture.adapter_type()
            );
            assert!(
                snapshot.kinematics.tas.value().is_finite(),
                "TAS should be finite for {:?}",
                fixture.adapter_type()
            );
            assert!(
                snapshot.kinematics.g_force.value().is_finite(),
                "G-force should be finite for {:?}",
                fixture.adapter_type()
            );
            assert!(
                snapshot.environment.altitude.is_finite(),
                "Altitude should be finite for {:?}",
                fixture.adapter_type()
            );
        }
    }
}
