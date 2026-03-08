// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests for flight-streamdeck crate.
//!
//! Tests the full pipeline: device detection → key event → action lookup →
//! icon rendering → profile layout.

use flight_streamdeck::{
    ActionRegistry, AircraftType, Brightness, DeviceInfo, DeviceManager, DialAction, DialEvent,
    KeyEvent, ProfileLayout, ProfileManager, StreamDeckModel, TouchEvent, TouchType,
    validate_image_size,
};
use flight_streamdeck::render::{IconRenderer, IconTheme};
use flight_streamdeck::layout::{Page, PageLayout};
use flight_streamdeck::{AppVersion, VersionCompatibility};

use std::collections::HashMap;

// ── Full pipeline: key press → action → icon ─────────────────────────────────

#[test]
fn test_pipeline_key_press_to_icon() {
    // 1. Register a device
    let mut mgr = DeviceManager::new();
    mgr.register_device(DeviceInfo {
        id: "sd-orig".into(),
        model: StreamDeckModel::Original,
        serial: Some("SN001".into()),
        firmware_version: None,
        connected: true,
    });

    // 2. Simulate a key press
    let event = KeyEvent { key_index: 0, pressed: true };
    let dev = mgr.get_device("sd-orig").unwrap();
    assert!(event.is_valid_for(dev.model));

    // 3. Look up the action for that key
    let registry = ActionRegistry::builtin();
    let action = registry.get("com.flighthub.ap-toggle").unwrap();
    assert_eq!(action.key_label, "AP");

    // 4. Render an icon for the device model
    let renderer = IconRenderer::new(dev.model);
    let icon = renderer
        .toggle_icon(&action.key_label, action.category.icon_theme())
        .unwrap();
    assert_eq!(icon.0.size, 72); // Original = 72x72
    assert!(!icon.0.active);
    assert!(icon.1.active);
}

#[test]
fn test_pipeline_mk2_device() {
    let mut mgr = DeviceManager::new();
    mgr.register_device(DeviceInfo {
        id: "sd-mk2".into(),
        model: StreamDeckModel::Mk2,
        serial: None,
        firmware_version: None,
        connected: true,
    });

    let dev = mgr.get_device("sd-mk2").unwrap();
    assert_eq!(dev.model.key_count(), 15);
    assert_eq!(dev.model.product_id(), 0x0080);

    let event = KeyEvent { key_index: 14, pressed: true };
    assert!(event.is_valid_for(dev.model));

    let renderer = IconRenderer::new(dev.model);
    let icon = renderer.value_icon("HDG", "270", IconTheme::Navigation).unwrap();
    assert_eq!(icon.size, 72);
    assert!(validate_image_size(dev.model, 72, 72));
}

// ── Full pipeline: dial rotation → action ────────────────────────────────────

#[test]
fn test_pipeline_plus_dial_to_action() {
    let mut mgr = DeviceManager::new();
    mgr.register_device(DeviceInfo {
        id: "sd-plus".into(),
        model: StreamDeckModel::Plus,
        serial: None,
        firmware_version: None,
        connected: true,
    });

    let dev = mgr.get_device("sd-plus").unwrap();
    assert!(dev.model.has_dials());

    // Dial rotate clockwise
    let dial = DialEvent { dial_index: 0, action: DialAction::Rotate(1) };
    assert!(dial.is_valid_for(dev.model));

    // Look up encoder action
    let registry = ActionRegistry::builtin();
    let enc = registry.get("com.flighthub.vor-obs-inc").unwrap();
    assert_eq!(enc.behavior, flight_streamdeck::actions::ActionBehavior::Encoder);

    // Touch event on LCD strip
    let touch = TouchEvent { x: 200, y: 50, touch_type: TouchType::Tap };
    assert!(touch.is_valid_for(dev.model));
}

// ── Multi-device management ──────────────────────────────────────────────────

