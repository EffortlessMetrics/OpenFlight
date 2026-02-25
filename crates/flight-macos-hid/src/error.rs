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
}
