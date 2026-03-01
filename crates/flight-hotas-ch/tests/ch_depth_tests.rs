// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Depth tests for CH Products flight peripherals.
//!
//! Covers device identification, axis parsing, button matrix, profile
//! generation, and CH-specific quirks across Fighterstick, Combatstick,
//! Pro Throttle, Pro Pedals, and Throttle Quadrant.

use flight_hotas_ch::devices::{ChDevice, DeviceEntry, DEVICE_TABLE, all_devices, identify_device};
use flight_hotas_ch::profiles::{
    AxisNormalization, DeviceProfile, device_profile, profiled_devices,
};
use flight_hotas_ch::protocol::{
    FourWayHat, PovDirection, extract_buttons, normalize_bipolar, normalize_unipolar,
    read_axis_u16, validate_report_id,
};
use flight_hotas_ch::{
    CH_COMBAT_STICK_PID, CH_ECLIPSE_YOKE_PID, CH_FIGHTERSTICK_PID, CH_FLIGHT_YOKE_PID,
    CH_PRO_PEDALS_PID, CH_PRO_THROTTLE_PID, CH_VENDOR_ID, ChError, ChModel, ch_model,
    is_ch_device,
};
use flight_hotas_ch::{
    COMBATSTICK_MIN_REPORT_BYTES, CombatstickState, FIGHTERSTICK_MIN_REPORT_BYTES,
    FighterstickState, PRO_PEDALS_MIN_REPORT_BYTES, PRO_THROTTLE_MIN_REPORT_BYTES,
    ProPedalsState, ProThrottleState, normalize_axis, normalize_pedal, normalize_throttle,
    parse_combatstick, parse_fighterstick, parse_pro_pedals, parse_pro_throttle,
};
use flight_hotas_ch::{
    ECLIPSE_YOKE_MIN_REPORT_BYTES, EclipseYokeState, FLIGHT_YOKE_MIN_REPORT_BYTES,
    FlightYokeState, parse_eclipse_yoke, parse_flight_yoke,
};

// ─── Report builders ─────────────────────────────────────────────────────────

fn build_fighterstick(x: u16, y: u16, z: u16, buttons: u8, hat_btn: u8) -> [u8; 9] {
    let mut r = [0u8; 9];
    r[0] = 0x01;
    r[1..3].copy_from_slice(&x.to_le_bytes());
    r[3..5].copy_from_slice(&y.to_le_bytes());
    r[5..7].copy_from_slice(&z.to_le_bytes());
    r[7] = buttons;
    r[8] = hat_btn;
    r
}

fn build_combatstick(x: u16, y: u16, z: u16, buttons: u8, hat_btn: u8) -> [u8; 9] {
    build_fighterstick(x, y, z, buttons, hat_btn) // same layout
}

fn build_pro_throttle(throttle: u16, a2: u16, a3: u16, buttons: u8, hat_btn: u8) -> [u8; 9] {
    let mut r = [0u8; 9];
    r[0] = 0x01;
    r[1..3].copy_from_slice(&throttle.to_le_bytes());
    r[3..5].copy_from_slice(&a2.to_le_bytes());
    r[5..7].copy_from_slice(&a3.to_le_bytes());
    r[7] = buttons;
    r[8] = hat_btn;
    r
}

fn build_pro_pedals(rudder: u16, left: u16, right: u16) -> [u8; 7] {
    let mut r = [0u8; 7];
    r[0] = 0x01;
    r[1..3].copy_from_slice(&rudder.to_le_bytes());
    r[3..5].copy_from_slice(&left.to_le_bytes());
    r[5..7].copy_from_slice(&right.to_le_bytes());
    r
}

fn build_eclipse_yoke(
    roll: u16,
    pitch: u16,
    throttle: u16,
    btn: [u8; 3],
    hat_extra: u8,
) -> [u8; 11] {
    let mut r = [0u8; 11];
    r[0] = 0x01;
    r[1..3].copy_from_slice(&roll.to_le_bytes());
    r[3..5].copy_from_slice(&pitch.to_le_bytes());
    r[5..7].copy_from_slice(&throttle.to_le_bytes());
    r[7] = btn[0];
    r[8] = btn[1];
    r[9] = btn[2];
    r[10] = hat_extra;
    r
}

