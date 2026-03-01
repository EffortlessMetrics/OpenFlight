// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Request handlers for gRPC services.
//!
//! All handlers delegate to a [`ServiceContext`] trait so that tests can
//! substitute mock implementations without needing a full service stack.

use crate::{
    ServerConfig,
    negotiation::negotiate_features,
    proto::{
        self, ApplyProfileRequest, ApplyProfileResponse, ConfigureTelemetryRequest,
        ConfigureTelemetryResponse, DetectCurveConflictsRequest, DetectCurveConflictsResponse,
        DeviceEvent, DisableAdapterRequest, DisableAdapterResponse, EnableAdapterRequest,
        EnableAdapterResponse, GetActiveProfileRequest, GetActiveProfileResponse,
        GetCapabilityModeRequest, GetCapabilityModeResponse, GetSecurityStatusRequest,
        GetSecurityStatusResponse, GetServiceInfoRequest, GetServiceInfoResponse,
        GetSupportBundleRequest, GetSupportBundleResponse, HealthEvent, HealthSubscribeRequest,
        ListAdaptersRequest, ListAdaptersResponse, ListDevicesRequest, ListDevicesResponse,
        ListProfilesRequest, ListProfilesResponse, NegotiateFeaturesRequest,
        NegotiateFeaturesResponse, OneClickResolveRequest, OneClickResolveResponse,
        ResolveCurveConflictRequest, ResolveCurveConflictResponse, ServiceStatus,
        SetCapabilityModeRequest, SetCapabilityModeResponse, SubscribeDeviceEventsRequest,
        SubscribeTelemetryRequest, TelemetryEvent,
        flight_service_server::FlightService as GrpcFlightService,
    },
};

use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc, time::SystemTime};
use tokio::sync::broadcast;
use tonic::{Request, Response, Status};
use tracing::{debug, info, warn};

// ---------------------------------------------------------------------------
// Simplified domain types for ServiceContext consumers
// ---------------------------------------------------------------------------

/// Lightweight device info independent of protobuf types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceInfo {
    /// Unique device identifier.
    pub id: String,
    /// Human-readable device name.
    pub name: String,
    /// Device type string (e.g. `"joystick"`, `"throttle"`).
    pub device_type: String,
    /// Whether the device is currently connected.
    pub connected: bool,
}

/// Lightweight profile info independent of protobuf types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileInfo {
    /// Profile name.
    pub name: String,
    /// Whether this profile is currently active.
    pub active: bool,
    /// Aircraft binding, if any.
    pub aircraft: Option<String>,
}

/// Aggregate health status for the service.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthStatus {
    /// Overall healthy flag.
    pub healthy: bool,
    /// Uptime in seconds.
    pub uptime_secs: u64,
    /// Per-component health.
    pub components: Vec<ComponentHealth>,
}

/// Health status for a single component.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentHealth {
    /// Component name.
    pub name: String,
    /// Whether this component is healthy.
    pub healthy: bool,
    /// Optional detail when unhealthy.
    pub detail: Option<String>,
}

/// Snapshot of runtime performance metrics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    /// p99 jitter in milliseconds.
    pub jitter_p99_ms: f64,
    /// p99 HID write latency in microseconds.
    pub hid_latency_p99_us: f64,
    /// Number of missed RT ticks.
    pub missed_ticks: u32,
    /// CPU usage percentage.
    pub cpu_usage_percent: f64,
    /// Memory usage in bytes.
    pub memory_usage_bytes: u64,
}

/// Simplified adapter info for ServiceContext consumers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdapterStatus {
    /// Simulator identifier (e.g. `"msfs"`, `"xplane"`, `"dcs"`).
    pub sim_id: String,
    /// Human-readable display name.
    pub display_name: String,
    /// Whether the adapter is enabled.
    pub enabled: bool,
    /// Whether the adapter is currently connected to its simulator.
    pub connected: bool,
    /// Detected sim version, if connected.
    pub version: Option<String>,
    /// Last error message, if any.
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// ServiceContext — abstraction for testability
// ---------------------------------------------------------------------------

