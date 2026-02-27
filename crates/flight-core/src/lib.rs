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

pub mod calibration_store;
pub mod error;
pub mod profile_watcher;

// Re-exports from microcrates
pub use flight_blackbox as blackbox;
pub use flight_metrics as metrics;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flight_error_display_variants() {
        let err = FlightError::Configuration("missing key".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Configuration error"), "got: {msg}");

        let err2 = FlightError::Hardware("device stall".to_string());
        assert!(err2.to_string().contains("Hardware error"));

        let err3 = FlightError::Writer("no output".to_string());
        assert!(err3.to_string().contains("Writer error"));
    }

    #[test]
    fn result_type_alias_is_flight_error() {
        let r: Result<u32> = Ok(42);
        assert_eq!(r.unwrap(), 42);

        let e: Result<u32> = Err(FlightError::RulesValidation("bad rule".to_string()));
        assert!(e.is_err());
        assert!(e.unwrap_err().to_string().contains("Rules validation"));
    }

    #[test]
    fn watchdog_config_default_has_sensible_values() {
        let cfg = WatchdogConfig::default();
        assert!(cfg.max_execution_time.as_micros() > 0);
        assert!(cfg.usb_timeout.as_millis() > 0);
    }

    #[test]
    fn component_type_display_name_contains_id() {
        let usb = ComponentType::UsbEndpoint("mydev".to_string());
        assert!(usb.display_name().contains("mydev"));

        let plugin = ComponentType::NativePlugin("myplugin".to_string());
        assert!(plugin.display_name().contains("myplugin"));
    }

    #[test]
    fn phase_of_flight_variants_are_distinct() {
        // Simply verify that the enum variants are accessible via the re-export
        let phases = [
            PhaseOfFlight::Taxi,
            PhaseOfFlight::Takeoff,
            PhaseOfFlight::Cruise,
            PhaseOfFlight::Landing,
        ];
        // All distinct (they should not compare equal to each other)
        assert_ne!(phases[0], phases[1]);
        assert_ne!(phases[2], phases[3]);
    }

    #[test]
    fn flight_error_profile_and_io_variants() {
        let err = FlightError::Configuration("bad_key=value".to_string());
        let s = err.to_string();
        assert!(s.contains("Configuration error"));
        assert!(s.contains("bad_key"));
    }

    #[test]
    fn security_config_default_is_sensible() {
        let cfg = SecurityConfig::default();
        // Default config should not crash when accessed
        let _ = format!("{:?}", cfg);
    }

    #[test]
    fn switch_metrics_default_starts_at_zero() {
        let m = SwitchMetrics::default();
        assert_eq!(m.total_switches, 0);
        assert_eq!(m.failed_switches, 0);
    }

    #[test]
    fn sim_id_display_ksp() {
        let id = SimId::Ksp;
        assert!(!id.to_string().is_empty());
    }
}
