// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! IOKit HID Manager — device enumeration and hot-plug monitoring.

use crate::HidError;
use crate::callback::HotplugEventQueue;
use crate::device::HidDeviceInfo;
use crate::traits::{MacDeviceScanner, MacHotplugEvent, MacHotplugMonitor};

/// Matching criteria applied to `IOHIDManager`.
#[derive(Debug, Clone, Default)]
pub struct DeviceMatchCriteria {
    pub usage_page: Option<u16>,
    pub usage: Option<u16>,
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
}

// ═══════════════════════════════════════════════════════════════════════════
// MacHidManager — macOS IOKit implementation
// ═══════════════════════════════════════════════════════════════════════════

/// IOKit HID Manager wrapper.
///
/// On **macOS** this creates a real `IOHIDManagerRef` via `IOHIDManagerCreate`,
/// registers device-matching and removal callbacks, and schedules on the
/// current run loop. Devices are enumerated via `IOHIDManagerCopyDevices` and
/// property queries use `IOHIDDeviceGetProperty`.
///
/// On **other platforms** the manager provides a mock backend with injectable
/// devices and events, suitable for cross-platform testing.
#[derive(Debug)]
pub struct MacHidManager {
    criteria: DeviceMatchCriteria,
    opened: bool,
    event_queue: HotplugEventQueue,

    // -- macOS: real IOKit handles --
    #[cfg(target_os = "macos")]
    manager_ref: crate::ffi::IOHIDManagerRef,

    // -- non-macOS: mock device list --
    #[cfg(not(target_os = "macos"))]
    mock_devices: Vec<HidDeviceInfo>,
}

// ── macOS implementation ─────────────────────────────────────────────────

#[cfg(target_os = "macos")]
impl MacHidManager {
    /// Create a new (unopened) HID manager backed by IOKit.
    pub fn new() -> Result<Self, HidError> {
        use crate::ffi;
        use std::ptr;

        let mgr = unsafe { ffi::IOHIDManagerCreate(ptr::null(), ffi::K_IOHID_OPTIONS_TYPE_NONE) };
        if mgr.is_null() {
            return Err(HidError::ManagerCreateFailed {
                reason: "IOHIDManagerCreate returned null".into(),
            });
        }
        Ok(Self {
            criteria: DeviceMatchCriteria::default(),
            opened: false,
            event_queue: HotplugEventQueue::new(),
            manager_ref: mgr,
        })
    }

    /// Open the manager: set matching, register callbacks, start enumeration.
    pub fn open(&mut self) -> Result<(), HidError> {
        use crate::ffi;
        use core_foundation::array::CFArray;
        use core_foundation::base::TCFType;
        use core_foundation::dictionary::CFDictionary;
        use core_foundation::number::CFNumber;
        use core_foundation::runloop::CFRunLoop;
        use core_foundation::string::CFString;
        use std::os::raw::c_void;

        // Build matching dictionary from criteria.
        let mut dict_keys: Vec<CFString> = Vec::new();
        let mut dict_vals: Vec<CFNumber> = Vec::new();
        if let Some(up) = self.criteria.usage_page {
            dict_keys.push(CFString::new(ffi::K_IOHID_DEVICE_USAGE_PAGE_KEY));
            dict_vals.push(CFNumber::from(i32::from(up)));
        }
        if let Some(u) = self.criteria.usage {
            dict_keys.push(CFString::new(ffi::K_IOHID_DEVICE_USAGE_KEY));
            dict_vals.push(CFNumber::from(i32::from(u)));
        }
        if let Some(vid) = self.criteria.vendor_id {
            dict_keys.push(CFString::new(ffi::K_IOHID_VENDOR_ID_KEY));
            dict_vals.push(CFNumber::from(i32::from(vid)));
        }
        if let Some(pid) = self.criteria.product_id {
            dict_keys.push(CFString::new(ffi::K_IOHID_PRODUCT_ID_KEY));
            dict_vals.push(CFNumber::from(i32::from(pid)));
        }

        if !dict_keys.is_empty() {
            let key_refs: Vec<_> = dict_keys.iter().map(|k| k.as_CFType()).collect();
            let val_refs: Vec<_> = dict_vals.iter().map(|v| v.as_CFType()).collect();
            let dict = CFDictionary::from_CFType_pairs(&key_refs, &val_refs);
            let arr = CFArray::from_CFTypes(&[dict.as_CFType()]);
            unsafe {
                ffi::IOHIDManagerSetDeviceMatchingMultiple(
                    self.manager_ref,
                    arr.as_concrete_TypeRef(),
                );
            }
        }

        // Register attach/detach callbacks.
        let ctx = Box::into_raw(Box::new(self.event_queue.clone())) as *mut c_void;
        unsafe {
            ffi::IOHIDManagerRegisterDeviceMatchingCallback(
                self.manager_ref,
                device_attach_callback,
                ctx,
            );
            ffi::IOHIDManagerRegisterDeviceRemovalCallback(
                self.manager_ref,
                device_detach_callback,
                ctx,
            );
        }

        // Schedule on current run loop.
        let run_loop = CFRunLoop::get_current();
        let mode = unsafe { core_foundation_sys::runloop::kCFRunLoopDefaultMode };
        unsafe {
            ffi::IOHIDManagerScheduleWithRunLoop(
                self.manager_ref,
                run_loop.as_concrete_TypeRef(),
                mode,
            );
        }

        // Open the manager.
        let ret =
            unsafe { ffi::IOHIDManagerOpen(self.manager_ref, ffi::K_IOHID_OPTIONS_TYPE_NONE) };
        if ret != ffi::K_IO_RETURN_SUCCESS {
            return Err(HidError::OpenFailed { code: ret });
        }
        self.opened = true;
        Ok(())
    }

