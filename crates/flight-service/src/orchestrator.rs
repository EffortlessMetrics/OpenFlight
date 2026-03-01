// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Service Orchestrator
//!
//! Top-level runtime coordinator that wires all subsystems together,
//! enforces boot-sequence ordering and dependency-aware shutdown.
//!
//! # Boot sequence
//!
//! 1. Bus initialisation
//! 2. Scheduler start
//! 3. Adapter start (depends on bus)
//! 4. Watchdog start (after all subsystems)
//!
//! Shutdown proceeds in reverse order.

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::config_watcher::{ChangeType, ConfigWatcher};

// ---------------------------------------------------------------------------
// Boot sequence state machine
// ---------------------------------------------------------------------------

/// Ordered boot phases for the service orchestrator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BootSequence {
    /// Not yet started.
    Initializing,
    /// Event bus is ready.
    BusReady,
    /// RT scheduler is ready.
    SchedulerReady,
    /// Sim adapters are ready.
    AdaptersReady,
    /// All subsystems running.
    Running,
    /// Graceful shutdown in progress.
    ShuttingDown,
    /// Fully stopped.
    Stopped,
}

impl BootSequence {
    /// Valid successor states.
    fn valid_next(self) -> &'static [BootSequence] {
        match self {
            Self::Initializing => &[Self::BusReady, Self::Stopped],
            Self::BusReady => &[Self::SchedulerReady, Self::ShuttingDown, Self::Stopped],
            Self::SchedulerReady => &[Self::AdaptersReady, Self::ShuttingDown, Self::Stopped],
            Self::AdaptersReady => &[Self::Running, Self::ShuttingDown, Self::Stopped],
            Self::Running => &[Self::ShuttingDown],
            Self::ShuttingDown => &[Self::Stopped],
            Self::Stopped => &[Self::Initializing],
        }
    }

    /// Whether transitioning to `next` is allowed.
    pub fn can_transition_to(self, next: Self) -> bool {
        self.valid_next().contains(&next)
    }
}

impl fmt::Display for BootSequence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Initializing => "Initializing",
            Self::BusReady => "BusReady",
            Self::SchedulerReady => "SchedulerReady",
            Self::AdaptersReady => "AdaptersReady",
            Self::Running => "Running",
            Self::ShuttingDown => "ShuttingDown",
            Self::Stopped => "Stopped",
        };
        f.write_str(s)
    }
}

// ---------------------------------------------------------------------------
// Subsystem health
// ---------------------------------------------------------------------------

/// Health state of a single subsystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubsystemHealth {
    Healthy,
    Degraded,
    Failed,
    Unknown,
}

// ---------------------------------------------------------------------------
// SubsystemHandle
// ---------------------------------------------------------------------------

/// Generic lifecycle wrapper for a subsystem.
#[derive(Debug, Clone)]
pub struct SubsystemHandle {
    name: String,
    running: bool,
    health: SubsystemHealth,
    start_time: Option<Instant>,
    error_count: u64,
    last_error: Option<String>,
}

impl SubsystemHandle {
    /// Create a new handle for the given subsystem name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            running: false,
            health: SubsystemHealth::Unknown,
            start_time: None,
            error_count: 0,
            last_error: None,
        }
    }

    /// Start the subsystem. Returns `Err` if already running.
    pub fn start(&mut self) -> Result<(), OrchestratorError> {
        if self.running {
            return Err(OrchestratorError::SubsystemAlreadyRunning(
                self.name.clone(),
            ));
        }
        self.running = true;
        self.health = SubsystemHealth::Healthy;
        self.start_time = Some(Instant::now());
        self.last_error = None;
        Ok(())
    }

    /// Stop the subsystem. Returns `Err` if not running.
    pub fn stop(&mut self) -> Result<(), OrchestratorError> {
        if !self.running {
            return Err(OrchestratorError::SubsystemNotRunning(self.name.clone()));
        }
        self.running = false;
        self.health = SubsystemHealth::Unknown;
        Ok(())
    }

    /// Current health.
    pub fn health(&self) -> SubsystemHealth {
        self.health
    }

    /// Whether the subsystem is running.
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Subsystem name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Cumulative error count.
    pub fn error_count(&self) -> u64 {
        self.error_count
    }

    /// The most recent error message, if any.
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// How long the subsystem has been running.
    pub fn uptime(&self) -> Option<std::time::Duration> {
        self.start_time.map(|t| t.elapsed())
    }

    /// Record an error and transition health to `Degraded`.
    pub fn record_error(&mut self, msg: impl Into<String>) {
        self.error_count += 1;
        self.last_error = Some(msg.into());
        if self.health == SubsystemHealth::Healthy {
            self.health = SubsystemHealth::Degraded;
        }
    }

    /// Mark the subsystem as failed.
    pub fn mark_failed(&mut self, msg: impl Into<String>) {
        self.error_count += 1;
        self.last_error = Some(msg.into());
        self.health = SubsystemHealth::Failed;
        self.running = false;
    }

    /// Recover a degraded subsystem back to healthy.
    pub fn recover(&mut self) {
        if self.health == SubsystemHealth::Degraded {
            self.health = SubsystemHealth::Healthy;
            self.last_error = None;
        }
    }
}

// ---------------------------------------------------------------------------
// Orchestrator status
// ---------------------------------------------------------------------------

/// Aggregated status snapshot for the orchestrator.
#[derive(Debug, Clone)]
pub struct OrchestratorStatus {
    pub boot_phase: BootSequence,
    pub subsystems: HashMap<String, SubsystemStatus>,
    pub overall_health: SubsystemHealth,
}

/// Per-subsystem status entry.
#[derive(Debug, Clone)]
pub struct SubsystemStatus {
    pub running: bool,
    pub health: SubsystemHealth,
    pub error_count: u64,
    pub last_error: Option<String>,
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// A device connect / disconnect event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceEvent {
    Connected {
        device_id: String,
        device_type: String,
    },
    Disconnected {
        device_id: String,
    },
}

/// A simulator adapter event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterEvent {
    SimConnected { sim_name: String },
    SimDisconnected { sim_name: String },
    DataReceived { sim_name: String },
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors the orchestrator may return.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrchestratorError {
    InvalidTransition {
        from: BootSequence,
        to: BootSequence,
    },
    SubsystemAlreadyRunning(String),
    SubsystemNotRunning(String),
    SubsystemNotFound(String),
    SubsystemStartFailed {
        name: String,
        reason: String,
    },
    StartFailed(String),
    AlreadyRunning,
    NotRunning,
}

impl fmt::Display for OrchestratorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTransition { from, to } => {
                write!(f, "invalid transition {from} -> {to}")
            }
            Self::SubsystemAlreadyRunning(n) => write!(f, "subsystem '{n}' already running"),
            Self::SubsystemNotRunning(n) => write!(f, "subsystem '{n}' not running"),
            Self::SubsystemNotFound(n) => write!(f, "subsystem '{n}' not found"),
            Self::SubsystemStartFailed { name, reason } => {
                write!(f, "subsystem '{name}' failed to start: {reason}")
            }
            Self::StartFailed(msg) => write!(f, "start failed: {msg}"),
            Self::AlreadyRunning => f.write_str("orchestrator is already running"),
            Self::NotRunning => f.write_str("orchestrator is not running"),
        }
    }
}

impl std::error::Error for OrchestratorError {}

