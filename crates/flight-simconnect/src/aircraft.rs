//! Aircraft detection and identification for MSFS
//!
//! Provides aircraft detection via ATC model/type for auto-profile switching
//! and aircraft-specific variable mapping selection.

use flight_bus::types::AircraftId;
use flight_simconnect_sys::{
    constants::*, SimConnectApi, HSIMCONNECT, SIMCONNECT_DATADEFID, SIMCONNECT_DATATYPE,
    SIMCONNECT_PERIOD, SIMCONNECT_REQUESTID,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::CString;
use thiserror::Error;
use tracing::{debug, info};

/// Aircraft information from SimConnect
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AircraftInfo {
    /// Aircraft title from SimConnect
    pub title: String,
    /// ATC model (e.g., "C172")
    pub atc_model: String,
    /// ATC type (e.g., "CESSNA")
    pub atc_type: String,
    /// ATC airline (if applicable)
    pub atc_airline: Option<String>,
    /// ATC flight number (if applicable)
    pub atc_flight_number: Option<String>,
    /// Aircraft category (e.g., "Airplane", "Helicopter")
    pub category: AircraftCategory,
    /// Engine type
    pub engine_type: EngineType,
    /// Number of engines
    pub engine_count: u8,
}

/// Aircraft category enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AircraftCategory {
    Airplane,
    Helicopter,
    Glider,
    Unknown,
}

/// Engine type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineType {
    Piston,
    Turboprop,
    Jet,
    Turboshaft,
    Electric,
    Unknown,
}

/// Aircraft detection error types
#[derive(Debug, Error)]
pub enum DetectionError {
    #[error("SimConnect API error: {0}")]
    SimConnect(#[from] flight_simconnect_sys::SimConnectError),
    #[error("Aircraft data not available")]
    DataNotAvailable,
    #[error("Invalid aircraft data format")]
    InvalidFormat,
    #[error("Aircraft detection timeout")]
    Timeout,
}

/// Aircraft detector for MSFS
pub struct AircraftDetector {
    definition_id: SIMCONNECT_DATADEFID,
    request_id: SIMCONNECT_REQUESTID,
    current_aircraft: Option<AircraftInfo>,
    detection_callbacks: Vec<Box<dyn Fn(&AircraftInfo) + Send + Sync>>,
}

impl AircraftDetector {
    /// Create a new aircraft detector
    pub fn new() -> Self {
        Self {
            definition_id: DATA_DEFINITION_AIRCRAFT + 100, // Use unique ID
            request_id: REQUEST_AIRCRAFT_DATA + 100,
            current_aircraft: None,
            detection_callbacks: Vec::new(),
        }
    }

    /// Setup aircraft detection data definition
    pub fn setup_detection(
        &self,
        api: &SimConnectApi,
        handle: HSIMCONNECT,
    ) -> Result<(), DetectionError> {
        // Add aircraft identification variables
        api.add_to_data_definition(
            handle,
            self.definition_id,
            "TITLE",
            "",
            SIMCONNECT_DATATYPE::STRING256,
            0.0,
            0,
        )?;

        api.add_to_data_definition(
            handle,
            self.definition_id,
            "ATC MODEL",
            "",
            SIMCONNECT_DATATYPE::STRING32,
            0.0,
            1,
        )?;

        api.add_to_data_definition(
            handle,
            self.definition_id,
            "ATC TYPE",
            "",
            SIMCONNECT_DATATYPE::STRING32,
            0.0,
            2,
        )?;

        api.add_to_data_definition(
            handle,
            self.definition_id,
            "ATC AIRLINE",
            "",
            SIMCONNECT_DATATYPE::STRING64,
            0.0,
            3,
        )?;

        api.add_to_data_definition(
            handle,
            self.definition_id,
            "ATC FLIGHT NUMBER",
            "",
            SIMCONNECT_DATATYPE::STRING32,
            0.0,
            4,
        )?;

        api.add_to_data_definition(
            handle,
            self.definition_id,
            "CATEGORY",
            "",
            SIMCONNECT_DATATYPE::STRING32,
            0.0,
            5,
        )?;

        api.add_to_data_definition(
            handle,
            self.definition_id,
            "ENGINE TYPE",
            "enum",
            SIMCONNECT_DATATYPE::INT32,
            0.0,
            6,
        )?;

        api.add_to_data_definition(
            handle,
            self.definition_id,
            "NUMBER OF ENGINES",
            "number",
            SIMCONNECT_DATATYPE::INT32,
            0.0,
            7,
        )?;

        Ok(())
    }

