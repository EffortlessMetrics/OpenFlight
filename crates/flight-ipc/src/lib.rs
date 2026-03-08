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
//! use flight_ipc::client::FlightClient;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut client = FlightClient::connect().await?;
//!     
//!     // Use the client for IPC operations
//!     println!("Connected to Flight service");
//!     
//!     Ok(())
//! }
//! ```
//!
//! ## Server Setup
//!
//! ```rust,no_run
//! use flight_ipc::server::FlightServer;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let server = FlightServer::new();
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

/// Auto-generated gRPC types from `flight.v1` protobuf definitions
pub mod proto {
    #![allow(missing_docs)] // generated code
    tonic::include_proto!("flight.v1");

    pub use flight_service_client::FlightServiceClient as GrpcFlightServiceClient;
    pub use flight_service_server::{
        FlightService as GrpcFlightService, FlightServiceServer as GrpcFlightServiceServer,
    };
}

pub mod client;
pub mod connection_pool;
#[cfg(test)]
mod fd_safety_tests;
pub mod handlers;
pub mod message_types;
pub mod messages;
pub mod negotiation;
pub mod rate_limiter;
pub mod server;
pub mod subscription;
pub mod subscriptions;
pub mod transport;

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

/// IPC-level errors for connection, protocol negotiation, and serialization failures
#[derive(Debug, Error)]
pub enum IpcError {
    /// Underlying transport I/O failure
    #[error("Transport error: {0}")]
    Transport(#[from] transport::TransportError),

    /// Client and server advertise incompatible protocol versions
    #[error("Protocol version mismatch: client={client}, server={server}")]
    VersionMismatch {
        /// The version the client presented
        client: String,
        /// The version the server requires
        server: String,
    },

    /// A required feature is not supported by this endpoint
    #[error("Feature not supported: {feature}")]
    UnsupportedFeature {
        /// The feature identifier that was requested
        feature: String,
    },

    /// Could not establish a connection to the daemon
    #[error("Connection failed: {reason}")]
    ConnectionFailed {
        /// Human-readable description of the failure cause
        reason: String,
    },

    /// JSON serialization/deserialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// gRPC status error from tonic
    #[error("gRPC error: {0}")]
    Grpc(#[from] tonic::Status),
}

/// Feature negotiation result
#[derive(Debug, Clone)]
pub struct NegotiationResult {
    /// Protocol version advertised by the server
    pub server_version: String,
    /// Feature flags that are active for this connection
    pub enabled_features: Vec<String>,
    /// Transport mechanism selected for the connection
    pub transport_type: TransportType,
}

/// Client configuration
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Protocol version this client implements
    pub client_version: String,
    /// Feature flags the client is willing to use
    pub supported_features: Vec<String>,
    /// Preferred transport for this client
    pub preferred_transport: TransportType,
    /// Connection attempt timeout in milliseconds
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
    /// Protocol version this server implements
    pub server_version: String,
    /// Feature flags enabled on this server
    pub enabled_features: Vec<String>,
    /// Address the gRPC server binds to (e.g. `127.0.0.1:50051`)
    pub bind_address: String,
    /// Maximum simultaneous client connections
    pub max_connections: usize,
    /// Per-request timeout
    pub request_timeout: std::time::Duration,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            server_version: PROTOCOL_VERSION.to_string(),
            enabled_features: SUPPORTED_FEATURES.iter().map(|s| s.to_string()).collect(),
            bind_address: default_bind_address(),
            max_connections: 100,
            request_timeout: std::time::Duration::from_secs(5),
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