fn build_flight_yoke(
    roll: u16,
    pitch: u16,
    throttle: u16,
    btn0: u8,
    btn1: u8,
    hat_extra: u8,
) -> [u8; 10] {
    let mut r = [0u8; 10];
    r[0] = 0x01;
    r[1..3].copy_from_slice(&roll.to_le_bytes());
    r[3..5].copy_from_slice(&pitch.to_le_bytes());
    r[5..7].copy_from_slice(&throttle.to_le_bytes());
    r[7] = btn0;
    r[8] = btn1;
    r[9] = hat_extra;
    r
}

// ═════════════════════════════════════════════════════════════════════════════
// 1. DEVICE IDENTIFICATION (6 tests)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn id_all_ch_devices_share_vendor_id_0x068e() {
    assert_eq!(CH_VENDOR_ID, 0x068E);
    for entry in DEVICE_TABLE {
        assert!(
            is_ch_device(CH_VENDOR_ID, entry.pid),
            "{:?} not recognized with VID 0x068E + PID 0x{:04X}",
            entry.device,
            entry.pid,
        );
    }
}

#[test]
fn id_each_device_has_unique_pid() {
    let pids: Vec<u16> = DEVICE_TABLE.iter().map(|e| e.pid).collect();
    for (i, p) in pids.iter().enumerate() {
        for (j, q) in pids.iter().enumerate() {
            if i != j {
                assert_ne!(p, q, "PID collision between devices {i} and {j}");
            }
        }
    }
}

#[test]
fn id_model_discrimination_via_pid() {
    let cases: &[(u16, ChDevice)] = &[
        (CH_FIGHTERSTICK_PID, ChDevice::Fighterstick),
        (CH_COMBAT_STICK_PID, ChDevice::CombatStick),
        (CH_PRO_THROTTLE_PID, ChDevice::ProThrottle),
        (CH_PRO_PEDALS_PID, ChDevice::ProPedals),
        (CH_ECLIPSE_YOKE_PID, ChDevice::EclipseYoke),
        (CH_FLIGHT_YOKE_PID, ChDevice::FlightYoke),
    ];
    for &(pid, expected) in cases {
        assert_eq!(
            identify_device(CH_VENDOR_ID, pid),
            Some(expected),
            "PID 0x{pid:04X} should identify as {expected:?}",
        );
    }
}

#[test]
fn id_ch_model_enum_maps_to_correct_pids() {
    let cases: &[(u16, ChModel)] = &[
        (CH_FIGHTERSTICK_PID, ChModel::Fighterstick),
        (CH_COMBAT_STICK_PID, ChModel::CombatStick),
        (CH_PRO_THROTTLE_PID, ChModel::ProThrottle),
        (CH_PRO_PEDALS_PID, ChModel::ProPedals),
        (CH_ECLIPSE_YOKE_PID, ChModel::EclipseYoke),
        (CH_FLIGHT_YOKE_PID, ChModel::FlightYoke),
    ];
    for &(pid, expected_model) in cases {
        assert_eq!(
            ch_model(pid),
            Some(expected_model),
            "ch_model(0x{pid:04X}) mismatch",
        );
    }
}

#[test]
fn id_non_ch_vendor_always_rejected() {
    let bogus_vids: &[u16] = &[0x0000, 0x046D, 0x06A3, 0xFFFF, 0x1234];
    for &vid in bogus_vids {
        for entry in DEVICE_TABLE {
            assert_eq!(
                identify_device(vid, entry.pid),
                None,
                "VID 0x{vid:04X} should not match any CH device",
            );
        }
    }
}

