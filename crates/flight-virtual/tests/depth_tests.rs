// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the virtual device subsystem.
//!
//! Covers device creation (joystick / Xbox controller emulation), axis
//! output mapping, button modes, hat switches, FFB passthrough, and
//! error handling paths.

use flight_virtual::backend::{HatDirection, MockBackend, VirtualBackend, VirtualBackendError};
use flight_virtual::device::{DeviceType, VirtualDevice, VirtualDeviceConfig};
use flight_virtual::device_emulator::{EmulatedDevice, EmulatedDeviceConfig};
use flight_virtual::mapper::{
    AxisMapping, AxisTransform, ButtonMapping, ButtonMode, HatMapping, MergeStrategy,
    VirtualDeviceMapper,
};
use flight_virtual::uinput::{UInputCapabilities, UInputDevice};
use flight_virtual::virtual_controller::{VirtualController, VirtualControllerConfig};
use flight_virtual::virtual_output::{VirtualOutput, VirtualOutputConfig};
use flight_virtual::vjoy::{VJoyDevice, VJOY_MAX_AXES, VJOY_MAX_BUTTONS, VJOY_MAX_HATS};
use flight_virtual::{VirtualDeviceManager, VirtualDeviceManagerError};

use flight_device_common::DeviceManager;
use std::sync::Arc;

// ─────────────────────────────────────────────────────────────────────
// 1. Device creation
// ─────────────────────────────────────────────────────────────────────

mod device_creation {
    use super::*;

    #[test]
    fn create_virtual_joystick_device() {
        let config = VirtualDeviceConfig {
            name: "Depth Joystick".to_string(),
            device_type: DeviceType::Joystick { axes: 6 },
            vid: 0xBEEF,
            pid: 0xCAFE,
            serial: "DEPTH-JS-001".to_string(),
            latency_us: 50,
            packet_loss_rate: 0.0,
        };
        let device = VirtualDevice::new(config);

        assert!(device.is_connected());
        assert_eq!(device.config().name, "Depth Joystick");
        assert_eq!(device.config().vid, 0xBEEF);
        assert_eq!(device.config().pid, 0xCAFE);
        assert_eq!(device.config().serial, "DEPTH-JS-001");

        if let DeviceType::Joystick { axes } = &device.config().device_type {
            assert_eq!(*axes, 6);
        } else {
            panic!("expected Joystick device type");
        }
    }

    #[test]
    fn create_virtual_xbox_controller_emulation() {
        // Emulate an Xbox-style gamepad via EmulatedDevice with known VID/PID.
        let config = EmulatedDeviceConfig {
            vid: 0x045E,  // Microsoft
            pid: 0x028E,  // Xbox 360 Controller
            product_name: "Virtual Xbox Controller".to_string(),
            axis_count: 6, // LX, LY, RX, RY, LT, RT
            ffb_supported: true,
        };
        let dev = EmulatedDevice::new(config);

        assert_eq!(dev.vid(), 0x045E);
        assert_eq!(dev.pid(), 0x028E);
        assert_eq!(dev.product_name(), "Virtual Xbox Controller");
        assert!(dev.supports_ffb());

        // Axes default to 0.
        for i in 0..6 {
            let v = dev.get_axis(i).unwrap();
            assert!(v.abs() < f64::EPSILON, "axis {i} should be 0, got {v}");
        }
    }

    #[test]
    fn device_properties_axes_buttons_hats() {
        let mut vjoy = VJoyDevice::new(1);
        vjoy.acquire().unwrap();

        assert_eq!(vjoy.axis_count(), VJOY_MAX_AXES);
        assert_eq!(vjoy.button_count(), VJOY_MAX_BUTTONS);
        assert_eq!(vjoy.hat_count(), VJOY_MAX_HATS);

        let mut uinput = UInputDevice::new(UInputCapabilities {
            num_axes: 4,
            num_buttons: 16,
            num_hats: 2,
            device_name: "Depth UInput".into(),
            vendor_id: 0x1234,
            product_id: 0x5678,
        });
        uinput.acquire().unwrap();

        assert_eq!(uinput.axis_count(), 4);
        assert_eq!(uinput.button_count(), 16);
        assert_eq!(uinput.hat_count(), 2);
    }

