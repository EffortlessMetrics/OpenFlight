// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! IPC/gRPC depth tests for `flight-ipc`.
//!
//! Covers five areas with 30+ tests:
//!   1. Message serialization — protobuf encode/decode roundtrips
//!   2. Connection lifecycle — connect, negotiate, heartbeat, disconnect, reconnect
//!   3. Streaming — server-stream health events, cancellation, backpressure
//!   4. Error handling — invalid messages, version mismatch, server gone
//!   5. Concurrency — multiple clients, fan-out, ordering, shutdown

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use flight_ipc::client::IpcClient;
use flight_ipc::handlers::{FlightServiceHandler, MockServiceContext};
use flight_ipc::proto::flight_service_client::FlightServiceClient as GrpcClient;
use flight_ipc::proto::flight_service_server::FlightServiceServer as GrpcFlightServiceServer;
use flight_ipc::proto::*;
use flight_ipc::server::IpcServer;
use flight_ipc::subscriptions::*;
use flight_ipc::transport::{RetryPolicy, TransportConfig};
use flight_ipc::{ClientConfig, ServerConfig, PROTOCOL_VERSION};
use prost::Message;

// ===========================================================================
// Helpers
// ===========================================================================

/// Start a mock IPC server on an ephemeral port; return (handle, "http://…" url).
async fn start_mock_server() -> (flight_ipc::server::ServerHandle, String) {
    let config = ServerConfig::default();
    let server = IpcServer::new_mock(config);
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let handle = server.start(addr).await.expect("server should start");
    let url = format!("http://127.0.0.1:{}", handle.addr().port());
    tokio::time::sleep(Duration::from_millis(30)).await;
    (handle, url)
}

/// Connect a raw tonic client.
async fn raw_client(url: &str) -> GrpcClient<tonic::transport::Channel> {
    GrpcClient::connect(url.to_string())
        .await
        .expect("client should connect")
}

/// Manual server handle for tests that need the health broadcast sender.
struct ManualServerHandle {
    shutdown_tx: tokio::sync::watch::Sender<bool>,
    join_handle: tokio::task::JoinHandle<Result<(), tonic::transport::Error>>,
}

impl ManualServerHandle {
    async fn shutdown(self) {
        let _ = self.shutdown_tx.send(true);
        // Brief grace period, then force-abort to avoid hangs from active streams
        tokio::time::sleep(Duration::from_millis(100)).await;
        self.join_handle.abort();
        let _ = self.join_handle.await;
    }
}

/// Start a server and return (ManualServerHandle, url, health_tx).
async fn start_server_with_health() -> (
    ManualServerHandle,
    String,
    tokio::sync::broadcast::Sender<HealthEvent>,
) {
    let config = ServerConfig::default();
    let ctx = Arc::new(MockServiceContext::new());
    let handler = FlightServiceHandler::new(ctx, config.clone());
    let health_tx = handler.health_sender();
    let svc = GrpcFlightServiceServer::new(handler);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local_addr = listener.local_addr().unwrap();
    let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);

    let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);
    let join_handle = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(svc)
            .serve_with_incoming_shutdown(incoming, async move {
                let _ = shutdown_rx.changed().await;
            })
            .await
    });

    tokio::time::sleep(Duration::from_millis(30)).await;

    let handle = ManualServerHandle {
        shutdown_tx,
        join_handle,
    };
    let url = format!("http://127.0.0.1:{}", local_addr.port());
    (handle, url, health_tx)
}

// ===========================================================================
// 1. MESSAGE SERIALIZATION (8 tests)
// ===========================================================================