    /// Return metadata for all currently matching devices.
    pub fn devices(&self) -> Vec<HidDeviceInfo> {
        use crate::ffi;
        use core_foundation::base::TCFType;
        use core_foundation::set::CFSet;

        if !self.opened {
            return Vec::new();
        }
        let set_ref = unsafe { ffi::IOHIDManagerCopyDevices(self.manager_ref) };
        if set_ref.is_null() {
            return Vec::new();
        }
        let set: CFSet<*const std::os::raw::c_void> =
            unsafe { CFSet::wrap_under_create_rule(set_ref) };
        let mut result = Vec::new();
        for device_ptr in set.iter() {
            let device = *device_ptr as ffi::IOHIDDeviceRef;
            if let Some(info) = device_info_from_ref(device) {
                result.push(info);
            }
        }
        result
    }
}

/// Extract `HidDeviceInfo` from an `IOHIDDeviceRef` by querying properties.
#[cfg(target_os = "macos")]
fn device_info_from_ref(device: crate::ffi::IOHIDDeviceRef) -> Option<HidDeviceInfo> {
    use crate::ffi;
    unsafe {
        let vid = ffi::get_device_int_property(device, ffi::K_IOHID_VENDOR_ID_KEY)? as u16;
        let pid = ffi::get_device_int_property(device, ffi::K_IOHID_PRODUCT_ID_KEY)? as u16;
        let product =
            ffi::get_device_string_property(device, ffi::K_IOHID_PRODUCT_KEY).unwrap_or_default();
        let manufacturer = ffi::get_device_string_property(device, ffi::K_IOHID_MANUFACTURER_KEY)
            .unwrap_or_default();
        let serial = ffi::get_device_string_property(device, ffi::K_IOHID_SERIAL_NUMBER_KEY)
            .unwrap_or_default();
        let location =
            ffi::get_device_int_property(device, ffi::K_IOHID_LOCATION_ID_KEY).unwrap_or(0) as u32;
        let usage_page = ffi::get_device_int_property(device, ffi::K_IOHID_PRIMARY_USAGE_PAGE_KEY)
            .unwrap_or(0) as u16;
        let usage = ffi::get_device_int_property(device, ffi::K_IOHID_PRIMARY_USAGE_KEY)
            .unwrap_or(0) as u16;

        Some(HidDeviceInfo {
            vendor_id: vid,
            product_id: pid,
            product_string: product,
            manufacturer_string: manufacturer,
            serial_number: serial,
            usage_page,
            usage,
            location_id: location,
        })
    }
}

/// IOKit callback: device attached.
#[cfg(target_os = "macos")]
unsafe extern "C" fn device_attach_callback(
    context: *mut std::os::raw::c_void,
    _result: crate::ffi::IOReturn,
    _sender: *mut std::os::raw::c_void,
    device: crate::ffi::IOHIDDeviceRef,
) {
    if context.is_null() || device.is_null() {
        return;
    }
    let queue = unsafe { &*(context as *const HotplugEventQueue) };
    if let Some(info) = device_info_from_ref(device) {
        queue.push(MacHotplugEvent::Attached(info));
    }
}