// ---------------------------------------------------------------------------
// ServiceConfig (orchestrator-level)
// ---------------------------------------------------------------------------

/// Configuration for the orchestrator.
#[derive(Debug, Clone)]
pub struct ServiceConfig {
    /// Global shutdown timeout in milliseconds.
    pub shutdown_timeout_ms: u64,
    /// Whether to enable the watchdog subsystem.
    pub enable_watchdog: bool,
    /// Whether to enable simulator adapters.
    pub enable_adapters: bool,
    /// Per-simulator adapter enable flags.
    pub adapter_flags: AdapterFlags,
    /// Optional path to the service configuration file for hot-reload.
    pub config_path: Option<PathBuf>,
}

/// Per-simulator adapter enable flags.
#[derive(Debug, Clone, Default)]
pub struct AdapterFlags {
    pub msfs: bool,
    pub xplane: bool,
    pub dcs: bool,
    pub ac7: bool,
    pub wingman: bool,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            shutdown_timeout_ms: 5_000,
            enable_watchdog: true,
            enable_adapters: true,
            adapter_flags: AdapterFlags {
                msfs: true,
                xplane: true,
                dcs: true,
                ac7: true,
                wingman: true,
            },
            config_path: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Profile wrapper (for hot-swap)
// ---------------------------------------------------------------------------

/// A compiled profile ready to be swapped into the RT spine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledProfile {
    pub name: String,
    pub version: u64,
    /// Number of axis pipelines included in this compiled profile.
    pub axis_count: usize,
    /// Simulator filter from the source profile (e.g. "msfs", "xplane").
    pub sim_filter: Option<String>,
    /// Aircraft filter from the source profile (e.g. "C172").
    pub aircraft_filter: Option<String>,
}

// ---------------------------------------------------------------------------
// Orchestrator metrics
// ---------------------------------------------------------------------------

/// Aggregated metrics collected by the orchestrator.
#[derive(Debug, Clone)]
pub struct OrchestratorMetrics {
    /// Total profile swaps performed since boot.
    pub profile_swaps: u64,
    /// Total device connect events.
    pub device_connects: u64,
    /// Total device disconnect events.
    pub device_disconnects: u64,
    /// Total adapter connect events.
    pub adapter_connects: u64,
    /// Total adapter disconnect events.
    pub adapter_disconnects: u64,
    /// Total config-reload attempts.
    pub config_reloads: u64,
    /// Total subsystem restarts.
    pub subsystem_restarts: u64,
    /// Per-adapter state map.
    pub adapter_states: HashMap<String, AdapterLifecycleState>,
    /// Orchestrator uptime since last start.
    pub uptime: Option<Duration>,
}

/// Lifecycle state of a single simulator adapter tracked by the orchestrator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterLifecycleState {
    Disabled,
    Stopped,
    Running,
    Error(String),
}

// ---------------------------------------------------------------------------
// ServiceOrchestrator
// ---------------------------------------------------------------------------

/// Well-known subsystem names.
pub const SUBSYSTEM_BUS: &str = "bus";
pub const SUBSYSTEM_SCHEDULER: &str = "scheduler";
pub const SUBSYSTEM_ADAPTERS: &str = "adapters";
pub const SUBSYSTEM_WATCHDOG: &str = "watchdog";

/// Top-level runtime coordinator.
///
/// Manages the lifecycle of all subsystems, enforces boot-order
/// dependencies, and provides aggregated health status.
pub struct ServiceOrchestrator {
    config: ServiceConfig,
    phase: BootSequence,
    subsystems: HashMap<String, SubsystemHandle>,
    /// Boot order defines the start sequence; shutdown is the reverse.
    boot_order: Vec<String>,
    active_profile: Option<CompiledProfile>,
    profile_version: u64,
    connected_devices: HashMap<String, String>,
    connected_sims: Vec<String>,
    /// Per-adapter lifecycle state.
    adapter_states: HashMap<String, AdapterLifecycleState>,
    /// Config watcher for hot-reload.
    config_watcher: Option<ConfigWatcher>,
    /// Cumulative counters.
    metrics: InternalMetrics,
    /// Instant when the orchestrator entered `Running`.
    started_at: Option<Instant>,
}

/// Internal mutable counters.
struct InternalMetrics {
    profile_swaps: u64,
    device_connects: u64,
    device_disconnects: u64,
    adapter_connects: u64,
    adapter_disconnects: u64,
    config_reloads: u64,
    subsystem_restarts: u64,
}

impl ServiceOrchestrator {
    /// Create a new orchestrator from the given configuration.
    pub fn new(config: ServiceConfig) -> Self {
        let mut subsystems = HashMap::new();
        let mut boot_order = Vec::new();

        // Register subsystems in dependency order.
        for name in [SUBSYSTEM_BUS, SUBSYSTEM_SCHEDULER] {
            subsystems.insert(name.to_string(), SubsystemHandle::new(name));
            boot_order.push(name.to_string());
        }

        if config.enable_adapters {
            subsystems.insert(
                SUBSYSTEM_ADAPTERS.to_string(),
                SubsystemHandle::new(SUBSYSTEM_ADAPTERS),
            );
            boot_order.push(SUBSYSTEM_ADAPTERS.to_string());
        }

        if config.enable_watchdog {
            subsystems.insert(
                SUBSYSTEM_WATCHDOG.to_string(),
                SubsystemHandle::new(SUBSYSTEM_WATCHDOG),
            );
            boot_order.push(SUBSYSTEM_WATCHDOG.to_string());
        }

        // Initialise per-adapter lifecycle states from flags.
        let adapter_states = Self::init_adapter_states(&config);

        // Set up config watcher if a config path was provided.
        let config_watcher = config.config_path.as_ref().map(|path| {
            let mut watcher = ConfigWatcher::new(Duration::from_secs(2));
            watcher.watch(path);
            watcher
        });

        Self {
            config,
            phase: BootSequence::Initializing,
            subsystems,
            boot_order,
            active_profile: None,
            profile_version: 0,
            connected_devices: HashMap::new(),
            connected_sims: Vec::new(),
            adapter_states,
            config_watcher,
            metrics: InternalMetrics {
                profile_swaps: 0,
                device_connects: 0,
                device_disconnects: 0,
                adapter_connects: 0,
                adapter_disconnects: 0,
                config_reloads: 0,
                subsystem_restarts: 0,
            },
            started_at: None,
        }
    }

    /// Build a new orchestrator from a [`FlightServiceConfig`](crate::FlightServiceConfig).
    ///
    /// This bridges the service-level config to the orchestrator, extracting
    /// adapter enable flags and config-file path.
    pub fn from_service_config(
        svc: &crate::service::FlightServiceConfig,
        config_path: Option<PathBuf>,
    ) -> Self {
        let cfg = ServiceConfig {
            shutdown_timeout_ms: 5_000,
            enable_watchdog: true,
            enable_adapters: true,
            adapter_flags: AdapterFlags {
                msfs: svc.auto_switch_config.adapters.enable_msfs,
                xplane: svc.auto_switch_config.adapters.enable_xplane,
                dcs: svc.auto_switch_config.adapters.enable_dcs,
                ac7: svc.auto_switch_config.adapters.enable_ac7,
                wingman: svc.auto_switch_config.adapters.enable_wingman,
            },
            config_path,
        };
        Self::new(cfg)
    }