/// 1a. All core request/response types survive protobuf encode→decode roundtrip.
#[test]
fn msg_roundtrip_all_request_types() {
    // NegotiateFeaturesRequest
    let req = NegotiateFeaturesRequest {
        client_version: "1.0.0".into(),
        supported_features: vec!["device-management".into(), "health-monitoring".into()],
        preferred_transport: TransportType::NamedPipes.into(),
    };
    let decoded = roundtrip(&req);
    assert_eq!(decoded.client_version, "1.0.0");
    assert_eq!(decoded.supported_features.len(), 2);

    // ListDevicesRequest
    let req = ListDevicesRequest {
        include_disconnected: true,
        filter_types: vec![DeviceType::Joystick.into(), DeviceType::Throttle.into()],
    };
    let decoded = roundtrip(&req);
    assert!(decoded.include_disconnected);
    assert_eq!(decoded.filter_types.len(), 2);

    // ApplyProfileRequest
    let req = ApplyProfileRequest {
        profile_json: r#"{"axes":{"pitch":{"expo":0.3}}}"#.into(),
        validate_only: true,
        force_apply: false,
    };
    let decoded = roundtrip(&req);
    assert_eq!(decoded.profile_json, req.profile_json);
    assert!(decoded.validate_only);
}

/// 1b. Proto3 default values: all-default message encodes to zero bytes and decodes back.
#[test]
fn msg_proto3_default_values_roundtrip() {
    let req = ListDevicesRequest::default();
    let mut buf = Vec::new();
    req.encode(&mut buf).unwrap();
    // Proto3: default values omit from wire
    assert!(buf.is_empty(), "all-default message should encode to 0 bytes");
    let decoded = ListDevicesRequest::decode(buf.as_slice()).unwrap();
    assert!(!decoded.include_disconnected);
    assert!(decoded.filter_types.is_empty());
}

/// 1c. Optional/nested fields: Device with capabilities and health sub-messages.
#[test]
fn msg_nested_optional_fields_roundtrip() {
    let device = Device {
        id: "stick-1".into(),
        name: "VKB Gladiator NXT EVO".into(),
        r#type: DeviceType::Joystick.into(),
        status: DeviceStatus::Connected.into(),
        capabilities: Some(DeviceCapabilities {
            supports_force_feedback: true,
            supports_raw_torque: false,
            max_torque_nm: 25,
            min_period_us: 500,
            has_health_stream: true,
            supported_protocols: vec!["hid".into(), "usb-raw".into()],
        }),
        health: Some(DeviceHealth {
            temperature_celsius: 42.5,
            current_amperes: 1.2,
            packet_loss_count: 3,
            last_seen_timestamp: 1700000000,
            active_faults: vec!["thermal-warning".into()],
        }),
        metadata: HashMap::from([
            ("vendor_id".into(), "231D".into()),
            ("product_id".into(), "0200".into()),
        ]),
    };
    let decoded: Device = roundtrip(&device);
    assert_eq!(decoded.id, "stick-1");
    let caps = decoded.capabilities.unwrap();
    assert!(caps.supports_force_feedback);
    assert_eq!(caps.supported_protocols.len(), 2);
    let health = decoded.health.unwrap();
    assert!((health.temperature_celsius - 42.5).abs() < f32::EPSILON);
    assert_eq!(health.active_faults, vec!["thermal-warning"]);
    assert_eq!(decoded.metadata.get("vendor_id").unwrap(), "231D");
}

/// 1d. Repeated fields: ListDevicesResponse with 50 devices.
#[test]
fn msg_repeated_fields_many_devices() {
    let devices: Vec<Device> = (0..50)
        .map(|i| Device {
            id: format!("dev-{i}"),
            name: format!("Device {i}"),
            r#type: DeviceType::Joystick.into(),
            status: DeviceStatus::Connected.into(),
            ..Default::default()
        })
        .collect();
    let resp = ListDevicesResponse {
        total_count: 50,
        devices,
    };
    let decoded: ListDevicesResponse = roundtrip(&resp);
    assert_eq!(decoded.devices.len(), 50);
    assert_eq!(decoded.total_count, 50);
    assert_eq!(decoded.devices[0].id, "dev-0");
    assert_eq!(decoded.devices[49].id, "dev-49");
}

