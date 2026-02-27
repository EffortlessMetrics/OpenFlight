// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HTTP health endpoint for the flight service.

use axum::{Json, Router, routing::get};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::net::TcpListener;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: HealthStatus,
    pub version: String,
    pub uptime_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Ok,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone)]
pub struct HealthEndpointState {
    pub status: HealthStatus,
    pub version: String,
    pub started_at: std::time::Instant,
}

impl HealthEndpointState {
    pub fn new(version: impl Into<String>) -> Self {
        Self {
            status: HealthStatus::Ok,
            version: version.into(),
            started_at: std::time::Instant::now(),
        }
    }

    pub fn to_response(&self) -> HealthResponse {
        HealthResponse {
            status: self.status.clone(),
            version: self.version.clone(),
            uptime_secs: self.started_at.elapsed().as_secs(),
        }
    }
}

async fn health_handler(
    axum::extract::State(state): axum::extract::State<
        Arc<tokio::sync::RwLock<HealthEndpointState>>,
    >,
) -> Json<HealthResponse> {
    let state = state.read().await;
    Json(state.to_response())
}

pub fn health_router(state: Arc<tokio::sync::RwLock<HealthEndpointState>>) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .with_state(state)
}

pub async fn serve_health(
    port: u16,
    state: Arc<tokio::sync::RwLock<HealthEndpointState>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&addr).await?;
    let app = health_router(state);
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum_test::TestServer;

    #[tokio::test]
    async fn test_health_ok() {
        let state = Arc::new(tokio::sync::RwLock::new(HealthEndpointState::new("0.1.0")));
        let app = health_router(state);
        let server = TestServer::new(app).unwrap();
        let resp = server.get("/health").await;
        resp.assert_status(StatusCode::OK);
        let body: HealthResponse = resp.json();
        assert_eq!(body.status, HealthStatus::Ok);
        assert_eq!(body.version, "0.1.0");
    }

    #[tokio::test]
    async fn test_health_degraded() {
        let state = Arc::new(tokio::sync::RwLock::new(HealthEndpointState::new("0.1.0")));
        {
            let mut s = state.write().await;
            s.status = HealthStatus::Degraded;
        }
        let app = health_router(state);
        let server = TestServer::new(app).unwrap();
        let resp = server.get("/health").await;
        resp.assert_status(StatusCode::OK); // still 200, body has degraded
        let body: HealthResponse = resp.json();
        assert_eq!(body.status, HealthStatus::Degraded);
    }

    #[test]
    fn test_health_status_serialization() {
        assert_eq!(
            serde_json::to_string(&HealthStatus::Ok).unwrap(),
            r#""ok""#
        );
        assert_eq!(
            serde_json::to_string(&HealthStatus::Degraded).unwrap(),
            r#""degraded""#
        );
    }
}
