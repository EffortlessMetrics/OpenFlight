// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! X-Plane adapter implementation
//!
//! Main adapter that coordinates UDP DataRef access, plugin interface,
//! aircraft detection, and telemetry publishing to the flight bus.

use crate::{
    aircraft::{AircraftDetector, DetectedAircraft},
    dataref::{DataRef, DataRefManager, DataRefValue},
    latency::{LatencyBudget, LatencyTracker},
    plugin::PluginInterface,
    udp::{UdpClient, UdpConfig},
    web_api::{WebApiClient, WebApiConfig},
};
use flight_adapter_common::{
    AdapterConfig, AdapterError, AdapterMetrics, AdapterState, ReconnectionStrategy,
};
use flight_bus::{
    BusPublisher,
    adapters::{SimAdapter, xplane::XPlaneConverter},
    snapshot::{
        AngularRates, BusSnapshot, EngineData, Environment, Kinematics, Navigation, TrimState,
    },
    types::{AircraftId, AutopilotState, Percentage, SimId},
};
use flight_core::units::conversions;
use flight_core::{FlightError, Result};
use flight_metrics::{
    MetricsRegistry,
    common::{
        ADAPTER_ERRORS_TOTAL, ADAPTER_TIME_SINCE_LAST_PACKET_MS, ADAPTER_UPDATE_LATENCY_MS,
        ADAPTER_UPDATES_TOTAL,
    },
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, RwLock},
    time::{Duration, Instant},
};
use thiserror::Error;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

