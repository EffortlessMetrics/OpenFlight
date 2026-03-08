// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Expanded tests for flight-hid-types: HidDeviceInfo construction,
//! field access, Debug formatting, Clone semantics, and property-based tests.

use flight_hid_types::HidDeviceInfo;
use proptest::prelude::*;

// ── Helper ──────────────────────────────────────────────────────────────────

fn make_device(vid: u16, pid: u16) -> HidDeviceInfo {
    HidDeviceInfo {
        vendor_id: vid,
        product_id: pid,
        serial_number: None,
        manufacturer: None,
        product_name: None,
        device_path: String::new(),
        usage_page: 0x01,
        usage: 0x04,
        report_descriptor: None,
    }
}

fn make_full_device() -> HidDeviceInfo {
    HidDeviceInfo {
        vendor_id: 0x231D,
        product_id: 0x0136,
        serial_number: Some("SN-1234".to_string()),
        manufacturer: Some("VKB".to_string()),
        product_name: Some("STECS Mini".to_string()),
        device_path: "/dev/hidraw0".to_string(),
        usage_page: 0x01,
        usage: 0x04,
        report_descriptor: Some(vec![0x05, 0x01, 0x09, 0x04]),
    }
}

// ── Basic construction ──────────────────────────────────────────────────────

#[test]
fn minimal_device_construction() {
    let dev = make_device(0, 0);
    assert_eq!(dev.vendor_id, 0);
    assert_eq!(dev.product_id, 0);
    assert!(dev.serial_number.is_none());
    assert!(dev.manufacturer.is_none());
    assert!(dev.product_name.is_none());
    assert!(dev.device_path.is_empty());
    assert!(dev.report_descriptor.is_none());
}

#[test]
fn full_device_all_fields_populated() {
    let dev = make_full_device();
    assert_eq!(dev.vendor_id, 0x231D);
    assert_eq!(dev.product_id, 0x0136);
    assert_eq!(dev.serial_number.as_deref(), Some("SN-1234"));
    assert_eq!(dev.manufacturer.as_deref(), Some("VKB"));
    assert_eq!(dev.product_name.as_deref(), Some("STECS Mini"));
    assert_eq!(dev.device_path, "/dev/hidraw0");
    assert_eq!(dev.usage_page, 0x01);
    assert_eq!(dev.usage, 0x04);
    assert_eq!(dev.report_descriptor.as_deref(), Some([0x05, 0x01, 0x09, 0x04].as_slice()));
}

// ── Clone semantics ─────────────────────────────────────────────────────────

#[test]
fn clone_preserves_all_fields() {
    let dev = make_full_device();
    let cloned = dev.clone();
    assert_eq!(cloned.vendor_id, dev.vendor_id);
    assert_eq!(cloned.product_id, dev.product_id);
    assert_eq!(cloned.serial_number, dev.serial_number);
    assert_eq!(cloned.manufacturer, dev.manufacturer);
    assert_eq!(cloned.product_name, dev.product_name);
    assert_eq!(cloned.device_path, dev.device_path);
    assert_eq!(cloned.usage_page, dev.usage_page);
    assert_eq!(cloned.usage, dev.usage);
    assert_eq!(cloned.report_descriptor, dev.report_descriptor);
}

#[test]
fn clone_is_independent() {
    let dev = make_full_device();
    let mut cloned = dev.clone();
    cloned.vendor_id = 0xBEEF;
    cloned.device_path = "changed".to_string();
    // Original unchanged
    assert_eq!(dev.vendor_id, 0x231D);
    assert_eq!(dev.device_path, "/dev/hidraw0");
}

#[test]
fn clone_none_fields() {
    let dev = make_device(1, 2);
    let cloned = dev.clone();
    assert!(cloned.serial_number.is_none());
    assert!(cloned.manufacturer.is_none());
    assert!(cloned.product_name.is_none());
    assert!(cloned.report_descriptor.is_none());
}

// ── Debug formatting ────────────────────────────────────────────────────────

#[test]
fn debug_contains_struct_name() {
    let dev = make_device(0x044F, 0xB679);
    let dbg = format!("{:?}", dev);
    assert!(dbg.contains("HidDeviceInfo"));
}

#[test]
fn debug_contains_field_names() {
    let dev = make_full_device();
    let dbg = format!("{:?}", dev);
    assert!(dbg.contains("vendor_id"));
    assert!(dbg.contains("product_id"));
    assert!(dbg.contains("device_path"));
    assert!(dbg.contains("usage_page"));
}

