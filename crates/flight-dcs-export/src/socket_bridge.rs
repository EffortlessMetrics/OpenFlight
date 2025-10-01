//! Socket bridge for DCS Export.lua communication
//!
//! Implements version negotiation and message framing for reliable
//! communication with DCS Export.lua scripts.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

/// Protocol version for DCS communication
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolVersion {
    pub major: u8,
    pub minor: u8,
}

impl ProtocolVersion {
    pub const V1_0: Self = Self { major: 1, minor: 0 };
    
    pub fn is_compatible(&self, other: &Self) -> bool {
        self.major == other.major
    }
}

impl std::fmt::Display for ProtocolVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// Configuration for socket bridge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocketBridgeConfig {
    /// Local address to bind to (default: 127.0.0.1:7778)
    pub bind_addr: SocketAddr,
    /// Connection timeout
    pub connect_timeout: Duration,
    /// Heartbeat interval
    pub heartbeat_interval: Duration,
    /// Maximum message size
    pub max_message_size: usize,
    /// Supported protocol versions (in preference order)
    pub supported_versions: Vec<ProtocolVersion>,
}

impl Default for SocketBridgeConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:7778".parse().unwrap(),
            connect_timeout: Duration::from_secs(5),
            heartbeat_interval: Duration::from_secs(10),
            max_message_size: 64 * 1024, // 64KB
            supported_versions: vec![ProtocolVersion::V1_0],
        }
    }
}

/// Messages exchanged with DCS Export.lua
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum DcsMessage {
    /// Version negotiation handshake
    Handshake {
        version: ProtocolVersion,
        features: Vec<String>,
    },
    /// Handshake acknowledgment
    HandshakeAck {
        version: ProtocolVersion,
        accepted_features: Vec<String>,
    },
    /// Telemetry data frame
    Telemetry {
        timestamp: u64,
        aircraft: String,
        session_type: String, // "SP" or "MP"
        data: HashMap<String, serde_json::Value>,
    },
    /// Heartbeat to maintain connection
    Heartbeat {
        timestamp: u64,
    },
    /// Error message
    Error {
        code: String,
        message: String,
    },
}

/// Socket bridge for DCS communication
pub struct SocketBridge {
    config: SocketBridgeConfig,
    listener: Option<TcpListener>,
    active_connections: HashMap<SocketAddr, ConnectionState>,
    message_tx: mpsc::UnboundedSender<(SocketAddr, DcsMessage)>,
    message_rx: mpsc::UnboundedReceiver<(SocketAddr, DcsMessage)>,
}

#[derive(Debug)]
struct ConnectionState {
    stream: TcpStream,
    version: Option<ProtocolVersion>,
    features: Vec<String>,
    last_heartbeat: Instant,
}

impl SocketBridge {
    /// Create new socket bridge
    pub fn new(config: SocketBridgeConfig) -> Self {
        let (message_tx, message_rx) = mpsc::unbounded_channel();
        
        Self {
            config,
            listener: None,
            active_connections: HashMap::new(),
            message_tx,
            message_rx,
        }
    }

    /// Start listening for DCS connections
    pub async fn start(&mut self) -> Result<()> {
        let listener = TcpListener::bind(self.config.bind_addr)
            .await
            .context("Failed to bind socket bridge")?;
        
        info!("DCS socket bridge listening on {}", self.config.bind_addr);
        self.listener = Some(listener);
        Ok(())
    }

    /// Accept new DCS connection
    pub async fn accept_connection(&mut self) -> Result<Option<SocketAddr>> {
        let listener = match &self.listener {
            Some(l) => l,
            None => return Ok(None),
        };

        match timeout(Duration::from_millis(100), listener.accept()).await {
            Ok(Ok((stream, addr))) => {
                info!("DCS connection from {}", addr);
                
                let connection = ConnectionState {
                    stream,
                    version: None,
                    features: Vec::new(),
                    last_heartbeat: Instant::now(),
                };
                
                self.active_connections.insert(addr, connection);
                Ok(Some(addr))
            }
            Ok(Err(e)) => {
                error!("Failed to accept DCS connection: {}", e);
                Err(e.into())
            }
            Err(_) => Ok(None), // Timeout, no connection pending
        }
    }

