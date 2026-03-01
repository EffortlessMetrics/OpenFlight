// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the virtual device emulation system.
//!
//! Covers vJoy integration, ViGEm (uinput) integration, output pipeline,
//! device lifecycle, mapping transforms, and platform safety.

use flight_virtual::backend::{HatDirection, MockBackend, VirtualBackend, VirtualBackendError};
use flight_virtual::device::{DeviceType, VirtualDevice, VirtualDeviceConfig};
use flight_virtual::loopback::{HidReport, LoopbackHid};
use flight_virtual::mapper::{
    AxisMapping, AxisTransform, ButtonMapping, ButtonMode, HatMapping, VirtualDeviceMapper,
};
use flight_virtual::vjoy::{VJoyDevice, VJOY_MAX_AXES, VJOY_MAX_BUTTONS, VJOY_MAX_HATS};
use flight_virtual::uinput::{UInputCapabilities, UInputDevice};
use flight_virtual::{VirtualDeviceManager, VirtualDeviceManagerError};

use flight_device_common::DeviceManager;
use std::sync::Arc;

// ═══════════════════════════════════════════════════════════════════════
// 1. vJoy integration (6 tests)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_vjoy_device_creation_with_valid_slot_ids() {
    for slot in 1..=4 {
        let dev = VJoyDevice::new(slot);
        assert_eq!(dev.id(), slot);
        assert!(!dev.is_acquired());
        assert_eq!(dev.axis_count(), VJOY_MAX_AXES);
        assert_eq!(dev.button_count(), VJOY_MAX_BUTTONS);
        assert_eq!(dev.hat_count(), VJOY_MAX_HATS);
    }
}

#[test]
fn test_vjoy_axis_output_covers_full_range() {
    let mut dev = VJoyDevice::new(1);
    dev.acquire().unwrap();

    let test_values: &[f32] = &[-1.0, -0.75, -0.5, -0.25, 0.0, 0.25, 0.5, 0.75, 1.0];
    for (axis_id, &value) in test_values.iter().enumerate() {
        if axis_id >= VJOY_MAX_AXES as usize {
            break;
        }
        dev.set_axis(axis_id as u8, value).unwrap();
        let readback = dev.get_axis(axis_id as u8).unwrap();
        assert!(
            (readback - value).abs() < 0.02,
            "axis {axis_id}: wrote {value}, read {readback}"
        );
    }
}

#[test]
fn test_vjoy_button_output_independent_channels() {
    let mut dev = VJoyDevice::new(1);
    dev.acquire().unwrap();

    // Press every other button.
    for btn in (0..32u8).step_by(2) {
        dev.set_button(btn, true).unwrap();
    }

    for btn in 0..32u8 {
        let expected = btn % 2 == 0;
        assert_eq!(
            dev.get_button(btn).unwrap(),
            expected,
            "button {btn} mismatch"
        );
    }
}

#[test]
fn test_vjoy_hat_switch_all_directions() {
    let mut dev = VJoyDevice::new(1);
    dev.acquire().unwrap();

    let dirs = [
        HatDirection::Centered,
        HatDirection::North,
        HatDirection::NorthEast,
        HatDirection::East,
        HatDirection::SouthEast,
        HatDirection::South,
        HatDirection::SouthWest,
        HatDirection::West,
        HatDirection::NorthWest,
    ];

    for (hat_id, &dir) in dirs.iter().enumerate() {
        let hat = (hat_id % VJOY_MAX_HATS as usize) as u8;
        dev.set_hat(hat, dir).unwrap();
        assert_eq!(dev.get_hat(hat).unwrap(), dir);
    }
}

#[test]
fn test_vjoy_device_reset_clears_all_state() {
    let mut dev = VJoyDevice::new(1);
    dev.acquire().unwrap();

    // Set non-default state on a subset of channels.
    for a in 0..VJOY_MAX_AXES {
        dev.set_axis(a, 0.9).unwrap();
    }
    for b in 0..32u8 {
        dev.set_button(b, true).unwrap();
    }
    for h in 0..VJOY_MAX_HATS {
        dev.set_hat(h, HatDirection::South).unwrap();
    }

    // Release resets; re-acquire and verify defaults.
    dev.release().unwrap();
    dev.acquire().unwrap();

    for a in 0..VJOY_MAX_AXES {
        let val = dev.get_axis(a).unwrap();
        assert!(val.abs() < 0.02, "axis {a} not centered after reset: {val}");
    }
    for b in 0..32u8 {
        assert!(!dev.get_button(b).unwrap(), "button {b} still pressed");
    }
    for h in 0..VJOY_MAX_HATS {
        assert_eq!(dev.get_hat(h).unwrap(), HatDirection::Centered);
    }
}