/// Abstraction over real subsystems so handlers can be tested with mocks.
pub trait ServiceContext: Send + Sync + 'static {
    // ---- Device domain ----

    /// List connected devices.
    fn list_devices(&self, request: &ListDevicesRequest) -> Result<ListDevicesResponse, Status>;

    // ---- Profile domain ----

    /// Apply (or validate) a profile.
    fn apply_profile(&self, request: &ApplyProfileRequest) -> Result<ApplyProfileResponse, Status>;

    // ---- Diagnostics / health domain ----

    /// Return daemon version, uptime, and enabled features.
    fn get_service_info(&self) -> Result<GetServiceInfoResponse, Status>;

    /// Return aggregated health/diagnostics check.
    fn health_check(&self) -> Result<proto::HealthEvent, Status> {
        Ok(proto::HealthEvent {
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
            r#type: proto::HealthEventType::Info.into(),
            message: "healthy".to_string(),
            ..Default::default()
        })
    }

    // ---- Curve conflict domain ----

    /// Detect curve conflicts.
    fn detect_curve_conflicts(
        &self,
        _request: &DetectCurveConflictsRequest,
    ) -> Result<DetectCurveConflictsResponse, Status> {
        Ok(DetectCurveConflictsResponse {
            success: true,
            conflicts: vec![],
            error_message: String::new(),
        })
    }

    /// Resolve a single curve conflict.
    fn resolve_curve_conflict(
        &self,
        _request: &ResolveCurveConflictRequest,
    ) -> Result<ResolveCurveConflictResponse, Status> {
        Ok(ResolveCurveConflictResponse {
            success: true,
            error_message: String::new(),
            result: None,
        })
    }

    /// One-click resolve for curve conflicts.
    fn one_click_resolve(
        &self,
        _request: &OneClickResolveRequest,
    ) -> Result<OneClickResolveResponse, Status> {
        Ok(OneClickResolveResponse {
            success: true,
            error_message: String::new(),
            result: None,
        })
    }

    // ---- Capability domain ----

    /// Set capability mode.
    fn set_capability_mode(
        &self,
        _request: &SetCapabilityModeRequest,
    ) -> Result<SetCapabilityModeResponse, Status> {
        Ok(SetCapabilityModeResponse {
            success: true,
            error_message: String::new(),
            affected_axes: vec![],
            applied_limits: None,
        })
    }

    /// Get capability mode.
    fn get_capability_mode(
        &self,
        _request: &GetCapabilityModeRequest,
    ) -> Result<GetCapabilityModeResponse, Status> {
        Ok(GetCapabilityModeResponse {
            success: true,
            error_message: String::new(),
            axis_status: vec![],
        })
    }

    // ---- Security domain ----

    /// Get security status.
    fn get_security_status(
        &self,
        _request: &GetSecurityStatusRequest,
    ) -> Result<GetSecurityStatusResponse, Status> {
        Ok(GetSecurityStatusResponse {
            success: true,
            error_message: String::new(),
            plugins: vec![],
            telemetry_enabled: false,
            telemetry_data_types: vec![],
        })
    }

    /// Configure telemetry.
    fn configure_telemetry(
        &self,
        _request: &ConfigureTelemetryRequest,
    ) -> Result<ConfigureTelemetryResponse, Status> {
        Ok(ConfigureTelemetryResponse {
            success: true,
            error_message: String::new(),
        })
    }

    /// Get support bundle.
    fn get_support_bundle(
        &self,
        _request: &GetSupportBundleRequest,
    ) -> Result<GetSupportBundleResponse, Status> {
        Ok(GetSupportBundleResponse {
            success: true,
            error_message: String::new(),
            redacted_data: "{}".to_string(),
            bundle_size_bytes: 2,
        })
    }

    // ---- Simplified convenience methods (domain-type based) ----

    /// List connected devices as simplified [`DeviceInfo`] structs.
    fn list_device_info(&self) -> Vec<DeviceInfo> {
        vec![]
    }

    /// Look up a single device by ID.
    fn get_device_info(&self, _id: &str) -> Option<DeviceInfo> {
        None
    }

    /// List all known profiles.
    fn list_profiles(&self) -> Vec<ProfileInfo> {
        vec![]
    }

    /// Return the name of the currently active profile, if any.
    fn get_active_profile(&self) -> Option<String> {
        None
    }

    /// Activate a profile by name.
    fn activate_profile(&self, _name: &str) -> Result<(), String> {
        Ok(())
    }

    /// Return aggregate system health.
    fn system_health(&self) -> HealthStatus {
        HealthStatus {
            healthy: true,
            uptime_secs: 0,
            components: vec![],
        }
    }

    /// Return a snapshot of runtime metrics.
    fn get_metrics(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            jitter_p99_ms: 0.0,
            hid_latency_p99_us: 0.0,
            missed_ticks: 0,
            cpu_usage_percent: 0.0,
            memory_usage_bytes: 0,
        }
    }

    // ---- Profile RPC domain ----

    /// List profiles via RPC (as proto response).
    fn list_profiles_rpc(
        &self,
        _request: &ListProfilesRequest,
    ) -> Result<ListProfilesResponse, Status> {
        let profiles = self.list_profiles();
        let summaries = profiles
            .into_iter()
            .map(|p| proto::ProfileSummary {
                name: p.name,
                active: p.active,
                aircraft: p.aircraft.unwrap_or_default(),
                path: String::new(),
            })
            .collect();
        Ok(ListProfilesResponse {
            success: true,
            profiles: summaries,
            error_message: String::new(),
        })
    }

    /// Get the active profile via RPC (as proto response).
    fn get_active_profile_rpc(
        &self,
        _request: &GetActiveProfileRequest,
    ) -> Result<GetActiveProfileResponse, Status> {
        Ok(GetActiveProfileResponse {
            success: true,
            profile_name: self.get_active_profile().unwrap_or_default(),
            error_message: String::new(),
        })
    }

    // ---- Adapter domain ----

    /// List simulator adapters and their status.
    fn list_adapters(&self) -> Vec<AdapterStatus> {
        vec![]
    }

    /// Enable a simulator adapter.
    fn enable_adapter(&self, _sim_id: &str) -> Result<(), String> {
        Ok(())
    }

    /// Disable a simulator adapter.
    fn disable_adapter(&self, _sim_id: &str) -> Result<(), String> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// FlightServiceHandler — implements the generated gRPC trait
// ---------------------------------------------------------------------------

/// Concrete gRPC handler that bridges the generated [`GrpcFlightService`]
/// trait to a pluggable [`ServiceContext`].
pub struct FlightServiceHandler<C: ServiceContext> {
    ctx: Arc<C>,
    config: ServerConfig,
    health_tx: broadcast::Sender<HealthEvent>,
}

