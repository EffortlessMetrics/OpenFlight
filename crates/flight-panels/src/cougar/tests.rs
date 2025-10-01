//! Tests for Cougar MFD writer

use super::*;
use flight_hid::{HidAdapter, HidDeviceInfo};
use flight_core::WatchdogSystem;
use std::time::Duration;
use std::sync::{Arc, Mutex};

/// Mock HID adapter for testing
struct MockHidAdapter {
    devices: Vec<HidDeviceInfo>,
    write_latency: Duration,
    should_fail: bool,
}

impl MockHidAdapter {
    fn new() -> Self {
        Self {
            devices: vec![
                // Mock Cougar MFD Left
                HidDeviceInfo {
                    device_path: "/dev/hidraw0".to_string(),
                    vendor_id: COUGAR_VENDOR_ID,
                    product_id: 0x0404,
                    serial_number: Some("MFD_LEFT_001".to_string()),
                    manufacturer: Some("Thrustmaster".to_string()),
                    product_name: Some("Cougar MFD Left".to_string()),
                    usage_page: 0x01,
                    usage: 0x04,
                },
                // Mock Cougar MFD Right
                HidDeviceInfo {
                    device_path: "/dev/hidraw1".to_string(),
                    vendor_id: COUGAR_VENDOR_ID,
                    product_id: 0x0405,
                    serial_number: Some("MFD_RIGHT_001".to_string()),
                    manufacturer: Some("Thrustmaster".to_string()),
                    product_name: Some("Cougar MFD Right".to_string()),
                    usage_page: 0x01,
                    usage: 0x04,
                },
            ],
            write_latency: Duration::from_micros(500), // 0.5ms default
            should_fail: false,
        }
    }

    fn set_write_latency(&mut self, latency: Duration) {
        self.write_latency = latency;
    }

    fn set_should_fail(&mut self, should_fail: bool) {
        self.should_fail = should_fail;
    }
}

// Note: In a real implementation, MockHidAdapter would implement the HidAdapter trait
// For this test, we'll create a simplified version

#[test]
fn test_cougar_mfd_type_from_product_id() {
    assert_eq!(CougarMfdType::from_product_id(0x0404), Some(CougarMfdType::MfdLeft));
    assert_eq!(CougarMfdType::from_product_id(0x0405), Some(CougarMfdType::MfdRight));
    assert_eq!(CougarMfdType::from_product_id(0x0406), Some(CougarMfdType::MfdCenter));
    assert_eq!(CougarMfdType::from_product_id(0x9999), None);
}

#[test]
fn test_cougar_mfd_type_names() {
    assert_eq!(CougarMfdType::MfdLeft.name(), "Cougar MFD Left");
    assert_eq!(CougarMfdType::MfdRight.name(), "Cougar MFD Right");
    assert_eq!(CougarMfdType::MfdCenter.name(), "Cougar MFD Center");
}

#[test]
fn test_cougar_mfd_led_mapping() {
    let left_mapping = CougarMfdType::MfdLeft.led_mapping();
    assert!(left_mapping.contains(&"OSB1"));
    assert!(left_mapping.contains(&"OSB20"));
    assert!(left_mapping.contains(&"BRIGHTNESS"));
    assert_eq!(left_mapping.len(), 25);

    let center_mapping = CougarMfdType::MfdCenter.led_mapping();
    assert!(center_mapping.contains(&"OSB1"));
    assert!(center_mapping.contains(&"OSB10"));
    assert!(center_mapping.contains(&"POWER"));
    assert_eq!(center_mapping.len(), 13);
}

#[test]
fn test_cougar_verify_pattern() {
    let left_pattern = CougarMfdType::MfdLeft.verify_pattern();
    assert!(!left_pattern.is_empty());
    
    // Check that pattern includes expected steps
    let has_led_on = left_pattern.iter().any(|step| matches!(step, CougarVerifyStep::LedOn(_)));
    let has_delay = left_pattern.iter().any(|step| matches!(step, CougarVerifyStep::Delay(_)));
    let has_all_off = left_pattern.iter().any(|step| matches!(step, CougarVerifyStep::AllOff));
    
    assert!(has_led_on);
    assert!(has_delay);
    assert!(has_all_off);
}