#[test]
fn id_device_name_and_pid_round_trip() {
    for entry in DEVICE_TABLE {
        let name = entry.device.name();
        let pid = entry.device.pid();
        assert!(!name.is_empty(), "empty name for {:?}", entry.device);
        assert_eq!(pid, entry.pid, "pid() mismatch for {:?}", entry.device);
        // Round-trip: identify by pid, then check name
        let dev = identify_device(CH_VENDOR_ID, pid).unwrap();
        assert_eq!(dev.name(), name);
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// 2. AXIS PARSING (8 tests)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn axis_fighterstick_16bit_resolution_full_range() {
    // Test that every axis can capture both 0 and 65535 — full 16-bit range
    let r_min = build_fighterstick(0, 0, 0, 0, 0);
    let r_max = build_fighterstick(65535, 65535, 65535, 0, 0);
    let s_min = parse_fighterstick(&r_min).unwrap();
    let s_max = parse_fighterstick(&r_max).unwrap();
    assert_eq!((s_min.x, s_min.y, s_min.z), (0, 0, 0));
    assert_eq!((s_max.x, s_max.y, s_max.z), (65535, 65535, 65535));
}

#[test]
fn axis_combatstick_center_normalization() {
    let r = build_combatstick(32768, 32768, 32768, 0, 0);
    let s = parse_combatstick(&r).unwrap();
    let nx = normalize_axis(s.x);
    let ny = normalize_axis(s.y);
    let nz = normalize_axis(s.z);
    assert!(nx.abs() < 0.001, "x center: {nx}");
    assert!(ny.abs() < 0.001, "y center: {ny}");
    assert!(nz.abs() < 0.001, "z center: {nz}");
}

#[test]
fn axis_pro_throttle_unipolar_range() {
    for raw in [0u16, 1, 16384, 32768, 49152, 65534, 65535] {
        let n = normalize_throttle(raw);
        assert!(
            (0.0..=1.0).contains(&n),
            "throttle raw={raw} normalized to {n}",
        );
    }
    // Monotonicity
    let mut prev = normalize_throttle(0);
    for raw in (1..=65535u16).step_by(1000) {
        let cur = normalize_throttle(raw);
        assert!(cur >= prev, "throttle not monotonic at raw={raw}");
        prev = cur;
    }
}

#[test]
fn axis_pro_pedals_rudder_bipolar_symmetry() {
    let left_full = normalize_pedal(0);
    let center = normalize_pedal(32768);
    let right_full = normalize_pedal(65535);

    assert!((left_full + 1.0).abs() < 1e-4, "left full: {left_full}");
    assert!(center.abs() < 0.001, "center: {center}");
    assert!((right_full - 1.0).abs() < 1e-4, "right full: {right_full}");

    // Symmetry around center
    let left_quarter = normalize_pedal(16384);
    let right_quarter = normalize_pedal(49152);
    assert!(
        (left_quarter + right_quarter).abs() < 0.01,
        "quarter-deflection asymmetry: L={left_quarter}, R={right_quarter}",
    );
}

#[test]
fn axis_pro_pedals_toe_brakes_independent() {
    let r = build_pro_pedals(32768, 65535, 0);
    let s = parse_pro_pedals(&r).unwrap();
    assert_eq!(s.rudder, 32768, "rudder should be at center");
    assert_eq!(s.left_toe, 65535, "left toe fully pressed");
    assert_eq!(s.right_toe, 0, "right toe released");

    let r2 = build_pro_pedals(32768, 0, 65535);
    let s2 = parse_pro_pedals(&r2).unwrap();
    assert_eq!(s2.left_toe, 0, "left toe released");
    assert_eq!(s2.right_toe, 65535, "right toe fully pressed");
}

#[test]
fn axis_le_byte_order_verified() {
    // Verify little-endian encoding: 0x0102 → [0x02, 0x01]
    let r = build_fighterstick(0x0102, 0x0304, 0x0506, 0, 0);
    assert_eq!(r[1], 0x02, "low byte of x");
    assert_eq!(r[2], 0x01, "high byte of x");
    let s = parse_fighterstick(&r).unwrap();
    assert_eq!(s.x, 0x0102);
    assert_eq!(s.y, 0x0304);
    assert_eq!(s.z, 0x0506);
}

#[test]
fn axis_protocol_read_axis_u16_matches_parser() {
    let report = build_fighterstick(12345, 54321, 33333, 0, 0);
    assert_eq!(read_axis_u16(&report, 1).unwrap(), 12345);
    assert_eq!(read_axis_u16(&report, 3).unwrap(), 54321);
    assert_eq!(read_axis_u16(&report, 5).unwrap(), 33333);
}

#[test]
fn axis_normalize_bipolar_unipolar_consistency() {
    // Both normalizers should agree at extremes
    assert!((normalize_bipolar(0) + 1.0).abs() < 1e-4);
    assert!((normalize_bipolar(65535) - 1.0).abs() < 1e-4);
    assert!((normalize_unipolar(0)).abs() < 1e-4);
    assert!((normalize_unipolar(65535) - 1.0).abs() < 1e-4);
    // bipolar midpoint ≈ 0, unipolar midpoint ≈ 0.5
    assert!(normalize_bipolar(32768).abs() < 0.001);
    assert!((normalize_unipolar(32768) - 0.5).abs() < 0.001);
}

// ═════════════════════════════════════════════════════════════════════════════
// 3. BUTTON MATRIX (8 tests)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn btn_fighterstick_individual_buttons_0_to_11() {
    // Test each button individually in the 12-bit range
    for bit in 0..12u32 {
        let (lo, hi) = if bit < 8 {
            (1u8 << bit, 0u8)
        } else {
            (0u8, 1u8 << (bit - 8))
        };
        let r = build_fighterstick(0, 0, 0, lo, hi);
        let s = parse_fighterstick(&r).unwrap();
        assert_eq!(
            s.buttons,
            1 << bit,
            "expected only button {bit} set, got 0x{:08X}",
            s.buttons,
        );
    }
}