    /// Start aircraft detection
    pub fn start_detection(
        &self,
        api: &SimConnectApi,
        handle: HSIMCONNECT,
    ) -> Result<(), DetectionError> {
        api.request_data_on_sim_object(
            handle,
            self.request_id,
            self.definition_id,
            SIMCONNECT_OBJECT_ID_USER,
            SIMCONNECT_PERIOD::ONCE,
        )?;

        Ok(())
    }

    /// Process aircraft data from SimConnect
    pub fn process_aircraft_data(&mut self, data: &[u8]) -> Result<Option<AircraftInfo>, DetectionError> {
        if data.len() < 256 + 32 + 32 + 64 + 32 + 32 + 4 + 4 {
            return Err(DetectionError::InvalidFormat);
        }

        let mut offset = 0;

        // Extract title (256 bytes)
        let title = extract_string(&data[offset..offset + 256]);
        offset += 256;

        // Extract ATC model (32 bytes)
        let atc_model = extract_string(&data[offset..offset + 32]);
        offset += 32;

        // Extract ATC type (32 bytes)
        let atc_type = extract_string(&data[offset..offset + 32]);
        offset += 32;

        // Extract ATC airline (64 bytes)
        let atc_airline = extract_optional_string(&data[offset..offset + 64]);
        offset += 64;

        // Extract ATC flight number (32 bytes)
        let atc_flight_number = extract_optional_string(&data[offset..offset + 32]);
        offset += 32;

        // Extract category (32 bytes)
        let category_str = extract_string(&data[offset..offset + 32]);
        let category = parse_aircraft_category(&category_str);
        offset += 32;

        // Extract engine type (4 bytes)
        let engine_type_raw = i32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        let engine_type = parse_engine_type(engine_type_raw);
        offset += 4;

        // Extract engine count (4 bytes)
        let engine_count = i32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as u8;

        let aircraft_info = AircraftInfo {
            title,
            atc_model: atc_model.clone(),
            atc_type,
            atc_airline,
            atc_flight_number,
            category,
            engine_type,
            engine_count,
        };

        // Check if aircraft changed
        let aircraft_changed = self.current_aircraft.as_ref()
            .map(|current| current.atc_model != aircraft_info.atc_model)
            .unwrap_or(true);

        if aircraft_changed {
            info!("Aircraft detected: {} ({})", aircraft_info.title, aircraft_info.atc_model);
            self.current_aircraft = Some(aircraft_info.clone());

            // Notify callbacks
            for callback in &self.detection_callbacks {
                callback(&aircraft_info);
            }

            Ok(Some(aircraft_info))
        } else {
            Ok(None)
        }
    }

    /// Get current aircraft information
    pub fn current_aircraft(&self) -> Option<&AircraftInfo> {
        self.current_aircraft.as_ref()
    }

    /// Add aircraft detection callback
    pub fn add_detection_callback<F>(&mut self, callback: F)
    where
        F: Fn(&AircraftInfo) + Send + Sync + 'static,
    {
        self.detection_callbacks.push(Box::new(callback));
    }

    /// Get request ID for this detector
    pub fn request_id(&self) -> SIMCONNECT_REQUESTID {
        self.request_id
    }

