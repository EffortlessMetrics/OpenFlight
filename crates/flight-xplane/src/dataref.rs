// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! DataRef management for X-Plane communication
//!
//! Provides structures and utilities for managing X-Plane DataRefs,
//! including value types, request management, and aircraft-specific DataRef sets.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;

/// DataRef value types supported by X-Plane
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DataRefValue {
    Float(f32),
    Double(f64),
    Int(i32),
    FloatArray(Vec<f32>),
    IntArray(Vec<i32>),
}

impl fmt::Display for DataRefValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataRefValue::Float(v) => write!(f, "{}", v),
            DataRefValue::Double(v) => write!(f, "{}", v),
            DataRefValue::Int(v) => write!(f, "{}", v),
            DataRefValue::FloatArray(v) => write!(f, "{:?}", v),
            DataRefValue::IntArray(v) => write!(f, "{:?}", v),
        }
    }
}

/// DataRef definition
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataRef {
    pub name: String,
    pub description: String,
    pub units: Option<String>,
    pub writable: bool,
    pub value_type: DataRefType,
}

/// DataRef type information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DataRefType {
    Float,
    Double,
    Int,
    FloatArray { size: Option<usize> },
    IntArray { size: Option<usize> },
}

impl DataRef {
    pub fn new(name: String) -> Self {
        Self {
            name,
            description: String::new(),
            units: None,
            writable: false,
            value_type: DataRefType::Float,
        }
    }

    pub fn with_description(mut self, description: String) -> Self {
        self.description = description;
        self
    }

    pub fn with_units(mut self, units: String) -> Self {
        self.units = Some(units);
        self
    }

    pub fn writable(mut self) -> Self {
        self.writable = true;
        self
    }

    pub fn with_type(mut self, value_type: DataRefType) -> Self {
        self.value_type = value_type;
        self
    }
}

/// DataRef request configuration
#[derive(Debug, Clone)]
pub struct DataRefRequest {
    pub dataref: DataRef,
    pub frequency: f32,
    pub priority: RequestPriority,
}

/// Request priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RequestPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

/// DataRef manager for organizing and requesting DataRefs
#[derive(Clone)]
pub struct DataRefManager {
    /// All known DataRefs indexed by name
    datarefs: HashMap<String, DataRef>,
    /// Aircraft-specific DataRef sets
    aircraft_datarefs: HashMap<String, HashSet<String>>,
    /// Default DataRefs for all aircraft
    default_datarefs: HashSet<String>,
}

impl DataRefManager {
    pub fn new() -> Self {
        let mut manager = Self {
            datarefs: HashMap::new(),
            aircraft_datarefs: HashMap::new(),
            default_datarefs: HashSet::new(),
        };

        manager.initialize_default_datarefs();
        manager.initialize_aircraft_specific_datarefs();

        manager
    }

