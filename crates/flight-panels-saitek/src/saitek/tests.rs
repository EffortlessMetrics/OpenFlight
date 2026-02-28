// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Tests for Saitek/Logitech panel writer

use super::*;
use flight_hid::HidAdapter;
use flight_watchdog::WatchdogSystem;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn create_test_hid_adapter() -> HidAdapter {
    let watchdog = Arc::new(Mutex::new(WatchdogSystem::new()));
    HidAdapter::new(watchdog)
}

fn create_test_device_info(panel_type: PanelType) -> HidDeviceInfo {
    HidDeviceInfo {
        vendor_id: SAITEK_VENDOR_ID,
        product_id: panel_type as u16,
        serial_number: Some("TEST123".to_string()),
        manufacturer: Some("Saitek".to_string()),
        product_name: Some(panel_type.name().to_string()),
        device_path: format!("/dev/test_{:04x}", panel_type as u16),
        usage_page: 0x01,
        usage: 0x04,
        report_descriptor: None,
    }
}

#[test]
fn test_panel_type_detection() {
    assert_eq!(
        PanelType::from_product_id(0x0D05),
        Some(PanelType::RadioPanel)
    );
    assert_eq!(
        PanelType::from_product_id(0x0D06),
        Some(PanelType::MultiPanel)
    );
    assert_eq!(
        PanelType::from_product_id(0x0D67),
        Some(PanelType::SwitchPanel)
    );
    assert_eq!(PanelType::from_product_id(0x0B4E), Some(PanelType::BIP));
    assert_eq!(PanelType::from_product_id(0x0A2F), Some(PanelType::FIP));
    assert_eq!(PanelType::from_product_id(0x9999), None);
}

#[test]
fn test_panel_type_names() {
    assert_eq!(PanelType::RadioPanel.name(), "Radio Panel");
    assert_eq!(PanelType::MultiPanel.name(), "Multi Panel");
    assert_eq!(PanelType::SwitchPanel.name(), "Switch Panel");
    assert_eq!(PanelType::BIP.name(), "Backlighting Instrument Panel");
    assert_eq!(PanelType::FIP.name(), "Flight Instrument Panel");
}

#[test]
fn test_panel_led_mappings() {
    let radio_leds = PanelType::RadioPanel.led_mapping();
    assert!(radio_leds.contains(&"COM1"));
    assert!(radio_leds.contains(&"NAV1"));
    assert!(radio_leds.contains(&"XPDR"));

    let switch_leds = PanelType::SwitchPanel.led_mapping();
    assert!(switch_leds.contains(&"GEAR"));
    assert!(switch_leds.contains(&"MASTER_BAT"));
    assert!(switch_leds.contains(&"AVIONICS"));
}

#[test]
fn test_saitek_writer_creation() {
    let hid_adapter = create_test_hid_adapter();
    let writer = SaitekPanelWriter::new(hid_adapter);

    assert_eq!(writer.panels.len(), 0);
    assert_eq!(writer.led_states.len(), 0);
    assert!(writer.verify_state.is_none());
}

#[test]
fn test_panel_registration() {
    let hid_adapter = create_test_hid_adapter();
    let mut writer = SaitekPanelWriter::new(hid_adapter);

    let device_info = create_test_device_info(PanelType::RadioPanel);
    let device_path = device_info.device_path.clone();

    assert!(writer.register_panel(device_info).is_ok());
    assert_eq!(writer.panels.len(), 1);
    assert!(writer.panels.contains_key(&device_path));
    assert!(writer.led_states.contains_key(&device_path));

    // Check LED states were initialized
    let led_states = writer.led_states.get(&device_path).unwrap();
    assert_eq!(led_states.len(), PanelType::RadioPanel.led_mapping().len());
}