#[test]
fn test_multi_device_fleet() {
    let mut mgr = DeviceManager::new();

    let models = StreamDeckModel::all();
    for (i, model) in models.iter().enumerate() {
        mgr.register_device(DeviceInfo {
            id: format!("dev-{}", i),
            model: *model,
            serial: Some(format!("SN-{:04}", i)),
            firmware_version: None,
            connected: true,
        });
    }

    assert_eq!(mgr.discover().len(), models.len());

    // Set different brightness per device
    for i in 0..models.len() {
        let b = Brightness::new((i as u8 * 15).min(100)).unwrap();
        mgr.set_brightness(&format!("dev-{}", i), b).unwrap();
    }

    // Disconnect one
    mgr.disconnect_device("dev-0");
    assert_eq!(mgr.discover().len(), models.len() - 1);
}

// ── Profile layout across all models ─────────────────────────────────────────

#[test]
fn test_profile_layout_all_displayable_models() {
    let templates: HashMap<String, _> = flight_streamdeck::actions::builtin_templates()
        .into_iter()
        .map(|t| (t.id.clone(), t))
        .collect();

    for model in StreamDeckModel::all() {
        if model.grid_layout().is_none() {
            continue;
        }
        for aircraft in &[AircraftType::GA, AircraftType::Airbus, AircraftType::Helo] {
            let layout = ProfileLayout::build(*aircraft, *model, &templates).unwrap();
            assert!(layout.page_count() >= 1, "{:?} + {:?}", aircraft, model);
        }
    }
}

// ── Page navigation stress ───────────────────────────────────────────────────

#[test]
fn test_page_navigation_stress() {
    let mut layout = PageLayout::new(StreamDeckModel::Mini).unwrap();
    for i in 0..10 {
        layout.add_page(Page::new(&format!("Page {}", i)));
    }

    // Forward 25 times (wraps)
    for _ in 0..25 {
        layout.next_page();
    }
    assert_eq!(layout.current_index(), 5); // 25 % 10

    // Backward 7 times
    for _ in 0..7 {
        layout.prev_page();
    }
    assert_eq!(layout.current_index(), 8); // (5 - 7 + 10) % 10
}

// ── Version compatibility + profiles ─────────────────────────────────────────

#[test]
fn test_version_and_profile_integration() {
    let compat = VersionCompatibility::new();
    let v = AppVersion::new(6, 2, 0);
    assert!(compat.is_compatible(&v).unwrap());

    let mut pm = ProfileManager::new();
    pm.load_sample_profiles().unwrap();
    assert_eq!(pm.get_profiles().len(), 3);
}

// ── Device identification from USB PID ───────────────────────────────────────

#[test]
fn test_device_identification_from_pid() {
    let known_pids: &[(u16, StreamDeckModel)] = &[
        (0x0060, StreamDeckModel::Original),
        (0x0063, StreamDeckModel::Mini),
        (0x006C, StreamDeckModel::Xl),
        (0x0080, StreamDeckModel::Mk2),
        (0x0084, StreamDeckModel::Plus),
        (0x0086, StreamDeckModel::Pedal),
        (0x009A, StreamDeckModel::Neo),
    ];

    for (pid, expected_model) in known_pids {
        let model = StreamDeckModel::from_product_id(*pid).unwrap();
        assert_eq!(model, *expected_model);
        assert_eq!(model.product_id(), *pid);
    }
}

// ── Image size validation for rendering pipeline ─────────────────────────────

#[test]
fn test_image_rendering_pipeline() {
    let model_sizes: &[(StreamDeckModel, u32)] = &[
        (StreamDeckModel::Original, 72),
        (StreamDeckModel::Mk2, 72),
        (StreamDeckModel::Mini, 80),
        (StreamDeckModel::Xl, 96),
        (StreamDeckModel::Plus, 120),
        (StreamDeckModel::Neo, 96),
    ];

    for (model, size) in model_sizes {
        // Correct size passes
        assert!(validate_image_size(*model, *size, *size), "{:?}@{}x{}", model, size, size);

        // Wrong size fails
        assert!(!validate_image_size(*model, size + 1, *size), "{:?} mismatch", model);
        assert!(!validate_image_size(*model, *size, size + 1), "{:?} mismatch", model);

        // Icon renderer agrees
        let renderer = IconRenderer::new(*model);
        assert_eq!(renderer.icon_size(), Some(*size));
    }
}
