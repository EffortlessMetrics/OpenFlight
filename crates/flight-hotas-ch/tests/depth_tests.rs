// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Depth tests for the CH Products device support crate (`flight-hotas-ch`).
//!
//! These tests exercise cross-cutting invariants, edge cases in HID report
//! parsing, normalization symmetry, profile-to-parser consistency, and the
//! device identification database. The goal is to catch regressions that unit
//! tests confined to a single module would miss.

use flight_hotas_ch::devices::{
    CH_COMBAT_STICK_PID, CH_ECLIPSE_YOKE_PID, CH_FIGHTERSTICK_PID, CH_FLIGHT_YOKE_PID,
    CH_PRO_PEDALS_PID, CH_PRO_THROTTLE_PID, CH_VENDOR_ID, ChDevice, DEVICE_TABLE, all_devices,
    identify_device,
};
use flight_hotas_ch::health::{ChHealthMonitor, ChHealthStatus};
use flight_hotas_ch::profiles::{AxisNormalization, device_profile, profiled_devices};
use flight_hotas_ch::protocol::{
    AXIS_CENTER, AXIS_MAX, FourWayHat, PovDirection, REPORT_ID, extract_buttons, normalize_bipolar,
    normalize_unipolar, read_axis_u16, validate_report_id,
};
use flight_hotas_ch::{ChError, ChModel, normalize_axis, normalize_pedal, normalize_throttle};
use flight_hotas_ch::{
    parse_combatstick, parse_eclipse_yoke, parse_fighterstick, parse_flight_yoke, parse_pro_pedals,
    parse_pro_throttle,
};

// ═══════════════════════════════════════════════════════════════════════════════
// Report builders
// ═══════════════════════════════════════════════════════════════════════════════

fn fs_report(x: u16, y: u16, z: u16, buttons: u8, extra: u8) -> [u8; 9] {
    let mut r = [0u8; 9];
    r[0] = REPORT_ID;
    r[1..3].copy_from_slice(&x.to_le_bytes());
    r[3..5].copy_from_slice(&y.to_le_bytes());
    r[5..7].copy_from_slice(&z.to_le_bytes());
    r[7] = buttons;
    r[8] = extra;
    r
}

fn pt_report(throttle: u16, a2: u16, a3: u16, buttons: u8, extra: u8) -> [u8; 9] {
    let mut r = [0u8; 9];
    r[0] = REPORT_ID;
    r[1..3].copy_from_slice(&throttle.to_le_bytes());
    r[3..5].copy_from_slice(&a2.to_le_bytes());
    r[5..7].copy_from_slice(&a3.to_le_bytes());
    r[7] = buttons;
    r[8] = extra;
    r
}

fn pp_report(rudder: u16, left: u16, right: u16) -> [u8; 7] {
    let mut r = [0u8; 7];
    r[0] = REPORT_ID;
    r[1..3].copy_from_slice(&rudder.to_le_bytes());
    r[3..5].copy_from_slice(&left.to_le_bytes());
    r[5..7].copy_from_slice(&right.to_le_bytes());
    r
}

fn cs_report(x: u16, y: u16, z: u16, buttons: u8, extra: u8) -> [u8; 9] {
    let mut r = [0u8; 9];
    r[0] = REPORT_ID;
    r[1..3].copy_from_slice(&x.to_le_bytes());
    r[3..5].copy_from_slice(&y.to_le_bytes());
    r[5..7].copy_from_slice(&z.to_le_bytes());
    r[7] = buttons;
    r[8] = extra;
    r
}

fn ey_report(roll: u16, pitch: u16, throttle: u16, btn: [u8; 3], hat_extra: u8) -> [u8; 11] {
    let mut r = [0u8; 11];
    r[0] = REPORT_ID;
    r[1..3].copy_from_slice(&roll.to_le_bytes());
    r[3..5].copy_from_slice(&pitch.to_le_bytes());
    r[5..7].copy_from_slice(&throttle.to_le_bytes());
    r[7] = btn[0];
    r[8] = btn[1];
    r[9] = btn[2];
    r[10] = hat_extra;
    r
}

fn fy_report(roll: u16, pitch: u16, throttle: u16, b0: u8, b1: u8, hat_extra: u8) -> [u8; 10] {
    let mut r = [0u8; 10];
    r[0] = REPORT_ID;
    r[1..3].copy_from_slice(&roll.to_le_bytes());
    r[3..5].copy_from_slice(&pitch.to_le_bytes());
    r[5..7].copy_from_slice(&throttle.to_le_bytes());
    r[7] = b0;
    r[8] = b1;
    r[9] = hat_extra;
    r
}

