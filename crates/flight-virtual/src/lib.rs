#![cfg_attr(
    test,
    allow(
        unused_imports,
        unused_variables,
        unused_mut,
        unused_assignments,
        unused_parens,
        dead_code
    )
)]
// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Virtual HID device implementation for CI testing
//!
//! Provides loopback HID devices that simulate real hardware
//! without requiring physical devices for testing.

use flight_device_common::{DeviceHealth, DeviceId, DeviceManager};
use parking_lot::Mutex;
use std::sync::Arc;
use std::{error::Error, fmt};

pub mod device;
pub mod loopback;
pub mod ofp1_emulator;
pub mod perf_gate;

pub use device::{DeviceType, VirtualDevice, VirtualDeviceConfig};
pub use loopback::{HidReport, LoopbackHid};
pub use ofp1_emulator::{EmulatorFaultType, EmulatorStatistics, Ofp1Emulator, Ofp1EmulatorConfig};
pub use perf_gate::{PerfGate, PerfGateConfig, PerfResult};

#[cfg(test)]
mod integration_tests;

/// Virtual device manager for testing
pub struct VirtualDeviceManager {
    devices: Mutex<Vec<Arc<VirtualDevice>>>,
    loopback: Option<LoopbackHid>,
}

/// Errors returned by `VirtualDeviceManager` when using shared manager APIs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VirtualDeviceManagerError {
    DuplicateDevice(DeviceId),
    DeviceNotFound(DeviceId),
}

impl fmt::Display for VirtualDeviceManagerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateDevice(id) => write!(f, "device already registered: {id}"),
            Self::DeviceNotFound(id) => write!(f, "device not found: {id}"),
        }
    }
}

impl Error for VirtualDeviceManagerError {}

impl VirtualDeviceManager {
    /// Create new virtual device manager
    pub fn new() -> Self {
        Self {
            devices: Mutex::new(Vec::new()),
            loopback: None,
        }
    }

    /// Create virtual device with specified configuration
    pub fn create_device(&self, config: VirtualDeviceConfig) -> Arc<VirtualDevice> {
        let device = Arc::new(VirtualDevice::new(config));
        self.devices.lock().push(device.clone());
        device
    }

    /// Enable HID loopback for testing
    pub fn enable_loopback(&mut self) -> &mut LoopbackHid {
        self.loopback = Some(LoopbackHid::new());
        self.loopback
            .as_mut()
            .expect("Loopback was just initialized")
    }

    /// Get all virtual devices
    pub fn devices(&self) -> Vec<Arc<VirtualDevice>> {
        self.devices.lock().clone()
    }

    /// Run performance gate test
    pub fn run_perf_gate(&self, config: PerfGateConfig) -> PerfResult {
        let mut gate = PerfGate::new(config);
        gate.run()
    }

    fn find_device(&self, id: &DeviceId) -> Option<Arc<VirtualDevice>> {
        self.devices
            .lock()
            .iter()
            .find(|device| device.device_id() == *id)
            .cloned()
    }
}

impl Default for VirtualDeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceManager for VirtualDeviceManager {
    type Device = Arc<VirtualDevice>;
    type Error = VirtualDeviceManagerError;

    fn enumerate_devices(&mut self) -> Result<Vec<Self::Device>, Self::Error> {
        Ok(self.devices())
    }

    fn register_device(&mut self, device: Self::Device) -> Result<(), Self::Error> {
        let id = device.device_id();
        let mut devices = self.devices.lock();
        if devices.iter().any(|existing| existing.device_id() == id) {
            return Err(VirtualDeviceManagerError::DuplicateDevice(id));
        }

        devices.push(device);
        Ok(())
    }

    fn unregister_device(&mut self, id: &DeviceId) -> Result<(), Self::Error> {
        let mut devices = self.devices.lock();
        if let Some(index) = devices.iter().position(|device| device.device_id() == *id) {
            devices.remove(index);
            return Ok(());
        }

        Err(VirtualDeviceManagerError::DeviceNotFound(id.clone()))
    }

    fn get_device_health(&self, id: &DeviceId) -> Option<DeviceHealth> {
        let device = self.find_device(id)?;
        if !device.is_connected() {
            return Some(DeviceHealth::Failed {
                error: "virtual device disconnected".to_string(),
            });
        }

        let health = device.get_health();
        if health.temperature_c >= 80.0 {
            return Some(DeviceHealth::Degraded {
                reason: format!("temperature high: {:.1}C", health.temperature_c),
            });
        }

        if !(4.75..=5.25).contains(&health.voltage_v) {
            return Some(DeviceHealth::Degraded {
                reason: format!("voltage out of range: {:.2}V", health.voltage_v),
            });
        }

        Some(DeviceHealth::Healthy)
    }
}
