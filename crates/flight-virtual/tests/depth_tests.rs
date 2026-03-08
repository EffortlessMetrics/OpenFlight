// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the `flight-virtual` crate.
//!
//! Covers the full public surface: `VirtualDevice`, `VirtualDeviceManager`,
//! `VirtualBackend` (Mock / VJoy / UInput), `VirtualDeviceMapper`,
//! `LoopbackHid`, and `HatDirection`.

use std::sync::Arc;
use std::time::Duration;

use flight_device_common::{DeviceHealth, DeviceId, DeviceManager, IdentifiedDevice};
use flight_virtual::backend::MockBackend;
use flight_virtual::device::{DeviceType, VirtualDevice, VirtualDeviceConfig};
use flight_virtual::loopback::{HidReport, LoopbackHid};
use flight_virtual::mapper::{
    AxisMapping, AxisTransform, ButtonMapping, ButtonMode, HatMapping, MergeStrategy,
    VirtualDeviceMapper,
};
use flight_virtual::uinput::{UInputCapabilities, UInputDevice};
use flight_virtual::vjoy::VJoyDevice;
use flight_virtual::{
    HatDirection, VirtualBackend, VirtualBackendError, VirtualDeviceManager,
    VirtualDeviceManagerError,
};

// ═══════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════

fn default_device() -> VirtualDevice {
    VirtualDevice::new(VirtualDeviceConfig::default())
}

fn joystick_config(serial: &str) -> VirtualDeviceConfig {
    VirtualDeviceConfig {
        name: format!("Test Stick {serial}"),
        device_type: DeviceType::Joystick { axes: 4 },
        vid: 0xAAAA,
        pid: 0xBBBB,
        serial: serial.to_string(),
        latency_us: 50,
        packet_loss_rate: 0.0,
    }
}

fn acquired_mock() -> MockBackend {
    let mut m = MockBackend::joystick();
    m.acquire().unwrap();
    m
}

fn mapper_with_mock() -> VirtualDeviceMapper<MockBackend> {
    VirtualDeviceMapper::new(acquired_mock())
}

// ═══════════════════════════════════════════════════════════════════════
// 1. VirtualDevice — creation & config
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn device_default_config_values() {
    let dev = default_device();
    let cfg = dev.config();
    assert_eq!(cfg.name, "Virtual Flight Stick");
    assert_eq!(cfg.vid, 0x1234);
    assert_eq!(cfg.pid, 0x5678);
    assert_eq!(cfg.serial, "VIRT001");
    assert_eq!(cfg.latency_us, 100);
    assert_eq!(cfg.packet_loss_rate, 0.0);
    assert!(matches!(cfg.device_type, DeviceType::Joystick { axes: 3 }));
}

