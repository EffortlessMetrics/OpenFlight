//! Event management for SimConnect
//!
//! Provides event mapping, Input Events for modern aircraft compatibility,
//! and system event subscription for MSFS integration.

use flight_simconnect_sys::{
    constants::*, SimConnectApi, HSIMCONNECT, SIMCONNECT_EVENTID,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use tracing::{debug, warn};

/// SimConnect event types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SimEvent {
    /// Standard SimConnect event
    Standard {
        name: String,
        data: Option<u32>,
    },
    /// Input Event (modern aircraft)
    Input {
        hash: u64,
        value: f64,
    },
    /// System event
    System {
        name: String,
    },
}

/// Input Event for modern aircraft compatibility
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputEvent {
    /// Event name/hash
    pub name: String,
    /// Event hash (calculated from name)
    pub hash: u64,
    /// Event value
    pub value: f64,
    /// Event description
    pub description: Option<String>,
}

/// Event management error types
#[derive(Debug, Error)]
pub enum EventError {
    #[error("SimConnect API error: {0}")]
    SimConnect(#[from] flight_simconnect_sys::SimConnectError),
    #[error("Event not found: {0}")]
    EventNotFound(String),
    #[error("Invalid event data")]
    InvalidData,
    #[error("Event mapping error: {0}")]
    MappingError(String),
}

/// Event manager for SimConnect
pub struct EventManager {
    /// Standard event mappings (name -> event ID)
    standard_events: HashMap<String, SIMCONNECT_EVENTID>,
    /// Input event mappings (name -> hash)
    input_events: HashMap<String, u64>,
    /// System event mappings (name -> event ID)
    system_events: HashMap<String, SIMCONNECT_EVENTID>,
    /// Reverse mapping (event ID -> name)
    event_names: HashMap<SIMCONNECT_EVENTID, String>,
    /// Next available event ID
    next_event_id: SIMCONNECT_EVENTID,
}

impl EventManager {
    /// Create a new event manager
    pub fn new() -> Self {
        Self {
            standard_events: HashMap::new(),
            input_events: HashMap::new(),
            system_events: HashMap::new(),
            event_names: HashMap::new(),
            next_event_id: EVENT_AIRCRAFT_LOADED,
        }
    }

    /// Map a standard SimConnect event
    pub fn map_standard_event(
        &mut self,
        api: &SimConnectApi,
        handle: HSIMCONNECT,
        event_name: &str,
    ) -> Result<SIMCONNECT_EVENTID, EventError> {
        if let Some(&event_id) = self.standard_events.get(event_name) {
            return Ok(event_id);
        }

        let event_id = self.next_event_id;
        self.next_event_id += 1;

        api.map_client_event_to_sim_event(handle, event_id, event_name)?;

        self.standard_events.insert(event_name.to_string(), event_id);
        self.event_names.insert(event_id, event_name.to_string());

        debug!("Mapped standard event: {} -> {}", event_name, event_id);
        Ok(event_id)
    }

    /// Subscribe to a system event
    pub fn subscribe_system_event(
        &mut self,
        api: &SimConnectApi,
        handle: HSIMCONNECT,
        event_name: &str,
    ) -> Result<SIMCONNECT_EVENTID, EventError> {
        if let Some(&event_id) = self.system_events.get(event_name) {
            return Ok(event_id);
        }

        let event_id = self.next_event_id;
        self.next_event_id += 1;

        api.subscribe_to_system_event(handle, event_id, event_name)?;

        self.system_events.insert(event_name.to_string(), event_id);
        self.event_names.insert(event_id, event_name.to_string());

        debug!("Subscribed to system event: {} -> {}", event_name, event_id);
        Ok(event_id)
    }

    /// Register an Input Event
    pub fn register_input_event(&mut self, event_name: &str) -> u64 {
        if let Some(&hash) = self.input_events.get(event_name) {
            return hash;
        }

        let hash = calculate_input_event_hash(event_name);
        self.input_events.insert(event_name.to_string(), hash);

        debug!("Registered input event: {} -> 0x{:016X}", event_name, hash);
        hash
    }

    /// Transmit a standard event
    pub fn transmit_standard_event(
        &self,
        api: &SimConnectApi,
        handle: HSIMCONNECT,
        event_name: &str,
        data: Option<u32>,
    ) -> Result<(), EventError> {
        let event_id = self.standard_events.get(event_name)
            .ok_or_else(|| EventError::EventNotFound(event_name.to_string()))?;

        api.transmit_client_event(
            handle,
            SIMCONNECT_OBJECT_ID_USER,
            *event_id,
            data.unwrap_or(0),
        )?;

        debug!("Transmitted standard event: {} (data: {:?})", event_name, data);
        Ok(())
    }

