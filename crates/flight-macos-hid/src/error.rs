// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Error type for the macOS HID layer.

use thiserror::Error;

/// Errors returned by the macOS HID layer.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum HidError {
    /// The operation is not supported on the current platform.
    /// All public APIs return this on non-macOS builds.
    #[error("macOS HID layer is not supported on this platform")]
    UnsupportedPlatform,

    /// The IOKit HID Manager could not be created.
    #[error("failed to create IOHIDManager: {reason}")]
    ManagerCreateFailed { reason: String },

    /// Opening the HID manager or a device failed.
    #[error("open failed with IOKit return code {code:#010x}")]
    OpenFailed { code: i32 },

    /// A report read timed out.
    #[error("report read timed out after {timeout_ms}ms")]
    ReadTimeout { timeout_ms: u32 },

    /// A report write failed.
    #[error("report write failed with IOKit return code {code:#010x}")]
    WriteFailed { code: i32 },

    /// Device was disconnected.
    #[error("HID device disconnected")]
    DeviceDisconnected,

    /// Failed to register a device callback with IOKit.
    #[error("callback registration failed: {reason}")]
    CallbackRegistrationFailed { reason: String },

    /// Failed to query a device property via IOHIDDeviceGetProperty.
    #[error("failed to query device property: {property}")]
    PropertyQueryFailed { property: String },

    /// The manager is not yet open.
    #[error("manager is not open")]
    NotOpen,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let e = HidError::UnsupportedPlatform;
        assert!(e.to_string().contains("not supported"));
    }

    #[test]
    fn test_open_failed_display() {
        let e = HidError::OpenFailed {
            code: 0xe00002c5u32 as i32,
        };
        assert!(e.to_string().contains("0xe00002c5"));
    }

    #[test]
    fn test_callback_failed_display() {
        let e = HidError::CallbackRegistrationFailed {
            reason: "null manager".into(),
        };
        assert!(e.to_string().contains("callback registration failed"));
        assert!(e.to_string().contains("null manager"));
    }

    #[test]
    fn test_property_query_failed_display() {
        let e = HidError::PropertyQueryFailed {
            property: "VendorID".into(),
        };
        assert!(e.to_string().contains("VendorID"));
    }

    #[test]
    fn test_not_open_display() {
        let e = HidError::NotOpen;
        assert!(e.to_string().contains("not open"));
    }

    #[test]
    fn test_error_clone_eq() {
        let e1 = HidError::DeviceDisconnected;
        let e2 = e1.clone();
        assert_eq!(e1, e2);
    }
}
