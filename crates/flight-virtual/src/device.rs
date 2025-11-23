// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Virtual device implementation
//!
//! Simulates flight control devices for testing without hardware

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

/// Type of virtual device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceType {
    /// Joystick with multiple axes
    Joystick { axes: u8 },
    /// Force feedback stick
    ForceFeedback { axes: u8, max_torque_nm: f32 },
    /// Throttle quadrant
    Throttle { levers: u8 },
    /// Rudder pedals
    Rudder,
    /// Flight panel with LEDs and switches
    Panel { leds: u8, switches: u8 },
}

/// Configuration for virtual device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualDeviceConfig {
    /// Device name
    pub name: String,
    /// Device type and capabilities
    pub device_type: DeviceType,
    /// Vendor ID (simulated)
    pub vid: u16,
    /// Product ID (simulated)
    pub pid: u16,
    /// Serial number
    pub serial: String,
    /// Simulate device latency (microseconds)
    pub latency_us: u32,
    /// Simulate occasional packet loss
    pub packet_loss_rate: f64,
}

impl Default for VirtualDeviceConfig {
    fn default() -> Self {
        Self {
            name: "Virtual Flight Stick".to_string(),
            device_type: DeviceType::Joystick { axes: 3 },
            vid: 0x1234,
            pid: 0x5678,
            serial: "VIRT001".to_string(),
            latency_us: 100,
            packet_loss_rate: 0.0,
        }
    }
}

/// Virtual device state
#[derive(Debug)]
pub struct DeviceState {
    /// Axis values (normalized -1.0 to 1.0)
    pub axes: Vec<f32>,
    /// Button states (bit mask)
    pub buttons: u32,
    /// LED states (bit mask)
    pub leds: u32,
    /// Last update timestamp
    pub last_update: Instant,
}

impl Default for DeviceState {
    fn default() -> Self {
        Self {
            axes: vec![0.0; 8], // Support up to 8 axes
            buttons: 0,
            leds: 0,
            last_update: Instant::now(),
        }
    }
}

/// Virtual HID device
pub struct VirtualDevice {
    config: VirtualDeviceConfig,
    state: Mutex<DeviceState>,
    stats: DeviceStats,
    connected: AtomicBool,
}

/// Device statistics
#[derive(Debug)]
pub struct DeviceStats {
    /// Total input reports generated
    pub input_reports: AtomicU64,
    /// Total output reports received
    pub output_reports: AtomicU64,
    /// Simulated packet losses
    pub packet_losses: AtomicU64,
    /// Total bytes transferred
    pub bytes_transferred: AtomicU64,
}

impl VirtualDevice {
    /// Create new virtual device
    pub fn new(config: VirtualDeviceConfig) -> Self {
        Self {
            config,
            state: Mutex::new(DeviceState::default()),
            stats: DeviceStats {
                input_reports: AtomicU64::new(0),
                output_reports: AtomicU64::new(0),
                packet_losses: AtomicU64::new(0),
                bytes_transferred: AtomicU64::new(0),
            },
            connected: AtomicBool::new(true),
        }
    }

    /// Get device configuration
    pub fn config(&self) -> &VirtualDeviceConfig {
        &self.config
    }

    /// Check if device is connected
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    /// Simulate device disconnect
    pub fn disconnect(&self) {
        self.connected.store(false, Ordering::Relaxed);
    }

    /// Simulate device reconnect
    pub fn reconnect(&self) {
        self.connected.store(true, Ordering::Relaxed);
    }

    /// Update axis value
    pub fn set_axis(&self, axis: usize, value: f32) {
        let mut state = self.state.lock();
        if axis < state.axes.len() {
            state.axes[axis] = value.clamp(-1.0, 1.0);
            state.last_update = Instant::now();
        }
    }

    /// Update button state
    pub fn set_button(&self, button: u8, pressed: bool) {
        let mut state = self.state.lock();
        if pressed {
            state.buttons |= 1 << button;
        } else {
            state.buttons &= !(1 << button);
        }
        state.last_update = Instant::now();
    }

    /// Get current device state
    pub fn get_state(&self) -> DeviceState {
        let state = self.state.lock();
        DeviceState {
            axes: state.axes.clone(),
            buttons: state.buttons,
            leds: state.leds,
            last_update: state.last_update,
        }
    }

    /// Generate HID input report
    pub fn generate_input_report(&self) -> Option<Vec<u8>> {
        if !self.is_connected() {
            return None;
        }

        // Simulate packet loss
        if self.config.packet_loss_rate > 0.0
            && rand::random::<f64>() < self.config.packet_loss_rate
        {
            self.stats.packet_losses.fetch_add(1, Ordering::Relaxed);
            return None;
        }

        let state = self.state.lock();
        let mut report = Vec::new();

        // Report ID
        report.push(0x01);

        // Axes (16-bit values)
        for &axis in &state.axes {
            let value = ((axis + 1.0) * 32767.5) as u16;
            report.push((value & 0xFF) as u8);
            report.push((value >> 8) as u8);
        }

        // Buttons (32-bit)
        let buttons = state.buttons;
        report.push((buttons & 0xFF) as u8);
        report.push(((buttons >> 8) & 0xFF) as u8);
        report.push(((buttons >> 16) & 0xFF) as u8);
        report.push(((buttons >> 24) & 0xFF) as u8);

        self.stats.input_reports.fetch_add(1, Ordering::Relaxed);
        self.stats
            .bytes_transferred
            .fetch_add(report.len() as u64, Ordering::Relaxed);

        Some(report)
    }

