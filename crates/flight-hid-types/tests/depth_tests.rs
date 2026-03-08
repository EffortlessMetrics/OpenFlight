// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for flight-hid-types: usage tables, descriptor parsing,
//! bit-level extraction, collection nesting, error handling, and
//! property-based fuzzing.

use flight_hid_types::*;

// ═══════════════════════════════════════════════════════════════════════════
// §1  Usage page constants
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn usage_page_standard_values() {
    assert_eq!(usage_page::GENERIC_DESKTOP, 0x01);
    assert_eq!(usage_page::SIMULATION, 0x02);
    assert_eq!(usage_page::VR, 0x03);
    assert_eq!(usage_page::SPORT, 0x04);
    assert_eq!(usage_page::GAME, 0x05);
    assert_eq!(usage_page::GENERIC_DEVICE, 0x06);
    assert_eq!(usage_page::KEYBOARD, 0x07);
    assert_eq!(usage_page::LED, 0x08);
    assert_eq!(usage_page::BUTTON, 0x09);
    assert_eq!(usage_page::ORDINAL, 0x0A);
    assert_eq!(usage_page::CONSUMER, 0x0C);
    assert_eq!(usage_page::PID, 0x0F);
}

#[test]
fn usage_page_vendor_range() {
    assert_eq!(usage_page::VENDOR_MIN, 0xFF00);
    assert_eq!(usage_page::VENDOR_MAX, 0xFFFF);
    assert!(usage_page::is_vendor(0xFF00));
    assert!(usage_page::is_vendor(0xFFFF));
    assert!(!usage_page::is_vendor(0x01));
    assert!(!usage_page::is_vendor(0xFEFF));
}

// ═══════════════════════════════════════════════════════════════════════════
// §2  Generic Desktop usage IDs
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn usage_desktop_device_types() {
    assert_eq!(usage_desktop::POINTER, 0x01);
    assert_eq!(usage_desktop::MOUSE, 0x02);
    assert_eq!(usage_desktop::JOYSTICK, 0x04);
    assert_eq!(usage_desktop::GAME_PAD, 0x05);
    assert_eq!(usage_desktop::KEYBOARD, 0x06);
    assert_eq!(usage_desktop::MULTI_AXIS, 0x08);
}

#[test]
fn usage_desktop_axis_ids() {
    assert_eq!(usage_desktop::X, 0x30);
    assert_eq!(usage_desktop::Y, 0x31);
    assert_eq!(usage_desktop::Z, 0x32);
    assert_eq!(usage_desktop::RX, 0x33);
    assert_eq!(usage_desktop::RY, 0x34);
    assert_eq!(usage_desktop::RZ, 0x35);
    assert_eq!(usage_desktop::SLIDER, 0x36);
    assert_eq!(usage_desktop::DIAL, 0x37);
    assert_eq!(usage_desktop::WHEEL, 0x38);
    assert_eq!(usage_desktop::HAT_SWITCH, 0x39);
}