/// IOKit callback: device detached.
#[cfg(target_os = "macos")]
unsafe extern "C" fn device_detach_callback(
    context: *mut std::os::raw::c_void,
    _result: crate::ffi::IOReturn,
    _sender: *mut std::os::raw::c_void,
    device: crate::ffi::IOHIDDeviceRef,
) {
    use crate::ffi;
    if context.is_null() || device.is_null() {
        return;
    }
    let queue = unsafe { &*(context as *const HotplugEventQueue) };
    let vid = unsafe { ffi::get_device_int_property(device, ffi::K_IOHID_VENDOR_ID_KEY) }
        .unwrap_or(0) as u16;
    let pid = unsafe { ffi::get_device_int_property(device, ffi::K_IOHID_PRODUCT_ID_KEY) }
        .unwrap_or(0) as u16;
    let loc = unsafe { ffi::get_device_int_property(device, ffi::K_IOHID_LOCATION_ID_KEY) }
        .unwrap_or(0) as u32;
    queue.push(MacHotplugEvent::Detached {
        vendor_id: vid,
        product_id: pid,
        location_id: loc,
    });
}

// ── non-macOS mock implementation ────────────────────────────────────────

#[cfg(not(target_os = "macos"))]
impl MacHidManager {
    /// Create a new mock HID manager (non-macOS).
    pub fn new() -> Result<Self, HidError> {
        Ok(Self {
            criteria: DeviceMatchCriteria::default(),
            opened: false,
            event_queue: HotplugEventQueue::new(),
            mock_devices: Vec::new(),
        })
    }

    /// Open the mock manager. Always succeeds.
    pub fn open(&mut self) -> Result<(), HidError> {
        self.opened = true;
        Ok(())
    }

    /// Return metadata for all mock devices.
    pub fn devices(&self) -> Vec<HidDeviceInfo> {
        if !self.opened {
            return Vec::new();
        }
        self.mock_devices.clone()
    }

    /// Inject a mock device (test helper, non-macOS only).
    ///
    /// Pushes an `Attached` event into the event queue.
    pub fn inject_device(&mut self, info: HidDeviceInfo) {
        self.mock_devices.push(info.clone());
        self.event_queue.push(MacHotplugEvent::Attached(info));
    }

    /// Remove a mock device by VID/PID (test helper, non-macOS only).
    ///
    /// Pushes a `Detached` event into the event queue.
    pub fn remove_device(&mut self, vendor_id: u16, product_id: u16) {
        if let Some(pos) = self
            .mock_devices
            .iter()
            .position(|d| d.vendor_id == vendor_id && d.product_id == product_id)
        {
            let info = self.mock_devices.remove(pos);
            self.event_queue.push(MacHotplugEvent::Detached {
                vendor_id,
                product_id,
                location_id: info.location_id,
            });
        }
    }

    /// Remove all mock devices.
    pub fn clear_devices(&mut self) {
        self.mock_devices.clear();
    }
}

// ── Shared (all platforms) ───────────────────────────────────────────────

impl MacHidManager {
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

    /// Current matching criteria.
    pub fn criteria(&self) -> &DeviceMatchCriteria {
        &self.criteria
    }

    /// Whether the manager has been opened.
    pub fn is_open(&self) -> bool {
        self.opened
    }

    /// Access the underlying event queue.
    pub fn event_queue(&self) -> &HotplugEventQueue {
        &self.event_queue
    }
}

// ── Trait implementations ────────────────────────────────────────────────

impl MacDeviceScanner for MacHidManager {
    fn enumerate(&mut self) -> Vec<HidDeviceInfo> {
        self.devices()
    }
}

impl MacHotplugMonitor for MacHidManager {
    fn poll_events(&mut self) -> Vec<MacHotplugEvent> {
        self.event_queue.drain()
    }
}