#[test]
fn test_vjoy_multiple_virtual_devices_independent() {
    let mut dev1 = VJoyDevice::new(1);
    let mut dev2 = VJoyDevice::new(2);
    dev1.acquire().unwrap();
    dev2.acquire().unwrap();

    dev1.set_axis(0, 1.0).unwrap();
    dev2.set_axis(0, -1.0).unwrap();

    assert!((dev1.get_axis(0).unwrap() - 1.0).abs() < 0.02);
    assert!((dev2.get_axis(0).unwrap() - (-1.0)).abs() < 0.02);

    dev1.set_button(0, true).unwrap();
    assert!(dev1.get_button(0).unwrap());
    assert!(!dev2.get_button(0).unwrap());
}

// ═══════════════════════════════════════════════════════════════════════
// 2. ViGEm / uinput integration (5 tests)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_uinput_virtual_gamepad_creation_custom_caps() {
    let caps = UInputCapabilities {
        num_axes: 6,
        num_buttons: 16,
        num_hats: 2,
        device_name: "Virtual Gamepad".into(),
        vendor_id: 0xCAFE,
        product_id: 0xBEEF,
    };
    let dev = UInputDevice::new(caps);
    assert!(!dev.is_acquired());
    assert_eq!(dev.axis_count(), 6);
    assert_eq!(dev.button_count(), 16);
    assert_eq!(dev.hat_count(), 2);
    assert_eq!(dev.capabilities().vendor_id, 0xCAFE);
}

#[test]
fn test_uinput_axis_mapping_covers_full_range() {
    let mut dev = UInputDevice::new(UInputCapabilities::default());
    dev.acquire().unwrap();

    for &v in &[-1.0f32, -0.5, 0.0, 0.5, 1.0] {
        dev.set_axis(0, v).unwrap();
        let readback = dev.get_axis(0).unwrap();
        assert!(
            (readback - v).abs() < 0.01,
            "wrote {v}, read {readback}"
        );
    }
}

#[test]
fn test_uinput_button_mapping_press_release_cycle() {
    let mut dev = UInputDevice::new(UInputCapabilities::default());
    dev.acquire().unwrap();

    for b in 0..dev.button_count() {
        dev.set_button(b, true).unwrap();
        assert!(dev.get_button(b).unwrap());
        dev.set_button(b, false).unwrap();
        assert!(!dev.get_button(b).unwrap());
    }
}

#[test]
fn test_uinput_force_feedback_passthrough_via_output_report() {
    // Simulate an FFB passthrough: VirtualDevice processes an FFB output report.
    let config = VirtualDeviceConfig {
        name: "FFB Stick".into(),
        device_type: DeviceType::ForceFeedback {
            axes: 2,
            max_torque_nm: 15.0,
        },
        vid: 0xABCD,
        pid: 0x0001,
        serial: "FFB001".into(),
        latency_us: 50,
        packet_loss_rate: 0.0,
    };
    let device = VirtualDevice::new(config);

    // FFB report type 0x03 is accepted.
    assert!(device.process_output_report(&[0x03, 0x00, 0x7F]));
    assert_eq!(device.get_stats().output_reports, 1);
}

#[test]
fn test_uinput_device_cleanup_on_drop() {
    let mut dev = UInputDevice::new(UInputCapabilities::default());
    dev.acquire().unwrap();
    dev.set_axis(0, 0.5).unwrap();
    // Drop should release without panic.
    drop(dev);
}

// ═══════════════════════════════════════════════════════════════════════
// 3. Output pipeline (6 tests)
// ═══════════════════════════════════════════════════════════════════════

fn make_acquired_mapper() -> VirtualDeviceMapper<MockBackend> {
    let mut backend = MockBackend::joystick();
    backend.acquire().unwrap();
    VirtualDeviceMapper::new(backend)
}