    fn init_adapter_states(config: &ServiceConfig) -> HashMap<String, AdapterLifecycleState> {
        let mut states = HashMap::new();
        if !config.enable_adapters {
            return states;
        }
        let flags = [
            ("msfs", config.adapter_flags.msfs),
            ("xplane", config.adapter_flags.xplane),
            ("dcs", config.adapter_flags.dcs),
            ("ac7", config.adapter_flags.ac7),
            ("wingman", config.adapter_flags.wingman),
        ];
        for (name, enabled) in flags {
            states.insert(
                name.to_string(),
                if enabled {
                    AdapterLifecycleState::Stopped
                } else {
                    AdapterLifecycleState::Disabled
                },
            );
        }
        states
    }

    // -- lifecycle -----------------------------------------------------------

    /// Boot the service: init bus → start scheduler → start adapters → start
    /// watchdog.  Each step transitions the boot-sequence state machine.
    pub fn start(&mut self) -> Result<(), OrchestratorError> {
        if self.phase == BootSequence::Running {
            return Err(OrchestratorError::AlreadyRunning);
        }

        // Bus
        self.transition(BootSequence::BusReady)?;
        self.start_subsystem(SUBSYSTEM_BUS)?;

        // Scheduler
        self.transition(BootSequence::SchedulerReady)?;
        self.start_subsystem(SUBSYSTEM_SCHEDULER)?;

        // Adapters (optional)
        if self.config.enable_adapters {
            self.transition(BootSequence::AdaptersReady)?;
            self.start_subsystem(SUBSYSTEM_ADAPTERS)?;
        } else {
            self.transition(BootSequence::AdaptersReady)?;
        }

        // Watchdog (optional, always last)
        if self.config.enable_watchdog {
            self.start_subsystem(SUBSYSTEM_WATCHDOG)?;
        }

        self.transition(BootSequence::Running)?;
        self.started_at = Some(Instant::now());

        // Mark enabled adapters as Running after the adapters subsystem starts.
        if self.config.enable_adapters {
            for (name, state) in &mut self.adapter_states {
                if *state == AdapterLifecycleState::Stopped {
                    *state = AdapterLifecycleState::Running;
                    let _ = name; // used for logging in production
                }
            }
        }

        Ok(())
    }

    /// Graceful shutdown in reverse boot order.
    pub fn stop(&mut self) -> Result<(), OrchestratorError> {
        if self.phase == BootSequence::Stopped || self.phase == BootSequence::Initializing {
            return Err(OrchestratorError::NotRunning);
        }

        self.transition(BootSequence::ShuttingDown)?;

        // Stop in reverse order, ignoring subsystems that are not running.
        let reverse_order: Vec<String> = self.boot_order.iter().rev().cloned().collect();
        for name in &reverse_order {
            if let Some(handle) = self.subsystems.get_mut(name)
                && handle.is_running()
            {
                let _ = handle.stop();
            }
        }

        self.phase = BootSequence::Stopped;

        // Reset adapter states to Stopped (keep Disabled as-is).
        for state in self.adapter_states.values_mut() {
            if *state != AdapterLifecycleState::Disabled {
                *state = AdapterLifecycleState::Stopped;
            }
        }
        self.started_at = None;

        Ok(())
    }

    // -- status --------------------------------------------------------------

    /// Aggregate status from all subsystems.
    pub fn status(&self) -> OrchestratorStatus {
        let mut subsystem_map = HashMap::new();
        let mut worst = SubsystemHealth::Healthy;

        for (name, handle) in &self.subsystems {
            let h = handle.health();
            if health_severity(h) > health_severity(worst) {
                worst = h;
            }
            subsystem_map.insert(
                name.clone(),
                SubsystemStatus {
                    running: handle.is_running(),
                    health: h,
                    error_count: handle.error_count(),
                    last_error: handle.last_error().map(String::from),
                },
            );
        }

        OrchestratorStatus {
            boot_phase: self.phase,
            subsystems: subsystem_map,
            overall_health: worst,
        }
    }

    /// Current boot phase.
    pub fn phase(&self) -> BootSequence {
        self.phase
    }

    /// Whether the orchestrator is fully running.
    pub fn is_running(&self) -> bool {
        self.phase == BootSequence::Running
    }

    // -- profile hot-swap ----------------------------------------------------

    /// Compile a profile off-thread (simulated) and swap it atomically.
    ///
    /// In production the compilation happens on a background thread; here
    /// we simulate the two-phase commit for testability.
    pub fn handle_profile_change(
        &mut self,
        profile_name: &str,
    ) -> Result<CompiledProfile, OrchestratorError> {
        if self.phase != BootSequence::Running {
            return Err(OrchestratorError::NotRunning);
        }

        self.profile_version += 1;
        let compiled = CompiledProfile {
            name: profile_name.to_string(),
            version: self.profile_version,
            axis_count: 0,
            sim_filter: None,
            aircraft_filter: None,
        };
        self.active_profile = Some(compiled.clone());
        self.metrics.profile_swaps += 1;
        Ok(compiled)
    }

    /// Compile and swap a full [`Profile`](flight_core::profile::Profile).
    ///
    /// Populates the compiled profile with axis count, sim, and aircraft
    /// metadata from the source profile.
    pub fn handle_profile_update(
        &mut self,
        profile: &flight_core::profile::Profile,
    ) -> Result<CompiledProfile, OrchestratorError> {
        if self.phase != BootSequence::Running {
            return Err(OrchestratorError::NotRunning);
        }

        self.profile_version += 1;
        let compiled = CompiledProfile {
            name: profile
                .aircraft
                .as_ref()
                .map(|a| a.icao.clone())
                .unwrap_or_else(|| "default".to_string()),
            version: self.profile_version,
            axis_count: profile.axes.len(),
            sim_filter: profile.sim.clone(),
            aircraft_filter: profile.aircraft.as_ref().map(|a| a.icao.clone()),
        };
        self.active_profile = Some(compiled.clone());
        self.metrics.profile_swaps += 1;
        Ok(compiled)
    }

    /// Currently active compiled profile, if any.
    pub fn active_profile(&self) -> Option<&CompiledProfile> {
        self.active_profile.as_ref()
    }

    // -- device events -------------------------------------------------------

    /// Handle a device connect or disconnect event.
    pub fn handle_device_change(&mut self, event: DeviceEvent) -> Result<(), OrchestratorError> {
        if self.phase != BootSequence::Running {
            return Err(OrchestratorError::NotRunning);
        }

        match event {
            DeviceEvent::Connected {
                device_id,
                device_type,
            } => {
                self.connected_devices.insert(device_id, device_type);
                self.metrics.device_connects += 1;
            }
            DeviceEvent::Disconnected { device_id } => {
                self.connected_devices.remove(&device_id);
                self.metrics.device_disconnects += 1;
            }
        }
        Ok(())
    }

    /// Currently connected devices (id → type).
    pub fn connected_devices(&self) -> &HashMap<String, String> {
        &self.connected_devices
    }

    // -- adapter events ------------------------------------------------------

