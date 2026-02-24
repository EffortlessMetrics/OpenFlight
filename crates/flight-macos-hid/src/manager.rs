// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! IOKit HID Manager — device enumeration.

use crate::{HidDeviceInfo, HidError};

/// Matching criteria applied to `IOHIDManager`.
#[derive(Debug, Clone, Default)]
pub struct DeviceMatchCriteria {
    pub usage_page: Option<u16>,
    pub usage: Option<u16>,
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
}

/// IOKit HID Manager wrapper.
///
/// Enumerates HID devices attached to the system. On macOS this creates an
/// `IOHIDManager` via `IOHIDManagerCreate`. On other platforms the manager
/// compiles but all methods return [`HidError::UnsupportedPlatform`].
///
/// # Example
///
/// ```no_run
/// use flight_macos_hid::{HidManager, HidError};
///
/// let mut mgr = HidManager::new().expect("manager");
/// mgr.set_device_matching(0x01, 0x04);
/// mgr.open().expect("open");
/// for d in mgr.devices() { println!("{:04x}:{:04x}", d.vendor_id, d.product_id); }
/// ```
pub struct HidManager {
    criteria: DeviceMatchCriteria,
    #[cfg(not(target_os = "macos"))]
    _phantom: (),
    #[cfg(target_os = "macos")]
    // TODO: hold IOHIDManagerRef + CFRunLoopRef
    _phantom: (),
}

impl HidManager {
    /// Create a new (unopened) HID manager.
    ///
    /// Returns [`HidError::UnsupportedPlatform`] on non-macOS.
    pub fn new() -> Result<Self, HidError> {
        #[cfg(target_os = "macos")]
        {
            // TODO: IOHIDManagerCreate(kCFAllocatorDefault, kIOHIDOptionsTypeNone)
            unimplemented!("IOKit HID Manager not yet wired — see flight-macos-hid TODO")
        }
        #[cfg(not(target_os = "macos"))]
        Ok(Self {
            criteria: DeviceMatchCriteria::default(),
            _phantom: (),
        })
    }

    /// Restrict enumeration to devices matching a HID usage page and usage.
    pub fn set_device_matching(&mut self, usage_page: u16, usage: u16) {
        self.criteria.usage_page = Some(usage_page);
        self.criteria.usage = Some(usage);
    }

    /// Restrict enumeration to a specific vendor/product.
    pub fn set_vendor_product(&mut self, vendor_id: u16, product_id: u16) {
        self.criteria.vendor_id = Some(vendor_id);
        self.criteria.product_id = Some(product_id);
    }

    /// Open the manager and start device enumeration.
    ///
    /// On macOS: `IOHIDManagerSetDeviceMatchingMultiple` + `IOHIDManagerOpen`
    /// + schedule on run loop.
    pub fn open(&mut self) -> Result<(), HidError> {
        #[cfg(target_os = "macos")]
        {
            // TODO: IOHIDManagerOpen(mgr_ref, kIOHIDOptionsTypeNone)
            //       IOHIDManagerScheduleWithRunLoop(mgr_ref, CFRunLoopGetCurrent(), kCFRunLoopDefaultMode)
            unimplemented!()
        }
        #[cfg(not(target_os = "macos"))]
        Err(HidError::UnsupportedPlatform)
    }

    /// Return metadata for all currently matching devices.
    pub fn devices(&self) -> Vec<HidDeviceInfo> {
        #[cfg(target_os = "macos")]
        {
            // TODO: IOHIDManagerCopyDevices -> iterate CFSet -> IOHIDDeviceGetProperty
            unimplemented!()
        }
        #[cfg(not(target_os = "macos"))]
        Vec::new()
    }

    /// Current matching criteria.
    pub fn criteria(&self) -> &DeviceMatchCriteria {
        &self.criteria
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_manager_ok_on_non_macos() {
        // On non-macOS, new() succeeds (returns stub manager)
        #[cfg(not(target_os = "macos"))]
        {
            let mgr = HidManager::new().unwrap();
            assert!(mgr.devices().is_empty());
        }
    }

    #[test]
    fn test_criteria_stored() {
        #[cfg(not(target_os = "macos"))]
        {
            let mut mgr = HidManager::new().unwrap();
            mgr.set_device_matching(0x01, 0x04);
            assert_eq!(mgr.criteria().usage_page, Some(0x01));
            assert_eq!(mgr.criteria().usage, Some(0x04));
        }
    }

    #[test]
    fn test_open_unsupported_on_non_macos() {
        #[cfg(not(target_os = "macos"))]
        {
            let mut mgr = HidManager::new().unwrap();
            assert_eq!(mgr.open(), Err(HidError::UnsupportedPlatform));
        }
    }

    #[test]
    fn test_vendor_product_stored() {
        #[cfg(not(target_os = "macos"))]
        {
            let mut mgr = HidManager::new().unwrap();
            mgr.set_vendor_product(0x044F, 0xB67B);
            assert_eq!(mgr.criteria().vendor_id, Some(0x044F));
            assert_eq!(mgr.criteria().product_id, Some(0xB67B));
        }
    }
}