#[test]
fn debug_shows_none_for_optional_fields() {
    let dev = make_device(1, 1);
    let dbg = format!("{:?}", dev);
    assert!(dbg.contains("None"));
}

#[test]
fn debug_shows_some_for_populated_fields() {
    let dev = make_full_device();
    let dbg = format!("{:?}", dev);
    assert!(dbg.contains("Some"));
    assert!(dbg.contains("VKB"));
    assert!(dbg.contains("STECS Mini"));
}

// ── Known vendor/product IDs ────────────────────────────────────────────────

#[test]
fn known_vid_pid_vkb() {
    let dev = make_device(0x231D, 0x0136);
    assert_eq!(dev.vendor_id, 0x231D);
    assert_eq!(dev.product_id, 0x0136);
}

#[test]
fn known_vid_pid_thrustmaster() {
    let dev = make_device(0x044F, 0xB679);
    assert_eq!(dev.vendor_id, 0x044F);
    assert_eq!(dev.product_id, 0xB679);
}

#[test]
fn known_vid_pid_logitech() {
    let dev = make_device(0x046D, 0xC215);
    assert_eq!(dev.vendor_id, 0x046D);
    assert_eq!(dev.product_id, 0xC215);
}

#[test]
fn known_vid_pid_virpil() {
    let dev = make_device(0x3344, 0x0194);
    assert_eq!(dev.vendor_id, 0x3344);
    assert_eq!(dev.product_id, 0x0194);
}

// ── Boundary values ─────────────────────────────────────────────────────────

#[test]
fn max_vid_pid_values() {
    let dev = make_device(u16::MAX, u16::MAX);
    assert_eq!(dev.vendor_id, 0xFFFF);
    assert_eq!(dev.product_id, 0xFFFF);
}

#[test]
fn zero_vid_pid_values() {
    let dev = make_device(0, 0);
    assert_eq!(dev.vendor_id, 0);
    assert_eq!(dev.product_id, 0);
}

#[test]
fn max_usage_page_and_usage() {
    let dev = HidDeviceInfo {
        vendor_id: 0,
        product_id: 0,
        serial_number: None,
        manufacturer: None,
        product_name: None,
        device_path: String::new(),
        usage_page: u16::MAX,
        usage: u16::MAX,
        report_descriptor: None,
    };
    assert_eq!(dev.usage_page, 0xFFFF);
    assert_eq!(dev.usage, 0xFFFF);
}

// ── Device path formats ─────────────────────────────────────────────────────

#[test]
fn windows_device_path() {
    let dev = HidDeviceInfo {
        vendor_id: 0x044F,
        product_id: 0xB679,
        serial_number: None,
        manufacturer: None,
        product_name: None,
        device_path: r"\\?\HID#VID_044F&PID_B679#7&1234&0&0000#{4d1e55b2}".to_string(),
        usage_page: 0x01,
        usage: 0x04,
        report_descriptor: None,
    };
    assert!(dev.device_path.contains("HID#VID_044F"));
}

#[test]
fn linux_device_path() {
    let dev = HidDeviceInfo {
        vendor_id: 0x231D,
        product_id: 0x0136,
        serial_number: None,
        manufacturer: None,
        product_name: None,
        device_path: "/dev/hidraw3".to_string(),
        usage_page: 0x01,
        usage: 0x04,
        report_descriptor: None,
    };
    assert!(dev.device_path.starts_with("/dev/"));
}

#[test]
fn empty_device_path() {
    let dev = make_device(1, 1);
    assert!(dev.device_path.is_empty());
}

// ── Report descriptor ───────────────────────────────────────────────────────

#[test]
fn report_descriptor_empty_vec() {
    let dev = HidDeviceInfo {
        vendor_id: 0,
        product_id: 0,
        serial_number: None,
        manufacturer: None,
        product_name: None,
        device_path: String::new(),
        usage_page: 0,
        usage: 0,
        report_descriptor: Some(vec![]),
    };
    assert_eq!(dev.report_descriptor.unwrap().len(), 0);
}

