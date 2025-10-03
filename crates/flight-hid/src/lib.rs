// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID device management with watchdog integration
//!
//! Provides USB HID device monitoring, endpoint management, and integration
//! with the watchdog system for fault detection and quarantine.

pub mod ofp1;
#[cfg(test)]
mod ofp1_tests;
#[cfg(test)]
mod fd_safety_tests;

use flight_core::{
    WatchdogSystem, WatchdogConfig, ComponentType, WatchdogEvent,
    FlightError, Result
};
use std::time::{Duration, Instant};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{debug, warn, error, info};

/// HID device endpoint identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EndpointId {
    pub device_path: String,
    pub endpoint_type: EndpointType,
}

/// Types of HID endpoints
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EndpointType {
    Input,
    Output,
    Feature,
}

/// HID device information
#[derive(Debug, Clone)]
pub struct HidDeviceInfo {
    pub vendor_id: u16,
    pub product_id: u16,
    pub serial_number: Option<String>,
    pub manufacturer: Option<String>,
    pub product_name: Option<String>,
    pub device_path: String,
    pub usage_page: u16,
    pub usage: u16,
}

/// HID endpoint state tracking
#[derive(Debug)]
struct EndpointState {
    /// Last successful operation
    last_success: Instant,
    /// Consecutive failure count
    consecutive_failures: u32,
    /// Whether endpoint is currently stalled
    is_stalled: bool,
    /// Frame stall counter
    frame_stall_count: u32,
    /// Total bytes transferred
    bytes_transferred: u64,
    /// Operation count
    operation_count: u64,
}

/// HID operation result
#[derive(Debug)]
pub enum HidOperationResult {
    Success { bytes_transferred: usize },
    Stall,
    Timeout,
    Error { error_code: i32, description: String },
}

/// HID adapter with watchdog integration
pub struct HidAdapter {
    /// Watchdog system for monitoring
    watchdog: Arc<Mutex<WatchdogSystem>>,
    /// Connected devices
    devices: HashMap<String, HidDeviceInfo>,
    /// Endpoint states
    endpoint_states: HashMap<EndpointId, EndpointState>,
    /// Whether adapter is running
    is_running: bool,
}

impl HidAdapter {
    /// Create new HID adapter with watchdog integration
    pub fn new(watchdog: Arc<Mutex<WatchdogSystem>>) -> Self {
        Self {
            watchdog,
            devices: HashMap::new(),
            endpoint_states: HashMap::new(),
            is_running: false,
        }
    }

    /// Start the HID adapter
    pub fn start(&mut self) -> Result<()> {
        if self.is_running {
            return Ok(());
        }

        info!("Starting HID adapter with watchdog integration");
        
        // Enumerate existing devices
        self.enumerate_devices()?;
        
        self.is_running = true;
        Ok(())
    }

    /// Stop the HID adapter
    pub fn stop(&mut self) {
        if !self.is_running {
            return;
        }

        info!("Stopping HID adapter");
        
        // Unregister all endpoints from watchdog
        for endpoint_id in self.endpoint_states.keys() {
            let component = ComponentType::UsbEndpoint(endpoint_id.device_path.clone());
            if let Ok(mut watchdog) = self.watchdog.lock() {
                watchdog.unregister_component(&component);
            }
        }
        
        self.devices.clear();
        self.endpoint_states.clear();
        self.is_running = false;
    }

    /// Enumerate and register HID devices
    fn enumerate_devices(&mut self) -> Result<()> {
        debug!("Enumerating HID devices");
        
        // This would normally use platform-specific HID enumeration
        // For now, we'll simulate device discovery
        
        // Example device registration
        let device_info = HidDeviceInfo {
            vendor_id: 0x046d,
            product_id: 0xc262,
            serial_number: Some("123456789".to_string()),
            manufacturer: Some("Logitech".to_string()),
            product_name: Some("Flight Yoke".to_string()),
            device_path: "/dev/hidraw0".to_string(),
            usage_page: 0x01,
            usage: 0x04,
        };
        
        self.register_device(device_info)?;
        
        Ok(())
    }

