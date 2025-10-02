//! Sample StreamDeck profiles for different aircraft types
//!
//! Provides pre-built profiles for GA, Airbus, and Helicopter aircraft
//! with common actions and layouts for flight simulation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use anyhow::Result;
use tracing::info;

/// Aircraft types for sample profiles
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AircraftType {
    GA,      // General Aviation
    Airbus,  // Airbus commercial aircraft
    Helo,    // Helicopter
}

/// StreamDeck action configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamDeckAction {
    pub uuid: String,
    pub name: String,
    pub icon: Option<String>,
    pub tooltip: Option<String>,
    pub states: Vec<ActionState>,
    pub settings: serde_json::Value,
}

/// Action state configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionState {
    pub image: Option<String>,
    pub title: Option<String>,
    pub title_color: Option<String>,
    pub title_alignment: Option<String>,
    pub font_family: Option<String>,
    pub font_size: Option<u32>,
    pub show_title: Option<bool>,
}

/// StreamDeck profile configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamDeckProfile {
    pub name: String,
    pub device_type: String,
    pub version: String,
    pub actions: HashMap<String, StreamDeckAction>,
    pub application_version: String,
    pub plugin_version: String,
}

/// Profile manager for StreamDeck profiles
pub struct ProfileManager {
    profiles: HashMap<AircraftType, serde_json::Value>,
}

impl ProfileManager {
    pub fn new() -> Self {
        Self {
            profiles: HashMap::new(),
        }
    }

    /// Load sample profiles for all aircraft types
    pub fn load_sample_profiles(&mut self) -> Result<()> {
        info!("Loading sample StreamDeck profiles");

        // Load GA profile
        let ga_profile = self.create_ga_profile()?;
        self.profiles.insert(AircraftType::GA, serde_json::to_value(ga_profile)?);

        // Load Airbus profile
        let airbus_profile = self.create_airbus_profile()?;
        self.profiles.insert(AircraftType::Airbus, serde_json::to_value(airbus_profile)?);

        // Load Helicopter profile
        let helo_profile = self.create_helo_profile()?;
        self.profiles.insert(AircraftType::Helo, serde_json::to_value(helo_profile)?);

        info!("Loaded {} sample profiles", self.profiles.len());
        Ok(())
    }

    /// Get all loaded profiles
    pub fn get_profiles(&self) -> &HashMap<AircraftType, serde_json::Value> {
        &self.profiles
    }

    /// Get profile for specific aircraft type
    pub fn get_profile(&self, aircraft_type: AircraftType) -> Option<&serde_json::Value> {
        self.profiles.get(&aircraft_type)
    }