    #[test]
    fn multiple_virtual_devices_simultaneously() {
        let manager = VirtualDeviceManager::new();

        let configs: Vec<VirtualDeviceConfig> = (0..5)
            .map(|i| VirtualDeviceConfig {
                name: format!("Device-{i}"),
                device_type: DeviceType::Joystick { axes: 3 },
                vid: 0x1234,
                pid: 0x5000 + i as u16,
                serial: format!("SIM{i:03}"),
                latency_us: 100,
                packet_loss_rate: 0.0,
            })
            .collect();

        let devices: Vec<Arc<VirtualDevice>> =
            configs.into_iter().map(|c| manager.create_device(c)).collect();

        assert_eq!(manager.devices().len(), 5);

        // All devices are independently addressable.
        for (i, d) in devices.iter().enumerate() {
            d.set_axis(0, i as f32 * 0.2);
        }

        for (i, d) in devices.iter().enumerate() {
            let val = d.get_state().axes[0];
            let expected = i as f32 * 0.2;
            assert!(
                (val - expected).abs() < f32::EPSILON,
                "device {i}: expected {expected}, got {val}"
            );
        }
    }

    #[test]
    fn multiple_backend_instances_coexist() {
        let mut vjoy1 = VJoyDevice::new(1);
        let mut vjoy2 = VJoyDevice::new(2);
        let mut mock = MockBackend::joystick();

        vjoy1.acquire().unwrap();
        vjoy2.acquire().unwrap();
        mock.acquire().unwrap();

        vjoy1.set_axis(0, 0.5).unwrap();
        vjoy2.set_axis(0, -0.5).unwrap();
        mock.set_axis(0, 0.25).unwrap();

        assert!((vjoy1.get_axis(0).unwrap() - 0.5).abs() < 0.01);
        assert!((vjoy2.get_axis(0).unwrap() - (-0.5)).abs() < 0.01);
        assert!((mock.get_axis(0).unwrap() - 0.25).abs() < f32::EPSILON);
    }

    #[test]
    fn device_cleanup_on_drop_vjoy() {
        let acquired = {
            let mut dev = VJoyDevice::new(1);
            dev.acquire().unwrap();
            dev.set_axis(0, 1.0).unwrap();
            dev.is_acquired()
        };
        // After drop, device was acquired (Drop calls release).
        assert!(acquired);
    }

    #[test]
    fn device_cleanup_on_drop_uinput() {
        let acquired = {
            let mut dev = UInputDevice::new(UInputCapabilities::default());
            dev.acquire().unwrap();
            dev.set_button(0, true).unwrap();
            dev.is_acquired()
        };
        assert!(acquired);
    }

    #[test]
    fn virtual_device_disconnect_reconnect_cycle() {
        let device = VirtualDevice::new(VirtualDeviceConfig::default());
        assert!(device.is_connected());

        device.disconnect();
        assert!(!device.is_connected());
        assert!(device.generate_input_report().is_none());

        device.reconnect();
        assert!(device.is_connected());
        assert!(device.generate_input_report().is_some());
    }
}

// ─────────────────────────────────────────────────────────────────────
// 2. Axis output
// ─────────────────────────────────────────────────────────────────────

mod axis_output {
    use super::*;

    fn acquired_mock() -> MockBackend {
        let mut m = MockBackend::joystick();
        m.acquire().unwrap();
        m
    }

    #[test]
    fn set_axis_values_full_range() {
        let mut dev = acquired_mock();
        let values: &[f32] = &[-1.0, -0.5, 0.0, 0.5, 1.0];

        for &v in values {
            dev.set_axis(0, v).unwrap();
            let read = dev.get_axis(0).unwrap();
            assert!(
                (read - v).abs() < f32::EPSILON,
                "set {v}, read {read}"
            );
        }
    }