    /// Transmit an Input Event (requires MSFS 2024 or compatible aircraft)
    pub fn transmit_input_event(
        &self,
        api: &SimConnectApi,
        handle: HSIMCONNECT,
        event_name: &str,
        value: f64,
    ) -> Result<(), EventError> {
        let hash = self.input_events.get(event_name)
            .ok_or_else(|| EventError::EventNotFound(event_name.to_string()))?;

        // Input Events use a different API call (would need additional SimConnect functions)
        // For now, we'll log the attempt
        debug!("Input event transmission requested: {} (0x{:016X}) = {}", event_name, hash, value);
        
        // TODO: Implement actual Input Event transmission when API is available
        warn!("Input Event transmission not yet implemented: {}", event_name);
        
        Ok(())
    }

    /// Get event name by ID
    pub fn get_event_name(&self, event_id: SIMCONNECT_EVENTID) -> Option<&str> {
        self.event_names.get(&event_id).map(|s| s.as_str())
    }

    /// Get all registered standard events
    pub fn standard_events(&self) -> &HashMap<String, SIMCONNECT_EVENTID> {
        &self.standard_events
    }

    /// Get all registered input events
    pub fn input_events(&self) -> &HashMap<String, u64> {
        &self.input_events
    }

    /// Get all registered system events
    pub fn system_events(&self) -> &HashMap<String, SIMCONNECT_EVENTID> {
        &self.system_events
    }

    /// Setup common flight control events
    pub fn setup_common_events(
        &mut self,
        api: &SimConnectApi,
        handle: HSIMCONNECT,
    ) -> Result<(), EventError> {
        // Flight controls
        self.map_standard_event(api, handle, "AXIS_ELEVATOR_SET")?;
        self.map_standard_event(api, handle, "AXIS_AILERONS_SET")?;
        self.map_standard_event(api, handle, "AXIS_RUDDER_SET")?;
        self.map_standard_event(api, handle, "AXIS_THROTTLE_SET")?;
        self.map_standard_event(api, handle, "AXIS_MIXTURE_SET")?;
        self.map_standard_event(api, handle, "AXIS_PROPELLER_SET")?;

        // Landing gear
        self.map_standard_event(api, handle, "GEAR_TOGGLE")?;
        self.map_standard_event(api, handle, "GEAR_UP")?;
        self.map_standard_event(api, handle, "GEAR_DOWN")?;

        // Flaps
        self.map_standard_event(api, handle, "FLAPS_INCR")?;
        self.map_standard_event(api, handle, "FLAPS_DECR")?;
        self.map_standard_event(api, handle, "FLAPS_SET")?;

        // Autopilot
        self.map_standard_event(api, handle, "AP_MASTER")?;
        self.map_standard_event(api, handle, "AP_ALT_HOLD")?;
        self.map_standard_event(api, handle, "AP_HDG_HOLD")?;
        self.map_standard_event(api, handle, "AP_SPD_HOLD")?;

        // Lights
        self.map_standard_event(api, handle, "TOGGLE_NAV_LIGHTS")?;
        self.map_standard_event(api, handle, "TOGGLE_BEACON_LIGHTS")?;
        self.map_standard_event(api, handle, "TOGGLE_STROBE_LIGHTS")?;
        self.map_standard_event(api, handle, "TOGGLE_LANDING_LIGHTS")?;

        // System events
        self.subscribe_system_event(api, handle, "AircraftLoaded")?;
        self.subscribe_system_event(api, handle, "SimStart")?;
        self.subscribe_system_event(api, handle, "SimStop")?;
        self.subscribe_system_event(api, handle, "Pause")?;

        // Input Events for modern aircraft
        self.register_input_event("AXIS_ELEVATOR_SET");
        self.register_input_event("AXIS_AILERONS_SET");
        self.register_input_event("AXIS_RUDDER_SET");
        self.register_input_event("AXIS_THROTTLE_SET");

        Ok(())
    }
}

impl Default for EventManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate hash for Input Event name (simplified CRC32-like algorithm)
fn calculate_input_event_hash(event_name: &str) -> u64 {
    // This is a simplified hash calculation
    // The actual MSFS Input Event hash algorithm may be different
    let mut hash: u64 = 0xCBF29CE484222325; // FNV-1a offset basis
    
    for byte in event_name.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001B3); // FNV-1a prime
    }
    
    hash
}

/// Common event definitions for different aircraft categories
pub struct CommonEvents;