    /// Initialize default DataRefs used for all aircraft
    fn initialize_default_datarefs(&mut self) {
        let default_datarefs = vec![
            // Basic flight data
            DataRef::new("sim/flightmodel/position/indicated_airspeed".to_string())
                .with_description("Indicated airspeed".to_string())
                .with_units("m/s".to_string()),
            DataRef::new("sim/flightmodel/position/true_airspeed".to_string())
                .with_description("True airspeed".to_string())
                .with_units("m/s".to_string()),
            DataRef::new("sim/flightmodel/position/groundspeed".to_string())
                .with_description("Ground speed".to_string())
                .with_units("m/s".to_string()),
            // Attitude
            DataRef::new("sim/flightmodel/position/theta".to_string())
                .with_description("Pitch angle".to_string())
                .with_units("degrees".to_string()),
            DataRef::new("sim/flightmodel/position/phi".to_string())
                .with_description("Roll angle".to_string())
                .with_units("degrees".to_string()),
            DataRef::new("sim/flightmodel/position/psi".to_string())
                .with_description("Heading angle".to_string())
                .with_units("degrees".to_string()),
            // Angle of attack and sideslip
            DataRef::new("sim/flightmodel/position/alpha".to_string())
                .with_description("Angle of attack".to_string())
                .with_units("degrees".to_string()),
            DataRef::new("sim/flightmodel/position/beta".to_string())
                .with_description("Sideslip angle".to_string())
                .with_units("degrees".to_string()),
            // G-forces
            DataRef::new("sim/flightmodel/forces/g_nrml".to_string())
                .with_description("Normal G-force".to_string())
                .with_units("G".to_string()),
            DataRef::new("sim/flightmodel/forces/g_side".to_string())
                .with_description("Side G-force".to_string())
                .with_units("G".to_string()),
            DataRef::new("sim/flightmodel/forces/g_axil".to_string())
                .with_description("Axial G-force".to_string())
                .with_units("G".to_string()),
            // Position
            DataRef::new("sim/flightmodel/position/latitude".to_string())
                .with_description("Latitude".to_string())
                .with_units("degrees".to_string())
                .with_type(DataRefType::Double),
            DataRef::new("sim/flightmodel/position/longitude".to_string())
                .with_description("Longitude".to_string())
                .with_units("degrees".to_string())
                .with_type(DataRefType::Double),
            DataRef::new("sim/flightmodel/position/elevation".to_string())
                .with_description("Elevation above sea level".to_string())
                .with_units("meters".to_string()),
            // Vertical speed
            DataRef::new("sim/flightmodel/position/vh_ind".to_string())
                .with_description("Vertical speed".to_string())
                .with_units("m/s".to_string()),
            // Ground track
            DataRef::new("sim/flightmodel/position/hpath".to_string())
                .with_description("Ground track".to_string())
                .with_units("degrees".to_string()),
            // Aircraft configuration
            DataRef::new("sim/aircraft/parts/acf_gear_deploy".to_string())
                .with_description("Gear deployment ratio".to_string())
                .with_units("ratio".to_string()),
            DataRef::new("sim/aircraft/parts/acf_flap_deploy".to_string())
                .with_description("Flap deployment ratio".to_string())
                .with_units("ratio".to_string()),
            DataRef::new("sim/aircraft/parts/acf_speedbrake_deploy".to_string())
                .with_description("Speedbrake deployment ratio".to_string())
                .with_units("ratio".to_string()),
            // Environment
            DataRef::new("sim/weather/temperature_ambient_c".to_string())
                .with_description("Ambient temperature".to_string())
                .with_units("Celsius".to_string()),
            DataRef::new("sim/weather/wind_speed_kt[0]".to_string())
                .with_description("Wind speed at surface".to_string())
                .with_units("knots".to_string()),
            DataRef::new("sim/weather/wind_direction_degt[0]".to_string())
                .with_description("Wind direction at surface".to_string())
                .with_units("degrees".to_string()),
            // Aircraft identification
            DataRef::new("sim/aircraft/view/acf_ICAO".to_string())
                .with_description("Aircraft ICAO code".to_string()),
            DataRef::new("sim/aircraft/view/acf_descrip".to_string())
                .with_description("Aircraft description".to_string()),
            DataRef::new("sim/aircraft/view/acf_author".to_string())
                .with_description("Aircraft author".to_string()),
            // Version info
            DataRef::new("sim/version/xplane_internal_version".to_string())
                .with_description("X-Plane version".to_string())
                .with_type(DataRefType::Int),
        ];

        for dataref in default_datarefs {
            let name = dataref.name.clone();
            self.datarefs.insert(name.clone(), dataref);
            self.default_datarefs.insert(name);
        }
    }

