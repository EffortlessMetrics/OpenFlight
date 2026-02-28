// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! OFP-1 protocol integration for FFB engine
//!
//! This module integrates the OFP-1 protocol with the FFB engine,
//! providing capability negotiation, torque path stability, and
//! health stream monitoring for raw torque mode operation.

use crossbeam::channel::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

use crate::{DeviceCapabilities, FfbEngine, FfbError, Result};
use flight_hid::ofp1::{
    HealthStatusReport, Ofp1Device, Ofp1Error, Ofp1HealthMonitor, Ofp1NegotiationResult,
    Ofp1Negotiator, StatusFlags, report_ids, utils,
};

/// OFP-1 FFB integration manager
pub struct Ofp1FfbIntegration {
    /// FFB engine reference
    ffb_engine: Arc<Mutex<FfbEngine>>,
    /// OFP-1 device interface
    device: Box<dyn Ofp1Device + Send>,
    /// Negotiation result
    negotiation_result: Option<Ofp1NegotiationResult>,
    /// Health monitor
    health_monitor: Ofp1HealthMonitor,
    /// Torque command sender
    command_sender: Sender<TorqueCommand>,
    /// Torque command receiver
    command_receiver: Receiver<TorqueCommand>,
    /// Health update sender
    health_sender: Sender<HealthUpdate>,
    /// Health update receiver
    health_receiver: Receiver<HealthUpdate>,
    /// Integration thread handle
    integration_thread: Option<thread::JoinHandle<()>>,
    /// Whether integration is running
    is_running: bool,
    /// Last command sequence number
    _last_sequence: u16,
    /// Torque path stability tracker
    stability_tracker: TorquePathStabilityTracker,
}

/// Internal torque command for thread communication
#[derive(Debug, Clone)]
struct TorqueCommand {
    torque_nm: f32,
    enable: bool,
    high_torque: bool,
    interlock_ok: bool,
    emergency_stop: bool,
}

/// Internal health update for thread communication
#[derive(Debug, Clone)]
struct HealthUpdate {
    health_report: HealthStatusReport,
    _timestamp: Instant,
}

/// Torque path stability tracking
#[derive(Debug)]
struct TorquePathStabilityTracker {
    /// Recent torque commands
    recent_commands: Vec<(Instant, f32)>,
    /// Maximum history size
    max_history: usize,
    /// Stability window duration
    stability_window: Duration,
    /// Maximum allowed deviation for stability
    max_deviation: f32,
}

impl TorquePathStabilityTracker {
    fn new() -> Self {
        Self {
            recent_commands: Vec::new(),
            max_history: 1000,
            stability_window: Duration::from_secs(1),
            max_deviation: 0.1, // 10% deviation threshold
        }
    }

    /// Record a torque command
    fn record_command(&mut self, torque_nm: f32) {
        let now = Instant::now();
        self.recent_commands.push((now, torque_nm));

        // Remove old entries
        let cutoff = now - self.stability_window;
        self.recent_commands
            .retain(|(timestamp, _)| *timestamp > cutoff);

        // Limit history size
        if self.recent_commands.len() > self.max_history {
            let excess = self.recent_commands.len() - self.max_history;
            self.recent_commands.drain(0..excess);
        }
    }

    /// Check if torque path is stable
    fn is_stable(&self) -> bool {
        if self.recent_commands.len() < 10 {
            return false; // Need minimum samples
        }

        let values: Vec<f32> = self
            .recent_commands
            .iter()
            .map(|(_, torque)| *torque)
            .collect();
        let mean = values.iter().sum::<f32>() / values.len() as f32;

        // Calculate standard deviation
        let variance = values
            .iter()
            .map(|value| (value - mean).powi(2))
            .sum::<f32>()
            / values.len() as f32;
        let std_dev = variance.sqrt();

        // Check if standard deviation is within acceptable range
        std_dev <= self.max_deviation
    }

    /// Get stability metrics
    fn get_metrics(&self) -> TorqueStabilityMetrics {
        if self.recent_commands.is_empty() {
            return TorqueStabilityMetrics::default();
        }

        let values: Vec<f32> = self
            .recent_commands
            .iter()
            .map(|(_, torque)| *torque)
            .collect();
        let mean = values.iter().sum::<f32>() / values.len() as f32;

        let variance = values
            .iter()
            .map(|value| (value - mean).powi(2))
            .sum::<f32>()
            / values.len() as f32;
        let std_dev = variance.sqrt();

        let min_torque = values.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let max_torque = values.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));

        TorqueStabilityMetrics {
            sample_count: values.len(),
            mean_torque: mean,
            std_deviation: std_dev,
            min_torque,
            max_torque,
            is_stable: self.is_stable(),
        }
    }
}

