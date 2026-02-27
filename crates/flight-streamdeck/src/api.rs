// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! StreamDeck Web API implementation
//!
//! Provides REST API endpoints for StreamDeck plugin integration with
//! telemetry data, profile management, and event handling.

use crate::compatibility::CompatibilityStatus;
use crate::{AircraftType, AppVersion, ProfileManager, VersionCompatibility};
use axum::{
    Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
};
use flight_bus::BusSnapshot;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// API error types
#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Version not supported: {0}")]
    VersionNotSupported(String),

    #[error("Profile not found: {0}")]
    ProfileNotFound(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Internal server error: {0}")]
    InternalError(String),

    #[error("Telemetry not available")]
    TelemetryNotAvailable,
}

impl From<ApiError> for StatusCode {
    fn from(error: ApiError) -> Self {
        match error {
            ApiError::VersionNotSupported(_) => StatusCode::BAD_REQUEST,
            ApiError::ProfileNotFound(_) => StatusCode::NOT_FOUND,
            ApiError::InvalidRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::TelemetryNotAvailable => StatusCode::SERVICE_UNAVAILABLE,
        }
    }
}

/// API response wrapper
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub timestamp: u64,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    pub fn error(error: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
}

/// Version check request
#[derive(Debug, Serialize, Deserialize)]
pub struct VersionCheckRequest {
    pub app_version: String,
    pub plugin_uuid: String,
}

/// Version check response
#[derive(Debug, Serialize, Deserialize)]
pub struct VersionCheckResponse {
    pub compatible: bool,
    pub status: String,
    pub available_features: Vec<String>,
    pub user_guidance: String,
    pub api_version: String,
}

/// Telemetry request parameters
#[derive(Debug, Deserialize)]
pub struct TelemetryQuery {
    pub fields: Option<String>, // Comma-separated list of fields
    pub format: Option<String>, // "json" or "compact"
}

/// Telemetry response
#[derive(Debug, Serialize, Deserialize)]
pub struct TelemetryResponse {
    pub timestamp: u64,
    pub sim: String,
    pub aircraft: String,
    pub data: serde_json::Value,
}

/// Profile list response
#[derive(Debug, Serialize, Deserialize)]
pub struct ProfileListResponse {
    pub profiles: HashMap<String, ProfileInfo>,
}

/// Profile information
#[derive(Debug, Serialize, Deserialize)]
pub struct ProfileInfo {
    pub name: String,
    pub aircraft_type: String,
    pub description: String,
    pub actions_count: u32,
    pub last_modified: u64,
}

/// Event subscription request
#[derive(Debug, Deserialize)]
pub struct EventSubscriptionRequest {
    pub events: Vec<String>,
    pub callback_url: Option<String>,
}

/// Event subscription response
#[derive(Debug, Serialize, Deserialize)]
pub struct EventSubscriptionResponse {
    pub subscription_id: String,
    pub subscribed_events: Vec<String>,
    pub websocket_url: String,
}

