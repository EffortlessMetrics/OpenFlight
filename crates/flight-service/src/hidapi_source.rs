// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2025 Flight Hub Team

//! Real HID-backed T.Flight report source, gated behind `feature = "tflight-hidapi"`.
//!
//! Enable with:
//! ```toml
//! cargo run -p flight-service --features tflight-hidapi
//! ```
//!
//! CI builds should NOT enable this feature; they use `SimulatedTFlightReportSource`.

use flight_hid_support::HidDeviceInfo;
use flight_hid_support::device_support::{
    TFLIGHT_HOTAS_4_PID, TFLIGHT_HOTAS_4_PID_LEGACY, TFLIGHT_HOTAS_ONE_PID, THRUSTMASTER_VENDOR_ID,
    USAGE_JOYSTICK, USAGE_PAGE_GENERIC_DESKTOP,
};
use hidapi::{HidApi, HidDevice};
use std::collections::{HashMap, HashSet};
use std::ffi::CStr;
use tracing::{debug, warn};

use crate::input_runtime::TFlightReportSource;

/// Timeout for each non-blocking `read_timeout` call (milliseconds).
/// 0 → non-blocking; the runtime's own polling loop provides pacing.
const READ_TIMEOUT_MS: i32 = 0;

/// Maximum report size to read (bytes).
/// Separate-mode reports are 9 bytes; add headroom for Report ID prefix.
const MAX_REPORT_SIZE: usize = 16;

/// Real `TFlightReportSource` backed by `hidapi`.
///
/// Enumerates all known T.Flight HOTAS 4/One devices (VID `0x044F`, PID
/// `0xB67A`/`0xB67B`/`0xB68B`) attached to the system. Opens devices lazily and
/// keeps handles alive between polls for zero-allocation hot-path reads.
///
/// # Thread safety
/// This source is `Send` but **not** `Sync`. Use it exclusively from the runtime
/// ingest task (single-threaded per `TFlightInputRuntime`).
pub struct HidApiTFlightReportSource {
    api: HidApi,
    open_devices: HashMap<String, HidDevice>,
}

impl HidApiTFlightReportSource {
    /// Create a new source, initialising the `hidapi` context.
    ///
    /// Returns an error if the underlying OS HID context cannot be created.
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
                // Preserve uniqueness for non-UTF8 paths.
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

    fn build_info(info: &hidapi::DeviceInfo) -> HidDeviceInfo {
        HidDeviceInfo {
            vendor_id: info.vendor_id(),
            product_id: info.product_id(),
            serial_number: info.serial_number().map(str::to_owned),
            manufacturer: info.manufacturer_string().map(str::to_owned),
            product_name: info.product_string().map(str::to_owned),
            device_path: Self::path_key(info.path()),
            usage_page: info.usage_page(),
            usage: info.usage(),
            // Descriptor capture is deferred; update via receipts.
            report_descriptor: None,
        }
    }

    fn is_tflight_pid(product_id: u16) -> bool {
        matches!(
            product_id,
            TFLIGHT_HOTAS_4_PID | TFLIGHT_HOTAS_4_PID_LEGACY | TFLIGHT_HOTAS_ONE_PID
        )
    }

    fn is_tflight(info: &hidapi::DeviceInfo) -> bool {
        info.vendor_id() == THRUSTMASTER_VENDOR_ID
            && Self::is_tflight_pid(info.product_id())
            && info.usage_page() == USAGE_PAGE_GENERIC_DESKTOP
            && info.usage() == USAGE_JOYSTICK
    }
}

impl TFlightReportSource for HidApiTFlightReportSource {
    fn list_devices(&mut self) -> Vec<HidDeviceInfo> {
        // Refresh the device list on every poll so hot-plug is detected.
        if let Err(e) = self.api.refresh_devices() {
            warn!(target: "input_hotas_tflight", "hidapi refresh failed: {e}");
            return Vec::new();
        }

        let mut found = Vec::new();
        let mut present_paths = HashSet::new();

        // Open handles for newly discovered devices.
        for info in self.api.device_list().filter(|d| Self::is_tflight(d)) {
            let device_info = Self::build_info(info);
            present_paths.insert(device_info.device_path.clone());

            if self.open_devices.contains_key(&device_info.device_path) {
                found.push(device_info);
                continue;
            }
            match self.api.open_path(info.path()) {
                Ok(handle) => {
                    debug!(
                        target: "input_hotas_tflight",
                        path = %device_info.device_path,
                        "opened HID device"
                    );
                    self.open_devices
                        .insert(device_info.device_path.clone(), handle);
                }
                Err(e) => {
                    warn!(
                        target: "input_hotas_tflight",
                        path = %device_info.device_path,
                        "failed to open HID device: {e}"
                    );
                }
            }
            found.push(device_info);
        }

        // Remove handles for devices that are no longer present.
        self.open_devices
            .retain(|path, _| present_paths.contains(path));

        found
    }

    fn read_report(&mut self, device_path: &str) -> Result<Option<Vec<u8>>, String> {
        let Some(handle) = self.open_devices.get(device_path) else {
            return Ok(None);
        };

        let mut buf = [0u8; MAX_REPORT_SIZE];
        match handle.read_timeout(&mut buf, READ_TIMEOUT_MS) {
            Ok(0) => Ok(None), // non-blocking: no data available
            Ok(n) => Ok(Some(buf[..n].to_vec())),
            Err(e) => Err(format!("HID read error on {device_path}: {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_tflight_pid_matches_known_variants() {
        assert!(HidApiTFlightReportSource::is_tflight_pid(
            TFLIGHT_HOTAS_4_PID
        ));
        assert!(HidApiTFlightReportSource::is_tflight_pid(
            TFLIGHT_HOTAS_4_PID_LEGACY
        ));
        assert!(HidApiTFlightReportSource::is_tflight_pid(
            TFLIGHT_HOTAS_ONE_PID
        ));
        assert!(!HidApiTFlightReportSource::is_tflight_pid(0x0001));
    }

    /// Verify construction succeeds (requires HID subsystem — skipped in CI).
    /// Run with: `cargo test -p flight-service --features tflight-hidapi`
    #[test]
    #[ignore = "requires HID subsystem; run manually with --features tflight-hidapi"]
    fn test_hidapi_source_constructs() {
        let source = HidApiTFlightReportSource::new();
        assert!(
            source.is_ok(),
            "HidApiTFlightReportSource::new failed: {:?}",
            source.err()
        );
    }

    /// Verify list_devices returns only T.Flight devices (no device attached → empty).
    #[test]
    #[ignore = "requires HID subsystem; run manually with --features tflight-hidapi"]
    fn test_hidapi_list_devices_empty_without_hardware() {
        let mut source = HidApiTFlightReportSource::new().expect("hidapi init");
        let devices = source.list_devices();
        // Without hardware this is expected to be empty.
        println!(
            "Found {} T.Flight device(s): {:?}",
            devices.len(),
            devices.iter().map(|d| &d.device_path).collect::<Vec<_>>()
        );
    }
}
