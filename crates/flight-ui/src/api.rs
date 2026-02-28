// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! REST API endpoints for the Flight Hub dashboard.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use tokio::sync::RwLock;

use crate::dashboard::{DashboardState, DeviceStatus, HealthStatus, ProfileEntry, WsMessage};
use crate::websocket::WsBroadcast;

/// Shared application state for all handlers.
pub type AppState = Arc<RwLock<DashboardState>>;

/// Shared broadcast channel handle.
pub type BroadcastHandle = Arc<WsBroadcast>;

/// Combined state accessible from handlers.
#[derive(Clone)]
pub struct ApiState {
    pub dashboard: AppState,
    pub broadcast: BroadcastHandle,
}

/// Build the full API router.
pub fn api_router(state: ApiState) -> Router {
    Router::new()
        .route("/api/v1/status", get(get_status))
        .route("/api/v1/devices", get(get_devices))
        .route("/api/v1/devices/{id}", get(get_device))
        .route("/api/v1/profiles", get(get_profiles))
        .route("/api/v1/profiles/{name}/activate", post(activate_profile))
        .route("/api/v1/axes", get(get_axes))
        .route("/api/v1/health", get(get_health))
        .with_state(state)
}

async fn get_status(State(state): State<ApiState>) -> Json<DashboardState> {
    let dashboard = state.dashboard.read().await;
    Json(dashboard.clone())
}

async fn get_devices(State(state): State<ApiState>) -> Json<Vec<DeviceStatus>> {
    let dashboard = state.dashboard.read().await;
    Json(dashboard.devices.clone())
}

async fn get_device(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<DeviceStatus>, StatusCode> {
    let dashboard = state.dashboard.read().await;
    dashboard
        .devices
        .iter()
        .find(|d| d.id == id)
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn get_profiles(State(state): State<ApiState>) -> Json<Vec<ProfileEntry>> {
    let dashboard = state.dashboard.read().await;
    let entries = vec![ProfileEntry {
        name: dashboard.profile.clone(),
        active: true,
    }];
    Json(entries)
}

async fn activate_profile(State(state): State<ApiState>, Path(name): Path<String>) -> StatusCode {
    {
        let mut dashboard = state.dashboard.write().await;
        dashboard.profile = name.clone();
    }
    let _ = state.broadcast.send(WsMessage::AdapterEvent {
        adapter: "profile".into(),
        connected: true,
    });
    StatusCode::OK
}

async fn get_axes(State(state): State<ApiState>) -> Json<std::collections::HashMap<String, f64>> {
    let dashboard = state.dashboard.read().await;
    Json(dashboard.axis_values.clone())
}

async fn get_health(State(state): State<ApiState>) -> Json<HealthStatus> {
    let dashboard = state.dashboard.read().await;
    Json(dashboard.health)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dashboard::{AdapterStatus, DashboardState};
    use axum_test::TestServer;
    use chrono::Utc;

    fn test_state() -> ApiState {
        let mut dash = DashboardState::new();
        dash.devices.push(DeviceStatus {
            id: "stick-1".into(),
            name: "Warthog Stick".into(),
            connected: true,
            axis_count: 3,
            button_count: 19,
            last_seen: Utc::now(),
        });
        dash.adapters.push(AdapterStatus {
            name: "simconnect".into(),
            connected: true,
            sim_name: "MSFS".into(),
            aircraft: Some("F-16C".into()),
            fps: Some(60.0),
        });
        dash.axis_values.insert("roll".into(), 0.25);
        dash.axis_values.insert("pitch".into(), -0.1);
        dash.profile = "combat".into();

        ApiState {
            dashboard: Arc::new(RwLock::new(dash)),
            broadcast: Arc::new(WsBroadcast::new(64)),
        }
    }

    fn server(state: ApiState) -> TestServer {
        let app = api_router(state);
        TestServer::new(app).unwrap()
    }

    #[tokio::test]
    async fn test_get_status() {
        let srv = server(test_state());
        let resp = srv.get("/api/v1/status").await;
        resp.assert_status_ok();
        let body: DashboardState = resp.json();
        assert_eq!(body.profile, "combat");
        assert_eq!(body.devices.len(), 1);
    }

    #[tokio::test]
    async fn test_get_devices() {
        let srv = server(test_state());
        let resp = srv.get("/api/v1/devices").await;
        resp.assert_status_ok();
        let body: Vec<DeviceStatus> = resp.json();
        assert_eq!(body.len(), 1);
        assert_eq!(body[0].name, "Warthog Stick");
    }

    #[tokio::test]
    async fn test_get_device_found() {
        let srv = server(test_state());
        let resp = srv.get("/api/v1/devices/stick-1").await;
        resp.assert_status_ok();
        let body: DeviceStatus = resp.json();
        assert_eq!(body.id, "stick-1");
    }

    #[tokio::test]
    async fn test_get_device_not_found() {
        let srv = server(test_state());
        let resp = srv.get("/api/v1/devices/unknown-99").await;
        resp.assert_status(StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_get_profiles() {
        let srv = server(test_state());
        let resp = srv.get("/api/v1/profiles").await;
        resp.assert_status_ok();
        let body: Vec<ProfileEntry> = resp.json();
        assert_eq!(body.len(), 1);
        assert!(body[0].active);
        assert_eq!(body[0].name, "combat");
    }

    #[tokio::test]
    async fn test_activate_profile() {
        let state = test_state();
        let srv = server(state.clone());
        let resp = srv.post("/api/v1/profiles/landing/activate").await;
        resp.assert_status_ok();
        let dash = state.dashboard.read().await;
        assert_eq!(dash.profile, "landing");
    }

    #[tokio::test]
    async fn test_get_axes() {
        let srv = server(test_state());
        let resp = srv.get("/api/v1/axes").await;
        resp.assert_status_ok();
        let body: std::collections::HashMap<String, f64> = resp.json();
        assert!(body.contains_key("roll"));
        assert!(body.contains_key("pitch"));
    }

    #[tokio::test]
    async fn test_get_health() {
        let srv = server(test_state());
        let resp = srv.get("/api/v1/health").await;
        resp.assert_status_ok();
        let body: HealthStatus = resp.json();
        assert_eq!(body, HealthStatus::Ok);
    }
}