    /// Process messages from DCS connections
    pub async fn process_messages(&mut self) -> Result<Vec<(SocketAddr, DcsMessage)>> {
        let mut messages = Vec::new();
        
        // Collect all available messages without blocking
        while let Ok((addr, message)) = self.message_rx.try_recv() {
            messages.push((addr, message));
        }

        // Process each active connection
        let mut to_remove = Vec::new();
        let addrs: Vec<SocketAddr> = self.active_connections.keys().copied().collect();
        
        for addr in addrs {
            if let Some(connection) = self.active_connections.get_mut(&addr) {
                match Self::read_messages_from_connection_static(addr, connection).await {
                    Ok(conn_messages) => {
                        messages.extend(conn_messages);
                    }
                    Err(e) => {
                        warn!("Connection {} error: {}", addr, e);
                        to_remove.push(addr);
                    }
                }
            }
        }

        // Remove failed connections
        for addr in to_remove {
            self.active_connections.remove(&addr);
            info!("Removed failed DCS connection {}", addr);
        }

        Ok(messages)
    }

    /// Read messages from a specific connection (static version to avoid borrowing issues)
    async fn read_messages_from_connection_static(
        addr: SocketAddr,
        connection: &mut ConnectionState,
    ) -> Result<Vec<(SocketAddr, DcsMessage)>> {
        let mut messages = Vec::new();
        let mut reader = BufReader::new(&mut connection.stream);
        let mut line = String::new();

        // Try to read a line without blocking
        match timeout(Duration::from_millis(1), reader.read_line(&mut line)).await {
            Ok(Ok(0)) => {
                // Connection closed
                return Err(anyhow::anyhow!("Connection closed"));
            }
            Ok(Ok(_)) => {
                // Parse message
                match serde_json::from_str::<DcsMessage>(&line.trim()) {
                    Ok(message) => {
                        debug!("Received DCS message from {}: {:?}", addr, message);
                        messages.push((addr, message));
                    }
                    Err(e) => {
                        warn!("Failed to parse DCS message from {}: {}", addr, e);
                    }
                }
            }
            Ok(Err(e)) => {
                return Err(e.into());
            }
            Err(_) => {
                // Timeout, no data available
            }
        }

        Ok(messages)
    }

    /// Send message to DCS connection
    pub async fn send_message(&mut self, addr: SocketAddr, message: DcsMessage) -> Result<()> {
        let connection = self.active_connections.get_mut(&addr)
            .context("Connection not found")?;

        let json = serde_json::to_string(&message)
            .context("Failed to serialize message")?;
        
        let line = format!("{}\n", json);
        
        connection.stream.write_all(line.as_bytes()).await
            .context("Failed to send message")?;
        
        connection.stream.flush().await
            .context("Failed to flush message")?;

        debug!("Sent DCS message to {}: {:?}", addr, message);
        Ok(())
    }

    /// Perform handshake with DCS connection
    pub async fn handshake(&mut self, addr: SocketAddr, client_message: DcsMessage) -> Result<()> {
        let (client_version, client_features) = match client_message {
            DcsMessage::Handshake { version, features } => (version, features),
            _ => return Err(anyhow::anyhow!("Expected handshake message")),
        };

        // Find compatible version
        let compatible_version = self.config.supported_versions
            .iter()
            .find(|v| v.is_compatible(&client_version))
            .copied()
            .unwrap_or(ProtocolVersion::V1_0);

        // Filter supported features
        let accepted_features = self.filter_supported_features(&client_features);

        // Update connection state
        if let Some(connection) = self.active_connections.get_mut(&addr) {
            connection.version = Some(compatible_version);
            connection.features = accepted_features.clone();
        }

        // Send handshake acknowledgment
        let ack = DcsMessage::HandshakeAck {
            version: compatible_version,
            accepted_features,
        };

        self.send_message(addr, ack).await?;
        
        info!("Completed DCS handshake with {} (version {})", addr, compatible_version);
        Ok(())
    }

