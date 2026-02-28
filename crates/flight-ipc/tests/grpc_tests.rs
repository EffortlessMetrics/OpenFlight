// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! gRPC server/client integration tests.
//!
//! These tests spin up a real tonic server on localhost:0 (ephemeral port),
//! connect a generated client, and exercise the RPC contract end-to-end.

use flight_ipc::{
    ServerConfig,
    client::IpcClient,
    handlers::{FlightServiceHandler, MockServiceContext},
    proto::{
        self, ApplyProfileRequest, DetectCurveConflictsRequest, GetCapabilityModeRequest,
        GetServiceInfoRequest, GetSupportBundleRequest, ListDevicesRequest,
        NegotiateFeaturesRequest, SetCapabilityModeRequest, TransportType,
        flight_service_client::FlightServiceClient as GrpcClient,
        flight_service_server::FlightServiceServer as GrpcFlightServiceServer,
    },
    server::IpcServer,
};
use std::{net::SocketAddr, sync::Arc, time::Duration};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Start an [`IpcServer`] backed by [`MockServiceContext`] on an ephemeral
/// port and return the server handle plus the listening address formatted
/// as `"http://127.0.0.1:{port}"`.
async fn start_test_server() -> (flight_ipc::server::ServerHandle, String) {
    let config = ServerConfig {
        max_connections: 10,
        request_timeout: Duration::from_secs(5),
        ..ServerConfig::default()
    };

    let server = IpcServer::new_mock(config);
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let handle = server.start(addr).await.expect("server should start");
    let url = format!("http://127.0.0.1:{}", handle.addr().port());

    // Small delay for server to become ready
    tokio::time::sleep(Duration::from_millis(50)).await;

    (handle, url)
}

/// Connect a raw tonic client to the given URL.
async fn raw_client(url: &str) -> GrpcClient<tonic::transport::Channel> {
    GrpcClient::connect(url.to_string())
        .await
        .expect("client should connect")
}

// ---------------------------------------------------------------------------
// Server start / shutdown tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn server_starts_on_ephemeral_port() {
    let (handle, _url) = start_test_server().await;
    assert_ne!(handle.addr().port(), 0);
    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn server_graceful_shutdown() {
    let (handle, url) = start_test_server().await;

    // Verify it's alive
    let mut client = raw_client(&url).await;
    let resp = client
        .get_service_info(GetServiceInfoRequest {})
        .await
        .unwrap();
    assert_eq!(resp.into_inner().version, flight_ipc::PROTOCOL_VERSION);

    // Shut down
    handle.shutdown().await.unwrap();

    // After shutdown, new connections should fail eventually
    tokio::time::sleep(Duration::from_millis(100)).await;
    let result = GrpcClient::connect(url).await;
    // Connection might succeed (from OS buffer) but request should fail
    if let Ok(mut c) = result {
        let r = c.get_service_info(GetServiceInfoRequest {}).await;
        // Either fails outright or returns Unavailable
        assert!(r.is_err());
    }
}

#[tokio::test]
async fn server_double_shutdown_is_safe() {
    let (handle, _url) = start_test_server().await;
    // Calling shutdown on the handle once is OK
    handle.shutdown().await.unwrap();
    // The handle is consumed, so double-call isn't even possible at the
    // type level — this test just verifies single shutdown doesn't panic.
}

// ---------------------------------------------------------------------------
// NegotiateFeatures
// ---------------------------------------------------------------------------

