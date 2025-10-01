//! Multiplayer session detection and feature blocking
//!
//! Implements MP session detection to enforce integrity contract.
//! Blocks restricted features in MP sessions with clear UI messaging.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

/// Session type detection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionType {
    /// Single player session - all features available
    SinglePlayer,
    /// Multiplayer session - restricted features blocked
    Multiplayer,
    /// Unknown session type - assume MP restrictions
    Unknown,
}

impl SessionType {
    /// Check if feature is allowed in this session type
    pub fn allows_feature(&self, feature: &str) -> bool {
        match self {
            SessionType::SinglePlayer => true,
            SessionType::Multiplayer | SessionType::Unknown => {
                !MP_BLOCKED_FEATURES.contains(&feature)
            }
        }
    }

    /// Get blocked features for this session type
    pub fn blocked_features(&self) -> Vec<&'static str> {
        match self {
            SessionType::SinglePlayer => Vec::new(),
            SessionType::Multiplayer | SessionType::Unknown => {
                MP_BLOCKED_FEATURES.iter().copied().collect()
            }
        }
    }
}

impl std::fmt::Display for SessionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionType::SinglePlayer => write!(f, "Single Player"),
            SessionType::Multiplayer => write!(f, "Multiplayer"),
            SessionType::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Features that are blocked in multiplayer sessions
const MP_BLOCKED_FEATURES: &[&str] = &[
    "telemetry_weapons",
    "telemetry_countermeasures", 
    "telemetry_rwr",
    "telemetry_datalink",
    "control_weapons",
    "control_countermeasures",
    "control_sensors",
];

/// MP session information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MpSession {
    /// Session type
    pub session_type: SessionType,
    /// Server name (if MP)
    pub server_name: Option<String>,
    /// Player count (if MP)
    pub player_count: Option<u32>,
    /// Mission name
    pub mission_name: Option<String>,
    /// Detected timestamp
    pub detected_at: u64,
}

/// MP detection errors
#[derive(Error, Debug)]
pub enum MpDetectionError {
    #[error("Feature '{feature}' is blocked in {session_type} sessions")]
    FeatureBlocked {
        feature: String,
        session_type: SessionType,
    },
    #[error("Session type detection failed: {reason}")]
    DetectionFailed { reason: String },
    #[error("Invalid session data: {reason}")]
    InvalidData { reason: String },
}

/// MP session detector
pub struct MpDetector {
    current_session: Option<MpSession>,
    blocked_features: HashSet<String>,
}

impl MpDetector {
    /// Create new MP detector
    pub fn new() -> Self {
        Self {
            current_session: None,
            blocked_features: HashSet::new(),
        }
    }

