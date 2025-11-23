// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! OFP-1 reference emulator for testing and development
//!
//! This module provides a virtual OFP-1 device that implements the complete
//! protocol for testing without requiring physical hardware.

use crossbeam::channel::{self, Receiver, Sender};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

// Re-export OFP-1 types from flight-hid
pub use flight_hid::ofp1::{
    CapabilitiesReport, CapabilityFlags, CommandFlags, HealthStatusReport, MAX_TORQUE_PROTOCOL,
    OFP1_VERSION, Ofp1Device, Ofp1Error, Result as Ofp1Result, StatusFlags, TorqueCommandReport,
    report_ids, utils,
};

/// Configuration for OFP-1 emulator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ofp1EmulatorConfig {
    /// Device vendor ID
    pub vendor_id: u16,
    /// Device product ID
    pub product_id: u16,
    /// Maximum torque in mNm
    pub max_torque_mnm: u32,
    /// Minimum update period in microseconds
    pub min_period_us: u32,
    /// Device capabilities
    pub capabilities: CapabilityFlags,
    /// Device serial number
    pub serial_number: String,
    /// Health update rate in Hz
    pub health_update_rate_hz: u32,
    /// Simulate faults for testing
    pub simulate_faults: bool,
    /// Fault injection probability (0.0 to 1.0)
    pub fault_probability: f32,
}

impl Default for Ofp1EmulatorConfig {
    fn default() -> Self {
        let mut capabilities = CapabilityFlags::new();
        capabilities.set_flag(CapabilityFlags::BIDIRECTIONAL);
        capabilities.set_flag(CapabilityFlags::HEALTH_STREAM);
        capabilities.set_flag(CapabilityFlags::PHYSICAL_INTERLOCK);
        capabilities.set_flag(CapabilityFlags::TEMPERATURE_SENSOR);
        capabilities.set_flag(CapabilityFlags::CURRENT_SENSOR);
        capabilities.set_flag(CapabilityFlags::ENCODER_FEEDBACK);
        capabilities.set_flag(CapabilityFlags::EMERGENCY_STOP);
        capabilities.set_flag(CapabilityFlags::LED_INDICATORS);

        Self {
            vendor_id: 0x1234,
            product_id: 0x5678,
            max_torque_mnm: 20000, // 20 Nm - matches test expectations
            min_period_us: 500,    // 2 kHz - matches test expectations
            capabilities,
            serial_number: "EMU12345".to_string(),
            health_update_rate_hz: 100,
            simulate_faults: false,
            fault_probability: 0.01,
        }
    }
}

/// Internal state of the emulated device
#[derive(Debug)]
struct EmulatorState {
    /// Current torque command
    current_torque: AtomicU16, // Stored as u16 to allow atomic operations
    /// Current sequence number
    sequence: AtomicU16,
    /// Device status flags
    status_flags: AtomicU16,
    /// Motor temperature in 0.1°C
    temperature_dc: AtomicU16,
    /// Motor current in mA
    current_ma: AtomicU16,
    /// Encoder position
    encoder_position: AtomicU32,
    /// Device uptime in seconds
    uptime_s: AtomicU32,
    /// Whether device is connected
    connected: AtomicBool,
    /// Emergency stop state
    emergency_stop: AtomicBool,
    /// High torque mode enabled
    high_torque_enabled: AtomicBool,
    /// Interlock satisfied
    interlock_satisfied: AtomicBool,
}

impl EmulatorState {
    fn new() -> Self {
        let mut status = StatusFlags::new();
        status.set_flag(StatusFlags::READY);

        Self {
            current_torque: AtomicU16::new(0),
            sequence: AtomicU16::new(0),
            status_flags: AtomicU16::new(status.0),
            temperature_dc: AtomicU16::new(250), // 25.0°C
            current_ma: AtomicU16::new(0),
            encoder_position: AtomicU32::new(0),
            uptime_s: AtomicU32::new(0),
            connected: AtomicBool::new(true),
            emergency_stop: AtomicBool::new(false),
            high_torque_enabled: AtomicBool::new(false),
            interlock_satisfied: AtomicBool::new(false),
        }
    }

