// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Shared HID data types used across crates.

/// HID device information
#[derive(Debug, Clone)]
pub struct HidDeviceInfo {
    pub vendor_id: u16,
    pub product_id: u16,
    pub serial_number: Option<String>,
    pub manufacturer: Option<String>,
    pub product_name: Option<String>,
    pub device_path: String,
    pub usage_page: u16,
    pub usage: u16,
    /// Optional HID report descriptor for usage parsing and quirks.
    pub report_descriptor: Option<Vec<u8>>,
}
