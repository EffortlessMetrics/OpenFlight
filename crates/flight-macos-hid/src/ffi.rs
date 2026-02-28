// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Raw IOKit HID FFI declarations and safe property-query wrappers.
//!
//! This module is only compiled on macOS (`cfg(target_os = "macos")`).
//! It declares the subset of IOKit HID Manager functions used by
//! [`MacHidManager`](crate::MacHidManager) and
//! [`MacHidDevice`](crate::MacHidDevice).

#![allow(non_snake_case, non_upper_case_globals, clippy::upper_case_acronyms)]

use core_foundation::base::TCFType;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_foundation_sys::array::CFArrayRef;
use core_foundation_sys::base::{CFAllocatorRef, CFGetTypeID, CFTypeRef};
use core_foundation_sys::number::CFNumberGetTypeID;
use core_foundation_sys::runloop::{CFRunLoopRef, CFRunLoopSourceRef};
use core_foundation_sys::set::CFSetRef;
use core_foundation_sys::string::{CFStringGetTypeID, CFStringRef};
use std::os::raw::c_void;

// ── IOKit type aliases ───────────────────────────────────────────────────

pub type IOHIDManagerRef = *mut c_void;
pub type IOHIDDeviceRef = *mut c_void;
pub type IOReturn = i32;
pub type IOOptionBits = u32;

pub const K_IO_RETURN_SUCCESS: IOReturn = 0;
pub const K_IOHID_OPTIONS_TYPE_NONE: IOOptionBits = 0;

// ── IOHIDReportType ──────────────────────────────────────────────────────

pub const K_IOHID_REPORT_TYPE_OUTPUT: u32 = 1;

// ── Callback signatures ──────────────────────────────────────────────────

pub type IOHIDDeviceCallback = unsafe extern "C" fn(
    context: *mut c_void,
    result: IOReturn,
    sender: *mut c_void,
    device: IOHIDDeviceRef,
);

pub type IOHIDReportCallback = unsafe extern "C" fn(
    context: *mut c_void,
    result: IOReturn,
    sender: IOHIDDeviceRef,
    report_type: u32,
    report_id: u32,
    report: *const u8,
    report_length: isize,
);

// ── IOKit extern declarations ────────────────────────────────────────────

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    pub fn IOHIDManagerCreate(allocator: CFAllocatorRef, options: IOOptionBits) -> IOHIDManagerRef;

    pub fn IOHIDManagerSetDeviceMatchingMultiple(manager: IOHIDManagerRef, multiple: CFArrayRef);

    pub fn IOHIDManagerOpen(manager: IOHIDManagerRef, options: IOOptionBits) -> IOReturn;

    pub fn IOHIDManagerClose(manager: IOHIDManagerRef, options: IOOptionBits) -> IOReturn;

    pub fn IOHIDManagerRegisterDeviceMatchingCallback(
        manager: IOHIDManagerRef,
        callback: IOHIDDeviceCallback,
        context: *mut c_void,
    );

    pub fn IOHIDManagerRegisterDeviceRemovalCallback(
        manager: IOHIDManagerRef,
        callback: IOHIDDeviceCallback,
        context: *mut c_void,
    );

    pub fn IOHIDManagerScheduleWithRunLoop(
        manager: IOHIDManagerRef,
        run_loop: CFRunLoopRef,
        run_loop_mode: CFStringRef,
    );

    pub fn IOHIDManagerCopyDevices(manager: IOHIDManagerRef) -> CFSetRef;

    pub fn IOHIDDeviceGetProperty(device: IOHIDDeviceRef, key: CFStringRef) -> CFTypeRef;

    pub fn IOHIDDeviceOpen(device: IOHIDDeviceRef, options: IOOptionBits) -> IOReturn;

    pub fn IOHIDDeviceClose(device: IOHIDDeviceRef, options: IOOptionBits) -> IOReturn;

    pub fn IOHIDDeviceRegisterInputReportCallback(
        device: IOHIDDeviceRef,
        report: *mut u8,
        report_length: isize,
        callback: IOHIDReportCallback,
        context: *mut c_void,
    );

    pub fn IOHIDDeviceScheduleWithRunLoop(
        device: IOHIDDeviceRef,
        run_loop: CFRunLoopRef,
        run_loop_mode: CFStringRef,
    );

    pub fn IOHIDDeviceSetReport(
        device: IOHIDDeviceRef,
        report_type: u32,
        report_id: isize,
        report: *const u8,
        report_length: isize,
    ) -> IOReturn;

    pub fn IOHIDManagerCreateRunLoopSource(
        manager: IOHIDManagerRef,
        order: isize,
    ) -> CFRunLoopSourceRef;
}

// ── Property key constants ───────────────────────────────────────────────

pub const K_IOHID_VENDOR_ID_KEY: &str = "VendorID";
pub const K_IOHID_PRODUCT_ID_KEY: &str = "ProductID";
pub const K_IOHID_PRODUCT_KEY: &str = "Product";
pub const K_IOHID_MANUFACTURER_KEY: &str = "Manufacturer";
pub const K_IOHID_SERIAL_NUMBER_KEY: &str = "SerialNumber";
pub const K_IOHID_LOCATION_ID_KEY: &str = "LocationID";
pub const K_IOHID_PRIMARY_USAGE_PAGE_KEY: &str = "PrimaryUsagePage";
pub const K_IOHID_PRIMARY_USAGE_KEY: &str = "PrimaryUsage";
pub const K_IOHID_DEVICE_USAGE_PAGE_KEY: &str = "DeviceUsagePage";
pub const K_IOHID_DEVICE_USAGE_KEY: &str = "DeviceUsage";

// ── Safe property helpers ────────────────────────────────────────────────

/// Query an integer property from an `IOHIDDeviceRef`.
///
/// # Safety
///
/// `device` must be a valid, non-null `IOHIDDeviceRef`.
pub unsafe fn get_device_int_property(device: IOHIDDeviceRef, key: &str) -> Option<i64> {
    let cf_key = CFString::new(key);
    let value = unsafe { IOHIDDeviceGetProperty(device, cf_key.as_concrete_TypeRef()) };
    if value.is_null() {
        return None;
    }
    let type_id = unsafe { CFGetTypeID(value) };
    if type_id == unsafe { CFNumberGetTypeID() } {
        let num: CFNumber = unsafe { CFNumber::wrap_under_get_rule(value as *const _ as *const _) };
        num.to_i64()
    } else {
        None
    }
}

/// Query a string property from an `IOHIDDeviceRef`.
///
/// # Safety
///
/// `device` must be a valid, non-null `IOHIDDeviceRef`.
pub unsafe fn get_device_string_property(device: IOHIDDeviceRef, key: &str) -> Option<String> {
    let cf_key = CFString::new(key);
    let value = unsafe { IOHIDDeviceGetProperty(device, cf_key.as_concrete_TypeRef()) };
    if value.is_null() {
        return None;
    }
    let type_id = unsafe { CFGetTypeID(value) };
    if type_id == unsafe { CFStringGetTypeID() } {
        let s: CFString = unsafe { CFString::wrap_under_get_rule(value as *const _ as *const _) };
        Some(s.to_string())
    } else {
        None
    }
}