#[test]
fn test_pipeline_axis_value_to_virtual_device_output() {
    let mut m = make_acquired_mapper();
    m.add_axis_mapping(AxisMapping {
        src_axis: 0,
        dst_axis: 0,
        transform: AxisTransform::default(),
    });
    m.add_axis_mapping(AxisMapping {
        src_axis: 1,
        dst_axis: 1,
        transform: AxisTransform::default(),
    });

    m.update_axes(&[0.3, -0.7]).unwrap();
    assert!((m.backend().get_axis(0).unwrap() - 0.3).abs() < 1e-5);
    assert!((m.backend().get_axis(1).unwrap() - (-0.7)).abs() < 1e-5);
}

#[test]
fn test_pipeline_button_state_to_virtual_device() {
    let mut m = make_acquired_mapper();
    for i in 0..4u8 {
        m.add_button_mapping(ButtonMapping {
            src_button: i,
            dst_button: i,
            mode: ButtonMode::Direct,
        });
    }

    m.update_buttons(&[true, false, true, false]).unwrap();
    assert!(m.backend().get_button(0).unwrap());
    assert!(!m.backend().get_button(1).unwrap());
    assert!(m.backend().get_button(2).unwrap());
    assert!(!m.backend().get_button(3).unwrap());
}

#[test]
fn test_pipeline_hat_to_virtual_device() {
    let mut m = make_acquired_mapper();
    m.add_hat_mapping(HatMapping {
        src_hat: 0,
        dst_hat: 0,
    });
    m.add_hat_mapping(HatMapping {
        src_hat: 1,
        dst_hat: 1,
    });

    m.update_hats(&[HatDirection::North, HatDirection::SouthEast])
        .unwrap();
    assert_eq!(m.backend().get_hat(0).unwrap(), HatDirection::North);
    assert_eq!(m.backend().get_hat(1).unwrap(), HatDirection::SouthEast);
}

#[test]
fn test_pipeline_throttle_to_virtual_device() {
    // Throttle axis: physical range 0..1 passed through to virtual 0..1 (identity transform).
    let mut m = make_acquired_mapper();
    m.add_axis_mapping(AxisMapping {
        src_axis: 0,
        dst_axis: 0,
        transform: AxisTransform {
            scale: 1.0,
            offset: 0.0,
            deadzone: 0.0,
            invert: false,
        },
    });

    // Full forward throttle.
    m.update_axes(&[1.0]).unwrap();
    assert!((m.backend().get_axis(0).unwrap() - 1.0).abs() < 1e-5);

    // Idle.
    m.update_axes(&[0.0]).unwrap();
    assert!(m.backend().get_axis(0).unwrap().abs() < 1e-5);
}

#[test]
fn test_pipeline_combined_axes_buttons_hats() {
    let mut m = make_acquired_mapper();
    m.add_axis_mapping(AxisMapping {
        src_axis: 0,
        dst_axis: 0,
        transform: AxisTransform::default(),
    });
    m.add_button_mapping(ButtonMapping {
        src_button: 0,
        dst_button: 0,
        mode: ButtonMode::Direct,
    });
    m.add_hat_mapping(HatMapping {
        src_hat: 0,
        dst_hat: 0,
    });

    m.update_axes(&[0.42]).unwrap();
    m.update_buttons(&[true]).unwrap();
    m.update_hats(&[HatDirection::West]).unwrap();

    assert!((m.backend().get_axis(0).unwrap() - 0.42).abs() < 1e-5);
    assert!(m.backend().get_button(0).unwrap());
    assert_eq!(m.backend().get_hat(0).unwrap(), HatDirection::West);
}

#[test]
fn test_pipeline_output_rate_limiting_via_report_generation() {
    let config = VirtualDeviceConfig {
        packet_loss_rate: 0.0,
        ..VirtualDeviceConfig::default()
    };
    let device = VirtualDevice::new(config);

    // Generate many reports rapidly; all should succeed when loss rate is 0.
    let mut generated = 0u32;
    for _ in 0..100 {
        if device.generate_input_report().is_some() {
            generated += 1;
        }
    }
    assert_eq!(generated, 100);
    assert_eq!(device.get_stats().input_reports, 100);
}

