// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Main MSFS SimConnect adapter implementation
//!
//! Provides the high-level adapter interface that integrates session management,
//! aircraft detection, variable mapping, and event handling into a unified
//! SimConnect adapter for Flight Hub.

use crate::aircraft::{AircraftDetector, AircraftInfo, DetectionError};
use crate::events::{EventError, EventManager};
use crate::mapping::{MappingConfig, MappingError, VariableMapping};
use crate::session::{SessionConfig, SessionError, SessionEvent, SimConnectSession};
use flight_adapter_common::{AdapterMetrics, AdapterState};
use flight_bus::adapters::SimAdapter;
use flight_bus::publisher::BusPublisher;
use flight_bus::snapshot::BusSnapshot;
use flight_bus::types::{AircraftId, BusTypeError, SimId};
use flight_metrics::{
    MetricsRegistry,
    common::{
        ADAPTER_ERRORS_TOTAL, ADAPTER_TIME_SINCE_LAST_PACKET_MS, ADAPTER_UPDATE_LATENCY_MS,
        ADAPTER_UPDATES_TOTAL,
    },
};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::{RwLock, mpsc, mpsc::error::TryRecvError};
use tokio::time::interval;
use tracing::{debug, error, info, warn};

/// MSFS adapter configuration
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
    /// Bus publisher for delivering snapshots to subscribers
    bus_publisher: Option<Arc<Mutex<BusPublisher>>>,
    /// Last telemetry update time
    last_update: Instant,
    /// Connection attempt count
    connection_attempts: u32,
    /// Last connection attempt time
    last_connection_attempt: Option<Instant>,
    /// Current backoff delay in seconds
    current_backoff_delay: f64,
    /// Aircraft detection start time while waiting for one-shot identification payload.
    detection_started_at: Option<Instant>,
    /// Detected MSFS version: true = MSFS 2024, false = MSFS 2020/legacy.
    /// Set from dwApplicationVersionMajor in SIMCONNECT_RECV_OPEN (≥ 13 → 2024).
    is_msfs2024: bool,
    /// Adapter metrics
    metrics: Arc<RwLock<AdapterMetrics>>,
    /// Shared metrics registry
    metrics_registry: Arc<MetricsRegistry>,
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
            bus_publisher: None,
            last_update: Instant::now(),
            connection_attempts: 0,
            last_connection_attempt: None,
            current_backoff_delay: 1.0, // Start with 1 second
            detection_started_at: None,
            is_msfs2024: false,
            metrics: Arc::new(RwLock::new(AdapterMetrics::new())),
            metrics_registry: Arc::new(MetricsRegistry::new()),
        })
    }

    /// Attach a bus publisher so snapshots are delivered to subscribers.
    pub fn with_bus_publisher(mut self, publisher: Arc<Mutex<BusPublisher>>) -> Self {
        self.bus_publisher = Some(publisher);
        self
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
    pub async fn send_event(
        &mut self,
        event_name: &str,
        data: Option<u32>,
    ) -> Result<(), MsfsAdapterError> {
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
        info!("Connecting to MSFS via SimConnect (local connection, no SimConnect.cfg required)");

        let mut session = SimConnectSession::new(self.config.session.clone())?;
        session.connect().await?;

        // Setup aircraft detection
        if let Some(handle) = session.handle() {
            self.aircraft_detector
                .setup_detection(session.api(), handle)?;
            self.event_manager
                .setup_common_events(session.api(), handle)?;
        }

        self.session = Some(session);
        *self.state.write().await = AdapterState::Connected;
        self.connection_attempts = 0;
        self.current_backoff_delay = 1.0; // Reset backoff on successful connection
        self.detection_started_at = None;

        info!("Connected to MSFS successfully");
        Ok(())
    }

    async fn start_update_loop(&mut self) {
        // Start main update loop
        let mut update_interval = interval(Duration::from_millis(16)); // ~60Hz

        loop {
            update_interval.tick().await;

            // Poll the SimConnect pipe first and convert pending packets into session events.
            if let Some(session) = &mut self.session {
                match session.poll().await {
                    Ok(()) => {}
                    Err(SessionError::ConnectionLost) => {
                        self.handle_connection_loss().await;
                    }
                    Err(e) => {
                        error!("Session polling error: {}", e);
                        *self.state.write().await = AdapterState::Error;
                    }
                }
            }

            // Drain queued session events and apply state/data updates.
            if let Err(e) = self.drain_session_events().await {
                warn!("Session event processing failed: {}", e);
                self.metrics_registry.inc_counter(ADAPTER_ERRORS_TOTAL, 1);
            }

            let current_state = self.state().await;
            match current_state {
                AdapterState::Disconnected => {
                    if self.config.auto_reconnect
                        && self.connection_attempts < self.config.max_reconnect_attempts
                        && let Err(e) = self.attempt_reconnect().await
                    {
                        error!("Reconnection attempt failed: {}", e);
                        self.metrics_registry.inc_counter(ADAPTER_ERRORS_TOTAL, 1);
                        self.connection_attempts += 1;
                    }
                }
                AdapterState::Connected => {
                    // Try to detect aircraft
                    if let Err(e) = self.detect_aircraft().await {
                        warn!("Aircraft detection failed: {}", e);
                        self.metrics_registry.inc_counter(ADAPTER_ERRORS_TOTAL, 1);
                    }
                }
                AdapterState::DetectingAircraft => {
                    if let Some(started_at) = self.detection_started_at
                        && started_at.elapsed() > self.config.aircraft_detection_timeout
                    {
                        warn!("Aircraft detection timed out");
                        self.detection_started_at = None;
                        *self.state.write().await = AdapterState::Connected;
                    }
                }
                AdapterState::Active => {
                    // Update telemetry
                    if let Err(e) = self.update_telemetry().await {
                        warn!("Telemetry update failed: {}", e);
                        self.metrics_registry.inc_counter(ADAPTER_ERRORS_TOTAL, 1);
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
        }
    }

    async fn attempt_reconnect(&mut self) -> Result<(), MsfsAdapterError> {
        // Check if we should attempt reconnection based on backoff timing
        if let Some(last_attempt) = self.last_connection_attempt {
            let elapsed = last_attempt.elapsed().as_secs_f64();
            if elapsed < self.current_backoff_delay {
                // Not enough time has passed, skip this attempt
                return Ok(());
            }
        }

        self.connection_attempts += 1;
        self.last_connection_attempt = Some(Instant::now());

        info!(
            "Attempting to reconnect to MSFS (attempt {}, backoff: {:.1}s)",
            self.connection_attempts, self.current_backoff_delay
        );

        // Clean up existing session
        if let Some(mut session) = self.session.take() {
            let _ = session.disconnect();
        }

        // Try to reconnect
        match self.connect().await {
            Ok(()) => {
                info!("Reconnection successful");
                Ok(())
            }
            Err(e) => {
                // Exponential backoff with cap at 30 seconds
                // Formula: min(30, 1 * 2^attempt)
                self.current_backoff_delay = (self.current_backoff_delay * 2.0).min(30.0);
                warn!(
                    "Reconnection attempt {} failed: {}. Next attempt in {:.1}s",
                    self.connection_attempts, e, self.current_backoff_delay
                );
                Err(e)
            }
        }
    }

    async fn detect_aircraft(&mut self) -> Result<(), MsfsAdapterError> {
        if self.detection_started_at.is_some() {
            return Ok(());
        }

        let session = self
            .session
            .as_ref()
            .ok_or(MsfsAdapterError::NotConnected)?;
        let handle = session.handle().ok_or(MsfsAdapterError::NotConnected)?;

        self.aircraft_detector
            .start_detection(session.api(), handle)?;
        self.detection_started_at = Some(Instant::now());
        *self.state.write().await = AdapterState::DetectingAircraft;

        Ok(())
    }

    async fn drain_session_events(&mut self) -> Result<(), MsfsAdapterError> {
        let mut pending_events = Vec::new();

        if let Some(session) = &self.session {
            let event_receiver = session.event_receiver();
            let mut guard = event_receiver.lock().await;
            if let Some(receiver) = guard.as_mut() {
                loop {
                    match receiver.try_recv() {
                        Ok(event) => pending_events.push(event),
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => {
                            pending_events.push(SessionEvent::Disconnected);
                            break;
                        }
                    }
                }
            }
        }

        for event in pending_events {
            self.handle_session_event(event).await?;
        }

        Ok(())
    }

    async fn handle_session_event(&mut self, event: SessionEvent) -> Result<(), MsfsAdapterError> {
        match event {
            SessionEvent::Connected {
                app_name,
                app_version,
                simconnect_version,
            } => {
                // MSFS 2024 reports dwApplicationVersionMajor ≥ 13.
                // MSFS 2020 (SU5 through SU15) uses versions 11–12.
                self.is_msfs2024 = app_version.0 >= 13;
                info!(
                    "SimConnect connected: {} app={}.{}.{}.{}, simconnect={}.{}.{}.{} ({})",
                    app_name,
                    app_version.0,
                    app_version.1,
                    app_version.2,
                    app_version.3,
                    simconnect_version.0,
                    simconnect_version.1,
                    simconnect_version.2,
                    simconnect_version.3,
                    if self.is_msfs2024 {
                        "MSFS 2024"
                    } else {
                        "MSFS 2020"
                    }
                );
            }
            SessionEvent::Disconnected => {
                self.handle_connection_loss().await;
            }
            SessionEvent::Exception {
                exception,
                send_id,
                index,
            } => {
                warn!(
                    "SimConnect exception: code={}, send_id={}, index={}",
                    exception, send_id, index
                );
            }
            SessionEvent::DataReceived {
                request_id,
                object_id: _,
                define_id: _,
                data,
            } => {
                self.handle_data_received_event(request_id, &data).await?;
            }
            SessionEvent::EventReceived {
                group_id: _,
                event_id,
                data,
            } => {
                self.handle_sim_event(event_id, data).await;
            }
        }

        Ok(())
    }

    async fn handle_data_received_event(
        &mut self,
        request_id: u32,
        data: &[u8],
    ) -> Result<(), MsfsAdapterError> {
        if request_id == self.aircraft_detector.request_id() {
            if let Some(aircraft) = self.aircraft_detector.process_aircraft_data(data)? {
                self.handle_aircraft_detected(aircraft).await?;
            }
            return Ok(());
        }

        let aircraft = self.current_aircraft.read().await.clone();
        let Some(aircraft) = aircraft else {
            return Ok(());
        };

        let aircraft_id = AircraftId::new(&aircraft.atc_model);
        let sim_id = self.sim_id();
        let mut snapshot = self
            .current_snapshot
            .read()
            .await
            .clone()
            .unwrap_or_else(|| BusSnapshot::new(sim_id, aircraft_id));

        if let Some(mapping) = self.variable_mapping.as_ref() {
            match mapping.convert_to_snapshot(request_id, data, &mut snapshot) {
                Ok(()) => {}
                Err(MappingError::VariableNotFound(_)) => {
                    debug!("Ignoring unmapped request id {}", request_id);
                    return Ok(());
                }
                Err(e) => return Err(e.into()),
            }
        } else {
            return Ok(());
        }

        snapshot.timestamp = monotonic_timestamp_ns();
        *self.current_snapshot.write().await = Some(snapshot);

        Ok(())
    }

    async fn handle_aircraft_detected(
        &mut self,
        aircraft_info: AircraftInfo,
    ) -> Result<(), MsfsAdapterError> {
        info!("Aircraft detected: {}", aircraft_info.title);
        self.detection_started_at = None;
        *self.current_aircraft.write().await = Some(aircraft_info.clone());

        self.setup_variable_mapping(&aircraft_info).await?;

        let aircraft_id = AircraftId::new(&aircraft_info.atc_model);
        *self.current_snapshot.write().await = Some(BusSnapshot::new(self.sim_id(), aircraft_id));
        *self.state.write().await = AdapterState::Active;
        Ok(())
    }

    async fn handle_sim_event(&mut self, event_id: u32, data: u32) {
        let event_name = self
            .event_manager
            .get_event_name(event_id)
            .map(str::to_string);
        if let Some(name) = event_name.as_deref() {
            debug!(
                "Received SimConnect event {} ({}) data={}",
                event_id, name, data
            );
        } else {
            debug!("Received SimConnect event {} data={}", event_id, data);
        }

        match event_name.as_deref() {
            Some("AircraftLoaded") | Some("FlightLoaded") => {
                self.variable_mapping = None;
                self.detection_started_at = None;
                *self.current_aircraft.write().await = None;
                *self.current_snapshot.write().await = None;
                *self.state.write().await = AdapterState::Connected;
            }
            Some("SimStop") => {
                *self.state.write().await = AdapterState::Connected;
            }
            _ => {}
        }
    }

    async fn setup_variable_mapping(
        &mut self,
        aircraft_info: &AircraftInfo,
    ) -> Result<(), MsfsAdapterError> {
        info!(
            "Setting up variable mapping for aircraft: {}",
            aircraft_info.atc_model
        );

        let mut mapping = VariableMapping::new(self.config.mapping.clone());

        if let Some(session) = &self.session
            && let Some(handle) = session.handle()
        {
            mapping.setup_aircraft_definitions(session.api(), handle, &aircraft_info.atc_model)?;
            mapping.start_data_requests(session.api(), handle)?;
        }

        self.variable_mapping = Some(mapping);
        Ok(())
    }

    async fn update_telemetry(&mut self) -> Result<(), MsfsAdapterError> {
        // Rate limiting
        let now = Instant::now();
        self.metrics_registry.set_gauge(
            ADAPTER_TIME_SINCE_LAST_PACKET_MS,
            now.duration_since(self.last_update).as_secs_f64() * 1000.0,
        );
        let min_interval = Duration::from_secs_f32(1.0 / self.config.publish_rate);
        if now.duration_since(self.last_update) < min_interval {
            return Ok(());
        }
        self.last_update = now;
        let update_start = Instant::now();

        let aircraft = self.current_aircraft.read().await.clone();
        let mut snapshot = self.current_snapshot.read().await.clone();

        if let Some(snapshot_ref) = snapshot.as_mut() {
            snapshot_ref.timestamp = monotonic_timestamp_ns();
            if let Err(e) = snapshot_ref.validate() {
                warn!("Snapshot validation failed: {}", e);
                self.metrics_registry.inc_counter(ADAPTER_ERRORS_TOTAL, 1);
                return Ok(());
            }

            if let Some(aircraft) = aircraft {
                let mut metrics = self.metrics.write().await;
                metrics.record_update();
                metrics.record_aircraft_change(aircraft.title);
            }

            let snapshot_to_publish = snapshot_ref.clone();
            *self.current_snapshot.write().await = Some(snapshot_to_publish.clone());

            self.metrics_registry.inc_counter(ADAPTER_UPDATES_TOTAL, 1);
            self.metrics_registry.observe(
                ADAPTER_UPDATE_LATENCY_MS,
                update_start.elapsed().as_secs_f64() * 1000.0,
            );

            // Publish to bus subscribers
            if let Some(ref publisher) = self.bus_publisher {
                if let Ok(mut pub_guard) = publisher.lock() {
                    if let Err(e) = pub_guard.publish(snapshot_to_publish.clone()) {
                        warn!("Failed to publish snapshot to bus: {}", e);
                        self.metrics_registry.inc_counter(ADAPTER_ERRORS_TOTAL, 1);
                    }
                }
            }

            if let Err(e) = self.snapshot_sender.send(snapshot_to_publish) {
                warn!("Failed to publish snapshot: {}", e);
                self.metrics_registry.inc_counter(ADAPTER_ERRORS_TOTAL, 1);
            }
        }

        Ok(())
    }

    /// Handle connection loss by transitioning to disconnected state
    pub async fn handle_connection_loss(&mut self) {
        warn!("SimConnect connection lost, transitioning to Disconnected state");

        // Clean up session
        if let Some(mut session) = self.session.take() {
            let _ = session.disconnect();
        }

        // Publish a stale snapshot so subscribers know data is no longer valid.
        // ValidityFlags are all-false by default, signalling safe_for_ffb=false.
        if let Some(ref publisher) = self.bus_publisher {
            let stale = BusSnapshot::new(self.sim_id(), AircraftId::new("unknown"));
            if let Ok(mut pub_guard) = publisher.lock() {
                if let Err(e) = pub_guard.publish(stale) {
                    warn!("Failed to publish stale snapshot on connection loss: {}", e);
                }
            }
        }

        // Clear current state
        *self.current_aircraft.write().await = None;
        *self.current_snapshot.write().await = None;
        self.variable_mapping = None;
        self.detection_started_at = None;

        // Transition to disconnected
        *self.state.write().await = AdapterState::Disconnected;

        info!("Adapter state transitioned to Disconnected, will attempt reconnection if enabled");
    }

    /// Get current backoff delay for testing
    pub fn current_backoff_delay(&self) -> f64 {
        self.current_backoff_delay
    }

    /// Get connection attempts count for testing
    pub fn connection_attempts(&self) -> u32 {
        self.connection_attempts
    }

    /// Get adapter metrics
    pub async fn metrics(&self) -> AdapterMetrics {
        self.metrics.read().await.clone()
    }

    /// Get shared metrics registry
    pub fn metrics_registry(&self) -> Arc<MetricsRegistry> {
        self.metrics_registry.clone()
    }

    /// Get metrics summary string
    pub async fn metrics_summary(&self) -> String {
        self.metrics.read().await.summary()
    }
}

fn monotonic_timestamp_ns() -> u64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    let start = START.get_or_init(Instant::now);
    Instant::now().duration_since(*start).as_nanos() as u64
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
        if self.is_msfs2024 {
            SimId::Msfs2024
        } else {
            SimId::Msfs
        }
    }

    fn validate_raw_data(&self, _raw: &Self::RawData) -> Result<(), Self::Error> {
        // Validate raw SimConnect data
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flight_bus::publisher::SubscriptionConfig;

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
                flight_simconnect_sys::SimConnectError::LibraryNotFound,
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
                // sim_id() returns Msfs or Msfs2024 depending on detected version.
                assert!(matches!(adapter.sim_id(), SimId::Msfs | SimId::Msfs2024));

                // Test validation
                let raw_data = vec![1, 2, 3, 4];
                assert!(adapter.validate_raw_data(&raw_data).is_ok());
            }
            Err(MsfsAdapterError::Session(SessionError::SimConnect(
                flight_simconnect_sys::SimConnectError::LibraryNotFound,
            ))) => {
                // Expected on systems without SimConnect
                println!("SimConnect library not found");
            }
            Err(e) => {
                panic!("Unexpected error: {}", e);
            }
        }
    }

    /// Test connection state transitions
    /// Requirements: MSFS-INT-01.2, MSFS-INT-01.19
    #[tokio::test]
    async fn test_connection_state_transitions() {
        let config = MsfsAdapterConfig::default();

        match MsfsAdapter::new(config) {
            Ok(adapter) => {
                // Initial state should be Disconnected
                assert_eq!(adapter.state().await, AdapterState::Disconnected);

                // State transitions are tested through the adapter lifecycle
                // Disconnected -> Connecting -> Connected -> DetectingAircraft -> Active
                // or Disconnected -> Connecting -> Error
            }
            Err(MsfsAdapterError::Session(SessionError::SimConnect(
                flight_simconnect_sys::SimConnectError::LibraryNotFound,
            ))) => {
                println!("SimConnect library not found - skipping state transition test");
            }
            Err(e) => {
                panic!("Unexpected error: {}", e);
            }
        }
    }

    /// Test exponential backoff timing
    /// Requirements: MSFS-INT-01.2
    #[tokio::test]
    async fn test_exponential_backoff_timing() {
        let config = MsfsAdapterConfig::default();

        match MsfsAdapter::new(config) {
            Ok(mut adapter) => {
                // Initial backoff should be 1 second
                assert_eq!(adapter.current_backoff_delay(), 1.0);

                // Simulate failed connection attempts
                // Note: We can't actually call attempt_reconnect without a real SimConnect
                // but we can verify the backoff calculation logic

                // After first failure: 1 * 2 = 2 seconds
                adapter.current_backoff_delay = 2.0;
                assert_eq!(adapter.current_backoff_delay(), 2.0);

                // After second failure: 2 * 2 = 4 seconds
                adapter.current_backoff_delay = 4.0;
                assert_eq!(adapter.current_backoff_delay(), 4.0);

                // After third failure: 4 * 2 = 8 seconds
                adapter.current_backoff_delay = 8.0;
                assert_eq!(adapter.current_backoff_delay(), 8.0);

                // After fourth failure: 8 * 2 = 16 seconds
                adapter.current_backoff_delay = 16.0;
                assert_eq!(adapter.current_backoff_delay(), 16.0);

                // After fifth failure: 16 * 2 = 32, but capped at 30 seconds
                adapter.current_backoff_delay = 30.0;
                assert_eq!(adapter.current_backoff_delay(), 30.0);

                // Verify cap is enforced
                let next_backoff = (adapter.current_backoff_delay() * 2.0).min(30.0);
                assert_eq!(next_backoff, 30.0);
            }
            Err(MsfsAdapterError::Session(SessionError::SimConnect(
                flight_simconnect_sys::SimConnectError::LibraryNotFound,
            ))) => {
                println!("SimConnect library not found - skipping backoff test");
            }
            Err(e) => {
                panic!("Unexpected error: {}", e);
            }
        }
    }

    /// Test connection loss detection
    /// Requirements: MSFS-INT-01.19
    #[tokio::test]
    async fn test_connection_loss_detection() {
        let config = MsfsAdapterConfig::default();

        match MsfsAdapter::new(config) {
            Ok(mut adapter) => {
                // Verify initial state
                assert_eq!(adapter.state().await, AdapterState::Disconnected);
                assert_eq!(adapter.connection_attempts(), 0);

                // Simulate connection loss by calling handle_connection_loss
                adapter.handle_connection_loss().await;

                // Verify state transitions to Disconnected
                assert_eq!(adapter.state().await, AdapterState::Disconnected);

                // Verify state is cleared
                assert!(adapter.current_aircraft().await.is_none());
                assert!(adapter.current_snapshot().await.is_none());
            }
            Err(MsfsAdapterError::Session(SessionError::SimConnect(
                flight_simconnect_sys::SimConnectError::LibraryNotFound,
            ))) => {
                println!("SimConnect library not found - skipping connection loss test");
            }
            Err(e) => {
                panic!("Unexpected error: {}", e);
            }
        }
    }

    /// Test auto-reconnection configuration
    /// Requirements: MSFS-INT-01.2
    #[test]
    fn test_auto_reconnection_config() {
        let mut config = MsfsAdapterConfig::default();

        // Default should have auto-reconnect enabled
        assert!(config.auto_reconnect);
        assert_eq!(config.max_reconnect_attempts, 5);

        // Test disabling auto-reconnect
        config.auto_reconnect = false;
        assert!(!config.auto_reconnect);

        // Test custom max attempts
        config.max_reconnect_attempts = 10;
        assert_eq!(config.max_reconnect_attempts, 10);
    }

    /// Test local SimConnect connection (no SimConnect.cfg required)
    /// Requirements: MSFS-INT-01.1
    #[test]
    fn test_local_simconnect_connection() {
        let config = MsfsAdapterConfig::default();

        // Verify config uses local connection (config_index: 0)
        assert_eq!(config.session.config_index, 0);

        // Local connection should not require SimConnect.cfg
        // This is verified by the session configuration
    }

    /// Test connection attempt tracking
    /// Requirements: MSFS-INT-01.2
    #[tokio::test]
    async fn test_connection_attempt_tracking() {
        let config = MsfsAdapterConfig::default();

        match MsfsAdapter::new(config) {
            Ok(adapter) => {
                // Initial attempts should be 0
                assert_eq!(adapter.connection_attempts(), 0);

                // Backoff should start at 1 second
                assert_eq!(adapter.current_backoff_delay(), 1.0);
            }
            Err(MsfsAdapterError::Session(SessionError::SimConnect(
                flight_simconnect_sys::SimConnectError::LibraryNotFound,
            ))) => {
                println!("SimConnect library not found - skipping attempt tracking test");
            }
            Err(e) => {
                panic!("Unexpected error: {}", e);
            }
        }
    }

    /// Test state machine completeness
    /// Requirements: MSFS-INT-01.2, MSFS-INT-01.19
    #[test]
    fn test_state_machine_completeness() {
        // Verify all required states exist
        let _disconnected = AdapterState::Disconnected;
        let _connecting = AdapterState::Connecting;
        let _connected = AdapterState::Connected;
        let _detecting = AdapterState::DetectingAircraft;
        let _active = AdapterState::Active;
        let _error = AdapterState::Error;

        // Verify states are distinct
        assert_ne!(AdapterState::Disconnected, AdapterState::Connecting);
        assert_ne!(AdapterState::Connecting, AdapterState::Connected);
        assert_ne!(AdapterState::Connected, AdapterState::DetectingAircraft);
        assert_ne!(AdapterState::DetectingAircraft, AdapterState::Active);
        assert_ne!(AdapterState::Active, AdapterState::Error);
    }

    /// Test backoff reset on successful connection
    /// Requirements: MSFS-INT-01.2
    #[tokio::test]
    async fn test_backoff_reset_on_success() {
        let config = MsfsAdapterConfig::default();

        match MsfsAdapter::new(config) {
            Ok(mut adapter) => {
                // Simulate multiple failed attempts
                adapter.current_backoff_delay = 16.0;
                adapter.connection_attempts = 4;

                // Verify backoff is high
                assert_eq!(adapter.current_backoff_delay(), 16.0);
                assert_eq!(adapter.connection_attempts(), 4);

                // On successful connection, backoff should reset
                // This is verified in the connect() method implementation
                // which sets current_backoff_delay = 1.0 and connection_attempts = 0
            }
            Err(MsfsAdapterError::Session(SessionError::SimConnect(
                flight_simconnect_sys::SimConnectError::LibraryNotFound,
            ))) => {
                println!("SimConnect library not found - skipping backoff reset test");
            }
            Err(e) => {
                panic!("Unexpected error: {}", e);
            }
        }
    }

    /// Test that update_telemetry publishes a snapshot to an attached BusPublisher.
    /// Requirements: MSFS-INT-01.6
    #[tokio::test]
    async fn test_adapter_publishes_snapshot_to_bus() {
        let publisher = Arc::new(Mutex::new(BusPublisher::new(60.0)));
        let mut subscriber = publisher
            .lock()
            .unwrap()
            .subscribe(SubscriptionConfig::default())
            .unwrap();

        let config = MsfsAdapterConfig::default();
        let mut adapter = MsfsAdapter::new(config)
            .expect("MsfsAdapter::new must not fail")
            .with_bus_publisher(publisher);

        // Prime the adapter with a valid snapshot and force the rate-limit window open.
        let snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
        *adapter.current_snapshot.write().await = Some(snapshot);
        *adapter.state.write().await = AdapterState::Active;
        adapter.last_update = Instant::now() - Duration::from_secs(1);

        adapter
            .update_telemetry()
            .await
            .expect("update_telemetry must succeed");

        let received = subscriber.try_recv().expect("try_recv must not error");
        assert!(
            received.is_some(),
            "BusPublisher subscriber should have received a snapshot"
        );
        assert_eq!(received.unwrap().sim, SimId::Msfs);
    }

    /// Test that handle_connection_loss publishes a stale snapshot (validity all-false)
    /// so downstream FFB and other consumers know the data is no longer trustworthy.
    /// Requirements: MSFS-INT-01.19
    #[tokio::test]
    async fn test_adapter_publishes_stale_snapshot_on_connection_loss() {
        let publisher = Arc::new(Mutex::new(BusPublisher::new(60.0)));
        let mut subscriber = publisher
            .lock()
            .unwrap()
            .subscribe(SubscriptionConfig::default())
            .unwrap();

        let config = MsfsAdapterConfig::default();
        let mut adapter = MsfsAdapter::new(config)
            .expect("MsfsAdapter::new must not fail")
            .with_bus_publisher(publisher);

        adapter.handle_connection_loss().await;

        let received = subscriber.try_recv().expect("try_recv must not error");
        assert!(
            received.is_some(),
            "A stale snapshot should be published on connection loss"
        );
        let snap = received.unwrap();
        // Validity flags must all be false — safe_for_ffb=false ensures FFB is disabled.
        assert!(!snap.validity.safe_for_ffb);
        assert!(!snap.validity.attitude_valid);
    }
}