impl<C: ServiceContext> FlightServiceHandler<C> {
    /// Create a new handler backed by `ctx`.
    pub fn new(ctx: Arc<C>, config: ServerConfig) -> Self {
        let (health_tx, _) = broadcast::channel(1000);
        Self {
            ctx,
            config,
            health_tx,
        }
    }

    /// Obtain a broadcast sender for publishing health events.
    pub fn health_sender(&self) -> broadcast::Sender<HealthEvent> {
        self.health_tx.clone()
    }
}

#[tonic::async_trait]
impl<C: ServiceContext> GrpcFlightService for FlightServiceHandler<C> {
    async fn negotiate_features(
        &self,
        request: Request<NegotiateFeaturesRequest>,
    ) -> Result<Response<NegotiateFeaturesResponse>, Status> {
        let inner = request.into_inner();
        debug!("Feature negotiation from client v{}", inner.client_version);

        let response = negotiate_features(&inner, &self.config.enabled_features)
            .map_err(|e| Status::invalid_argument(format!("Negotiation failed: {e}")))?;

        if response.success {
            info!(
                "Negotiation OK with client v{}. Features: {:?}",
                inner.client_version, response.enabled_features
            );
        } else {
            warn!(
                "Negotiation FAILED with client v{}: {}",
                inner.client_version, response.error_message
            );
        }

        Ok(Response::new(response))
    }

    async fn list_devices(
        &self,
        request: Request<ListDevicesRequest>,
    ) -> Result<Response<ListDevicesResponse>, Status> {
        let inner = request.into_inner();
        debug!("list_devices request: {:?}", inner);
        self.ctx.list_devices(&inner).map(Response::new)
    }

    type HealthSubscribeStream =
        std::pin::Pin<Box<dyn futures_core::Stream<Item = Result<HealthEvent, Status>> + Send>>;

    async fn health_subscribe(
        &self,
        request: Request<HealthSubscribeRequest>,
    ) -> Result<Response<Self::HealthSubscribeStream>, Status> {
        let inner = request.into_inner();
        debug!("health_subscribe request: {:?}", inner);

        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let mut receiver = self.health_tx.subscribe();

        tokio::spawn(async move {
            while let Ok(event) = receiver.recv().await {
                if tx.send(Ok(event)).await.is_err() {
                    break;
                }
            }
        });

        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream)))
    }

    async fn apply_profile(
        &self,
        request: Request<ApplyProfileRequest>,
    ) -> Result<Response<ApplyProfileResponse>, Status> {
        let inner = request.into_inner();
        debug!("apply_profile request");
        self.ctx.apply_profile(&inner).map(Response::new)
    }

    async fn detect_curve_conflicts(
        &self,
        request: Request<DetectCurveConflictsRequest>,
    ) -> Result<Response<DetectCurveConflictsResponse>, Status> {
        let inner = request.into_inner();
        self.ctx.detect_curve_conflicts(&inner).map(Response::new)
    }

    async fn resolve_curve_conflict(
        &self,
        request: Request<ResolveCurveConflictRequest>,
    ) -> Result<Response<ResolveCurveConflictResponse>, Status> {
        let inner = request.into_inner();
        self.ctx.resolve_curve_conflict(&inner).map(Response::new)
    }

    async fn one_click_resolve(
        &self,
        request: Request<OneClickResolveRequest>,
    ) -> Result<Response<OneClickResolveResponse>, Status> {
        let inner = request.into_inner();
        self.ctx.one_click_resolve(&inner).map(Response::new)
    }

    async fn set_capability_mode(
        &self,
        request: Request<SetCapabilityModeRequest>,
    ) -> Result<Response<SetCapabilityModeResponse>, Status> {
        let inner = request.into_inner();
        self.ctx.set_capability_mode(&inner).map(Response::new)
    }

    async fn get_capability_mode(
        &self,
        request: Request<GetCapabilityModeRequest>,
    ) -> Result<Response<GetCapabilityModeResponse>, Status> {
        let inner = request.into_inner();
        self.ctx.get_capability_mode(&inner).map(Response::new)
    }

    async fn get_service_info(
        &self,
        _request: Request<GetServiceInfoRequest>,
    ) -> Result<Response<GetServiceInfoResponse>, Status> {
        self.ctx.get_service_info().map(Response::new)
    }

    async fn get_security_status(
        &self,
        request: Request<GetSecurityStatusRequest>,
    ) -> Result<Response<GetSecurityStatusResponse>, Status> {
        let inner = request.into_inner();
        self.ctx.get_security_status(&inner).map(Response::new)
    }

    async fn configure_telemetry(
        &self,
        request: Request<ConfigureTelemetryRequest>,
    ) -> Result<Response<ConfigureTelemetryResponse>, Status> {
        let inner = request.into_inner();
        self.ctx.configure_telemetry(&inner).map(Response::new)
    }

    async fn get_support_bundle(
        &self,
        request: Request<GetSupportBundleRequest>,
    ) -> Result<Response<GetSupportBundleResponse>, Status> {
        let inner = request.into_inner();
        self.ctx.get_support_bundle(&inner).map(Response::new)
    }

    async fn list_profiles(
        &self,
        request: Request<ListProfilesRequest>,
    ) -> Result<Response<ListProfilesResponse>, Status> {
        let inner = request.into_inner();
        debug!("list_profiles request");
        self.ctx.list_profiles_rpc(&inner).map(Response::new)
    }

    async fn get_active_profile(
        &self,
        request: Request<GetActiveProfileRequest>,
    ) -> Result<Response<GetActiveProfileResponse>, Status> {
        let inner = request.into_inner();
        debug!("get_active_profile request");
        self.ctx.get_active_profile_rpc(&inner).map(Response::new)
    }

    async fn list_adapters(
        &self,
        _request: Request<ListAdaptersRequest>,
    ) -> Result<Response<ListAdaptersResponse>, Status> {
        debug!("list_adapters request");
        let adapters = self
            .ctx
            .list_adapters()
            .into_iter()
            .map(|a| proto::AdapterInfo {
                sim_id: a.sim_id,
                display_name: a.display_name,
                enabled: a.enabled,
                state: if a.connected {
                    proto::AdapterState::Connected.into()
                } else {
                    proto::AdapterState::Disconnected.into()
                },
                version: a.version.unwrap_or_default(),
                error_message: a.error.unwrap_or_default(),
            })
            .collect();
        Ok(Response::new(ListAdaptersResponse {
            success: true,
            adapters,
            error_message: String::new(),
        }))
    }

    async fn enable_adapter(
        &self,
        request: Request<EnableAdapterRequest>,
    ) -> Result<Response<EnableAdapterResponse>, Status> {
        let inner = request.into_inner();
        debug!("enable_adapter request: {}", inner.sim_id);
        match self.ctx.enable_adapter(&inner.sim_id) {
            Ok(()) => Ok(Response::new(EnableAdapterResponse {
                success: true,
                error_message: String::new(),
            })),
            Err(e) => Ok(Response::new(EnableAdapterResponse {
                success: false,
                error_message: e,
            })),
        }
    }

    async fn disable_adapter(
        &self,
        request: Request<DisableAdapterRequest>,
    ) -> Result<Response<DisableAdapterResponse>, Status> {
        let inner = request.into_inner();
        debug!("disable_adapter request: {}", inner.sim_id);
        match self.ctx.disable_adapter(&inner.sim_id) {
            Ok(()) => Ok(Response::new(DisableAdapterResponse {
                success: true,
                error_message: String::new(),
            })),
            Err(e) => Ok(Response::new(DisableAdapterResponse {
                success: false,
                error_message: e,
            })),
        }
    }

    type SubscribeDeviceEventsStream =
        std::pin::Pin<Box<dyn futures_core::Stream<Item = Result<DeviceEvent, Status>> + Send>>;

    async fn subscribe_device_events(
        &self,
        request: Request<SubscribeDeviceEventsRequest>,
    ) -> Result<Response<Self::SubscribeDeviceEventsStream>, Status> {
        let inner = request.into_inner();
        debug!("subscribe_device_events request: {:?}", inner);

        // Create a channel-backed stream (events fed by service internals)
        let (_tx, rx) = tokio::sync::mpsc::channel::<Result<DeviceEvent, Status>>(100);
        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream)))
    }

    type SubscribeTelemetryStream =
        std::pin::Pin<Box<dyn futures_core::Stream<Item = Result<TelemetryEvent, Status>> + Send>>;

    async fn subscribe_telemetry(
        &self,
        request: Request<SubscribeTelemetryRequest>,
    ) -> Result<Response<Self::SubscribeTelemetryStream>, Status> {
        let inner = request.into_inner();
        debug!("subscribe_telemetry request: {:?}", inner);

        let (_tx, rx) = tokio::sync::mpsc::channel::<Result<TelemetryEvent, Status>>(100);
        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream)))
    }
}