    /// Register a new HID device
    pub fn register_device(&mut self, device_info: HidDeviceInfo) -> Result<()> {
        info!("Registering HID device: {} (VID:{:04X} PID:{:04X})", 
              device_info.product_name.as_deref().unwrap_or("Unknown"),
              device_info.vendor_id, 
              device_info.product_id);

        let device_path = device_info.device_path.clone();
        
        // Register endpoints with watchdog
        let endpoints = vec![
            EndpointId { device_path: device_path.clone(), endpoint_type: EndpointType::Input },
            EndpointId { device_path: device_path.clone(), endpoint_type: EndpointType::Output },
        ];

        for endpoint_id in &endpoints {
            let component = ComponentType::UsbEndpoint(format!("{}:{:?}", endpoint_id.device_path, endpoint_id.endpoint_type));
            
            let config = WatchdogConfig {
                usb_timeout: Duration::from_millis(100),
                max_consecutive_failures: 3,
                max_failures_per_window: 10,
                failure_rate_window: Duration::from_secs(60),
                enable_nan_guards: false, // Not applicable for USB endpoints
                is_critical: true, // USB endpoints are critical for operation
                ..Default::default()
            };

            if let Ok(mut watchdog) = self.watchdog.lock() {
                watchdog.register_component(component, config);
            }

            // Initialize endpoint state
            self.endpoint_states.insert(endpoint_id.clone(), EndpointState {
                last_success: Instant::now(),
                consecutive_failures: 0,
                is_stalled: false,
                frame_stall_count: 0,
                bytes_transferred: 0,
                operation_count: 0,
            });
        }

        self.devices.insert(device_path, device_info);
        Ok(())
    }

    /// Unregister a HID device
    pub fn unregister_device(&mut self, device_path: &str) -> Result<()> {
        info!("Unregistering HID device: {}", device_path);

        // Remove from devices
        self.devices.remove(device_path);

        // Remove endpoint states and unregister from watchdog
        let endpoints_to_remove: Vec<_> = self.endpoint_states.keys()
            .filter(|id| id.device_path == device_path)
            .cloned()
            .collect();

        for endpoint_id in endpoints_to_remove {
            let component = ComponentType::UsbEndpoint(format!("{}:{:?}", endpoint_id.device_path, endpoint_id.endpoint_type));
            
            if let Ok(mut watchdog) = self.watchdog.lock() {
                watchdog.unregister_component(&component);
            }
            
            self.endpoint_states.remove(&endpoint_id);
        }

        Ok(())
    }

    /// Perform HID input operation with watchdog monitoring
    pub fn read_input(&mut self, device_path: &str, buffer: &mut [u8]) -> Result<HidOperationResult> {
        let endpoint_id = EndpointId {
            device_path: device_path.to_string(),
            endpoint_type: EndpointType::Input,
        };

        self.perform_operation(&endpoint_id, |_| {
            // Simulate HID read operation
            // In real implementation, this would call platform-specific HID APIs
            
            // Simulate occasional stalls for testing
            if rand::random::<f32>() < 0.01 { // 1% chance of stall
                HidOperationResult::Stall
            } else if rand::random::<f32>() < 0.005 { // 0.5% chance of error
                HidOperationResult::Error {
                    error_code: -1,
                    description: "Simulated USB error".to_string(),
                }
            } else {
                // Simulate successful read
                let bytes_read = std::cmp::min(buffer.len(), 8);
                HidOperationResult::Success { bytes_transferred: bytes_read }
            }
        })
    }

    /// Perform HID output operation with watchdog monitoring
    pub fn write_output(&mut self, device_path: &str, data: &[u8]) -> Result<HidOperationResult> {
        let endpoint_id = EndpointId {
            device_path: device_path.to_string(),
            endpoint_type: EndpointType::Output,
        };

        self.perform_operation(&endpoint_id, |_| {
            // Simulate HID write operation
            // In real implementation, this would call platform-specific HID APIs
            
            // Check if component is quarantined
            let component = ComponentType::UsbEndpoint(format!("{}:{:?}", endpoint_id.device_path, endpoint_id.endpoint_type));
            
            // Simulate occasional stalls for testing
            if rand::random::<f32>() < 0.02 { // 2% chance of stall for output
                HidOperationResult::Stall
            } else if rand::random::<f32>() < 0.01 { // 1% chance of error
                HidOperationResult::Error {
                    error_code: -2,
                    description: "Simulated USB write error".to_string(),
                }
            } else {
                // Simulate successful write
                HidOperationResult::Success { bytes_transferred: data.len() }
            }
        })
    }

