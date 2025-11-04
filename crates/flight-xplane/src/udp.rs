// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! UDP client for X-Plane DataRef communication
//!
//! Implements the X-Plane UDP protocol for requesting and receiving DataRef values.
//! Supports both RREF (regular requests) and DREF (data requests) protocols.

use crate::dataref::{DataRef, DataRefValue};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};
use thiserror::Error;
use tokio::{
    net::UdpSocket,
    sync::oneshot,
    time::timeout,
};
use tracing::{debug, error, trace};

/// UDP client errors
#[derive(Error, Debug)]
pub enum UdpError {
    #[error("Network error: {0}")]
    Network(#[from] std::io::Error),
    #[error("Protocol error: {message}")]
    Protocol { message: String },
    #[error("Timeout waiting for response")]
    Timeout,
    #[error("Invalid response format")]
    InvalidResponse,
    #[error("DataRef not found: {name}")]
    DataRefNotFound { name: String },
    #[error("Connection not established")]
    NotConnected,
}

/// UDP configuration for X-Plane communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UdpConfig {
    /// X-Plane host address
    pub host: IpAddr,
    /// X-Plane UDP port (default 49000)
    pub port: u16,
    /// Local bind port (0 for automatic)
    pub local_port: u16,
    /// Request timeout duration
    pub timeout: Duration,
    /// Maximum concurrent requests
    pub max_concurrent_requests: usize,
    /// Enable data output subscriptions
    pub enable_data_output: bool,
}

impl Default for UdpConfig {
    fn default() -> Self {
        Self {
            host: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            port: 49000,
            local_port: 0,
            timeout: Duration::from_millis(100),
            max_concurrent_requests: 50,
            enable_data_output: true,
        }
    }
}

/// X-Plane UDP message types
#[derive(Debug, Clone)]
enum UdpMessage {
    /// Request DataRef value (RREF)
    RequestDataRef {
        id: u32,
        frequency: f32,
        dataref_name: String,
    },
    /// Set DataRef value (DREF)
    SetDataRef {
        dataref_name: String,
        value: DataRefValue,
    },
    /// Subscribe to data output (DATA)
    SubscribeData { indices: Vec<u32> },
}

/// Pending request tracking
#[derive(Debug)]
struct PendingRequest {
    dataref_name: String,
    sender: oneshot::Sender<Result<DataRefValue, UdpError>>,
    timestamp: Instant,
}

/// UDP client for X-Plane communication
#[derive(Clone)]
pub struct UdpClient {
    config: UdpConfig,
    socket: Arc<UdpSocket>,
    pending_requests: Arc<RwLock<HashMap<u32, PendingRequest>>>,
    next_request_id: Arc<RwLock<u32>>,
    dataref_cache: Arc<RwLock<HashMap<String, (DataRefValue, Instant)>>>,
}

impl UdpClient {
    /// Create a new UDP client
    pub fn new(config: UdpConfig) -> Result<Self, UdpError> {
        let local_addr = SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            config.local_port,
        );

        // Create socket - this is synchronous but should be fast
        let socket = std::net::UdpSocket::bind(local_addr)?;
        socket.set_nonblocking(true)?;
        
        let socket = Arc::new(UdpSocket::from_std(socket)?);

        let client = Self {
            config,
            socket,
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
            next_request_id: Arc::new(RwLock::new(1)),
            dataref_cache: Arc::new(RwLock::new(HashMap::new())),
        };

        // Start response handler
        client.start_response_handler();

