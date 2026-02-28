// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Transport layer abstractions for cross-platform IPC

use anyhow::Result;
use flight_core::{IpcClientInfo, SecurityManager};
use std::path::PathBuf;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite};

// Conditionally import items only used in platform-specific modules
#[cfg(any(
    all(windows, feature = "named-pipes"),
    all(unix, feature = "unix-sockets")
))]
use std::pin::Pin;

#[cfg(any(
    all(windows, feature = "named-pipes"),
    all(unix, feature = "unix-sockets")
))]
use std::task::{Context, Poll};

#[cfg(any(
    all(windows, feature = "named-pipes"),
    all(unix, feature = "unix-sockets")
))]
use tokio::io::ReadBuf;

// ---------------------------------------------------------------------------
// Transport configuration — configures tonic channels & servers
// ---------------------------------------------------------------------------

/// Configuration for the IPC transport layer.
///
/// Centralises timeout, keepalive, and retry settings that are applied to both
/// the tonic [`Endpoint`](tonic::transport::Endpoint) (client side) and
/// [`Server`](tonic::transport::Server) (server side).
#[derive(Debug, Clone)]
pub struct TransportConfig {
    /// Timeout for establishing a new connection.
    pub connect_timeout: std::time::Duration,
    /// Per-request timeout applied to every RPC.
    pub request_timeout: std::time::Duration,
    /// Interval between HTTP/2 keepalive pings.
    pub keepalive_interval: std::time::Duration,
    /// How long to wait for a keepalive acknowledgement before considering the
    /// connection dead.
    pub keepalive_timeout: std::time::Duration,
    /// Retry policy for connection attempts and transient failures.
    pub retry_policy: RetryPolicy,
    /// Interval between background health probes (0 = disabled).
    pub health_check_interval: std::time::Duration,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            connect_timeout: std::time::Duration::from_secs(5),
            request_timeout: std::time::Duration::from_secs(5),
            keepalive_interval: std::time::Duration::from_secs(10),
            keepalive_timeout: std::time::Duration::from_secs(5),
            retry_policy: RetryPolicy::default(),
            health_check_interval: std::time::Duration::from_secs(15),
        }
    }
}

impl TransportConfig {
    /// Build a tonic [`Endpoint`] with the settings from this config.
    pub fn configure_endpoint(
        &self,
        addr: &str,
    ) -> Result<tonic::transport::Endpoint, TransportError> {
        let ep = tonic::transport::Endpoint::from_shared(addr.to_string())
            .map_err(|_| TransportError::InvalidAddress {
                address: addr.to_string(),
            })?
            .connect_timeout(self.connect_timeout)
            .timeout(self.request_timeout)
            .keep_alive_while_idle(true)
            .http2_keep_alive_interval(self.keepalive_interval)
            .keep_alive_timeout(self.keepalive_timeout);
        Ok(ep)
    }

    /// Apply keepalive and timeout settings to a tonic
    /// [`Server`](tonic::transport::Server) builder.
    pub fn configure_server(&self, builder: tonic::transport::Server) -> tonic::transport::Server {
        builder
            .timeout(self.request_timeout)
            .http2_keepalive_interval(Some(self.keepalive_interval))
            .http2_keepalive_timeout(Some(self.keepalive_timeout))
    }
}

// ---------------------------------------------------------------------------
// Retry policy
// ---------------------------------------------------------------------------

/// Exponential back-off retry policy for transient connection failures.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts before giving up.
    pub max_retries: u32,
    /// Initial delay before the first retry.
    pub base_delay: std::time::Duration,
    /// Upper bound on the delay between retries.
    pub max_delay: std::time::Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 5,
            base_delay: std::time::Duration::from_millis(100),
            max_delay: std::time::Duration::from_secs(5),
        }
    }
}

impl RetryPolicy {
    /// Compute the delay for attempt `n` (0-based) using capped exponential
    /// back-off.
    pub fn delay_for(&self, attempt: u32) -> std::time::Duration {
        let delay = self.base_delay.saturating_mul(1u32.wrapping_shl(attempt));
        std::cmp::min(delay, self.max_delay)
    }

