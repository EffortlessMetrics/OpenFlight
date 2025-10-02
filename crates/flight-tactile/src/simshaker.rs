//! SimShaker-class application bridge for tactile output

use crate::channel::ChannelOutput;
use serde::{Deserialize, Serialize};
use std::net::{UdpSocket, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::RwLock;
use thiserror::Error;
use tracing::{debug, warn, error};

/// Configuration for SimShaker bridge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimShakerConfig {
    /// Target IP address for SimShaker application
    pub target_address: String,
    /// Target port for SimShaker application
    pub target_port: u16,
    /// Local bind port (0 for automatic)
    pub local_port: u16,
    /// Update rate in Hz (10-60 Hz recommended)
    pub update_rate_hz: f32,
    /// Connection timeout in milliseconds
    pub timeout_ms: u64,
    /// Maximum packet size
    pub max_packet_size: usize,
    /// Channel count (typically 8 for SimShaker)
    pub channel_count: u8,
}

impl Default for SimShakerConfig {
    fn default() -> Self {
        Self {
            target_address: "127.0.0.1".to_string(),
            target_port: 4123, // Common SimShaker port
            local_port: 0, // Auto-assign
            update_rate_hz: 30.0, // 30 Hz update rate
            timeout_ms: 1000,
            max_packet_size: 256,
            channel_count: 8,
        }
    }
}

/// Status of SimShaker bridge connection
#[derive(Debug, Clone, PartialEq)]
pub enum SimShakerStatus {
    /// Not connected
    Disconnected,
    /// Connecting to target
    Connecting,
    /// Connected and active
    Connected,
    /// Connection failed
    Failed(String),
}

/// Errors that can occur in SimShaker bridge
#[derive(Debug, Error)]
pub enum SimShakerError {
    #[error("Network error: {0}")]
    Network(#[from] std::io::Error),
    #[error("Configuration error: {0}")]
    Configuration(String),
    #[error("Protocol error: {0}")]
    Protocol(String),
    #[error("Timeout error: {0}")]
    Timeout(String),
}

/// Statistics for SimShaker bridge performance
#[derive(Debug, Clone)]
pub struct SimShakerStats {
    pub packets_sent: u64,
    pub packets_failed: u64,
    pub bytes_sent: u64,
    pub last_send_time: Option<Instant>,
    pub connection_uptime: Duration,
    pub average_latency_us: f32,
    pub status: SimShakerStatus,
}

impl Default for SimShakerStats {
    fn default() -> Self {
        Self {
            packets_sent: 0,
            packets_failed: 0,
            bytes_sent: 0,
            last_send_time: None,
            connection_uptime: Duration::ZERO,
            average_latency_us: 0.0,
            status: SimShakerStatus::Disconnected,
        }
    }
}

/// SimShaker protocol packet structure
#[derive(Debug, Clone)]
struct SimShakerPacket {
    /// Packet header (magic bytes)
    header: [u8; 4],
    /// Channel data (8 channels, 0-255 intensity each)
    channels: [u8; 8],
    /// Packet sequence number
    sequence: u32,
    /// Checksum
    checksum: u16,
}

impl SimShakerPacket {
    /// Create a new packet with channel data
    fn new(channel_data: &[u8; 8], sequence: u32) -> Self {
        let mut packet = Self {
            header: [0x53, 0x48, 0x4B, 0x52], // "SHKR" magic
            channels: *channel_data,
            sequence,
            checksum: 0,
        };
        
        packet.checksum = packet.calculate_checksum();
        packet
    }

    /// Calculate packet checksum
    fn calculate_checksum(&self) -> u16 {
        let mut sum: u32 = 0;
        
        // Include header
        for &byte in &self.header {
            sum = sum.wrapping_add(byte as u32);
        }
        
        // Include channels
        for &byte in &self.channels {
            sum = sum.wrapping_add(byte as u32);
        }
        
        // Include sequence
        sum = sum.wrapping_add(self.sequence);
        
        (sum & 0xFFFF) as u16
    }

    /// Serialize packet to bytes
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(18);
        
        bytes.extend_from_slice(&self.header);
        bytes.extend_from_slice(&self.channels);
        bytes.extend_from_slice(&self.sequence.to_le_bytes());
        bytes.extend_from_slice(&self.checksum.to_le_bytes());
        
        bytes
    }
}

/// Bridge to SimShaker-class applications
pub struct SimShakerBridge {
    config: SimShakerConfig,
    socket: Option<UdpSocket>,
    target_addr: Option<SocketAddr>,
    stats: Arc<RwLock<SimShakerStats>>,
    sequence_number: u32,
    last_update: Instant,
    update_interval: Duration,
    channel_values: [u8; 8],
}