    /// Perform a HID operation with watchdog monitoring
    fn perform_operation<F>(&mut self, endpoint_id: &EndpointId, operation: F) -> Result<HidOperationResult>
    where
        F: FnOnce(&EndpointId) -> HidOperationResult,
    {
        let component = ComponentType::UsbEndpoint(format!("{}:{:?}", endpoint_id.device_path, endpoint_id.endpoint_type));
        
        // Check if component is quarantined
        if let Ok(watchdog) = self.watchdog.lock() {
            if watchdog.is_quarantined(&component) {
                return Err(FlightError::Configuration(format!(
                    "USB endpoint {} is quarantined", 
                    endpoint_id.device_path
                )));
            }
        }

        let start_time = Instant::now();
        let result = operation(endpoint_id);
        let operation_time = start_time.elapsed();

        // Update endpoint state and notify watchdog
        if let Some(state) = self.endpoint_states.get_mut(endpoint_id) {
            state.operation_count += 1;

            match &result {
                HidOperationResult::Success { bytes_transferred } => {
                    state.last_success = Instant::now();
                    state.consecutive_failures = 0;
                    state.is_stalled = false;
                    state.frame_stall_count = 0;
                    state.bytes_transferred += *bytes_transferred as u64;

                    // Notify watchdog of success
                    if let Ok(mut watchdog) = self.watchdog.lock() {
                        watchdog.record_usb_success(&endpoint_id.device_path);
                    }

                    debug!("USB operation successful: {} bytes in {:?}", bytes_transferred, operation_time);
                }
                HidOperationResult::Stall => {
                    state.consecutive_failures += 1;
                    state.is_stalled = true;
                    state.frame_stall_count += 1;

                    // Notify watchdog of stall
                    if let Ok(mut watchdog) = self.watchdog.lock() {
                        if let Some(event) = watchdog.record_usb_stall(&endpoint_id.device_path) {
                            warn!("USB stall detected and reported to watchdog: {:?}", event);
                        }
                    }

                    warn!("USB endpoint stalled: {}", endpoint_id.device_path);
                }
                HidOperationResult::Timeout => {
                    state.consecutive_failures += 1;

                    // Check for timeout with watchdog
                    if let Ok(mut watchdog) = self.watchdog.lock() {
                        if let Some(event) = watchdog.check_usb_timeout(&endpoint_id.device_path) {
                            error!("USB timeout detected and reported to watchdog: {:?}", event);
                        }
                    }

                    error!("USB endpoint timeout: {}", endpoint_id.device_path);
                }
                HidOperationResult::Error { error_code, description } => {
                    state.consecutive_failures += 1;

                    // Notify watchdog of error
                    if let Ok(mut watchdog) = self.watchdog.lock() {
                        let context = format!("Error {}: {}", error_code, description);
                        let event = watchdog.record_usb_error(&endpoint_id.device_path, &context);
                        error!("USB error detected and reported to watchdog: {:?}", event);
                    }

                    error!("USB endpoint error: {} - {}", error_code, description);
                }
            }
        }

        Ok(result)
    }

    /// Get device information
    pub fn get_device_info(&self, device_path: &str) -> Option<&HidDeviceInfo> {
        self.devices.get(device_path)
    }

    /// Get all connected devices
    pub fn get_all_devices(&self) -> Vec<&HidDeviceInfo> {
        self.devices.values().collect()
    }

    /// Get endpoint state
    pub fn get_endpoint_state(&self, endpoint_id: &EndpointId) -> Option<&EndpointState> {
        self.endpoint_states.get(endpoint_id)
    }

    /// Check endpoint health
    pub fn check_endpoint_health(&mut self, device_path: &str) -> Result<Vec<WatchdogEvent>> {
        let mut events = Vec::new();

        // Check all endpoints for this device
        let endpoints: Vec<_> = self.endpoint_states.keys()
            .filter(|id| id.device_path == device_path)
            .cloned()
            .collect();

        for endpoint_id in endpoints {
            if let Some(state) = self.endpoint_states.get(&endpoint_id) {
                // Check for timeout
                if let Ok(mut watchdog) = self.watchdog.lock() {
                    if let Some(event) = watchdog.check_usb_timeout(&endpoint_id.device_path) {
                        events.push(event);
                    }

                    // Check for endpoint wedge condition
                    let is_responsive = state.last_success.elapsed() < Duration::from_millis(100);
                    if let Some(event) = watchdog.check_endpoint_wedge(is_responsive) {
                        events.push(event);
                    }
                }
            }
        }

        Ok(events)
    }