    fn get_status_flags(&self) -> StatusFlags {
        StatusFlags(self.status_flags.load(Ordering::Relaxed))
    }

    fn set_status_flag(&self, flag: u16) {
        let current = self.status_flags.load(Ordering::Relaxed);
        self.status_flags.store(current | flag, Ordering::Relaxed);
    }

    fn clear_status_flag(&self, flag: u16) {
        let current = self.status_flags.load(Ordering::Relaxed);
        self.status_flags.store(current & !flag, Ordering::Relaxed);
    }
}

/// OFP-1 reference emulator
pub struct Ofp1Emulator {
    config: Ofp1EmulatorConfig,
    state: Arc<EmulatorState>,
    device_path: String,
    command_sender: Sender<TorqueCommandReport>,
    command_receiver: Receiver<TorqueCommandReport>,
    health_sender: Sender<HealthStatusReport>,
    health_receiver: Receiver<HealthStatusReport>,
    simulation_thread: Option<thread::JoinHandle<()>>,
    start_time: Instant,
    // Cached capabilities for synchronous access
    caps: CapabilitiesReport,
    // Last torque command for tracking
    last_torque: Option<TorqueCommandReport>,
}

impl Ofp1Emulator {
    /// Create new OFP-1 emulator with default configuration
    pub fn new(device_path: String) -> Self {
        Self::with_config(device_path, Ofp1EmulatorConfig::default())
    }

    /// Create new OFP-1 emulator with custom configuration
    pub fn with_config(device_path: String, config: Ofp1EmulatorConfig) -> Self {
        let state = Arc::new(EmulatorState::new());
        let (command_sender, command_receiver) = channel::bounded(100);
        let (health_sender, health_receiver) = channel::bounded(1000);

        // Build capabilities report
        let mut serial_bytes = [0u8; 8];
        let serial_str = config.serial_number.as_bytes();
        let copy_len = serial_str.len().min(7);
        serial_bytes[..copy_len].copy_from_slice(&serial_str[..copy_len]);

        let caps = CapabilitiesReport {
            report_id: report_ids::CAPABILITIES,
            protocol_version: OFP1_VERSION,
            vendor_id: config.vendor_id,
            product_id: config.product_id,
            max_torque_mnm: config.max_torque_mnm,
            min_period_us: config.min_period_us,
            capability_flags: config.capabilities,
            serial_number: serial_bytes,
            reserved: [0; 8],
        };

        Self {
            config,
            state,
            device_path,
            command_sender,
            command_receiver,
            health_sender,
            health_receiver,
            simulation_thread: None,
            start_time: Instant::now(),
            caps,
            last_torque: None,
        }
    }

    /// Start the emulator simulation
    pub fn start(&mut self) -> Ofp1Result<()> {
        if self.simulation_thread.is_some() {
            return Ok(()); // Already started
        }

        info!("Starting OFP-1 emulator: {}", self.device_path);

        let state = self.state.clone();
        let config = self.config.clone();
        let health_sender = self.health_sender.clone();
        let command_receiver = self.command_receiver.clone();
        let start_time = self.start_time;

        let handle = thread::spawn(move || {
            Self::simulation_loop(state, config, health_sender, command_receiver, start_time);
        });

        self.simulation_thread = Some(handle);

        // Mark device as ready
        self.state.set_status_flag(StatusFlags::READY);

        Ok(())
    }

    /// Stop the emulator simulation
    pub fn stop(&mut self) {
        info!("Stopping OFP-1 emulator: {}", self.device_path);

        self.state.connected.store(false, Ordering::Relaxed);

        if let Some(handle) = self.simulation_thread.take() {
            handle.join().expect("Emulator simulation thread panicked");
        }
    }

    /// Get emulator configuration
    pub fn config(&self) -> &Ofp1EmulatorConfig {
        &self.config
    }

