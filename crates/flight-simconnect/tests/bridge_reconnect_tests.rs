#![cfg(windows)]
// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Tests for SimConnectBridge reconnection, SimVar write, and snapshot APIs.
//!
//! These tests exercise the Reconnecting state in the bridge state machine,
//! the `write_simvar` method, and telemetry snapshot publishing.

use flight_simconnect::{
    BackendError, BridgeConfig, DispatchMessage, MockSimConnectBackend, SimConnectAdapterState,
    SimConnectBridge,
};
use std::time::Duration;

// Well-known define/request IDs (must match bridge constants).
const DEF_TELEMETRY: u32 = 1;
const REQ_TELEMETRY: u32 = 1;
const DEF_AIRCRAFT: u32 = 2;
const REQ_AIRCRAFT: u32 = 2;

fn default_bridge() -> SimConnectBridge<MockSimConnectBackend> {
    SimConnectBridge::new(MockSimConnectBackend::new(), BridgeConfig::default())
}

fn connected_bridge() -> SimConnectBridge<MockSimConnectBackend> {
    let mut b = default_bridge();
    b.connect().unwrap();
    b
}

// ---------------------------------------------------------------------------
// Reconnecting state tests
// ---------------------------------------------------------------------------

/// Connection loss from Connected transitions to Reconnecting.
#[test]
fn bridge_quit_transitions_to_reconnecting() {
    let mut b = connected_bridge();
    b.backend_mut().push_dispatch(DispatchMessage::Quit);
    b.poll().unwrap();
    assert_eq!(b.state(), SimConnectAdapterState::Reconnecting);
}

/// Connection loss clears the latest snapshot.
#[test]
fn bridge_quit_clears_snapshot() {
    let mut b = connected_bridge();
    let n = b.registered_vars().len();
    // Populate a snapshot.
    b.backend_mut()
        .push_dispatch(DispatchMessage::SimObjectData {
            define_id: DEF_TELEMETRY,
            request_id: REQ_TELEMETRY,
            values: vec![1.0; n],
        });
    b.poll().unwrap();
    assert!(b.take_snapshot().is_some());

    // Quit clears it.
    b.backend_mut().push_dispatch(DispatchMessage::Quit);
    b.poll().unwrap();
    assert!(b.take_snapshot().is_none());
}

/// Reconnect from Reconnecting state succeeds.
#[test]
fn bridge_reconnect_from_reconnecting_succeeds() {
    let mut b = connected_bridge();
    b.backend_mut().push_dispatch(DispatchMessage::Quit);
    b.poll().unwrap();
    assert_eq!(b.state(), SimConnectAdapterState::Reconnecting);

    let ok = b.try_reconnect().unwrap();
    assert!(ok);
    assert_eq!(b.state(), SimConnectAdapterState::Connected);
}

/// Multiple reconnection cycles work.
#[test]
fn bridge_multiple_reconnect_cycles() {
    let mut b = connected_bridge();

    for _ in 0..3 {
        // Lose connection.
        b.backend_mut().push_dispatch(DispatchMessage::Quit);
        b.poll().unwrap();
        assert_eq!(b.state(), SimConnectAdapterState::Reconnecting);

        // Reconnect.
        let ok = b.try_reconnect().unwrap();
        assert!(ok);
        assert_eq!(b.state(), SimConnectAdapterState::Connected);
    }
}

/// Exhausted retries with max_retries=2 eventually reach Error.
#[test]
fn bridge_exhausted_retries_reach_error() {
    let config = BridgeConfig {
        max_retries: 2,
        ..Default::default()
    };
    let mut b = SimConnectBridge::new(MockSimConnectBackend::new(), config);

    // First connect fails → Reconnecting (error_count=1 < 2).
    b.backend_mut().fail_next_open = true;
    let _ = b.connect();
    assert_eq!(b.state(), SimConnectAdapterState::Reconnecting);

    // Second connect fails → Error (error_count=2 == max_retries).
    b.backend_mut().fail_next_open = true;
    let _ = b.try_reconnect();
    assert_eq!(b.state(), SimConnectAdapterState::Error);

    // Further reconnect attempts fail.
    let res = b.try_reconnect();
    assert!(res.is_err());
}

/// Backoff delay increases across failures.
#[test]
fn bridge_backoff_increases() {
    let mut b = default_bridge();
    let d1 = b.next_reconnect_delay();
    let d2 = b.next_reconnect_delay();
    assert!(d2 > d1, "backoff must increase: {d1:?} vs {d2:?}");
}

/// Successful connect resets backoff.
#[test]
fn bridge_connect_resets_backoff() {
    let mut b = default_bridge();
    let _ = b.next_reconnect_delay();
    let _ = b.next_reconnect_delay();
    b.connect().unwrap();
    let d = b.next_reconnect_delay();
    assert_eq!(d, Duration::from_secs(1), "backoff must reset");
}

// ---------------------------------------------------------------------------
// SimVar write tests
// ---------------------------------------------------------------------------

/// `write_simvar` sends data through the backend.
#[test]
fn bridge_write_simvar_sends_data() {
    let mut b = connected_bridge();
    b.write_simvar(DEF_TELEMETRY, &[10.0, 20.0]).unwrap();
    let data = b.backend().written_data();
    assert_eq!(data.len(), 1);
    assert_eq!(data[0].0, DEF_TELEMETRY);
    assert_eq!(data[0].1, vec![10.0, 20.0]);
}

