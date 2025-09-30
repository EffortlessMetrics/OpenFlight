//! Flight Hub IPC Layer
//!
//! Provides protobuf-based IPC communication between Flight Hub components
//! using named pipes on Windows and Unix domain sockets on Linux.

use thiserror::Error;

pub mod proto {
    tonic::include_proto!("flight.v1");
}

pub mod transport;
pub mod client;
pub mod server;
pub mod negotiation;

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
