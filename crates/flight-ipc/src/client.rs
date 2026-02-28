// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! IPC client implementation
//!
//! Wraps the tonic-generated [`FlightServiceClient`] with ergonomic helpers,
//! automatic reconnection with exponential back-off, and per-call timeouts.

use crate::{
    ClientConfig, IpcError,
    proto::{
        ApplyProfileRequest, ApplyProfileResponse, DetectCurveConflictsRequest,
        DetectCurveConflictsResponse, GetServiceInfoRequest, GetServiceInfoResponse,
        ListDevicesRequest, ListDevicesResponse, NegotiateFeaturesRequest, OneClickResolveRequest,
        OneClickResolveResponse, ResolveCurveConflictRequest, ResolveCurveConflictResponse,
        SetCapabilityModeRequest, SetCapabilityModeResponse,
        flight_service_client::FlightServiceClient as GrpcClient,
    },
};
use std::time::Duration;
use tracing::{debug, info, warn};

/// Maximum number of reconnection attempts before giving up.
const MAX_RECONNECT_ATTEMPTS: u32 = 5;

/// Base delay between reconnection attempts (doubled each attempt).
const RECONNECT_BASE_DELAY: Duration = Duration::from_millis(100);

/// Flight Hub IPC client with automatic reconnection.
pub struct IpcClient {
    inner: GrpcClient<tonic::transport::Channel>,
    endpoint: tonic::transport::Endpoint,
    config: ClientConfig,
}

impl IpcClient {
    /// Connect to the gRPC server at `addr` (e.g. `"http://127.0.0.1:50051"`).
    pub async fn connect(addr: &str) -> Result<Self, IpcError> {
        Self::connect_with_config(addr, ClientConfig::default()).await
    }

    /// Connect with custom configuration.
    pub async fn connect_with_config(addr: &str, config: ClientConfig) -> Result<Self, IpcError> {
        let endpoint = tonic::transport::Endpoint::from_shared(addr.to_string())
            .map_err(|e| IpcError::ConnectionFailed {
                reason: format!("Invalid endpoint: {e}"),
            })?
            .timeout(Duration::from_millis(config.connection_timeout_ms))
            .connect_timeout(Duration::from_millis(config.connection_timeout_ms));

        let channel = endpoint
            .connect()
            .await
            .map_err(|e| IpcError::ConnectionFailed {
                reason: format!("Failed to connect to {addr}: {e}"),
            })?;

        let inner = GrpcClient::new(channel);
        info!("Connected to Flight IPC server at {addr}");

        Ok(Self {
            inner,
            endpoint,
            config,
        })
    }

    /// Re-establish the connection using exponential back-off.
    async fn reconnect(&mut self) -> Result<(), IpcError> {
        let mut delay = RECONNECT_BASE_DELAY;

        for attempt in 1..=MAX_RECONNECT_ATTEMPTS {
            debug!("Reconnection attempt {attempt}/{MAX_RECONNECT_ATTEMPTS}");
            tokio::time::sleep(delay).await;

            match self.endpoint.connect().await {
                Ok(channel) => {
                    self.inner = GrpcClient::new(channel);
                    info!("Reconnected on attempt {attempt}");
                    return Ok(());
                }
                Err(e) => {
                    warn!("Reconnect attempt {attempt} failed: {e}");
                    delay = delay.saturating_mul(2);
                }
            }
        }

        Err(IpcError::ConnectionFailed {
            reason: format!("Failed to reconnect after {MAX_RECONNECT_ATTEMPTS} attempts"),
        })
    }

    // ------------------------------------------------------------------
    // Device service
    // ------------------------------------------------------------------

    /// List connected devices.
    pub async fn list_devices(&mut self) -> Result<ListDevicesResponse, IpcError> {
        let resp = self.inner.list_devices(ListDevicesRequest::default()).await;

        match resp {
            Ok(r) => Ok(r.into_inner()),
            Err(status) if Self::is_connection_error(&status) => {
                self.reconnect().await?;
                Ok(self
                    .inner
                    .list_devices(ListDevicesRequest::default())
                    .await?
                    .into_inner())
            }
            Err(e) => Err(IpcError::Grpc(e)),
        }
    }

