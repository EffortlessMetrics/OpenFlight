//! OFP-1 (Open Force Protocol v1) implementation
//!
//! This module implements the OFP-1 protocol for raw torque force feedback devices.
//! OFP-1 defines a standardized HID interface for high-performance force feedback
//! with health monitoring and capability negotiation.

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use thiserror::Error;

/// OFP-1 HID report IDs
pub mod report_ids {
    /// Feature report for device capabilities (bidirectional)
    pub const CAPABILITIES: u8 = 0x32;
    /// Output report for torque commands (host to device)
    pub const TORQUE_COMMAND: u8 = 0x30;
    /// Input report for health/status (device to host)
    pub const HEALTH_STATUS: u8 = 0x31;
}

/// OFP-1 protocol version
pub const OFP1_VERSION: u16 = 0x0100; // Version 1.0

/// Maximum torque value in the protocol (corresponds to device max)
pub const MAX_TORQUE_PROTOCOL: i16 = 32767;

/// OFP-1 specific errors
#[derive(Debug, Error)]
pub enum Ofp1Error {
    #[error("Protocol version mismatch: device {device_version:04X}, expected {expected_version:04X}")]
    VersionMismatch { device_version: u16, expected_version: u16 },
    
    #[error("Device capabilities invalid: {reason}")]
    InvalidCapabilities { reason: String },
    
    #[error("Health stream timeout: no data for {elapsed:?}")]
    HealthTimeout { elapsed: Duration },
    
    #[error("Device fault reported: {fault_code:02X} - {description}")]
    DeviceFault { fault_code: u8, description: String },
    
    #[error("Torque command out of range: {value} (max: {max})")]
    TorqueOutOfRange { value: i16, max: i16 },
    
    #[error("HID communication error: {message}")]
    HidError { message: String },
}

pub type Result<T> = std::result::Result<T, Ofp1Error>;

/// Device capabilities report (Feature 0x32)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(C, packed)]
pub struct CapabilitiesReport {
    /// Report ID (0x32)
    pub report_id: u8,
    /// Protocol version (0x0100 for OFP-1)
    pub protocol_version: u16,
    /// Device vendor ID
    pub vendor_id: u16,
    /// Device product ID
    pub product_id: u16,
    /// Maximum torque in mNm (millinewton-meters)
    pub max_torque_mnm: u32,
    /// Minimum update period in microseconds
    pub min_period_us: u32,
    /// Device capability flags
    pub capability_flags: CapabilityFlags,
    /// Device serial number (8 bytes, null-terminated)
    pub serial_number: [u8; 8],
    /// Reserved for future use
    pub reserved: [u8; 8],
}

/// Device capability flags
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(transparent)]
pub struct CapabilityFlags(pub u32);

impl CapabilityFlags {
    /// Device supports bidirectional torque
    pub const BIDIRECTIONAL: u32 = 0x0001;
    /// Device has temperature sensor
    pub const TEMPERATURE_SENSOR: u32 = 0x0002;
    /// Device has current sensor
    pub const CURRENT_SENSOR: u32 = 0x0004;
    /// Device supports physical interlock
    pub const PHYSICAL_INTERLOCK: u32 = 0x0008;
    /// Device has encoder feedback
    pub const ENCODER_FEEDBACK: u32 = 0x0010;
    /// Device supports health streaming
    pub const HEALTH_STREAM: u32 = 0x0020;
    /// Device supports emergency stop
    pub const EMERGENCY_STOP: u32 = 0x0040;
    /// Device has LED indicators
    pub const LED_INDICATORS: u32 = 0x0080;
    
    pub fn new() -> Self {
        Self(0)
    }
    
    pub fn has_flag(&self, flag: u32) -> bool {
        (self.0 & flag) != 0
    }
    
    pub fn set_flag(&mut self, flag: u32) {
        self.0 |= flag;
    }
    
    pub fn clear_flag(&mut self, flag: u32) {
        self.0 &= !flag;
    }
}

