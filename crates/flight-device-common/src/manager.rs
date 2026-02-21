// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Shared device manager traits.

use crate::{DeviceHealth, DeviceId};

/// Trait for devices that can expose a stable `DeviceId`.
pub trait IdentifiedDevice {
    fn device_id(&self) -> DeviceId;
}

/// Common management contract for device layers.
pub trait DeviceManager {
    type Device;
    type Error;

    fn enumerate_devices(&mut self) -> Result<Vec<Self::Device>, Self::Error>;
    fn register_device(&mut self, device: Self::Device) -> Result<(), Self::Error>;
    fn unregister_device(&mut self, id: &DeviceId) -> Result<(), Self::Error>;
    fn get_device_health(&self, id: &DeviceId) -> Option<DeviceHealth>;
}