    /// Get a single device by ID (filters the full device list).
    pub async fn get_device(&mut self, id: &str) -> Result<Option<crate::proto::Device>, IpcError> {
        let resp = self.list_devices().await?;
        Ok(resp.devices.into_iter().find(|d| d.id == id))
    }

    // ------------------------------------------------------------------
    // Profile service
    // ------------------------------------------------------------------

    /// Apply a profile.
    pub async fn apply_profile(
        &mut self,
        request: ApplyProfileRequest,
    ) -> Result<ApplyProfileResponse, IpcError> {
        Ok(self.inner.apply_profile(request).await?.into_inner())
    }

    // ------------------------------------------------------------------
    // Diagnostics / system service
    // ------------------------------------------------------------------

    /// Get service info (version, uptime, status).
    pub async fn get_service_info(&mut self) -> Result<GetServiceInfoResponse, IpcError> {
        Ok(self
            .inner
            .get_service_info(GetServiceInfoRequest {})
            .await?
            .into_inner())
    }

    // ------------------------------------------------------------------
    // Conflict resolution
    // ------------------------------------------------------------------

    /// Detect curve conflicts.
    pub async fn detect_curve_conflicts(
        &mut self,
        request: DetectCurveConflictsRequest,
    ) -> Result<DetectCurveConflictsResponse, IpcError> {
        Ok(self
            .inner
            .detect_curve_conflicts(request)
            .await?
            .into_inner())
    }

    /// Resolve a single curve conflict.
    pub async fn resolve_curve_conflict(
        &mut self,
        request: ResolveCurveConflictRequest,
    ) -> Result<ResolveCurveConflictResponse, IpcError> {
        Ok(self
            .inner
            .resolve_curve_conflict(request)
            .await?
            .into_inner())
    }

    /// One-click resolve.
    pub async fn one_click_resolve(
        &mut self,
        request: OneClickResolveRequest,
    ) -> Result<OneClickResolveResponse, IpcError> {
        Ok(self.inner.one_click_resolve(request).await?.into_inner())
    }

    /// Set capability mode.
    pub async fn set_capability_mode(
        &mut self,
        request: SetCapabilityModeRequest,
    ) -> Result<SetCapabilityModeResponse, IpcError> {
        Ok(self.inner.set_capability_mode(request).await?.into_inner())
    }

    /// Negotiate features with the server.
    pub async fn negotiate_features(
        &mut self,
    ) -> Result<crate::proto::NegotiateFeaturesResponse, IpcError> {
        let request = NegotiateFeaturesRequest {
            client_version: self.config.client_version.clone(),
            supported_features: self.config.supported_features.clone(),
            preferred_transport: self.config.preferred_transport.into(),
        };
        Ok(self.inner.negotiate_features(request).await?.into_inner())
    }

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    fn is_connection_error(status: &tonic::Status) -> bool {
        matches!(
            status.code(),
            tonic::Code::Unavailable | tonic::Code::Unknown
        )
    }
}

// ---------------------------------------------------------------------------
// Legacy FlightClient wrapper — kept for backward compatibility
// ---------------------------------------------------------------------------

/// Legacy client. Prefer [`IpcClient`] for new code.
pub struct FlightClient {
    inner: IpcClient,
}

impl FlightClient {
    /// Connect using defaults.
    pub async fn connect() -> Result<Self, IpcError> {
        let inner = IpcClient::connect("http://127.0.0.1:50051").await?;
        Ok(Self { inner })
    }

    /// List devices.
    pub async fn list_devices(
        &mut self,
        _request: ListDevicesRequest,
    ) -> Result<ListDevicesResponse, IpcError> {
        self.inner.list_devices().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connect_refused() {
        // Connecting to a port where nothing is listening should fail.
        let result = IpcClient::connect("http://127.0.0.1:19999").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("Failed to connect") || msg.contains("Connection"));
    }
}