    /// Get adapter statistics
    pub fn get_statistics(&self) -> HidAdapterStats {
        let total_devices = self.devices.len();
        let total_endpoints = self.endpoint_states.len();
        
        let total_operations: u64 = self.endpoint_states.values()
            .map(|state| state.operation_count)
            .sum();
        
        let total_bytes: u64 = self.endpoint_states.values()
            .map(|state| state.bytes_transferred)
            .sum();
        
        let failed_endpoints = self.endpoint_states.values()
            .filter(|state| state.consecutive_failures > 0)
            .count();

        let stalled_endpoints = self.endpoint_states.values()
            .filter(|state| state.is_stalled)
            .count();

        HidAdapterStats {
            total_devices,
            total_endpoints,
            total_operations,
            total_bytes,
            failed_endpoints,
            stalled_endpoints,
        }
    }

    /// Attempt to recover a quarantined endpoint
    pub fn attempt_endpoint_recovery(&mut self, device_path: &str) -> Result<bool> {
        let component = ComponentType::UsbEndpoint(device_path.to_string());
        
        if let Ok(mut watchdog) = self.watchdog.lock() {
            if watchdog.attempt_recovery(&component) {
                info!("Attempting recovery for USB endpoint: {}", device_path);
                
                // Reset local endpoint state
                let endpoints_to_reset: Vec<_> = self.endpoint_states.keys()
                    .filter(|id| id.device_path == device_path)
                    .cloned()
                    .collect();

                for endpoint_id in endpoints_to_reset {
                    if let Some(state) = self.endpoint_states.get_mut(&endpoint_id) {
                        state.consecutive_failures = 0;
                        state.is_stalled = false;
                        state.frame_stall_count = 0;
                        state.last_success = Instant::now();
                    }
                }
                
                return Ok(true);
            }
        }
        
        Ok(false)
    }

    /// Perform OFP-1 capability negotiation with device
    pub fn negotiate_ofp1_capabilities(&self, device_path: &str) -> Result<Option<crate::ofp1::Ofp1NegotiationResult>> {
        // This would normally perform HID Feature report exchange
        // For now, simulate successful negotiation for testing
        
        if let Some(device_info) = self.get_device_info(device_path) {
            // Check if device supports OFP-1 (based on VID/PID or other criteria)
            if self.is_ofp1_compatible(device_info) {
                let negotiator = crate::ofp1::Ofp1Negotiator::new();
                
                // Simulate getting capabilities from device
                let capabilities = self.simulate_device_capabilities(device_info)?;
                
                match negotiator.negotiate(&capabilities) {
                    Ok(result) => {
                        info!("OFP-1 negotiation successful for {}: {:?}", device_path, result);
                        Ok(Some(result))
                    }
                    Err(e) => {
                        warn!("OFP-1 negotiation failed for {}: {}", device_path, e);
                        Ok(None)
                    }
                }
            } else {
                debug!("Device {} is not OFP-1 compatible", device_path);
                Ok(None)
            }
        } else {
            Err(FlightError::Configuration(format!("Device not found: {}", device_path)))
        }
    }

    /// Check if device is OFP-1 compatible
    fn is_ofp1_compatible(&self, device_info: &HidDeviceInfo) -> bool {
        // Check for known OFP-1 compatible devices
        // This would normally check VID/PID against a database
        
        // For testing, consider Logitech devices as potentially OFP-1 compatible
        device_info.vendor_id == 0x046d || 
        device_info.vendor_id == 0x1234 // Test VID
    }