#[test]
fn usage_desktop_axes_are_contiguous() {
    let axes = [
        usage_desktop::X,
        usage_desktop::Y,
        usage_desktop::Z,
        usage_desktop::RX,
        usage_desktop::RY,
        usage_desktop::RZ,
        usage_desktop::SLIDER,
        usage_desktop::DIAL,
        usage_desktop::WHEEL,
        usage_desktop::HAT_SWITCH,
    ];
    for pair in axes.windows(2) {
        assert_eq!(pair[1], pair[0] + 1, "axis IDs should be contiguous");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// §3  Simulation Controls usage IDs
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn usage_simulation_values() {
    assert_eq!(usage_simulation::FLIGHT_SIMULATION, 0x01);
    assert_eq!(usage_simulation::AUTOMOBILE_SIMULATION, 0x02);
    assert_eq!(usage_simulation::AILERON, 0xB0);
    assert_eq!(usage_simulation::AILERON_TRIM, 0xB1);
    assert_eq!(usage_simulation::ELEVATOR, 0xB8);
    assert_eq!(usage_simulation::ELEVATOR_TRIM, 0xB9);
    assert_eq!(usage_simulation::RUDDER, 0xBA);
    assert_eq!(usage_simulation::THROTTLE, 0xBB);
    assert_eq!(usage_simulation::FLIGHT_COMMUNICATIONS, 0xBC);
}

// ═══════════════════════════════════════════════════════════════════════════
// §4  ReportType enum
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn report_type_display() {
    assert_eq!(ReportType::Input.to_string(), "Input");
    assert_eq!(ReportType::Output.to_string(), "Output");
    assert_eq!(ReportType::Feature.to_string(), "Feature");
}

#[test]
fn report_type_equality_and_copy() {
    let a = ReportType::Input;
    let b = a; // Copy
    assert_eq!(a, b);
    assert_ne!(ReportType::Input, ReportType::Output);
}

#[test]
fn report_type_debug() {
    assert_eq!(format!("{:?}", ReportType::Feature), "Feature");
}

// ═══════════════════════════════════════════════════════════════════════════
// §5  CollectionType enum
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn collection_type_from_value_known() {
    assert_eq!(CollectionType::from_value(0x00), CollectionType::Physical);
    assert_eq!(
        CollectionType::from_value(0x01),
        CollectionType::Application
    );
    assert_eq!(CollectionType::from_value(0x02), CollectionType::Logical);
    assert_eq!(CollectionType::from_value(0x03), CollectionType::Report);
    assert_eq!(
        CollectionType::from_value(0x04),
        CollectionType::NamedArray
    );
    assert_eq!(
        CollectionType::from_value(0x05),
        CollectionType::UsageSwitch
    );
    assert_eq!(
        CollectionType::from_value(0x06),
        CollectionType::UsageModifier
    );
}

#[test]
fn collection_type_from_value_other() {
    assert_eq!(
        CollectionType::from_value(0x80),
        CollectionType::Other(0x80)
    );
    assert_eq!(
        CollectionType::from_value(0xFF),
        CollectionType::Other(0xFF)
    );
}

#[test]
fn collection_type_display() {
    assert_eq!(CollectionType::Application.to_string(), "Application");
    assert_eq!(CollectionType::Physical.to_string(), "Physical");
    assert_eq!(CollectionType::Other(0x42).to_string(), "Other(0x42)");
}

// ═══════════════════════════════════════════════════════════════════════════
// §6  MainItemFlags
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn main_item_flags_zero_is_data_array_absolute() {
    let f = MainItemFlags(0x00);
    assert!(!f.is_constant());
    assert!(!f.is_variable());
    assert!(!f.is_relative());
    assert!(!f.is_wrap());
    assert!(!f.is_nonlinear());
    assert!(!f.is_no_preferred());
    assert!(!f.is_null_state());
    assert!(!f.is_buffered_bytes());
}

#[test]
fn main_item_flags_constant() {
    let f = MainItemFlags(0x01);
    assert!(f.is_constant());
    assert!(!f.is_variable());
}

#[test]
fn main_item_flags_variable_absolute() {
    let f = MainItemFlags(0x02); // Data, Variable, Absolute
    assert!(!f.is_constant());
    assert!(f.is_variable());
    assert!(!f.is_relative());
}

#[test]
fn main_item_flags_all_set() {
    let f = MainItemFlags(0x1FF);
    assert!(f.is_constant());
    assert!(f.is_variable());
    assert!(f.is_relative());
    assert!(f.is_wrap());
    assert!(f.is_nonlinear());
    assert!(f.is_no_preferred());
    assert!(f.is_null_state());
    assert!(f.is_buffered_bytes());
}

#[test]
fn main_item_flags_null_state() {
    let f = MainItemFlags(0x42); // Variable + Null State
    assert!(f.is_variable());
    assert!(f.is_null_state());
    assert!(!f.is_constant());
}

// ═══════════════════════════════════════════════════════════════════════════
// §7  ReportField helpers
// ═══════════════════════════════════════════════════════════════════════════

fn make_field(usage_page: u16, usage: u16, size: u32, count: u32) -> ReportField {
    ReportField {
        report_type: ReportType::Input,
        flags: MainItemFlags(0x02),
        usage_page,
        usage,
        logical_min: 0,
        logical_max: 1023,
        physical_min: 0,
        physical_max: 0,
        report_size: size,
        report_count: count,
        report_id: None,
    }
}

#[test]
fn report_field_total_bits() {
    let f = make_field(usage_page::GENERIC_DESKTOP, usage_desktop::X, 16, 3);
    assert_eq!(f.total_bits(), 48);
}

#[test]
fn report_field_is_button() {
    let f = make_field(usage_page::BUTTON, 0x01, 1, 8);
    assert!(f.is_button());
    assert!(!f.is_hat());
    assert!(!f.is_axis());
}

#[test]
fn report_field_is_hat() {
    let f = make_field(
        usage_page::GENERIC_DESKTOP,
        usage_desktop::HAT_SWITCH,
        4,
        1,
    );
    assert!(f.is_hat());
    assert!(!f.is_button());
    assert!(!f.is_axis());
}

#[test]
fn report_field_is_axis() {
    let f = make_field(usage_page::GENERIC_DESKTOP, usage_desktop::X, 16, 1);
    assert!(f.is_axis());
    assert!(!f.is_button());
    assert!(!f.is_hat());
}

#[test]
fn report_field_single_bit_non_button_is_not_axis() {
    let f = make_field(usage_page::GENERIC_DESKTOP, usage_desktop::X, 1, 1);
    assert!(!f.is_axis(), "1-bit field should not be classified as axis");
}

// ═══════════════════════════════════════════════════════════════════════════
// §8  Descriptor builder helpers
// ═══════════════════════════════════════════════════════════════════════════

/// Build a minimal joystick descriptor with N axes (16-bit) and M buttons.
fn joystick_descriptor(axes: u8, buttons: u8) -> Vec<u8> {
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
        d.push(0x01);
        d.push(0x29);
        d.push(buttons);
        d.extend_from_slice(&[0x15, 0x00, 0x25, 0x01]);
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
            d.extend_from_slice(&[0x81, 0x01]); // Input (Constant)
        }
    }

    d.push(0xC0); // End Collection
    d
}