    /// Execute `f` with retries according to this policy.
    ///
    /// Returns `Ok(T)` on the first successful attempt or the last error
    /// after all retries are exhausted.
    pub async fn retry<F, Fut, T, E>(&self, mut f: F) -> Result<T, E>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
    {
        let mut last_err: Option<E> = None;
        for attempt in 0..=self.max_retries {
            match f().await {
                Ok(val) => return Ok(val),
                Err(e) => {
                    last_err = Some(e);
                    if attempt < self.max_retries {
                        tokio::time::sleep(self.delay_for(attempt)).await;
                    }
                }
            }
        }
        Err(last_err.expect("at least one attempt was made"))
    }
}

// ---------------------------------------------------------------------------
// Health monitor
// ---------------------------------------------------------------------------

/// Background connection health monitor.
///
/// Periodically probes the remote endpoint and reports health via a
/// [`tokio::sync::watch`] channel.
#[derive(Debug)]
pub struct HealthMonitor {
    healthy: tokio::sync::watch::Receiver<bool>,
    _cancel: tokio_util::sync::DropGuard,
}

impl HealthMonitor {
    /// Spawn a health-monitoring task that calls `probe` at `interval`.
    ///
    /// The probe function should return `true` when the connection is healthy.
    /// The monitor stops automatically when the returned [`HealthMonitor`] is
    /// dropped.
    pub fn spawn<F, Fut>(interval: std::time::Duration, probe: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = bool> + Send,
    {
        let (tx, rx) = tokio::sync::watch::channel(true);
        let token = tokio_util::sync::CancellationToken::new();
        let cancel_clone = token.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    () = cancel_clone.cancelled() => break,
                    () = tokio::time::sleep(interval) => {
                        let ok = probe().await;
                        // If the receiver is gone, stop.
                        if tx.send(ok).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        Self {
            healthy: rx,
            _cancel: token.drop_guard(),
        }
    }

    /// Returns the last-known health status.
    pub fn is_healthy(&self) -> bool {
        *self.healthy.borrow()
    }

    /// Wait until the health status changes and return the new value.
    pub async fn changed(&mut self) -> bool {
        let _ = self.healthy.changed().await;
        *self.healthy.borrow()
    }
}

// ---------------------------------------------------------------------------
// ManagedConnection — tonic channel with health checks and auto-reconnect
// ---------------------------------------------------------------------------

/// A managed gRPC connection that wraps a tonic [`Channel`] with automatic
/// reconnection and health monitoring.
pub struct ManagedConnection {
    channel: tonic::transport::Channel,
    endpoint: tonic::transport::Endpoint,
    config: TransportConfig,
    health: Option<HealthMonitor>,
}

impl std::fmt::Debug for ManagedConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManagedConnection")
            .field("config", &self.config)
            .field(
                "healthy",
                &self.health.as_ref().is_none_or(|h| h.is_healthy()),
            )
            .finish()
    }
}

impl ManagedConnection {
    /// Establish a new managed connection to `addr`.
    pub async fn connect(addr: &str, config: TransportConfig) -> Result<Self, TransportError> {
        let endpoint = config.configure_endpoint(addr)?;
        let channel = config
            .retry_policy
            .retry(|| {
                let ep = endpoint.clone();
                async move {
                    ep.connect()
                        .await
                        .map_err(|e| TransportError::Io(std::io::Error::other(e.to_string())))
                }
            })
            .await?;

        let mut conn = Self {
            channel,
            endpoint,
            config,
            health: None,
        };
        conn.start_health_monitor();
        Ok(conn)
    }

    /// Return a clone of the underlying tonic [`Channel`].
    pub fn channel(&self) -> tonic::transport::Channel {
        self.channel.clone()
    }

    /// Returns `true` when the last health check passed (or monitoring is
    /// disabled).
    pub fn is_healthy(&self) -> bool {
        self.health.as_ref().is_none_or(|h| h.is_healthy())
    }