    /// Trigger emergency stop
    pub fn trigger_emergency_stop(&self) {
        info!("Emergency stop triggered on emulator: {}", self.device_path);
        self.state.emergency_stop.store(true, Ordering::Relaxed);
        self.state.set_status_flag(StatusFlags::EMERGENCY_STOP);
        self.state.current_torque.store(0, Ordering::Relaxed);

        // Flush old health reports so next read gets fresh state
        self.flush_health_channel();
    }

    /// Clear emergency stop
    pub fn clear_emergency_stop(&self) {
        info!("Emergency stop cleared on emulator: {}", self.device_path);
        self.state.emergency_stop.store(false, Ordering::Relaxed);
        self.state.clear_status_flag(StatusFlags::EMERGENCY_STOP);
    }

    /// Set interlock state
    pub fn set_interlock_satisfied(&self, satisfied: bool) {
        self.state
            .interlock_satisfied
            .store(satisfied, Ordering::Relaxed);
        if satisfied {
            self.state.set_status_flag(StatusFlags::INTERLOCK_OK);
        } else {
            self.state.clear_status_flag(StatusFlags::INTERLOCK_OK);
        }
    }

    /// Flush old health reports from channel (for testing)
    fn flush_health_channel(&self) {
        while self.health_receiver.try_recv().is_ok() {
            // Drain all pending reports
        }
    }

    /// Inject a fault for testing
    pub fn inject_fault(&self, fault_type: EmulatorFaultType) {
        match fault_type {
            EmulatorFaultType::TemperatureFault => {
                self.state.temperature_dc.store(1000, Ordering::Relaxed); // 100°C
                self.state.set_status_flag(StatusFlags::TEMP_FAULT);
            }
            EmulatorFaultType::CurrentFault => {
                self.state.current_ma.store(50000, Ordering::Relaxed); // 50A
                self.state.set_status_flag(StatusFlags::CURRENT_FAULT);
            }
            EmulatorFaultType::EncoderFault => {
                self.state.set_status_flag(StatusFlags::ENCODER_FAULT);
            }
            EmulatorFaultType::CommunicationFault => {
                self.state.set_status_flag(StatusFlags::COMM_FAULT);
            }
            EmulatorFaultType::DeviceFault => {
                self.state.set_status_flag(StatusFlags::DEVICE_FAULT);
            }
        }

        // Flush old health reports so next read gets fresh state
        self.flush_health_channel();

        warn!("Fault injected: {:?}", fault_type);
    }

    /// Clear all faults
    pub fn clear_faults(&self) {
        let mut status = self.state.get_status_flags();
        status.clear_flag(StatusFlags::TEMP_FAULT);
        status.clear_flag(StatusFlags::CURRENT_FAULT);
        status.clear_flag(StatusFlags::ENCODER_FAULT);
        status.clear_flag(StatusFlags::COMM_FAULT);
        status.clear_flag(StatusFlags::DEVICE_FAULT);
        status.clear_flag(StatusFlags::TEMP_WARNING);
        status.clear_flag(StatusFlags::CURRENT_WARNING);

        self.state.status_flags.store(status.0, Ordering::Relaxed);

        // Reset sensor values to normal
        self.state.temperature_dc.store(250, Ordering::Relaxed); // 25°C
        self.state.current_ma.store(1000, Ordering::Relaxed); // 1A

        // Flush old health reports so next read gets fresh state
        self.flush_health_channel();

        info!("All faults cleared");
    }