impl SimShakerBridge {
    /// Create a new SimShaker bridge
    pub fn new(config: SimShakerConfig) -> Result<Self, SimShakerError> {
        // Validate configuration
        if config.update_rate_hz <= 0.0 || config.update_rate_hz > 1000.0 {
            return Err(SimShakerError::Configuration(
                "Update rate must be between 0 and 1000 Hz".to_string()
            ));
        }

        if config.channel_count == 0 || config.channel_count > 8 {
            return Err(SimShakerError::Configuration(
                "Channel count must be between 1 and 8".to_string()
            ));
        }

        let update_interval = Duration::from_secs_f32(1.0 / config.update_rate_hz);

        Ok(Self {
            config,
            socket: None,
            target_addr: None,
            stats: Arc::new(RwLock::new(SimShakerStats::default())),
            sequence_number: 0,
            last_update: Instant::now(),
            update_interval,
            channel_values: [0; 8],
        })
    }

    /// Start the SimShaker bridge
    pub fn start(&mut self) -> Result<(), SimShakerError> {
        debug!("Starting SimShaker bridge");
        
        // Update status
        self.stats.write().status = SimShakerStatus::Connecting;

        // Create UDP socket
        let bind_addr = format!("0.0.0.0:{}", self.config.local_port);
        let socket = UdpSocket::bind(&bind_addr)?;
        
        // Set socket timeout
        socket.set_write_timeout(Some(Duration::from_millis(self.config.timeout_ms)))?;
        socket.set_read_timeout(Some(Duration::from_millis(100)))?; // Short read timeout for non-blocking

        // Resolve target address
        let target_addr = format!("{}:{}", self.config.target_address, self.config.target_port)
            .parse::<SocketAddr>()
            .map_err(|e| SimShakerError::Configuration(format!("Invalid target address: {}", e)))?;

        self.socket = Some(socket);
        self.target_addr = Some(target_addr);

        // Send initial handshake packet
        self.send_handshake()?;

        // Update status
        let mut stats = self.stats.write();
        stats.status = SimShakerStatus::Connected;
        stats.connection_uptime = Duration::ZERO;

        debug!("SimShaker bridge started successfully");
        Ok(())
    }

    /// Stop the SimShaker bridge
    pub fn stop(&mut self) {
        debug!("Stopping SimShaker bridge");
        
        // Send zero packet to clear effects
        self.channel_values = [0; 8];
        let _ = self.send_packet();

        self.socket = None;
        self.target_addr = None;
        self.stats.write().status = SimShakerStatus::Disconnected;
    }

    /// Update bridge with channel outputs
    pub fn update(&mut self, outputs: &[ChannelOutput]) -> Result<(), SimShakerError> {
        let now = Instant::now();
        
        // Check if it's time to send an update
        if now.duration_since(self.last_update) < self.update_interval {
            return Ok(());
        }

        // Convert channel outputs to byte values
        self.update_channel_values(outputs);

        // Send packet
        self.send_packet()?;

        self.last_update = now;
        Ok(())
    }

    /// Update internal channel values from outputs
    fn update_channel_values(&mut self, outputs: &[ChannelOutput]) {
        // Clear all channels first
        self.channel_values = [0; 8];

        // Update channels from outputs
        for output in outputs {
            let channel_index = output.channel_id.value() as usize;
            if channel_index < self.config.channel_count as usize {
                // Convert 0.0-1.0 intensity to 0-255 byte value
                let intensity_byte = (output.intensity.value() * 255.0).round() as u8;
                self.channel_values[channel_index] = intensity_byte;
            }
        }
    }

    /// Send handshake packet to establish connection
    fn send_handshake(&mut self) -> Result<(), SimShakerError> {
        debug!("Sending SimShaker handshake");
        
        // Send a zero packet as handshake
        self.channel_values = [0; 8];
        self.send_packet()
    }

    /// Send current channel data packet
    fn send_packet(&mut self) -> Result<(), SimShakerError> {
        let socket = self.socket.as_ref()
            .ok_or_else(|| SimShakerError::Protocol("Socket not initialized".to_string()))?;
        
        let target_addr = self.target_addr
            .ok_or_else(|| SimShakerError::Protocol("Target address not set".to_string()))?;

        // Create packet
        let packet = SimShakerPacket::new(&self.channel_values, self.sequence_number);
        let packet_bytes = packet.to_bytes();

        // Send packet
        let send_start = Instant::now();
        match socket.send_to(&packet_bytes, target_addr) {
            Ok(bytes_sent) => {
                let send_duration = send_start.elapsed();
                
                // Update statistics
                let mut stats = self.stats.write();
                stats.packets_sent += 1;
                stats.bytes_sent += bytes_sent as u64;
                stats.last_send_time = Some(send_start);
                
                // Update average latency (simple moving average)
                let latency_us = send_duration.as_micros() as f32;
                if stats.average_latency_us == 0.0 {
                    stats.average_latency_us = latency_us;
                } else {
                    stats.average_latency_us = stats.average_latency_us * 0.9 + latency_us * 0.1;
                }

                self.sequence_number = self.sequence_number.wrapping_add(1);
                
                debug!("Sent SimShaker packet: {} bytes, seq {}", bytes_sent, packet.sequence);
                Ok(())
            }
            Err(e) => {
                warn!("Failed to send SimShaker packet: {}", e);
                
                // Update error statistics
                self.stats.write().packets_failed += 1;
                
                // Update status on repeated failures
                if self.stats.read().packets_failed > 10 {
                    self.stats.write().status = SimShakerStatus::Failed(e.to_string());
                }
                
                Err(SimShakerError::Network(e))
            }
        }
    }