/// Torque command report (OUT 0x30)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(C, packed)]
pub struct TorqueCommandReport {
    /// Report ID (0x30)
    pub report_id: u8,
    /// Sequence number for tracking
    pub sequence: u16,
    /// Torque command (-32767 to +32767, scaled to device max)
    pub torque_command: i16,
    /// Command flags
    pub command_flags: CommandFlags,
    /// Timestamp (microseconds since device boot)
    pub timestamp_us: u32,
    /// Reserved for future use
    pub reserved: [u8; 5],
}

/// Command flags for torque commands
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(transparent)]
pub struct CommandFlags(pub u8);

impl CommandFlags {
    /// Enable torque output
    pub const ENABLE: u8 = 0x01;
    /// Emergency stop (immediate torque to zero)
    pub const EMERGENCY_STOP: u8 = 0x02;
    /// High torque mode enabled
    pub const HIGH_TORQUE: u8 = 0x04;
    /// Interlock satisfied
    pub const INTERLOCK_OK: u8 = 0x08;
    
    pub fn new() -> Self {
        Self(0)
    }
    
    pub fn has_flag(&self, flag: u8) -> bool {
        (self.0 & flag) != 0
    }
    
    pub fn set_flag(&mut self, flag: u8) {
        self.0 |= flag;
    }
    
    pub fn clear_flag(&mut self, flag: u8) {
        self.0 &= !flag;
    }
}

/// Health status report (IN 0x31)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(C, packed)]
pub struct HealthStatusReport {
    /// Report ID (0x31)
    pub report_id: u8,
    /// Sequence number (echoes last command sequence)
    pub sequence: u16,
    /// Device status flags
    pub status_flags: StatusFlags,
    /// Current torque output (-32767 to +32767)
    pub current_torque: i16,
    /// Motor temperature in 0.1°C (0 = not available)
    pub temperature_dc: u16,
    /// Motor current in mA (0 = not available)
    pub current_ma: u16,
    /// Encoder position (0 = not available)
    pub encoder_position: u32,
    /// Device uptime in seconds
    pub uptime_s: u32,
    /// Reserved for future use
    pub reserved: [u8; 2],
}

/// Device status flags
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(transparent)]
pub struct StatusFlags(pub u16);

impl StatusFlags {
    /// Device is ready for operation
    pub const READY: u16 = 0x0001;
    /// Torque output is enabled
    pub const TORQUE_ENABLED: u16 = 0x0002;
    /// High torque mode is active
    pub const HIGH_TORQUE_ACTIVE: u16 = 0x0004;
    /// Physical interlock is satisfied
    pub const INTERLOCK_OK: u16 = 0x0008;
    /// Temperature warning
    pub const TEMP_WARNING: u16 = 0x0010;
    /// Temperature fault (over limit)
    pub const TEMP_FAULT: u16 = 0x0020;
    /// Current warning
    pub const CURRENT_WARNING: u16 = 0x0040;
    /// Current fault (over limit)
    pub const CURRENT_FAULT: u16 = 0x0080;
    /// Encoder fault
    pub const ENCODER_FAULT: u16 = 0x0100;
    /// Communication fault
    pub const COMM_FAULT: u16 = 0x0200;
    /// Emergency stop active
    pub const EMERGENCY_STOP: u16 = 0x0400;
    /// Device fault (generic)
    pub const DEVICE_FAULT: u16 = 0x0800;
    
    pub fn new() -> Self {
        Self(0)
    }
    
    pub fn has_flag(&self, flag: u16) -> bool {
        (self.0 & flag) != 0
    }
    
    pub fn set_flag(&mut self, flag: u16) {
        self.0 |= flag;
    }
    
    pub fn clear_flag(&mut self, flag: u16) {
        self.0 &= !flag;
    }
    
    /// Check if any fault flags are set
    pub fn has_fault(&self) -> bool {
        const FAULT_MASK: u16 = StatusFlags::TEMP_FAULT 
            | StatusFlags::CURRENT_FAULT 
            | StatusFlags::ENCODER_FAULT 
            | StatusFlags::COMM_FAULT 
            | StatusFlags::DEVICE_FAULT;
        (self.0 & FAULT_MASK) != 0
    }
    
