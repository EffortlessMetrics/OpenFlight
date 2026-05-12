// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Shared HID data types used across OpenFlight crates.
//!
//! Provides USB HID usage page/usage constants, report descriptor parsing,
//! bit-level report field extraction, and device information types.

use core::fmt;

// ── Usage page constants (USB HID Usage Tables §3) ───────────────────────

/// Standard HID usage page identifiers.
pub mod usage_page {
    pub const GENERIC_DESKTOP: u16 = 0x01;
    pub const SIMULATION: u16 = 0x02;
    pub const VR: u16 = 0x03;
    pub const SPORT: u16 = 0x04;
    pub const GAME: u16 = 0x05;
    pub const GENERIC_DEVICE: u16 = 0x06;
    pub const KEYBOARD: u16 = 0x07;
    pub const LED: u16 = 0x08;
    pub const BUTTON: u16 = 0x09;
    pub const ORDINAL: u16 = 0x0A;
    pub const TELEPHONY: u16 = 0x0B;
    pub const CONSUMER: u16 = 0x0C;
    /// Physical Interface Device (force feedback).
    pub const PID: u16 = 0x0F;
    pub const VENDOR_MIN: u16 = 0xFF00;
    pub const VENDOR_MAX: u16 = 0xFFFF;

    /// Returns `true` when the page falls in the vendor-defined range.
    pub const fn is_vendor(page: u16) -> bool {
        page >= VENDOR_MIN
    }
}

/// Generic Desktop page usage IDs (USB HID Usage Tables §4).
pub mod usage_desktop {
    pub const POINTER: u16 = 0x01;
    pub const MOUSE: u16 = 0x02;
    pub const JOYSTICK: u16 = 0x04;
    pub const GAME_PAD: u16 = 0x05;
    pub const KEYBOARD: u16 = 0x06;
    pub const MULTI_AXIS: u16 = 0x08;
    pub const X: u16 = 0x30;
    pub const Y: u16 = 0x31;
    pub const Z: u16 = 0x32;
    pub const RX: u16 = 0x33;
    pub const RY: u16 = 0x34;
    pub const RZ: u16 = 0x35;
    pub const SLIDER: u16 = 0x36;
    pub const DIAL: u16 = 0x37;
    pub const WHEEL: u16 = 0x38;
    pub const HAT_SWITCH: u16 = 0x39;
}

/// Simulation Controls page usage IDs (USB HID Usage Tables §5).
pub mod usage_simulation {
    pub const FLIGHT_SIMULATION: u16 = 0x01;
    pub const AUTOMOBILE_SIMULATION: u16 = 0x02;
    pub const AILERON: u16 = 0xB0;
    pub const AILERON_TRIM: u16 = 0xB1;
    pub const ELEVATOR: u16 = 0xB8;
    pub const ELEVATOR_TRIM: u16 = 0xB9;
    pub const RUDDER: u16 = 0xBA;
    pub const THROTTLE: u16 = 0xBB;
    pub const FLIGHT_COMMUNICATIONS: u16 = 0xBC;
}

mod descriptor_parser;

pub use descriptor_parser::parse_descriptor;

// ── Error type ───────────────────────────────────────────────────────────

/// Errors produced during HID descriptor parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DescriptorError {
    /// The descriptor byte stream is empty.
    Empty,
    /// An item header references data beyond the end of the descriptor.
    Truncated { offset: usize },
    /// An End Collection was encountered without a matching Begin.
    UnmatchedEnd { offset: usize },
    /// The descriptor ended with unclosed collections.
    UnclosedCollection { count: usize },
}

impl fmt::Display for DescriptorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "empty descriptor"),
            Self::Truncated { offset } => {
                write!(f, "truncated item at byte offset {offset}")
            }
            Self::UnmatchedEnd { offset } => {
                write!(f, "unmatched end-collection at byte offset {offset}")
            }
            Self::UnclosedCollection { count } => {
                write!(f, "unclosed collection(s): {count} remaining")
            }
        }
    }
}

// ── Public types ─────────────────────────────────────────────────────────

/// Classification of a HID main item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReportType {
    Input,
    Output,
    Feature,
}

impl fmt::Display for ReportType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input => write!(f, "Input"),
            Self::Output => write!(f, "Output"),
            Self::Feature => write!(f, "Feature"),
        }
    }
}

/// The type of a HID collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CollectionType {
    Physical,
    Application,
    Logical,
    Report,
    NamedArray,
    UsageSwitch,
    UsageModifier,
    /// Any value not covered above.
    Other(u32),
}