    /// Get current device statistics
    pub fn get_statistics(&self) -> EmulatorStatistics {
        let current_torque_raw = self.state.current_torque.load(Ordering::Relaxed);
        let current_torque_i16 = if current_torque_raw > 32767 {
            (current_torque_raw as i32 - 65536) as i16
        } else {
            current_torque_raw as i16
        };

        EmulatorStatistics {
            uptime: self.start_time.elapsed(),
            current_torque_protocol: current_torque_i16,
            current_torque_nm: utils::torque_protocol_to_nm(
                current_torque_i16,
                self.config.max_torque_mnm as f32 / 1000.0,
            ),
            temperature_c: self.state.temperature_dc.load(Ordering::Relaxed) as f32 / 10.0,
            current_a: self.state.current_ma.load(Ordering::Relaxed) as f32 / 1000.0,
            encoder_position: self.state.encoder_position.load(Ordering::Relaxed),
            status_flags: self.state.get_status_flags(),
            is_connected: self.state.connected.load(Ordering::Relaxed),
            emergency_stop_active: self.state.emergency_stop.load(Ordering::Relaxed),
            high_torque_enabled: self.state.high_torque_enabled.load(Ordering::Relaxed),
            interlock_satisfied: self.state.interlock_satisfied.load(Ordering::Relaxed),
        }
    }

    /// Main simulation loop (runs in separate thread)
    fn simulation_loop(
        state: Arc<EmulatorState>,
        config: Ofp1EmulatorConfig,
        health_sender: Sender<HealthStatusReport>,
        command_receiver: Receiver<TorqueCommandReport>,
        start_time: Instant,
    ) {
        let health_interval = Duration::from_millis(1000 / config.health_update_rate_hz as u64);
        let mut last_health_time = Instant::now();
        let mut rng_state = 12345u32; // Simple PRNG state

        while state.connected.load(Ordering::Relaxed) {
            // Process torque commands
            while let Ok(command) = command_receiver.try_recv() {
                Self::process_torque_command(&state, &config, command);
            }

            // Update device simulation
            Self::update_simulation(&state, &config, &mut rng_state);

            // Send health reports
            if last_health_time.elapsed() >= health_interval {
                let health_report = Self::create_health_report(&state, start_time);
                if health_sender.try_send(health_report).is_err() {
                    // Health channel full, drop oldest
                    debug!("Health channel full, dropping report");
                }
                last_health_time = Instant::now();
            }

            // Small sleep to prevent busy waiting
            thread::sleep(Duration::from_micros(100));
        }

        info!("OFP-1 emulator simulation loop ended");
    }

    /// Process incoming torque command
    fn process_torque_command(
        state: &Arc<EmulatorState>,
        config: &Ofp1EmulatorConfig,
        command: TorqueCommandReport,
    ) {
        // Validate command
        if let Err(e) = utils::validate_torque_command(&command) {
            warn!("Invalid torque command: {}", e);
            return;
        }

        // Update sequence number
        state.sequence.store(command.sequence, Ordering::Relaxed);

        // Check emergency stop
        if command.command_flags.has_flag(CommandFlags::EMERGENCY_STOP) {
            state.emergency_stop.store(true, Ordering::Relaxed);
            state.set_status_flag(StatusFlags::EMERGENCY_STOP);
            state.current_torque.store(0, Ordering::Relaxed);
            return;
        }

        // Check if emergency stop is active
        if state.emergency_stop.load(Ordering::Relaxed) {
            return; // Ignore commands during emergency stop
        }

        // Update interlock state
        let interlock_ok = command.command_flags.has_flag(CommandFlags::INTERLOCK_OK);
        state
            .interlock_satisfied
            .store(interlock_ok, Ordering::Relaxed);
        if interlock_ok {
            state.set_status_flag(StatusFlags::INTERLOCK_OK);
        } else {
            state.clear_status_flag(StatusFlags::INTERLOCK_OK);
        }

        // Update high torque mode
        let high_torque = command.command_flags.has_flag(CommandFlags::HIGH_TORQUE);
        state
            .high_torque_enabled
            .store(high_torque, Ordering::Relaxed);
        if high_torque {
            state.set_status_flag(StatusFlags::HIGH_TORQUE_ACTIVE);
        } else {
            state.clear_status_flag(StatusFlags::HIGH_TORQUE_ACTIVE);
        }

        // Apply torque command if enabled
        if command.command_flags.has_flag(CommandFlags::ENABLE) {
            // Convert signed i16 to u16 for atomic storage
            let torque_u16 = if command.torque_command < 0 {
                (command.torque_command as i32 + 65536) as u16
            } else {
                command.torque_command as u16
            };

            state.current_torque.store(torque_u16, Ordering::Relaxed);
            state.set_status_flag(StatusFlags::TORQUE_ENABLED);

            let torque_command = command.torque_command; // Copy to avoid packed field reference
            debug!(
                "Torque command applied: {} (protocol: {})",
                utils::torque_protocol_to_nm(torque_command, config.max_torque_mnm as f32 / 1000.0),
                torque_command
            );
        } else {
            state.current_torque.store(0, Ordering::Relaxed);
            state.clear_status_flag(StatusFlags::TORQUE_ENABLED);
        }
    }