    /// Check if any warning flags are set
    pub fn has_warning(&self) -> bool {
        const WARNING_MASK: u16 = StatusFlags::TEMP_WARNING | StatusFlags::CURRENT_WARNING;
        (self.0 & WARNING_MASK) != 0
    }
}

/// OFP-1 device interface
pub trait Ofp1Device {
    /// Get device capabilities
    fn get_capabilities(&self) -> Result<CapabilitiesReport>;
    
    /// Send torque command
    fn send_torque_command(&mut self, command: TorqueCommandReport) -> Result<()>;
    
    /// Read health status (non-blocking)
    fn read_health_status(&mut self) -> Result<Option<HealthStatusReport>>;
    
    /// Check if device is connected
    fn is_connected(&self) -> bool;
    
    /// Get device path/identifier
    fn device_path(&self) -> &str;
}

/// OFP-1 capability negotiation
pub struct Ofp1Negotiator {
    /// Required minimum protocol version
    pub min_protocol_version: u16,
    /// Required capabilities
    pub required_capabilities: CapabilityFlags,
    /// Preferred update rate in Hz
    pub preferred_update_rate_hz: u32,
    /// Maximum acceptable latency in microseconds
    pub max_latency_us: u32,
}

impl Ofp1Negotiator {
    /// Create new negotiator with default requirements
    pub fn new() -> Self {
        let mut required_caps = CapabilityFlags::new();
        required_caps.set_flag(CapabilityFlags::HEALTH_STREAM);
        
        Self {
            min_protocol_version: OFP1_VERSION,
            required_capabilities: required_caps,
            preferred_update_rate_hz: 500,
            max_latency_us: 2000,
        }
    }
    
    /// Negotiate capabilities with device
    pub fn negotiate(&self, capabilities: &CapabilitiesReport) -> Result<Ofp1NegotiationResult> {
        // Check protocol version
        if capabilities.protocol_version < self.min_protocol_version {
            return Err(Ofp1Error::VersionMismatch {
                device_version: capabilities.protocol_version,
                expected_version: self.min_protocol_version,
            });
        }
        
        // Check required capabilities (copy to avoid packed field reference)
        let capability_flags = capabilities.capability_flags;
        for flag in [
            CapabilityFlags::HEALTH_STREAM,
            CapabilityFlags::BIDIRECTIONAL,
        ] {
            if self.required_capabilities.has_flag(flag) && !capability_flags.has_flag(flag) {
                return Err(Ofp1Error::InvalidCapabilities {
                    reason: format!("Missing required capability: 0x{:08X}", flag),
                });
            }
        }
        
        // Calculate effective update rate
        let max_rate_hz = if capabilities.min_period_us > 0 {
            1_000_000 / capabilities.min_period_us
        } else {
            1000 // Default to 1kHz if not specified
        };
        
        let effective_rate_hz = max_rate_hz.min(self.preferred_update_rate_hz);
        
        // Validate torque range
        if capabilities.max_torque_mnm == 0 {
            return Err(Ofp1Error::InvalidCapabilities {
                reason: "Invalid max torque (0 mNm)".to_string(),
            });
        }
        
        let max_torque_nm = capabilities.max_torque_mnm as f32 / 1000.0;
        
        // Check for high torque support
        let supports_high_torque = max_torque_nm >= 5.0 
            && capability_flags.has_flag(CapabilityFlags::PHYSICAL_INTERLOCK);
        
        Ok(Ofp1NegotiationResult {
            protocol_version: capabilities.protocol_version,
            effective_update_rate_hz: effective_rate_hz,
            max_torque_nm,
            supports_high_torque,
            has_temperature_sensor: capability_flags.has_flag(CapabilityFlags::TEMPERATURE_SENSOR),
            has_current_sensor: capability_flags.has_flag(CapabilityFlags::CURRENT_SENSOR),
            has_encoder_feedback: capability_flags.has_flag(CapabilityFlags::ENCODER_FEEDBACK),
            supports_emergency_stop: capability_flags.has_flag(CapabilityFlags::EMERGENCY_STOP),
            device_serial: String::from_utf8_lossy(&capabilities.serial_number).trim_end_matches('\0').to_string(),
        })
    }
}