/// Torque path stability metrics
#[derive(Debug, Clone, Default)]
pub struct TorqueStabilityMetrics {
    pub sample_count: usize,
    pub mean_torque: f32,
    pub std_deviation: f32,
    pub min_torque: f32,
    pub max_torque: f32,
    pub is_stable: bool,
}

impl Ofp1FfbIntegration {
    /// Create new OFP-1 FFB integration
    pub fn new(
        ffb_engine: Arc<Mutex<FfbEngine>>,
        device: Box<dyn Ofp1Device + Send>,
    ) -> Result<Self> {
        let health_monitor = Ofp1HealthMonitor::new(Duration::from_millis(200)); // 200ms timeout
        let (command_sender, command_receiver) = channel::bounded(100);
        let (health_sender, health_receiver) = channel::bounded(1000);

        Ok(Self {
            ffb_engine,
            device,
            negotiation_result: None,
            health_monitor,
            command_sender,
            command_receiver,
            health_sender,
            health_receiver,
            integration_thread: None,
            is_running: false,
            _last_sequence: 0,
            stability_tracker: TorquePathStabilityTracker::new(),
        })
    }

    /// Start OFP-1 integration
    pub fn start(&mut self) -> Result<()> {
        if self.is_running {
            return Ok(());
        }

        info!(
            "Starting OFP-1 FFB integration for device: {}",
            self.device.device_path()
        );

        // Perform capability negotiation
        self.negotiate_capabilities()?;

        // Update FFB engine with negotiated capabilities
        self.update_ffb_engine_capabilities()?;

        // Start integration thread
        self.start_integration_thread()?;

        self.is_running = true;
        Ok(())
    }

    /// Stop OFP-1 integration
    pub fn stop(&mut self) {
        if !self.is_running {
            return;
        }

        info!(
            "Stopping OFP-1 FFB integration for device: {}",
            self.device.device_path()
        );

        self.is_running = false;

        // Close the command channel by dropping all senders so the integration
        // thread's recv() returns Err(Disconnected) and the loop exits cleanly.
        let (new_sender, new_receiver) = crossbeam::channel::bounded(100);
        let _ = std::mem::replace(&mut self.command_sender, new_sender);
        let _ = std::mem::replace(&mut self.command_receiver, new_receiver);

        if let Some(handle) = self.integration_thread.take() {
            let _ = handle.join();
        }
    }

    /// Perform OFP-1 capability negotiation
    fn negotiate_capabilities(&mut self) -> Result<()> {
        let capabilities = self
            .device
            .get_capabilities()
            .map_err(|e| FfbError::DeviceError {
                message: format!("Failed to get capabilities: {}", e),
            })?;

        let negotiator = Ofp1Negotiator::new();
        let result = negotiator
            .negotiate(&capabilities)
            .map_err(|e| FfbError::DeviceError {
                message: format!("Capability negotiation failed: {}", e),
            })?;

        info!("OFP-1 capability negotiation successful: {:?}", result);
        self.negotiation_result = Some(result);

        Ok(())
    }

    /// Update FFB engine with negotiated capabilities
    fn update_ffb_engine_capabilities(&self) -> Result<()> {
        if let Some(ref result) = self.negotiation_result {
            let device_caps = DeviceCapabilities {
                supports_pid: false, // OFP-1 is raw torque, not PID
                supports_raw_torque: true,
                max_torque_nm: result.max_torque_nm,
                min_period_us: 1_000_000 / result.effective_update_rate_hz,
                has_health_stream: true,
                supports_interlock: result.supports_high_torque,
            };

            if let Ok(mut engine) = self.ffb_engine.lock() {
                engine.set_device_capabilities(device_caps)?;
            }
        }

        Ok(())
    }