    /// Update device simulation (temperature, current, encoder, etc.)
    fn update_simulation(
        state: &Arc<EmulatorState>,
        config: &Ofp1EmulatorConfig,
        rng_state: &mut u32,
    ) {
        // Update uptime
        let uptime = state.uptime_s.load(Ordering::Relaxed);
        state.uptime_s.store(uptime + 1, Ordering::Relaxed);

        // Get current torque for simulation
        let torque_raw = state.current_torque.load(Ordering::Relaxed);
        let torque_i16 = if torque_raw > 32767 {
            (torque_raw as i32 - 65536) as i16
        } else {
            torque_raw as i16
        };
        let torque_abs = torque_i16.abs() as f32 / MAX_TORQUE_PROTOCOL as f32;

        // Simulate temperature based on torque load
        let base_temp = 250u16; // 25°C
        let load_temp = (torque_abs * 200.0) as u16; // Up to 20°C increase at full torque
        let temp_noise = (Self::simple_random(rng_state) % 20) as u16; // ±2°C noise
        let new_temp = base_temp + load_temp + temp_noise;
        state.temperature_dc.store(new_temp, Ordering::Relaxed);

        // Check temperature warnings/faults
        if new_temp > 800 {
            // 80°C
            state.set_status_flag(StatusFlags::TEMP_FAULT);
        } else if new_temp > 700 {
            // 70°C
            state.set_status_flag(StatusFlags::TEMP_WARNING);
        } else {
            state.clear_status_flag(StatusFlags::TEMP_FAULT);
            state.clear_status_flag(StatusFlags::TEMP_WARNING);
        }

        // Simulate current based on torque
        let base_current = 500u16; // 0.5A idle
        let load_current = (torque_abs * 10000.0) as u16; // Up to 10A at full torque
        let current_noise = (Self::simple_random(rng_state) % 200) as u16; // ±0.2A noise
        let new_current = base_current + load_current + current_noise;
        state.current_ma.store(new_current, Ordering::Relaxed);

        // Check current warnings/faults
        if new_current > 15000 {
            // 15A
            state.set_status_flag(StatusFlags::CURRENT_FAULT);
        } else if new_current > 12000 {
            // 12A
            state.set_status_flag(StatusFlags::CURRENT_WARNING);
        } else {
            state.clear_status_flag(StatusFlags::CURRENT_FAULT);
            state.clear_status_flag(StatusFlags::CURRENT_WARNING);
        }

        // Simulate encoder position (simple integration)
        let encoder_pos = state.encoder_position.load(Ordering::Relaxed);
        let encoder_delta = (torque_i16 as i32 / 1000) as u32; // Slow movement
        state
            .encoder_position
            .store(encoder_pos.wrapping_add(encoder_delta), Ordering::Relaxed);

        // Inject random faults if enabled
        if config.simulate_faults && Self::simple_random_f32(rng_state) < config.fault_probability {
            let fault_type = match Self::simple_random(rng_state) % 5 {
                0 => StatusFlags::TEMP_FAULT,
                1 => StatusFlags::CURRENT_FAULT,
                2 => StatusFlags::ENCODER_FAULT,
                3 => StatusFlags::COMM_FAULT,
                _ => StatusFlags::DEVICE_FAULT,
            };
            state.set_status_flag(fault_type);
            debug!("Random fault injected: 0x{:04X}", fault_type);
        }
    }