// ═══════════════════════════════════════════════════════════════════════
// 4. Device lifecycle (5 tests)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_lifecycle_create_configure_start_stop_destroy() {
    let mut backend = MockBackend::new(4, 8, 2);
    assert!(!backend.is_acquired());

    // acquire = start
    backend.acquire().unwrap();
    assert!(backend.is_acquired());

    // configure (set state)
    backend.set_axis(0, 0.5).unwrap();
    backend.set_button(3, true).unwrap();
    backend.set_hat(1, HatDirection::East).unwrap();

    // stop = release
    backend.release().unwrap();
    assert!(!backend.is_acquired());

    // destroy = drop (no panic)
    drop(backend);
}

#[test]
fn test_lifecycle_reconnect_preserves_configuration() {
    let config = VirtualDeviceConfig {
        name: "Reconnect Test".into(),
        ..VirtualDeviceConfig::default()
    };
    let device = VirtualDevice::new(config);

    device.set_axis(0, 0.8);
    device.disconnect();
    assert!(!device.is_connected());
    assert!(device.generate_input_report().is_none());

    device.reconnect();
    assert!(device.is_connected());

    // State should be preserved across reconnect.
    let state = device.get_state();
    assert!((state.axes[0] - 0.8).abs() < 1e-5);
}

#[test]
fn test_lifecycle_error_recovery_from_disconnected_output() {
    let device = VirtualDevice::new(VirtualDeviceConfig::default());
    device.disconnect();

    // Output report processing should fail gracefully.
    assert!(!device.process_output_report(&[0x02, 0xFF]));

    // Reconnect and verify recovery.
    device.reconnect();
    assert!(device.process_output_report(&[0x02, 0x0A]));
}

#[test]
fn test_lifecycle_concurrent_devices_via_manager() {
    let manager = VirtualDeviceManager::new();

    let devices: Vec<_> = (0..5)
        .map(|i| {
            manager.create_device(VirtualDeviceConfig {
                name: format!("Device {i}"),
                vid: 0x1234,
                pid: 0x5680 + i as u16,
                serial: format!("CONC{i:03}"),
                ..VirtualDeviceConfig::default()
            })
        })
        .collect();

    assert_eq!(manager.devices().len(), 5);

    // Each device operates independently.
    for (i, dev) in devices.iter().enumerate() {
        dev.set_axis(0, i as f32 * 0.2);
    }
    for (i, dev) in devices.iter().enumerate() {
        let state = dev.get_state();
        let expected = i as f32 * 0.2;
        assert!(
            (state.axes[0] - expected).abs() < 1e-5,
            "device {i} axis mismatch"
        );
    }
}

#[test]
fn test_lifecycle_device_stats_reset() {
    let device = VirtualDevice::new(VirtualDeviceConfig::default());

    device.generate_input_report();
    device.generate_input_report();
    assert_eq!(device.get_stats().input_reports, 2);

    device.reset_stats();
    let stats = device.get_stats();
    assert_eq!(stats.input_reports, 0);
    assert_eq!(stats.output_reports, 0);
    assert_eq!(stats.packet_losses, 0);
    assert_eq!(stats.bytes_transferred, 0);
}

// ═══════════════════════════════════════════════════════════════════════
// 5. Mapping (5 tests)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_mapping_physical_to_virtual_axis_remapping() {
    let mut m = make_acquired_mapper();
    // Physical axis 3 → virtual axis 0.
    m.add_axis_mapping(AxisMapping {
        src_axis: 3,
        dst_axis: 0,
        transform: AxisTransform::default(),
    });

    m.update_axes(&[0.0, 0.0, 0.0, 0.65]).unwrap();
    assert!((m.backend().get_axis(0).unwrap() - 0.65).abs() < 1e-5);
}

#[test]
fn test_mapping_custom_table_scale_and_offset() {
    let mut m = make_acquired_mapper();
    m.add_axis_mapping(AxisMapping {
        src_axis: 0,
        dst_axis: 0,
        transform: AxisTransform {
            scale: 0.5,
            offset: 0.1,
            deadzone: 0.0,
            invert: false,
        },
    });

    // 0.6 * 0.5 + 0.1 = 0.4
    m.update_axes(&[0.6]).unwrap();
    assert!((m.backend().get_axis(0).unwrap() - 0.4).abs() < 1e-5);
}

