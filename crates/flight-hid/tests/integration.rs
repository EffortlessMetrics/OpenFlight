// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests for the `flight-hid` crate.
//!
//! These complement the proptest invariants in `proptest_axis_calibration.rs`
//! with deterministic scenario-based tests covering concrete calibration
//! values and the `HidAdapter` device-lifecycle API.

use flight_hid::calibration::AxisCalibration;
use flight_hid::{EndpointId, EndpointType, HidAdapter, HidDeviceInfo, HidOperationResult};
use flight_watchdog::WatchdogSystem;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

// ── helpers ───────────────────────────────────────────────────────────────────

fn make_adapter() -> HidAdapter {
    let watchdog = Arc::new(Mutex::new(WatchdogSystem::new()));
    HidAdapter::new(watchdog)
}

fn device_info(vid: u16, pid: u16, path: &str) -> HidDeviceInfo {
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

fn stick_16bit() -> AxisCalibration {
    AxisCalibration {
        raw_min: 0,
        raw_max: 65535,
        raw_center: 32767,
        deadzone: 0.0,
        output_min: -1.0,
        output_max: 1.0,
        reversed: false,
    }
}

// ── AxisCalibration: concrete pipeline tests ──────────────────────────────────

/// A 16-bit stick must map raw endpoints to ±1.0 and raw_center to 0.0.
#[test]
fn test_full_pipeline_normalizes_axis() {
    let cal = stick_16bit();
    assert!((cal.normalize(32767) - 0.0_f32).abs() < 1e-3, "centre → 0");
    assert!((cal.normalize(65535) - 1.0_f32).abs() < 1e-3, "max → 1");
    assert!((cal.normalize(0) - (-1.0_f32)).abs() < 1e-3, "min → -1");
    // Values beyond the hardware range must be clamped to the output range.
    assert!(
        (cal.normalize(u32::MAX) - 1.0_f32).abs() < 1e-3,
        "over-range clamped to 1"
    );
}

/// Inputs inside the deadzone radius must all map to the same centre value.
#[test]
fn test_deadzone_center_region() {
    let cal = AxisCalibration {
        raw_min: 0,
        raw_max: 65535,
        raw_center: 32767,
        deadzone: 0.10, // 10 % deadzone
        output_min: -1.0,
        output_max: 1.0,
        reversed: false,
    };

    let center_out = cal.normalize(32767);

    // +1 000 counts ≈ 3 % of half-range — well inside the 10 % deadzone.
    let near_center = cal.normalize(32767 + 1_000);
    assert_eq!(
        center_out, near_center,
        "small perturbation inside deadzone must snap to centre"
    );

    // A value near the physical maximum must escape the deadzone.
    let far = cal.normalize(65535);
    assert!(
        (far - center_out).abs() > 0.5,
        "large input must escape deadzone"
    );
}

/// When `reversed = true`, raw_min maps to output_max and raw_max to output_min.
#[test]
fn test_reversed_axis_inverts_range() {
    let cal = AxisCalibration {
        raw_min: 0,
        raw_max: 1023,
        raw_center: 511,
        deadzone: 0.0,
        output_min: -1.0,
        output_max: 1.0,
        reversed: true,
    };

    assert!(
        (cal.normalize(0) - 1.0_f32).abs() < 1e-3,
        "reversed: raw_min → output_max"
    );
    assert!(
        (cal.normalize(1023) - (-1.0_f32)).abs() < 1e-3,
        "reversed: raw_max → output_min"
    );
}

/// A unipolar throttle axis (output [0, 1]) normalises correctly.
#[test]
fn test_unipolar_throttle_axis() {
    let cal = AxisCalibration {
        raw_min: 0,
        raw_max: 255,
        raw_center: 0,
        deadzone: 0.0,
        output_min: 0.0,
        output_max: 1.0,
        reversed: false,
    };

    assert!(
        (cal.normalize(0) - 0.0_f32).abs() < 1e-4,
        "throttle min → 0"
    );
    assert!(
        (cal.normalize(255) - 1.0_f32).abs() < 1e-4,
        "throttle max → 1"
    );
    let mid = 127.0_f32 / 255.0_f32;
    assert!((cal.normalize(127) - mid).abs() < 1e-4, "throttle midpoint");
}

/// When raw_min == raw_max (degenerate), normalize returns the output midpoint.
#[test]
fn test_degenerate_raw_range_returns_midpoint() {
    let cal = AxisCalibration {
        raw_min: 100,
        raw_max: 100,
        raw_center: 100,
        deadzone: 0.0,
        output_min: -1.0,
        output_max: 1.0,
        reversed: false,
    };

    assert!(
        (cal.normalize(100) - 0.0_f32).abs() < 1e-4,
        "degenerate range → midpoint (0.0)"
    );
}

// ── HidAdapter: device lifecycle tests ───────────────────────────────────────

/// All fields in a `HidDeviceInfo` are preserved after registration.
#[test]
fn test_device_info_fields_preserved_on_registration() {
    let mut adapter = make_adapter();
    let info = HidDeviceInfo {
        vendor_id: 0xABCD,
        product_id: 0x1234,
        serial_number: Some("SN-001".to_string()),
        manufacturer: Some("ACME Corp".to_string()),
        product_name: Some("Flight Stick Pro".to_string()),
        device_path: "test/path/0".to_string(),
        usage_page: 0x01,
        usage: 0x04,
        report_descriptor: Some(vec![0x05, 0x01]),
    };

    adapter.register_device(info).unwrap();
    let retrieved = adapter
        .get_device_info("test/path/0")
        .expect("device must be present after registration");

    assert_eq!(retrieved.vendor_id, 0xABCD);
    assert_eq!(retrieved.product_id, 0x1234);
    assert_eq!(retrieved.serial_number.as_deref(), Some("SN-001"));
    assert_eq!(retrieved.manufacturer.as_deref(), Some("ACME Corp"));
    assert_eq!(retrieved.product_name.as_deref(), Some("Flight Stick Pro"));
    assert_eq!(retrieved.usage_page, 0x01);
    assert_eq!(retrieved.usage, 0x04);
    assert!(retrieved.report_descriptor.is_some());
}

/// A device with usage_page=0 and usage=0 (no specific HID usage) is still
/// accepted by the adapter without error.
#[test]
fn test_device_with_zero_axes_is_valid() {
    let mut adapter = make_adapter();
    let info = HidDeviceInfo {
        vendor_id: 0x1111,
        product_id: 0x2222,
        serial_number: None,
        manufacturer: None,
        product_name: None,
        device_path: "test/zero-usage".to_string(),
        usage_page: 0x00,
        usage: 0x00,
        report_descriptor: None,
    };

    assert!(adapter.register_device(info).is_ok());
    assert_eq!(adapter.get_all_devices().len(), 1);
}

/// Multiple devices can be registered and all are returned by `get_all_devices`.
#[test]
fn test_multiple_devices_registered_and_visible() {
    let mut adapter = make_adapter();
    for i in 0..3u16 {
        adapter
            .register_device(device_info(0x1234, i, &format!("test/dev/{i}")))
            .unwrap();
    }
    assert_eq!(adapter.get_all_devices().len(), 3);
}

/// Looking up an unregistered path returns `None`, not a panic.
#[test]
fn test_unregistered_path_returns_none() {
    let adapter = make_adapter();
    assert!(adapter.get_device_info("nonexistent/path").is_none());
}

/// Reading from a device path with no open handle returns the `Error` variant
/// wrapped in `Ok` (not `Err`).  The adapter must not panic.
#[test]
fn test_read_from_unregistered_path_returns_error_result() {
    let mut adapter = make_adapter();
    let mut buf = [0u8; 64];
    let result = adapter
        .read_input("nonexistent/path", &mut buf)
        .expect("perform_operation must not propagate Err for missing handle");
    assert!(
        matches!(result, HidOperationResult::Error { .. }),
        "expected Error variant for unknown path, got {:?}",
        result
    );
}

/// Writing to a device path with no open handle returns the `Error` variant
/// wrapped in `Ok`.
#[test]
fn test_write_to_unregistered_path_returns_error_result() {
    let mut adapter = make_adapter();
    let data = [0x01u8; 8];
    let result = adapter
        .write_output("nonexistent/path", &data)
        .expect("perform_operation must not propagate Err for missing handle");
    assert!(
        matches!(result, HidOperationResult::Error { .. }),
        "expected Error variant for unknown path, got {:?}",
        result
    );
}

/// Statistics correctly reflect the number of registered devices and their
/// endpoints (2 endpoints — Input + Output — per device).
#[test]
fn test_statistics_reflect_registered_devices() {
    let mut adapter = make_adapter();
    adapter
        .register_device(device_info(0x1, 0x1, "path/a"))
        .unwrap();
    adapter
        .register_device(device_info(0x2, 0x2, "path/b"))
        .unwrap();

    let stats = adapter.get_statistics();
    assert_eq!(stats.total_devices, 2);
    // Two devices × 2 endpoints each (Input + Output) = 4.
    assert_eq!(stats.total_endpoints, 4);
    assert_eq!(stats.total_operations, 0);
    assert_eq!(stats.total_bytes, 0);
    assert_eq!(stats.failed_endpoints, 0);
}

/// `EndpointId` equality is determined by both `device_path` and
/// `endpoint_type`; the two together form a unique key suitable for use in a
/// `HashSet` or `HashMap`.
#[test]
fn test_endpoint_id_equality_and_hashing() {
    let a = EndpointId {
        device_path: "path/dev".to_string(),
        endpoint_type: EndpointType::Input,
    };
    let b = EndpointId {
        device_path: "path/dev".to_string(),
        endpoint_type: EndpointType::Input,
    };
    let c = EndpointId {
        device_path: "path/dev".to_string(),
        endpoint_type: EndpointType::Output,
    };

    assert_eq!(a, b, "same path + type must be equal");
    assert_ne!(a, c, "different endpoint type must not be equal");

    let mut set = HashSet::new();
    set.insert(a);
    set.insert(b); // duplicate — must not grow the set
    set.insert(c);
    assert_eq!(
        set.len(),
        2,
        "distinct endpoint types must hash differently"
    );
}

/// `EndpointType::Feature` is a valid variant and compares correctly.
#[test]
fn test_endpoint_type_feature_variant() {
    let feat = EndpointId {
        device_path: "path/dev".to_string(),
        endpoint_type: EndpointType::Feature,
    };
    let input = EndpointId {
        device_path: "path/dev".to_string(),
        endpoint_type: EndpointType::Input,
    };
    assert_ne!(feat, input);
    assert_eq!(
        format!("{:?}", feat.endpoint_type),
        "Feature",
        "Debug output must match variant name"
    );
}