    /// Initialize aircraft-specific DataRef sets
    fn initialize_aircraft_specific_datarefs(&mut self) {
        // General aviation aircraft
        let ga_datarefs = vec![
            // Engine data for piston engines
            DataRef::new("sim/flightmodel/engine/ENGN_running[0]".to_string())
                .with_description("Engine 1 running".to_string())
                .with_type(DataRefType::Int),
            DataRef::new("sim/flightmodel/engine/ENGN_N1_[0]".to_string())
                .with_description("Engine 1 N1".to_string())
                .with_units("percent".to_string()),
            DataRef::new("sim/flightmodel/engine/ENGN_MPR[0]".to_string())
                .with_description("Engine 1 manifold pressure".to_string())
                .with_units("inHg".to_string()),
            DataRef::new("sim/flightmodel/engine/ENGN_EGT[0]".to_string())
                .with_description("Engine 1 EGT".to_string())
                .with_units("degrees C".to_string()),
            DataRef::new("sim/flightmodel/engine/ENGN_CHT[0]".to_string())
                .with_description("Engine 1 CHT".to_string())
                .with_units("degrees C".to_string()),
        ];

        for dataref in ga_datarefs {
            let name = dataref.name.clone();
            self.datarefs.insert(name.clone(), dataref);
        }

        // Add GA aircraft types
        let ga_aircraft = vec!["C172", "C182", "C208", "PA28", "SR22"];
        for aircraft in ga_aircraft {
            let mut datarefs = self.default_datarefs.clone();
            datarefs.insert("sim/flightmodel/engine/ENGN_running[0]".to_string());
            datarefs.insert("sim/flightmodel/engine/ENGN_N1_[0]".to_string());
            datarefs.insert("sim/flightmodel/engine/ENGN_MPR[0]".to_string());
            datarefs.insert("sim/flightmodel/engine/ENGN_EGT[0]".to_string());
            datarefs.insert("sim/flightmodel/engine/ENGN_CHT[0]".to_string());
            self.aircraft_datarefs
                .insert(aircraft.to_string(), datarefs);
        }

        // Commercial aircraft
        let airliner_datarefs = vec![
            // Multiple engines
            DataRef::new("sim/flightmodel/engine/ENGN_running[1]".to_string())
                .with_description("Engine 2 running".to_string())
                .with_type(DataRefType::Int),
            DataRef::new("sim/flightmodel/engine/ENGN_N1_[1]".to_string())
                .with_description("Engine 2 N1".to_string())
                .with_units("percent".to_string()),
            // Autopilot
            DataRef::new("sim/cockpit/autopilot/autopilot_mode".to_string())
                .with_description("Autopilot mode".to_string())
                .with_type(DataRefType::Int),
            DataRef::new("sim/cockpit/autopilot/altitude".to_string())
                .with_description("Autopilot altitude target".to_string())
                .with_units("feet".to_string()),
            DataRef::new("sim/cockpit/autopilot/heading".to_string())
                .with_description("Autopilot heading target".to_string())
                .with_units("degrees".to_string()),
        ];

        for dataref in airliner_datarefs {
            let name = dataref.name.clone();
            self.datarefs.insert(name.clone(), dataref);
        }

        // Add airliner aircraft types
        let airliner_aircraft = vec!["A320", "A321", "B737", "B738", "B777", "B787"];
        for aircraft in airliner_aircraft {
            let mut datarefs = self.default_datarefs.clone();
            datarefs.insert("sim/flightmodel/engine/ENGN_running[0]".to_string());
            datarefs.insert("sim/flightmodel/engine/ENGN_N1_[0]".to_string());
            datarefs.insert("sim/flightmodel/engine/ENGN_running[1]".to_string());
            datarefs.insert("sim/flightmodel/engine/ENGN_N1_[1]".to_string());
            datarefs.insert("sim/cockpit/autopilot/autopilot_mode".to_string());
            datarefs.insert("sim/cockpit/autopilot/altitude".to_string());
            datarefs.insert("sim/cockpit/autopilot/heading".to_string());
            self.aircraft_datarefs
                .insert(aircraft.to_string(), datarefs);
        }

        // Helicopter-specific DataRefs
        let helo_datarefs = vec![
            DataRef::new("sim/flightmodel/engine/ENGN_Nrotor".to_string())
                .with_description("Main rotor RPM".to_string())
                .with_units("percent".to_string()),
            DataRef::new("sim/flightmodel/engine/ENGN_Nturb".to_string())
                .with_description("Turbine RPM".to_string())
                .with_units("percent".to_string()),
            DataRef::new("sim/flightmodel/engine/ENGN_torq".to_string())
                .with_description("Engine torque".to_string())
                .with_units("percent".to_string()),
            DataRef::new("sim/joystick/yoke_pitch_ratio".to_string())
                .with_description("Collective position".to_string())
                .with_units("ratio".to_string()),
            DataRef::new("sim/joystick/yoke_heading_ratio".to_string())
                .with_description("Anti-torque pedal position".to_string())
                .with_units("ratio".to_string()),
        ];

        for dataref in helo_datarefs {
            let name = dataref.name.clone();
            self.datarefs.insert(name.clone(), dataref);
        }

        // Add helicopter aircraft types
        let helo_aircraft = vec!["UH1H", "AH64", "R22", "R44", "EC35"];
        for aircraft in helo_aircraft {
            let mut datarefs = self.default_datarefs.clone();
            datarefs.insert("sim/flightmodel/engine/ENGN_Nrotor".to_string());
            datarefs.insert("sim/flightmodel/engine/ENGN_Nturb".to_string());
            datarefs.insert("sim/flightmodel/engine/ENGN_torq".to_string());
            datarefs.insert("sim/joystick/yoke_pitch_ratio".to_string());
            datarefs.insert("sim/joystick/yoke_heading_ratio".to_string());
            self.aircraft_datarefs
                .insert(aircraft.to_string(), datarefs);
        }
    }

