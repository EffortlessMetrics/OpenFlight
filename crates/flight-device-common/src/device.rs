// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Shared device identifiers.

use std::fmt;

/// Stable identifier for a hardware or virtual device instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DeviceId {
    pub vendor_id: u16,
    pub product_id: u16,
    pub serial_number: Option<String>,
    pub device_path: String,
}

impl DeviceId {
    /// Create a new device identifier.
    pub fn new(
        vendor_id: u16,
        product_id: u16,
        serial_number: Option<String>,
        device_path: impl Into<String>,
    ) -> Self {
        Self {
            vendor_id,
            product_id,
            serial_number,
            device_path: device_path.into(),
        }
    }

    /// Create a synthetic ID for virtual devices.
    pub fn virtual_device(serial: impl Into<String>) -> Self {
        let serial = serial.into();
        Self {
            vendor_id: 0,
            product_id: 0,
            device_path: format!("virtual://{serial}"),
            serial_number: Some(serial),
        }
    }

    /// Return `vid:pid` in lowercase hex.
    pub fn vid_pid(&self) -> String {
        format!("{:04x}:{:04x}", self.vendor_id, self.product_id)
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(serial) = &self.serial_number {
            write!(f, "{}#{} ({})", self.vid_pid(), serial, self.device_path)
        } else {
            write!(f, "{} ({})", self.vid_pid(), self.device_path)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DeviceId;

    #[test]
    fn test_vid_pid_format() {
        let id = DeviceId::new(0x06a3, 0x0762, None, "hid://x52");
        assert_eq!(id.vid_pid(), "06a3:0762");
    }

    #[test]
    fn test_virtual_device_builder() {
        let id = DeviceId::virtual_device("VIRT001");
        assert_eq!(id.device_path, "virtual://VIRT001");
        assert_eq!(id.serial_number.as_deref(), Some("VIRT001"));
    }
}
