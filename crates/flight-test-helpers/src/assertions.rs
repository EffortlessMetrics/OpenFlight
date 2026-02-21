// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Shared assertion helpers for integration and acceptance tests.

use flight_adapter_common::AdapterState;
use flight_bus::BusSnapshot;
use flight_device_common::DeviceHealth;

/// Assert an adapter state transition produced the expected state.
pub fn assert_adapter_state_transition(expected: AdapterState, actual: AdapterState) {
    assert_eq!(
        expected, actual,
        "unexpected adapter state transition: expected {expected:?}, got {actual:?}"
    );
}

/// Assert that a bus snapshot passes structural validation.
pub fn assert_snapshot_valid(snapshot: &BusSnapshot) {
    if let Err(err) = snapshot.validate() {
        panic!("bus snapshot validation failed: {err}");
    }
}

/// Assert that a device is still operational (healthy or degraded).
pub fn assert_device_connected(health: &DeviceHealth) {
    assert!(
        health.is_operational(),
        "device is not operational: {:?}",
        health
    );
}