    /// Create GA (General Aviation) sample profile
    fn create_ga_profile(&self) -> Result<StreamDeckProfile> {
        let mut actions = HashMap::new();

        // Landing lights action
        actions.insert("0,0".to_string(), StreamDeckAction {
            uuid: "com.flighthub.landing-lights".to_string(),
            name: "Landing Lights".to_string(),
            icon: Some("landing_lights.png".to_string()),
            tooltip: Some("Toggle landing lights".to_string()),
            states: vec![
                ActionState {
                    image: Some("landing_lights_off.png".to_string()),
                    title: Some("LAND\nOFF".to_string()),
                    title_color: Some("#FFFFFF".to_string()),
                    title_alignment: Some("bottom".to_string()),
                    font_family: Some("Arial".to_string()),
                    font_size: Some(12),
                    show_title: Some(true),
                },
                ActionState {
                    image: Some("landing_lights_on.png".to_string()),
                    title: Some("LAND\nON".to_string()),
                    title_color: Some("#00FF00".to_string()),
                    title_alignment: Some("bottom".to_string()),
                    font_family: Some("Arial".to_string()),
                    font_size: Some(12),
                    show_title: Some(true),
                },
            ],
            settings: serde_json::json!({
                "telemetry_variable": "LIGHT_LANDING",
                "command_on": "LANDING_LIGHTS_ON",
                "command_off": "LANDING_LIGHTS_OFF"
            }),
        });

        // Navigation lights action
        actions.insert("0,1".to_string(), StreamDeckAction {
            uuid: "com.flighthub.nav-lights".to_string(),
            name: "Navigation Lights".to_string(),
            icon: Some("nav_lights.png".to_string()),
            tooltip: Some("Toggle navigation lights".to_string()),
            states: vec![
                ActionState {
                    image: Some("nav_lights_off.png".to_string()),
                    title: Some("NAV\nOFF".to_string()),
                    title_color: Some("#FFFFFF".to_string()),
                    title_alignment: Some("bottom".to_string()),
                    font_family: Some("Arial".to_string()),
                    font_size: Some(12),
                    show_title: Some(true),
                },
                ActionState {
                    image: Some("nav_lights_on.png".to_string()),
                    title: Some("NAV\nON".to_string()),
                    title_color: Some("#00FF00".to_string()),
                    title_alignment: Some("bottom".to_string()),
                    font_family: Some("Arial".to_string()),
                    font_size: Some(12),
                    show_title: Some(true),
                },
            ],
            settings: serde_json::json!({
                "telemetry_variable": "LIGHT_NAV",
                "command_on": "TOGGLE_NAV_LIGHTS",
                "command_off": "TOGGLE_NAV_LIGHTS"
            }),
        });

        // Gear action
        actions.insert("1,0".to_string(), StreamDeckAction {
            uuid: "com.flighthub.gear".to_string(),
            name: "Landing Gear".to_string(),
            icon: Some("gear.png".to_string()),
            tooltip: Some("Toggle landing gear".to_string()),
            states: vec![
                ActionState {
                    image: Some("gear_up.png".to_string()),
                    title: Some("GEAR\nUP".to_string()),
                    title_color: Some("#FFFFFF".to_string()),
                    title_alignment: Some("bottom".to_string()),
                    font_family: Some("Arial".to_string()),
                    font_size: Some(12),
                    show_title: Some(true),
                },
                ActionState {
                    image: Some("gear_down.png".to_string()),
                    title: Some("GEAR\nDOWN".to_string()),
                    title_color: Some("#00FF00".to_string()),
                    title_alignment: Some("bottom".to_string()),
                    font_family: Some("Arial".to_string()),
                    font_size: Some(12),
                    show_title: Some(true),
                },
            ],
            settings: serde_json::json!({
                "telemetry_variable": "GEAR_POSITION",
                "command": "GEAR_TOGGLE"
            }),
        });

        // Flaps action
        actions.insert("1,1".to_string(), StreamDeckAction {
            uuid: "com.flighthub.flaps".to_string(),
            name: "Flaps".to_string(),
            icon: Some("flaps.png".to_string()),
            tooltip: Some("Cycle flaps position".to_string()),
            states: vec![
                ActionState {
                    image: Some("flaps_0.png".to_string()),
                    title: Some("FLAPS\n0°".to_string()),
                    title_color: Some("#FFFFFF".to_string()),
                    title_alignment: Some("bottom".to_string()),
                    font_family: Some("Arial".to_string()),
                    font_size: Some(12),
                    show_title: Some(true),
                },
            ],
            settings: serde_json::json!({
                "telemetry_variable": "FLAPS_HANDLE_PERCENT",
                "command": "FLAPS_INCR"
            }),
        });

        Ok(StreamDeckProfile {
            name: "Flight Hub - General Aviation".to_string(),
            device_type: "StreamDeck".to_string(),
            version: "1.0.0".to_string(),
            actions,
            application_version: "6.2.0".to_string(),
            plugin_version: "1.0.0".to_string(),
        })
    }

