// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! macOS HID device layer for Flight Hub.
//!
//! This crate provides a platform-consistent API for HID device enumeration
//! and report I/O. On **macOS** the implementation is backed by IOKit's
//! `IOHIDManager`. On other platforms all methods return
//! [`HidError::UnsupportedPlatform`] so the workspace compiles everywhere
//! while the real port lives behind `#[cfg(target_os = "macos")]`.
//!
//! # Usage
//!
//! ```no_run
//! use flight_macos_hid::{HidManager, HidError};
//!
//! fn main() -> Result<(), HidError> {
//!     let mut mgr = HidManager::new()?;
//!     // Match joysticks: usage page 0x01, usage 0x04
//!     mgr.set_device_matching(0x01, 0x04);
//!     mgr.open()?;
//!     for dev in mgr.devices() {
//!         println!("{:04x}:{:04x} – {}", dev.vendor_id, dev.product_id, dev.product_string);
//!     }
//!     Ok(())
//! }
//! ```

pub mod device;
pub mod error;
pub mod manager;
pub mod timing;

pub use device::{HidDevice, HidDeviceInfo};
pub use error::HidError;
pub use manager::HidManager;
pub use timing::MacosClock;

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
}
