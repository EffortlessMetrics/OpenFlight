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
use flight_bus::{
    adapters::{xplane::XPlaneConverter, SimAdapter},
    snapshot::{BusSnapshot, EngineData, Environment, Kinematics, Navigation},
    types::{AircraftId, SimId},
    BusPublisher,
};
use flight_core::{FlightError, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
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
    bus_publisher: Arc<BusPublisher>,
    current_aircraft: Arc<RwLock<Option<DetectedAircraft>>>,
    running: Arc<RwLock<bool>>,
}

impl XPlaneAdapter {
    /// Create a new X-Plane adapter
    pub fn new(config: XPlaneAdapterConfig, bus_publisher: Arc<BusPublisher>) -> Result<Self> {
        let udp_client = UdpClient::new(config.udp.clone())
            .map_err(|e| FlightError::Configuration(format!("UDP client error: {}", e)))?;
        
        let web_api_client = if let Some(web_config) = &config.web_api {
            Some(WebApiClient::new(web_config.clone())
                .map_err(|e| FlightError::Configuration(format!("Web API client error: {}", e)))?)
        } else {
            None
        };

        let plugin_interface = if config.enable_plugin {
            Some(PluginInterface::new()
                .map_err(|e| FlightError::Configuration(format!("Plugin interface error: {}", e)))?)
        } else {
            None
        };

        let dataref_manager = DataRefManager::new();
        let aircraft_detector = AircraftDetector::new();
        let latency_tracker = LatencyTracker::new(LatencyBudget::new(
            Duration::from_millis(config.latency_budget_ms),
        ));

        Ok(Self {
            config,
            udp_client,
            web_api_client,
            plugin_interface,
            dataref_manager,
            aircraft_detector,
            latency_tracker,
            bus_publisher,
            current_aircraft: Arc::new(RwLock::new(None)),
            running: Arc::new(RwLock::new(false)),
        })
    }

    /// Start the adapter
    pub async fn start(&self) -> Result<()> {
        info!("Starting X-Plane adapter");
        
        // Set running flag
        *self.running.write().unwrap() = true;

        // Test connection to X-Plane
        self.test_connection().await?;

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
                Err(XPlaneError::NotRunning.into())
            }
            Err(_) => {
                error!("Connection to X-Plane timed out");
                Err(XPlaneError::Timeout.into())
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
        let _bus_publisher = self.bus_publisher.clone();
        let current_aircraft = self.current_aircraft.clone();
        let running = self.running.clone();

        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(1000 / config.publish_rate_hz as u64));
            
