// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! IPC server implementation

use crate::{
    ServerConfig,
    negotiation::negotiate_features,
    proto::{
        ApplyProfileRequest, ApplyProfileResponse, GetServiceInfoRequest, GetServiceInfoResponse,
        HealthEvent, HealthSubscribeRequest, ListDevicesRequest, ListDevicesResponse,
        NegotiateFeaturesRequest, NegotiateFeaturesResponse, ServiceStatus,
    },
};

/// FlightService trait - manually defined since we're using prost-build only
#[tonic::async_trait]
pub trait FlightService: Send + Sync + 'static {
    type HealthSubscribeStream: futures_core::Stream<Item = Result<HealthEvent, tonic::Status>>
        + Send
        + 'static;

    async fn negotiate_features(
        &self,
        request: tonic::Request<NegotiateFeaturesRequest>,
    ) -> Result<tonic::Response<NegotiateFeaturesResponse>, tonic::Status>;

    async fn list_devices(
        &self,
        request: tonic::Request<ListDevicesRequest>,
    ) -> Result<tonic::Response<ListDevicesResponse>, tonic::Status>;

    async fn health_subscribe(
        &self,
        request: tonic::Request<HealthSubscribeRequest>,
    ) -> Result<tonic::Response<Self::HealthSubscribeStream>, tonic::Status>;

    async fn apply_profile(
        &self,
        request: tonic::Request<ApplyProfileRequest>,
    ) -> Result<tonic::Response<ApplyProfileResponse>, tonic::Status>;

    async fn get_service_info(
        &self,
        request: tonic::Request<GetServiceInfoRequest>,
    ) -> Result<tonic::Response<GetServiceInfoResponse>, tonic::Status>;
}

/// FlightServiceServer wrapper for tonic server
pub struct FlightServiceServer<T> {
    #[allow(dead_code)] // Reserved for future tonic integration
    inner: T,
}

impl<T> FlightServiceServer<T>
where
    T: FlightService,
{
    pub fn new(service: T) -> Self {
        Self { inner: service }
    }
}
use anyhow::Result;
use flight_core::SecurityManager;
use std::{collections::HashMap, sync::Arc, time::SystemTime};
use tokio::sync::broadcast;
use tonic::{Request, Response, Status};
use tracing::{debug, info, warn};

/// Flight Hub service implementation
pub struct FlightServiceImpl {
    config: ServerConfig,
    health_sender: broadcast::Sender<HealthEvent>,
    service_start_time: SystemTime,
    security_manager: Arc<tokio::sync::RwLock<SecurityManager>>,
    // In a real implementation, these would be injected dependencies
    device_manager: Arc<dyn DeviceManager>,
    profile_manager: Arc<dyn ProfileManager>,
}

/// Device manager trait (to be implemented by actual device management)
pub trait DeviceManager: Send + Sync {
    fn list_devices(&self, request: &ListDevicesRequest) -> Result<ListDevicesResponse, Status>;
}

/// Profile manager trait (to be implemented by actual profile management)
pub trait ProfileManager: Send + Sync {
    fn apply_profile(&self, request: &ApplyProfileRequest) -> Result<ApplyProfileResponse, Status>;
}

/// Mock implementations for testing
#[derive(Debug)]
pub struct MockDeviceManager;

impl DeviceManager for MockDeviceManager {
    fn list_devices(&self, _request: &ListDevicesRequest) -> Result<ListDevicesResponse, Status> {
        Ok(ListDevicesResponse {
            devices: vec![],
            total_count: 0,
        })
    }
}

#[derive(Debug)]
pub struct MockProfileManager;

impl ProfileManager for MockProfileManager {
    fn apply_profile(
        &self,
        _request: &ApplyProfileRequest,
    ) -> Result<ApplyProfileResponse, Status> {
        Ok(ApplyProfileResponse {
            success: true,
            error_message: String::new(),
            validation_errors: vec![],
            effective_profile_hash: "mock-hash".to_string(),
            compile_time_ms: 10,
        })
    }
}

