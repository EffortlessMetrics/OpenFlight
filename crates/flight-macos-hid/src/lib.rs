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
