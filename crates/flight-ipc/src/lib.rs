// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Flight Hub IPC Layer
//!
//! Provides protobuf-based IPC communication between Flight Hub components
//! using named pipes on Windows and Unix domain sockets on Linux.
//!
//! # Overview
//!
//! This crate implements the inter-process communication layer for Flight Hub with:
//!
//! - **Cross-platform Transport**: Named pipes (Windows) and Unix sockets (Linux)
//! - **Protocol Versioning**: Feature negotiation and compatibility checking
//! - **Type Safety**: Protobuf-generated types with validation
//! - **Security**: Local-only communication with OS ACLs
//!
//! # Examples
//!
//! ## Client Connection
//!
//! ```rust,no_run
//! use flight_ipc::{ClientConfig, client::FlightClient};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = ClientConfig::default();
//!     let mut client = FlightClient::connect(config).await?;
//!     
//!     let devices = client.list_devices().await?;
//!     println!("Found {} devices", devices.len());
//!     
//!     Ok(())
//! }
//! ```
//!
//! ## Server Setup
//!
//! ```rust,no_run
//! use flight_ipc::{ServerConfig, server::FlightServer};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = ServerConfig::default();
//!     let server = FlightServer::new(config);
//!     
//!     server.serve().await?;
//!     Ok(())
//! }
//! ```
//!
//! # Protocol Versioning
//!
//! The IPC layer supports feature negotiation to handle version compatibility:
//!
//! - Clients declare supported features during connection
//! - Server responds with enabled feature set
//! - Incompatible versions are rejected with clear error messages

use thiserror::Error;

pub mod proto {
    tonic::include_proto!("flight.v1");
}

pub mod transport;
pub mod client;
pub mod server;
pub mod negotiation;
#[cfg(test)]
mod fd_safety_tests;

pub use proto::*;

/// IPC protocol version - increment for breaking changes
pub const PROTOCOL_VERSION: &str = "1.0.0";

/// Supported feature flags
pub const SUPPORTED_FEATURES: &[&str] = &[
    "device-management",
    "health-monitoring", 
    "profile-management",
    "force-feedback",
    "real-time-telemetry",
];

#[derive(Debug, Error)]
pub enum IpcError {
    #[error("Transport error: {0}")]
    Transport(#[from] transport::TransportError),
    
    #[error("Protocol version mismatch: client={client}, server={server}")]
    VersionMismatch { client: String, server: String },
    
    #[error("Feature not supported: {feature}")]
    UnsupportedFeature { feature: String },
    
    #[error("Connection failed: {reason}")]
    ConnectionFailed { reason: String },
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("gRPC error: {0}")]
    Grpc(#[from] tonic::Status),
}

/// Feature negotiation result
#[derive(Debug, Clone)]
pub struct NegotiationResult {
    pub server_version: String,
    pub enabled_features: Vec<String>,
    pub transport_type: TransportType,
}

/// Client configuration
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub client_version: String,
    pub supported_features: Vec<String>,
    pub preferred_transport: TransportType,
    pub connection_timeout_ms: u64,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            client_version: PROTOCOL_VERSION.to_string(),
            supported_features: SUPPORTED_FEATURES.iter().map(|s| s.to_string()).collect(),
            preferred_transport: default_transport_type(),
            connection_timeout_ms: 5000,
        }
    }
}

/// Server configuration  
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub server_version: String,
    pub enabled_features: Vec<String>,
    pub bind_address: String,
    pub max_connections: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            server_version: PROTOCOL_VERSION.to_string(),
            enabled_features: SUPPORTED_FEATURES.iter().map(|s| s.to_string()).collect(),
            bind_address: default_bind_address(),
            max_connections: 100,
        }
    }
}

/// Get the default transport type for the current platform
pub fn default_transport_type() -> TransportType {
    #[cfg(windows)]
    return TransportType::NamedPipes;
    
    #[cfg(unix)]
    return TransportType::UnixSockets;
}

/// Get the default bind address for the current platform
pub fn default_bind_address() -> String {
    #[cfg(windows)]
    return r"\\.\pipe\flight-hub".to_string();
    
    #[cfg(unix)]
    return "/tmp/flight-hub.sock".to_string();
}
