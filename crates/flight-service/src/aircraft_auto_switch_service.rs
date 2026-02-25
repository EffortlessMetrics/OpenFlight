// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Aircraft Auto-Switch Service
//!
//! Integrates process detection, aircraft detection, and profile switching
//! to provide seamless aircraft auto-switching with ≤500ms response time.

use flight_bus::{BusPublisher, BusSnapshot, Subscriber as BusSubscriber, SubscriptionConfig};
use flight_core::{
    AircraftAutoSwitch, AutoSwitchConfig, DetectedAircraft, DetectedProcess, FlightError,
    PhaseOfFlight, ProcessDetectionConfig, ProcessDetector, Result, SessionError, SwitchMetrics,
};
// Import bus and core types with aliases to avoid conflicts
use flight_bus::{AircraftId as BusAircraftId, SimId as BusSimId};
use flight_core::aircraft_switch::{
    AircraftId as CoreAircraftId, SimId as CoreSimId, TelemetrySnapshot,
};
#[cfg(windows)]
use flight_simconnect::{
    AircraftDetector as MsfsAircraftDetector, AircraftInfo as MsfsAircraftInfo,
};
use flight_xplane::{
    AircraftDetector as XPlaneAircraftDetector, DetectedAircraft as XPlaneDetectedAircraft,
};
// Avoid type-name collision with local stub
use flight_ac7_telemetry::{Ac7TelemetryAdapter as Ac7AdapterApi, Ac7TelemetryConfig};
use flight_dcs_export::DcsAdapter as DcsAdapterApi;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, mpsc, watch};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

// ============================================================================
// Type Mapping Functions (Bus ↔ Core)
// ============================================================================
// These private functions convert between flight-bus and flight-core types
// to handle API drift. They avoid orphan rule violations by not using trait impls.

/// Map flight-bus SimId to flight-core SimId
fn map_sim_id(sim: BusSimId) -> CoreSimId {
    match sim {
        BusSimId::Msfs => CoreSimId::Msfs,
        BusSimId::Msfs2024 => CoreSimId::Msfs2024,
        BusSimId::XPlane => CoreSimId::XPlane,
        BusSimId::Dcs => CoreSimId::Dcs,
        BusSimId::AceCombat7 => CoreSimId::AceCombat7,
        BusSimId::WarThunder => CoreSimId::WarThunder,
        BusSimId::EliteDangerous => CoreSimId::EliteDangerous,
        BusSimId::Ksp => CoreSimId::Ksp,
        BusSimId::Wingman => CoreSimId::Wingman,
        BusSimId::Unknown => CoreSimId::Unknown,
    }
}

/// Map flight-bus AircraftId to flight-core AircraftId
fn map_aircraft_id(id: BusAircraftId) -> CoreAircraftId {
    CoreAircraftId {
        icao: id.icao,
        variant: id.variant,
    }
}

/// Map BusSnapshot to TelemetrySnapshot for auto-switch system
///
/// This creates a minimal snapshot with only the fields needed for
/// phase-of-flight determination and aircraft switching logic.
fn map_snapshot(bus: &BusSnapshot) -> TelemetrySnapshot {
    TelemetrySnapshot {
        sim: map_sim_id(bus.sim),
        aircraft: map_aircraft_id(bus.aircraft.clone()),
        timestamp: bus.timestamp,
        ias_knots: bus.kinematics.ias.to_knots(),
        ground_speed_knots: bus.kinematics.ground_speed.to_knots(),
        altitude_feet: bus.environment.altitude,
        vertical_speed_fpm: bus.kinematics.vertical_speed,
        gear_down: bus.config.gear.all_down(),
    }
}

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
    /// Enable Ace Combat 7 adapter
    pub enable_ac7: bool,
    /// Enable Project Wingman adapter
    pub enable_wingman: bool,
}

/// Aircraft auto-switch service
pub struct AircraftAutoSwitchService {
    config: AircraftAutoSwitchServiceConfig,
    auto_switch: Arc<AircraftAutoSwitch>,
    process_detector: Arc<ProcessDetector>,
    adapters: Arc<RwLock<SimAdapters>>,
    bus_subscriber: Arc<RwLock<Option<BusSubscriber>>>,
    service_tx: mpsc::UnboundedSender<ServiceEvent>,
    service_rx: Arc<RwLock<Option<mpsc::UnboundedReceiver<ServiceEvent>>>>,
    /// Per-adapter detection/error counters, updated by the event loop.
    adapter_metrics: Arc<RwLock<HashMap<BusSimId, AdapterMetrics>>>,
}