        Ok(client)
    }

    /// Request a DataRef value
    pub async fn request_dataref(&self, dataref: &DataRef) -> Result<DataRefValue, UdpError> {
        // Check cache first
        if let Some((value, timestamp)) = self.get_cached_value(&dataref.name)
            && timestamp.elapsed() < Duration::from_millis(50) {
            trace!("Returning cached value for {}", dataref.name);
            return Ok(value);
        }

        let request_id = self.get_next_request_id();
        let (sender, receiver) = oneshot::channel();

        // Store pending request
        {
            let mut pending = self.pending_requests.write().unwrap();
            pending.insert(
                request_id,
                PendingRequest {
                    dataref_name: dataref.name.clone(),
                    sender,
                    timestamp: Instant::now(),
                },
            );
        }

        // Send RREF request
        let message = UdpMessage::RequestDataRef {
            id: request_id,
            frequency: 1.0, // One-time request
            dataref_name: dataref.name.clone(),
        };

        self.send_message(message).await?;

        // Wait for response
        match timeout(self.config.timeout, receiver).await {
            Ok(Ok(result)) => {
                // Cache successful result
                if let Ok(ref value) = result {
                    self.cache_value(dataref.name.clone(), value.clone());
                }
                result
            }
            Ok(Err(_)) => Err(UdpError::Protocol {
                message: "Response channel closed".to_string(),
            }),
            Err(_) => {
                // Clean up pending request on timeout
                self.pending_requests.write().unwrap().remove(&request_id);
                Err(UdpError::Timeout)
            }
        }
    }

    /// Set a DataRef value
    pub async fn set_dataref(&self, name: &str, value: DataRefValue) -> Result<(), UdpError> {
        let message = UdpMessage::SetDataRef {
            dataref_name: name.to_string(),
            value,
        };

        self.send_message(message).await?;
        Ok(())
    }

    /// Subscribe to data output
    pub async fn subscribe_data_output(&self, indices: Vec<u32>) -> Result<(), UdpError> {
        if !self.config.enable_data_output {
            return Err(UdpError::Protocol {
                message: "Data output not enabled".to_string(),
            });
        }

        let message = UdpMessage::SubscribeData { indices };
        self.send_message(message).await?;
        Ok(())
    }

    /// Send a UDP message to X-Plane
    async fn send_message(&self, message: UdpMessage) -> Result<(), UdpError> {
        let packet = self.encode_message(message)?;
        let target_addr = SocketAddr::new(self.config.host, self.config.port);

        trace!("Sending UDP packet to {}: {} bytes", target_addr, packet.len());
        
        self.socket.send_to(&packet, target_addr).await?;
        Ok(())
    }

    /// Encode a message into X-Plane UDP protocol format
    fn encode_message(&self, message: UdpMessage) -> Result<Vec<u8>, UdpError> {
        match message {
            UdpMessage::RequestDataRef {
                id,
                frequency,
                dataref_name,
            } => {
                let mut packet = Vec::new();
                packet.extend_from_slice(b"RREF");
                packet.push(0); // Null terminator for command
                packet.extend_from_slice(&frequency.to_le_bytes());
                packet.extend_from_slice(&id.to_le_bytes());
                packet.extend_from_slice(dataref_name.as_bytes());
                
                // Pad to 400 bytes (X-Plane requirement)
                packet.resize(413, 0);
                Ok(packet)
            }
            UdpMessage::SetDataRef { dataref_name, value } => {
                let mut packet = Vec::new();
                packet.extend_from_slice(b"DREF");
                packet.push(0); // Null terminator for command

                // Encode value based on type
                match value {
                    DataRefValue::Float(f) => {
                        packet.extend_from_slice(&f.to_le_bytes());
                    }
                    DataRefValue::Double(d) => {
                        packet.extend_from_slice(&(d as f32).to_le_bytes());
                    }
                    DataRefValue::Int(i) => {
                        packet.extend_from_slice(&(i as f32).to_le_bytes());
                    }
                    DataRefValue::IntArray(_) | DataRefValue::FloatArray(_) => {
                        return Err(UdpError::Protocol {
                            message: "Array DataRefs not supported via UDP".to_string(),
                        });
                    }
                }

                packet.extend_from_slice(dataref_name.as_bytes());
                packet.resize(509, 0); // Pad to required size
                Ok(packet)
            }
            UdpMessage::SubscribeData { indices } => {
                let mut packet = Vec::new();
                packet.extend_from_slice(b"DSEL");
                packet.push(0); // Null terminator for command

                for &index in &indices {
                    packet.extend_from_slice(&index.to_le_bytes());
                }

                Ok(packet)
            }
        }
    }

    /// Start the response handler task
    fn start_response_handler(&self) {
        let socket = self.socket.clone();
        let pending_requests = self.pending_requests.clone();
        let dataref_cache = self.dataref_cache.clone();

        tokio::spawn(async move {
            let mut buffer = vec![0u8; 1024];

            loop {
                match socket.recv_from(&mut buffer).await {
                    Ok((len, _addr)) => {
                        if let Err(e) = Self::handle_response(
                            &buffer[..len],
                            &pending_requests,
                            &dataref_cache,
                        ) {
                            debug!("Failed to handle UDP response: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("UDP receive error: {}", e);
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                }
            }
        });

        // Start cleanup task for expired requests
        let pending_requests_cleanup = self.pending_requests.clone();
        let timeout = self.config.timeout;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            
            loop {
                interval.tick().await;
                
                let mut expired_ids = Vec::new();
                {
                    let pending = pending_requests_cleanup.read().unwrap();
                    for (&id, request) in pending.iter() {
                        if request.timestamp.elapsed() > timeout * 2 {
                            expired_ids.push(id);
                        }
                    }
                }
                
                if !expired_ids.is_empty() {
                    let mut pending = pending_requests_cleanup.write().unwrap();
                    for id in expired_ids {
                        if let Some(request) = pending.remove(&id) {
                            let _ = request.sender.send(Err(UdpError::Timeout));
                            debug!("Cleaned up expired request for {}", request.dataref_name);
                        }
                    }
                }
            }
        });
    }

    /// Handle incoming UDP response
    fn handle_response(
        data: &[u8],
        pending_requests: &Arc<RwLock<HashMap<u32, PendingRequest>>>,
        dataref_cache: &Arc<RwLock<HashMap<String, (DataRefValue, Instant)>>>,
    ) -> Result<(), UdpError> {
        if data.len() < 5 {
            return Err(UdpError::InvalidResponse);
        }

        let command = &data[0..4];
        
        match command {
            b"RREF" => {
                // DataRef response
                if data.len() < 13 {
                    return Err(UdpError::InvalidResponse);
                }

                let id = u32::from_le_bytes([data[5], data[6], data[7], data[8]]);
                let value_bytes = [data[9], data[10], data[11], data[12]];
                let value = f32::from_le_bytes(value_bytes);

                trace!("Received RREF response: id={}, value={}", id, value);

                // Find and complete pending request
                let mut pending = pending_requests.write().unwrap();
                if let Some(request) = pending.remove(&id) {
                    let dataref_value = DataRefValue::Float(value);
                    
                    // Cache the value
                    {
                        let mut cache = dataref_cache.write().unwrap();
                        cache.insert(request.dataref_name.clone(), (dataref_value.clone(), Instant::now()));
                    }

                    let _ = request.sender.send(Ok(dataref_value));
                } else {
                    debug!("Received response for unknown request ID: {}", id);
                }
            }
            b"DATA" => {
                // Data output response
                Self::handle_data_output(data, dataref_cache)?;
            }
            _ => {
                debug!("Unknown UDP command: {:?}", std::str::from_utf8(command));
            }
        }

        Ok(())
    }

    /// Handle DATA output messages
    fn handle_data_output(
        data: &[u8],
        dataref_cache: &Arc<RwLock<HashMap<String, (DataRefValue, Instant)>>>,
    ) -> Result<(), UdpError> {
        // DATA messages contain multiple 36-byte records
        let mut offset = 5; // Skip "DATA" + null terminator
        
        while offset + 36 <= data.len() {
            let index = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);

            // Extract 8 float values
            let mut values = Vec::new();
            for i in 0..8 {
                let value_offset = offset + 4 + (i * 4);
                let value_bytes = [
                    data[value_offset],
                    data[value_offset + 1],
                    data[value_offset + 2],
                    data[value_offset + 3],
                ];
                values.push(f32::from_le_bytes(value_bytes));
            }

            // Map index to known DataRefs and cache values
            Self::cache_data_output_values(index, &values, dataref_cache);

            offset += 36;
        }

        Ok(())
    }

    /// Cache values from DATA output based on index
    fn cache_data_output_values(
        index: u32,
        values: &[f32],
        dataref_cache: &Arc<RwLock<HashMap<String, (DataRefValue, Instant)>>>,
    ) {
        let now = Instant::now();
        let mut cache = dataref_cache.write().unwrap();

        // Map common DATA indices to DataRef names
        match index {
            3 => {
                // Speeds
                if values.len() >= 8 {
                    cache.insert(
                        "sim/flightmodel/position/indicated_airspeed".to_string(),
                        (DataRefValue::Float(values[0]), now),
                    );
                    cache.insert(
                        "sim/flightmodel/position/true_airspeed".to_string(),
                        (DataRefValue::Float(values[1]), now),
                    );
                    cache.insert(
                        "sim/flightmodel/position/groundspeed".to_string(),
                        (DataRefValue::Float(values[2]), now),
                    );
                }
            }
            17 => {
                // Pitch, roll, headings
                if values.len() >= 8 {
                    cache.insert(
                        "sim/flightmodel/position/theta".to_string(),
                        (DataRefValue::Float(values[0]), now),
                    );
                    cache.insert(
                        "sim/flightmodel/position/phi".to_string(),
                        (DataRefValue::Float(values[1]), now),
                    );
                    cache.insert(
                        "sim/flightmodel/position/psi".to_string(),
                        (DataRefValue::Float(values[2]), now),
                    );
                }
            }
            20 => {
                // Position
                if values.len() >= 8 {
                    cache.insert(
                        "sim/flightmodel/position/latitude".to_string(),
                        (DataRefValue::Double(values[0] as f64), now),
                    );
                    cache.insert(
                        "sim/flightmodel/position/longitude".to_string(),
                        (DataRefValue::Double(values[1] as f64), now),
                    );
                    cache.insert(
                        "sim/flightmodel/position/elevation".to_string(),
                        (DataRefValue::Float(values[2]), now),
                    );
                }
            }
            _ => {
                // Unknown index, skip
                trace!("Unknown DATA index: {}", index);
            }
        }
    }

    /// Get next request ID
    fn get_next_request_id(&self) -> u32 {
        let mut id = self.next_request_id.write().unwrap();
        let current = *id;
        *id = id.wrapping_add(1);
        if *id == 0 {
            *id = 1; // Skip 0 as it might be reserved
        }
        current
    }

    /// Get cached DataRef value
    fn get_cached_value(&self, name: &str) -> Option<(DataRefValue, Instant)> {
        let cache = self.dataref_cache.read().unwrap();
        cache.get(name).cloned()
    }

    /// Cache a DataRef value
    fn cache_value(&self, name: String, value: DataRefValue) {
        let mut cache = self.dataref_cache.write().unwrap();
        cache.insert(name, (value, Instant::now()));
    }

    /// Get connection statistics
    pub fn get_stats(&self) -> UdpStats {
        let pending_count = self.pending_requests.read().unwrap().len();
        let cache_count = self.dataref_cache.read().unwrap().len();
        
        UdpStats {
            pending_requests: pending_count,
            cached_datarefs: cache_count,
            next_request_id: *self.next_request_id.read().unwrap(),
        }
    }
}

