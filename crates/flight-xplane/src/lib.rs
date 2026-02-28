// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

#![allow(clippy::collapsible_if)]
#![allow(clippy::unnecessary_map_or)]
#![allow(clippy::needless_range_loop)]
#![allow(dead_code)]

//! X-Plane adapter for Flight Hub
//!
//! Provides DataRef access via UDP and plugin interfaces, aircraft detection,
//! web API integration, and comprehensive latency measurement and validation.

pub mod adapter;
pub mod adapter_state;
pub mod aircraft;
pub mod aircraft_db;
pub mod aircraft_detection;
pub mod control;
pub mod control_injection;
pub mod dataref;
pub mod dataref_db;
pub mod datarefs;
pub mod failure_state;
pub mod fixtures;
pub mod latency;
pub mod plugin;
pub mod plugin_protocol;
pub mod udp;
pub mod udp_protocol;
pub mod web_api;

pub use adapter::{XPlaneAdapter, XPlaneAdapterConfig, XPlaneError};
pub use adapter_state::{AdapterEvent, AdapterStateMachine, TransitionError, XPlaneAdapterState};
pub use aircraft::{AircraftDetector, DetectedAircraft, XPlaneAircraftInfo};
pub use aircraft_db::{AircraftCategory, AircraftDatabase, XPlaneAircraftEntry};
pub use aircraft_detection::{
    AircraftChange, AircraftDbMatch, EnhancedAircraftDetector, EnhancedAircraftId,
};
pub use control::ControlOutput;
pub use control_injection::{ControlInjectionError, ControlInjectorConfig, XPlaneControlInjector};
pub use dataref::{DataRef, DataRefManager, DataRefRequest, DataRefValue};
pub use dataref_db::{DatarefDatabase, DatarefInfo, DatarefType};
pub use datarefs::{DatarefManager, DatarefSubscription};
pub use failure_state::FailureState;
pub use flight_adapter_common::{AdapterMetrics, AdapterState};
pub use latency::{LatencyBudget, LatencyMeasurement, LatencyTracker};
pub use plugin::{PluginInterface, PluginMessage, PluginResponse};
pub use plugin_protocol::{
    PluginDiscovery, PluginDiscoveryState, PluginProtoMessage, ProtocolError,
};
pub use udp::{UdpClient, UdpConfig, UdpError};
pub use udp_protocol::{
    DataGroup, ParseError, XPlaneDataPacket, build_cmnd_command, build_dref_command,
    parse_data_packet, parse_rref_response,
};
pub use web_api::{WebApiClient, WebApiConfig, WebApiError};