    /// Create health status report
    fn create_health_report(state: &Arc<EmulatorState>, start_time: Instant) -> HealthStatusReport {
        let torque_raw = state.current_torque.load(Ordering::Relaxed);
        let torque_i16 = if torque_raw > 32767 {
            (torque_raw as i32 - 65536) as i16
        } else {
            torque_raw as i16
        };

        HealthStatusReport {
            report_id: report_ids::HEALTH_STATUS,
            sequence: state.sequence.load(Ordering::Relaxed),
            status_flags: state.get_status_flags(),
            current_torque: torque_i16,
            temperature_dc: state.temperature_dc.load(Ordering::Relaxed),
            current_ma: state.current_ma.load(Ordering::Relaxed),
            encoder_position: state.encoder_position.load(Ordering::Relaxed),
            uptime_s: start_time.elapsed().as_secs() as u32,
            reserved: [0; 2],
        }
    }

    /// Simple pseudo-random number generator
    fn simple_random(state: &mut u32) -> u32 {
        *state = state.wrapping_mul(1103515245).wrapping_add(12345);
        *state
    }

    /// Simple pseudo-random float (0.0 to 1.0)
    fn simple_random_f32(state: &mut u32) -> f32 {
        Self::simple_random(state) as f32 / u32::MAX as f32
    }
}

impl Drop for Ofp1Emulator {
    fn drop(&mut self) {
        self.stop();
    }
}

impl Ofp1Device for Ofp1Emulator {
    fn get_capabilities(&self) -> Ofp1Result<CapabilitiesReport> {
        Ok(self.caps)
    }

    fn send_torque_command(&mut self, command: TorqueCommandReport) -> Ofp1Result<()> {
        if !self.state.connected.load(Ordering::Relaxed) {
            return Err(Ofp1Error::HidError {
                message: "Device not connected".to_string(),
            });
        }

        // Update sequence immediately for synchronous tests
        self.state
            .sequence
            .store(command.sequence, Ordering::Relaxed);
        self.last_torque = Some(command);

        self.command_sender
            .try_send(command)
            .map_err(|e| Ofp1Error::HidError {
                message: format!("Command queue full: {}", e),
            })?;

        Ok(())
    }

    fn read_health_status(&mut self) -> Ofp1Result<Option<HealthStatusReport>> {
        // Best-effort drain of the async stream so we don't race with older frames
        while self.health_receiver.try_recv().is_ok() {}

        // Always generate a fresh snapshot from current atomic state
        let health_report = Self::create_health_report(&self.state, self.start_time);
        Ok(Some(health_report))
    }

    fn is_connected(&self) -> bool {
        self.state.connected.load(Ordering::Relaxed)
    }

    fn device_path(&self) -> &str {
        &self.device_path
    }
}

/// Types of faults that can be injected for testing
#[derive(Debug, Clone, Copy)]
pub enum EmulatorFaultType {
    TemperatureFault,
    CurrentFault,
    EncoderFault,
    CommunicationFault,
    DeviceFault,
}

/// Statistics from the emulator
#[derive(Debug, Clone)]
pub struct EmulatorStatistics {
    pub uptime: Duration,
    pub current_torque_protocol: i16,
    pub current_torque_nm: f32,
    pub temperature_c: f32,
    pub current_a: f32,
    pub encoder_position: u32,
    pub status_flags: StatusFlags,
    pub is_connected: bool,
    pub emergency_stop_active: bool,
    pub high_torque_enabled: bool,
    pub interlock_satisfied: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_emulator_creation() {
        let emulator = Ofp1Emulator::new("/dev/virtual0".to_string());
        assert_eq!(emulator.device_path(), "/dev/virtual0");
        assert!(emulator.is_connected());
    }

