// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Service lifecycle state machine with ordered startup/shutdown phases.
//!
//! Defines the full lifecycle:
//! `Starting → Running → Degraded → ShuttingDown → Stopped`
//!
//! Startup phases execute in order:
//!   1. Config load
//!   2. Bus init
//!   3. Axis engine start
//!   4. Adapter discovery
//!   5. Profile compile
//!   6. Watchdog start
//!   7. IPC listener start
//!
//! Shutdown is the reverse of startup.

use std::fmt;

/// Service lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LifecycleState {
    /// Service is initializing — startup sequence in progress.
    Starting,
    /// All subsystems operational.
    Running,
    /// One or more non-critical subsystems have failed.
    Degraded,
    /// Graceful shutdown in progress.
    ShuttingDown,
    /// Service fully stopped.
    Stopped,
}

impl LifecycleState {
    /// Valid successor states.
    fn valid_next(self) -> &'static [LifecycleState] {
        match self {
            Self::Starting => &[Self::Running, Self::Degraded, Self::Stopped],
            Self::Running => &[Self::Degraded, Self::ShuttingDown],
            Self::Degraded => &[Self::Running, Self::ShuttingDown],
            Self::ShuttingDown => &[Self::Stopped],
            Self::Stopped => &[Self::Starting],
        }
    }

    /// Whether transitioning to `next` is valid.
    pub fn can_transition_to(self, next: Self) -> bool {
        self.valid_next().contains(&next)
    }
}

impl fmt::Display for LifecycleState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Starting => f.write_str("Starting"),
            Self::Running => f.write_str("Running"),
            Self::Degraded => f.write_str("Degraded"),
            Self::ShuttingDown => f.write_str("ShuttingDown"),
            Self::Stopped => f.write_str("Stopped"),
        }
    }
}

/// Individual startup phase in the boot sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StartupStep {
    ConfigLoad,
    BusInit,
    AxisEngineStart,
    AdapterDiscovery,
    ProfileCompile,
    WatchdogStart,
    IpcListenerStart,
}

impl StartupStep {
    /// All steps in startup order.
    pub const ORDERED: &'static [StartupStep] = &[
        Self::ConfigLoad,
        Self::BusInit,
        Self::AxisEngineStart,
        Self::AdapterDiscovery,
        Self::ProfileCompile,
        Self::WatchdogStart,
        Self::IpcListenerStart,
    ];

    /// Name for display / logging.
    pub fn name(self) -> &'static str {
        match self {
            Self::ConfigLoad => "config_load",
            Self::BusInit => "bus_init",
            Self::AxisEngineStart => "axis_engine_start",
            Self::AdapterDiscovery => "adapter_discovery",
            Self::ProfileCompile => "profile_compile",
            Self::WatchdogStart => "watchdog_start",
            Self::IpcListenerStart => "ipc_listener_start",
        }
    }
}

impl fmt::Display for StartupStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Outcome of a single startup step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepOutcome {
    /// Step completed successfully.
    Ok,
    /// Step completed with a non-fatal warning.
    Warning(String),
    /// Step failed — startup should abort.
    Failed(String),
}

/// Record of a completed startup step.
#[derive(Debug, Clone)]
pub struct StepRecord {
    pub step: StartupStep,
    pub outcome: StepOutcome,
}

/// Lifecycle error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleError {
    /// Invalid state transition.
    InvalidTransition {
        from: LifecycleState,
        to: LifecycleState,
    },
    /// Startup failed at a specific step.
    StartupFailed { step: StartupStep, reason: String },
    /// Service is not in expected state.
    UnexpectedState(LifecycleState),
}

impl fmt::Display for LifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTransition { from, to } => {
                write!(f, "invalid lifecycle transition: {from} → {to}")
            }
            Self::StartupFailed { step, reason } => {
                write!(f, "startup failed at {step}: {reason}")
            }
            Self::UnexpectedState(s) => write!(f, "unexpected state: {s}"),
        }
    }
}

