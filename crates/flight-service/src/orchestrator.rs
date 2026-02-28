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
use std::time::Instant;

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
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            shutdown_timeout_ms: 5_000,
            enable_watchdog: true,
            enable_adapters: true,
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

        Self {
            config,
            phase: BootSequence::Initializing,
            subsystems,
            boot_order,
            active_profile: None,
            profile_version: 0,
            connected_devices: HashMap::new(),
            connected_sims: Vec::new(),
        }
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
        };
        self.active_profile = Some(compiled.clone());
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
            }
            DeviceEvent::Disconnected { device_id } => {
                self.connected_devices.remove(&device_id);
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
            AdapterEvent::SimConnected { sim_name } => {
                if !self.connected_sims.contains(&sim_name) {
                    self.connected_sims.push(sim_name);
                }
            }
            AdapterEvent::SimDisconnected { sim_name } => {
                self.connected_sims.retain(|s| s != &sim_name);
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
        handle.start()
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

        for (_, sub) in &status.subsystems {
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
        for (_, sub) in &status.subsystems {
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
        };
        let b = CompiledProfile {
            name: "a".into(),
            version: 1,
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
}