#[test]
fn btn_combatstick_button_zero_and_hat_zero() {
    // When hat is center (0) and button 0 is pressed
    let r = build_combatstick(0, 0, 0, 0x01, 0x00);
    let s = parse_combatstick(&r).unwrap();
    assert_eq!(s.buttons & 1, 1, "button 0 should be set");
    assert_eq!(s.hat, 0, "hat should be center");
}

#[test]
fn btn_pro_throttle_14bit_button_range() {
    // Pro Throttle has buttons in bits [7:0] and [13:8]
    let r = build_pro_throttle(0, 0, 0, 0xFF, 0x0F);
    let s = parse_pro_throttle(&r).unwrap();
    // low 8 bits + high 4 bits in low nibble of byte 8 shifted << 8 = 12 bits
    assert_eq!(s.buttons, 0x0FFF, "all 12 reportable buttons set");
}

#[test]
fn btn_hat_all_8way_positions_combatstick() {
    let expected_dirs = [0, 1, 2, 3, 4, 5, 6, 7, 8]; // center + 8 directions
    for &dir in &expected_dirs {
        let r = build_combatstick(0, 0, 0, 0, dir << 4);
        let s = parse_combatstick(&r).unwrap();
        assert_eq!(s.hat, dir, "hat direction {dir}");
    }
}

#[test]
fn btn_fighterstick_hat_4way_encoding() {
    // Fighterstick hat[0] uses 4-way: 0=C, 1=N, 2=E, 3=S, 4=W
    for &(nibble, expected) in &[
        (0u8, FourWayHat::Center),
        (1, FourWayHat::North),
        (2, FourWayHat::East),
        (3, FourWayHat::South),
        (4, FourWayHat::West),
    ] {
        assert_eq!(
            FourWayHat::from_nibble(nibble),
            Some(expected),
            "4-way nibble {nibble}",
        );
    }
    // Out-of-range nibbles
    for n in 5..=15 {
        assert!(
            FourWayHat::from_nibble(n).is_none(),
            "nibble {n} should be invalid for 4-way hat",
        );
    }
}

#[test]
fn btn_hat_pov_direction_to_degrees_coverage() {
    let dirs = [
        (PovDirection::North, 0u16),
        (PovDirection::NorthEast, 45),
        (PovDirection::East, 90),
        (PovDirection::SouthEast, 135),
        (PovDirection::South, 180),
        (PovDirection::SouthWest, 225),
        (PovDirection::West, 270),
        (PovDirection::NorthWest, 315),
    ];
    for (dir, angle) in dirs {
        assert_eq!(dir.to_degrees(), Some(angle), "{dir:?}");
        assert!(dir.is_active());
    }
    assert_eq!(PovDirection::Center.to_degrees(), None);
    assert!(!PovDirection::Center.is_active());
}