/// Simulator adapters
struct SimAdapters {
    msfs: Option<MsfsAdapter>,
    xplane: Option<XPlaneAdapter>,
    dcs: Option<DcsAdapter>,
    ac7: Option<Ac7Adapter>,
    wingman: Option<WingmanWrapper>,
}

/// MSFS adapter wrapper
#[cfg(windows)]
struct MsfsAdapter {
    #[allow(dead_code)]
    detector: MsfsAircraftDetector,
    #[allow(dead_code)]
    current_aircraft: Option<MsfsAircraftInfo>,
}

#[cfg(not(windows))]
struct MsfsAdapter;

/// X-Plane adapter wrapper
struct XPlaneAdapter {
    #[allow(dead_code)]
    detector: XPlaneAircraftDetector,
    #[allow(dead_code)]
    current_aircraft: Option<XPlaneDetectedAircraft>,
}

/// DCS adapter wrapper
struct DcsAdapter {
    #[allow(dead_code)]
    adapter: DcsAdapterApi,
    #[allow(dead_code)]
    current_aircraft: Option<BusAircraftId>,
}

/// Ace Combat 7 adapter wrapper
struct Ac7Adapter {
    shutdown_tx: watch::Sender<bool>,
    join_handle: JoinHandle<()>,
}

impl Ac7Adapter {
    async fn stop(self) {
        let _ = self.shutdown_tx.send(true);
        let _ = self.join_handle.await;
    }
}

/// Project Wingman adapter wrapper (no telemetry API; tracks process detection only).
struct WingmanWrapper {
    #[allow(dead_code)]
    process_name: String,
}

/// Service event for internal processing
#[derive(Debug)]
#[allow(dead_code)]
#[allow(clippy::large_enum_variant)]
enum ServiceEvent {
    ProcessDetected(DetectedProcess),
    ProcessLost(BusSimId),
    AircraftDetected(BusSimId, BusAircraftId),
    TelemetryUpdate(BusSnapshot),
    AdapterError(BusSimId, String),
    Shutdown,
}