// ---------------------------------------------------------------------------
// DefaultServiceContext — production implementation backed by real subsystems
// ---------------------------------------------------------------------------

/// Default production [`ServiceContext`] wired to real subsystems.
pub struct DefaultServiceContext {
    config: ServerConfig,
    service_start_time: SystemTime,
    device_manager: Arc<dyn crate::server::DeviceManager>,
    profile_manager: Arc<dyn crate::server::ProfileManager>,
}

impl DefaultServiceContext {
    /// Create a default context with the given managers.
    pub fn new(
        config: ServerConfig,
        device_manager: Arc<dyn crate::server::DeviceManager>,
        profile_manager: Arc<dyn crate::server::ProfileManager>,
    ) -> Self {
        Self {
            config,
            service_start_time: SystemTime::now(),
            device_manager,
            profile_manager,
        }
    }
}

impl ServiceContext for DefaultServiceContext {
    fn list_devices(&self, request: &ListDevicesRequest) -> Result<ListDevicesResponse, Status> {
        self.device_manager.list_devices(request)
    }

    fn apply_profile(&self, request: &ApplyProfileRequest) -> Result<ApplyProfileResponse, Status> {
        self.profile_manager.apply_profile(request)
    }

    fn get_service_info(&self) -> Result<GetServiceInfoResponse, Status> {
        let uptime = self
            .service_start_time
            .elapsed()
            .unwrap_or_default()
            .as_secs() as i64;

        let mut capabilities = HashMap::new();
        for feature in &self.config.enabled_features {
            capabilities.insert(feature.clone(), "enabled".to_string());
        }

        Ok(GetServiceInfoResponse {
            version: self.config.server_version.clone(),
            uptime_seconds: uptime,
            status: ServiceStatus::Running.into(),
            capabilities,
        })
    }
}

// ---------------------------------------------------------------------------
// MockServiceContext — for unit and integration tests
// ---------------------------------------------------------------------------

