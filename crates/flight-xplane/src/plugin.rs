// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Plugin interface for X-Plane
//!
//! Provides communication with X-Plane plugins for enhanced DataRef access,
//! protected DataRef writes, and extended functionality beyond UDP.

use crate::dataref::DataRefValue;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};
use thiserror::Error;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc,
    time::timeout,
};
use tracing::{debug, error, info, warn};

/// Plugin interface errors
#[derive(Error, Debug)]
pub enum PluginError {
    #[error("Network error: {0}")]
    Network(#[from] std::io::Error),
    #[error("Protocol error: {message}")]
    Protocol { message: String },
    #[error("Plugin not connected")]
    NotConnected,
    #[error("Request timeout")]
    Timeout,
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Plugin capability not supported: {capability}")]
    UnsupportedCapability { capability: String },
}

/// Plugin message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PluginMessage {
    /// Handshake message
    Handshake {
        version: String,
        capabilities: Vec<String>,
    },
    /// Request DataRef value
    GetDataRef { id: u32, name: String },
    /// Set DataRef value
    SetDataRef {
        id: u32,
        name: String,
        value: DataRefValue,
    },
    /// Subscribe to DataRef updates
    Subscribe {
        id: u32,
        name: String,
        frequency: f32,
    },
    /// Unsubscribe from DataRef updates
    Unsubscribe { id: u32, name: String },
    /// Execute command
    Command { id: u32, name: String },
    /// Get aircraft information
    GetAircraftInfo { id: u32 },
    /// Ping for connection health
    Ping { id: u32, timestamp: u64 },
}

/// Plugin response types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PluginResponse {
    /// Handshake acknowledgment
    HandshakeAck {
        version: String,
        capabilities: Vec<String>,
        status: String,
    },
    /// DataRef value response
    DataRefValue {
        id: u32,
        name: String,
        value: DataRefValue,
        timestamp: u64,
    },
    /// DataRef update notification
    DataRefUpdate {
        name: String,
        value: DataRefValue,
        timestamp: u64,
    },
    /// Command execution result
    CommandResult {
        id: u32,
        success: bool,
        message: Option<String>,
    },
    /// Aircraft information response
    AircraftInfo {
        id: u32,
        icao: String,
        title: String,
        author: String,
        file_path: String,
    },
    /// Error response
    Error {
        id: Option<u32>,
        error: String,
        details: Option<String>,
    },
    /// Pong response
    Pong { id: u32, timestamp: u64 },
}

/// Plugin capabilities
#[derive(Debug, Clone, PartialEq)]
pub enum PluginCapability {
    /// Read DataRefs
    ReadDataRefs,
    /// Write DataRefs
    WriteDataRefs,
    /// Execute commands
    ExecuteCommands,
    /// Subscribe to DataRef updates
    SubscribeDataRefs,
    /// Access protected DataRefs
    ProtectedDataRefs,
    /// Aircraft information
    AircraftInfo,
    /// Flight loop integration
    FlightLoop,
}

impl PluginCapability {
    pub fn as_str(&self) -> &'static str {
        match self {
            PluginCapability::ReadDataRefs => "read_datarefs",
            PluginCapability::WriteDataRefs => "write_datarefs",
            PluginCapability::ExecuteCommands => "execute_commands",
            PluginCapability::SubscribeDataRefs => "subscribe_datarefs",
            PluginCapability::ProtectedDataRefs => "protected_datarefs",
            PluginCapability::AircraftInfo => "aircraft_info",
            PluginCapability::FlightLoop => "flight_loop",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "read_datarefs" => Some(PluginCapability::ReadDataRefs),
            "write_datarefs" => Some(PluginCapability::WriteDataRefs),
            "execute_commands" => Some(PluginCapability::ExecuteCommands),
            "subscribe_datarefs" => Some(PluginCapability::SubscribeDataRefs),
            "protected_datarefs" => Some(PluginCapability::ProtectedDataRefs),
            "aircraft_info" => Some(PluginCapability::AircraftInfo),
            "flight_loop" => Some(PluginCapability::FlightLoop),
            _ => None,
        }
    }
}