impl CommonEvents {
    /// Get flight control events for general aviation aircraft
    pub fn ga_flight_controls() -> Vec<&'static str> {
        vec![
            "AXIS_ELEVATOR_SET",
            "AXIS_AILERONS_SET",
            "AXIS_RUDDER_SET",
            "AXIS_THROTTLE_SET",
            "AXIS_MIXTURE_SET",
            "AXIS_PROPELLER_SET",
        ]
    }

    /// Get flight control events for jet aircraft
    pub fn jet_flight_controls() -> Vec<&'static str> {
        vec![
            "AXIS_ELEVATOR_SET",
            "AXIS_AILERONS_SET",
            "AXIS_RUDDER_SET",
            "AXIS_THROTTLE_SET",
            "AXIS_SPOILER_SET",
        ]
    }

    /// Get flight control events for helicopters
    pub fn helicopter_flight_controls() -> Vec<&'static str> {
        vec![
            "AXIS_CYCLIC_LATERAL_SET",
            "AXIS_CYCLIC_LONGITUDINAL_SET",
            "AXIS_COLLECTIVE_SET",
            "AXIS_PEDAL_SET",
        ]
    }

    /// Get system events for all aircraft
    pub fn system_events() -> Vec<&'static str> {
        vec![
            "AircraftLoaded",
            "SimStart",
            "SimStop",
            "Pause",
            "FlightLoaded",
            "FlightSaved",
        ]
    }

    /// Get autopilot events
    pub fn autopilot_events() -> Vec<&'static str> {
        vec![
            "AP_MASTER",
            "AP_ALT_HOLD",
            "AP_HDG_HOLD",
            "AP_SPD_HOLD",
            "AP_VS_HOLD",
            "AP_NAV1_HOLD",
            "AP_APR_HOLD",
        ]
    }

    /// Get lighting events
    pub fn lighting_events() -> Vec<&'static str> {
        vec![
            "TOGGLE_NAV_LIGHTS",
            "TOGGLE_BEACON_LIGHTS",
            "TOGGLE_STROBE_LIGHTS",
            "TOGGLE_LANDING_LIGHTS",
            "TOGGLE_TAXI_LIGHTS",
            "TOGGLE_LOGO_LIGHTS",
            "TOGGLE_WING_LIGHTS",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_manager_creation() {
        let manager = EventManager::new();
        assert_eq!(manager.standard_events.len(), 0);
        assert_eq!(manager.input_events.len(), 0);
        assert_eq!(manager.system_events.len(), 0);
        assert_eq!(manager.next_event_id, EVENT_AIRCRAFT_LOADED);
    }

    #[test]
    fn test_input_event_hash_calculation() {
        let hash1 = calculate_input_event_hash("AXIS_ELEVATOR_SET");
        let hash2 = calculate_input_event_hash("AXIS_ELEVATOR_SET");
        let hash3 = calculate_input_event_hash("AXIS_AILERONS_SET");

        // Same input should produce same hash
        assert_eq!(hash1, hash2);
        // Different inputs should produce different hashes
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_input_event_registration() {
        let mut manager = EventManager::new();
        
        let hash1 = manager.register_input_event("AXIS_ELEVATOR_SET");
        let hash2 = manager.register_input_event("AXIS_ELEVATOR_SET"); // Same event
        let hash3 = manager.register_input_event("AXIS_AILERONS_SET"); // Different event

        assert_eq!(hash1, hash2); // Should return same hash for same event
        assert_ne!(hash1, hash3); // Should return different hash for different event
        assert_eq!(manager.input_events.len(), 2);
    }

    #[test]
    fn test_common_events() {
        let ga_controls = CommonEvents::ga_flight_controls();
        assert!(ga_controls.contains(&"AXIS_ELEVATOR_SET"));
        assert!(ga_controls.contains(&"AXIS_MIXTURE_SET"));

        let jet_controls = CommonEvents::jet_flight_controls();
        assert!(jet_controls.contains(&"AXIS_ELEVATOR_SET"));
        assert!(jet_controls.contains(&"AXIS_SPOILER_SET"));
        assert!(!jet_controls.contains(&"AXIS_MIXTURE_SET"));

        let helo_controls = CommonEvents::helicopter_flight_controls();
        assert!(helo_controls.contains(&"AXIS_COLLECTIVE_SET"));
        assert!(helo_controls.contains(&"AXIS_PEDAL_SET"));

        let system_events = CommonEvents::system_events();
        assert!(system_events.contains(&"AircraftLoaded"));
        assert!(system_events.contains(&"SimStart"));
    }

    #[test]
    fn test_sim_event_types() {
        let standard_event = SimEvent::Standard {
            name: "GEAR_TOGGLE".to_string(),
            data: Some(1),
        };

        let input_event = SimEvent::Input {
            hash: 0x1234567890ABCDEF,
            value: 0.5,
        };

        let system_event = SimEvent::System {
            name: "AircraftLoaded".to_string(),
        };

        match standard_event {
            SimEvent::Standard { name, data } => {
                assert_eq!(name, "GEAR_TOGGLE");
                assert_eq!(data, Some(1));
            }
            _ => panic!("Wrong event type"),
        }

        match input_event {
            SimEvent::Input { hash, value } => {
                assert_eq!(hash, 0x1234567890ABCDEF);
                assert_eq!(value, 0.5);
            }
            _ => panic!("Wrong event type"),
        }

        match system_event {
            SimEvent::System { name } => {
                assert_eq!(name, "AircraftLoaded");
            }
            _ => panic!("Wrong event type"),
        }
    }
}