impl std::error::Error for LifecycleError {}

/// Callback that executes a startup step. Return the outcome.
pub type StepHandler = Box<dyn Fn(StartupStep) -> StepOutcome + Send + Sync>;

/// Callback that executes a shutdown step. Return the outcome.
pub type ShutdownStepHandler = Box<dyn Fn(StartupStep) -> StepOutcome + Send + Sync>;

/// The service lifecycle manager.
///
/// Coordinates ordered startup, tracks the lifecycle state, and
/// executes reverse-order shutdown.
pub struct LifecycleManager {
    state: LifecycleState,
    completed_steps: Vec<StepRecord>,
    warnings: Vec<String>,
    startup_handler: Option<StepHandler>,
    shutdown_handler: Option<ShutdownStepHandler>,
}

impl LifecycleManager {
    /// Create a new lifecycle manager in the Stopped state.
    pub fn new() -> Self {
        Self {
            state: LifecycleState::Stopped,
            completed_steps: Vec::new(),
            warnings: Vec::new(),
            startup_handler: None,
            shutdown_handler: None,
        }
    }

    /// Set the handler called for each startup step.
    pub fn set_startup_handler(&mut self, handler: StepHandler) {
        self.startup_handler = Some(handler);
    }

    /// Set the handler called for each shutdown step.
    pub fn set_shutdown_handler(&mut self, handler: ShutdownStepHandler) {
        self.shutdown_handler = Some(handler);
    }

    /// Current lifecycle state.
    pub fn state(&self) -> LifecycleState {
        self.state
    }

    /// Completed startup steps.
    pub fn completed_steps(&self) -> &[StepRecord] {
        &self.completed_steps
    }

    /// Warnings accumulated during startup.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Execute the full startup sequence.
    ///
    /// Transitions: Stopped → Starting → Running (or Degraded/Stopped on failure).
    pub fn start(&mut self) -> Result<(), LifecycleError> {
        self.transition(LifecycleState::Starting)?;
        self.completed_steps.clear();
        self.warnings.clear();

        for &step in StartupStep::ORDERED {
            let outcome = if let Some(ref handler) = self.startup_handler {
                handler(step)
            } else {
                StepOutcome::Ok
            };

            match &outcome {
                StepOutcome::Ok => {}
                StepOutcome::Warning(msg) => {
                    self.warnings.push(format!("{}: {}", step.name(), msg));
                }
                StepOutcome::Failed(reason) => {
                    let reason = reason.clone();
                    self.completed_steps.push(StepRecord { step, outcome });
                    self.state = LifecycleState::Stopped;
                    return Err(LifecycleError::StartupFailed { step, reason });
                }
            }

            self.completed_steps.push(StepRecord { step, outcome });
        }

        if self.warnings.is_empty() {
            self.state = LifecycleState::Running;
        } else {
            self.state = LifecycleState::Degraded;
        }

        Ok(())
    }

    /// Execute graceful shutdown in reverse startup order.
    ///
    /// Transitions: Running/Degraded → ShuttingDown → Stopped.
    pub fn shutdown(&mut self) -> Result<Vec<StepRecord>, LifecycleError> {
        if self.state != LifecycleState::Running && self.state != LifecycleState::Degraded {
            return Err(LifecycleError::UnexpectedState(self.state));
        }
        self.state = LifecycleState::ShuttingDown;

        let mut shutdown_records = Vec::new();

        for step in StartupStep::ORDERED.iter().rev().copied() {
            let outcome = if let Some(ref handler) = self.shutdown_handler {
                handler(step)
            } else {
                StepOutcome::Ok
            };
            shutdown_records.push(StepRecord { step, outcome });
        }

        self.state = LifecycleState::Stopped;
        Ok(shutdown_records)
    }