    /// Simulate device capabilities for testing
    fn simulate_device_capabilities(&self, device_info: &HidDeviceInfo) -> Result<crate::ofp1::CapabilitiesReport> {
        let mut capabilities = crate::ofp1::CapabilityFlags::new();
        capabilities.set_flag(crate::ofp1::CapabilityFlags::BIDIRECTIONAL);
        capabilities.set_flag(crate::ofp1::CapabilityFlags::HEALTH_STREAM);
        
        // Add more capabilities based on device type
        if device_info.vendor_id == 0x1234 {
            capabilities.set_flag(crate::ofp1::CapabilityFlags::PHYSICAL_INTERLOCK);
            capabilities.set_flag(crate::ofp1::CapabilityFlags::TEMPERATURE_SENSOR);
            capabilities.set_flag(crate::ofp1::CapabilityFlags::CURRENT_SENSOR);
        }

        let serial_str = device_info.serial_number.as_deref().unwrap_or("UNKNOWN");
        let mut serial_bytes = [0u8; 8];
        let copy_len = serial_str.len().min(7);
        serial_bytes[..copy_len].copy_from_slice(serial_str.as_bytes());

        Ok(crate::ofp1::CapabilitiesReport {
            report_id: crate::ofp1::report_ids::CAPABILITIES,
            protocol_version: crate::ofp1::OFP1_VERSION,
            vendor_id: device_info.vendor_id,
            product_id: device_info.product_id,
            max_torque_mnm: 15000, // 15 Nm
            min_period_us: 2000,   // 500 Hz
            capability_flags: capabilities,
            serial_number: serial_bytes,
            reserved: [0; 8],
        })
    }

    /// Send OFP-1 torque command to device
    pub fn send_ofp1_torque_command(&mut self, device_path: &str, command: crate::ofp1::TorqueCommandReport) -> Result<HidOperationResult> {
        // Validate command first
        if let Err(e) = crate::ofp1::utils::validate_torque_command(&command) {
            return Err(FlightError::Configuration(format!("Invalid OFP-1 command: {}", e)));
        }

        // Convert to HID output report and send
        // In real implementation, this would serialize the command struct to bytes
        let command_bytes = self.serialize_ofp1_command(&command)?;
        
        self.write_output(device_path, &command_bytes)
    }

    /// Read OFP-1 health status from device
    pub fn read_ofp1_health_status(&mut self, device_path: &str) -> Result<Option<crate::ofp1::HealthStatusReport>> {
        // Read HID input report
        let mut buffer = [0u8; 32]; // OFP-1 health report size
        
        match self.read_input(device_path, &mut buffer)? {
            HidOperationResult::Success { bytes_transferred } => {
                if bytes_transferred >= std::mem::size_of::<crate::ofp1::HealthStatusReport>() {
                    let health_report = self.deserialize_ofp1_health(&buffer)?;
                    
                    // Validate report
                    if let Err(e) = crate::ofp1::utils::validate_health_status(&health_report) {
                        warn!("Invalid OFP-1 health report from {}: {}", device_path, e);
                        return Ok(None);
                    }
                    
                    Ok(Some(health_report))
                } else {
                    debug!("Insufficient data for OFP-1 health report: {} bytes", bytes_transferred);
                    Ok(None)
                }
            }
            _ => Ok(None), // No data available or error
        }
    }

    /// Serialize OFP-1 torque command to bytes
    fn serialize_ofp1_command(&self, command: &crate::ofp1::TorqueCommandReport) -> Result<Vec<u8>> {
        // In real implementation, this would use proper serialization
        // For now, simulate with a simple byte array
        Ok(vec![
            command.report_id,
            (command.sequence & 0xFF) as u8,
            (command.sequence >> 8) as u8,
            (command.torque_command & 0xFF) as u8,
            (command.torque_command >> 8) as u8,
            command.command_flags.0,
            (command.timestamp_us & 0xFF) as u8,
            (command.timestamp_us >> 8) as u8,
            (command.timestamp_us >> 16) as u8,
            (command.timestamp_us >> 24) as u8,
        ])
    }

    /// Deserialize OFP-1 health status from bytes
    fn deserialize_ofp1_health(&self, buffer: &[u8]) -> Result<crate::ofp1::HealthStatusReport> {
        // In real implementation, this would use proper deserialization
        // For now, simulate with a basic structure
        if buffer.len() < 16 {
            return Err(FlightError::Configuration("Buffer too small for health report".to_string()));
        }

        let mut status_flags = crate::ofp1::StatusFlags::new();
        status_flags.set_flag(crate::ofp1::StatusFlags::READY);
        status_flags.set_flag(crate::ofp1::StatusFlags::TORQUE_ENABLED);

        Ok(crate::ofp1::HealthStatusReport {
            report_id: buffer[0],
            sequence: u16::from_le_bytes([buffer[1], buffer[2]]),
            status_flags,
            current_torque: i16::from_le_bytes([buffer[5], buffer[6]]),
            temperature_dc: 250, // 25.0°C
            current_ma: 1000,    // 1A
            encoder_position: 0,
            uptime_s: 60,
            reserved: [0; 2],
        })
    }
}