/// Build a HOTAS-style descriptor with axes, buttons, and hats.
fn hotas_descriptor(axes: u8, buttons: u8, hats: u8) -> Vec<u8> {
    let mut d = vec![0x05, 0x01, 0x09, 0x04, 0xA1, 0x01];

    if axes > 0 {
        d.extend_from_slice(&[0x05, 0x01]);
        d.push(0x19);
        d.push(0x30);
        d.push(0x29);
        d.push(0x30 + axes - 1);
        d.extend_from_slice(&[0x15, 0x00, 0x26, 0xFF, 0x03]);
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
        d.extend_from_slice(&[0x15, 0x01, 0x25, 0x08]);
        d.push(0x75);
        d.push(0x04);
        d.push(0x95);
        d.push(hats);
        d.extend_from_slice(&[0x81, 0x42]);
    }

    if buttons > 0 {
        d.extend_from_slice(&[0x05, 0x09]);
        d.push(0x19);
        d.push(0x01);
        d.push(0x29);
        d.push(buttons);
        d.extend_from_slice(&[0x15, 0x00, 0x25, 0x01]);
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

// ═══════════════════════════════════════════════════════════════════════════
// §9  Descriptor parsing — happy paths
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn parse_simple_2_axis() {
    let desc = parse_descriptor(&joystick_descriptor(2, 0)).unwrap();
    assert_eq!(desc.axis_count(), 2);
    assert_eq!(desc.button_count(), 0);
    assert_eq!(desc.hat_count(), 0);
    assert_eq!(desc.report_size_bits, 32);
}

#[test]
fn parse_3_axis_8_buttons() {
    let desc = parse_descriptor(&joystick_descriptor(3, 8)).unwrap();
    assert_eq!(desc.axis_count(), 3);
    assert_eq!(desc.button_count(), 8);
    assert_eq!(desc.report_size_bits, 56); // 3×16 + 8×1
}

#[test]
fn parse_axes_buttons_hats() {
    let desc = parse_descriptor(&hotas_descriptor(6, 32, 1)).unwrap();
    assert_eq!(desc.axis_count(), 6);
    assert_eq!(desc.button_count(), 32);
    assert_eq!(desc.hat_count(), 1);
}

#[test]
fn parse_two_hats() {
    let desc = parse_descriptor(&hotas_descriptor(4, 12, 2)).unwrap();
    assert_eq!(desc.hat_count(), 2);
    assert_eq!(desc.button_count(), 12);
}

#[test]
fn axis_ranges_match() {
    let desc = parse_descriptor(&joystick_descriptor(3, 0)).unwrap();
    let ranges = desc.axis_ranges();
    assert_eq!(ranges.len(), 3);
    for &(lo, hi) in &ranges {
        assert_eq!(lo, 0);
        assert_eq!(hi, 1023);
    }
}

#[test]
fn collection_structure_application() {
    let desc = parse_descriptor(&joystick_descriptor(2, 4)).unwrap();
    assert_eq!(desc.collections.len(), 1);
    let col = &desc.collections[0];
    assert_eq!(col.collection_type, CollectionType::Application);
    assert_eq!(col.usage_page, usage_page::GENERIC_DESKTOP);
    assert_eq!(col.usage, usage_desktop::JOYSTICK);
}

#[test]
fn all_fields_returns_every_field() {
    let desc = parse_descriptor(&joystick_descriptor(2, 8)).unwrap();
    let fields = desc.all_fields();
    // 2 axes merged into 1 field, 8 buttons = 1 field, padding = 1 field
    assert!(fields.len() >= 2, "expected at least 2 fields, got {}", fields.len());
}

#[test]
fn descriptor_display() {
    let desc = parse_descriptor(&joystick_descriptor(2, 4)).unwrap();
    let s = desc.to_string();
    assert!(s.contains("axes: 2"));
    assert!(s.contains("buttons: 4"));
}

#[test]
fn descriptor_debug_contains_struct_name() {
    let desc = parse_descriptor(&joystick_descriptor(1, 0)).unwrap();
    let s = format!("{desc:?}");
    assert!(s.contains("ReportDescriptor"));
}

// ═══════════════════════════════════════════════════════════════════════════
// §10  Signed logical ranges
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn signed_logical_min_negative128() {
    let mut d = vec![0x05, 0x01, 0x09, 0x04, 0xA1, 0x01];
    d.extend_from_slice(&[0x05, 0x01, 0x09, 0x30]); // Usage Page / Usage (X)
    d.extend_from_slice(&[0x15, 0x80]); // Logical Min (-128)
    d.extend_from_slice(&[0x25, 0x7F]); // Logical Max (127)
    d.extend_from_slice(&[0x75, 0x08, 0x95, 0x01]); // Size 8, Count 1
    d.extend_from_slice(&[0x81, 0x02]); // Input
    d.push(0xC0);

    let desc = parse_descriptor(&d).unwrap();
    let ranges = desc.axis_ranges();
    assert_eq!(ranges[0], (-128, 127));
}

#[test]
fn signed_16bit_range() {
    let mut d = vec![0x05, 0x01, 0x09, 0x04, 0xA1, 0x01];
    d.extend_from_slice(&[0x05, 0x01, 0x09, 0x30]);
    d.extend_from_slice(&[0x16, 0x00, 0x80]); // Logical Min (-32768) two-byte
    d.extend_from_slice(&[0x26, 0xFF, 0x7F]); // Logical Max (32767)
    d.extend_from_slice(&[0x75, 0x10, 0x95, 0x01]);
    d.extend_from_slice(&[0x81, 0x02]);
    d.push(0xC0);

    let desc = parse_descriptor(&d).unwrap();
    let ranges = desc.axis_ranges();
    assert_eq!(ranges[0], (-32768, 32767));
}

// ═══════════════════════════════════════════════════════════════════════════
// §11  Output and Feature items
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn output_item_in_collection() {
    let mut d = vec![0x05, 0x01, 0x09, 0x04, 0xA1, 0x01];
    // Input axis
    d.extend_from_slice(&[0x05, 0x01, 0x09, 0x30]);
    d.extend_from_slice(&[0x15, 0x00, 0x26, 0xFF, 0x03]);
    d.extend_from_slice(&[0x75, 0x10, 0x95, 0x01, 0x81, 0x02]);
    // Output LED
    d.extend_from_slice(&[0x05, 0x08, 0x09, 0x01]); // LED page
    d.extend_from_slice(&[0x75, 0x08, 0x95, 0x01, 0x91, 0x02]); // Output
    d.push(0xC0);

    let desc = parse_descriptor(&d).unwrap();
    let outputs: Vec<_> = desc
        .all_fields()
        .into_iter()
        .filter(|f| f.report_type == ReportType::Output)
        .collect();
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].usage_page, usage_page::LED);
}