    /// Start integration thread
    fn start_integration_thread(&mut self) -> Result<()> {
        let ffb_engine = self.ffb_engine.clone();
        let command_receiver = self.command_receiver.clone();
        let health_sender = self.health_sender.clone();
        let device_path = self.device.device_path().to_string();

        // We can't move the device into the thread due to trait object limitations
        // In a real implementation, this would use a different approach

        let handle = thread::spawn(move || {
            Self::integration_loop(ffb_engine, command_receiver, health_sender, device_path);
        });

        self.integration_thread = Some(handle);
        Ok(())
    }

    /// Main integration loop (runs in separate thread)
    fn integration_loop(
        _ffb_engine: Arc<Mutex<FfbEngine>>,
        command_receiver: Receiver<TorqueCommand>,
        health_sender: Sender<HealthUpdate>,
        device_path: String,
    ) {
        let mut sequence = 0u16;

        while let Ok(command) = command_receiver.recv() {
            // Process torque command
            sequence = sequence.wrapping_add(1);

            // In real implementation, this would send the command to the actual device
            debug!(
                "Processing OFP-1 torque command: {:?} (seq: {})",
                command, sequence
            );

            // Simulate health report generation
            let health_report = Self::create_simulated_health_report(sequence, &command);

            let health_update = HealthUpdate {
                health_report,
                _timestamp: Instant::now(),
            };

            if health_sender.try_send(health_update).is_err() {
                debug!("Health update channel full, dropping report");
            }

            // Small delay to prevent busy waiting
            thread::sleep(Duration::from_millis(1));
        }

        info!("OFP-1 integration loop ended for device: {}", device_path);
    }

    /// Create simulated health report for testing
    fn create_simulated_health_report(
        sequence: u16,
        command: &TorqueCommand,
    ) -> HealthStatusReport {
        let mut status_flags = StatusFlags::new();
        status_flags.set_flag(StatusFlags::READY);

        if command.enable {
            status_flags.set_flag(StatusFlags::TORQUE_ENABLED);
        }

        if command.high_torque {
            status_flags.set_flag(StatusFlags::HIGH_TORQUE_ACTIVE);
        }

        if command.interlock_ok {
            status_flags.set_flag(StatusFlags::INTERLOCK_OK);
        }

        if command.emergency_stop {
            status_flags.set_flag(StatusFlags::EMERGENCY_STOP);
        }

        // Convert torque to protocol value
        let torque_protocol = utils::torque_nm_to_protocol(command.torque_nm, 15.0);

        HealthStatusReport {
            report_id: report_ids::HEALTH_STATUS,
            sequence,
            status_flags,
            current_torque: torque_protocol,
            temperature_dc: 250, // 25.0°C
            current_ma: 1000,    // 1A
            encoder_position: 0,
            uptime_s: 60,
            reserved: [0; 2],
        }
    }

    /// Send torque command to device
    pub fn send_torque_command(
        &mut self,
        torque_nm: f32,
        enable: bool,
        high_torque: bool,
        interlock_ok: bool,
    ) -> Result<()> {
        if !self.is_running {
            return Err(FfbError::DeviceError {
                message: "Integration not running".to_string(),
            });
        }

        // Record command for stability tracking
        self.stability_tracker.record_command(torque_nm);

        let command = TorqueCommand {
            torque_nm,
            enable,
            high_torque,
            interlock_ok,
            emergency_stop: false,
        };

        self.command_sender
            .try_send(command)
            .map_err(|e| FfbError::DeviceError {
                message: format!("Command queue full: {}", e),
            })?;

        Ok(())
    }

    /// Trigger emergency stop
    pub fn trigger_emergency_stop(&mut self) -> Result<()> {
        let command = TorqueCommand {
            torque_nm: 0.0,
            enable: false,
            high_torque: false,
            interlock_ok: false,
            emergency_stop: true,
        };

        self.command_sender
            .try_send(command)
            .map_err(|e| FfbError::DeviceError {
                message: format!("Emergency stop failed: {}", e),
            })?;

        Ok(())
    }