impl FlightServiceImpl {
    /// Create a new service implementation
    pub fn new(config: ServerConfig) -> Self {
        let (health_sender, _) = broadcast::channel(1000);

        Self {
            config,
            health_sender,
            service_start_time: SystemTime::now(),
            security_manager: Arc::new(tokio::sync::RwLock::new(SecurityManager::new())),
            device_manager: Arc::new(MockDeviceManager),
            profile_manager: Arc::new(MockProfileManager),
        }
    }

    /// Create with custom managers (for dependency injection)
    pub fn with_managers(
        config: ServerConfig,
        device_manager: Arc<dyn DeviceManager>,
        profile_manager: Arc<dyn ProfileManager>,
    ) -> Self {
        let (health_sender, _) = broadcast::channel(1000);

        Self {
            config,
            health_sender,
            service_start_time: SystemTime::now(),
            security_manager: Arc::new(tokio::sync::RwLock::new(SecurityManager::new())),
            device_manager,
            profile_manager,
        }
    }

    /// Create with custom security manager
    pub fn with_security_manager(
        config: ServerConfig,
        security_manager: SecurityManager,
        device_manager: Arc<dyn DeviceManager>,
        profile_manager: Arc<dyn ProfileManager>,
    ) -> Self {
        let (health_sender, _) = broadcast::channel(1000);

        Self {
            config,
            health_sender,
            service_start_time: SystemTime::now(),
            security_manager: Arc::new(tokio::sync::RwLock::new(security_manager)),
            device_manager,
            profile_manager,
        }
    }

    /// Get a health event sender for publishing events
    pub fn health_sender(&self) -> broadcast::Sender<HealthEvent> {
        self.health_sender.clone()
    }
}

#[tonic::async_trait]
impl FlightService for FlightServiceImpl {
    async fn negotiate_features(
        &self,
        request: tonic::Request<NegotiateFeaturesRequest>,
    ) -> Result<tonic::Response<NegotiateFeaturesResponse>, tonic::Status> {
        let request = request.into_inner();

        debug!(
            "Feature negotiation request from client version: {}",
            request.client_version
        );

        let response = negotiate_features(&request, &self.config.enabled_features)
            .map_err(|e| Status::invalid_argument(format!("Negotiation failed: {}", e)))?;

        if response.success {
            info!(
                "Feature negotiation successful with client {}. Enabled features: {:?}",
                request.client_version, response.enabled_features
            );
        } else {
            warn!(
                "Feature negotiation failed with client {}: {}",
                request.client_version, response.error_message
            );
        }

        Ok(tonic::Response::new(response))
    }

    async fn list_devices(
        &self,
        request: Request<ListDevicesRequest>,
    ) -> Result<Response<ListDevicesResponse>, Status> {
        let request = request.into_inner();

        debug!("List devices request: {:?}", request);

        let response = self.device_manager.list_devices(&request)?;

        Ok(Response::new(response))
    }

    type HealthSubscribeStream = std::pin::Pin<
        Box<dyn futures_core::Stream<Item = Result<HealthEvent, tonic::Status>> + Send>,
    >;

    async fn health_subscribe(
        &self,
        request: tonic::Request<HealthSubscribeRequest>,
    ) -> Result<tonic::Response<Self::HealthSubscribeStream>, tonic::Status> {
        let request = request.into_inner();

        debug!("Health subscribe request: {:?}", request);

        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let mut receiver = self.health_sender.subscribe();

        // Spawn a task to forward broadcast messages to the stream
        tokio::spawn(async move {
            while let Ok(event) = receiver.recv().await {
                if tx.send(Ok(event)).await.is_err() {
                    break; // Client disconnected
                }
            }
        });

        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        let boxed_stream: Self::HealthSubscribeStream = Box::pin(stream);

        Ok(tonic::Response::new(boxed_stream))
    }