/// Multiple writes accumulate in the backend.
#[test]
fn bridge_write_simvar_multiple_writes() {
    let mut b = connected_bridge();
    b.write_simvar(DEF_TELEMETRY, &[1.0]).unwrap();
    b.write_simvar(DEF_AIRCRAFT, &[2.0, 3.0]).unwrap();
    let data = b.backend().written_data();
    assert_eq!(data.len(), 2);
    assert_eq!(data[0].0, DEF_TELEMETRY);
    assert_eq!(data[1].0, DEF_AIRCRAFT);
}

/// `write_simvar` propagates backend errors.
#[test]
fn bridge_write_simvar_propagates_error() {
    let mut b = connected_bridge();
    b.backend_mut().fail_next_write = true;
    let res = b.write_simvar(DEF_TELEMETRY, &[1.0]);
    assert!(matches!(res, Err(BackendError::EventFailed(_))));
}

// ---------------------------------------------------------------------------
// Snapshot / telemetry publishing tests
// ---------------------------------------------------------------------------

/// No snapshot before any telemetry.
#[test]
fn bridge_no_snapshot_before_telemetry() {
    let b = connected_bridge();
    assert!(b.take_snapshot().is_none());
    assert!(!b.has_pending_telemetry());
}

/// Telemetry data produces a snapshot.
#[test]
fn bridge_telemetry_produces_snapshot() {
    let mut b = connected_bridge();
    let n = b.registered_vars().len();
    b.backend_mut()
        .push_dispatch(DispatchMessage::SimObjectData {
            define_id: DEF_TELEMETRY,
            request_id: REQ_TELEMETRY,
            values: vec![42.0; n],
        });
    b.poll().unwrap();
    assert!(b.has_pending_telemetry());
    let snap = b.take_snapshot().unwrap();
    assert_eq!(snap.values.len(), n);
    for val in snap.values.values() {
        assert!((val - 42.0).abs() < f64::EPSILON);
    }
}

/// Successive telemetry updates overwrite the snapshot.
#[test]
fn bridge_telemetry_overwrites_snapshot() {
    let mut b = connected_bridge();
    let n = b.registered_vars().len();

    b.backend_mut()
        .push_dispatch(DispatchMessage::SimObjectData {
            define_id: DEF_TELEMETRY,
            request_id: REQ_TELEMETRY,
            values: vec![1.0; n],
        });
    b.poll().unwrap();

    b.backend_mut()
        .push_dispatch(DispatchMessage::SimObjectData {
            define_id: DEF_TELEMETRY,
            request_id: REQ_TELEMETRY,
            values: vec![99.0; n],
        });
    b.poll().unwrap();

    let snap = b.take_snapshot().unwrap();
    for val in snap.values.values() {
        assert!((val - 99.0).abs() < f64::EPSILON);
    }
}

// ---------------------------------------------------------------------------
// Full lifecycle with reconnection
// ---------------------------------------------------------------------------

/// Connect → receive data → lose connection → reconnect → receive data again.
#[test]
fn bridge_full_lifecycle_with_reconnect() {
    let mut b = default_bridge();
    assert_eq!(b.state(), SimConnectAdapterState::Disconnected);

    // Connect.
    b.connect().unwrap();
    assert_eq!(b.state(), SimConnectAdapterState::Connected);

    // Receive aircraft data → Active.
    b.backend_mut()
        .push_dispatch(DispatchMessage::SimObjectData {
            define_id: DEF_AIRCRAFT,
            request_id: REQ_AIRCRAFT,
            values: vec![1.0],
        });
    b.poll().unwrap();
    assert_eq!(b.state(), SimConnectAdapterState::Active);

    // Receive telemetry.
    let n = b.registered_vars().len();
    b.backend_mut()
        .push_dispatch(DispatchMessage::SimObjectData {
            define_id: DEF_TELEMETRY,
            request_id: REQ_TELEMETRY,
            values: vec![50.0; n],
        });
    b.poll().unwrap();
    assert!(b.take_snapshot().is_some());

    // Write a SimVar.
    b.write_simvar(DEF_TELEMETRY, &[100.0]).unwrap();

    // Lose connection → Reconnecting.
    b.backend_mut().push_dispatch(DispatchMessage::Quit);
    b.poll().unwrap();
    assert_eq!(b.state(), SimConnectAdapterState::Reconnecting);
    assert!(b.take_snapshot().is_none());

    // Reconnect.
    let ok = b.try_reconnect().unwrap();
    assert!(ok);
    assert_eq!(b.state(), SimConnectAdapterState::Connected);

    // Receive new telemetry.
    let n = b.registered_vars().len();
    b.backend_mut()
        .push_dispatch(DispatchMessage::SimObjectData {
            define_id: DEF_TELEMETRY,
            request_id: REQ_TELEMETRY,
            values: vec![75.0; n],
        });
    b.poll().unwrap();
    let snap = b.take_snapshot().unwrap();
    assert_eq!(snap.values.len(), n);

    // Disconnect cleanly.
    b.disconnect().unwrap();
    assert_eq!(b.state(), SimConnectAdapterState::Disconnected);
}