    /// Attempt to re-establish the connection using the configured retry
    /// policy.
    pub async fn reconnect(&mut self) -> Result<(), TransportError> {
        let endpoint = self.endpoint.clone();
        let channel = self
            .config
            .retry_policy
            .retry(|| {
                let ep = endpoint.clone();
                async move {
                    ep.connect()
                        .await
                        .map_err(|e| TransportError::Io(std::io::Error::other(e.to_string())))
                }
            })
            .await?;
        self.channel = channel;
        self.start_health_monitor();
        Ok(())
    }

    /// Shut down the connection and stop the health monitor.
    pub fn shutdown(&mut self) {
        // Dropping the health monitor cancels the background task via the
        // CancellationToken's DropGuard.
        self.health.take();
    }

    // -- internal -----------------------------------------------------------

    fn start_health_monitor(&mut self) {
        if self.config.health_check_interval.is_zero() {
            return;
        }
        let channel = self.channel.clone();
        let timeout = self.config.request_timeout;
        self.health = Some(HealthMonitor::spawn(
            self.config.health_check_interval,
            move || {
                let mut client =
                    crate::proto::flight_service_client::FlightServiceClient::new(channel.clone());
                async move {
                    let result = tokio::time::timeout(
                        timeout,
                        client.get_service_info(crate::proto::GetServiceInfoRequest {}),
                    )
                    .await;
                    matches!(result, Ok(Ok(_)))
                }
            },
        ));
    }
}

/// Errors produced by the transport layer
#[derive(Debug, Error)]
pub enum TransportError {
    /// OS-level I/O failure (named pipe, Unix socket, etc.)
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// The requested transport is not available on this platform
    #[error("Transport not supported on this platform")]
    UnsupportedPlatform,

    /// Connection attempt exceeded the configured timeout
    #[error("Connection timeout")]
    Timeout,

    /// The supplied endpoint address could not be parsed or resolved
    #[error("Invalid address: {address}")]
    InvalidAddress {
        /// The malformed address string
        address: String,
    },
}

/// Cross-platform transport abstraction
pub trait Transport: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static {}