    /// Get bridge statistics
    pub fn get_stats(&self) -> SimShakerStats {
        self.stats.read().clone()
    }

    /// Get current status
    pub fn get_status(&self) -> SimShakerStatus {
        self.stats.read().status.clone()
    }

    /// Test connection with a test pattern
    pub fn test_connection(&mut self) -> Result<(), SimShakerError> {
        debug!("Testing SimShaker connection");
        
        // Send test pattern: all channels at 50%
        self.channel_values = [128; 8];
        self.send_packet()?;
        
        // Wait briefly
        std::thread::sleep(Duration::from_millis(100));
        
        // Send zero pattern
        self.channel_values = [0; 8];
        self.send_packet()?;
        
        Ok(())
    }

    /// Update configuration (requires restart to take effect)
    pub fn update_config(&mut self, config: SimShakerConfig) -> Result<(), SimShakerError> {
        // Validate new configuration
        if config.update_rate_hz <= 0.0 || config.update_rate_hz > 1000.0 {
            return Err(SimShakerError::Configuration(
                "Update rate must be between 0 and 1000 Hz".to_string()
            ));
        }

        self.config = config;
        self.update_interval = Duration::from_secs_f32(1.0 / self.config.update_rate_hz);
        
        Ok(())
    }

    /// Get current configuration
    pub fn get_config(&self) -> &SimShakerConfig {
        &self.config
    }

    /// Force send current channel state (ignores rate limiting)
    pub fn force_send(&mut self) -> Result<(), SimShakerError> {
        self.send_packet()
    }
}

impl Drop for SimShakerBridge {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::EffectIntensity;

    #[test]
    fn test_simshaker_config_validation() {
        let mut config = SimShakerConfig::default();
        
        // Valid config should work
        assert!(SimShakerBridge::new(config.clone()).is_ok());
        
        // Invalid update rate should fail
        config.update_rate_hz = 0.0;
        assert!(SimShakerBridge::new(config.clone()).is_err());
        
        config.update_rate_hz = 2000.0;
        assert!(SimShakerBridge::new(config.clone()).is_err());
        
        // Invalid channel count should fail
        config.update_rate_hz = 30.0;
        config.channel_count = 0;
        assert!(SimShakerBridge::new(config.clone()).is_err());
        
        config.channel_count = 10;
        assert!(SimShakerBridge::new(config).is_err());
    }

    #[test]
    fn test_simshaker_packet_creation() {
        let channel_data = [128, 64, 192, 32, 255, 0, 96, 160];
        let packet = SimShakerPacket::new(&channel_data, 12345);
        
        assert_eq!(packet.header, [0x53, 0x48, 0x4B, 0x52]);
        assert_eq!(packet.channels, channel_data);
        assert_eq!(packet.sequence, 12345);
        assert!(packet.checksum > 0);
        
        let bytes = packet.to_bytes();
        assert_eq!(bytes.len(), 18); // 4 + 8 + 4 + 2
    }

    #[test]
    fn test_channel_value_conversion() {
        use crate::ChannelId;
        
        let config = SimShakerConfig::default();
        let mut bridge = SimShakerBridge::new(config).unwrap();
        
        let outputs = vec![
            ChannelOutput::new(ChannelId::new(0), EffectIntensity::new(0.0).unwrap()),
            ChannelOutput::new(ChannelId::new(1), EffectIntensity::new(0.5).unwrap()),
            ChannelOutput::new(ChannelId::new(2), EffectIntensity::new(1.0).unwrap()),
        ];
        
        bridge.update_channel_values(&outputs);
        
        assert_eq!(bridge.channel_values[0], 0);
        assert_eq!(bridge.channel_values[1], 128); // 0.5 * 255 = 127.5 -> 128
        assert_eq!(bridge.channel_values[2], 255);
        assert_eq!(bridge.channel_values[3], 0); // Unused channel
    }

    #[test]
    fn test_stats_initialization() {
        let config = SimShakerConfig::default();
        let bridge = SimShakerBridge::new(config).unwrap();
        
        let stats = bridge.get_stats();
        assert_eq!(stats.packets_sent, 0);
        assert_eq!(stats.packets_failed, 0);
        assert_eq!(stats.bytes_sent, 0);
        assert_eq!(stats.status, SimShakerStatus::Disconnected);
    }
}