// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Integration tests for the CH Products device support crate.
//!
//! These tests exercise the public API: axis presets, ChModel variants,
//! the health monitor state machine, and the raw HID report parsers.

use flight_hotas_ch::{ChAxisPreset, ChHealthMonitor, ChHealthStatus, ChModel, recommended_preset};
use flight_hotas_ch::{
    ChError, normalize_axis, normalize_pedal, normalize_throttle, parse_fighterstick,
    parse_pro_pedals, parse_pro_throttle,
};

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn all_models() -> [ChModel; 6] {
    [
        ChModel::Fighterstick,
        ChModel::CombatStick,
        ChModel::ProThrottle,
        ChModel::ProPedals,
        ChModel::EclipseYoke,
        ChModel::FlightYoke,
    ]
}

// ─── Preset coverage ─────────────────────────────────────────────────────────

#[test]
fn ch_all_models_have_presets() {
    for model in all_models() {
        let preset = recommended_preset(model);
        assert_eq!(
            preset.device, model,
            "{model:?}: preset.device does not match requested model"
        );
    }
}

#[test]
fn ch_axis_preset_deadzone_in_valid_range() {
    for model in all_models() {
        let ChAxisPreset { deadzone, .. } = recommended_preset(model);
        assert!(
            (0.0..=0.5).contains(&deadzone),
            "{model:?}: deadzone {deadzone:.4} not in [0.0, 0.5]"
        );
    }
}

#[test]
fn ch_axis_preset_expo_in_valid_range() {
    for model in all_models() {
        let ChAxisPreset { expo, .. } = recommended_preset(model);
        assert!(
            (0.0..=1.0).contains(&expo),
            "{model:?}: expo {expo:.4} not in [0.0, 1.0]"
        );
    }
}

#[test]
fn ch_axis_preset_deadzone_is_finite() {
    for model in all_models() {
        let ChAxisPreset { deadzone, .. } = recommended_preset(model);
        assert!(deadzone.is_finite(), "{model:?}: deadzone is not finite");
    }
}

#[test]
fn ch_axis_preset_expo_is_finite() {
    for model in all_models() {
        let ChAxisPreset { expo, .. } = recommended_preset(model);
        assert!(expo.is_finite(), "{model:?}: expo is not finite");
    }
}

#[test]
fn ch_yokes_invert_throttle() {
    for model in [ChModel::EclipseYoke, ChModel::FlightYoke] {
        assert!(
            recommended_preset(model).invert_throttle,
            "{model:?} should have invert_throttle = true"
        );
    }
}

#[test]
fn ch_non_yokes_do_not_invert_throttle() {
    for model in [
        ChModel::Fighterstick,
        ChModel::CombatStick,
        ChModel::ProThrottle,
        ChModel::ProPedals,
    ] {
        assert!(
            !recommended_preset(model).invert_throttle,
            "{model:?} should have invert_throttle = false"
        );
    }
}

// ─── Health monitor state machine ────────────────────────────────────────────

#[test]
fn ch_health_monitor_initial_state_is_unknown() {
    for model in all_models() {
        let monitor = ChHealthMonitor::new(model);
        assert_eq!(
            monitor.status(),
            &ChHealthStatus::Unknown,
            "{model:?}: initial status should be Unknown"
        );
    }
}

#[test]
fn ch_health_monitor_transitions_to_connected() {
    let mut monitor = ChHealthMonitor::new(ChModel::Fighterstick);
    monitor.update_status(ChHealthStatus::Connected);
    assert_eq!(monitor.status(), &ChHealthStatus::Connected);
}

#[test]
fn ch_health_monitor_transitions_to_disconnected() {
    let mut monitor = ChHealthMonitor::new(ChModel::ProThrottle);
    monitor.update_status(ChHealthStatus::Connected);
    monitor.update_status(ChHealthStatus::Disconnected);
    assert_eq!(monitor.status(), &ChHealthStatus::Disconnected);
}

#[test]
fn ch_health_monitor_reconnects_after_disconnect() {
    let mut monitor = ChHealthMonitor::new(ChModel::ProPedals);
    monitor.update_status(ChHealthStatus::Disconnected);
    monitor.update_status(ChHealthStatus::Connected);
    assert_eq!(monitor.status(), &ChHealthStatus::Connected);
}

#[test]
fn ch_health_monitor_preserves_model() {
    for model in all_models() {
        let monitor = ChHealthMonitor::new(model);
        assert_eq!(monitor.model(), model, "model should be preserved");
    }
}

#[test]
fn ch_health_monitor_all_models_full_cycle() {
    for model in all_models() {
        let mut monitor = ChHealthMonitor::new(model);
        assert_eq!(monitor.status(), &ChHealthStatus::Unknown);
        monitor.update_status(ChHealthStatus::Connected);
        assert_eq!(monitor.status(), &ChHealthStatus::Connected);
        monitor.update_status(ChHealthStatus::Disconnected);
        assert_eq!(monitor.status(), &ChHealthStatus::Disconnected);
        monitor.update_status(ChHealthStatus::Connected);
        assert_eq!(monitor.status(), &ChHealthStatus::Connected);
    }
}

// ─── HID report parser tests ──────────────────────────────────────────────────

fn make_fighterstick_report(x: u16, y: u16, z: u16, buttons: u8, extra: u8) -> [u8; 9] {
    let mut r = [0u8; 9];
    r[0] = 0x01;
    r[1..3].copy_from_slice(&x.to_le_bytes());
    r[3..5].copy_from_slice(&y.to_le_bytes());
    r[5..7].copy_from_slice(&z.to_le_bytes());
    r[7] = buttons;
    r[8] = extra;
    r
}

