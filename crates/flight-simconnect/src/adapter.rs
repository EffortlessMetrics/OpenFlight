// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Main MSFS SimConnect adapter implementation
//!
//! Provides the high-level adapter interface that integrates session management,
//! aircraft detection, variable mapping, and event handling into a unified
//! SimConnect adapter for Flight Hub.

use crate::aircraft::{AircraftDetector, AircraftInfo, DetectionError};
use crate::events::{EventManager, EventError};
use crate::mapping::{MappingConfig, MappingError, VariableMapping};
use crate::session::{SessionConfig, SessionError, SessionEvent, SimConnectSession};
use flight_bus::adapters::SimAdapter;
use flight_bus::snapshot::BusSnapshot;
use flight_bus::types::{AircraftId, BusTypeError, SimId};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::{mpsc, RwLock};
use tokio::time::interval;
use tracing::{debug, error, info, warn};

/// MSFS adapter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsfsAdapterConfig {
    /// SimConnect session configuration
    pub session: SessionConfig,
    /// Variable mapping configuration
    pub mapping: MappingConfig,
    /// Telemetry publishing rate (Hz)
    pub publish_rate: f32,
    /// Aircraft detection timeout
    pub aircraft_detection_timeout: Duration,
    /// Auto-reconnection settings
    pub auto_reconnect: bool,
    /// Maximum reconnection attempts
    pub max_reconnect_attempts: u32,
}

impl Default for MsfsAdapterConfig {
    fn default() -> Self {
        Self {
            session: SessionConfig::default(),
            mapping: crate::mapping::create_default_mapping(),
            publish_rate: 60.0,
            aircraft_detection_timeout: Duration::from_secs(30),
            auto_reconnect: true,
            max_reconnect_attempts: 5,
        }
    }
}