#[test]
fn device_custom_types() {
    for dt in [
        DeviceType::Throttle { levers: 4 },
        DeviceType::Rudder,
        DeviceType::Panel {
            leds: 16,
            switches: 8,
        },
        DeviceType::ForceFeedback {
            axes: 2,
            max_torque_nm: 15.0,
        },
    ] {
        let cfg = VirtualDeviceConfig {
            device_type: dt,
            ..VirtualDeviceConfig::default()
        };
        let dev = VirtualDevice::new(cfg);
        assert!(dev.is_connected());
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 2. VirtualDevice — axis behaviour
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn device_axis_clamps_positive() {
    let dev = default_device();
    dev.set_axis(0, 5.0);
    assert!((dev.get_state().axes[0] - 1.0).abs() < 1e-5);
}

#[test]
fn device_axis_clamps_negative() {
    let dev = default_device();
    dev.set_axis(0, -5.0);
    assert!((dev.get_state().axes[0] - (-1.0)).abs() < 1e-5);
}

#[test]
fn device_axis_exact_boundaries() {
    let dev = default_device();
    for v in [-1.0_f32, 0.0, 1.0] {
        dev.set_axis(0, v);
        assert!((dev.get_state().axes[0] - v).abs() < 1e-5, "v={v}");
    }
}

#[test]
fn device_axis_out_of_range_index_ignored() {
    let dev = default_device();
    // 8 axes by default — index 8 should be silently ignored
    dev.set_axis(8, 0.5);
    // All axes should still be at defaults (0.0)
    for &v in &dev.get_state().axes {
        assert!(v.abs() < 1e-5);
    }
}

#[test]
fn device_multiple_axes_independent() {
    let dev = default_device();
    dev.set_axis(0, 0.1);
    dev.set_axis(1, 0.2);
    dev.set_axis(2, 0.3);
    let s = dev.get_state();
    assert!((s.axes[0] - 0.1).abs() < 1e-5);
    assert!((s.axes[1] - 0.2).abs() < 1e-5);
    assert!((s.axes[2] - 0.3).abs() < 1e-5);
}

// ═══════════════════════════════════════════════════════════════════════
// 3. VirtualDevice — buttons
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn device_button_toggle_on_off() {
    let dev = default_device();
    dev.set_button(5, true);
    assert_ne!(dev.get_state().buttons & (1 << 5), 0);

    dev.set_button(5, false);
    assert_eq!(dev.get_state().buttons & (1 << 5), 0);
}

#[test]
fn device_buttons_are_independent() {
    let dev = default_device();
    dev.set_button(0, true);
    dev.set_button(3, true);
    let b = dev.get_state().buttons;
    assert_ne!(b & 0x01, 0);
    assert_ne!(b & 0x08, 0);
    assert_eq!(b & 0x02, 0); // button 1 not set
}

// ═══════════════════════════════════════════════════════════════════════
// 4. VirtualDevice — HID reports
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn device_hid_report_starts_with_report_id() {
    let dev = default_device();
    let report = dev.generate_input_report().unwrap();
    assert_eq!(report[0], 0x01);
}

#[test]
fn device_hid_report_encodes_center_axis() {
    let dev = default_device();
    dev.set_axis(0, 0.0); // center = 32768 approx
    let report = dev.generate_input_report().unwrap();
    let lo = report[1] as u16;
    let hi = report[2] as u16;
    let raw = lo | (hi << 8);
    // Center should map to ~32767-32768
    assert!((32766..=32769).contains(&raw), "raw={raw}");
}

#[test]
fn device_hid_report_increments_stat() {
    let dev = default_device();
    assert_eq!(dev.get_stats().input_reports, 0);
    dev.generate_input_report();
    dev.generate_input_report();
    assert_eq!(dev.get_stats().input_reports, 2);
}

#[test]
fn device_hid_report_none_when_disconnected() {
    let dev = default_device();
    dev.disconnect();
    assert!(dev.generate_input_report().is_none());
}

// ═══════════════════════════════════════════════════════════════════════
// 5. VirtualDevice — output reports
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn device_output_report_led_control() {
    let dev = default_device();
    assert!(dev.process_output_report(&[0x02, 0xFF]));
    assert_eq!(dev.get_state().leds, 0xFF);
    assert_eq!(dev.get_stats().output_reports, 1);
}

#[test]
fn device_output_report_unknown_type_returns_false() {
    let dev = default_device();
    assert!(!dev.process_output_report(&[0xAA, 0x00]));
}

#[test]
fn device_output_report_empty_returns_false() {
    let dev = default_device();
    assert!(!dev.process_output_report(&[]));
}

#[test]
fn device_output_report_rejected_when_disconnected() {
    let dev = default_device();
    dev.disconnect();
    assert!(!dev.process_output_report(&[0x02, 0x0F]));
}

// ═══════════════════════════════════════════════════════════════════════
// 6. VirtualDevice — connect / disconnect / reconnect
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn device_disconnect_reconnect_cycle() {
    let dev = default_device();
    assert!(dev.is_connected());

    dev.disconnect();
    assert!(!dev.is_connected());

    dev.reconnect();
    assert!(dev.is_connected());
    assert!(dev.generate_input_report().is_some());
}

// ═══════════════════════════════════════════════════════════════════════
// 7. VirtualDevice — stats reset
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn device_stats_reset_clears_all() {
    let dev = default_device();
    dev.generate_input_report();
    dev.process_output_report(&[0x02, 0x01]);
    dev.reset_stats();
    let s = dev.get_stats();
    assert_eq!(s.input_reports, 0);
    assert_eq!(s.output_reports, 0);
    assert_eq!(s.bytes_transferred, 0);
}

// ═══════════════════════════════════════════════════════════════════════
// 8. VirtualDevice — IdentifiedDevice trait
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn device_identified_device_trait() {
    let dev = default_device();
    let id: DeviceId = IdentifiedDevice::device_id(&dev);
    assert!(id.device_path.contains("virtual://"));
}

// ═══════════════════════════════════════════════════════════════════════
// 9. VirtualDeviceManager
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn manager_create_device_and_enumerate() {
    let mgr = VirtualDeviceManager::new();
    let _d1 = mgr.create_device(joystick_config("A"));
    let _d2 = mgr.create_device(joystick_config("B"));
    assert_eq!(mgr.devices().len(), 2);
}

#[test]
fn manager_register_unregister() {
    let mut mgr = VirtualDeviceManager::new();
    let dev = Arc::new(VirtualDevice::new(joystick_config("R1")));
    let id = dev.device_id();

    mgr.register_device(dev).unwrap();
    assert_eq!(mgr.enumerate_devices().unwrap().len(), 1);

    mgr.unregister_device(&id).unwrap();
    assert!(mgr.enumerate_devices().unwrap().is_empty());
}

#[test]
fn manager_duplicate_registration_error() {
    let mut mgr = VirtualDeviceManager::new();
    let dev = Arc::new(VirtualDevice::new(joystick_config("DUP")));
    mgr.register_device(dev.clone()).unwrap();
    let err = mgr.register_device(dev).unwrap_err();
    assert!(matches!(err, VirtualDeviceManagerError::DuplicateDevice(_)));
}

#[test]
fn manager_unregister_missing_error() {
    let mut mgr = VirtualDeviceManager::new();
    let fake_id = DeviceId::new(0, 0, None, "fake");
    let err = mgr.unregister_device(&fake_id).unwrap_err();
    assert!(matches!(err, VirtualDeviceManagerError::DeviceNotFound(_)));
}

#[test]
fn manager_health_healthy_device() {
    let mut mgr = VirtualDeviceManager::new();
    let dev = Arc::new(VirtualDevice::new(joystick_config("H1")));
    let id = dev.device_id();
    mgr.register_device(dev).unwrap();

    let health = mgr.get_device_health(&id).unwrap();
    // Randomized health may yield Healthy or Degraded — both are operational.
    assert!(health.is_operational());
}

#[test]
fn manager_health_disconnected_device() {
    let mut mgr = VirtualDeviceManager::new();
    let dev = Arc::new(VirtualDevice::new(joystick_config("DC")));
    let id = dev.device_id();
    dev.disconnect();
    mgr.register_device(dev).unwrap();

    let health = mgr.get_device_health(&id);
    assert!(matches!(health, Some(DeviceHealth::Failed { .. })));
}

#[test]
fn manager_health_unknown_device_returns_none() {
    let mgr = VirtualDeviceManager::new();
    let fake_id = DeviceId::new(0, 0, None, "nope");
    assert!(mgr.get_device_health(&fake_id).is_none());
}

#[test]
fn manager_error_display() {
    let e = VirtualDeviceManagerError::DuplicateDevice(DeviceId::new(1, 2, None, "x"));
    assert!(e.to_string().contains("already registered"));

    let e = VirtualDeviceManagerError::DeviceNotFound(DeviceId::new(1, 2, None, "x"));
    assert!(e.to_string().contains("not found"));
}

#[test]
fn manager_enable_loopback() {
    let mut mgr = VirtualDeviceManager::new();
    let lb = mgr.enable_loopback();
    let report = HidReport::new(0x01, vec![0xAB]);
    assert!(lb.send_input_report(report));
}

// ═══════════════════════════════════════════════════════════════════════
// 10. HatDirection
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn hat_all_8_directions_round_trip() {
    for raw in 0..=7u8 {
        let dir = HatDirection::from_hid(raw);
        assert_eq!(dir.to_hid(), raw, "failed for raw={raw}");
    }
}

#[test]
fn hat_out_of_range_is_centered() {
    for v in [8, 0x0F, 0x80, 0xFE] {
        assert_eq!(HatDirection::from_hid(v), HatDirection::Centered);
    }
}

#[test]
fn hat_centered_hid_value_is_0xff() {
    assert_eq!(HatDirection::Centered.to_hid(), 0xFF);
}

#[test]
fn hat_display_all_labels() {
    let labels = [
        (HatDirection::Centered, "Centered"),
        (HatDirection::North, "N"),
        (HatDirection::NorthEast, "NE"),
        (HatDirection::East, "E"),
        (HatDirection::SouthEast, "SE"),
        (HatDirection::South, "S"),
        (HatDirection::SouthWest, "SW"),
        (HatDirection::West, "W"),
        (HatDirection::NorthWest, "NW"),
    ];
    for (dir, expected) in labels {
        assert_eq!(dir.to_string(), expected);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 11. VirtualBackendError
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn backend_error_display_all_variants() {
    let cases: Vec<(VirtualBackendError, &str)> = vec![
        (VirtualBackendError::NotAcquired(1), "device 1 not acquired"),
        (
            VirtualBackendError::AlreadyAcquired(2),
            "device 2 already acquired",
        ),
        (VirtualBackendError::InvalidAxis(3), "invalid axis id 3"),
        (VirtualBackendError::InvalidButton(4), "invalid button id 4"),
        (VirtualBackendError::InvalidHat(5), "invalid hat id 5"),
        (
            VirtualBackendError::DriverNotAvailable,
            "virtual device driver not available",
        ),
        (
            VirtualBackendError::PlatformError("oops".into()),
            "platform error: oops",
        ),
    ];
    for (err, expected) in cases {
        assert_eq!(err.to_string(), expected);
    }
}

#[test]
fn backend_error_is_std_error() {
    let e: Box<dyn std::error::Error> = Box::new(VirtualBackendError::DriverNotAvailable);
    assert!(!e.to_string().is_empty());
}

// ═══════════════════════════════════════════════════════════════════════
// 12. MockBackend — lifecycle
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mock_double_acquire_fails() {
    let mut m = MockBackend::joystick();
    m.acquire().unwrap();
    assert!(matches!(
        m.acquire(),
        Err(VirtualBackendError::AlreadyAcquired(_))
    ));
}

#[test]
fn mock_double_release_fails() {
    let mut m = MockBackend::joystick();
    assert!(matches!(
        m.release(),
        Err(VirtualBackendError::NotAcquired(_))
    ));
}

#[test]
fn mock_release_resets_all_state() {
    let mut m = acquired_mock();
    m.set_axis(0, 0.9).unwrap();
    m.set_button(1, true).unwrap();
    m.set_hat(0, HatDirection::South).unwrap();

    m.release().unwrap();
    m.acquire().unwrap();

    assert!((m.get_axis(0).unwrap()).abs() < 1e-5);
    assert!(!m.get_button(1).unwrap());
    assert_eq!(m.get_hat(0).unwrap(), HatDirection::Centered);
}

// ═══════════════════════════════════════════════════════════════════════
// 13. MockBackend — axis clamping
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mock_axis_clamps_to_unit_range() {
    let mut m = acquired_mock();
    m.set_axis(0, 99.0).unwrap();
    assert!((m.get_axis(0).unwrap() - 1.0).abs() < 1e-5);

    m.set_axis(0, -99.0).unwrap();
    assert!((m.get_axis(0).unwrap() - (-1.0)).abs() < 1e-5);
}

#[test]
fn mock_axis_nan_clamps_gracefully() {
    let mut m = acquired_mock();
    // f32::NAN.clamp(-1, 1) is NAN on some platforms; we just verify no panic.
    let _ = m.set_axis(0, f32::NAN);
}

// ═══════════════════════════════════════════════════════════════════════
// 14. MockBackend — out-of-range IDs
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mock_invalid_axis_id() {
    let mut m = acquired_mock();
    assert!(matches!(
        m.set_axis(m.axis_count(), 0.0),
        Err(VirtualBackendError::InvalidAxis(_))
    ));
    assert!(matches!(
        m.get_axis(m.axis_count()),
        Err(VirtualBackendError::InvalidAxis(_))
    ));
}

#[test]
fn mock_invalid_button_id() {
    let mut m = acquired_mock();
    assert!(matches!(
        m.set_button(m.button_count(), true),
        Err(VirtualBackendError::InvalidButton(_))
    ));
    assert!(matches!(
        m.get_button(m.button_count()),
        Err(VirtualBackendError::InvalidButton(_))
    ));
}

#[test]
fn mock_invalid_hat_id() {
    let mut m = acquired_mock();
    assert!(matches!(
        m.set_hat(m.hat_count(), HatDirection::North),
        Err(VirtualBackendError::InvalidHat(_))
    ));
    assert!(matches!(
        m.get_hat(m.hat_count()),
        Err(VirtualBackendError::InvalidHat(_))
    ));
}

// ═══════════════════════════════════════════════════════════════════════
// 15. MockBackend — operations require acquisition
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mock_set_axis_without_acquire() {
    let mut m = MockBackend::joystick();
    assert!(matches!(
        m.set_axis(0, 0.0),
        Err(VirtualBackendError::NotAcquired(_))
    ));
}

#[test]
fn mock_set_button_without_acquire() {
    let mut m = MockBackend::joystick();
    assert!(matches!(
        m.set_button(0, true),
        Err(VirtualBackendError::NotAcquired(_))
    ));
}

#[test]
fn mock_set_hat_without_acquire() {
    let mut m = MockBackend::joystick();
    assert!(matches!(
        m.set_hat(0, HatDirection::East),
        Err(VirtualBackendError::NotAcquired(_))
    ));
}

// ═══════════════════════════════════════════════════════════════════════
// 16. MockBackend — capability counts
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mock_custom_capability_counts() {
    let m = MockBackend::new(3, 10, 2);
    assert_eq!(m.axis_count(), 3);
    assert_eq!(m.button_count(), 10);
    assert_eq!(m.hat_count(), 2);
}

#[test]
fn mock_joystick_defaults() {
    let m = MockBackend::joystick();
    assert_eq!(m.axis_count(), 8);
    assert_eq!(m.button_count(), 32);
    assert_eq!(m.hat_count(), 4);
}

// ═══════════════════════════════════════════════════════════════════════
// 17. VJoyDevice — backend trait
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn vjoy_acquire_release_lifecycle() {
    let mut dev = VJoyDevice::new(1);
    assert!(!dev.is_acquired());

    dev.acquire().unwrap();
    assert!(dev.is_acquired());

    dev.release().unwrap();
    assert!(!dev.is_acquired());
}

#[test]
fn vjoy_axis_round_trip_precision() {
    let mut dev = VJoyDevice::new(1);
    dev.acquire().unwrap();

    for val in [-1.0_f32, -0.5, 0.0, 0.5, 1.0] {
        dev.set_axis(0, val).unwrap();
        let got = dev.get_axis(0).unwrap();
        assert!((got - val).abs() < 0.01, "expected ~{val}, got {got}");
    }
}

#[test]
fn vjoy_all_hats_enumerable() {
    let mut dev = VJoyDevice::new(1);
    dev.acquire().unwrap();
    let dirs = [
        HatDirection::North,
        HatDirection::NorthEast,
        HatDirection::East,
        HatDirection::SouthEast,
        HatDirection::South,
        HatDirection::SouthWest,
        HatDirection::West,
        HatDirection::NorthWest,
        HatDirection::Centered,
    ];
    for (i, &dir) in dirs.iter().enumerate() {
        let hat_id = (i % dev.hat_count() as usize) as u8;
        dev.set_hat(hat_id, dir).unwrap();
        assert_eq!(dev.get_hat(hat_id).unwrap(), dir);
    }
}

#[test]
fn vjoy_release_resets_all_state() {
    let mut dev = VJoyDevice::new(2);
    dev.acquire().unwrap();
    dev.set_axis(0, 1.0).unwrap();
    dev.set_button(10, true).unwrap();
    dev.set_hat(0, HatDirection::East).unwrap();

    dev.release().unwrap();
    dev.acquire().unwrap();

    assert!(dev.get_axis(0).unwrap().abs() < 0.01);
    assert!(!dev.get_button(10).unwrap());
    assert_eq!(dev.get_hat(0).unwrap(), HatDirection::Centered);
}

// ═══════════════════════════════════════════════════════════════════════
// 18. UInputDevice — backend trait
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn uinput_custom_caps() {
    let caps = UInputCapabilities {
        num_axes: 4,
        num_buttons: 12,
        num_hats: 1,
        device_name: "Depth Test Stick".into(),
        vendor_id: 0xDEAD,
        product_id: 0xBEEF,
    };
    let mut dev = UInputDevice::new(caps);
    dev.acquire().unwrap();

    assert_eq!(dev.axis_count(), 4);
    assert_eq!(dev.button_count(), 12);
    assert_eq!(dev.hat_count(), 1);

    // Out-of-range for reduced set
    assert!(dev.set_axis(4, 0.0).is_err());
    assert!(dev.set_button(12, true).is_err());
    assert!(dev.set_hat(1, HatDirection::North).is_err());
}

#[test]
fn uinput_axis_round_trip_precision() {
    let mut dev = UInputDevice::new(UInputCapabilities::default());
    dev.acquire().unwrap();

    for val in [-1.0_f32, -0.5, 0.0, 0.5, 1.0] {
        dev.set_axis(0, val).unwrap();
        let got = dev.get_axis(0).unwrap();
        assert!((got - val).abs() < 0.01, "expected ~{val}, got {got}");
    }
}

#[test]
fn uinput_release_resets_state() {
    let mut dev = UInputDevice::new(UInputCapabilities::default());
    dev.acquire().unwrap();
    dev.set_axis(1, 0.8).unwrap();
    dev.set_button(5, true).unwrap();
    dev.set_hat(2, HatDirection::West).unwrap();

    dev.release().unwrap();
    dev.acquire().unwrap();

    assert!(dev.get_axis(1).unwrap().abs() < 0.01);
    assert!(!dev.get_button(5).unwrap());
    assert_eq!(dev.get_hat(2).unwrap(), HatDirection::Centered);
}

// ═══════════════════════════════════════════════════════════════════════
// 19. AxisTransform
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn transform_identity_passthrough() {
    let t = AxisTransform::default();
    for v in [-1.0_f32, -0.5, 0.0, 0.5, 1.0] {
        assert!((t.apply(v) - v).abs() < 1e-5, "v={v}");
    }
}

#[test]
fn transform_deadzone_zeroes_inner() {
    let t = AxisTransform {
        deadzone: 0.15,
        ..Default::default()
    };
    assert!(t.apply(0.10).abs() < 1e-5);
    assert!(t.apply(-0.10).abs() < 1e-5);
}

#[test]
fn transform_deadzone_rescales_outer() {
    let t = AxisTransform {
        deadzone: 0.2,
        ..Default::default()
    };
    // Full deflection should still reach ±1
    assert!((t.apply(1.0) - 1.0).abs() < 0.01);
    assert!((t.apply(-1.0) - (-1.0)).abs() < 0.01);
}

#[test]
fn transform_invert_flips_sign() {
    let t = AxisTransform {
        invert: true,
        ..Default::default()
    };
    assert!((t.apply(0.6) - (-0.6)).abs() < 1e-5);
    assert!((t.apply(-0.6) - 0.6).abs() < 1e-5);
}

#[test]
fn transform_scale_halves_range() {
    let t = AxisTransform {
        scale: 0.5,
        ..Default::default()
    };
    assert!((t.apply(1.0) - 0.5).abs() < 1e-5);
}

#[test]
fn transform_offset_shifts_center() {
    let t = AxisTransform {
        offset: 0.3,
        ..Default::default()
    };
    assert!((t.apply(0.0) - 0.3).abs() < 1e-5);
}

#[test]
fn transform_result_clamps() {
    let t = AxisTransform {
        scale: 3.0,
        ..Default::default()
    };
    assert!((t.apply(1.0) - 1.0).abs() < 1e-5);
    assert!((t.apply(-1.0) - (-1.0)).abs() < 1e-5);
}

#[test]
fn transform_combined_invert_deadzone_scale() {
    let t = AxisTransform {
        invert: true,
        deadzone: 0.1,
        scale: 0.5,
        offset: 0.0,
    };
    // Input 0.05 → inverted = -0.05 → inside deadzone → 0 → *0.5 = 0
    assert!(t.apply(0.05).abs() < 1e-5);
    // Input 1.0 → inverted = -1.0 → outside dz → rescaled to -1.0 → *0.5 = -0.5
    assert!((t.apply(1.0) - (-0.5)).abs() < 0.02);
}

// ═══════════════════════════════════════════════════════════════════════
// 20. Mapper — axis passthrough
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mapper_axis_simple_passthrough() {
    let mut m = mapper_with_mock();
    m.add_axis_mapping(AxisMapping {
        src_axis: 0,
        dst_axis: 0,
        transform: AxisTransform::default(),
    });
    m.update_axes(&[0.42]).unwrap();
    assert!((m.backend().get_axis(0).unwrap() - 0.42).abs() < 1e-5);
}

#[test]
fn mapper_axis_remap_src_to_different_dst() {
    let mut m = mapper_with_mock();
    m.add_axis_mapping(AxisMapping {
        src_axis: 2,
        dst_axis: 5,
        transform: AxisTransform::default(),
    });
    m.update_axes(&[0.0, 0.0, 0.77]).unwrap();
    assert!((m.backend().get_axis(5).unwrap() - 0.77).abs() < 1e-5);
}

// ═══════════════════════════════════════════════════════════════════════
// 21. Mapper — merge strategies
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mapper_merge_first_non_zero_picks_first_active() {
    let mut m = mapper_with_mock();
    m.set_merge_strategy(MergeStrategy::FirstNonZero);
    m.add_axis_mapping(AxisMapping {
        src_axis: 0,
        dst_axis: 0,
        transform: AxisTransform::default(),
    });
    m.add_axis_mapping(AxisMapping {
        src_axis: 1,
        dst_axis: 0,
        transform: AxisTransform::default(),
    });

    m.update_axes(&[0.3, 0.9]).unwrap();
    // First non-zero is 0.3
    assert!((m.backend().get_axis(0).unwrap() - 0.3).abs() < 1e-5);
}

#[test]
fn mapper_merge_sum_clamps_overflow() {
    let mut m = mapper_with_mock();
    m.set_merge_strategy(MergeStrategy::Sum);
    m.add_axis_mapping(AxisMapping {
        src_axis: 0,
        dst_axis: 0,
        transform: AxisTransform::default(),
    });
    m.add_axis_mapping(AxisMapping {
        src_axis: 1,
        dst_axis: 0,
        transform: AxisTransform::default(),
    });

    m.update_axes(&[0.7, 0.8]).unwrap();
    // Sum = 1.5 → clamped to 1.0
    assert!((m.backend().get_axis(0).unwrap() - 1.0).abs() < 1e-5);
}

#[test]
fn mapper_merge_max_abs_picks_largest_magnitude() {
    let mut m = mapper_with_mock();
    m.set_merge_strategy(MergeStrategy::MaxAbs);
    m.add_axis_mapping(AxisMapping {
        src_axis: 0,
        dst_axis: 0,
        transform: AxisTransform::default(),
    });
    m.add_axis_mapping(AxisMapping {
        src_axis: 1,
        dst_axis: 0,
        transform: AxisTransform::default(),
    });

    m.update_axes(&[0.2, -0.9]).unwrap();
    assert!((m.backend().get_axis(0).unwrap() - (-0.9)).abs() < 1e-5);
}

// ═══════════════════════════════════════════════════════════════════════
// 22. Mapper — one-to-many (split)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mapper_split_one_physical_to_two_virtual() {
    let mut m = mapper_with_mock();
    m.add_axis_mapping(AxisMapping {
        src_axis: 0,
        dst_axis: 0,
        transform: AxisTransform::default(),
    });
    m.add_axis_mapping(AxisMapping {
        src_axis: 0,
        dst_axis: 1,
        transform: AxisTransform {
            invert: true,
            ..Default::default()
        },
    });

    m.update_axes(&[0.5]).unwrap();
    assert!((m.backend().get_axis(0).unwrap() - 0.5).abs() < 1e-5);
    assert!((m.backend().get_axis(1).unwrap() - (-0.5)).abs() < 1e-5);
}

// ═══════════════════════════════════════════════════════════════════════
// 23. Mapper — button modes
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mapper_button_direct_mirrors_physical() {
    let mut m = mapper_with_mock();
    m.add_button_mapping(ButtonMapping {
        src_button: 0,
        dst_button: 0,
        mode: ButtonMode::Direct,
    });

    m.update_buttons(&[true]).unwrap();
    assert!(m.backend().get_button(0).unwrap());

    m.update_buttons(&[false]).unwrap();
    assert!(!m.backend().get_button(0).unwrap());
}

#[test]
fn mapper_button_toggle_flips_on_rising_edge() {
    let mut m = mapper_with_mock();
    m.add_button_mapping(ButtonMapping {
        src_button: 0,
        dst_button: 0,
        mode: ButtonMode::Toggle,
    });

    // Press → on
    m.update_buttons(&[true]).unwrap();
    assert!(m.backend().get_button(0).unwrap());

    // Hold → still on (no rising edge)
    m.update_buttons(&[true]).unwrap();
    assert!(m.backend().get_button(0).unwrap());

    // Release → still on
    m.update_buttons(&[false]).unwrap();
    assert!(m.backend().get_button(0).unwrap());

    // Press again → off
    m.update_buttons(&[true]).unwrap();
    assert!(!m.backend().get_button(0).unwrap());
}

#[test]
fn mapper_button_pulse_auto_expires() {
    let mut m = mapper_with_mock();
    m.add_button_mapping(ButtonMapping {
        src_button: 0,
        dst_button: 0,
        mode: ButtonMode::Pulse { ticks: 2 },
    });

    // Rising edge triggers pulse
    m.update_buttons(&[true]).unwrap();
    assert!(m.backend().get_button(0).unwrap());

    // Release — pulse tick 1 (1 remaining)
    m.update_buttons(&[false]).unwrap();
    assert!(m.backend().get_button(0).unwrap());

    // Tick 2 → pulse expires
    m.update_buttons(&[false]).unwrap();
    assert!(!m.backend().get_button(0).unwrap());
}

#[test]
fn mapper_button_remap_src_to_different_dst() {
    let mut m = mapper_with_mock();
    m.add_button_mapping(ButtonMapping {
        src_button: 3,
        dst_button: 7,
        mode: ButtonMode::Direct,
    });

    m.update_buttons(&[false, false, false, true]).unwrap();
    assert!(m.backend().get_button(7).unwrap());
}

// ═══════════════════════════════════════════════════════════════════════
// 24. Mapper — hat passthrough & remap
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mapper_hat_passthrough() {
    let mut m = mapper_with_mock();
    m.add_hat_mapping(HatMapping {
        src_hat: 0,
        dst_hat: 0,
    });
    m.update_hats(&[HatDirection::SouthWest]).unwrap();
    assert_eq!(m.backend().get_hat(0).unwrap(), HatDirection::SouthWest);
}

#[test]
fn mapper_hat_remap_source_to_target() {
    let mut m = mapper_with_mock();
    m.add_hat_mapping(HatMapping {
        src_hat: 1,
        dst_hat: 3,
    });
    m.update_hats(&[HatDirection::Centered, HatDirection::NorthWest])
        .unwrap();
    assert_eq!(m.backend().get_hat(3).unwrap(), HatDirection::NorthWest);
}

// ═══════════════════════════════════════════════════════════════════════
// 25. Mapper — edge cases
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mapper_no_mappings_is_noop() {
    let mut m = mapper_with_mock();
    m.update_axes(&[0.5]).unwrap();
    m.update_buttons(&[true]).unwrap();
    m.update_hats(&[HatDirection::East]).unwrap();

    // Nothing should have changed
    assert!(m.backend().get_axis(0).unwrap().abs() < 1e-5);
    assert!(!m.backend().get_button(0).unwrap());
    assert_eq!(m.backend().get_hat(0).unwrap(), HatDirection::Centered);
}

#[test]
fn mapper_missing_physical_source_defaults_to_zero() {
    let mut m = mapper_with_mock();
    m.add_axis_mapping(AxisMapping {
        src_axis: 10,
        dst_axis: 0,
        transform: AxisTransform::default(),
    });
    // Only 2 physical axes — src 10 is missing → 0.0
    m.update_axes(&[0.5, 0.6]).unwrap();
    assert!(m.backend().get_axis(0).unwrap().abs() < 1e-5);
}

#[test]
fn mapper_missing_physical_button_defaults_to_false() {
    let mut m = mapper_with_mock();
    m.add_button_mapping(ButtonMapping {
        src_button: 10,
        dst_button: 0,
        mode: ButtonMode::Direct,
    });
    m.update_buttons(&[true, false]).unwrap();
    // src 10 missing → false
    assert!(!m.backend().get_button(0).unwrap());
}

#[test]
fn mapper_missing_physical_hat_defaults_to_centered() {
    let mut m = mapper_with_mock();
    m.add_hat_mapping(HatMapping {
        src_hat: 5,
        dst_hat: 0,
    });
    m.update_hats(&[HatDirection::North]).unwrap();
    // src 5 missing → Centered
    assert_eq!(m.backend().get_hat(0).unwrap(), HatDirection::Centered);
}

// ═══════════════════════════════════════════════════════════════════════
// 26. LoopbackHid
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn loopback_send_receive_round_trip() {
    let lb = LoopbackHid::new();
    let report = HidReport::new(0x01, vec![0xDE, 0xAD]);
    assert!(lb.send_input_report(report));

    let rx = lb.receive_input_report().unwrap();
    assert_eq!(rx.report_id, 0x01);
    assert_eq!(rx.data, vec![0xDE, 0xAD]);
}

#[test]
fn loopback_bidirectional() {
    let lb = LoopbackHid::new();
    lb.send_input_report(HidReport::new(0x01, vec![0xAA]));
    lb.send_output_report(HidReport::new(0x02, vec![0xBB]));

    assert_eq!(lb.receive_input_report().unwrap().report_id, 0x01);
    assert_eq!(lb.receive_output_report().unwrap().report_id, 0x02);

    let stats = lb.get_stats();
    assert_eq!(stats.input_reports_sent, 1);
    assert_eq!(stats.output_reports_received, 1);
}

#[test]
fn loopback_overflow_drops_reports() {
    let lb = LoopbackHid::with_config(4, Duration::from_micros(1));
    for i in 0..20u8 {
        lb.send_input_report(HidReport::new(0x01, vec![i]));
    }
    let stats = lb.get_stats();
    assert!(stats.dropped_reports > 0);
}

#[test]
fn loopback_stats_reset() {
    let lb = LoopbackHid::new();
    lb.send_input_report(HidReport::new(0x01, vec![0x00]));
    lb.receive_input_report();
    lb.reset_stats();

    let stats = lb.get_stats();
    assert_eq!(stats.input_reports_sent, 0);
    assert_eq!(stats.bytes_transferred, 0);
    assert_eq!(stats.max_latency_us, 0);
}

#[test]
fn loopback_empty_receive_is_none() {
    let lb = LoopbackHid::new();
    assert!(lb.receive_input_report().is_none());
    assert!(lb.receive_output_report().is_none());
}

// ═══════════════════════════════════════════════════════════════════════
// 27. HidReport
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn hid_report_size() {
    let r = HidReport::new(0x01, vec![0; 10]);
    assert_eq!(r.size(), 11); // 1 byte report_id + 10 data
}

#[test]
fn hid_report_to_bytes() {
    let r = HidReport::new(0x05, vec![0xAA, 0xBB]);
    let bytes = r.to_bytes();
    assert_eq!(bytes, vec![0x05, 0xAA, 0xBB]);
}

#[test]
fn hid_report_empty_data() {
    let r = HidReport::new(0x01, vec![]);
    assert_eq!(r.size(), 1);
    assert_eq!(r.to_bytes(), vec![0x01]);
}
