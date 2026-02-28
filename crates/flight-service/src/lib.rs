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
pub mod audit_log;
pub mod capability_service;
pub mod config_validator;
pub mod config_watcher;
pub mod crash_report;
pub mod curve_conflict_service;
pub mod degradation_manager;
pub mod diagnostic_bundle;
pub mod error_taxonomy;
pub mod event_journal;
pub mod first_run;
pub mod graceful_drain;
pub mod health;
pub mod health_http;
pub mod health_report;
pub mod input_runtime;
pub mod instance_lock;
pub mod metrics_server;
pub mod one_click_resolver;
pub mod perf_profiler;
pub mod plugin;
pub mod plugin_registry;
pub mod power;
pub mod safe_mode;
pub mod service;
pub mod shutdown_coordinator;
pub mod startup_sequence;
pub mod stecs_runtime;
pub mod task_supervisor;

#[cfg(feature = "tflight-hidapi")]
pub mod hidapi_source;
#[cfg(feature = "stecs-hidapi")]
pub mod stecs_hidapi_source;

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
    AdapterConfigs, AdapterMetrics, AdapterState, AircraftAutoSwitchService,
    AircraftAutoSwitchServiceConfig, BusSubscriptionConfig, ServiceMetrics,
};
pub use capability_service::{
    AxisCapabilityStatus, CapabilityService, CapabilityServiceConfig, SetCapabilityResult,
};
pub use curve_conflict_service::{CurveConflictService, CurveConflictServiceConfig};
pub use error_taxonomy::{ErrorCategory, ErrorCode, ErrorTaxonomy, StableError};
pub use health::{ComponentHealth, HealthEvent, HealthStatus, HealthStream};
pub use input_runtime::{
    SimulatedTFlightReportSource, TFlightInputRuntime, TFlightReportSource, TFlightRuntimeConfig,
    TFlightSnapshot,
};
pub use one_click_resolver::{
    OneClickResolver, OneClickResolverConfig, OneClickResult, ResolutionMetrics,
    VerificationOutcome,
};
pub use power::{PowerCheck, PowerCheckStatus, PowerChecker, PowerStatus, RemediationStep};
pub use safe_mode::{
    RtPrivilegeStatus, SafeModeConfig, SafeModeDiagnostic, SafeModeManager, SafeModeStatus,
    ValidationResult,
};
pub use service::{FlightService, FlightServiceConfig, ServiceState, TFlightYawPolicyConfig};
pub use stecs_runtime::{
    SimulatedVkbStecsReportSource, VkbStecsInputRuntime, VkbStecsReportSource,
    VkbStecsRuntimeConfig, VkbStecsSnapshot,
};

pub use perf_profiler::{PerfProfiler, PerfReport, SpanStats};
pub use plugin::{Plugin, PluginError, PluginErrorKind, PluginState, PluginTier};
pub use plugin_registry::PluginRegistry;