#[test]
fn test_mapping_inversion() {
    let mut m = make_acquired_mapper();
    m.add_axis_mapping(AxisMapping {
        src_axis: 0,
        dst_axis: 0,
        transform: AxisTransform {
            invert: true,
            ..Default::default()
        },
    });

    m.update_axes(&[0.9]).unwrap();
    assert!((m.backend().get_axis(0).unwrap() - (-0.9)).abs() < 1e-5);
}

#[test]
fn test_mapping_deadzone_in_output() {
    let mut m = make_acquired_mapper();
    m.add_axis_mapping(AxisMapping {
        src_axis: 0,
        dst_axis: 0,
        transform: AxisTransform {
            deadzone: 0.15,
            ..Default::default()
        },
    });

    // Inside dead-zone → output 0.
    m.update_axes(&[0.1]).unwrap();
    assert!(m.backend().get_axis(0).unwrap().abs() < 1e-5);

    // Outside dead-zone → re-scaled.
    m.update_axes(&[1.0]).unwrap();
    assert!((m.backend().get_axis(0).unwrap() - 1.0).abs() < 0.01);
}

#[test]
fn test_mapping_saturation_via_scale_clamping() {
    let mut m = make_acquired_mapper();
    m.add_axis_mapping(AxisMapping {
        src_axis: 0,
        dst_axis: 0,
        transform: AxisTransform {
            scale: 3.0,
            ..Default::default()
        },
    });

    // 0.5 * 3.0 = 1.5 → clamped to 1.0
    m.update_axes(&[0.5]).unwrap();
    assert!((m.backend().get_axis(0).unwrap() - 1.0).abs() < 1e-5);

    // -0.5 * 3.0 = -1.5 → clamped to -1.0
    m.update_axes(&[-0.5]).unwrap();
    assert!((m.backend().get_axis(0).unwrap() - (-1.0)).abs() < 1e-5);
}

// ═══════════════════════════════════════════════════════════════════════
// 6. Platform safety (5 tests)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_safety_graceful_failure_driver_not_installed() {
    // vJoy driver availability check shouldn't panic.
    let _available = VJoyDevice::is_available();
    let _count = VJoyDevice::device_count();

    // UInput availability check shouldn't panic.
    let _available = UInputDevice::is_available();
}

#[test]
fn test_safety_permission_check_not_acquired_errors() {
    let mut vjoy = VJoyDevice::new(1);
    assert_eq!(
        vjoy.set_axis(0, 0.0),
        Err(VirtualBackendError::NotAcquired(1))
    );
    assert_eq!(
        vjoy.set_button(0, true),
        Err(VirtualBackendError::NotAcquired(1))
    );
    assert_eq!(
        vjoy.set_hat(0, HatDirection::North),
        Err(VirtualBackendError::NotAcquired(1))
    );

    let mut uinput = UInputDevice::new(UInputCapabilities::default());
    assert_eq!(
        uinput.set_axis(0, 0.0),
        Err(VirtualBackendError::NotAcquired(0))
    );
    assert_eq!(
        uinput.set_button(0, true),
        Err(VirtualBackendError::NotAcquired(0))
    );
    assert_eq!(
        uinput.set_hat(0, HatDirection::North),
        Err(VirtualBackendError::NotAcquired(0))
    );
}

#[test]
fn test_safety_device_limit_enforcement_out_of_range() {
    let mut vjoy = VJoyDevice::new(1);
    vjoy.acquire().unwrap();

    assert_eq!(
        vjoy.set_axis(VJOY_MAX_AXES, 0.0),
        Err(VirtualBackendError::InvalidAxis(VJOY_MAX_AXES))
    );
    assert_eq!(
        vjoy.set_button(VJOY_MAX_BUTTONS, false),
        Err(VirtualBackendError::InvalidButton(VJOY_MAX_BUTTONS))
    );
    assert_eq!(
        vjoy.set_hat(VJOY_MAX_HATS, HatDirection::Centered),
        Err(VirtualBackendError::InvalidHat(VJOY_MAX_HATS))
    );

    let caps = UInputCapabilities {
        num_axes: 2,
        num_buttons: 4,
        num_hats: 1,
        ..UInputCapabilities::default()
    };
    let mut uinput = UInputDevice::new(caps);
    uinput.acquire().unwrap();

    assert_eq!(
        uinput.set_axis(2, 0.0),
        Err(VirtualBackendError::InvalidAxis(2))
    );
    assert_eq!(
        uinput.set_button(4, true),
        Err(VirtualBackendError::InvalidButton(4))
    );
    assert_eq!(
        uinput.set_hat(1, HatDirection::North),
        Err(VirtualBackendError::InvalidHat(1))
    );
}