    /// Convert aircraft info to Flight Hub aircraft ID
    pub fn to_aircraft_id(&self, info: &AircraftInfo) -> AircraftId {
        // Use ATC model as the primary identifier, fallback to title
        let icao = if !info.atc_model.is_empty() {
            info.atc_model.clone()
        } else {
            // Extract ICAO from title if possible
            extract_icao_from_title(&info.title).unwrap_or_else(|| "UNKNOWN".to_string())
        };

        AircraftId::new(&icao)
    }
}

impl Default for AircraftDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract null-terminated string from byte buffer
fn extract_string(data: &[u8]) -> String {
    let null_pos = data.iter().position(|&b| b == 0).unwrap_or(data.len());
    String::from_utf8_lossy(&data[..null_pos]).to_string()
}

/// Extract optional string (empty if all zeros or whitespace)
fn extract_optional_string(data: &[u8]) -> Option<String> {
    let s = extract_string(data);
    if s.trim().is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Parse aircraft category from string
fn parse_aircraft_category(category: &str) -> AircraftCategory {
    match category.to_uppercase().as_str() {
        "AIRPLANE" => AircraftCategory::Airplane,
        "HELICOPTER" => AircraftCategory::Helicopter,
        "GLIDER" => AircraftCategory::Glider,
        _ => AircraftCategory::Unknown,
    }
}

/// Parse engine type from SimConnect enum value
fn parse_engine_type(engine_type: i32) -> EngineType {
    match engine_type {
        0 => EngineType::Piston,
        1 => EngineType::Jet,
        2 => EngineType::Unknown, // None
        3 => EngineType::Turboprop,
        4 => EngineType::Turboshaft,
        5 => EngineType::Electric,
        _ => EngineType::Unknown,
    }
}

/// Extract ICAO code from aircraft title
fn extract_icao_from_title(title: &str) -> Option<String> {
    // Common patterns for ICAO extraction
    let patterns = [
        // "Cessna 172 Skyhawk" -> "C172"
        r"(?i)cessna\s+(\d+)",
        // "Boeing 737-800" -> "B738"
        r"(?i)boeing\s+(\d+)-?(\d+)?",
        // "Airbus A320neo" -> "A320"
        r"(?i)airbus\s+a(\d+)",
        // "Piper PA-28" -> "PA28"
        r"(?i)piper\s+pa-?(\d+)",
    ];

    for pattern in &patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            if let Some(captures) = re.captures(title) {
                if let Some(model) = captures.get(1) {
                    return Some(format!("C{}", model.as_str())); // Simplified mapping
                }
            }
        }
    }

    // Fallback: try to extract any alphanumeric sequence that looks like an ICAO
    if let Ok(re) = regex::Regex::new(r"[A-Z]\d{3}|[A-Z]{2}\d{2}|[A-Z]{3}\d") {
        if let Some(m) = re.find(title) {
            return Some(m.as_str().to_string());
        }
    }

    None
}

/// Aircraft mapping database for common aircraft types
pub struct AircraftDatabase {
    mappings: HashMap<String, AircraftMapping>,
}

#[derive(Debug, Clone)]
pub struct AircraftMapping {
    pub icao: String,
    pub name: String,
    pub category: AircraftCategory,
    pub engine_type: EngineType,
    pub profile_hints: Vec<String>,
}

impl AircraftDatabase {
    /// Create a new aircraft database with common mappings
    pub fn new() -> Self {
        let mut mappings = HashMap::new();

        // General Aviation
        mappings.insert("C172".to_string(), AircraftMapping {
            icao: "C172".to_string(),
            name: "Cessna 172".to_string(),
            category: AircraftCategory::Airplane,
            engine_type: EngineType::Piston,
            profile_hints: vec!["ga".to_string(), "single-engine".to_string()],
        });

        mappings.insert("PA28".to_string(), AircraftMapping {
            icao: "PA28".to_string(),
            name: "Piper Cherokee".to_string(),
            category: AircraftCategory::Airplane,
            engine_type: EngineType::Piston,
            profile_hints: vec!["ga".to_string(), "single-engine".to_string()],
        });

        // Commercial Aviation
        mappings.insert("A320".to_string(), AircraftMapping {
            icao: "A320".to_string(),
            name: "Airbus A320".to_string(),
            category: AircraftCategory::Airplane,
            engine_type: EngineType::Jet,
            profile_hints: vec!["airliner".to_string(), "fbw".to_string()],
        });

        mappings.insert("B738".to_string(), AircraftMapping {
            icao: "B738".to_string(),
            name: "Boeing 737-800".to_string(),
            category: AircraftCategory::Airplane,
            engine_type: EngineType::Jet,
            profile_hints: vec!["airliner".to_string(), "boeing".to_string()],
        });

        // Helicopters
        mappings.insert("R22".to_string(), AircraftMapping {
            icao: "R22".to_string(),
            name: "Robinson R22".to_string(),
            category: AircraftCategory::Helicopter,
            engine_type: EngineType::Piston,
            profile_hints: vec!["helicopter".to_string(), "training".to_string()],
        });

        Self { mappings }
    }

