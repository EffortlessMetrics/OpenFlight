// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! X-Plane adapter for Flight Hub
//!
//! Provides DataRef access via UDP and plugin interfaces, aircraft detection,
//! web API integration, and comprehensive latency measurement and validation.

pub mod adapter;
pub mod aircraft;
pub mod dataref;
pub mod fixtures;
pub mod latency;
pub mod plugin;
pub mod udp;
pub mod web_api;

pub use adapter::{XPlaneAdapter, XPlaneAdapterConfig, XPlaneError};
pub use flight_adapter_common::{AdapterMetrics, AdapterState};
pub use aircraft::{AircraftDetector, DetectedAircraft, XPlaneAircraftInfo};
pub use dataref::{DataRef, DataRefManager, DataRefRequest, DataRefValue};
pub use latency::{LatencyBudget, LatencyMeasurement, LatencyTracker};
pub use plugin::{PluginInterface, PluginMessage, PluginResponse};
pub use udp::{UdpClient, UdpConfig, UdpError};
pub use web_api::{WebApiClient, WebApiConfig, WebApiError};