    #[test]
    fn test_emulator_capabilities() {
        let emulator = Ofp1Emulator::new("/dev/virtual0".to_string());
        let caps = emulator.get_capabilities().unwrap();

        assert_eq!(caps.report_id, report_ids::CAPABILITIES);
        // Copy fields to avoid packed field references
        let protocol_version = caps.protocol_version;
        let max_torque_mnm = caps.max_torque_mnm;
        let capability_flags = caps.capability_flags;
        assert_eq!(protocol_version, OFP1_VERSION);
        assert_eq!(max_torque_mnm, 20000); // Updated to match new default
        assert!(capability_flags.has_flag(CapabilityFlags::HEALTH_STREAM));
    }

    #[test]
    fn test_emulator_torque_command() {
        let mut emulator = Ofp1Emulator::new("/dev/virtual0".to_string());
        emulator.start().unwrap();

        let mut command = TorqueCommandReport {
            report_id: report_ids::TORQUE_COMMAND,
            sequence: 1,
            torque_command: 16384, // Half scale
            command_flags: CommandFlags::new(),
            timestamp_us: 0,
            reserved: [0; 5],
        };

        command.command_flags.set_flag(CommandFlags::ENABLE);

        assert!(emulator.send_torque_command(command).is_ok());

        // Give simulation time to process
        thread::sleep(Duration::from_millis(10));

        let stats = emulator.get_statistics();
        assert_eq!(stats.current_torque_protocol, 16384);

        emulator.stop();
    }

    #[test]
    fn test_emulator_health_stream() {
        let mut emulator = Ofp1Emulator::new("/dev/virtual0".to_string());
        emulator.start().unwrap();

        // Wait for health reports
        thread::sleep(Duration::from_millis(50));

        let health = emulator.read_health_status().unwrap();
        assert!(health.is_some());

        let health_report = health.unwrap();
        // Copy fields to avoid packed field references
        let report_id = health_report.report_id;
        let status_flags = health_report.status_flags;
        assert_eq!(report_id, report_ids::HEALTH_STATUS);
        assert!(status_flags.has_flag(StatusFlags::READY));

        emulator.stop();
    }

    #[test]
    fn test_emulator_emergency_stop() {
        let mut emulator = Ofp1Emulator::new("/dev/virtual0".to_string());
        emulator.start().unwrap();

        emulator.trigger_emergency_stop();

        let stats = emulator.get_statistics();
        assert!(stats.emergency_stop_active);
        assert!(stats.status_flags.has_flag(StatusFlags::EMERGENCY_STOP));
        assert_eq!(stats.current_torque_protocol, 0);

        emulator.stop();
    }

    #[test]
    fn test_emulator_fault_injection() {
        let mut emulator = Ofp1Emulator::new("/dev/virtual0".to_string());
        emulator.start().unwrap();

        emulator.inject_fault(EmulatorFaultType::TemperatureFault);

        let stats = emulator.get_statistics();
        assert!(stats.status_flags.has_flag(StatusFlags::TEMP_FAULT));
        assert!(stats.temperature_c > 90.0); // Should be high

        emulator.clear_faults();

        let stats = emulator.get_statistics();
        assert!(!stats.status_flags.has_flag(StatusFlags::TEMP_FAULT));

        emulator.stop();
    }

    #[test]
    fn test_emulator_interlock() {
        let emulator = Ofp1Emulator::new("/dev/virtual0".to_string());

        assert!(!emulator.get_statistics().interlock_satisfied);

        emulator.set_interlock_satisfied(true);
        assert!(emulator.get_statistics().interlock_satisfied);
        assert!(
            emulator
                .get_statistics()
                .status_flags
                .has_flag(StatusFlags::INTERLOCK_OK)
        );

        emulator.set_interlock_satisfied(false);
        assert!(!emulator.get_statistics().interlock_satisfied);
        assert!(
            !emulator
                .get_statistics()
                .status_flags
                .has_flag(StatusFlags::INTERLOCK_OK)
        );
    }
}
