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
use tokio::{net::UdpSocket, sync::oneshot, time::timeout};
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
        let local_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), config.local_port);

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
            && timestamp.elapsed() < Duration::from_millis(50)
        {
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

        trace!(
            "Sending UDP packet to {}: {} bytes",
            target_addr,
            packet.len()
        );

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
            UdpMessage::SetDataRef {
                dataref_name,
                value,
            } => {
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
                        if let Err(e) =
                            Self::handle_response(&buffer[..len], &pending_requests, &dataref_cache)
                        {
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
                        cache.insert(
                            request.dataref_name.clone(),
                            (dataref_value.clone(), Instant::now()),
                        );
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
    ///
    /// Parses X-Plane DATA packets which contain 36-byte records.
    /// Each record has:
    /// - 4 bytes: data group index (i32)
    /// - 32 bytes: 8 float values (8 × 4 bytes)
    ///
    /// Requirements: XPLANE-INT-01.2, XPLANE-INT-01.3, XPLANE-INT-01.6
    fn handle_data_output(
        data: &[u8],
        dataref_cache: &Arc<RwLock<HashMap<String, (DataRefValue, Instant)>>>,
    ) -> Result<(), UdpError> {
        // Verify minimum packet size (header + at least one record)
        if data.len() < 5 {
            return Err(UdpError::InvalidResponse);
        }

        // Verify DATA header
        if &data[0..4] != b"DATA" {
            return Err(UdpError::Protocol {
                message: "Invalid DATA packet header".to_string(),
            });
        }

        // DATA messages contain multiple 36-byte records
        let mut offset = 5; // Skip "DATA" + null terminator

        while offset + 36 <= data.len() {
            // Parse data group index (4 bytes, little-endian i32)
            let index = i32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);

            // Extract 8 float values (32 bytes total)
            let mut values = [0.0f32; 8];
            for i in 0..8 {
                let value_offset = offset + 4 + (i * 4);
                let value_bytes = [
                    data[value_offset],
                    data[value_offset + 1],
                    data[value_offset + 2],
                    data[value_offset + 3],
                ];
                values[i] = f32::from_le_bytes(value_bytes);
            }

            // Map index to known DataRefs and cache values
            // Gracefully handle missing/unknown data groups (XPLANE-INT-01.6)
            Self::cache_data_output_values(index, &values, dataref_cache);

            offset += 36;
        }

        // Check for incomplete record at end (malformed packet)
        if offset < data.len() && data.len() - offset < 36 {
            trace!(
                "Incomplete DATA record at end of packet: {} bytes remaining",
                data.len() - offset
            );
        }

        Ok(())
    }

    /// Cache values from DATA output based on index
    ///
    /// Maps X-Plane DATA output indices to DataRef names and caches the values.
    /// Supports the required data groups for Flight Hub v1:
    /// - Group 3: Speeds (IAS, TAS, GS)
    /// - Group 4: Mach, VVI, G-load
    /// - Group 16: Angular velocities (P, Q, R)
    /// - Group 17: Pitch, roll, headings
    /// - Group 18: Alpha, beta
    /// - Group 21: Body velocities
    ///
    /// Gracefully handles missing or unknown data groups (XPLANE-INT-01.6)
    ///
    /// Requirements: XPLANE-INT-01.3
    fn cache_data_output_values(
        index: i32,
        values: &[f32; 8],
        dataref_cache: &Arc<RwLock<HashMap<String, (DataRefValue, Instant)>>>,
    ) {
        let now = Instant::now();
        let mut cache = dataref_cache.write().unwrap();

        // Map DATA indices to DataRef names per X-Plane documentation
        // Requirements: XPLANE-INT-01.3
        match index {
            3 => {
                // Group 3: Speeds (knots)
                // [0] = IAS, [1] = TAS, [2] = Ground speed, [3] = unused, [4] = unused, [5] = unused, [6] = unused, [7] = unused
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
                trace!(
                    "Cached speeds: IAS={}, TAS={}, GS={}",
                    values[0], values[1], values[2]
                );
            }
            4 => {
                // Group 4: Mach, VVI, G-load
                // [0] = Mach, [1] = VVI (ft/min), [2] = unused, [3] = unused, [4] = G-normal, [5] = G-axial, [6] = G-side, [7] = unused
                cache.insert(
                    "sim/flightmodel/misc/machno".to_string(),
                    (DataRefValue::Float(values[0]), now),
                );
                cache.insert(
                    "sim/flightmodel/position/vh_ind".to_string(),
                    (DataRefValue::Float(values[1]), now),
                );
                cache.insert(
                    "sim/flightmodel/forces/g_nrml".to_string(),
                    (DataRefValue::Float(values[4]), now),
                );
                cache.insert(
                    "sim/flightmodel/forces/g_axil".to_string(),
                    (DataRefValue::Float(values[5]), now),
                );
                cache.insert(
                    "sim/flightmodel/forces/g_side".to_string(),
                    (DataRefValue::Float(values[6]), now),
                );
                trace!(
                    "Cached Mach/VVI/G: Mach={}, VVI={}, Gnrml={}",
                    values[0], values[1], values[4]
                );
            }
            16 => {
                // Group 16: Angular velocities (deg/s)
                // [0] = P, [1] = Q, [2] = R, [3-7] = unused
                cache.insert(
                    "sim/flightmodel/position/P".to_string(),
                    (DataRefValue::Float(values[0]), now),
                );
                cache.insert(
                    "sim/flightmodel/position/Q".to_string(),
                    (DataRefValue::Float(values[1]), now),
                );
                cache.insert(
                    "sim/flightmodel/position/R".to_string(),
                    (DataRefValue::Float(values[2]), now),
                );
                trace!(
                    "Cached angular rates: P={}, Q={}, R={}",
                    values[0], values[1], values[2]
                );
            }
            17 => {
                // Group 17: Pitch, roll, headings (degrees)
                // [0] = pitch, [1] = roll, [2] = heading true, [3] = heading mag, [4-7] = unused
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
                cache.insert(
                    "sim/flightmodel/position/magpsi".to_string(),
                    (DataRefValue::Float(values[3]), now),
                );
                trace!(
                    "Cached attitude: pitch={}, roll={}, heading={}",
                    values[0], values[1], values[2]
                );
            }
            18 => {
                // Group 18: Alpha, beta, etc. (degrees)
                // [0] = alpha (AOA), [1] = beta (sideslip), [2] = hpath, [3] = vpath, [4-7] = unused
                cache.insert(
                    "sim/flightmodel/position/alpha".to_string(),
                    (DataRefValue::Float(values[0]), now),
                );
                cache.insert(
                    "sim/flightmodel/position/beta".to_string(),
                    (DataRefValue::Float(values[1]), now),
                );
                cache.insert(
                    "sim/flightmodel/position/hpath".to_string(),
                    (DataRefValue::Float(values[2]), now),
                );
                cache.insert(
                    "sim/flightmodel/position/vpath".to_string(),
                    (DataRefValue::Float(values[3]), now),
                );
                trace!("Cached aero: alpha={}, beta={}", values[0], values[1]);
            }
            21 => {
                // Group 21: Body velocities (m/s)
                // [0] = vx, [1] = vy, [2] = vz, [3-7] = unused
                cache.insert(
                    "sim/flightmodel/position/local_vx".to_string(),
                    (DataRefValue::Float(values[0]), now),
                );
                cache.insert(
                    "sim/flightmodel/position/local_vy".to_string(),
                    (DataRefValue::Float(values[1]), now),
                );
                cache.insert(
                    "sim/flightmodel/position/local_vz".to_string(),
                    (DataRefValue::Float(values[2]), now),
                );
                trace!(
                    "Cached body velocities: vx={}, vy={}, vz={}",
                    values[0], values[1], values[2]
                );
            }
            20 => {
                // Group 20: Position (lat/lon/alt) - bonus, not required for v1 but useful
                // [0] = latitude, [1] = longitude, [2] = altitude MSL (ft), [3-7] = unused
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
                trace!(
                    "Cached position: lat={}, lon={}, alt={}",
                    values[0], values[1], values[2]
                );
            }
            _ => {
                // Unknown or unsupported index - gracefully ignore (XPLANE-INT-01.6)
                trace!("Received DATA for unsupported index: {}", index);
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

/// Parse a raw X-Plane UDP response packet.
///
/// Exposed for fuzz testing: accepts arbitrary bytes and returns any parse error.
/// Never panics — the caller should treat all `Err` variants as expected.
pub fn parse_udp_packet(data: &[u8]) -> Result<(), UdpError> {
    let pending = Arc::new(RwLock::new(HashMap::new()));
    let cache = Arc::new(RwLock::new(HashMap::new()));
    UdpClient::handle_response(data, &pending, &cache)
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

    /// Test DATA packet parsing with valid data
    /// Requirements: XPLANE-INT-01.2, SIM-TEST-01.3
    #[test]
    fn test_data_output_handling() {
        // Create a mock DATA packet
        let mut data = Vec::new();
        data.extend_from_slice(b"DATA");
        data.push(0); // Null terminator

        // Add a record for index 3 (speeds)
        data.extend_from_slice(&3i32.to_le_bytes()); // Index (changed to i32)
        data.extend_from_slice(&150.0f32.to_le_bytes()); // IAS
        data.extend_from_slice(&155.0f32.to_le_bytes()); // TAS
        data.extend_from_slice(&145.0f32.to_le_bytes()); // GS
        data.extend_from_slice(&0.0f32.to_le_bytes()); // Unused
        data.extend_from_slice(&0.0f32.to_le_bytes()); // Unused
        data.extend_from_slice(&0.0f32.to_le_bytes()); // Unused
        data.extend_from_slice(&0.0f32.to_le_bytes()); // Unused
        data.extend_from_slice(&0.0f32.to_le_bytes()); // Unused

        let cache = Arc::new(RwLock::new(HashMap::new()));
        let result = UdpClient::handle_data_output(&data, &cache);
        assert!(result.is_ok());

        // Check that values were cached
        let cache_guard = cache.read().unwrap();
        assert!(cache_guard.contains_key("sim/flightmodel/position/indicated_airspeed"));

        if let Some((DataRefValue::Float(ias), _)) =
            cache_guard.get("sim/flightmodel/position/indicated_airspeed")
        {
            assert_eq!(*ias, 150.0);
        } else {
            panic!("Expected cached IAS value");
        }
    }

    /// Test DATA packet parsing with multiple data groups
    /// Requirements: XPLANE-INT-01.2, XPLANE-INT-01.3
    #[test]
    fn test_data_output_multiple_groups() {
        let mut data = Vec::new();
        data.extend_from_slice(b"DATA");
        data.push(0);

        // Add group 3 (speeds)
        data.extend_from_slice(&3i32.to_le_bytes());
        data.extend_from_slice(&150.0f32.to_le_bytes()); // IAS
        data.extend_from_slice(&155.0f32.to_le_bytes()); // TAS
        data.extend_from_slice(&145.0f32.to_le_bytes()); // GS
        for _ in 3..8 {
            data.extend_from_slice(&0.0f32.to_le_bytes());
        }

        // Add group 17 (attitude)
        data.extend_from_slice(&17i32.to_le_bytes());
        data.extend_from_slice(&5.0f32.to_le_bytes()); // pitch
        data.extend_from_slice(&10.0f32.to_le_bytes()); // roll
        data.extend_from_slice(&270.0f32.to_le_bytes()); // heading true
        data.extend_from_slice(&275.0f32.to_le_bytes()); // heading mag
        for _ in 4..8 {
            data.extend_from_slice(&0.0f32.to_le_bytes());
        }

        // Add group 4 (Mach/VVI/G)
        data.extend_from_slice(&4i32.to_le_bytes());
        data.extend_from_slice(&0.45f32.to_le_bytes()); // Mach
        data.extend_from_slice(&500.0f32.to_le_bytes()); // VVI
        data.extend_from_slice(&0.0f32.to_le_bytes()); // unused
        data.extend_from_slice(&0.0f32.to_le_bytes()); // unused
        data.extend_from_slice(&1.2f32.to_le_bytes()); // G-normal
        data.extend_from_slice(&0.1f32.to_le_bytes()); // G-axial
        data.extend_from_slice(&0.05f32.to_le_bytes()); // G-side
        data.extend_from_slice(&0.0f32.to_le_bytes()); // unused

        let cache = Arc::new(RwLock::new(HashMap::new()));
        let result = UdpClient::handle_data_output(&data, &cache);
        assert!(result.is_ok());

        let cache_guard = cache.read().unwrap();

        // Verify speeds
        assert!(cache_guard.contains_key("sim/flightmodel/position/indicated_airspeed"));
        assert!(cache_guard.contains_key("sim/flightmodel/position/true_airspeed"));
        assert!(cache_guard.contains_key("sim/flightmodel/position/groundspeed"));

        // Verify attitude
        assert!(cache_guard.contains_key("sim/flightmodel/position/theta"));
        assert!(cache_guard.contains_key("sim/flightmodel/position/phi"));
        assert!(cache_guard.contains_key("sim/flightmodel/position/psi"));

        // Verify Mach/VVI/G
        assert!(cache_guard.contains_key("sim/flightmodel/misc/machno"));
        assert!(cache_guard.contains_key("sim/flightmodel/position/vh_ind"));
        assert!(cache_guard.contains_key("sim/flightmodel/forces/g_nrml"));

        // Verify values
        if let Some((DataRefValue::Float(pitch), _)) =
            cache_guard.get("sim/flightmodel/position/theta")
        {
            assert_eq!(*pitch, 5.0);
        }
        if let Some((DataRefValue::Float(g_nrml), _)) =
            cache_guard.get("sim/flightmodel/forces/g_nrml")
        {
            assert_eq!(*g_nrml, 1.2);
        }
    }

    /// Test DATA packet parsing with all required groups (3, 4, 16, 17, 18, 21)
    /// Requirements: XPLANE-INT-01.3
    #[test]
    fn test_data_output_all_required_groups() {
        let mut data = Vec::new();
        data.extend_from_slice(b"DATA");
        data.push(0);

        // Helper to add a data group
        let mut add_group = |index: i32, values: [f32; 8]| {
            data.extend_from_slice(&index.to_le_bytes());
            for value in values {
                data.extend_from_slice(&value.to_le_bytes());
            }
        };

        // Group 3: Speeds
        add_group(3, [150.0, 155.0, 145.0, 0.0, 0.0, 0.0, 0.0, 0.0]);

        // Group 4: Mach/VVI/G
        add_group(4, [0.45, 500.0, 0.0, 0.0, 1.2, 0.1, 0.05, 0.0]);

        // Group 16: Angular velocities
        add_group(16, [2.5, -1.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0]);

        // Group 17: Attitude
        add_group(17, [5.0, 10.0, 270.0, 275.0, 0.0, 0.0, 0.0, 0.0]);

        // Group 18: Alpha/Beta
        add_group(18, [3.5, -0.5, 270.0, 0.0, 0.0, 0.0, 0.0, 0.0]);

        // Group 21: Body velocities
        add_group(21, [75.0, 2.0, -1.5, 0.0, 0.0, 0.0, 0.0, 0.0]);

        let cache = Arc::new(RwLock::new(HashMap::new()));
        let result = UdpClient::handle_data_output(&data, &cache);
        assert!(result.is_ok());

        let cache_guard = cache.read().unwrap();

        // Verify all required groups are cached
        // Group 3
        assert!(cache_guard.contains_key("sim/flightmodel/position/indicated_airspeed"));
        // Group 4
        assert!(cache_guard.contains_key("sim/flightmodel/misc/machno"));
        assert!(cache_guard.contains_key("sim/flightmodel/forces/g_nrml"));
        // Group 16
        assert!(cache_guard.contains_key("sim/flightmodel/position/P"));
        assert!(cache_guard.contains_key("sim/flightmodel/position/Q"));
        assert!(cache_guard.contains_key("sim/flightmodel/position/R"));
        // Group 17
        assert!(cache_guard.contains_key("sim/flightmodel/position/theta"));
        // Group 18
        assert!(cache_guard.contains_key("sim/flightmodel/position/alpha"));
        assert!(cache_guard.contains_key("sim/flightmodel/position/beta"));
        // Group 21
        assert!(cache_guard.contains_key("sim/flightmodel/position/local_vx"));
        assert!(cache_guard.contains_key("sim/flightmodel/position/local_vy"));
        assert!(cache_guard.contains_key("sim/flightmodel/position/local_vz"));
    }

    /// Test handling of missing data groups (graceful degradation)
    /// Requirements: XPLANE-INT-01.6
    #[test]
    fn test_data_output_missing_groups() {
        let mut data = Vec::new();
        data.extend_from_slice(b"DATA");
        data.push(0);

        // Only add group 3, omit other groups
        data.extend_from_slice(&3i32.to_le_bytes());
        data.extend_from_slice(&150.0f32.to_le_bytes());
        for _ in 1..8 {
            data.extend_from_slice(&0.0f32.to_le_bytes());
        }

        let cache = Arc::new(RwLock::new(HashMap::new()));
        let result = UdpClient::handle_data_output(&data, &cache);

        // Should succeed even with missing groups
        assert!(result.is_ok());

        let cache_guard = cache.read().unwrap();

        // Group 3 should be cached
        assert!(cache_guard.contains_key("sim/flightmodel/position/indicated_airspeed"));

        // Other groups should not be present
        assert!(!cache_guard.contains_key("sim/flightmodel/position/theta"));
        assert!(!cache_guard.contains_key("sim/flightmodel/position/alpha"));
    }

    /// Test handling of unknown data group indices
    /// Requirements: XPLANE-INT-01.6
    #[test]
    fn test_data_output_unknown_group() {
        let mut data = Vec::new();
        data.extend_from_slice(b"DATA");
        data.push(0);

        // Add an unknown group index (999)
        data.extend_from_slice(&999i32.to_le_bytes());
        for _ in 0..8 {
            data.extend_from_slice(&42.0f32.to_le_bytes());
        }

        // Add a known group (3)
        data.extend_from_slice(&3i32.to_le_bytes());
        data.extend_from_slice(&150.0f32.to_le_bytes());
        for _ in 1..8 {
            data.extend_from_slice(&0.0f32.to_le_bytes());
        }

        let cache = Arc::new(RwLock::new(HashMap::new()));
        let result = UdpClient::handle_data_output(&data, &cache);

        // Should succeed and skip unknown group
        assert!(result.is_ok());

        let cache_guard = cache.read().unwrap();

        // Known group should be cached
        assert!(cache_guard.contains_key("sim/flightmodel/position/indicated_airspeed"));
    }

    /// Test malformed packet handling - packet too short
    /// Requirements: XPLANE-INT-01.6
    #[test]
    fn test_data_output_malformed_too_short() {
        let data = vec![b'D', b'A', b'T', b'A']; // Missing null terminator and data

        let cache = Arc::new(RwLock::new(HashMap::new()));
        let result = UdpClient::handle_data_output(&data, &cache);

        // Should return error for malformed packet
        assert!(result.is_err());
    }

    /// Test malformed packet handling - invalid header
    /// Requirements: XPLANE-INT-01.6
    #[test]
    fn test_data_output_malformed_invalid_header() {
        let mut data = Vec::new();
        data.extend_from_slice(b"XXXX"); // Invalid header
        data.push(0);

        // Add valid data
        data.extend_from_slice(&3i32.to_le_bytes());
        for _ in 0..8 {
            data.extend_from_slice(&0.0f32.to_le_bytes());
        }

        let cache = Arc::new(RwLock::new(HashMap::new()));
        let result = UdpClient::handle_data_output(&data, &cache);

        // Should return error for invalid header
        assert!(result.is_err());
    }

    /// Test malformed packet handling - incomplete record
    /// Requirements: XPLANE-INT-01.6
    #[test]
    fn test_data_output_malformed_incomplete_record() {
        let mut data = Vec::new();
        data.extend_from_slice(b"DATA");
        data.push(0);

        // Add complete record
        data.extend_from_slice(&3i32.to_le_bytes());
        for _ in 0..8 {
            data.extend_from_slice(&150.0f32.to_le_bytes());
        }

        // Add incomplete record (only 20 bytes instead of 36)
        data.extend_from_slice(&17i32.to_le_bytes());
        for _ in 0..4 {
            data.extend_from_slice(&5.0f32.to_le_bytes());
        }
        // Missing 4 more floats

        let cache = Arc::new(RwLock::new(HashMap::new()));
        let result = UdpClient::handle_data_output(&data, &cache);

        // Should succeed and process complete record, ignore incomplete
        assert!(result.is_ok());

        let cache_guard = cache.read().unwrap();

        // First record should be cached
        assert!(cache_guard.contains_key("sim/flightmodel/position/indicated_airspeed"));

        // Incomplete record should not be cached
        assert!(!cache_guard.contains_key("sim/flightmodel/position/theta"));
    }

    /// Test DATA packet with exact 36-byte record boundary
    /// Requirements: XPLANE-INT-01.2
    #[test]
    fn test_data_output_exact_record_boundary() {
        let mut data = Vec::new();
        data.extend_from_slice(b"DATA");
        data.push(0);

        // Add exactly one 36-byte record
        data.extend_from_slice(&3i32.to_le_bytes()); // 4 bytes
        for _ in 0..8 {
            data.extend_from_slice(&150.0f32.to_le_bytes()); // 32 bytes
        }
        // Total: 5 (header) + 36 (record) = 41 bytes

        assert_eq!(data.len(), 41);

        let cache = Arc::new(RwLock::new(HashMap::new()));
        let result = UdpClient::handle_data_output(&data, &cache);

        assert!(result.is_ok());

        let cache_guard = cache.read().unwrap();
        assert!(cache_guard.contains_key("sim/flightmodel/position/indicated_airspeed"));
    }

    /// Test empty DATA packet (header only)
    /// Requirements: XPLANE-INT-01.6
    #[test]
    fn test_data_output_empty_packet() {
        let mut data = Vec::new();
        data.extend_from_slice(b"DATA");
        data.push(0);

        let cache = Arc::new(RwLock::new(HashMap::new()));
        let result = UdpClient::handle_data_output(&data, &cache);

        // Should succeed with no records
        assert!(result.is_ok());

        let cache_guard = cache.read().unwrap();
        assert!(cache_guard.is_empty());
    }

    // ---------- handle_response edge-case tests ----------

    /// Empty byte slice (0 bytes) must return an error, not panic.
    #[test]
    fn test_handle_response_empty_slice() {
        let data: &[u8] = &[];
        let pending = Arc::new(RwLock::new(HashMap::new()));
        let cache = Arc::new(RwLock::new(HashMap::new()));
        let result = UdpClient::handle_response(data, &pending, &cache);
        assert!(result.is_err(), "expected error for empty slice");
    }

    /// A 4-byte slice (below the minimum 5-byte threshold) must return an error.
    #[test]
    fn test_handle_response_too_short() {
        let data: &[u8] = b"RREF"; // 4 bytes, missing null terminator byte
        let pending = Arc::new(RwLock::new(HashMap::new()));
        let cache = Arc::new(RwLock::new(HashMap::new()));
        let result = UdpClient::handle_response(data, &pending, &cache);
        assert!(result.is_err(), "expected error for 4-byte slice");
    }

    /// An unknown 4-byte command should be silently ignored (returns Ok).
    #[test]
    fn test_handle_response_unknown_command() {
        let mut data = Vec::new();
        data.extend_from_slice(b"XXXX"); // Unknown command
        data.push(0);
        // Pad with dummy bytes so the packet passes the length check
        data.extend_from_slice(&[0u8; 10]);
        let pending = Arc::new(RwLock::new(HashMap::new()));
        let cache = Arc::new(RwLock::new(HashMap::new()));
        let result = UdpClient::handle_response(&data, &pending, &cache);
        assert!(result.is_ok(), "unknown command should be silently ignored");
    }

    /// A valid DATA packet through handle_response caches values correctly.
    #[test]
    fn test_handle_response_valid_data_packet() {
        let mut data = Vec::new();
        data.extend_from_slice(b"DATA");
        data.push(0);
        // Group 3: speeds
        data.extend_from_slice(&3i32.to_le_bytes());
        data.extend_from_slice(&120.0f32.to_le_bytes()); // IAS
        for _ in 1..8 {
            data.extend_from_slice(&0.0f32.to_le_bytes());
        }

        let pending = Arc::new(RwLock::new(HashMap::new()));
        let cache = Arc::new(RwLock::new(HashMap::new()));
        let result = UdpClient::handle_response(&data, &pending, &cache);
        assert!(result.is_ok(), "valid DATA packet should be handled without error");

        let cache_guard = cache.read().unwrap();
        assert!(
            cache_guard.contains_key("sim/flightmodel/position/indicated_airspeed"),
            "IAS should be cached after DATA packet"
        );
    }

    /// An RREF packet that is too short (< 13 bytes) must return an error, not panic.
    #[test]
    fn test_handle_response_rref_too_short() {
        let mut data = Vec::new();
        data.extend_from_slice(b"RREF");
        data.push(0); // 5 bytes total — valid header length but RREF needs >= 13
        let pending = Arc::new(RwLock::new(HashMap::new()));
        let cache = Arc::new(RwLock::new(HashMap::new()));
        let result = UdpClient::handle_response(&data, &pending, &cache);
        assert!(result.is_err(), "RREF packet shorter than 13 bytes should fail");
    }
}
