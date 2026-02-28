// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! IPC client implementation
//!
//! Wraps the tonic-generated [`FlightServiceClient`] with ergonomic helpers,
//! automatic reconnection with exponential back-off, and per-call timeouts.

use crate::{
    ClientConfig, IpcError,
    proto::{
        ApplyProfileRequest, ApplyProfileResponse, ConfigureTelemetryRequest,
        ConfigureTelemetryResponse, DetectCurveConflictsRequest, DetectCurveConflictsResponse,
        GetCapabilityModeRequest, GetCapabilityModeResponse, GetSecurityStatusRequest,
        GetSecurityStatusResponse, GetServiceInfoRequest, GetServiceInfoResponse,
        GetSupportBundleRequest, GetSupportBundleResponse, ListDevicesRequest, ListDevicesResponse,
        NegotiateFeaturesRequest, OneClickResolveRequest, OneClickResolveResponse,
        ResolveCurveConflictRequest, ResolveCurveConflictResponse, SetCapabilityModeRequest,
        SetCapabilityModeResponse, flight_service_client::FlightServiceClient as GrpcClient,
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

impl std::fmt::Debug for IpcClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IpcClient")
            .field("config", &self.config)
            .finish()
    }
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

    /// Disconnect from the server. The client cannot be used after this call
    /// without calling [`reconnect`](Self::reconnect).
    pub async fn disconnect(&mut self) {
        // Dropping the inner client closes the underlying channel.
        // Re-create a stub pointing at an invalid channel so subsequent
        // calls fail fast rather than using a stale connection.
        if let Ok(endpoint) = tonic::transport::Endpoint::from_shared("http://[::1]:0") {
            // Best-effort: create a lazy channel that will fail on first use.
            let channel = endpoint.connect_lazy();
            self.inner = GrpcClient::new(channel);
        }
        debug!("Client disconnected");
    }

    /// Check whether the server is reachable by issuing a lightweight RPC.
    pub async fn is_connected(&mut self) -> bool {
        self.get_service_info().await.is_ok()
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

    /// Get capability mode.
    pub async fn get_capability_mode(
        &mut self,
        request: GetCapabilityModeRequest,
    ) -> Result<GetCapabilityModeResponse, IpcError> {
        Ok(self.inner.get_capability_mode(request).await?.into_inner())
    }

    /// Get security status.
    pub async fn get_security_status(&mut self) -> Result<GetSecurityStatusResponse, IpcError> {
        Ok(self
            .inner
            .get_security_status(GetSecurityStatusRequest {})
            .await?
            .into_inner())
    }

    /// Configure telemetry settings.
    pub async fn configure_telemetry(
        &mut self,
        request: ConfigureTelemetryRequest,
    ) -> Result<ConfigureTelemetryResponse, IpcError> {
        Ok(self.inner.configure_telemetry(request).await?.into_inner())
    }

    /// Get a redacted support bundle for diagnostics.
    pub async fn get_support_bundle(&mut self) -> Result<GetSupportBundleResponse, IpcError> {
        Ok(self
            .inner
            .get_support_bundle(GetSupportBundleRequest {})
            .await?
            .into_inner())
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

    /// Connect with custom configuration.
    pub async fn connect_with_config(config: ClientConfig) -> Result<Self, IpcError> {
        let addr = "http://127.0.0.1:50051";
        let inner = IpcClient::connect_with_config(addr, config).await?;
        Ok(Self { inner })
    }

    /// List devices.
    pub async fn list_devices(
        &mut self,
        _request: ListDevicesRequest,
    ) -> Result<ListDevicesResponse, IpcError> {
        self.inner.list_devices().await
    }

    /// Get service info.
    pub async fn get_service_info(&mut self) -> Result<GetServiceInfoResponse, IpcError> {
        self.inner.get_service_info().await
    }

    /// Apply profile.
    pub async fn apply_profile(
        &mut self,
        request: ApplyProfileRequest,
    ) -> Result<ApplyProfileResponse, IpcError> {
        self.inner.apply_profile(request).await
    }

    /// One-click resolve.
    pub async fn one_click_resolve(
        &mut self,
        request: OneClickResolveRequest,
    ) -> Result<OneClickResolveResponse, IpcError> {
        self.inner.one_click_resolve(request).await
    }

    /// Set capability mode.
    pub async fn set_capability_mode(
        &mut self,
        request: SetCapabilityModeRequest,
    ) -> Result<SetCapabilityModeResponse, IpcError> {
        self.inner.set_capability_mode(request).await
    }

    /// Detect curve conflicts.
    pub async fn detect_curve_conflicts(
        &mut self,
        request: DetectCurveConflictsRequest,
    ) -> Result<DetectCurveConflictsResponse, IpcError> {
        self.inner.detect_curve_conflicts(request).await
    }

    /// Resolve curve conflict.
    pub async fn resolve_curve_conflict(
        &mut self,
        request: ResolveCurveConflictRequest,
    ) -> Result<ResolveCurveConflictResponse, IpcError> {
        self.inner.resolve_curve_conflict(request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::IpcServer;
    use std::net::SocketAddr;

    /// Start a mock server on an ephemeral port and return (addr, handle).
    async fn start_test_server() -> (SocketAddr, crate::server::ServerHandle) {
        let config = crate::ServerConfig::default();
        let server = IpcServer::new_mock(config);
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let handle = server.start(addr).await.unwrap();
        (handle.addr(), handle)
    }

    // -----------------------------------------------------------------------
    // Connection tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_connect_refused() {
        let result = IpcClient::connect("http://127.0.0.1:19999").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("Failed to connect") || msg.contains("Connection"));
    }

    #[tokio::test]
    async fn test_connect_to_running_server() {
        let (addr, handle) = start_test_server().await;
        let result = IpcClient::connect(&format!("http://{addr}")).await;
        assert!(result.is_ok());
        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_connect_with_custom_config() {
        let (addr, handle) = start_test_server().await;
        let config = ClientConfig {
            connection_timeout_ms: 2000,
            ..ClientConfig::default()
        };
        let result = IpcClient::connect_with_config(&format!("http://{addr}"), config).await;
        assert!(result.is_ok());
        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_disconnect_then_reconnect() {
        let (addr, handle) = start_test_server().await;
        let addr_str = format!("http://{addr}");
        let mut client = IpcClient::connect(&addr_str).await.unwrap();

        // Should work before disconnect
        assert!(client.get_service_info().await.is_ok());

        client.disconnect().await;

        // After disconnect, calls should fail
        assert!(client.get_service_info().await.is_err());

        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_is_connected_true() {
        let (addr, handle) = start_test_server().await;
        let mut client = IpcClient::connect(&format!("http://{addr}")).await.unwrap();
        assert!(client.is_connected().await);
        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_connect_invalid_address() {
        let result = IpcClient::connect("not-a-valid-url").await;
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // RPC endpoint tests (against running mock server)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_list_devices() {
        let (addr, handle) = start_test_server().await;
        let mut client = IpcClient::connect(&format!("http://{addr}")).await.unwrap();

        let resp = client.list_devices().await.unwrap();
        assert_eq!(resp.total_count, 0);
        assert!(resp.devices.is_empty());

        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_get_device_not_found() {
        let (addr, handle) = start_test_server().await;
        let mut client = IpcClient::connect(&format!("http://{addr}")).await.unwrap();

        let device = client.get_device("nonexistent").await.unwrap();
        assert!(device.is_none());

        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_get_service_info() {
        let (addr, handle) = start_test_server().await;
        let mut client = IpcClient::connect(&format!("http://{addr}")).await.unwrap();

        let info = client.get_service_info().await.unwrap();
        assert_eq!(info.version, crate::PROTOCOL_VERSION);

        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_apply_profile() {
        let (addr, handle) = start_test_server().await;
        let mut client = IpcClient::connect(&format!("http://{addr}")).await.unwrap();

        let resp = client
            .apply_profile(ApplyProfileRequest {
                profile_json: "{}".to_string(),
                validate_only: true,
                force_apply: false,
            })
            .await
            .unwrap();
        assert!(resp.success);

        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_negotiate_features() {
        let (addr, handle) = start_test_server().await;
        let mut client = IpcClient::connect(&format!("http://{addr}")).await.unwrap();

        let resp = client.negotiate_features().await.unwrap();
        assert!(resp.success);

        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_detect_curve_conflicts() {
        let (addr, handle) = start_test_server().await;
        let mut client = IpcClient::connect(&format!("http://{addr}")).await.unwrap();

        let resp = client
            .detect_curve_conflicts(DetectCurveConflictsRequest::default())
            .await
            .unwrap();
        assert!(resp.success);

        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_get_security_status() {
        let (addr, handle) = start_test_server().await;
        let mut client = IpcClient::connect(&format!("http://{addr}")).await.unwrap();

        let resp = client.get_security_status().await.unwrap();
        assert!(resp.success);

        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_get_support_bundle() {
        let (addr, handle) = start_test_server().await;
        let mut client = IpcClient::connect(&format!("http://{addr}")).await.unwrap();

        let resp = client.get_support_bundle().await.unwrap();
        assert!(resp.success);
        assert!(!resp.redacted_data.is_empty());

        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_configure_telemetry() {
        let (addr, handle) = start_test_server().await;
        let mut client = IpcClient::connect(&format!("http://{addr}")).await.unwrap();

        let resp = client
            .configure_telemetry(ConfigureTelemetryRequest {
                enabled: true,
                data_types: vec!["Performance".to_string()],
            })
            .await
            .unwrap();
        assert!(resp.success);

        handle.shutdown().await.unwrap();
    }

    // -----------------------------------------------------------------------
    // Concurrent request tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_multiple_concurrent_requests() {
        let (addr, handle) = start_test_server().await;
        let addr_str = format!("http://{addr}");

        // Spawn multiple concurrent clients
        let mut tasks = Vec::new();
        for _ in 0..5 {
            let addr_clone = addr_str.clone();
            tasks.push(tokio::spawn(async move {
                let mut client = IpcClient::connect(&addr_clone).await.unwrap();
                let info = client.get_service_info().await.unwrap();
                assert_eq!(info.version, crate::PROTOCOL_VERSION);
            }));
        }

        for task in tasks {
            task.await.unwrap();
        }

        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_concurrent_different_rpcs() {
        let (addr, handle) = start_test_server().await;
        let addr_str = format!("http://{addr}");

        let a = addr_str.clone();
        let t1 = tokio::spawn(async move {
            let mut c = IpcClient::connect(&a).await.unwrap();
            c.list_devices().await.unwrap()
        });

        let a = addr_str.clone();
        let t2 = tokio::spawn(async move {
            let mut c = IpcClient::connect(&a).await.unwrap();
            c.get_service_info().await.unwrap()
        });

        let a = addr_str.clone();
        let t3 = tokio::spawn(async move {
            let mut c = IpcClient::connect(&a).await.unwrap();
            c.get_security_status().await.unwrap()
        });

        let (r1, r2, r3) = tokio::join!(t1, t2, t3);
        assert!(r1.unwrap().devices.is_empty());
        assert_eq!(r2.unwrap().version, crate::PROTOCOL_VERSION);
        assert!(r3.unwrap().success);

        handle.shutdown().await.unwrap();
    }

    // -----------------------------------------------------------------------
    // Error handling tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_error_after_server_shutdown() {
        let (addr, handle) = start_test_server().await;
        let mut client = IpcClient::connect(&format!("http://{addr}")).await.unwrap();

        // Works before shutdown
        assert!(client.get_service_info().await.is_ok());

        handle.shutdown().await.unwrap();

        // Subsequent call should fail
        let result = client.get_service_info().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_debug_impl() {
        let (addr, handle) = start_test_server().await;
        let client = IpcClient::connect(&format!("http://{addr}")).await.unwrap();
        let debug = format!("{:?}", client);
        assert!(debug.contains("IpcClient"));
        handle.shutdown().await.unwrap();
    }
}
