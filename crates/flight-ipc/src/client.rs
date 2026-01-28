// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! IPC client implementation

use crate::{
    ClientConfig, IpcError, NegotiationResult,
    negotiation::{Version, validate_required_features},
    proto::{
        ApplyProfileRequest, ApplyProfileResponse, DetectCurveConflictsRequest,
        DetectCurveConflictsResponse, ListDevicesRequest, ListDevicesResponse,
        NegotiateFeaturesRequest, OneClickResolveRequest, OneClickResolveResponse,
        ResolveCurveConflictRequest, ResolveCurveConflictResponse, SetCapabilityModeRequest,
        SetCapabilityModeResponse,
    },
};
use anyhow::Result;
use std::time::Duration;
use tracing::{debug, info};

/// Flight Hub IPC client
pub struct FlightClient {
    _channel: tonic::transport::Channel,
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

        let mut flight_client = Self {
            _channel: channel,
            config,
            negotiation_result: None,
        };

        // Perform feature negotiation
        flight_client.negotiate_features().await?;

        Ok(flight_client)
    }

    /// Create transport channel based on configuration
    async fn create_channel(config: &ClientConfig) -> Result<tonic::transport::Channel, IpcError> {
        let _address = crate::default_bind_address();

        // For now, use a simple TCP connection for development
        // In production, this would use the actual transport layer
        let endpoint = tonic::transport::Endpoint::from_static("http://127.0.0.1:50051")
            .timeout(Duration::from_millis(config.connection_timeout_ms));

        let channel = endpoint
            .connect()
            .await
            .map_err(|e| IpcError::ConnectionFailed {
                reason: format!("Failed to connect: {}", e),
            })?;

        Ok(channel)
    }

    /// Negotiate features with the server
    async fn negotiate_features(&mut self) -> Result<(), IpcError> {
        let _request = NegotiateFeaturesRequest {
            client_version: self.config.client_version.clone(),
            supported_features: self.config.supported_features.clone(),
            preferred_transport: self.config.preferred_transport.into(),
        };

        debug!("Negotiating features with server");

        // Placeholder for gRPC call - in real implementation this would use generated client
        let response = crate::proto::NegotiateFeaturesResponse {
            success: true,
            server_version: "1.0.0".to_string(),
            enabled_features: self.config.supported_features.clone(),
            negotiated_transport: self.config.preferred_transport.into(),
            error_message: String::new(),
        };

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

    /// Subscribe to health events (placeholder implementation)
    pub async fn subscribe_health(&mut self) -> Result<(), IpcError> {
        self.require_feature("health-monitoring")?;

        // Placeholder - in real implementation this would make gRPC call
        info!("Health subscription requested");

        Ok(())
    }

    /// Get service information (placeholder implementation)
    pub async fn get_service_info(
        &mut self,
    ) -> Result<crate::proto::GetServiceInfoResponse, IpcError> {
        // Placeholder - in real implementation this would make gRPC call
        let response = crate::proto::GetServiceInfoResponse {
            version: "1.0.0".to_string(),
            uptime_seconds: 0,
            status: crate::proto::ServiceStatus::Running.into(),
            capabilities: std::collections::HashMap::new(),
        };

        Ok(response)
    }

    /// List connected devices (placeholder implementation)
    pub async fn list_devices(
        &mut self,
        _request: ListDevicesRequest,
    ) -> Result<ListDevicesResponse, IpcError> {
        Ok(ListDevicesResponse {
            devices: vec![],
            total_count: 0,
        })
    }

    /// Resolve conflicts (placeholder implementation)
    pub async fn one_click_resolve(
        &mut self,
        _request: OneClickResolveRequest,
    ) -> Result<OneClickResolveResponse, IpcError> {
        Ok(OneClickResolveResponse {
            success: false,
            error_message: "Not implemented".to_string(),
            result: None,
        })
    }

    /// Set capability mode (placeholder implementation)
    pub async fn set_capability_mode(
        &mut self,
        _request: SetCapabilityModeRequest,
    ) -> Result<SetCapabilityModeResponse, IpcError> {
        Ok(SetCapabilityModeResponse {
            success: false,
            error_message: "Not implemented".to_string(),
            affected_axes: vec![],
            applied_limits: None,
        })
    }

    /// Detect curve conflicts (placeholder implementation)
    pub async fn detect_curve_conflicts(
        &mut self,
        _request: DetectCurveConflictsRequest,
    ) -> Result<DetectCurveConflictsResponse, IpcError> {
        Ok(DetectCurveConflictsResponse {
            success: false,
            error_message: "Not implemented".to_string(),
            conflicts: vec![],
        })
    }

    /// Resolve curve conflict (placeholder implementation)
    pub async fn resolve_curve_conflict(
        &mut self,
        _request: ResolveCurveConflictRequest,
    ) -> Result<ResolveCurveConflictResponse, IpcError> {
        Ok(ResolveCurveConflictResponse {
            success: false,
            error_message: "Not implemented".to_string(),
            result: None,
        })
    }

    /// Apply profile (placeholder implementation)
    pub async fn apply_profile(
        &mut self,
        _request: ApplyProfileRequest,
    ) -> Result<ApplyProfileResponse, IpcError> {
        Ok(ApplyProfileResponse {
            success: false,
            error_message: "Not implemented".to_string(),
            validation_errors: vec![],
            effective_profile_hash: String::new(),
            compile_time_ms: 0,
        })
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
