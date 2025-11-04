#![cfg_attr(test, allow(unused_imports, unused_variables, unused_mut, unused_assignments, unused_parens, dead_code))]

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Flight Bus - Normalized telemetry model and publisher
//!
//! Provides a comprehensive telemetry bus system with type-safe units,
//! rate-limited publishing, and consistent test fixtures for flight simulation data.

pub mod adapters;
pub mod fixtures;
pub mod publisher;
pub mod snapshot;
pub mod types;

// Re-export main types for convenience
pub use fixtures::{ScenarioType, SnapshotFixture, SnapshotValidator, ValidationTolerance};
pub use publisher::{BusPublisher, PublisherError, Subscriber, SubscriberId, SubscriptionConfig};
pub use snapshot::{
    AircraftConfig, BusSnapshot, EngineData, Environment, HeloData, Kinematics, LightsConfig,
    Navigation,
};
pub use types::{
    AircraftId, AutopilotState, BusTypeError, GForce, GearPosition, GearState, Mach, Percentage,
    SimId, ValidatedAngle, ValidatedSpeed,
};