    /// Handle a simulator adapter event.
    pub fn handle_adapter_event(&mut self, event: AdapterEvent) -> Result<(), OrchestratorError> {
        if self.phase != BootSequence::Running {
            return Err(OrchestratorError::NotRunning);
        }

        match event {
            AdapterEvent::SimConnected { ref sim_name } => {
                if !self.connected_sims.contains(sim_name) {
                    self.connected_sims.push(sim_name.clone());
                    self.metrics.adapter_connects += 1;
                    // Update per-adapter lifecycle if we recognise the sim name.
                    let key = sim_name.to_lowercase();
                    if let Some(state) = self.adapter_states.get_mut(&key) {
                        *state = AdapterLifecycleState::Running;
                    }
                }
            }
            AdapterEvent::SimDisconnected { ref sim_name } => {
                self.connected_sims.retain(|s| s != sim_name);
                self.metrics.adapter_disconnects += 1;
                let key = sim_name.to_lowercase();
                if let Some(state) = self.adapter_states.get_mut(&key)
                    && *state != AdapterLifecycleState::Disabled
                {
                    *state = AdapterLifecycleState::Stopped;
                }
            }
            AdapterEvent::DataReceived { .. } => {
                // Handled by the adapter subsystem itself.
            }
        }
        Ok(())
    }

    /// Currently connected simulators.
    pub fn connected_sims(&self) -> &[String] {
        &self.connected_sims
    }

    // -- subsystem management ------------------------------------------------

    /// Restart a single subsystem by name.
    pub fn restart_subsystem(&mut self, name: &str) -> Result<(), OrchestratorError> {
        let handle = self
            .subsystems
            .get_mut(name)
            .ok_or_else(|| OrchestratorError::SubsystemNotFound(name.to_string()))?;

        if handle.is_running() {
            handle.stop().ok();
        }
        handle.start()?;
        self.metrics.subsystem_restarts += 1;
        Ok(())
    }

    /// Record an error against a specific subsystem.
    pub fn record_subsystem_error(
        &mut self,
        name: &str,
        error: &str,
    ) -> Result<(), OrchestratorError> {
        let handle = self
            .subsystems
            .get_mut(name)
            .ok_or_else(|| OrchestratorError::SubsystemNotFound(name.to_string()))?;
        handle.record_error(error);
        Ok(())
    }

    /// Mark a subsystem as failed.
    pub fn fail_subsystem(&mut self, name: &str, reason: &str) -> Result<(), OrchestratorError> {
        let handle = self
            .subsystems
            .get_mut(name)
            .ok_or_else(|| OrchestratorError::SubsystemNotFound(name.to_string()))?;
        handle.mark_failed(reason);
        Ok(())
    }

    /// Look up a subsystem handle by name.
    pub fn subsystem(&self, name: &str) -> Option<&SubsystemHandle> {
        self.subsystems.get(name)
    }

    /// The boot order (start sequence).
    pub fn boot_order(&self) -> &[String] {
        &self.boot_order
    }

    /// The orchestrator configuration.
    pub fn config(&self) -> &ServiceConfig {
        &self.config
    }

    // -- metrics -------------------------------------------------------------

    /// Collect an aggregated metrics snapshot.
    pub fn metrics(&self) -> OrchestratorMetrics {
        OrchestratorMetrics {
            profile_swaps: self.metrics.profile_swaps,
            device_connects: self.metrics.device_connects,
            device_disconnects: self.metrics.device_disconnects,
            adapter_connects: self.metrics.adapter_connects,
            adapter_disconnects: self.metrics.adapter_disconnects,
            config_reloads: self.metrics.config_reloads,
            subsystem_restarts: self.metrics.subsystem_restarts,
            adapter_states: self.adapter_states.clone(),
            uptime: self.started_at.map(|t| t.elapsed()),
        }
    }

    // -- per-adapter lifecycle -----------------------------------------------

    /// Start a named adapter. Only transitions `Stopped → Running`.
    pub fn start_adapter(&mut self, adapter: &str) -> Result<(), OrchestratorError> {
        let state = self
            .adapter_states
            .get_mut(adapter)
            .ok_or_else(|| OrchestratorError::SubsystemNotFound(adapter.to_string()))?;
        match state {
            AdapterLifecycleState::Disabled => Err(OrchestratorError::SubsystemStartFailed {
                name: adapter.to_string(),
                reason: "adapter is disabled in config".to_string(),
            }),
            AdapterLifecycleState::Running => Err(OrchestratorError::SubsystemAlreadyRunning(
                adapter.to_string(),
            )),
            AdapterLifecycleState::Stopped | AdapterLifecycleState::Error(_) => {
                *state = AdapterLifecycleState::Running;
                self.metrics.adapter_connects += 1;
                Ok(())
            }
        }
    }

    /// Stop a named adapter. Only transitions `Running → Stopped`.
    pub fn stop_adapter(&mut self, adapter: &str) -> Result<(), OrchestratorError> {
        let state = self
            .adapter_states
            .get_mut(adapter)
            .ok_or_else(|| OrchestratorError::SubsystemNotFound(adapter.to_string()))?;
        match state {
            AdapterLifecycleState::Running | AdapterLifecycleState::Error(_) => {
                *state = AdapterLifecycleState::Stopped;
                self.metrics.adapter_disconnects += 1;
                Ok(())
            }
            _ => Err(OrchestratorError::SubsystemNotRunning(
                adapter.to_string(),
            )),
        }
    }

    /// Record an error against a named adapter.
    pub fn record_adapter_error(&mut self, adapter: &str, error: &str) {
        if let Some(state) = self.adapter_states.get_mut(adapter) {
            *state = AdapterLifecycleState::Error(error.to_string());
        }
    }

    /// Current per-adapter lifecycle states.
    pub fn adapter_states(&self) -> &HashMap<String, AdapterLifecycleState> {
        &self.adapter_states
    }

    // -- config hot-reload ---------------------------------------------------

    /// Poll the config watcher and return paths that changed since last check.
    pub fn poll_config_changes(&mut self) -> Vec<PathBuf> {
        let watcher = match self.config_watcher.as_mut() {
            Some(w) => w,
            None => return Vec::new(),
        };
        let changes = watcher.check_for_changes();
        let mut paths = Vec::new();
        for change in changes {
            if change.change_type != ChangeType::Deleted {
                paths.push(change.path);
            }
        }
        if !paths.is_empty() {
            self.metrics.config_reloads += 1;
        }
        paths
    }

    /// Apply a new [`ServiceConfig`] at runtime (e.g. after a config reload).
    ///
    /// Updates adapter enable flags and propagates changes to per-adapter
    /// lifecycle states without restarting the orchestrator.
    pub fn apply_config_update(&mut self, new_config: ServiceConfig) {
        self.config = new_config;
        let updated = Self::init_adapter_states(&self.config);
        // Build a list of mutations to avoid conflicting borrows.
        let mutations: Vec<(String, AdapterLifecycleState)> = updated
            .iter()
            .filter_map(|(name, desired)| {
                match self.adapter_states.get(name) {
                    Some(current) => {
                        if *desired == AdapterLifecycleState::Disabled
                            && *current != AdapterLifecycleState::Disabled
                        {
                            return Some((name.clone(), AdapterLifecycleState::Disabled));
                        }
                        if *desired != AdapterLifecycleState::Disabled
                            && *current == AdapterLifecycleState::Disabled
                        {
                            return Some((name.clone(), AdapterLifecycleState::Stopped));
                        }
                        None
                    }
                    None => Some((name.clone(), desired.clone())),
                }
            })
            .collect();
        for (name, state) in mutations {
            self.adapter_states.insert(name, state);
        }
    }

    // -- internal helpers ----------------------------------------------------