#[test]
fn feature_item_in_collection() {
    let mut d = vec![0x05, 0x01, 0x09, 0x04, 0xA1, 0x01];
    d.extend_from_slice(&[0x05, 0x01, 0x09, 0x30]);
    d.extend_from_slice(&[0x75, 0x08, 0x95, 0x01, 0xB1, 0x02]); // Feature
    d.push(0xC0);

    let desc = parse_descriptor(&d).unwrap();
    let features: Vec<_> = desc
        .all_fields()
        .into_iter()
        .filter(|f| f.report_type == ReportType::Feature)
        .collect();
    assert_eq!(features.len(), 1);
}

#[test]
fn constant_padding_not_counted_as_buttons() {
    let d = joystick_descriptor(0, 5); // 5 buttons → 3 bits padding
    let desc = parse_descriptor(&d).unwrap();
    assert_eq!(desc.button_count(), 5); // Only 5, not 8
}

// ═══════════════════════════════════════════════════════════════════════════
// §12  Error handling
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn empty_descriptor_error() {
    assert_eq!(parse_descriptor(&[]), Err(DescriptorError::Empty));
}

#[test]
fn truncated_short_item() {
    // Usage Page prefix (1-byte data) with no data byte following
    assert!(matches!(
        parse_descriptor(&[0x05]),
        Err(DescriptorError::Truncated { offset: 0 })
    ));
}