impl CollectionType {
    /// Create from the raw collection type value in the descriptor.
    pub fn from_value(v: u32) -> Self {
        match v {
            0x00 => Self::Physical,
            0x01 => Self::Application,
            0x02 => Self::Logical,
            0x03 => Self::Report,
            0x04 => Self::NamedArray,
            0x05 => Self::UsageSwitch,
            0x06 => Self::UsageModifier,
            other => Self::Other(other),
        }
    }
}

impl fmt::Display for CollectionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Physical => write!(f, "Physical"),
            Self::Application => write!(f, "Application"),
            Self::Logical => write!(f, "Logical"),
            Self::Report => write!(f, "Report"),
            Self::NamedArray => write!(f, "NamedArray"),
            Self::UsageSwitch => write!(f, "UsageSwitch"),
            Self::UsageModifier => write!(f, "UsageModifier"),
            Self::Other(v) => write!(f, "Other({v:#x})"),
        }
    }
}

/// Bit flags from a Main item's data byte (HID spec §6.2.2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MainItemFlags(pub u32);

impl MainItemFlags {
    pub const fn is_constant(self) -> bool {
        self.0 & 0x01 != 0
    }
    pub const fn is_variable(self) -> bool {
        self.0 & 0x02 != 0
    }
    pub const fn is_relative(self) -> bool {
        self.0 & 0x04 != 0
    }
    pub const fn is_wrap(self) -> bool {
        self.0 & 0x08 != 0
    }
    pub const fn is_nonlinear(self) -> bool {
        self.0 & 0x10 != 0
    }
    pub const fn is_no_preferred(self) -> bool {
        self.0 & 0x20 != 0
    }
    pub const fn is_null_state(self) -> bool {
        self.0 & 0x40 != 0
    }
    pub const fn is_buffered_bytes(self) -> bool {
        self.0 & 0x100 != 0
    }
}

/// A single parsed HID data field (axis, button group, hat, etc.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportField {
    pub report_type: ReportType,
    pub flags: MainItemFlags,
    pub usage_page: u16,
    pub usage: u16,
    pub logical_min: i32,
    pub logical_max: i32,
    pub physical_min: i32,
    pub physical_max: i32,
    pub report_size: u32,
    pub report_count: u32,
    pub report_id: Option<u8>,
}

impl ReportField {
    /// Total number of bits occupied by this field.
    pub const fn total_bits(&self) -> u32 {
        self.report_size * self.report_count
    }

    /// Returns `true` when this field describes button data.
    pub fn is_button(&self) -> bool {
        self.usage_page == usage_page::BUTTON
    }

    /// Returns `true` when this field describes a hat switch.
    pub fn is_hat(&self) -> bool {
        self.usage_page == usage_page::GENERIC_DESKTOP && self.usage == usage_desktop::HAT_SWITCH
    }

    /// Returns `true` when this field describes an axis (multi-bit,
    /// non-button, non-hat input).
    pub fn is_axis(&self) -> bool {
        if self.is_button() || self.is_hat() {
            return false;
        }
        self.report_size > 1
    }
}

/// A HID collection with its contained fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HidCollection {
    pub usage_page: u16,
    pub usage: u16,
    pub collection_type: CollectionType,
    pub fields: Vec<ReportField>,
}

/// Top-level result of parsing a complete HID report descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportDescriptor {
    pub collections: Vec<HidCollection>,
    pub total_axes: u32,
    pub total_buttons: u32,
    pub total_hats: u32,
    pub report_size_bits: u32,
}

impl ReportDescriptor {
    pub fn axis_count(&self) -> u32 {
        self.total_axes
    }
    pub fn button_count(&self) -> u32 {
        self.total_buttons
    }
    pub fn hat_count(&self) -> u32 {
        self.total_hats
    }

    /// Logical (min, max) range for each axis, in descriptor order.
    pub fn axis_ranges(&self) -> Vec<(i32, i32)> {
        let mut ranges = Vec::new();
        for col in &self.collections {
            for field in &col.fields {
                if field.report_type == ReportType::Input && field.is_axis() {
                    for _ in 0..field.report_count {
                        ranges.push((field.logical_min, field.logical_max));
                    }
                }
            }
        }
        ranges
    }

    /// Return all fields across all collections.
    pub fn all_fields(&self) -> Vec<&ReportField> {
        self.collections.iter().flat_map(|c| &c.fields).collect()
    }
}

impl fmt::Display for ReportDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ReportDescriptor {{ axes: {}, buttons: {}, hats: {}, bits: {} }}",
            self.total_axes, self.total_buttons, self.total_hats, self.report_size_bits
        )
    }
}

// ── Bit-level extraction ─────────────────────────────────────────────────

