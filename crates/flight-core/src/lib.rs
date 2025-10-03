//! Flight Hub Core Library
//!
//! Provides core data structures, profile management, and shared utilities
//! for the Flight Hub flight simulation input management system.
//!
//! # Overview
//!
//! This crate contains the foundational components used throughout Flight Hub:
//!
//! - **Profile Management**: JSON Schema-based flight profiles with validation and merging
//! - **Rules Engine**: DSL for panel LED control with hysteresis and rate limiting
//! - **Security**: Plugin capability management and IPC access control
//! - **Diagnostics**: Blackbox recording and watchdog systems
//! - **Aircraft Detection**: Auto-switching between aircraft profiles
//!
//! # Examples
//!
//! ## Basic Profile Creation
//!
//! ```rust
//! use flight_core::profile::{Profile, AxisConfig, AircraftId};
//! use std::collections::HashMap;
//!
//! let mut axes = HashMap::new();
//! axes.insert("pitch".to_string(), AxisConfig {
//!     deadzone: Some(0.03),
//!     expo: Some(0.2),
//!     slew_rate: Some(1.2),
//!     detents: vec![],
//!     curve: None,
//! });
//!
//! let profile = Profile {
//!     schema: "flight.profile/1".to_string(),
//!     sim: Some("msfs".to_string()),
//!     aircraft: Some(AircraftId { icao: "C172".to_string() }),
//!     axes,
//!     pof_overrides: None,
//! };
//!
//! // Validate the profile
//! profile.validate().expect("Profile should be valid");
//! ```
//!
//! ## Rules DSL Usage
//!
//! ```rust
//! use flight_core::rules::{RulesSchema, Rule};
//!
//! let rules = RulesSchema {
//!     schema: "flight.ledmap/1".to_string(),
//!     rules: vec![
//!         Rule {
//!             when: "gear_down".to_string(),
//!             do_action: "led.panel('GEAR').on()".to_string(),
//!             action: "led.panel('GEAR').on()".to_string(),
//!         }
//!     ],
//!     defaults: None,
//! };
//!
//! rules.validate().expect("Rules should be valid");
//! ```

pub mod error;
pub mod profile;
pub mod rules;
pub mod units;
pub mod writers;
pub mod watchdog;
pub mod aircraft_switch;
pub mod process_detection;
pub mod blackbox;
pub mod security;

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
pub use security::{
    SecurityManager, SecurityConfig, TelemetryConfig, AclConfig, PluginCapabilityManifest,
    PluginCapability, PluginType, SignatureStatus, TelemetryDataType, IpcClientInfo,
    SecurityError
};