#[test]
fn truncated_two_byte_item() {
    // 0x06 = Usage Page with 2-byte data, but only 1 byte follows
    assert!(matches!(
        parse_descriptor(&[0x06, 0x01]),
        Err(DescriptorError::Truncated { offset: 0 })
    ));
}

#[test]
fn unmatched_end_collection() {
    assert!(matches!(
        parse_descriptor(&[0xC0]),
        Err(DescriptorError::UnmatchedEnd { offset: 0 })
    ));
}

#[test]
fn unclosed_single_collection() {
    let d = [0x05, 0x01, 0x09, 0x04, 0xA1, 0x01];
    assert!(matches!(
        parse_descriptor(&d),
        Err(DescriptorError::UnclosedCollection { count: 1 })
    ));
}

#[test]
fn unclosed_nested_collections() {
    let d = [
        0x05, 0x01, 0x09, 0x04, 0xA1, 0x01, // Collection (Application)
        0xA1, 0x00, // Collection (Physical) — nested
    ];
    assert!(matches!(
        parse_descriptor(&d),
        Err(DescriptorError::UnclosedCollection { count: 2 })
    ));
}

#[test]
fn descriptor_error_display() {
    let e = DescriptorError::Empty;
    assert_eq!(e.to_string(), "empty descriptor");

    let e = DescriptorError::Truncated { offset: 42 };
    assert!(e.to_string().contains("42"));

    let e = DescriptorError::UnmatchedEnd { offset: 7 };
    assert!(e.to_string().contains("unmatched"));

    let e = DescriptorError::UnclosedCollection { count: 3 };
    assert!(e.to_string().contains("3"));
}

// ═══════════════════════════════════════════════════════════════════════════
// §13  Long items
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn long_item_skipped_gracefully() {
    let mut d = vec![0x05, 0x01, 0x09, 0x04, 0xA1, 0x01];
    // Long item: 0xFE, length=2, tag=0x10, data=[0xAA, 0xBB]
    d.extend_from_slice(&[0xFE, 0x02, 0x10, 0xAA, 0xBB]);
    d.push(0xC0);

    let desc = parse_descriptor(&d).unwrap();
    assert_eq!(desc.collections.len(), 1);
}

#[test]
fn truncated_long_item() {
    // Long item prefix with not enough data
    let d = [0xFE, 0x05, 0x10, 0xAA]; // claims 5 bytes but only 1 available
    assert!(matches!(
        parse_descriptor(&d),
        Err(DescriptorError::Truncated { .. })
    ));
}

#[test]
fn long_item_at_end_with_no_length() {
    let d = [0xFE]; // Long item sentinel with no length byte
    assert!(matches!(
        parse_descriptor(&d),
        Err(DescriptorError::Truncated { .. })
    ));
}

// ═══════════════════════════════════════════════════════════════════════════
// §14  Collection nesting
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn nested_physical_collection_fields_flattened() {
    let mut d = vec![0x05, 0x01, 0x09, 0x04, 0xA1, 0x01]; // Application
    // Nested Physical collection
    d.extend_from_slice(&[0xA1, 0x00]); // Collection (Physical)
    d.extend_from_slice(&[0x05, 0x01, 0x09, 0x30]);
    d.extend_from_slice(&[0x15, 0x00, 0x26, 0xFF, 0x03]);
    d.extend_from_slice(&[0x75, 0x10, 0x95, 0x02, 0x81, 0x02]);
    d.push(0xC0); // End Physical
    d.push(0xC0); // End Application

    let desc = parse_descriptor(&d).unwrap();
    assert_eq!(desc.axis_count(), 2);
    // Child items are flattened into parent
    assert_eq!(desc.collections.len(), 1);
    assert!(!desc.collections[0].fields.is_empty());
}

