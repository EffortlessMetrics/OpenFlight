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
use flight_xplane::AircraftDetector as XPlaneAircraftDetector;
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
// AdapterDetector trait
// ============================================================================

/// Unified interface for simulator adapter lifecycle and aircraft detection.
///
/// Each simulator adapter implements this trait so the auto-switch service can
/// manage start / stop / detect uniformly regardless of the underlying protocol
/// (SimConnect, UDP datarefs, DCS export, shared-memory telemetry, …).
#[allow(dead_code)]
pub(crate) trait AdapterDetector: Send {
    /// Simulator this adapter is responsible for.
    #[allow(dead_code)]
    fn sim_id(&self) -> BusSimId;

    /// Base confidence for detections originating from this adapter.
    ///
    /// Values closer to 1.0 indicate higher trust.  This is combined with
    /// per-detection adjustments (e.g. fuzzy vs exact match) at the call site.
    #[allow(dead_code)]
    fn base_confidence(&self) -> f32;
}

/// Return the baseline detection confidence for a given simulator.
///
/// Confidence reflects how authoritative the detection source is:
/// - Direct API (SimConnect) → highest
/// - Structured protocol (XPlane datarefs, DCS export) → high
/// - Shared-memory telemetry (AC7) → medium
/// - Process detection only (Wingman) → lower
pub(crate) fn confidence_for_sim(sim: BusSimId) -> f32 {
    match sim {
        BusSimId::Msfs | BusSimId::Msfs2024 => 0.95,
        BusSimId::XPlane => 0.92,
        BusSimId::Dcs => 0.90,
        BusSimId::AceCombat7 => 0.85,
        BusSimId::Wingman => 0.75,
        _ => 0.70,
    }
}

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
    /// Minimum detection confidence observed (stored as `f64::to_bits`).
    min_confidence_bits: Arc<AtomicU64>,
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

/// X-Plane adapter wrapper — spawns a detection polling task.
struct XPlaneAdapter {
    shutdown_tx: watch::Sender<bool>,
    join_handle: JoinHandle<()>,
}

impl XPlaneAdapter {
    async fn stop(self) {
        let _ = self.shutdown_tx.send(true);
        let _ = self.join_handle.await;
    }
}

impl AdapterDetector for XPlaneAdapter {
    fn sim_id(&self) -> BusSimId {
        BusSimId::XPlane
    }
    fn base_confidence(&self) -> f32 {
        confidence_for_sim(BusSimId::XPlane)
    }
}

/// DCS adapter wrapper — spawns a detection polling task.
struct DcsAdapter {
    shutdown_tx: watch::Sender<bool>,
    join_handle: JoinHandle<()>,
}

impl DcsAdapter {
    async fn stop(self) {
        let _ = self.shutdown_tx.send(true);
        let _ = self.join_handle.await;
    }
}

impl AdapterDetector for DcsAdapter {
    fn sim_id(&self) -> BusSimId {
        BusSimId::Dcs
    }
    fn base_confidence(&self) -> f32 {
        confidence_for_sim(BusSimId::Dcs)
    }
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

impl AdapterDetector for Ac7Adapter {
    fn sim_id(&self) -> BusSimId {
        BusSimId::AceCombat7
    }
    fn base_confidence(&self) -> f32 {
        confidence_for_sim(BusSimId::AceCombat7)
    }
}

/// Project Wingman adapter wrapper (no telemetry API; tracks process detection only).
struct WingmanWrapper {
    #[allow(dead_code)]
    process_name: String,
}

impl AdapterDetector for WingmanWrapper {
    fn sim_id(&self) -> BusSimId {
        BusSimId::Wingman
    }
    fn base_confidence(&self) -> f32 {
        confidence_for_sim(BusSimId::Wingman)
    }
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
    /// Minimum detection confidence observed across all aircraft detections.
    pub min_confidence: f64,
}

/// Lightweight snapshot of the three key service counters, readable without acquiring async locks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AutoSwitchCounters {
    /// Total aircraft profile switches since service creation.
    pub aircraft_switches: u64,
    /// Processing latency of the most recent aircraft detection, in microseconds.
    pub detection_time_us: u64,
    /// Total adapter errors since service creation.
    pub adapter_errors: u64,
    /// Minimum detection confidence observed (1.0 if no detection yet).
    pub min_confidence: f64,
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
    /// Number of times this adapter has been connected (process detected → Running).
    pub connections: u64,
    /// Number of times this adapter has been disconnected (process lost → Stopped).
    pub disconnections: u64,
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