/// Result of OFP-1 capability negotiation
#[derive(Debug, Clone)]
pub struct Ofp1NegotiationResult {
    /// Negotiated protocol version
    pub protocol_version: u16,
    /// Effective update rate in Hz
    pub effective_update_rate_hz: u32,
    /// Maximum torque in Newton-meters
    pub max_torque_nm: f32,
    /// Whether high torque mode is supported
    pub supports_high_torque: bool,
    /// Device has temperature sensor
    pub has_temperature_sensor: bool,
    /// Device has current sensor
    pub has_current_sensor: bool,
    /// Device has encoder feedback
    pub has_encoder_feedback: bool,
    /// Device supports emergency stop
    pub supports_emergency_stop: bool,
    /// Device serial number
    pub device_serial: String,
}

/// Health stream monitor for OFP-1 devices
pub struct Ofp1HealthMonitor {
    /// Last received health report
    last_health: Option<HealthStatusReport>,
    /// Timestamp of last health report
    last_health_time: Option<Instant>,
    /// Health timeout threshold
    health_timeout: Duration,
    /// Fault history
    fault_history: Vec<(Instant, StatusFlags)>,
    /// Maximum fault history entries
    max_fault_history: usize,
}

impl Ofp1HealthMonitor {
    /// Create new health monitor
    pub fn new(health_timeout: Duration) -> Self {
        Self {
            last_health: None,
            last_health_time: None,
            health_timeout,
            fault_history: Vec::new(),
            max_fault_history: 100,
        }
    }
    
    /// Update with new health report
    pub fn update_health(&mut self, health: HealthStatusReport) -> Result<()> {
        let now = Instant::now();
        
        // Check for faults (copy to avoid packed field reference)
        let status_flags = health.status_flags;
        if status_flags.has_fault() {
            self.fault_history.push((now, status_flags));
            
            // Limit fault history size
            if self.fault_history.len() > self.max_fault_history {
                self.fault_history.remove(0);
            }
            
            // Return fault error
            let fault_code = status_flags.0;
            let description = self.describe_fault(status_flags);
            return Err(Ofp1Error::DeviceFault { fault_code: fault_code as u8, description });
        }
        
        self.last_health = Some(health);
        self.last_health_time = Some(now);
        
        Ok(())
    }
    
    /// Check if health stream is current
    pub fn is_health_current(&self) -> bool {
        if let Some(last_time) = self.last_health_time {
            last_time.elapsed() < self.health_timeout
        } else {
            false
        }
    }
    
    /// Get last health report
    pub fn last_health(&self) -> Option<&HealthStatusReport> {
        self.last_health.as_ref()
    }
    
    /// Check for health timeout
    pub fn check_timeout(&self) -> Result<()> {
        if let Some(last_time) = self.last_health_time {
            let elapsed = last_time.elapsed();
            if elapsed > self.health_timeout {
                return Err(Ofp1Error::HealthTimeout { elapsed });
            }
        }
        Ok(())
    }
    
    /// Get fault history
    pub fn fault_history(&self) -> &[(Instant, StatusFlags)] {
        &self.fault_history
    }
    
    /// Describe fault flags in human-readable form
    fn describe_fault(&self, flags: StatusFlags) -> String {
        let mut descriptions = Vec::new();
        
        if flags.has_flag(StatusFlags::TEMP_FAULT) {
            descriptions.push("Temperature over limit");
        }
        if flags.has_flag(StatusFlags::CURRENT_FAULT) {
            descriptions.push("Current over limit");
        }
        if flags.has_flag(StatusFlags::ENCODER_FAULT) {
            descriptions.push("Encoder fault");
        }
        if flags.has_flag(StatusFlags::COMM_FAULT) {
            descriptions.push("Communication fault");
        }
        if flags.has_flag(StatusFlags::DEVICE_FAULT) {
            descriptions.push("Device fault");
        }
        
        if descriptions.is_empty() {
            "Unknown fault".to_string()
        } else {
            descriptions.join(", ")
        }
    }
}

