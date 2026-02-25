// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID device info and device handle.

use crate::HidError;

/// Static metadata about a discovered HID device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HidDeviceInfo {
    pub vendor_id: u16,
    pub product_id: u16,
    pub product_string: String,
    pub manufacturer_string: String,
    pub serial_number: String,
    pub usage_page: u16,
    pub usage: u16,
    pub location_id: u32,
}

/// An open HID device handle.
///
/// On macOS this wraps an `IOHIDDeviceRef` with an associated `CFRunLoop`
/// queue. On other platforms every method returns
/// [`HidError::UnsupportedPlatform`].
#[derive(Debug)]
pub struct HidDevice {
    #[cfg(not(target_os = "macos"))]
    _phantom: (),
    #[cfg(target_os = "macos")]
    // TODO: hold IOHIDDeviceRef and associated queue
    _phantom: (),
}

impl HidDevice {
    /// Open a device given its discovery info.
    ///
    /// # Errors
    ///
    /// Returns [`HidError::UnsupportedPlatform`] on non-macOS.
    pub fn open(_info: &HidDeviceInfo) -> Result<Self, HidError> {
        #[cfg(target_os = "macos")]
        {
            // IOHIDDeviceOpen not yet wired.
            Err(HidError::OpenFailed { code: -1 })
        }
        #[cfg(not(target_os = "macos"))]
        Err(HidError::UnsupportedPlatform)
    }

    /// Read an input report into `buf`, blocking for up to `timeout_ms`.
    ///
    /// Returns the number of bytes read on success.
    pub fn read_report(&self, _buf: &mut [u8], _timeout_ms: u32) -> Result<usize, HidError> {
        #[cfg(target_os = "macos")]
        {
            Err(HidError::ReadTimeout {
                timeout_ms: _timeout_ms,
            })
        }
        #[cfg(not(target_os = "macos"))]
        Err(HidError::UnsupportedPlatform)
    }

    /// Write an output report.
    pub fn write_report(&self, _buf: &[u8]) -> Result<(), HidError> {
        #[cfg(target_os = "macos")]
        {
            Err(HidError::WriteFailed { code: -1 })
        }
        #[cfg(not(target_os = "macos"))]
        Err(HidError::UnsupportedPlatform)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_info() -> HidDeviceInfo {
        HidDeviceInfo {
            vendor_id: 0x044F,
            product_id: 0xB67B,
            product_string: "T.Flight HOTAS 4".into(),
            manufacturer_string: "Thrustmaster".into(),
            serial_number: "".into(),
            usage_page: 0x01,
            usage: 0x04,
            location_id: 0,
        }
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_open_unsupported_on_non_macos() {
        let result = HidDevice::open(&dummy_info());
        assert!(matches!(result, Err(HidError::UnsupportedPlatform)));
    }

    #[test]
    fn test_device_info_clone() {
        let info = dummy_info();
        let info2 = info.clone();
        assert_eq!(info.vendor_id, info2.vendor_id);
    }
}
