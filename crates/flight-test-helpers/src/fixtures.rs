// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Common fixture builders.

use flight_device_common::DeviceId;
use std::time::Duration;

/// Shared timing configuration used by helper utilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestConfig {
    pub timeout: Duration,
    pub poll_interval: Duration,
}

/// Builder for `TestConfig`.
#[derive(Debug, Clone)]
pub struct TestConfigBuilder {
    config: TestConfig,
}

impl Default for TestConfigBuilder {
    fn default() -> Self {
        Self {
            config: TestConfig {
                timeout: Duration::from_secs(2),
                poll_interval: Duration::from_millis(10),
            },
        }
    }
}

impl TestConfigBuilder {
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.config.timeout = timeout;
        self
    }

    pub fn with_poll_interval(mut self, poll_interval: Duration) -> Self {
        self.config.poll_interval = poll_interval;
        self
    }

    pub fn build(self) -> TestConfig {
        self.config
    }
}

/// Builder for synthetic device IDs used by tests.
#[derive(Debug, Clone)]
pub struct TestDeviceBuilder {
    vendor_id: u16,
    product_id: u16,
    serial_number: Option<String>,
    device_path: String,
}

impl Default for TestDeviceBuilder {
    fn default() -> Self {
        Self {
            vendor_id: 0x1234,
            product_id: 0x5678,
            serial_number: Some("TEST0001".to_string()),
            device_path: "test://device/0".to_string(),
        }
    }
}

impl TestDeviceBuilder {
    pub fn with_vid_pid(mut self, vendor_id: u16, product_id: u16) -> Self {
        self.vendor_id = vendor_id;
        self.product_id = product_id;
        self
    }

    pub fn with_serial(mut self, serial_number: impl Into<String>) -> Self {
        self.serial_number = Some(serial_number.into());
        self
    }

    pub fn with_path(mut self, device_path: impl Into<String>) -> Self {
        self.device_path = device_path.into();
        self
    }

    pub fn build(self) -> DeviceId {
        DeviceId::new(
            self.vendor_id,
            self.product_id,
            self.serial_number,
            self.device_path,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{TestConfigBuilder, TestDeviceBuilder};
    use std::time::Duration;

    #[test]
    fn test_config_builder_defaults() {
        let config = TestConfigBuilder::default().build();
        assert_eq!(config.timeout, Duration::from_secs(2));
        assert_eq!(config.poll_interval, Duration::from_millis(10));
    }

    #[test]
    fn test_config_builder_customization() {
        let config = TestConfigBuilder::default()
            .with_timeout(Duration::from_millis(500))
            .with_poll_interval(Duration::from_millis(25))
            .build();
        assert_eq!(config.timeout, Duration::from_millis(500));
        assert_eq!(config.poll_interval, Duration::from_millis(25));
    }

    #[test]
    fn test_device_builder_default_vid_pid() {
        let id = TestDeviceBuilder::default().build();
        assert_eq!(id.vendor_id, 0x1234);
        assert_eq!(id.product_id, 0x5678);
    }

    #[test]
    fn test_device_builder_customization() {
        let id = TestDeviceBuilder::default()
            .with_vid_pid(0x06a3, 0x0762)
            .with_serial("X52")
            .with_path("hid://x52")
            .build();

        assert_eq!(id.vid_pid(), "06a3:0762");
        assert_eq!(id.serial_number.as_deref(), Some("X52"));
        assert_eq!(id.device_path, "hid://x52");
    }
}
