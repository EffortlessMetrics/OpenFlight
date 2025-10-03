// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! IPC server implementation

use crate::{
    negotiation::negotiate_features,
    proto::{
        ApplyProfileRequest, ApplyProfileResponse, GetServiceInfoRequest, GetServiceInfoResponse,
        HealthEvent, HealthSubscribeRequest, ListDevicesRequest, ListDevicesResponse,
        NegotiateFeaturesRequest, NegotiateFeaturesResponse, ServiceStatus,
    },
    ServerConfig,
};

/// FlightService trait - manually defined since we're not using tonic-build service generation
#[tonic::async_trait]
pub trait FlightService: Send + Sync + 'static {
    type HealthSubscribeStream: futures_core::Stream<Item = Result<HealthEvent, tonic::Status>> + Send + 'static;
    
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
use anyhow::Result;
use flight_core::{SecurityManager, TelemetryDataType};
use std::{collections::HashMap, sync::Arc, time::SystemTime};
use tokio::sync::broadcast;
use tonic::{transport::Server, Request, Response, Status};
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
    fn apply_profile(&self, _request: &ApplyProfileRequest) -> Result<ApplyProfileResponse, Status> {
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
    
    type HealthSubscribeStream = std::pin::Pin<Box<dyn futures_core::Stream<Item = Result<HealthEvent, tonic::Status>> + Send>>;
    
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
    
    async fn detect_curve_conflicts(
        &self,
        request: Request<crate::proto::DetectCurveConflictsRequest>,
    ) -> Result<Response<crate::proto::DetectCurveConflictsResponse>, Status> {
        let _request = request.into_inner();
        
        debug!("Detect curve conflicts request");
        
        // Mock implementation - in real implementation this would delegate to curve conflict service
        let response = crate::proto::DetectCurveConflictsResponse {
            success: true,
            conflicts: vec![],
            error_message: String::new(),
        };
        
        Ok(Response::new(response))
    }
    
    async fn resolve_curve_conflict(
        &self,
        request: Request<crate::proto::ResolveCurveConflictRequest>,
    ) -> Result<Response<crate::proto::ResolveCurveConflictResponse>, Status> {
        let _request = request.into_inner();
        
        debug!("Resolve curve conflict request");
        
        // Mock implementation - in real implementation this would delegate to curve conflict service
        let response = crate::proto::ResolveCurveConflictResponse {
            success: true,
            error_message: String::new(),
            result: None,
        };
        
        Ok(Response::new(response))
    }
    
    async fn one_click_resolve(
        &self,
        request: Request<crate::proto::OneClickResolveRequest>,
    ) -> Result<Response<crate::proto::OneClickResolveResponse>, Status> {
        let request = request.into_inner();
        
        debug!("One-click resolve request for axis: {}", request.axis_name);
        
        // Mock implementation - in real implementation this would delegate to curve conflict service
        let response = crate::proto::OneClickResolveResponse {
            success: true,
            error_message: String::new(),
            result: None, // Would contain OneClickResult in real implementation
        };
        
        Ok(Response::new(response))
    }
    
    async fn set_capability_mode(
        &self,
        request: Request<crate::proto::SetCapabilityModeRequest>,
    ) -> Result<Response<crate::proto::SetCapabilityModeResponse>, Status> {
        let request = request.into_inner();
        
        debug!(
            "Set capability mode request: mode={:?}, axes={:?}",
            request.mode, request.axis_names
        );
        
        // Mock implementation - in real implementation this would delegate to capability service
        let response = crate::proto::SetCapabilityModeResponse {
            success: true,
            error_message: String::new(),
            affected_axes: request.axis_names.clone(),
            applied_limits: Some(crate::proto::CapabilityLimits {
                max_axis_output: 1.0,
                max_ffb_torque: 50.0,
                max_slew_rate: 100.0,
                max_curve_expo: 1.0,
                allow_high_torque: true,
                allow_custom_curves: true,
            }),
        };
        
        Ok(Response::new(response))
    }
    
    async fn get_capability_mode(
        &self,
        request: Request<crate::proto::GetCapabilityModeRequest>,
    ) -> Result<Response<crate::proto::GetCapabilityModeResponse>, Status> {
        let request = request.into_inner();
        
        debug!("Get capability mode request for axes: {:?}", request.axis_names);
        
        // Mock implementation - in real implementation this would delegate to capability service
        let response = crate::proto::GetCapabilityModeResponse {
            success: true,
            error_message: String::new(),
            axis_status: vec![], // Would contain actual axis status in real implementation
        };
        
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
        capabilities.insert("telemetry_enabled".to_string(), telemetry_config.enabled.to_string());
        capabilities.insert("security_enforced".to_string(), "true".to_string());
        
        let response = GetServiceInfoResponse {
            version: self.config.server_version.clone(),
            uptime_seconds: uptime,
            status: ServiceStatus::Running.into(),
            capabilities,
        };
        
        Ok(Response::new(response))
    }
    
    /// Get security status and plugin information
    async fn get_security_status(
        &self,
        _request: Request<crate::proto::GetSecurityStatusRequest>,
    ) -> Result<Response<crate::proto::GetSecurityStatusResponse>, Status> {
        let security_manager = self.security_manager.read().await;
        
        let plugin_registry = security_manager.get_plugin_registry();
        let mut plugins = Vec::new();
        
        for (name, manifest) in plugin_registry {
            plugins.push(crate::proto::PluginInfo {
                name: name.clone(),
                version: manifest.version.clone(),
                plugin_type: match manifest.plugin_type {
                    flight_core::PluginType::Wasm => crate::proto::PluginType::Wasm.into(),
                    flight_core::PluginType::Native => crate::proto::PluginType::Native.into(),
                },
                signature_status: match &manifest.signature {
                    flight_core::SignatureStatus::Signed { issuer, .. } => {
                        format!("Signed by {}", issuer)
                    }
                    flight_core::SignatureStatus::Unsigned => "Unsigned".to_string(),
                    flight_core::SignatureStatus::Invalid { reason } => {
                        format!("Invalid: {}", reason)
                    }
                },
                capabilities: manifest.capabilities.iter()
                    .map(|cap| format!("{:?}", cap))
                    .collect(),
            });
        }
        
        let telemetry_config = security_manager.get_telemetry_config();
        
        let response = crate::proto::GetSecurityStatusResponse {
            success: true,
            error_message: String::new(),
            plugins,
            telemetry_enabled: telemetry_config.enabled,
            telemetry_data_types: telemetry_config.collected_data.iter()
                .map(|dt| format!("{:?}", dt))
                .collect(),
        };
        
        Ok(Response::new(response))
    }
    
    /// Configure telemetry collection
    async fn configure_telemetry(
        &self,
        request: Request<crate::proto::ConfigureTelemetryRequest>,
    ) -> Result<Response<crate::proto::ConfigureTelemetryResponse>, Status> {
        let request = request.into_inner();
        let mut security_manager = self.security_manager.write().await;
        
        if request.enabled {
            // Convert string data types to enum
            let mut data_types = std::collections::HashSet::new();
            for dt_str in &request.data_types {
                match dt_str.as_str() {
                    "Performance" => { data_types.insert(TelemetryDataType::Performance); }
                    "Errors" => { data_types.insert(TelemetryDataType::Errors); }
                    "Usage" => { data_types.insert(TelemetryDataType::Usage); }
                    "DeviceEvents" => { data_types.insert(TelemetryDataType::DeviceEvents); }
                    "ProfileEvents" => { data_types.insert(TelemetryDataType::ProfileEvents); }
                    _ => {
                        return Err(Status::invalid_argument(format!("Unknown data type: {}", dt_str)));
                    }
                }
            }
            
            security_manager.enable_telemetry(data_types)
                .map_err(|e| Status::internal(format!("Failed to enable telemetry: {}", e)))?;
        } else {
            security_manager.disable_telemetry();
        }
        
        let response = crate::proto::ConfigureTelemetryResponse {
            success: true,
            error_message: String::new(),
        };
        
        Ok(Response::new(response))
    }
    
    /// Get redacted support bundle data
    async fn get_support_bundle(
        &self,
        _request: Request<crate::proto::GetSupportBundleRequest>,
    ) -> Result<Response<crate::proto::GetSupportBundleResponse>, Status> {
        let security_manager = self.security_manager.read().await;
        let redacted_data = security_manager.get_redacted_support_data();
        
        // Convert HashMap to JSON string
        let data_json = serde_json::to_string(&redacted_data)
            .map_err(|e| Status::internal(format!("Failed to serialize support data: {}", e)))?;
        
        let bundle_size = data_json.len() as u64;
        
        let response = crate::proto::GetSupportBundleResponse {
            success: true,
            error_message: String::new(),
            redacted_data: data_json,
            bundle_size_bytes: bundle_size,
        };
        
        Ok(Response::new(response))
    }
}

/// Flight Hub IPC server
pub struct FlightServer {
    service: FlightServiceImpl,
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
        let addr = "127.0.0.1:50051".parse()?; // For development
        
        info!("Starting Flight Hub IPC server on {}", addr);
        
        Server::builder()
            .add_service(FlightServiceServer::new(self.service))
            .serve(addr)
            .await?;
        
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
        assert!(response.enabled_features.contains(&"device-management".to_string()));
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