#[test]
fn report_descriptor_large() {
    let desc = vec![0xAA; 4096];
    let dev = HidDeviceInfo {
        vendor_id: 0,
        product_id: 0,
        serial_number: None,
        manufacturer: None,
        product_name: None,
        device_path: String::new(),
        usage_page: 0,
        usage: 0,
        report_descriptor: Some(desc),
    };
    assert_eq!(dev.report_descriptor.unwrap().len(), 4096);
}

#[test]
fn report_descriptor_typical_hid_joystick() {
    // Typical HID report descriptor header for a joystick
    let desc = vec![
        0x05, 0x01, // Usage Page (Generic Desktop)
        0x09, 0x04, // Usage (Joystick)
        0xA1, 0x01, // Collection (Application)
        0x09, 0x30, // Usage (X)
        0x09, 0x31, // Usage (Y)
        0x15, 0x00, // Logical Minimum (0)
        0x26, 0xFF, 0x03, // Logical Maximum (1023)
        0x75, 0x10, // Report Size (16)
        0x95, 0x02, // Report Count (2)
        0x81, 0x02, // Input (Data, Var, Abs)
        0xC0, // End Collection
    ];
    let dev = HidDeviceInfo {
        vendor_id: 0x231D,
        product_id: 0x0136,
        serial_number: None,
        manufacturer: None,
        product_name: None,
        device_path: String::new(),
        usage_page: 0x01,
        usage: 0x04,
        report_descriptor: Some(desc.clone()),
    };
    let rd = dev.report_descriptor.unwrap();
    assert_eq!(rd[0], 0x05); // Usage Page tag
    assert_eq!(rd[1], 0x01); // Generic Desktop
    assert_eq!(rd[2], 0x09); // Usage tag
    assert_eq!(rd[3], 0x04); // Joystick
}

// ── Usage page constants ────────────────────────────────────────────────────

#[test]
fn usage_page_generic_desktop() {
    let dev = make_device(1, 1);
    assert_eq!(dev.usage_page, 0x01);
}

#[test]
fn usage_page_simulation() {
    let dev = HidDeviceInfo {
        usage_page: 0x02,
        usage: 0xB8, // Aileron
        ..make_device(1, 1)
    };
    assert_eq!(dev.usage_page, 0x02);
    assert_eq!(dev.usage, 0xB8);
}

// ── String field edge cases ─────────────────────────────────────────────────

#[test]
fn unicode_product_name() {
    let dev = HidDeviceInfo {
        product_name: Some("Flügelsteuerung™".to_string()),
        ..make_device(1, 1)
    };
    assert_eq!(dev.product_name.as_deref(), Some("Flügelsteuerung™"));
}

#[test]
fn empty_string_vs_none() {
    let dev = HidDeviceInfo {
        serial_number: Some(String::new()),
        manufacturer: None,
        ..make_device(1, 1)
    };
    assert_eq!(dev.serial_number.as_deref(), Some(""));
    assert!(dev.manufacturer.is_none());
}

#[test]
fn long_serial_number() {
    let long = "A".repeat(256);
    let dev = HidDeviceInfo {
        serial_number: Some(long.clone()),
        ..make_device(1, 1)
    };
    assert_eq!(dev.serial_number.unwrap().len(), 256);
}

// ── proptest ────────────────────────────────────────────────────────────────

proptest! {
    #[test]
    fn clone_always_preserves_vid_pid(vid: u16, pid: u16) {
        let dev = make_device(vid, pid);
        let cloned = dev.clone();
        prop_assert_eq!(cloned.vendor_id, vid);
        prop_assert_eq!(cloned.product_id, pid);
    }

    #[test]
    fn clone_always_preserves_usage(page: u16, usage: u16) {
        let dev = HidDeviceInfo {
            usage_page: page,
            usage,
            ..make_device(0, 0)
        };
        let cloned = dev.clone();
        prop_assert_eq!(cloned.usage_page, page);
        prop_assert_eq!(cloned.usage, usage);
    }

    #[test]
    fn debug_never_panics(vid: u16, pid: u16) {
        let dev = make_device(vid, pid);
        let dbg = format!("{:?}", dev);
        prop_assert!(!dbg.is_empty());
    }

    #[test]
    fn report_descriptor_clone_preserves_length(len in 0..512usize) {
        let desc = vec![0xABu8; len];
        let dev = HidDeviceInfo {
            report_descriptor: Some(desc),
            ..make_device(0, 0)
        };
        let cloned = dev.clone();
        prop_assert_eq!(cloned.report_descriptor.unwrap().len(), len);
    }
}
