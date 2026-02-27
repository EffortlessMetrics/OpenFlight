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

//! Flight Bus - Normalized telemetry model and publisher
//!
//! Provides a comprehensive telemetry bus system with type-safe units,
//! rate-limited publishing, and consistent test fixtures for flight simulation data.

pub mod adapter_fixtures;
pub mod adapters;
pub mod e2e_test;
pub mod fixtures;
pub mod integration_test;
pub mod publisher;
pub mod replay;
pub mod snapshot;
pub mod telemetry_aggregator;
pub mod types;

// Re-export main types for convenience
pub use adapter_fixtures::{
    AdapterFixture, BuiltinFixtures, DcsFixture, FixtureConverter, FixtureError, FixtureLoader,
    MsfsFixture, XPlaneFixture,
};
pub use e2e_test::{
    E2EDiagnostics, E2ETestConfig, E2ETestError, E2ETestResult, EndToEndTest, FfbStateSnapshot,
    FrameHistoryEntry, MockFfbEngine, MockTelemetryBus, SafetyViolationDetail, SafetyViolationType,
    SnapshotStateInfo,
};
pub use fixtures::{ScenarioType, SnapshotFixture, SnapshotValidator, ValidationTolerance};
pub use integration_test::{
    AdapterIntegrationTest, AdapterType, IntegrationTestResult, MockAdapter, PhaseResult, TestError,
};
pub use publisher::{BusPublisher, PublisherError, Subscriber, SubscriberId, SubscriptionConfig};
pub use replay::{ReplayConfig, ReplayIterator, TelemetryRecord, TelemetryRecording};
pub use snapshot::{
    AircraftConfig, BusSnapshot, EngineData, Environment, HeloData, Kinematics, LightsConfig,
    Navigation,
};
pub use telemetry_aggregator::{BusTelemetry, TelemetryAggregator, TopicMetrics};
pub use types::{
    AircraftId, AutopilotState, BusTypeError, GForce, GearPosition, GearState, Mach, Percentage,
    SimId, ValidatedAngle, ValidatedSpeed,
};
