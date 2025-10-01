//! Aircraft detection for X-Plane
//!
//! Provides aircraft detection capabilities by querying X-Plane DataRefs
//! to identify the currently loaded aircraft and its characteristics.

use crate::{
    dataref::{DataRef, DataRefValue},
    udp::UdpClient,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use tracing::{debug, info, warn};

/// Aircraft detection errors
#[derive(Error, Debug)]
pub enum AircraftDetectionError {
    #[error("Failed to query aircraft DataRefs: {message}")]
    DataRefQuery { message: String },
    #[error("Aircraft information incomplete: missing {field}")]
    IncompleteInfo { field: String },
    #[error("Invalid aircraft data: {reason}")]
    InvalidData { reason: String },
    #[error("Communication error: {0}")]
    Communication(#[from] crate::udp::UdpError),
}

/// Detected aircraft information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectedAircraft {
    pub icao: String,
    pub title: String,
    pub author: String,
}

/// Extended aircraft information from X-Plane
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XPlaneAircraftInfo {
    pub basic: DetectedAircraft,
    pub file_path: Option<String>,
    pub engine_count: u32,
    pub aircraft_type: AircraftType,
    pub max_weight: Option<f32>,
    pub fuel_capacity: Option<f32>,
    pub engine_type: EngineType,
}

/// Aircraft type classification
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum AircraftType {
    GeneralAviation,
    Airliner,
    Fighter,
    Helicopter,
    Glider,
    Seaplane,
    Unknown,
}

/// Engine type classification
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum EngineType {
    Piston,
    Turboprop,
    Jet,
    Turboshaft,
    Electric,
    Unknown,
}

/// Aircraft detector for X-Plane
#[derive(Clone)]
pub struct AircraftDetector {
    /// Cache of known aircraft mappings
    aircraft_mappings: HashMap<String, AircraftType>,
    /// Engine type mappings
    engine_mappings: HashMap<String, EngineType>,
}

impl AircraftDetector {
    pub fn new() -> Self {
        let mut detector = Self {
            aircraft_mappings: HashMap::new(),
            engine_mappings: HashMap::new(),
        };

        detector.initialize_aircraft_mappings();
        detector.initialize_engine_mappings();
        
        detector
    }

    /// Detect currently loaded aircraft
    pub async fn detect_aircraft(&self, udp_client: &UdpClient) -> Result<DetectedAircraft, AircraftDetectionError> {
        debug!("Starting aircraft detection");

        // Query basic aircraft information DataRefs
        let aircraft_datarefs = vec![
            DataRef::new("sim/aircraft/view/acf_ICAO".to_string()),
            DataRef::new("sim/aircraft/view/acf_descrip".to_string()),
            DataRef::new("sim/aircraft/view/acf_author".to_string()),
        ];

        let mut aircraft_data = HashMap::new();

        // Collect aircraft information
        for dataref in &aircraft_datarefs {
            match udp_client.request_dataref(dataref).await {
                Ok(value) => {
                    aircraft_data.insert(dataref.name.clone(), value);
                }
                Err(e) => {
                    warn!("Failed to get aircraft DataRef {}: {}", dataref.name, e);
                    return Err(AircraftDetectionError::DataRefQuery {
                        message: format!("Failed to get {}: {}", dataref.name, e),
                    });
                }
            }
        }

        // Extract aircraft information
        let icao = self.extract_string_value(&aircraft_data, "sim/aircraft/view/acf_ICAO")?;
        let title = self.extract_string_value(&aircraft_data, "sim/aircraft/view/acf_descrip")?;
        let author = self.extract_string_value(&aircraft_data, "sim/aircraft/view/acf_author")?;

        // Clean up the ICAO code (X-Plane sometimes includes extra characters)
        let icao = self.clean_icao_code(&icao);

        let detected = DetectedAircraft {
            icao: icao.clone(),
            title: title.clone(),
            author: author.clone(),
        };

        info!("Detected aircraft: {} - {} by {}", icao, title, author);
        
        Ok(detected)
    }

    /// Get extended aircraft information
    pub async fn get_extended_info(&self, udp_client: &UdpClient, basic: DetectedAircraft) -> Result<XPlaneAircraftInfo, AircraftDetectionError> {
        debug!("Getting extended aircraft information for {}", basic.icao);

        // Query extended DataRefs
        let extended_datarefs = vec![
            DataRef::new("sim/aircraft/view/acf_file_path".to_string()),
            DataRef::new("sim/aircraft/engine/acf_num_engines".to_string()),
            DataRef::new("sim/aircraft/weight/acf_m_max".to_string()),
            DataRef::new("sim/aircraft/overflow/acf_fuel_tot".to_string()),
            DataRef::new("sim/aircraft/prop/acf_en_type[0]".to_string()),
        ];

        let mut extended_data = HashMap::new();

        // Collect extended information (best effort)
        for dataref in &extended_datarefs {
            match udp_client.request_dataref(dataref).await {
                Ok(value) => {
                    extended_data.insert(dataref.name.clone(), value);
                }
                Err(e) => {
                    debug!("Could not get extended DataRef {}: {}", dataref.name, e);
                    // Continue with partial information
                }
            }
        }

        // Extract extended information
        let file_path = self.extract_string_value(&extended_data, "sim/aircraft/view/acf_file_path").ok();
        
        let engine_count = self.extract_int_value(&extended_data, "sim/aircraft/engine/acf_num_engines")
            .unwrap_or(1) as u32;
        
        let max_weight = self.extract_float_value(&extended_data, "sim/aircraft/weight/acf_m_max").ok();
        
        let fuel_capacity = self.extract_float_value(&extended_data, "sim/aircraft/overflow/acf_fuel_tot").ok();

        // Determine aircraft and engine types
        let aircraft_type = self.classify_aircraft_type(&basic.icao, &basic.title, engine_count);
        let engine_type = self.classify_engine_type(&basic.icao, &extended_data);

        Ok(XPlaneAircraftInfo {
            basic,
            file_path,
            engine_count,
            aircraft_type,
            max_weight,
            fuel_capacity,
            engine_type,
        })
    }

    /// Initialize aircraft type mappings
    fn initialize_aircraft_mappings(&mut self) {
        let mappings = vec![
            // General Aviation
            ("C172", AircraftType::GeneralAviation),
            ("C182", AircraftType::GeneralAviation),
            ("C208", AircraftType::GeneralAviation),
            ("PA28", AircraftType::GeneralAviation),
            ("SR22", AircraftType::GeneralAviation),
            ("BE36", AircraftType::GeneralAviation),
            ("M20P", AircraftType::GeneralAviation),
            
            // Airliners
            ("A320", AircraftType::Airliner),
            ("A321", AircraftType::Airliner),
            ("A330", AircraftType::Airliner),
            ("A340", AircraftType::Airliner),
            ("A380", AircraftType::Airliner),
            ("B737", AircraftType::Airliner),
            ("B738", AircraftType::Airliner),
            ("B747", AircraftType::Airliner),
            ("B777", AircraftType::Airliner),
            ("B787", AircraftType::Airliner),
            ("CRJ2", AircraftType::Airliner),
            ("E145", AircraftType::Airliner),
            
            // Helicopters
            ("UH1H", AircraftType::Helicopter),
            ("AH64", AircraftType::Helicopter),
            ("R22", AircraftType::Helicopter),
            ("R44", AircraftType::Helicopter),
            ("EC35", AircraftType::Helicopter),
            ("S76", AircraftType::Helicopter),
            
            // Fighters
            ("F16", AircraftType::Fighter),
            ("F18", AircraftType::Fighter),
            ("F22", AircraftType::Fighter),
            ("A10", AircraftType::Fighter),
            
            // Gliders
            ("ASK21", AircraftType::Glider),
            ("DG808", AircraftType::Glider),
            
            // Seaplanes
            ("DHC2", AircraftType::Seaplane),
            ("C208F", AircraftType::Seaplane),
        ];

        for (icao, aircraft_type) in mappings {
            self.aircraft_mappings.insert(icao.to_string(), aircraft_type);
        }
    }

    /// Initialize engine type mappings
    fn initialize_engine_mappings(&mut self) {
        let mappings = vec![
            // Piston engines
            ("C172", EngineType::Piston),
            ("C182", EngineType::Piston),
            ("PA28", EngineType::Piston),
            ("SR22", EngineType::Piston),
            
            // Turboprops
            ("C208", EngineType::Turboprop),
            ("DHC2", EngineType::Turboprop),
            
            // Jets
            ("A320", EngineType::Jet),
            ("A321", EngineType::Jet),
            ("B737", EngineType::Jet),
            ("B777", EngineType::Jet),
            ("F16", EngineType::Jet),
            ("F18", EngineType::Jet),
            
            // Turboshafts (helicopters)
            ("UH1H", EngineType::Turboshaft),
            ("AH64", EngineType::Turboshaft),
            ("EC35", EngineType::Turboshaft),
            ("S76", EngineType::Turboshaft),
        ];

        for (icao, engine_type) in mappings {
            self.engine_mappings.insert(icao.to_string(), engine_type);
        }
    }

    /// Extract string value from DataRef response
    fn extract_string_value(&self, data: &HashMap<String, DataRefValue>, key: &str) -> Result<String, AircraftDetectionError> {
        match data.get(key) {
            Some(DataRefValue::Float(f)) => {
                // X-Plane sometimes returns strings as float arrays or encoded floats
                // This is a simplified approach - in practice, string DataRefs are more complex
                Ok(format!("{}", *f as i32))
            }
            Some(DataRefValue::Int(i)) => {
                Ok(i.to_string())
            }
            Some(DataRefValue::FloatArray(arr)) => {
                // Convert float array to string (common for X-Plane string DataRefs)
                let bytes: Vec<u8> = arr.iter()
                    .take_while(|&&f| f != 0.0)
                    .map(|&f| f as u8)
                    .collect();
                
                String::from_utf8(bytes)
                    .map_err(|_| AircraftDetectionError::InvalidData {
                        reason: "Invalid UTF-8 in string DataRef".to_string(),
                    })
            }
            Some(value) => {
                // Fallback: convert any value to string
                Ok(value.to_string())
            }
            None => Err(AircraftDetectionError::IncompleteInfo {
                field: key.to_string(),
            }),
        }
    }

    /// Extract integer value from DataRef response
    fn extract_int_value(&self, data: &HashMap<String, DataRefValue>, key: &str) -> Result<i32, AircraftDetectionError> {
        match data.get(key) {
            Some(DataRefValue::Int(i)) => Ok(*i),
            Some(DataRefValue::Float(f)) => Ok(*f as i32),
            Some(DataRefValue::Double(d)) => Ok(*d as i32),
            _ => Err(AircraftDetectionError::IncompleteInfo {
                field: key.to_string(),
            }),
        }
    }

    /// Extract float value from DataRef response
    fn extract_float_value(&self, data: &HashMap<String, DataRefValue>, key: &str) -> Result<f32, AircraftDetectionError> {
        match data.get(key) {
            Some(DataRefValue::Float(f)) => Ok(*f),
            Some(DataRefValue::Double(d)) => Ok(*d as f32),
            Some(DataRefValue::Int(i)) => Ok(*i as f32),
            _ => Err(AircraftDetectionError::IncompleteInfo {
                field: key.to_string(),
            }),
        }
    }

    /// Clean up ICAO code from X-Plane
    fn clean_icao_code(&self, raw_icao: &str) -> String {
        // Remove common X-Plane suffixes and clean up
        let cleaned = raw_icao
            .trim()
            .replace('\0', "") // Remove null terminators
            .replace(' ', "")   // Remove spaces
            .to_uppercase();

        // Take first 4 characters for standard ICAO
        if cleaned.len() > 4 {
            cleaned[..4].to_string()
        } else {
            cleaned
        }
    }

    /// Classify aircraft type based on ICAO and title
    fn classify_aircraft_type(&self, icao: &str, title: &str, engine_count: u32) -> AircraftType {
        // Check direct mapping first
        if let Some(aircraft_type) = self.aircraft_mappings.get(icao) {
            return aircraft_type.clone();
        }

        // Classify based on title keywords
        let title_lower = title.to_lowercase();
        
        if title_lower.contains("helicopter") || title_lower.contains("helo") {
            return AircraftType::Helicopter;
        }
        
        if title_lower.contains("fighter") || title_lower.contains("f-") || title_lower.contains("f/") {
            return AircraftType::Fighter;
        }
        
        if title_lower.contains("glider") || title_lower.contains("sailplane") {
            return AircraftType::Glider;
        }
        
        if title_lower.contains("seaplane") || title_lower.contains("floatplane") {
            return AircraftType::Seaplane;
        }
        
        // Classify based on engine count and other characteristics
        if engine_count >= 2 {
            // Multi-engine aircraft are likely airliners or large GA
            if title_lower.contains("boeing") || title_lower.contains("airbus") || 
               title_lower.contains("737") || title_lower.contains("320") ||
               title_lower.contains("777") || title_lower.contains("787") {
                return AircraftType::Airliner;
            }
        }
        
        // Default to GA for single-engine aircraft
        if engine_count == 1 {
            return AircraftType::GeneralAviation;
        }

        AircraftType::Unknown
    }

    /// Classify engine type based on ICAO and DataRef information
    fn classify_engine_type(&self, icao: &str, _extended_data: &HashMap<String, DataRefValue>) -> EngineType {
        // Check direct mapping first
        if let Some(engine_type) = self.engine_mappings.get(icao) {
            return engine_type.clone();
        }

        // TODO: Use extended_data to determine engine type from X-Plane DataRefs
        // For now, use ICAO-based classification
        
        // Default classification based on aircraft type
        match self.aircraft_mappings.get(icao) {
            Some(AircraftType::GeneralAviation) => EngineType::Piston,
            Some(AircraftType::Airliner) => EngineType::Jet,
            Some(AircraftType::Fighter) => EngineType::Jet,
            Some(AircraftType::Helicopter) => EngineType::Turboshaft,
            _ => EngineType::Unknown,
        }
    }

    /// Check if aircraft has changed
    pub fn has_aircraft_changed(&self, current: &DetectedAircraft, previous: &Option<DetectedAircraft>) -> bool {
        match previous {
            Some(prev) => current.icao != prev.icao || current.title != prev.title,
            None => true,
        }
    }

    /// Get aircraft capabilities based on type
    pub fn get_aircraft_capabilities(&self, aircraft_type: AircraftType) -> Vec<String> {
        match aircraft_type {
            AircraftType::GeneralAviation => vec![
                "basic_flight_controls".to_string(),
                "engine_management".to_string(),
                "navigation".to_string(),
            ],
            AircraftType::Airliner => vec![
                "basic_flight_controls".to_string(),
                "engine_management".to_string(),
                "navigation".to_string(),
                "autopilot".to_string(),
                "flight_management".to_string(),
                "multiple_engines".to_string(),
            ],
            AircraftType::Helicopter => vec![
                "helicopter_controls".to_string(),
                "collective".to_string(),
                "anti_torque".to_string(),
                "rotor_management".to_string(),
            ],
            AircraftType::Fighter => vec![
                "basic_flight_controls".to_string(),
                "engine_management".to_string(),
                "weapons_systems".to_string(),
                "high_performance".to_string(),
            ],
            AircraftType::Glider => vec![
                "basic_flight_controls".to_string(),
                "soaring".to_string(),
            ],
            AircraftType::Seaplane => vec![
                "basic_flight_controls".to_string(),
                "engine_management".to_string(),
                "water_operations".to_string(),
            ],
            AircraftType::Unknown => vec![
                "basic_flight_controls".to_string(),
            ],
        }
    }
}

impl Default for AircraftDetector {
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
        assert!(!detector.aircraft_mappings.is_empty());
        assert!(!detector.engine_mappings.is_empty());
    }

    #[test]
    fn test_icao_cleaning() {
        let detector = AircraftDetector::new();
        
        assert_eq!(detector.clean_icao_code("C172\0\0\0"), "C172");
        assert_eq!(detector.clean_icao_code("c172 "), "C172");
        assert_eq!(detector.clean_icao_code("C172SP"), "C172");
        assert_eq!(detector.clean_icao_code("A320"), "A320");
    }

    #[test]
    fn test_aircraft_type_classification() {
        let detector = AircraftDetector::new();
        
        // Direct mapping
        assert_eq!(
            detector.classify_aircraft_type("C172", "Cessna 172", 1),
            AircraftType::GeneralAviation
        );
        
        assert_eq!(
            detector.classify_aircraft_type("A320", "Airbus A320", 2),
            AircraftType::Airliner
        );
        
        // Title-based classification
        assert_eq!(
            detector.classify_aircraft_type("UNKN", "Test Helicopter", 1),
            AircraftType::Helicopter
        );
        
        assert_eq!(
            detector.classify_aircraft_type("UNKN", "F-16 Fighter", 1),
            AircraftType::Fighter
        );
        
        // Engine count-based classification
        assert_eq!(
            detector.classify_aircraft_type("UNKN", "Unknown Aircraft", 1),
            AircraftType::GeneralAviation
        );
    }

    #[test]
    fn test_engine_type_classification() {
        let detector = AircraftDetector::new();
        let empty_data = HashMap::new();
        
        assert_eq!(
            detector.classify_engine_type("C172", &empty_data),
            EngineType::Piston
        );
        
        assert_eq!(
            detector.classify_engine_type("A320", &empty_data),
            EngineType::Jet
        );
        
        assert_eq!(
            detector.classify_engine_type("UH1H", &empty_data),
            EngineType::Turboshaft
        );
    }

    #[test]
    fn test_aircraft_change_detection() {
        let detector = AircraftDetector::new();
        
        let aircraft1 = DetectedAircraft {
            icao: "C172".to_string(),
            title: "Cessna 172".to_string(),
            author: "Laminar Research".to_string(),
        };
        
        let aircraft2 = DetectedAircraft {
            icao: "A320".to_string(),
            title: "Airbus A320".to_string(),
            author: "FlightFactor".to_string(),
        };
        
        // No previous aircraft
        assert!(detector.has_aircraft_changed(&aircraft1, &None));
        
        // Same aircraft
        assert!(!detector.has_aircraft_changed(&aircraft1, &Some(aircraft1.clone())));
        
        // Different aircraft
        assert!(detector.has_aircraft_changed(&aircraft2, &Some(aircraft1)));
    }

    #[test]
    fn test_aircraft_capabilities() {
        let detector = AircraftDetector::new();
        
        let ga_caps = detector.get_aircraft_capabilities(AircraftType::GeneralAviation);
        assert!(ga_caps.contains(&"basic_flight_controls".to_string()));
        assert!(ga_caps.contains(&"engine_management".to_string()));
        
        let airliner_caps = detector.get_aircraft_capabilities(AircraftType::Airliner);
        assert!(airliner_caps.contains(&"autopilot".to_string()));
        assert!(airliner_caps.contains(&"multiple_engines".to_string()));
        
        let helo_caps = detector.get_aircraft_capabilities(AircraftType::Helicopter);
        assert!(helo_caps.contains(&"collective".to_string()));
        assert!(helo_caps.contains(&"anti_torque".to_string()));
    }

    #[test]
    fn test_value_extraction() {
        let detector = AircraftDetector::new();
        let mut data = HashMap::new();
        
        // Test string extraction from float array (simulating X-Plane string DataRef)
        data.insert(
            "test_string".to_string(),
            DataRefValue::FloatArray(vec![67.0, 49.0, 55.0, 50.0, 0.0]), // "C172"
        );
        
        let result = detector.extract_string_value(&data, "test_string");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "C172");
        
        // Test integer extraction
        data.insert("test_int".to_string(), DataRefValue::Int(42));
        let result = detector.extract_int_value(&data, "test_int");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        
        // Test float extraction
        data.insert("test_float".to_string(), DataRefValue::Float(3.14));
        let result = detector.extract_float_value(&data, "test_float");
        assert!(result.is_ok());
        assert!((result.unwrap() - 3.14).abs() < 0.001);
    }
}