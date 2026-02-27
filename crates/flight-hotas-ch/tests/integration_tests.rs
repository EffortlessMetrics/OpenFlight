// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Integration tests for the CH Products device support crate.
//!
//! These tests exercise the public API: axis presets, ChModel variants,
//! and the health monitor state machine.

use flight_hotas_ch::{ChAxisPreset, ChHealthMonitor, ChHealthStatus, ChModel, recommended_preset};

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