/// Service metrics
#[derive(Debug, Clone)]
pub struct ServiceMetrics {
    pub auto_switch_metrics: SwitchMetrics,
    pub process_detection_metrics: flight_core::DetectionMetrics,
    pub adapter_metrics: HashMap<BusSimId, AdapterMetrics>,
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
                enable_ac7: true,
                enable_wingman: true,
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
            adapter_metrics: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start the aircraft auto-switch service
    pub async fn start(&self, bus_publisher: &mut BusPublisher) -> Result<()> {
        // Start auto-switch system
        self.auto_switch.start().await?;

        // Start process detector
        Arc::clone(&self.process_detector).start().await?;

        // Subscribe to bus for telemetry updates
        let subscriber = bus_publisher
            .subscribe(SubscriptionConfig::default())
            .map_err(|e| {
                FlightError::Session(SessionError::AutoSwitch(format!(
                    "Failed to subscribe to bus: {}",
                    e
                )))
            })?;

        *self.bus_subscriber.write().await = Some(subscriber);

        // Start service event loop
        let mut rx = self.service_rx.write().await.take().ok_or_else(|| {
            FlightError::Session(SessionError::AutoSwitch(
                "Service already started".to_string(),
            ))
        })?;

        let auto_switch = Arc::clone(&self.auto_switch);
        let _process_detector = Arc::clone(&self.process_detector);
        let adapters = Arc::clone(&self.adapters);
        let config = self.config.clone();
        let service_tx = self.service_tx.clone();
        let adapter_metrics = Arc::clone(&self.adapter_metrics);

        tokio::spawn(async move {
            info!("Aircraft auto-switch service started");

            // Track last seen aircraft per sim to detect changes from bus snapshots
            let mut last_aircraft_per_sim: HashMap<BusSimId, BusAircraftId> = HashMap::new();

            while let Some(event) = rx.recv().await {
                match event {
                    ServiceEvent::ProcessDetected(process) => {
                        if let Err(e) =
                            Self::handle_process_detected(process, &adapters, &config, &service_tx)
                                .await
                        {
                            error!("Failed to handle process detection: {}", e);
                        }
                    }
                    ServiceEvent::ProcessLost(sim) => {
                        if let Err(e) = Self::handle_process_lost(sim, &adapters).await {
                            error!("Failed to handle process loss: {}", e);
                        }
                        // Clear aircraft tracking for this sim so next snapshot triggers detection
                        last_aircraft_per_sim.remove(&sim);
                    }
                    ServiceEvent::AircraftDetected(sim, aircraft_id) => {
                        let detection_start = Instant::now();
                        let detected_aircraft = DetectedAircraft {
                            sim: map_sim_id(sim),
                            aircraft_id: map_aircraft_id(aircraft_id),
                            process_name: format!("{}_process", sim),
                            detection_time: detection_start,
                            confidence: 0.9,
                        };


                        if let Err(e) = auto_switch.on_aircraft_detected(detected_aircraft).await {
                            error!("Failed to handle aircraft detection: {}", e);
                            // Track detection error
                            let mut metrics = adapter_metrics.write().await;
                            let entry = metrics.entry(sim).or_default();
                            entry.detection_errors += 1;
                        } else {
                            // Track successful detection + time
                            let elapsed = detection_start.elapsed();
                            let mut metrics = adapter_metrics.write().await;
                            let entry = metrics.entry(sim).or_default();
                            entry.aircraft_detections += 1;
                            entry.last_detection = Some(Instant::now());
                            // Exponential moving average of detection time
                            let alpha = 0.1_f64;
                            let new_sample = elapsed.as_secs_f64();
                            let old_avg = entry.average_detection_time.as_secs_f64();
                            entry.average_detection_time = Duration::from_secs_f64(
                                alpha * new_sample + (1.0 - alpha) * old_avg,
                            );
                        }
                    }
                    ServiceEvent::TelemetryUpdate(snapshot) => {
                        if let Err(e) = auto_switch
                            .on_telemetry_update(map_snapshot(&snapshot))
                            .await
                        {
                            error!("Failed to handle telemetry update: {}", e);
                        }
                        // Detect aircraft changes from snapshot aircraft field and
                        // emit AircraftDetected events (covers XPlane/DCS/MSFS adapters
                        // that publish aircraft identity via bus snapshots rather than
                        // direct detection callbacks).
                        let snap_aircraft = snapshot.aircraft.clone();
                        let changed = match last_aircraft_per_sim.get(&snapshot.sim) {
                            Some(last) => last.icao != snap_aircraft.icao,
                            None => !snap_aircraft.icao.is_empty(),
                        };
                        if changed && !snap_aircraft.icao.is_empty() {
                            last_aircraft_per_sim.insert(snapshot.sim, snap_aircraft.clone());
                            let _ = service_tx.send(ServiceEvent::AircraftDetected(
                                snapshot.sim,
                                snap_aircraft,
                            ));
                        }
                    }
                    ServiceEvent::AdapterError(sim, error) => {
                        warn!("Adapter error for {}: {}", sim, error);
                        let mut metrics = adapter_metrics.write().await;
                        metrics.entry(sim).or_default().detection_errors += 1;
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
        self.service_tx.send(ServiceEvent::Shutdown).map_err(|e| {
            FlightError::Session(SessionError::AutoSwitch(format!(
                "Failed to send shutdown: {}",
                e
            )))
        })?;

        Ok(())
    }

    /// Get current service metrics
    pub async fn get_metrics(&self) -> ServiceMetrics {
        let auto_switch_metrics = self.auto_switch.get_metrics().await;
        let process_detection_metrics = self.process_detector.get_metrics().await;

        // Clone the real per-adapter counters tracked by the event loop
        let adapter_metrics = self.adapter_metrics.read().await.clone();

        ServiceMetrics {
            total_aircraft_switches: auto_switch_metrics.total_switches,
            average_detection_time: auto_switch_metrics.average_switch_time,
            auto_switch_metrics,
            process_detection_metrics,
            adapter_metrics,
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
    pub async fn force_switch(&self, aircraft_id: BusAircraftId) -> Result<()> {
        self.auto_switch
            .force_switch(map_aircraft_id(aircraft_id))
            .await
            .map_err(FlightError::Session)
    }

    /// Start monitoring process detection
    async fn start_process_monitoring(&self) -> Result<()> {
        let process_detector = Arc::clone(&self.process_detector);
        let service_tx = self.service_tx.clone();
        let scan_interval = self.config.process_detection.detection_interval;

        tokio::spawn(async move {
            let mut last_processes = HashMap::new();
            let mut interval = tokio::time::interval(scan_interval);

            'monitor: loop {
                interval.tick().await;

                if service_tx.is_closed() {
                    break;
                }

                if let Err(err) = process_detector.scan_once().await {
                    warn!("Process detector scan failed: {}", err);
                    continue;
                }

                let current_processes = process_detector.get_detected_processes().await;

                // Check for new processes
                for (sim, process) in &current_processes {
                    if !last_processes.contains_key(sim)
                        && service_tx
                            .send(ServiceEvent::ProcessDetected(process.clone()))
                            .is_err()
                    {
                        break 'monitor;
                    }
                }

                // Check for lost processes
                for sim in last_processes.keys() {
                    if !current_processes.contains_key(sim) {
                        // Convert CoreSimId to BusSimId for event
                        let bus_sim = match sim {
                            CoreSimId::Msfs => BusSimId::Msfs,
                            CoreSimId::Msfs2024 => BusSimId::Msfs2024,
                            CoreSimId::XPlane => BusSimId::XPlane,
                            CoreSimId::Dcs => BusSimId::Dcs,
                            CoreSimId::AceCombat7 => BusSimId::AceCombat7,
                            CoreSimId::WarThunder => BusSimId::WarThunder,
                            CoreSimId::EliteDangerous => BusSimId::EliteDangerous,
                            CoreSimId::Ksp => BusSimId::Ksp,
                            CoreSimId::Wingman => BusSimId::Wingman,
                            CoreSimId::Unknown => continue,
                        };
                        if service_tx.send(ServiceEvent::ProcessLost(bus_sim)).is_err() {
                            break 'monitor;
                        }
                    }
                }

                last_processes = current_processes;
            }

            debug!("Process monitor loop stopped");
        });

        Ok(())
    }

    /// Start monitoring bus updates
    async fn start_bus_monitoring(&self) -> Result<()> {
        let service_tx = self.service_tx.clone();
        let telemetry_rate = self.config.bus_subscription.telemetry_rate.max(1.0);
        let bus_subscriber = Arc::clone(&self.bus_subscriber);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs_f32(1.0 / telemetry_rate));

            loop {
                interval.tick().await;

                if service_tx.is_closed() {
                    break;
                }

                let latest_snapshot = {
                    let mut guard = bus_subscriber.write().await;
                    let mut latest = None;
                    if let Some(subscriber) = guard.as_mut() {
                        loop {
                            match subscriber.try_recv() {
                                Ok(Some(snapshot)) => {
                                    latest = Some(snapshot);
                                }
                                Ok(None) => break,
                                Err(err) => {
                                    warn!("Bus subscriber error while polling telemetry: {}", err);
                                    break;
                                }
                            }
                        }
                    }
                    latest
                };

                if let Some(snapshot) = latest_snapshot
                    && service_tx
                        .send(ServiceEvent::TelemetryUpdate(snapshot))
                        .is_err()
                {
                    break;
                }
            }

            debug!("Bus monitor loop stopped");
        });

        Ok(())
    }

    /// Handle process detected event
    async fn handle_process_detected(
        process: DetectedProcess,
        adapters: &Arc<RwLock<SimAdapters>>,
        config: &AircraftAutoSwitchServiceConfig,
        service_tx: &mpsc::UnboundedSender<ServiceEvent>,
    ) -> Result<()> {
        info!(
            "Starting adapter for detected process: {} ({})",
            process.process_name, process.sim
        );

        let mut adapters_guard = adapters.write().await;

        // Convert core SimId to bus SimId for matching
        let bus_sim = match process.sim {
            CoreSimId::Msfs => BusSimId::Msfs,
            CoreSimId::Msfs2024 => BusSimId::Msfs2024,
            CoreSimId::XPlane => BusSimId::XPlane,
            CoreSimId::Dcs => BusSimId::Dcs,
            CoreSimId::AceCombat7 => BusSimId::AceCombat7,
            CoreSimId::WarThunder => BusSimId::WarThunder,
            CoreSimId::EliteDangerous => BusSimId::EliteDangerous,
            CoreSimId::Ksp => BusSimId::Ksp,
            CoreSimId::Wingman => BusSimId::Wingman,
            CoreSimId::Unknown => BusSimId::Unknown,
        };

        match bus_sim {
            BusSimId::Msfs if config.adapters.enable_msfs => {
                if adapters_guard.msfs.is_none() {
                    #[cfg(windows)]
                    {
                        let detector = MsfsAircraftDetector::new();
                        // MSFS detection requires an active SimConnect handle (HSIMCONNECT) obtained
                        // from the SimConnect adapter's connection lifecycle. The detector is stored
                        // here and setup/start are called by the SimConnect adapter when it connects.
                        adapters_guard.msfs = Some(MsfsAdapter {
                            detector,
                            current_aircraft: None,
                        });
                    }
                    #[cfg(not(windows))]
                    {
                        adapters_guard.msfs = Some(MsfsAdapter);
                    }
                }
            }
            BusSimId::XPlane if config.adapters.enable_xplane => {
                if adapters_guard.xplane.is_none() {
                    let detector = XPlaneAircraftDetector::new();
                    // X-Plane aircraft detection is driven by the XPlane UDP adapter's telemetry
                    // loop. The detector is stored here; aircraft events flow via the bus subscriber
                    // path set up in start_bus_monitoring().
                    adapters_guard.xplane = Some(XPlaneAdapter {
                        detector,
                        current_aircraft: None,
                    });
                }
            }
            BusSimId::Dcs if config.adapters.enable_dcs => {
                if adapters_guard.dcs.is_none() {
                    let adapter = DcsAdapterApi::new(Default::default());
                    // DCS aircraft detection is driven by the DCS export adapter's run() loop.
                    // The adapter is stored here; aircraft events flow via the bus subscriber
                    // path set up in start_bus_monitoring().
                    adapters_guard.dcs = Some(DcsAdapter {
                        adapter,
                        current_aircraft: None,
                    });
                }
            }
            BusSimId::AceCombat7 if config.adapters.enable_ac7 => {
                if adapters_guard.ac7.is_none() {
                    let adapter = Self::spawn_ac7_adapter_task(
                        Ac7TelemetryConfig::default(),
                        service_tx.clone(),
                    );
                    adapters_guard.ac7 = Some(adapter);
                }
            }
            BusSimId::Wingman if config.adapters.enable_wingman => {
                if adapters_guard.wingman.is_none() {
                    adapters_guard.wingman = Some(WingmanWrapper {
                        process_name: process.process_name.clone(),
                    });
                }
            }
            _ => {
                debug!("Adapter not enabled or supported for sim: {}", process.sim);
            }
        }

        Ok(())
    }

    fn spawn_ac7_adapter_task(
        adapter_config: Ac7TelemetryConfig,
        service_tx: mpsc::UnboundedSender<ServiceEvent>,
    ) -> Ac7Adapter {
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let join_handle = tokio::spawn(async move {
            let mut adapter = Ac7AdapterApi::new(adapter_config);
            if let Err(err) = adapter.start().await {
                let _ = service_tx.send(ServiceEvent::AdapterError(
                    BusSimId::AceCombat7,
                    format!("failed to start AC7 adapter: {}", err),
                ));
                return;
            }

            let mut last_aircraft: Option<BusAircraftId> = None;
            loop {
                tokio::select! {
                    changed = shutdown_rx.changed() => {
                        if changed.is_err() || *shutdown_rx.borrow() {
                            break;
                        }
                    }
                    packet = adapter.poll_once() => {
                        match packet {
                            Ok(Some(snapshot)) => {
                                let aircraft = snapshot.aircraft.clone();

                                if service_tx.send(ServiceEvent::TelemetryUpdate(snapshot)).is_err() {
                                    break;
                                }

                                if last_aircraft.as_ref() != Some(&aircraft) {
                                    last_aircraft = Some(aircraft.clone());
                                    if service_tx.send(ServiceEvent::AircraftDetected(BusSimId::AceCombat7, aircraft)).is_err() {
                                        break;
                                    }
                                }
                            }
                            Ok(None) => {}
                            Err(err) => {
                                if service_tx
                                    .send(ServiceEvent::AdapterError(BusSimId::AceCombat7, err.to_string()))
                                    .is_err()
                                {
                                    break;
                                }
                            }
                        }
                    }
                }
            }

            adapter.stop();
        });

        Ac7Adapter {
            shutdown_tx,
            join_handle,
        }
    }

    /// Handle process lost event
    async fn handle_process_lost(sim: BusSimId, adapters: &Arc<RwLock<SimAdapters>>) -> Result<()> {
        info!("Stopping adapter for lost process: {}", sim);

        let mut adapters_guard = adapters.write().await;
        let mut ac7_to_stop = None;

        match sim {
            BusSimId::Msfs => {
                adapters_guard.msfs = None;
            }
            BusSimId::XPlane => {
                adapters_guard.xplane = None;
            }
            BusSimId::Dcs => {
                adapters_guard.dcs = None;
            }
            BusSimId::AceCombat7 => {
                ac7_to_stop = adapters_guard.ac7.take();
            }
            BusSimId::Wingman => {
                adapters_guard.wingman = None;
            }
            _ => {}
        }

        drop(adapters_guard);

        if let Some(adapter) = ac7_to_stop {
            adapter.stop().await;
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
            ac7: None,
            wingman: None,
        }
    }

    async fn stop_all(&mut self) -> Result<()> {
        // Stop all adapters
        self.msfs = None;
        self.xplane = None;
        self.dcs = None;
        if let Some(adapter) = self.ac7.take() {
            adapter.stop().await;
        }
        self.wingman = None;
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
        config.adapters.enable_ac7 = true;

        let service = AircraftAutoSwitchService::new(config);
        assert!(!service.config.adapters.enable_msfs);
        assert!(service.config.adapters.enable_xplane);
        assert!(!service.config.adapters.enable_dcs);
        assert!(service.config.adapters.enable_ac7);
    }

    #[tokio::test]
    async fn test_force_switch() {
        let config = AircraftAutoSwitchServiceConfig::default();
        let service = AircraftAutoSwitchService::new(config);

        let aircraft_id = BusAircraftId::new("C172");

        // This should not fail even without starting the service
        // (it will just queue the request)
        assert!(service.force_switch(aircraft_id).await.is_ok());
    }

    /// Verify that the aircraft-change detection logic embedded in TelemetryUpdate
    /// correctly recognises a new or changed aircraft ICAO.
    #[test]
    fn test_aircraft_change_detection_logic() {
        use flight_bus::types::{AircraftId, SimId};
        use std::collections::HashMap;

        // Simulate the per-sim tracking map used inside the event loop
        let mut last_aircraft_per_sim: HashMap<SimId, AircraftId> = HashMap::new();

        let sim = SimId::XPlane;
        let a172 = AircraftId::new("C172");
        let a320 = AircraftId::new("A320");
        let empty = AircraftId::new("");

        // First snapshot with non-empty ICAO → "changed" (not in map)
        let changed = match last_aircraft_per_sim.get(&sim) {
            Some(last) => last.icao != a172.icao,
            None => !a172.icao.is_empty(),
        };
        assert!(changed, "first non-empty aircraft should be detected as changed");
        last_aircraft_per_sim.insert(sim, a172.clone());

        // Same ICAO again → not changed
        let changed = match last_aircraft_per_sim.get(&sim) {
            Some(last) => last.icao != a172.icao,
            None => !a172.icao.is_empty(),
        };
        assert!(!changed, "same aircraft should not be detected as changed");

        // Different ICAO → changed
        let changed = match last_aircraft_per_sim.get(&sim) {
            Some(last) => last.icao != a320.icao,
            None => !a320.icao.is_empty(),
        };
        assert!(changed, "different aircraft should be detected as changed");
        last_aircraft_per_sim.insert(sim, a320.clone());

        // Empty ICAO should never trigger detection
        let changed_and_nonempty = {
            let changed = match last_aircraft_per_sim.get(&sim) {
                Some(last) => last.icao != empty.icao,
                None => !empty.icao.is_empty(),
            };
            changed && !empty.icao.is_empty()
        };
        assert!(!changed_and_nonempty, "empty ICAO should not trigger aircraft detection");

        // After process loss: remove from map → next non-empty triggers detection
        last_aircraft_per_sim.remove(&sim);
        let changed = match last_aircraft_per_sim.get(&sim) {
            Some(last) => last.icao != a172.icao,
            None => !a172.icao.is_empty(),
        };
        assert!(changed, "after process loss, same aircraft should be detected again");
    }
}