    /// Get required DataRefs for a specific aircraft
    pub fn get_required_datarefs(&self, aircraft_icao: &str) -> Vec<DataRef> {
        let dataref_names = self
            .aircraft_datarefs
            .get(aircraft_icao)
            .unwrap_or(&self.default_datarefs);

        dataref_names
            .iter()
            .filter_map(|name| self.datarefs.get(name).cloned())
            .collect()
    }

    /// Get a specific DataRef by name
    pub fn get_dataref(&self, name: &str) -> Option<&DataRef> {
        self.datarefs.get(name)
    }

    /// Add a custom DataRef
    pub fn add_dataref(&mut self, dataref: DataRef) {
        let name = dataref.name.clone();
        self.datarefs.insert(name, dataref);
    }

    /// Add DataRef to aircraft-specific set
    pub fn add_aircraft_dataref(&mut self, aircraft_icao: &str, dataref_name: &str) {
        self.aircraft_datarefs
            .entry(aircraft_icao.to_string())
            .or_insert_with(|| self.default_datarefs.clone())
            .insert(dataref_name.to_string());
    }

    /// Get all known DataRefs
    pub fn get_all_datarefs(&self) -> &HashMap<String, DataRef> {
        &self.datarefs
    }

    /// Get aircraft-specific DataRef names
    pub fn get_aircraft_dataref_names(&self, aircraft_icao: &str) -> Vec<String> {
        self.aircraft_datarefs
            .get(aircraft_icao)
            .unwrap_or(&self.default_datarefs)
            .iter()
            .cloned()
            .collect()
    }

    /// Create a prioritized request list
    pub fn create_request_list(&self, aircraft_icao: &str) -> Vec<DataRefRequest> {
        let datarefs = self.get_required_datarefs(aircraft_icao);
        let mut requests = Vec::new();

        for dataref in datarefs {
            let priority = self.determine_priority(&dataref);
            let frequency = self.determine_frequency(&dataref, priority);

            requests.push(DataRefRequest {
                dataref,
                frequency,
                priority,
            });
        }

        // Sort by priority (highest first)
        requests.sort_by(|a, b| b.priority.cmp(&a.priority));
        requests
    }

    /// Determine request priority for a DataRef
    fn determine_priority(&self, dataref: &DataRef) -> RequestPriority {
        match dataref.name.as_str() {
            // Critical flight data
            name if name.contains("indicated_airspeed") => RequestPriority::Critical,
            name if name.contains("position/latitude") => RequestPriority::Critical,
            name if name.contains("position/longitude") => RequestPriority::Critical,
            name if name.contains("position/elevation") => RequestPriority::Critical,

            // Important attitude data
            name if name.contains("position/theta") => RequestPriority::High,
            name if name.contains("position/phi") => RequestPriority::High,
            name if name.contains("position/psi") => RequestPriority::High,
            name if name.contains("position/alpha") => RequestPriority::High,

            // Engine data
            name if name.contains("ENGN_running") => RequestPriority::High,
            name if name.contains("ENGN_N1") => RequestPriority::Normal,

            // Configuration data
            name if name.contains("acf_gear_deploy") => RequestPriority::Normal,
            name if name.contains("acf_flap_deploy") => RequestPriority::Normal,

            // Environmental data
            name if name.contains("weather/") => RequestPriority::Low,

            // Default priority
            _ => RequestPriority::Normal,
        }
    }

    /// Determine request frequency based on priority
    fn determine_frequency(&self, _dataref: &DataRef, priority: RequestPriority) -> f32 {
        match priority {
            RequestPriority::Critical => 30.0, // 30 Hz
            RequestPriority::High => 20.0,     // 20 Hz
            RequestPriority::Normal => 10.0,   // 10 Hz
            RequestPriority::Low => 5.0,       // 5 Hz
        }
    }
}