/// Extract an unsigned value from a raw HID report at the given bit offset
/// and size. Returns `None` when the report is too short.
pub fn extract_bits(report: &[u8], bit_offset: u32, bit_size: u32) -> Option<u32> {
    if bit_size == 0 || bit_size > 32 {
        return None;
    }
    let end_bit = bit_offset.checked_add(bit_size)?;
    let needed_bytes = end_bit.div_ceil(8);
    if (report.len() as u32) < needed_bytes {
        return None;
    }

    let mut value = 0u32;
    for i in 0..bit_size {
        let abs_bit = bit_offset + i;
        let byte_idx = (abs_bit / 8) as usize;
        let bit_idx = abs_bit % 8;
        if (report[byte_idx] >> bit_idx) & 1 == 1 {
            value |= 1 << i;
        }
    }
    Some(value)
}

/// Extract a signed value from a raw HID report at the given bit offset
/// and size. Returns `None` when the report is too short.
pub fn extract_bits_signed(report: &[u8], bit_offset: u32, bit_size: u32) -> Option<i32> {
    let raw = extract_bits(report, bit_offset, bit_size)?;
    if bit_size >= 32 {
        return Some(raw as i32);
    }
    // Sign-extend
    if (raw >> (bit_size - 1)) & 1 == 1 {
        Some((raw | (!0u32 << bit_size)) as i32)
    } else {
        Some(raw as i32)
    }
}

// ── HidDeviceInfo ────────────────────────────────────────────────────────

/// HID device information.
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

impl HidDeviceInfo {
    /// Returns `true` when this device is on a vendor-defined usage page.
    pub fn is_vendor_page(&self) -> bool {
        usage_page::is_vendor(self.usage_page)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hid_device_info_clone() {
        let info = HidDeviceInfo {
            vendor_id: 0x231D,
            product_id: 0x0136,
            serial_number: Some("SN001".to_string()),
            manufacturer: Some("VKB".to_string()),
            product_name: Some("STECS Mini Left".to_string()),
            device_path: "/dev/hidraw0".to_string(),
            usage_page: 0x01,
            usage: 0x04,
            report_descriptor: Some(vec![0x05, 0x01, 0x09, 0x04]),
        };
        let cloned = info.clone();
        assert_eq!(cloned.vendor_id, 0x231D);
        assert_eq!(cloned.product_id, 0x0136);
        assert_eq!(cloned.serial_number.as_deref(), Some("SN001"));
        assert_eq!(cloned.product_name.as_deref(), Some("STECS Mini Left"));
    }

    #[test]
    fn hid_device_info_optional_fields() {
        let info = HidDeviceInfo {
            vendor_id: 0x044F,
            product_id: 0xB679,
            serial_number: None,
            manufacturer: None,
            product_name: None,
            device_path: "\\\\?\\HID#VID_044F&PID_B679".to_string(),
            usage_page: 0x01,
            usage: 0x04,
            report_descriptor: None,
        };
        assert!(info.serial_number.is_none());
        assert!(info.report_descriptor.is_none());
        assert_eq!(info.usage_page, 0x01);
    }

    #[test]
    fn hid_device_info_with_descriptor() {
        let descriptor = vec![0x05, 0x01, 0x09, 0x04, 0xA1, 0x01];
        let info = HidDeviceInfo {
            vendor_id: 0x231D,
            product_id: 0x0138,
            serial_number: None,
            manufacturer: None,
            product_name: None,
            device_path: String::new(),
            usage_page: 0x01,
            usage: 0x04,
            report_descriptor: Some(descriptor.clone()),
        };
        assert_eq!(info.report_descriptor.unwrap(), descriptor);
    }

    #[test]
    fn hid_device_info_debug_format() {
        let info = HidDeviceInfo {
            vendor_id: 0x044F,
            product_id: 0xB679,
            serial_number: None,
            manufacturer: Some("Thrustmaster".to_string()),
            product_name: None,
            device_path: String::new(),
            usage_page: 0,
            usage: 0,
            report_descriptor: None,
        };
        let s = format!("{:?}", info);
        assert!(s.contains("0x044F") || s.contains("1103")); // vendor_id in some form
        assert!(s.contains("Thrustmaster"));
    }

    #[test]
    fn hid_device_info_max_vid_pid() {
        let info = HidDeviceInfo {
            vendor_id: 0xFFFF,
            product_id: 0xFFFF,
            serial_number: None,
            manufacturer: None,
            product_name: None,
            device_path: String::new(),
            usage_page: 0xFFFF,
            usage: 0xFFFF,
            report_descriptor: None,
        };
        assert_eq!(info.vendor_id, 0xFFFF);
        assert_eq!(info.product_id, 0xFFFF);
        assert_eq!(info.usage_page, 0xFFFF);
    }
}