    /// Process health updates
    pub fn process_health_updates(&mut self) -> Result<Vec<HealthStatusReport>> {
        let mut health_reports = Vec::new();

        while let Ok(health_update) = self.health_receiver.try_recv() {
            // Update health monitor
            if let Err(e) = self
                .health_monitor
                .update_health(health_update.health_report.clone())
            {
                warn!("Health monitor error: {}", e);

                // Notify FFB engine of fault
                if let Ok(mut engine) = self.ffb_engine.lock() {
                    // Convert OFP-1 error to FFB fault type
                    let fault_type = match e {
                        Ofp1Error::DeviceFault { .. } => crate::FaultType::DeviceTimeout,
                        Ofp1Error::HealthTimeout { .. } => crate::FaultType::DeviceTimeout,
                        _ => crate::FaultType::UsbStall,
                    };

                    if let Err(ffb_err) = engine.process_fault(fault_type) {
                        error!("FFB engine fault processing failed: {}", ffb_err);
                    }
                }
            }

            health_reports.push(health_update.health_report);
        }

        // Check for health timeout
        if let Err(e) = self.health_monitor.check_timeout() {
            warn!("Health timeout detected: {}", e);
        }

        Ok(health_reports)
    }

    /// Get negotiation result
    pub fn negotiation_result(&self) -> Option<&Ofp1NegotiationResult> {
        self.negotiation_result.as_ref()
    }

    /// Get health monitor
    pub fn health_monitor(&self) -> &Ofp1HealthMonitor {
        &self.health_monitor
    }

    /// Get torque path stability metrics
    pub fn get_stability_metrics(&self) -> TorqueStabilityMetrics {
        self.stability_tracker.get_metrics()
    }

    /// Check if device is connected and healthy
    pub fn is_healthy(&self) -> bool {
        self.device.is_connected() && self.health_monitor.is_health_current()
    }

    /// Get device path
    pub fn device_path(&self) -> &str {
        self.device.device_path()
    }
}

impl Drop for Ofp1FfbIntegration {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FfbConfig, FfbEngine, FfbMode};
    use flight_virtual::ofp1_emulator::Ofp1Emulator;
    use std::sync::{Arc, Mutex};

    fn create_test_integration() -> Ofp1FfbIntegration {
        let config = FfbConfig {
            max_torque_nm: 15.0,
            fault_timeout_ms: 50,
            interlock_required: true,
            mode: FfbMode::RawTorque,
            device_path: Some("/dev/test0".to_string()),
        };

        let ffb_engine = Arc::new(Mutex::new(FfbEngine::new(config).unwrap()));
        let device = Box::new(Ofp1Emulator::new("/dev/test0".to_string()));

        Ofp1FfbIntegration::new(ffb_engine, device).unwrap()
    }

    #[test]
    fn test_integration_creation() {
        let integration = create_test_integration();
        assert_eq!(integration.device_path(), "/dev/test0");
        assert!(!integration.is_running);
    }

    #[test]
    fn test_capability_negotiation() {
        let mut integration = create_test_integration();
        assert!(integration.negotiate_capabilities().is_ok());
        assert!(integration.negotiation_result.is_some());

        let result = integration.negotiation_result().unwrap();
        assert_eq!(result.max_torque_nm, 20.0); // Emulator reports 20 Nm (20000 mNm)
        assert!(result.effective_update_rate_hz > 0);
    }

    #[test]
    fn test_torque_stability_tracker() {
        let mut tracker = TorquePathStabilityTracker::new();

        // Add stable torque values
        for _ in 0..20 {
            tracker.record_command(5.0);
        }

        let metrics = tracker.get_metrics();
        assert!(metrics.is_stable);
        assert_eq!(metrics.mean_torque, 5.0);
        assert!(metrics.std_deviation < 0.01);

        // Add unstable values
        for i in 0..10 {
            tracker.record_command(i as f32);
        }

        let metrics = tracker.get_metrics();
        assert!(!metrics.is_stable);
        assert!(metrics.std_deviation > 0.1);
    }

    #[test]
    fn test_integration_start_stop() {
        let mut integration = create_test_integration();

        assert!(integration.start().is_ok());
        assert!(integration.is_running);

        integration.stop();
        assert!(!integration.is_running);
    }

    #[test]
    fn test_torque_command_sending() {
        let mut integration = create_test_integration();
        integration.start().unwrap();

        assert!(
            integration
                .send_torque_command(5.0, true, false, true)
                .is_ok()
        );

        let metrics = integration.get_stability_metrics();
        assert_eq!(metrics.sample_count, 1);
        assert_eq!(metrics.mean_torque, 5.0);

        integration.stop();
    }

    #[test]
    fn test_emergency_stop() {
        let mut integration = create_test_integration();
        integration.start().unwrap();

        assert!(integration.trigger_emergency_stop().is_ok());

        integration.stop();
    }
}