impl Default for Ofp1Negotiator {
    fn default() -> Self {
        Self::new()
    }
}

/// Utility functions for OFP-1 protocol
pub mod utils {
    use super::*;
    
    /// Convert torque in Newton-meters to protocol value
    pub fn torque_nm_to_protocol(torque_nm: f32, max_torque_nm: f32) -> i16 {
        let normalized = (torque_nm / max_torque_nm).clamp(-1.0, 1.0);
        (normalized * MAX_TORQUE_PROTOCOL as f32) as i16
    }
    
    /// Convert protocol torque value to Newton-meters
    pub fn torque_protocol_to_nm(protocol_value: i16, max_torque_nm: f32) -> f32 {
        let normalized = protocol_value as f32 / MAX_TORQUE_PROTOCOL as f32;
        normalized * max_torque_nm
    }
    
    /// Validate torque command report
    pub fn validate_torque_command(command: &TorqueCommandReport) -> Result<()> {
        if command.report_id != report_ids::TORQUE_COMMAND {
            return Err(Ofp1Error::HidError {
                message: format!("Invalid report ID: expected 0x{:02X}, got 0x{:02X}", 
                    report_ids::TORQUE_COMMAND, command.report_id),
            });
        }
        
        Ok(())
    }
    
    /// Validate health status report
    pub fn validate_health_status(health: &HealthStatusReport) -> Result<()> {
        if health.report_id != report_ids::HEALTH_STATUS {
            return Err(Ofp1Error::HidError {
                message: format!("Invalid report ID: expected 0x{:02X}, got 0x{:02X}", 
                    report_ids::HEALTH_STATUS, health.report_id),
            });
        }
        
        Ok(())
    }
    