#[tokio::test]
async fn negotiate_features_success() {
    let (handle, url) = start_test_server().await;
    let mut client = raw_client(&url).await;

    let resp = client
        .negotiate_features(NegotiateFeaturesRequest {
            client_version: "1.0.0".to_string(),
            supported_features: vec!["device-management".to_string()],
            preferred_transport: TransportType::NamedPipes.into(),
        })
        .await
        .unwrap()
        .into_inner();

    assert!(resp.success);
    assert!(
        resp.enabled_features
            .contains(&"device-management".to_string())
    );

    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn negotiate_features_version_mismatch() {
    let (handle, url) = start_test_server().await;
    let mut client = raw_client(&url).await;

    let resp = client
        .negotiate_features(NegotiateFeaturesRequest {
            client_version: "99.0.0".to_string(),
            supported_features: vec![],
            preferred_transport: TransportType::Unspecified.into(),
        })
        .await
        .unwrap()
        .into_inner();

    // Negotiation reports failure but doesn't error at the transport level
    assert!(!resp.success);
    assert!(!resp.error_message.is_empty());

    handle.shutdown().await.unwrap();
}

// ---------------------------------------------------------------------------
// ListDevices
// ---------------------------------------------------------------------------

#[tokio::test]
async fn list_devices_empty() {
    let (handle, url) = start_test_server().await;
    let mut client = raw_client(&url).await;

    let resp = client
        .list_devices(ListDevicesRequest::default())
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.total_count, 0);
    assert!(resp.devices.is_empty());

    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn list_devices_with_custom_context() {
    let mut mock = MockServiceContext::new();
    mock.devices = vec![proto::Device {
        id: "test-stick-1".to_string(),
        name: "Test Joystick".to_string(),
        r#type: proto::DeviceType::Joystick.into(),
        status: proto::DeviceStatus::Connected.into(),
        capabilities: None,
        health: None,
        metadata: Default::default(),
    }];

    let config = ServerConfig::default();
    let ctx = Arc::new(mock);

    // Build handler directly and wire into tonic
    let handler = FlightServiceHandler::new(ctx, config.clone());
    let svc = GrpcFlightServiceServer::new(handler);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);

    let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);
    let server_task = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(svc)
            .serve_with_incoming_shutdown(incoming, async move {
                let _ = shutdown_rx.changed().await;
            })
            .await
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    let url = format!("http://127.0.0.1:{}", addr.port());
    let mut client = raw_client(&url).await;

    let resp = client
        .list_devices(ListDevicesRequest::default())
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.total_count, 1);
    assert_eq!(resp.devices[0].id, "test-stick-1");
    assert_eq!(resp.devices[0].name, "Test Joystick");

    let _ = shutdown_tx.send(true);
    let _ = server_task.await;
}

// ---------------------------------------------------------------------------
// ApplyProfile
// ---------------------------------------------------------------------------

#[tokio::test]
async fn apply_profile_success() {
    let (handle, url) = start_test_server().await;
    let mut client = raw_client(&url).await;

    let resp = client
        .apply_profile(ApplyProfileRequest {
            profile_json: "{}".to_string(),
            validate_only: false,
            force_apply: false,
        })
        .await
        .unwrap()
        .into_inner();

    assert!(resp.success);
    assert_eq!(resp.effective_profile_hash, "mock-hash");

    handle.shutdown().await.unwrap();
}

// ---------------------------------------------------------------------------
// GetServiceInfo (system / diagnostics)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_service_info() {
    let (handle, url) = start_test_server().await;
    let mut client = raw_client(&url).await;

    let resp = client
        .get_service_info(GetServiceInfoRequest {})
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.version, flight_ipc::PROTOCOL_VERSION);
    assert_eq!(resp.status(), proto::ServiceStatus::Running);

    handle.shutdown().await.unwrap();
}

// ---------------------------------------------------------------------------
// DetectCurveConflicts
// ---------------------------------------------------------------------------

#[tokio::test]
async fn detect_curve_conflicts_empty() {
    let (handle, url) = start_test_server().await;
    let mut client = raw_client(&url).await;

    let resp = client
        .detect_curve_conflicts(DetectCurveConflictsRequest::default())
        .await
        .unwrap()
        .into_inner();

    assert!(resp.success);
    assert!(resp.conflicts.is_empty());

    handle.shutdown().await.unwrap();
}

// ---------------------------------------------------------------------------
// Capability management
// ---------------------------------------------------------------------------

#[tokio::test]
async fn set_and_get_capability_mode() {
    let (handle, url) = start_test_server().await;
    let mut client = raw_client(&url).await;

    let set_resp = client
        .set_capability_mode(SetCapabilityModeRequest {
            mode: proto::CapabilityMode::Demo.into(),
            axis_names: vec![],
            audit_enabled: false,
        })
        .await
        .unwrap()
        .into_inner();
    assert!(set_resp.success);

    let get_resp = client
        .get_capability_mode(GetCapabilityModeRequest { axis_names: vec![] })
        .await
        .unwrap()
        .into_inner();
    assert!(get_resp.success);

    handle.shutdown().await.unwrap();
}

// ---------------------------------------------------------------------------
// Security / telemetry
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_support_bundle() {
    let (handle, url) = start_test_server().await;
    let mut client = raw_client(&url).await;

    let resp = client
        .get_support_bundle(GetSupportBundleRequest {})
        .await
        .unwrap()
        .into_inner();

    assert!(resp.success);

    handle.shutdown().await.unwrap();
}