    /// Create Airbus sample profile
    fn create_airbus_profile(&self) -> Result<StreamDeckProfile> {
        let mut actions = HashMap::new();

        // Autopilot master action
        actions.insert("0,0".to_string(), StreamDeckAction {
            uuid: "com.flighthub.ap-master".to_string(),
            name: "Autopilot Master".to_string(),
            icon: Some("ap_master.png".to_string()),
            tooltip: Some("Toggle autopilot master".to_string()),
            states: vec![
                ActionState {
                    image: Some("ap_off.png".to_string()),
                    title: Some("AP\nOFF".to_string()),
                    title_color: Some("#FFFFFF".to_string()),
                    title_alignment: Some("bottom".to_string()),
                    font_family: Some("Arial".to_string()),
                    font_size: Some(12),
                    show_title: Some(true),
                },
                ActionState {
                    image: Some("ap_on.png".to_string()),
                    title: Some("AP\nON".to_string()),
                    title_color: Some("#00FF00".to_string()),
                    title_alignment: Some("bottom".to_string()),
                    font_family: Some("Arial".to_string()),
                    font_size: Some(12),
                    show_title: Some(true),
                },
            ],
            settings: serde_json::json!({
                "telemetry_variable": "AUTOPILOT_MASTER",
                "command": "AP_MASTER"
            }),
        });

        // Altitude hold action
        actions.insert("0,1".to_string(), StreamDeckAction {
            uuid: "com.flighthub.alt-hold".to_string(),
            name: "Altitude Hold".to_string(),
            icon: Some("alt_hold.png".to_string()),
            tooltip: Some("Toggle altitude hold".to_string()),
            states: vec![
                ActionState {
                    image: Some("alt_hold_off.png".to_string()),
                    title: Some("ALT\nOFF".to_string()),
                    title_color: Some("#FFFFFF".to_string()),
                    title_alignment: Some("bottom".to_string()),
                    font_family: Some("Arial".to_string()),
                    font_size: Some(12),
                    show_title: Some(true),
                },
                ActionState {
                    image: Some("alt_hold_on.png".to_string()),
                    title: Some("ALT\nHOLD".to_string()),
                    title_color: Some("#00FF00".to_string()),
                    title_alignment: Some("bottom".to_string()),
                    font_family: Some("Arial".to_string()),
                    font_size: Some(12),
                    show_title: Some(true),
                },
            ],
            settings: serde_json::json!({
                "telemetry_variable": "AUTOPILOT_ALTITUDE_LOCK",
                "command": "AP_ALT_HOLD"
            }),
        });

        // Heading hold action
        actions.insert("1,0".to_string(), StreamDeckAction {
            uuid: "com.flighthub.hdg-hold".to_string(),
            name: "Heading Hold".to_string(),
            icon: Some("hdg_hold.png".to_string()),
            tooltip: Some("Toggle heading hold".to_string()),
            states: vec![
                ActionState {
                    image: Some("hdg_hold_off.png".to_string()),
                    title: Some("HDG\nOFF".to_string()),
                    title_color: Some("#FFFFFF".to_string()),
                    title_alignment: Some("bottom".to_string()),
                    font_family: Some("Arial".to_string()),
                    font_size: Some(12),
                    show_title: Some(true),
                },
                ActionState {
                    image: Some("hdg_hold_on.png".to_string()),
                    title: Some("HDG\nHOLD".to_string()),
                    title_color: Some("#00FF00".to_string()),
                    title_alignment: Some("bottom".to_string()),
                    font_family: Some("Arial".to_string()),
                    font_size: Some(12),
                    show_title: Some(true),
                },
            ],
            settings: serde_json::json!({
                "telemetry_variable": "AUTOPILOT_HEADING_LOCK",
                "command": "AP_HDG_HOLD"
            }),
        });

        // Approach mode action
        actions.insert("1,1".to_string(), StreamDeckAction {
            uuid: "com.flighthub.approach".to_string(),
            name: "Approach Mode".to_string(),
            icon: Some("approach.png".to_string()),
            tooltip: Some("Toggle approach mode".to_string()),
            states: vec![
                ActionState {
                    image: Some("approach_off.png".to_string()),
                    title: Some("APPR\nOFF".to_string()),
                    title_color: Some("#FFFFFF".to_string()),
                    title_alignment: Some("bottom".to_string()),
                    font_family: Some("Arial".to_string()),
                    font_size: Some(12),
                    show_title: Some(true),
                },
                ActionState {
                    image: Some("approach_on.png".to_string()),
                    title: Some("APPR\nON".to_string()),
                    title_color: Some("#00FF00".to_string()),
                    title_alignment: Some("bottom".to_string()),
                    font_family: Some("Arial".to_string()),
                    font_size: Some(12),
                    show_title: Some(true),
                },
            ],
            settings: serde_json::json!({
                "telemetry_variable": "AUTOPILOT_APPROACH_HOLD",
                "command": "AP_APR_HOLD"
            }),
        });

        Ok(StreamDeckProfile {
            name: "Flight Hub - Airbus".to_string(),
            device_type: "StreamDeck".to_string(),
            version: "1.0.0".to_string(),
            actions,
            application_version: "6.2.0".to_string(),
            plugin_version: "1.0.0".to_string(),
        })
    }