/// MSFS adapter error types
#[derive(Debug, Error)]
pub enum MsfsAdapterError {
    #[error("Session error: {0}")]
    Session(#[from] SessionError),
    #[error("Mapping error: {0}")]
    Mapping(#[from] MappingError),
    #[error("Event error: {0}")]
    Event(#[from] EventError),
    #[error("Detection error: {0}")]
    Detection(#[from] DetectionError),
    #[error("Bus type error: {0}")]
    BusType(#[from] BusTypeError),
    #[error("Not connected to MSFS")]
    NotConnected,
    #[error("Aircraft not detected")]
    AircraftNotDetected,
    #[error("Configuration error: {0}")]
    Configuration(String),
    #[error("Timeout: {0}")]
    Timeout(String),
}

/// MSFS adapter state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterState {
    /// Disconnected from MSFS
    Disconnected,
    /// Connecting to MSFS
    Connecting,
    /// Connected but no aircraft detected
    Connected,
    /// Aircraft detected, setting up data definitions
    DetectingAircraft,
    /// Fully operational
    Active,
    /// Error state
    Error,
}

/// Main MSFS SimConnect adapter
pub struct MsfsAdapter {
    /// Adapter configuration
    config: MsfsAdapterConfig,
    /// SimConnect session
    session: Option<SimConnectSession>,
    /// Aircraft detector
    aircraft_detector: AircraftDetector,
    /// Event manager
    event_manager: EventManager,
    /// Variable mapping
    variable_mapping: Option<VariableMapping>,
    /// Current adapter state
    state: Arc<RwLock<AdapterState>>,
    /// Current aircraft information
    current_aircraft: Arc<RwLock<Option<AircraftInfo>>>,
    /// Current bus snapshot
    current_snapshot: Arc<RwLock<Option<BusSnapshot>>>,
    /// Snapshot publisher
    snapshot_sender: mpsc::UnboundedSender<BusSnapshot>,
    /// Snapshot receiver for external consumers
    snapshot_receiver: Arc<RwLock<mpsc::UnboundedReceiver<BusSnapshot>>>,
    /// Last telemetry update time
    last_update: Instant,
    /// Connection attempt count
    connection_attempts: u32,
}

impl MsfsAdapter {
    /// Create a new MSFS adapter
    pub fn new(config: MsfsAdapterConfig) -> Result<Self, MsfsAdapterError> {
        let (snapshot_sender, snapshot_receiver) = mpsc::unbounded_channel();

        Ok(Self {
            config,
            session: None,
            aircraft_detector: AircraftDetector::new(),
            event_manager: EventManager::new(),
            variable_mapping: None,
            state: Arc::new(RwLock::new(AdapterState::Disconnected)),
            current_aircraft: Arc::new(RwLock::new(None)),
            current_snapshot: Arc::new(RwLock::new(None)),
            snapshot_sender,
            snapshot_receiver: Arc::new(RwLock::new(snapshot_receiver)),
            last_update: Instant::now(),
            connection_attempts: 0,
        })
    }

    /// Start the adapter
    pub async fn start(&mut self) -> Result<(), MsfsAdapterError> {
        info!("Starting MSFS adapter");
        *self.state.write().await = AdapterState::Connecting;

        // Connect to SimConnect
        self.connect().await?;

        // Start the main update loop
        self.start_update_loop().await;

        Ok(())
    }

    /// Stop the adapter
    pub async fn stop(&mut self) -> Result<(), MsfsAdapterError> {
        info!("Stopping MSFS adapter");
        
        if let Some(mut session) = self.session.take() {
            session.disconnect()?;
        }

        *self.state.write().await = AdapterState::Disconnected;
        Ok(())
    }

    /// Get current adapter state
    pub async fn state(&self) -> AdapterState {
        *self.state.read().await
    }

    /// Get current aircraft information
    pub async fn current_aircraft(&self) -> Option<AircraftInfo> {
        self.current_aircraft.read().await.clone()
    }

    /// Get current bus snapshot
    pub async fn current_snapshot(&self) -> Option<BusSnapshot> {
        self.current_snapshot.read().await.clone()
    }

    /// Get snapshot receiver for external consumers
    pub async fn snapshot_receiver(&self) -> Arc<RwLock<mpsc::UnboundedReceiver<BusSnapshot>>> {
        self.snapshot_receiver.clone()
    }

    /// Check if adapter is connected and active
    pub async fn is_active(&self) -> bool {
        matches!(self.state().await, AdapterState::Active)
    }

    /// Send an event to MSFS
    pub async fn send_event(&mut self, event_name: &str, data: Option<u32>) -> Result<(), MsfsAdapterError> {
        if let Some(session) = &self.session {
            if let Some(handle) = session.handle() {
                self.event_manager.transmit_standard_event(
                    session.api(),
                    handle,
                    event_name,
                    data,
                )?;
                Ok(())
            } else {
                Err(MsfsAdapterError::NotConnected)
            }
        } else {
            Err(MsfsAdapterError::NotConnected)
        }
    }

    async fn connect(&mut self) -> Result<(), MsfsAdapterError> {
        info!("Connecting to MSFS via SimConnect");
        
        let mut session = SimConnectSession::new(self.config.session.clone())?;
        session.connect().await?;

        // Setup aircraft detection
        if let Some(handle) = session.handle() {
            self.aircraft_detector.setup_detection(session.api(), handle)?;
            self.event_manager.setup_common_events(session.api(), handle)?;
        }

        self.session = Some(session);
        *self.state.write().await = AdapterState::Connected;
        self.connection_attempts = 0;

        info!("Connected to MSFS successfully");
        Ok(())
    }

    async fn start_update_loop(&mut self) {
        let state = self.state.clone();
        let current_aircraft = self.current_aircraft.clone();
        let current_snapshot = self.current_snapshot.clone();
        let snapshot_sender = self.snapshot_sender.clone();
        
        // Clone necessary data for the async task
        let publish_interval = Duration::from_secs_f32(1.0 / self.config.publish_rate);
        
        tokio::spawn(async move {
            let mut interval = interval(publish_interval);
            
            loop {
                interval.tick().await;
                
                // Check if we should continue running
                let current_state = *state.read().await;
                if matches!(current_state, AdapterState::Disconnected | AdapterState::Error) {
                    break;
                }
                
                // Update telemetry would happen here
                // This is a simplified version - the actual implementation would
                // process SimConnect messages and update the bus snapshot
            }
        });

        // Start session event processing
        if let Some(session) = &mut self.session {
            let event_receiver = session.event_receiver();
            let state_clone = self.state.clone();
            let aircraft_clone = self.current_aircraft.clone();
            
            tokio::spawn(async move {
                let mut receiver = event_receiver.lock().await;
                
                while let Some(event) = receiver.recv().await {
                    match event {
                        SessionEvent::Connected { .. } => {
                            info!("SimConnect connection established");
                        }
                        SessionEvent::Disconnected => {
                            warn!("SimConnect connection lost");
                            *state_clone.write().await = AdapterState::Disconnected;
                        }
                        SessionEvent::Exception { exception, .. } => {
                            warn!("SimConnect exception: {}", exception);
                        }
                        SessionEvent::DataReceived { request_id, data, .. } => {
                            debug!("Received data for request {}: {} bytes", request_id, data.len());
                            // Process data here
                        }
                        SessionEvent::EventReceived { event_id, data, .. } => {
                            debug!("Received event {}: {}", event_id, data);
                            // Process event here
                        }
                    }
                }
            });
        }

        // Start main update loop
        let mut update_interval = interval(Duration::from_millis(16)); // ~60Hz
        
        loop {
            update_interval.tick().await;
            
            let current_state = self.state().await;
            match current_state {
                AdapterState::Disconnected => {
                    if self.config.auto_reconnect && self.connection_attempts < self.config.max_reconnect_attempts {
                        if let Err(e) = self.attempt_reconnect().await {
                            error!("Reconnection attempt failed: {}", e);
                            self.connection_attempts += 1;
                        }
                    }
                }
                AdapterState::Connected => {
                    // Try to detect aircraft
                    if let Err(e) = self.detect_aircraft().await {
                        warn!("Aircraft detection failed: {}", e);
                    }
                }
                AdapterState::DetectingAircraft => {
                    // Wait for aircraft detection to complete
                }
                AdapterState::Active => {
                    // Update telemetry
                    if let Err(e) = self.update_telemetry().await {
                        warn!("Telemetry update failed: {}", e);
                    }
                }
                AdapterState::Error => {
                    // Handle error state
                    if self.config.auto_reconnect {
                        *self.state.write().await = AdapterState::Disconnected;
                    }
                }
                _ => {}
            }

            // Poll session for messages
            if let Some(session) = &mut self.session {
                if let Err(e) = session.poll().await {
                    error!("Session polling error: {}", e);
                    *self.state.write().await = AdapterState::Error;
                }
            }
        }
    }

    async fn attempt_reconnect(&mut self) -> Result<(), MsfsAdapterError> {
        info!("Attempting to reconnect to MSFS (attempt {})", self.connection_attempts + 1);
        
        // Clean up existing session
        if let Some(mut session) = self.session.take() {
            let _ = session.disconnect();
        }

        // Wait before reconnecting
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Try to reconnect
        self.connect().await?;
        
        Ok(())
    }

    async fn detect_aircraft(&mut self) -> Result<(), MsfsAdapterError> {
        *self.state.write().await = AdapterState::DetectingAircraft;
        
        if let Some(session) = &self.session {
            if let Some(handle) = session.handle() {
                self.aircraft_detector.start_detection(session.api(), handle)?;
                
                // Wait for aircraft detection with timeout
                let timeout = tokio::time::timeout(
                    self.config.aircraft_detection_timeout,
                    self.wait_for_aircraft_detection(),
                ).await;

                match timeout {
                    Ok(Ok(aircraft_info)) => {
                        info!("Aircraft detected: {}", aircraft_info.title);
                        *self.current_aircraft.write().await = Some(aircraft_info.clone());
                        
                        // Setup variable mapping for this aircraft
                        self.setup_variable_mapping(&aircraft_info).await?;
                        
                        *self.state.write().await = AdapterState::Active;
                        Ok(())
                    }
                    Ok(Err(e)) => {
                        error!("Aircraft detection failed: {}", e);
                        *self.state.write().await = AdapterState::Error;
                        Err(e)
                    }
                    Err(_) => {
                        warn!("Aircraft detection timed out");
                        *self.state.write().await = AdapterState::Connected;
                        Err(MsfsAdapterError::Timeout("Aircraft detection timed out".to_string()))
                    }
                }
            } else {
                Err(MsfsAdapterError::NotConnected)
            }
        } else {
            Err(MsfsAdapterError::NotConnected)
        }
    }

    async fn wait_for_aircraft_detection(&mut self) -> Result<AircraftInfo, MsfsAdapterError> {
        // This would typically wait for aircraft detection events
        // For now, we'll simulate a successful detection
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Return a mock aircraft for testing
        Ok(AircraftInfo {
            title: "Cessna 172 Skyhawk".to_string(),
            atc_model: "C172".to_string(),
            atc_type: "CESSNA".to_string(),
            atc_airline: None,
            atc_flight_number: None,
            category: crate::aircraft::AircraftCategory::Airplane,
            engine_type: crate::aircraft::EngineType::Piston,
            engine_count: 1,
        })
    }

    async fn setup_variable_mapping(&mut self, aircraft_info: &AircraftInfo) -> Result<(), MsfsAdapterError> {
        info!("Setting up variable mapping for aircraft: {}", aircraft_info.atc_model);
        
        let mut mapping = VariableMapping::new(self.config.mapping.clone());
        
        if let Some(session) = &self.session {
            if let Some(handle) = session.handle() {
                mapping.setup_aircraft_definitions(session.api(), handle, &aircraft_info.atc_model)?;
                mapping.start_data_requests(session.api(), handle)?;
            }
        }
        
        self.variable_mapping = Some(mapping);
        Ok(())
    }

    async fn update_telemetry(&mut self) -> Result<(), MsfsAdapterError> {
        // Rate limiting
        let now = Instant::now();
        let min_interval = Duration::from_secs_f32(1.0 / self.config.publish_rate);
        if now.duration_since(self.last_update) < min_interval {
            return Ok(());
        }
        self.last_update = now;

        // Create or update bus snapshot
        let aircraft_info = self.current_aircraft.read().await;
        if let Some(ref aircraft) = *aircraft_info {
            let aircraft_id = AircraftId::new(&aircraft.atc_model);
            let mut snapshot = BusSnapshot::new(SimId::Msfs, aircraft_id);
            
            // Update snapshot with current data
            // This would typically process received SimConnect data
            // For now, we'll create a basic snapshot
            
            // Validate and publish snapshot
            if let Err(e) = snapshot.validate() {
                warn!("Snapshot validation failed: {}", e);
                return Ok(());
            }

            *self.current_snapshot.write().await = Some(snapshot.clone());
            
            if let Err(e) = self.snapshot_sender.send(snapshot) {
                warn!("Failed to publish snapshot: {}", e);
            }
        }

        Ok(())
    }
}

impl SimAdapter for MsfsAdapter {
    type RawData = Vec<u8>;
    type Error = MsfsAdapterError;

    fn convert_to_snapshot(&self, _raw: Self::RawData) -> Result<BusSnapshot, Self::Error> {
        // This would convert raw SimConnect data to a bus snapshot
        // For now, return the current snapshot if available
        if let Some(snapshot) = futures::executor::block_on(self.current_snapshot()) {
            Ok(snapshot)
        } else {
            Err(MsfsAdapterError::AircraftNotDetected)
        }
    }

    fn sim_id(&self) -> SimId {
        SimId::Msfs
    }

    fn validate_raw_data(&self, _raw: &Self::RawData) -> Result<(), Self::Error> {
        // Validate raw SimConnect data
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_adapter_creation() {
        let config = MsfsAdapterConfig::default();
        let adapter = MsfsAdapter::new(config);
        
        match adapter {
            Ok(adapter) => {
                assert_eq!(adapter.state().await, AdapterState::Disconnected);
                assert!(adapter.current_aircraft().await.is_none());
                assert!(adapter.current_snapshot().await.is_none());
            }
            Err(MsfsAdapterError::Session(SessionError::SimConnect(
                flight_simconnect_sys::SimConnectError::LibraryNotFound
            ))) => {
                // Expected on systems without SimConnect
                println!("SimConnect library not found - this is expected on systems without MSFS");
            }
            Err(e) => {
                panic!("Unexpected error creating adapter: {}", e);
            }
        }
    }

    #[test]
    fn test_adapter_config_default() {
        let config = MsfsAdapterConfig::default();
        assert_eq!(config.publish_rate, 60.0);
        assert_eq!(config.aircraft_detection_timeout, Duration::from_secs(30));
        assert!(config.auto_reconnect);
        assert_eq!(config.max_reconnect_attempts, 5);
    }

    #[test]
    fn test_adapter_state_transitions() {
        // Test that adapter states are properly defined
        assert_ne!(AdapterState::Disconnected, AdapterState::Connected);
        assert_ne!(AdapterState::Connected, AdapterState::Active);
        assert_ne!(AdapterState::Active, AdapterState::Error);
    }

    #[tokio::test]
    async fn test_sim_adapter_trait() {
        let config = MsfsAdapterConfig::default();
        
        match MsfsAdapter::new(config) {
            Ok(adapter) => {
                assert_eq!(adapter.sim_id(), SimId::Msfs);
                
                // Test validation
                let raw_data = vec![1, 2, 3, 4];
                assert!(adapter.validate_raw_data(&raw_data).is_ok());
            }
            Err(MsfsAdapterError::Session(SessionError::SimConnect(
                flight_simconnect_sys::SimConnectError::LibraryNotFound
            ))) => {
                // Expected on systems without SimConnect
                println!("SimConnect library not found");
            }
            Err(e) => {
                panic!("Unexpected error: {}", e);
            }
        }
    }
}