/// Test-only [`ServiceContext`] with canned responses.
#[derive(Debug, Default)]
pub struct MockServiceContext {
    /// Devices to return from `list_devices`.
    pub devices: Vec<proto::Device>,
    /// Version string to report.
    pub version: String,
    /// Simplified device list for convenience methods.
    pub device_info: Vec<DeviceInfo>,
    /// Profile list for convenience methods.
    pub profiles: Vec<ProfileInfo>,
    /// Active profile name.
    pub active_profile: Option<String>,
    /// Custom health status.
    pub health: Option<HealthStatus>,
    /// Custom metrics snapshot.
    pub metrics: Option<MetricsSnapshot>,
    /// Adapter status list.
    pub adapters: Vec<AdapterStatus>,
}

impl MockServiceContext {
    /// Construct a mock with sensible defaults.
    pub fn new() -> Self {
        Self {
            devices: vec![],
            version: crate::PROTOCOL_VERSION.to_string(),
            device_info: vec![],
            profiles: vec![],
            active_profile: None,
            health: None,
            metrics: None,
            adapters: vec![],
        }
    }

    /// Builder: set simplified device info list.
    pub fn with_device_info(mut self, devices: Vec<DeviceInfo>) -> Self {
        self.device_info = devices;
        self
    }

    /// Builder: set profiles.
    pub fn with_profiles(mut self, profiles: Vec<ProfileInfo>) -> Self {
        self.profiles = profiles;
        self
    }

    /// Builder: set active profile name.
    pub fn with_active_profile(mut self, name: impl Into<String>) -> Self {
        self.active_profile = Some(name.into());
        self
    }

    /// Builder: set health status.
    pub fn with_health(mut self, health: HealthStatus) -> Self {
        self.health = Some(health);
        self
    }

    /// Builder: set metrics snapshot.
    pub fn with_metrics(mut self, metrics: MetricsSnapshot) -> Self {
        self.metrics = Some(metrics);
        self
    }

    /// Builder: set adapter statuses.
    pub fn with_adapters(mut self, adapters: Vec<AdapterStatus>) -> Self {
        self.adapters = adapters;
        self
    }
}

impl ServiceContext for MockServiceContext {
    fn list_devices(&self, _request: &ListDevicesRequest) -> Result<ListDevicesResponse, Status> {
        Ok(ListDevicesResponse {
            total_count: self.devices.len() as i32,
            devices: self.devices.clone(),
        })
    }

    fn apply_profile(
        &self,
        _request: &ApplyProfileRequest,
    ) -> Result<ApplyProfileResponse, Status> {
        Ok(ApplyProfileResponse {
            success: true,
            error_message: String::new(),
            validation_errors: vec![],
            effective_profile_hash: "mock-hash".to_string(),
            compile_time_ms: 1,
        })
    }

    fn get_service_info(&self) -> Result<GetServiceInfoResponse, Status> {
        Ok(GetServiceInfoResponse {
            version: self.version.clone(),
            uptime_seconds: 0,
            status: ServiceStatus::Running.into(),
            capabilities: HashMap::new(),
        })
    }

    fn list_device_info(&self) -> Vec<DeviceInfo> {
        self.device_info.clone()
    }

    fn get_device_info(&self, id: &str) -> Option<DeviceInfo> {
        self.device_info.iter().find(|d| d.id == id).cloned()
    }

    fn list_profiles(&self) -> Vec<ProfileInfo> {
        self.profiles.clone()
    }

    fn get_active_profile(&self) -> Option<String> {
        self.active_profile.clone()
    }

    fn activate_profile(&self, _name: &str) -> Result<(), String> {
        Ok(())
    }

    fn system_health(&self) -> HealthStatus {
        self.health.clone().unwrap_or(HealthStatus {
            healthy: true,
            uptime_secs: 0,
            components: vec![],
        })
    }

    fn get_metrics(&self) -> MetricsSnapshot {
        self.metrics.clone().unwrap_or(MetricsSnapshot {
            jitter_p99_ms: 0.0,
            hid_latency_p99_us: 0.0,
            missed_ticks: 0,
            cpu_usage_percent: 0.0,
            memory_usage_bytes: 0,
        })
    }

    fn list_adapters(&self) -> Vec<AdapterStatus> {
        self.adapters.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- helper builders --

    fn mock_device(id: &str, name: &str) -> DeviceInfo {
        DeviceInfo {
            id: id.to_string(),
            name: name.to_string(),
            device_type: "joystick".to_string(),
            connected: true,
        }
    }

    fn mock_profile(name: &str, active: bool) -> ProfileInfo {
        ProfileInfo {
            name: name.to_string(),
            active,
            aircraft: None,
        }
    }

    // -----------------------------------------------------------------------
    // Handler gRPC delegation tests (existing + expanded)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn handler_negotiate_features_success() {
        let ctx = Arc::new(MockServiceContext::new());
        let handler = FlightServiceHandler::new(ctx, ServerConfig::default());

        let req = Request::new(NegotiateFeaturesRequest {
            client_version: "1.0.0".to_string(),
            supported_features: vec!["device-management".to_string()],
            preferred_transport: proto::TransportType::NamedPipes.into(),
        });

        let resp = handler.negotiate_features(req).await.unwrap().into_inner();
        assert!(resp.success);
        assert!(
            resp.enabled_features
                .contains(&"device-management".to_string())
        );
    }

    #[tokio::test]
    async fn handler_list_devices_empty() {
        let ctx = Arc::new(MockServiceContext::new());
        let handler = FlightServiceHandler::new(ctx, ServerConfig::default());

        let resp = handler
            .list_devices(Request::new(ListDevicesRequest::default()))
            .await
            .unwrap()
            .into_inner();

        assert_eq!(resp.total_count, 0);
        assert!(resp.devices.is_empty());
    }