    /// Validate capabilities report
    pub fn validate_capabilities(caps: &CapabilitiesReport) -> Result<()> {
        if caps.report_id != report_ids::CAPABILITIES {
            return Err(Ofp1Error::HidError {
                message: format!("Invalid report ID: expected 0x{:02X}, got 0x{:02X}", 
                    report_ids::CAPABILITIES, caps.report_id),
            });
        }
        
        if caps.protocol_version == 0 {
            return Err(Ofp1Error::InvalidCapabilities {
                reason: "Protocol version cannot be 0".to_string(),
            });
        }
        
        if caps.max_torque_mnm == 0 {
            return Err(Ofp1Error::InvalidCapabilities {
                reason: "Max torque cannot be 0".to_string(),
            });
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_capability_flags() {
        let mut flags = CapabilityFlags::new();
        assert!(!flags.has_flag(CapabilityFlags::BIDIRECTIONAL));
        
        flags.set_flag(CapabilityFlags::BIDIRECTIONAL);
        assert!(flags.has_flag(CapabilityFlags::BIDIRECTIONAL));
        
        flags.clear_flag(CapabilityFlags::BIDIRECTIONAL);
        assert!(!flags.has_flag(CapabilityFlags::BIDIRECTIONAL));
    }
    
    #[test]
    fn test_status_flags() {
        let mut flags = StatusFlags::new();
        assert!(!flags.has_fault());
        assert!(!flags.has_warning());
        
        flags.set_flag(StatusFlags::TEMP_FAULT);
        assert!(flags.has_fault());
        
        flags.clear_flag(StatusFlags::TEMP_FAULT);
        flags.set_flag(StatusFlags::TEMP_WARNING);
        assert!(flags.has_warning());
        assert!(!flags.has_fault());
    }
    
    #[test]
    fn test_torque_conversion() {
        let max_torque = 15.0; // 15 Nm max
        
        // Test full scale positive
        let protocol_val = utils::torque_nm_to_protocol(15.0, max_torque);
        assert_eq!(protocol_val, MAX_TORQUE_PROTOCOL);
        
        let nm_val = utils::torque_protocol_to_nm(protocol_val, max_torque);
        assert!((nm_val - 15.0).abs() < 0.01);
        
        // Test full scale negative
        let protocol_val = utils::torque_nm_to_protocol(-15.0, max_torque);
        assert_eq!(protocol_val, -MAX_TORQUE_PROTOCOL);
        
        // Test zero
        let protocol_val = utils::torque_nm_to_protocol(0.0, max_torque);
        assert_eq!(protocol_val, 0);
        
        // Test clamping
        let protocol_val = utils::torque_nm_to_protocol(20.0, max_torque);
        assert_eq!(protocol_val, MAX_TORQUE_PROTOCOL);
    }
    
    #[test]
    fn test_negotiation() {
        let negotiator = Ofp1Negotiator::new();
        
        // Valid capabilities
        let mut caps = CapabilitiesReport {
            report_id: report_ids::CAPABILITIES,
            protocol_version: OFP1_VERSION,
            vendor_id: 0x1234,
            product_id: 0x5678,
            max_torque_mnm: 15000, // 15 Nm
            min_period_us: 2000,   // 500 Hz
            capability_flags: CapabilityFlags::new(),
            serial_number: *b"TEST1234",
            reserved: [0; 8],
        };
        
        caps.capability_flags.set_flag(CapabilityFlags::HEALTH_STREAM);
        caps.capability_flags.set_flag(CapabilityFlags::BIDIRECTIONAL);
        caps.capability_flags.set_flag(CapabilityFlags::PHYSICAL_INTERLOCK);
        
        let result = negotiator.negotiate(&caps).unwrap();
        assert_eq!(result.protocol_version, OFP1_VERSION);
        assert_eq!(result.effective_update_rate_hz, 500);
        assert_eq!(result.max_torque_nm, 15.0);
        assert!(result.supports_high_torque);
        
        // Test version mismatch
        caps.protocol_version = 0x0050; // Old version
        let result = negotiator.negotiate(&caps);
        assert!(matches!(result, Err(Ofp1Error::VersionMismatch { .. })));
    }
    
    #[test]
    fn test_health_monitor() {
        let mut monitor = Ofp1HealthMonitor::new(Duration::from_millis(100));
        
        // Test healthy report
        let healthy_report = HealthStatusReport {
            report_id: report_ids::HEALTH_STATUS,
            sequence: 1,
            status_flags: StatusFlags::new(),
            current_torque: 0,
            temperature_dc: 250, // 25.0°C
            current_ma: 1000,    // 1A
            encoder_position: 0,
            uptime_s: 60,
            reserved: [0; 2],
        };
        
        assert!(monitor.update_health(healthy_report).is_ok());
        assert!(monitor.is_health_current());
        
        // Test fault report
        let mut fault_report = healthy_report;
        fault_report.status_flags.set_flag(StatusFlags::TEMP_FAULT);
        
        let result = monitor.update_health(fault_report);
        assert!(matches!(result, Err(Ofp1Error::DeviceFault { .. })));
        assert_eq!(monitor.fault_history().len(), 1);
    }
    
    #[test]
    fn test_report_validation() {
        // Valid capabilities report
        let caps = CapabilitiesReport {
            report_id: report_ids::CAPABILITIES,
            protocol_version: OFP1_VERSION,
            vendor_id: 0x1234,
            product_id: 0x5678,
            max_torque_mnm: 15000,
            min_period_us: 2000,
            capability_flags: CapabilityFlags::new(),
            serial_number: *b"TEST1234",
            reserved: [0; 8],
        };
        
        assert!(utils::validate_capabilities(&caps).is_ok());
        
        // Invalid report ID
        let mut invalid_caps = caps;
        invalid_caps.report_id = 0xFF;
        assert!(utils::validate_capabilities(&invalid_caps).is_err());
        
        // Invalid torque
        let mut invalid_caps = caps;
        invalid_caps.max_torque_mnm = 0;
        assert!(utils::validate_capabilities(&invalid_caps).is_err());
    }
}