    /// Get aircraft mapping by ICAO code
    pub fn get_mapping(&self, icao: &str) -> Option<&AircraftMapping> {
        self.mappings.get(icao)
    }

    /// Add or update aircraft mapping
    pub fn add_mapping(&mut self, icao: String, mapping: AircraftMapping) {
        self.mappings.insert(icao, mapping);
    }

    /// Get all available ICAO codes
    pub fn available_aircraft(&self) -> Vec<&str> {
        self.mappings.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for AircraftDatabase {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aircraft_detector_creation() {
        let detector = AircraftDetector::new();
        assert!(detector.current_aircraft().is_none());
        assert_eq!(detector.detection_callbacks.len(), 0);
    }

    #[test]
    fn test_string_extraction() {
        let data = b"Hello\0World\0\0\0";
        assert_eq!(extract_string(data), "Hello");

        let empty_data = b"\0\0\0\0";
        assert_eq!(extract_string(empty_data), "");

        let no_null_data = b"Test";
        assert_eq!(extract_string(no_null_data), "Test");
    }

    #[test]
    fn test_optional_string_extraction() {
        let data = b"Value\0\0\0";
        assert_eq!(extract_optional_string(data), Some("Value".to_string()));

        let empty_data = b"\0\0\0\0";
        assert_eq!(extract_optional_string(empty_data), None);

        let whitespace_data = b"   \0\0\0";
        assert_eq!(extract_optional_string(whitespace_data), None);
    }

    #[test]
    fn test_aircraft_category_parsing() {
        assert_eq!(parse_aircraft_category("AIRPLANE"), AircraftCategory::Airplane);
        assert_eq!(parse_aircraft_category("airplane"), AircraftCategory::Airplane);
        assert_eq!(parse_aircraft_category("HELICOPTER"), AircraftCategory::Helicopter);
        assert_eq!(parse_aircraft_category("GLIDER"), AircraftCategory::Glider);
        assert_eq!(parse_aircraft_category("UNKNOWN"), AircraftCategory::Unknown);
    }

    #[test]
    fn test_engine_type_parsing() {
        assert_eq!(parse_engine_type(0), EngineType::Piston);
        assert_eq!(parse_engine_type(1), EngineType::Jet);
        assert_eq!(parse_engine_type(3), EngineType::Turboprop);
        assert_eq!(parse_engine_type(4), EngineType::Turboshaft);
        assert_eq!(parse_engine_type(5), EngineType::Electric);
        assert_eq!(parse_engine_type(999), EngineType::Unknown);
    }

    #[test]
    fn test_aircraft_database() {
        let db = AircraftDatabase::new();
        
        let c172 = db.get_mapping("C172").unwrap();
        assert_eq!(c172.name, "Cessna 172");
        assert_eq!(c172.category, AircraftCategory::Airplane);
        assert_eq!(c172.engine_type, EngineType::Piston);

        let a320 = db.get_mapping("A320").unwrap();
        assert_eq!(a320.name, "Airbus A320");
        assert_eq!(a320.category, AircraftCategory::Airplane);
        assert_eq!(a320.engine_type, EngineType::Jet);

        assert!(db.get_mapping("NONEXISTENT").is_none());
    }

    #[test]
    fn test_aircraft_id_conversion() {
        let detector = AircraftDetector::new();
        
        let aircraft_info = AircraftInfo {
            title: "Cessna 172 Skyhawk".to_string(),
            atc_model: "C172".to_string(),
            atc_type: "CESSNA".to_string(),
            atc_airline: None,
            atc_flight_number: None,
            category: AircraftCategory::Airplane,
            engine_type: EngineType::Piston,
            engine_count: 1,
        };

        let aircraft_id = detector.to_aircraft_id(&aircraft_info);
        assert_eq!(aircraft_id.icao, "C172");
    }
}