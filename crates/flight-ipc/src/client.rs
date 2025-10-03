// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! IPC client implementation

use crate::{
    negotiation::{validate_required_features, Version},
    proto::{
        flight_service_client::FlightServiceClient, GetServiceInfoRequest, HealthSubscribeRequest,
        ListDevicesRequest, NegotiateFeaturesRequest,
    },
    ClientConfig, IpcError, NegotiationResult,
};
use anyhow::Result;
use std::time::Duration;
use tonic::transport::{Channel, Endpoint};
use tracing::{debug, info};

/// Flight Hub IPC client
pub struct FlightClient {
    client: FlightServiceClient<Channel>,
    config: ClientConfig,
    negotiation_result: Option<NegotiationResult>,
}

impl FlightClient {
    /// Create a new client with default configuration
    pub async fn connect() -> Result<Self, IpcError> {
        Self::connect_with_config(ClientConfig::default()).await
    }
    
    /// Create a new client with custom configuration
    pub async fn connect_with_config(config: ClientConfig) -> Result<Self, IpcError> {
        let channel = Self::create_channel(&config).await?;
        let client = FlightServiceClient::new(channel);
        
        let mut flight_client = Self {
            client,
            config,
            negotiation_result: None,
        };
        
        // Perform feature negotiation
        flight_client.negotiate_features().await?;
        
        Ok(flight_client)
    }
    
    /// Create transport channel based on configuration
    async fn create_channel(config: &ClientConfig) -> Result<Channel, IpcError> {
        let _address = crate::default_bind_address();
        
        // For now, use a simple TCP connection for development
        // In production, this would use the actual transport layer
        let endpoint = Endpoint::from_static("http://127.0.0.1:50051")
            .timeout(Duration::from_millis(config.connection_timeout_ms));
        
        let channel = endpoint.connect().await.map_err(|e| IpcError::ConnectionFailed {
            reason: format!("Failed to connect: {}", e),
        })?;
        
        Ok(channel)
    }
    
    /// Negotiate features with the server
    async fn negotiate_features(&mut self) -> Result<(), IpcError> {
        let request = NegotiateFeaturesRequest {
            client_version: self.config.client_version.clone(),
            supported_features: self.config.supported_features.clone(),
            preferred_transport: self.config.preferred_transport.into(),
        };
        
        debug!("Negotiating features with server");
        
        let response = self
            .client
            .negotiate_features(request)
            .await?
            .into_inner();
        
        if !response.success {
            return Err(IpcError::ConnectionFailed {
                reason: response.error_message,
            });
        }
        
        // Validate version compatibility
        let server_version = Version::parse(&response.server_version)?;
        let client_version = Version::parse(&self.config.client_version)?;
        
        if !server_version.is_compatible_with(&client_version) {
            return Err(IpcError::VersionMismatch {
                client: self.config.client_version.clone(),
                server: response.server_version,
            });
        }
        
        self.negotiation_result = Some(NegotiationResult {
            server_version: response.server_version.clone(),
            enabled_features: response.enabled_features.clone(),
            transport_type: response.negotiated_transport(),
        });
        
        info!(
            "Feature negotiation successful. Enabled features: {:?}",
            self.negotiation_result.as_ref().unwrap().enabled_features
        );
        
        Ok(())
    }
    
    /// Get the negotiation result
    pub fn negotiation_result(&self) -> Option<&NegotiationResult> {
        self.negotiation_result.as_ref()
    }
    

    
    /// Subscribe to health events
    pub async fn subscribe_health(
        &mut self,
    ) -> Result<tonic::Streaming<crate::proto::HealthEvent>, IpcError> {
        self.require_feature("health-monitoring")?;
        
        let request = HealthSubscribeRequest {
            filter_types: vec![],
            device_ids: vec![],
            include_performance_metrics: true,
        };
        
        let response = self.client.health_subscribe(request).await?.into_inner();
        
        Ok(response)
    }
    
    /// Get service information
    pub async fn get_service_info(&mut self) -> Result<crate::proto::GetServiceInfoResponse, IpcError> {
        let request = GetServiceInfoRequest {};
        
        let response = self.client.get_service_info(request).await?.into_inner();
        
        Ok(response)
    }
    
    /// Apply a profile
    pub async fn apply_profile(&mut self, request: crate::proto::ApplyProfileRequest) -> Result<crate::proto::ApplyProfileResponse, IpcError> {
        self.require_feature("profile-management")?;
        
        let response = self.client.apply_profile(request).await?.into_inner();
        
        Ok(response)
    }
    
    /// List devices with full request parameters
    pub async fn list_devices(&mut self, request: crate::proto::ListDevicesRequest) -> Result<crate::proto::ListDevicesResponse, IpcError> {
        self.require_feature("device-management")?;
        
        let response = self.client.list_devices(request).await?.into_inner();
        
        Ok(response)
    }
    
    /// Detect curve conflicts
    pub async fn detect_curve_conflicts(&mut self, request: crate::proto::DetectCurveConflictsRequest) -> Result<crate::proto::DetectCurveConflictsResponse, IpcError> {
        self.require_feature("profile-management")?;
        
        let response = self.client.detect_curve_conflicts(request).await?.into_inner();
        
        Ok(response)
    }
    
    /// Resolve curve conflict
    pub async fn resolve_curve_conflict(&mut self, request: crate::proto::ResolveCurveConflictRequest) -> Result<crate::proto::ResolveCurveConflictResponse, IpcError> {
        self.require_feature("profile-management")?;
        
        let response = self.client.resolve_curve_conflict(request).await?.into_inner();
        
        Ok(response)
    }
    
    /// One-click resolve curve conflict
    pub async fn one_click_resolve(&mut self, request: crate::proto::OneClickResolveRequest) -> Result<crate::proto::OneClickResolveResponse, IpcError> {
        self.require_feature("profile-management")?;
        
        let response = self.client.one_click_resolve(request).await?.into_inner();
        
        Ok(response)
    }
    
    /// Set capability mode
    pub async fn set_capability_mode(&mut self, request: crate::proto::SetCapabilityModeRequest) -> Result<crate::proto::SetCapabilityModeResponse, IpcError> {
        self.require_feature("force-feedback")?;
        
        let response = self.client.set_capability_mode(request).await?.into_inner();
        
        Ok(response)
    }
    
    /// Get capability mode
    pub async fn get_capability_mode(&mut self, request: crate::proto::GetCapabilityModeRequest) -> Result<crate::proto::GetCapabilityModeResponse, IpcError> {
        self.require_feature("force-feedback")?;
        
        let response = self.client.get_capability_mode(request).await?.into_inner();
        
        Ok(response)
    }
    
    /// Validate that a required feature is enabled
    fn require_feature(&self, feature: &str) -> Result<(), IpcError> {
        if let Some(negotiation) = &self.negotiation_result {
            validate_required_features(&negotiation.enabled_features, &[feature.to_string()])?;
        } else {
            return Err(IpcError::ConnectionFailed {
                reason: "Feature negotiation not completed".to_string(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // Note: These tests would require a running server
    // In practice, you'd use mock servers or integration test setup
    
    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_client_connection() {
        let client = FlightClient::connect().await;
        assert!(client.is_ok());
    }
}