    /// Create Helicopter sample profile
    fn create_helo_profile(&self) -> Result<StreamDeckProfile> {
        let mut actions = HashMap::new();

        // Engine start action
        actions.insert("0,0".to_string(), StreamDeckAction {
            uuid: "com.flighthub.engine-start".to_string(),
            name: "Engine Start".to_string(),
            icon: Some("engine_start.png".to_string()),
            tooltip: Some("Toggle engine".to_string()),
            states: vec![
                ActionState {
                    image: Some("engine_off.png".to_string()),
                    title: Some("ENG\nOFF".to_string()),
                    title_color: Some("#FFFFFF".to_string()),
                    title_alignment: Some("bottom".to_string()),
                    font_family: Some("Arial".to_string()),
                    font_size: Some(12),
                    show_title: Some(true),
                },
                ActionState {
                    image: Some("engine_on.png".to_string()),
                    title: Some("ENG\nON".to_string()),
                    title_color: Some("#00FF00".to_string()),
                    title_alignment: Some("bottom".to_string()),
                    font_family: Some("Arial".to_string()),
                    font_size: Some(12),
                    show_title: Some(true),
                },
            ],
            settings: serde_json::json!({
                "telemetry_variable": "GENERAL_ENG_COMBUSTION:1",
                "command": "TOGGLE_ENGINE1_FAILURE"
            }),
        });

        // Rotor brake action
        actions.insert("0,1".to_string(), StreamDeckAction {
            uuid: "com.flighthub.rotor-brake".to_string(),
            name: "Rotor Brake".to_string(),
            icon: Some("rotor_brake.png".to_string()),
            tooltip: Some("Toggle rotor brake".to_string()),
            states: vec![
                ActionState {
                    image: Some("rotor_brake_off.png".to_string()),
                    title: Some("BRAKE\nOFF".to_string()),
                    title_color: Some("#FFFFFF".to_string()),
                    title_alignment: Some("bottom".to_string()),
                    font_family: Some("Arial".to_string()),
                    font_size: Some(12),
                    show_title: Some(true),
                },
                ActionState {
                    image: Some("rotor_brake_on.png".to_string()),
                    title: Some("BRAKE\nON".to_string()),
                    title_color: Some("#FF0000".to_string()),
                    title_alignment: Some("bottom".to_string()),
                    font_family: Some("Arial".to_string()),
                    font_size: Some(12),
                    show_title: Some(true),
                },
            ],
            settings: serde_json::json!({
                "telemetry_variable": "ROTOR_BRAKE",
                "command": "ROTOR_BRAKE"
            }),
        });

        // Collective friction action
        actions.insert("1,0".to_string(), StreamDeckAction {
            uuid: "com.flighthub.collective-friction".to_string(),
            name: "Collective Friction".to_string(),
            icon: Some("collective_friction.png".to_string()),
            tooltip: Some("Toggle collective friction".to_string()),
            states: vec![
                ActionState {
                    image: Some("friction_off.png".to_string()),
                    title: Some("FRIC\nOFF".to_string()),
                    title_color: Some("#FFFFFF".to_string()),
                    title_alignment: Some("bottom".to_string()),
                    font_family: Some("Arial".to_string()),
                    font_size: Some(12),
                    show_title: Some(true),
                },
                ActionState {
                    image: Some("friction_on.png".to_string()),
                    title: Some("FRIC\nON".to_string()),
                    title_color: Some("#FFFF00".to_string()),
                    title_alignment: Some("bottom".to_string()),
                    font_family: Some("Arial".to_string()),
                    font_size: Some(12),
                    show_title: Some(true),
                },
            ],
            settings: serde_json::json!({
                "telemetry_variable": "COLLECTIVE_FRICTION",
                "command": "COLLECTIVE_FRICTION_TOGGLE"
            }),
        });

        // Anti-torque trim action
        actions.insert("1,1".to_string(), StreamDeckAction {
            uuid: "com.flighthub.anti-torque-trim".to_string(),
            name: "Anti-Torque Trim".to_string(),
            icon: Some("anti_torque_trim.png".to_string()),
            tooltip: Some("Reset anti-torque trim".to_string()),
            states: vec![
                ActionState {
                    image: Some("trim_reset.png".to_string()),
                    title: Some("TRIM\nRESET".to_string()),
                    title_color: Some("#FFFFFF".to_string()),
                    title_alignment: Some("bottom".to_string()),
                    font_family: Some("Arial".to_string()),
                    font_size: Some(12),
                    show_title: Some(true),
                },
            ],
            settings: serde_json::json!({
                "command": "RUDDER_TRIM_RESET"
            }),
        });

        Ok(StreamDeckProfile {
            name: "Flight Hub - Helicopter".to_string(),
            device_type: "StreamDeck".to_string(),
            version: "1.0.0".to_string(),
            actions,
            application_version: "6.2.0".to_string(),
            plugin_version: "1.0.0".to_string(),
        })
    }
}

