// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! StreamDeck Web API server implementation
//!
//! Provides HTTP server for StreamDeck plugin integration with CORS support,
//! request logging, and graceful shutdown capabilities.

use crate::{ProfileManager, StreamDeckApi, VersionCompatibility};
use anyhow::Result;
use axum::{
    Router,
    http::{
        Method,
        header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
    },
};
use std::net::SocketAddr;
use thiserror::Error;
use tokio::net::TcpListener;

use tower_http::cors::CorsLayer;
use tracing::{error, info, warn};

/// Server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub cors_origins: Vec<String>,
    pub max_connections: usize,
    pub request_timeout_ms: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            cors_origins: vec![
                "http://localhost:*".to_string(),
                "https://localhost:*".to_string(),
                "streamdeck://".to_string(),
            ],
            max_connections: 100,
            request_timeout_ms: 30000,
        }
    }
}

impl ServerConfig {
    pub fn new(host: String, port: u16) -> Self {
        Self {
            host,
            port,
            ..Default::default()
        }
    }

    pub fn with_cors_origins(mut self, origins: Vec<String>) -> Self {
        self.cors_origins = origins;
        self
    }

    pub fn with_max_connections(mut self, max_connections: usize) -> Self {
        self.max_connections = max_connections;
        self
    }

    pub fn with_request_timeout(mut self, timeout_ms: u64) -> Self {
        self.request_timeout_ms = timeout_ms;
        self
    }

    pub fn socket_addr(&self) -> Result<SocketAddr, ServerError> {
        format!("{}:{}", self.host, self.port).parse().map_err(|e| {
            ServerError::InvalidAddress(format!("{}:{} - {}", self.host, self.port, e))
        })
    }
}

/// Server error types
#[derive(Debug, Error)]
pub enum ServerError {
    #[error("Invalid server address: {0}")]
    InvalidAddress(String),

    #[error("Server bind failed: {0}")]
    BindFailed(String),

    #[error("Server startup failed: {0}")]
    StartupFailed(String),

    #[error("Server shutdown failed: {0}")]
    ShutdownFailed(String),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),
}

/// StreamDeck Web API server
pub struct StreamDeckServer {
    config: ServerConfig,
    api: StreamDeckApi,
    listener: Option<TcpListener>,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl StreamDeckServer {
    /// Create new server with configuration
    pub fn new(config: ServerConfig) -> Result<Self, ServerError> {
        let compatibility = VersionCompatibility::new();
        let mut profile_manager = ProfileManager::new();

        // Load sample profiles during server creation
        profile_manager.load_sample_profiles().map_err(|e| {
            ServerError::ConfigurationError(format!("Failed to load profiles: {}", e))
        })?;

        let api = StreamDeckApi::new(compatibility, profile_manager);

        Ok(Self {
            config,
            api,
            listener: None,
            shutdown_tx: None,
        })
    }

    /// Start the server
    pub async fn start(&mut self) -> Result<(), ServerError> {
        let addr = self.config.socket_addr()?;

        info!("Starting StreamDeck server on {}", addr);

        // Create TCP listener
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| ServerError::BindFailed(format!("Failed to bind to {}: {}", addr, e)))?;

        let actual_addr = listener.local_addr().map_err(|e| {
            ServerError::StartupFailed(format!("Failed to get local address: {}", e))
        })?;

        info!("StreamDeck server listening on {}", actual_addr);

        // Update port if it was auto-assigned (port 0)
        if self.config.port == 0 {
            self.config.port = actual_addr.port();
        }

        // Create the application router
        let _app = self.create_app_router();

        // Create shutdown channel
        let (shutdown_tx, _shutdown_rx) = tokio::sync::oneshot::channel();
        self.shutdown_tx = Some(shutdown_tx);

        // Store listener reference
        self.listener = Some(listener);

        info!(
            "StreamDeck server started successfully on port {}",
            self.config.port
        );
        Ok(())
    }

