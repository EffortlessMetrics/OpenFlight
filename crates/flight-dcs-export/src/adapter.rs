// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! DCS World adapter implementation
//!
//! Main adapter that coordinates socket bridge, MP detection, and telemetry publishing.
//! Enforces MP integrity contract and provides clear user messaging.

use crate::aircraft_db;
use crate::mp_detection::{MpDetectionError, MpDetector, SessionType};
use crate::protocol::DcsTelemetryPacket;
use crate::socket_bridge::{DcsMessage, ProtocolVersion, SocketBridge, SocketBridgeConfig};
use anyhow::Result;
use flight_adapter_common::{AdapterConfig, AdapterError, AdapterMetrics, AdapterState};
use flight_bus::{BusPublisher, BusSnapshot, PublisherError, snapshot::*, types::*};
use flight_metrics::{
    MetricsRegistry,
    common::{
        ADAPTER_ERRORS_TOTAL, ADAPTER_TIME_SINCE_LAST_PACKET_MS, ADAPTER_UPDATE_LATENCY_MS,
        ADAPTER_UPDATES_TOTAL,
    },
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::time::interval;
use tracing::{debug, info, warn};

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
            update_rate: 30.0,                          // 30Hz
            connection_timeout: Duration::from_secs(2), // 2 second timeout per requirements
            enforce_mp_integrity: true,
        }
    }
}

impl AdapterConfig for DcsAdapterConfig {
    fn publish_rate_hz(&self) -> f32 {
        self.update_rate
    }

    fn connection_timeout(&self) -> Duration {
        self.connection_timeout
    }

    fn max_reconnect_attempts(&self) -> u32 {
        0
    }

    fn enable_auto_reconnect(&self) -> bool {
        false
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
    #[error(transparent)]
    Adapter(#[from] AdapterError),
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
    pub(crate) mp_detector: MpDetector,
    state: AdapterState,
    metrics: AdapterMetrics,
    metrics_registry: MetricsRegistry,
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
            state: AdapterState::Disconnected,
            metrics: AdapterMetrics::new(),
            metrics_registry: MetricsRegistry::new(),
            active_connection: None,
            last_publish: Instant::now(),
            blocked_features_notified: HashMap::new(),
        }
    }

    /// Start the DCS adapter
    pub async fn start(&mut self) -> Result<(), DcsAdapterError> {
        info!("Starting DCS adapter");

        self.state = AdapterState::Connecting;
        if let Err(err) = self.socket_bridge.start().await {
            self.metrics_registry.inc_counter(ADAPTER_ERRORS_TOTAL, 1);
            self.state = AdapterState::Error;
            return Err(err.into());
        }

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
        match self.socket_bridge.accept_connection().await {
            Ok(Some(addr)) => {
                info!("New DCS connection from {}", addr);
            }
            Ok(None) => {}
            Err(err) => {
                self.metrics_registry.inc_counter(ADAPTER_ERRORS_TOTAL, 1);
                return Err(err.into());
            }
        }

        // Process messages
        let messages = self
            .socket_bridge
            .process_messages()
            .await
            .inspect_err(|_| {
                self.metrics_registry.inc_counter(ADAPTER_ERRORS_TOTAL, 1);
            })?;
        for (addr, message) in messages {
            if let Err(err) = self.handle_message(addr, message).await {
                self.metrics_registry.inc_counter(ADAPTER_ERRORS_TOTAL, 1);
                return Err(err);
            }
        }

        // Maintain connections
        self.socket_bridge
            .maintain_connections()
            .await
            .inspect_err(|_| {
                self.metrics_registry.inc_counter(ADAPTER_ERRORS_TOTAL, 1);
            })?;

        // Check connection health
        self.check_connection_health().await?;

        Ok(())
    }