    /// Process HID output report (LEDs, force feedback, etc.)
    pub fn process_output_report(&self, report: &[u8]) -> bool {
        if !self.is_connected() || report.is_empty() {
            return false;
        }

        let mut state = self.state.lock();

        match report[0] {
            0x02 => {
                // LED control report
                if report.len() >= 2 {
                    state.leds = report[1] as u32;
                }
            }
            0x03 => {
                // Force feedback report (for FFB devices)
                // Process force feedback commands
            }
            _ => {
                // Unknown report type
                return false;
            }
        }

        self.stats.output_reports.fetch_add(1, Ordering::Relaxed);
        self.stats
            .bytes_transferred
            .fetch_add(report.len() as u64, Ordering::Relaxed);

        true
    }

    /// Get device statistics
    pub fn get_stats(&self) -> DeviceStatsSnapshot {
        DeviceStatsSnapshot {
            input_reports: self.stats.input_reports.load(Ordering::Relaxed),
            output_reports: self.stats.output_reports.load(Ordering::Relaxed),
            packet_losses: self.stats.packet_losses.load(Ordering::Relaxed),
            bytes_transferred: self.stats.bytes_transferred.load(Ordering::Relaxed),
        }
    }

    /// Reset device statistics
    pub fn reset_stats(&self) {
        self.stats.input_reports.store(0, Ordering::Relaxed);
        self.stats.output_reports.store(0, Ordering::Relaxed);
        self.stats.packet_losses.store(0, Ordering::Relaxed);
        self.stats.bytes_transferred.store(0, Ordering::Relaxed);
    }

    /// Simulate device health data
    pub fn get_health(&self) -> DeviceHealth {
        DeviceHealth {
            temperature_c: 45.0 + (rand::random_f32() - 0.5) * 10.0,
            voltage_v: 5.0 + (rand::random_f32() - 0.5) * 0.2,
            current_ma: 150.0 + (rand::random_f32() - 0.5) * 50.0,
            uptime_ms: self.state.lock().last_update.elapsed().as_millis() as u64,
        }
    }
}

/// Snapshot of device statistics
#[derive(Debug, Clone)]
pub struct DeviceStatsSnapshot {
    pub input_reports: u64,
    pub output_reports: u64,
    pub packet_losses: u64,
    pub bytes_transferred: u64,
}

/// Device health information
#[derive(Debug, Clone)]
pub struct DeviceHealth {
    pub temperature_c: f32,
    pub voltage_v: f32,
    pub current_ma: f32,
    pub uptime_ms: u64,
}

// Placeholder for rand functionality (would use rand crate in real implementation)
mod rand {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};

    pub fn random<T>() -> T
    where
        T: From<f64>,
    {
        let mut hasher = DefaultHasher::new();
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .hash(&mut hasher);
        let hash = hasher.finish();
        T::from((hash as f64) / (u64::MAX as f64))
    }

    pub fn random_f32() -> f32 {
        let mut hasher = DefaultHasher::new();
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .hash(&mut hasher);
        let hash = hasher.finish();
        (hash as f64 / u64::MAX as f64) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_virtual_device_creation() {
        let config = VirtualDeviceConfig::default();
        let device = VirtualDevice::new(config);

        assert!(device.is_connected());
        assert_eq!(device.config().name, "Virtual Flight Stick");
    }

    #[test]
    fn test_axis_control() {
        let device = VirtualDevice::new(VirtualDeviceConfig::default());

        device.set_axis(0, 0.5);
        device.set_axis(1, -0.8);

        let state = device.get_state();
        assert_eq!(state.axes[0], 0.5);
        assert_eq!(state.axes[1], -0.8);
    }

    #[test]
    fn test_button_control() {
        let device = VirtualDevice::new(VirtualDeviceConfig::default());

        device.set_button(0, true);
        device.set_button(2, true);

        let state = device.get_state();
        assert_eq!(state.buttons & 0x01, 0x01); // Button 0
        assert_eq!(state.buttons & 0x04, 0x04); // Button 2
        assert_eq!(state.buttons & 0x02, 0x00); // Button 1 not pressed
    }

    #[test]
    fn test_hid_report_generation() {
        let device = VirtualDevice::new(VirtualDeviceConfig::default());

        device.set_axis(0, 0.0); // Center
        device.set_axis(1, 1.0); // Max
        device.set_axis(2, -1.0); // Min
        device.set_button(0, true);

        let report = device.generate_input_report().unwrap();

        // Should have report ID + axes + buttons
        assert!(report.len() > 10);
        assert_eq!(report[0], 0x01); // Report ID

        let stats = device.get_stats();
        assert_eq!(stats.input_reports, 1);
    }

    #[test]
    fn test_disconnect_behavior() {
        let device = VirtualDevice::new(VirtualDeviceConfig::default());

        assert!(device.generate_input_report().is_some());

        device.disconnect();
        assert!(!device.is_connected());
        assert!(device.generate_input_report().is_none());

        device.reconnect();
        assert!(device.is_connected());
        assert!(device.generate_input_report().is_some());
    }
}