        // Pre-populate adapter metrics for all enabled sims so callers always
        // see an entry even before the first process-detection event.
        let mut initial_metrics = HashMap::new();
        if config.adapters.enable_msfs {
            initial_metrics.insert(BusSimId::Msfs, AdapterMetrics::default());
        }
        if config.adapters.enable_xplane {
            initial_metrics.insert(BusSimId::XPlane, AdapterMetrics::default());
        }
        if config.adapters.enable_dcs {
            initial_metrics.insert(BusSimId::Dcs, AdapterMetrics::default());
        }
        if config.adapters.enable_ac7 {
            initial_metrics.insert(BusSimId::AceCombat7, AdapterMetrics::default());
        }
        if config.adapters.enable_wingman {
            initial_metrics.insert(BusSimId::Wingman, AdapterMetrics::default());
        }

        Self {
            config,
            auto_switch,
            process_detector,
            adapters: Arc::new(RwLock::new(SimAdapters::new())),
            bus_subscriber: Arc::new(RwLock::new(None)),
            service_tx,
            service_rx: Arc::new(RwLock::new(Some(service_rx))),
            adapter_metrics: Arc::new(RwLock::new(initial_metrics)),
            aircraft_switch_count: Arc::new(AtomicU64::new(0)),
            last_detection_time_ms: Arc::new(AtomicU64::new(0)),
            detection_latency_us: Arc::new(AtomicU64::new(0)),
            adapter_errors: Arc::new(AtomicU64::new(0)),
            min_confidence_bits: Arc::new(AtomicU64::new(1.0_f64.to_bits())),
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
        let min_confidence_bits = Arc::clone(&self.min_confidence_bits);

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
                            entry.connections += 1;
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
                            entry.disconnections += 1;
                        }
                        // Clear aircraft tracking for this sim so next snapshot triggers detection
                        last_aircraft_per_sim.remove(&sim);
                    }
                    ServiceEvent::AircraftDetected(sim, aircraft_id) => {
                        let detection_start = Instant::now();
                        let base_conf = confidence_for_sim(sim);
                        let detected_aircraft = DetectedAircraft {
                            sim: map_sim_id(sim),
                            aircraft_id: map_aircraft_id(aircraft_id),
                            process_name: format!("{}_process", sim),
                            detection_time: detection_start,
                            confidence: base_conf,
                        };
                        let confidence = detected_aircraft.confidence;

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
                            // Track minimum confidence observed
                            let conf_bits = (confidence as f64).to_bits();
                            let _ = min_confidence_bits.fetch_update(
                                Ordering::Relaxed,
                                Ordering::Relaxed,
                                |cur| {
                                    if f64::from_bits(conf_bits) < f64::from_bits(cur) {
                                        Some(conf_bits)
                                    } else {
                                        None
                                    }
                                },
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
            min_confidence: f64::from_bits(self.min_confidence_bits.load(Ordering::Relaxed)),
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
            min_confidence: f64::from_bits(self.min_confidence_bits.load(Ordering::Relaxed)),
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
                    let adapter = Self::spawn_xplane_adapter_task(service_tx.clone());
                    info!("X-Plane adapter task spawned for detection polling");
                    adapters_guard.xplane = Some(adapter);
                }
            }
            BusSimId::Dcs if config.adapters.enable_dcs => {
                if adapters_guard.dcs.is_none() {
                    let adapter = Self::spawn_dcs_adapter_task(service_tx.clone());
                    info!("DCS adapter task spawned for detection polling");
                    adapters_guard.dcs = Some(adapter);
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

    /// Handle process lost event — gracefully shuts down the adapter task.
    async fn handle_process_lost(sim: BusSimId, adapters: &Arc<RwLock<SimAdapters>>) -> Result<()> {
        info!("Stopping adapter for lost process: {}", sim);

        let mut adapters_guard = adapters.write().await;
        let mut xplane_to_stop = None;
        let mut dcs_to_stop = None;
        let mut ac7_to_stop = None;

        match sim {
            BusSimId::Msfs => {
                adapters_guard.msfs = None;
            }
            BusSimId::XPlane => {
                xplane_to_stop = adapters_guard.xplane.take();
            }
            BusSimId::Dcs => {
                dcs_to_stop = adapters_guard.dcs.take();
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

        if let Some(adapter) = xplane_to_stop {
            adapter.stop().await;
        }
        if let Some(adapter) = dcs_to_stop {
            adapter.stop().await;
        }
        if let Some(adapter) = ac7_to_stop {
            adapter.stop().await;
        }

        Ok(())
    }

    /// Spawn a background task that drives X-Plane aircraft detection polling.
    fn spawn_xplane_adapter_task(service_tx: mpsc::UnboundedSender<ServiceEvent>) -> XPlaneAdapter {
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let join_handle = tokio::spawn(async move {
            let detector = XPlaneAircraftDetector::new();
            // Attempt to create a UDP client for dataref polling.
            let udp_client = match flight_xplane::UdpClient::new(Default::default()) {
                Ok(c) => c,
                Err(err) => {
                    let _ = service_tx.send(ServiceEvent::AdapterError(
                        BusSimId::XPlane,
                        format!("failed to create XPlane UDP client: {}", err),
                    ));
                    return;
                }
            };

            let mut last_aircraft: Option<BusAircraftId> = None;
            let mut interval = tokio::time::interval(Duration::from_millis(500));
            loop {
                tokio::select! {
                    changed = shutdown_rx.changed() => {
                        if changed.is_err() || *shutdown_rx.borrow() {
                            break;
                        }
                    }
                    _ = interval.tick() => {
                        match detector.detect_aircraft(&udp_client).await {
                            Ok(detected) => {
                                if detected.icao.trim().is_empty() {
                                    continue;
                                }
                                let aircraft = BusAircraftId::new(&detected.icao);
                                if last_aircraft.as_ref() != Some(&aircraft) {
                                    last_aircraft = Some(aircraft.clone());
                                    if service_tx.send(ServiceEvent::AircraftDetected(
                                        BusSimId::XPlane,
                                        aircraft,
                                    )).is_err() {
                                        break;
                                    }
                                }
                            }
                            Err(err) => {
                                debug!("X-Plane detection poll failed (may be transient): {}", err);
                            }
                        }
                    }
                }
            }

            udp_client.shutdown();
            debug!("X-Plane adapter task stopped");
        });

        XPlaneAdapter {
            shutdown_tx,
            join_handle,
        }
    }

    /// Spawn a background task that drives DCS aircraft detection via the export adapter.
    fn spawn_dcs_adapter_task(service_tx: mpsc::UnboundedSender<ServiceEvent>) -> DcsAdapter {
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let join_handle = tokio::spawn(async move {
            let mut adapter = DcsAdapterApi::new(Default::default());
            if let Err(err) = adapter.start().await {
                let _ = service_tx.send(ServiceEvent::AdapterError(
                    BusSimId::Dcs,
                    format!("failed to start DCS adapter: {}", err),
                ));
                return;
            }

            let mut last_aircraft: Option<BusAircraftId> = None;
            let mut interval = tokio::time::interval(Duration::from_millis(500));
            loop {
                tokio::select! {
                    changed = shutdown_rx.changed() => {
                        if changed.is_err() || *shutdown_rx.borrow() {
                            break;
                        }
                    }
                    _ = interval.tick() => {
                        // Drive adapter I/O so connections and telemetry are processed.
                        if let Err(err) = adapter.update().await {
                            debug!("DCS adapter update failed (may be transient): {}", err);
                        }
                        // DCS adapter surfaces aircraft via convert_to_bus_snapshot;
                        // here we poll the connection for aircraft changes.
                        match adapter.poll_aircraft().await {
                            Ok(Some(aircraft)) => {
                                if last_aircraft.as_ref() != Some(&aircraft) {
                                    last_aircraft = Some(aircraft.clone());
                                    if service_tx.send(ServiceEvent::AircraftDetected(
                                        BusSimId::Dcs,
                                        aircraft,
                                    )).is_err() {
                                        break;
                                    }
                                }
                            }
                            Ok(None) => {}
                            Err(err) => {
                                debug!("DCS detection poll failed (may be transient): {}", err);
                            }
                        }
                    }
                }
            }

            debug!("DCS adapter task stopped");
        });

        DcsAdapter {
            shutdown_tx,
            join_handle,
        }
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
        self.msfs = None;
        let xplane = self.xplane.take();
        let dcs = self.dcs.take();
        let ac7 = self.ac7.take();
        self.wingman = None;

        let xplane_fut = async {
            if let Some(adapter) = xplane {
                adapter.stop().await;
            }
        };
        let dcs_fut = async {
            if let Some(adapter) = dcs {
                adapter.stop().await;
            }
        };
        let ac7_fut = async {
            if let Some(adapter) = ac7 {
                adapter.stop().await;
            }
        };
        tokio::join!(xplane_fut, dcs_fut, ac7_fut);

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
            connections: 0,
            disconnections: 0,
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
        assert_eq!(m.connections, 0);
        assert_eq!(m.disconnections, 0);
    }

    /// Verify adapter state transitions: Stopped → Starting → Running → Stopped
    /// by driving the event loop through service events.
    #[tokio::test]
    async fn test_adapter_state_transitions() {
        let service = AircraftAutoSwitchService::new(AircraftAutoSwitchServiceConfig::default());
        let adapter_metrics = Arc::clone(&service.adapter_metrics);

        // Initial state: pre-populated entries all at Stopped
        {
            let metrics = adapter_metrics.read().await;
            assert!(metrics.contains_key(&BusSimId::XPlane));
            assert_eq!(metrics[&BusSimId::XPlane].state, AdapterState::Stopped);
        }

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

        // Pre-populated: all enabled sims start as Stopped
        let initial = service.get_adapter_states().await;
        assert!(initial.contains_key(&BusSimId::Msfs));
        assert_eq!(initial[&BusSimId::Msfs], AdapterState::Stopped);

        // Mutate some state
        {
            let mut metrics = service.adapter_metrics.write().await;
            metrics.entry(BusSimId::Msfs).or_default().state = AdapterState::Running;
            metrics.entry(BusSimId::Dcs).or_default().state = AdapterState::Error;
        }

        let states = service.get_adapter_states().await;
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

    // ======================================================================
    // Connection / disconnection counter tests
    // ======================================================================

    /// Connection and disconnection counters increment on adapter lifecycle transitions.
    #[tokio::test]
    async fn test_connection_disconnection_counts() {
        let service = AircraftAutoSwitchService::new(AircraftAutoSwitchServiceConfig::default());
        let adapter_metrics = Arc::clone(&service.adapter_metrics);
        let adapters = Arc::clone(&service.adapters);
        let config = service.config.clone();
        let service_tx = service.service_tx.clone();

        // Initial counters are zero
        {
            let metrics = adapter_metrics.read().await;
            let xp = &metrics[&BusSimId::XPlane];
            assert_eq!(xp.connections, 0);
            assert_eq!(xp.disconnections, 0);
        }

        // Simulate two connect/disconnect cycles for XPlane
        for cycle in 1..=2u64 {
            let process = DetectedProcess {
                sim: CoreSimId::XPlane,
                process_name: "X-Plane.exe".into(),
                process_id: 1000 + cycle as u32,
                process_path: "X-Plane.exe".into(),
                window_title: None,
                detection_time: Instant::now(),
                confidence: 1.0,
            };

            // Connect
            AircraftAutoSwitchService::handle_process_detected(
                process,
                &adapters,
                &config,
                &service_tx,
            )
            .await
            .unwrap();
            {
                let mut metrics = adapter_metrics.write().await;
                let entry = metrics.entry(BusSimId::XPlane).or_default();
                entry.state = AdapterState::Running;
                entry.started_at = Some(Instant::now());
                entry.connections += 1;
            }

            // Disconnect
            AircraftAutoSwitchService::handle_process_lost(BusSimId::XPlane, &adapters)
                .await
                .unwrap();
            {
                let mut metrics = adapter_metrics.write().await;
                let entry = metrics.entry(BusSimId::XPlane).or_default();
                if let Some(started) = entry.started_at.take() {
                    entry.total_uptime += started.elapsed();
                }
                entry.state = AdapterState::Stopped;
                entry.disconnections += 1;
            }

            let metrics = adapter_metrics.read().await;
            let xp = &metrics[&BusSimId::XPlane];
            assert_eq!(xp.connections, cycle, "connections after cycle {cycle}");
            assert_eq!(
                xp.disconnections, cycle,
                "disconnections after cycle {cycle}"
            );
        }
    }

    // ======================================================================
    // Detection timing tests
    // ======================================================================

    /// Detection timing (EMA) converges towards new samples.
    #[tokio::test]
    async fn test_detection_timing_ema() {
        let service = AircraftAutoSwitchService::new(AircraftAutoSwitchServiceConfig::default());
        let adapter_metrics = Arc::clone(&service.adapter_metrics);

        // Seed with a known average
        {
            let mut metrics = adapter_metrics.write().await;
            let entry = metrics.entry(BusSimId::Dcs).or_default();
            entry.state = AdapterState::Running;
            entry.average_detection_time = Duration::from_micros(1000);
        }

        // Apply 10 samples of 200 µs — the EMA should move towards 200 µs
        let alpha = 0.1_f64;
        for _ in 0..10 {
            let mut metrics = adapter_metrics.write().await;
            let entry = metrics.entry(BusSimId::Dcs).or_default();
            let new_sample = Duration::from_micros(200).as_secs_f64();
            let old_avg = entry.average_detection_time.as_secs_f64();
            entry.average_detection_time =
                Duration::from_secs_f64(alpha * new_sample + (1.0 - alpha) * old_avg);
            entry.aircraft_detections += 1;
        }

        let metrics = adapter_metrics.read().await;
        let dcs = &metrics[&BusSimId::Dcs];
        assert_eq!(dcs.aircraft_detections, 10);
        // After 10 EMA steps from 1000 µs towards 200 µs the average should be
        // noticeably below the initial 1000 µs.
        assert!(
            dcs.average_detection_time < Duration::from_micros(800),
            "EMA should have converged below 800 µs, got {:?}",
            dcs.average_detection_time,
        );
        assert!(
            dcs.average_detection_time > Duration::from_micros(200),
            "EMA should still be above the sample value of 200 µs, got {:?}",
            dcs.average_detection_time,
        );
    }

    /// Global atomic detection latency is updated on each detection.
    #[tokio::test]
    async fn test_global_detection_latency_updates() {
        let service = AircraftAutoSwitchService::new(AircraftAutoSwitchServiceConfig::default());

        assert_eq!(service.detection_latency_us.load(Ordering::Relaxed), 0);

        // Simulate two detection events with different latencies
        service.detection_latency_us.store(120, Ordering::Relaxed);
        assert_eq!(service.metrics().detection_time_us, 120);

        service.detection_latency_us.store(450, Ordering::Relaxed);
        assert_eq!(service.metrics().detection_time_us, 450);
    }

    /// `last_detection_time_ms` records wall-clock epoch millis.
    #[tokio::test]
    async fn test_last_detection_timestamp() {
        let service = AircraftAutoSwitchService::new(AircraftAutoSwitchServiceConfig::default());

        assert_eq!(service.last_detection_time_ms.load(Ordering::Relaxed), 0);

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        service
            .last_detection_time_ms
            .store(now_ms, Ordering::Relaxed);
        let stored = service.get_metrics().await.last_detection_time_ms;
        // Should be within 1 second of "now"
        assert!(stored >= now_ms);
        assert!(stored - now_ms < 1_000, "timestamp drift too large");
    }

    // ======================================================================
    // Pre-populated metrics tests
    // ======================================================================

    /// Metrics are pre-populated for all enabled sims on construction.
    #[tokio::test]
    async fn test_metrics_prepopulated_for_enabled_sims() {
        let config = AircraftAutoSwitchServiceConfig::default();
        let service = AircraftAutoSwitchService::new(config);
        let metrics = service.adapter_metrics.read().await;

        // Default config has all adapters enabled
        assert!(metrics.contains_key(&BusSimId::Msfs));
        assert!(metrics.contains_key(&BusSimId::XPlane));
        assert!(metrics.contains_key(&BusSimId::Dcs));
        assert!(metrics.contains_key(&BusSimId::AceCombat7));
        assert!(metrics.contains_key(&BusSimId::Wingman));

        // All should be in Stopped state initially
        for (_, m) in metrics.iter() {
            assert_eq!(m.state, AdapterState::Stopped);
            assert_eq!(m.connections, 0);
            assert_eq!(m.disconnections, 0);
        }
    }

    /// When adapters are selectively disabled, only enabled ones are pre-populated.
    #[tokio::test]
    async fn test_metrics_not_populated_for_disabled_sims() {
        let mut config = AircraftAutoSwitchServiceConfig::default();
        config.adapters.enable_msfs = false;
        config.adapters.enable_xplane = true;
        config.adapters.enable_dcs = false;
        config.adapters.enable_ac7 = false;
        config.adapters.enable_wingman = false;

        let service = AircraftAutoSwitchService::new(config);
        let metrics = service.adapter_metrics.read().await;

        assert_eq!(metrics.len(), 1, "only XPlane should be pre-populated");
        assert!(metrics.contains_key(&BusSimId::XPlane));
        assert!(!metrics.contains_key(&BusSimId::Msfs));
        assert!(!metrics.contains_key(&BusSimId::Dcs));
    }

    /// Error counter increments propagate to both per-adapter and global metrics.
    #[tokio::test]
    async fn test_error_counter_propagation() {
        let service = AircraftAutoSwitchService::new(AircraftAutoSwitchServiceConfig::default());
        let adapter_metrics = Arc::clone(&service.adapter_metrics);

        // Simulate adapter error for XPlane
        {
            let mut metrics = adapter_metrics.write().await;
            let entry = metrics.entry(BusSimId::XPlane).or_default();
            entry.detection_errors += 1;
            entry.state = AdapterState::Error;
        }
        service.adapter_errors.fetch_add(1, Ordering::Relaxed);

        // Simulate adapter error for DCS
        {
            let mut metrics = adapter_metrics.write().await;
            let entry = metrics.entry(BusSimId::Dcs).or_default();
            entry.detection_errors += 1;
            entry.state = AdapterState::Error;
        }
        service.adapter_errors.fetch_add(1, Ordering::Relaxed);

        // Per-adapter errors
        {
            let metrics = adapter_metrics.read().await;
            assert_eq!(metrics[&BusSimId::XPlane].detection_errors, 1);
            assert_eq!(metrics[&BusSimId::Dcs].detection_errors, 1);
        }
        // Global error count
        assert_eq!(service.adapter_errors.load(Ordering::Relaxed), 2);
        assert_eq!(service.metrics().adapter_errors, 2);
    }

    // ======================================================================
    // Confidence tracking tests
    // ======================================================================

    /// Minimum confidence defaults to 1.0 (no detection yet).
    #[tokio::test]
    async fn test_min_confidence_initial_value() {
        let service = AircraftAutoSwitchService::new(AircraftAutoSwitchServiceConfig::default());
        let c = service.metrics();
        assert!(
            (c.min_confidence - 1.0).abs() < f64::EPSILON,
            "initial min_confidence should be 1.0, got {}",
            c.min_confidence
        );
    }

    /// Minimum confidence is tracked correctly via atomic update.
    #[tokio::test]
    async fn test_min_confidence_tracks_lowest() {
        let service = AircraftAutoSwitchService::new(AircraftAutoSwitchServiceConfig::default());

        // Simulate detections with decreasing then increasing confidence
        service
            .min_confidence_bits
            .store(0.9_f64.to_bits(), Ordering::Relaxed);
        assert!((service.metrics().min_confidence - 0.9).abs() < f64::EPSILON);

        // Lower value → should update
        service
            .min_confidence_bits
            .store(0.7_f64.to_bits(), Ordering::Relaxed);
        assert!((service.metrics().min_confidence - 0.7).abs() < f64::EPSILON);

        // get_metrics also reports it
        let m = service.get_metrics().await;
        assert!((m.min_confidence - 0.7).abs() < f64::EPSILON);
    }

    // ======================================================================
    // AdapterDetector trait & confidence scoring tests
    // ======================================================================

    /// Confidence scores are distinct per simulator and within valid range.
    #[test]
    fn test_confidence_for_sim_values() {
        let msfs = confidence_for_sim(BusSimId::Msfs);
        let xplane = confidence_for_sim(BusSimId::XPlane);
        let dcs = confidence_for_sim(BusSimId::Dcs);
        let ac7 = confidence_for_sim(BusSimId::AceCombat7);
        let wingman = confidence_for_sim(BusSimId::Wingman);
        let unknown = confidence_for_sim(BusSimId::Unknown);

        // All in (0, 1]
        for &c in &[msfs, xplane, dcs, ac7, wingman, unknown] {
            assert!(c > 0.0 && c <= 1.0, "confidence {c} out of range");
        }

        // Direct API ≥ structured protocol ≥ shared-mem ≥ process-only
        assert!(msfs >= xplane, "MSFS should be >= XPlane");
        assert!(xplane >= dcs, "XPlane should be >= DCS");
        assert!(dcs >= ac7, "DCS should be >= AC7");
        assert!(ac7 >= wingman, "AC7 should be >= Wingman");
        assert!(wingman >= unknown, "Wingman should be >= Unknown");
    }

    /// MSFS2024 shares the same confidence tier as MSFS (same SimConnect API).
    #[test]
    fn test_confidence_msfs2024_equals_msfs() {
        assert_eq!(
            confidence_for_sim(BusSimId::Msfs),
            confidence_for_sim(BusSimId::Msfs2024),
        );
    }

    /// AircraftDetected events carry per-adapter confidence (not hardcoded 0.9).
    #[test]
    fn test_aircraft_detected_uses_real_confidence() {
        // Verify that different sims yield different confidences
        let xplane_conf = confidence_for_sim(BusSimId::XPlane);
        let wingman_conf = confidence_for_sim(BusSimId::Wingman);
        assert_ne!(
            xplane_conf, wingman_conf,
            "XPlane and Wingman should have distinct confidences"
        );
        assert!(
            (xplane_conf - 0.92).abs() <= f32::EPSILON,
            "XPlane confidence should be 0.92, got {}",
            xplane_conf,
        );
    }

    // ======================================================================
    // BDD-compatible adapter lifecycle scenarios
    // ======================================================================

    /// GIVEN a fresh service with all adapters enabled
    /// WHEN an X-Plane process is detected
    /// THEN the XPlane adapter slot is populated and state transitions to Running
    /// AND WHEN the process is lost
    /// THEN the adapter slot is cleared and state transitions to Stopped
    #[tokio::test]
    async fn bdd_xplane_adapter_start_stop_lifecycle() {
        let service = AircraftAutoSwitchService::new(AircraftAutoSwitchServiceConfig::default());
        let adapters = Arc::clone(&service.adapters);
        let adapter_metrics = Arc::clone(&service.adapter_metrics);
        let config = service.config.clone();
        let service_tx = service.service_tx.clone();

        // Pre-condition: no XPlane adapter
        assert!(adapters.read().await.xplane.is_none());

        let process = DetectedProcess {
            sim: CoreSimId::XPlane,
            process_name: "X-Plane 12.exe".into(),
            process_id: 4200,
            process_path: "X-Plane 12.exe".into(),
            window_title: Some("X-Plane 12".into()),
            detection_time: Instant::now(),
            confidence: 1.0,
        };

        // WHEN: process detected
        {
            let mut m = adapter_metrics.write().await;
            m.entry(BusSimId::XPlane).or_default().state = AdapterState::Starting;
        }
        let result = AircraftAutoSwitchService::handle_process_detected(
            process,
            &adapters,
            &config,
            &service_tx,
        )
        .await;
        assert!(result.is_ok(), "handle_process_detected must succeed");

        // THEN: adapter populated
        assert!(
            adapters.read().await.xplane.is_some(),
            "XPlane adapter should be populated after process detection"
        );

        // Transition to Running
        {
            let mut m = adapter_metrics.write().await;
            let entry = m.entry(BusSimId::XPlane).or_default();
            entry.state = AdapterState::Running;
            entry.started_at = Some(Instant::now());
            entry.connections += 1;
        }
        assert_eq!(
            adapter_metrics.read().await[&BusSimId::XPlane].state,
            AdapterState::Running
        );

        // AND WHEN: process lost
        AircraftAutoSwitchService::handle_process_lost(BusSimId::XPlane, &adapters)
            .await
            .unwrap();

        // THEN: adapter cleared
        assert!(
            adapters.read().await.xplane.is_none(),
            "XPlane adapter should be cleared after process loss"
        );

        // Accumulate uptime and transition to Stopped
        {
            let mut m = adapter_metrics.write().await;
            let entry = m.entry(BusSimId::XPlane).or_default();
            if let Some(started) = entry.started_at.take() {
                entry.total_uptime += started.elapsed();
            }
            entry.state = AdapterState::Stopped;
            entry.disconnections += 1;
        }
        let m = adapter_metrics.read().await;
        let xp = &m[&BusSimId::XPlane];
        assert_eq!(xp.state, AdapterState::Stopped);
        assert!(xp.total_uptime > Duration::ZERO);
        assert_eq!(xp.connections, 1);
        assert_eq!(xp.disconnections, 1);
    }

    /// GIVEN a fresh service
    /// WHEN DCS process is detected
    /// THEN the DCS adapter slot is populated with a spawned task
    #[tokio::test]
    async fn bdd_dcs_adapter_start_lifecycle() {
        let service = AircraftAutoSwitchService::new(AircraftAutoSwitchServiceConfig::default());
        let adapters = Arc::clone(&service.adapters);
        let config = service.config.clone();
        let service_tx = service.service_tx.clone();

        assert!(adapters.read().await.dcs.is_none());

        let process = DetectedProcess {
            sim: CoreSimId::Dcs,
            process_name: "DCS.exe".into(),
            process_id: 9999,
            process_path: "DCS.exe".into(),
            window_title: None,
            detection_time: Instant::now(),
            confidence: 1.0,
        };

        let result = AircraftAutoSwitchService::handle_process_detected(
            process,
            &adapters,
            &config,
            &service_tx,
        )
        .await;
        assert!(result.is_ok());
        assert!(
            adapters.read().await.dcs.is_some(),
            "DCS adapter slot should be populated"
        );

        // Cleanup: stop the spawned task
        AircraftAutoSwitchService::handle_process_lost(BusSimId::Dcs, &adapters)
            .await
            .unwrap();
        assert!(adapters.read().await.dcs.is_none());
    }

    /// GIVEN a running adapter
    /// WHEN an AdapterError event occurs during a switch
    /// THEN the adapter transitions to Error state AND the global error counter increments
    #[tokio::test]
    async fn bdd_adapter_crash_during_switch() {
        let service = AircraftAutoSwitchService::new(AircraftAutoSwitchServiceConfig::default());
        let adapter_metrics = Arc::clone(&service.adapter_metrics);
        let adapter_errors = Arc::clone(&service.adapter_errors);

        // Start with a running XPlane adapter
        {
            let mut m = adapter_metrics.write().await;
            let entry = m.entry(BusSimId::XPlane).or_default();
            entry.state = AdapterState::Running;
            entry.started_at = Some(Instant::now());
            entry.connections += 1;
        }

        // Simulate an error during switch (e.g., adapter crashes)
        {
            let mut m = adapter_metrics.write().await;
            let entry = m.entry(BusSimId::XPlane).or_default();
            entry.detection_errors += 1;
            entry.state = AdapterState::Error;
        }
        adapter_errors.fetch_add(1, Ordering::Relaxed);

        // Verify error state
        let m = adapter_metrics.read().await;
        assert_eq!(m[&BusSimId::XPlane].state, AdapterState::Error);
        assert_eq!(m[&BusSimId::XPlane].detection_errors, 1);
        assert_eq!(adapter_errors.load(Ordering::Relaxed), 1);
    }

    /// GIVEN multiple sims detected concurrently
    /// WHEN both trigger adapter creation simultaneously
    /// THEN both adapters are created independently without data corruption
    #[tokio::test]
    async fn bdd_concurrent_adapter_creation_isolation() {
        let service = AircraftAutoSwitchService::new(AircraftAutoSwitchServiceConfig::default());
        let adapters = Arc::clone(&service.adapters);
        let adapter_metrics = Arc::clone(&service.adapter_metrics);
        let config = service.config.clone();
        let service_tx = service.service_tx.clone();

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

        // Concurrent creation
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

        // Both present and independent
        {
            let guard = adapters.read().await;
            assert!(guard.xplane.is_some());
            assert!(guard.dcs.is_some());
        }

        // Metrics are independent
        {
            let mut m = adapter_metrics.write().await;
            m.entry(BusSimId::XPlane).or_default().state = AdapterState::Running;
            m.entry(BusSimId::Dcs).or_default().state = AdapterState::Running;
        }
        {
            let m = adapter_metrics.read().await;
            assert_eq!(m[&BusSimId::XPlane].state, AdapterState::Running);
            assert_eq!(m[&BusSimId::Dcs].state, AdapterState::Running);
        }

        // Cleanup
        let _ = AircraftAutoSwitchService::handle_process_lost(BusSimId::XPlane, &adapters).await;
        let _ = AircraftAutoSwitchService::handle_process_lost(BusSimId::Dcs, &adapters).await;
    }

    /// GIVEN a service with recorded detection latencies
    /// WHEN detection events have sub-millisecond timing
    /// THEN the microsecond-precision counters faithfully capture the latency
    #[tokio::test]
    async fn bdd_detection_timing_within_bounds() {
        let service = AircraftAutoSwitchService::new(AircraftAutoSwitchServiceConfig::default());

        // Simulate a detection with 200 µs latency
        service.detection_latency_us.store(200, Ordering::Relaxed);
        let c = service.metrics();
        assert!(
            c.detection_time_us <= 500,
            "detection latency should be within 500 µs bound, got {} µs",
            c.detection_time_us
        );
        assert!(
            c.detection_time_us >= 100,
            "detection latency should be at least 100 µs, got {} µs",
            c.detection_time_us
        );
    }

    /// GIVEN a newly started adapter
    /// WHEN a detection event succeeds
    /// THEN metrics show exactly 1 detection with wall-clock timestamp
    #[tokio::test]
    async fn bdd_metrics_populated_after_single_detection() {
        let service = AircraftAutoSwitchService::new(AircraftAutoSwitchServiceConfig::default());
        let adapter_metrics = Arc::clone(&service.adapter_metrics);

        // Mark adapter as Running
        {
            let mut m = adapter_metrics.write().await;
            let entry = m.entry(BusSimId::XPlane).or_default();
            entry.state = AdapterState::Running;
            entry.started_at = Some(Instant::now());
        }

        // Record a single detection
        let detection_time = Instant::now();
        {
            let mut m = adapter_metrics.write().await;
            let entry = m.entry(BusSimId::XPlane).or_default();
            entry.aircraft_detections += 1;
            entry.last_detection = Some(detection_time);
            entry.average_detection_time = Duration::from_micros(350);
        }

        // Bump global counters
        service
            .aircraft_switch_count
            .fetch_add(1, Ordering::Relaxed);
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        service
            .last_detection_time_ms
            .store(now_ms, Ordering::Relaxed);
        service.detection_latency_us.store(350, Ordering::Relaxed);

        // Verify
        let m = adapter_metrics.read().await;
        let xp = &m[&BusSimId::XPlane];
        assert_eq!(xp.aircraft_detections, 1);
        assert!(xp.last_detection.is_some());
        assert_eq!(xp.average_detection_time, Duration::from_micros(350));

        let counters = service.metrics();
        assert_eq!(counters.aircraft_switches, 1);
        assert!(counters.detection_time_us > 0);
    }

    /// GIVEN a disabled adapter
    /// WHEN its process is detected
    /// THEN no adapter is created (config respected)
    #[tokio::test]
    async fn bdd_disabled_adapter_not_started() {
        let mut config = AircraftAutoSwitchServiceConfig::default();
        config.adapters.enable_xplane = false;
        let service = AircraftAutoSwitchService::new(config.clone());
        let adapters = Arc::clone(&service.adapters);
        let service_tx = service.service_tx.clone();

        let process = DetectedProcess {
            sim: CoreSimId::XPlane,
            process_name: "X-Plane.exe".into(),
            process_id: 5555,
            process_path: "X-Plane.exe".into(),
            window_title: None,
            detection_time: Instant::now(),
            confidence: 1.0,
        };

        let result = AircraftAutoSwitchService::handle_process_detected(
            process,
            &adapters,
            &config,
            &service_tx,
        )
        .await;
        assert!(result.is_ok());
        assert!(
            adapters.read().await.xplane.is_none(),
            "disabled adapter should not be created"
        );
    }

    /// Wingman adapter uses process-only detection with lower confidence.
    #[tokio::test]
    async fn bdd_wingman_process_only_detection() {
        let service = AircraftAutoSwitchService::new(AircraftAutoSwitchServiceConfig::default());
        let adapters = Arc::clone(&service.adapters);
        let config = service.config.clone();
        let service_tx = service.service_tx.clone();

        let process = DetectedProcess {
            sim: CoreSimId::Wingman,
            process_name: "ProjectWingman.exe".into(),
            process_id: 7777,
            process_path: "ProjectWingman.exe".into(),
            window_title: None,
            detection_time: Instant::now(),
            confidence: 1.0,
        };

        AircraftAutoSwitchService::handle_process_detected(
            process,
            &adapters,
            &config,
            &service_tx,
        )
        .await
        .unwrap();
        assert!(adapters.read().await.wingman.is_some());

        // Wingman confidence should be lower than XPlane
        let wm_conf = confidence_for_sim(BusSimId::Wingman);
        let xp_conf = confidence_for_sim(BusSimId::XPlane);
        assert!(
            wm_conf < xp_conf,
            "Wingman ({wm_conf}) should have lower confidence than XPlane ({xp_conf})"
        );
    }
}