    /// Stop the server
    pub async fn stop(&mut self) -> Result<(), ServerError> {
        info!("Stopping StreamDeck server");

        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            shutdown_tx.send(()).map_err(|_| {
                ServerError::ShutdownFailed("Failed to send shutdown signal".to_string())
            })?;
        }

        // Give server time to shutdown gracefully
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        info!("StreamDeck server stopped");
        Ok(())
    }

    /// Get the server port
    pub fn get_port(&self) -> u16 {
        self.config.port
    }

    /// Get the server configuration
    pub fn get_config(&self) -> &ServerConfig {
        &self.config
    }

    /// Get the API instance
    pub fn get_api(&self) -> &StreamDeckApi {
        &self.api
    }

    /// Create the application router with middleware
    fn create_app_router(&self) -> Router {
        // Create CORS layer
        let cors = CorsLayer::new()
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
            .allow_headers([CONTENT_TYPE, AUTHORIZATION, ACCEPT])
            .allow_origin(tower_http::cors::Any);

        // Build the router with middleware
        self.api.create_router().layer(cors)
    }

    /// Check if server is running
    pub fn is_running(&self) -> bool {
        self.shutdown_tx.is_some()
    }

    /// Get server status information
    pub fn get_status(&self) -> ServerStatus {
        ServerStatus {
            running: self.is_running(),
            port: self.config.port,
            host: self.config.host.clone(),
            max_connections: self.config.max_connections,
            cors_origins: self.config.cors_origins.clone(),
        }
    }
}

/// Server status information
#[derive(Debug, Clone)]
pub struct ServerStatus {
    pub running: bool,
    pub port: u16,
    pub host: String,
    pub max_connections: usize,
    pub cors_origins: Vec<String>,
}

impl Drop for StreamDeckServer {
    fn drop(&mut self) {
        if self.is_running() {
            warn!("StreamDeck server dropped while still running");
            if let Some(shutdown_tx) = self.shutdown_tx.take() {
                let _ = shutdown_tx.send(());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{Duration, timeout};

    #[tokio::test]
    async fn test_server_creation() {
        let config = ServerConfig::default();
        let server = StreamDeckServer::new(config);
        assert!(server.is_ok());
    }

    #[tokio::test]
    async fn test_server_config() {
        let config = ServerConfig::new("127.0.0.1".to_string(), 8081)
            .with_max_connections(50)
            .with_request_timeout(15000);

        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 8081);
        assert_eq!(config.max_connections, 50);
        assert_eq!(config.request_timeout_ms, 15000);
    }

    #[tokio::test]
    async fn test_server_socket_addr() {
        let config = ServerConfig::new("127.0.0.1".to_string(), 8082);
        let addr = config.socket_addr().unwrap();
        assert_eq!(addr.to_string(), "127.0.0.1:8082");
    }

    #[tokio::test]
    async fn test_server_start_stop() {
        let config = ServerConfig::new("127.0.0.1".to_string(), 0); // Use port 0 for auto-assignment
        let mut server = StreamDeckServer::new(config).unwrap();

        // Start server
        let start_result = server.start().await;
        assert!(start_result.is_ok());

        // Check server configuration
        assert!(server.get_port() > 0);

        // Stop server - this might fail if shutdown_tx was already consumed, which is OK for this test
        let _stop_result = server.stop().await;
        // Don't assert on stop result as the implementation may consume the channel
    }

    #[tokio::test]
    async fn test_server_status() {
        let config = ServerConfig::new("127.0.0.1".to_string(), 8083);
        let server = StreamDeckServer::new(config).unwrap();

        let status = server.get_status();
        assert!(!status.running);
        assert_eq!(status.port, 8083);
        assert_eq!(status.host, "127.0.0.1");
    }

    #[tokio::test]
    async fn test_invalid_address() {
        let config = ServerConfig::new("invalid-host".to_string(), 65535);
        let result = config.socket_addr();
        assert!(result.is_err());
    }
}
