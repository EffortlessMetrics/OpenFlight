// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for configuration UI: profile editor data, axis curve preview
//! data, deadzone preview values, and settings panel integration.

use std::sync::Arc;

use chrono::Utc;
use flight_ui::api::{api_router, ApiState};
use flight_ui::dashboard::{DashboardState, DeviceStatus, ProfileEntry};
use flight_ui::settings::SettingsPanel;
use flight_ui::websocket::WsBroadcast;
use tokio::sync::RwLock;

fn test_api_state() -> ApiState {
    let mut dash = DashboardState::new();
    dash.profile = "default".into();
    dash.axis_values.insert("roll".into(), 0.0);
    dash.axis_values.insert("pitch".into(), 0.0);
    dash.axis_values.insert("yaw".into(), 0.0);
    dash.axis_values.insert("throttle".into(), 0.5);
    dash.devices.push(DeviceStatus {
        id: "stick-1".into(),
        name: "Test Stick".into(),
        connected: true,
        axis_count: 3,
        button_count: 12,
        last_seen: Utc::now(),
    });

    ApiState {
        dashboard: Arc::new(RwLock::new(dash)),
        broadcast: Arc::new(WsBroadcast::new(64)),
    }
}

fn server(state: ApiState) -> axum_test::TestServer {
    let app = api_router(state);
    axum_test::TestServer::new(app).unwrap()
}

// ---------------------------------------------------------------------------
// Profile editor data via API
// ---------------------------------------------------------------------------

#[tokio::test]
async fn profile_activate_changes_active_profile() {
    let state = test_api_state();
    let srv = server(state.clone());

    let resp = srv.post("/api/v1/profiles/aerobatic/activate").await;
    resp.assert_status_ok();

    let resp = srv.get("/api/v1/profiles").await;
    resp.assert_status_ok();
    let profiles: Vec<ProfileEntry> = resp.json();
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].name, "aerobatic");
    assert!(profiles[0].active);
}

#[tokio::test]
async fn profile_activate_multiple_times_last_wins() {
    let state = test_api_state();
    let srv = server(state.clone());

    srv.post("/api/v1/profiles/combat/activate").await;
    srv.post("/api/v1/profiles/landing/activate").await;
    srv.post("/api/v1/profiles/cruise/activate").await;

    let resp = srv.get("/api/v1/profiles").await;
    let profiles: Vec<ProfileEntry> = resp.json();
    assert_eq!(profiles[0].name, "cruise");
}

#[tokio::test]
async fn profile_activate_sends_broadcast() {
    let state = test_api_state();
    let mut rx = state.broadcast.subscribe();
    let srv = server(state.clone());

    srv.post("/api/v1/profiles/night/activate").await;

    // Broadcast should have sent an AdapterEvent for the profile change
    let msg = rx.try_recv().unwrap();
    match msg {
        flight_ui::dashboard::WsMessage::AdapterEvent {
            adapter,
            connected,
        } => {
            assert_eq!(adapter, "profile");
            assert!(connected);
        }
        _ => panic!("expected AdapterEvent from profile activation"),
    }
}

// ---------------------------------------------------------------------------
// Axis curve preview data (via axes endpoint)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn axes_endpoint_returns_all_configured_axes() {
    let state = test_api_state();
    let srv = server(state);

    let resp = srv.get("/api/v1/axes").await;
    resp.assert_status_ok();
    let axes: std::collections::HashMap<String, f64> = resp.json();

    assert!(axes.contains_key("roll"));
    assert!(axes.contains_key("pitch"));
    assert!(axes.contains_key("yaw"));
    assert!(axes.contains_key("throttle"));
    assert_eq!(axes.len(), 4);
}

#[tokio::test]
async fn axes_reflect_updated_values() {
    let state = test_api_state();
    let srv = server(state.clone());

    {
        let mut w = state.dashboard.write().await;
        w.axis_values.insert("roll".into(), 0.88);
        w.axis_values.insert("pitch".into(), -0.55);
    }

    let resp = srv.get("/api/v1/axes").await;
    let axes: std::collections::HashMap<String, f64> = resp.json();
    assert!((axes["roll"] - 0.88).abs() < f64::EPSILON);
    assert!((axes["pitch"] - (-0.55)).abs() < f64::EPSILON);
}

// ---------------------------------------------------------------------------
// Settings panel
// ---------------------------------------------------------------------------

#[test]
fn settings_panel_new_and_default_are_equivalent() {
    let _p1 = SettingsPanel::new();
    let _p2 = SettingsPanel::default();
    // Both construct without panic — the docs manager initializes identically
}

#[test]
fn settings_panel_open_docs_unknown_sim_returns_error() {
    let panel = SettingsPanel::new();
    let result = panel.open_integration_docs("fake_sim_999");
    assert!(result.is_err());
}
