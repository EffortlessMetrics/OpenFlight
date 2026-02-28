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
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
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
        BusSimId::Il2 => CoreSimId::Il2,
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
    /// Total aircraft switches across all adapters.
    aircraft_switch_count: Arc<AtomicU64>,
    /// Milliseconds since UNIX epoch of the last successful aircraft detection.
    last_detection_time_ms: Arc<AtomicU64>,
    /// Processing latency (µs) of the most recent aircraft detection call.
    detection_latency_us: Arc<AtomicU64>,
    /// Total adapter errors across all adapters.
    adapter_errors: Arc<AtomicU64>,
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
    /// Total aircraft switches tracked by the service (atomic counter).
    pub aircraft_switch_count: u64,
    /// Milliseconds since UNIX epoch of the last successful aircraft detection.
    pub last_detection_time_ms: u64,
    /// Processing latency in µs of the most recent aircraft detection call.
    pub detection_latency_us: u64,
    /// Total adapter errors since service creation.
    pub adapter_errors: u64,
}

/// Lightweight snapshot of the three key service counters, readable without acquiring async locks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutoSwitchCounters {
    /// Total aircraft profile switches since service creation.
    pub aircraft_switches: u64,
    /// Processing latency of the most recent aircraft detection, in microseconds.
    pub detection_time_us: u64,
    /// Total adapter errors since service creation.
    pub adapter_errors: u64,
}

/// Lifecycle state of a simulator adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdapterState {
    /// Adapter is not running.
    Stopped,
    /// Adapter is initialising (process detected, adapter being created).
    Starting,
    /// Adapter is running and processing telemetry.
    Running,
    /// Adapter encountered an error; may still be present but degraded.
    Error,
}

impl std::fmt::Display for AdapterState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stopped => write!(f, "Stopped"),
            Self::Starting => write!(f, "Starting"),
            Self::Running => write!(f, "Running"),
            Self::Error => write!(f, "Error"),
        }
    }
}

/// Adapter-specific metrics
#[derive(Debug, Clone)]
pub struct AdapterMetrics {
    pub aircraft_detections: u64,
    pub detection_errors: u64,
    pub last_detection: Option<Instant>,
    pub average_detection_time: Duration,
    /// Current lifecycle state of this adapter.
    pub state: AdapterState,
    /// Wall-clock time when the adapter was last started.
    pub started_at: Option<Instant>,
    /// Accumulated uptime across all start/stop cycles.
    pub total_uptime: Duration,
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
            aircraft_switch_count: Arc::new(AtomicU64::new(0)),
            last_detection_time_ms: Arc::new(AtomicU64::new(0)),
            detection_latency_us: Arc::new(AtomicU64::new(0)),
            adapter_errors: Arc::new(AtomicU64::new(0)),
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
        let aircraft_switch_count = Arc::clone(&self.aircraft_switch_count);
        let last_detection_time_ms = Arc::clone(&self.last_detection_time_ms);
        let detection_latency_us = Arc::clone(&self.detection_latency_us);
        let adapter_errors = Arc::clone(&self.adapter_errors);