// ═══════════════════════════════════════════════════════════════════════════════
// 1. Protocol — normalization symmetry and boundary values
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn bipolar_normalization_is_antisymmetric() {
    // Values equidistant from center should produce approximately opposite signs.
    let lo = normalize_bipolar(1000);
    let hi = normalize_bipolar(AXIS_MAX - 1000);
    assert!(
        (lo + hi).abs() < 0.01,
        "bipolar({lo}) + bipolar({hi}) should be near 0"
    );
}

#[test]
fn bipolar_normalization_monotonic_over_sweep() {
    let mut prev = normalize_bipolar(0);
    for raw in (1..=AXIS_MAX).step_by(256) {
        let cur = normalize_bipolar(raw);
        assert!(
            cur >= prev,
            "bipolar not monotonic at raw={raw}: {prev} > {cur}"
        );
        prev = cur;
    }
}

#[test]
fn unipolar_normalization_monotonic_over_sweep() {
    let mut prev = normalize_unipolar(0);
    for raw in (1..=AXIS_MAX).step_by(256) {
        let cur = normalize_unipolar(raw);
        assert!(
            cur >= prev,
            "unipolar not monotonic at raw={raw}: {prev} > {cur}"
        );
        prev = cur;
    }
}

#[test]
fn unipolar_and_bipolar_agree_at_extremes() {
    // At 0: unipolar=0.0, bipolar=-1.0
    assert!((normalize_unipolar(0) - 0.0).abs() < 1e-6);
    assert!((normalize_bipolar(0) + 1.0).abs() < 1e-4);
    // At max: unipolar=1.0, bipolar=1.0
    assert!((normalize_unipolar(AXIS_MAX) - 1.0).abs() < 1e-4);
    assert!((normalize_bipolar(AXIS_MAX) - 1.0).abs() < 1e-4);
}

