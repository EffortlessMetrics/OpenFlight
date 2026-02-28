// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Variable mapping and data definitions for SimConnect
//!
//! Provides mapping between SimConnect variables and normalized Flight Hub telemetry,
//! with support for different aircraft types and variable coverage matrices.

use crate::aircraft_db::{AircraftType, MsfsAircraftInfo};
use crate::var_registry::SimVarRegistry;
use flight_bus::adapters::msfs::MsfsConverter;
use flight_bus::snapshot::{
    AircraftConfig, BusSnapshot, EngineData, Environment, HeloData, Kinematics, Navigation,
};
use flight_bus::types::{AutopilotState, GearPosition, Percentage};
use flight_simconnect_sys::{
    HSIMCONNECT, SIMCONNECT_DATADEFID, SIMCONNECT_DATATYPE, SIMCONNECT_PERIOD,
    SIMCONNECT_REQUESTID, SimConnectApi, constants::*,
};
use std::collections::HashMap;
use thiserror::Error;

/// Standard acceleration due to gravity in ft/s², used to convert
/// ACCELERATION BODY X/Z SimVars (ft/s²) to G-force values.
const STANDARD_GRAVITY_FT_S2: f64 = 32.174;

/// Variable mapping configuration
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MappingConfig {
    /// Aircraft-specific variable mappings
    pub aircraft_mappings: HashMap<String, AircraftMapping>,
    /// Default mapping for unknown aircraft
    pub default_mapping: AircraftMapping,
    /// Update rates for different data categories
    pub update_rates: UpdateRates,
}

/// Aircraft-specific variable mapping
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AircraftMapping {
    /// Kinematics variables
    pub kinematics: KinematicsMapping,
    /// Aircraft configuration variables
    pub config: ConfigMapping,
    /// Engine variables
    pub engines: Vec<EngineMapping>,
    /// Environment variables
    pub environment: EnvironmentMapping,
    /// Navigation variables
    pub navigation: NavigationMapping,
    /// Helicopter-specific variables (optional)
    pub helicopter: Option<HeloMapping>,
}

/// Kinematics variable mapping
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KinematicsMapping {
    pub ias: String,
    pub tas: String,
    pub ground_speed: String,
    pub aoa: String,
    pub sideslip: String,
    pub bank: String,
    pub pitch: String,
    pub heading: String,
    pub g_force: String,
    pub g_lateral: String,
    pub g_longitudinal: String,
    pub mach: String,
    pub vertical_speed: String,
}

/// Aircraft configuration variable mapping
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ConfigMapping {
    pub gear_nose: String,
    pub gear_left: String,
    pub gear_right: String,
    pub flaps: String,
    pub spoilers: String,
    pub ap_master: String,
    pub ap_altitude_hold: String,
    pub ap_heading_hold: String,
    pub ap_speed_hold: String,
    pub ap_altitude: String,
    pub ap_heading: String,
    pub ap_speed: String,
    pub lights: LightsMapping,
    pub fuel_tanks: Vec<String>,
}

/// Lights variable mapping
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LightsMapping {
    pub nav: String,
    pub beacon: String,
    pub strobe: String,
    pub landing: String,
    pub taxi: String,
    pub logo: String,
    pub wing: String,
}

/// Engine variable mapping
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EngineMapping {
    pub index: u8,
    pub running: String,
    pub rpm: String,
    pub manifold_pressure: Option<String>,
    pub egt: Option<String>,
    pub cht: Option<String>,
    pub fuel_flow: Option<String>,
    pub oil_pressure: Option<String>,
    pub oil_temperature: Option<String>,
}

/// Environment variable mapping
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EnvironmentMapping {
    pub altitude: String,
    pub pressure_altitude: String,
    pub oat: String,
    pub wind_speed: String,
    pub wind_direction: String,
    pub visibility: String,
    pub cloud_coverage: String,
}

/// Navigation variable mapping
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NavigationMapping {
    pub latitude: String,
    pub longitude: String,
    pub ground_track: String,
    pub distance_to_dest: Option<String>,
    pub time_to_dest: Option<String>,
    pub active_waypoint: Option<String>,
}

/// Helicopter variable mapping
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HeloMapping {
    pub nr: String,
    pub np: String,
    pub torque: String,
    pub collective: String,
    pub pedals: String,
}

/// Update rates for different data categories
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UpdateRates {
    /// Kinematics update rate (Hz)
    pub kinematics: f32,
    /// Configuration update rate (Hz)
    pub config: f32,
    /// Engine update rate (Hz)
    pub engines: f32,
    /// Environment update rate (Hz)
    pub environment: f32,
    /// Navigation update rate (Hz)
    pub navigation: f32,
}

impl Default for UpdateRates {
    fn default() -> Self {
        Self {
            kinematics: 60.0,
            config: 30.0,
            engines: 30.0,
            environment: 10.0,
            navigation: 5.0,
        }
    }
}