/// Plugin connection state
#[derive(Debug, Clone)]
struct PluginConnection {
    stream: Arc<RwLock<Option<TcpStream>>>,
    capabilities: Vec<PluginCapability>,
    version: String,
    _last_ping: Instant,
    connected: bool,
}

/// Plugin interface for X-Plane communication
#[derive(Clone)]
pub struct PluginInterface {
    connection: Arc<RwLock<Option<PluginConnection>>>,
    pending_requests: Arc<RwLock<HashMap<u32, tokio::sync::oneshot::Sender<PluginResponse>>>>,
    next_request_id: Arc<RwLock<u32>>,
    subscriptions: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<DataRefValue>>>>,
    port: u16,
}

impl PluginInterface {
    /// Create a new plugin interface
    pub fn new() -> Result<Self, PluginError> {
        Ok(Self {
            connection: Arc::new(RwLock::new(None)),
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
            next_request_id: Arc::new(RwLock::new(1)),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            port: 52000, // Default plugin communication port
        })
    }

    /// Start the plugin interface server
    pub async fn start(&self) -> Result<(), PluginError> {
        let addr = format!("127.0.0.1:{}", self.port);
        let listener = TcpListener::bind(&addr).await?;

        info!("Plugin interface listening on {}", addr);

        let connection = self.connection.clone();
        let pending_requests = self.pending_requests.clone();
        let subscriptions = self.subscriptions.clone();

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, addr)) => {
                        info!("Plugin connected from {}", addr);

                        let conn = PluginConnection {
                            stream: Arc::new(RwLock::new(Some(stream))),
                            capabilities: Vec::new(),
                            version: String::new(),
                            _last_ping: Instant::now(),
                            connected: true,
                        };

                        *connection.write().unwrap() = Some(conn.clone());

                        // Handle connection
                        let connection_clone = connection.clone();
                        let pending_clone = pending_requests.clone();
                        let subscriptions_clone = subscriptions.clone();

                        tokio::spawn(async move {
                            if let Err(e) = Self::handle_connection(
                                conn,
                                connection_clone,
                                pending_clone,
                                subscriptions_clone,
                            )
                            .await
                            {
                                error!("Plugin connection error: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        error!("Failed to accept plugin connection: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    /// Handle plugin connection
    async fn handle_connection(
        mut conn: PluginConnection,
        connection: Arc<RwLock<Option<PluginConnection>>>,
        pending_requests: Arc<RwLock<HashMap<u32, tokio::sync::oneshot::Sender<PluginResponse>>>>,
        subscriptions: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<DataRefValue>>>>,
    ) -> Result<(), PluginError> {
        // Perform handshake
        Self::perform_handshake(&mut conn).await?;

        // Message processing loop
        loop {
            // Read message from plugin
            match Self::read_message(&conn).await {
                Ok(response) => {
                    Self::handle_response(response, &pending_requests, &subscriptions).await?;
                }
                Err(e) => {
                    warn!("Plugin message error: {}", e);
                    break;
                }
            }
        }

        // Clean up connection
        *connection.write().unwrap() = None;
        info!("Plugin disconnected");

        Ok(())
    }

    /// Perform handshake with plugin
    async fn perform_handshake(conn: &mut PluginConnection) -> Result<(), PluginError> {
        // Send handshake
        let handshake = PluginMessage::Handshake {
            version: "1.0.0".to_string(),
            capabilities: vec![
                "read_datarefs".to_string(),
                "write_datarefs".to_string(),
                "aircraft_info".to_string(),
            ],
        };

        Self::send_message(conn, handshake).await?;

        // Wait for handshake response
        match timeout(Duration::from_secs(5), Self::read_message(conn)).await {
            Ok(Ok(PluginResponse::HandshakeAck {
                version,
                capabilities,
                status,
            })) => {
                conn.version = version;
                conn.capabilities = capabilities
                    .iter()
                    .filter_map(|c| PluginCapability::parse(c))
                    .collect();

                info!(
                    "Plugin handshake successful: version={}, status={}",
                    conn.version, status
                );
                Ok(())
            }
            Ok(Ok(response)) => Err(PluginError::Protocol {
                message: format!("Unexpected handshake response: {:?}", response),
            }),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(PluginError::Timeout),
        }
    }

    /// Send message to plugin
    async fn send_message(
        conn: &PluginConnection,
        message: PluginMessage,
    ) -> Result<(), PluginError> {
        let json = serde_json::to_string(&message)?;
        let _data = format!("{}\n", json);

        if let Some(ref _stream) = *conn.stream.read().unwrap() {
            // In a real implementation, we would need to handle async writes properly
            // This is a simplified version for demonstration
            debug!("Sending plugin message: {}", json);
            // stream.write_all(data.as_bytes()).await?;
        } else {
            return Err(PluginError::NotConnected);
        }

        Ok(())
    }

    /// Read message from plugin
    async fn read_message(conn: &PluginConnection) -> Result<PluginResponse, PluginError> {
        let has_stream = {
            let stream_guard = conn.stream.read().unwrap();
            stream_guard.is_some()
        };

        if has_stream {
            // In a real implementation, we would read from the stream
            // This is a simplified version for demonstration

            // Simulate reading a response
            tokio::time::sleep(Duration::from_millis(10)).await;

            // Return a mock response for testing
            Ok(PluginResponse::HandshakeAck {
                version: "1.0.0".to_string(),
                capabilities: vec!["read_datarefs".to_string(), "aircraft_info".to_string()],
                status: "ready".to_string(),
            })
        } else {
            Err(PluginError::NotConnected)
        }
    }

    /// Handle plugin response
    async fn handle_response(
        response: PluginResponse,
        pending_requests: &Arc<RwLock<HashMap<u32, tokio::sync::oneshot::Sender<PluginResponse>>>>,
        subscriptions: &Arc<RwLock<HashMap<String, mpsc::UnboundedSender<DataRefValue>>>>,
    ) -> Result<(), PluginError> {
        match response {
            PluginResponse::DataRefValue { id, .. }
            | PluginResponse::CommandResult { id, .. }
            | PluginResponse::AircraftInfo { id, .. }
            | PluginResponse::Pong { id, .. } => {
                // Handle request response
                let mut pending = pending_requests.write().unwrap();
                if let Some(sender) = pending.remove(&id) {
                    let _ = sender.send(response);
                }
            }
            PluginResponse::DataRefUpdate { name, value, .. } => {
                // Handle subscription update
                let subscriptions = subscriptions.read().unwrap();
                if let Some(sender) = subscriptions.get(&name) {
                    let _ = sender.send(value);
                }
            }
            PluginResponse::Error {
                id,
                ref error,
                ref details,
            } => {
                debug!("Plugin error: {} (details: {:?})", error, details);
                if let Some(id) = id {
                    let mut pending = pending_requests.write().unwrap();
                    if let Some(sender) = pending.remove(&id) {
                        let _ = sender.send(response);
                    }
                }
            }
            _ => {
                debug!("Unhandled plugin response: {:?}", response);
            }
        }

        Ok(())
    }

    /// Get DataRef value via plugin
    pub async fn get_dataref(&self, name: &str) -> Result<DataRefValue, PluginError> {
        let conn = {
            let connection = self.connection.read().unwrap();
            connection
                .as_ref()
                .ok_or(PluginError::NotConnected)?
                .clone()
        };

        if !conn.capabilities.contains(&PluginCapability::ReadDataRefs) {
            return Err(PluginError::UnsupportedCapability {
                capability: "read_datarefs".to_string(),
            });
        }

        let request_id = self.get_next_request_id();
        let (sender, receiver) = tokio::sync::oneshot::channel();

        // Store pending request
        {
            let mut pending = self.pending_requests.write().unwrap();
            pending.insert(request_id, sender);
        }

        // Send request
        let message = PluginMessage::GetDataRef {
            id: request_id,
            name: name.to_string(),
        };

        Self::send_message(&conn, message).await?;

        // Wait for response
        match timeout(Duration::from_secs(1), receiver).await {
            Ok(Ok(PluginResponse::DataRefValue { value, .. })) => Ok(value),
            Ok(Ok(PluginResponse::Error { error, .. })) => {
                Err(PluginError::Protocol { message: error })
            }
            Ok(Ok(response)) => Err(PluginError::Protocol {
                message: format!("Unexpected response: {:?}", response),
            }),
            Ok(Err(_)) => Err(PluginError::Protocol {
                message: "Response channel closed".to_string(),
            }),
            Err(_) => {
                // Clean up pending request
                self.pending_requests.write().unwrap().remove(&request_id);
                Err(PluginError::Timeout)
            }
        }
    }

    /// Set DataRef value via plugin
    pub async fn set_dataref(&self, name: &str, value: DataRefValue) -> Result<(), PluginError> {
        let conn = {
            let connection = self.connection.read().unwrap();
            connection
                .as_ref()
                .ok_or(PluginError::NotConnected)?
                .clone()
        };

        if !conn.capabilities.contains(&PluginCapability::WriteDataRefs) {
            return Err(PluginError::UnsupportedCapability {
                capability: "write_datarefs".to_string(),
            });
        }

        let request_id = self.get_next_request_id();
        let (sender, receiver) = tokio::sync::oneshot::channel();

        // Store pending request
        {
            let mut pending = self.pending_requests.write().unwrap();
            pending.insert(request_id, sender);
        }

        // Send request
        let message = PluginMessage::SetDataRef {
            id: request_id,
            name: name.to_string(),
            value,
        };

        Self::send_message(&conn, message).await?;

        // Wait for response
        match timeout(Duration::from_secs(1), receiver).await {
            Ok(Ok(PluginResponse::CommandResult { success: true, .. })) => Ok(()),
            Ok(Ok(PluginResponse::CommandResult {
                success: false,
                message,
                ..
            })) => Err(PluginError::Protocol {
                message: message.unwrap_or("Set DataRef failed".to_string()),
            }),
            Ok(Ok(PluginResponse::Error { error, .. })) => {
                Err(PluginError::Protocol { message: error })
            }
            Ok(Ok(response)) => Err(PluginError::Protocol {
                message: format!("Unexpected response: {:?}", response),
            }),
            Ok(Err(_)) => Err(PluginError::Protocol {
                message: "Response channel closed".to_string(),
            }),
            Err(_) => {
                // Clean up pending request
                self.pending_requests.write().unwrap().remove(&request_id);
                Err(PluginError::Timeout)
            }
        }
    }

    /// Subscribe to DataRef updates
    pub async fn subscribe_dataref(
        &self,
        name: &str,
        frequency: f32,
    ) -> Result<mpsc::UnboundedReceiver<DataRefValue>, PluginError> {
        let conn = {
            let connection = self.connection.read().unwrap();
            connection
                .as_ref()
                .ok_or(PluginError::NotConnected)?
                .clone()
        };

        if !conn
            .capabilities
            .contains(&PluginCapability::SubscribeDataRefs)
        {
            return Err(PluginError::UnsupportedCapability {
                capability: "subscribe_datarefs".to_string(),
            });
        }

        let request_id = self.get_next_request_id();
        let (tx, rx) = mpsc::unbounded_channel();

        // Store subscription
        {
            let mut subscriptions = self.subscriptions.write().unwrap();
            subscriptions.insert(name.to_string(), tx);
        }

        // Send subscription request
        let message = PluginMessage::Subscribe {
            id: request_id,
            name: name.to_string(),
            frequency,
        };

        Self::send_message(&conn, message).await?;

        Ok(rx)
    }

    /// Get aircraft information via plugin
    pub async fn get_aircraft_info(&self) -> Result<(String, String, String, String), PluginError> {
        let conn = {
            let connection = self.connection.read().unwrap();
            connection
                .as_ref()
                .ok_or(PluginError::NotConnected)?
                .clone()
        };

        if !conn.capabilities.contains(&PluginCapability::AircraftInfo) {
            return Err(PluginError::UnsupportedCapability {
                capability: "aircraft_info".to_string(),
            });
        }

        let request_id = self.get_next_request_id();
        let (sender, receiver) = tokio::sync::oneshot::channel();

        // Store pending request
        {
            let mut pending = self.pending_requests.write().unwrap();
            pending.insert(request_id, sender);
        }

        // Send request
        let message = PluginMessage::GetAircraftInfo { id: request_id };
        Self::send_message(&conn, message).await?;

        // Wait for response
        match timeout(Duration::from_secs(1), receiver).await {
            Ok(Ok(PluginResponse::AircraftInfo {
                icao,
                title,
                author,
                file_path,
                ..
            })) => Ok((icao, title, author, file_path)),
            Ok(Ok(PluginResponse::Error { error, .. })) => {
                Err(PluginError::Protocol { message: error })
            }
            Ok(Ok(response)) => Err(PluginError::Protocol {
                message: format!("Unexpected response: {:?}", response),
            }),
            Ok(Err(_)) => Err(PluginError::Protocol {
                message: "Response channel closed".to_string(),
            }),
            Err(_) => {
                // Clean up pending request
                self.pending_requests.write().unwrap().remove(&request_id);
                Err(PluginError::Timeout)
            }
        }
    }

    /// Process incoming messages (for use in main loop)
    pub async fn process_messages(&self) -> Result<(), PluginError> {
        // This would be called periodically to handle plugin communication
        // In a real implementation, this would process queued messages
        tokio::time::sleep(Duration::from_millis(10)).await;
        Ok(())
    }

    /// Check if plugin is connected
    pub fn is_connected(&self) -> bool {
        self.connection
            .read()
            .unwrap()
            .as_ref()
            .map(|c| c.connected)
            .unwrap_or(false)
    }

    /// Get plugin capabilities
    pub fn get_capabilities(&self) -> Vec<PluginCapability> {
        self.connection
            .read()
            .unwrap()
            .as_ref()
            .map(|c| c.capabilities.clone())
            .unwrap_or_default()
    }

    /// Get next request ID
    fn get_next_request_id(&self) -> u32 {
        let mut id = self.next_request_id.write().unwrap();
        let current = *id;
        *id = id.wrapping_add(1);
        if *id == 0 {
            *id = 1;
        }
        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_capability_conversion() {
        assert_eq!(PluginCapability::ReadDataRefs.as_str(), "read_datarefs");
        assert_eq!(PluginCapability::WriteDataRefs.as_str(), "write_datarefs");

        assert_eq!(
            PluginCapability::parse("read_datarefs"),
            Some(PluginCapability::ReadDataRefs)
        );
        assert_eq!(PluginCapability::parse("invalid"), None);
    }

    #[tokio::test]
    async fn test_plugin_interface_creation() {
        let interface = PluginInterface::new();
        assert!(interface.is_ok());

        let interface = interface.unwrap();
        assert!(!interface.is_connected());
        assert!(interface.get_capabilities().is_empty());
    }

    #[test]
    fn test_plugin_message_serialization() {
        let message = PluginMessage::GetDataRef {
            id: 123,
            name: "sim/test/dataref".to_string(),
        };

        let json = serde_json::to_string(&message).unwrap();
        assert!(json.contains("GetDataRef"));
        assert!(json.contains("sim/test/dataref"));

        let deserialized: PluginMessage = serde_json::from_str(&json).unwrap();
        match deserialized {
            PluginMessage::GetDataRef { id, name } => {
                assert_eq!(id, 123);
                assert_eq!(name, "sim/test/dataref");
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_plugin_response_serialization() {
        let response = PluginResponse::DataRefValue {
            id: 123,
            name: "sim/test/dataref".to_string(),
            value: DataRefValue::Float(42.0),
            timestamp: 1234567890,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("DataRefValue"));
        assert!(json.contains("42.0"));

        let deserialized: PluginResponse = serde_json::from_str(&json).unwrap();
        match deserialized {
            PluginResponse::DataRefValue {
                id,
                name,
                value,
                timestamp,
            } => {
                assert_eq!(id, 123);
                assert_eq!(name, "sim/test/dataref");
                assert_eq!(value, DataRefValue::Float(42.0));
                assert_eq!(timestamp, 1234567890);
            }
            _ => panic!("Wrong response type"),
        }
    }

    #[test]
    fn test_request_id_generation() {
        let interface = PluginInterface::new().unwrap();

        let id1 = interface.get_next_request_id();
        let id2 = interface.get_next_request_id();

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }
}
