//! Tests for Saitek/Logitech panel writer

use super::*;
use flight_hid::{HidAdapter};
use flight_core::WatchdogSystem;
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
    }
}

#[test]
fn test_panel_type_detection() {
    assert_eq!(PanelType::from_product_id(0x0D05), Some(PanelType::RadioPanel));
    assert_eq!(PanelType::from_product_id(0x0D06), Some(PanelType::MultiPanel));
    assert_eq!(PanelType::from_product_id(0x0D67), Some(PanelType::SwitchPanel));
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
    assert!(writer.set_led(&device_path, "GEAR", &target, &led_state).is_ok());
    
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
    assert!(writer.set_led(&device_path, "COM1", &target, &led_state).is_ok());
    
    // Immediate second write should be rate limited (but still return Ok)
    assert!(writer.set_led(&device_path, "COM1", &target, &led_state).is_ok());
}

#[test]
fn test_verify_pattern_generation() {
    let radio_pattern = PanelType::RadioPanel.verify_pattern();
    assert!(!radio_pattern.is_empty());
    
    // Check that pattern contains expected steps
    let has_led_on = radio_pattern.iter().any(|step| matches!(step, VerifyStep::LedOn(_)));
    let has_led_off = radio_pattern.iter().any(|step| matches!(step, VerifyStep::LedOff(_)));
    let has_delay = radio_pattern.iter().any(|step| matches!(step, VerifyStep::Delay(_)));
    
    assert!(has_led_on);
    assert!(has_led_off);
    assert!(has_delay);
    
    let bip_pattern = PanelType::BIP.verify_pattern();
    let has_all_on = bip_pattern.iter().any(|step| matches!(step, VerifyStep::AllOn));
    let has_all_off = bip_pattern.iter().any(|step| matches!(step, VerifyStep::AllOff));
    let has_blink = bip_pattern.iter().any(|step| matches!(step, VerifyStep::LedBlink(_, _)));
    
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
    
    // Update test multiple times to completion
    let mut result = None;
    for _ in 0..100 { // Limit iterations to prevent infinite loop
        match writer.update_verify_test() {
            Ok(Some(test_result)) => {
                result = Some(test_result);
                break;
            }
            Ok(None) => {
                // Test still running, add small delay to simulate time passage
                std::thread::sleep(Duration::from_millis(1));
                continue;
            }
            Err(e) => panic!("Verify test failed: {}", e),
        }
    }
    
    assert!(result.is_some());
    let test_result = result.unwrap();
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
        
        writer.set_led(&device_path, "ALT", &target, &led_state).unwrap();
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
    writer.set_led(&device_path, "MASTER_WARNING", &target, &blink_state).unwrap();
    
    // Update blink states multiple times
    for _ in 0..10 {
        writer.update_blink_states().unwrap();
        std::thread::sleep(Duration::from_millis(10)); // Allow time for blink toggle
    }
    
    // Verify that blink updates occurred (latency samples should increase)
    let stats = writer.get_latency_stats().unwrap();
    assert!(stats.sample_count > 1, "Blink updates should generate latency samples");
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
    writer.set_led(&device_path, "AUTOTHROTTLE", &target, &led_state).unwrap();
    
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

#[test]
fn test_min_interval_enforcement() {
    let hid_adapter = create_test_hid_adapter();
    let mut writer = SaitekPanelWriter::new(hid_adapter);
    
    // Set minimum interval to 50ms for testing
    writer.min_write_interval = Duration::from_millis(50);
    
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
    
    let start_time = Instant::now();
    
    // Multiple rapid writes should be rate limited
    for _ in 0..5 {
        writer.set_led(&device_path, "COM1", &target, &led_state).unwrap();
    }
    
    let elapsed = start_time.elapsed();
    
    // Should have been rate limited - not all writes should have occurred
    // In a real implementation, we'd check actual write timestamps
    assert!(elapsed >= Duration::from_millis(1), "Some rate limiting should have occurred");
    
    // Only first write should have generated latency sample due to rate limiting
    let stats = writer.get_latency_stats().unwrap();
    assert_eq!(stats.sample_count, 1, "Only first write should have occurred due to rate limiting");
}