    /// Update session information from DCS telemetry
    pub fn update_session(&mut self, session_data: &serde_json::Value) -> Result<(), MpDetectionError> {
        let session_type = self.detect_session_type(session_data)?;
        
        let session = MpSession {
            session_type,
            server_name: session_data.get("server_name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            player_count: session_data.get("player_count")
                .and_then(|v| v.as_u64())
                .map(|n| n as u32),
            mission_name: session_data.get("mission_name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            detected_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        // Update blocked features based on session type
        self.blocked_features.clear();
        for feature in session_type.blocked_features() {
            self.blocked_features.insert(feature.to_string());
        }

        self.current_session = Some(session);
        Ok(())
    }

    /// Detect session type from telemetry data
    fn detect_session_type(&self, data: &serde_json::Value) -> Result<SessionType, MpDetectionError> {
        // Check for explicit session type indicator
        if let Some(session_type_str) = data.get("session_type").and_then(|v| v.as_str()) {
            return match session_type_str.to_uppercase().as_str() {
                "SP" | "SINGLE" | "SINGLEPLAYER" => Ok(SessionType::SinglePlayer),
                "MP" | "MULTI" | "MULTIPLAYER" => Ok(SessionType::Multiplayer),
                _ => Ok(SessionType::Unknown),
            };
        }

        // Fallback detection methods
        
        // Check for server name (indicates MP)
        if data.get("server_name").and_then(|v| v.as_str()).is_some() {
            return Ok(SessionType::Multiplayer);
        }

        // Check for player count > 1
        if let Some(player_count) = data.get("player_count").and_then(|v| v.as_u64()) {
            return if player_count > 1 {
                Ok(SessionType::Multiplayer)
            } else {
                Ok(SessionType::SinglePlayer)
            };
        }

        // Check for multiplayer-specific fields
        let mp_indicators = ["coalition", "side", "group_id", "unit_id"];
        for indicator in &mp_indicators {
            if data.get(indicator).is_some() {
                return Ok(SessionType::Multiplayer);
            }
        }

        // Default to unknown (apply MP restrictions)
        Ok(SessionType::Unknown)
    }

    /// Check if feature is allowed in current session
    pub fn is_feature_allowed(&self, feature: &str) -> bool {
        match &self.current_session {
            Some(session) => session.session_type.allows_feature(feature),
            None => false, // No session detected, block everything
        }
    }

    /// Validate feature access
    pub fn validate_feature(&self, feature: &str) -> Result<(), MpDetectionError> {
        if !self.is_feature_allowed(feature) {
            let session_type = self.current_session
                .as_ref()
                .map(|s| s.session_type)
                .unwrap_or(SessionType::Unknown);
            
            return Err(MpDetectionError::FeatureBlocked {
                feature: feature.to_string(),
                session_type,
            });
        }
        Ok(())
    }

    /// Get current session info
    pub fn current_session(&self) -> Option<&MpSession> {
        self.current_session.as_ref()
    }

    /// Get blocked features for current session
    pub fn blocked_features(&self) -> Vec<String> {
        self.blocked_features.iter().cloned().collect()
    }

    /// Generate user-friendly message for blocked feature
    pub fn blocked_feature_message(&self, feature: &str) -> Option<String> {
        if !self.is_feature_allowed(feature) {
            let session_type = self.current_session
                .as_ref()
                .map(|s| s.session_type)
                .unwrap_or(SessionType::Unknown);

            Some(format!(
                "Feature '{}' is not available in {} sessions for DCS multiplayer integrity. \
                This feature is only available in single-player missions.",
                feature, session_type
            ))
        } else {
            None
        }
    }

    /// Check if currently in MP session
    pub fn is_multiplayer(&self) -> bool {
        matches!(
            self.current_session.as_ref().map(|s| s.session_type),
            Some(SessionType::Multiplayer)
        )
    }

    /// Get MP session banner message
    pub fn mp_banner_message(&self) -> Option<String> {
        if self.is_multiplayer() {
            let server_name = self.current_session
                .as_ref()
                .and_then(|s| s.server_name.as_ref())
                .map(|n| format!(" on {}", n))
                .unwrap_or_default();

            Some(format!(
                "DCS Multiplayer Session{} - Some features are restricted for server integrity",
                server_name
            ))
        } else {
            None
        }
    }
}

impl Default for MpDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_session_type_feature_blocking() {
        assert!(SessionType::SinglePlayer.allows_feature("telemetry_weapons"));
        assert!(!SessionType::Multiplayer.allows_feature("telemetry_weapons"));
        assert!(!SessionType::Unknown.allows_feature("telemetry_weapons"));
        
        assert!(SessionType::Multiplayer.allows_feature("telemetry_basic"));
        assert!(SessionType::Unknown.allows_feature("telemetry_basic"));
    }

    #[test]
    fn test_mp_detector_explicit_session_type() {
        let mut detector = MpDetector::new();
        
        let sp_data = json!({
            "session_type": "SP",
            "mission_name": "Test Mission"
        });
        
        detector.update_session(&sp_data).unwrap();
        assert!(detector.is_feature_allowed("telemetry_weapons"));
        assert!(!detector.is_multiplayer());
        
        let mp_data = json!({
            "session_type": "MP",
            "server_name": "Test Server",
            "player_count": 5
        });
        
        detector.update_session(&mp_data).unwrap();
        assert!(!detector.is_feature_allowed("telemetry_weapons"));
        assert!(detector.is_multiplayer());
    }

    #[test]
    fn test_mp_detector_fallback_detection() {
        let mut detector = MpDetector::new();
        
        // Server name indicates MP
        let mp_data = json!({
            "server_name": "Test Server"
        });
        
        detector.update_session(&mp_data).unwrap();
        assert!(!detector.is_feature_allowed("telemetry_weapons"));
        assert!(detector.is_multiplayer());
        
        // Player count > 1 indicates MP
        let mp_data2 = json!({
            "player_count": 3
        });
        
        detector.update_session(&mp_data2).unwrap();
        assert!(!detector.is_feature_allowed("telemetry_weapons"));
        
        // Player count = 1 indicates SP
        let sp_data = json!({
            "player_count": 1
        });
        
        detector.update_session(&sp_data).unwrap();
        assert!(detector.is_feature_allowed("telemetry_weapons"));
        assert!(!detector.is_multiplayer());
    }

    #[test]
    fn test_feature_validation() {
        let mut detector = MpDetector::new();
        
        let mp_data = json!({
            "session_type": "MP"
        });
        
        detector.update_session(&mp_data).unwrap();
        
        // Allowed feature should pass
        assert!(detector.validate_feature("telemetry_basic").is_ok());
        
        // Blocked feature should fail
        let result = detector.validate_feature("telemetry_weapons");
        assert!(result.is_err());
        
        match result.unwrap_err() {
            MpDetectionError::FeatureBlocked { feature, session_type } => {
                assert_eq!(feature, "telemetry_weapons");
                assert_eq!(session_type, SessionType::Multiplayer);
            }
            _ => panic!("Wrong error type"),
        }
    }

    #[test]
    fn test_blocked_feature_message() {
        let mut detector = MpDetector::new();
        
        let mp_data = json!({
            "session_type": "MP",
            "server_name": "Test Server"
        });
        
        detector.update_session(&mp_data).unwrap();
        
        let message = detector.blocked_feature_message("telemetry_weapons");
        assert!(message.is_some());
        assert!(message.unwrap().contains("multiplayer integrity"));
        
        let message = detector.blocked_feature_message("telemetry_basic");
        assert!(message.is_none());
    }

    #[test]
    fn test_mp_banner_message() {
        let mut detector = MpDetector::new();
        
        // SP session - no banner
        let sp_data = json!({
            "session_type": "SP"
        });
        
        detector.update_session(&sp_data).unwrap();
        assert!(detector.mp_banner_message().is_none());
        
        // MP session - show banner
        let mp_data = json!({
            "session_type": "MP",
            "server_name": "Test Server"
        });
        
        detector.update_session(&mp_data).unwrap();
        let banner = detector.mp_banner_message();
        assert!(banner.is_some());
        assert!(banner.unwrap().contains("Test Server"));
    }
}