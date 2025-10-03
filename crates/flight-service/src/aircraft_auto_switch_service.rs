// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Aircraft Auto-Switch Service
//!
//! Integrates process detection, aircraft detection, and profile switching
//! to provide seamless aircraft auto-switching with ≤500ms response time.

use flight_core::{
    AircraftAutoSwitch, AutoSwitchConfig, DetectedAircraft, ProcessDetector, 
    ProcessDetectionConfig, DetectedProcess, PhaseOfFlight, SwitchMetrics, Result, FlightError
};
use flight_bus::{BusSnapshot, BusPublisher, Subscriber, SubscriberId, SimId, AircraftId};
use flight_simconnect::{AircraftDetector as MsfsAircraftDetector, AircraftInfo as MsfsAircraftInfo};
use flight_xplane::{AircraftDetector as XPlaneAircraftDetector, DetectedAircraft as XPlaneDetectedAircraft};
use flight_dcs_export::{DcsAdapter};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn, error};

/// Aircraft auto-switch service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AircraftAutoSwitchServiceConfig {
    /// Auto-switch system configuration
    pub auto_switch: AutoSwitchConfig,
    /// Process detection configuration
    pub process_detection: ProcessDetectionConfig,
    /// Bus subscription configuration
    pub bus_subscription: BusSubscriptionConfig,
    /// Adapter configurations
    pub adapters: AdapterConfigs,
}

/// Bus subscription configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusSubscriptionConfig {
    /// Telemetry update rate for PoF tracking (Hz)
    pub telemetry_rate: f32,
    /// Buffer size for telemetry updates
    pub buffer_size: usize,
}

/// Adapter configurations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterConfigs {
    /// Enable MSFS adapter
    pub enable_msfs: bool,
    /// Enable X-Plane adapter
    pub enable_xplane: bool,
    /// Enable DCS adapter
    pub enable_dcs: bool,
}

/// Aircraft auto-switch service
pub struct AircraftAutoSwitchService {
    config: AircraftAutoSwitchServiceConfig,
    auto_switch: Arc<AircraftAutoSwitch>,
    process_detector: Arc<ProcessDetector>,
    adapters: Arc<RwLock<SimAdapters>>,
    bus_subscriber: Arc<RwLock<Option<Box<dyn Subscriber + Send + Sync>>>>,
    service_tx: mpsc::UnboundedSender<ServiceEvent>,
    service_rx: Arc<RwLock<Option<mpsc::UnboundedReceiver<ServiceEvent>>>>,
}

/// Simulator adapters
struct SimAdapters {
    msfs: Option<MsfsAdapter>,
    xplane: Option<XPlaneAdapter>,
    dcs: Option<DcsAdapter>,
}

/// MSFS adapter wrapper
struct MsfsAdapter {
    detector: MsfsAircraftDetector,
    current_aircraft: Option<MsfsAircraftInfo>,
}

/// X-Plane adapter wrapper
struct XPlaneAdapter {
    detector: XPlaneAircraftDetector,
    current_aircraft: Option<XPlaneDetectedAircraft>,
}

/// DCS adapter wrapper
struct DcsAdapter {
    adapter: flight_dcs_export::DcsAdapter,
    current_aircraft: Option<AircraftId>,
}

/// Service event for internal processing
#[derive(Debug)]
enum ServiceEvent {
    ProcessDetected(DetectedProcess),
    ProcessLost(SimId),
    AircraftDetected(SimId, AircraftId),
    TelemetryUpdate(BusSnapshot),
    AdapterError(SimId, String),
    Shutdown,
}

/// Service metrics
#[derive(Debug, Clone)]
pub struct ServiceMetrics {
    pub auto_switch_metrics: SwitchMetrics,
    pub process_detection_metrics: flight_core::DetectionMetrics,
    pub adapter_metrics: HashMap<SimId, AdapterMetrics>,
    pub total_aircraft_switches: u64,
    pub average_detection_time: Duration,
}

/// Adapter-specific metrics
#[derive(Debug, Clone)]
pub struct AdapterMetrics {
    pub aircraft_detections: u64,
    pub detection_errors: u64,
    pub last_detection: Option<Instant>,
    pub average_detection_time: Duration,
}

impl Default for AircraftAutoSwitchServiceConfig {
    fn default() -> Self {
        Self {
            auto_switch: AutoSwitchConfig::default(),
            process_detection: ProcessDetectionConfig::default(),
            bus_subscription: BusSubscriptionConfig {
                telemetry_rate: 30.0, // 30 Hz for PoF tracking
                buffer_size: 100,
            },
            adapters: AdapterConfigs {
                enable_msfs: true,
                enable_xplane: true,
                enable_dcs: true,
            },
        }
    }
}