    #[tokio::test]
    async fn handler_get_service_info() {
        let ctx = Arc::new(MockServiceContext::new());
        let handler = FlightServiceHandler::new(ctx, ServerConfig::default());

        let resp = handler
            .get_service_info(Request::new(GetServiceInfoRequest {}))
            .await
            .unwrap()
            .into_inner();

        assert_eq!(resp.version, crate::PROTOCOL_VERSION);
        assert_eq!(resp.status(), ServiceStatus::Running);
    }

    #[tokio::test]
    async fn handler_apply_profile() {
        let ctx = Arc::new(MockServiceContext::new());
        let handler = FlightServiceHandler::new(ctx, ServerConfig::default());

        let resp = handler
            .apply_profile(Request::new(ApplyProfileRequest {
                profile_json: "{}".to_string(),
                validate_only: false,
                force_apply: false,
            }))
            .await
            .unwrap()
            .into_inner();

        assert!(resp.success);
    }

    #[tokio::test]
    async fn handler_detect_curve_conflicts() {
        let ctx = Arc::new(MockServiceContext::new());
        let handler = FlightServiceHandler::new(ctx, ServerConfig::default());

        let resp = handler
            .detect_curve_conflicts(Request::new(DetectCurveConflictsRequest::default()))
            .await
            .unwrap()
            .into_inner();

        assert!(resp.success);
        assert!(resp.conflicts.is_empty());
    }

    // -----------------------------------------------------------------------
    // Handler: resolve + one-click + capability + security + telemetry
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn handler_resolve_curve_conflict() {
        let ctx = Arc::new(MockServiceContext::new());
        let handler = FlightServiceHandler::new(ctx, ServerConfig::default());

        let resp = handler
            .resolve_curve_conflict(Request::new(ResolveCurveConflictRequest::default()))
            .await
            .unwrap()
            .into_inner();

        assert!(resp.success);
    }

    #[tokio::test]
    async fn handler_one_click_resolve() {
        let ctx = Arc::new(MockServiceContext::new());
        let handler = FlightServiceHandler::new(ctx, ServerConfig::default());

        let resp = handler
            .one_click_resolve(Request::new(OneClickResolveRequest::default()))
            .await
            .unwrap()
            .into_inner();

        assert!(resp.success);
    }

    #[tokio::test]
    async fn handler_set_capability_mode() {
        let ctx = Arc::new(MockServiceContext::new());
        let handler = FlightServiceHandler::new(ctx, ServerConfig::default());

        let resp = handler
            .set_capability_mode(Request::new(SetCapabilityModeRequest::default()))
            .await
            .unwrap()
            .into_inner();

        assert!(resp.success);
    }

    #[tokio::test]
    async fn handler_get_capability_mode() {
        let ctx = Arc::new(MockServiceContext::new());
        let handler = FlightServiceHandler::new(ctx, ServerConfig::default());

        let resp = handler
            .get_capability_mode(Request::new(GetCapabilityModeRequest::default()))
            .await
            .unwrap()
            .into_inner();

        assert!(resp.success);
    }

    #[tokio::test]
    async fn handler_get_security_status() {
        let ctx = Arc::new(MockServiceContext::new());
        let handler = FlightServiceHandler::new(ctx, ServerConfig::default());

        let resp = handler
            .get_security_status(Request::new(GetSecurityStatusRequest {}))
            .await
            .unwrap()
            .into_inner();

        assert!(resp.success);
    }

    #[tokio::test]
    async fn handler_configure_telemetry() {
        let ctx = Arc::new(MockServiceContext::new());
        let handler = FlightServiceHandler::new(ctx, ServerConfig::default());

        let resp = handler
            .configure_telemetry(Request::new(ConfigureTelemetryRequest {
                enabled: true,
                data_types: vec!["Performance".to_string()],
            }))
            .await
            .unwrap()
            .into_inner();

        assert!(resp.success);
    }

    #[tokio::test]
    async fn handler_get_support_bundle() {
        let ctx = Arc::new(MockServiceContext::new());
        let handler = FlightServiceHandler::new(ctx, ServerConfig::default());

        let resp = handler
            .get_support_bundle(Request::new(GetSupportBundleRequest {}))
            .await
            .unwrap()
            .into_inner();

        assert!(resp.success);
        assert!(!resp.redacted_data.is_empty());
    }

    // -----------------------------------------------------------------------
    // MockServiceContext: simplified convenience method tests
    // -----------------------------------------------------------------------

    #[test]
    fn mock_list_device_info_empty_default() {
        let ctx = MockServiceContext::new();
        assert!(ctx.list_device_info().is_empty());
    }

