// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the HID device layer.
//!
//! Covers device enumeration, stable IDs, HID report parsing, descriptor
//! handling, device capabilities, and error recovery paths.

use flight_hid::descriptor_parser::{
    self, CollectionType, DescriptorError, HidItemType,
};
use flight_hid::device_id::DeviceId;
use flight_hid::discovery::{DeviceDiscovery, DeviceEvent, MockScanner};
use flight_hid::report_builder::HidReportBuilder;
use flight_hid::stable_id::{
    DeviceFingerprint, DeviceRegistry, MatchStrictness, StableDeviceId,
};
use flight_hid::{HidAdapter, HidDeviceInfo, HidOperationResult};
use flight_watchdog::WatchdogSystem;
use std::sync::{Arc, Mutex};

// ── Helpers ──────────────────────────────────────────────────────────────────

fn make_adapter() -> HidAdapter {
    let watchdog = Arc::new(Mutex::new(WatchdogSystem::new()));
    HidAdapter::new(watchdog)
}

fn make_fingerprint(vid: u16, pid: u16, serial: Option<&str>) -> DeviceFingerprint {
    DeviceFingerprint {
        vid,
        pid,
        serial: serial.map(String::from),
        manufacturer: Some("TestMfg".into()),
        product: Some("TestDevice".into()),
        interface_number: None,
        usage_page: 0x01,
        usage: 0x04,
        usb_path: Some(format!("1-{vid}.{pid}")),
    }
}

fn make_device_info(vid: u16, pid: u16, path: &str) -> HidDeviceInfo {
    HidDeviceInfo {
        vendor_id: vid,
        product_id: pid,
        serial_number: None,
        manufacturer: None,
        product_name: None,
        device_path: path.to_string(),
        usage_page: 0x01,
        usage: 0x04,
        report_descriptor: None,
    }
}

