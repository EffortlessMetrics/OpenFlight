// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for data binding: IPC data → view model mapping,
//! subscription updates, broadcast delivery, and stale data handling.

use std::sync::Arc;

use chrono::Utc;
use flight_ui::api::ApiState;
use flight_ui::dashboard::{
    AdapterStatus, DashboardState, DeviceEventKind, DeviceStatus, HealthStatus, WsMessage,
};
use flight_ui::websocket::WsBroadcast;
use tokio::sync::RwLock;

fn make_populated_state() -> DashboardState {
    let mut dash = DashboardState::new();
    dash.profile = "test-profile".into();
    dash.health = HealthStatus::Ok;
    dash.uptime_secs = 120;
    dash.devices.push(DeviceStatus {
        id: "stick-1".into(),
        name: "Stick".into(),
        connected: true,
        axis_count: 3,
        button_count: 12,
        last_seen: Utc::now(),
    });
    dash.adapters.push(AdapterStatus {
        name: "simconnect".into(),
        connected: true,
        sim_name: "MSFS".into(),
        aircraft: Some("F-16C".into()),
        fps: Some(60.0),
    });
    dash.axis_values.insert("roll".into(), 0.0);
    dash.axis_values.insert("pitch".into(), 0.0);
    dash
}

fn make_api_state(dash: DashboardState) -> (ApiState, Arc<WsBroadcast>) {
    let bc = Arc::new(WsBroadcast::new(64));
    let state = ApiState {
        dashboard: Arc::new(RwLock::new(dash)),
        broadcast: Arc::clone(&bc),
    };
    (state, bc)
}

// ---------------------------------------------------------------------------
// IPC data → view model mapping
// ---------------------------------------------------------------------------

#[tokio::test]
async fn axis_values_updated_in_shared_state() {
    let (state, _bc) = make_api_state(make_populated_state());

    {
        let mut w = state.dashboard.write().await;
        w.axis_values.insert("roll".into(), 0.75);
        w.axis_values.insert("pitch".into(), -0.33);
    }

    let r = state.dashboard.read().await;
    assert!((r.axis_values["roll"] - 0.75).abs() < f64::EPSILON);
    assert!((r.axis_values["pitch"] - (-0.33)).abs() < f64::EPSILON);
}

#[tokio::test]
async fn device_added_reflected_in_state() {
    let (state, _bc) = make_api_state(DashboardState::new());

    {
        let mut w = state.dashboard.write().await;
        w.devices.push(DeviceStatus {
            id: "mfd-1".into(),
            name: "Cougar MFD".into(),
            connected: true,
            axis_count: 0,
            button_count: 20,
            last_seen: Utc::now(),
        });
    }

    let r = state.dashboard.read().await;
    assert_eq!(r.devices.len(), 1);
    assert_eq!(r.devices[0].id, "mfd-1");
}

// ---------------------------------------------------------------------------
// Subscription updates via broadcast
// ---------------------------------------------------------------------------

#[tokio::test]
async fn broadcast_delivers_axis_update_to_subscriber() {
    let bc = WsBroadcast::new(64);
    let mut rx = bc.subscribe();

    bc.send(WsMessage::AxisUpdate {
        axis: "yaw".into(),
        value: 0.42,
    })
    .unwrap();

    let msg = rx.try_recv().unwrap();
    match msg {
        WsMessage::AxisUpdate { axis, value } => {
            assert_eq!(axis, "yaw");
            assert!((value - 0.42).abs() < f64::EPSILON);
        }
        _ => panic!("expected AxisUpdate"),
    }
}

#[tokio::test]
async fn broadcast_delivers_device_event_to_subscriber() {
    let bc = WsBroadcast::new(64);
    let mut rx = bc.subscribe();

    bc.send(WsMessage::DeviceEvent {
        device_id: "pedals-1".into(),
        event: DeviceEventKind::Disconnected,
    })
    .unwrap();

    let msg = rx.try_recv().unwrap();
    match msg {
        WsMessage::DeviceEvent { device_id, event } => {
            assert_eq!(device_id, "pedals-1");
            assert_eq!(event, DeviceEventKind::Disconnected);
        }
        _ => panic!("expected DeviceEvent"),
    }
}

#[tokio::test]
async fn broadcast_multiple_subscribers_all_receive() {
    let bc = WsBroadcast::new(64);
    let mut rx1 = bc.subscribe();
    let mut rx2 = bc.subscribe();
    let mut rx3 = bc.subscribe();

    bc.send(WsMessage::AdapterEvent {
        adapter: "xplane".into(),
        connected: true,
    })
    .unwrap();

    for rx in [&mut rx1, &mut rx2, &mut rx3] {
        let msg = rx.try_recv().unwrap();
        assert!(matches!(msg, WsMessage::AdapterEvent { .. }));
    }
}

// ---------------------------------------------------------------------------
// Stale data handling (broadcast lag)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn broadcast_lag_does_not_panic() {
    let bc = WsBroadcast::new(4); // small capacity
    let mut rx = bc.subscribe();

    // Overflow the channel
    for i in 0..10 {
        let _ = bc.send(WsMessage::AxisUpdate {
            axis: format!("axis_{i}"),
            value: i as f64,
        });
    }

    // Receiver should get a Lagged error, then recover
    let result = rx.try_recv();
    // Either we get a message or a Lagged error — neither should panic
    match result {
        Ok(_) => {}
        Err(tokio::sync::broadcast::error::TryRecvError::Lagged(n)) => {
            assert!(n > 0, "lagged count must be positive");
        }
        Err(other) => panic!("unexpected error: {other:?}"),
    }
}

#[tokio::test]
async fn state_snapshot_is_point_in_time() {
    let (state, _bc) = make_api_state(make_populated_state());

    // Take a snapshot
    let snapshot = state.dashboard.read().await.clone();

    // Mutate the shared state
    {
        let mut w = state.dashboard.write().await;
        w.profile = "changed-after-snapshot".into();
        w.axis_values.insert("roll".into(), 0.99);
    }

    // Snapshot should still reflect old values
    assert_eq!(snapshot.profile, "test-profile");
    assert!((snapshot.axis_values["roll"]).abs() < f64::EPSILON);
}

#[tokio::test]
async fn health_change_broadcast_delivery() {
    let (state, bc) = make_api_state(make_populated_state());
    let mut rx = bc.subscribe();

    // Update health and broadcast
    {
        let mut w = state.dashboard.write().await;
        w.health = HealthStatus::Unavailable;
    }
    bc.send(WsMessage::AdapterEvent {
        adapter: "health".into(),
        connected: false,
    })
    .unwrap();

    let msg = rx.try_recv().unwrap();
    match msg {
        WsMessage::AdapterEvent {
            adapter,
            connected,
        } => {
            assert_eq!(adapter, "health");
            assert!(!connected);
        }
        _ => panic!("expected AdapterEvent"),
    }

    let r = state.dashboard.read().await;
    assert_eq!(r.health, HealthStatus::Unavailable);
}