/// HID adapter statistics
#[derive(Debug, Clone)]
pub struct HidAdapterStats {
    pub total_devices: usize,
    pub total_endpoints: usize,
    pub total_operations: u64,
    pub total_bytes: u64,
    pub failed_endpoints: usize,
    pub stalled_endpoints: usize,
}

impl EndpointState {
    /// Get success rate for this endpoint
    pub fn success_rate(&self) -> f64 {
        if self.operation_count == 0 {
            return 1.0;
        }
        
        let successful_operations = self.operation_count.saturating_sub(self.consecutive_failures as u64);
        successful_operations as f64 / self.operation_count as f64
    }

    /// Get average bytes per operation
    pub fn avg_bytes_per_operation(&self) -> f64 {
        if self.operation_count == 0 {
            return 0.0;
        }
        
        self.bytes_transferred as f64 / self.operation_count as f64
    }
}

// Add rand dependency for simulation
extern crate rand;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn create_test_adapter() -> HidAdapter {
        let watchdog = Arc::new(Mutex::new(WatchdogSystem::new()));
        HidAdapter::new(watchdog)
    }

    #[test]
    fn test_device_registration() {
        let mut adapter = create_test_adapter();
        
        let device_info = HidDeviceInfo {
            vendor_id: 0x1234,
            product_id: 0x5678,
            serial_number: Some("TEST123".to_string()),
            manufacturer: Some("Test Manufacturer".to_string()),
            product_name: Some("Test Device".to_string()),
            device_path: "/dev/test0".to_string(),
            usage_page: 0x01,
            usage: 0x04,
        };

        assert!(adapter.register_device(device_info.clone()).is_ok());
        assert!(adapter.get_device_info(&device_info.device_path).is_some());
        assert_eq!(adapter.get_all_devices().len(), 1);
    }

    #[test]
    fn test_device_unregistration() {
        let mut adapter = create_test_adapter();
        
        let device_info = HidDeviceInfo {
            vendor_id: 0x1234,
            product_id: 0x5678,
            serial_number: Some("TEST123".to_string()),
            manufacturer: Some("Test Manufacturer".to_string()),
            product_name: Some("Test Device".to_string()),
            device_path: "/dev/test0".to_string(),
            usage_page: 0x01,
            usage: 0x04,
        };

        adapter.register_device(device_info.clone()).unwrap();
        assert!(adapter.unregister_device(&device_info.device_path).is_ok());
        assert!(adapter.get_device_info(&device_info.device_path).is_none());
        assert_eq!(adapter.get_all_devices().len(), 0);
    }

    #[test]
    fn test_endpoint_operations() {
        let mut adapter = create_test_adapter();
        
        let device_info = HidDeviceInfo {
            vendor_id: 0x1234,
            product_id: 0x5678,
            serial_number: Some("TEST123".to_string()),
            manufacturer: Some("Test Manufacturer".to_string()),
            product_name: Some("Test Device".to_string()),
            device_path: "/dev/test0".to_string(),
            usage_page: 0x01,
            usage: 0x04,
        };

        adapter.register_device(device_info.clone()).unwrap();

        // Test read operation
        let mut buffer = [0u8; 64];
        let result = adapter.read_input(&device_info.device_path, &mut buffer);
        assert!(result.is_ok());

        // Test write operation
        let data = [1, 2, 3, 4];
        let result = adapter.write_output(&device_info.device_path, &data);
        assert!(result.is_ok());
    }

    #[test]
    fn test_adapter_statistics() {
        let adapter = create_test_adapter();
        let stats = adapter.get_statistics();
        
        assert_eq!(stats.total_devices, 0);
        assert_eq!(stats.total_endpoints, 0);
        assert_eq!(stats.total_operations, 0);
        assert_eq!(stats.total_bytes, 0);
    }
}