impl Default for ProfileManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Sample profiles utility struct
pub struct SampleProfiles;

impl SampleProfiles {
    /// Get all available aircraft types
    pub fn get_aircraft_types() -> Vec<AircraftType> {
        vec![AircraftType::GA, AircraftType::Airbus, AircraftType::Helo]
    }

    /// Get description for aircraft type
    pub fn get_aircraft_description(aircraft_type: AircraftType) -> &'static str {
        match aircraft_type {
            AircraftType::GA => "General Aviation aircraft (Cessna, Piper, etc.)",
            AircraftType::Airbus => "Airbus commercial aircraft (A320, A330, etc.)",
            AircraftType::Helo => "Helicopter aircraft (Bell, Robinson, etc.)",
        }
    }

    /// Get recommended StreamDeck layout for aircraft type
    pub fn get_recommended_layout(aircraft_type: AircraftType) -> &'static str {
        match aircraft_type {
            AircraftType::GA => "2x2 grid with essential controls",
            AircraftType::Airbus => "3x3 grid with autopilot functions",
            AircraftType::Helo => "2x3 grid with rotor and engine controls",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_manager_creation() {
        let manager = ProfileManager::new();
        assert_eq!(manager.profiles.len(), 0);
    }

    #[test]
    fn test_load_sample_profiles() {
        let mut manager = ProfileManager::new();
        manager.load_sample_profiles().unwrap();
        
        assert_eq!(manager.profiles.len(), 3);
        assert!(manager.get_profile(AircraftType::GA).is_some());
        assert!(manager.get_profile(AircraftType::Airbus).is_some());
        assert!(manager.get_profile(AircraftType::Helo).is_some());
    }

    #[test]
    fn test_ga_profile_creation() {
        let manager = ProfileManager::new();
        let profile = manager.create_ga_profile().unwrap();
        
        assert_eq!(profile.name, "Flight Hub - General Aviation");
        assert!(!profile.actions.is_empty());
        assert!(profile.actions.contains_key("0,0")); // Landing lights
        assert!(profile.actions.contains_key("1,0")); // Gear
    }

    #[test]
    fn test_airbus_profile_creation() {
        let manager = ProfileManager::new();
        let profile = manager.create_airbus_profile().unwrap();
        
        assert_eq!(profile.name, "Flight Hub - Airbus");
        assert!(!profile.actions.is_empty());
        assert!(profile.actions.contains_key("0,0")); // AP Master
        assert!(profile.actions.contains_key("1,1")); // Approach
    }

    #[test]
    fn test_helo_profile_creation() {
        let manager = ProfileManager::new();
        let profile = manager.create_helo_profile().unwrap();
        
        assert_eq!(profile.name, "Flight Hub - Helicopter");
        assert!(!profile.actions.is_empty());
        assert!(profile.actions.contains_key("0,0")); // Engine start
        assert!(profile.actions.contains_key("1,0")); // Collective friction
    }

    #[test]
    fn test_sample_profiles_utility() {
        let aircraft_types = SampleProfiles::get_aircraft_types();
        assert_eq!(aircraft_types.len(), 3);
        
        let description = SampleProfiles::get_aircraft_description(AircraftType::GA);
        assert!(description.contains("General Aviation"));
        
        let layout = SampleProfiles::get_recommended_layout(AircraftType::Airbus);
        assert!(layout.contains("3x3"));
    }
}