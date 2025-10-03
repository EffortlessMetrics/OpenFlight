// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Flight Hub Service Library
//!
//! Provides the main service implementation for Flight Hub including
//! axis processing, curve conflict detection, and simulator integration.

pub mod curve_conflict_service;
pub mod one_click_resolver;
pub mod capability_service;
pub mod aircraft_auto_switch_service;
pub mod power;
pub mod safe_mode;
pub mod service;
pub mod health;
pub mod error_taxonomy;

#[cfg(test)]
mod integration_tests;
#[cfg(test)]
mod capability_integration_tests;
#[cfg(test)]
mod test_service;
#[cfg(test)]
mod acceptance_tests;

pub use curve_conflict_service::{CurveConflictService, CurveConflictServiceConfig};
pub use one_click_resolver::{OneClickResolver, OneClickResolverConfig, OneClickResult, VerificationOutcome, ResolutionMetrics};
pub use capability_service::{CapabilityService, CapabilityServiceConfig, SetCapabilityResult, AxisCapabilityStatus};
pub use aircraft_auto_switch_service::{
    AircraftAutoSwitchService, AircraftAutoSwitchServiceConfig, BusSubscriptionConfig, 
    AdapterConfigs, ServiceMetrics, AdapterMetrics
};
pub use power::{PowerChecker, PowerStatus, PowerCheckStatus, PowerCheck, RemediationStep};
pub use safe_mode::{SafeModeManager, SafeModeConfig, SafeModeStatus, RtPrivilegeStatus, ValidationResult};
pub use service::{FlightService, FlightServiceConfig, ServiceState};
pub use health::{HealthStream, HealthEvent, HealthStatus, ComponentHealth};
pub use error_taxonomy::{ErrorTaxonomy, ErrorCode, ErrorCategory, StableError};