#[test]
fn btn_eclipse_yoke_32_buttons_across_4_bytes() {
    // Eclipse Yoke: buttons span bytes 7,8,9 and low nibble of byte 10
    let r = build_eclipse_yoke(0, 0, 0, [0xFF, 0xFF, 0xFF], 0x0F);
    let s = parse_eclipse_yoke(&r).unwrap();
    assert_eq!(s.buttons, 0x0FFF_FFFF, "28 bits of buttons");
    assert_eq!(s.hat, 0, "hat center when high nibble is 0");

    // Single bit test: button 16 (byte 9, bit 0)
    let r2 = build_eclipse_yoke(0, 0, 0, [0, 0, 0x01], 0);
    let s2 = parse_eclipse_yoke(&r2).unwrap();
    assert_eq!(s2.buttons, 1 << 16);
}

#[test]
fn btn_flight_yoke_20_buttons_across_3_bytes() {
    // Flight Yoke: buttons in bytes 7,8 and low nibble of byte 9
    let r = build_flight_yoke(0, 0, 0, 0xFF, 0xFF, 0x0F);
    let s = parse_flight_yoke(&r).unwrap();
    assert_eq!(s.buttons, 0x000F_FFFF, "20 bits of buttons");
    assert_eq!(s.hat, 0, "hat center");

    // Hat + buttons simultaneously
    let r2 = build_flight_yoke(0, 0, 0, 0x01, 0, 0x31); // hat=3, btn16=1
    let s2 = parse_flight_yoke(&r2).unwrap();
    assert_eq!(s2.hat, 3, "hat east");
    assert_eq!(s2.buttons, 0x0001_0001, "button 0 and button 16 set");
}

// ═════════════════════════════════════════════════════════════════════════════
// 4. PROFILE GENERATION (4 tests)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn profile_fighterstick_default_mappings() {
    let p = device_profile(ChDevice::Fighterstick).unwrap();
    assert_eq!(p.name, "CH Fighterstick");
    assert_eq!(p.axes.len(), 3);
    assert_eq!(p.button_count, 32);
    assert_eq!(p.hat_count, 4);

    let ids: Vec<&str> = p.axes.iter().map(|a| a.id).collect();
    assert!(ids.contains(&"x"), "missing aileron axis");
    assert!(ids.contains(&"y"), "missing elevator axis");
    assert!(ids.contains(&"z"), "missing twist axis");

    // All stick axes should be bipolar
    for ax in &p.axes {
        assert!(
            matches!(ax.normalization, AxisNormalization::Bipolar { .. }),
            "axis {} should be bipolar",
            ax.id,
        );
    }
}

#[test]
fn profile_combined_stick_throttle_pedals_no_id_conflicts() {
    // When using Fighterstick + Pro Throttle + Pro Pedals together,
    // there should be no axis ID collisions across devices.
    let stick = device_profile(ChDevice::Fighterstick).unwrap();
    let throttle = device_profile(ChDevice::ProThrottle).unwrap();
    let pedals = device_profile(ChDevice::ProPedals).unwrap();

    let mut all_ids: Vec<&str> = Vec::new();
    all_ids.extend(stick.axes.iter().map(|a| a.id));
    all_ids.extend(throttle.axes.iter().map(|a| a.id));
    all_ids.extend(pedals.axes.iter().map(|a| a.id));

    let count = all_ids.len();
    all_ids.sort();
    all_ids.dedup();
    assert_eq!(
        all_ids.len(),
        count,
        "axis ID collision in combined stick+throttle+pedals setup",
    );
}

#[test]
fn profile_all_devices_have_valid_deadzones_and_filters() {
    for dev in profiled_devices() {
        let p = device_profile(dev).unwrap();
        for ax in &p.axes {
            assert!(
                ax.deadzone >= 0.0 && ax.deadzone <= 0.20,
                "{:?}/{}: deadzone {} too large",
                dev,
                ax.id,
                ax.deadzone,
            );
            if let Some(alpha) = ax.filter_alpha {
                assert!(
                    alpha > 0.0 && alpha <= 1.0,
                    "{:?}/{}: filter alpha {} invalid",
                    dev,
                    ax.id,
                    alpha,
                );
            }
        }
    }
}