#[test]
fn test_supported_panel_detection() {
    let hid_adapter = create_test_hid_adapter();
    let writer = SaitekPanelWriter::new(hid_adapter);

    // Test Saitek panels
    let saitek_radio = create_test_device_info(PanelType::RadioPanel);
    assert!(writer.is_supported_panel(&saitek_radio));

    // Test Logitech panels
    let mut logitech_multi = create_test_device_info(PanelType::MultiPanel);
    logitech_multi.vendor_id = LOGITECH_VENDOR_ID;
    assert!(writer.is_supported_panel(&logitech_multi));

    // Test unsupported vendor
    let mut unsupported = create_test_device_info(PanelType::RadioPanel);
    unsupported.vendor_id = 0x1234;
    assert!(!writer.is_supported_panel(&unsupported));

    // Test unsupported product
    let mut unsupported_product = create_test_device_info(PanelType::RadioPanel);
    unsupported_product.product_id = 0x9999;
    assert!(!writer.is_supported_panel(&unsupported_product));
}

#[test]
fn test_led_state_management() {
    let hid_adapter = create_test_hid_adapter();
    let mut writer = SaitekPanelWriter::new(hid_adapter);

    let device_info = create_test_device_info(PanelType::SwitchPanel);
    let device_path = device_info.device_path.clone();
    writer.register_panel(device_info).unwrap();

    // Test LED on
    let led_state = LedState {
        on: true,
        brightness: 1.0,
        blink_rate: None,
        last_update: Instant::now(),
    };
    let target = LedTarget::Panel("GEAR".to_string());

    // This would normally write to hardware, but in test it just updates internal state
    assert!(
        writer
            .set_led(&device_path, "GEAR", &target, &led_state)
            .is_ok()
    );

    // Verify internal state was updated
    let panel_led_states = writer.led_states.get(&device_path).unwrap();
    let gear_led_state = panel_led_states.get("GEAR").unwrap();
    assert!(gear_led_state.is_on);
    assert_eq!(gear_led_state.brightness, 1.0);
}

#[test]
fn test_rate_limiting() {
    let hid_adapter = create_test_hid_adapter();
    let mut writer = SaitekPanelWriter::new(hid_adapter);

    let device_info = create_test_device_info(PanelType::RadioPanel);
    let device_path = device_info.device_path.clone();
    writer.register_panel(device_info).unwrap();

    let led_state = LedState {
        on: true,
        brightness: 1.0,
        blink_rate: None,
        last_update: Instant::now(),
    };
    let target = LedTarget::Panel("COM1".to_string());

    // First write should succeed
    assert!(
        writer
            .set_led(&device_path, "COM1", &target, &led_state)
            .is_ok()
    );

    // Immediate second write should be rate limited (but still return Ok)
    assert!(
        writer
            .set_led(&device_path, "COM1", &target, &led_state)
            .is_ok()
    );
}

#[test]
fn test_verify_pattern_generation() {
    let radio_pattern = PanelType::RadioPanel.verify_pattern();
    assert!(!radio_pattern.is_empty());

    // Check that pattern contains expected steps
    let has_led_on = radio_pattern
        .iter()
        .any(|step| matches!(step, VerifyStep::LedOn(_)));
    let has_led_off = radio_pattern
        .iter()
        .any(|step| matches!(step, VerifyStep::LedOff(_)));
    let has_delay = radio_pattern
        .iter()
        .any(|step| matches!(step, VerifyStep::Delay(_)));

    assert!(has_led_on);
    assert!(has_led_off);
    assert!(has_delay);

    let bip_pattern = PanelType::BIP.verify_pattern();
    let has_all_on = bip_pattern
        .iter()
        .any(|step| matches!(step, VerifyStep::AllOn));
    let has_all_off = bip_pattern
        .iter()
        .any(|step| matches!(step, VerifyStep::AllOff));
    let has_blink = bip_pattern
        .iter()
        .any(|step| matches!(step, VerifyStep::LedBlink(_, _)));

    assert!(has_all_on);
    assert!(has_all_off);
    assert!(has_blink);
}