    #[test]
    fn mock_list_device_info_with_devices() {
        let ctx = MockServiceContext::new().with_device_info(vec![
            mock_device("js-1", "Joystick Alpha"),
            mock_device("th-1", "Throttle Beta"),
        ]);
        let devices = ctx.list_device_info();
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].id, "js-1");
        assert_eq!(devices[1].name, "Throttle Beta");
    }

    #[test]
    fn mock_get_device_info_found() {
        let ctx =
            MockServiceContext::new().with_device_info(vec![mock_device("dev-42", "My Stick")]);
        let d = ctx.get_device_info("dev-42");
        assert!(d.is_some());
        assert_eq!(d.unwrap().name, "My Stick");
    }

    #[test]
    fn mock_get_device_info_not_found() {
        let ctx = MockServiceContext::new();
        assert!(ctx.get_device_info("no-such").is_none());
    }

    #[test]
    fn mock_list_profiles() {
        let ctx = MockServiceContext::new().with_profiles(vec![
            mock_profile("default", true),
            mock_profile("combat", false),
        ]);
        let profiles = ctx.list_profiles();
        assert_eq!(profiles.len(), 2);
        assert!(profiles[0].active);
        assert!(!profiles[1].active);
    }

    #[test]
    fn mock_get_active_profile() {
        let ctx = MockServiceContext::new().with_active_profile("combat");
        assert_eq!(ctx.get_active_profile(), Some("combat".to_string()));
    }

    #[test]
    fn mock_get_active_profile_none() {
        let ctx = MockServiceContext::new();
        assert!(ctx.get_active_profile().is_none());
    }

    #[test]
    fn mock_activate_profile_succeeds() {
        let ctx = MockServiceContext::new();
        assert!(ctx.activate_profile("any-profile").is_ok());
    }

    #[test]
    fn mock_system_health_default() {
        let ctx = MockServiceContext::new();
        let health = ctx.system_health();
        assert!(health.healthy);
        assert!(health.components.is_empty());
    }

    #[test]
    fn mock_system_health_custom() {
        let ctx = MockServiceContext::new().with_health(HealthStatus {
            healthy: false,
            uptime_secs: 600,
            components: vec![ComponentHealth {
                name: "ffb-engine".to_string(),
                healthy: false,
                detail: Some("envelope exceeded".to_string()),
            }],
        });
        let health = ctx.system_health();
        assert!(!health.healthy);
        assert_eq!(health.uptime_secs, 600);
        assert_eq!(health.components.len(), 1);
        assert_eq!(health.components[0].name, "ffb-engine");
    }

    #[test]
    fn mock_get_metrics_default() {
        let ctx = MockServiceContext::new();
        let m = ctx.get_metrics();
        assert_eq!(m.jitter_p99_ms, 0.0);
        assert_eq!(m.missed_ticks, 0);
    }

    #[test]
    fn mock_get_metrics_custom() {
        let ctx = MockServiceContext::new().with_metrics(MetricsSnapshot {
            jitter_p99_ms: 0.42,
            hid_latency_p99_us: 280.0,
            missed_ticks: 3,
            cpu_usage_percent: 12.5,
            memory_usage_bytes: 1024 * 1024,
        });
        let m = ctx.get_metrics();
        assert!((m.jitter_p99_ms - 0.42).abs() < f64::EPSILON);
        assert_eq!(m.missed_ticks, 3);
        assert_eq!(m.memory_usage_bytes, 1024 * 1024);
    }

    // -----------------------------------------------------------------------
    // Domain types: serialization round-trip
    // -----------------------------------------------------------------------

    #[test]
    fn device_info_serde_roundtrip() {
        let d = mock_device("d1", "Test");
        let json = serde_json::to_string(&d).unwrap();
        let restored: DeviceInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(d, restored);
    }

    #[test]
    fn profile_info_serde_roundtrip() {
        let p = ProfileInfo {
            name: "combat".into(),
            active: true,
            aircraft: Some("F-16C".into()),
        };
        let json = serde_json::to_string(&p).unwrap();
        let restored: ProfileInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(p, restored);
    }

    #[test]
    fn health_status_serde_roundtrip() {
        let h = HealthStatus {
            healthy: true,
            uptime_secs: 42,
            components: vec![ComponentHealth {
                name: "axis".into(),
                healthy: true,
                detail: None,
            }],
        };
        let json = serde_json::to_string(&h).unwrap();
        let restored: HealthStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(h, restored);
    }

    #[test]
    fn metrics_snapshot_serde_roundtrip() {
        let m = MetricsSnapshot {
            jitter_p99_ms: 0.3,
            hid_latency_p99_us: 250.0,
            missed_ticks: 1,
            cpu_usage_percent: 5.0,
            memory_usage_bytes: 2048,
        };
        let json = serde_json::to_string(&m).unwrap();
        let restored: MetricsSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(m, restored);
    }

    // -----------------------------------------------------------------------
    // Handler: health_sender broadcast channel
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn handler_health_sender_delivers_events() {
        let ctx = Arc::new(MockServiceContext::new());
        let handler = FlightServiceHandler::new(ctx, ServerConfig::default());

        let tx = handler.health_sender();
        let mut rx = tx.subscribe();

        let event = HealthEvent {
            timestamp: 1234,
            r#type: proto::HealthEventType::Warning.into(),
            message: "test warning".to_string(),
            ..Default::default()
        };
        tx.send(event.clone()).unwrap();

        let received = rx.recv().await.unwrap();
        assert_eq!(received.message, "test warning");
    }

    // -----------------------------------------------------------------------
    // MockServiceContext builder chaining
    // -----------------------------------------------------------------------

    #[test]
    fn mock_builder_chaining() {
        let ctx = MockServiceContext::new()
            .with_device_info(vec![mock_device("d1", "Dev")])
            .with_profiles(vec![mock_profile("p1", true)])
            .with_active_profile("p1")
            .with_health(HealthStatus {
                healthy: true,
                uptime_secs: 100,
                components: vec![],
            })
            .with_metrics(MetricsSnapshot {
                jitter_p99_ms: 0.1,
                hid_latency_p99_us: 100.0,
                missed_ticks: 0,
                cpu_usage_percent: 2.0,
                memory_usage_bytes: 512,
            });

        assert_eq!(ctx.list_device_info().len(), 1);
        assert_eq!(ctx.list_profiles().len(), 1);
        assert_eq!(ctx.get_active_profile(), Some("p1".to_string()));
        assert!(ctx.system_health().healthy);
        assert!((ctx.get_metrics().jitter_p99_ms - 0.1).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // Handler: new profile/adapter/streaming RPCs
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn handler_list_profiles() {
        let ctx = Arc::new(
            MockServiceContext::new().with_profiles(vec![
                mock_profile("default", true),
                mock_profile("combat", false),
            ]),
        );
        let handler = FlightServiceHandler::new(ctx, ServerConfig::default());

        let resp = handler
            .list_profiles(Request::new(ListProfilesRequest {
                include_inactive: true,
            }))
            .await
            .unwrap()
            .into_inner();

        assert!(resp.success);
        assert_eq!(resp.profiles.len(), 2);
        assert_eq!(resp.profiles[0].name, "default");
        assert!(resp.profiles[0].active);
    }

    #[tokio::test]
    async fn handler_get_active_profile() {
        let ctx = Arc::new(MockServiceContext::new().with_active_profile("combat"));
        let handler = FlightServiceHandler::new(ctx, ServerConfig::default());

        let resp = handler
            .get_active_profile(Request::new(GetActiveProfileRequest {}))
            .await
            .unwrap()
            .into_inner();

        assert!(resp.success);
        assert_eq!(resp.profile_name, "combat");
    }

    #[tokio::test]
    async fn handler_get_active_profile_none() {
        let ctx = Arc::new(MockServiceContext::new());
        let handler = FlightServiceHandler::new(ctx, ServerConfig::default());

        let resp = handler
            .get_active_profile(Request::new(GetActiveProfileRequest {}))
            .await
            .unwrap()
            .into_inner();

        assert!(resp.success);
        assert!(resp.profile_name.is_empty());
    }

    #[tokio::test]
    async fn handler_list_adapters_empty() {
        let ctx = Arc::new(MockServiceContext::new());
        let handler = FlightServiceHandler::new(ctx, ServerConfig::default());

        let resp = handler
            .list_adapters(Request::new(ListAdaptersRequest {}))
            .await
            .unwrap()
            .into_inner();

        assert!(resp.success);
        assert!(resp.adapters.is_empty());
    }

    #[tokio::test]
    async fn handler_list_adapters_with_data() {
        let ctx = Arc::new(MockServiceContext::new().with_adapters(vec![
            AdapterStatus {
                sim_id: "msfs".into(),
                display_name: "MSFS 2024".into(),
                enabled: true,
                connected: true,
                version: Some("1.0".into()),
                error: None,
            },
            AdapterStatus {
                sim_id: "dcs".into(),
                display_name: "DCS World".into(),
                enabled: true,
                connected: false,
                version: None,
                error: Some("Export.lua missing".into()),
            },
        ]));
        let handler = FlightServiceHandler::new(ctx, ServerConfig::default());

        let resp = handler
            .list_adapters(Request::new(ListAdaptersRequest {}))
            .await
            .unwrap()
            .into_inner();

        assert!(resp.success);
        assert_eq!(resp.adapters.len(), 2);
        assert_eq!(resp.adapters[0].sim_id, "msfs");
        assert!(resp.adapters[0].enabled);
        assert_eq!(resp.adapters[1].error_message, "Export.lua missing");
    }

    #[tokio::test]
    async fn handler_enable_adapter() {
        let ctx = Arc::new(MockServiceContext::new());
        let handler = FlightServiceHandler::new(ctx, ServerConfig::default());

        let resp = handler
            .enable_adapter(Request::new(EnableAdapterRequest {
                sim_id: "msfs".into(),
            }))
            .await
            .unwrap()
            .into_inner();

        assert!(resp.success);
    }

    #[tokio::test]
    async fn handler_disable_adapter() {
        let ctx = Arc::new(MockServiceContext::new());
        let handler = FlightServiceHandler::new(ctx, ServerConfig::default());

        let resp = handler
            .disable_adapter(Request::new(DisableAdapterRequest {
                sim_id: "xplane".into(),
            }))
            .await
            .unwrap()
            .into_inner();

        assert!(resp.success);
    }

    // -----------------------------------------------------------------------
    // Domain types: AdapterStatus serialization
    // -----------------------------------------------------------------------

    #[test]
    fn adapter_status_serde_roundtrip() {
        let a = AdapterStatus {
            sim_id: "msfs".into(),
            display_name: "MSFS 2024".into(),
            enabled: true,
            connected: true,
            version: Some("2024.1".into()),
            error: None,
        };
        let json = serde_json::to_string(&a).unwrap();
        let restored: AdapterStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(a, restored);
    }

    #[test]
    fn mock_list_adapters_default_empty() {
        let ctx = MockServiceContext::new();
        assert!(ctx.list_adapters().is_empty());
    }

    #[test]
    fn mock_list_adapters_with_data() {
        let ctx = MockServiceContext::new().with_adapters(vec![AdapterStatus {
            sim_id: "xplane".into(),
            display_name: "X-Plane 12".into(),
            enabled: false,
            connected: false,
            version: None,
            error: None,
        }]);
        let adapters = ctx.list_adapters();
        assert_eq!(adapters.len(), 1);
        assert_eq!(adapters[0].sim_id, "xplane");
    }
}
