// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID traits for the macOS layer.
//!
//! These mirror the [`DeviceScanner`] and [`HotplugMonitor`] traits from
//! `flight-hid`, allowing [`MacHidManager`](crate::MacHidManager) to serve
//! as a platform-specific backend that can be adapted to the common
//! `flight-hid` trait interface.

use crate::device::HidDeviceInfo;

// ── MacHotplugEvent ──────────────────────────────────────────────────────

/// Event emitted when a HID device is attached or detached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacHotplugEvent {
    /// A matching device was attached.
    Attached(HidDeviceInfo),
    /// A device was detached.
    Detached {
        vendor_id: u16,
        product_id: u16,
        location_id: u32,
    },
}

impl MacHotplugEvent {
    /// Returns `true` for attach events.
    pub fn is_attach(&self) -> bool {
        matches!(self, MacHotplugEvent::Attached(_))
    }

    /// Returns `true` for detach events.
    pub fn is_detach(&self) -> bool {
        matches!(self, MacHotplugEvent::Detached { .. })
    }

    /// Vendor ID from either event variant.
    pub fn vendor_id(&self) -> u16 {
        match self {
            MacHotplugEvent::Attached(info) => info.vendor_id,
            MacHotplugEvent::Detached { vendor_id, .. } => *vendor_id,
        }
    }

    /// Product ID from either event variant.
    pub fn product_id(&self) -> u16 {
        match self {
            MacHotplugEvent::Attached(info) => info.product_id,
            MacHotplugEvent::Detached { product_id, .. } => *product_id,
        }
    }
}

// ── Trait: MacDeviceScanner ──────────────────────────────────────────────

/// Enumerates HID devices on the platform.
///
/// Mirrors `flight_hid::discovery::DeviceScanner`.
pub trait MacDeviceScanner: Send {
    /// Enumerate all currently-connected HID devices matching the criteria.
    fn enumerate(&mut self) -> Vec<HidDeviceInfo>;
}

// ── Trait: MacHotplugMonitor ─────────────────────────────────────────────

/// Receives device attach/detach events.
///
/// Mirrors `flight_hid::hotplug::HotplugMonitor`.
pub trait MacHotplugMonitor: Send {
    /// Poll for pending attach/detach events (non-blocking).
    fn poll_events(&mut self) -> Vec<MacHotplugEvent>;
}

// ── Trait: MacInputReportReader ──────────────────────────────────────────

/// Reads input reports from an open HID device.
pub trait MacInputReportReader: Send {
    /// Read the next input report. Returns `(report_id, data)`.
    /// Non-blocking; returns `None` if no report is available.
    fn next_report(&mut self) -> Option<(u8, Vec<u8>)>;
}

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
            location_id: 0x1234,
        }
    }

    #[test]
    fn test_hotplug_event_attach() {
        let ev = MacHotplugEvent::Attached(sample_info());
        assert!(ev.is_attach());
        assert!(!ev.is_detach());
        assert_eq!(ev.vendor_id(), 0x044F);
        assert_eq!(ev.product_id(), 0xB67B);
    }

    #[test]
    fn test_hotplug_event_detach() {
        let ev = MacHotplugEvent::Detached {
            vendor_id: 0x044F,
            product_id: 0xB67B,
            location_id: 0x1234,
        };
        assert!(!ev.is_attach());
        assert!(ev.is_detach());
        assert_eq!(ev.vendor_id(), 0x044F);
        assert_eq!(ev.product_id(), 0xB67B);
    }

    #[test]
    fn test_hotplug_event_clone_eq() {
        let ev1 = MacHotplugEvent::Attached(sample_info());
        let ev2 = ev1.clone();
        assert_eq!(ev1, ev2);
    }
}
