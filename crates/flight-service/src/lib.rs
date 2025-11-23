#![cfg_attr(
    test,
    allow(
        unused_imports,
        unused_variables,
        unused_mut,
        unused_assignments,
        unused_parens,
        dead_code
    )
)]
// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Flight Hub Service Library
//!
//! Provides the main service implementation for Flight Hub including
//! axis processing, curve conflict detection, and simulator integration.

pub mod aircraft_auto_switch_service;
pub mod capability_service;
pub mod curve_conflict_service;
pub mod error_taxonomy;
pub mod health;
pub mod one_click_resolver;
pub mod power;
pub mod safe_mode;
pub mod service;

#[cfg(test)]
mod acceptance_tests;
#[cfg(test)]
mod capability_integration_tests;
#[cfg(test)]
mod fd_safety_tests;
#[cfg(test)]
mod integration_tests;
#[cfg(test)]
mod test_service;

pub use aircraft_auto_switch_service::{
    AdapterConfigs, AdapterMetrics, AircraftAutoSwitchService, AircraftAutoSwitchServiceConfig,
    BusSubscriptionConfig, ServiceMetrics,
};
pub use capability_service::{
    AxisCapabilityStatus, CapabilityService, CapabilityServiceConfig, SetCapabilityResult,
};
pub use curve_conflict_service::{CurveConflictService, CurveConflictServiceConfig};
pub use error_taxonomy::{ErrorCategory, ErrorCode, ErrorTaxonomy, StableError};
pub use health::{ComponentHealth, HealthEvent, HealthStatus, HealthStream};
pub use one_click_resolver::{
    OneClickResolver, OneClickResolverConfig, OneClickResult, ResolutionMetrics,
    VerificationOutcome,
};
pub use power::{PowerCheck, PowerCheckStatus, PowerChecker, PowerStatus, RemediationStep};
pub use safe_mode::{
    RtPrivilegeStatus, SafeModeConfig, SafeModeManager, SafeModeStatus, ValidationResult,
};
pub use service::{FlightService, FlightServiceConfig, ServiceState};