/// Named pipe transport (Windows)
#[cfg(all(windows, feature = "named-pipes"))]
pub mod named_pipes {
    use super::*;
    use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeServer};

    pub struct NamedPipeTransport {
        inner: NamedPipeInner,
    }

    enum NamedPipeInner {
        Server(NamedPipeServer),
        Client(tokio::net::windows::named_pipe::NamedPipeClient),
    }

    impl NamedPipeTransport {
        pub async fn connect(address: &str) -> Result<Self, TransportError> {
            let client = ClientOptions::new().open(address)?;
            Ok(Self {
                inner: NamedPipeInner::Client(client),
            })
        }

        pub async fn bind(address: &str) -> Result<Self, TransportError> {
            let server = tokio::net::windows::named_pipe::ServerOptions::new()
                .first_pipe_instance(true)
                .create(address)?;

            Ok(Self {
                inner: NamedPipeInner::Server(server),
            })
        }
    }

    impl AsyncRead for NamedPipeTransport {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<std::io::Result<()>> {
            match &mut self.inner {
                NamedPipeInner::Server(server) => Pin::new(server).poll_read(cx, buf),
                NamedPipeInner::Client(client) => Pin::new(client).poll_read(cx, buf),
            }
        }
    }

    impl AsyncWrite for NamedPipeTransport {
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<Result<usize, std::io::Error>> {
            match &mut self.inner {
                NamedPipeInner::Server(server) => Pin::new(server).poll_write(cx, buf),
                NamedPipeInner::Client(client) => Pin::new(client).poll_write(cx, buf),
            }
        }

        fn poll_flush(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<(), std::io::Error>> {
            match &mut self.inner {
                NamedPipeInner::Server(server) => Pin::new(server).poll_flush(cx),
                NamedPipeInner::Client(client) => Pin::new(client).poll_flush(cx),
            }
        }

        fn poll_shutdown(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<(), std::io::Error>> {
            match &mut self.inner {
                NamedPipeInner::Server(server) => Pin::new(server).poll_shutdown(cx),
                NamedPipeInner::Client(client) => Pin::new(client).poll_shutdown(cx),
            }
        }
    }

    impl Transport for NamedPipeTransport {}
}

/// Unix domain socket transport (Linux/macOS)
#[cfg(all(unix, feature = "unix-sockets"))]
pub mod unix_sockets {
    use super::*;
    use tokio::net::{UnixListener, UnixStream};

    pub struct UnixSocketTransport {
        stream: UnixStream,
    }

    impl UnixSocketTransport {
        pub async fn connect(address: &str) -> Result<Self, TransportError> {
            let stream = UnixStream::connect(address).await?;
            Ok(Self { stream })
        }

        pub async fn bind(address: &str) -> Result<UnixListener, TransportError> {
            // Remove existing socket file if it exists
            let _ = std::fs::remove_file(address);
            let listener = UnixListener::bind(address)?;
            Ok(listener)
        }

        pub fn from_stream(stream: UnixStream) -> Self {
            Self { stream }
        }
    }

    impl AsyncRead for UnixSocketTransport {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<std::io::Result<()>> {
            Pin::new(&mut self.stream).poll_read(cx, buf)
        }
    }

    impl AsyncWrite for UnixSocketTransport {
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<Result<usize, std::io::Error>> {
            Pin::new(&mut self.stream).poll_write(cx, buf)
        }

        fn poll_flush(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<(), std::io::Error>> {
            Pin::new(&mut self.stream).poll_flush(cx)
        }

        fn poll_shutdown(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<(), std::io::Error>> {
            Pin::new(&mut self.stream).poll_shutdown(cx)
        }
    }

    impl Transport for UnixSocketTransport {}
}

/// Create a transport based on the specified type and address with ACL validation
pub async fn create_transport_with_acl(
    transport_type: crate::TransportType,
    address: &str,
    is_server: bool,
    security_manager: Option<&SecurityManager>,
) -> Result<Box<dyn Transport>, TransportError> {
    // Validate ACL if security manager is provided and this is a client connection
    if let Some(security_manager) = security_manager
        && !is_server
    {
        let client_info = get_client_info()?;
        security_manager
            .validate_ipc_acl(&client_info)
            .map_err(|e| TransportError::InvalidAddress {
                address: format!("ACL validation failed: {}", e),
            })?;
    }

    create_transport(transport_type, address, is_server).await
}

/// Create a transport based on the specified type and address
pub async fn create_transport(
    #[cfg_attr(
        not(any(feature = "named-pipes", feature = "unix-sockets")),
        allow(unused_variables)
    )]
    transport_type: crate::TransportType,
    #[cfg_attr(
        not(any(feature = "named-pipes", feature = "unix-sockets")),
        allow(unused_variables)
    )]
    address: &str,
    #[cfg_attr(
        not(any(feature = "named-pipes", feature = "unix-sockets")),
        allow(unused_variables)
    )]
    is_server: bool,
) -> Result<Box<dyn Transport>, TransportError> {
    match transport_type {
        #[cfg(all(windows, feature = "named-pipes"))]
        crate::TransportType::NamedPipes => {
            if is_server {
                let transport = named_pipes::NamedPipeTransport::bind(address).await?;
                Ok(Box::new(transport))
            } else {
                let transport = named_pipes::NamedPipeTransport::connect(address).await?;
                Ok(Box::new(transport))
            }
        }

        #[cfg(all(unix, feature = "unix-sockets"))]
        crate::TransportType::UnixSockets => {
            if is_server {
                return Err(TransportError::UnsupportedPlatform);
            } else {
                let transport = unix_sockets::UnixSocketTransport::connect(address).await?;
                Ok(Box::new(transport))
            }
        }

        _ => Err(TransportError::UnsupportedPlatform),
    }
}