#[test]
fn multiple_top_level_collections() {
    let mut d = Vec::new();
    // First collection: joystick
    d.extend_from_slice(&[0x05, 0x01, 0x09, 0x04, 0xA1, 0x01]);
    d.extend_from_slice(&[0x05, 0x01, 0x09, 0x30]);
    d.extend_from_slice(&[0x15, 0x00, 0x26, 0xFF, 0x03]);
    d.extend_from_slice(&[0x75, 0x10, 0x95, 0x01, 0x81, 0x02]);
    d.push(0xC0);
    // Second collection: game pad
    d.extend_from_slice(&[0x05, 0x01, 0x09, 0x05, 0xA1, 0x01]);
    d.extend_from_slice(&[0x05, 0x01, 0x09, 0x31]);
    d.extend_from_slice(&[0x15, 0x00, 0x26, 0xFF, 0x03]);
    d.extend_from_slice(&[0x75, 0x10, 0x95, 0x01, 0x81, 0x02]);
    d.push(0xC0);

    let desc = parse_descriptor(&d).unwrap();
    assert_eq!(desc.collections.len(), 2);
    assert_eq!(desc.collections[0].usage, usage_desktop::JOYSTICK);
    assert_eq!(desc.collections[1].usage, usage_desktop::GAME_PAD);
    assert_eq!(desc.axis_count(), 2);
}

// ═══════════════════════════════════════════════════════════════════════════
// §15  Report ID support
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn report_id_propagated_to_fields() {
    let mut d = vec![0x05, 0x01, 0x09, 0x04, 0xA1, 0x01];
    d.extend_from_slice(&[0x85, 0x01]); // Report ID 1
    d.extend_from_slice(&[0x05, 0x01, 0x09, 0x30]);
    d.extend_from_slice(&[0x15, 0x00, 0x26, 0xFF, 0x03]);
    d.extend_from_slice(&[0x75, 0x10, 0x95, 0x01, 0x81, 0x02]);
    d.push(0xC0);

    let desc = parse_descriptor(&d).unwrap();
    let field = &desc.collections[0].fields[0];
    assert_eq!(field.report_id, Some(1));
}

#[test]
fn no_report_id_defaults_to_none() {
    let desc = parse_descriptor(&joystick_descriptor(1, 0)).unwrap();
    let field = &desc.collections[0].fields[0];
    assert_eq!(field.report_id, None);
}

// ═══════════════════════════════════════════════════════════════════════════
// §16  Global Push/Pop
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn global_push_pop_restores_state() {
    let mut d = vec![0x05, 0x01, 0x09, 0x04, 0xA1, 0x01];
    // Set range 0..1023, Push, change range, Input, Pop, Input
    d.extend_from_slice(&[0x05, 0x01, 0x09, 0x30]);
    d.extend_from_slice(&[0x15, 0x00, 0x26, 0xFF, 0x03]); // 0..1023
    d.extend_from_slice(&[0x75, 0x10, 0x95, 0x01]);
    d.extend_from_slice(&[0xA4]); // Push
    d.extend_from_slice(&[0x15, 0x00, 0x25, 0x01]); // Change to 0..1
    d.extend_from_slice(&[0x05, 0x09, 0x19, 0x01, 0x29, 0x08]); // Buttons
    d.extend_from_slice(&[0x75, 0x01, 0x95, 0x08, 0x81, 0x02]); // Input
    d.extend_from_slice(&[0xB4]); // Pop
    d.extend_from_slice(&[0x05, 0x01, 0x09, 0x31]); // Y axis
    d.extend_from_slice(&[0x81, 0x02]); // Input — should use restored 0..1023 range
    d.push(0xC0);

    let desc = parse_descriptor(&d).unwrap();
    let ranges = desc.axis_ranges();
    // The Y axis (after Pop) should have the original range
    assert_eq!(ranges.len(), 1); // Only one axis field (Y) after pop
    assert_eq!(ranges[0], (0, 1023));
}