    /// Manually transition to degraded mode.
    pub fn degrade(&mut self) -> Result<(), LifecycleError> {
        self.transition(LifecycleState::Degraded)
    }

    /// Recover from degraded back to running.
    pub fn recover(&mut self) -> Result<(), LifecycleError> {
        if self.state != LifecycleState::Degraded {
            return Err(LifecycleError::UnexpectedState(self.state));
        }
        self.state = LifecycleState::Running;
        Ok(())
    }

    fn transition(&mut self, next: LifecycleState) -> Result<(), LifecycleError> {
        if !self.state.can_transition_to(next) {
            return Err(LifecycleError::InvalidTransition {
                from: self.state,
                to: next,
            });
        }
        self.state = next;
        Ok(())
    }
}

impl Default for LifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state_is_stopped() {
        let lm = LifecycleManager::new();
        assert_eq!(lm.state(), LifecycleState::Stopped);
    }

    #[test]
    fn start_transitions_to_running() {
        let mut lm = LifecycleManager::new();
        lm.start().unwrap();
        assert_eq!(lm.state(), LifecycleState::Running);
    }

    #[test]
    fn start_completes_all_steps() {
        let mut lm = LifecycleManager::new();
        lm.start().unwrap();
        assert_eq!(lm.completed_steps().len(), StartupStep::ORDERED.len());
    }

    #[test]
    fn startup_order_is_correct() {
        let mut lm = LifecycleManager::new();
        lm.start().unwrap();
        let steps: Vec<StartupStep> = lm.completed_steps().iter().map(|r| r.step).collect();
        assert_eq!(steps, StartupStep::ORDERED);
    }

    #[test]
    fn shutdown_reverse_order() {
        let mut lm = LifecycleManager::new();
        lm.start().unwrap();
        let records = lm.shutdown().unwrap();

        let steps: Vec<StartupStep> = records.iter().map(|r| r.step).collect();
        let expected: Vec<StartupStep> = StartupStep::ORDERED.iter().rev().copied().collect();
        assert_eq!(steps, expected);
        assert_eq!(lm.state(), LifecycleState::Stopped);
    }

    #[test]
    fn startup_failure_stops() {
        let mut lm = LifecycleManager::new();
        lm.set_startup_handler(Box::new(|step| {
            if step == StartupStep::AxisEngineStart {
                StepOutcome::Failed("engine crash".into())
            } else {
                StepOutcome::Ok
            }
        }));

        let err = lm.start().unwrap_err();
        assert!(matches!(
            err,
            LifecycleError::StartupFailed {
                step: StartupStep::AxisEngineStart,
                ..
            }
        ));
        assert_eq!(lm.state(), LifecycleState::Stopped);
    }

    #[test]
    fn startup_failure_records_partial_steps() {
        let mut lm = LifecycleManager::new();
        lm.set_startup_handler(Box::new(|step| {
            if step == StartupStep::AdapterDiscovery {
                StepOutcome::Failed("no adapters".into())
            } else {
                StepOutcome::Ok
            }
        }));

        let _ = lm.start();
        // Should have ConfigLoad, BusInit, AxisEngineStart, AdapterDiscovery (failed)
        assert_eq!(lm.completed_steps().len(), 4);
    }

    #[test]
    fn startup_warnings_collected() {
        let mut lm = LifecycleManager::new();
        lm.set_startup_handler(Box::new(|step| {
            if step == StartupStep::WatchdogStart {
                StepOutcome::Warning("watchdog running in limited mode".into())
            } else {
                StepOutcome::Ok
            }
        }));

        lm.start().unwrap();
        assert_eq!(lm.state(), LifecycleState::Degraded);
        assert_eq!(lm.warnings().len(), 1);
        assert!(lm.warnings()[0].contains("watchdog"));
    }

    #[test]
    fn degrade_and_recover() {
        let mut lm = LifecycleManager::new();
        lm.start().unwrap();

        lm.degrade().unwrap();
        assert_eq!(lm.state(), LifecycleState::Degraded);

        lm.recover().unwrap();
        assert_eq!(lm.state(), LifecycleState::Running);
    }

    #[test]
    fn shutdown_from_degraded() {
        let mut lm = LifecycleManager::new();
        lm.start().unwrap();
        lm.degrade().unwrap();

        let records = lm.shutdown().unwrap();
        assert_eq!(records.len(), StartupStep::ORDERED.len());
        assert_eq!(lm.state(), LifecycleState::Stopped);
    }

    #[test]
    fn shutdown_from_stopped_fails() {
        let mut lm = LifecycleManager::new();
        let err = lm.shutdown().unwrap_err();
        assert!(matches!(err, LifecycleError::UnexpectedState(_)));
    }

    #[test]
    fn invalid_transition_errors() {
        let lm = LifecycleManager::new();
        assert!(!LifecycleState::Stopped.can_transition_to(LifecycleState::Running));
        assert!(!LifecycleState::Running.can_transition_to(LifecycleState::Starting));
        assert!(!LifecycleState::ShuttingDown.can_transition_to(LifecycleState::Running));
        // verify the manager is still in stopped state
        assert_eq!(lm.state(), LifecycleState::Stopped);
    }

    #[test]
    fn lifecycle_state_display() {
        assert_eq!(format!("{}", LifecycleState::Starting), "Starting");
        assert_eq!(format!("{}", LifecycleState::Running), "Running");
        assert_eq!(format!("{}", LifecycleState::Degraded), "Degraded");
        assert_eq!(format!("{}", LifecycleState::ShuttingDown), "ShuttingDown");
        assert_eq!(format!("{}", LifecycleState::Stopped), "Stopped");
    }

    #[test]
    fn restart_after_stop() {
        let mut lm = LifecycleManager::new();
        lm.start().unwrap();
        lm.shutdown().unwrap();
        assert_eq!(lm.state(), LifecycleState::Stopped);

        // Can restart
        lm.start().unwrap();
        assert_eq!(lm.state(), LifecycleState::Running);
    }

    #[test]
    fn shutdown_handler_called_in_reverse() {
        use std::sync::{Arc, Mutex};

        let order = Arc::new(Mutex::new(Vec::new()));
        let order_clone = order.clone();

        let mut lm = LifecycleManager::new();
        lm.set_shutdown_handler(Box::new(move |step| {
            order_clone.lock().unwrap().push(step);
            StepOutcome::Ok
        }));

        lm.start().unwrap();
        lm.shutdown().unwrap();

        let recorded = order.lock().unwrap();
        let expected: Vec<StartupStep> = StartupStep::ORDERED.iter().rev().copied().collect();
        assert_eq!(*recorded, expected);
    }

    #[test]
    fn valid_transitions() {
        assert!(LifecycleState::Stopped.can_transition_to(LifecycleState::Starting));
        assert!(LifecycleState::Starting.can_transition_to(LifecycleState::Running));
        assert!(LifecycleState::Running.can_transition_to(LifecycleState::Degraded));
        assert!(LifecycleState::Running.can_transition_to(LifecycleState::ShuttingDown));
        assert!(LifecycleState::Degraded.can_transition_to(LifecycleState::Running));
        assert!(LifecycleState::Degraded.can_transition_to(LifecycleState::ShuttingDown));
        assert!(LifecycleState::ShuttingDown.can_transition_to(LifecycleState::Stopped));
    }

    #[test]
    fn startup_step_names() {
        assert_eq!(StartupStep::ConfigLoad.name(), "config_load");
        assert_eq!(StartupStep::BusInit.name(), "bus_init");
        assert_eq!(StartupStep::AxisEngineStart.name(), "axis_engine_start");
        assert_eq!(StartupStep::IpcListenerStart.name(), "ipc_listener_start");
    }
}
