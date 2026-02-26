// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests: CH Products device identification, presets, and bus pipeline.
//!
//! CH Products devices use the OS HID stack for raw axis/button delivery, so
//! there is no byte-level parser to fuzz here. These tests cover device
//! identification, preset configuration, health monitoring, and the flight-bus
//! round-trip for a snapshot originating from a CH device session.

use flight_bus::{
    BusPublisher, SubscriptionConfig,
    snapshot::BusSnapshot,
    types::{AircraftId, SimId},
};
use flight_hotas_ch::{
    CH_COMBAT_STICK_PID, CH_ECLIPSE_YOKE_PID, CH_FIGHTERSTICK_PID, CH_FLIGHT_YOKE_PID,
    CH_PRO_PEDALS_PID, CH_PRO_THROTTLE_PID, CH_VENDOR_ID, ChModel, ch_model,
    health::{ChHealthMonitor, ChHealthStatus},
    is_ch_device,
    presets::recommended_preset,
};

// ── helpers ───────────────────────────────────────────────────────────────────

/// Publish a snapshot through a fresh bus and return the received snapshot.
fn publish_and_receive(snapshot: BusSnapshot) -> BusSnapshot {
    let mut publisher = BusPublisher::new(60.0);
    let mut subscriber = publisher.subscribe(SubscriptionConfig::default()).unwrap();
    publisher.publish(snapshot).expect("publish must succeed");
    subscriber
        .try_recv()
        .expect("channel must not error")
        .expect("snapshot must be present after publish")
}

// ── CH Products device identification tests ───────────────────────────────────

/// `is_ch_device` returns true for all known CH PIDs.
#[test]
fn ch_products_is_ch_device_returns_true_for_known_pids() {
    let known_pids = [
        CH_PRO_THROTTLE_PID,
        CH_PRO_PEDALS_PID,
        CH_FIGHTERSTICK_PID,
        CH_COMBAT_STICK_PID,
        CH_ECLIPSE_YOKE_PID,
        CH_FLIGHT_YOKE_PID,
    ];
    for pid in known_pids {
        assert!(
            is_ch_device(CH_VENDOR_ID, pid),
            "expected is_ch_device(0x{CH_VENDOR_ID:04X}, 0x{pid:04X}) = true"
        );
    }
}

/// `is_ch_device` returns false for non-CH vendor IDs.
#[test]
fn ch_products_is_ch_device_rejects_unknown_vendor() {
    assert!(!is_ch_device(0x044F, CH_FIGHTERSTICK_PID)); // Thrustmaster VID
    assert!(!is_ch_device(0x3344, CH_FIGHTERSTICK_PID)); // VIRPIL VID
    assert!(!is_ch_device(0x0000, 0x0000));
}

/// `ch_model` returns the expected variant for each CH PID.
#[test]
fn ch_products_ch_model_returns_correct_variant() {
    assert_eq!(ch_model(CH_PRO_THROTTLE_PID), Some(ChModel::ProThrottle));
    assert_eq!(ch_model(CH_PRO_PEDALS_PID), Some(ChModel::ProPedals));
    assert_eq!(ch_model(CH_FIGHTERSTICK_PID), Some(ChModel::Fighterstick));
    assert_eq!(ch_model(CH_COMBAT_STICK_PID), Some(ChModel::CombatStick));
    assert_eq!(ch_model(CH_ECLIPSE_YOKE_PID), Some(ChModel::EclipseYoke));
    assert_eq!(ch_model(CH_FLIGHT_YOKE_PID), Some(ChModel::FlightYoke));
    assert_eq!(ch_model(0xFFFF), None, "unknown PID should return None");
}

/// `recommended_preset` for Fighterstick has reasonable deadzone/expo values.
#[test]
fn ch_products_recommended_preset_fighterstick() {
    let preset = recommended_preset(ChModel::Fighterstick);
    assert_eq!(preset.device, ChModel::Fighterstick);
    assert!(
        (0.0..=0.5).contains(&preset.deadzone),
        "deadzone out of range: {}",
        preset.deadzone
    );
    assert!(
        (0.0..=1.0).contains(&preset.expo),
        "expo out of range: {}",
        preset.expo
    );
}

/// `recommended_preset` for all models returns valid deadzone/expo bounds.
#[test]
fn ch_products_recommended_preset_all_models_valid() {
    let models = [
        ChModel::ProThrottle,
        ChModel::ProPedals,
        ChModel::Fighterstick,
        ChModel::CombatStick,
        ChModel::EclipseYoke,
        ChModel::FlightYoke,
    ];
    for model in models {
        let preset = recommended_preset(model);
        assert!(
            (0.0..=0.5).contains(&preset.deadzone),
            "{:?} deadzone out of range: {}",
            model,
            preset.deadzone
        );
        assert!(
            (0.0..=1.0).contains(&preset.expo),
            "{:?} expo out of range: {}",
            model,
            preset.expo
        );
    }
}

/// `ChHealthMonitor` starts in Unknown state.
#[test]
fn ch_products_health_monitor_initial_state() {
    let monitor = ChHealthMonitor::new(ChModel::Fighterstick);
    assert_eq!(monitor.model(), ChModel::Fighterstick);
    assert_eq!(*monitor.status(), ChHealthStatus::Unknown);
}

/// `ChHealthMonitor` transitions correctly between states.
#[test]
fn ch_products_health_monitor_state_transitions() {
    let mut monitor = ChHealthMonitor::new(ChModel::EclipseYoke);

    monitor.update_status(ChHealthStatus::Connected);
    assert_eq!(*monitor.status(), ChHealthStatus::Connected);

    monitor.update_status(ChHealthStatus::Disconnected);
    assert_eq!(*monitor.status(), ChHealthStatus::Disconnected);

    monitor.update_status(ChHealthStatus::Unknown);
    assert_eq!(*monitor.status(), ChHealthStatus::Unknown);
}

/// Bus round-trip: snapshot from a CH Products session is received correctly.
#[test]
fn ch_products_bus_round_trip() {
    let snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    let received = publish_and_receive(snapshot);

    assert_eq!(received.sim, SimId::Msfs);
    assert_eq!(received.aircraft.icao, "C172");
}