    async fn apply_profile(
        &self,
        request: Request<ApplyProfileRequest>,
    ) -> Result<Response<ApplyProfileResponse>, Status> {
        let request = request.into_inner();

        debug!("Apply profile request");

        let response = self.profile_manager.apply_profile(&request)?;

        Ok(Response::new(response))
    }

    async fn get_service_info(
        &self,
        _request: Request<GetServiceInfoRequest>,
    ) -> Result<Response<GetServiceInfoResponse>, Status> {
        let uptime = self
            .service_start_time
            .elapsed()
            .unwrap_or_default()
            .as_secs() as i64;

        let mut capabilities = HashMap::new();
        for feature in &self.config.enabled_features {
            capabilities.insert(feature.clone(), "enabled".to_string());
        }

        // Add security status to capabilities
        let security_manager = self.security_manager.read().await;
        let telemetry_config = security_manager.get_telemetry_config();
        capabilities.insert(
            "telemetry_enabled".to_string(),
            telemetry_config.enabled.to_string(),
        );
        capabilities.insert("security_enforced".to_string(), "true".to_string());

        let response = GetServiceInfoResponse {
            version: self.config.server_version.clone(),
            uptime_seconds: uptime,
            status: ServiceStatus::Running.into(),
            capabilities,
        };

        Ok(Response::new(response))
    }
}

/// Flight Hub IPC server
pub struct FlightServer {
    service: FlightServiceImpl,
    #[allow(dead_code)] // Stored for future use in serve() implementation
    config: ServerConfig,
}

impl FlightServer {
    /// Create a new server with default configuration
    pub fn new() -> Self {
        Self::with_config(ServerConfig::default())
    }

    /// Create a new server with custom configuration
    pub fn with_config(config: ServerConfig) -> Self {
        let service = FlightServiceImpl::new(config.clone());

        Self { service, config }
    }

    /// Create with custom service implementation
    pub fn with_service(service: FlightServiceImpl, config: ServerConfig) -> Self {
        Self { service, config }
    }

    /// Start the server
    pub async fn serve(self) -> Result<(), Box<dyn std::error::Error>> {
        let addr: std::net::SocketAddr = "127.0.0.1:50051".parse()?; // For development

        info!("Starting Flight Hub IPC server on {}", addr);

        // For now, we'll use a simple HTTP server since we don't have full gRPC generation
        // In a complete implementation, this would use the generated FlightServiceServer
        info!("Flight Hub IPC server would start on {}", addr);

        // Placeholder - in real implementation this would be:
        // Server::builder()
        //     .add_service(FlightServiceServer::new(self.service))
        //     .serve(addr)
        //     .await?;

        Ok(())
    }

    /// Get a reference to the service for testing
    pub fn service(&self) -> &FlightServiceImpl {
        &self.service
    }
}

impl Default for FlightServer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::TransportType;

    #[tokio::test]
    async fn test_feature_negotiation() {
        let server = FlightServer::new();
        let service = server.service();

        let request = Request::new(NegotiateFeaturesRequest {
            client_version: "1.0.0".to_string(),
            supported_features: vec!["device-management".to_string()],
            preferred_transport: TransportType::NamedPipes.into(),
        });

        let response = service.negotiate_features(request).await.unwrap();
        let response = response.into_inner();

        assert!(response.success);
        assert!(
            response
                .enabled_features
                .contains(&"device-management".to_string())
        );
    }

    #[tokio::test]
    async fn test_service_info() {
        let server = FlightServer::new();
        let service = server.service();

        let request = Request::new(GetServiceInfoRequest {});

        let response = service.get_service_info(request).await.unwrap();
        let response = response.into_inner();

        assert_eq!(response.version, crate::PROTOCOL_VERSION);
        assert_eq!(response.status(), ServiceStatus::Running);
    }
}