// ---------------------------------------------------------------------------
// IpcClient (wrapper) tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ipc_client_list_devices() {
    let (handle, url) = start_test_server().await;

    let mut client = IpcClient::connect(&url).await.unwrap();
    let resp = client.list_devices().await.unwrap();
    assert_eq!(resp.total_count, 0);

    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn ipc_client_get_service_info() {
    let (handle, url) = start_test_server().await;

    let mut client = IpcClient::connect(&url).await.unwrap();
    let resp = client.get_service_info().await.unwrap();
    assert_eq!(resp.version, flight_ipc::PROTOCOL_VERSION);

    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn ipc_client_negotiate_features() {
    let (handle, url) = start_test_server().await;

    let mut client = IpcClient::connect(&url).await.unwrap();
    let resp = client.negotiate_features().await.unwrap();
    assert!(resp.success);

    handle.shutdown().await.unwrap();
}

// ---------------------------------------------------------------------------
// Connection error handling
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ipc_client_connection_refused() {
    // Nothing listening on this port
    let result = IpcClient::connect("http://127.0.0.1:19998").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn raw_client_connection_refused() {
    let result = GrpcClient::connect("http://127.0.0.1:19997").await;
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Timeout handling
// ---------------------------------------------------------------------------

#[tokio::test]
async fn server_request_timeout_is_configured() {
    // We just verify the server starts correctly with a custom timeout.
    // Actual timeout enforcement depends on slow backends which we don't
    // simulate here.
    let config = ServerConfig {
        request_timeout: Duration::from_millis(200),
        ..ServerConfig::default()
    };
    let server = IpcServer::new_mock(config);
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let handle = server.start(addr).await.unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;

    let url = format!("http://127.0.0.1:{}", handle.addr().port());
    let mut client = raw_client(&url).await;

    // Quick request should succeed within timeout
    let resp = client
        .get_service_info(GetServiceInfoRequest {})
        .await
        .unwrap()
        .into_inner();
    assert_eq!(resp.version, flight_ipc::PROTOCOL_VERSION);

    handle.shutdown().await.unwrap();
}

// ---------------------------------------------------------------------------
// Max connections (concurrency limit)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn server_max_connections_configured() {
    let config = ServerConfig {
        max_connections: 2,
        ..ServerConfig::default()
    };

    let server = IpcServer::new_mock(config);
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let handle = server.start(addr).await.unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;

    let url = format!("http://127.0.0.1:{}", handle.addr().port());

    // Connect multiple clients — they should all succeed since tonic's
    // concurrency_limit_per_connection limits in-flight *requests*, not
    // *connections*. We just verify the server functions correctly.
    let mut clients = Vec::new();
    for _ in 0..3 {
        clients.push(raw_client(&url).await);
    }

    // All clients should be able to make requests
    for client in &mut clients {
        let resp = client
            .get_service_info(GetServiceInfoRequest {})
            .await
            .unwrap()
            .into_inner();
        assert_eq!(resp.version, flight_ipc::PROTOCOL_VERSION);
    }

    handle.shutdown().await.unwrap();
}

// ---------------------------------------------------------------------------
// Multiple sequential requests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn multiple_sequential_requests() {
    let (handle, url) = start_test_server().await;
    let mut client = raw_client(&url).await;

    for _ in 0..10 {
        let resp = client
            .get_service_info(GetServiceInfoRequest {})
            .await
            .unwrap()
            .into_inner();
        assert_eq!(resp.status(), proto::ServiceStatus::Running);
    }

    handle.shutdown().await.unwrap();
}

// ---------------------------------------------------------------------------
// Concurrent requests from multiple clients
// ---------------------------------------------------------------------------

#[tokio::test]
async fn concurrent_requests_from_multiple_clients() {
    let (handle, url) = start_test_server().await;

    let mut tasks = Vec::new();
    for _ in 0..5 {
        let u = url.clone();
        tasks.push(tokio::spawn(async move {
            let mut c = raw_client(&u).await;
            let resp = c
                .get_service_info(GetServiceInfoRequest {})
                .await
                .unwrap()
                .into_inner();
            assert_eq!(resp.version, flight_ipc::PROTOCOL_VERSION);
        }));
    }

    for t in tasks {
        t.await.unwrap();
    }

    handle.shutdown().await.unwrap();
}