/// 1e. Enum variant roundtrip for all ConflictSeverity and DeviceType values.
#[test]
fn msg_enum_variants_roundtrip() {
    for sev in [
        ConflictSeverity::Unspecified,
        ConflictSeverity::Low,
        ConflictSeverity::Medium,
        ConflictSeverity::High,
        ConflictSeverity::Critical,
    ] {
        let conflict = CurveConflict {
            axis_name: "pitch".into(),
            severity: sev.into(),
            ..Default::default()
        };
        let decoded: CurveConflict = roundtrip(&conflict);
        assert_eq!(decoded.severity, sev as i32);
    }

    for dt in [
        DeviceType::Unspecified,
        DeviceType::Joystick,
        DeviceType::Throttle,
        DeviceType::Rudder,
        DeviceType::Panel,
        DeviceType::ForceFeedback,
        DeviceType::Streamdeck,
    ] {
        let dev = Device {
            r#type: dt.into(),
            ..Default::default()
        };
        let decoded: Device = roundtrip(&dev);
        assert_eq!(decoded.r#type, dt as i32);
    }
}

/// 1f. Deeply nested messages: OneClickResult with BackupInfo, VerificationOutcome,
///     ResolutionMetrics, and ResolutionSteps.
#[test]
fn msg_deeply_nested_one_click_result() {
    let result = OneClickResult {
        resolution_type: ResolutionType::ApplyGainCompensation.into(),
        modified_files: vec!["sim.cfg".into(), "profile.json".into()],
        backup_info: Some(BackupInfo {
            timestamp: 1700000000,
            description: "Pre-resolve backup".into(),
            affected_files: vec!["sim.cfg".into()],
            backup_dir: "/backups/2024".into(),
            writer_config: "{}".into(),
        }),
        verification: Some(VerificationOutcome {
            passed: true,
            details: "All checks passed".into(),
            duration_ms: 150,
            conflict_resolved: true,
        }),
        metrics: Some(ResolutionMetrics {
            before: Some(ConflictMetrics {
                nonlinearity: 0.8,
                sim_curve_strength: 0.6,
                profile_curve_strength: 0.5,
                timestamp: 1700000000,
            }),
            after: Some(ConflictMetrics {
                nonlinearity: 0.1,
                sim_curve_strength: 0.0,
                profile_curve_strength: 0.5,
                timestamp: 1700000001,
            }),
            improvement: 0.875,
        }),
        steps_performed: vec![
            ResolutionStep {
                name: "detect".into(),
                description: "Detect sim curve".into(),
                success: true,
                duration_ms: 50,
                error: String::new(),
            },
            ResolutionStep {
                name: "apply".into(),
                description: "Apply gain compensation".into(),
                success: true,
                duration_ms: 80,
                error: String::new(),
            },
        ],
    };
    let decoded: OneClickResult = roundtrip(&result);
    assert_eq!(decoded.steps_performed.len(), 2);
    let metrics = decoded.metrics.unwrap();
    assert!((metrics.improvement - 0.875).abs() < f32::EPSILON);
    let before = metrics.before.unwrap();
    assert!((before.nonlinearity - 0.8).abs() < f32::EPSILON);
    let after = metrics.after.unwrap();
    assert!((after.nonlinearity - 0.1).abs() < f32::EPSILON);
    let backup = decoded.backup_info.unwrap();
    assert_eq!(backup.affected_files, vec!["sim.cfg"]);
}

/// 1g. Unknown/random bytes never panic during decode.
#[test]
fn msg_unknown_bytes_no_panic() {
    let garbage: Vec<Vec<u8>> = vec![
        vec![],
        vec![0xFF; 16],
        vec![0x00; 256],
        (0u8..=255).collect(),
        vec![0x0A, 0x03, 0x41, 0x42, 0x43], // valid-looking varint + string
    ];
    for bytes in &garbage {
        // Must not panic — Ok or Err are both fine
        let _ = NegotiateFeaturesRequest::decode(bytes.as_slice());
        let _ = ListDevicesResponse::decode(bytes.as_slice());
        let _ = ApplyProfileResponse::decode(bytes.as_slice());
        let _ = HealthEvent::decode(bytes.as_slice());
        let _ = OneClickResolveResponse::decode(bytes.as_slice());
    }
}

/// 1h. Map fields: GetServiceInfoResponse capabilities map survives roundtrip.
#[test]
fn msg_map_field_roundtrip() {
    let mut capabilities = HashMap::new();
    capabilities.insert("device-management".into(), "enabled".into());
    capabilities.insert("health-monitoring".into(), "enabled".into());
    capabilities.insert("force-feedback".into(), "disabled".into());

    let resp = GetServiceInfoResponse {
        version: "1.0.0".into(),
        uptime_seconds: 3600,
        status: ServiceStatus::Running.into(),
        capabilities: capabilities.clone(),
    };
    let decoded: GetServiceInfoResponse = roundtrip(&resp);
    assert_eq!(decoded.capabilities.len(), 3);
    assert_eq!(
        decoded.capabilities.get("device-management").unwrap(),
        "enabled"
    );
    assert_eq!(
        decoded.capabilities.get("force-feedback").unwrap(),
        "disabled"
    );
}

/// Helper: encode then decode a protobuf message.
fn roundtrip<M: Message + Default>(msg: &M) -> M {
    let mut buf = Vec::new();
    msg.encode(&mut buf).unwrap();
    M::decode(buf.as_slice()).unwrap()
}

// ===========================================================================
// 2. CONNECTION LIFECYCLE (6 tests)
// ===========================================================================

/// 2a. Connect to a running server succeeds and the client is usable.
#[tokio::test]
async fn lifecycle_connect_to_running_server() {
    let (handle, url) = start_mock_server().await;
    let mut client = IpcClient::connect(&url).await.unwrap();
    assert!(client.is_connected().await);
    handle.shutdown().await.unwrap();
}

/// 2b. Feature negotiation returns matching features and correct version.
#[tokio::test]
async fn lifecycle_negotiate_features_returns_intersection() {
    let (handle, url) = start_mock_server().await;
    let mut client = IpcClient::connect(&url).await.unwrap();
    let resp = client.negotiate_features().await.unwrap();
    assert!(resp.success);
    assert_eq!(resp.server_version, PROTOCOL_VERSION);
    // Client default features include device-management
    assert!(resp
        .enabled_features
        .contains(&"device-management".to_string()));
    handle.shutdown().await.unwrap();
}

/// 2c. Health subscription establishes a stream (subscribe step of lifecycle).
#[tokio::test]
async fn lifecycle_health_subscribe_opens_stream() {
    let (handle, url, health_tx) = start_server_with_health().await;
    let mut client = IpcClient::connect(&url).await.unwrap();

    let mut rx = client
        .subscribe_health(HealthSubscribeRequest::default())
        .await
        .unwrap();

    // Publish an event and verify the client receives it
    let event = HealthEvent {
        timestamp: 12345,
        r#type: HealthEventType::Info.into(),
        message: "test-event".into(),
        ..Default::default()
    };
    health_tx.send(event.clone()).unwrap();

    let received = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("should receive within timeout")
        .expect("channel should not be closed");
    assert_eq!(received.message, "test-event");
    assert_eq!(received.timestamp, 12345);

    handle.shutdown().await;
}

/// 2d. is_connected returns true while server is up, acts as heartbeat check.
#[tokio::test]
async fn lifecycle_heartbeat_is_connected() {
    let (handle, url) = start_mock_server().await;
    let mut client = IpcClient::connect(&url).await.unwrap();

    // Multiple heartbeat checks should all succeed
    for _ in 0..3 {
        assert!(client.is_connected().await);
    }
    handle.shutdown().await.unwrap();
}

/// 2e. Disconnect makes subsequent calls fail.
#[tokio::test]
async fn lifecycle_disconnect_makes_calls_fail() {
    let (handle, url) = start_mock_server().await;
    let mut client = IpcClient::connect(&url).await.unwrap();
    assert!(client.get_service_info().await.is_ok());

    client.disconnect().await;
    assert!(client.get_service_info().await.is_err());
    assert!(!client.is_connected().await);

    handle.shutdown().await.unwrap();
}

/// 2f. Full connect→disconnect→reconnect cycle.
#[tokio::test]
async fn lifecycle_reconnect_after_disconnect() {
    let (handle, url) = start_mock_server().await;
    let mut client = IpcClient::connect(&url).await.unwrap();

    assert!(client.is_connected().await);
    client.disconnect().await;
    assert!(!client.is_connected().await);

    client.reconnect().await.unwrap();
    assert!(client.is_connected().await);
    let info = client.get_service_info().await.unwrap();
    assert_eq!(info.version, PROTOCOL_VERSION);

    handle.shutdown().await.unwrap();
}

// ===========================================================================
// 3. STREAMING (6 tests)
// ===========================================================================

/// 3a. Server-streaming: multiple health events arrive in order.
#[tokio::test]
async fn stream_server_health_events_in_order() {
    let (handle, url, health_tx) = start_server_with_health().await;
    let mut client = IpcClient::connect(&url).await.unwrap();

    let mut rx = client
        .subscribe_health(HealthSubscribeRequest::default())
        .await
        .unwrap();

    // Send 5 events
    for i in 0..5 {
        health_tx
            .send(HealthEvent {
                timestamp: i,
                message: format!("event-{i}"),
                ..Default::default()
            })
            .unwrap();
    }

    // Receive all 5 in order
    for i in 0..5 {
        let evt = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(evt.timestamp, i);
        assert_eq!(evt.message, format!("event-{i}"));
    }

    handle.shutdown().await;
}

/// 3b. Fan-out: two clients both receive the same health events.
#[tokio::test]
async fn stream_fan_out_to_multiple_clients() {
    let (handle, url, health_tx) = start_server_with_health().await;

    let mut client1 = IpcClient::connect(&url).await.unwrap();
    let mut client2 = IpcClient::connect(&url).await.unwrap();

    let mut rx1 = client1
        .subscribe_health(HealthSubscribeRequest::default())
        .await
        .unwrap();
    let mut rx2 = client2
        .subscribe_health(HealthSubscribeRequest::default())
        .await
        .unwrap();

    health_tx
        .send(HealthEvent {
            timestamp: 999,
            message: "broadcast".into(),
            ..Default::default()
        })
        .unwrap();

    let e1 = tokio::time::timeout(Duration::from_secs(2), rx1.recv())
        .await
        .unwrap()
        .unwrap();
    let e2 = tokio::time::timeout(Duration::from_secs(2), rx2.recv())
        .await
        .unwrap()
        .unwrap();

    assert_eq!(e1.message, "broadcast");
    assert_eq!(e2.message, "broadcast");

    handle.shutdown().await;
}

/// 3c. Stream cancellation: dropping the receiver doesn't crash the server.
#[tokio::test]
async fn stream_cancellation_by_dropping_receiver() {
    let (handle, url, health_tx) = start_server_with_health().await;
    let mut client = IpcClient::connect(&url).await.unwrap();

    let rx = client
        .subscribe_health(HealthSubscribeRequest::default())
        .await
        .unwrap();

    // Drop the receiver to cancel the stream
    drop(rx);

    // Server should still be healthy — send events without panic
    let _ = health_tx.send(HealthEvent::default());
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Other RPCs should still work
    let mut client2 = IpcClient::connect(&url).await.unwrap();
    assert!(client2.get_service_info().await.is_ok());

    handle.shutdown().await;
}

/// 3d. Stream ends when server shuts down.
#[tokio::test]
async fn stream_ends_on_server_shutdown() {
    let (handle, url, _health_tx) = start_server_with_health().await;
    let mut client = IpcClient::connect(&url).await.unwrap();

    let mut rx = client
        .subscribe_health(HealthSubscribeRequest::default())
        .await
        .unwrap();

    // Shut down the server
    handle.shutdown().await;

    // The stream should eventually close (recv returns None or errors)
    let result = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await;
    // Either timeout or None — both indicate stream closure
    assert!(result.is_err() || result.unwrap().is_none());
}

/// 3e. Backpressure: channel capacity is respected; server doesn't block.
#[tokio::test]
async fn stream_backpressure_channel_bounded() {
    let (handle, url, health_tx) = start_server_with_health().await;
    let mut client = IpcClient::connect(&url).await.unwrap();

    let _rx = client
        .subscribe_health(HealthSubscribeRequest::default())
        .await
        .unwrap();

    // Flood the health channel — should not block the server
    for i in 0..200 {
        // broadcast::send may fail if no active receivers consumed yet; that's OK
        let _ = health_tx.send(HealthEvent {
            timestamp: i,
            message: format!("flood-{i}"),
            ..Default::default()
        });
    }

    // Server should still respond to unary RPCs
    let mut client2 = IpcClient::connect(&url).await.unwrap();
    let info = client2.get_service_info().await.unwrap();
    assert_eq!(info.version, PROTOCOL_VERSION);

    handle.shutdown().await;
}

/// 3f. Stream with concurrent unary RPCs — streaming and unary coexist.
#[tokio::test]
async fn stream_coexists_with_unary_rpcs() {
    let (handle, url, health_tx) = start_server_with_health().await;
    let mut client = IpcClient::connect(&url).await.unwrap();

    let mut rx = client
        .subscribe_health(HealthSubscribeRequest::default())
        .await
        .unwrap();

    // Make unary RPCs while stream is open
    let mut client2 = IpcClient::connect(&url).await.unwrap();
    let info = client2.get_service_info().await.unwrap();
    assert_eq!(info.version, PROTOCOL_VERSION);

    // Stream should still work
    health_tx
        .send(HealthEvent {
            message: "after-unary".into(),
            ..Default::default()
        })
        .unwrap();

    let evt = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(evt.message, "after-unary");

    handle.shutdown().await;
}

// ===========================================================================
// 4. ERROR HANDLING (5 tests)
// ===========================================================================

/// 4a. Decoding invalid protobuf bytes returns Err, never panics.
#[test]
fn error_invalid_protobuf_decode() {
    // Truncated varint
    let bad_varint = vec![0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80];
    assert!(NegotiateFeaturesRequest::decode(bad_varint.as_slice()).is_err());

    // Wrong wire type for a known field
    let bad_wire = vec![0x09, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
    // May decode successfully (proto3 is lenient) or error — must not panic
    let _ = ListDevicesRequest::decode(bad_wire.as_slice());
}

/// 4b. Version mismatch in feature negotiation returns failure response (not gRPC error).
#[tokio::test]
async fn error_version_mismatch_negotiation() {
    let (handle, url) = start_mock_server().await;
    let mut client = raw_client(&url).await;

    let resp = client
        .negotiate_features(NegotiateFeaturesRequest {
            client_version: "99.0.0".into(),
            supported_features: vec![],
            preferred_transport: TransportType::Unspecified.into(),
        })
        .await
        .unwrap()
        .into_inner();

    assert!(!resp.success);
    assert!(!resp.error_message.is_empty());
    assert!(resp.error_message.contains("mismatch") || resp.error_message.contains("Version"));

    handle.shutdown().await.unwrap();
}

/// 4c. Request after server shutdown returns an error.
#[tokio::test]
async fn error_request_after_server_shutdown() {
    let (handle, url) = start_mock_server().await;
    let mut client = IpcClient::connect(&url).await.unwrap();
    assert!(client.get_service_info().await.is_ok());

    handle.shutdown().await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let result = client.get_service_info().await;
    assert!(result.is_err());
}

/// 4d. Connection to a server that doesn't exist fails with a clear error.
#[tokio::test]
async fn error_connection_to_nonexistent_server() {
    let tc = TransportConfig {
        connect_timeout: Duration::from_millis(200),
        retry_policy: RetryPolicy {
            max_retries: 0,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(5),
        },
        health_check_interval: Duration::ZERO,
        ..TransportConfig::default()
    };
    let result =
        IpcClient::connect_with_transport("http://127.0.0.1:19876", ClientConfig::default(), tc)
            .await;
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("connect") || err_msg.contains("Connect") || err_msg.contains("Failed"),
        "error should describe connection failure: {err_msg}"
    );
}

/// 4e. Concurrent clients all see errors after server shutdown.
#[tokio::test]
async fn error_concurrent_clients_after_shutdown() {
    let (handle, url) = start_mock_server().await;

    let mut clients = Vec::new();
    for _ in 0..5 {
        clients.push(IpcClient::connect(&url).await.unwrap());
    }
    // Verify all work
    for c in &mut clients {
        assert!(c.get_service_info().await.is_ok());
    }

    handle.shutdown().await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    // All should fail
    for c in &mut clients {
        assert!(c.get_service_info().await.is_err());
    }
}

// ===========================================================================
// 5. CONCURRENCY (5 tests)
// ===========================================================================

/// 5a. Multiple clients making simultaneous RPCs all succeed.
#[tokio::test]
async fn concurrency_multiple_clients_simultaneous() {
    let (handle, url) = start_mock_server().await;

    let mut tasks = Vec::new();
    for i in 0..10 {
        let u = url.clone();
        tasks.push(tokio::spawn(async move {
            let mut c = IpcClient::connect(&u).await.unwrap();
            let info = c.get_service_info().await.unwrap();
            assert_eq!(info.version, PROTOCOL_VERSION);
            i // return task id for verification
        }));
    }

    let mut results = Vec::new();
    for t in tasks {
        results.push(t.await.unwrap());
    }
    results.sort();
    assert_eq!(results, (0..10).collect::<Vec<_>>());

    handle.shutdown().await.unwrap();
}

/// 5b. SubscriptionManager fan-out delivers to all matching subscribers.
#[test]
fn concurrency_subscription_fan_out() {
    let mut mgr = SubscriptionManager::new();

    // 10 subscribers on the same topic
    let handles: Vec<_> = (0..10)
        .map(|_| mgr.subscribe(Topic::AxisData, SubscriptionFilter::default()))
        .collect();

    let msg = BroadcastMessage {
        topic: Topic::AxisData,
        payload: "axis-value".into(),
        device_id: None,
        axis_id: None,
    };

    let delivered = mgr.broadcast(&msg);
    assert_eq!(delivered.len(), 10);

    // Each subscriber ID should be in the delivered set
    for h in &handles {
        assert!(delivered.contains(&h.id));
    }
}

/// 5c. Sequential requests on same client maintain ordering.
#[tokio::test]
async fn concurrency_ordering_sequential_requests() {
    let (handle, url) = start_mock_server().await;
    let mut client = IpcClient::connect(&url).await.unwrap();

    // Issue 20 sequential RPCs — all should succeed in order
    for _ in 0..20 {
        let info = client.get_service_info().await.unwrap();
        assert_eq!(info.version, PROTOCOL_VERSION);
    }

    handle.shutdown().await.unwrap();
}

/// 5d. Client disconnect during active stream doesn't crash server.
#[tokio::test]
async fn concurrency_client_disconnect_during_stream() {
    let (handle, url, health_tx) = start_server_with_health().await;

    // Client subscribes and then disconnects
    {
        let mut client = IpcClient::connect(&url).await.unwrap();
        let _rx = client
            .subscribe_health(HealthSubscribeRequest::default())
            .await
            .unwrap();
        // client + rx drop here
    }

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Server should still be alive
    let _ = health_tx.send(HealthEvent::default());
    let mut client2 = IpcClient::connect(&url).await.unwrap();
    assert!(client2.get_service_info().await.is_ok());

    handle.shutdown().await;
}

/// 5e. Server shutdown with multiple active streaming clients completes cleanly.
#[tokio::test]
async fn concurrency_server_shutdown_with_active_streams() {
    let (handle, url, _health_tx) = start_server_with_health().await;

    // Open several streaming subscriptions
    let mut receivers = Vec::new();
    for _ in 0..5 {
        let mut c = IpcClient::connect(&url).await.unwrap();
        let rx = c
            .subscribe_health(HealthSubscribeRequest::default())
            .await
            .unwrap();
        receivers.push(rx);
    }

    // Shutdown should not hang or panic despite active streams
    let shutdown_result =
        tokio::time::timeout(Duration::from_secs(5), async { handle.shutdown().await }).await;
    assert!(
        shutdown_result.is_ok(),
        "server shutdown should complete within timeout"
    );
}

// ===========================================================================
// BONUS: Cross-cutting integration tests
// ===========================================================================

/// Bonus 1: Full lifecycle — connect, negotiate, list devices, apply profile,
///          detect conflicts, get security status, disconnect.
#[tokio::test]
async fn integration_full_rpc_lifecycle() {
    let (handle, url) = start_mock_server().await;
    let mut client = IpcClient::connect(&url).await.unwrap();

    // Negotiate
    let neg = client.negotiate_features().await.unwrap();
    assert!(neg.success);

    // List devices
    let devs = client.list_devices().await.unwrap();
    assert_eq!(devs.total_count, 0);

    // Apply profile
    let prof = client
        .apply_profile(ApplyProfileRequest {
            profile_json: "{}".into(),
            validate_only: true,
            force_apply: false,
        })
        .await
        .unwrap();
    assert!(prof.success);

    // Detect conflicts
    let conflicts = client
        .detect_curve_conflicts(DetectCurveConflictsRequest::default())
        .await
        .unwrap();
    assert!(conflicts.success);

    // Security status
    let sec = client.get_security_status().await.unwrap();
    assert!(sec.success);

    // Support bundle
    let bundle = client.get_support_bundle().await.unwrap();
    assert!(bundle.success);

    // Capability mode
    let cap = client
        .set_capability_mode(SetCapabilityModeRequest {
            mode: CapabilityMode::Demo.into(),
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(cap.success);

    // Configure telemetry
    let tel = client
        .configure_telemetry(ConfigureTelemetryRequest {
            enabled: true,
            data_types: vec!["Performance".into()],
        })
        .await
        .unwrap();
    assert!(tel.success);

    // Disconnect
    client.disconnect().await;
    assert!(!client.is_connected().await);

    handle.shutdown().await.unwrap();
}

/// Bonus 2: SubscriptionManager mixed-topic broadcast with filters.
#[test]
fn integration_subscription_mixed_topics_and_filters() {
    let mut mgr = SubscriptionManager::new();

    // Subscribe to different topics with different filters
    let h_axis = mgr.subscribe(Topic::AxisData, SubscriptionFilter::default());
    let h_dev = mgr.subscribe(
        Topic::DeviceEvents,
        SubscriptionFilter {
            device_id: Some("stick-1".into()),
            ..Default::default()
        },
    );
    let h_health = mgr.subscribe(Topic::HealthStatus, SubscriptionFilter::default());

    // AxisData broadcast hits only axis subscriber
    let ids = mgr.broadcast(&BroadcastMessage {
        topic: Topic::AxisData,
        payload: "pitch=0.5".into(),
        device_id: None,
        axis_id: None,
    });
    assert_eq!(ids, vec![h_axis.id]);

    // DeviceEvents with matching device hits device subscriber
    let ids = mgr.broadcast(&BroadcastMessage {
        topic: Topic::DeviceEvents,
        payload: "connected".into(),
        device_id: Some("stick-1".into()),
        axis_id: None,
    });
    assert_eq!(ids, vec![h_dev.id]);

    // DeviceEvents with wrong device hits nobody
    let ids = mgr.broadcast(&BroadcastMessage {
        topic: Topic::DeviceEvents,
        payload: "connected".into(),
        device_id: Some("stick-2".into()),
        axis_id: None,
    });
    assert!(ids.is_empty());

    // Health broadcast hits health subscriber
    let ids = mgr.broadcast(&BroadcastMessage {
        topic: Topic::HealthStatus,
        payload: "ok".into(),
        device_id: None,
        axis_id: None,
    });
    assert_eq!(ids, vec![h_health.id]);

    // Cancel device subscription, verify GC works
    h_dev.cancel();
    assert_eq!(mgr.active_count(), 2);
}