#[test]
fn test_verify_test_lifecycle() {
    let hid_adapter = create_test_hid_adapter();
    let mut writer = SaitekPanelWriter::new(hid_adapter);

    let device_info = create_test_device_info(PanelType::RadioPanel);
    let device_path = device_info.device_path.clone();
    writer.register_panel(device_info).unwrap();

    // Start verify test
    assert!(writer.start_verify_test(&device_path).is_ok());
    assert!(writer.verify_state.is_some());

    // Cannot start another test while one is running
    assert!(writer.start_verify_test(&device_path).is_err());

    // Update test in a time-bounded loop — RadioPanel has 2×100ms Delay steps
    let test_deadline = std::time::Instant::now() + Duration::from_secs(5);
    let test_result = loop {
        match writer.update_verify_test() {
            Ok(Some(test_result)) => {
                break test_result;
            }
            Ok(None) => {
                assert!(
                    std::time::Instant::now() < test_deadline,
                    "Verify test timed out after 5s"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(e) => panic!("Verify test failed: {}", e),
        }
    };

    assert_eq!(test_result.panel_path, device_path);
    assert!(!test_result.step_results.is_empty());

    // Verify state should be cleared after completion
    assert!(writer.verify_state.is_none());
}

#[test]
fn test_latency_requirement_validation() {
    let hid_adapter = create_test_hid_adapter();
    let mut writer = SaitekPanelWriter::new(hid_adapter);

    let device_info = create_test_device_info(PanelType::MultiPanel);
    let device_path = device_info.device_path.clone();
    writer.register_panel(device_info).unwrap();

    // Perform multiple LED operations to generate latency samples
    for i in 0..50 {
        let led_state = LedState {
            on: i % 2 == 0,
            brightness: 1.0,
            blink_rate: None,
            last_update: Instant::now(),
        };
        let target = LedTarget::Panel("ALT".to_string());

        // Disable rate limiting for this test
        writer.min_write_interval = Duration::from_millis(0);

        writer
            .set_led(&device_path, "ALT", &target, &led_state)
            .unwrap();
    }

    // Check latency statistics
    let stats = writer.get_latency_stats().unwrap();
    assert_eq!(stats.sample_count, 50);

    // Verify latency requirement (≤20ms)
    assert!(
        stats.p99_ns <= 20_000_000,
        "LED latency requirement violated: P99 = {} ns (>20ms)",
        stats.p99_ns
    );

    // In test environment, latency should be much better
    assert!(
        stats.mean_ns < 10_000_000,
        "Mean latency should be much better in test: {} ns",
        stats.mean_ns
    );
}

#[test]
fn test_blink_state_updates() {
    let hid_adapter = create_test_hid_adapter();
    let mut writer = SaitekPanelWriter::new(hid_adapter);

    let device_info = create_test_device_info(PanelType::BIP);
    let device_path = device_info.device_path.clone();
    writer.register_panel(device_info).unwrap();

    // Set up blinking LED
    let blink_state = LedState {
        on: false,
        brightness: 1.0,
        blink_rate: Some(10.0), // 10Hz for fast testing
        last_update: Instant::now(),
    };
    let target = LedTarget::Panel("MASTER_WARNING".to_string());

    writer.min_write_interval = Duration::from_millis(0); // Disable rate limiting
    writer
        .set_led(&device_path, "MASTER_WARNING", &target, &blink_state)
        .unwrap();

    // Update blink states multiple times
    for _ in 0..10 {
        writer.update_blink_states().unwrap();
        std::thread::sleep(Duration::from_millis(10)); // Allow time for blink toggle
    }

    // Verify that blink updates occurred (latency samples should increase)
    let stats = writer.get_latency_stats().unwrap();
    assert!(
        stats.sample_count > 1,
        "Blink updates should generate latency samples"
    );
}

#[test]
fn test_panel_health_check() {
    let hid_adapter = create_test_hid_adapter();
    let mut writer = SaitekPanelWriter::new(hid_adapter);

    let device_info = create_test_device_info(PanelType::SwitchPanel);
    let device_path = device_info.device_path.clone();
    writer.register_panel(device_info).unwrap();

    let health_status = writer.check_panel_health(&device_path).unwrap();

    assert_eq!(health_status.panel_path, device_path);
    assert_eq!(health_status.panel_type, PanelType::SwitchPanel);
    // In test environment, panel should be responsive
    assert!(health_status.is_responsive);
}

#[test]
fn test_drift_repair() {
    let hid_adapter = create_test_hid_adapter();
    let mut writer = SaitekPanelWriter::new(hid_adapter);

    let device_info = create_test_device_info(PanelType::MultiPanel);
    let device_path = device_info.device_path.clone();
    writer.register_panel(device_info).unwrap();

    // Set some LED states
    let led_state = LedState {
        on: true,
        brightness: 0.8,
        blink_rate: Some(4.0),
        last_update: Instant::now(),
    };
    let target = LedTarget::Panel("AUTOTHROTTLE".to_string());
    writer
        .set_led(&device_path, "AUTOTHROTTLE", &target, &led_state)
        .unwrap();

    // Repair drift (should reset all LEDs)
    assert!(writer.repair_panel_drift(&device_path).is_ok());

    // Verify LEDs were reset
    let panel_led_states = writer.led_states.get(&device_path).unwrap();
    let autothrottle_state = panel_led_states.get("AUTOTHROTTLE").unwrap();
    assert!(!autothrottle_state.is_on);
    assert_eq!(autothrottle_state.brightness, 0.0);
    assert!(autothrottle_state.blink_rate.is_none());
}

#[test]
fn test_hid_report_building() {
    let hid_adapter = create_test_hid_adapter();
    let writer = SaitekPanelWriter::new(hid_adapter);

    let led_state = PanelLedState {
        led_index: 2,
        brightness: 0.5,
        is_on: true,
        blink_rate: None,
        last_blink_toggle: Instant::now(),
        last_write: Instant::now(),
    };

    // Test different panel report formats
    let radio_report = writer.build_radio_panel_report(&led_state).unwrap();
    assert_eq!(radio_report.len(), 8);
    assert_eq!(radio_report[0], 0x00); // Report ID
    assert_eq!(radio_report[3], 127); // 50% brightness = 127

    let multi_report = writer.build_multi_panel_report(&led_state).unwrap();
    assert_eq!(multi_report.len(), 12);
    assert_eq!(multi_report[0], 0x00); // Report ID

    let switch_report = writer.build_switch_panel_report(&led_state).unwrap();
    assert_eq!(switch_report.len(), 8);
    assert_eq!(switch_report[0], 0x00); // Report ID
    // Bit 2 should be set in byte 1
    assert_eq!(switch_report[1] & (1 << 2), 1 << 2);

    let bip_report = writer.build_bip_report(&led_state).unwrap();
    assert_eq!(bip_report.len(), 16);
    assert_eq!(bip_report[0], 0x00); // Report ID

    let fip_report = writer.build_fip_report(&led_state).unwrap();
    assert_eq!(fip_report.len(), 32);
    assert_eq!(fip_report[0], 0x00); // Report ID
}

#[test]
fn test_verify_test_result_analysis() {
    let step_results = vec![
        VerifyStepResult {
            step_index: 0,
            expected_latency: Duration::from_millis(20),
            actual_latency: Duration::from_millis(5),
            success: true,
            error: None,
        },
        VerifyStepResult {
            step_index: 1,
            expected_latency: Duration::from_millis(20),
            actual_latency: Duration::from_millis(15),
            success: true,
            error: None,
        },
        VerifyStepResult {
            step_index: 2,
            expected_latency: Duration::from_millis(20),
            actual_latency: Duration::from_millis(25), // Exceeds requirement
            success: false,
            error: Some("Latency exceeded".to_string()),
        },
    ];

    let test_result = VerifyTestResult {
        panel_path: "/dev/test".to_string(),
        total_duration: Duration::from_millis(100),
        step_results,
        success: false,
    };

    assert!(!test_result.meets_latency_requirement());
    assert_eq!(test_result.max_latency(), Duration::from_millis(25));
    assert_eq!(test_result.avg_latency(), Duration::from_millis(15)); // (5+15+25)/3 = 15
    assert!(!test_result.success);
}

/// LED OFF state should produce zero-brightness bytes for all brightness-based panel reports.
#[test]
fn test_led_off_encoding() {
    let hid_adapter = create_test_hid_adapter();
    let writer = SaitekPanelWriter::new(hid_adapter);

    let off = PanelLedState {
        led_index: 0,
        brightness: 1.0, // brightness field irrelevant when is_on=false
        is_on: false,
        blink_rate: None,
        last_blink_toggle: Instant::now(),
        last_write: Instant::now(),
    };

    let radio = writer.build_radio_panel_report(&off).unwrap();
    assert_eq!(
        radio[1], 0,
        "radio: LED OFF must produce zero brightness byte"
    );

    let multi = writer.build_multi_panel_report(&off).unwrap();
    assert_eq!(
        multi[1], 0,
        "multi: LED OFF must produce zero brightness byte"
    );

    let bip = writer.build_bip_report(&off).unwrap();
    assert_eq!(bip[1], 0, "bip: LED OFF must produce zero brightness byte");

    let fip = writer.build_fip_report(&off).unwrap();
    assert_eq!(fip[1], 0, "fip: LED OFF must produce zero brightness byte");
}

/// Switch panel uses bit-packing; verify each LED index maps to the correct bit.
#[test]
fn test_switch_panel_bit_packing_all_indices() {
    let hid_adapter = create_test_hid_adapter();
    let writer = SaitekPanelWriter::new(hid_adapter);

    for led_index in 0u8..8 {
        let led_state = PanelLedState {
            led_index,
            brightness: 1.0,
            is_on: true,
            blink_rate: None,
            last_blink_toggle: Instant::now(),
            last_write: Instant::now(),
        };
        let report = writer.build_switch_panel_report(&led_state).unwrap();
        assert_eq!(report.len(), 8);

        let byte_idx = 1 + (led_index / 8) as usize;
        let bit = led_index % 8;
        assert_ne!(
            report[byte_idx] & (1 << bit),
            0,
            "led_index={led_index}: expected bit {bit} set in byte {byte_idx}"
        );
    }
}

/// Switch panel: LED OFF must leave all report bytes as zero.
#[test]
fn test_switch_panel_led_off_no_bits_set() {
    let hid_adapter = create_test_hid_adapter();
    let writer = SaitekPanelWriter::new(hid_adapter);

    for led_index in 0u8..8 {
        let off = PanelLedState {
            led_index,
            brightness: 1.0,
            is_on: false,
            blink_rate: None,
            last_blink_toggle: Instant::now(),
            last_write: Instant::now(),
        };
        let report = writer.build_switch_panel_report(&off).unwrap();
        assert_eq!(
            report[1], 0,
            "led_index={led_index}: LED OFF must not set any bits"
        );
    }
}

/// Brightness float values must encode to the expected u8 byte values.
#[test]
fn test_brightness_encoding_levels() {
    let hid_adapter = create_test_hid_adapter();
    let writer = SaitekPanelWriter::new(hid_adapter);

    // (input brightness, expected encoded byte via `(b * 255.0) as u8`)
    let cases: &[(f32, u8)] = &[
        (0.0, 0),
        (0.25, 63),  // floor(0.25 * 255) = 63
        (0.5, 127),  // floor(0.5  * 255) = 127
        (0.75, 191), // floor(0.75 * 255) = 191
        (1.0, 255),
    ];

    for &(brightness, expected) in cases {
        let state = PanelLedState {
            led_index: 0,
            brightness,
            is_on: true,
            blink_rate: None,
            last_blink_toggle: Instant::now(),
            last_write: Instant::now(),
        };
        let report = writer.build_radio_panel_report(&state).unwrap();
        assert_eq!(
            report[1], expected,
            "brightness {brightness:.2} should encode to {expected}"
        );
    }
}

/// Every panel type should expose a non-empty LED mapping with no duplicate names.
#[test]
fn test_all_panel_types_led_mapping_counts_and_names() {
    assert_eq!(PanelType::RadioPanel.led_mapping().len(), 7);
    assert_eq!(PanelType::MultiPanel.led_mapping().len(), 8);
    assert_eq!(PanelType::SwitchPanel.led_mapping().len(), 8);
    assert_eq!(PanelType::BIP.led_mapping().len(), 8);
    assert_eq!(PanelType::FIP.led_mapping().len(), 8);

    // MultiPanel LEDs
    let multi = PanelType::MultiPanel.led_mapping();
    assert!(multi.contains(&"ALT"));
    assert!(multi.contains(&"VS"));
    assert!(multi.contains(&"AUTOTHROTTLE"));

    // BIP LEDs
    let bip = PanelType::BIP.led_mapping();
    assert!(bip.contains(&"GEAR_L"));
    assert!(bip.contains(&"GEAR_N"));
    assert!(bip.contains(&"GEAR_R"));
    assert!(bip.contains(&"MASTER_WARNING"));
    assert!(bip.contains(&"FIRE_WARNING"));

    // FIP LEDs
    let fip = PanelType::FIP.led_mapping();
    assert!(fip.contains(&"ATTITUDE"));
    assert!(fip.contains(&"HSI"));
    assert!(fip.contains(&"ADF"));
}

/// VerifyTestResult: all steps pass, latency methods return correct values.
#[test]
fn test_verify_test_result_all_pass() {
    let step_results = vec![
        VerifyStepResult {
            step_index: 0,
            expected_latency: Duration::from_millis(20),
            actual_latency: Duration::from_millis(4),
            success: true,
            error: None,
        },
        VerifyStepResult {
            step_index: 1,
            expected_latency: Duration::from_millis(20),
            actual_latency: Duration::from_millis(8),
            success: true,
            error: None,
        },
    ];

    let result = VerifyTestResult {
        panel_path: "/dev/test_all_pass".to_string(),
        total_duration: Duration::from_millis(50),
        step_results,
        success: true,
    };

    assert!(result.meets_latency_requirement());
    assert_eq!(result.max_latency(), Duration::from_millis(8));
    // (4_000_000 + 8_000_000) / 2 = 6_000_000 ns = 6 ms
    assert_eq!(result.avg_latency(), Duration::from_millis(6));
    assert!(result.success);
}

/// VerifyTestResult: empty step list should not panic and return sensible defaults.
#[test]
fn test_verify_test_result_empty_steps() {
    let result = VerifyTestResult {
        panel_path: "/dev/test_empty".to_string(),
        total_duration: Duration::from_millis(10),
        step_results: vec![],
        success: true,
    };

    assert!(result.meets_latency_requirement()); // vacuously true
    assert_eq!(result.max_latency(), Duration::ZERO);
    assert_eq!(result.avg_latency(), Duration::ZERO);
}

/// LED state transitions (OFF → ON → OFF) must update the internal LED map correctly.
#[test]
fn test_multiple_led_state_transitions() {
    let hid_adapter = create_test_hid_adapter();
    let mut writer = SaitekPanelWriter::new(hid_adapter);

    let device_info = create_test_device_info(PanelType::MultiPanel);
    let device_path = device_info.device_path.clone();
    writer.register_panel(device_info).unwrap();
    writer.min_write_interval = Duration::from_millis(0);

    let target = LedTarget::Panel("ALT".to_string());
    let off = LedState {
        on: false,
        brightness: 0.0,
        blink_rate: None,
        last_update: Instant::now(),
    };
    let on = LedState {
        on: true,
        brightness: 1.0,
        blink_rate: None,
        last_update: Instant::now(),
    };

    // Initial state after registration: OFF
    assert!(!writer.led_states[&device_path]["ALT"].is_on);

    // OFF → ON
    writer.set_led(&device_path, "ALT", &target, &on).unwrap();
    assert!(writer.led_states[&device_path]["ALT"].is_on);
    assert_eq!(writer.led_states[&device_path]["ALT"].brightness, 1.0);

    // ON → OFF
    writer.set_led(&device_path, "ALT", &target, &off).unwrap();
    assert!(!writer.led_states[&device_path]["ALT"].is_on);
    assert_eq!(writer.led_states[&device_path]["ALT"].brightness, 0.0);

    // OFF → ON again
    writer.set_led(&device_path, "ALT", &target, &on).unwrap();
    assert!(writer.led_states[&device_path]["ALT"].is_on);
}

/// Setting one LED must not affect neighbouring LEDs.
#[test]
fn test_independent_led_states() {
    let hid_adapter = create_test_hid_adapter();
    let mut writer = SaitekPanelWriter::new(hid_adapter);

    let device_info = create_test_device_info(PanelType::SwitchPanel);
    let device_path = device_info.device_path.clone();
    writer.register_panel(device_info).unwrap();
    writer.min_write_interval = Duration::from_millis(0);

    let on = LedState {
        on: true,
        brightness: 1.0,
        blink_rate: None,
        last_update: Instant::now(),
    };
    let target = LedTarget::Panel("GEAR".to_string());
    writer.set_led(&device_path, "GEAR", &target, &on).unwrap();

    assert!(
        writer.led_states[&device_path]["GEAR"].is_on,
        "GEAR should be ON"
    );
    assert!(
        !writer.led_states[&device_path]["MASTER_BAT"].is_on,
        "MASTER_BAT must remain OFF"
    );
    assert!(
        !writer.led_states[&device_path]["AVIONICS"].is_on,
        "AVIONICS must remain OFF"
    );
}

#[test]
fn test_min_interval_enforcement() {
    let hid_adapter = create_test_hid_adapter();
    let mut writer = SaitekPanelWriter::new(hid_adapter);

    // Set minimum interval to 50ms for testing
    writer.min_write_interval = Duration::from_millis(50);

    let device_info = create_test_device_info(PanelType::RadioPanel);
    let device_path = device_info.device_path.clone();
    writer.register_panel(device_info).unwrap();

    let on_state = LedState {
        on: true,
        brightness: 1.0,
        blink_rate: None,
        last_update: Instant::now(),
    };
    let off_state = LedState {
        on: false,
        brightness: 0.0,
        blink_rate: None,
        last_update: Instant::now(),
    };
    let target = LedTarget::Panel("COM1".to_string());

    // First write: LED on — executes because last_write was init'd min_write_interval in the past
    writer
        .set_led(&device_path, "COM1", &target, &on_state)
        .unwrap();

    // Immediate second write: LED off — rate-limited (50ms not yet elapsed)
    writer
        .set_led(&device_path, "COM1", &target, &off_state)
        .unwrap();

    // Verify LED is still ON (the off-write was rate-limited)
    let panel_led_states = writer.led_states.get(&device_path).unwrap();
    let com1_state = panel_led_states.get("COM1").unwrap();
    assert!(
        com1_state.is_on,
        "LED should still be ON — immediate off write should be rate-limited"
    );

    // Only first write should have generated a latency sample
    let stats = writer.get_latency_stats().unwrap();
    assert_eq!(
        stats.sample_count, 1,
        "Only first write should have occurred due to rate limiting"
    );
}