#[test]
fn profile_throttle_axes_are_unipolar_stick_axes_are_bipolar() {
    let throttle = device_profile(ChDevice::ProThrottle).unwrap();
    for ax in &throttle.axes {
        match ax.id {
            "throttle" | "rotary" => {
                assert!(
                    matches!(ax.normalization, AxisNormalization::Unipolar { .. }),
                    "Pro Throttle axis '{}' should be unipolar",
                    ax.id,
                );
            }
            "mini_stick_x" | "mini_stick_y" => {
                assert!(
                    matches!(ax.normalization, AxisNormalization::Bipolar { .. }),
                    "Pro Throttle axis '{}' (mini-stick) should be bipolar",
                    ax.id,
                );
            }
            other => panic!("unexpected axis '{other}' in Pro Throttle profile"),
        }
    }

    let pedals = device_profile(ChDevice::ProPedals).unwrap();
    for ax in &pedals.axes {
        match ax.id {
            "rudder" => assert!(matches!(ax.normalization, AxisNormalization::Bipolar { .. })),
            "left_toe" | "right_toe" => {
                assert!(matches!(ax.normalization, AxisNormalization::Unipolar { .. }));
            }
            other => panic!("unexpected axis '{other}' in Pro Pedals profile"),
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// 5. QUIRKS — CH-specific calibration, low-res axes, timing (4 tests)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn quirk_potentiometer_jitter_suppressed_by_deadzone() {
    // CH Products use potentiometers which have inherent jitter.
    // Verify that the recommended deadzones are at least 1% for every axis.
    for dev in profiled_devices() {
        let p = device_profile(dev).unwrap();
        for ax in &p.axes {
            assert!(
                ax.deadzone >= 0.01,
                "{:?}/{}: deadzone {} is below 1%% — CH potentiometers need at least 1%%",
                dev,
                ax.id,
                ax.deadzone,
            );
        }
    }
}

#[test]
fn quirk_low_resolution_axis_noise_floor() {
    // CH Products older devices have 8-bit ADCs internally even though they
    // report 16-bit values. Test that small differences near center are
    // absorbed by normalization without sign-flip noise.
    let center = 32768u16;
    let jitter_values = [
        center - 256,
        center - 128,
        center - 1,
        center,
        center + 1,
        center + 128,
        center + 256,
    ];
    for &raw in &jitter_values {
        let n = normalize_axis(raw);
        assert!(
            n.abs() < 0.02,
            "raw={raw} normalized to {n}, expected near 0 (jitter zone)",
        );
    }
}

#[test]
fn quirk_report_id_always_0x01() {
    // CH Products devices always use report ID 0x01.
    // Parsers must reject any other report ID.
    for bad_id in [0x00u8, 0x02, 0x03, 0xFF] {
        let mut report = [0u8; 11];
        report[0] = bad_id;
        assert!(parse_fighterstick(&report).is_err());
        assert!(parse_combatstick(&report).is_err());
        assert!(parse_pro_throttle(&report).is_err());
        assert!(parse_eclipse_yoke(&report).is_err());
        assert!(parse_flight_yoke(&report[..10]).is_err());

        let mut short = [0u8; 7];
        short[0] = bad_id;
        assert!(parse_pro_pedals(&short).is_err());
    }
    // validate_report_id rejects wrong IDs
    assert!(validate_report_id(&[0x00]).is_err());
    assert!(validate_report_id(&[0x02]).is_err());
    assert!(validate_report_id(&[0x01]).is_ok());
}

#[test]
fn quirk_minimum_report_lengths_match_documentation() {
    // Verify the min report sizes are consistent with the documented formats.
    assert_eq!(FIGHTERSTICK_MIN_REPORT_BYTES, 9, "Fighterstick: 1+2+2+2+1+1");
    assert_eq!(COMBATSTICK_MIN_REPORT_BYTES, 9, "Combatstick: same as Fighterstick");
    assert_eq!(PRO_THROTTLE_MIN_REPORT_BYTES, 9, "Pro Throttle: 1+2+2+2+1+1");
    assert_eq!(PRO_PEDALS_MIN_REPORT_BYTES, 7, "Pro Pedals: 1+2+2+2 (no buttons)");
    assert_eq!(ECLIPSE_YOKE_MIN_REPORT_BYTES, 11, "Eclipse Yoke: 1+2+2+2+1+1+1+1");
    assert_eq!(FLIGHT_YOKE_MIN_REPORT_BYTES, 10, "Flight Yoke: 1+2+2+2+1+1+1");
}

// ═════════════════════════════════════════════════════════════════════════════
// 6. ADDITIONAL DEPTH — error paths, boundary conditions (6 tests)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn error_too_short_reports_all_parsers() {
    // Each parser must return TooShort for undersized inputs
    assert!(matches!(
        parse_fighterstick(&[0x01; 8]).unwrap_err(),
        ChError::TooShort { need: 9, got: 8 }
    ));
    assert!(matches!(
        parse_combatstick(&[0x01; 8]).unwrap_err(),
        ChError::TooShort { need: 9, got: 8 }
    ));
    assert!(matches!(
        parse_pro_throttle(&[0x01; 8]).unwrap_err(),
        ChError::TooShort { need: 9, got: 8 }
    ));
    assert!(matches!(
        parse_pro_pedals(&[0x01; 6]).unwrap_err(),
        ChError::TooShort { need: 7, got: 6 }
    ));
    assert!(matches!(
        parse_eclipse_yoke(&[0x01; 10]).unwrap_err(),
        ChError::TooShort { need: 11, got: 10 }
    ));
    assert!(matches!(
        parse_flight_yoke(&[0x01; 9]).unwrap_err(),
        ChError::TooShort { need: 10, got: 9 }
    ));
}

#[test]
fn error_empty_reports_all_parsers() {
    assert!(parse_fighterstick(&[]).is_err());
    assert!(parse_combatstick(&[]).is_err());
    assert!(parse_pro_throttle(&[]).is_err());
    assert!(parse_pro_pedals(&[]).is_err());
    assert!(parse_eclipse_yoke(&[]).is_err());
    assert!(parse_flight_yoke(&[]).is_err());
}

#[test]
fn boundary_oversized_report_accepted() {
    // Reports larger than minimum should parse fine (extra bytes ignored)
    let mut big = [0u8; 64];
    big[0] = 0x01;
    big[1] = 0xFF;
    big[2] = 0xFF;
    assert!(parse_fighterstick(&big).is_ok());
    assert!(parse_combatstick(&big).is_ok());
    assert!(parse_pro_throttle(&big).is_ok());
    assert!(parse_pro_pedals(&big).is_ok());
    assert!(parse_eclipse_yoke(&big).is_ok());
    assert!(parse_flight_yoke(&big).is_ok());

    let s = parse_fighterstick(&big).unwrap();
    assert_eq!(s.x, 65535, "x should read from bytes 1-2 regardless of size");
}

#[test]
fn boundary_hat_and_buttons_simultaneous() {
    // Hat direction and button bits share the same byte; verify no cross-talk
    // Fighterstick byte 8: high nibble = hat, low nibble = buttons[11:8]
    let r = build_fighterstick(0, 0, 0, 0, 0x4F); // hat=4(West), buttons[11:8]=0xF
    let s = parse_fighterstick(&r).unwrap();
    assert_eq!(s.hats[0], 4, "hat should be West");
    assert_eq!(s.buttons, 0x0F00, "only high button bits set");
}

#[test]
fn boundary_extract_buttons_helper() {
    assert_eq!(extract_buttons(0b1010_0101, 0), 0b1010_0101);
    assert_eq!(extract_buttons(0b1010_0101, 8), 0b1010_0101_0000_0000);
    assert_eq!(extract_buttons(0xFF, 24), 0xFF00_0000);
    assert_eq!(extract_buttons(0, 0), 0);
}

#[test]
fn boundary_all_axes_at_midpoint_produce_near_zero() {
    // All bipolar normalization at midpoint should be near zero
    let fs = parse_fighterstick(&build_fighterstick(32768, 32768, 32768, 0, 0)).unwrap();
    assert!(normalize_axis(fs.x).abs() < 0.001);
    assert!(normalize_axis(fs.y).abs() < 0.001);
    assert!(normalize_axis(fs.z).abs() < 0.001);

    let cs = parse_combatstick(&build_combatstick(32768, 32768, 32768, 0, 0)).unwrap();
    assert!(normalize_axis(cs.x).abs() < 0.001);
    assert!(normalize_axis(cs.y).abs() < 0.001);
    assert!(normalize_axis(cs.z).abs() < 0.001);

    let pp = parse_pro_pedals(&build_pro_pedals(32768, 32768, 32768)).unwrap();
    assert!(normalize_pedal(pp.rudder).abs() < 0.001);
}