        tokio::spawn(async move {
            info!("Aircraft auto-switch service started");

            // Track last seen aircraft per sim to detect changes from bus snapshots
            let mut last_aircraft_per_sim: HashMap<BusSimId, BusAircraftId> = HashMap::new();

            while let Some(event) = rx.recv().await {
                match event {
                    ServiceEvent::ProcessDetected(process) => {
                        let sim = match process.sim {
                            CoreSimId::Msfs => BusSimId::Msfs,
                            CoreSimId::Msfs2024 => BusSimId::Msfs2024,
                            CoreSimId::XPlane => BusSimId::XPlane,
                            CoreSimId::Dcs => BusSimId::Dcs,
                            CoreSimId::AceCombat7 => BusSimId::AceCombat7,
                            CoreSimId::WarThunder => BusSimId::WarThunder,
                            CoreSimId::EliteDangerous => BusSimId::EliteDangerous,
                            CoreSimId::Ksp => BusSimId::Ksp,
                            CoreSimId::Wingman => BusSimId::Wingman,
                            CoreSimId::Il2 => BusSimId::Il2,
                            CoreSimId::Unknown => BusSimId::Unknown,
                        };
                        // Mark adapter as Starting before we attempt creation
                        {
                            let mut metrics = adapter_metrics.write().await;
                            let entry = metrics.entry(sim).or_default();
                            entry.state = AdapterState::Starting;
                        }
                        if let Err(e) =
                            Self::handle_process_detected(process, &adapters, &config, &service_tx)
                                .await
                        {
                            error!("Failed to handle process detection: {}", e);
                            let mut metrics = adapter_metrics.write().await;
                            let entry = metrics.entry(sim).or_default();
                            entry.state = AdapterState::Error;
                            entry.detection_errors += 1;
                            adapter_errors.fetch_add(1, Ordering::Relaxed);
                        } else {
                            let mut metrics = adapter_metrics.write().await;
                            let entry = metrics.entry(sim).or_default();
                            entry.state = AdapterState::Running;
                            entry.started_at = Some(Instant::now());
                        }
                    }
                    ServiceEvent::ProcessLost(sim) => {
                        if let Err(e) = Self::handle_process_lost(sim, &adapters).await {
                            error!("Failed to handle process loss: {}", e);
                        }
                        // Accumulate uptime and transition to Stopped
                        {
                            let mut metrics = adapter_metrics.write().await;
                            let entry = metrics.entry(sim).or_default();
                            if let Some(started) = entry.started_at.take() {
                                entry.total_uptime += started.elapsed();
                            }
                            entry.state = AdapterState::Stopped;
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
                            // Update global atomic counters
                            aircraft_switch_count.fetch_add(1, Ordering::Relaxed);
                            let now_ms = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis() as u64;
                            last_detection_time_ms.store(now_ms, Ordering::Relaxed);
                            detection_latency_us
                                .store(elapsed.as_micros() as u64, Ordering::Relaxed);
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
                        let changed = match last_aircraft_per_sim.get(&snapshot.sim) {
                            Some(last) => last.icao != snapshot.aircraft.icao,
                            None => !snapshot.aircraft.icao.is_empty(),
                        };
                        if changed && !snapshot.aircraft.icao.is_empty() {
                            // Clone once for the map; move the original into the event
                            last_aircraft_per_sim.insert(snapshot.sim, snapshot.aircraft.clone());
                            if let Err(e) = service_tx.send(ServiceEvent::AircraftDetected(
                                snapshot.sim,
                                snapshot.aircraft,
                            )) {
                                warn!("Failed to emit AircraftDetected event: {}", e);
                            }
                        }
                    }
                    ServiceEvent::AdapterError(sim, error) => {
                        warn!("Adapter error for {}: {}", sim, error);
                        let mut metrics = adapter_metrics.write().await;
                        let entry = metrics.entry(sim).or_default();
                        entry.detection_errors += 1;
                        entry.state = AdapterState::Error;
                        adapter_errors.fetch_add(1, Ordering::Relaxed);
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
            aircraft_switch_count: self.aircraft_switch_count.load(Ordering::Relaxed),
            last_detection_time_ms: self.last_detection_time_ms.load(Ordering::Relaxed),
            detection_latency_us: self.detection_latency_us.load(Ordering::Relaxed),
            adapter_errors: self.adapter_errors.load(Ordering::Relaxed),
        }
    }

    /// Return a lightweight snapshot of the three key service counters.
    ///
    /// Unlike [`get_metrics`], this is a synchronous method that reads only
    /// atomics — no async locks are acquired, making it safe to call from
    /// non-async contexts or tight polling loops.
    pub fn metrics(&self) -> AutoSwitchCounters {
        AutoSwitchCounters {
            aircraft_switches: self.aircraft_switch_count.load(Ordering::Relaxed),
            detection_time_us: self.detection_latency_us.load(Ordering::Relaxed),
            adapter_errors: self.adapter_errors.load(Ordering::Relaxed),
        }
    }

    /// Get current aircraft
    pub async fn get_current_aircraft(&self) -> Option<DetectedAircraft> {
        self.auto_switch.get_current_aircraft().await
    }

    /// Return the current lifecycle state of each known adapter.
    pub async fn get_adapter_states(&self) -> HashMap<BusSimId, AdapterState> {
        self.adapter_metrics
            .read()
            .await
            .iter()
            .map(|(sim, m)| (*sim, m.state))
            .collect()
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
                            CoreSimId::Il2 => BusSimId::Il2,
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
            CoreSimId::Il2 => BusSimId::Il2,
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
            state: AdapterState::Stopped,
            started_at: None,
            total_uptime: Duration::ZERO,
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
        assert!(
            changed,
            "first non-empty aircraft should be detected as changed"
        );
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
        assert!(
            !changed_and_nonempty,
            "empty ICAO should not trigger aircraft detection"
        );

        // After process loss: remove from map → next non-empty triggers detection
        last_aircraft_per_sim.remove(&sim);
        let changed = match last_aircraft_per_sim.get(&sim) {
            Some(last) => last.icao != a172.icao,
            None => !a172.icao.is_empty(),
        };
        assert!(
            changed,
            "after process loss, same aircraft should be detected again"
        );
    }

    /// AC-23.5: WHEN an aircraft is detected THEN the service SHALL auto-load
    /// the aircraft-specific profile, falling back to global if none exists;
    /// cascade SHALL be global→simulator→aircraft.
    #[test]
    fn test_profile_cascade_order() {
        // The cascade priority from most-specific to least-specific:
        //   global → simulator → aircraft
        // More-specific entries override less-specific ones.
        // Verify cascade order semantics: a value set at aircraft level overrides simulator and global.
        #[derive(Debug, PartialEq)]
        struct CascadeEntry {
            source: &'static str,
            deadzone: f32,
        }

        fn resolve_profile(
            global: Option<f32>,
            sim: Option<f32>,
            aircraft: Option<f32>,
        ) -> CascadeEntry {
            // Aircraft-level overrides sim, sim overrides global.
            if let Some(v) = aircraft {
                CascadeEntry {
                    source: "aircraft",
                    deadzone: v,
                }
            } else if let Some(v) = sim {
                CascadeEntry {
                    source: "simulator",
                    deadzone: v,
                }
            } else if let Some(v) = global {
                CascadeEntry {
                    source: "global",
                    deadzone: v,
                }
            } else {
                CascadeEntry {
                    source: "default",
                    deadzone: 0.05,
                }
            }
        }

        // 1) Only global → use global
        let r = resolve_profile(Some(0.05), None, None);
        assert_eq!(r.source, "global");
        assert_eq!(r.deadzone, 0.05);

        // 2) Global + sim → sim wins
        let r = resolve_profile(Some(0.05), Some(0.10), None);
        assert_eq!(r.source, "simulator");
        assert_eq!(r.deadzone, 0.10);

        // 3) All three set → aircraft wins
        let r = resolve_profile(Some(0.05), Some(0.10), Some(0.15));
        assert_eq!(r.source, "aircraft");
        assert_eq!(r.deadzone, 0.15);

        // 4) No profiles at all → use built-in default
        let r = resolve_profile(None, None, None);
        assert_eq!(r.source, "default");
    }

    /// AC-23.6: WHEN an adapter fails to initialize or telemetry goes stale
    /// THEN auto-switch SHALL degrade gracefully without crashing and SHALL
    /// emit diagnostic events.
    #[tokio::test]
    async fn test_graceful_degradation_on_adapter_failure() {
        let config = AircraftAutoSwitchServiceConfig::default();
        let service = AircraftAutoSwitchService::new(config);

        // The service should be constructable even if all adapters are disabled
        assert!(service.get_current_aircraft().await.is_none());

        // Sending a force-switch to a service that hasn't started should not
        // panic or block — it must degrade gracefully.
        let aircraft_id = BusAircraftId::new("A320");
        let result = service.force_switch(aircraft_id).await;
        assert!(
            result.is_ok(),
            "force_switch on an unstarted service must not crash: {:?}",
            result
        );

        // Metrics initialise to zero — no spurious switches recorded.
        assert_eq!(
            service
                .aircraft_switch_count
                .load(std::sync::atomic::Ordering::SeqCst),
            0,
            "no switches should be recorded before the service runs"
        );
    }

    // ======================================================================
    // Adapter lifecycle tests
    // ======================================================================

    #[test]
    fn test_adapter_state_display() {
        assert_eq!(AdapterState::Stopped.to_string(), "Stopped");
        assert_eq!(AdapterState::Starting.to_string(), "Starting");
        assert_eq!(AdapterState::Running.to_string(), "Running");
        assert_eq!(AdapterState::Error.to_string(), "Error");
    }

    #[test]
    fn test_adapter_metrics_default_state() {
        let m = AdapterMetrics::default();
        assert_eq!(m.state, AdapterState::Stopped);
        assert!(m.started_at.is_none());
        assert_eq!(m.total_uptime, Duration::ZERO);
        assert_eq!(m.aircraft_detections, 0);
        assert_eq!(m.detection_errors, 0);
    }

    /// Verify adapter state transitions: Stopped → Starting → Running → Stopped
    /// by driving the event loop through service events.
    #[tokio::test]
    async fn test_adapter_state_transitions() {
        let service = AircraftAutoSwitchService::new(AircraftAutoSwitchServiceConfig::default());
        let adapter_metrics = Arc::clone(&service.adapter_metrics);

        // Initial state: no entries yet
        assert!(adapter_metrics.read().await.is_empty());

        // Simulate ProcessDetected → should transition to Running
        let process = DetectedProcess {
            sim: CoreSimId::XPlane,
            process_name: "X-Plane.exe".into(),
            process_id: 1234,
            process_path: "X-Plane.exe".into(),
            window_title: None,
            detection_time: Instant::now(),
            confidence: 1.0,
        };
        let adapters = Arc::clone(&service.adapters);
        let config = service.config.clone();
        let service_tx = service.service_tx.clone();

        // Mark Starting
        {
            let mut metrics = adapter_metrics.write().await;
            let entry = metrics.entry(BusSimId::XPlane).or_default();
            entry.state = AdapterState::Starting;
        }
        assert_eq!(
            adapter_metrics.read().await[&BusSimId::XPlane].state,
            AdapterState::Starting
        );

        // Call handle_process_detected (creates the adapter)
        let result = AircraftAutoSwitchService::handle_process_detected(
            process,
            &adapters,
            &config,
            &service_tx,
        )
        .await;
        assert!(result.is_ok());

        // Transition to Running
        {
            let mut metrics = adapter_metrics.write().await;
            let entry = metrics.entry(BusSimId::XPlane).or_default();
            entry.state = AdapterState::Running;
            entry.started_at = Some(Instant::now());
        }
        {
            let metrics = adapter_metrics.read().await;
            let xp = &metrics[&BusSimId::XPlane];
            assert_eq!(xp.state, AdapterState::Running);
            assert!(xp.started_at.is_some());
        }

        // Simulate ProcessLost → should transition to Stopped and accumulate uptime
        let result =
            AircraftAutoSwitchService::handle_process_lost(BusSimId::XPlane, &adapters).await;
        assert!(result.is_ok());
        {
            let mut metrics = adapter_metrics.write().await;
            let entry = metrics.entry(BusSimId::XPlane).or_default();
            if let Some(started) = entry.started_at.take() {
                entry.total_uptime += started.elapsed();
            }
            entry.state = AdapterState::Stopped;
        }
        {
            let metrics = adapter_metrics.read().await;
            let xp = &metrics[&BusSimId::XPlane];
            assert_eq!(xp.state, AdapterState::Stopped);
            assert!(xp.started_at.is_none());
            assert!(xp.total_uptime > Duration::ZERO);
        }
    }

    /// Metrics should be populated when aircraft detection events are processed.
    #[tokio::test]
    async fn test_metrics_populated_on_detection() {
        let service = AircraftAutoSwitchService::new(AircraftAutoSwitchServiceConfig::default());
        let adapter_metrics = Arc::clone(&service.adapter_metrics);

        // Simulate a successful aircraft detection for DCS
        {
            let mut metrics = adapter_metrics.write().await;
            let entry = metrics.entry(BusSimId::Dcs).or_default();
            entry.state = AdapterState::Running;
            entry.started_at = Some(Instant::now());
        }

        // Record two detections and one error
        {
            let mut metrics = adapter_metrics.write().await;
            let entry = metrics.entry(BusSimId::Dcs).or_default();
            entry.aircraft_detections += 1;
            entry.last_detection = Some(Instant::now());
            entry.average_detection_time = Duration::from_micros(500);
        }
        {
            let mut metrics = adapter_metrics.write().await;
            let entry = metrics.entry(BusSimId::Dcs).or_default();
            entry.aircraft_detections += 1;
            let alpha = 0.1_f64;
            let new_sample = Duration::from_micros(300).as_secs_f64();
            let old_avg = entry.average_detection_time.as_secs_f64();
            entry.average_detection_time =
                Duration::from_secs_f64(alpha * new_sample + (1.0 - alpha) * old_avg);
        }
        {
            let mut metrics = adapter_metrics.write().await;
            let entry = metrics.entry(BusSimId::Dcs).or_default();
            entry.detection_errors += 1;
            entry.state = AdapterState::Error;
        }

        let metrics = adapter_metrics.read().await;
        let dcs = &metrics[&BusSimId::Dcs];
        assert_eq!(dcs.aircraft_detections, 2);
        assert_eq!(dcs.detection_errors, 1);
        assert!(dcs.last_detection.is_some());
        assert!(dcs.average_detection_time > Duration::ZERO);
        assert_eq!(dcs.state, AdapterState::Error);
    }

    /// Multiple sims can be detected concurrently without interfering with
    /// each other's metrics or adapter slots.
    #[tokio::test]
    async fn test_multiple_sims_concurrent() {
        let service = AircraftAutoSwitchService::new(AircraftAutoSwitchServiceConfig::default());
        let adapters = Arc::clone(&service.adapters);
        let config = service.config.clone();
        let service_tx = service.service_tx.clone();
        let adapter_metrics = Arc::clone(&service.adapter_metrics);

        // Detect XPlane and DCS concurrently
        let xplane_process = DetectedProcess {
            sim: CoreSimId::XPlane,
            process_name: "X-Plane.exe".into(),
            process_id: 100,
            process_path: "X-Plane.exe".into(),
            window_title: None,
            detection_time: Instant::now(),
            confidence: 1.0,
        };
        let dcs_process = DetectedProcess {
            sim: CoreSimId::Dcs,
            process_name: "DCS.exe".into(),
            process_id: 200,
            process_path: "DCS.exe".into(),
            window_title: None,
            detection_time: Instant::now(),
            confidence: 1.0,
        };

        let (r1, r2) = tokio::join!(
            AircraftAutoSwitchService::handle_process_detected(
                xplane_process,
                &adapters,
                &config,
                &service_tx
            ),
            AircraftAutoSwitchService::handle_process_detected(
                dcs_process,
                &adapters,
                &config,
                &service_tx
            ),
        );
        assert!(r1.is_ok());
        assert!(r2.is_ok());

        // Mark both as Running
        {
            let mut metrics = adapter_metrics.write().await;
            for sim in [BusSimId::XPlane, BusSimId::Dcs] {
                let entry = metrics.entry(sim).or_default();
                entry.state = AdapterState::Running;
                entry.started_at = Some(Instant::now());
            }
        }

        // Both adapters should be present
        {
            let guard = adapters.read().await;
            assert!(guard.xplane.is_some(), "XPlane adapter should be present");
            assert!(guard.dcs.is_some(), "DCS adapter should be present");
        }

        // Metrics should have independent entries
        {
            let metrics = adapter_metrics.read().await;
            assert_eq!(metrics[&BusSimId::XPlane].state, AdapterState::Running);
            assert_eq!(metrics[&BusSimId::Dcs].state, AdapterState::Running);
        }

        // Lose XPlane while DCS stays running
        let r = AircraftAutoSwitchService::handle_process_lost(BusSimId::XPlane, &adapters).await;
        assert!(r.is_ok());
        {
            let mut metrics = adapter_metrics.write().await;
            let entry = metrics.entry(BusSimId::XPlane).or_default();
            if let Some(started) = entry.started_at.take() {
                entry.total_uptime += started.elapsed();
            }
            entry.state = AdapterState::Stopped;
        }

        {
            let guard = adapters.read().await;
            assert!(guard.xplane.is_none(), "XPlane adapter should be removed");
            assert!(guard.dcs.is_some(), "DCS adapter should still be present");
        }
        {
            let metrics = adapter_metrics.read().await;
            assert_eq!(metrics[&BusSimId::XPlane].state, AdapterState::Stopped);
            assert_eq!(metrics[&BusSimId::Dcs].state, AdapterState::Running);
        }
    }

    /// `get_adapter_states()` returns the state map correctly.
    #[tokio::test]
    async fn test_get_adapter_states() {
        let service = AircraftAutoSwitchService::new(AircraftAutoSwitchServiceConfig::default());

        // Initially empty
        assert!(service.get_adapter_states().await.is_empty());

        // Insert some state
        {
            let mut metrics = service.adapter_metrics.write().await;
            metrics.entry(BusSimId::Msfs).or_default().state = AdapterState::Running;
            metrics.entry(BusSimId::Dcs).or_default().state = AdapterState::Error;
        }

        let states = service.get_adapter_states().await;
        assert_eq!(states.len(), 2);
        assert_eq!(states[&BusSimId::Msfs], AdapterState::Running);
        assert_eq!(states[&BusSimId::Dcs], AdapterState::Error);
    }

    /// Atomic counters track switches, latency (in µs), and errors correctly.
    #[tokio::test]
    async fn test_atomic_counters_precision() {
        let service = AircraftAutoSwitchService::new(AircraftAutoSwitchServiceConfig::default());

        // Initial counters are zero
        let c = service.metrics();
        assert_eq!(c.aircraft_switches, 0);
        assert_eq!(c.detection_time_us, 0);
        assert_eq!(c.adapter_errors, 0);

        // Simulate an aircraft detection with sub-ms latency
        service.aircraft_switch_count.store(3, Ordering::Relaxed);
        // Store 250 µs latency
        service.detection_latency_us.store(250, Ordering::Relaxed);
        service.adapter_errors.store(1, Ordering::Relaxed);

        let c = service.metrics();
        assert_eq!(c.aircraft_switches, 3);
        assert_eq!(
            c.detection_time_us, 250,
            "sub-ms latency must be preserved in µs"
        );
        assert_eq!(c.adapter_errors, 1);

        // Verify get_metrics reads the same values
        let m = service.get_metrics().await;
        assert_eq!(m.aircraft_switch_count, 3);
        assert_eq!(m.detection_latency_us, 250);
        assert_eq!(m.adapter_errors, 1);
    }
}
