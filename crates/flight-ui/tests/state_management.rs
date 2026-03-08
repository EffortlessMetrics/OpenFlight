// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for UI state management: state transitions, profile switching,
//! health state changes, and concurrent access.

use std::sync::Arc;

use chrono::Utc;
use flight_ui::dashboard::{
    AdapterStatus, DashboardState, DeviceStatus, HealthStatus, WsMessage,
};
use flight_ui::websocket::WsBroadcast;
use tokio::sync::RwLock;

// ---------------------------------------------------------------------------
// App state transitions
// ---------------------------------------------------------------------------

#[test]
fn health_status_transitions_ok_to_degraded() {
    let mut state = DashboardState::new();
    assert_eq!(state.health, HealthStatus::Ok);
    state.health = HealthStatus::Degraded;
    assert_eq!(state.health, HealthStatus::Degraded);
}

#[test]
fn health_status_transitions_degraded_to_unavailable() {
    let mut state = DashboardState::new();
    state.health = HealthStatus::Degraded;
    state.health = HealthStatus::Unavailable;
    assert_eq!(state.health, HealthStatus::Unavailable);
}

#[test]
fn health_status_recovery_unavailable_to_ok() {
    let mut state = DashboardState::new();
    state.health = HealthStatus::Unavailable;
    state.health = HealthStatus::Ok;
    assert_eq!(state.health, HealthStatus::Ok);
}

#[test]
fn uptime_increments_preserved() {
    let mut state = DashboardState::new();
    assert_eq!(state.uptime_secs, 0);
    state.uptime_secs = 86400; // 24 hours
    let json = serde_json::to_string(&state).unwrap();
    let restored: DashboardState = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.uptime_secs, 86400);
}

// ---------------------------------------------------------------------------
// Profile switching (state mutation)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn profile_switch_via_shared_state() {
    let state = Arc::new(RwLock::new(DashboardState::new()));
    assert_eq!(state.read().await.profile, "default");

    {
        let mut w = state.write().await;
        w.profile = "combat".into();
    }
    assert_eq!(state.read().await.profile, "combat");

    {
        let mut w = state.write().await;
        w.profile = "landing".into();
    }
    assert_eq!(state.read().await.profile, "landing");
}

#[tokio::test]
async fn concurrent_reads_see_consistent_state() {
    let mut dash = DashboardState::new();
    dash.profile = "formation".into();
    dash.health = HealthStatus::Degraded;
    dash.uptime_secs = 999;
    let state = Arc::new(RwLock::new(dash));

    let mut handles = Vec::new();
    for _ in 0..10 {
        let s = Arc::clone(&state);
        handles.push(tokio::spawn(async move {
            let r = s.read().await;
            assert_eq!(r.profile, "formation");
            assert_eq!(r.health, HealthStatus::Degraded);
            assert_eq!(r.uptime_secs, 999);
        }));
    }
    for h in handles {
        h.await.unwrap();
    }
}

// ---------------------------------------------------------------------------
// Device list management
// ---------------------------------------------------------------------------

#[test]
fn add_and_remove_devices() {
    let mut state = DashboardState::new();
    assert!(state.devices.is_empty());

    state.devices.push(DeviceStatus {
        id: "stick-1".into(),
        name: "Stick".into(),
        connected: true,
        axis_count: 3,
        button_count: 12,
        last_seen: Utc::now(),
    });
    assert_eq!(state.devices.len(), 1);

    state.devices.retain(|d| d.id != "stick-1");
    assert!(state.devices.is_empty());
}

#[test]
fn adapter_connect_disconnect_cycle() {
    let mut state = DashboardState::new();
    state.adapters.push(AdapterStatus {
        name: "simconnect".into(),
        connected: false,
        sim_name: "MSFS".into(),
        aircraft: None,
        fps: None,
    });

    // Simulate connect
    state.adapters[0].connected = true;
    state.adapters[0].aircraft = Some("C172".into());
    state.adapters[0].fps = Some(60.0);
    assert!(state.adapters[0].connected);

    // Simulate disconnect
    state.adapters[0].connected = false;
    state.adapters[0].aircraft = None;
    state.adapters[0].fps = None;
    assert!(!state.adapters[0].connected);
    assert!(state.adapters[0].aircraft.is_none());
}

// ---------------------------------------------------------------------------
// Broadcast state (theme/modal equivalent via broadcast subscribers)
// ---------------------------------------------------------------------------

#[test]
fn broadcast_subscriber_lifecycle() {
    let bc = WsBroadcast::new(32);
    assert_eq!(bc.receiver_count(), 0);

    let rx1 = bc.subscribe();
    let rx2 = bc.subscribe();
    let rx3 = bc.subscribe();
    assert_eq!(bc.receiver_count(), 3);

    drop(rx1);
    assert_eq!(bc.receiver_count(), 2);
    drop(rx2);
    drop(rx3);
    assert_eq!(bc.receiver_count(), 0);
}

#[test]
fn broadcast_send_after_all_receivers_dropped() {
    let bc = WsBroadcast::new(16);
    let rx = bc.subscribe();
    drop(rx);

    let result = bc.send(WsMessage::AxisUpdate {
        axis: "roll".into(),
        value: 0.5,
    });
    assert!(result.is_err(), "send with no receivers must error");
}
