//! Flight Hub Service Library
//!
//! Provides the main service implementation for Flight Hub including
//! axis processing, curve conflict detection, and simulator integration.

pub mod curve_conflict_service;
pub mod one_click_resolver;

#[cfg(test)]
mod integration_tests;

pub use curve_conflict_service::{CurveConflictService, CurveConflictServiceConfig};
pub use one_click_resolver::{OneClickResolver, OneClickResolverConfig, OneClickResult, VerificationOutcome, ResolutionMetrics};