// ═══════════════════════════════════════════════════════════════════════════
// §17  Bit-level extraction
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn extract_bits_single_byte() {
    let report = [0b1010_0101];
    assert_eq!(extract_bits(&report, 0, 1), Some(1)); // bit 0
    assert_eq!(extract_bits(&report, 1, 1), Some(0)); // bit 1
    assert_eq!(extract_bits(&report, 0, 8), Some(0xA5));
}

#[test]
fn extract_bits_cross_byte_boundary() {
    let report = [0xFF, 0x00];
    assert_eq!(extract_bits(&report, 4, 8), Some(0x0F)); // last 4 of byte0 + first 4 of byte1
}

#[test]
fn extract_bits_16bit_value() {
    let report = [0x00, 0xFF, 0x03]; // bytes at offset 0
    assert_eq!(extract_bits(&report, 8, 16), Some(0x03FF));
}

#[test]
fn extract_bits_out_of_bounds() {
    let report = [0xFF];
    assert_eq!(extract_bits(&report, 0, 16), None); // need 2 bytes
    assert_eq!(extract_bits(&report, 4, 8), None); // bits 4..12 exceed 1 byte
}

#[test]
fn extract_bits_zero_size() {
    assert_eq!(extract_bits(&[0xFF], 0, 0), None);
}

#[test]
fn extract_bits_max_32() {
    let report = [0xFF; 4];
    assert_eq!(extract_bits(&report, 0, 32), Some(0xFFFF_FFFF));
}

#[test]
fn extract_bits_over_32_returns_none() {
    let report = [0xFF; 8];
    assert_eq!(extract_bits(&report, 0, 33), None);
}

#[test]
fn extract_bits_signed_positive() {
    let report = [0x7F]; // +127 in signed 8-bit
    assert_eq!(extract_bits_signed(&report, 0, 8), Some(127));
}

#[test]
fn extract_bits_signed_negative() {
    let report = [0x80]; // -128 in signed 8-bit
    assert_eq!(extract_bits_signed(&report, 0, 8), Some(-128));
}

#[test]
fn extract_bits_signed_4bit() {
    let report = [0x0F]; // 0b1111 in 4-bit = -1
    assert_eq!(extract_bits_signed(&report, 0, 4), Some(-1));
}

#[test]
fn extract_bits_signed_out_of_bounds() {
    assert_eq!(extract_bits_signed(&[0xFF], 0, 16), None);
}

// ═══════════════════════════════════════════════════════════════════════════
// §18  HidDeviceInfo
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn hid_device_info_vendor_page_detection() {
    let info = HidDeviceInfo {
        vendor_id: 0x231D,
        product_id: 0x0136,
        serial_number: None,
        manufacturer: None,
        product_name: None,
        device_path: String::new(),
        usage_page: 0xFF00,
        usage: 0x01,
        report_descriptor: None,
    };
    assert!(info.is_vendor_page());
}

#[test]
fn hid_device_info_standard_page_not_vendor() {
    let info = HidDeviceInfo {
        vendor_id: 0x231D,
        product_id: 0x0136,
        serial_number: None,
        manufacturer: None,
        product_name: None,
        device_path: String::new(),
        usage_page: usage_page::GENERIC_DESKTOP,
        usage: usage_desktop::JOYSTICK,
        report_descriptor: None,
    };
    assert!(!info.is_vendor_page());
}

// ═══════════════════════════════════════════════════════════════════════════
// §19  Realistic device descriptors
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn realistic_6_axis_32_button_hotas() {
    let desc = parse_descriptor(&hotas_descriptor(6, 32, 1)).unwrap();
    assert_eq!(desc.axis_count(), 6);
    assert_eq!(desc.button_count(), 32);
    assert_eq!(desc.hat_count(), 1);
    let ranges = desc.axis_ranges();
    assert_eq!(ranges.len(), 6);
}

#[test]
fn zero_axes_zero_buttons() {
    // Minimal valid collection with no data items
    let d = [0x05, 0x01, 0x09, 0x04, 0xA1, 0x01, 0xC0];
    let desc = parse_descriptor(&d).unwrap();
    assert_eq!(desc.axis_count(), 0);
    assert_eq!(desc.button_count(), 0);
    assert_eq!(desc.hat_count(), 0);
    assert_eq!(desc.report_size_bits, 0);
}