impl Default for DataRefManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dataref_creation() {
        let dataref = DataRef::new("test/dataref".to_string())
            .with_description("Test DataRef".to_string())
            .with_units("units".to_string())
            .writable()
            .with_type(DataRefType::Float);

        assert_eq!(dataref.name, "test/dataref");
        assert_eq!(dataref.description, "Test DataRef");
        assert_eq!(dataref.units, Some("units".to_string()));
        assert!(dataref.writable);
        assert_eq!(dataref.value_type, DataRefType::Float);
    }

    #[test]
    fn test_dataref_value_display() {
        assert_eq!(DataRefValue::Float(42.0).to_string(), "42");
        assert_eq!(DataRefValue::Int(123).to_string(), "123");
        assert_eq!(DataRefValue::Double(3.14159).to_string(), "3.14159");
    }

    #[test]
    fn test_dataref_manager_initialization() {
        let manager = DataRefManager::new();

        // Should have default DataRefs
        assert!(!manager.default_datarefs.is_empty());
        assert!(
            manager
                .default_datarefs
                .contains("sim/flightmodel/position/indicated_airspeed")
        );

        // Should have aircraft-specific sets
        assert!(manager.aircraft_datarefs.contains_key("C172"));
        assert!(manager.aircraft_datarefs.contains_key("A320"));
        assert!(manager.aircraft_datarefs.contains_key("UH1H"));
    }

    #[test]
    fn test_aircraft_specific_datarefs() {
        let manager = DataRefManager::new();

        // GA aircraft should have engine DataRefs
        let c172_datarefs = manager.get_required_datarefs("C172");
        let has_engine_dataref = c172_datarefs
            .iter()
            .any(|dr| dr.name.contains("ENGN_running"));
        assert!(has_engine_dataref);

        // Helicopters should have rotor DataRefs
        let uh1h_datarefs = manager.get_required_datarefs("UH1H");
        let has_rotor_dataref = uh1h_datarefs
            .iter()
            .any(|dr| dr.name.contains("ENGN_Nrotor"));
        assert!(has_rotor_dataref);

        // Unknown aircraft should get defaults
        let unknown_datarefs = manager.get_required_datarefs("UNKNOWN");
        assert!(!unknown_datarefs.is_empty());
    }

    #[test]
    fn test_request_list_creation() {
        let manager = DataRefManager::new();
        let requests = manager.create_request_list("C172");

        assert!(!requests.is_empty());

        // Should be sorted by priority
        for i in 1..requests.len() {
            assert!(requests[i - 1].priority >= requests[i].priority);
        }

        // Critical DataRefs should have higher frequency
        let critical_request = requests
            .iter()
            .find(|r| r.priority == RequestPriority::Critical);
        assert!(critical_request.is_some());
        assert!(critical_request.unwrap().frequency >= 30.0);
    }

    #[test]
    fn test_priority_determination() {
        let manager = DataRefManager::new();

        let ias_dataref = DataRef::new("sim/flightmodel/position/indicated_airspeed".to_string());
        let priority = manager.determine_priority(&ias_dataref);
        assert_eq!(priority, RequestPriority::Critical);

        let weather_dataref = DataRef::new("sim/weather/temperature_ambient_c".to_string());
        let priority = manager.determine_priority(&weather_dataref);
        assert_eq!(priority, RequestPriority::Low);
    }

    #[test]
    fn test_custom_dataref_addition() {
        let mut manager = DataRefManager::new();

        let custom_dataref = DataRef::new("custom/test/dataref".to_string())
            .with_description("Custom test DataRef".to_string());

        manager.add_dataref(custom_dataref.clone());

        let retrieved = manager.get_dataref("custom/test/dataref");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().description, "Custom test DataRef");
    }

    #[test]
    fn test_aircraft_dataref_addition() {
        let mut manager = DataRefManager::new();

        manager.add_aircraft_dataref("TEST_AIRCRAFT", "custom/dataref");

        let datarefs = manager.get_aircraft_dataref_names("TEST_AIRCRAFT");
        assert!(datarefs.contains(&"custom/dataref".to_string()));
    }
}