/// UDP client statistics
#[derive(Debug, Clone)]
pub struct UdpStats {
    pub pending_requests: usize,
    pub cached_datarefs: usize,
    pub next_request_id: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    

    #[test]
    fn test_udp_config_defaults() {
        let config = UdpConfig::default();
        assert_eq!(config.host, IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
        assert_eq!(config.port, 49000);
        assert_eq!(config.local_port, 0);
        assert!(config.enable_data_output);
    }

    #[tokio::test]
    async fn test_message_encoding() {
        let config = UdpConfig::default();
        let client = UdpClient::new(config).unwrap();

        // Test RREF encoding
        let message = UdpMessage::RequestDataRef {
            id: 123,
            frequency: 1.0,
            dataref_name: "sim/test/dataref".to_string(),
        };

        let packet = client.encode_message(message).unwrap();
        assert_eq!(&packet[0..4], b"RREF");
        assert_eq!(packet.len(), 413);
    }

    #[tokio::test]
    async fn test_request_id_generation() {
        let config = UdpConfig::default();
        let client = UdpClient::new(config).unwrap();

        let id1 = client.get_next_request_id();
        let id2 = client.get_next_request_id();
        
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }

    #[tokio::test]
    async fn test_cache_operations() {
        let config = UdpConfig::default();
        let client = UdpClient::new(config).unwrap();

        let name = "test/dataref".to_string();
        let value = DataRefValue::Float(42.0);

        // Cache value
        client.cache_value(name.clone(), value.clone());

        // Retrieve cached value
        let cached = client.get_cached_value(&name);
        assert!(cached.is_some());
        
        let (cached_value, _timestamp) = cached.unwrap();
        assert_eq!(cached_value, value);
    }

    #[tokio::test]
    async fn test_client_creation() {
        let config = UdpConfig::default();
        let client = UdpClient::new(config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_data_output_handling() {
        // Create a mock DATA packet
        let mut data = Vec::new();
        data.extend_from_slice(b"DATA");
        data.push(0); // Null terminator
        
        // Add a record for index 3 (speeds)
        data.extend_from_slice(&3u32.to_le_bytes()); // Index
        data.extend_from_slice(&150.0f32.to_le_bytes()); // IAS
        data.extend_from_slice(&155.0f32.to_le_bytes()); // TAS
        data.extend_from_slice(&145.0f32.to_le_bytes()); // GS
        data.extend_from_slice(&0.0f32.to_le_bytes());   // Unused
        data.extend_from_slice(&0.0f32.to_le_bytes());   // Unused
        data.extend_from_slice(&0.0f32.to_le_bytes());   // Unused
        data.extend_from_slice(&0.0f32.to_le_bytes());   // Unused
        data.extend_from_slice(&0.0f32.to_le_bytes());   // Unused

        let cache = Arc::new(RwLock::new(HashMap::new()));
        let result = UdpClient::handle_data_output(&data, &cache);
        assert!(result.is_ok());

        // Check that values were cached
        let cache_guard = cache.read().unwrap();
        assert!(cache_guard.contains_key("sim/flightmodel/position/indicated_airspeed"));
        
        if let Some((DataRefValue::Float(ias), _)) = cache_guard.get("sim/flightmodel/position/indicated_airspeed") {
            assert_eq!(*ias, 150.0);
        } else {
            panic!("Expected cached IAS value");
        }
    }
}