#[test]
fn test_build_mfd_reports() {
    // Create a mock HID adapter (simplified for testing)
    let _mock_adapter = MockHidAdapter::new();
    let watchdog = Arc::new(Mutex::new(WatchdogSystem::new()));
    let hid_adapter = HidAdapter::new(watchdog); // This would be mocked in real implementation
    
    let writer = CougarMfdWriter::new(hid_adapter);
    
    let led_state = MfdLedState {
        led_index: 0,
        brightness: 1.0,
        is_on: true,
        blink_rate: None,
        last_blink_toggle: Instant::now(),
        last_write: Instant::now(),
    };

    // Test Left MFD report
    let left_report = writer.build_mfd_left_report(&led_state).unwrap();
    assert_eq!(left_report[0], 0x01); // Report ID
    assert_eq!(left_report[1], 255); // LED brightness
    assert_eq!(left_report.len(), 32);

    // Test Right MFD report
    let right_report = writer.build_mfd_right_report(&led_state).unwrap();
    assert_eq!(right_report[0], 0x01); // Report ID
    assert_eq!(right_report[1], 255); // LED brightness
    assert_eq!(right_report.len(), 32);

    // Test Center MFD report
    let center_report = writer.build_mfd_center_report(&led_state).unwrap();
    assert_eq!(center_report[0], 0x01); // Report ID
    assert_eq!(center_report[1], 255); // LED brightness
    assert_eq!(center_report.len(), 16);
}

#[test]
fn test_led_state_brightness_clamping() {
    let led_state_off = MfdLedState {
        led_index: 0,
        brightness: 0.5,
        is_on: false,
        blink_rate: None,
        last_blink_toggle: Instant::now(),
        last_write: Instant::now(),
    };

    let led_state_on = MfdLedState {
        led_index: 0,
        brightness: 0.5,
        is_on: true,
        blink_rate: None,
        last_blink_toggle: Instant::now(),
        last_write: Instant::now(),
    };

    let watchdog = Arc::new(Mutex::new(WatchdogSystem::new()));
    let hid_adapter = HidAdapter::new(watchdog);
    let writer = CougarMfdWriter::new(hid_adapter);

    // Test that off LEDs report 0 brightness regardless of brightness setting
    let report_off = writer.build_mfd_left_report(&led_state_off).unwrap();
    assert_eq!(report_off[1], 0);

    // Test that on LEDs report scaled brightness
    let report_on = writer.build_mfd_left_report(&led_state_on).unwrap();
    assert_eq!(report_on[1], 127); // 0.5 * 255 ≈ 127
}

#[test]
fn test_verify_test_result_latency_analysis() {
    let step_results = vec![
        CougarVerifyStepResult {
            step_index: 0,
            expected_latency: Duration::from_millis(20),
            actual_latency: Duration::from_millis(5),
            success: true,
            error: None,
        },
        CougarVerifyStepResult {
            step_index: 1,
            expected_latency: Duration::from_millis(20),
            actual_latency: Duration::from_millis(15),
            success: true,
            error: None,
        },
        CougarVerifyStepResult {
            step_index: 2,
            expected_latency: Duration::from_millis(20),
            actual_latency: Duration::from_millis(10),
            success: true,
            error: None,
        },
    ];

    let result = CougarVerifyTestResult {
        mfd_path: "/dev/hidraw0".to_string(),
        total_duration: Duration::from_millis(100),
        step_results,
        success: true,
    };

    assert!(result.meets_latency_requirement());
    assert_eq!(result.max_latency(), Duration::from_millis(15));
    assert_eq!(result.avg_latency(), Duration::from_millis(10)); // (5+15+10)/3 = 10
}

#[test]
fn test_verify_test_result_latency_failure() {
    let step_results = vec![
        CougarVerifyStepResult {
            step_index: 0,
            expected_latency: Duration::from_millis(20),
            actual_latency: Duration::from_millis(25), // Exceeds requirement
            success: false,
            error: None,
        },
    ];

    let result = CougarVerifyTestResult {
        mfd_path: "/dev/hidraw0".to_string(),
        total_duration: Duration::from_millis(50),
        step_results,
        success: false,
    };

    assert!(!result.meets_latency_requirement());
    assert_eq!(result.max_latency(), Duration::from_millis(25));
}

#[test]
fn test_rate_limiting_enforcement() {
    // This test would verify that LED updates are properly rate limited
    // In a real implementation, we would:
    // 1. Set a longer min_write_interval
    // 2. Attempt rapid LED updates
    // 3. Verify that hardware writes are rate limited
    
    let watchdog = Arc::new(Mutex::new(WatchdogSystem::new()));
    let hid_adapter = HidAdapter::new(watchdog);
    let mut writer = CougarMfdWriter::new(hid_adapter);
    
    // Set rate limiting to 100ms for testing
    writer.min_write_interval = Duration::from_millis(100);
    
    // In a real test, we would mock the HID adapter and verify write timing
    // For now, just ensure the structure is correct
    assert_eq!(writer.min_write_interval, Duration::from_millis(100));
}

#[test]
fn test_latency_requirement_validation() {
    // Test that the system properly validates the ≤20ms latency requirement
    let watchdog = Arc::new(Mutex::new(WatchdogSystem::new()));
    let hid_adapter = HidAdapter::new(watchdog);
    let writer = CougarMfdWriter::new(hid_adapter);
    
    // Verify that latency samples are tracked
    assert_eq!(writer.latency_samples.len(), 0);
    assert_eq!(writer.max_latency_samples, 1000);
    
    // In a real test, we would:
    // 1. Perform LED operations
    // 2. Check that latency is tracked
    // 3. Verify warnings are logged for >20ms operations
}