fn make_pro_throttle_report(throttle: u16, a2: u16, a3: u16, buttons: u8, extra: u8) -> [u8; 9] {
    let mut r = [0u8; 9];
    r[0] = 0x01;
    r[1..3].copy_from_slice(&throttle.to_le_bytes());
    r[3..5].copy_from_slice(&a2.to_le_bytes());
    r[5..7].copy_from_slice(&a3.to_le_bytes());
    r[7] = buttons;
    r[8] = extra;
    r
}

fn make_pedals_report(rudder: u16, left: u16, right: u16) -> [u8; 7] {
    let mut r = [0u8; 7];
    r[0] = 0x01;
    r[1..3].copy_from_slice(&rudder.to_le_bytes());
    r[3..5].copy_from_slice(&left.to_le_bytes());
    r[5..7].copy_from_slice(&right.to_le_bytes());
    r
}

#[test]
fn test_fighterstick_center_position() {
    let report = make_fighterstick_report(32768, 32768, 32768, 0, 0);
    let state = parse_fighterstick(&report).unwrap();
    assert_eq!(state.x, 32768);
    assert_eq!(state.y, 32768);
    assert_eq!(state.z, 32768);
    assert_eq!(state.buttons, 0);
    assert_eq!(state.hats, [0, 0, 0, 0]);
    assert!(normalize_axis(state.x).abs() < 0.01, "center should normalize near 0.0");
}

#[test]
fn test_fighterstick_full_deflection_x() {
    let report = make_fighterstick_report(65535, 0, 0, 0, 0);
    let state = parse_fighterstick(&report).unwrap();
    assert_eq!(state.x, 65535);
    assert!(
        (normalize_axis(state.x) - 1.0).abs() < 1e-4,
        "full deflection should normalize to 1.0"
    );
}

#[test]
fn test_fighterstick_buttons_all_set() {
    // byte 7 = 0xFF → buttons[7:0]; low nibble of byte 8 = 0xF → buttons[11:8]
    let report = make_fighterstick_report(0, 0, 0, 0xFF, 0x0F);
    let state = parse_fighterstick(&report).unwrap();
    assert_eq!(state.buttons, 0x0FFF, "all available buttons should be set");
    assert_eq!(state.hats[0], 0, "hat should be center when high nibble is 0");
}

#[test]
fn test_fighterstick_hat_north() {
    // high nibble of byte 8 = 1 = North
    let report = make_fighterstick_report(0, 0, 0, 0, 0x10);
    let state = parse_fighterstick(&report).unwrap();
    assert_eq!(state.hats[0], 1, "hat[0] should be 1 (North)");
    assert_eq!(state.buttons, 0, "no buttons should be set");
}

#[test]
fn test_fighterstick_too_short() {
    let err = parse_fighterstick(&[0x01; 8]).unwrap_err();
    assert!(
        matches!(err, ChError::TooShort { need: 9, got: 8 }),
        "expected TooShort error, got {err}"
    );
}

#[test]
fn test_pro_throttle_full_forward() {
    let report = make_pro_throttle_report(65535, 0, 0, 0, 0);
    let state = parse_pro_throttle(&report).unwrap();
    assert_eq!(state.throttle_main, 65535);
    assert!((normalize_throttle(state.throttle_main) - 1.0).abs() < 1e-4);
}

#[test]
fn test_pro_throttle_zero() {
    let report = make_pro_throttle_report(0, 0, 0, 0, 0);
    let state = parse_pro_throttle(&report).unwrap();
    assert_eq!(state.throttle_main, 0);
    assert_eq!(state.axis2, 0);
    assert_eq!(state.axis3, 0);
    assert_eq!(state.buttons, 0);
    assert_eq!(state.hat, 0);
    assert!((normalize_throttle(state.throttle_main) - 0.0).abs() < 1e-4);
}

#[test]
fn test_pro_throttle_buttons() {
    let report = make_pro_throttle_report(0, 0, 0, 0b0000_0101, 0);
    let state = parse_pro_throttle(&report).unwrap();
    assert!(state.buttons & 0b0001 != 0, "button 0 should be set");
    assert!(state.buttons & 0b0100 != 0, "button 2 should be set");
    assert!(state.buttons & 0b0010 == 0, "button 1 should not be set");
}

#[test]
fn test_pro_pedals_full_right() {
    let report = make_pedals_report(65535, 0, 0);
    let state = parse_pro_pedals(&report).unwrap();
    assert_eq!(state.rudder, 65535);
    assert!(
        (normalize_pedal(state.rudder) - 1.0).abs() < 1e-4,
        "full right rudder should normalize to 1.0"
    );
}

#[test]
fn test_pro_pedals_toe_brake() {
    let report = make_pedals_report(0, 65535, 0);
    let state = parse_pro_pedals(&report).unwrap();
    assert_eq!(state.left_toe, 65535);
    assert_eq!(state.right_toe, 0);
    assert!(
        (normalize_pedal(state.left_toe) - 1.0).abs() < 1e-4,
        "fully pressed toe brake should normalize to 1.0"
    );
}

#[test]
fn test_normalize_axis_midpoint() {
    let mid = normalize_axis(32768);
    assert!(mid.abs() < 0.001, "midpoint should normalize near 0.0, got {mid}");
}

#[test]
fn test_normalize_throttle_max() {
    assert!((normalize_throttle(65535) - 1.0).abs() < 1e-4, "max throttle should normalize to 1.0");
    assert!((normalize_throttle(0) - 0.0).abs() < 1e-4, "zero throttle should normalize to 0.0");
}
