#![cfg_attr(test, allow(unused_imports, unused_variables, unused_mut, unused_assignments, unused_parens, dead_code))]

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Virtual HID device implementation for CI testing
//!
//! Provides loopback HID devices that simulate real hardware
//! without requiring physical devices for testing.

use std::sync::Arc;
use parking_lot::Mutex;

pub mod device;
pub mod loopback;
pub mod perf_gate;
pub mod ofp1_emulator;

pub use device::{VirtualDevice, VirtualDeviceConfig, DeviceType};
pub use loopback::{LoopbackHid, HidReport};
pub use perf_gate::{PerfGate, PerfGateConfig, PerfResult};
pub use ofp1_emulator::{Ofp1Emulator, Ofp1EmulatorConfig, EmulatorFaultType, EmulatorStatistics};

#[cfg(test)]
mod integration_tests;

/// Virtual device manager for testing
pub struct VirtualDeviceManager {
    devices: Mutex<Vec<Arc<VirtualDevice>>>,
    loopback: Option<LoopbackHid>,
}

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
        self.loopback.as_mut().expect("Loopback was just initialized")
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
}

impl Default for VirtualDeviceManager {
    fn default() -> Self {
        Self::new()
    }
}