/// Build a minimal HID descriptor for a joystick with given axes and buttons.
fn build_joystick_descriptor(axes: u8, buttons: u8) -> Vec<u8> {
    let mut d = vec![
        0x05, 0x01, // Usage Page (Generic Desktop)
        0x09, 0x04, // Usage (Joystick)
        0xA1, 0x01, // Collection (Application)
    ];

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
        d.extend_from_slice(&[0x81, 0x02]); // Input (Data, Variable, Absolute)
    }

    if buttons > 0 {
        d.extend_from_slice(&[0x05, 0x09]); // Usage Page (Button)
        d.push(0x19);
        d.push(0x01); // Usage Min
        d.push(0x29);
        d.push(buttons); // Usage Max
        d.extend_from_slice(&[0x15, 0x00]); // Logical Min (0)
        d.extend_from_slice(&[0x25, 0x01]); // Logical Max (1)
        d.push(0x75);
        d.push(0x01); // Report Size (1)
        d.push(0x95);
        d.push(buttons); // Report Count
        d.extend_from_slice(&[0x81, 0x02]); // Input

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

/// Build a HOTAS descriptor with axes, buttons, and hat switches.
fn build_hotas_descriptor(axes: u8, buttons: u8, hats: u8) -> Vec<u8> {
    let mut d = vec![0x05, 0x01, 0x09, 0x04, 0xA1, 0x01];

    if axes > 0 {
        d.extend_from_slice(&[0x05, 0x01]);
        d.push(0x19);
        d.push(0x30);
        d.push(0x29);
        d.push(0x30 + axes - 1);
        d.extend_from_slice(&[0x15, 0x00]);
        d.extend_from_slice(&[0x26, 0xFF, 0x03]); // Logical Max (1023)
        d.push(0x75);
        d.push(0x10);
        d.push(0x95);
        d.push(axes);
        d.extend_from_slice(&[0x81, 0x02]);
    }

    if hats > 0 {
        d.extend_from_slice(&[0x05, 0x01]);
        d.push(0x09);
        d.push(0x39); // Hat Switch
        d.extend_from_slice(&[0x15, 0x01]); // Logical Min (1)
        d.extend_from_slice(&[0x25, 0x08]); // Logical Max (8)
        d.push(0x75);
        d.push(0x04); // Report Size (4)
        d.push(0x95);
        d.push(hats);
        d.extend_from_slice(&[0x81, 0x42]); // Input (Data, Var, Abs, Null)
    }

    if buttons > 0 {
        d.extend_from_slice(&[0x05, 0x09]);
        d.push(0x19);
        d.push(0x01);
        d.push(0x29);
        d.push(buttons);
        d.extend_from_slice(&[0x15, 0x00]);
        d.extend_from_slice(&[0x25, 0x01]);
        d.push(0x75);
        d.push(0x01);
        d.push(0x95);
        d.push(buttons);
        d.extend_from_slice(&[0x81, 0x02]);

        let pad = (8 - (buttons % 8)) % 8;
        if pad > 0 {
            d.push(0x75);
            d.push(0x01);
            d.push(0x95);
            d.push(pad);
            d.extend_from_slice(&[0x81, 0x01]);
        }
    }

    d.push(0xC0);
    d
}

// ═══════════════════════════════════════════════════════════════════════════════
// 1. Device enumeration
// ═══════════════════════════════════════════════════════════════════════════════

mod enumeration {
    use super::*;

    #[test]
    fn list_devices_returns_expected_structure() {
        let fps = vec![
            make_fingerprint(0x044F, 0x0402, Some("WH001")),
            make_fingerprint(0x231D, 0x0136, Some("VKB001")),
        ];
        let scanner = MockScanner::new(fps);
        let mut disc = DeviceDiscovery::with_defaults(scanner, DeviceRegistry::new());
        let found = disc.scan();

        assert_eq!(found.len(), 2);
        assert_eq!(found[0].fingerprint.vid, 0x044F);
        assert_eq!(found[1].fingerprint.vid, 0x231D);
        assert!(found.iter().all(|d| d.is_new));
    }

    #[test]
    fn filter_by_vid_pid() {
        let fps = vec![
            make_fingerprint(0x044F, 0x0402, Some("WH001")),
            make_fingerprint(0x231D, 0x0136, Some("VKB001")),
            make_fingerprint(0x044F, 0xB679, None),
        ];
        let scanner = MockScanner::new(fps);
        let mut disc = DeviceDiscovery::with_defaults(scanner, DeviceRegistry::new());
        let found = disc.scan();

        let thrustmaster: Vec<_> = found
            .iter()
            .filter(|d| d.fingerprint.vid == 0x044F)
            .collect();
        assert_eq!(thrustmaster.len(), 2);

        let vkb: Vec<_> = found
            .iter()
            .filter(|d| d.fingerprint.vid == 0x231D && d.fingerprint.pid == 0x0136)
            .collect();
        assert_eq!(vkb.len(), 1);
    }

    #[test]
    fn multiple_devices_same_vid_pid_get_unique_ids() {
        let fps = vec![
            make_fingerprint(0x044F, 0x0402, Some("SN_A")),
            make_fingerprint(0x044F, 0x0402, Some("SN_B")),
        ];
        let scanner = MockScanner::new(fps);
        let mut disc = DeviceDiscovery::with_defaults(scanner, DeviceRegistry::new());
        let found = disc.scan();

        assert_eq!(found.len(), 2);
        assert_ne!(
            found[0].stable_id, found[1].stable_id,
            "devices with same VID/PID but different serials must get different IDs"
        );
    }

    #[test]
    fn empty_device_list_handled_gracefully() {
        let scanner = MockScanner::new(vec![]);
        let mut disc = DeviceDiscovery::with_defaults(scanner, DeviceRegistry::new());
        let found = disc.scan();

        assert!(found.is_empty());
        assert!(disc.connected_ids().is_empty());
        assert!(disc.registry().is_empty());
    }

    #[test]
    fn scan_is_idempotent_for_same_devices() {
        let fps = vec![make_fingerprint(0x044F, 0x0402, Some("WH001"))];
        let scanner = MockScanner::new(fps);
        let mut disc = DeviceDiscovery::with_defaults(scanner, DeviceRegistry::new());

        let first = disc.scan();
        let second = disc.scan();

        assert!(first[0].is_new);
        assert!(!second[0].is_new, "second scan should not mark as new");
        assert_eq!(first[0].stable_id, second[0].stable_id);
    }

    #[test]
    fn connect_disconnect_events_track_correctly() {
        let scanner = MockScanner::new(vec![]);
        let mut disc = DeviceDiscovery::with_defaults(scanner, DeviceRegistry::new());

        // Initial: empty
        let events = disc.poll_events();
        assert!(events.is_empty());

        // Connect two devices
        disc.scanner_mut().set_devices(vec![
            make_fingerprint(0x044F, 0x0402, Some("SN1")),
            make_fingerprint(0x231D, 0x0136, Some("SN2")),
        ]);
        let events = disc.poll_events();
        assert_eq!(events.len(), 2);
        let connects = events
            .iter()
            .filter(|e| matches!(e, DeviceEvent::Connected(_)))
            .count();
        assert_eq!(connects, 2);

        // Disconnect one
        disc.scanner_mut()
            .set_devices(vec![make_fingerprint(0x231D, 0x0136, Some("SN2"))]);
        let events = disc.poll_events();
        let disconnects = events
            .iter()
            .filter(|e| matches!(e, DeviceEvent::Disconnected(_)))
            .count();
        assert_eq!(disconnects, 1);
    }

    #[test]
    fn reconnected_device_is_not_new() {
        let fp = make_fingerprint(0x044F, 0x0402, Some("SN1"));
        let scanner = MockScanner::new(vec![fp.clone()]);
        let mut disc = DeviceDiscovery::with_defaults(scanner, DeviceRegistry::new());

        disc.poll_events(); // connect

        disc.scanner_mut().set_devices(vec![]); // disconnect
        disc.poll_events();

        disc.scanner_mut().set_devices(vec![fp]); // reconnect
        let events = disc.poll_events();
        assert_eq!(events.len(), 1);
        if let DeviceEvent::Connected(d) = &events[0] {
            assert!(!d.is_new, "reconnected device must not be marked new");
        } else {
            panic!("expected Connected event");
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. Stable device IDs
// ═══════════════════════════════════════════════════════════════════════════════

mod stable_ids {
    use super::*;

    #[test]
    fn same_device_same_stable_id_across_sessions() {
        let id1 = StableDeviceId::new(0x044F, 0x0402, Some("WH001"), Some(0), None);
        let id2 = StableDeviceId::new(0x044F, 0x0402, Some("WH001"), Some(0), None);
        assert_eq!(id1, id2, "same inputs must produce identical stable IDs");
    }

    #[test]
    fn different_usb_port_same_logical_id_with_serial() {
        let id_port_a = StableDeviceId::new(0x044F, 0x0402, Some("WH001"), None, Some("1-2.3"));
        let id_port_b = StableDeviceId::new(0x044F, 0x0402, Some("WH001"), None, Some("1-4.1"));
        assert_eq!(
            id_port_a, id_port_b,
            "with serial, USB port should not affect stable ID"
        );
    }

    #[test]
    fn different_port_different_id_without_serial() {
        let id_a = StableDeviceId::new(0x044F, 0x0402, None, None, Some("1-2.3"));
        let id_b = StableDeviceId::new(0x044F, 0x0402, None, None, Some("1-4.1"));
        assert_ne!(
            id_a, id_b,
            "without serial, different ports must yield different IDs"
        );
    }

    #[test]
    fn id_generation_is_deterministic() {
        let inputs = [
            (0x044F_u16, 0x0402_u16, Some("SN1"), Some(0_u8), None),
            (0x231D, 0x0136, Some("VKB"), None, Some("1-4")),
            (0x044F, 0xB679, None, None, Some("1-5.2")),
        ];

        for &(vid, pid, serial, iface, path) in &inputs {
            let a = StableDeviceId::new(vid, pid, serial, iface, path);
            let b = StableDeviceId::new(vid, pid, serial, iface, path);
            assert_eq!(a, b, "ID generation must be a pure function of inputs");
        }
    }

    #[test]
    fn id_pure_function_property() {
        // Same inputs from different construction paths must agree.
        let fp = DeviceFingerprint {
            vid: 0x044F,
            pid: 0x0402,
            serial: Some("WH001".into()),
            manufacturer: Some("Thrustmaster".into()),
            product: Some("Warthog".into()),
            interface_number: Some(0),
            usage_page: 0x01,
            usage: 0x04,
            usb_path: Some("1-2.3".into()),
        };

        let id_from_fp = StableDeviceId::from_fingerprint(&fp);
        let id_direct = StableDeviceId::new(
            fp.vid,
            fp.pid,
            fp.serial.as_deref(),
            fp.interface_number,
            fp.usb_path.as_deref(),
        );
        assert_eq!(id_from_fp, id_direct);
    }

    #[test]
    fn device_id_matches_ignores_missing_serial() {
        let a = DeviceId::new(0x044F, 0x0402, Some("SN1".into()), 0x01, 0x04, None);
        let b = DeviceId::new(0x044F, 0x0402, None, 0x01, 0x04, None);
        assert!(a.matches(&b), "missing serial should not prevent match");
    }

    #[test]
    fn device_id_matches_ignores_missing_interface() {
        let a = DeviceId::new(0x044F, 0x0402, None, 0x01, 0x04, Some(0));
        let b = DeviceId::new(0x044F, 0x0402, None, 0x01, 0x04, None);
        assert!(
            a.matches(&b),
            "missing interface should not prevent match"
        );
    }

    #[test]
    fn device_id_hash_differs_by_serial_presence() {
        let with = DeviceId::new(0x044F, 0x0402, Some("".into()), 0x01, 0x04, None);
        let without = DeviceId::new(0x044F, 0x0402, None, 0x01, 0x04, None);
        assert_ne!(
            with.stable_hash(),
            without.stable_hash(),
            "Some(\"\") vs None must produce different hashes"
        );
    }

    #[test]
    fn fingerprint_registry_round_trip() {
        let mut reg = DeviceRegistry::new();
        let fp = make_fingerprint(0x044F, 0x0402, Some("SN1"));
        let id = reg.register(fp.clone());
        let retrieved = reg.lookup(id).unwrap();
        assert_eq!(retrieved.vid, fp.vid);
        assert_eq!(retrieved.pid, fp.pid);
        assert_eq!(retrieved.serial, fp.serial);
    }

    #[test]
    fn match_strictness_exact_and_relaxed() {
        let id = StableDeviceId::new(0x044F, 0x0402, Some("SN1"), None, None);
        let other = StableDeviceId::new(0x044F, 0x0402, Some("SN2"), None, None);

        assert!(id.matches(id, MatchStrictness::Exact));
        assert!(id.matches(id, MatchStrictness::Relaxed));
        assert!(!id.matches(other, MatchStrictness::Exact));
    }

    #[test]
    fn stable_id_display_is_16_hex_chars() {
        let id = StableDeviceId::new(0x044F, 0x0402, Some("WH001"), None, None);
        let display = format!("{id}");
        assert_eq!(display.len(), 16);
        assert!(
            display.chars().all(|c| c.is_ascii_hexdigit()),
            "display should be hex only"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. HID report parsing (via HidReportBuilder)
// ═══════════════════════════════════════════════════════════════════════════════

mod report_parsing {
    use super::*;

    #[test]
    fn parse_5_byte_report() {
        let report = HidReportBuilder::new(5).set_byte(0, 0xAA).build();
        assert_eq!(report.len(), 5);
        assert_eq!(report[0], 0xAA);
        assert!(report[1..].iter().all(|&b| b == 0));
    }

    #[test]
    fn parse_64_byte_report() {
        let report = HidReportBuilder::new(64)
            .set_byte(0, 0x01)
            .set_byte(63, 0xFF)
            .build();
        assert_eq!(report.len(), 64);
        assert_eq!(report[0], 0x01);
        assert_eq!(report[63], 0xFF);
    }

    #[test]
    fn extract_16bit_axes_from_report() {
        let report = HidReportBuilder::new(6)
            .set_u16_le(0, 0x8000) // axis 0: midpoint
            .set_u16_le(2, 0x0000) // axis 1: minimum
            .set_u16_le(4, 0xFFFF) // axis 2: maximum
            .build();

        let axis0 = u16::from_le_bytes([report[0], report[1]]);
        let axis1 = u16::from_le_bytes([report[2], report[3]]);
        let axis2 = u16::from_le_bytes([report[4], report[5]]);

        assert_eq!(axis0, 0x8000);
        assert_eq!(axis1, 0x0000);
        assert_eq!(axis2, 0xFFFF);
    }

    #[test]
    fn extract_buttons_from_report() {
        let report = HidReportBuilder::new(2)
            .set_bit(0, true) // button 1
            .set_bit(1, false) // button 2
            .set_bit(2, true) // button 3
            .set_bit(7, true) // button 8
            .set_bit(8, true) // button 9
            .build();

        assert_eq!(report[0] & 0x01, 1, "button 1 pressed");
        assert_eq!(report[0] & 0x02, 0, "button 2 released");
        assert_eq!(report[0] & 0x04, 4, "button 3 pressed");
        assert_eq!(report[0] & 0x80, 0x80, "button 8 pressed");
        assert_eq!(report[1] & 0x01, 1, "button 9 pressed");
    }

    #[test]
    fn extract_hat_from_report_nibble() {
        // Hat switches are often 4-bit values (0-8)
        let report = HidReportBuilder::new(1).set_byte(0, 0x03).build();
        let hat = report[0] & 0x0F;
        assert_eq!(hat, 3, "hat direction East");
    }

    #[test]
    fn boundary_values_16bit() {
        let report_min = HidReportBuilder::new(2).set_u16_le(0, 0x0000).build();
        let report_max = HidReportBuilder::new(2).set_u16_le(0, 0xFFFF).build();

        assert_eq!(
            u16::from_le_bytes([report_min[0], report_min[1]]),
            0x0000
        );
        assert_eq!(
            u16::from_le_bytes([report_max[0], report_max[1]]),
            0xFFFF
        );
    }

    #[test]
    fn report_with_report_id_prefix() {
        let report = HidReportBuilder::new(3)
            .with_report_id(0x01)
            .set_byte(0, 0xAB)
            .build();

        assert_eq!(report.len(), 4, "report ID adds one byte");
        assert_eq!(report[0], 0x01, "first byte is report ID");
        assert_eq!(report[1], 0xAB, "data starts at index 1");
    }

    #[test]
    fn push_bit_builds_sequential_fields() {
        let report = HidReportBuilder::new(2)
            .push_bit(true) // bit 0
            .push_bit(true) // bit 1
            .push_bit(false) // bit 2
            .push_bit(true) // bit 3
            .push_bit(false) // bit 4
            .push_bit(false) // bit 5
            .push_bit(false) // bit 6
            .push_bit(false) // bit 7
            .push_bit(true) // bit 8 (byte 1)
            .build();

        assert_eq!(report[0], 0x0B); // bits: 1101_0000 reversed = 0000_1011
        assert_eq!(report[1], 0x01);
    }

    #[test]
    fn clear_resets_without_changing_size() {
        let mut builder = HidReportBuilder::new(4).with_report_id(0x02);
        builder.set_byte_mut(0, 0xFF);
        builder.set_byte_mut(1, 0xAA);
        builder.clear();

        let report = builder.build();
        assert_eq!(report.len(), 5); // report ID + 4 data bytes
        assert_eq!(report[0], 0x02); // ID preserved
        assert!(report[1..].iter().all(|&b| b == 0), "data should be zeroed");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. Descriptor parsing
// ═══════════════════════════════════════════════════════════════════════════════

mod descriptor_parsing {
    use super::*;

    #[test]
    fn parse_standard_2_axis_4_button_joystick() {
        let desc_bytes = build_joystick_descriptor(2, 4);
        let desc = descriptor_parser::parse_descriptor(&desc_bytes).unwrap();

        assert_eq!(desc.axis_count(), 2);
        assert_eq!(desc.button_count(), 4);
        assert_eq!(desc.hat_count(), 0);
    }

    #[test]
    fn parse_6_axis_32_button_1_hat_hotas() {
        let desc_bytes = build_hotas_descriptor(6, 32, 1);
        let desc = descriptor_parser::parse_descriptor(&desc_bytes).unwrap();

        assert_eq!(desc.axis_count(), 6);
        assert_eq!(desc.button_count(), 32);
        assert_eq!(desc.hat_count(), 1);
    }

    #[test]
    fn extract_usage_pages_and_collection_info() {
        let desc_bytes = build_joystick_descriptor(3, 8);
        let desc = descriptor_parser::parse_descriptor(&desc_bytes).unwrap();

        assert_eq!(desc.collections.len(), 1);
        let col = &desc.collections[0];
        assert_eq!(col.usage_page, 0x01); // Generic Desktop
        assert_eq!(col.usage, 0x04); // Joystick
        assert_eq!(col.collection_type, CollectionType::Application);
    }

    #[test]
    fn malformed_descriptor_truncated_no_panic() {
        // Descriptor that claims a data byte but doesn't have it
        let truncated = [0x05]; // Usage Page prefix, missing data
        let result = descriptor_parser::parse_descriptor(&truncated);
        assert!(matches!(result, Err(DescriptorError::Truncated { .. })));
    }

    #[test]
    fn malformed_descriptor_unmatched_end() {
        let bad = [0xC0]; // End Collection with no open collection
        let result = descriptor_parser::parse_descriptor(&bad);
        assert!(matches!(result, Err(DescriptorError::UnmatchedEnd { .. })));
    }

    #[test]
    fn malformed_descriptor_unclosed_collection() {
        let bad = [0x05, 0x01, 0x09, 0x04, 0xA1, 0x01]; // open collection, never closed
        let result = descriptor_parser::parse_descriptor(&bad);
        assert!(matches!(
            result,
            Err(DescriptorError::UnclosedCollection { count: 1 })
        ));
    }

    #[test]
    fn empty_descriptor_returns_error_not_panic() {
        let result = descriptor_parser::parse_descriptor(&[]);
        assert_eq!(result, Err(DescriptorError::Empty));
    }

    #[test]
    fn axis_ranges_reflect_descriptor_logical_bounds() {
        let desc_bytes = build_joystick_descriptor(3, 0);
        let desc = descriptor_parser::parse_descriptor(&desc_bytes).unwrap();
        let ranges = desc.axis_ranges();

        assert_eq!(ranges.len(), 3);
        for &(lo, hi) in &ranges {
            assert_eq!(lo, 0);
            assert_eq!(hi, 1023);
        }
    }

    #[test]
    fn signed_axis_range_negative_minimum() {
        let mut d = Vec::new();
        d.extend_from_slice(&[0x05, 0x01, 0x09, 0x04, 0xA1, 0x01]);
        d.extend_from_slice(&[0x05, 0x01]);
        d.extend_from_slice(&[0x09, 0x30]); // Usage (X)
        d.extend_from_slice(&[0x15, 0x80]); // Logical Min (-128)
        d.extend_from_slice(&[0x25, 0x7F]); // Logical Max (127)
        d.extend_from_slice(&[0x75, 0x08]); // Report Size (8)
        d.extend_from_slice(&[0x95, 0x01]); // Report Count (1)
        d.extend_from_slice(&[0x81, 0x02]); // Input
        d.push(0xC0);

        let desc = descriptor_parser::parse_descriptor(&d).unwrap();
        let ranges = desc.axis_ranges();
        assert_eq!(ranges[0], (-128, 127));
    }

    #[test]
    fn report_size_bits_computed_correctly() {
        let desc_bytes = build_joystick_descriptor(2, 8);
        let desc = descriptor_parser::parse_descriptor(&desc_bytes).unwrap();

        // 2 axes × 16 bits + 8 buttons × 1 bit = 40 bits
        assert_eq!(desc.report_size_bits, 40);
    }

    #[test]
    fn multiple_hat_switches() {
        let desc_bytes = build_hotas_descriptor(4, 8, 2);
        let desc = descriptor_parser::parse_descriptor(&desc_bytes).unwrap();
        assert_eq!(desc.hat_count(), 2);
    }

    #[test]
    fn descriptor_display_includes_counts() {
        let desc_bytes = build_hotas_descriptor(6, 32, 1);
        let desc = descriptor_parser::parse_descriptor(&desc_bytes).unwrap();
        let display = format!("{desc}");

        assert!(display.contains("axes: 6"));
        assert!(display.contains("buttons: 32"));
        assert!(display.contains("hats: 1"));
    }

    #[test]
    fn long_item_skipped_without_error() {
        let mut d = Vec::new();
        d.extend_from_slice(&[0x05, 0x01, 0x09, 0x04, 0xA1, 0x01]);
        // Long item: prefix=0xFE, length=3, tag=0x10, data=[0xAA, 0xBB, 0xCC]
        d.extend_from_slice(&[0xFE, 0x03, 0x10, 0xAA, 0xBB, 0xCC]);
        d.push(0xC0);

        let desc = descriptor_parser::parse_descriptor(&d).unwrap();
        assert_eq!(desc.collections.len(), 1);
    }

    #[test]
    fn output_items_parsed_alongside_input() {
        let mut d = Vec::new();
        d.extend_from_slice(&[0x05, 0x01, 0x09, 0x04, 0xA1, 0x01]);

        // 1 input axis
        d.extend_from_slice(&[0x05, 0x01, 0x09, 0x30]);
        d.extend_from_slice(&[0x15, 0x00, 0x26, 0xFF, 0x03]);
        d.extend_from_slice(&[0x75, 0x10, 0x95, 0x01, 0x81, 0x02]);

        // 1 output byte (LED)
        d.extend_from_slice(&[0x05, 0x08, 0x09, 0x01]);
        d.extend_from_slice(&[0x75, 0x08, 0x95, 0x01, 0x91, 0x02]);

        d.push(0xC0);

        let desc = descriptor_parser::parse_descriptor(&d).unwrap();
        assert_eq!(desc.axis_count(), 1);
        let output_items: Vec<_> = desc.collections[0]
            .items
            .iter()
            .filter(|i| i.item_type == HidItemType::Output)
            .collect();
        assert_eq!(output_items.len(), 1);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. Device capabilities
// ═══════════════════════════════════════════════════════════════════════════════

mod capabilities {
    use super::*;

    #[test]
    fn capability_report_lists_axes_buttons_hats() {
        let desc_bytes = build_hotas_descriptor(6, 32, 1);
        let desc = descriptor_parser::parse_descriptor(&desc_bytes).unwrap();

        assert_eq!(desc.axis_count(), 6);
        assert_eq!(desc.button_count(), 32);
        assert_eq!(desc.hat_count(), 1);
    }

    #[test]
    fn axis_only_device() {
        let desc_bytes = build_joystick_descriptor(4, 0);
        let desc = descriptor_parser::parse_descriptor(&desc_bytes).unwrap();

        assert_eq!(desc.axis_count(), 4);
        assert_eq!(desc.button_count(), 0);
        assert_eq!(desc.hat_count(), 0);
    }

    #[test]
    fn button_only_device() {
        let desc_bytes = build_joystick_descriptor(0, 16);
        let desc = descriptor_parser::parse_descriptor(&desc_bytes).unwrap();

        assert_eq!(desc.axis_count(), 0);
        assert_eq!(desc.button_count(), 16);
    }

    #[test]
    fn capabilities_consistent_for_same_device_type() {
        // Parsing the same descriptor twice must yield identical capabilities.
        let desc_bytes = build_hotas_descriptor(6, 32, 2);

        let desc1 = descriptor_parser::parse_descriptor(&desc_bytes).unwrap();
        let desc2 = descriptor_parser::parse_descriptor(&desc_bytes).unwrap();

        assert_eq!(desc1.axis_count(), desc2.axis_count());
        assert_eq!(desc1.button_count(), desc2.button_count());
        assert_eq!(desc1.hat_count(), desc2.hat_count());
        assert_eq!(desc1.report_size_bits, desc2.report_size_bits);
        assert_eq!(desc1.axis_ranges(), desc2.axis_ranges());
    }

    #[test]
    fn quirks_database_detects_known_devices() {
        use flight_hid::quirks::QuirksDatabase;

        let db = QuirksDatabase::with_defaults();

        // Known device: T16000M
        assert!(db.has_quirks(0x044F, 0xB10A));

        // Unknown device
        assert!(!db.has_quirks(0xFFFF, 0xFFFF));
    }

    #[test]
    fn polling_rate_implied_by_report_size() {
        // Larger reports imply more data per poll cycle.
        let small = build_joystick_descriptor(2, 4);
        let large = build_hotas_descriptor(8, 64, 2);

        let desc_small = descriptor_parser::parse_descriptor(&small).unwrap();
        let desc_large = descriptor_parser::parse_descriptor(&large).unwrap();

        assert!(
            desc_large.report_size_bits > desc_small.report_size_bits,
            "larger device should have more report bits"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 6. Error handling
// ═══════════════════════════════════════════════════════════════════════════════

mod error_handling {
    use super::*;

    #[test]
    fn device_not_found_returns_appropriate_error() {
        let adapter = make_adapter();
        assert!(
            adapter.get_device_info("nonexistent/device/path").is_none(),
            "querying a missing device should return None"
        );
    }

    #[test]
    fn read_from_missing_device_returns_error_variant() {
        let mut adapter = make_adapter();
        let mut buf = [0u8; 64];
        let result = adapter.read_input("missing/device", &mut buf).unwrap();
        assert!(
            matches!(result, HidOperationResult::Error { .. }),
            "read on missing device path must return Error variant"
        );
    }

    #[test]
    fn write_to_missing_device_returns_error_variant() {
        let mut adapter = make_adapter();
        let data = [0u8; 8];
        let result = adapter.write_output("missing/device", &data).unwrap();
        assert!(
            matches!(result, HidOperationResult::Error { .. }),
            "write on missing device path must return Error variant"
        );
    }

    #[test]
    fn device_disconnect_during_poll_produces_event() {
        let fp = make_fingerprint(0x044F, 0x0402, Some("SN1"));
        let scanner = MockScanner::new(vec![fp]);
        let mut disc = DeviceDiscovery::with_defaults(scanner, DeviceRegistry::new());

        disc.poll_events(); // initial connect

        // Simulate disconnection
        disc.scanner_mut().set_devices(vec![]);
        let events = disc.poll_events();

        assert_eq!(events.len(), 1);
        assert!(
            matches!(&events[0], DeviceEvent::Disconnected(_)),
            "disconnect must produce Disconnected event"
        );
    }

    #[test]
    fn registry_load_nonexistent_file_returns_error() {
        let result = DeviceRegistry::load(std::path::Path::new("/no/such/file.json"));
        assert!(result.is_err());
    }

    #[test]
    fn registry_load_invalid_json_returns_error() {
        let dir = std::env::temp_dir().join("flight_hid_depth_test_invalid");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad.json");
        std::fs::write(&path, "{{invalid json}}").unwrap();

        let result = DeviceRegistry::load(&path);
        assert!(result.is_err(), "invalid JSON must produce an error");

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn descriptor_errors_are_displayable() {
        let errors = [
            DescriptorError::Empty,
            DescriptorError::Truncated { offset: 42 },
            DescriptorError::UnmatchedEnd { offset: 10 },
            DescriptorError::UnclosedCollection { count: 2 },
        ];

        for err in &errors {
            let msg = format!("{err}");
            assert!(!msg.is_empty(), "error Display must produce non-empty string");
        }
    }

    #[test]
    fn adapter_statistics_after_failed_operations() {
        let mut adapter = make_adapter();
        adapter
            .register_device(make_device_info(0x01, 0x01, "test/dev"))
            .unwrap();

        let stats = adapter.get_statistics();
        assert_eq!(stats.total_devices, 1);
        assert_eq!(stats.total_operations, 0);
    }
}