/// API state shared between handlers
#[derive(Clone)]
pub struct ApiState {
    pub compatibility: Arc<RwLock<VersionCompatibility>>,
    pub profile_manager: Arc<RwLock<ProfileManager>>,
    pub telemetry: Arc<RwLock<Option<BusSnapshot>>>,
    pub event_subscriptions: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

impl ApiState {
    pub fn new(compatibility: VersionCompatibility, profile_manager: ProfileManager) -> Self {
        Self {
            compatibility: Arc::new(RwLock::new(compatibility)),
            profile_manager: Arc::new(RwLock::new(profile_manager)),
            telemetry: Arc::new(RwLock::new(None)),
            event_subscriptions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

/// StreamDeck API implementation
pub struct StreamDeckApi {
    state: ApiState,
}

impl StreamDeckApi {
    pub fn new(compatibility: VersionCompatibility, profile_manager: ProfileManager) -> Self {
        Self {
            state: ApiState::new(compatibility, profile_manager),
        }
    }

    /// Create the API router with all endpoints
    pub fn create_router(&self) -> Router {
        Router::new()
            .route("/api/v1/version/check", post(version_check))
            .route("/api/v1/telemetry", get(get_telemetry))
            .route("/api/v1/profiles", get(list_profiles))
            .route("/api/v1/profiles/{aircraft_type}", get(get_profile))
            .route("/api/v1/events/subscribe", post(subscribe_events))
            .route("/api/v1/health", get(health_check))
            .route("/api/v1/status", get(get_status))
            .with_state(self.state.clone())
    }

    /// Update telemetry data
    pub async fn update_telemetry(&self, snapshot: BusSnapshot) {
        let mut telemetry = self.state.telemetry.write().await;
        *telemetry = Some(snapshot);
    }

    /// Get current API state
    pub fn get_state(&self) -> &ApiState {
        &self.state
    }
}

/// Version check endpoint
async fn version_check(
    State(state): State<ApiState>,
    Json(request): Json<VersionCheckRequest>,
) -> Result<Json<ApiResponse<VersionCheckResponse>>, StatusCode> {
    info!("Version check request: {:?}", request);

    let app_version = match AppVersion::from_string(&request.app_version) {
        Ok(version) => version,
        Err(e) => {
            warn!("Invalid version format: {}", e);
            return Ok(Json(ApiResponse::error(format!(
                "Invalid version format: {}",
                e
            ))));
        }
    };

    let mut compatibility = state.compatibility.write().await;

    let is_compatible = match compatibility.is_compatible(&app_version) {
        Ok(compatible) => compatible,
        Err(e) => {
            error!("Version compatibility check failed: {}", e);
            return Ok(Json(ApiResponse::success(VersionCheckResponse {
                compatible: false,
                status: "unsupported".to_string(),
                available_features: Vec::new(),
                user_guidance: e.to_string(),
                api_version: "1.0.0".to_string(),
            })));
        }
    };

    if is_compatible {
        if let Err(e) = compatibility.set_app_version(app_version) {
            warn!("Failed to set app version: {}", e);
        }
    }

    let status = match compatibility.get_compatibility_status() {
        Some(CompatibilityStatus::FullySupported) => "fully_supported",
        Some(CompatibilityStatus::PartiallySupported { .. }) => "partially_supported",
        Some(CompatibilityStatus::Deprecated { .. }) => "deprecated",
        Some(CompatibilityStatus::Unsupported { .. }) => "unsupported",
        None => "unknown",
    };

    let response = VersionCheckResponse {
        compatible: is_compatible,
        status: status.to_string(),
        available_features: compatibility.get_available_features(),
        user_guidance: compatibility.get_user_guidance(),
        api_version: "1.0.0".to_string(),
    };

    Ok(Json(ApiResponse::success(response)))
}

/// Get telemetry data endpoint
async fn get_telemetry(
    State(state): State<ApiState>,
    Query(query): Query<TelemetryQuery>,
) -> Result<Json<ApiResponse<TelemetryResponse>>, StatusCode> {
    let telemetry = state.telemetry.read().await;

    let snapshot = match telemetry.as_ref() {
        Some(snapshot) => snapshot,
        None => {
            return Ok(Json(ApiResponse::error(
                "Telemetry not available".to_string(),
            )));
        }
    };

    let data = if let Some(fields) = query.fields {
        // Filter specific fields
        let requested_fields: Vec<&str> = fields.split(',').collect();
        let mut filtered_data = serde_json::Map::new();

        let full_data = serde_json::to_value(snapshot).map_err(|e| {
            error!("Failed to serialize telemetry: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        if let serde_json::Value::Object(obj) = full_data {
            for field in requested_fields {
                if let Some(value) = obj.get(field.trim()) {
                    filtered_data.insert(field.trim().to_string(), value.clone());
                }
            }
        }

        serde_json::Value::Object(filtered_data)
    } else {
        // Return all data
        serde_json::to_value(snapshot).map_err(|e| {
            error!("Failed to serialize telemetry: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
    };

    let response = TelemetryResponse {
        timestamp: snapshot.timestamp,
        sim: format!("{:?}", snapshot.sim),
        aircraft: format!("{:?}", snapshot.aircraft),
        data,
    };

    Ok(Json(ApiResponse::success(response)))
}

/// List available profiles endpoint
async fn list_profiles(
    State(state): State<ApiState>,
) -> Result<Json<ApiResponse<ProfileListResponse>>, StatusCode> {
    let profile_manager = state.profile_manager.read().await;
    let profiles = profile_manager.get_profiles();

    let mut profile_info = HashMap::new();

    for (aircraft_type, profile_data) in profiles {
        let info = ProfileInfo {
            name: format!("{:?} Sample Profile", aircraft_type),
            aircraft_type: format!("{:?}", aircraft_type),
            description: format!("Sample StreamDeck profile for {:?} aircraft", aircraft_type),
            actions_count: count_actions_in_profile(profile_data),
            last_modified: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        profile_info.insert(format!("{:?}", aircraft_type), info);
    }

    let response = ProfileListResponse {
        profiles: profile_info,
    };

    Ok(Json(ApiResponse::success(response)))
}

/// Get specific profile endpoint
async fn get_profile(
    State(state): State<ApiState>,
    Path(aircraft_type): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    let profile_manager = state.profile_manager.read().await;

    let aircraft_type_enum = match aircraft_type.to_lowercase().as_str() {
        "ga" => AircraftType::GA,
        "airbus" => AircraftType::Airbus,
        "helo" => AircraftType::Helo,
        _ => {
            return Ok(Json(ApiResponse::error(format!(
                "Unknown aircraft type: {}",
                aircraft_type
            ))));
        }
    };

    let profiles = profile_manager.get_profiles();

    if let Some(profile) = profiles.get(&aircraft_type_enum) {
        Ok(Json(ApiResponse::success(profile.clone())))
    } else {
        Ok(Json(ApiResponse::error(format!(
            "Profile not found for aircraft type: {}",
            aircraft_type
        ))))
    }
}

/// Subscribe to events endpoint
async fn subscribe_events(
    State(state): State<ApiState>,
    Json(request): Json<EventSubscriptionRequest>,
) -> Result<Json<ApiResponse<EventSubscriptionResponse>>, StatusCode> {
    let subscription_id = uuid::Uuid::new_v4().to_string();

    let mut subscriptions = state.event_subscriptions.write().await;
    subscriptions.insert(subscription_id.clone(), request.events.clone());

    let response = EventSubscriptionResponse {
        subscription_id: subscription_id.clone(),
        subscribed_events: request.events,
        websocket_url: format!("ws://localhost:8080/api/v1/events/ws/{}", subscription_id),
    };

    info!("Created event subscription: {}", subscription_id);
    Ok(Json(ApiResponse::success(response)))
}

/// Health check endpoint
async fn health_check() -> Json<ApiResponse<serde_json::Value>> {
    let health_data = serde_json::json!({
        "status": "healthy",
        "version": "1.0.0",
        "uptime": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    });

    Json(ApiResponse::success(health_data))
}

/// Get API status endpoint
async fn get_status(State(state): State<ApiState>) -> Json<ApiResponse<serde_json::Value>> {
    let compatibility = state.compatibility.read().await;
    let telemetry = state.telemetry.read().await;
    let subscriptions = state.event_subscriptions.read().await;

    let status_data = serde_json::json!({
        "api_version": "1.0.0",
        "telemetry_available": telemetry.is_some(),
        "active_subscriptions": subscriptions.len(),
        "available_features": compatibility.get_available_features(),
        "compatibility_status": compatibility.get_compatibility_status()
    });

    Json(ApiResponse::success(status_data))
}

/// Helper function to count actions in a profile
fn count_actions_in_profile(profile: &serde_json::Value) -> u32 {
    if let Some(actions) = profile.get("actions") {
        if let Some(actions_array) = actions.as_array() {
            return actions_array.len() as u32;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ProfileManager, VersionCompatibility};
    use axum::http::StatusCode;
    use axum_test::TestServer;

    async fn create_test_api() -> StreamDeckApi {
        let compatibility = VersionCompatibility::new();
        let mut profile_manager = ProfileManager::new();
        profile_manager.load_sample_profiles().unwrap();

        StreamDeckApi::new(compatibility, profile_manager)
    }

    #[tokio::test]
    async fn test_version_check_endpoint() {
        let api = create_test_api().await;
        let app = api.create_router();
        let server = TestServer::new(app).unwrap();

        let request = VersionCheckRequest {
            app_version: "6.2.0".to_string(),
            plugin_uuid: "test-uuid".to_string(),
        };

        let response = server.post("/api/v1/version/check").json(&request).await;

        assert_eq!(response.status_code(), StatusCode::OK);

        let body: ApiResponse<VersionCheckResponse> = response.json();
        assert!(body.success);
        assert!(body.data.is_some());

        let data = body.data.unwrap();
        assert!(data.compatible);
        assert_eq!(data.status, "fully_supported");
    }

    #[tokio::test]
    async fn test_health_check_endpoint() {
        let api = create_test_api().await;
        let app = api.create_router();
        let server = TestServer::new(app).unwrap();

        let response = server.get("/api/v1/health").await;
        assert_eq!(response.status_code(), StatusCode::OK);

        let body: ApiResponse<serde_json::Value> = response.json();
        assert!(body.success);
        assert!(body.data.is_some());
    }

    #[tokio::test]
    async fn test_list_profiles_endpoint() {
        let api = create_test_api().await;
        let app = api.create_router();
        let server = TestServer::new(app).unwrap();

        let response = server.get("/api/v1/profiles").await;
        assert_eq!(response.status_code(), StatusCode::OK);

        let body: ApiResponse<ProfileListResponse> = response.json();
        assert!(body.success);
        assert!(body.data.is_some());

        let data = body.data.unwrap();
        assert!(!data.profiles.is_empty());
    }

    #[tokio::test]
    async fn test_get_profile_endpoint() {
        let api = create_test_api().await;
        let app = api.create_router();
        let server = TestServer::new(app).unwrap();

        let response = server.get("/api/v1/profiles/ga").await;
        assert_eq!(response.status_code(), StatusCode::OK);

        let body: ApiResponse<serde_json::Value> = response.json();
        assert!(body.success);
        assert!(body.data.is_some());
    }

    #[tokio::test]
    async fn test_telemetry_endpoint_no_data() {
        let api = create_test_api().await;
        let app = api.create_router();
        let server = TestServer::new(app).unwrap();

        let response = server.get("/api/v1/telemetry").await;
        assert_eq!(response.status_code(), StatusCode::OK);

        let body: ApiResponse<TelemetryResponse> = response.json();
        assert!(!body.success);
        assert!(body.error.is_some());
    }
}
