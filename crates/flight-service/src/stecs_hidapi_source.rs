// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Real HID-backed VKB STECS report source, gated behind `feature = "stecs-hidapi"`.

use std::collections::{HashMap, HashSet};
use std::ffi::CStr;

use flight_hid_support::HidDeviceInfo;
use flight_hid_support::device_support::{
    VKB_STECS_LEFT_SPACE_MINI_PID, VKB_STECS_LEFT_SPACE_MINI_PLUS_PID,
    VKB_STECS_LEFT_SPACE_STANDARD_PID, VKB_STECS_RIGHT_SPACE_MINI_PID,
    VKB_STECS_RIGHT_SPACE_MINI_PLUS_PID, VKB_STECS_RIGHT_SPACE_STANDARD_PID, VKB_VENDOR_ID,
};
use hidapi::{HidApi, HidDevice};
use tracing::{debug, warn};

use crate::stecs_runtime::VkbStecsReportSource;

/// Timeout for each non-blocking `read_timeout` call (milliseconds).
const READ_TIMEOUT_MS: i32 = 0;
/// Maximum HID report size read per poll.
const MAX_REPORT_SIZE: usize = 64;

/// Real `VkbStecsReportSource` backed by `hidapi`.
pub struct HidApiVkbStecsReportSource {
    api: HidApi,
    open_devices: HashMap<String, HidDevice>,
}

impl HidApiVkbStecsReportSource {
    /// Create a new source, initializing the `hidapi` context.
    pub fn new() -> Result<Self, String> {
        let api = HidApi::new().map_err(|e| format!("hidapi init failed: {e}"))?;
        Ok(Self {
            api,
            open_devices: HashMap::new(),
        })
    }

    fn path_key(path: &CStr) -> String {
        match path.to_str() {
            Ok(path) => path.to_owned(),
            Err(_) => {
                let bytes = path.to_bytes();
                let mut hex = String::with_capacity(bytes.len() * 2);
                for &byte in bytes {
                    use std::fmt::Write;
                    let _ = write!(&mut hex, "{byte:02x}");
                }
                format!("hidpath:{hex}")
            }
        }
    }

    fn build_device_path(info: &hidapi::DeviceInfo) -> String {
        let mut path = Self::path_key(info.path());
        let interface = info.interface_number();
        if interface >= 0 {
            path.push_str(&format!("#if{interface}"));
        }
        path
    }

    fn build_info(info: &hidapi::DeviceInfo) -> HidDeviceInfo {
        HidDeviceInfo {
            vendor_id: info.vendor_id(),
            product_id: info.product_id(),
            serial_number: info.serial_number().map(str::to_owned),
            manufacturer: info.manufacturer_string().map(str::to_owned),
            product_name: info.product_string().map(str::to_owned),
            device_path: Self::build_device_path(info),
            usage_page: info.usage_page(),
            usage: info.usage(),
            report_descriptor: None,
        }
    }

    fn is_stecs_pid(product_id: u16) -> bool {
        matches!(
            product_id,
            VKB_STECS_LEFT_SPACE_MINI_PID
                | VKB_STECS_RIGHT_SPACE_MINI_PID
                | VKB_STECS_LEFT_SPACE_MINI_PLUS_PID
                | VKB_STECS_RIGHT_SPACE_MINI_PLUS_PID
                | VKB_STECS_LEFT_SPACE_STANDARD_PID
                | VKB_STECS_RIGHT_SPACE_STANDARD_PID
        )
    }

    fn is_stecs(info: &hidapi::DeviceInfo) -> bool {
        info.vendor_id() == VKB_VENDOR_ID && Self::is_stecs_pid(info.product_id())
    }
}

impl VkbStecsReportSource for HidApiVkbStecsReportSource {
    fn list_devices(&mut self) -> Vec<HidDeviceInfo> {
        if let Err(error) = self.api.refresh_devices() {
            warn!(target: "input_hotas_vkb_stecs", "hidapi refresh failed: {error}");
            return Vec::new();
        }

        let mut found = Vec::new();
        let mut present_paths = HashSet::new();

        for info in self
            .api
            .device_list()
            .filter(|device| Self::is_stecs(device))
        {
            let device_info = Self::build_info(info);
            present_paths.insert(device_info.device_path.clone());

            if self.open_devices.contains_key(&device_info.device_path) {
                found.push(device_info);
                continue;
            }

            match self.api.open_path(info.path()) {
                Ok(handle) => {
                    debug!(
                        target: "input_hotas_vkb_stecs",
                        path = %device_info.device_path,
                        "opened HID interface"
                    );
                    self.open_devices
                        .insert(device_info.device_path.clone(), handle);
                }
                Err(error) => {
                    warn!(
                        target: "input_hotas_vkb_stecs",
                        path = %device_info.device_path,
                        "failed to open HID interface: {error}"
                    );
                }
            }

            found.push(device_info);
        }

        self.open_devices
            .retain(|path, _| present_paths.contains(path));

        found
    }

    fn read_report(&mut self, device_path: &str) -> Result<Option<Vec<u8>>, String> {
        let Some(handle) = self.open_devices.get(device_path) else {
            return Ok(None);
        };

        let mut buffer = [0u8; MAX_REPORT_SIZE];
        match handle.read_timeout(&mut buffer, READ_TIMEOUT_MS) {
            Ok(0) => Ok(None),
            Ok(bytes) => Ok(Some(buffer[..bytes].to_vec())),
            Err(error) => Err(format!("HID read error on {device_path}: {error}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_stecs_pid_matches_known_variants() {
        assert!(HidApiVkbStecsReportSource::is_stecs_pid(
            VKB_STECS_LEFT_SPACE_MINI_PID
        ));
        assert!(HidApiVkbStecsReportSource::is_stecs_pid(
            VKB_STECS_RIGHT_SPACE_MINI_PLUS_PID
        ));
        assert!(HidApiVkbStecsReportSource::is_stecs_pid(
            VKB_STECS_RIGHT_SPACE_STANDARD_PID
        ));
        assert!(!HidApiVkbStecsReportSource::is_stecs_pid(0x0001));
    }

    /// Verify construction succeeds (requires HID subsystem — skipped in CI).
    /// Run with: `cargo test -p flight-service --features stecs-hidapi`
    #[test]
    #[ignore = "requires HID subsystem; run manually with --features stecs-hidapi"]
    fn test_hidapi_source_constructs() {
        let source = HidApiVkbStecsReportSource::new();
        assert!(
            source.is_ok(),
            "HidApiVkbStecsReportSource::new failed: {:?}",
            source.err()
        );
    }
}