    fn transition(&mut self, next: BootSequence) -> Result<(), OrchestratorError> {
        if !self.phase.can_transition_to(next) {
            return Err(OrchestratorError::InvalidTransition {
                from: self.phase,
                to: next,
            });
        }
        self.phase = next;
        Ok(())
    }

    fn start_subsystem(&mut self, name: &str) -> Result<(), OrchestratorError> {
        let handle = self
            .subsystems
            .get_mut(name)
            .ok_or_else(|| OrchestratorError::SubsystemNotFound(name.to_string()))?;
        handle
            .start()
            .map_err(|e| OrchestratorError::SubsystemStartFailed {
                name: name.to_string(),
                reason: e.to_string(),
            })
    }
}

/// Map health to a numeric severity for comparison.
fn health_severity(h: SubsystemHealth) -> u8 {
    match h {
        SubsystemHealth::Healthy => 0,
        SubsystemHealth::Unknown => 1,
        SubsystemHealth::Degraded => 2,
        SubsystemHealth::Failed => 3,
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- helpers -------------------------------------------------------------

    fn default_orchestrator() -> ServiceOrchestrator {
        ServiceOrchestrator::new(ServiceConfig::default())
    }

    fn minimal_orchestrator() -> ServiceOrchestrator {
        ServiceOrchestrator::new(ServiceConfig {
            enable_watchdog: false,
            enable_adapters: false,
            ..ServiceConfig::default()
        })
    }

    fn started_orchestrator() -> ServiceOrchestrator {
        let mut orch = default_orchestrator();
        orch.start().expect("start should succeed");
        orch
    }

    // -- boot sequence ordering ----------------------------------------------

    #[test]
    fn boot_sequence_transitions_in_order() {
        let seq = BootSequence::Initializing;
        assert!(seq.can_transition_to(BootSequence::BusReady));

        let seq = BootSequence::BusReady;
        assert!(seq.can_transition_to(BootSequence::SchedulerReady));

        let seq = BootSequence::SchedulerReady;
        assert!(seq.can_transition_to(BootSequence::AdaptersReady));

        let seq = BootSequence::AdaptersReady;
        assert!(seq.can_transition_to(BootSequence::Running));

        let seq = BootSequence::Running;
        assert!(seq.can_transition_to(BootSequence::ShuttingDown));

        let seq = BootSequence::ShuttingDown;
        assert!(seq.can_transition_to(BootSequence::Stopped));
    }

    #[test]
    fn boot_sequence_rejects_invalid_transitions() {
        assert!(!BootSequence::Initializing.can_transition_to(BootSequence::Running));
        assert!(!BootSequence::Running.can_transition_to(BootSequence::BusReady));
        assert!(!BootSequence::Stopped.can_transition_to(BootSequence::Running));
    }

    #[test]
    fn boot_sequence_display() {
        assert_eq!(format!("{}", BootSequence::Initializing), "Initializing");
        assert_eq!(format!("{}", BootSequence::Running), "Running");
        assert_eq!(format!("{}", BootSequence::Stopped), "Stopped");
    }

    #[test]
    fn start_walks_through_phases() {
        let mut orch = default_orchestrator();
        assert_eq!(orch.phase(), BootSequence::Initializing);

        orch.start().unwrap();
        assert_eq!(orch.phase(), BootSequence::Running);
        assert!(orch.is_running());
    }

    #[test]
    fn start_minimal_config() {
        let mut orch = minimal_orchestrator();
        orch.start().unwrap();
        assert!(orch.is_running());
        // Only bus + scheduler should be running
        assert!(orch.subsystem(SUBSYSTEM_BUS).unwrap().is_running());
        assert!(orch.subsystem(SUBSYSTEM_SCHEDULER).unwrap().is_running());
        assert!(orch.subsystem(SUBSYSTEM_ADAPTERS).is_none());
        assert!(orch.subsystem(SUBSYSTEM_WATCHDOG).is_none());
    }

    #[test]
    fn start_when_already_running_fails() {
        let mut orch = started_orchestrator();
        let err = orch.start().unwrap_err();
        assert_eq!(err, OrchestratorError::AlreadyRunning);
    }

    #[test]
    fn all_subsystems_running_after_start() {
        let orch = started_orchestrator();
        for name in orch.boot_order() {
            let handle = orch.subsystem(name).unwrap();
            assert!(handle.is_running(), "{name} should be running");
            assert_eq!(handle.health(), SubsystemHealth::Healthy);
        }
    }

    #[test]
    fn boot_order_is_bus_scheduler_adapters_watchdog() {
        let orch = default_orchestrator();
        let order: Vec<&str> = orch.boot_order().iter().map(String::as_str).collect();
        assert_eq!(
            order,
            vec![
                SUBSYSTEM_BUS,
                SUBSYSTEM_SCHEDULER,
                SUBSYSTEM_ADAPTERS,
                SUBSYSTEM_WATCHDOG,
            ]
        );
    }

    // -- graceful shutdown ---------------------------------------------------

    #[test]
    fn stop_transitions_to_stopped() {
        let mut orch = started_orchestrator();
        orch.stop().unwrap();
        assert_eq!(orch.phase(), BootSequence::Stopped);
        assert!(!orch.is_running());
    }

    #[test]
    fn stop_shuts_down_subsystems_in_reverse() {
        let mut orch = started_orchestrator();
        orch.stop().unwrap();

        for name in orch.boot_order() {
            let handle = orch.subsystem(name).unwrap();
            assert!(!handle.is_running(), "{name} should be stopped");
        }
    }

    #[test]
    fn stop_when_not_running_fails() {
        let mut orch = default_orchestrator();
        let err = orch.stop().unwrap_err();
        assert_eq!(err, OrchestratorError::NotRunning);
    }

    #[test]
    fn stop_twice_fails() {
        let mut orch = started_orchestrator();
        orch.stop().unwrap();
        let err = orch.stop().unwrap_err();
        assert_eq!(err, OrchestratorError::NotRunning);
    }

    #[test]
    fn can_restart_after_stop() {
        let mut orch = default_orchestrator();
        orch.start().unwrap();
        orch.stop().unwrap();
        // Stopped -> Initializing -> ... -> Running
        orch.phase = BootSequence::Initializing;
        orch.start().unwrap();
        assert!(orch.is_running());
    }

    // -- subsystem failure handling ------------------------------------------

    #[test]
    fn subsystem_failure_does_not_stop_others() {
        let mut orch = started_orchestrator();

        orch.fail_subsystem(SUBSYSTEM_ADAPTERS, "connection lost")
            .unwrap();

        // Adapters failed but others continue
        assert!(!orch.subsystem(SUBSYSTEM_ADAPTERS).unwrap().is_running());
        assert_eq!(
            orch.subsystem(SUBSYSTEM_ADAPTERS).unwrap().health(),
            SubsystemHealth::Failed
        );
        assert!(orch.subsystem(SUBSYSTEM_BUS).unwrap().is_running());
        assert!(orch.subsystem(SUBSYSTEM_SCHEDULER).unwrap().is_running());
        assert!(orch.subsystem(SUBSYSTEM_WATCHDOG).unwrap().is_running());
    }

    #[test]
    fn subsystem_error_degrades_health() {
        let mut orch = started_orchestrator();
        orch.record_subsystem_error(SUBSYSTEM_BUS, "queue overflow")
            .unwrap();

        let handle = orch.subsystem(SUBSYSTEM_BUS).unwrap();
        assert_eq!(handle.health(), SubsystemHealth::Degraded);
        assert_eq!(handle.error_count(), 1);
        assert_eq!(handle.last_error(), Some("queue overflow"));
    }

    #[test]
    fn error_on_unknown_subsystem() {
        let mut orch = started_orchestrator();
        let err = orch
            .record_subsystem_error("nonexistent", "err")
            .unwrap_err();
        assert_eq!(
            err,
            OrchestratorError::SubsystemNotFound("nonexistent".to_string())
        );
    }

    #[test]
    fn subsystem_handle_recovery() {
        let mut handle = SubsystemHandle::new("test");
        handle.start().unwrap();
        handle.record_error("minor glitch");
        assert_eq!(handle.health(), SubsystemHealth::Degraded);

        handle.recover();
        assert_eq!(handle.health(), SubsystemHealth::Healthy);
        assert!(handle.last_error().is_none());
    }

    #[test]
    fn subsystem_handle_double_start_fails() {
        let mut handle = SubsystemHandle::new("test");
        handle.start().unwrap();
        let err = handle.start().unwrap_err();
        assert_eq!(
            err,
            OrchestratorError::SubsystemAlreadyRunning("test".to_string())
        );
    }

    #[test]
    fn subsystem_handle_stop_when_not_running_fails() {
        let mut handle = SubsystemHandle::new("test");
        let err = handle.stop().unwrap_err();
        assert_eq!(
            err,
            OrchestratorError::SubsystemNotRunning("test".to_string())
        );
    }

    #[test]
    fn subsystem_handle_mark_failed_stops_running() {
        let mut handle = SubsystemHandle::new("test");
        handle.start().unwrap();
        handle.mark_failed("fatal");
        assert!(!handle.is_running());
        assert_eq!(handle.health(), SubsystemHealth::Failed);
        assert_eq!(handle.error_count(), 1);
    }

    #[test]
    fn subsystem_handle_uptime_is_some_when_running() {
        let mut handle = SubsystemHandle::new("test");
        assert!(handle.uptime().is_none());
        handle.start().unwrap();
        assert!(handle.uptime().is_some());
    }

    // -- profile hot-swap during operation -----------------------------------

    #[test]
    fn profile_change_while_running() {
        let mut orch = started_orchestrator();
        assert!(orch.active_profile().is_none());

        let compiled = orch.handle_profile_change("fighter-jet").unwrap();
        assert_eq!(compiled.name, "fighter-jet");
        assert_eq!(compiled.version, 1);

        let active = orch.active_profile().unwrap();
        assert_eq!(active.name, "fighter-jet");
    }

    #[test]
    fn profile_change_increments_version() {
        let mut orch = started_orchestrator();
        orch.handle_profile_change("p1").unwrap();
        let p2 = orch.handle_profile_change("p2").unwrap();
        assert_eq!(p2.version, 2);
        assert_eq!(orch.active_profile().unwrap().name, "p2");
    }

    #[test]
    fn profile_change_when_not_running_fails() {
        let mut orch = default_orchestrator();
        let err = orch.handle_profile_change("anything").unwrap_err();
        assert_eq!(err, OrchestratorError::NotRunning);
    }

    // -- device connect / disconnect during operation ------------------------

    #[test]
    fn device_connect_and_disconnect() {
        let mut orch = started_orchestrator();
        assert!(orch.connected_devices().is_empty());

        orch.handle_device_change(DeviceEvent::Connected {
            device_id: "js-1".into(),
            device_type: "joystick".into(),
        })
        .unwrap();
        assert_eq!(orch.connected_devices().len(), 1);
        assert_eq!(orch.connected_devices()["js-1"], "joystick");

        orch.handle_device_change(DeviceEvent::Disconnected {
            device_id: "js-1".into(),
        })
        .unwrap();
        assert!(orch.connected_devices().is_empty());
    }

    #[test]
    fn device_change_when_not_running_fails() {
        let mut orch = default_orchestrator();
        let err = orch
            .handle_device_change(DeviceEvent::Connected {
                device_id: "x".into(),
                device_type: "y".into(),
            })
            .unwrap_err();
        assert_eq!(err, OrchestratorError::NotRunning);
    }

    #[test]
    fn multiple_devices_tracked() {
        let mut orch = started_orchestrator();
        orch.handle_device_change(DeviceEvent::Connected {
            device_id: "js-1".into(),
            device_type: "joystick".into(),
        })
        .unwrap();
        orch.handle_device_change(DeviceEvent::Connected {
            device_id: "th-1".into(),
            device_type: "throttle".into(),
        })
        .unwrap();
        assert_eq!(orch.connected_devices().len(), 2);

        // Disconnect one
        orch.handle_device_change(DeviceEvent::Disconnected {
            device_id: "js-1".into(),
        })
        .unwrap();
        assert_eq!(orch.connected_devices().len(), 1);
        assert!(orch.connected_devices().contains_key("th-1"));
    }

    // -- adapter events ------------------------------------------------------

    #[test]
    fn adapter_sim_connect_and_disconnect() {
        let mut orch = started_orchestrator();
        assert!(orch.connected_sims().is_empty());

        orch.handle_adapter_event(AdapterEvent::SimConnected {
            sim_name: "MSFS".into(),
        })
        .unwrap();
        assert_eq!(orch.connected_sims(), &["MSFS"]);

        orch.handle_adapter_event(AdapterEvent::SimDisconnected {
            sim_name: "MSFS".into(),
        })
        .unwrap();
        assert!(orch.connected_sims().is_empty());
    }

    #[test]
    fn adapter_duplicate_connect_ignored() {
        let mut orch = started_orchestrator();
        orch.handle_adapter_event(AdapterEvent::SimConnected {
            sim_name: "DCS".into(),
        })
        .unwrap();
        orch.handle_adapter_event(AdapterEvent::SimConnected {
            sim_name: "DCS".into(),
        })
        .unwrap();
        assert_eq!(orch.connected_sims().len(), 1);
    }

    #[test]
    fn adapter_data_received_is_noop() {
        let mut orch = started_orchestrator();
        orch.handle_adapter_event(AdapterEvent::DataReceived {
            sim_name: "X-Plane".into(),
        })
        .unwrap();
        assert!(orch.connected_sims().is_empty());
    }

    #[test]
    fn adapter_event_when_not_running_fails() {
        let mut orch = default_orchestrator();
        let err = orch
            .handle_adapter_event(AdapterEvent::SimConnected {
                sim_name: "x".into(),
            })
            .unwrap_err();
        assert_eq!(err, OrchestratorError::NotRunning);
    }

    // -- status aggregation --------------------------------------------------

    #[test]
    fn status_aggregation_all_healthy() {
        let orch = started_orchestrator();
        let status = orch.status();

        assert_eq!(status.boot_phase, BootSequence::Running);
        assert_eq!(status.overall_health, SubsystemHealth::Healthy);
        assert_eq!(status.subsystems.len(), 4);

        for sub in status.subsystems.values() {
            assert!(sub.running);
            assert_eq!(sub.health, SubsystemHealth::Healthy);
            assert_eq!(sub.error_count, 0);
            assert!(sub.last_error.is_none());
        }
    }

    #[test]
    fn status_aggregation_with_degraded_subsystem() {
        let mut orch = started_orchestrator();
        orch.record_subsystem_error(SUBSYSTEM_BUS, "slow").unwrap();

        let status = orch.status();
        assert_eq!(status.overall_health, SubsystemHealth::Degraded);
        assert_eq!(
            status.subsystems[SUBSYSTEM_BUS].health,
            SubsystemHealth::Degraded
        );
    }

    #[test]
    fn status_aggregation_with_failed_subsystem() {
        let mut orch = started_orchestrator();
        orch.fail_subsystem(SUBSYSTEM_ADAPTERS, "crash").unwrap();

        let status = orch.status();
        assert_eq!(status.overall_health, SubsystemHealth::Failed);
    }

    #[test]
    fn status_before_start() {
        let orch = default_orchestrator();
        let status = orch.status();
        assert_eq!(status.boot_phase, BootSequence::Initializing);
        // All subsystems should be not-running
        for sub in status.subsystems.values() {
            assert!(!sub.running);
        }
    }

    // -- restart individual subsystem ----------------------------------------

    #[test]
    fn restart_subsystem_success() {
        let mut orch = started_orchestrator();
        orch.record_subsystem_error(SUBSYSTEM_BUS, "transient")
            .unwrap();
        assert_eq!(
            orch.subsystem(SUBSYSTEM_BUS).unwrap().health(),
            SubsystemHealth::Degraded
        );

        orch.restart_subsystem(SUBSYSTEM_BUS).unwrap();

        let handle = orch.subsystem(SUBSYSTEM_BUS).unwrap();
        assert!(handle.is_running());
        assert_eq!(handle.health(), SubsystemHealth::Healthy);
    }

    #[test]
    fn restart_failed_subsystem() {
        let mut orch = started_orchestrator();
        orch.fail_subsystem(SUBSYSTEM_ADAPTERS, "crash").unwrap();
        assert!(!orch.subsystem(SUBSYSTEM_ADAPTERS).unwrap().is_running());

        orch.restart_subsystem(SUBSYSTEM_ADAPTERS).unwrap();
        assert!(orch.subsystem(SUBSYSTEM_ADAPTERS).unwrap().is_running());
        assert_eq!(
            orch.subsystem(SUBSYSTEM_ADAPTERS).unwrap().health(),
            SubsystemHealth::Healthy
        );
    }

    #[test]
    fn restart_unknown_subsystem_fails() {
        let mut orch = started_orchestrator();
        let err = orch.restart_subsystem("does-not-exist").unwrap_err();
        assert_eq!(
            err,
            OrchestratorError::SubsystemNotFound("does-not-exist".to_string())
        );
    }

    // -- error display -------------------------------------------------------

    #[test]
    fn error_display_messages() {
        let e = OrchestratorError::AlreadyRunning;
        assert_eq!(format!("{e}"), "orchestrator is already running");

        let e = OrchestratorError::NotRunning;
        assert_eq!(format!("{e}"), "orchestrator is not running");

        let e = OrchestratorError::SubsystemNotFound("xyz".into());
        assert_eq!(format!("{e}"), "subsystem 'xyz' not found");

        let e = OrchestratorError::InvalidTransition {
            from: BootSequence::Running,
            to: BootSequence::BusReady,
        };
        assert_eq!(format!("{e}"), "invalid transition Running -> BusReady");
    }

    // -- config defaults -----------------------------------------------------

    #[test]
    fn default_config_enables_watchdog_and_adapters() {
        let cfg = ServiceConfig::default();
        assert!(cfg.enable_watchdog);
        assert!(cfg.enable_adapters);
        assert_eq!(cfg.shutdown_timeout_ms, 5_000);
    }

    // -- compiled profile equality -------------------------------------------

    #[test]
    fn compiled_profile_equality() {
        let a = CompiledProfile {
            name: "a".into(),
            version: 1,
            axis_count: 0,
            sim_filter: None,
            aircraft_filter: None,
        };
        let b = CompiledProfile {
            name: "a".into(),
            version: 1,
            axis_count: 0,
            sim_filter: None,
            aircraft_filter: None,
        };
        assert_eq!(a, b);
    }

    // -- device event equality -----------------------------------------------

    #[test]
    fn device_event_equality() {
        let a = DeviceEvent::Connected {
            device_id: "1".into(),
            device_type: "j".into(),
        };
        let b = DeviceEvent::Connected {
            device_id: "1".into(),
            device_type: "j".into(),
        };
        assert_eq!(a, b);
    }

    // -- full lifecycle integration ------------------------------------------

    #[test]
    fn full_lifecycle_integration() {
        let mut orch = default_orchestrator();

        // Start
        orch.start().unwrap();
        assert!(orch.is_running());

        // Profile swap
        let p = orch.handle_profile_change("cessna-172").unwrap();
        assert_eq!(p.version, 1);

        // Device events
        orch.handle_device_change(DeviceEvent::Connected {
            device_id: "hotas-1".into(),
            device_type: "hotas".into(),
        })
        .unwrap();

        // Adapter events
        orch.handle_adapter_event(AdapterEvent::SimConnected {
            sim_name: "MSFS 2024".into(),
        })
        .unwrap();

        // Subsystem degrades
        orch.record_subsystem_error(SUBSYSTEM_ADAPTERS, "timeout")
            .unwrap();
        let status = orch.status();
        assert_eq!(status.overall_health, SubsystemHealth::Degraded);

        // Restart degraded subsystem
        orch.restart_subsystem(SUBSYSTEM_ADAPTERS).unwrap();
        let status = orch.status();
        assert_eq!(status.overall_health, SubsystemHealth::Healthy);

        // New profile while devices and sims are connected
        let p2 = orch.handle_profile_change("f-18c").unwrap();
        assert_eq!(p2.version, 2);

        // Shutdown
        orch.stop().unwrap();
        assert_eq!(orch.phase(), BootSequence::Stopped);

        // Nothing should be running
        for name in orch.boot_order() {
            assert!(!orch.subsystem(name).unwrap().is_running());
        }
    }

    // -- metrics population --------------------------------------------------

    #[test]
    fn metrics_start_at_zero() {
        let orch = default_orchestrator();
        let m = orch.metrics();
        assert_eq!(m.profile_swaps, 0);
        assert_eq!(m.device_connects, 0);
        assert_eq!(m.device_disconnects, 0);
        assert_eq!(m.adapter_connects, 0);
        assert_eq!(m.adapter_disconnects, 0);
        assert_eq!(m.config_reloads, 0);
        assert_eq!(m.subsystem_restarts, 0);
        assert!(m.uptime.is_none());
    }

    #[test]
    fn metrics_populated_after_events() {
        let mut orch = started_orchestrator();

        orch.handle_profile_change("p1").unwrap();
        orch.handle_profile_change("p2").unwrap();

        orch.handle_device_change(DeviceEvent::Connected {
            device_id: "js-1".into(),
            device_type: "joystick".into(),
        })
        .unwrap();
        orch.handle_device_change(DeviceEvent::Disconnected {
            device_id: "js-1".into(),
        })
        .unwrap();

        orch.handle_adapter_event(AdapterEvent::SimConnected {
            sim_name: "msfs".into(),
        })
        .unwrap();
        orch.handle_adapter_event(AdapterEvent::SimDisconnected {
            sim_name: "msfs".into(),
        })
        .unwrap();

        orch.restart_subsystem(SUBSYSTEM_BUS).unwrap();

        let m = orch.metrics();
        assert_eq!(m.profile_swaps, 2);
        assert_eq!(m.device_connects, 1);
        assert_eq!(m.device_disconnects, 1);
        assert_eq!(m.adapter_connects, 1);
        assert_eq!(m.adapter_disconnects, 1);
        assert_eq!(m.subsystem_restarts, 1);
        assert!(m.uptime.is_some());
    }

    // -- per-adapter lifecycle -----------------------------------------------

    #[test]
    fn adapter_states_initialised_from_flags() {
        let cfg = ServiceConfig {
            adapter_flags: AdapterFlags {
                msfs: true,
                xplane: false,
                dcs: true,
                ac7: false,
                wingman: true,
            },
            ..ServiceConfig::default()
        };
        let orch = ServiceOrchestrator::new(cfg);
        let states = orch.adapter_states();
        assert_eq!(states["msfs"], AdapterLifecycleState::Stopped);
        assert_eq!(states["xplane"], AdapterLifecycleState::Disabled);
        assert_eq!(states["dcs"], AdapterLifecycleState::Stopped);
        assert_eq!(states["ac7"], AdapterLifecycleState::Disabled);
        assert_eq!(states["wingman"], AdapterLifecycleState::Stopped);
    }

    #[test]
    fn adapter_states_running_after_start() {
        let mut orch = default_orchestrator();
        orch.start().unwrap();
        for (_, state) in orch.adapter_states() {
            // All enabled by default → should be Running.
            assert_eq!(*state, AdapterLifecycleState::Running);
        }
    }

    #[test]
    fn adapter_start_stop_lifecycle() {
        let mut orch = started_orchestrator();
        // Stop the msfs adapter
        orch.stop_adapter("msfs").unwrap();
        assert_eq!(
            orch.adapter_states()["msfs"],
            AdapterLifecycleState::Stopped
        );

        // Restart it
        orch.start_adapter("msfs").unwrap();
        assert_eq!(
            orch.adapter_states()["msfs"],
            AdapterLifecycleState::Running
        );
    }

    #[test]
    fn adapter_error_and_restart() {
        let mut orch = started_orchestrator();
        orch.record_adapter_error("dcs", "connection timed out");
        assert!(matches!(
            orch.adapter_states()["dcs"],
            AdapterLifecycleState::Error(_)
        ));

        // Can restart from Error state
        orch.start_adapter("dcs").unwrap();
        assert_eq!(orch.adapter_states()["dcs"], AdapterLifecycleState::Running);
    }

    #[test]
    fn start_disabled_adapter_fails() {
        let cfg = ServiceConfig {
            adapter_flags: AdapterFlags {
                msfs: false,
                ..AdapterFlags::default()
            },
            ..ServiceConfig::default()
        };
        let mut orch = ServiceOrchestrator::new(cfg);
        orch.start().unwrap();

        let err = orch.start_adapter("msfs").unwrap_err();
        assert!(matches!(
            err,
            OrchestratorError::SubsystemStartFailed { .. }
        ));
    }

    // -- config watcher integration ------------------------------------------

    #[test]
    fn config_watcher_created_when_path_provided() {
        let dir = tempfile::tempdir().unwrap();
        let cfg_path = dir.path().join("service.json");
        std::fs::write(&cfg_path, "{}").unwrap();

        let cfg = ServiceConfig {
            config_path: Some(cfg_path.clone()),
            ..ServiceConfig::default()
        };
        let mut orch = ServiceOrchestrator::new(cfg);

        // First poll primes the state — may or may not detect a change
        // depending on mtime resolution, so just capture the baseline.
        let _ = orch.poll_config_changes();
        let baseline = orch.metrics().config_reloads;

        // Modify the file
        std::fs::write(&cfg_path, "{\"updated\":true}").unwrap();
        let changed = orch.poll_config_changes();
        assert!(!changed.is_empty(), "should detect config file change");
        assert_eq!(orch.metrics().config_reloads, baseline + 1);
    }

    #[test]
    fn poll_config_changes_empty_without_path() {
        let mut orch = default_orchestrator();
        let changed = orch.poll_config_changes();
        assert!(changed.is_empty());
    }

    // -- apply_config_update -------------------------------------------------

    #[test]
    fn apply_config_update_toggles_adapters() {
        let mut orch = started_orchestrator();
        // All enabled and Running after start
        assert_eq!(
            orch.adapter_states()["xplane"],
            AdapterLifecycleState::Running
        );

        // Disable xplane via config update
        let mut new_cfg = orch.config().clone();
        new_cfg.adapter_flags.xplane = false;
        orch.apply_config_update(new_cfg);
        assert_eq!(
            orch.adapter_states()["xplane"],
            AdapterLifecycleState::Disabled
        );

        // Re-enable → goes to Stopped (not Running; needs explicit start)
        let mut new_cfg2 = orch.config().clone();
        new_cfg2.adapter_flags.xplane = true;
        orch.apply_config_update(new_cfg2);
        assert_eq!(
            orch.adapter_states()["xplane"],
            AdapterLifecycleState::Stopped
        );
    }

    // -- from_service_config -------------------------------------------------

    #[test]
    fn from_service_config_maps_adapter_flags() {
        use crate::service::FlightServiceConfig;

        let mut svc_cfg = FlightServiceConfig::default();
        svc_cfg.auto_switch_config.adapters.enable_msfs = false;
        svc_cfg.auto_switch_config.adapters.enable_dcs = false;

        let orch = ServiceOrchestrator::from_service_config(&svc_cfg, None);
        assert!(!orch.config().adapter_flags.msfs);
        assert!(orch.config().adapter_flags.xplane);
        assert!(!orch.config().adapter_flags.dcs);
    }

    // -- handle_profile_update with real Profile -----------------------------

    #[test]
    fn handle_profile_update_populates_fields() {
        use flight_core::profile::{AircraftId, AxisConfig, Profile};

        let mut orch = started_orchestrator();
        let mut axes = HashMap::new();
        axes.insert(
            "pitch".to_string(),
            AxisConfig {
                deadzone: Some(0.03),
                expo: None,
                slew_rate: None,
                detents: vec![],
                curve: None,
                filter: None,
            },
        );
        axes.insert(
            "roll".to_string(),
            AxisConfig {
                deadzone: Some(0.05),
                expo: None,
                slew_rate: None,
                detents: vec![],
                curve: None,
                filter: None,
            },
        );
        let profile = Profile {
            schema: "flight.profile/1".to_string(),
            sim: Some("msfs".to_string()),
            aircraft: Some(AircraftId { icao: "C172".to_string() }),
            axes,
            pof_overrides: None,
        };

        let compiled = orch.handle_profile_update(&profile).unwrap();
        assert_eq!(compiled.name, "C172");
        assert_eq!(compiled.axis_count, 2);
        assert_eq!(compiled.sim_filter, Some("msfs".to_string()));
        assert_eq!(compiled.aircraft_filter, Some("C172".to_string()));
        assert_eq!(compiled.version, 1);
        assert_eq!(orch.metrics().profile_swaps, 1);
    }

    // -- adapter states reset on stop ----------------------------------------

    #[test]
    fn adapter_states_reset_on_stop() {
        let mut orch = started_orchestrator();
        // All adapters should be Running
        for (_, state) in orch.adapter_states() {
            assert_eq!(*state, AdapterLifecycleState::Running);
        }
        orch.stop().unwrap();
        // All should be Stopped
        for (_, state) in orch.adapter_states() {
            assert_eq!(*state, AdapterLifecycleState::Stopped);
        }
    }
}
