//! DCS World adapter implementation
//!
//! Main adapter that coordinates socket bridge, MP detection, and telemetry publishing.
//! Enforces MP integrity contract and provides clear user messaging.

use crate::mp_detection::{MpDetector, MpDetectionError, SessionType};
use crate::socket_bridge::{DcsMessage, ProtocolVersion, SocketBridge, SocketBridgeConfig};
use anyhow::Result;
use flight_bus::{
    snapshot::*, types::*, BusPublisher, BusSnapshot, PublisherError,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

/// DCS adapter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DcsAdapterConfig {
    /// Socket bridge configuration
    pub socket_config: SocketBridgeConfig,
    /// Bus publisher max rate
    pub bus_max_rate_hz: f32,
    /// Telemetry update rate (Hz)
    pub update_rate: f32,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Enable MP integrity enforcement
    pub enforce_mp_integrity: bool,
}

impl Default for DcsAdapterConfig {
    fn default() -> Self {
        Self {
            socket_config: SocketBridgeConfig::default(),
            bus_max_rate_hz: 60.0,
            update_rate: 30.0, // 30Hz
            connection_timeout: Duration::from_secs(10),
            enforce_mp_integrity: true,
        }
    }
}

/// DCS adapter errors
#[derive(Error, Debug)]
pub enum DcsAdapterError {
    #[error("Socket bridge error: {0}")]
    SocketBridge(#[from] anyhow::Error),
    #[error("Bus publisher error: {0}")]
    BusPublisher(#[from] PublisherError),
    #[error("MP detection error: {0}")]
    MpDetection(#[from] MpDetectionError),
    #[error("Telemetry parsing error: {field}")]
    TelemetryParsing { field: String },
    #[error("No DCS connection available")]
    NoConnection,
    #[error("Connection timeout")]
    ConnectionTimeout,
    #[error("Invalid aircraft data: {reason}")]
    InvalidAircraft { reason: String },
}

/// DCS connection state
#[derive(Debug, Clone)]
pub struct DcsConnection {
    pub addr: SocketAddr,
    pub version: ProtocolVersion,
    pub features: Vec<String>,
    pub last_telemetry: Instant,
    pub aircraft: Option<AircraftId>,
    pub session_type: SessionType,
}

/// DCS World adapter
pub struct DcsAdapter {
    config: DcsAdapterConfig,
    socket_bridge: SocketBridge,
    bus_publisher: BusPublisher,
    mp_detector: MpDetector,
    active_connection: Option<DcsConnection>,
    last_publish: Instant,
    blocked_features_notified: HashMap<String, Instant>,
}

impl DcsAdapter {
    /// Create new DCS adapter
    pub fn new(config: DcsAdapterConfig) -> Self {
        let socket_bridge = SocketBridge::new(config.socket_config.clone());
        let bus_publisher = BusPublisher::new(config.bus_max_rate_hz);
        let mp_detector = MpDetector::new();

        Self {
            config,
            socket_bridge,
            bus_publisher,
            mp_detector,
            active_connection: None,
            last_publish: Instant::now(),
            blocked_features_notified: HashMap::new(),
        }
    }

    /// Start the DCS adapter
    pub async fn start(&mut self) -> Result<(), DcsAdapterError> {
        info!("Starting DCS adapter");
        
        self.socket_bridge.start().await?;
        
        info!("DCS adapter started, waiting for connections");
        Ok(())
    }

    /// Main adapter loop
    pub async fn run(&mut self) -> Result<(), DcsAdapterError> {
        let mut update_interval = interval(Duration::from_secs_f32(1.0 / self.config.update_rate));

        loop {
            tokio::select! {
                _ = update_interval.tick() => {
                    self.update().await?;
                }
            }
        }
    }

    /// Update adapter state
    async fn update(&mut self) -> Result<(), DcsAdapterError> {
        // Accept new connections
        if let Some(addr) = self.socket_bridge.accept_connection().await? {
            info!("New DCS connection from {}", addr);
        }

        // Process messages
        let messages = self.socket_bridge.process_messages().await?;
        for (addr, message) in messages {
            self.handle_message(addr, message).await?;
        }

        // Maintain connections
        self.socket_bridge.maintain_connections().await?;

        // Check connection health
        self.check_connection_health().await?;

        Ok(())
    }

    /// Handle message from DCS
    async fn handle_message(&mut self, addr: SocketAddr, message: DcsMessage) -> Result<(), DcsAdapterError> {
        match message {
            DcsMessage::Handshake { .. } => {
                self.handle_handshake(addr, message).await?;
            }
            DcsMessage::Telemetry { .. } => {
                self.handle_telemetry(addr, message).await?;
            }
            DcsMessage::Heartbeat { .. } => {
                self.handle_heartbeat(addr).await?;
            }
            DcsMessage::Error { code, message } => {
                warn!("DCS error from {}: {} - {}", addr, code, message);
            }
            _ => {
                debug!("Unhandled message from {}: {:?}", addr, message);
            }
        }
        Ok(())
    }

    /// Handle handshake from DCS
    async fn handle_handshake(&mut self, addr: SocketAddr, message: DcsMessage) -> Result<(), DcsAdapterError> {
        self.socket_bridge.handshake(addr, message).await?;
        
        if let Some((version, features)) = self.socket_bridge.get_connection_info(addr) {
            let connection = DcsConnection {
                addr,
                version,
                features,
                last_telemetry: Instant::now(),
                aircraft: None,
                session_type: SessionType::Unknown,
            };
            
            self.active_connection = Some(connection);
            info!("DCS handshake completed with {} (version {})", addr, version);
        }

        Ok(())
    }

    /// Handle telemetry from DCS
    async fn handle_telemetry(&mut self, addr: SocketAddr, message: DcsMessage) -> Result<(), DcsAdapterError> {
        let (timestamp, aircraft_name, session_type_str, data) = match message {
            DcsMessage::Telemetry { timestamp, aircraft, session_type, data } => {
                (timestamp, aircraft, session_type, data)
            }
            _ => return Ok(()),
        };

        // Update MP detector
        let mut session_data = data.clone();
        session_data.insert("session_type".to_string(), serde_json::Value::String(session_type_str));
        self.mp_detector.update_session(&serde_json::Value::Object(
            session_data.into_iter().collect()
        ))?;

        // Update connection state
        if let Some(connection) = &mut self.active_connection {
            if connection.addr == addr {
                connection.last_telemetry = Instant::now();
                connection.aircraft = Some(AircraftId::new(aircraft_name.clone()));
                connection.session_type = self.mp_detector.current_session()
                    .map(|s| s.session_type)
                    .unwrap_or(SessionType::Unknown);
            }
        }

        // Check feature restrictions
        if self.config.enforce_mp_integrity {
            self.check_feature_restrictions(&data).await?;
        }

        // Convert to bus snapshot and publish
        let snapshot = self.convert_to_bus_snapshot(timestamp, &aircraft_name, &data)?;
        self.publish_snapshot(snapshot).await?;

        Ok(())
    }

    /// Handle heartbeat from DCS
    async fn handle_heartbeat(&mut self, addr: SocketAddr) -> Result<(), DcsAdapterError> {
        if let Some(connection) = &mut self.active_connection {
            if connection.addr == addr {
                connection.last_telemetry = Instant::now();
            }
        }
        Ok(())
    }

    /// Check feature restrictions for MP integrity
    async fn check_feature_restrictions(&mut self, data: &HashMap<String, serde_json::Value>) -> Result<(), DcsAdapterError> {
        // Check for restricted data in MP sessions
        let restricted_fields = ["weapons", "countermeasures", "rwr_contacts"];
        
        for field in &restricted_fields {
            if data.contains_key(*field) {
                if let Err(e) = self.mp_detector.validate_feature(&format!("telemetry_{}", field)) {
                    // Log blocked feature (rate limited)
                    let now = Instant::now();
                    let last_notified = self.blocked_features_notified.get(*field).copied()
                        .unwrap_or(Instant::now() - Duration::from_secs(60));
                    
                    if now.duration_since(last_notified) > Duration::from_secs(30) {
                        warn!("Blocked restricted feature '{}' in MP session: {}", field, e);
                        self.blocked_features_notified.insert(field.to_string(), now);
                    }
                }
            }
        }

        Ok(())
    }

    /// Convert DCS telemetry to bus snapshot
    fn convert_to_bus_snapshot(
        &self,
        timestamp: u64,
        aircraft_name: &str,
        data: &HashMap<String, serde_json::Value>,
    ) -> Result<BusSnapshot, DcsAdapterError> {
        let aircraft = AircraftId::new(aircraft_name);
        let mut snapshot = BusSnapshot::new(SimId::Dcs, aircraft);
        
        // Override timestamp from DCS
        snapshot.timestamp = timestamp * 1_000_000; // Convert ms to ns

        // Parse kinematics
        if let Some(ias) = data.get("ias").and_then(|v| v.as_f64()) {
            snapshot.kinematics.ias = ValidatedSpeed::new_knots(ias as f32)
                .map_err(|_| DcsAdapterError::TelemetryParsing { field: "ias".to_string() })?;
        }

        if let Some(tas) = data.get("tas").and_then(|v| v.as_f64()) {
            snapshot.kinematics.tas = ValidatedSpeed::new_knots(tas as f32)
                .map_err(|_| DcsAdapterError::TelemetryParsing { field: "tas".to_string() })?;
        }

        if let Some(altitude) = data.get("altitude_asl").and_then(|v| v.as_f64()) {
            snapshot.environment.altitude = altitude as f32;
        }

        if let Some(heading) = data.get("heading").and_then(|v| v.as_f64()) {
            snapshot.kinematics.heading = ValidatedAngle::new_degrees(heading as f32)
                .map_err(|_| DcsAdapterError::TelemetryParsing { field: "heading".to_string() })?;
        }

        if let Some(pitch) = data.get("pitch").and_then(|v| v.as_f64()) {
            snapshot.kinematics.pitch = ValidatedAngle::new_degrees(pitch as f32)
                .map_err(|_| DcsAdapterError::TelemetryParsing { field: "pitch".to_string() })?;
        }

        if let Some(bank) = data.get("bank").and_then(|v| v.as_f64()) {
            snapshot.kinematics.bank = ValidatedAngle::new_degrees(bank as f32)
                .map_err(|_| DcsAdapterError::TelemetryParsing { field: "bank".to_string() })?;
        }

        if let Some(vs) = data.get("vertical_speed").and_then(|v| v.as_f64()) {
            snapshot.kinematics.vertical_speed = vs as f32;
        }

        // Parse G-forces
        if let Some(g_force) = data.get("g_force").and_then(|v| v.as_f64()) {
            snapshot.kinematics.g_force = GForce::new(g_force as f32)
                .map_err(|_| DcsAdapterError::TelemetryParsing { field: "g_force".to_string() })?;
        }

        if let Some(g_lateral) = data.get("g_lateral").and_then(|v| v.as_f64()) {
            snapshot.kinematics.g_lateral = GForce::new(g_lateral as f32)
                .map_err(|_| DcsAdapterError::TelemetryParsing { field: "g_lateral".to_string() })?;
        }

        if let Some(g_longitudinal) = data.get("g_longitudinal").and_then(|v| v.as_f64()) {
            snapshot.kinematics.g_longitudinal = GForce::new(g_longitudinal as f32)
                .map_err(|_| DcsAdapterError::TelemetryParsing { field: "g_longitudinal".to_string() })?;
        }

        // Parse position
        if let Some(lat) = data.get("latitude").and_then(|v| v.as_f64()) {
            snapshot.navigation.latitude = lat;
        }

        if let Some(lon) = data.get("longitude").and_then(|v| v.as_f64()) {
            snapshot.navigation.longitude = lon;
        }

        // Parse engines (if available and allowed)
        if let Some(engines_data) = data.get("engines").and_then(|v| v.as_object()) {
            for (idx_str, engine_data) in engines_data {
                if let Ok(index) = idx_str.parse::<u8>() {
                    let engine = EngineData {
                        index,
                        running: true, // Assume running if data present
                        rpm: engine_data.get("rpm")
                            .and_then(|v| v.as_f64())
                            .and_then(|rpm| Percentage::new(rpm as f32).ok())
                            .unwrap_or_else(|| Percentage::new(0.0).unwrap()),
                        manifold_pressure: None,
                        egt: engine_data.get("temperature")
                            .and_then(|v| v.as_f64())
                            .map(|t| t as f32),
                        cht: None,
                        fuel_flow: engine_data.get("fuel_flow")
                            .and_then(|v| v.as_f64())
                            .map(|f| f as f32),
                        oil_pressure: None,
                        oil_temperature: None,
                    };
                    snapshot.engines.push(engine);
                }
            }
        }

        Ok(snapshot)
    }

    /// Publish snapshot to bus
    async fn publish_snapshot(&mut self, snapshot: BusSnapshot) -> Result<(), DcsAdapterError> {
        // Rate limit publishing
        let now = Instant::now();
        let min_interval = Duration::from_secs_f32(1.0 / self.config.update_rate);
        
        if now.duration_since(self.last_publish) < min_interval {
            return Ok(());
        }

        // Validate snapshot
        snapshot.validate().map_err(|e| DcsAdapterError::TelemetryParsing { 
            field: format!("snapshot validation: {}", e) 
        })?;

        // Publish to bus
        self.bus_publisher.publish(snapshot)?;
        self.last_publish = now;

        debug!("Published DCS telemetry to bus");
        Ok(())
    }

    /// Check connection health
    async fn check_connection_health(&mut self) -> Result<(), DcsAdapterError> {
        if let Some(connection) = &self.active_connection {
            let now = Instant::now();
            let timeout = self.config.connection_timeout;
            
            if now.duration_since(connection.last_telemetry) > timeout {
                warn!("DCS connection {} timed out", connection.addr);
                self.active_connection = None;
            }
        }
        Ok(())
    }

    /// Get current connection status
    pub fn connection_status(&self) -> Option<&DcsConnection> {
        self.active_connection.as_ref()
    }

    /// Get MP session info
    pub fn mp_session_info(&self) -> Option<String> {
        self.mp_detector.mp_banner_message()
    }

    /// Get blocked features for current session
    pub fn blocked_features(&self) -> Vec<String> {
        self.mp_detector.blocked_features()
    }

    /// Check if feature is blocked with user message
    pub fn check_feature_blocked(&self, feature: &str) -> Option<String> {
        self.mp_detector.blocked_feature_message(feature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create_test_adapter() -> DcsAdapter {
        let config = DcsAdapterConfig::default();
        DcsAdapter::new(config)
    }

    #[test]
    fn test_adapter_creation() {
        let adapter = create_test_adapter();
        assert!(adapter.active_connection.is_none());
        assert_eq!(adapter.blocked_features().len(), 0);
    }

    #[test]
    fn test_telemetry_conversion() {
        let adapter = create_test_adapter();
        
        let data = json!({
            "ias": 150.0,
            "tas": 155.0,
            "altitude_asl": 5000.0,
            "heading": 90.0,
            "pitch": 5.0,
            "bank": -10.0,
            "g_force": 1.2,
            "latitude": 45.0,
            "longitude": -122.0
        }).as_object().unwrap().clone();

        let snapshot = adapter.convert_to_bus_snapshot(
            1000,
            "F-16C",
            &data.into_iter().collect()
        ).unwrap();

        assert_eq!(snapshot.sim, SimId::Dcs);
        assert_eq!(snapshot.aircraft.icao, "F-16C");
        assert_eq!(snapshot.kinematics.ias.value(), 150.0);
        assert_eq!(snapshot.kinematics.heading.value(), 90.0);
        assert_eq!(snapshot.navigation.latitude, 45.0);
    }

    #[test]
    fn test_feature_restriction_checking() {
        let mut adapter = create_test_adapter();
        
        // Set up MP session
        let session_data = json!({
            "session_type": "MP",
            "server_name": "Test Server"
        });
        
        adapter.mp_detector.update_session(&session_data).unwrap();
        
        // Check blocked feature message
        let message = adapter.check_feature_blocked("telemetry_weapons");
        assert!(message.is_some());
        assert!(message.unwrap().contains("multiplayer integrity"));
        
        // Check allowed feature
        let message = adapter.check_feature_blocked("telemetry_basic");
        assert!(message.is_none());
    }
}