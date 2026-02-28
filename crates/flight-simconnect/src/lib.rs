#![cfg(windows)]
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

//! MSFS SimConnect Adapter
//!
//! This crate provides a high-level interface to Microsoft Flight Simulator via SimConnect.
//! It implements the Flight Hub adapter pattern for normalized telemetry publishing and
//! aircraft detection with auto-profile switching.
//!
//! # Features
//! - SimConnect variable reading and event sending
//! - Input Events for modern aircraft compatibility
//! - Aircraft detection via ATC model/type
//! - Normalized telemetry publishing at 30-60Hz
//! - Integration tests with recorded session fixtures
//!
//! # Requirements
//! - Windows operating system
//! - Microsoft Flight Simulator 2020 or later
//! - SimConnect SDK (dynamic loading supported)

pub mod adapter;
pub mod adapter_state;
pub mod aircraft;
pub mod aircraft_db;
pub mod aircraft_detection;
pub mod camera;
pub mod connection;
pub mod control_injection;
pub mod engine_params;
pub mod event_mapping;
pub mod events;
pub mod fixtures;
pub mod injection;
pub mod mapping;
pub mod sanity_gate;
pub mod session;
pub mod simconnect_bridge;
pub mod subscription;
pub mod transport;
pub mod var_registry;
pub mod weather;

// Re-export main types
pub use adapter::{MsfsAdapter, MsfsAdapterConfig, MsfsAdapterError};
pub use aircraft::{AircraftDetector, AircraftInfo, DetectionError};
pub use camera::{CameraChannel, CameraConfig, format_camera_simvar};
pub use engine_params::{EngineParameters, parse_engine_params, simvars_for_engines};
pub use events::{EventManager, InputEvent, SimEvent};
pub use fixtures::{FixturePlayer, FixtureRecorder, SessionFixture};
pub use flight_adapter_common::{AdapterMetrics, AdapterState};
pub use injection::{AxisInjectionConfig, AxisInjector};
pub use mapping::{MappingConfig, MappingError, VariableMapping};
pub use sanity_gate::{SanityGate, SanityGateConfig, SanityState};
pub use session::{SessionConfig, SessionError, SimConnectSession};
pub use subscription::{
    CORE_SUBSCRIPTION_VARS, DataSubscription, DataSubscriptionConfig, SubscriptionVariable,
};
pub use weather::{WeatherConfig, WeatherData, parse_weather_simvars};

pub use adapter_state::{
    SimConnectAdapterState, SimConnectEvent, SimConnectStateMachine, SimConnectTransitionError,
};
pub use aircraft_db::{AircraftType, MsfsAircraftDb, MsfsAircraftInfo};
pub use event_mapping::{
    SimEventCategory, SimEventDef, SimEventMapper, catalog_by_category, catalog_lookup,
};
pub use var_registry::{SimVar, SimVarCategory, SimVarRegistry};

pub use aircraft_detection::{
    AircraftDetectionEngine, AircraftEntry, DetectionResult, MatchConfidence, SimAircraftData,
};
pub use connection::{
    ConnectionConfig, ConnectionEvent, ConnectionState, ConnectionTransitionError,
    ExponentialBackoff, HealthMonitor, SimConnectConnection,
};
pub use control_injection::{
    AxisId, ControlInjectorConfig, InjectionCommand, RateLimiter, SimControlInjector,
};
pub use simconnect_bridge::{
    AircraftChanged, BackendError, BridgeConfig, DispatchMessage, MockSimConnectBackend,
    SimConnectBackend, SimConnectBridge, VarSnapshot,
};