/// Variable mapping error types
#[derive(Debug, Error)]
pub enum MappingError {
    #[error("SimConnect API error: {0}")]
    SimConnect(#[from] flight_simconnect_sys::SimConnectError),
    #[error("Bus type error: {0}")]
    BusType(#[from] flight_bus::types::BusTypeError),
    #[error("Transport error: {0}")]
    Transport(#[from] crate::transport::TransportError),
    #[error("Variable not found: {0}")]
    VariableNotFound(String),
    #[error("Invalid data type for variable: {0}")]
    InvalidDataType(String),
    #[error("Data conversion error: {0}")]
    ConversionError(String),
    #[error("Aircraft mapping not found: {0}")]
    AircraftMappingNotFound(String),
}

/// SimConnect variable mapping manager
pub struct VariableMapping {
    config: MappingConfig,
    data_definitions: HashMap<SIMCONNECT_DATADEFID, DataDefinition>,
    request_mappings: HashMap<SIMCONNECT_REQUESTID, RequestMapping>,
    next_definition_id: SIMCONNECT_DATADEFID,
    next_request_id: SIMCONNECT_REQUESTID,
}

/// Data definition information
#[derive(Debug, Clone)]
struct DataDefinition {
    #[allow(dead_code)]
    id: SIMCONNECT_DATADEFID,
    variables: Vec<VariableDefinition>,
    data_size: usize,
}

/// Variable definition within a data definition
#[derive(Debug, Clone)]
struct VariableDefinition {
    #[allow(dead_code)]
    name: String,
    #[allow(dead_code)]
    units: String,
    data_type: SIMCONNECT_DATATYPE,
    offset: usize,
    #[allow(dead_code)]
    size: usize,
}

/// Request mapping information
#[derive(Debug, Clone)]
struct RequestMapping {
    #[allow(dead_code)]
    request_id: SIMCONNECT_REQUESTID,
    definition_id: SIMCONNECT_DATADEFID,
    category: DataCategory,
    period: SIMCONNECT_PERIOD,
    engine_index: Option<u8>,
}

/// Data category enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum DataCategory {
    Kinematics,
    Config,
    Engines,
    Environment,
    Navigation,
    Helicopter,
}

impl VariableMapping {
    /// Create a new variable mapping manager
    pub fn new(config: MappingConfig) -> Self {
        Self {
            config,
            data_definitions: HashMap::new(),
            request_mappings: HashMap::new(),
            next_definition_id: DATA_DEFINITION_AIRCRAFT,
            next_request_id: REQUEST_AIRCRAFT_DATA,
        }
    }

    /// Setup data definitions for an aircraft
    pub fn setup_aircraft_definitions(
        &mut self,
        api: &SimConnectApi,
        handle: HSIMCONNECT,
        aircraft_id: &str,
    ) -> Result<(), MappingError> {
        // Clone all needed data in a scoped block to end immutable borrow
        let (kin, cfg, engs, env, nav, helo) = {
            let m = self.get_aircraft_mapping(aircraft_id);
            (
                m.kinematics.clone(),
                m.config.clone(),
                m.engines.clone(),
                m.environment.clone(),
                m.navigation.clone(),
                m.helicopter.clone(),
            )
        }; // immutable borrow ends here

        // Now safe to call &mut self methods
        self.setup_kinematics_definition(api, handle, &kin)?;
        self.setup_config_definition(api, handle, &cfg)?;
        for e in &engs {
            self.setup_engine_definition(api, handle, e)?;
        }
        self.setup_environment_definition(api, handle, &env)?;
        self.setup_navigation_definition(api, handle, &nav)?;
        if let Some(h) = helo.as_ref() {
            self.setup_helicopter_definition(api, handle, h)?;
        }
        Ok(())
    }

    /// Start data requests for an aircraft
    pub fn start_data_requests(
        &mut self,
        api: &SimConnectApi,
        handle: HSIMCONNECT,
    ) -> Result<(), MappingError> {
        // Use scoped borrow pattern to avoid potential conflicts
        let requests: Vec<_> = {
            self.request_mappings
                .iter()
                .map(|(request_id, mapping)| (*request_id, mapping.definition_id, mapping.period))
                .collect()
        }; // immutable borrow ends here

        for (request_id, definition_id, period) in requests {
            api.request_data_on_sim_object(
                handle,
                request_id,
                definition_id,
                SIMCONNECT_OBJECT_ID_USER,
                period,
            )?;
        }
        Ok(())
    }

    /// Convert received data to bus snapshot
    pub fn convert_to_snapshot(
        &self,
        request_id: SIMCONNECT_REQUESTID,
        data: &[u8],
        current_snapshot: &mut BusSnapshot,
    ) -> Result<(), MappingError> {
        let mapping = self
            .request_mappings
            .get(&request_id)
            .ok_or_else(|| MappingError::VariableNotFound(format!("Request ID {}", request_id)))?;

        let definition = self
            .data_definitions
            .get(&mapping.definition_id)
            .ok_or_else(|| {
                MappingError::VariableNotFound(format!("Definition ID {}", mapping.definition_id))
            })?;

        match mapping.category {
            DataCategory::Kinematics => {
                self.convert_kinematics_data(definition, data, &mut current_snapshot.kinematics)?;
                current_snapshot.validity.attitude_valid = true;
                current_snapshot.validity.velocities_valid = true;
                current_snapshot.validity.kinematics_valid = true;
                current_snapshot.validity.aero_valid = true;
            }
            DataCategory::Config => {
                self.convert_config_data(definition, data, &mut current_snapshot.config)?;
            }
            DataCategory::Engines => {
                self.convert_engine_data(
                    definition,
                    data,
                    &mut current_snapshot.engines,
                    mapping.engine_index,
                )?;
            }
            DataCategory::Environment => {
                self.convert_environment_data(definition, data, &mut current_snapshot.environment)?;
            }
            DataCategory::Navigation => {
                self.convert_navigation_data(definition, data, &mut current_snapshot.navigation)?;
                current_snapshot.validity.position_valid = true;
            }
            DataCategory::Helicopter => {
                if current_snapshot.helo.is_none() {
                    current_snapshot.helo = Some(HeloData {
                        nr: Percentage::new(0.0).unwrap(),
                        np: Percentage::new(0.0).unwrap(),
                        torque: Percentage::new(0.0).unwrap(),
                        collective: Percentage::new(0.0).unwrap(),
                        pedals: 0.0,
                    });
                }
                if let Some(ref mut helo) = current_snapshot.helo {
                    self.convert_helicopter_data(definition, data, helo)?;
                }
            }
        }

        Ok(())
    }

    fn get_aircraft_mapping(&self, aircraft_id: &str) -> &AircraftMapping {
        self.config
            .aircraft_mappings
            .get(aircraft_id)
            .unwrap_or(&self.config.default_mapping)
    }

    fn setup_kinematics_definition(
        &mut self,
        api: &SimConnectApi,
        handle: HSIMCONNECT,
        mapping: &KinematicsMapping,
    ) -> Result<(), MappingError> {
        let def_id = self.next_definition_id;
        self.next_definition_id += 1;

        let mut variables = Vec::new();
        let mut offset = 0;

        // Add all kinematics variables
        let kinematics_vars: &[(&String, &str, SIMCONNECT_DATATYPE)] = &[
            (&mapping.ias, "knots", SIMCONNECT_DATATYPE::FLOAT64),
            (&mapping.tas, "knots", SIMCONNECT_DATATYPE::FLOAT64),
            (&mapping.ground_speed, "knots", SIMCONNECT_DATATYPE::FLOAT64),
            (&mapping.aoa, "degrees", SIMCONNECT_DATATYPE::FLOAT64),
            (&mapping.sideslip, "degrees", SIMCONNECT_DATATYPE::FLOAT64),
            (&mapping.bank, "degrees", SIMCONNECT_DATATYPE::FLOAT64),
            (&mapping.pitch, "degrees", SIMCONNECT_DATATYPE::FLOAT64),
            (&mapping.heading, "degrees", SIMCONNECT_DATATYPE::FLOAT64),
            (&mapping.g_force, "gforce", SIMCONNECT_DATATYPE::FLOAT64),
            // ACCELERATION BODY X/Z are returned in feet per second squared.
            // Divide by standard gravity (32.174 ft/s²) to get G.
            (
                &mapping.g_lateral,
                "feet per second squared",
                SIMCONNECT_DATATYPE::FLOAT64,
            ),
            (
                &mapping.g_longitudinal,
                "feet per second squared",
                SIMCONNECT_DATATYPE::FLOAT64,
            ),
            (&mapping.mach, "mach", SIMCONNECT_DATATYPE::FLOAT64),
            (
                &mapping.vertical_speed,
                "feet per minute",
                SIMCONNECT_DATATYPE::FLOAT64,
            ),
        ];

        for (i, (var_name, units, data_type)) in kinematics_vars.iter().enumerate() {
            api.add_to_data_definition(handle, def_id, var_name, units, *data_type, 0.0, i as u32)?;

            let size = match data_type {
                SIMCONNECT_DATATYPE::FLOAT64 => 8,
                SIMCONNECT_DATATYPE::FLOAT32 => 4,
                SIMCONNECT_DATATYPE::INT32 => 4,
                _ => 4,
            };

            variables.push(VariableDefinition {
                name: var_name.to_string(),
                units: units.to_string(),
                data_type: *data_type,
                offset,
                size,
            });

            offset += size;
        }

        let definition = DataDefinition {
            id: def_id,
            variables,
            data_size: offset,
        };

        self.data_definitions.insert(def_id, definition);

        // Create request mapping
        let request_id = self.next_request_id;
        self.next_request_id += 1;

        let period = hz_to_period(self.config.update_rates.kinematics);
        self.request_mappings.insert(
            request_id,
            RequestMapping {
                request_id,
                definition_id: def_id,
                category: DataCategory::Kinematics,
                period,
                engine_index: None,
            },
        );

        Ok(())
    }

    fn setup_config_definition(
        &mut self,
        api: &SimConnectApi,
        handle: HSIMCONNECT,
        mapping: &ConfigMapping,
    ) -> Result<(), MappingError> {
        let def_id = self.next_definition_id;
        self.next_definition_id += 1;

        let mut variables = Vec::new();
        let mut offset = 0;

        // Add configuration variables
        let config_vars = [
            (&mapping.gear_nose, "percent", SIMCONNECT_DATATYPE::FLOAT64),
            (&mapping.gear_left, "percent", SIMCONNECT_DATATYPE::FLOAT64),
            (&mapping.gear_right, "percent", SIMCONNECT_DATATYPE::FLOAT64),
            (&mapping.flaps, "percent", SIMCONNECT_DATATYPE::FLOAT64),
            (&mapping.spoilers, "percent", SIMCONNECT_DATATYPE::FLOAT64),
            (&mapping.ap_master, "bool", SIMCONNECT_DATATYPE::INT32),
            (
                &mapping.ap_altitude_hold,
                "bool",
                SIMCONNECT_DATATYPE::INT32,
            ),
            (&mapping.ap_heading_hold, "bool", SIMCONNECT_DATATYPE::INT32),
            (&mapping.ap_speed_hold, "bool", SIMCONNECT_DATATYPE::INT32),
            (&mapping.ap_altitude, "feet", SIMCONNECT_DATATYPE::FLOAT64),
            (&mapping.ap_heading, "degrees", SIMCONNECT_DATATYPE::FLOAT64),
            (&mapping.ap_speed, "knots", SIMCONNECT_DATATYPE::FLOAT64),
        ];

        for (i, (var_name, units, data_type)) in config_vars.iter().enumerate() {
            api.add_to_data_definition(handle, def_id, var_name, units, *data_type, 0.0, i as u32)?;

            let size = match data_type {
                SIMCONNECT_DATATYPE::FLOAT64 => 8,
                SIMCONNECT_DATATYPE::FLOAT32 => 4,
                SIMCONNECT_DATATYPE::INT32 => 4,
                _ => 4,
            };

            variables.push(VariableDefinition {
                name: var_name.to_string(),
                units: units.to_string(),
                data_type: *data_type,
                offset,
                size,
            });

            offset += size;
        }

        // Add lights variables
        let lights_vars = [
            (&mapping.lights.nav, "bool", SIMCONNECT_DATATYPE::INT32),
            (&mapping.lights.beacon, "bool", SIMCONNECT_DATATYPE::INT32),
            (&mapping.lights.strobe, "bool", SIMCONNECT_DATATYPE::INT32),
            (&mapping.lights.landing, "bool", SIMCONNECT_DATATYPE::INT32),
            (&mapping.lights.taxi, "bool", SIMCONNECT_DATATYPE::INT32),
            (&mapping.lights.logo, "bool", SIMCONNECT_DATATYPE::INT32),
            (&mapping.lights.wing, "bool", SIMCONNECT_DATATYPE::INT32),
        ];

        for (i, (var_name, units, data_type)) in lights_vars.iter().enumerate() {
            let datum_id = config_vars.len() + i;
            api.add_to_data_definition(
                handle,
                def_id,
                var_name,
                units,
                *data_type,
                0.0,
                datum_id as u32,
            )?;

            variables.push(VariableDefinition {
                name: var_name.to_string(),
                units: units.to_string(),
                data_type: *data_type,
                offset,
                size: 4,
            });

            offset += 4;
        }

        // Add fuel quantity variables. We keep the values in the generic config fuel map.
        for (i, tank_var) in mapping.fuel_tanks.iter().enumerate() {
            let datum_id = config_vars.len() + lights_vars.len() + i;
            api.add_to_data_definition(
                handle,
                def_id,
                tank_var,
                "gallons",
                SIMCONNECT_DATATYPE::FLOAT64,
                0.0,
                datum_id as u32,
            )?;

            variables.push(VariableDefinition {
                name: tank_var.clone(),
                units: "gallons".to_string(),
                data_type: SIMCONNECT_DATATYPE::FLOAT64,
                offset,
                size: 8,
            });

            offset += 8;
        }

        let definition = DataDefinition {
            id: def_id,
            variables,
            data_size: offset,
        };

        self.data_definitions.insert(def_id, definition);

        // Create request mapping
        let request_id = self.next_request_id;
        self.next_request_id += 1;

        let period = hz_to_period(self.config.update_rates.config);
        self.request_mappings.insert(
            request_id,
            RequestMapping {
                request_id,
                definition_id: def_id,
                category: DataCategory::Config,
                period,
                engine_index: None,
            },
        );

        Ok(())
    }

    fn setup_engine_definition(
        &mut self,
        api: &SimConnectApi,
        handle: HSIMCONNECT,
        mapping: &EngineMapping,
    ) -> Result<(), MappingError> {
        let def_id = self.next_definition_id;
        self.next_definition_id += 1;

        let mut variables = Vec::new();
        let mut offset = 0;
        let mut datum_id = 0u32;

        // Required fields
        api.add_to_data_definition(
            handle,
            def_id,
            &mapping.running,
            "bool",
            SIMCONNECT_DATATYPE::INT32,
            0.0,
            datum_id,
        )?;
        variables.push(VariableDefinition {
            name: mapping.running.clone(),
            units: "bool".to_string(),
            data_type: SIMCONNECT_DATATYPE::INT32,
            offset,
            size: 4,
        });
        offset += 4;
        datum_id += 1;

        api.add_to_data_definition(
            handle,
            def_id,
            &mapping.rpm,
            "percent",
            SIMCONNECT_DATATYPE::FLOAT64,
            0.0,
            datum_id,
        )?;
        variables.push(VariableDefinition {
            name: mapping.rpm.clone(),
            units: "percent".to_string(),
            data_type: SIMCONNECT_DATATYPE::FLOAT64,
            offset,
            size: 8,
        });
        offset += 8;
        datum_id += 1;

        // Optional fields
        add_optional_engine_var(
            api,
            handle,
            def_id,
            &mut datum_id,
            &mut offset,
            &mut variables,
            &mapping.manifold_pressure,
            "inHg",
        )?;
        add_optional_engine_var(
            api,
            handle,
            def_id,
            &mut datum_id,
            &mut offset,
            &mut variables,
            &mapping.egt,
            "fahrenheit",
        )?;
        add_optional_engine_var(
            api,
            handle,
            def_id,
            &mut datum_id,
            &mut offset,
            &mut variables,
            &mapping.cht,
            "fahrenheit",
        )?;
        add_optional_engine_var(
            api,
            handle,
            def_id,
            &mut datum_id,
            &mut offset,
            &mut variables,
            &mapping.fuel_flow,
            "gallons per hour",
        )?;
        add_optional_engine_var(
            api,
            handle,
            def_id,
            &mut datum_id,
            &mut offset,
            &mut variables,
            &mapping.oil_pressure,
            "psi",
        )?;
        add_optional_engine_var(
            api,
            handle,
            def_id,
            &mut datum_id,
            &mut offset,
            &mut variables,
            &mapping.oil_temperature,
            "fahrenheit",
        )?;

        let definition = DataDefinition {
            id: def_id,
            variables,
            data_size: offset,
        };

        self.data_definitions.insert(def_id, definition);

        // Create request mapping
        let request_id = self.next_request_id;
        self.next_request_id += 1;

        let period = hz_to_period(self.config.update_rates.engines);
        self.request_mappings.insert(
            request_id,
            RequestMapping {
                request_id,
                definition_id: def_id,
                category: DataCategory::Engines,
                period,
                engine_index: Some(mapping.index),
            },
        );

        Ok(())
    }

    fn setup_environment_definition(
        &mut self,
        api: &SimConnectApi,
        handle: HSIMCONNECT,
        mapping: &EnvironmentMapping,
    ) -> Result<(), MappingError> {
        let def_id = self.next_definition_id;
        self.next_definition_id += 1;

        let mut variables = Vec::new();
        let mut offset = 0usize;

        let environment_vars = [
            (&mapping.altitude, "feet", SIMCONNECT_DATATYPE::FLOAT64),
            (
                &mapping.pressure_altitude,
                "feet",
                SIMCONNECT_DATATYPE::FLOAT64,
            ),
            (&mapping.oat, "celsius", SIMCONNECT_DATATYPE::FLOAT64),
            (&mapping.wind_speed, "knots", SIMCONNECT_DATATYPE::FLOAT64),
            (
                &mapping.wind_direction,
                "degrees",
                SIMCONNECT_DATATYPE::FLOAT64,
            ),
            (
                &mapping.visibility,
                "statute miles",
                SIMCONNECT_DATATYPE::FLOAT64,
            ),
            (
                &mapping.cloud_coverage,
                "percent",
                SIMCONNECT_DATATYPE::FLOAT64,
            ),
        ];

        for (i, (var_name, units, data_type)) in environment_vars.iter().enumerate() {
            api.add_to_data_definition(handle, def_id, var_name, units, *data_type, 0.0, i as u32)?;

            let size = size_for_datatype(*data_type);
            variables.push(VariableDefinition {
                name: var_name.to_string(),
                units: units.to_string(),
                data_type: *data_type,
                offset,
                size,
            });
            offset += size;
        }

        self.data_definitions.insert(
            def_id,
            DataDefinition {
                id: def_id,
                variables,
                data_size: offset,
            },
        );

        let request_id = self.next_request_id;
        self.next_request_id += 1;
        let period = hz_to_period(self.config.update_rates.environment);

        self.request_mappings.insert(
            request_id,
            RequestMapping {
                request_id,
                definition_id: def_id,
                category: DataCategory::Environment,
                period,
                engine_index: None,
            },
        );

        Ok(())
    }

    fn setup_navigation_definition(
        &mut self,
        api: &SimConnectApi,
        handle: HSIMCONNECT,
        mapping: &NavigationMapping,
    ) -> Result<(), MappingError> {
        let def_id = self.next_definition_id;
        self.next_definition_id += 1;

        let mut variables = Vec::new();
        let mut offset = 0usize;
        let mut datum_id = 0u32;

        let core_vars = [
            (&mapping.latitude, "degrees", SIMCONNECT_DATATYPE::FLOAT64),
            (&mapping.longitude, "degrees", SIMCONNECT_DATATYPE::FLOAT64),
            (
                &mapping.ground_track,
                "degrees",
                SIMCONNECT_DATATYPE::FLOAT64,
            ),
        ];

        for (var_name, units, data_type) in &core_vars {
            api.add_to_data_definition(handle, def_id, var_name, units, *data_type, 0.0, datum_id)?;
            let size = size_for_datatype(*data_type);
            variables.push(VariableDefinition {
                name: var_name.to_string(),
                units: units.to_string(),
                data_type: *data_type,
                offset,
                size,
            });
            offset += size;
            datum_id += 1;
        }

        if let Some(distance) = &mapping.distance_to_dest {
            api.add_to_data_definition(
                handle,
                def_id,
                distance,
                "meters",
                SIMCONNECT_DATATYPE::FLOAT64,
                0.0,
                datum_id,
            )?;
            variables.push(VariableDefinition {
                name: distance.clone(),
                units: "meters".to_string(),
                data_type: SIMCONNECT_DATATYPE::FLOAT64,
                offset,
                size: 8,
            });
            offset += 8;
            datum_id += 1;
        }

        if let Some(time_to_dest) = &mapping.time_to_dest {
            api.add_to_data_definition(
                handle,
                def_id,
                time_to_dest,
                "seconds",
                SIMCONNECT_DATATYPE::FLOAT64,
                0.0,
                datum_id,
            )?;
            variables.push(VariableDefinition {
                name: time_to_dest.clone(),
                units: "seconds".to_string(),
                data_type: SIMCONNECT_DATATYPE::FLOAT64,
                offset,
                size: 8,
            });
            offset += 8;
            datum_id += 1;
        }

        if let Some(active_waypoint) = &mapping.active_waypoint {
            api.add_to_data_definition(
                handle,
                def_id,
                active_waypoint,
                "",
                SIMCONNECT_DATATYPE::STRING32,
                0.0,
                datum_id,
            )?;
            variables.push(VariableDefinition {
                name: active_waypoint.clone(),
                units: "".to_string(),
                data_type: SIMCONNECT_DATATYPE::STRING32,
                offset,
                size: size_for_datatype(SIMCONNECT_DATATYPE::STRING32),
            });
            offset += size_for_datatype(SIMCONNECT_DATATYPE::STRING32);
        }

        self.data_definitions.insert(
            def_id,
            DataDefinition {
                id: def_id,
                variables,
                data_size: offset,
            },
        );

        let request_id = self.next_request_id;
        self.next_request_id += 1;
        let period = hz_to_period(self.config.update_rates.navigation);

        self.request_mappings.insert(
            request_id,
            RequestMapping {
                request_id,
                definition_id: def_id,
                category: DataCategory::Navigation,
                period,
                engine_index: None,
            },
        );

        Ok(())
    }

    fn setup_helicopter_definition(
        &mut self,
        api: &SimConnectApi,
        handle: HSIMCONNECT,
        mapping: &HeloMapping,
    ) -> Result<(), MappingError> {
        let def_id = self.next_definition_id;
        self.next_definition_id += 1;

        let mut variables = Vec::new();
        let mut offset = 0usize;

        let helo_vars = [
            (&mapping.nr, "percent", SIMCONNECT_DATATYPE::FLOAT64),
            (&mapping.np, "percent", SIMCONNECT_DATATYPE::FLOAT64),
            (&mapping.torque, "percent", SIMCONNECT_DATATYPE::FLOAT64),
            (&mapping.collective, "percent", SIMCONNECT_DATATYPE::FLOAT64),
            (&mapping.pedals, "position", SIMCONNECT_DATATYPE::FLOAT64),
        ];

        for (i, (var_name, units, data_type)) in helo_vars.iter().enumerate() {
            api.add_to_data_definition(handle, def_id, var_name, units, *data_type, 0.0, i as u32)?;
            let size = size_for_datatype(*data_type);
            variables.push(VariableDefinition {
                name: var_name.to_string(),
                units: units.to_string(),
                data_type: *data_type,
                offset,
                size,
            });
            offset += size;
        }

        self.data_definitions.insert(
            def_id,
            DataDefinition {
                id: def_id,
                variables,
                data_size: offset,
            },
        );

        let request_id = self.next_request_id;
        self.next_request_id += 1;

        self.request_mappings.insert(
            request_id,
            RequestMapping {
                request_id,
                definition_id: def_id,
                category: DataCategory::Helicopter,
                period: hz_to_period(self.config.update_rates.config),
                engine_index: None,
            },
        );

        Ok(())
    }

    fn convert_kinematics_data(
        &self,
        definition: &DataDefinition,
        data: &[u8],
        kinematics: &mut Kinematics,
    ) -> Result<(), MappingError> {
        if data.len() < definition.data_size {
            return Err(MappingError::ConversionError(
                "Insufficient data".to_string(),
            ));
        }

        // Extract values from data buffer based on variable definitions
        for (i, var_def) in definition.variables.iter().enumerate() {
            let value = match var_def.data_type {
                SIMCONNECT_DATATYPE::FLOAT64 => {
                    if data.len() >= var_def.offset + 8 {
                        f64::from_le_bytes([
                            data[var_def.offset],
                            data[var_def.offset + 1],
                            data[var_def.offset + 2],
                            data[var_def.offset + 3],
                            data[var_def.offset + 4],
                            data[var_def.offset + 5],
                            data[var_def.offset + 6],
                            data[var_def.offset + 7],
                        ])
                    } else {
                        continue;
                    }
                }
                _ => continue,
            };

            // Map to kinematics fields based on variable index
            match i {
                0 => kinematics.ias = MsfsConverter::convert_ias(value)?,
                1 => kinematics.tas = MsfsConverter::convert_tas(value)?,
                2 => kinematics.ground_speed = MsfsConverter::convert_ground_speed(value)?,
                3 => kinematics.aoa = MsfsConverter::convert_angle_degrees(value)?,
                4 => kinematics.sideslip = MsfsConverter::convert_angle_degrees(value)?,
                5 => kinematics.bank = MsfsConverter::convert_angle_degrees(value)?,
                6 => kinematics.pitch = MsfsConverter::convert_angle_degrees(value)?,
                7 => kinematics.heading = MsfsConverter::convert_angle_degrees(value)?,
                8 => kinematics.g_force = MsfsConverter::convert_g_force(value)?,
                // ACCELERATION BODY X/Z arrive in ft/s²; convert to G
                9 => {
                    kinematics.g_lateral =
                        MsfsConverter::convert_g_force(value / STANDARD_GRAVITY_FT_S2)?
                }
                10 => {
                    kinematics.g_longitudinal =
                        MsfsConverter::convert_g_force(value / STANDARD_GRAVITY_FT_S2)?
                }
                11 => kinematics.mach = MsfsConverter::convert_mach(value)?,
                12 => kinematics.vertical_speed = value as f32,
                _ => {}
            }
        }

        Ok(())
    }

    fn convert_config_data(
        &self,
        definition: &DataDefinition,
        data: &[u8],
        config: &mut AircraftConfig,
    ) -> Result<(), MappingError> {
        if data.len() < definition.data_size {
            return Err(MappingError::ConversionError(
                "Insufficient data".to_string(),
            ));
        }

        let mut ap_master = false;
        let mut ap_altitude_hold = false;
        let mut ap_heading_hold = false;
        let mut ap_speed_hold = false;

        for (i, var_def) in definition.variables.iter().enumerate() {
            match i {
                0 => {
                    if let Some(value) = read_numeric_as_f64(data, var_def) {
                        config.gear.nose = percentage_to_gear_position(value as f32);
                    }
                }
                1 => {
                    if let Some(value) = read_numeric_as_f64(data, var_def) {
                        config.gear.left = percentage_to_gear_position(value as f32);
                    }
                }
                2 => {
                    if let Some(value) = read_numeric_as_f64(data, var_def) {
                        config.gear.right = percentage_to_gear_position(value as f32);
                    }
                }
                3 => {
                    if let Some(value) = read_numeric_as_f64(data, var_def) {
                        config.flaps = normalize_percentage_value(value as f32)?;
                    }
                }
                4 => {
                    if let Some(value) = read_numeric_as_f64(data, var_def) {
                        config.spoilers = normalize_percentage_value(value as f32)?;
                    }
                }
                5 => {
                    ap_master = read_bool(data, var_def).unwrap_or(false);
                }
                6 => {
                    ap_altitude_hold = read_bool(data, var_def).unwrap_or(false);
                }
                7 => {
                    ap_heading_hold = read_bool(data, var_def).unwrap_or(false);
                }
                8 => {
                    ap_speed_hold = read_bool(data, var_def).unwrap_or(false);
                }
                9 => {
                    config.ap_altitude = read_numeric_as_f64(data, var_def).map(|v| v as f32);
                }
                10 => {
                    config.ap_heading = match read_numeric_as_f64(data, var_def) {
                        Some(value) => Some(MsfsConverter::convert_angle_degrees(value)?),
                        None => None,
                    };
                }
                11 => {
                    config.ap_speed = match read_numeric_as_f64(data, var_def) {
                        Some(value) => Some(MsfsConverter::convert_ias(value)?),
                        None => None,
                    };
                }
                12 => config.lights.nav = read_bool(data, var_def).unwrap_or(false),
                13 => config.lights.beacon = read_bool(data, var_def).unwrap_or(false),
                14 => config.lights.strobe = read_bool(data, var_def).unwrap_or(false),
                15 => config.lights.landing = read_bool(data, var_def).unwrap_or(false),
                16 => config.lights.taxi = read_bool(data, var_def).unwrap_or(false),
                17 => config.lights.logo = read_bool(data, var_def).unwrap_or(false),
                18 => config.lights.wing = read_bool(data, var_def).unwrap_or(false),
                _ => {
                    if let Some(value) = read_numeric_as_f64(data, var_def) {
                        let key = fuel_tank_key(&var_def.name);
                        config
                            .fuel
                            .insert(key, normalize_percentage_value(value as f32)?);
                    }
                }
            }
        }

        config.ap_state = if !ap_master {
            AutopilotState::Off
        } else if ap_altitude_hold || ap_heading_hold || ap_speed_hold {
            AutopilotState::Engaged
        } else {
            AutopilotState::Armed
        };

        Ok(())
    }

    fn convert_engine_data(
        &self,
        definition: &DataDefinition,
        data: &[u8],
        engines: &mut Vec<EngineData>,
        engine_index: Option<u8>,
    ) -> Result<(), MappingError> {
        if data.len() < definition.data_size {
            return Err(MappingError::ConversionError(
                "Insufficient data".to_string(),
            ));
        }

        let target_index = engine_index.unwrap_or(0);
        let engine = get_or_create_engine(engines, target_index);

        for var_def in &definition.variables {
            let name = var_def.name.to_ascii_uppercase();

            if name.contains("COMBUSTION") {
                engine.running = read_bool(data, var_def).unwrap_or(false);
                continue;
            }

            if name.contains("RPM") && !name.contains("OIL") {
                if let Some(value) = read_numeric_as_f64(data, var_def) {
                    engine.rpm = normalize_percentage_value(value as f32)?;
                }
                continue;
            }

            if name.contains("MANIFOLD") {
                engine.manifold_pressure = read_numeric_as_f64(data, var_def).map(|v| v as f32);
                continue;
            }

            if name.contains("EXHAUST GAS") || name.contains("EGT") {
                engine.egt =
                    read_numeric_as_f64(data, var_def).map(|v| fahrenheit_to_celsius(v as f32));
                continue;
            }

            if name.contains("CYLINDER HEAD") || name.contains("CHT") {
                engine.cht =
                    read_numeric_as_f64(data, var_def).map(|v| fahrenheit_to_celsius(v as f32));
                continue;
            }

            if name.contains("FUEL FLOW") {
                engine.fuel_flow = read_numeric_as_f64(data, var_def).map(|v| v as f32);
                continue;
            }

            if name.contains("OIL PRESSURE") {
                engine.oil_pressure = read_numeric_as_f64(data, var_def).map(|v| v as f32);
                continue;
            }

            if name.contains("OIL TEMPERATURE") {
                engine.oil_temperature =
                    read_numeric_as_f64(data, var_def).map(|v| fahrenheit_to_celsius(v as f32));
            }
        }

        Ok(())
    }

    fn convert_environment_data(
        &self,
        definition: &DataDefinition,
        data: &[u8],
        environment: &mut Environment,
    ) -> Result<(), MappingError> {
        if data.len() < definition.data_size {
            return Err(MappingError::ConversionError(
                "Insufficient data".to_string(),
            ));
        }

        for (i, var_def) in definition.variables.iter().enumerate() {
            let value = match read_numeric_as_f64(data, var_def) {
                Some(value) => value,
                None => continue,
            };

            match i {
                0 => environment.altitude = value as f32,
                1 => environment.pressure_altitude = value as f32,
                2 => environment.oat = value as f32,
                3 => environment.wind_speed = MsfsConverter::convert_ground_speed(value)?,
                4 => environment.wind_direction = MsfsConverter::convert_angle_degrees(value)?,
                5 => environment.visibility = value as f32,
                6 => environment.cloud_coverage = normalize_percentage_value(value as f32)?,
                _ => {}
            }
        }

        Ok(())
    }

    fn convert_navigation_data(
        &self,
        definition: &DataDefinition,
        data: &[u8],
        navigation: &mut Navigation,
    ) -> Result<(), MappingError> {
        if data.len() < definition.data_size {
            return Err(MappingError::ConversionError(
                "Insufficient data".to_string(),
            ));
        }

        for (i, var_def) in definition.variables.iter().enumerate() {
            match i {
                0 => {
                    if let Some(value) = read_numeric_as_f64(data, var_def) {
                        navigation.latitude = value;
                    }
                }
                1 => {
                    if let Some(value) = read_numeric_as_f64(data, var_def) {
                        navigation.longitude = value;
                    }
                }
                2 => {
                    if let Some(value) = read_numeric_as_f64(data, var_def) {
                        navigation.ground_track = MsfsConverter::convert_angle_degrees(value)?;
                    }
                }
                _ => match var_def.data_type {
                    SIMCONNECT_DATATYPE::STRING8
                    | SIMCONNECT_DATATYPE::STRING32
                    | SIMCONNECT_DATATYPE::STRING64
                    | SIMCONNECT_DATATYPE::STRING128
                    | SIMCONNECT_DATATYPE::STRING256
                    | SIMCONNECT_DATATYPE::STRING260 => {
                        navigation.active_waypoint = read_string(data, var_def);
                    }
                    _ => {
                        if let Some(value) = read_numeric_as_f64(data, var_def) {
                            if navigation.distance_to_dest.is_none() {
                                navigation.distance_to_dest = Some(value as f32);
                            } else {
                                navigation.time_to_dest = Some(value as f32);
                            }
                        }
                    }
                },
            }
        }

        Ok(())
    }

    fn convert_helicopter_data(
        &self,
        definition: &DataDefinition,
        data: &[u8],
        helo: &mut HeloData,
    ) -> Result<(), MappingError> {
        if data.len() < definition.data_size {
            return Err(MappingError::ConversionError(
                "Insufficient data".to_string(),
            ));
        }

        for (i, var_def) in definition.variables.iter().enumerate() {
            let value = match read_numeric_as_f64(data, var_def) {
                Some(value) => value as f32,
                None => continue,
            };

            match i {
                0 => helo.nr = normalize_percentage_value(value)?,
                1 => helo.np = normalize_percentage_value(value)?,
                2 => helo.torque = normalize_percentage_value(value)?,
                3 => helo.collective = normalize_percentage_value(value)?,
                4 => {
                    helo.pedals = if value.abs() <= 1.0 {
                        value * 100.0
                    } else {
                        value
                    }
                    .clamp(-100.0, 100.0)
                }
                _ => {}
            }
        }

        Ok(())
    }
}

fn add_optional_engine_var(
    api: &SimConnectApi,
    handle: HSIMCONNECT,
    definition_id: SIMCONNECT_DATADEFID,
    datum_id: &mut u32,
    offset: &mut usize,
    variables: &mut Vec<VariableDefinition>,
    variable_name: &Option<String>,
    units: &str,
) -> Result<(), MappingError> {
    if let Some(var_name) = variable_name {
        api.add_to_data_definition(
            handle,
            definition_id,
            var_name,
            units,
            SIMCONNECT_DATATYPE::FLOAT64,
            0.0,
            *datum_id,
        )?;

        variables.push(VariableDefinition {
            name: var_name.clone(),
            units: units.to_string(),
            data_type: SIMCONNECT_DATATYPE::FLOAT64,
            offset: *offset,
            size: 8,
        });

        *datum_id += 1;
        *offset += 8;
    }

    Ok(())
}

fn size_for_datatype(data_type: SIMCONNECT_DATATYPE) -> usize {
    match data_type {
        SIMCONNECT_DATATYPE::INT32 => 4,
        SIMCONNECT_DATATYPE::INT64 => 8,
        SIMCONNECT_DATATYPE::FLOAT32 => 4,
        SIMCONNECT_DATATYPE::FLOAT64 => 8,
        SIMCONNECT_DATATYPE::STRING8 => 8,
        SIMCONNECT_DATATYPE::STRING32 => 32,
        SIMCONNECT_DATATYPE::STRING64 => 64,
        SIMCONNECT_DATATYPE::STRING128 => 128,
        SIMCONNECT_DATATYPE::STRING256 => 256,
        SIMCONNECT_DATATYPE::STRING260 => 260,
        _ => 0,
    }
}

fn read_numeric_as_f64(data: &[u8], var_def: &VariableDefinition) -> Option<f64> {
    match var_def.data_type {
        SIMCONNECT_DATATYPE::FLOAT64 if data.len() >= var_def.offset + 8 => Some(
            f64::from_le_bytes(data[var_def.offset..var_def.offset + 8].try_into().ok()?),
        ),
        SIMCONNECT_DATATYPE::FLOAT32 if data.len() >= var_def.offset + 4 => Some(
            f32::from_le_bytes(data[var_def.offset..var_def.offset + 4].try_into().ok()?) as f64,
        ),
        SIMCONNECT_DATATYPE::INT32 if data.len() >= var_def.offset + 4 => Some(i32::from_le_bytes(
            data[var_def.offset..var_def.offset + 4].try_into().ok()?,
        ) as f64),
        SIMCONNECT_DATATYPE::INT64 if data.len() >= var_def.offset + 8 => Some(i64::from_le_bytes(
            data[var_def.offset..var_def.offset + 8].try_into().ok()?,
        ) as f64),
        _ => None,
    }
}

fn read_bool(data: &[u8], var_def: &VariableDefinition) -> Option<bool> {
    read_numeric_as_f64(data, var_def).map(|value| value != 0.0)
}

fn read_string(data: &[u8], var_def: &VariableDefinition) -> Option<String> {
    let size = var_def.size;
    if size == 0 || data.len() < var_def.offset + size {
        return None;
    }

    let bytes = &data[var_def.offset..var_def.offset + size];
    let null_pos = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    let value = String::from_utf8_lossy(&bytes[..null_pos])
        .trim()
        .to_string();

    if value.is_empty() { None } else { Some(value) }
}

fn normalize_percentage_value(value: f32) -> Result<Percentage, MappingError> {
    if (0.0..=1.0).contains(&value) {
        return Ok(Percentage::from_normalized(value)?);
    }

    Percentage::new(value.clamp(0.0, 100.0)).map_err(MappingError::BusType)
}

fn percentage_to_gear_position(value: f32) -> GearPosition {
    let normalized = if value <= 1.0 { value * 100.0 } else { value };
    if normalized <= 5.0 {
        GearPosition::Up
    } else if normalized >= 95.0 {
        GearPosition::Down
    } else {
        GearPosition::Transitioning
    }
}

fn get_or_create_engine(engines: &mut Vec<EngineData>, index: u8) -> &mut EngineData {
    if let Some(pos) = engines.iter().position(|engine| engine.index == index) {
        return &mut engines[pos];
    }

    engines.push(EngineData {
        index,
        running: false,
        rpm: Percentage::new(0.0).expect("0% RPM is valid"),
        manifold_pressure: None,
        egt: None,
        cht: None,
        fuel_flow: None,
        oil_pressure: None,
        oil_temperature: None,
    });

    engines
        .last_mut()
        .expect("engine list cannot be empty after push")
}

fn fuel_tank_key(simvar_name: &str) -> String {
    let uppercase = simvar_name.to_ascii_uppercase();
    let tank_name = uppercase
        .replace("FUEL TANK ", "")
        .replace(" QUANTITY", "")
        .replace(" TANK", "")
        .trim()
        .to_ascii_lowercase();

    tank_name.split_whitespace().collect::<Vec<_>>().join("_")
}

fn fahrenheit_to_celsius(value_f: f32) -> f32 {
    (value_f - 32.0) * (5.0 / 9.0)
}

/// Convert update rate in Hz to SimConnect period
fn hz_to_period(hz: f32) -> SIMCONNECT_PERIOD {
    if hz >= 60.0 {
        SIMCONNECT_PERIOD::VISUAL_FRAME
    } else if hz >= 30.0 {
        SIMCONNECT_PERIOD::SIM_FRAME
    } else {
        SIMCONNECT_PERIOD::SECOND
    }
}

/// Create default mapping configuration
pub fn create_default_mapping() -> MappingConfig {
    let default_kinematics = KinematicsMapping {
        ias: "AIRSPEED INDICATED".to_string(),
        tas: "AIRSPEED TRUE".to_string(),
        ground_speed: "GROUND VELOCITY".to_string(),
        aoa: "INCIDENCE ALPHA".to_string(),
        sideslip: "INCIDENCE BETA".to_string(),
        bank: "PLANE BANK DEGREES".to_string(),
        pitch: "PLANE PITCH DEGREES".to_string(),
        heading: "PLANE HEADING DEGREES MAGNETIC".to_string(),
        g_force: "G FORCE".to_string(),
        g_lateral: "ACCELERATION BODY X".to_string(),
        g_longitudinal: "ACCELERATION BODY Z".to_string(),
        mach: "AIRSPEED MACH".to_string(),
        vertical_speed: "VERTICAL SPEED".to_string(),
    };

    let default_config = ConfigMapping {
        gear_nose: "GEAR CENTER POSITION".to_string(),
        gear_left: "GEAR LEFT POSITION".to_string(),
        gear_right: "GEAR RIGHT POSITION".to_string(),
        flaps: "FLAPS HANDLE PERCENT".to_string(),
        spoilers: "SPOILERS HANDLE POSITION".to_string(),
        ap_master: "AUTOPILOT MASTER".to_string(),
        ap_altitude_hold: "AUTOPILOT ALTITUDE LOCK".to_string(),
        ap_heading_hold: "AUTOPILOT HEADING LOCK".to_string(),
        ap_speed_hold: "AUTOPILOT AIRSPEED HOLD".to_string(),
        ap_altitude: "AUTOPILOT ALTITUDE LOCK VAR".to_string(),
        ap_heading: "AUTOPILOT HEADING LOCK DIR".to_string(),
        ap_speed: "AUTOPILOT AIRSPEED HOLD VAR".to_string(),
        lights: LightsMapping {
            nav: "LIGHT NAV".to_string(),
            beacon: "LIGHT BEACON".to_string(),
            strobe: "LIGHT STROBE".to_string(),
            landing: "LIGHT LANDING".to_string(),
            taxi: "LIGHT TAXI".to_string(),
            logo: "LIGHT LOGO".to_string(),
            wing: "LIGHT WING".to_string(),
        },
        fuel_tanks: vec![
            "FUEL TANK LEFT MAIN QUANTITY".to_string(),
            "FUEL TANK RIGHT MAIN QUANTITY".to_string(),
        ],
    };

    let default_engine = EngineMapping {
        index: 0,
        running: "GENERAL ENG COMBUSTION:1".to_string(),
        rpm: "GENERAL ENG RPM:1".to_string(),
        manifold_pressure: Some("RECIP ENG MANIFOLD PRESSURE:1".to_string()),
        egt: Some("GENERAL ENG EXHAUST GAS TEMPERATURE:1".to_string()),
        cht: Some("RECIP ENG CYLINDER HEAD TEMPERATURE:1".to_string()),
        fuel_flow: Some("GENERAL ENG FUEL FLOW GPH:1".to_string()),
        oil_pressure: Some("GENERAL ENG OIL PRESSURE:1".to_string()),
        oil_temperature: Some("GENERAL ENG OIL TEMPERATURE:1".to_string()),
    };

    let default_environment = EnvironmentMapping {
        altitude: "INDICATED ALTITUDE".to_string(),
        pressure_altitude: "PRESSURE ALTITUDE".to_string(),
        oat: "AMBIENT TEMPERATURE".to_string(),
        wind_speed: "AMBIENT WIND VELOCITY".to_string(),
        wind_direction: "AMBIENT WIND DIRECTION".to_string(),
        visibility: "AMBIENT VISIBILITY".to_string(),
        cloud_coverage: "AMBIENT CLOUD COVERAGE".to_string(),
    };

    let default_navigation = NavigationMapping {
        latitude: "PLANE LATITUDE".to_string(),
        longitude: "PLANE LONGITUDE".to_string(),
        ground_track: "GPS GROUND TRUE TRACK".to_string(),
        distance_to_dest: Some("GPS WP DISTANCE".to_string()),
        time_to_dest: Some("GPS WP ETE".to_string()),
        active_waypoint: Some("GPS WP NEXT ID".to_string()),
    };

    let default_mapping = AircraftMapping {
        kinematics: default_kinematics,
        config: default_config,
        engines: vec![default_engine],
        environment: default_environment,
        navigation: default_navigation,
        helicopter: None,
    };

    MappingConfig {
        aircraft_mappings: HashMap::new(),
        default_mapping,
        update_rates: UpdateRates::default(),
    }
}

/// Create a mapping configuration for a known aircraft using the registry and aircraft DB.
///
/// This uses the aircraft type (single prop, twin jet, helicopter, etc.) to decide
/// how many engines to map and whether helicopter variables are needed.  The
/// `SimVarRegistry` is consulted to validate that referenced variables exist.
pub fn create_mapping_for_aircraft(
    db_info: &MsfsAircraftInfo,
    registry: &SimVarRegistry,
) -> MappingConfig {
    let base = create_default_mapping();

    // Determine engine count from the aircraft type.
    let engine_count: u8 = match db_info.category {
        AircraftType::SingleProp | AircraftType::Turboprop | AircraftType::SingleJet => 1,
        AircraftType::TwinProp | AircraftType::TwinJet => 2,
        AircraftType::Helicopter => 1,
        AircraftType::Glider => 0,
    };

    // Build engine mappings from the registry.
    let engines: Vec<EngineMapping> = (0..engine_count)
        .map(|i| {
            let idx = i + 1; // SimConnect engines are 1-indexed
            let rpm_var = format!("GENERAL ENG RPM:{idx}");
            let use_n1 = db_info.special_vars.iter().any(|v| v.starts_with("ENG N1"));

            EngineMapping {
                index: i,
                running: format!("GENERAL ENG COMBUSTION:{idx}"),
                rpm: if use_n1 {
                    format!("ENG N1 RPM:{idx}")
                } else if registry.contains(&rpm_var) {
                    rpm_var
                } else {
                    format!("GENERAL ENG RPM:{idx}")
                },
                manifold_pressure: if matches!(
                    db_info.category,
                    AircraftType::SingleProp | AircraftType::TwinProp
                ) {
                    Some(format!("RECIP ENG MANIFOLD PRESSURE:{idx}"))
                } else {
                    None
                },
                egt: Some(format!("GENERAL ENG EXHAUST GAS TEMPERATURE:{idx}")),
                cht: if matches!(
                    db_info.category,
                    AircraftType::SingleProp | AircraftType::TwinProp
                ) {
                    Some(format!("RECIP ENG CYLINDER HEAD TEMPERATURE:{idx}"))
                } else {
                    None
                },
                fuel_flow: Some(format!("GENERAL ENG FUEL FLOW GPH:{idx}")),
                oil_pressure: Some(format!("GENERAL ENG OIL PRESSURE:{idx}")),
                oil_temperature: Some(format!("GENERAL ENG OIL TEMPERATURE:{idx}")),
            }
        })
        .collect();

    // Helicopter-specific mapping when the DB says it's a helo.
    let helicopter = if db_info.category == AircraftType::Helicopter {
        Some(HeloMapping {
            nr: "ROTOR RPM:1".to_string(),
            np: "TURB ENG FREE TURBINE TORQUE:1".to_string(),
            torque: "ENG TORQUE PERCENT:1".to_string(),
            collective: "COLLECTIVE POSITION".to_string(),
            pedals: "RUDDER POSITION".to_string(),
        })
    } else {
        None
    };

    let aircraft_mapping = AircraftMapping {
        kinematics: base.default_mapping.kinematics.clone(),
        config: base.default_mapping.config.clone(),
        engines,
        environment: base.default_mapping.environment.clone(),
        navigation: base.default_mapping.navigation.clone(),
        helicopter,
    };

    // Register the aircraft-specific mapping under its ICAO code.
    let mut aircraft_mappings = HashMap::new();
    aircraft_mappings.insert(db_info.icao_code.to_string(), aircraft_mapping.clone());

    MappingConfig {
        aircraft_mappings,
        default_mapping: aircraft_mapping,
        update_rates: base.update_rates,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_mapping_creation() {
        let config = create_default_mapping();
        assert!(!config.default_mapping.kinematics.ias.is_empty());
        assert!(!config.default_mapping.config.gear_nose.is_empty());
        assert!(!config.default_mapping.engines.is_empty());
        assert_eq!(config.update_rates.kinematics, 60.0);
    }

    #[test]
    fn test_hz_to_period_conversion() {
        assert_eq!(hz_to_period(60.0), SIMCONNECT_PERIOD::VISUAL_FRAME);
        assert_eq!(hz_to_period(30.0), SIMCONNECT_PERIOD::SIM_FRAME);
        assert_eq!(hz_to_period(10.0), SIMCONNECT_PERIOD::SECOND);
    }

    #[test]
    fn test_variable_mapping_creation() {
        let config = create_default_mapping();
        let mapping = VariableMapping::new(config);
        assert_eq!(mapping.next_definition_id, DATA_DEFINITION_AIRCRAFT);
        assert_eq!(mapping.next_request_id, REQUEST_AIRCRAFT_DATA);
    }

    #[test]
    fn test_create_mapping_for_single_prop() {
        let registry = SimVarRegistry::new();
        let db = crate::aircraft_db::MsfsAircraftDb::new();
        let c172 = db.get("C172").unwrap();
        let config = create_mapping_for_aircraft(c172, &registry);

        // Single-prop → one engine
        assert_eq!(config.default_mapping.engines.len(), 1);
        assert!(
            config.default_mapping.engines[0]
                .manifold_pressure
                .is_some()
        );
        assert!(config.default_mapping.engines[0].cht.is_some());
        assert!(config.default_mapping.helicopter.is_none());
        assert!(config.aircraft_mappings.contains_key("C172"));
    }

    #[test]
    fn test_create_mapping_for_twin_jet() {
        let registry = SimVarRegistry::new();
        let db = crate::aircraft_db::MsfsAircraftDb::new();
        let a320 = db.get("A320").unwrap();
        let config = create_mapping_for_aircraft(a320, &registry);

        // Twin jet → two engines, using N1 because special_vars contain "ENG N1"
        assert_eq!(config.default_mapping.engines.len(), 2);
        assert!(config.default_mapping.engines[0].rpm.contains("N1"));
        // Jets have no manifold pressure or CHT
        assert!(
            config.default_mapping.engines[0]
                .manifold_pressure
                .is_none()
        );
        assert!(config.default_mapping.engines[0].cht.is_none());
        assert!(config.default_mapping.helicopter.is_none());
    }

    #[test]
    fn test_create_mapping_for_helicopter() {
        let registry = SimVarRegistry::new();
        let db = crate::aircraft_db::MsfsAircraftDb::new();
        let b06 = db.get("B06").unwrap();
        let config = create_mapping_for_aircraft(b06, &registry);

        assert_eq!(config.default_mapping.engines.len(), 1);
        assert!(config.default_mapping.helicopter.is_some());
        let helo = config.default_mapping.helicopter.as_ref().unwrap();
        assert_eq!(helo.nr, "ROTOR RPM:1");
    }

    #[test]
    fn test_create_mapping_for_glider() {
        let registry = SimVarRegistry::new();
        let db = crate::aircraft_db::MsfsAircraftDb::new();
        let dg1t = db.get("DG1T").unwrap();
        let config = create_mapping_for_aircraft(dg1t, &registry);

        assert!(config.default_mapping.engines.is_empty());
        assert!(config.default_mapping.helicopter.is_none());
    }

    // ── Data conversion pipeline tests ─────────────────────────────────

    /// Helper: write an f64 value as little-endian bytes into a buffer.
    fn write_f64(buf: &mut Vec<u8>, value: f64) {
        buf.extend_from_slice(&value.to_le_bytes());
    }

    /// Helper: write an i32 value as little-endian bytes into a buffer.
    fn write_i32(buf: &mut Vec<u8>, value: i32) {
        buf.extend_from_slice(&value.to_le_bytes());
    }

    /// Build a VariableMapping with a kinematics definition already set up.
    fn build_kinematics_mapping() -> (VariableMapping, SIMCONNECT_REQUESTID) {
        let config = create_default_mapping();
        let mut mapping = VariableMapping::new(config);
        // Manually build a kinematics definition so we can test convert_to_snapshot
        // without needing a real SimConnect API.
        let def_id = mapping.next_definition_id;
        mapping.next_definition_id += 1;
        let request_id = mapping.next_request_id;
        mapping.next_request_id += 1;

        let fields = [
            "AIRSPEED INDICATED",
            "AIRSPEED TRUE",
            "GROUND VELOCITY",
            "INCIDENCE ALPHA",
            "INCIDENCE BETA",
            "PLANE BANK DEGREES",
            "PLANE PITCH DEGREES",
            "PLANE HEADING DEGREES MAGNETIC",
            "G FORCE",
            "ACCELERATION BODY X",
            "ACCELERATION BODY Z",
            "AIRSPEED MACH",
            "VERTICAL SPEED",
        ];
        let mut vars = Vec::new();
        let mut offset = 0;
        for name in &fields {
            vars.push(VariableDefinition {
                name: name.to_string(),
                units: "".to_string(),
                data_type: SIMCONNECT_DATATYPE::FLOAT64,
                offset,
                size: 8,
            });
            offset += 8;
        }
        mapping.data_definitions.insert(
            def_id,
            DataDefinition {
                id: def_id,
                variables: vars,
                data_size: offset,
            },
        );
        mapping.request_mappings.insert(
            request_id,
            RequestMapping {
                request_id,
                definition_id: def_id,
                category: DataCategory::Kinematics,
                period: SIMCONNECT_PERIOD::VISUAL_FRAME,
                engine_index: None,
            },
        );
        (mapping, request_id)
    }

    #[test]
    fn test_convert_kinematics_data_pipeline() {
        let (mapping, request_id) = build_kinematics_mapping();

        // Construct a raw data buffer matching the kinematics definition layout.
        let mut buf = Vec::new();
        write_f64(&mut buf, 120.0); // IAS (knots)
        write_f64(&mut buf, 130.0); // TAS
        write_f64(&mut buf, 115.0); // ground speed
        write_f64(&mut buf, 5.0); // AoA (degrees)
        write_f64(&mut buf, 1.0); // sideslip
        write_f64(&mut buf, -10.0); // bank
        write_f64(&mut buf, 3.0); // pitch
        write_f64(&mut buf, 90.0); // heading
        write_f64(&mut buf, 1.2); // g-force
        write_f64(&mut buf, 3.2174); // g-lateral (ft/s², ~0.1G)
        write_f64(&mut buf, 0.0); // g-longitudinal
        write_f64(&mut buf, 0.18); // mach
        write_f64(&mut buf, 500.0); // VS (fpm)

        let mut snapshot = BusSnapshot::default();
        mapping
            .convert_to_snapshot(request_id, &buf, &mut snapshot)
            .expect("conversion must succeed");

        assert!((snapshot.kinematics.ias.to_knots() - 120.0).abs() < 0.1);
        assert!((snapshot.kinematics.tas.to_knots() - 130.0).abs() < 0.1);
        assert!((snapshot.kinematics.pitch.to_degrees() - 3.0).abs() < 0.1);
        assert!((snapshot.kinematics.heading.to_degrees() - 90.0).abs() < 0.1);
        assert!((snapshot.kinematics.vertical_speed - 500.0).abs() < 0.1);
        assert!(snapshot.validity.attitude_valid);
        assert!(snapshot.validity.velocities_valid);
        assert!(snapshot.validity.kinematics_valid);
    }

    #[test]
    fn test_convert_kinematics_insufficient_data() {
        let (mapping, request_id) = build_kinematics_mapping();

        let buf = vec![0u8; 4]; // Way too small
        let mut snapshot = BusSnapshot::default();
        assert!(
            mapping
                .convert_to_snapshot(request_id, &buf, &mut snapshot)
                .is_err()
        );
    }

    #[test]
    fn test_convert_environment_data_pipeline() {
        let config = create_default_mapping();
        let mut mapping = VariableMapping::new(config);

        let def_id = mapping.next_definition_id;
        mapping.next_definition_id += 1;
        let request_id = mapping.next_request_id;
        mapping.next_request_id += 1;

        let fields = [
            "INDICATED ALTITUDE",
            "PRESSURE ALTITUDE",
            "AMBIENT TEMPERATURE",
            "AMBIENT WIND VELOCITY",
            "AMBIENT WIND DIRECTION",
            "AMBIENT VISIBILITY",
            "AMBIENT CLOUD COVERAGE",
        ];
        let mut vars = Vec::new();
        let mut offset = 0;
        for name in &fields {
            vars.push(VariableDefinition {
                name: name.to_string(),
                units: "".to_string(),
                data_type: SIMCONNECT_DATATYPE::FLOAT64,
                offset,
                size: 8,
            });
            offset += 8;
        }
        mapping.data_definitions.insert(
            def_id,
            DataDefinition {
                id: def_id,
                variables: vars,
                data_size: offset,
            },
        );
        mapping.request_mappings.insert(
            request_id,
            RequestMapping {
                request_id,
                definition_id: def_id,
                category: DataCategory::Environment,
                period: SIMCONNECT_PERIOD::SECOND,
                engine_index: None,
            },
        );

        let mut buf = Vec::new();
        write_f64(&mut buf, 5000.0); // altitude
        write_f64(&mut buf, 5200.0); // pressure alt
        write_f64(&mut buf, 15.0); // OAT
        write_f64(&mut buf, 10.0); // wind speed
        write_f64(&mut buf, 270.0); // wind dir
        write_f64(&mut buf, 10.0); // visibility
        write_f64(&mut buf, 25.0); // cloud coverage

        let mut snapshot = BusSnapshot::default();
        mapping
            .convert_to_snapshot(request_id, &buf, &mut snapshot)
            .expect("conversion must succeed");

        assert!((snapshot.environment.altitude - 5000.0).abs() < 0.1);
        assert!((snapshot.environment.oat - 15.0).abs() < 0.1);
    }

    #[test]
    fn test_convert_engine_data_pipeline() {
        let config = create_default_mapping();
        let mut mapping = VariableMapping::new(config);

        let def_id = mapping.next_definition_id;
        mapping.next_definition_id += 1;
        let request_id = mapping.next_request_id;
        mapping.next_request_id += 1;

        let mut vars = Vec::new();
        let mut offset = 0;
        // running (i32)
        vars.push(VariableDefinition {
            name: "GENERAL ENG COMBUSTION:1".to_string(),
            units: "bool".to_string(),
            data_type: SIMCONNECT_DATATYPE::INT32,
            offset,
            size: 4,
        });
        offset += 4;
        // rpm (f64)
        vars.push(VariableDefinition {
            name: "GENERAL ENG RPM:1".to_string(),
            units: "percent".to_string(),
            data_type: SIMCONNECT_DATATYPE::FLOAT64,
            offset,
            size: 8,
        });
        offset += 8;

        mapping.data_definitions.insert(
            def_id,
            DataDefinition {
                id: def_id,
                variables: vars,
                data_size: offset,
            },
        );
        mapping.request_mappings.insert(
            request_id,
            RequestMapping {
                request_id,
                definition_id: def_id,
                category: DataCategory::Engines,
                period: SIMCONNECT_PERIOD::SIM_FRAME,
                engine_index: Some(0),
            },
        );

        let mut buf = Vec::new();
        write_i32(&mut buf, 1); // running = true
        write_f64(&mut buf, 75.0); // RPM 75%

        let mut snapshot = BusSnapshot::default();
        mapping
            .convert_to_snapshot(request_id, &buf, &mut snapshot)
            .expect("conversion must succeed");

        assert_eq!(snapshot.engines.len(), 1);
        assert!(snapshot.engines[0].running);
        assert!((snapshot.engines[0].rpm.value() - 75.0).abs() < 0.1);
    }

    #[test]
    fn test_unknown_request_id_returns_error() {
        let config = create_default_mapping();
        let mapping = VariableMapping::new(config);

        let mut snapshot = BusSnapshot::default();
        let result = mapping.convert_to_snapshot(9999, &[0u8; 8], &mut snapshot);
        assert!(result.is_err());
    }
}
