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
pub mod aircraft;
pub mod events;
pub mod fixtures;
pub mod injection;
pub mod mapping;
pub mod sanity_gate;
pub mod session;
pub mod subscription;
pub mod transport;

// Re-export main types
pub use adapter::{MsfsAdapter, MsfsAdapterConfig, MsfsAdapterError};
pub use aircraft::{AircraftDetector, AircraftInfo, DetectionError};
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
