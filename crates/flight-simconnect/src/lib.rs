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
pub mod mapping;
pub mod session;

// Re-export main types
pub use adapter::{MsfsAdapter, MsfsAdapterConfig, MsfsAdapterError};
pub use aircraft::{AircraftDetector, AircraftInfo, DetectionError};
pub use events::{EventManager, InputEvent, SimEvent};
pub use fixtures::{SessionFixture, FixtureRecorder, FixturePlayer};
pub use mapping::{VariableMapping, MappingConfig, MappingError};
pub use session::{SimConnectSession, SessionConfig, SessionError};
