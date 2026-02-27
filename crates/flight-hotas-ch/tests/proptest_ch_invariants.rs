// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Additional proptest invariants for the CH Products device support crate.
//!
//! CH Products devices use OS-mediated HID — no raw byte parser is required.
//! These invariants cover the preset and health-monitoring public API.

use flight_hotas_ch::{
    CH_COMBAT_STICK_PID, CH_ECLIPSE_YOKE_PID, CH_FIGHTERSTICK_PID, CH_FLIGHT_YOKE_PID,
    CH_PRO_PEDALS_PID, CH_PRO_THROTTLE_PID, CH_VENDOR_ID, ChHealthMonitor, ChHealthStatus, ChModel,
    ch_model, is_ch_device, recommended_preset,
};
use proptest::prelude::*;

// ── helpers ───────────────────────────────────────────────────────────────────

/// All six CH Products models in a fixed array, used to drive model selection
/// via a proptest index strategy.
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

/// All six known CH Products PIDs.
fn all_pids() -> [u16; 6] {
    [
        CH_FIGHTERSTICK_PID,
        CH_COMBAT_STICK_PID,
        CH_PRO_THROTTLE_PID,
        CH_PRO_PEDALS_PID,
        CH_ECLIPSE_YOKE_PID,
        CH_FLIGHT_YOKE_PID,
    ]
}

// ── proptest invariants ───────────────────────────────────────────────────────

proptest! {
    /// The recommended deadzone for any valid model index is always in [0.0, 0.1].
    ///
    /// This guards against accidental formula changes that push deadzone out of
    /// the safe operating range for CH Products devices.
    #[test]
    fn prop_deadzone_always_in_valid_range(idx in 0usize..6) {
        let preset = recommended_preset(all_models()[idx]);
        prop_assert!(
            preset.deadzone >= 0.0 && preset.deadzone <= 0.1,
            "deadzone {:.4} outside [0.0, 0.1] for {:?}",
            preset.deadzone,
            preset.device,
        );
    }

    /// The recommended expo for any valid model index is always in [0.0, 0.5].
    #[test]
    fn prop_expo_always_in_valid_range(idx in 0usize..6) {
        let preset = recommended_preset(all_models()[idx]);
        prop_assert!(
            preset.expo >= 0.0 && preset.expo <= 0.5,
            "expo {:.4} outside [0.0, 0.5] for {:?}",
            preset.expo,
            preset.device,
        );
    }

    /// `recommended_preset` is deterministic — the same model always produces the
    /// same deadzone and expo values.
    #[test]
    fn prop_preset_is_deterministic(idx in 0usize..6) {
        let model = all_models()[idx];
        let a = recommended_preset(model);
        let b = recommended_preset(model);
        prop_assert_eq!(a.deadzone, b.deadzone, "deadzone not stable for {:?}", model);
        prop_assert_eq!(a.expo, b.expo, "expo not stable for {:?}", model);
        prop_assert_eq!(a.invert_throttle, b.invert_throttle, "invert_throttle not stable for {:?}", model);
    }

    /// `is_ch_device` returns `true` for the CH vendor ID with any known CH PID.
    #[test]
    fn prop_is_ch_device_true_for_known_pids(pid_idx in 0usize..6) {
        let pid = all_pids()[pid_idx];
        prop_assert!(
            is_ch_device(CH_VENDOR_ID, pid),
            "is_ch_device should be true for VID=0x{:04X} PID=0x{:04X}",
            CH_VENDOR_ID,
            pid,
        );
    }

    /// `is_ch_device` always returns `false` when the vendor ID is not CH's.
    #[test]
    fn prop_is_ch_device_false_for_non_ch_vendor(vid in 0u16..u16::MAX, pid in 0u16..u16::MAX) {
        // Filter out the actual CH VID so the invariant is always valid.
        prop_assume!(vid != CH_VENDOR_ID);
        prop_assert!(
            !is_ch_device(vid, pid),
            "is_ch_device should be false for non-CH VID=0x{vid:04X}",
        );
    }

    /// `ch_model` returns `Some` for every known CH PID.
    #[test]
    fn prop_ch_model_some_for_known_pids(pid_idx in 0usize..6) {
        let pid = all_pids()[pid_idx];
        prop_assert!(
            ch_model(pid).is_some(),
            "ch_model should return Some for known PID=0x{pid:04X}",
        );
    }
}

// ── non-proptest invariants (model-coverage sanity) ───────────────────────────

/// `ChHealthMonitor::update_status` is idempotent when called twice with the
/// same status value.
#[test]
fn health_monitor_update_idempotent() {
    for model in all_models() {
        let mut monitor = ChHealthMonitor::new(model);
        monitor.update_status(ChHealthStatus::Connected);
        monitor.update_status(ChHealthStatus::Connected);
        assert_eq!(monitor.status(), &ChHealthStatus::Connected);
    }
}

/// The initial status of a freshly created `ChHealthMonitor` is always `Unknown`,
/// regardless of the device model.
#[test]
fn health_monitor_initial_status_always_unknown() {
    for model in all_models() {
        let monitor = ChHealthMonitor::new(model);
        assert_eq!(
            monitor.status(),
            &ChHealthStatus::Unknown,
            "{model:?} monitor should start Unknown",
        );
    }
}