    /// Handle message from DCS
    async fn handle_message(
        &mut self,
        addr: SocketAddr,
        message: DcsMessage,
    ) -> Result<(), DcsAdapterError> {
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
                self.metrics_registry.inc_counter(ADAPTER_ERRORS_TOTAL, 1);
            }
            _ => {
                debug!("Unhandled message from {}: {:?}", addr, message);
            }
        }
        Ok(())
    }

    /// Handle handshake from DCS
    async fn handle_handshake(
        &mut self,
        addr: SocketAddr,
        message: DcsMessage,
    ) -> Result<(), DcsAdapterError> {
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
            self.state = AdapterState::Connected;
            info!(
                "DCS handshake completed with {} (version {})",
                addr, version
            );
        }

        Ok(())
    }

    /// Handle telemetry from DCS
    async fn handle_telemetry(
        &mut self,
        addr: SocketAddr,
        message: DcsMessage,
    ) -> Result<(), DcsAdapterError> {
        let (timestamp, aircraft_name, session_type_str, data) = match message {
            DcsMessage::Telemetry {
                timestamp,
                aircraft,
                session_type,
                data,
            } => (timestamp, aircraft, session_type, data),
            _ => return Ok(()),
        };
        let update_start = Instant::now();

        // Update MP detector
        let mut session_data = data.clone();
        session_data.insert(
            "session_type".to_string(),
            serde_json::Value::String(session_type_str),
        );
        self.mp_detector.update_session(&serde_json::Value::Object(
            session_data.into_iter().collect(),
        ))?;

        // Update connection state
        if let Some(connection) = &mut self.active_connection
            && connection.addr == addr
        {
            connection.last_telemetry = Instant::now();
            connection.aircraft = Some(AircraftId::new(aircraft_name.clone()));
            connection.session_type = self
                .mp_detector
                .current_session()
                .map(|s| s.session_type)
                .unwrap_or(SessionType::Unknown);
        }

        // Filter restricted fields for MP integrity enforcement
        let data = if self.config.enforce_mp_integrity {
            let (filtered, _) = self.filter_restricted_fields(data);
            filtered
        } else {
            data
        };

        // Convert to bus snapshot and publish
        let snapshot = self.convert_to_bus_snapshot(timestamp, &aircraft_name, &data)?;
        self.publish_snapshot(snapshot).await?;

        let update_latency = update_start.elapsed();
        self.metrics.record_update();
        self.metrics.record_aircraft_change(aircraft_name.clone());
        self.metrics_registry.inc_counter(ADAPTER_UPDATES_TOTAL, 1);
        self.metrics_registry.observe(
            ADAPTER_UPDATE_LATENCY_MS,
            update_latency.as_secs_f64() * 1000.0,
        );
        if let Some(since) = self.time_since_last_telemetry() {
            self.metrics_registry.set_gauge(
                ADAPTER_TIME_SINCE_LAST_PACKET_MS,
                since.as_secs_f64() * 1000.0,
            );
        }
        self.state = AdapterState::Active;

        Ok(())
    }

    /// Handle heartbeat from DCS
    async fn handle_heartbeat(&mut self, addr: SocketAddr) -> Result<(), DcsAdapterError> {
        if let Some(connection) = &mut self.active_connection
            && connection.addr == addr
        {
            connection.last_telemetry = Instant::now();
            if !matches!(self.state, AdapterState::Active) {
                self.state = AdapterState::Connected;
            }
        }
        Ok(())
    }

    /// Filter restricted telemetry fields for MP integrity enforcement.
    ///
    /// Removes any fields that are blocked in the current session and emits a
    /// rate-limited user-friendly warning via `warn!` for each blocked field.
    /// Returns the filtered data map and the names of removed fields.
    pub fn filter_restricted_fields(
        &mut self,
        mut data: HashMap<String, serde_json::Value>,
    ) -> (HashMap<String, serde_json::Value>, Vec<String>) {
        // Maps data field key → MP feature name (must match MP_BLOCKED_FEATURES)
        const RESTRICTED: &[(&str, &str)] = &[
            ("weapons", "telemetry_weapons"),
            ("countermeasures", "telemetry_countermeasures"),
            ("rwr_contacts", "telemetry_rwr"),
        ];
        let mut blocked = Vec::new();

        for &(field, feature) in RESTRICTED {
            if data.contains_key(field) && self.mp_detector.validate_feature(feature).is_err() {
                // Emit user-friendly warning (rate-limited to once per 30 s per field)
                let now = Instant::now();
                let last = self
                    .blocked_features_notified
                    .get(field)
                    .copied()
                    .unwrap_or_else(|| now - Duration::from_secs(60));
                if now.duration_since(last) > Duration::from_secs(30) {
                    if let Some(msg) = self.mp_detector.blocked_feature_message(feature) {
                        warn!("[MP Integrity] {}", msg);
                    }
                    self.blocked_features_notified
                        .insert(field.to_string(), now);
                }
                data.remove(field);
                blocked.push(field.to_string());
            }
        }

        (data, blocked)
    }

    /// Convert DCS telemetry to bus snapshot
    pub fn convert_to_bus_snapshot(
        &self,
        _timestamp_ms: u64,
        aircraft_name: &str,
        data: &HashMap<String, serde_json::Value>,
    ) -> Result<BusSnapshot, DcsAdapterError> {
        let aircraft = AircraftId::new(aircraft_name);
        let mut snapshot = BusSnapshot::new(SimId::Dcs, aircraft);

        // BusSnapshot timestamp is monotonic since process start
        // Using Instant to approximate process-relative monotonic time
        static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
        let start = START.get_or_init(Instant::now);
        snapshot.timestamp = Instant::now().duration_since(*start).as_nanos() as u64;

        // Parse kinematics
        if let Some(ias) = data.get("ias").and_then(|v| v.as_f64()) {
            snapshot.kinematics.ias = ValidatedSpeed::new_knots(ias as f32).map_err(|_| {
                DcsAdapterError::TelemetryParsing {
                    field: "ias".to_string(),
                }
            })?;
        }

        if let Some(tas) = data.get("tas").and_then(|v| v.as_f64()) {
            snapshot.kinematics.tas = ValidatedSpeed::new_knots(tas as f32).map_err(|_| {
                DcsAdapterError::TelemetryParsing {
                    field: "tas".to_string(),
                }
            })?;
        }

        if let Some(altitude) = data.get("altitude_asl").and_then(|v| v.as_f64()) {
            snapshot.environment.altitude = altitude as f32;
        }

        if let Some(heading) = data.get("heading").and_then(|v| v.as_f64()) {
            snapshot.kinematics.heading =
                ValidatedAngle::new_degrees(heading as f32).map_err(|_| {
                    DcsAdapterError::TelemetryParsing {
                        field: "heading".to_string(),
                    }
                })?;
        }

        if let Some(pitch) = data.get("pitch").and_then(|v| v.as_f64()) {
            snapshot.kinematics.pitch =
                ValidatedAngle::new_degrees(pitch as f32).map_err(|_| {
                    DcsAdapterError::TelemetryParsing {
                        field: "pitch".to_string(),
                    }
                })?;
        }

        if let Some(bank) = data.get("bank").and_then(|v| v.as_f64()) {
            snapshot.kinematics.bank = ValidatedAngle::new_degrees(bank as f32).map_err(|_| {
                DcsAdapterError::TelemetryParsing {
                    field: "bank".to_string(),
                }
            })?;
        }

        if let Some(vs) = data.get("vertical_speed").and_then(|v| v.as_f64()) {
            snapshot.kinematics.vertical_speed = vs as f32;
        }

        // Parse G-forces
        if let Some(g_force) = data.get("g_force").and_then(|v| v.as_f64()) {
            snapshot.kinematics.g_force =
                GForce::new(g_force as f32).map_err(|_| DcsAdapterError::TelemetryParsing {
                    field: "g_force".to_string(),
                })?;
        }

        if let Some(g_lateral) = data.get("g_lateral").and_then(|v| v.as_f64()) {
            snapshot.kinematics.g_lateral =
                GForce::new(g_lateral as f32).map_err(|_| DcsAdapterError::TelemetryParsing {
                    field: "g_lateral".to_string(),
                })?;
        }

        if let Some(g_longitudinal) = data.get("g_longitudinal").and_then(|v| v.as_f64()) {
            snapshot.kinematics.g_longitudinal =
                GForce::new(g_longitudinal as f32).map_err(|_| {
                    DcsAdapterError::TelemetryParsing {
                        field: "g_longitudinal".to_string(),
                    }
                })?;
        }

        // Parse position
        if let Some(lat) = data.get("latitude").and_then(|v| v.as_f64()) {
            snapshot.navigation.latitude = lat;
        }

        if let Some(lon) = data.get("longitude").and_then(|v| v.as_f64()) {
            snapshot.navigation.longitude = lon;
        }

        // Parse angle of attack
        if let Some(aoa) = data.get("aoa").and_then(|v| v.as_f64()) {
            snapshot.kinematics.aoa = ValidatedAngle::new_degrees(aoa as f32).map_err(|_| {
                DcsAdapterError::TelemetryParsing {
                    field: "aoa".to_string(),
                }
            })?;
        }

        // Parse angular rates (rad/s, body frame)
        if let Some(p) = data.get("angular_velocity_x").and_then(|v| v.as_f64()) {
            snapshot.angular_rates.p = p as f32;
        }
        if let Some(q) = data.get("angular_velocity_y").and_then(|v| v.as_f64()) {
            snapshot.angular_rates.q = q as f32;
        }
        if let Some(r) = data.get("angular_velocity_z").and_then(|v| v.as_f64()) {
            snapshot.angular_rates.r = r as f32;
        }

        // Parse navigation: ground track and distance to destination
        if let Some(course) = data.get("course").and_then(|v| v.as_f64()) {
            snapshot.navigation.ground_track =
                ValidatedAngle::new_degrees(course as f32).map_err(|_| {
                    DcsAdapterError::TelemetryParsing {
                        field: "course".to_string(),
                    }
                })?;
        }
        if let Some(dist) = data.get("waypoint_distance").and_then(|v| v.as_f64()) {
            snapshot.navigation.distance_to_dest = Some(dist as f32);
        }

        // Parse aircraft configuration (gear, flaps)
        if let Some(gear_down) = data.get("gear_down").and_then(|v| v.as_f64()) {
            let pos = if gear_down > 0.9 {
                GearPosition::Down
            } else if gear_down < 0.1 {
                GearPosition::Up
            } else {
                GearPosition::Transitioning
            };
            snapshot.config.gear = GearState {
                nose: pos,
                left: pos,
                right: pos,
            };
        }
        if let Some(flaps) = data.get("flaps").and_then(|v| v.as_f64()) {
            snapshot.config.flaps =
                Percentage::new(flaps.clamp(0.0, 100.0) as f32).map_err(|_| {
                    DcsAdapterError::TelemetryParsing {
                        field: "flaps".to_string(),
                    }
                })?;
        }

        // Parse engines (if available and allowed)
        if let Some(engines_data) = data.get("engines").and_then(|v| v.as_object()) {
            for (idx_str, engine_data) in engines_data {
                if let Ok(index) = idx_str.parse::<u8>() {
                    let engine = EngineData {
                        index,
                        running: true, // Assume running if data present
                        rpm: engine_data
                            .get("rpm")
                            .and_then(|v| v.as_f64())
                            .and_then(|rpm| Percentage::new(rpm as f32).ok())
                            .unwrap_or_else(|| Percentage::new(0.0).unwrap()),
                        manifold_pressure: None,
                        egt: engine_data
                            .get("temperature")
                            .and_then(|v| v.as_f64())
                            .map(|t| t as f32),
                        cht: None,
                        fuel_flow: engine_data
                            .get("fuel_flow")
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

    /// Convert a protocol-parsed [`DcsTelemetryPacket`] (from raw Export.lua UDP
    /// data) into a [`BusSnapshot`] suitable for bus publishing.
    ///
    /// Unit conversions applied:
    /// - `airspeed_ms` (m/s) → knots
    /// - `vertical_speed_ms` (m/s) → ft/min  
    /// - `altitude_m` (m) → stored as-is (adapter convention: pass-through)
    /// - angles: degrees pass-through
    pub fn convert_packet_to_bus_snapshot(
        &self,
        packet: &DcsTelemetryPacket,
    ) -> Result<BusSnapshot, DcsAdapterError> {
        const MS_TO_KNOTS: f64 = 1.943_844;

        let aircraft = self.detect_aircraft(&packet.aircraft_name);
        let mut snapshot = BusSnapshot::new(SimId::Dcs, aircraft);

        // Monotonic timestamp
        static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
        let start = START.get_or_init(Instant::now);
        snapshot.timestamp = Instant::now().duration_since(*start).as_nanos() as u64;

        let fd = &packet.flight_data;

        // Airspeed: m/s → knots
        let ias_knots = fd.airspeed_ms * MS_TO_KNOTS;
        snapshot.kinematics.ias =
            ValidatedSpeed::new_knots(ias_knots as f32).map_err(|_| {
                DcsAdapterError::TelemetryParsing {
                    field: "airspeed_ms".to_string(),
                }
            })?;

        // Heading, pitch, roll, AoA — already in degrees
        snapshot.kinematics.heading =
            ValidatedAngle::new_degrees(fd.heading_deg as f32).map_err(|_| {
                DcsAdapterError::TelemetryParsing {
                    field: "heading_deg".to_string(),
                }
            })?;
        snapshot.kinematics.pitch =
            ValidatedAngle::new_degrees(fd.pitch_deg as f32).map_err(|_| {
                DcsAdapterError::TelemetryParsing {
                    field: "pitch_deg".to_string(),
                }
            })?;
        snapshot.kinematics.bank =
            ValidatedAngle::new_degrees(fd.roll_deg as f32).map_err(|_| {
                DcsAdapterError::TelemetryParsing {
                    field: "roll_deg".to_string(),
                }
            })?;
        snapshot.kinematics.aoa =
            ValidatedAngle::new_degrees(fd.aoa_deg as f32).map_err(|_| {
                DcsAdapterError::TelemetryParsing {
                    field: "aoa_deg".to_string(),
                }
            })?;

        // Altitude (meters, pass-through)
        snapshot.environment.altitude = fd.altitude_m as f32;

        // Vertical speed (m/s, pass-through)
        snapshot.kinematics.vertical_speed = fd.vertical_speed_ms as f32;

        // G-load
        snapshot.kinematics.g_force =
            GForce::new(fd.g_load as f32).map_err(|_| DcsAdapterError::TelemetryParsing {
                field: "g_load".to_string(),
            })?;

        // Gear positions
        for (i, &pos) in fd.gear_position.iter().enumerate() {
            let gear_pos = if pos > 0.9 {
                GearPosition::Down
            } else if pos < 0.1 {
                GearPosition::Up
            } else {
                GearPosition::Transitioning
            };
            match i {
                0 => snapshot.config.gear.nose = gear_pos,
                1 => snapshot.config.gear.left = gear_pos,
                2 => snapshot.config.gear.right = gear_pos,
                _ => {}
            }
        }

        // Engine RPM percentages
        for (i, &rpm) in fd.engine_rpm_percent.iter().enumerate() {
            let engine = EngineData {
                index: i as u8,
                running: rpm > 0.0,
                rpm: Percentage::new(rpm.clamp(0.0, 100.0) as f32)
                    .unwrap_or_else(|_| Percentage::new(0.0).unwrap()),
                manifold_pressure: None,
                egt: None,
                cht: None,
                fuel_flow: None,
                oil_pressure: None,
                oil_temperature: None,
            };
            snapshot.engines.push(engine);
        }

        Ok(snapshot)
    }

    /// Detect aircraft using the DCS aircraft database for metadata enrichment.
    ///
    /// If the aircraft is found in the database, the display name is used as the
    /// variant; otherwise a plain [`AircraftId`] is created from the raw name.
    pub fn detect_aircraft(&self, dcs_name: &str) -> AircraftId {
        if let Some(info) = aircraft_db::lookup(dcs_name) {
            AircraftId::with_variant(dcs_name, info.display_name)
        } else if let Some(info) = aircraft_db::lookup_fuzzy(dcs_name) {
            AircraftId::with_variant(info.dcs_name, info.display_name)
        } else {
            AircraftId::new(dcs_name)
        }
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
        snapshot
            .validate()
            .map_err(|e| DcsAdapterError::TelemetryParsing {
                field: format!("snapshot validation: {}", e),
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
            let since = now.duration_since(connection.last_telemetry);

            self.metrics_registry.set_gauge(
                ADAPTER_TIME_SINCE_LAST_PACKET_MS,
                since.as_secs_f64() * 1000.0,
            );

            if since > timeout {
                warn!("DCS connection {} timed out", connection.addr);
                self.metrics_registry.inc_counter(ADAPTER_ERRORS_TOTAL, 1);
                self.active_connection = None;
                self.state = AdapterState::Disconnected;
            }
        } else if matches!(self.state, AdapterState::Active | AdapterState::Connected) {
            self.state = AdapterState::Disconnected;
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

    /// Get current adapter state
    pub fn state(&self) -> AdapterState {
        self.state
    }

    /// Get adapter metrics snapshot
    pub fn metrics(&self) -> AdapterMetrics {
        self.metrics.clone()
    }

    /// Get shared metrics registry
    pub fn metrics_registry(&self) -> &MetricsRegistry {
        &self.metrics_registry
    }

    /// Get connection timeout status (for metrics)
    pub fn is_connection_timeout(&self) -> bool {
        if let Some(connection) = &self.active_connection {
            let now = Instant::now();
            now.duration_since(connection.last_telemetry) > self.config.connection_timeout
        } else {
            false
        }
    }

    /// Get time since last telemetry (for metrics)
    pub fn time_since_last_telemetry(&self) -> Option<Duration> {
        self.active_connection
            .as_ref()
            .map(|conn| Instant::now().duration_since(conn.last_telemetry))
    }

    /// Check if currently in multiplayer session (for testing)
    pub fn is_multiplayer(&self) -> bool {
        self.mp_detector.is_multiplayer()
    }

    /// Update MP detector session (for testing)
    pub fn update_mp_session(
        &mut self,
        session_data: &serde_json::Value,
    ) -> Result<(), DcsAdapterError> {
        self.mp_detector.update_session(session_data)?;
        Ok(())
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
        })
        .as_object()
        .unwrap()
        .clone();

        let snapshot = adapter
            .convert_to_bus_snapshot(1000, "F-16C", &data.into_iter().collect())
            .unwrap();

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

    // --- State machine transition tests ---

    #[test]
    fn test_initial_state_is_disconnected() {
        let adapter = create_test_adapter();
        assert_eq!(adapter.state(), AdapterState::Disconnected);
        assert!(adapter.active_connection.is_none());
    }

    #[tokio::test]
    async fn test_state_transitions_disconnected_connected_active() {
        let mut adapter = create_test_adapter();
        let addr: std::net::SocketAddr = "127.0.0.1:19901".parse().unwrap();

        // Simulate Disconnected → Connected (mirrors handle_handshake)
        adapter.active_connection = Some(DcsConnection {
            addr,
            version: crate::socket_bridge::ProtocolVersion::V1_0,
            features: vec!["telemetry_basic".to_string()],
            last_telemetry: Instant::now(),
            aircraft: None,
            session_type: crate::mp_detection::SessionType::Unknown,
        });
        adapter.state = AdapterState::Connected;
        assert_eq!(adapter.state(), AdapterState::Connected);
        assert!(adapter.connection_status().is_some());

        // Simulate Connected → Active (mirrors handle_telemetry)
        adapter.state = AdapterState::Active;
        assert_eq!(adapter.state(), AdapterState::Active);
    }

    #[tokio::test]
    async fn test_state_active_to_disconnected_on_timeout() {
        let mut adapter = create_test_adapter();
        let addr: std::net::SocketAddr = "127.0.0.1:19902".parse().unwrap();

        // Place adapter in Active state with a timed-out connection (10 s > 2 s timeout)
        adapter.active_connection = Some(DcsConnection {
            addr,
            version: crate::socket_bridge::ProtocolVersion::V1_0,
            features: vec![],
            last_telemetry: Instant::now() - Duration::from_secs(10),
            aircraft: None,
            session_type: crate::mp_detection::SessionType::Unknown,
        });
        adapter.state = AdapterState::Active;

        // check_connection_health must detect the timeout and transition → Disconnected
        adapter.check_connection_health().await.unwrap();

        assert_eq!(adapter.state(), AdapterState::Disconnected);
        assert!(adapter.active_connection.is_none());
    }

    #[tokio::test]
    async fn test_state_connected_to_disconnected_on_timeout() {
        let mut adapter = create_test_adapter();
        let addr: std::net::SocketAddr = "127.0.0.1:19903".parse().unwrap();

        adapter.active_connection = Some(DcsConnection {
            addr,
            version: crate::socket_bridge::ProtocolVersion::V1_0,
            features: vec![],
            last_telemetry: Instant::now() - Duration::from_secs(5),
            aircraft: None,
            session_type: crate::mp_detection::SessionType::Unknown,
        });
        adapter.state = AdapterState::Connected;

        adapter.check_connection_health().await.unwrap();

        assert_eq!(adapter.state(), AdapterState::Disconnected);
        assert!(adapter.active_connection.is_none());
    }
}