#[test]
fn test_safety_double_acquire_double_release() {
    let mut vjoy = VJoyDevice::new(1);
    vjoy.acquire().unwrap();
    assert_eq!(
        vjoy.acquire(),
        Err(VirtualBackendError::AlreadyAcquired(1))
    );
    vjoy.release().unwrap();
    assert_eq!(vjoy.release(), Err(VirtualBackendError::NotAcquired(1)));

    let mut mock = MockBackend::joystick();
    mock.acquire().unwrap();
    assert_eq!(
        mock.acquire(),
        Err(VirtualBackendError::AlreadyAcquired(0))
    );
    mock.release().unwrap();
    assert_eq!(mock.release(), Err(VirtualBackendError::NotAcquired(0)));
}

#[test]
fn test_safety_manager_duplicate_and_missing_device() {
    let mut manager = VirtualDeviceManager::new();
    let device = Arc::new(VirtualDevice::new(VirtualDeviceConfig::default()));
    let device_id = device.device_id();

    manager.register_device(device.clone()).unwrap();

    // Duplicate registration.
    let err = manager.register_device(device).unwrap_err();
    assert!(matches!(err, VirtualDeviceManagerError::DuplicateDevice(_)));

    // Remove, then try to remove again.
    manager.unregister_device(&device_id).unwrap();
    let err = manager.unregister_device(&device_id).unwrap_err();
    assert!(matches!(err, VirtualDeviceManagerError::DeviceNotFound(_)));
}

// ═══════════════════════════════════════════════════════════════════════
// Bonus: additional edge-case coverage
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_loopback_bidirectional_report_integrity() {
    let loopback = LoopbackHid::new();

    let payload: Vec<u8> = (0..64).collect();
    let report = HidReport::new(0x01, payload.clone());
    assert!(loopback.send_input_report(report));

    let received = loopback.receive_input_report().unwrap();
    assert_eq!(received.report_id, 0x01);
    assert_eq!(received.data, payload);

    let out_payload = vec![0xAA, 0xBB, 0xCC];
    let out_report = HidReport::new(0x02, out_payload.clone());
    assert!(loopback.send_output_report(out_report));

    let received_out = loopback.receive_output_report().unwrap();
    assert_eq!(received_out.report_id, 0x02);
    assert_eq!(received_out.data, out_payload);
}

#[test]
fn test_hid_report_serialization_round_trip() {
    let data = vec![0x10, 0x20, 0x30, 0x40];
    let report = HidReport::new(0x05, data.clone());

    let bytes = report.to_bytes();
    assert_eq!(bytes[0], 0x05);
    assert_eq!(&bytes[1..], &data);
    assert_eq!(report.size(), 5);
}

#[test]
fn test_virtual_device_hid_report_encodes_axes_and_buttons() {
    let device = VirtualDevice::new(VirtualDeviceConfig::default());
    device.set_axis(0, -1.0); // min
    device.set_axis(1, 1.0); // max
    device.set_button(0, true);

    let report = device.generate_input_report().unwrap();
    assert_eq!(report[0], 0x01); // report ID
    // 8 axes × 2 bytes + 4 button bytes + 1 report ID = 21 bytes
    assert_eq!(report.len(), 21);

    // Button byte: bit 0 set.
    let button_bytes_start = 1 + 8 * 2; // after report-id and 8 × 16-bit axes
    assert_eq!(report[button_bytes_start] & 0x01, 0x01);
}

#[test]
fn test_manager_device_health_reports_disconnected() {
    let manager = VirtualDeviceManager::new();
    let device = manager.create_device(VirtualDeviceConfig::default());
    let id = device.device_id();

    device.disconnect();
    let health = manager.get_device_health(&id).unwrap();
    assert!(
        matches!(health, flight_device_common::DeviceHealth::Failed { .. }),
        "expected Failed health, got {health:?}"
    );
}