impl AircraftAutoSwitchService {
    /// Create new aircraft auto-switch service
    pub fn new(config: AircraftAutoSwitchServiceConfig) -> Self {
        let auto_switch = Arc::new(AircraftAutoSwitch::new(config.auto_switch.clone()));
        let process_detector = Arc::new(ProcessDetector::new(config.process_detection.clone()));
        let (service_tx, service_rx) = mpsc::unbounded_channel();

        Self {
            config,
            auto_switch,
            process_detector,
            adapters: Arc::new(RwLock::new(SimAdapters::new())),
            bus_subscriber: Arc::new(RwLock::new(None)),
            service_tx,
            service_rx: Arc::new(RwLock::new(Some(service_rx))),
        }
    }

    /// Start the aircraft auto-switch service
    pub async fn start(&self, bus_publisher: Arc<BusPublisher>) -> Result<()> {
        // Start auto-switch system
        self.auto_switch.start().await?;

        // Start process detector
        self.process_detector.start().await?;

        // Subscribe to bus for telemetry updates
        let subscriber_id = SubscriberId::new("aircraft_auto_switch");
        let subscriber = bus_publisher.subscribe(subscriber_id, Default::default()).await
            .map_err(|e| FlightError::AutoSwitch(format!("Failed to subscribe to bus: {}", e)))?;
        
        *self.bus_subscriber.write().await = Some(subscriber);

        // Start service event loop
        let mut rx = self.service_rx.write().await.take()
            .ok_or_else(|| FlightError::AutoSwitch("Service already started".to_string()))?;

        let auto_switch = Arc::clone(&self.auto_switch);
        let process_detector = Arc::clone(&self.process_detector);
        let adapters = Arc::clone(&self.adapters);
        let config = self.config.clone();

        tokio::spawn(async move {
            info!("Aircraft auto-switch service started");

            while let Some(event) = rx.recv().await {
                match event {
                    ServiceEvent::ProcessDetected(process) => {
                        if let Err(e) = Self::handle_process_detected(
                            process,
                            &adapters,
                            &config,
                        ).await {
                            error!("Failed to handle process detection: {}", e);
                        }
                    }
                    ServiceEvent::ProcessLost(sim) => {
                        if let Err(e) = Self::handle_process_lost(sim, &adapters).await {
                            error!("Failed to handle process loss: {}", e);
                        }
                    }
                    ServiceEvent::AircraftDetected(sim, aircraft_id) => {
                        let detected_aircraft = DetectedAircraft {
                            sim,
                            aircraft_id,
                            process_name: format!("{}_process", sim),
                            detection_time: Instant::now(),
                            confidence: 0.9,
                        };

                        if let Err(e) = auto_switch.on_aircraft_detected(detected_aircraft).await {
                            error!("Failed to handle aircraft detection: {}", e);
                        }
                    }
                    ServiceEvent::TelemetryUpdate(snapshot) => {
                        if let Err(e) = auto_switch.on_telemetry_update(snapshot).await {
                            error!("Failed to handle telemetry update: {}", e);
                        }
                    }
                    ServiceEvent::AdapterError(sim, error) => {
                        warn!("Adapter error for {}: {}", sim, error);
                    }
                    ServiceEvent::Shutdown => {
                        info!("Aircraft auto-switch service shutting down");
                        break;
                    }
                }
            }

            info!("Aircraft auto-switch service stopped");
        });

        // Start monitoring process detection
        self.start_process_monitoring().await?;

        // Start monitoring bus updates
        self.start_bus_monitoring().await?;

        Ok(())
    }

    /// Stop the aircraft auto-switch service
    pub async fn stop(&self) -> Result<()> {
        // Stop process detector
        self.process_detector.stop().await?;

        // Stop adapters
        let mut adapters = self.adapters.write().await;
        adapters.stop_all().await?;

        // Send shutdown event
        self.service_tx.send(ServiceEvent::Shutdown)
            .map_err(|e| FlightError::AutoSwitch(format!("Failed to send shutdown: {}", e)))?;

        Ok(())
    }

    /// Get current service metrics
    pub async fn get_metrics(&self) -> ServiceMetrics {
        let auto_switch_metrics = self.auto_switch.get_metrics().await;
        let process_detection_metrics = self.process_detector.get_metrics().await;
        
        // TODO: Collect adapter metrics
        let adapter_metrics = HashMap::new();

        ServiceMetrics {
            auto_switch_metrics,
            process_detection_metrics,
            adapter_metrics,
            total_aircraft_switches: 0, // TODO: Track this
            average_detection_time: Duration::from_millis(0), // TODO: Calculate this
        }
    }

    /// Get current aircraft
    pub async fn get_current_aircraft(&self) -> Option<DetectedAircraft> {
        self.auto_switch.get_current_aircraft().await
    }

    /// Get current phase of flight
    pub async fn get_current_pof(&self) -> Option<PhaseOfFlight> {
        self.auto_switch.get_current_pof().await
    }

    /// Force switch to specific aircraft
    pub async fn force_switch(&self, aircraft_id: AircraftId) -> Result<()> {
        self.auto_switch.force_switch(aircraft_id).await
    }