/// Backward-compatible alias.
pub type HidManager = MacHidManager;

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_info() -> HidDeviceInfo {
        HidDeviceInfo {
            vendor_id: 0x044F,
            product_id: 0xB67B,
            product_string: "T.Flight HOTAS 4".into(),
            manufacturer_string: "Thrustmaster".into(),
            serial_number: String::new(),
            usage_page: 0x01,
            usage: 0x04,
            location_id: 0x1400,
        }
    }

    fn vkb_info() -> HidDeviceInfo {
        HidDeviceInfo {
            vendor_id: 0x231D,
            product_id: 0x0200,
            product_string: "Gladiator NXT EVO".into(),
            manufacturer_string: "VKB-Sim".into(),
            serial_number: "VKB0001".into(),
            usage_page: 0x01,
            usage: 0x04,
            location_id: 0x2800,
        }
    }

    // ── Construction ─────────────────────────────────────────────────

    #[test]
    fn test_new_manager() {
        #[cfg(not(target_os = "macos"))]
        {
            let mgr = MacHidManager::new().unwrap();
            assert!(!mgr.is_open());
            assert!(mgr.devices().is_empty());
        }
    }

    // ── Criteria ─────────────────────────────────────────────────────

    #[test]
    fn test_criteria_stored() {
        #[cfg(not(target_os = "macos"))]
        {
            let mut mgr = MacHidManager::new().unwrap();
            mgr.set_device_matching(0x01, 0x04);
            assert_eq!(mgr.criteria().usage_page, Some(0x01));
            assert_eq!(mgr.criteria().usage, Some(0x04));
        }
    }

    #[test]
    fn test_vendor_product_stored() {
        #[cfg(not(target_os = "macos"))]
        {
            let mut mgr = MacHidManager::new().unwrap();
            mgr.set_vendor_product(0x044F, 0xB67B);
            assert_eq!(mgr.criteria().vendor_id, Some(0x044F));
            assert_eq!(mgr.criteria().product_id, Some(0xB67B));
        }
    }

    // ── Open / close ─────────────────────────────────────────────────

    #[test]
    fn test_open_mock() {
        #[cfg(not(target_os = "macos"))]
        {
            let mut mgr = MacHidManager::new().unwrap();
            assert!(!mgr.is_open());
            mgr.open().unwrap();
            assert!(mgr.is_open());
        }
    }

    #[test]
    fn test_devices_empty_before_open() {
        #[cfg(not(target_os = "macos"))]
        {
            let mut mgr = MacHidManager::new().unwrap();
            mgr.inject_device(sample_info());
            // Not yet opened — devices() returns empty
            assert!(mgr.devices().is_empty());
        }
    }

    // ── Mock device injection ────────────────────────────────────────

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_inject_and_enumerate() {
        let mut mgr = MacHidManager::new().unwrap();
        mgr.open().unwrap();

        mgr.inject_device(sample_info());
        mgr.inject_device(vkb_info());

        let devs = mgr.devices();
        assert_eq!(devs.len(), 2);
        assert_eq!(devs[0].vendor_id, 0x044F);
        assert_eq!(devs[1].vendor_id, 0x231D);
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_remove_device() {
        let mut mgr = MacHidManager::new().unwrap();
        mgr.open().unwrap();
        mgr.inject_device(sample_info());
        mgr.inject_device(vkb_info());
        assert_eq!(mgr.devices().len(), 2);

        mgr.remove_device(0x044F, 0xB67B);
        assert_eq!(mgr.devices().len(), 1);
        assert_eq!(mgr.devices()[0].vendor_id, 0x231D);
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_clear_devices() {
        let mut mgr = MacHidManager::new().unwrap();
        mgr.open().unwrap();
        mgr.inject_device(sample_info());
        mgr.inject_device(vkb_info());
        mgr.clear_devices();
        assert!(mgr.devices().is_empty());
    }

    // ── Trait: MacDeviceScanner ──────────────────────────────────────

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_scanner_trait() {
        let mut mgr = MacHidManager::new().unwrap();
        mgr.open().unwrap();
        mgr.inject_device(sample_info());
        let devs = MacDeviceScanner::enumerate(&mut mgr);
        assert_eq!(devs.len(), 1);
    }

    // ── Trait: MacHotplugMonitor ─────────────────────────────────────

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_hotplug_events_on_inject() {
        let mut mgr = MacHidManager::new().unwrap();
        mgr.inject_device(sample_info());

        let events = MacHotplugMonitor::poll_events(&mut mgr);
        assert_eq!(events.len(), 1);
        assert!(events[0].is_attach());
        assert_eq!(events[0].vendor_id(), 0x044F);
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_hotplug_events_on_remove() {
        let mut mgr = MacHidManager::new().unwrap();
        mgr.inject_device(sample_info());
        // drain attach event
        let _ = MacHotplugMonitor::poll_events(&mut mgr);

        mgr.remove_device(0x044F, 0xB67B);
        let events = MacHotplugMonitor::poll_events(&mut mgr);
        assert_eq!(events.len(), 1);
        assert!(events[0].is_detach());
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_hotplug_attach_detach_sequence() {
        let mut mgr = MacHidManager::new().unwrap();
        mgr.inject_device(sample_info());
        mgr.inject_device(vkb_info());
        mgr.remove_device(0x044F, 0xB67B);

        let events = MacHotplugMonitor::poll_events(&mut mgr);
        assert_eq!(events.len(), 3);
        assert!(events[0].is_attach());
        assert!(events[1].is_attach());
        assert!(events[2].is_detach());
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_poll_events_drains() {
        let mut mgr = MacHidManager::new().unwrap();
        mgr.inject_device(sample_info());
        let _ = MacHotplugMonitor::poll_events(&mut mgr);
        // Second poll should be empty
        let events = MacHotplugMonitor::poll_events(&mut mgr);
        assert!(events.is_empty());
    }

    // ── Type alias ───────────────────────────────────────────────────

    #[test]
    fn test_hid_manager_alias() {
        // HidManager is a type alias for MacHidManager
        #[cfg(not(target_os = "macos"))]
        {
            let _mgr: HidManager = MacHidManager::new().unwrap();
        }
    }
}