#[test]
fn bipolar_center_is_near_zero() {
    assert!(normalize_bipolar(AXIS_CENTER).abs() < 0.001);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. Protocol — extract_buttons bit positions
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn extract_buttons_single_bit_per_shift() {
    for shift in 0..24u32 {
        let val = extract_buttons(0x01, shift);
        assert_eq!(val, 1u32 << shift, "shift={shift}");
    }
}

#[test]
fn extract_buttons_full_byte_combines_with_or() {
    let lo = extract_buttons(0xFF, 0);
    let hi = extract_buttons(0xFF, 8);
    assert_eq!(lo | hi, 0xFFFF);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. Protocol — read_axis_u16 endianness and error handling
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn read_axis_u16_little_endian_order() {
    // 0x0100 LE → [0x00, 0x01] → 256
    let data = [0x01, 0x00, 0x01];
    assert_eq!(read_axis_u16(&data, 1).unwrap(), 256);
}

#[test]
fn read_axis_u16_at_offset_zero() {
    let data = [0xCD, 0xAB];
    assert_eq!(read_axis_u16(&data, 0).unwrap(), 0xABCD);
}

#[test]
fn read_axis_u16_boundary_length_is_exact() {
    // Exactly 2 bytes at offset 0 — should succeed
    assert!(read_axis_u16(&[0x00, 0x00], 0).is_ok());
    // 1 byte at offset 0 — should fail
    assert!(read_axis_u16(&[0x00], 0).is_err());
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. Protocol — validate_report_id
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn validate_report_id_accepts_0x01() {
    assert!(validate_report_id(&[0x01]).is_ok());
}

#[test]
fn validate_report_id_rejects_every_other_byte() {
    for id in (0x00u8..=0xFF).filter(|b| *b != REPORT_ID) {
        let err = validate_report_id(&[id]).unwrap_err();
        assert!(
            matches!(err, ChError::InvalidReportId(x) if x == id),
            "expected InvalidReportId({id:#04x})"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. Protocol — PovDirection round-trip
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn pov_direction_round_trips_through_nibble() {
    for nibble in 0u8..=8 {
        let dir = PovDirection::from_nibble(nibble).unwrap();
        assert_eq!(dir as u8, nibble, "repr should match nibble for {dir:?}");
    }
}

#[test]
fn pov_direction_degrees_are_multiples_of_45() {
    for nibble in 1u8..=8 {
        let deg = PovDirection::from_nibble(nibble)
            .unwrap()
            .to_degrees()
            .unwrap();
        assert_eq!(
            deg % 45,
            0,
            "direction {nibble} angle {deg} not multiple of 45"
        );
    }
}

#[test]
fn four_way_hat_is_subset_of_pov() {
    // 4-way hats encode neutral at nibble 0 and N/E/S/W at nibbles 1–4,
    // which are also valid PovDirection values.
    for nibble in 0u8..=4 {
        assert!(FourWayHat::from_nibble(nibble).is_some());
        assert!(PovDirection::from_nibble(nibble).is_some());
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 6. All parsers — reject report ID 0x00
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn all_parsers_reject_report_id_zero() {
    let mut fs = fs_report(0, 0, 0, 0, 0);
    fs[0] = 0x00;
    assert!(matches!(
        parse_fighterstick(&fs),
        Err(ChError::InvalidReportId(0x00))
    ));

    let mut pt = pt_report(0, 0, 0, 0, 0);
    pt[0] = 0x00;
    assert!(matches!(
        parse_pro_throttle(&pt),
        Err(ChError::InvalidReportId(0x00))
    ));

    let mut pp = pp_report(0, 0, 0);
    pp[0] = 0x00;
    assert!(matches!(
        parse_pro_pedals(&pp),
        Err(ChError::InvalidReportId(0x00))
    ));

    let mut cs = cs_report(0, 0, 0, 0, 0);
    cs[0] = 0x00;
    assert!(matches!(
        parse_combatstick(&cs),
        Err(ChError::InvalidReportId(0x00))
    ));

    let mut ey = ey_report(0, 0, 0, [0; 3], 0);
    ey[0] = 0x00;
    assert!(matches!(
        parse_eclipse_yoke(&ey),
        Err(ChError::InvalidReportId(0x00))
    ));

    let mut fy = fy_report(0, 0, 0, 0, 0, 0);
    fy[0] = 0x00;
    assert!(matches!(
        parse_flight_yoke(&fy),
        Err(ChError::InvalidReportId(0x00))
    ));
}

// ═══════════════════════════════════════════════════════════════════════════════
// 7. All parsers — reject empty input
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn all_parsers_reject_empty_input() {
    let empty: &[u8] = &[];
    assert!(parse_fighterstick(empty).is_err());
    assert!(parse_pro_throttle(empty).is_err());
    assert!(parse_pro_pedals(empty).is_err());
    assert!(parse_combatstick(empty).is_err());
    assert!(parse_eclipse_yoke(empty).is_err());
    assert!(parse_flight_yoke(empty).is_err());
}

// ═══════════════════════════════════════════════════════════════════════════════
// 8. All parsers — accept oversized reports without error
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn all_parsers_accept_oversized_reports() {
    let big = {
        let mut v = vec![0u8; 64];
        v[0] = REPORT_ID;
        v
    };
    assert!(parse_fighterstick(&big).is_ok());
    assert!(parse_pro_throttle(&big).is_ok());
    assert!(parse_pro_pedals(&big).is_ok());
    assert!(parse_combatstick(&big).is_ok());
    assert!(parse_eclipse_yoke(&big).is_ok());
    assert!(parse_flight_yoke(&big).is_ok());
}

// ═══════════════════════════════════════════════════════════════════════════════
// 9. Fighterstick — hat and button byte interaction
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn fighterstick_hat_and_buttons_are_independent() {
    // Set hat = West (4), buttons[11:8] = 0b1010
    let r = fs_report(0, 0, 0, 0, 0x4A); // high nibble=4, low nibble=0xA
    let s = parse_fighterstick(&r).unwrap();
    assert_eq!(s.hats[0], 4, "hat should be West");
    assert_eq!(s.buttons >> 8, 0x0A, "upper button bits should be 0xA");
}

#[test]
fn fighterstick_all_hat_directions() {
    for dir in 0u8..=4 {
        let r = fs_report(0, 0, 0, 0, dir << 4);
        let s = parse_fighterstick(&r).unwrap();
        assert_eq!(s.hats[0], dir, "hat direction {dir}");
    }
}

#[test]
fn fighterstick_throttle_always_zero_in_9byte_report() {
    let r = fs_report(u16::MAX, u16::MAX, u16::MAX, 0xFF, 0xFF);
    let s = parse_fighterstick(&r).unwrap();
    assert_eq!(s.throttle, 0, "throttle must be 0 for a 9-byte report");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 10. Pro Throttle — axis isolation
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn pro_throttle_axes_are_independent() {
    let r = pt_report(1000, 2000, 3000, 0, 0);
    let s = parse_pro_throttle(&r).unwrap();
    assert_eq!(s.throttle_main, 1000);
    assert_eq!(s.axis2, 2000);
    assert_eq!(s.axis3, 3000);
    assert_eq!(s.axis4, 0, "axis4 must be 0 for 9-byte reports");
}

#[test]
fn pro_throttle_hat_sweep() {
    for dir in 0u8..=8 {
        let r = pt_report(0, 0, 0, 0, dir << 4);
        let s = parse_pro_throttle(&r).unwrap();
        assert_eq!(s.hat, dir, "hat direction {dir}");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 11. Pro Pedals — symmetric parsing
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn pro_pedals_left_right_symmetry() {
    let r = pp_report(32768, 50000, 50000);
    let s = parse_pro_pedals(&r).unwrap();
    assert_eq!(
        s.left_toe, s.right_toe,
        "symmetric inputs must produce equal values"
    );
}

#[test]
fn pro_pedals_axes_are_independent() {
    let r = pp_report(100, 200, 300);
    let s = parse_pro_pedals(&r).unwrap();
    assert_eq!(s.rudder, 100);
    assert_eq!(s.left_toe, 200);
    assert_eq!(s.right_toe, 300);
}

#[test]
fn pro_pedals_exact_minimum_length_accepted() {
    let r = pp_report(0, 0, 0);
    assert_eq!(r.len(), 7);
    assert!(parse_pro_pedals(&r).is_ok());
}

// ═══════════════════════════════════════════════════════════════════════════════
// 12. Combat Stick — hat sweep and button packing
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn combatstick_hat_sweep_0_to_8() {
    for dir in 0u8..=8 {
        let r = cs_report(0, 0, 0, 0, dir << 4);
        let s = parse_combatstick(&r).unwrap();
        assert_eq!(s.hat, dir);
    }
}

#[test]
fn combatstick_max_buttons() {
    let r = cs_report(0, 0, 0, 0xFF, 0x0F);
    let s = parse_combatstick(&r).unwrap();
    assert_eq!(s.buttons, 0x0FFF);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 13. Eclipse Yoke — wide button bitmask
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn eclipse_yoke_button_isolation_per_byte() {
    // Only byte 7 set
    let r = ey_report(0, 0, 0, [0xFF, 0x00, 0x00], 0x00);
    let s = parse_eclipse_yoke(&r).unwrap();
    assert_eq!(s.buttons, 0x0000_00FF);

    // Only byte 8 set
    let r = ey_report(0, 0, 0, [0x00, 0xFF, 0x00], 0x00);
    let s = parse_eclipse_yoke(&r).unwrap();
    assert_eq!(s.buttons, 0x0000_FF00);

    // Only byte 9 set
    let r = ey_report(0, 0, 0, [0x00, 0x00, 0xFF], 0x00);
    let s = parse_eclipse_yoke(&r).unwrap();
    assert_eq!(s.buttons, 0x00FF_0000);

    // Only low nibble of byte 10 set (buttons[27:24])
    let r = ey_report(0, 0, 0, [0x00, 0x00, 0x00], 0x0F);
    let s = parse_eclipse_yoke(&r).unwrap();
    assert_eq!(s.buttons, 0x0F00_0000);
}

#[test]
fn eclipse_yoke_hat_and_buttons_in_byte_10() {
    // hat = 7 (West), low nibble buttons = 0b1100
    let r = ey_report(0, 0, 0, [0; 3], 0x7C);
    let s = parse_eclipse_yoke(&r).unwrap();
    assert_eq!(s.hat, 7);
    assert_eq!(s.buttons >> 24, 0x0C);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 14. Flight Yoke — 20-bit button range
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn flight_yoke_max_20_button_bits() {
    let r = fy_report(0, 0, 0, 0xFF, 0xFF, 0x0F);
    let s = parse_flight_yoke(&r).unwrap();
    assert_eq!(s.buttons, 0x000F_FFFF, "20 button bits should be set");
}

#[test]
fn flight_yoke_hat_encoding_matches_pov() {
    for dir in 0u8..=8 {
        let r = fy_report(0, 0, 0, 0, 0, dir << 4);
        let s = parse_flight_yoke(&r).unwrap();
        assert_eq!(s.hat, dir);
    }
}

#[test]
fn flight_yoke_axes_at_extremes() {
    let r = fy_report(0, u16::MAX, 32768, 0, 0, 0);
    let s = parse_flight_yoke(&r).unwrap();
    assert_eq!(s.roll, 0);
    assert_eq!(s.pitch, u16::MAX);
    assert_eq!(s.throttle, 32768);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 15. Normalization — per-device normalize functions
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn normalize_axis_symmetry() {
    let lo = normalize_axis(1000);
    let hi = normalize_axis(AXIS_MAX - 1000);
    assert!((lo + hi).abs() < 0.01, "normalize_axis should be symmetric");
}

#[test]
fn normalize_throttle_is_strictly_unipolar() {
    for raw in (0..=AXIS_MAX).step_by(1024) {
        let v = normalize_throttle(raw);
        assert!(
            (0.0..=1.0).contains(&v),
            "throttle({raw}) = {v} out of [0,1]"
        );
    }
}

#[test]
fn normalize_pedal_full_range_sweep() {
    for raw in (0..=AXIS_MAX).step_by(1024) {
        let v = normalize_pedal(raw);
        assert!(
            (-1.0..=1.0).contains(&v),
            "pedal({raw}) = {v} out of [-1,1]"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 16. Device identification — exhaustive PID coverage
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn identify_device_returns_correct_variant_for_every_pid() {
    let expected: &[(u16, ChDevice)] = &[
        (CH_FIGHTERSTICK_PID, ChDevice::Fighterstick),
        (CH_PRO_THROTTLE_PID, ChDevice::ProThrottle),
        (CH_PRO_PEDALS_PID, ChDevice::ProPedals),
        (CH_COMBAT_STICK_PID, ChDevice::CombatStick),
        (CH_ECLIPSE_YOKE_PID, ChDevice::EclipseYoke),
        (CH_FLIGHT_YOKE_PID, ChDevice::FlightYoke),
    ];
    for &(pid, device) in expected {
        assert_eq!(
            identify_device(CH_VENDOR_ID, pid),
            Some(device),
            "PID {pid:#06x} should identify as {device:?}"
        );
    }
}

#[test]
fn identify_device_none_for_neighbouring_pids() {
    use std::collections::HashSet;
    let known: HashSet<u16> = DEVICE_TABLE.iter().map(|e| e.pid).collect();
    for &pid in &known {
        if pid > 0 && !known.contains(&(pid - 1)) {
            assert_eq!(identify_device(CH_VENDOR_ID, pid - 1), None);
        }
        if pid < u16::MAX && !known.contains(&(pid + 1)) {
            assert_eq!(identify_device(CH_VENDOR_ID, pid + 1), None);
        }
    }
}

#[test]
fn device_table_covers_all_enum_variants() {
    let all = [
        ChDevice::Fighterstick,
        ChDevice::ProThrottle,
        ChDevice::ProPedals,
        ChDevice::CombatStick,
        ChDevice::EclipseYoke,
        ChDevice::FlightYoke,
    ];
    for dev in &all {
        assert!(
            DEVICE_TABLE.iter().any(|e| e.device == *dev),
            "{dev:?} missing from DEVICE_TABLE"
        );
    }
}

#[test]
fn all_devices_returns_full_table() {
    assert_eq!(all_devices().len(), DEVICE_TABLE.len());
}

#[test]
fn device_name_and_pid_consistency() {
    for entry in DEVICE_TABLE.iter() {
        assert_eq!(entry.device.pid(), entry.pid);
        assert!(!entry.device.name().is_empty());
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 17. Profile ↔ parser consistency
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn profile_axis_count_matches_parser_fields() {
    // Fighterstick: x, y, z = 3 axes
    let p = device_profile(ChDevice::Fighterstick).unwrap();
    assert_eq!(p.axes.len(), 3, "Fighterstick profile should have 3 axes");

    // Pro Throttle: throttle, mini_stick_x, mini_stick_y, rotary = 4 axes
    let p = device_profile(ChDevice::ProThrottle).unwrap();
    assert_eq!(p.axes.len(), 4, "ProThrottle profile should have 4 axes");

    // Pro Pedals: rudder, left_toe, right_toe = 3 axes
    let p = device_profile(ChDevice::ProPedals).unwrap();
    assert_eq!(p.axes.len(), 3, "ProPedals profile should have 3 axes");
}

#[test]
fn profile_device_name_matches_device_enum_name() {
    for dev in profiled_devices() {
        let profile = device_profile(dev).unwrap();
        assert_eq!(
            profile.name,
            dev.name(),
            "{dev:?} profile name mismatch with device::name()"
        );
    }
}

#[test]
fn profile_deadzone_within_sane_absolute_bound() {
    // Every per-axis deadzone in every profile must be within [0.0, 0.15].
    // The per-device preset deadzone is a global recommendation; individual
    // axes (e.g. the ProThrottle mini-stick) may legitimately need larger values.
    for device in profiled_devices() {
        let profile = device_profile(device).unwrap();
        for axis in &profile.axes {
            assert!(
                axis.deadzone >= 0.0 && axis.deadzone <= 0.15,
                "{device:?}/{}: axis dz {} outside [0.0, 0.15]",
                axis.id,
                axis.deadzone,
            );
        }
    }
}

#[test]
fn every_profiled_device_is_in_device_table() {
    for dev in profiled_devices() {
        assert!(
            DEVICE_TABLE.iter().any(|e| e.device == dev),
            "{dev:?} profiled but not in DEVICE_TABLE"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 18. Profile — axis normalization types
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn stick_axes_use_bipolar_normalization() {
    for dev in [ChDevice::Fighterstick, ChDevice::CombatStick] {
        let p = device_profile(dev).unwrap();
        for ax in &p.axes {
            if ax.id == "x" || ax.id == "y" || ax.id == "z" {
                assert!(
                    matches!(ax.normalization, AxisNormalization::Bipolar { .. }),
                    "{dev:?}/{} should be bipolar",
                    ax.id
                );
            }
        }
    }
}

#[test]
fn throttle_axes_use_unipolar_normalization() {
    for dev in [
        ChDevice::ProThrottle,
        ChDevice::EclipseYoke,
        ChDevice::FlightYoke,
    ] {
        let p = device_profile(dev).unwrap();
        let throttle_ax = p.axes.iter().find(|a| a.id == "throttle").unwrap();
        assert!(
            matches!(
                throttle_ax.normalization,
                AxisNormalization::Unipolar { .. }
            ),
            "{dev:?} throttle should be unipolar"
        );
    }
}

#[test]
fn pedal_rudder_uses_bipolar_normalization() {
    let p = device_profile(ChDevice::ProPedals).unwrap();
    let rudder = p.axes.iter().find(|a| a.id == "rudder").unwrap();
    assert!(matches!(
        rudder.normalization,
        AxisNormalization::Bipolar { .. }
    ));
}

#[test]
fn toe_brakes_use_unipolar_normalization() {
    let p = device_profile(ChDevice::ProPedals).unwrap();
    for id in &["left_toe", "right_toe"] {
        let ax = p.axes.iter().find(|a| a.id == *id).unwrap();
        assert!(
            matches!(ax.normalization, AxisNormalization::Unipolar { .. }),
            "ProPedals/{id} should be unipolar"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 19. Health monitor — full cycle for every model via ChModel
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn health_monitor_unknown_to_connected_to_disconnected_to_connected() {
    let all_models = [
        ChModel::Fighterstick,
        ChModel::CombatStick,
        ChModel::ProThrottle,
        ChModel::ProPedals,
        ChModel::EclipseYoke,
        ChModel::FlightYoke,
    ];
    for model in all_models {
        let mut m = ChHealthMonitor::new(model);
        assert_eq!(m.status(), &ChHealthStatus::Unknown);
        m.update_status(ChHealthStatus::Connected);
        assert_eq!(m.status(), &ChHealthStatus::Connected);
        m.update_status(ChHealthStatus::Disconnected);
        assert_eq!(m.status(), &ChHealthStatus::Disconnected);
        m.update_status(ChHealthStatus::Connected);
        assert_eq!(m.status(), &ChHealthStatus::Connected);
        assert_eq!(m.model(), model);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 20. ChError — Display formatting
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn ch_error_too_short_display() {
    let err = ChError::TooShort { need: 9, got: 4 };
    let msg = err.to_string();
    assert!(msg.contains("9"), "should mention needed size");
    assert!(msg.contains("4"), "should mention actual size");
}

#[test]
fn ch_error_invalid_report_id_display() {
    let err = ChError::InvalidReportId(0xAB);
    let msg = err.to_string();
    assert!(
        msg.contains("0xab") || msg.contains("0xAB"),
        "should show hex ID"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 21. Cross-parser — identical raw bytes produce matching axes
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn fighterstick_and_combatstick_parse_same_axes_from_identical_bytes() {
    let raw = fs_report(12345, 54321, 33333, 0, 0);
    let fs = parse_fighterstick(&raw).unwrap();
    let cs = parse_combatstick(&raw).unwrap();
    assert_eq!(fs.x, cs.x);
    assert_eq!(fs.y, cs.y);
    assert_eq!(fs.z, cs.z);
}
