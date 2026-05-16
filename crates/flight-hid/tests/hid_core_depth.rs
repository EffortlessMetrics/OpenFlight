// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for HID core device management.
//!
//! Covers device enumeration, report descriptor parsing, report reading/writing,
//! stable device IDs, and hot-plug behaviour.

// ═══════════════════════════════════════════════════════════════════════════
// Test helpers
// ═══════════════════════════════════════════════════════════════════════════

use flight_hid::descriptor_parser::{self, CollectionType, HidItemType};
use flight_hid::device_id::DeviceId;
use flight_hid::discovery::{DeviceDiscovery, DeviceEvent, MockScanner};
use flight_hid::hotplug::{HotplugEvent, MockHotplugMonitor};
use flight_hid::report_builder::HidReportBuilder;
use flight_hid::stable_id::{DeviceFingerprint, DeviceRegistry, StableDeviceId};

fn warthog_fp() -> DeviceFingerprint {
    DeviceFingerprint {
        vid: 0x044F,
        pid: 0x0402,
        serial: Some("WH001".into()),
        manufacturer: Some("Thrustmaster".into()),
        product: Some("HOTAS Warthog Joystick".into()),
        interface_number: Some(0),
        usage_page: 0x01,
        usage: 0x04,
        usb_path: Some("1-2.3".into()),
    }
}

fn vkb_fp() -> DeviceFingerprint {
    DeviceFingerprint {
        vid: 0x231D,
        pid: 0x0136,
        serial: Some("VKB001".into()),
        manufacturer: Some("VKB".into()),
        product: Some("Gladiator NXT EVO".into()),
        interface_number: None,
        usage_page: 0x01,
        usage: 0x04,
        usb_path: Some("1-4.1".into()),
    }
}

fn t16000m_fp() -> DeviceFingerprint {
    DeviceFingerprint {
        vid: 0x044F,
        pid: 0xB679,
        serial: None,
        manufacturer: Some("Thrustmaster".into()),
        product: Some("T16000M".into()),
        interface_number: None,
        usage_page: 0x01,
        usage: 0x04,
        usb_path: Some("1-5.2".into()),
    }
}

fn virpil_fp() -> DeviceFingerprint {
    DeviceFingerprint {
        vid: 0x3344,
        pid: 0x01F4,
        serial: Some("VPC001".into()),
        manufacturer: Some("VIRPIL".into()),
        product: Some("VPC MongoosT-50CM3".into()),
        interface_number: Some(0),
        usage_page: 0x01,
        usage: 0x04,
        usb_path: Some("2-1.1".into()),
    }
}

fn winwing_fp() -> DeviceFingerprint {
    DeviceFingerprint {
        vid: 0x4098,
        pid: 0xBE62,
        serial: Some("WW001".into()),
        manufacturer: Some("WinWing".into()),
        product: Some("Orion 2 Throttle".into()),
        interface_number: Some(0),
        usage_page: 0x01,
        usage: 0x04,
        usb_path: Some("2-3.2".into()),
    }
}

