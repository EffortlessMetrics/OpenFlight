// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

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
//!     filter: None,
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

// Re-exports from microcrates
pub use flight_blackbox as blackbox;
pub use flight_process_detection as process_detection;
pub use flight_profile as profile;
pub use flight_rules as rules;
pub use flight_security as security;
pub use flight_session as aircraft_switch;
pub use flight_units as units;
pub use flight_watchdog as watchdog;
pub use flight_writers as writers;

pub use aircraft_switch::{
    AircraftAutoSwitch, AutoSwitchConfig, CompiledProfile, DetectedAircraft, HysteresisBand,
    PhaseOfFlight, PofHysteresisConfig, SessionError, SwitchMetrics, SwitchResult,
};
pub use blackbox::{BlackboxError, BlackboxHeader, BlackboxRecord};
pub use error::{FlightError, Result};
pub use process_detection::{
    DetectedProcess, DetectionMetrics, ProcessDefinition, ProcessDetectionConfig,
    ProcessDetectionError, ProcessDetector, SimId,
};
pub use profile::{CapabilityContext, CapabilityLimits, CapabilityMode};
pub use security::{
    AclConfig, IpcClientInfo, PluginCapability, PluginCapabilityManifest, PluginType,
    SecurityConfig, SecurityError, SecurityManager, SecurityVerifier, SignatureStatus,
    TelemetryConfig, TelemetryDataType, VerificationConfig, VerificationStatus,
};
pub use watchdog::{
    ComponentType, PluginOverrunStats, QuarantineStatus, SyntheticFault, WatchdogAction,
    WatchdogConfig, WatchdogError, WatchdogEvent, WatchdogEventType, WatchdogHealthSummary,
};
pub use writers::{
    BackupInfo, CurveConflictError, CurveConflictWriter, VerificationResult, WriteResult,
    WritersConfig,
};
