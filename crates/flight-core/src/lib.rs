//! Flight Hub Core Library
//!
//! Provides core data structures, profile management, and shared utilities
//! for the Flight Hub flight simulation input management system.

pub mod error;
pub mod profile;
pub mod rules;
pub mod units;
pub mod writers;
pub mod watchdog;
pub mod aircraft_switch;
pub mod process_detection;
pub mod blackbox;

pub use error::{FlightError, Result};
pub use writers::{CurveConflictWriter, WritersConfig, WriteResult, VerificationResult, BackupInfo};
pub use profile::{CapabilityMode, CapabilityLimits, CapabilityContext};
pub use watchdog::{
    WatchdogSystem, WatchdogConfig, WatchdogEvent, WatchdogEventType, WatchdogAction,
    ComponentType, QuarantineStatus, SyntheticFault, WatchdogError,
    PluginOverrunStats, WatchdogHealthSummary
};
pub use aircraft_switch::{
    AircraftAutoSwitch, AutoSwitchConfig, PofHysteresisConfig, HysteresisBand,
    DetectedAircraft, CompiledProfile, PhaseOfFlight, SwitchMetrics, SwitchResult
};
pub use process_detection::{
    ProcessDetector, ProcessDetectionConfig, ProcessDefinition, DetectedProcess, DetectionMetrics
};
pub use blackbox::{
    BlackboxWriter, BlackboxReader, BlackboxConfig, BlackboxStats, BlackboxError,
    BlackboxHeader, BlackboxFooter, IndexEntry, StreamType, BlackboxRecord
};