/// Build a joystick descriptor with given axes, buttons, and optional hat.
fn build_descriptor(axes: u8, buttons: u8, hats: u8) -> Vec<u8> {
    let mut d = Vec::new();
    // Usage Page (Generic Desktop) / Usage (Joystick) / Collection (Application)
    d.extend_from_slice(&[0x05, 0x01, 0x09, 0x04, 0xA1, 0x01]);

    if axes > 0 {
        d.extend_from_slice(&[0x05, 0x01]); // Usage Page (Generic Desktop)
        d.push(0x19);
        d.push(0x30); // Usage Min (X)
        d.push(0x29);
        d.push(0x30 + axes - 1); // Usage Max
        d.extend_from_slice(&[0x15, 0x00]); // Logical Min (0)
        d.extend_from_slice(&[0x26, 0xFF, 0x03]); // Logical Max (1023)
        d.push(0x75);
        d.push(0x10); // Report Size (16)
        d.push(0x95);
        d.push(axes); // Report Count
        d.extend_from_slice(&[0x81, 0x02]); // Input (Data, Var, Abs)
    }

    if hats > 0 {
        d.extend_from_slice(&[0x05, 0x01]); // Usage Page (Generic Desktop)
        d.push(0x09);
        d.push(0x39); // Usage (Hat Switch)
        d.extend_from_slice(&[0x15, 0x01]); // Logical Min (1)
        d.extend_from_slice(&[0x25, 0x08]); // Logical Max (8)
        d.push(0x75);
        d.push(0x04); // Report Size (4)
        d.push(0x95);
        d.push(hats); // Report Count
        d.extend_from_slice(&[0x81, 0x42]); // Input (Data, Var, Abs, Null)
    }

    if buttons > 0 {
        d.extend_from_slice(&[0x05, 0x09]); // Usage Page (Button)
        d.push(0x19);
        d.push(0x01); // Usage Min (1)
        d.push(0x29);
        d.push(buttons); // Usage Max
        d.extend_from_slice(&[0x15, 0x00]); // Logical Min (0)
        d.extend_from_slice(&[0x25, 0x01]); // Logical Max (1)
        d.push(0x75);
        d.push(0x01); // Report Size (1)
        d.push(0x95);
        d.push(buttons); // Report Count
        d.extend_from_slice(&[0x81, 0x02]); // Input (Data, Var, Abs)

        let pad = (8 - (buttons % 8)) % 8;
        if pad > 0 {
            d.push(0x75);
            d.push(0x01);
            d.push(0x95);
            d.push(pad);
            d.extend_from_slice(&[0x81, 0x01]); // Input (Constant)
        }
    }

    d.push(0xC0); // End Collection
    d
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. Device Enumeration (6 tests)
// ═══════════════════════════════════════════════════════════════════════════

mod device_enumeration {
    use super::*;

    #[test]
    fn enumerate_discovers_all_devices() {
        let scanner = MockScanner::new(vec![warthog_fp(), vkb_fp(), t16000m_fp()]);
        let mut disc = DeviceDiscovery::with_defaults(scanner, DeviceRegistry::new());
        let found = disc.scan();
        assert_eq!(found.len(), 3);
        let vids: Vec<u16> = found.iter().map(|d| d.fingerprint.vid).collect();
        assert!(vids.contains(&0x044F));
        assert!(vids.contains(&0x231D));
    }

    #[test]
    fn filter_by_vid_pid() {
        let scanner = MockScanner::new(vec![warthog_fp(), vkb_fp(), t16000m_fp()]);
        let mut disc = DeviceDiscovery::with_defaults(scanner, DeviceRegistry::new());
        let found = disc.scan();
        let thrustmaster = found
            .iter()
            .filter(|d| d.fingerprint.vid == 0x044F)
            .count();
        assert_eq!(thrustmaster, 2);
    }

    #[test]
    fn filter_by_usage_page() {
        let mut gamepad_fp = vkb_fp();
        gamepad_fp.usage_page = 0x01;
        gamepad_fp.usage = 0x05; // Gamepad usage

        let scanner = MockScanner::new(vec![warthog_fp(), gamepad_fp.clone()]);
        let mut disc = DeviceDiscovery::with_defaults(scanner, DeviceRegistry::new());
        let found = disc.scan();
        let joysticks: Vec<_> = found
            .iter()
            .filter(|d| d.fingerprint.usage_page == 0x01 && d.fingerprint.usage == 0x04)
            .collect();
        assert_eq!(joysticks.len(), 1);
        assert_eq!(joysticks[0].fingerprint.vid, 0x044F);
    }

    #[test]
    fn handle_zero_devices() {
        let scanner = MockScanner::new(vec![]);
        let mut disc = DeviceDiscovery::with_defaults(scanner, DeviceRegistry::new());
        let found = disc.scan();
        assert!(found.is_empty());
        assert!(disc.connected_ids().is_empty());
        assert_eq!(disc.registry().len(), 0);
    }

    #[test]
    fn handle_many_devices() {
        let mut devices = Vec::new();
        for i in 0..50u16 {
            devices.push(DeviceFingerprint {
                vid: 0x1000 + i,
                pid: 0x2000 + i,
                serial: Some(format!("SN{i:04}")),
                manufacturer: Some("TestMfg".into()),
                product: Some(format!("Device{i}")),
                interface_number: None,
                usage_page: 0x01,
                usage: 0x04,
                usb_path: Some(format!("1-{i}.1")),
            });
        }
        let scanner = MockScanner::new(devices);
        let mut disc = DeviceDiscovery::with_defaults(scanner, DeviceRegistry::new());
        let found = disc.scan();
        assert_eq!(found.len(), 50);
        assert_eq!(disc.registry().len(), 50);
        assert!(found.iter().all(|d| d.is_new));
    }

    #[test]
    fn device_added_removed_events() {
        // Test connect events via separate discovery instances to avoid
        // private scanner field access.
        let scanner = MockScanner::new(vec![warthog_fp()]);
        let mut disc = DeviceDiscovery::with_defaults(scanner, DeviceRegistry::new());

        // Initial connect
        let ev = disc.poll_events();
        assert_eq!(ev.len(), 1);
        assert!(matches!(&ev[0], DeviceEvent::Connected(d) if d.fingerprint.vid == 0x044F));

        // Test addition via a fresh discovery with two devices, sharing the same registry
        let reg = disc.registry().clone();
        let scanner2 = MockScanner::new(vec![warthog_fp(), vkb_fp()]);
        let mut disc2 = DeviceDiscovery::with_defaults(scanner2, reg);
        let found = disc2.scan();
        assert_eq!(found.len(), 2);
        // Warthog should not be new (already in registry); VKB should be new
        let vkb_device = found.iter().find(|d| d.fingerprint.vid == 0x231D).unwrap();
        assert!(vkb_device.is_new);
        let wh_device = found.iter().find(|d| d.fingerprint.vid == 0x044F).unwrap();
        assert!(!wh_device.is_new);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. Report Descriptor Parsing (6 tests)
// ═══════════════════════════════════════════════════════════════════════════

mod report_descriptor_parsing {
    use super::*;

    #[test]
    fn parse_input_output_feature_reports() {
        let mut d = Vec::new();
        d.extend_from_slice(&[0x05, 0x01, 0x09, 0x04, 0xA1, 0x01]);

        // Input: 2 axes
        d.extend_from_slice(&[0x05, 0x01, 0x19, 0x30, 0x29, 0x31]);
        d.extend_from_slice(&[0x15, 0x00, 0x26, 0xFF, 0x03]);
        d.extend_from_slice(&[0x75, 0x10, 0x95, 0x02, 0x81, 0x02]);

        // Output: 1 byte (LED)
        d.extend_from_slice(&[0x05, 0x08, 0x09, 0x01]);
        d.extend_from_slice(&[0x75, 0x08, 0x95, 0x01, 0x91, 0x02]);

        // Feature: 1 byte
        d.extend_from_slice(&[0x05, 0x08, 0x09, 0x02]);
        d.extend_from_slice(&[0x75, 0x08, 0x95, 0x01, 0xB1, 0x02]);

        d.push(0xC0);

        let desc = descriptor_parser::parse_descriptor(&d).unwrap();
        let items = &desc.collections[0].items;
        assert!(items.iter().any(|i| i.item_type == HidItemType::Input));
        assert!(items.iter().any(|i| i.item_type == HidItemType::Output));
        assert!(items.iter().any(|i| i.item_type == HidItemType::Feature));
    }

    #[test]
    fn parse_axis_usage_ranges() {
        let d = build_descriptor(4, 0, 0);
        let desc = descriptor_parser::parse_descriptor(&d).unwrap();
        assert_eq!(desc.axis_count(), 4);
        let ranges = desc.axis_ranges();
        assert_eq!(ranges.len(), 4);
        for &(lo, hi) in &ranges {
            assert_eq!(lo, 0);
            assert_eq!(hi, 1023);
        }
    }

    #[test]
    fn parse_button_usage() {
        let d = build_descriptor(0, 12, 0);
        let desc = descriptor_parser::parse_descriptor(&d).unwrap();
        assert_eq!(desc.button_count(), 12);
        assert_eq!(desc.axis_count(), 0);
        assert_eq!(desc.hat_count(), 0);
    }

    #[test]
    fn parse_hat_switch() {
        let d = build_descriptor(2, 8, 2);
        let desc = descriptor_parser::parse_descriptor(&d).unwrap();
        assert_eq!(desc.hat_count(), 2);
        assert_eq!(desc.axis_count(), 2);
        assert_eq!(desc.button_count(), 8);
    }

    #[test]
    fn parse_vendor_specific_usage_page() {
        // Vendor-defined usage page (0xFF00)
        let mut d = Vec::new();
        d.extend_from_slice(&[0x06, 0x00, 0xFF]); // Usage Page (Vendor 0xFF00, 2-byte)
        d.extend_from_slice(&[0x09, 0x01]); // Usage (1)
        d.extend_from_slice(&[0xA1, 0x01]); // Collection (Application)
        d.extend_from_slice(&[0x09, 0x20]); // Usage (vendor-specific 0x20)
        d.extend_from_slice(&[0x15, 0x00, 0x26, 0xFF, 0x00]);
        d.extend_from_slice(&[0x75, 0x08, 0x95, 0x04, 0x81, 0x02]);
        d.push(0xC0);

        let desc = descriptor_parser::parse_descriptor(&d).unwrap();
        assert_eq!(desc.collections.len(), 1);
        assert_eq!(desc.collections[0].usage_page, 0xFF00);
    }

    #[test]
    fn parse_nested_collections() {
        let mut d = Vec::new();
        // Outer: Application collection
        d.extend_from_slice(&[0x05, 0x01, 0x09, 0x04, 0xA1, 0x01]);
        // Inner: Physical collection
        d.extend_from_slice(&[0x05, 0x01, 0x09, 0x30, 0xA1, 0x00]);
        // Axis inside physical collection
        d.extend_from_slice(&[0x09, 0x30]); // Usage (X)
        d.extend_from_slice(&[0x15, 0x00, 0x26, 0xFF, 0x03]);
        d.extend_from_slice(&[0x75, 0x10, 0x95, 0x01, 0x81, 0x02]);
        d.push(0xC0); // End physical
        d.push(0xC0); // End application

        let desc = descriptor_parser::parse_descriptor(&d).unwrap();
        // Nested items get flattened into the parent collection
        assert_eq!(desc.collections.len(), 1);
        assert_eq!(desc.collections[0].collection_type, CollectionType::Application);
        assert_eq!(desc.axis_count(), 1);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. Report Reading (6 tests)
// ═══════════════════════════════════════════════════════════════════════════

mod report_reading {
    use super::*;

    #[test]
    fn read_input_report_structure() {
        // Verify descriptor correctly computes total report bit size
        let d = build_descriptor(3, 8, 0);
        let desc = descriptor_parser::parse_descriptor(&d).unwrap();
        // 3 axes × 16 bits + 8 buttons × 1 bit = 56 bits
        assert_eq!(desc.report_size_bits, 56);
    }

    #[test]
    fn field_extraction_from_items() {
        let d = build_descriptor(2, 4, 0);
        let desc = descriptor_parser::parse_descriptor(&d).unwrap();
        let items = &desc.collections[0].items;

        // First item should be the axis group
        let axis_item = &items[0];
        assert_eq!(axis_item.item_type, HidItemType::Input);
        assert_eq!(axis_item.report_size, 16);
        assert_eq!(axis_item.report_count, 2);

        // Second item should be buttons
        let button_item = &items[1];
        assert_eq!(button_item.item_type, HidItemType::Input);
        assert_eq!(button_item.report_size, 1);
        assert_eq!(button_item.report_count, 4);
    }

    #[test]
    fn multi_byte_field_16bit_axis() {
        let d = build_descriptor(1, 0, 0);
        let desc = descriptor_parser::parse_descriptor(&d).unwrap();
        let axis_item = &desc.collections[0].items[0];
        assert_eq!(axis_item.report_size, 16);
        assert_eq!(axis_item.logical_min, 0);
        assert_eq!(axis_item.logical_max, 1023);
    }

    #[test]
    fn signed_field_negative_range() {
        let mut d = Vec::new();
        d.extend_from_slice(&[0x05, 0x01, 0x09, 0x04, 0xA1, 0x01]);
        d.extend_from_slice(&[0x05, 0x01, 0x09, 0x30]); // X axis
        d.extend_from_slice(&[0x15, 0x80]); // Logical Min (-128)
        d.extend_from_slice(&[0x25, 0x7F]); // Logical Max (127)
        d.extend_from_slice(&[0x75, 0x08, 0x95, 0x01, 0x81, 0x02]);
        d.push(0xC0);

        let desc = descriptor_parser::parse_descriptor(&d).unwrap();
        let ranges = desc.axis_ranges();
        assert_eq!(ranges[0], (-128, 127));
    }

    #[test]
    fn padding_bits_counted_in_report_size() {
        // 5 buttons = 5 bits + 3 padding bits
        let d = build_descriptor(0, 5, 0);
        let desc = descriptor_parser::parse_descriptor(&d).unwrap();
        // 5 button bits + 3 padding bits = 8 bits
        assert_eq!(desc.report_size_bits, 8);
    }

    #[test]
    fn boundary_alignment_mixed_sizes() {
        let d = build_descriptor(2, 16, 1);
        let desc = descriptor_parser::parse_descriptor(&d).unwrap();
        // 2 axes × 16 bits = 32 bits
        // 1 hat × 4 bits = 4 bits
        // 16 buttons × 1 bit = 16 bits
        // Total: 32 + 4 + 16 = 52 bits
        assert_eq!(desc.report_size_bits, 52);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. Report Writing (5 tests)
// ═══════════════════════════════════════════════════════════════════════════

mod report_writing {
    use super::*;

    #[test]
    fn construct_output_report() {
        let report = HidReportBuilder::new(4)
            .with_report_id(0x01)
            .set_byte(0, 0xAA)
            .set_byte(1, 0xBB)
            .set_byte(2, 0xCC)
            .set_byte(3, 0xDD)
            .build();
        assert_eq!(report.len(), 5); // 1 ID + 4 data
        assert_eq!(report[0], 0x01);
        assert_eq!(report[1], 0xAA);
        assert_eq!(report[4], 0xDD);
    }

    #[test]
    fn led_control_bits() {
        // Simulate setting individual LED bits
        let report = HidReportBuilder::new(2)
            .with_report_id(0x02)
            .set_bit(0, true) // LED 0 on
            .set_bit(2, true) // LED 2 on
            .set_bit(4, true) // LED 4 on
            .build();
        assert_eq!(report[0], 0x02); // report ID
        assert_eq!(report[1], 0b0001_0101); // bits 0, 2, 4
        assert_eq!(report[2], 0x00);
    }

    #[test]
    fn feature_report_set_u16() {
        let report = HidReportBuilder::new(4)
            .with_report_id(0x03)
            .set_u16_le(0, 0x1234)
            .set_u16_le(2, 0x5678)
            .build();
        assert_eq!(report[0], 0x03);
        assert_eq!(report[1], 0x34); // low byte
        assert_eq!(report[2], 0x12); // high byte
        assert_eq!(report[3], 0x78);
        assert_eq!(report[4], 0x56);
    }

    #[test]
    fn report_size_validation() {
        let builder = HidReportBuilder::new(8);
        assert_eq!(builder.data_len(), 8);
        let report_no_id = builder.build();
        assert_eq!(report_no_id.len(), 8);

        let builder_with_id = HidReportBuilder::new(8).with_report_id(0x01);
        assert_eq!(builder_with_id.data_len(), 8);
        let report_with_id = builder_with_id.build();
        assert_eq!(report_with_id.len(), 9); // 1 ID + 8 data
    }

    #[test]
    fn byte_ordering_little_endian() {
        let report = HidReportBuilder::new(4)
            .set_u16_le(0, 0xBEEF)
            .set_u16_le(2, 0xCAFE)
            .build();
        // Little-endian: low byte first
        assert_eq!(report[0], 0xEF);
        assert_eq!(report[1], 0xBE);
        assert_eq!(report[2], 0xFE);
        assert_eq!(report[3], 0xCA);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. Stable Device ID (5 tests)
// ═══════════════════════════════════════════════════════════════════════════

mod stable_device_id {
    use super::*;

    #[test]
    fn generate_stable_id_from_vid_pid_serial_path() {
        let fp = DeviceFingerprint {
            vid: 0x044F,
            pid: 0x0402,
            serial: Some("ABC123".into()),
            manufacturer: None,
            product: None,
            interface_number: Some(0),
            usage_page: 0x01,
            usage: 0x04,
            usb_path: Some("1-2.3".into()),
        };
        let id = fp.stable_id();
        assert_ne!(id.as_u64(), 0);

        // Same fingerprint → same ID
        let id2 = fp.stable_id();
        assert_eq!(id, id2);
    }

    #[test]
    fn id_persistence_across_reconnect() {
        // Serial-based ID is independent of USB path
        let fp1 = DeviceFingerprint {
            vid: 0x044F,
            pid: 0x0402,
            serial: Some("SN001".into()),
            manufacturer: None,
            product: None,
            interface_number: Some(0),
            usage_page: 0x01,
            usage: 0x04,
            usb_path: Some("1-2.3".into()),
        };
        let fp2 = DeviceFingerprint {
            usb_path: Some("2-5.1".into()), // different port
            ..fp1.clone()
        };

        let id1 = fp1.stable_id();
        let id2 = fp2.stable_id();
        assert_eq!(
            id1, id2,
            "serial-based ID should survive USB port change"
        );
    }

    #[test]
    fn id_uniqueness_across_devices() {
        let ids: Vec<StableDeviceId> = [warthog_fp(), vkb_fp(), t16000m_fp(), virpil_fp(), winwing_fp()]
            .iter()
            .map(|fp| fp.stable_id())
            .collect();

        // All IDs should be distinct
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                assert_ne!(ids[i], ids[j], "IDs for device {i} and {j} must differ");
            }
        }
    }

    #[test]
    fn id_format_is_16_hex_chars() {
        let id = warthog_fp().stable_id();
        let formatted = format!("{id}");
        assert_eq!(formatted.len(), 16);
        assert!(
            formatted.chars().all(|c| c.is_ascii_hexdigit()),
            "ID display should be hex: {formatted}"
        );
    }

    #[test]
    fn missing_serial_uses_path_fallback() {
        let fp_no_serial = DeviceFingerprint {
            vid: 0x044F,
            pid: 0xB679,
            serial: None,
            manufacturer: None,
            product: None,
            interface_number: None,
            usage_page: 0x01,
            usage: 0x04,
            usb_path: Some("1-5.2".into()),
        };
        let fp_different_path = DeviceFingerprint {
            usb_path: Some("2-3.1".into()),
            ..fp_no_serial.clone()
        };

        let id1 = fp_no_serial.stable_id();
        let id2 = fp_different_path.stable_id();
        assert_ne!(
            id1, id2,
            "without serial, different paths should yield different IDs"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. Hot-plug (5 tests)
// ═══════════════════════════════════════════════════════════════════════════

mod hot_plug {
    use super::*;
    use flight_hid::HotplugMonitor;

    #[test]
    fn device_connect_event() {
        let mut monitor = MockHotplugMonitor::new();
        monitor.push_event(HotplugEvent::Connected {
            vid: 0x044F,
            pid: 0x0402,
            path: "/dev/hidraw0".into(),
        });
        let events = monitor.poll_events();
        assert_eq!(events.len(), 1);
        assert!(events[0].is_connect());
        assert_eq!(events[0].vid(), 0x044F);
        assert_eq!(events[0].pid(), 0x0402);
    }

    #[test]
    fn device_disconnect_event() {
        let mut monitor = MockHotplugMonitor::new();
        monitor.push_event(HotplugEvent::Disconnected {
            vid: 0x044F,
            pid: 0x0402,
            path: "/dev/hidraw0".into(),
        });
        let events = monitor.poll_events();
        assert_eq!(events.len(), 1);
        assert!(events[0].is_disconnect());
    }

    #[test]
    fn reconnect_same_device_resets_retry() {
        // Test the ReconnectState reset behavior directly since
        // ReconnectManager.policies is not accessible from external tests.
        use flight_hid::hotplug::ReconnectState;

        let mut state = ReconnectState::new(0x044F, 0x0402, "/dev/hidraw0".into(), 5);
        assert_eq!(state.attempts, 0);

        // Increment attempts so we can observe the reset
        state.increment_attempt();
        state.increment_attempt();
        assert_eq!(state.attempts, 2);
        assert!(state.should_retry());

        // Reset brings attempts back to zero
        state.reset();
        assert_eq!(state.attempts, 0);
    }

    #[test]
    fn rapid_plug_unplug_sequence() {
        let mut monitor = MockHotplugMonitor::new();
        // Rapid connect/disconnect/connect/disconnect
        for i in 0..10 {
            if i % 2 == 0 {
                monitor.push_event(HotplugEvent::Connected {
                    vid: 0x044F,
                    pid: 0x0402,
                    path: format!("/dev/hidraw{i}"),
                });
            } else {
                monitor.push_event(HotplugEvent::Disconnected {
                    vid: 0x044F,
                    pid: 0x0402,
                    path: format!("/dev/hidraw{}", i - 1),
                });
            }
        }
        let events = monitor.poll_events();
        assert_eq!(events.len(), 10);
        // Verify alternating pattern
        for (i, ev) in events.iter().enumerate() {
            if i % 2 == 0 {
                assert!(ev.is_connect(), "event {i} should be connect");
            } else {
                assert!(ev.is_disconnect(), "event {i} should be disconnect");
            }
        }
        // Queue should be drained
        assert!(monitor.poll_events().is_empty());
    }

    #[test]
    fn concurrent_enumeration_during_hotplug() {
        // Simulate a discovery scan while hot-plug events are happening.
        // Since scanner is private in integration tests, we simulate by
        // creating successive discovery instances with different device sets,
        // sharing the same registry.
        let mut registry = DeviceRegistry::new();

        // Phase 1: one device
        let scanner1 = MockScanner::new(vec![warthog_fp()]);
        let mut disc1 = DeviceDiscovery::with_defaults(scanner1, registry.clone());
        let events1 = disc1.poll_events();
        assert_eq!(events1.len(), 1);
        assert!(matches!(&events1[0], DeviceEvent::Connected(_)));

        // Update registry from disc1
        registry = disc1.registry().clone();

        // Phase 2: hot-plug adds a device
        let scanner2 = MockScanner::new(vec![warthog_fp(), vkb_fp()]);
        let mut disc2 = DeviceDiscovery::with_defaults(scanner2, registry.clone());
        let found2 = disc2.scan();
        assert_eq!(found2.len(), 2);
        let new_devices: Vec<_> = found2.iter().filter(|d| d.is_new).collect();
        assert_eq!(new_devices.len(), 1);
        assert_eq!(new_devices[0].fingerprint.vid, 0x231D);

        // Phase 3: hot-plug removes both, adds new one
        registry = disc2.registry().clone();
        let scanner3 = MockScanner::new(vec![t16000m_fp()]);
        let mut disc3 = DeviceDiscovery::with_defaults(scanner3, registry);
        let found3 = disc3.scan();
        assert_eq!(found3.len(), 1);
        assert!(found3[0].is_new);
        assert_eq!(found3[0].fingerprint.vid, 0x044F);
        assert_eq!(found3[0].fingerprint.pid, 0xB679);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Additional depth: cross-cutting edge cases (3 bonus tests)
// ═══════════════════════════════════════════════════════════════════════════

mod cross_cutting {
    use super::*;

    #[test]
    fn device_id_matches_semantics() {
        let a = DeviceId::new(0x044F, 0x0402, Some("SN1".into()), 0x01, 0x04, Some(0));
        let b = DeviceId::new(0x044F, 0x0402, Some("SN1".into()), 0x01, 0x04, Some(0));
        assert!(a.matches(&b));

        // Different serial → no match
        let c = DeviceId::new(0x044F, 0x0402, Some("SN2".into()), 0x01, 0x04, Some(0));
        assert!(!a.matches(&c));

        // Missing serial on one side → still matches
        let d = DeviceId::new(0x044F, 0x0402, None, 0x01, 0x04, Some(0));
        assert!(a.matches(&d));
    }

    #[test]
    fn report_builder_clear_preserves_size_and_id() {
        let mut builder = HidReportBuilder::new(6).with_report_id(0x05);
        builder.set_byte_mut(0, 0xFF);
        builder.set_byte_mut(5, 0xAA);
        builder.clear();

        let report = builder.build();
        assert_eq!(report.len(), 7); // 1 ID + 6 data
        assert_eq!(report[0], 0x05); // ID preserved
        assert!(report[1..].iter().all(|&b| b == 0)); // data cleared
    }

    #[test]
    fn registry_round_trip_preserves_all_fingerprints() {
        let mut reg = DeviceRegistry::new();
        let fps = [warthog_fp(), vkb_fp(), t16000m_fp(), virpil_fp(), winwing_fp()];
        for fp in &fps {
            reg.register(fp.clone());
        }
        assert_eq!(reg.len(), 5);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("depth_devices.json");

        reg.save(&path).unwrap();
        let loaded = DeviceRegistry::load(&path).unwrap();

        assert_eq!(loaded.len(), 5);
        for fp in &fps {
            let id = fp.stable_id();
            let loaded_fp = loaded.lookup(id).expect("device should survive round-trip");
            assert_eq!(loaded_fp.vid, fp.vid);
            assert_eq!(loaded_fp.pid, fp.pid);
            assert_eq!(loaded_fp.serial, fp.serial);
            assert_eq!(loaded_fp.manufacturer, fp.manufacturer);
            assert_eq!(loaded_fp.product, fp.product);
        }
        // dir is cleaned up on drop
    }
}