/// Get client information for ACL validation
fn get_client_info() -> Result<IpcClientInfo, TransportError> {
    #[cfg(windows)]
    {
        // On Windows, get current process info
        let process_id = std::process::id();
        let user_id = get_current_user_sid().unwrap_or_else(|_| "UNKNOWN".to_string());
        let executable_path = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("unknown"));

        Ok(IpcClientInfo {
            user_id,
            process_id,
            executable_path,
        })
    }

    #[cfg(unix)]
    {
        // On Unix, get current process info
        let process_id = std::process::id();
        let user_id = unsafe { libc::getuid() }.to_string();
        let executable_path = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("unknown"));

        Ok(IpcClientInfo {
            user_id,
            process_id,
            executable_path,
        })
    }
}

#[cfg(windows)]
fn get_current_user_sid() -> Result<String, std::io::Error> {
    // In a real implementation, this would use Windows APIs to get the current user SID
    // For now, return a placeholder
    Ok("S-1-5-21-000000000-000000000-000000000-1000".to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- TransportConfig ----------------------------------------------------

    #[test]
    fn default_transport_config_has_sane_values() {
        let cfg = TransportConfig::default();
        assert_eq!(cfg.connect_timeout, std::time::Duration::from_secs(5));
        assert_eq!(cfg.request_timeout, std::time::Duration::from_secs(5));
        assert_eq!(cfg.keepalive_interval, std::time::Duration::from_secs(10));
        assert_eq!(cfg.keepalive_timeout, std::time::Duration::from_secs(5));
        assert_eq!(
            cfg.health_check_interval,
            std::time::Duration::from_secs(15)
        );
    }

    #[test]
    fn configure_endpoint_valid_addr() {
        let cfg = TransportConfig::default();
        let ep = cfg.configure_endpoint("http://127.0.0.1:50051");
        assert!(ep.is_ok());
    }

    #[test]
    fn configure_endpoint_invalid_addr_returns_error() {
        let cfg = TransportConfig::default();
        let ep = cfg.configure_endpoint("not-a-url");
        assert!(ep.is_err());
    }

    // -- RetryPolicy --------------------------------------------------------

    #[test]
    fn default_retry_policy() {
        let rp = RetryPolicy::default();
        assert_eq!(rp.max_retries, 5);
        assert_eq!(rp.base_delay, std::time::Duration::from_millis(100));
        assert_eq!(rp.max_delay, std::time::Duration::from_secs(5));
    }

    #[test]
    fn delay_for_grows_exponentially() {
        let rp = RetryPolicy {
            base_delay: std::time::Duration::from_millis(100),
            max_delay: std::time::Duration::from_secs(60),
            max_retries: 10,
        };
        assert_eq!(rp.delay_for(0), std::time::Duration::from_millis(100));
        assert_eq!(rp.delay_for(1), std::time::Duration::from_millis(200));
        assert_eq!(rp.delay_for(2), std::time::Duration::from_millis(400));
        assert_eq!(rp.delay_for(3), std::time::Duration::from_millis(800));
    }

    #[test]
    fn delay_for_capped_at_max() {
        let rp = RetryPolicy {
            base_delay: std::time::Duration::from_millis(100),
            max_delay: std::time::Duration::from_millis(500),
            max_retries: 10,
        };
        // 2^3 * 100 = 800 > 500 → capped
        assert_eq!(rp.delay_for(3), std::time::Duration::from_millis(500));
    }

    #[tokio::test]
    async fn retry_succeeds_on_first_attempt() {
        let rp = RetryPolicy::default();
        let result: Result<&str, &str> = rp.retry(|| async { Ok("ok") }).await;
        assert_eq!(result.unwrap(), "ok");
    }

    #[tokio::test]
    async fn retry_succeeds_after_transient_failures() {
        let rp = RetryPolicy {
            max_retries: 3,
            base_delay: std::time::Duration::from_millis(1),
            max_delay: std::time::Duration::from_millis(10),
        };
        let counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let c = counter.clone();
        let result: Result<&str, &str> = rp
            .retry(|| {
                let c = c.clone();
                async move {
                    let n = c.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if n < 2 {
                        Err("transient")
                    } else {
                        Ok("recovered")
                    }
                }
            })
            .await;
        assert_eq!(result.unwrap(), "recovered");
        assert_eq!(counter.load(std::sync::atomic::Ordering::Relaxed), 3);
    }

    #[tokio::test]
    async fn retry_returns_last_error_when_exhausted() {
        let rp = RetryPolicy {
            max_retries: 2,
            base_delay: std::time::Duration::from_millis(1),
            max_delay: std::time::Duration::from_millis(5),
        };
        let result: Result<(), &str> = rp.retry(|| async { Err("fail") }).await;
        assert_eq!(result.unwrap_err(), "fail");
    }

    // -- HealthMonitor ------------------------------------------------------

    #[tokio::test]
    async fn health_monitor_reports_healthy_probe() {
        let monitor = HealthMonitor::spawn(std::time::Duration::from_millis(10), || async { true });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(monitor.is_healthy());
    }

    #[tokio::test]
    async fn health_monitor_reports_unhealthy_probe() {
        let mut monitor =
            HealthMonitor::spawn(std::time::Duration::from_millis(10), || async { false });
        // Wait for at least one probe cycle
        let healthy = monitor.changed().await;
        assert!(!healthy);
        assert!(!monitor.is_healthy());
    }

    #[tokio::test]
    async fn health_monitor_stops_on_drop() {
        let monitor = HealthMonitor::spawn(std::time::Duration::from_millis(5), || async { true });
        drop(monitor);
        // If the task didn't stop, this test would hang or leak — just
        // ensure we get here without panic.
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    // -- ManagedConnection --------------------------------------------------

    #[tokio::test]
    async fn managed_connection_connect_to_running_server() {
        let config = crate::ServerConfig::default();
        let server = crate::server::IpcServer::new_mock(config);
        let addr: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
        let handle = server.start(addr).await.unwrap();

        let tc = TransportConfig {
            health_check_interval: std::time::Duration::ZERO,
            ..TransportConfig::default()
        };
        let conn = ManagedConnection::connect(&format!("http://{}", handle.addr()), tc).await;
        assert!(conn.is_ok());

        let mut conn = conn.unwrap();
        assert!(conn.is_healthy());
        conn.shutdown();
        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn managed_connection_reconnect_after_disconnect() {
        let config = crate::ServerConfig::default();
        let server = crate::server::IpcServer::new_mock(config.clone());
        let addr: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
        let handle = server.start(addr).await.unwrap();
        let port = handle.addr().port();

        let tc = TransportConfig {
            health_check_interval: std::time::Duration::ZERO,
            retry_policy: RetryPolicy {
                max_retries: 10,
                base_delay: std::time::Duration::from_millis(50),
                max_delay: std::time::Duration::from_millis(500),
            },
            ..TransportConfig::default()
        };
        let mut conn = ManagedConnection::connect(&format!("http://127.0.0.1:{port}"), tc)
            .await
            .unwrap();

        // Shut down the original server
        handle.shutdown().await.unwrap();

        // Start a new server on the same port
        let server2 = crate::server::IpcServer::new_mock(config);
        let addr2: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
        let handle2 = server2.start(addr2).await.unwrap();

        // Reconnect should succeed
        let result = conn.reconnect().await;
        assert!(result.is_ok());
        conn.shutdown();
        handle2.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn managed_connection_connect_refused() {
        let tc = TransportConfig {
            retry_policy: RetryPolicy {
                max_retries: 1,
                base_delay: std::time::Duration::from_millis(1),
                max_delay: std::time::Duration::from_millis(5),
            },
            connect_timeout: std::time::Duration::from_millis(200),
            health_check_interval: std::time::Duration::ZERO,
            ..TransportConfig::default()
        };
        let result = ManagedConnection::connect("http://127.0.0.1:19999", tc).await;
        assert!(result.is_err());
    }
}