    /// Start monitoring process detection
    async fn start_process_monitoring(&self) -> Result<()> {
        let process_detector = Arc::clone(&self.process_detector);
        let service_tx = self.service_tx.clone();

        tokio::spawn(async move {
            let mut last_processes = HashMap::new();

            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;

                match process_detector.get_detected_processes().await {
                    current_processes => {
                        // Check for new processes
                        for (sim, process) in &current_processes {
                            if !last_processes.contains_key(sim) {
                                let _ = service_tx.send(ServiceEvent::ProcessDetected(process.clone()));
                            }
                        }

                        // Check for lost processes
                        for sim in last_processes.keys() {
                            if !current_processes.contains_key(sim) {
                                let _ = service_tx.send(ServiceEvent::ProcessLost(*sim));
                            }
                        }

                        last_processes = current_processes;
                    }
                }
            }
        });

        Ok(())
    }

    /// Start monitoring bus updates
    async fn start_bus_monitoring(&self) -> Result<()> {
        let service_tx = self.service_tx.clone();
        let telemetry_rate = self.config.bus_subscription.telemetry_rate;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs_f32(1.0 / telemetry_rate));

            loop {
                interval.tick().await;

                // TODO: Get latest bus snapshot and send telemetry update
                // This would require integration with the actual bus subscriber
                // For now, we'll skip this implementation detail
            }
        });

        Ok(())
    }

    /// Handle process detected event
    async fn handle_process_detected(
        process: DetectedProcess,
        adapters: &Arc<RwLock<SimAdapters>>,
        config: &AircraftAutoSwitchServiceConfig,
    ) -> Result<()> {
        info!("Starting adapter for detected process: {} ({})", process.process_name, process.sim);

        let mut adapters_guard = adapters.write().await;

        match process.sim {
            SimId::Msfs if config.adapters.enable_msfs => {
                if adapters_guard.msfs.is_none() {
                    let mut detector = MsfsAircraftDetector::new();
                    // TODO: Setup and start MSFS aircraft detection
                    adapters_guard.msfs = Some(MsfsAdapter {
                        detector,
                        current_aircraft: None,
                    });
                }
            }
            SimId::XPlane if config.adapters.enable_xplane => {
                if adapters_guard.xplane.is_none() {
                    let detector = XPlaneAircraftDetector::new();
                    // TODO: Setup and start X-Plane aircraft detection
                    adapters_guard.xplane = Some(XPlaneAdapter {
                        detector,
                        current_aircraft: None,
                    });
                }
            }
            SimId::Dcs if config.adapters.enable_dcs => {
                if adapters_guard.dcs.is_none() {
                    let adapter = flight_dcs_export::DcsAdapter::new(Default::default());
                    // TODO: Setup and start DCS aircraft detection
                    adapters_guard.dcs = Some(DcsAdapter {
                        adapter,
                        current_aircraft: None,
                    });
                }
            }
            _ => {
                debug!("Adapter not enabled or supported for sim: {}", process.sim);
            }
        }

        Ok(())
    }

    /// Handle process lost event
    async fn handle_process_lost(
        sim: SimId,
        adapters: &Arc<RwLock<SimAdapters>>,
    ) -> Result<()> {
        info!("Stopping adapter for lost process: {}", sim);

        let mut adapters_guard = adapters.write().await;

        match sim {
            SimId::Msfs => {
                adapters_guard.msfs = None;
            }
            SimId::XPlane => {
                adapters_guard.xplane = None;
            }
            SimId::Dcs => {
                adapters_guard.dcs = None;
            }
            _ => {}
        }

        Ok(())
    }
}

impl SimAdapters {
    fn new() -> Self {
        Self {
            msfs: None,
            xplane: None,
            dcs: None,
        }
    }

    async fn stop_all(&mut self) -> Result<()> {
        // Stop all adapters
        self.msfs = None;
        self.xplane = None;
        self.dcs = None;
        Ok(())
    }
}

impl Default for AdapterMetrics {
    fn default() -> Self {
        Self {
            aircraft_detections: 0,
            detection_errors: 0,
            last_detection: None,
            average_detection_time: Duration::from_millis(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flight_bus::{BusPublisher, PublisherError};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_service_creation() {
        let config = AircraftAutoSwitchServiceConfig::default();
        let service = AircraftAutoSwitchService::new(config);
        
        assert!(service.get_current_aircraft().await.is_none());
        assert!(service.get_current_pof().await.is_none());
    }

    #[tokio::test]
    async fn test_service_configuration() {
        let mut config = AircraftAutoSwitchServiceConfig::default();
        config.adapters.enable_msfs = false;
        config.adapters.enable_xplane = true;
        config.adapters.enable_dcs = false;

        let service = AircraftAutoSwitchService::new(config);
        assert!(!service.config.adapters.enable_msfs);
        assert!(service.config.adapters.enable_xplane);
        assert!(!service.config.adapters.enable_dcs);
    }

    #[tokio::test]
    async fn test_force_switch() {
        let config = AircraftAutoSwitchServiceConfig::default();
        let service = AircraftAutoSwitchService::new(config);
        
        let aircraft_id = AircraftId::new("C172");
        
        // This should not fail even without starting the service
        // (it will just queue the request)
        assert!(service.force_switch(aircraft_id).await.is_ok());
    }
}