    /// Filter client features to only supported ones
    fn filter_supported_features(&self, client_features: &[String]) -> Vec<String> {
        // Define supported features
        let supported = [
            "telemetry_basic",
            "telemetry_navigation", 
            "telemetry_engines",
            "telemetry_weapons", // MP-blocked
            "telemetry_countermeasures", // MP-blocked
            "session_detection",
        ];

        client_features
            .iter()
            .filter(|f| supported.contains(&f.as_str()))
            .cloned()
            .collect()
    }

    /// Check for stale connections and send heartbeats
    pub async fn maintain_connections(&mut self) -> Result<()> {
        let now = Instant::now();
        let mut to_remove = Vec::new();

        for (addr, connection) in &mut self.active_connections {
            // Check for stale connections
            if now.duration_since(connection.last_heartbeat) > self.config.heartbeat_interval * 3 {
                warn!("DCS connection {} timed out", addr);
                to_remove.push(*addr);
                continue;
            }

            // Send heartbeat if needed
            if now.duration_since(connection.last_heartbeat) > self.config.heartbeat_interval {
                let heartbeat = DcsMessage::Heartbeat {
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or(std::time::Duration::from_secs(0))
                        .as_millis() as u64,
                };
                
                // Send heartbeat directly to avoid borrowing issues
                let json = serde_json::to_string(&heartbeat)
                    .context("Failed to serialize heartbeat")?;
                let line = format!("{}\n", json);
                
                match connection.stream.write_all(line.as_bytes()).await {
                    Ok(()) => {
                        if let Err(_) = connection.stream.flush().await {
                            to_remove.push(*addr);
                        } else {
                            connection.last_heartbeat = now;
                        }
                    }
                    Err(_) => {
                        to_remove.push(*addr);
                    }
                }
            }
        }

        // Remove stale connections
        for addr in to_remove {
            self.active_connections.remove(&addr);
            info!("Removed stale DCS connection {}", addr);
        }

        Ok(())
    }

    /// Get active connection count
    pub fn connection_count(&self) -> usize {
        self.active_connections.len()
    }

    /// Get connection info
    pub fn get_connection_info(&self, addr: SocketAddr) -> Option<(ProtocolVersion, Vec<String>)> {
        self.active_connections.get(&addr)
            .and_then(|conn| conn.version.map(|v| (v, conn.features.clone())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_socket_bridge_creation() {
        let config = SocketBridgeConfig::default();
        let bridge = SocketBridge::new(config);
        assert_eq!(bridge.connection_count(), 0);
    }

    #[tokio::test]
    async fn test_protocol_version_compatibility() {
        let v1_0 = ProtocolVersion::V1_0;
        let v1_1 = ProtocolVersion { major: 1, minor: 1 };
        let v2_0 = ProtocolVersion { major: 2, minor: 0 };

        assert!(v1_0.is_compatible(&v1_1));
        assert!(v1_1.is_compatible(&v1_0));
        assert!(!v1_0.is_compatible(&v2_0));
    }

    #[tokio::test]
    async fn test_message_serialization() {
        let handshake = DcsMessage::Handshake {
            version: ProtocolVersion::V1_0,
            features: vec!["telemetry_basic".to_string()],
        };

        let json = serde_json::to_string(&handshake).unwrap();
        let parsed: DcsMessage = serde_json::from_str(&json).unwrap();

        match parsed {
            DcsMessage::Handshake { version, features } => {
                assert_eq!(version, ProtocolVersion::V1_0);
                assert_eq!(features, vec!["telemetry_basic"]);
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[tokio::test]
    async fn test_feature_filtering() {
        let config = SocketBridgeConfig::default();
        let bridge = SocketBridge::new(config);

        let client_features = vec![
            "telemetry_basic".to_string(),
            "telemetry_weapons".to_string(),
            "unsupported_feature".to_string(),
        ];

        let accepted = bridge.filter_supported_features(&client_features);
        assert!(accepted.contains(&"telemetry_basic".to_string()));
        assert!(accepted.contains(&"telemetry_weapons".to_string()));
        assert!(!accepted.contains(&"unsupported_feature".to_string()));
    }
}