            while *running.read().unwrap() {
                interval.tick().await;
                
                let start_time = Instant::now();
                
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
                            // Convert to bus snapshot
                            match Self::convert_raw_to_snapshot(raw_data) {
                                Ok(snapshot) => {
                                    // Measure latency
                                    let latency = start_time.elapsed();
                                    latency_tracker.record_measurement(latency);
                                    
                                    // Check latency budget
                                    if latency.as_millis() as u64 > config.latency_budget_ms {
                                        warn!(
                                            "Telemetry latency budget exceeded: {}ms > {}ms",
                                            latency.as_millis(),
                                            config.latency_budget_ms
                                        );
                                    }

                                    // Publish to bus
                                    // Note: In a real implementation, we would need to handle the mutable reference properly
                                    // For now, we'll skip the actual publishing in this simplified version
                                    debug!("Would publish snapshot: {:?}", snapshot.sim);
                                }
                                Err(e) => {
                                    error!("Failed to convert raw data to snapshot: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to collect telemetry data: {}", e);
                        }
                    }
                } else {
                    debug!("No aircraft detected, skipping telemetry collection");
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

        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(config.aircraft_detection_interval_s as u64));
            
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
                            *aircraft_guard = Some(detected);
                        }
                    }
                    Err(e) => {
                        debug!("Aircraft detection failed: {}", e);
                        // Don't clear current aircraft on detection failure
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
                if !dataref_values.contains_key(&dataref.name) {
                    if let Ok(value) = web_client.get_dataref(&dataref.name).await {
                        dataref_values.insert(dataref.name.clone(), value);
                    }
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
    fn convert_raw_to_snapshot(raw_data: XPlaneRawData) -> Result<BusSnapshot> {
        let aircraft_id = AircraftId::new(&raw_data.aircraft_info.icao);
        let mut snapshot = BusSnapshot::new(SimId::XPlane, aircraft_id);

        // Update timestamp
        snapshot.timestamp = raw_data.timestamp.elapsed().as_nanos() as u64;

        // Convert kinematics data
        snapshot.kinematics = Self::convert_kinematics(&raw_data.dataref_values)?;

        // Convert aircraft configuration
        snapshot.config = Self::convert_aircraft_config(&raw_data.dataref_values)?;

        // Convert engine data
        snapshot.engines = Self::convert_engine_data(&raw_data.dataref_values)?;

        // Convert environment data
        snapshot.environment = Self::convert_environment(&raw_data.dataref_values)?;

        // Convert navigation data
        snapshot.navigation = Self::convert_navigation(&raw_data.dataref_values)?;

        // Validate the snapshot
        snapshot.validate()
            .map_err(|e| FlightError::Configuration(format!("Snapshot validation error: {}", e)))?;

        Ok(snapshot)
    }

    /// Convert kinematics data from X-Plane DataRefs
    fn convert_kinematics(datarefs: &HashMap<String, DataRefValue>) -> Result<Kinematics> {
        let mut kinematics = Kinematics::default();

        // Airspeed (m/s to knots conversion)
        if let Some(DataRefValue::Float(ias_mps)) = datarefs.get("sim/flightmodel/position/indicated_airspeed") {
            kinematics.ias = XPlaneConverter::convert_airspeed_mps(*ias_mps)
                .map_err(|e| FlightError::Configuration(format!("IAS conversion error: {}", e)))?;
        }

        if let Some(DataRefValue::Float(tas_mps)) = datarefs.get("sim/flightmodel/position/true_airspeed") {
            kinematics.tas = XPlaneConverter::convert_airspeed_mps(*tas_mps)
                .map_err(|e| FlightError::Configuration(format!("TAS conversion error: {}", e)))?;
        }

        if let Some(DataRefValue::Float(gs_mps)) = datarefs.get("sim/flightmodel/position/groundspeed") {
            kinematics.ground_speed = XPlaneConverter::convert_airspeed_mps(*gs_mps)
                .map_err(|e| FlightError::Configuration(format!("Ground speed conversion error: {}", e)))?;
        }

        // Angles
        if let Some(DataRefValue::Float(aoa)) = datarefs.get("sim/flightmodel/position/alpha") {
            kinematics.aoa = XPlaneConverter::convert_angle_degrees(*aoa)
                .map_err(|e| FlightError::Configuration(format!("AOA conversion error: {}", e)))?;
        }

        if let Some(DataRefValue::Float(beta)) = datarefs.get("sim/flightmodel/position/beta") {
            kinematics.sideslip = XPlaneConverter::convert_angle_degrees(*beta)
                .map_err(|e| FlightError::Configuration(format!("Sideslip conversion error: {}", e)))?;
        }

        if let Some(DataRefValue::Float(phi)) = datarefs.get("sim/flightmodel/position/phi") {
            kinematics.bank = XPlaneConverter::convert_angle_degrees(*phi)
                .map_err(|e| FlightError::Configuration(format!("Bank conversion error: {}", e)))?;
        }

        if let Some(DataRefValue::Float(theta)) = datarefs.get("sim/flightmodel/position/theta") {
            kinematics.pitch = XPlaneConverter::convert_angle_degrees(*theta)
                .map_err(|e| FlightError::Configuration(format!("Pitch conversion error: {}", e)))?;
        }

        if let Some(DataRefValue::Float(psi)) = datarefs.get("sim/flightmodel/position/psi") {
            kinematics.heading = XPlaneConverter::convert_angle_degrees(*psi)
                .map_err(|e| FlightError::Configuration(format!("Heading conversion error: {}", e)))?;
        }

        // G-forces
        if let Some(DataRefValue::Float(g_normal)) = datarefs.get("sim/flightmodel/forces/g_nrml") {
            kinematics.g_force = XPlaneConverter::convert_g_force(*g_normal)
                .map_err(|e| FlightError::Configuration(format!("G-force conversion error: {}", e)))?;
        }

        if let Some(DataRefValue::Float(g_side)) = datarefs.get("sim/flightmodel/forces/g_side") {
            kinematics.g_lateral = XPlaneConverter::convert_g_force(*g_side)
                .map_err(|e| FlightError::Configuration(format!("Lateral G-force conversion error: {}", e)))?;
        }

        if let Some(DataRefValue::Float(g_axil)) = datarefs.get("sim/flightmodel/forces/g_axil") {
            kinematics.g_longitudinal = XPlaneConverter::convert_g_force(*g_axil)
                .map_err(|e| FlightError::Configuration(format!("Longitudinal G-force conversion error: {}", e)))?;
        }

        // Vertical speed (m/s to ft/min)
        if let Some(DataRefValue::Float(vs_mps)) = datarefs.get("sim/flightmodel/position/vh_ind") {
            kinematics.vertical_speed = vs_mps * 196.85; // m/s to ft/min
        }

        Ok(kinematics)
    }

    /// Convert aircraft configuration from X-Plane DataRefs
    fn convert_aircraft_config(datarefs: &HashMap<String, DataRefValue>) -> Result<flight_bus::snapshot::AircraftConfig> {
        let mut config = flight_bus::snapshot::AircraftConfig::default();

        // Gear positions
        if let Some(DataRefValue::Float(gear_deploy)) = datarefs.get("sim/aircraft/parts/acf_gear_deploy") {
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
        if let Some(DataRefValue::Float(flap_ratio)) = datarefs.get("sim/aircraft/parts/acf_flap_deploy") {
            config.flaps = XPlaneConverter::convert_ratio_to_percentage(*flap_ratio)
                .map_err(|e| FlightError::Configuration(format!("Flaps conversion error: {}", e)))?;
        }

        // Spoilers
        if let Some(DataRefValue::Float(speedbrake_ratio)) = datarefs.get("sim/aircraft/parts/acf_speedbrake_deploy") {
            config.spoilers = XPlaneConverter::convert_ratio_to_percentage(*speedbrake_ratio)
                .map_err(|e| FlightError::Configuration(format!("Spoilers conversion error: {}", e)))?;
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
                        XPlaneConverter::convert_n1_percentage(*n1)
                            .map_err(|e| FlightError::Configuration(format!("N1 conversion error: {}", e)))?
                    } else {
                        flight_bus::types::Percentage::new(0.0)
                            .map_err(|e| FlightError::Configuration(format!("Default RPM error: {}", e)))?
                    },
                    manifold_pressure: None, // TODO: Add if available
                    egt: None,               // TODO: Add if available
                    cht: None,               // TODO: Add if available
                    fuel_flow: None,         // TODO: Add if available
                    oil_pressure: None,      // TODO: Add if available
                    oil_temperature: None,   // TODO: Add if available
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
        if let Some(DataRefValue::Float(alt_m)) = datarefs.get("sim/flightmodel/position/elevation") {
            environment.altitude = XPlaneConverter::convert_altitude_m_to_ft(*alt_m);
        }

        // Temperature
        if let Some(DataRefValue::Float(temp_c)) = datarefs.get("sim/weather/temperature_ambient_c") {
            environment.oat = XPlaneConverter::convert_temperature_celsius(*temp_c);
        }

        // Wind
        if let Some(DataRefValue::Float(wind_speed_mps)) = datarefs.get("sim/weather/wind_speed_kt[0]") {
            environment.wind_speed = XPlaneConverter::convert_airspeed_mps(*wind_speed_mps)
                .map_err(|e| FlightError::Configuration(format!("Wind speed conversion error: {}", e)))?;
        }

        if let Some(DataRefValue::Float(wind_dir)) = datarefs.get("sim/weather/wind_direction_degt[0]") {
            environment.wind_direction = XPlaneConverter::convert_angle_degrees(*wind_dir)
                .map_err(|e| FlightError::Configuration(format!("Wind direction conversion error: {}", e)))?;
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

        if let Some(DataRefValue::Double(lon)) = datarefs.get("sim/flightmodel/position/longitude") {
            navigation.longitude = *lon;
        }

        // Ground track
        if let Some(DataRefValue::Float(track)) = datarefs.get("sim/flightmodel/position/hpath") {
            navigation.ground_track = XPlaneConverter::convert_angle_degrees(*track)
                .map_err(|e| FlightError::Configuration(format!("Ground track conversion error: {}", e)))?;
        }

        Ok(navigation)
    }

    /// Get current latency statistics
    pub fn get_latency_stats(&self) -> crate::latency::LatencyStats {
        self.latency_tracker.get_stats()
    }

    /// Get current aircraft information
    pub fn get_current_aircraft(&self) -> Option<DetectedAircraft> {
        self.current_aircraft.read().unwrap().clone()
    }

    /// Check if adapter is running
    pub fn is_running(&self) -> bool {
        *self.running.read().unwrap()
    }
}

impl SimAdapter for XPlaneAdapter {
    type RawData = XPlaneRawData;
    type Error = XPlaneError;

    fn convert_to_snapshot(&self, raw: Self::RawData) -> std::result::Result<BusSnapshot, XPlaneError> {
        match Self::convert_raw_to_snapshot(raw) {
            Ok(snapshot) => Ok(snapshot),
            Err(e) => Err(XPlaneError::DataRef {
                message: e.to_string(),
            })
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
    use std::sync::Arc;
    

    #[tokio::test]
    async fn test_adapter_creation() {
        let config = XPlaneAdapterConfig::default();
        let bus_publisher = Arc::new(BusPublisher::new(60.0));
        
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

    #[tokio::test]
    async fn test_raw_data_validation() {
        let config = XPlaneAdapterConfig::default();
        let bus_publisher = Arc::new(BusPublisher::new(60.0));
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
}