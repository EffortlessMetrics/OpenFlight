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
        GetCapabilityModeRequest, GetCapabilityModeResponse, GetSecurityStatusRequest,
        GetSecurityStatusResponse, GetServiceInfoRequest, GetServiceInfoResponse,
        GetSupportBundleRequest, GetSupportBundleResponse, HealthEvent, HealthSubscribeRequest,
        ListDevicesRequest, ListDevicesResponse, NegotiateFeaturesRequest,
        NegotiateFeaturesResponse, OneClickResolveRequest, OneClickResolveResponse,
        ResolveCurveConflictRequest, ResolveCurveConflictResponse, ServiceStatus,
        SetCapabilityModeRequest, SetCapabilityModeResponse,
        flight_service_server::FlightService as GrpcFlightService,
    },
};

use std::{collections::HashMap, sync::Arc, time::SystemTime};
use tokio::sync::broadcast;
use tonic::{Request, Response, Status};
use tracing::{debug, info, warn};

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
}

impl MockServiceContext {
    /// Construct a mock with sensible defaults.
    pub fn new() -> Self {
        Self {
            devices: vec![],
            version: crate::PROTOCOL_VERSION.to_string(),
        }
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