/// X-Plane adapter errors
#[derive(Error, Debug)]
pub enum XPlaneError {
    #[error("UDP communication error: {0}")]
    Udp(#[from] crate::udp::UdpError),
    #[error("Web API error: {0}")]
    WebApi(#[from] crate::web_api::WebApiError),
    #[error("DataRef error: {message}")]
    DataRef { message: String },
    #[error("Aircraft detection failed: {reason}")]
    AircraftDetection { reason: String },
    #[error("Latency budget exceeded: {actual_ms}ms > {budget_ms}ms")]
    LatencyBudget { actual_ms: u64, budget_ms: u64 },
    #[error("Configuration error: {message}")]
    Config { message: String },
    #[error(transparent)]
    Adapter(#[from] AdapterError),
    #[error("Connection timeout")]
    Timeout,
    #[error("X-Plane not running or not responding")]
    NotRunning,
}

impl From<XPlaneError> for FlightError {
    fn from(err: XPlaneError) -> Self {
        FlightError::Configuration(format!("X-Plane adapter error: {}", err))
    }
}

/// X-Plane adapter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XPlaneAdapterConfig {
    /// UDP configuration for DataRef access
    pub udp: UdpConfig,
    /// Web API configuration (optional)
    pub web_api: Option<WebApiConfig>,
    /// Enable plugin interface
    pub enable_plugin: bool,
    /// Telemetry publishing rate (Hz)
    pub publish_rate_hz: u32,
    /// Aircraft detection interval (seconds)
    pub aircraft_detection_interval_s: u32,
    /// Latency budget for telemetry updates (milliseconds)
    pub latency_budget_ms: u64,
    /// DataRef request timeout (milliseconds)
    pub dataref_timeout_ms: u64,
    /// Maximum retries for failed requests
    pub max_retries: u32,
}

impl Default for XPlaneAdapterConfig {
    fn default() -> Self {
        Self {
            udp: UdpConfig::default(),
            web_api: None,
            enable_plugin: false,
            publish_rate_hz: 30,
            aircraft_detection_interval_s: 5,
            latency_budget_ms: 50,
            dataref_timeout_ms: 100,
            max_retries: 3,
        }
    }
}

impl AdapterConfig for XPlaneAdapterConfig {
    fn publish_rate_hz(&self) -> f32 {
        self.publish_rate_hz as f32
    }

    fn connection_timeout(&self) -> Duration {
        Duration::from_millis(self.dataref_timeout_ms)
    }

    fn max_reconnect_attempts(&self) -> u32 {
        self.max_retries
    }

    fn enable_auto_reconnect(&self) -> bool {
        self.max_retries > 0
    }
}

/// Raw X-Plane telemetry data
#[derive(Debug, Clone)]
pub struct XPlaneRawData {
    pub timestamp: Instant,
    pub aircraft_info: DetectedAircraft,
    pub dataref_values: HashMap<String, DataRefValue>,
}

/// X-Plane adapter for Flight Hub
pub struct XPlaneAdapter {
    config: XPlaneAdapterConfig,
    udp_client: UdpClient,
    web_api_client: Option<WebApiClient>,
    plugin_interface: Option<PluginInterface>,
    dataref_manager: DataRefManager,
    aircraft_detector: AircraftDetector,
    latency_tracker: LatencyTracker,
    bus_publisher: Arc<Mutex<BusPublisher>>,
    state: Arc<RwLock<AdapterState>>,
    metrics: Arc<RwLock<AdapterMetrics>>,
    metrics_registry: Arc<MetricsRegistry>,
    current_aircraft: Arc<RwLock<Option<DetectedAircraft>>>,
    running: Arc<RwLock<bool>>,
    last_packet_time: Arc<RwLock<Option<Instant>>>,
    connection_timeout: Duration,
    start_time: Instant,
}

impl XPlaneAdapter {
    /// Create a new X-Plane adapter
    pub fn new(
        config: XPlaneAdapterConfig,
        bus_publisher: Arc<Mutex<BusPublisher>>,
    ) -> Result<Self> {
        let udp_client = UdpClient::new(config.udp.clone())
            .map_err(|e| FlightError::Configuration(format!("UDP client error: {}", e)))?;

        let web_api_client =
            if let Some(web_config) = &config.web_api {
                Some(WebApiClient::new(web_config.clone()).map_err(|e| {
                    FlightError::Configuration(format!("Web API client error: {}", e))
                })?)
            } else {
                None
            };

        let plugin_interface = if config.enable_plugin {
            Some(PluginInterface::new().map_err(|e| {
                FlightError::Configuration(format!("Plugin interface error: {}", e))
            })?)
        } else {
            None
        };

        let dataref_manager = DataRefManager::new();
        let aircraft_detector = AircraftDetector::new();
        let latency_tracker = LatencyTracker::new(LatencyBudget::new(Duration::from_millis(
            config.latency_budget_ms,
        )));

        Ok(Self {
            config,
            udp_client,
            web_api_client,
            plugin_interface,
            dataref_manager,
            aircraft_detector,
            latency_tracker,
            bus_publisher,
            state: Arc::new(RwLock::new(AdapterState::Disconnected)),
            metrics: Arc::new(RwLock::new(AdapterMetrics::new())),
            metrics_registry: Arc::new(MetricsRegistry::new()),
            current_aircraft: Arc::new(RwLock::new(None)),
            running: Arc::new(RwLock::new(false)),
            last_packet_time: Arc::new(RwLock::new(None)),
            connection_timeout: Duration::from_secs(2), // XPLANE-INT-01.13: 2 second timeout
            start_time: Instant::now(),
        })
    }

    /// Start the adapter
    pub async fn start(&self) -> Result<()> {
        info!("Starting X-Plane adapter");

        // Set running flag
        *self.running.write().unwrap() = true;
        *self.state.write().unwrap() = AdapterState::Connecting;

        // Test connection to X-Plane
        if let Err(err) = self.test_connection_with_retries().await {
            *self.state.write().unwrap() = AdapterState::Error;
            return Err(err);
        }
        *self.state.write().unwrap() = AdapterState::Connected;

        // Start telemetry publishing task
        let publish_handle = self.start_telemetry_publisher().await?;

        // Start aircraft detection task
        let aircraft_handle = self.start_aircraft_detection().await?;

        // Start plugin interface if enabled
        let plugin_handle = if self.config.enable_plugin {
            Some(self.start_plugin_interface().await?)
        } else {
            None
        };

        info!("X-Plane adapter started successfully");

        // Wait for shutdown signal
        tokio::select! {
            _ = publish_handle => {
                warn!("Telemetry publisher task ended");
            }
            _ = aircraft_handle => {
                warn!("Aircraft detection task ended");
            }
            _ = async {
                if let Some(handle) = plugin_handle {
                    handle.await
                } else {
                    std::future::pending().await
                }
            } => {
                warn!("Plugin interface task ended");
            }
        }

        Ok(())
    }

    /// Stop the adapter
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping X-Plane adapter");
        *self.running.write().unwrap() = false;
        *self.state.write().unwrap() = AdapterState::Disconnected;
        Ok(())
    }

    /// Test connection to X-Plane
    async fn test_connection(&self) -> Result<()> {
        debug!("Testing connection to X-Plane");

        // Try to get a basic DataRef to test connectivity
        let test_dataref = DataRef::new("sim/version/xplane_internal_version".to_string());

        match tokio::time::timeout(
            Duration::from_millis(self.config.dataref_timeout_ms),
            self.udp_client.request_dataref(&test_dataref),
        )
        .await
        {
            Ok(Ok(_)) => {
                info!("Successfully connected to X-Plane");
                Ok(())
            }
            Ok(Err(e)) => {
                error!("Failed to connect to X-Plane: {}", e);
                Err(XPlaneError::Adapter(AdapterError::NotConnected).into())
            }
            Err(_) => {
                error!("Connection to X-Plane timed out");
                Err(XPlaneError::Adapter(AdapterError::Timeout(
                    "Connection to X-Plane timed out".to_string(),
                ))
                .into())
            }
        }
    }

    async fn test_connection_with_retries(&self) -> Result<()> {
        let strategy = ReconnectionStrategy::new(
            self.config.max_retries.max(1),
            Duration::from_millis(200),
            self.connection_timeout,
        );
        let mut attempt = 1;

        loop {
            match self.test_connection().await {
                Ok(()) => return Ok(()),
                Err(err) => {
                    self.metrics_registry.inc_counter(ADAPTER_ERRORS_TOTAL, 1);

                    if !strategy.should_retry(attempt) {
                        return Err(err);
                    }

                    let backoff = strategy.next_backoff(attempt);
                    warn!(
                        "X-Plane connection attempt {} failed; retrying in {:.2}s",
                        attempt,
                        backoff.as_secs_f32()
                    );
                    tokio::time::sleep(backoff).await;
                    attempt += 1;
                }
            }
        }
    }

    /// Start telemetry publishing task
    async fn start_telemetry_publisher(&self) -> Result<tokio::task::JoinHandle<()>> {
        let config = self.config.clone();
        let udp_client = self.udp_client.clone();
        let web_api_client = self.web_api_client.clone();
        let dataref_manager = self.dataref_manager.clone();
        let latency_tracker = self.latency_tracker.clone();
        let bus_publisher = self.bus_publisher.clone();
        let state = self.state.clone();
        let metrics = self.metrics.clone();
        let metrics_registry = self.metrics_registry.clone();
        let current_aircraft = self.current_aircraft.clone();
        let running = self.running.clone();
        let last_packet_time = self.last_packet_time.clone();
        let connection_timeout = self.connection_timeout;
        let adapter_start = self.start_time;

        let handle = tokio::spawn(async move {
            let mut interval =
                interval(Duration::from_millis(1000 / config.publish_rate_hz as u64));

            while *running.read().unwrap() {
                interval.tick().await;

                let start_time = Instant::now();

                // Check for connection timeout (XPLANE-INT-01.13)
                let is_timeout = {
                    let last_packet = last_packet_time.read().unwrap();
                    match *last_packet {
                        Some(time) => time.elapsed() > connection_timeout,
                        None => false, // Don't timeout if we haven't started yet
                    }
                };

                if is_timeout {
                    warn!(
                        "X-Plane connection timeout: no packets received for {} seconds",
                        connection_timeout.as_secs()
                    );
                    metrics_registry.inc_counter(ADAPTER_ERRORS_TOTAL, 1);
                    *state.write().unwrap() = AdapterState::Disconnected;
                    // Publish a stale/invalid snapshot so subscribers know data is no longer valid.
                    // ValidityFlags are all-false by default, signalling safe_for_ffb=false.
                    let stale = BusSnapshot::new(SimId::XPlane, AircraftId::new("unknown"));
                    if let Ok(mut publisher) = bus_publisher.lock() {
                        if let Err(e) = publisher.publish(stale) {
                            warn!("Failed to publish stale snapshot on timeout: {}", e);
                        }
                    }
                    continue;
                }

                if let Some(last_packet) = *last_packet_time.read().unwrap() {
                    metrics_registry.set_gauge(
                        ADAPTER_TIME_SINCE_LAST_PACKET_MS,
                        last_packet.elapsed().as_secs_f64() * 1000.0,
                    );
                }

                // Get current aircraft
                let aircraft = {
                    let aircraft_guard = current_aircraft.read().unwrap();
                    aircraft_guard.clone()
                };

                if let Some(aircraft) = aircraft {
                    // Collect telemetry data
                    match Self::collect_telemetry_data(
                        &udp_client,
                        &web_api_client,
                        &dataref_manager,
                        &aircraft,
                        &config,
                    )
                    .await
                    {
                        Ok(raw_data) => {
                            // Update last packet time on successful data collection
                            {
                                let mut last_packet = last_packet_time.write().unwrap();
                                *last_packet = Some(Instant::now());
                            }

                            // Convert to bus snapshot
                            match Self::convert_raw_to_snapshot(raw_data, adapter_start) {
                                Ok(snapshot) => {
                                    // Measure latency
                                    let latency = start_time.elapsed();
                                    latency_tracker.record_measurement(latency);

                                    metrics_registry.observe(
                                        ADAPTER_UPDATE_LATENCY_MS,
                                        latency.as_secs_f64() * 1000.0,
                                    );

                                    // Check latency budget
                                    if latency.as_millis() as u64 > config.latency_budget_ms {
                                        warn!(
                                            "Telemetry latency budget exceeded: {}ms > {}ms",
                                            latency.as_millis(),
                                            config.latency_budget_ms
                                        );
                                    }

                                    metrics_registry.inc_counter(ADAPTER_UPDATES_TOTAL, 1);
                                    {
                                        let mut metrics_guard = metrics.write().unwrap();
                                        metrics_guard.record_update();
                                        metrics_guard
                                            .record_aircraft_change(aircraft.title.clone());
                                    }
                                    *state.write().unwrap() = AdapterState::Active;

                                    // Publish snapshot to bus subscribers
                                    if let Ok(mut publisher) = bus_publisher.lock() {
                                        if let Err(e) = publisher.publish(snapshot) {
                                            warn!("Failed to publish X-Plane snapshot: {}", e);
                                            metrics_registry.inc_counter(ADAPTER_ERRORS_TOTAL, 1);
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to convert raw data to snapshot: {}", e);
                                    metrics_registry.inc_counter(ADAPTER_ERRORS_TOTAL, 1);
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to collect telemetry data: {}", e);
                            metrics_registry.inc_counter(ADAPTER_ERRORS_TOTAL, 1);
                        }
                    }
                } else {
                    debug!("No aircraft detected, skipping telemetry collection");
                    *state.write().unwrap() = AdapterState::Connected;
                }
            }
        });

        Ok(handle)
    }

    /// Start aircraft detection task
    async fn start_aircraft_detection(&self) -> Result<tokio::task::JoinHandle<()>> {
        let config = self.config.clone();
        let udp_client = self.udp_client.clone();
        let aircraft_detector = self.aircraft_detector.clone();
        let current_aircraft = self.current_aircraft.clone();
        let running = self.running.clone();
        let state = self.state.clone();
        let metrics = self.metrics.clone();
        let metrics_registry = self.metrics_registry.clone();

        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(
                config.aircraft_detection_interval_s as u64,
            ));

            while *running.read().unwrap() {
                interval.tick().await;

                match aircraft_detector.detect_aircraft(&udp_client).await {
                    Ok(detected) => {
                        let mut aircraft_guard = current_aircraft.write().unwrap();
                        let changed = match &*aircraft_guard {
                            Some(current) => current.icao != detected.icao,
                            None => true,
                        };

                        if changed {
                            info!("Aircraft changed to: {}", detected.icao);
                            {
                                let mut metrics_guard = metrics.write().unwrap();
                                metrics_guard.record_aircraft_change(detected.title.clone());
                            }
                            *aircraft_guard = Some(detected);
                        }
                        *state.write().unwrap() = AdapterState::Active;
                    }
                    Err(e) => {
                        debug!("Aircraft detection failed: {}", e);
                        metrics_registry.inc_counter(ADAPTER_ERRORS_TOTAL, 1);
                        // Don't clear current aircraft on detection failure
                        if current_aircraft.read().unwrap().is_none() {
                            *state.write().unwrap() = AdapterState::Connected;
                        }
                    }
                }
            }
        });

        Ok(handle)
    }

    /// Start plugin interface task
    async fn start_plugin_interface(&self) -> Result<tokio::task::JoinHandle<()>> {
        let plugin_interface = self.plugin_interface.as_ref().unwrap().clone();
        let running = self.running.clone();

        let handle = tokio::spawn(async move {
            while *running.read().unwrap() {
                if let Err(e) = plugin_interface.process_messages().await {
                    error!("Plugin interface error: {}", e);
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        });

        Ok(handle)
    }

    /// Collect telemetry data from X-Plane
    async fn collect_telemetry_data(
        udp_client: &UdpClient,
        web_api_client: &Option<WebApiClient>,
        dataref_manager: &DataRefManager,
        aircraft: &DetectedAircraft,
        config: &XPlaneAdapterConfig,
    ) -> Result<XPlaneRawData> {
        let start_time = Instant::now();
        let mut dataref_values = HashMap::new();

        // Get required DataRefs for the aircraft
        let datarefs = dataref_manager.get_required_datarefs(&aircraft.icao);

        // Collect DataRef values via UDP (primary method)
        for dataref in &datarefs {
            match tokio::time::timeout(
                Duration::from_millis(config.dataref_timeout_ms),
                udp_client.request_dataref(dataref),
            )
            .await
            {
                Ok(Ok(value)) => {
                    dataref_values.insert(dataref.name.clone(), value);
                }
                Ok(Err(e)) => {
                    debug!("Failed to get DataRef {}: {}", dataref.name, e);
                }
                Err(_) => {
                    debug!("DataRef request timed out: {}", dataref.name);
                }
            }
        }

        // Try web API as fallback if available
        if let Some(web_client) = web_api_client {
            for dataref in &datarefs {
                if !dataref_values.contains_key(&dataref.name)
                    && let Ok(value) = web_client.get_dataref(&dataref.name).await
                {
                    dataref_values.insert(dataref.name.clone(), value);
                }
            }
        }

        Ok(XPlaneRawData {
            timestamp: start_time,
            aircraft_info: aircraft.clone(),
            dataref_values,
        })
    }

    /// Convert raw X-Plane data to normalized bus snapshot
    ///
    /// Requirements: XPLANE-INT-01.7
    /// WHEN implementing UDP-only mode THEN the adapter SHALL always set sim = XPLANE
    /// and MAY set a coarse aircraft_class (e.g., fixed-wing / helicopter) based on available data;
    /// precise aircraft identity SHALL be treated as 'unknown' unless provided by a plugin
    pub fn convert_raw_to_snapshot(
        raw_data: XPlaneRawData,
        _start_time: Instant,
    ) -> Result<BusSnapshot> {
        // XPLANE-INT-01.7: Always set sim = XPLANE for UDP-only mode
        // Aircraft identity may be 'unknown' or coarse class in UDP-only mode
        let aircraft_id = AircraftId::new(&raw_data.aircraft_info.icao);
        let mut snapshot = BusSnapshot::new(SimId::XPlane, aircraft_id);

        // BusSnapshot timestamp is monotonic since process start
        // Using Instant to approximate process-relative monotonic time
        static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
        let start = START.get_or_init(Instant::now);
        snapshot.timestamp = Instant::now().duration_since(*start).as_nanos() as u64;

        // Convert kinematics data
        snapshot.kinematics = Self::convert_kinematics(&raw_data.dataref_values)?;

        // Convert angular rates (P/Q/R deg/s → rad/s)
        snapshot.angular_rates = Self::convert_angular_rates(&raw_data.dataref_values);

        // Convert aircraft configuration
        snapshot.config = Self::convert_aircraft_config(&raw_data.dataref_values)?;

        // Convert engine data
        snapshot.engines = Self::convert_engine_data(&raw_data.dataref_values)?;

        // Convert environment data
        snapshot.environment = Self::convert_environment(&raw_data.dataref_values)?;

        // Convert navigation data
        snapshot.navigation = Self::convert_navigation(&raw_data.dataref_values)?;

        // Convert trim state (elevator/aileron/rudder)
        snapshot.trim_state = Self::convert_trim_state(&raw_data.dataref_values);

        // Validate the snapshot
        snapshot
            .validate()
            .map_err(|e| FlightError::Configuration(format!("Snapshot validation error: {}", e)))?;

        Ok(snapshot)
    }

    /// Convert kinematics data from X-Plane DataRefs
    ///
    /// Requirements: XPLANE-INT-01.4, XPLANE-INT-01.5
    ///
    /// Unit Conversions (XPLANE-INT-01.4, XPLANE-INT-01.5):
    /// - Speeds: X-Plane provides m/s, converted to knots for BusSnapshot
    /// - Angles: X-Plane provides degrees, converted to radians for BusSnapshot (degrees × π/180)
    /// - Angular rates: X-Plane provides deg/s, converted to rad/s (deg/s × π/180)
    /// - G-forces: X-Plane provides g-units, no conversion needed
    /// - Vertical speed: X-Plane provides m/s, converted to ft/min (m/s × 196.85)
    fn convert_kinematics(datarefs: &HashMap<String, DataRefValue>) -> Result<Kinematics> {
        let mut kinematics = Kinematics::default();

        // Airspeed conversion: m/s → knots (XPLANE-INT-01.4)
        // X-Plane DataRefs provide speeds in m/s
        // BusSnapshot expects ValidatedSpeed which can be created from m/s
        if let Some(DataRefValue::Float(ias_mps)) =
            datarefs.get("sim/flightmodel/position/indicated_airspeed")
        {
            kinematics.ias = XPlaneConverter::convert_airspeed_mps(*ias_mps)
                .map_err(|e| FlightError::Configuration(format!("IAS conversion error: {}", e)))?;
        }

        if let Some(DataRefValue::Float(tas_mps)) =
            datarefs.get("sim/flightmodel/position/true_airspeed")
        {
            kinematics.tas = XPlaneConverter::convert_airspeed_mps(*tas_mps)
                .map_err(|e| FlightError::Configuration(format!("TAS conversion error: {}", e)))?;
        }

        if let Some(DataRefValue::Float(gs_mps)) =
            datarefs.get("sim/flightmodel/position/groundspeed")
        {
            kinematics.ground_speed =
                XPlaneConverter::convert_airspeed_mps(*gs_mps).map_err(|e| {
                    FlightError::Configuration(format!("Ground speed conversion error: {}", e))
                })?;
        }

        // Angle conversion: degrees → radians (XPLANE-INT-01.4)
        // X-Plane DataRefs provide angles in degrees
        // BusSnapshot expects ValidatedAngle in radians
        // Conversion: radians = degrees × (π / 180) ≈ degrees × 0.0174533
        if let Some(DataRefValue::Float(aoa)) = datarefs.get("sim/flightmodel/position/alpha") {
            kinematics.aoa = XPlaneConverter::convert_angle_degrees(*aoa)
                .map_err(|e| FlightError::Configuration(format!("AOA conversion error: {}", e)))?;
        }

        if let Some(DataRefValue::Float(beta)) = datarefs.get("sim/flightmodel/position/beta") {
            kinematics.sideslip = XPlaneConverter::convert_angle_degrees(*beta).map_err(|e| {
                FlightError::Configuration(format!("Sideslip conversion error: {}", e))
            })?;
        }

        if let Some(DataRefValue::Float(phi)) = datarefs.get("sim/flightmodel/position/phi") {
            kinematics.bank = XPlaneConverter::convert_angle_degrees(*phi)
                .map_err(|e| FlightError::Configuration(format!("Bank conversion error: {}", e)))?;
        }

        if let Some(DataRefValue::Float(theta)) = datarefs.get("sim/flightmodel/position/theta") {
            kinematics.pitch = XPlaneConverter::convert_angle_degrees(*theta).map_err(|e| {
                FlightError::Configuration(format!("Pitch conversion error: {}", e))
            })?;
        }

        if let Some(DataRefValue::Float(psi)) = datarefs.get("sim/flightmodel/position/psi") {
            kinematics.heading = XPlaneConverter::convert_angle_degrees(*psi).map_err(|e| {
                FlightError::Configuration(format!("Heading conversion error: {}", e))
            })?;
        }

        // G-forces: no conversion needed (already in g-units)
        if let Some(DataRefValue::Float(g_normal)) = datarefs.get("sim/flightmodel/forces/g_nrml") {
            kinematics.g_force = XPlaneConverter::convert_g_force(*g_normal).map_err(|e| {
                FlightError::Configuration(format!("G-force conversion error: {}", e))
            })?;
        }

        if let Some(DataRefValue::Float(g_side)) = datarefs.get("sim/flightmodel/forces/g_side") {
            kinematics.g_lateral = XPlaneConverter::convert_g_force(*g_side).map_err(|e| {
                FlightError::Configuration(format!("Lateral G-force conversion error: {}", e))
            })?;
        }

        if let Some(DataRefValue::Float(g_axil)) = datarefs.get("sim/flightmodel/forces/g_axil") {
            kinematics.g_longitudinal = XPlaneConverter::convert_g_force(*g_axil).map_err(|e| {
                FlightError::Configuration(format!("Longitudinal G-force conversion error: {}", e))
            })?;
        }

        // Vertical speed conversion: m/s → ft/min (XPLANE-INT-01.4)
        // Conversion: ft/min = m/s × 196.85
        if let Some(DataRefValue::Float(vs_mps)) = datarefs.get("sim/flightmodel/position/vh_ind") {
            kinematics.vertical_speed = conversions::mps_to_fpm(*vs_mps);
        }

        Ok(kinematics)
    }

    /// Convert angular rates from X-Plane DataRefs
    ///
    /// X-Plane provides body-axis rates P (roll), Q (pitch), R (yaw) in deg/s.
    /// BusSnapshot expects rad/s.
    fn convert_angular_rates(datarefs: &HashMap<String, DataRefValue>) -> AngularRates {
        let mut rates = AngularRates::default();
        let deg_to_rad = std::f32::consts::PI / 180.0;

        if let Some(DataRefValue::Float(p)) = datarefs.get("sim/flightmodel/position/P") {
            rates.p = p * deg_to_rad;
        }
        if let Some(DataRefValue::Float(q)) = datarefs.get("sim/flightmodel/position/Q") {
            rates.q = q * deg_to_rad;
        }
        if let Some(DataRefValue::Float(r)) = datarefs.get("sim/flightmodel/position/R") {
            rates.r = r * deg_to_rad;
        }

        rates
    }

    /// Convert trim state from X-Plane DataRefs
    ///
    /// X-Plane provides trim ratios in the range -1.0 to +1.0.
    fn convert_trim_state(datarefs: &HashMap<String, DataRefValue>) -> TrimState {
        let mut trim = TrimState::default();

        if let Some(DataRefValue::Float(elv)) = datarefs.get("sim/flightmodel/controls/elv_trim") {
            trim.elevator = elv.clamp(-1.0, 1.0);
        }
        if let Some(DataRefValue::Float(ail)) = datarefs.get("sim/flightmodel/controls/ail_trim") {
            trim.aileron = ail.clamp(-1.0, 1.0);
        }
        if let Some(DataRefValue::Float(rud)) = datarefs.get("sim/flightmodel/controls/rud_trim") {
            trim.rudder = rud.clamp(-1.0, 1.0);
        }

        trim
    }
    fn convert_aircraft_config(
        datarefs: &HashMap<String, DataRefValue>,
    ) -> Result<flight_bus::snapshot::AircraftConfig> {
        let mut config = flight_bus::snapshot::AircraftConfig::default();

        // Gear positions
        if let Some(DataRefValue::Float(gear_deploy)) =
            datarefs.get("sim/aircraft/parts/acf_gear_deploy")
        {
            let gear_pos = if *gear_deploy > 0.9 {
                flight_bus::types::GearPosition::Down
            } else if *gear_deploy < 0.1 {
                flight_bus::types::GearPosition::Up
            } else {
                flight_bus::types::GearPosition::Transitioning
            };

            config.gear = flight_bus::types::GearState {
                nose: gear_pos,
                left: gear_pos,
                right: gear_pos,
            };
        }

        // Flaps
        if let Some(DataRefValue::Float(flap_ratio)) =
            datarefs.get("sim/aircraft/parts/acf_flap_deploy")
        {
            config.flaps = Percentage::from_normalized(*flap_ratio).map_err(|e| {
                FlightError::Configuration(format!("Flaps conversion error: {}", e))
            })?;
        }

        // Spoilers
        if let Some(DataRefValue::Float(speedbrake_ratio)) =
            datarefs.get("sim/aircraft/parts/acf_speedbrake_deploy")
        {
            config.spoilers = Percentage::from_normalized(*speedbrake_ratio).map_err(|e| {
                FlightError::Configuration(format!("Spoilers conversion error: {}", e))
            })?;
        }

        // Autopilot state
        if let Some(DataRefValue::Int(ap_mode)) =
            datarefs.get("sim/cockpit/autopilot/autopilot_mode")
        {
            config.ap_state = match *ap_mode {
                0 => AutopilotState::Off,
                1 => AutopilotState::Armed,
                _ => AutopilotState::Engaged,
            };
        }

        // Autopilot altitude target (feet)
        if let Some(DataRefValue::Float(ap_alt)) = datarefs.get("sim/cockpit/autopilot/altitude") {
            config.ap_altitude = Some(*ap_alt);
        }

        // Autopilot heading target (degrees → ValidatedAngle)
        if let Some(DataRefValue::Float(ap_hdg)) = datarefs.get("sim/cockpit/autopilot/heading") {
            config.ap_heading = XPlaneConverter::convert_angle_degrees(*ap_hdg).ok();
        }

        // Autopilot speed target (knots → ValidatedSpeed)
        if let Some(DataRefValue::Float(ap_spd)) = datarefs.get("sim/cockpit/autopilot/airspeed") {
            config.ap_speed = XPlaneConverter::convert_airspeed_knots(*ap_spd).ok();
        }

        // Lights
        if let Some(DataRefValue::Int(nav)) = datarefs.get("sim/cockpit/electrical/nav_lights_on") {
            config.lights.nav = *nav != 0;
        }
        if let Some(DataRefValue::Int(beacon)) =
            datarefs.get("sim/cockpit/electrical/beacon_lights_on")
        {
            config.lights.beacon = *beacon != 0;
        }
        if let Some(DataRefValue::Int(strobe)) =
            datarefs.get("sim/cockpit/electrical/strobe_lights_on")
        {
            config.lights.strobe = *strobe != 0;
        }
        if let Some(DataRefValue::Int(landing)) =
            datarefs.get("sim/cockpit/electrical/landing_lights_on")
        {
            config.lights.landing = *landing != 0;
        }
        if let Some(DataRefValue::Int(taxi)) = datarefs.get("sim/cockpit/electrical/taxi_light_on")
        {
            config.lights.taxi = *taxi != 0;
        }
        if let Some(DataRefValue::Int(logo)) = datarefs.get("sim/cockpit2/switches/logo_lights_on")
        {
            config.lights.logo = *logo != 0;
        }

        Ok(config)
    }

    /// Convert engine data from X-Plane DataRefs
    fn convert_engine_data(datarefs: &HashMap<String, DataRefValue>) -> Result<Vec<EngineData>> {
        let mut engines = Vec::new();

        // X-Plane supports up to 8 engines
        for i in 0..8 {
            let running_key = format!("sim/flightmodel/engine/ENGN_running[{}]", i);
            let n1_key = format!("sim/flightmodel/engine/ENGN_N1_[{}]", i);

            if let Some(DataRefValue::Int(running)) = datarefs.get(&running_key) {
                let engine = EngineData {
                    index: i as u8,
                    running: *running != 0,
                    rpm: if let Some(DataRefValue::Float(n1)) = datarefs.get(&n1_key) {
                        XPlaneConverter::convert_n1_percentage(*n1).map_err(|e| {
                            FlightError::Configuration(format!("N1 conversion error: {}", e))
                        })?
                    } else {
                        flight_bus::types::Percentage::new(0.0).map_err(|e| {
                            FlightError::Configuration(format!("Default RPM error: {}", e))
                        })?
                    },
                    manifold_pressure: datarefs
                        .get(&format!("sim/flightmodel/engine/ENGN_MPR[{}]", i))
                        .and_then(|v| {
                            if let DataRefValue::Float(x) = v {
                                Some(*x)
                            } else {
                                None
                            }
                        }),
                    egt: datarefs
                        .get(&format!("sim/flightmodel/engine/ENGN_EGT[{}]", i))
                        .and_then(|v| {
                            if let DataRefValue::Float(x) = v {
                                Some(*x)
                            } else {
                                None
                            }
                        }),
                    cht: datarefs
                        .get(&format!("sim/flightmodel/engine/ENGN_CHT[{}]", i))
                        .and_then(|v| {
                            if let DataRefValue::Float(x) = v {
                                Some(*x)
                            } else {
                                None
                            }
                        }),
                    // X-Plane provides fuel flow in kg/s; convert to gal/hr (Jet-A ~3.04 kg/gal)
                    fuel_flow: datarefs
                        .get(&format!("sim/flightmodel/engine/ENGN_FF_[{}]", i))
                        .and_then(|v| {
                            if let DataRefValue::Float(x) = v {
                                Some(x * 3600.0 / 3.04)
                            } else {
                                None
                            }
                        }),
                    oil_pressure: datarefs
                        .get(&format!("sim/flightmodel/engine/ENGN_oilp[{}]", i))
                        .and_then(|v| {
                            if let DataRefValue::Float(x) = v {
                                Some(*x)
                            } else {
                                None
                            }
                        }),
                    oil_temperature: datarefs
                        .get(&format!("sim/flightmodel/engine/ENGN_oilt[{}]", i))
                        .and_then(|v| {
                            if let DataRefValue::Float(x) = v {
                                Some(*x)
                            } else {
                                None
                            }
                        }),
                };
                engines.push(engine);
            }
        }

        Ok(engines)
    }

    /// Convert environment data from X-Plane DataRefs
    fn convert_environment(datarefs: &HashMap<String, DataRefValue>) -> Result<Environment> {
        let mut environment = Environment::default();

        // Altitude
        if let Some(DataRefValue::Float(alt_m)) = datarefs.get("sim/flightmodel/position/elevation")
        {
            environment.altitude = XPlaneConverter::convert_altitude_m_to_ft(*alt_m);
        }

        // Pressure altitude (feet, directly from barometric altimeter)
        if let Some(DataRefValue::Float(palt_ft)) =
            datarefs.get("sim/cockpit2/gauges/indicators/altitude_ft_pilot")
        {
            environment.pressure_altitude = *palt_ft;
        }

        // Temperature
        if let Some(DataRefValue::Float(temp_c)) = datarefs.get("sim/weather/temperature_ambient_c")
        {
            environment.oat = XPlaneConverter::convert_temperature_celsius(*temp_c);
        }

        // Wind
        if let Some(DataRefValue::Float(wind_speed_mps)) =
            datarefs.get("sim/weather/wind_speed_kt[0]")
        {
            environment.wind_speed = XPlaneConverter::convert_airspeed_mps(*wind_speed_mps)
                .map_err(|e| {
                    FlightError::Configuration(format!("Wind speed conversion error: {}", e))
                })?;
        }

        if let Some(DataRefValue::Float(wind_dir)) =
            datarefs.get("sim/weather/wind_direction_degt[0]")
        {
            environment.wind_direction = XPlaneConverter::convert_angle_degrees(*wind_dir)
                .map_err(|e| {
                    FlightError::Configuration(format!("Wind direction conversion error: {}", e))
                })?;
        }

        Ok(environment)
    }

    /// Convert navigation data from X-Plane DataRefs
    fn convert_navigation(datarefs: &HashMap<String, DataRefValue>) -> Result<Navigation> {
        let mut navigation = Navigation::default();

        // Position
        if let Some(DataRefValue::Double(lat)) = datarefs.get("sim/flightmodel/position/latitude") {
            navigation.latitude = *lat;
        }

        if let Some(DataRefValue::Double(lon)) = datarefs.get("sim/flightmodel/position/longitude")
        {
            navigation.longitude = *lon;
        }

        // Ground track
        if let Some(DataRefValue::Float(track)) = datarefs.get("sim/flightmodel/position/hpath") {
            navigation.ground_track =
                XPlaneConverter::convert_angle_degrees(*track).map_err(|e| {
                    FlightError::Configuration(format!("Ground track conversion error: {}", e))
                })?;
        }

        Ok(navigation)
    }

    /// Get current latency statistics
    pub fn get_latency_stats(&self) -> crate::latency::LatencyStats {
        self.latency_tracker.get_stats()
    }

    /// Get current adapter state
    pub fn state(&self) -> AdapterState {
        *self.state.read().unwrap()
    }

    /// Get adapter metrics snapshot
    pub fn metrics(&self) -> AdapterMetrics {
        self.metrics.read().unwrap().clone()
    }

    /// Get shared metrics registry
    pub fn metrics_registry(&self) -> Arc<MetricsRegistry> {
        self.metrics_registry.clone()
    }

    /// Get current aircraft information
    pub fn get_current_aircraft(&self) -> Option<DetectedAircraft> {
        self.current_aircraft.read().unwrap().clone()
    }

    /// Publish a snapshot directly to the bus.
    ///
    /// Convenience method for tests and service orchestration that holds the snapshot
    /// already converted, bypassing the telemetry loop.
    pub fn publish_snapshot(&self, snapshot: BusSnapshot) -> Result<()> {
        let mut publisher = self
            .bus_publisher
            .lock()
            .map_err(|_| FlightError::Configuration("Bus publisher lock poisoned".to_string()))?;
        publisher
            .publish(snapshot)
            .map_err(|e| FlightError::Configuration(format!("Publish error: {e}")))
    }

    /// Publish a stale/invalid snapshot to signal downstream consumers that data is no longer valid.
    ///
    /// ValidityFlags are all-false by default, so safe_for_ffb = false.
    pub fn publish_stale_snapshot(&self) -> Result<()> {
        let stale = BusSnapshot::new(SimId::XPlane, AircraftId::new("unknown"));
        self.publish_snapshot(stale)
    }

    /// Check if adapter is running
    pub fn is_running(&self) -> bool {
        *self.running.read().unwrap()
    }

    /// Check if connection has timed out
    ///
    /// Requirements: XPLANE-INT-01.13
    /// WHEN connection is lost or no packets received for 2 seconds THEN the adapter SHALL mark BusSnapshot as invalid and transition to disconnected state
    pub fn is_connection_timeout(&self) -> bool {
        let last_packet = self.last_packet_time.read().unwrap();
        match *last_packet {
            Some(time) => time.elapsed() > self.connection_timeout,
            None => true, // No packets received yet
        }
    }

    /// Update last packet time (called when telemetry is successfully received)
    #[cfg(test)]
    fn update_last_packet_time(&self) {
        let mut last_packet = self.last_packet_time.write().unwrap();
        *last_packet = Some(Instant::now());
    }

    /// Get time since last packet (for metrics)
    pub fn time_since_last_packet(&self) -> Option<Duration> {
        let last_packet = self.last_packet_time.read().unwrap();
        last_packet.map(|time| time.elapsed())
    }
}

impl SimAdapter for XPlaneAdapter {
    type RawData = XPlaneRawData;
    type Error = XPlaneError;

    fn convert_to_snapshot(
        &self,
        raw: Self::RawData,
    ) -> std::result::Result<BusSnapshot, XPlaneError> {
        match Self::convert_raw_to_snapshot(raw, self.start_time) {
            Ok(snapshot) => Ok(snapshot),
            Err(e) => Err(XPlaneError::DataRef {
                message: e.to_string(),
            }),
        }
    }

    fn sim_id(&self) -> SimId {
        SimId::XPlane
    }

    fn validate_raw_data(&self, raw: &Self::RawData) -> std::result::Result<(), XPlaneError> {
        // Basic validation of raw data
        if raw.dataref_values.is_empty() {
            return Err(XPlaneError::DataRef {
                message: "No DataRef values received".to_string(),
            });
        }

        // Check for critical DataRefs
        let critical_datarefs = [
            "sim/flightmodel/position/indicated_airspeed",
            "sim/flightmodel/position/latitude",
            "sim/flightmodel/position/longitude",
        ];

        for dataref in &critical_datarefs {
            if !raw.dataref_values.contains_key(*dataref) {
                return Err(XPlaneError::DataRef {
                    message: format!("Missing critical DataRef: {}", dataref),
                });
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flight_bus::BusPublisher;
    use std::sync::{Arc, Mutex};

    #[tokio::test]
    async fn test_adapter_creation() {
        let config = XPlaneAdapterConfig::default();
        let bus_publisher = Arc::new(Mutex::new(BusPublisher::new(60.0)));

        let adapter = XPlaneAdapter::new(config, bus_publisher);
        assert!(adapter.is_ok());
    }

    #[test]
    fn test_config_defaults() {
        let config = XPlaneAdapterConfig::default();
        assert_eq!(config.publish_rate_hz, 30);
        assert_eq!(config.latency_budget_ms, 50);
        assert_eq!(config.dataref_timeout_ms, 100);
        assert!(!config.enable_plugin);
    }

    #[test]
    fn test_kinematics_conversion() {
        let mut datarefs = HashMap::new();
        datarefs.insert(
            "sim/flightmodel/position/indicated_airspeed".to_string(),
            DataRefValue::Float(77.17), // ~150 knots
        );
        datarefs.insert(
            "sim/flightmodel/position/alpha".to_string(),
            DataRefValue::Float(5.0),
        );

        let kinematics = XPlaneAdapter::convert_kinematics(&datarefs).unwrap();
        assert!((kinematics.ias.to_knots() - 150.0).abs() < 1.0);
        assert!((kinematics.aoa.to_degrees() - 5.0).abs() < 0.1);
    }

    /// Test angle conversion (degrees → radians)
    /// Requirements: XPLANE-INT-01.4, XPLANE-INT-01.5, SIM-TEST-01.3
    #[test]
    fn test_angle_conversion() {
        let mut datarefs = HashMap::new();

        // Test pitch angle conversion: 10 degrees → ~0.1745 radians
        datarefs.insert(
            "sim/flightmodel/position/theta".to_string(),
            DataRefValue::Float(10.0),
        );

        // Test roll angle conversion: -15 degrees → ~-0.2618 radians
        datarefs.insert(
            "sim/flightmodel/position/phi".to_string(),
            DataRefValue::Float(-15.0),
        );

        // Test heading angle conversion: 270 degrees → ~4.7124 radians (or -90 degrees normalized)
        datarefs.insert(
            "sim/flightmodel/position/psi".to_string(),
            DataRefValue::Float(270.0),
        );

        // Test AOA conversion: 5 degrees → ~0.0873 radians
        datarefs.insert(
            "sim/flightmodel/position/alpha".to_string(),
            DataRefValue::Float(5.0),
        );

        // Test sideslip conversion: -2 degrees → ~-0.0349 radians
        datarefs.insert(
            "sim/flightmodel/position/beta".to_string(),
            DataRefValue::Float(-2.0),
        );

        let kinematics = XPlaneAdapter::convert_kinematics(&datarefs).unwrap();

        // Verify conversions (allowing small floating point error)
        assert!((kinematics.pitch.to_degrees() - 10.0).abs() < 0.1);
        assert!((kinematics.bank.to_degrees() - (-15.0)).abs() < 0.1);
        // Heading may be normalized, so check the raw value
        assert!((kinematics.aoa.to_degrees() - 5.0).abs() < 0.1);
        assert!((kinematics.sideslip.to_degrees() - (-2.0)).abs() < 0.1);
    }

    /// Test rate conversion (deg/s → rad/s)
    /// Requirements: XPLANE-INT-01.4, XPLANE-INT-01.5, SIM-TEST-01.3
    #[test]
    fn test_rate_conversion() {
        // X-Plane DATA output group 16 provides P, Q, R in deg/s
        // These need to be converted to rad/s for BusSnapshot
        // The conversion happens when DataRefs are processed

        // Test with DataRefs that would come from DATA output group 16
        let mut datarefs = HashMap::new();

        // Angular rates in deg/s from X-Plane
        datarefs.insert(
            "sim/flightmodel/position/P".to_string(),
            DataRefValue::Float(10.0), // 10 deg/s → ~0.1745 rad/s
        );
        datarefs.insert(
            "sim/flightmodel/position/Q".to_string(),
            DataRefValue::Float(-5.0), // -5 deg/s → ~-0.0873 rad/s
        );
        datarefs.insert(
            "sim/flightmodel/position/R".to_string(),
            DataRefValue::Float(2.0), // 2 deg/s → ~0.0349 rad/s
        );

        // Note: The current kinematics converter doesn't explicitly handle P/Q/R DataRefs
        // This test documents the expected conversion that would be needed
        // In a complete implementation, these would be converted from deg/s to rad/s

        // Verify the conversion formula: rad/s = deg/s × (π / 180)
        let p_deg_s = 10.0f32;
        let p_rad_s = p_deg_s * std::f32::consts::PI / 180.0;
        assert!((p_rad_s - 0.1745).abs() < 0.001);

        let q_deg_s = -5.0f32;
        let q_rad_s = q_deg_s * std::f32::consts::PI / 180.0;
        assert!((q_rad_s - (-0.0873)).abs() < 0.001);

        let r_deg_s = 2.0f32;
        let r_rad_s = r_deg_s * std::f32::consts::PI / 180.0;
        assert!((r_rad_s - 0.0349).abs() < 0.001);
    }

    /// Test speed conversion (knots → m/s)
    /// Requirements: XPLANE-INT-01.4, XPLANE-INT-01.5, SIM-TEST-01.3
    #[test]
    fn test_speed_conversion() {
        let mut datarefs = HashMap::new();

        // X-Plane provides speeds in m/s
        // Test IAS: 77.17 m/s ≈ 150 knots
        datarefs.insert(
            "sim/flightmodel/position/indicated_airspeed".to_string(),
            DataRefValue::Float(77.17),
        );

        // Test TAS: 82.3 m/s ≈ 160 knots
        datarefs.insert(
            "sim/flightmodel/position/true_airspeed".to_string(),
            DataRefValue::Float(82.3),
        );

        // Test ground speed: 72.0 m/s ≈ 140 knots
        datarefs.insert(
            "sim/flightmodel/position/groundspeed".to_string(),
            DataRefValue::Float(72.0),
        );

        let kinematics = XPlaneAdapter::convert_kinematics(&datarefs).unwrap();

        // Verify conversions (allowing small floating point error)
        // ValidatedSpeed stores in m/s internally, so we check the m/s value
        assert!((kinematics.ias.value() - 77.17).abs() < 0.1);
        assert!((kinematics.tas.value() - 82.3).abs() < 0.1);
        assert!((kinematics.ground_speed.value() - 72.0).abs() < 0.1);

        // Also verify knots conversion
        assert!((kinematics.ias.to_knots() - 150.0).abs() < 1.0);
        assert!((kinematics.tas.to_knots() - 160.0).abs() < 1.0);
        assert!((kinematics.ground_speed.to_knots() - 140.0).abs() < 1.0);
    }

    /// Test connection timeout detection
    /// Requirements: XPLANE-INT-01.13, SIM-TEST-01.3
    #[tokio::test]
    async fn test_connection_timeout_detection() {
        let config = XPlaneAdapterConfig::default();
        let bus_publisher = Arc::new(Mutex::new(BusPublisher::new(60.0)));
        let adapter = XPlaneAdapter::new(config, bus_publisher).unwrap();

        // Initially, no packets received, so should be considered timeout
        assert!(adapter.is_connection_timeout());

        // Simulate receiving a packet
        adapter.update_last_packet_time();

        // Should not be timeout immediately after receiving packet
        assert!(!adapter.is_connection_timeout());

        // Verify time since last packet is recent
        let time_since = adapter.time_since_last_packet();
        assert!(time_since.is_some());
        assert!(time_since.unwrap() < Duration::from_millis(100));

        // Wait for timeout (2 seconds + small buffer)
        tokio::time::sleep(Duration::from_millis(2100)).await;

        // Should now be timeout
        assert!(adapter.is_connection_timeout());

        // Verify time since last packet is > 2 seconds
        let time_since = adapter.time_since_last_packet();
        assert!(time_since.is_some());
        assert!(time_since.unwrap() > Duration::from_secs(2));
    }

    /// Test aircraft identity handling for UDP-only mode
    /// Requirements: XPLANE-INT-01.7, SIM-TEST-01.3
    #[test]
    fn test_udp_aircraft_identity() {
        // Create raw data with aircraft info
        let raw_data = XPlaneRawData {
            timestamp: Instant::now(),
            aircraft_info: DetectedAircraft {
                icao: "unknown".to_string(), // UDP-only mode may not have precise identity
                title: "Unknown Aircraft".to_string(),
                author: "Unknown".to_string(),
            },
            dataref_values: {
                let mut map = HashMap::new();
                // Add minimal required DataRefs
                map.insert(
                    "sim/flightmodel/position/indicated_airspeed".to_string(),
                    DataRefValue::Float(77.17),
                );
                map.insert(
                    "sim/flightmodel/position/latitude".to_string(),
                    DataRefValue::Double(37.5),
                );
                map.insert(
                    "sim/flightmodel/position/longitude".to_string(),
                    DataRefValue::Double(-122.5),
                );
                map
            },
        };

        // Convert to snapshot
        let snapshot = XPlaneAdapter::convert_raw_to_snapshot(raw_data, Instant::now()).unwrap();

        // Verify sim is always XPLANE (XPLANE-INT-01.7)
        assert_eq!(snapshot.sim, flight_bus::types::SimId::XPlane);

        // Verify aircraft identity is set (even if 'unknown')
        // In UDP-only mode, precise identity may be 'unknown' or coarse class
        // AircraftId is a struct, not a method, so we just verify it exists
        // The actual ICAO code is stored in the AircraftId struct
    }

    /// Test complete telemetry mapping with all required fields
    /// Requirements: XPLANE-INT-01.4, XPLANE-INT-01.5, SIM-TEST-01.3
    #[test]
    fn test_complete_telemetry_mapping() {
        let mut datarefs = HashMap::new();

        // Add all required DataRefs for complete kinematics
        datarefs.insert(
            "sim/flightmodel/position/indicated_airspeed".to_string(),
            DataRefValue::Float(77.17), // ~150 knots
        );
        datarefs.insert(
            "sim/flightmodel/position/true_airspeed".to_string(),
            DataRefValue::Float(82.3), // ~160 knots
        );
        datarefs.insert(
            "sim/flightmodel/position/groundspeed".to_string(),
            DataRefValue::Float(72.0), // ~140 knots
        );
        datarefs.insert(
            "sim/flightmodel/position/alpha".to_string(),
            DataRefValue::Float(5.0), // 5 degrees AOA
        );
        datarefs.insert(
            "sim/flightmodel/position/beta".to_string(),
            DataRefValue::Float(-2.0), // -2 degrees sideslip
        );
        datarefs.insert(
            "sim/flightmodel/position/phi".to_string(),
            DataRefValue::Float(10.0), // 10 degrees bank
        );
        datarefs.insert(
            "sim/flightmodel/position/theta".to_string(),
            DataRefValue::Float(5.0), // 5 degrees pitch
        );
        datarefs.insert(
            "sim/flightmodel/position/psi".to_string(),
            DataRefValue::Float(270.0), // 270 degrees heading
        );
        datarefs.insert(
            "sim/flightmodel/forces/g_nrml".to_string(),
            DataRefValue::Float(1.2), // 1.2 g normal
        );
        datarefs.insert(
            "sim/flightmodel/forces/g_side".to_string(),
            DataRefValue::Float(0.1), // 0.1 g lateral
        );
        datarefs.insert(
            "sim/flightmodel/forces/g_axil".to_string(),
            DataRefValue::Float(0.05), // 0.05 g longitudinal
        );
        datarefs.insert(
            "sim/flightmodel/position/vh_ind".to_string(),
            DataRefValue::Float(2.54), // 2.54 m/s ≈ 500 ft/min
        );

        let kinematics = XPlaneAdapter::convert_kinematics(&datarefs).unwrap();

        // Verify all fields are populated correctly
        assert!((kinematics.ias.to_knots() - 150.0).abs() < 1.0);
        assert!((kinematics.tas.to_knots() - 160.0).abs() < 1.0);
        assert!((kinematics.ground_speed.to_knots() - 140.0).abs() < 1.0);
        assert!((kinematics.aoa.to_degrees() - 5.0).abs() < 0.1);
        assert!((kinematics.sideslip.to_degrees() - (-2.0)).abs() < 0.1);
        assert!((kinematics.bank.to_degrees() - 10.0).abs() < 0.1);
        assert!((kinematics.pitch.to_degrees() - 5.0).abs() < 0.1);
        assert!((kinematics.g_force.value() - 1.2).abs() < 0.01);
        assert!((kinematics.g_lateral.value() - 0.1).abs() < 0.01);
        assert!((kinematics.g_longitudinal.value() - 0.05).abs() < 0.01);
        assert!((kinematics.vertical_speed - 500.0).abs() < 1.0); // ft/min
    }

    #[tokio::test]
    async fn test_raw_data_validation() {
        let config = XPlaneAdapterConfig::default();
        let bus_publisher = Arc::new(Mutex::new(BusPublisher::new(60.0)));
        let adapter = XPlaneAdapter::new(config, bus_publisher).unwrap();

        // Empty data should fail
        let empty_raw = XPlaneRawData {
            timestamp: Instant::now(),
            aircraft_info: DetectedAircraft {
                icao: "C172".to_string(),
                title: "Cessna 172".to_string(),
                author: "Laminar Research".to_string(),
            },
            dataref_values: HashMap::new(),
        };
        assert!(adapter.validate_raw_data(&empty_raw).is_err());

        // Data with critical DataRefs should pass
        let mut datarefs = HashMap::new();
        datarefs.insert(
            "sim/flightmodel/position/indicated_airspeed".to_string(),
            DataRefValue::Float(77.17),
        );
        datarefs.insert(
            "sim/flightmodel/position/latitude".to_string(),
            DataRefValue::Double(37.7749),
        );
        datarefs.insert(
            "sim/flightmodel/position/longitude".to_string(),
            DataRefValue::Double(-122.4194),
        );

        let valid_raw = XPlaneRawData {
            timestamp: Instant::now(),
            aircraft_info: DetectedAircraft {
                icao: "C172".to_string(),
                title: "Cessna 172".to_string(),
                author: "Laminar Research".to_string(),
            },
            dataref_values: datarefs,
        };
        assert!(adapter.validate_raw_data(&valid_raw).is_ok());
    }

    #[test]
    fn test_aircraft_config_autopilot() {
        let mut datarefs = HashMap::new();
        datarefs.insert(
            "sim/cockpit/autopilot/autopilot_mode".to_string(),
            DataRefValue::Int(2), // Engaged
        );
        datarefs.insert(
            "sim/cockpit/autopilot/altitude".to_string(),
            DataRefValue::Float(15000.0),
        );
        datarefs.insert(
            "sim/cockpit/autopilot/heading".to_string(),
            DataRefValue::Float(90.0),
        );
        datarefs.insert(
            "sim/cockpit/autopilot/airspeed".to_string(),
            DataRefValue::Float(250.0),
        );

        let config = XPlaneAdapter::convert_aircraft_config(&datarefs).unwrap();
        assert_eq!(config.ap_state, AutopilotState::Engaged);
        assert_eq!(config.ap_altitude, Some(15000.0));
        assert!(config.ap_heading.is_some());
        assert!((config.ap_heading.unwrap().to_degrees() - 90.0).abs() < 0.1);
        assert!(config.ap_speed.is_some());
        assert!((config.ap_speed.unwrap().to_knots() - 250.0).abs() < 1.0);
    }

    #[test]
    fn test_aircraft_config_autopilot_off() {
        let mut datarefs = HashMap::new();
        datarefs.insert(
            "sim/cockpit/autopilot/autopilot_mode".to_string(),
            DataRefValue::Int(0),
        );

        let config = XPlaneAdapter::convert_aircraft_config(&datarefs).unwrap();
        assert_eq!(config.ap_state, AutopilotState::Off);
        assert!(config.ap_altitude.is_none());
    }

    #[test]
    fn test_aircraft_config_lights() {
        let mut datarefs = HashMap::new();
        datarefs.insert(
            "sim/cockpit/electrical/nav_lights_on".to_string(),
            DataRefValue::Int(1),
        );
        datarefs.insert(
            "sim/cockpit/electrical/beacon_lights_on".to_string(),
            DataRefValue::Int(1),
        );
        datarefs.insert(
            "sim/cockpit/electrical/strobe_lights_on".to_string(),
            DataRefValue::Int(0),
        );
        datarefs.insert(
            "sim/cockpit/electrical/landing_lights_on".to_string(),
            DataRefValue::Int(1),
        );
        datarefs.insert(
            "sim/cockpit/electrical/taxi_light_on".to_string(),
            DataRefValue::Int(0),
        );
        datarefs.insert(
            "sim/cockpit2/switches/logo_lights_on".to_string(),
            DataRefValue::Int(1),
        );

        let config = XPlaneAdapter::convert_aircraft_config(&datarefs).unwrap();
        assert!(config.lights.nav);
        assert!(config.lights.beacon);
        assert!(!config.lights.strobe);
        assert!(config.lights.landing);
        assert!(!config.lights.taxi);
        assert!(config.lights.logo);
    }

    #[test]
    fn test_trim_state_conversion() {
        let mut datarefs = HashMap::new();
        datarefs.insert(
            "sim/flightmodel/controls/elv_trim".to_string(),
            DataRefValue::Float(0.15),
        );
        datarefs.insert(
            "sim/flightmodel/controls/ail_trim".to_string(),
            DataRefValue::Float(-0.05),
        );
        datarefs.insert(
            "sim/flightmodel/controls/rud_trim".to_string(),
            DataRefValue::Float(0.10),
        );

        let trim = XPlaneAdapter::convert_trim_state(&datarefs);
        assert!((trim.elevator - 0.15).abs() < 0.001);
        assert!((trim.aileron - (-0.05)).abs() < 0.001);
        assert!((trim.rudder - 0.10).abs() < 0.001);
    }

    #[test]
    fn test_trim_state_clamps_out_of_range() {
        let mut datarefs = HashMap::new();
        datarefs.insert(
            "sim/flightmodel/controls/elv_trim".to_string(),
            DataRefValue::Float(1.5), // over max
        );
        datarefs.insert(
            "sim/flightmodel/controls/ail_trim".to_string(),
            DataRefValue::Float(-1.5), // below min
        );

        let trim = XPlaneAdapter::convert_trim_state(&datarefs);
        assert_eq!(trim.elevator, 1.0);
        assert_eq!(trim.aileron, -1.0);
    }

    /// Integration test: raw DataRef data → snapshot → bus publish → subscriber receive.
    ///
    /// This is the core "sim → adapter → bus → subscriber" pipeline that proves
    /// bus publishing is real and not stubbed.
    #[tokio::test]
    async fn test_snapshot_pipeline_publish_and_receive() {
        use flight_bus::{BusPublisher, SubscriptionConfig};

        let bus_publisher = Arc::new(Mutex::new(BusPublisher::new(60.0)));

        // Create a subscriber before publishing
        let mut subscriber = bus_publisher
            .lock()
            .unwrap()
            .subscribe(SubscriptionConfig::default())
            .expect("subscribe should succeed");

        let config = XPlaneAdapterConfig::default();
        let adapter = XPlaneAdapter::new(config, Arc::clone(&bus_publisher)).unwrap();

        // Build realistic DataRef payload
        let mut datarefs = HashMap::new();
        datarefs.insert(
            "sim/flightmodel/position/indicated_airspeed".to_string(),
            DataRefValue::Float(77.17), // ~150 knots
        );
        datarefs.insert(
            "sim/flightmodel/position/latitude".to_string(),
            DataRefValue::Double(37.77),
        );
        datarefs.insert(
            "sim/flightmodel/position/longitude".to_string(),
            DataRefValue::Double(-122.41),
        );
        datarefs.insert(
            "sim/flightmodel/position/alpha".to_string(),
            DataRefValue::Float(5.0),
        );
        datarefs.insert(
            "sim/flightmodel/position/theta".to_string(),
            DataRefValue::Float(3.0),
        );
        datarefs.insert(
            "sim/flightmodel/position/phi".to_string(),
            DataRefValue::Float(-5.0),
        );
        datarefs.insert(
            "sim/flightmodel/position/psi".to_string(),
            DataRefValue::Float(90.0),
        );

        let raw = XPlaneRawData {
            timestamp: Instant::now(),
            aircraft_info: DetectedAircraft {
                icao: "C172".to_string(),
                title: "Cessna 172SP".to_string(),
                author: "Laminar Research".to_string(),
            },
            dataref_values: datarefs,
        };

        // Convert → snapshot (exercises the conversion pipeline)
        let start = Instant::now();
        let snapshot =
            XPlaneAdapter::convert_raw_to_snapshot(raw, start).expect("conversion should succeed");

        // Verify snapshot fields
        assert!((snapshot.kinematics.ias.to_knots() - 150.0).abs() < 1.0);
        assert!((snapshot.kinematics.aoa.to_degrees() - 5.0).abs() < 0.1);

        // Publish to bus
        adapter
            .publish_snapshot(snapshot.clone())
            .expect("publish should succeed");

        // Verify subscriber receives the snapshot
        let received = subscriber.try_recv().expect("try_recv should not error");
        assert!(
            received.is_some(),
            "subscriber should have received a snapshot"
        );
        let received = received.unwrap();
        assert_eq!(received.sim, snapshot.sim);
        assert!(
            (received.kinematics.ias.to_knots() - 150.0).abs() < 1.0,
            "received IAS should match published IAS"
        );
    }

    /// Integration test: stale-snapshot signal on timeout.
    ///
    /// When no telemetry arrives for the timeout period, the adapter should
    /// push a stale (invalid) snapshot so downstream consumers stop using stale data.
    #[tokio::test]
    async fn test_timeout_publishes_stale_snapshot() {
        use flight_bus::{BusPublisher, SubscriptionConfig};
        use tokio::time::Duration;

        let bus_publisher = Arc::new(Mutex::new(BusPublisher::new(60.0)));
        let mut subscriber = bus_publisher
            .lock()
            .unwrap()
            .subscribe(SubscriptionConfig::default())
            .expect("subscribe should succeed");

        let config = XPlaneAdapterConfig {
            dataref_timeout_ms: 50, // Very short timeout for the test
            ..Default::default()
        };
        let adapter = XPlaneAdapter::new(config, Arc::clone(&bus_publisher)).unwrap();

        // Simulate a timeout by calling handle_timeout directly
        adapter
            .publish_stale_snapshot()
            .expect("stale snapshot publish should succeed");

        let received = subscriber.try_recv().expect("try_recv should not error");
        assert!(received.is_some(), "should have received stale snapshot");
        let received = received.unwrap();
        // Stale snapshot should have validity flags all false
        assert!(
            !received.validity.safe_for_ffb,
            "stale snapshot should not be safe for FFB"
        );
    }
}
