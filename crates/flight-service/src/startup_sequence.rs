//! Service startup sequence with ordered initialization steps.

use tracing::{info, warn};

/// Startup phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupPhase {
    /// Initial state, not yet started.
    Idle,
    /// Checking environment (config, disk, permissions).
    PreFlight,
    /// Loading configuration.
    LoadingConfig,
    /// Initializing hardware enumeration.
    EnumeratingDevices,
    /// Starting axis engine.
    StartingAxisEngine,
    /// Starting simulator adapters.
    StartingAdapters,
    /// Service fully operational.
    Running,
    /// Startup failed; service in degraded mode.
    Failed(StartupFailure),
}

/// Startup failure reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupFailure {
    /// Config file missing or invalid.
    ConfigError,
    /// No input devices found.
    NoDevices,
    /// Real-time scheduling failed (non-fatal: uses normal scheduling).
    RtSchedulingUnavailable,
    /// Axis engine initialization failed.
    AxisEngineError,
}

/// Startup sequence state machine.
pub struct StartupSequence {
    phase: StartupPhase,
    warnings: Vec<String>,
}

impl StartupSequence {
    /// Create a new startup sequence.
    pub fn new() -> Self {
        Self {
            phase: StartupPhase::Idle,
            warnings: Vec::new(),
        }
    }

    /// Current phase.
    pub fn phase(&self) -> StartupPhase {
        self.phase
    }

    /// Collected startup warnings.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Advance through pre-flight checks.
    pub fn run_preflight(&mut self) {
        self.phase = StartupPhase::PreFlight;
        info!("startup: pre-flight checks");
    }

    /// Advance to config loading phase.
    pub fn loading_config(&mut self) {
        self.phase = StartupPhase::LoadingConfig;
        info!("startup: loading configuration");
    }

    /// Advance to device enumeration phase.
    pub fn enumerating_devices(&mut self) {
        self.phase = StartupPhase::EnumeratingDevices;
        info!("startup: enumerating devices");
    }

    /// Advance to axis engine startup phase.
    pub fn starting_axis_engine(&mut self) {
        self.phase = StartupPhase::StartingAxisEngine;
        info!("startup: starting axis engine");
    }

    /// Advance to adapter startup phase.
    pub fn starting_adapters(&mut self) {
        self.phase = StartupPhase::StartingAdapters;
        info!("startup: starting sim adapters");
    }

    /// Mark service as fully running.
    pub fn running(&mut self) {
        self.phase = StartupPhase::Running;
        if !self.warnings.is_empty() {
            warn!("startup complete with {} warning(s)", self.warnings.len());
        } else {
            info!("startup: service is running");
        }
    }

    /// Mark startup as failed.
    pub fn fail(&mut self, reason: StartupFailure) {
        self.phase = StartupPhase::Failed(reason);
        warn!("startup failed: {:?}", reason);
    }

    /// Add a startup warning.
    pub fn warn(&mut self, msg: impl Into<String>) {
        let s = msg.into();
        warn!("startup warning: {}", s);
        self.warnings.push(s);
    }

    /// Returns true if the service reached the Running state.
    pub fn is_running(&self) -> bool {
        self.phase == StartupPhase::Running
    }
}

impl Default for StartupSequence {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_sequence_starts_idle() {
        let seq = StartupSequence::new();
        assert_eq!(seq.phase(), StartupPhase::Idle);
        assert!(!seq.is_running());
    }

    #[test]
    fn sequence_advances_through_phases() {
        let mut seq = StartupSequence::new();
        seq.run_preflight();
        assert_eq!(seq.phase(), StartupPhase::PreFlight);
        seq.loading_config();
        assert_eq!(seq.phase(), StartupPhase::LoadingConfig);
        seq.enumerating_devices();
        assert_eq!(seq.phase(), StartupPhase::EnumeratingDevices);
        seq.starting_axis_engine();
        assert_eq!(seq.phase(), StartupPhase::StartingAxisEngine);
        seq.starting_adapters();
        assert_eq!(seq.phase(), StartupPhase::StartingAdapters);
        seq.running();
        assert_eq!(seq.phase(), StartupPhase::Running);
        assert!(seq.is_running());
    }

    #[test]
    fn fail_transitions_to_failed() {
        let mut seq = StartupSequence::new();
        seq.run_preflight();
        seq.fail(StartupFailure::ConfigError);
        assert_eq!(
            seq.phase(),
            StartupPhase::Failed(StartupFailure::ConfigError)
        );
        assert!(!seq.is_running());
    }

    #[test]
    fn warnings_are_collected() {
        let mut seq = StartupSequence::new();
        seq.warn("RT scheduling unavailable");
        seq.warn("No devices found");
        assert_eq!(seq.warnings().len(), 2);
        assert!(seq.warnings()[0].contains("RT"));
    }

    #[test]
    fn running_with_warnings_is_ok() {
        let mut seq = StartupSequence::new();
        seq.warn("minor issue");
        seq.running();
        assert!(seq.is_running());
        assert_eq!(seq.warnings().len(), 1);
    }

    #[test]
    fn default_is_idle() {
        let seq = StartupSequence::default();
        assert_eq!(seq.phase(), StartupPhase::Idle);
    }

    #[test]
    fn startup_failure_config_error() {
        let mut seq = StartupSequence::new();
        seq.fail(StartupFailure::ConfigError);
        matches!(
            seq.phase(),
            StartupPhase::Failed(StartupFailure::ConfigError)
        );
    }

    #[test]
    fn startup_failure_no_devices() {
        let mut seq = StartupSequence::new();
        seq.fail(StartupFailure::NoDevices);
        matches!(seq.phase(), StartupPhase::Failed(StartupFailure::NoDevices));
    }
}