// ═══════════════════════════════════════════════════════════════════════════
// §20  Descriptor Clone + Eq
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn descriptor_clone_equality() {
    let d1 = parse_descriptor(&joystick_descriptor(2, 4)).unwrap();
    let d2 = d1.clone();
    assert_eq!(d1, d2);
}

#[test]
fn different_descriptors_not_equal() {
    let d1 = parse_descriptor(&joystick_descriptor(2, 0)).unwrap();
    let d2 = parse_descriptor(&joystick_descriptor(3, 0)).unwrap();
    assert_ne!(d1, d2);
}

// ═══════════════════════════════════════════════════════════════════════════
// §21  Vendor usage pages
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn vendor_usage_page_in_descriptor() {
    let mut d = Vec::new();
    d.extend_from_slice(&[0x06, 0x00, 0xFF]); // Usage Page (Vendor 0xFF00) — 2-byte
    d.extend_from_slice(&[0x09, 0x01]); // Usage (1)
    d.extend_from_slice(&[0xA1, 0x01]); // Collection (Application)
    d.extend_from_slice(&[0x09, 0x02]);
    d.extend_from_slice(&[0x15, 0x00, 0x26, 0xFF, 0x00]);
    d.extend_from_slice(&[0x75, 0x08, 0x95, 0x01, 0x81, 0x02]);
    d.push(0xC0);

    let desc = parse_descriptor(&d).unwrap();
    assert_eq!(desc.collections[0].usage_page, 0xFF00);
    assert!(usage_page::is_vendor(desc.collections[0].usage_page));
}

// ═══════════════════════════════════════════════════════════════════════════
// §22  Physical range
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn physical_min_max_propagated() {
    let mut d = vec![0x05, 0x01, 0x09, 0x04, 0xA1, 0x01];
    d.extend_from_slice(&[0x05, 0x01, 0x09, 0x30]);
    d.extend_from_slice(&[0x15, 0x00, 0x26, 0xFF, 0x03]); // Logical 0..1023
    d.extend_from_slice(&[0x35, 0x00]); // Physical Min (0)
    d.extend_from_slice(&[0x46, 0x68, 0x01]); // Physical Max (360)
    d.extend_from_slice(&[0x75, 0x10, 0x95, 0x01, 0x81, 0x02]);
    d.push(0xC0);

    let desc = parse_descriptor(&d).unwrap();
    let field = &desc.collections[0].fields[0];
    assert_eq!(field.physical_min, 0);
    assert_eq!(field.physical_max, 360);
}

// ═══════════════════════════════════════════════════════════════════════════
// §23  Property-based tests
// ═══════════════════════════════════════════════════════════════════════════

mod proptest_suite {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn arbitrary_bytes_never_panic(data in proptest::collection::vec(any::<u8>(), 0..512)) {
            // The parser must never panic regardless of input.
            let _ = parse_descriptor(&data);
        }

        #[test]
        fn extract_bits_never_panics(
            data in proptest::collection::vec(any::<u8>(), 0..64),
            offset in 0u32..512,
            size in 0u32..64,
        ) {
            let _ = extract_bits(&data, offset, size);
            let _ = extract_bits_signed(&data, offset, size);
        }

        #[test]
        fn joystick_descriptor_always_parses(axes in 1u8..8, buttons in 0u8..33) {
            let d = joystick_descriptor(axes, buttons);
            let desc = parse_descriptor(&d).unwrap();
            prop_assert_eq!(desc.axis_count(), axes as u32);
            prop_assert_eq!(desc.button_count(), buttons as u32);
        }

        #[test]
        fn collection_type_round_trips(v in 0u32..256) {
            let ct = CollectionType::from_value(v);
            // Verify it doesn't panic and always returns a valid variant
            let _ = format!("{ct}");
            let _ = format!("{ct:?}");
        }

        #[test]
        fn main_item_flags_never_panics(raw in any::<u32>()) {
            let f = MainItemFlags(raw);
            let _ = f.is_constant();
            let _ = f.is_variable();
            let _ = f.is_relative();
            let _ = f.is_wrap();
            let _ = f.is_nonlinear();
            let _ = f.is_no_preferred();
            let _ = f.is_null_state();
            let _ = f.is_buffered_bytes();
        }

        #[test]
        fn usage_page_vendor_consistent(page in any::<u16>()) {
            let is_vendor = usage_page::is_vendor(page);
            prop_assert_eq!(is_vendor, page >= 0xFF00);
        }
    }
}
