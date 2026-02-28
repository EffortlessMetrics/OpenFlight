// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! macOS HID device layer for Flight Hub.
//!
//! This crate provides a platform-consistent API for HID device enumeration
//! and report I/O. On **macOS** the implementation is backed by IOKit's
//! `IOHIDManager` with real `IOHIDManagerCreate`, device matching/removal
//! callbacks, `IOHIDDeviceGetProperty` queries, and report-based input via
//! `IOHIDDeviceRegisterInputReportCallback`.
//!
//! On **other platforms** all types compile with a mock/stub backend that
//! supports device injection and report simulation for cross-platform testing.
//!
//! # Usage
//!
//! ```
//! use flight_macos_hid::{MacHidManager, HidError};
//!
//! # #[cfg(not(target_os = "macos"))]
//! # {
//! let mut mgr = MacHidManager::new().expect("create manager");
//! mgr.set_device_matching(0x01, 0x04); // joysticks
//! mgr.open().expect("open");
//! for dev in mgr.devices() {
//!     println!("{:04x}:{:04x} – {}", dev.vendor_id, dev.product_id, dev.product_string);
//! }
//! # }
//! ```

pub mod callback;
pub mod device;
pub mod error;
#[cfg(target_os = "macos")]
pub mod ffi;
pub mod manager;
pub mod timing;
pub mod traits;

pub use callback::{HotplugEventQueue, InputReport, InputReportQueue};
pub use device::{HidDevice, HidDeviceInfo, MacHidDevice};
pub use error::HidError;
pub use manager::{DeviceMatchCriteria, HidManager, MacHidManager};
pub use timing::MacosClock;
pub use traits::{MacDeviceScanner, MacHotplugEvent, MacHotplugMonitor, MacInputReportReader};

#[cfg(test)]
mod tests {
    /// AC-50.7: WHEN IOKit dependencies are declared THEN they SHALL appear
    /// only under `[target.'cfg(target_os = "macos")'.dependencies]` in Cargo.toml.
    #[test]
    fn test_iokit_deps_scoped_to_macos_target() {
        let cargo_toml = include_str!("../Cargo.toml");

        // Only IOKit *crate* names that could appear as dependency entries.
        // These are the actual crate names, not keywords or comments.
        let iokit_crates = ["io-kit-sys", "core-foundation", "objc", "block"];

        // Find where the macOS target section begins so we can exclude it.
        let macos_section_start = cargo_toml
            .find("[target.'cfg(target_os = \"macos\")'.dependencies]")
            .unwrap_or(usize::MAX);

        // Everything *before* the macOS target section must not contain IOKit
        // crate names as dependency keys (i.e., at the start of a non-comment line).
        let before_macos = if macos_section_start == usize::MAX {
            cargo_toml
        } else {
            &cargo_toml[..macos_section_start]
        };

        for crate_name in iokit_crates {
            // A dependency entry looks like `crate-name` at the start of a line
            // (possibly with leading whitespace), not inside a comment.
            for line in before_macos.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with('#') {
                    continue;
                }
                // Check for `crate-name =` or `crate-name.` patterns
                let is_dep = trimmed.starts_with(&format!("{} =", crate_name))
                    || trimmed.starts_with(&format!("{}.", crate_name));
                assert!(
                    !is_dep,
                    "IOKit dependency '{}' found outside macOS target section in line: {}",
                    crate_name, trimmed
                );
            }
        }
    }

    #[test]
    fn test_all_public_types_accessible() {
        // Verify that the public API surface is importable.
        let _: fn() -> Result<super::MacHidManager, super::HidError> = super::MacHidManager::new;
        let _ = super::DeviceMatchCriteria::default();
        let _ = super::HotplugEventQueue::new();
        let _ = super::InputReportQueue::new();
        let _ = super::MacosClock::new();
    }
}