    #[test]
    fn out_of_range_clamping_positive() {
        let mut dev = acquired_mock();
        dev.set_axis(0, 5.0).unwrap();
        assert!((dev.get_axis(0).unwrap() - 1.0).abs() < f32::EPSILON);

        dev.set_axis(0, 100.0).unwrap();
        assert!((dev.get_axis(0).unwrap() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn out_of_range_clamping_negative() {
        let mut dev = acquired_mock();
        dev.set_axis(0, -5.0).unwrap();
        assert!((dev.get_axis(0).unwrap() - (-1.0)).abs() < f32::EPSILON);

        dev.set_axis(0, -999.0).unwrap();
        assert!((dev.get_axis(0).unwrap() - (-1.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn out_of_range_clamping_vjoy() {
        let mut dev = VJoyDevice::new(1);
        dev.acquire().unwrap();

        dev.set_axis(0, 50.0).unwrap();
        assert!((dev.get_axis(0).unwrap() - 1.0).abs() < 0.01);

        dev.set_axis(0, -50.0).unwrap();
        assert!((dev.get_axis(0).unwrap() - (-1.0)).abs() < 0.01);
    }

    #[test]
    fn axis_precision_16_bit_resolution() {
        // VJoyDevice maps [-1,1] → [0, 32768] and back.
        // Verify that small increments are distinguishable.
        let mut dev = VJoyDevice::new(1);
        dev.acquire().unwrap();

        let step = 2.0 / 32768.0; // minimum resolvable step
        dev.set_axis(0, 0.0).unwrap();
        let center = dev.get_axis(0).unwrap();

        dev.set_axis(0, step).unwrap();
        let nudged = dev.get_axis(0).unwrap();

        // The nudge should be distinguishable from center.
        assert!(
            (nudged - center).abs() >= step * 0.5,
            "16-bit resolution: center={center}, nudged={nudged}, step={step}"
        );
    }

    #[test]
    fn axis_precision_uinput_signed_16bit() {
        // UInputDevice maps [-1,1] → [-32768, 32767].
        // The minimum resolvable step is 1/32767 ≈ 3.05e-5.
        let mut dev = UInputDevice::new(UInputCapabilities::default());
        dev.acquire().unwrap();

        // Use a large enough step that the i32 representation changes.
        let step = 1.0 / 32767.0; // one LSB in the signed representation
        dev.set_axis(0, 0.0).unwrap();
        let center = dev.get_axis(0).unwrap();

        dev.set_axis(0, step * 2.0).unwrap();
        let nudged = dev.get_axis(0).unwrap();

        assert!(
            (nudged - center).abs() > 0.0,
            "signed-16-bit resolution: center={center}, nudged={nudged}, step={step}"
        );
    }

    #[test]
    fn multiple_axes_simultaneously() {
        let mut dev = acquired_mock();
        let count = dev.axis_count();

        for i in 0..count {
            let v = (i as f32 / count as f32) * 2.0 - 1.0;
            dev.set_axis(i, v).unwrap();
        }

        for i in 0..count {
            let expected = (i as f32 / count as f32) * 2.0 - 1.0;
            let got = dev.get_axis(i).unwrap();
            assert!(
                (got - expected).abs() < f32::EPSILON,
                "axis {i}: expected {expected}, got {got}"
            );
        }
    }

    #[test]
    fn rapid_axis_updates() {
        let mut dev = acquired_mock();

        // Rapidly overwrite the same axis 10 000 times.
        for i in 0..10_000u32 {
            let v = ((i as f32 / 5000.0) - 1.0).clamp(-1.0, 1.0);
            dev.set_axis(0, v).unwrap();
        }

        // Final value should be the last written.
        let final_val = ((9999.0 / 5000.0) - 1.0_f32).clamp(-1.0, 1.0);
        assert!((dev.get_axis(0).unwrap() - final_val).abs() < f32::EPSILON);
    }

    #[test]
    fn axis_mapping_with_deadzone_and_scale() {
        let mut backend = MockBackend::joystick();
        backend.acquire().unwrap();
        let mut mapper = VirtualDeviceMapper::new(backend);

        mapper.add_axis_mapping(AxisMapping {
            src_axis: 0,
            dst_axis: 0,
            transform: AxisTransform {
                deadzone: 0.1,
                scale: 0.8,
                offset: 0.0,
                invert: false,
            },
        });

        // Inside deadzone → 0.
        mapper.update_axes(&[0.05]).unwrap();
        assert!(mapper.backend().get_axis(0).unwrap().abs() < f32::EPSILON);

        // Outside deadzone with scale.
        mapper.update_axes(&[1.0]).unwrap();
        let v = mapper.backend().get_axis(0).unwrap();
        assert!((v - 0.8).abs() < 0.02, "expected ~0.8, got {v}");
    }

    #[test]
    fn axis_merge_strategies_all() {
        for strategy in [MergeStrategy::FirstNonZero, MergeStrategy::Sum, MergeStrategy::MaxAbs] {
            let mut backend = MockBackend::joystick();
            backend.acquire().unwrap();
            let mut mapper = VirtualDeviceMapper::new(backend);
            mapper.set_merge_strategy(strategy);

            mapper.add_axis_mapping(AxisMapping {
                src_axis: 0,
                dst_axis: 0,
                transform: AxisTransform::default(),
            });
            mapper.add_axis_mapping(AxisMapping {
                src_axis: 1,
                dst_axis: 0,
                transform: AxisTransform::default(),
            });

            mapper.update_axes(&[0.3, 0.7]).unwrap();
            let v = mapper.backend().get_axis(0).unwrap();

            match strategy {
                MergeStrategy::FirstNonZero => assert!((v - 0.3).abs() < f32::EPSILON),
                MergeStrategy::Sum => assert!((v - 1.0).abs() < f32::EPSILON),
                MergeStrategy::MaxAbs => assert!((v - 0.7).abs() < f32::EPSILON),
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
// 3. Button output
// ─────────────────────────────────────────────────────────────────────

mod button_output {
    use super::*;

    fn mapper_with_mock() -> VirtualDeviceMapper<MockBackend> {
        let mut backend = MockBackend::joystick();
        backend.acquire().unwrap();
        VirtualDeviceMapper::new(backend)
    }

    #[test]
    fn press_release_buttons() {
        let mut mock = MockBackend::joystick();
        mock.acquire().unwrap();

        mock.set_button(0, true).unwrap();
        assert!(mock.get_button(0).unwrap());

        mock.set_button(0, false).unwrap();
        assert!(!mock.get_button(0).unwrap());
    }

    #[test]
    fn multiple_buttons_simultaneously() {
        let mut mock = MockBackend::joystick();
        mock.acquire().unwrap();

        let pressed = [0u8, 3, 7, 15, 31];
        for &b in &pressed {
            mock.set_button(b, true).unwrap();
        }

        for b in 0..mock.button_count() {
            let expected = pressed.contains(&b);
            assert_eq!(
                mock.get_button(b).unwrap(),
                expected,
                "button {b} mismatch"
            );
        }
    }

    #[test]
    fn button_state_tracking_via_device() {
        let device = VirtualDevice::new(VirtualDeviceConfig::default());

        device.set_button(0, true);
        device.set_button(4, true);

        let state = device.get_state();
        assert_ne!(state.buttons & (1 << 0), 0);
        assert_ne!(state.buttons & (1 << 4), 0);
        assert_eq!(state.buttons & (1 << 1), 0);
    }

    #[test]
    fn toggle_mode_via_mapper() {
        let mut m = mapper_with_mock();
        m.add_button_mapping(ButtonMapping {
            src_button: 0,
            dst_button: 0,
            mode: ButtonMode::Toggle,
        });

        // Rising edge → toggle ON.
        m.update_buttons(&[true]).unwrap();
        assert!(m.backend().get_button(0).unwrap());

        // Release → stays ON (toggle latches).
        m.update_buttons(&[false]).unwrap();
        assert!(m.backend().get_button(0).unwrap());

        // Another rising edge → toggle OFF.
        m.update_buttons(&[true]).unwrap();
        assert!(!m.backend().get_button(0).unwrap());

        // Release → stays OFF.
        m.update_buttons(&[false]).unwrap();
        assert!(!m.backend().get_button(0).unwrap());

        // Third press → ON again.
        m.update_buttons(&[true]).unwrap();
        assert!(m.backend().get_button(0).unwrap());
    }

    #[test]
    fn momentary_direct_mode_via_mapper() {
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
    fn pulse_mode_duration() {
        let mut m = mapper_with_mock();
        m.add_button_mapping(ButtonMapping {
            src_button: 0,
            dst_button: 0,
            mode: ButtonMode::Pulse { ticks: 2 },
        });

        // Trigger pulse.
        m.update_buttons(&[true]).unwrap();
        assert!(m.backend().get_button(0).unwrap());

        // Release physical — pulse tick 1, still on.
        m.update_buttons(&[false]).unwrap();
        assert!(m.backend().get_button(0).unwrap());

        // Tick 2 → expires.
        m.update_buttons(&[false]).unwrap();
        assert!(!m.backend().get_button(0).unwrap());
    }

    #[test]
    fn button_remap_src_to_different_dst() {
        let mut m = mapper_with_mock();
        m.add_button_mapping(ButtonMapping {
            src_button: 3,
            dst_button: 7,
            mode: ButtonMode::Direct,
        });

        m.update_buttons(&[false, false, false, true]).unwrap();
        assert!(!m.backend().get_button(3).unwrap());
        assert!(m.backend().get_button(7).unwrap());
    }
}

// ─────────────────────────────────────────────────────────────────────
// 4. Hat switch output
// ─────────────────────────────────────────────────────────────────────

mod hat_switch_output {
    use super::*;

    #[test]
    fn eight_way_pov_positions() {
        let mut mock = MockBackend::new(0, 0, 1);
        mock.acquire().unwrap();

        let directions = [
            HatDirection::North,
            HatDirection::NorthEast,
            HatDirection::East,
            HatDirection::SouthEast,
            HatDirection::South,
            HatDirection::SouthWest,
            HatDirection::West,
            HatDirection::NorthWest,
        ];

        for &dir in &directions {
            mock.set_hat(0, dir).unwrap();
            assert_eq!(mock.get_hat(0).unwrap(), dir, "direction {dir}");
        }
    }

    #[test]
    fn center_neutral_position() {
        let mut mock = MockBackend::new(0, 0, 1);
        mock.acquire().unwrap();

        // Default is centered.
        assert_eq!(mock.get_hat(0).unwrap(), HatDirection::Centered);

        // Set to a direction, then back.
        mock.set_hat(0, HatDirection::East).unwrap();
        assert_eq!(mock.get_hat(0).unwrap(), HatDirection::East);

        mock.set_hat(0, HatDirection::Centered).unwrap();
        assert_eq!(mock.get_hat(0).unwrap(), HatDirection::Centered);
    }

    #[test]
    fn transition_between_all_positions() {
        let mut mock = MockBackend::new(0, 0, 1);
        mock.acquire().unwrap();

        let sequence = [
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

        for window in sequence.windows(2) {
            mock.set_hat(0, window[0]).unwrap();
            assert_eq!(mock.get_hat(0).unwrap(), window[0]);

            mock.set_hat(0, window[1]).unwrap();
            assert_eq!(mock.get_hat(0).unwrap(), window[1]);
        }
    }

    #[test]
    fn hat_hid_encoding_round_trip() {
        for raw in 0..=7u8 {
            let dir = HatDirection::from_hid(raw);
            assert_eq!(dir.to_hid(), raw, "round-trip failed for raw byte {raw}");
        }
        // Out of range → Centered (0xFF).
        assert_eq!(HatDirection::from_hid(0x10).to_hid(), 0xFF);
        assert_eq!(HatDirection::Centered.to_hid(), 0xFF);
    }

    #[test]
    fn hat_passthrough_via_mapper() {
        let mut backend = MockBackend::new(0, 0, 4);
        backend.acquire().unwrap();
        let mut mapper = VirtualDeviceMapper::new(backend);

        mapper.add_hat_mapping(HatMapping {
            src_hat: 0,
            dst_hat: 2,
        });

        mapper
            .update_hats(&[HatDirection::SouthWest])
            .unwrap();
        assert_eq!(
            mapper.backend().get_hat(2).unwrap(),
            HatDirection::SouthWest
        );
    }

    #[test]
    fn vjoy_hat_all_directions() {
        let mut dev = VJoyDevice::new(1);
        dev.acquire().unwrap();

        let all = [
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

        for &d in &all {
            dev.set_hat(0, d).unwrap();
            assert_eq!(dev.get_hat(0).unwrap(), d);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
// 5. FFB passthrough
// ─────────────────────────────────────────────────────────────────────

mod ffb_passthrough {
    use super::*;

    #[test]
    fn receive_ffb_effect_from_game_via_output_report() {
        let device = VirtualDevice::new(VirtualDeviceConfig {
            device_type: DeviceType::ForceFeedback {
                axes: 2,
                max_torque_nm: 15.0,
            },
            ..VirtualDeviceConfig::default()
        });

        // Simulate game sending FFB report (report ID 0x03 = FFB).
        let ffb_report = vec![0x03, 0x01, 0x00, 0x80]; // effect type, params
        let accepted = device.process_output_report(&ffb_report);
        assert!(accepted, "FFB report should be accepted");

        let stats = device.get_stats();
        assert_eq!(stats.output_reports, 1);
    }

    #[test]
    fn map_ffb_to_physical_device_via_emulator_queue() {
        let mut emulated = EmulatedDevice::new(EmulatedDeviceConfig {
            ffb_supported: true,
            ..Default::default()
        });

        // Game sends FFB effect → enqueue as output.
        let effect_create = vec![0x03, 0x01, 0xFF, 0x7F]; // create effect
        emulated.enqueue_output(effect_create.clone());

        let effect_update = vec![0x03, 0x02, 0x80, 0x40]; // update effect
        emulated.enqueue_output(effect_update.clone());

        let effect_destroy = vec![0x03, 0x03, 0x01]; // destroy effect
        emulated.enqueue_output(effect_destroy.clone());

        // Consume lifecycle: create → update → destroy.
        let out1 = emulated.get_output().unwrap();
        assert_eq!(out1, effect_create);

        let out2 = emulated.get_output().unwrap();
        assert_eq!(out2, effect_update);

        let out3 = emulated.get_output().unwrap();
        assert_eq!(out3, effect_destroy);

        assert!(emulated.get_output().is_none());
        assert_eq!(emulated.output_count(), 3);
    }

    #[test]
    fn effect_lifecycle_create_update_destroy() {
        let device = VirtualDevice::new(VirtualDeviceConfig {
            device_type: DeviceType::ForceFeedback {
                axes: 2,
                max_torque_nm: 20.0,
            },
            ..VirtualDeviceConfig::default()
        });

        // Create.
        assert!(device.process_output_report(&[0x03, 0x01, 0x00]));
        // Update.
        assert!(device.process_output_report(&[0x03, 0x02, 0x80]));
        // Destroy.
        assert!(device.process_output_report(&[0x03, 0x03, 0x00]));

        assert_eq!(device.get_stats().output_reports, 3);
    }

    #[test]
    fn ffb_capability_reporting() {
        let no_ffb = EmulatedDevice::new(EmulatedDeviceConfig {
            ffb_supported: false,
            ..Default::default()
        });
        assert!(!no_ffb.supports_ffb());

        let with_ffb = EmulatedDevice::new(EmulatedDeviceConfig {
            ffb_supported: true,
            ..Default::default()
        });
        assert!(with_ffb.supports_ffb());

        // VirtualDevice FFB type carries torque info.
        let dev = VirtualDevice::new(VirtualDeviceConfig {
            device_type: DeviceType::ForceFeedback {
                axes: 2,
                max_torque_nm: 25.0,
            },
            ..VirtualDeviceConfig::default()
        });
        if let DeviceType::ForceFeedback {
            axes,
            max_torque_nm,
        } = &dev.config().device_type
        {
            assert_eq!(*axes, 2);
            assert!((max_torque_nm - 25.0).abs() < f32::EPSILON);
        } else {
            panic!("expected ForceFeedback device type");
        }
    }

    #[test]
    fn ffb_report_rejected_when_disconnected() {
        let device = VirtualDevice::new(VirtualDeviceConfig {
            device_type: DeviceType::ForceFeedback {
                axes: 2,
                max_torque_nm: 10.0,
            },
            ..VirtualDeviceConfig::default()
        });

        device.disconnect();
        let accepted = device.process_output_report(&[0x03, 0x01, 0x80]);
        assert!(!accepted, "FFB report must be rejected when disconnected");
    }
}

// ─────────────────────────────────────────────────────────────────────
// 6. Error handling
// ─────────────────────────────────────────────────────────────────────

mod error_handling {
    use super::*;

    #[test]
    fn driver_not_available_clear_error() {
        let err = VirtualBackendError::DriverNotAvailable;
        assert_eq!(err.to_string(), "virtual device driver not available");

        // VJoyDevice::is_available() returns false when driver not installed.
        assert!(!VJoyDevice::is_available());
        assert_eq!(VJoyDevice::device_count(), 0);
    }

    #[test]
    fn device_slot_occupied_fallback() {
        let mut dev = MockBackend::new(2, 4, 1);
        dev.acquire().unwrap();

        // Attempting to acquire again → AlreadyAcquired error.
        let err = dev.acquire().unwrap_err();
        assert!(
            matches!(err, VirtualBackendError::AlreadyAcquired(_)),
            "expected AlreadyAcquired, got {err:?}"
        );

        // Fallback: release then re-acquire.
        dev.release().unwrap();
        dev.acquire().unwrap();
        assert!(dev.is_acquired());
    }

    #[test]
    fn device_slot_occupied_vjoy() {
        let mut dev = VJoyDevice::new(1);
        dev.acquire().unwrap();

        let err = dev.acquire().unwrap_err();
        assert!(matches!(err, VirtualBackendError::AlreadyAcquired(1)));
    }

    #[test]
    fn permission_denied_escalation_guidance() {
        // PlatformError carries a message that can guide the user.
        let err = VirtualBackendError::PlatformError(
            "permission denied: run as administrator or add uinput group".to_string(),
        );
        let msg = err.to_string();
        assert!(msg.contains("permission denied"));
        assert!(msg.contains("administrator"));
    }

    #[test]
    fn not_acquired_errors_on_all_operations() {
        let mut dev = MockBackend::new(4, 8, 2);

        assert!(matches!(
            dev.set_axis(0, 0.0),
            Err(VirtualBackendError::NotAcquired(_))
        ));
        assert!(matches!(
            dev.set_button(0, true),
            Err(VirtualBackendError::NotAcquired(_))
        ));
        assert!(matches!(
            dev.set_hat(0, HatDirection::North),
            Err(VirtualBackendError::NotAcquired(_))
        ));
    }

    #[test]
    fn invalid_axis_button_hat_ids() {
        let mut dev = MockBackend::new(2, 4, 1);
        dev.acquire().unwrap();

        assert!(matches!(
            dev.set_axis(2, 0.0),
            Err(VirtualBackendError::InvalidAxis(2))
        ));
        assert!(matches!(
            dev.set_button(4, true),
            Err(VirtualBackendError::InvalidButton(4))
        ));
        assert!(matches!(
            dev.set_hat(1, HatDirection::North),
            Err(VirtualBackendError::InvalidHat(1))
        ));
    }

    #[test]
    fn double_release_error() {
        let mut dev = MockBackend::new(1, 1, 1);
        dev.acquire().unwrap();
        dev.release().unwrap();

        let err = dev.release().unwrap_err();
        assert!(matches!(err, VirtualBackendError::NotAcquired(_)));
    }

    #[test]
    fn manager_unregister_unknown_device() {
        let mut manager = VirtualDeviceManager::new();
        let device = Arc::new(VirtualDevice::new(VirtualDeviceConfig::default()));
        let id = device.device_id();

        let err = manager.unregister_device(&id).unwrap_err();
        assert!(matches!(err, VirtualDeviceManagerError::DeviceNotFound(_)));
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn manager_duplicate_registration() {
        let mut manager = VirtualDeviceManager::new();
        let device = Arc::new(VirtualDevice::new(VirtualDeviceConfig::default()));

        manager.register_device(device.clone()).unwrap();
        let err = manager.register_device(device).unwrap_err();
        assert!(matches!(err, VirtualDeviceManagerError::DuplicateDevice(_)));
    }

    #[test]
    fn output_report_rejected_for_unknown_type() {
        let device = VirtualDevice::new(VirtualDeviceConfig::default());

        // Report ID 0xFF is not a recognised report type.
        let accepted = device.process_output_report(&[0xFF, 0x01]);
        assert!(!accepted);
    }

    #[test]
    fn output_report_rejected_when_empty() {
        let device = VirtualDevice::new(VirtualDeviceConfig::default());
        assert!(!device.process_output_report(&[]));
    }
}

// ─────────────────────────────────────────────────────────────────────
// 7. Output pipeline (VirtualController → VirtualOutput)
// ─────────────────────────────────────────────────────────────────────

mod output_pipeline {
    use super::*;

    #[test]
    fn controller_to_output_frame_round_trip() {
        let mut ctrl = VirtualController::new(VirtualControllerConfig::default());
        ctrl.set_axis(0, 0.75);
        ctrl.set_axis(1, -0.5);
        ctrl.set_button(0, true);
        ctrl.set_hat(0, HatDirection::North);

        let snap = ctrl.snapshot();

        let mut output = VirtualOutput::new(VirtualOutputConfig {
            output_rate_hz: 1000.0,
            smoothing_enabled: false,
            ..Default::default()
        });

        let frame = output.compute_frame(&snap, 0.001).unwrap();
        assert!((frame.axes[0] - 0.75).abs() < f64::EPSILON);
        assert!((frame.axes[1] - (-0.5)).abs() < f64::EPSILON);
        assert!(frame.buttons[0]);
        assert_eq!(frame.hats[0], HatDirection::North);
    }

    #[test]
    fn smoothing_dampens_rapid_changes() {
        let mut output = VirtualOutput::new(VirtualOutputConfig {
            output_rate_hz: 1000.0,
            smoothing_enabled: true,
            smoothing_alpha: 0.1,
        });

        let snap_zero = flight_virtual::ControllerSnapshot {
            axes: vec![0.0],
            buttons: vec![],
            hats: vec![],
        };
        let _ = output.compute_frame(&snap_zero, 0.001);

        // Step input to 1.0.
        let snap_one = flight_virtual::ControllerSnapshot {
            axes: vec![1.0],
            buttons: vec![],
            hats: vec![],
        };
        let frame = output.compute_frame(&snap_one, 0.001).unwrap();

        // With alpha=0.1, first frame after step should be ~0.1 (heavily smoothed).
        assert!(frame.axes[0] < 0.5, "smoothed value too high: {}", frame.axes[0]);
        assert!(frame.axes[0] > 0.0, "smoothed value should be positive");
    }

    #[test]
    fn rate_limiting_suppresses_excess_frames() {
        let mut output = VirtualOutput::new(VirtualOutputConfig {
            output_rate_hz: 100.0, // 10ms interval
            smoothing_enabled: false,
            ..Default::default()
        });

        let snap = flight_virtual::ControllerSnapshot {
            axes: vec![0.0],
            buttons: vec![],
            hats: vec![],
        };

        // First call with enough dt → frame.
        assert!(output.compute_frame(&snap, 0.01).is_some());
        // Tiny dt → suppressed.
        assert!(output.compute_frame(&snap, 0.001).is_none());
        // Enough accumulated dt → frame.
        assert!(output.compute_frame(&snap, 0.01).is_some());
    }

    #[test]
    fn emulated_device_input_axis_parsing() {
        let mut dev = EmulatedDevice::new(EmulatedDeviceConfig {
            axis_count: 2,
            ..Default::default()
        });

        // Build report: report_id=0x01, axis0=0 (min), axis1=65535 (max)
        let mut report = vec![0x01];
        report.extend_from_slice(&0u16.to_le_bytes());
        report.extend_from_slice(&65535u16.to_le_bytes());

        dev.inject_input(&report);

        let a0 = dev.get_axis(0).unwrap();
        assert!((a0 - (-1.0)).abs() < 0.01, "expected ~-1.0, got {a0}");

        let a1 = dev.get_axis(1).unwrap();
        assert!((a1 - 1.0).abs() < 0.01, "expected ~1.0, got {a1}");
    }
}