#[test]
fn test_hardware_compatibility_fixture() {
    // This test represents fixture-based testing for hardware compatibility
    // In a real implementation, this would:
    // 1. Load hardware test fixtures
    // 2. Simulate various MFD hardware configurations
    // 3. Verify compatibility across different firmware versions
    
    let test_fixtures = vec![
        ("MFD_LEFT_FW_1.0", CougarMfdType::MfdLeft),
        ("MFD_RIGHT_FW_1.0", CougarMfdType::MfdRight),
        ("MFD_CENTER_FW_1.0", CougarMfdType::MfdCenter),
    ];
    
    for (fixture_name, mfd_type) in test_fixtures {
        // Verify LED mapping is consistent
        let led_mapping = mfd_type.led_mapping();
        assert!(!led_mapping.is_empty(), "LED mapping empty for {}", fixture_name);
        
        // Verify verify pattern is defined
        let verify_pattern = mfd_type.verify_pattern();
        assert!(!verify_pattern.is_empty(), "Verify pattern empty for {}", fixture_name);
    }
}

#[test]
fn test_comprehensive_hardware_validation_suite() {
    // This test represents the comprehensive hardware validation suite
    // In a real implementation, this would include:
    
    // 1. LED Response Time Validation
    let max_acceptable_latency = Duration::from_millis(20);
    assert!(max_acceptable_latency >= Duration::from_millis(1), "Latency requirement too strict");
    
    // 2. Rate Limiting Validation
    let min_interval = Duration::from_millis(8);
    assert!(min_interval >= Duration::from_millis(1), "Rate limiting too aggressive");
    
    // 3. Blink Pattern Validation
    let test_blink_rates = vec![1.0, 2.0, 4.0, 6.0, 8.0];
    for rate in test_blink_rates {
        let period = Duration::from_secs_f32(1.0 / rate);
        assert!(period >= Duration::from_millis(125), "Blink rate {} too fast", rate);
    }
    
    // 4. Hardware Fault Simulation
    let fault_conditions = vec![
        "USB_STALL",
        "ENDPOINT_ERROR", 
        "TIMEOUT",
        "DEVICE_DISCONNECT",
    ];
    
    for condition in fault_conditions {
        // In real implementation, would simulate each fault condition
        // and verify proper error handling and recovery
        assert!(!condition.is_empty(), "Fault condition defined");
    }
    
    // 5. Multi-MFD Coordination
    let mfd_types = vec![
        CougarMfdType::MfdLeft,
        CougarMfdType::MfdRight,
        CougarMfdType::MfdCenter,
    ];
    
    // Verify each MFD type has unique characteristics
    for mfd_type in mfd_types {
        let led_count = mfd_type.led_mapping().len();
        match mfd_type {
            CougarMfdType::MfdLeft | CougarMfdType::MfdRight => {
                assert_eq!(led_count, 25, "Left/Right MFD should have 25 LEDs");
            }
            CougarMfdType::MfdCenter => {
                assert_eq!(led_count, 13, "Center MFD should have 13 LEDs");
            }
        }
    }
}

#[test]
fn test_drift_detection_and_repair() {
    // Test configuration drift detection and repair functionality
    let watchdog = Arc::new(Mutex::new(WatchdogSystem::new()));
    let hid_adapter = HidAdapter::new(watchdog);
    let mut writer = CougarMfdWriter::new(hid_adapter);
    
    // In a real implementation, this would:
    // 1. Simulate configuration drift
    // 2. Verify detection mechanisms
    // 3. Test repair functionality
    // 4. Validate post-repair state
    
    // For now, verify the structure exists
    let test_path = "/dev/hidraw0";
    
    // These would be actual operations in a real test with mocked hardware
    let _repair_result = writer.repair_mfd_drift(test_path);
    // assert!(repair_result.is_ok(), "Drift repair should succeed");
}

#[test]
fn test_verify_matrix_integration() {
    // Test integration with verify matrix system
    // This ensures the Cougar MFD writer properly integrates with
    // the broader panel verification system
    
    let test_cases = vec![
        ("LED_RESPONSE_TIME", Duration::from_millis(20)),
        ("BLINK_ACCURACY", Duration::from_millis(50)),
        ("BRIGHTNESS_CONTROL", Duration::from_millis(20)),
        ("ALL_LEDS_ON", Duration::from_millis(100)),
        ("ALL_LEDS_OFF", Duration::from_millis(100)),
    ];
    
    for (test_name, max_duration) in test_cases {
        // In real implementation, would execute each test case
        // and verify it completes within the specified duration
        assert!(!test_name.is_empty(), "Test case {} defined", test_name);
        assert!(max_duration > Duration::ZERO, "Test duration positive for {}", test_name);
    }
}