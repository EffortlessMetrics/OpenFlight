// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for diagnostics UI: health status display, device tree,
//! adapter status, and validation/doc diagnostics.

use std::sync::Arc;

use chrono::Utc;
use flight_ui::api::{api_router, ApiState};
use flight_ui::dashboard::{AdapterStatus, DashboardState, DeviceStatus, HealthStatus};
use flight_ui::integration_docs::{InstallerSummary, ValidationResult};
use flight_ui::websocket::WsBroadcast;
use tokio::sync::RwLock;

fn diagnostics_state() -> ApiState {
    let mut dash = DashboardState::new();
    dash.health = HealthStatus::Degraded;
    dash.uptime_secs = 7200;

    // Multi-device tree
    for i in 0..4 {
        dash.devices.push(DeviceStatus {
            id: format!("dev-{i}"),
            name: format!("Device {i}"),
            connected: i < 3, // last one disconnected
            axis_count: i + 1,
            button_count: (i + 1) * 4,
            last_seen: Utc::now(),
        });
    }

    // Multi-adapter setup
    dash.adapters.push(AdapterStatus {
        name: "simconnect".into(),
        connected: true,
        sim_name: "MSFS 2024".into(),
        aircraft: Some("A-10C".into()),
        fps: Some(55.0),
    });
    dash.adapters.push(AdapterStatus {
        name: "xplane-udp".into(),
        connected: false,
        sim_name: "X-Plane 12".into(),
        aircraft: None,
        fps: None,
    });

    dash.axis_values.insert("roll".into(), 0.1);
    dash.axis_values.insert("pitch".into(), -0.2);

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
// Health status display
// ---------------------------------------------------------------------------

#[tokio::test]
async fn health_endpoint_reflects_degraded_state() {
    let state = diagnostics_state();
    let srv = server(state);

    let resp = srv.get("/api/v1/health").await;
    resp.assert_status_ok();
    let health: HealthStatus = resp.json();
    assert_eq!(health, HealthStatus::Degraded);
}

#[tokio::test]
async fn health_transitions_visible_through_api() {
    let state = diagnostics_state();
    let srv = server(state.clone());

    // Start as Degraded
    let health: HealthStatus = srv.get("/api/v1/health").await.json();
    assert_eq!(health, HealthStatus::Degraded);

    // Update to Unavailable
    {
        let mut w = state.dashboard.write().await;
        w.health = HealthStatus::Unavailable;
    }
    let health: HealthStatus = srv.get("/api/v1/health").await.json();
    assert_eq!(health, HealthStatus::Unavailable);

    // Recover to Ok
    {
        let mut w = state.dashboard.write().await;
        w.health = HealthStatus::Ok;
    }
    let health: HealthStatus = srv.get("/api/v1/health").await.json();
    assert_eq!(health, HealthStatus::Ok);
}

// ---------------------------------------------------------------------------
// Device tree display
// ---------------------------------------------------------------------------

#[tokio::test]
async fn device_tree_lists_all_devices() {
    let state = diagnostics_state();
    let srv = server(state);

    let resp = srv.get("/api/v1/devices").await;
    resp.assert_status_ok();
    let devices: Vec<DeviceStatus> = resp.json();
    assert_eq!(devices.len(), 4);
}

#[tokio::test]
async fn device_tree_individual_lookup() {
    let state = diagnostics_state();
    let srv = server(state);

    let resp = srv.get("/api/v1/devices/dev-2").await;
    resp.assert_status_ok();
    let dev: DeviceStatus = resp.json();
    assert_eq!(dev.id, "dev-2");
    assert_eq!(dev.name, "Device 2");
    assert!(dev.connected);
}

#[tokio::test]
async fn device_tree_disconnected_device_visible() {
    let state = diagnostics_state();
    let srv = server(state);

    // dev-3 is disconnected
    let resp = srv.get("/api/v1/devices/dev-3").await;
    resp.assert_status_ok();
    let dev: DeviceStatus = resp.json();
    assert!(!dev.connected);
}

#[tokio::test]
async fn device_lookup_missing_returns_not_found() {
    let state = diagnostics_state();
    let srv = server(state);

    let resp = srv.get("/api/v1/devices/no-such-device").await;
    resp.assert_status(axum::http::StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// Full status snapshot (trace viewer / metric display equivalent)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn status_snapshot_contains_full_state() {
    let state = diagnostics_state();
    let srv = server(state);

    let resp = srv.get("/api/v1/status").await;
    resp.assert_status_ok();
    let dash: DashboardState = resp.json();

    assert_eq!(dash.health, HealthStatus::Degraded);
    assert_eq!(dash.uptime_secs, 7200);
    assert_eq!(dash.devices.len(), 4);
    assert_eq!(dash.adapters.len(), 2);
    assert!(dash.axis_values.contains_key("roll"));
    assert!(dash.axis_values.contains_key("pitch"));
}

// ---------------------------------------------------------------------------
// Validation / doc diagnostics
// ---------------------------------------------------------------------------

#[test]
fn validation_result_empty_is_valid() {
    let result = ValidationResult::new();
    assert!(result.is_valid());
}

#[test]
fn validation_result_with_warnings_only_is_valid() {
    let mut result = ValidationResult::new();
    result.add_warning("cosmetic issue".into());
    result.add_warning("optional section missing".into());
    assert!(result.is_valid());
    assert_eq!(result.warnings.len(), 2);
}

#[test]
fn validation_result_mixed_errors_and_warnings() {
    let mut result = ValidationResult::new();
    result.add_warning("cosmetic".into());
    result.add_error("required section missing".into());
    result.add_warning("optional".into());
    assert!(!result.is_valid());
    assert_eq!(result.errors.len(), 1);
    assert_eq!(result.warnings.len(), 2);
}

#[test]
fn installer_summary_empty_does_not_require_admin() {
    let summary = InstallerSummary::new();
    assert!(!summary.requires_admin);
    assert_eq!(summary.total_files_modified, 0);
    assert!(summary.simulators_supported.is_empty());
    assert!(summary.network_ports_used.is_empty());
}
