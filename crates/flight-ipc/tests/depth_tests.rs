// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the IPC crate covering connection lifecycle, subscription
//! management, serialization round-trips, backpressure, concurrency, rate
//! limiting, connection pool behaviour, and property-based validation.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use flight_ipc::client::IpcClient;
use flight_ipc::connection_pool::{ConnectionPool, PoolError};
use flight_ipc::handlers::{FlightServiceHandler, MockServiceContext, ServiceContext};
use flight_ipc::messages::{ComponentStatus, IpcMessage, ServiceState};
use flight_ipc::negotiation::{self, Version};
use flight_ipc::proto::flight_service_server::FlightService as GrpcFlightService;
use flight_ipc::proto::{
    self, ApplyProfileRequest, ApplyProfileResponse, CapabilityMode, ConfigureTelemetryRequest,
    ConflictMetadata, ConflictSeverity, ConflictType, CurveConflict, DetectCurveConflictsRequest,
    Device, DeviceCapabilities, DeviceHealth, DeviceStatus, DeviceType,
    GetCapabilityModeRequest, GetSecurityStatusRequest,
    GetSupportBundleRequest, HealthEvent, HealthEventType, HealthSubscribeRequest,
    ListDevicesResponse, NegotiateFeaturesRequest, OneClickResolveRequest,
    PerformanceMetrics, ResolveCurveConflictRequest, ServiceStatus,
    SetCapabilityModeRequest, TransportType, ValidationError, ValidationErrorType,
};
use flight_ipc::rate_limiter::{RateLimitConfig, RateLimitResult, RateLimiter};
use flight_ipc::server::IpcServer;
use flight_ipc::subscriptions::{
    BroadcastMessage, SubscriptionFilter, SubscriptionManager, Topic,
};
use flight_ipc::transport::{RetryPolicy, TransportConfig};
use flight_ipc::{ClientConfig, IpcError, ServerConfig, PROTOCOL_VERSION, SUPPORTED_FEATURES};

use prost::Message;
use tonic::Request;

// ===========================================================================
// Helpers
// ===========================================================================

async fn start_mock_server() -> (flight_ipc::server::ServerHandle, String) {
    let config = ServerConfig {
        max_connections: 50,
        request_timeout: Duration::from_secs(5),
        ..ServerConfig::default()
    };
    let server = IpcServer::new_mock(config);
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let handle = server.start(addr).await.expect("server should start");
    let url = format!("http://127.0.0.1:{}", handle.addr().port());
    // Poll until server is ready (up to 500ms)
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(10)).await;
        if IpcClient::connect(&url).await.is_ok() {
            break;
        }
    }
    (handle, url)
}

async fn connect_client(url: &str) -> IpcClient {
    let tc = TransportConfig {
        connect_timeout: Duration::from_secs(2),
        request_timeout: Duration::from_secs(2),
        health_check_interval: Duration::ZERO,
        retry_policy: RetryPolicy {
            max_retries: 2,
            base_delay: Duration::from_millis(50),
            max_delay: Duration::from_millis(500),
        },
        ..TransportConfig::default()
    };
    IpcClient::connect_with_transport(url, ClientConfig::default(), tc)
        .await
        .expect("client should connect")
}

// ===========================================================================
// 1. Message serialization round-trips (protobuf)
// ===========================================================================

#[test]
fn proto_health_event_round_trip() {
    let event = HealthEvent {
        timestamp: 1_700_000_000,
        r#type: HealthEventType::Warning.into(),
        message: "jitter spike".to_string(),
        device_id: "dev-1".to_string(),
        error_code: "E1001".to_string(),
        metadata: HashMap::from([("key".to_string(), "val".to_string())]),
        performance: Some(PerformanceMetrics {
            jitter_p99_ms: 0.4,
            hid_latency_p99_us: 280.0,
            missed_ticks: 2,
            dropped_frames: 0,
            cpu_usage_percent: 12.5,
            memory_usage_bytes: 1024 * 1024,
        }),
    };
    let mut buf = Vec::new();
    event.encode(&mut buf).unwrap();
    let decoded = HealthEvent::decode(buf.as_slice()).unwrap();
    assert_eq!(decoded.timestamp, 1_700_000_000);
    assert_eq!(decoded.r#type, HealthEventType::Warning as i32);
    assert_eq!(decoded.error_code, "E1001");
    assert!(decoded.performance.is_some());
    let perf = decoded.performance.unwrap();
    assert!((perf.jitter_p99_ms - 0.4).abs() < 1e-5);
    assert_eq!(perf.missed_ticks, 2);
}

#[test]
fn proto_device_with_capabilities_and_health_round_trip() {
    let device = Device {
        id: "stick-1".to_string(),
        name: "VKB Gladiator NXT EVO".to_string(),
        r#type: DeviceType::Joystick.into(),
        status: DeviceStatus::Connected.into(),
        capabilities: Some(DeviceCapabilities {
            supports_force_feedback: true,
            supports_raw_torque: false,
            max_torque_nm: 25,
            min_period_us: 500,
            has_health_stream: true,
            supported_protocols: vec!["hid".to_string(), "di8".to_string()],
        }),
        health: Some(DeviceHealth {
            temperature_celsius: 42.5,
            current_amperes: 1.2,
            packet_loss_count: 3,
            last_seen_timestamp: 1_700_000_000,
            active_faults: vec!["overheat".to_string()],
        }),
        metadata: HashMap::from([("firmware".to_string(), "2.1.0".to_string())]),
    };
    let mut buf = Vec::new();
    device.encode(&mut buf).unwrap();
    let decoded = Device::decode(buf.as_slice()).unwrap();
    assert_eq!(decoded.id, "stick-1");
    let caps = decoded.capabilities.unwrap();
    assert!(caps.supports_force_feedback);
    assert_eq!(caps.max_torque_nm, 25);
    let health = decoded.health.unwrap();
    assert_eq!(health.packet_loss_count, 3);
    assert_eq!(health.active_faults, vec!["overheat"]);
}

#[test]
fn proto_curve_conflict_with_metadata_round_trip() {
    let conflict = CurveConflict {
        axis_name: "pitch".to_string(),
        conflict_type: ConflictType::DoubleCurve.into(),
        severity: ConflictSeverity::High.into(),
        description: "Both sim and profile apply curves".to_string(),
        suggested_resolutions: vec![],
        metadata: Some(ConflictMetadata {
            sim_curve_strength: 0.7,
            profile_curve_strength: 0.5,
            combined_nonlinearity: 0.85,
            test_inputs: vec![0.0, 0.25, 0.5, 0.75, 1.0],
            expected_outputs: vec![0.0, 0.25, 0.5, 0.75, 1.0],
            actual_outputs: vec![0.0, 0.1, 0.35, 0.7, 1.0],
            detection_timestamp: 1_700_000_000,
        }),
    };
    let mut buf = Vec::new();
    conflict.encode(&mut buf).unwrap();
    let decoded = CurveConflict::decode(buf.as_slice()).unwrap();
    assert_eq!(decoded.axis_name, "pitch");
    assert_eq!(decoded.conflict_type, ConflictType::DoubleCurve as i32);
    let meta = decoded.metadata.unwrap();
    assert_eq!(meta.test_inputs.len(), 5);
    assert!((meta.combined_nonlinearity - 0.85).abs() < 1e-6);
}

#[test]
fn proto_all_enum_values_survive_round_trip() {
    // DeviceType
    for val in [
        DeviceType::Unspecified,
        DeviceType::Joystick,
        DeviceType::Throttle,
        DeviceType::Rudder,
        DeviceType::Panel,
        DeviceType::ForceFeedback,
        DeviceType::Streamdeck,
    ] {
        let d = Device {
            r#type: val.into(),
            ..Default::default()
        };
        let mut buf = Vec::new();
        d.encode(&mut buf).unwrap();
        let decoded = Device::decode(buf.as_slice()).unwrap();
        assert_eq!(decoded.r#type, val as i32);
    }

    // ServiceStatus
    for val in [
        ServiceStatus::Unspecified,
        ServiceStatus::Starting,
        ServiceStatus::Running,
        ServiceStatus::Degraded,
        ServiceStatus::Stopping,
    ] {
        let resp = proto::GetServiceInfoResponse {
            status: val.into(),
            ..Default::default()
        };
        let mut buf = Vec::new();
        resp.encode(&mut buf).unwrap();
        let decoded = proto::GetServiceInfoResponse::decode(buf.as_slice()).unwrap();
        assert_eq!(decoded.status, val as i32);
    }
}

#[test]
fn proto_validation_error_all_types_round_trip() {
    for err_type in [
        ValidationErrorType::Unspecified,
        ValidationErrorType::Schema,
        ValidationErrorType::Monotonic,
        ValidationErrorType::Range,
        ValidationErrorType::Conflict,
    ] {
        let ve = ValidationError {
            error_type: err_type.into(),
            field_path: "test".to_string(),
            error_message: "msg".to_string(),
            line_number: 1,
            column_number: 1,
        };
        let resp = ApplyProfileResponse {
            validation_errors: vec![ve],
            ..Default::default()
        };
        let mut buf = Vec::new();
        resp.encode(&mut buf).unwrap();
        let decoded = ApplyProfileResponse::decode(buf.as_slice()).unwrap();
        assert_eq!(decoded.validation_errors[0].error_type, err_type as i32);
    }
}

// ===========================================================================
// 2. Connection lifecycle: connect → negotiate → RPC → disconnect
// ===========================================================================

#[tokio::test]
async fn lifecycle_connect_negotiate_rpc_disconnect() {
    let (handle, url) = start_mock_server().await;
    let mut client = connect_client(&url).await;

    // Negotiate features
    let neg = client.negotiate_features().await.unwrap();
    assert!(neg.success);
    assert!(!neg.enabled_features.is_empty());

    // RPC: service info
    let info = client.get_service_info().await.unwrap();
    assert_eq!(info.version, PROTOCOL_VERSION);
    assert_eq!(info.status(), ServiceStatus::Running);

    // RPC: list devices
    let devices = client.list_devices().await.unwrap();
    assert_eq!(devices.total_count, 0);

    // Disconnect
    client.disconnect().await;
    assert!(!client.is_connected().await);

    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn lifecycle_multiple_rpcs_on_single_connection() {
    let (handle, url) = start_mock_server().await;
    let mut client = connect_client(&url).await;

    // Issue multiple RPCs sequentially on the same connection
    for _ in 0..10 {
        let info = client.get_service_info().await.unwrap();
        assert_eq!(info.version, PROTOCOL_VERSION);
    }

    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn lifecycle_all_rpc_endpoints_callable() {
    let (handle, url) = start_mock_server().await;
    let mut client = connect_client(&url).await;

    assert!(client.get_service_info().await.is_ok());
    assert!(client.list_devices().await.is_ok());
    assert!(client.negotiate_features().await.is_ok());
    assert!(client
        .apply_profile(ApplyProfileRequest {
            profile_json: "{}".to_string(),
            validate_only: true,
            force_apply: false,
        })
        .await
        .is_ok());
    assert!(client
        .detect_curve_conflicts(DetectCurveConflictsRequest::default())
        .await
        .is_ok());
    assert!(client
        .resolve_curve_conflict(ResolveCurveConflictRequest::default())
        .await
        .is_ok());
    assert!(client
        .one_click_resolve(OneClickResolveRequest::default())
        .await
        .is_ok());
    assert!(client
        .set_capability_mode(SetCapabilityModeRequest::default())
        .await
        .is_ok());
    assert!(client
        .get_capability_mode(GetCapabilityModeRequest::default())
        .await
        .is_ok());
    assert!(client.get_security_status().await.is_ok());
    assert!(client
        .configure_telemetry(ConfigureTelemetryRequest {
            enabled: true,
            data_types: vec!["Performance".to_string()],
        })
        .await
        .is_ok());
    assert!(client.get_support_bundle().await.is_ok());

    handle.shutdown().await.unwrap();
}

// ===========================================================================
// 3. Subscription management
// ===========================================================================

#[test]
fn subscription_subscribe_to_all_topics() {
    let mut mgr = SubscriptionManager::new();
    let mut handles = Vec::new();
    for &topic in Topic::ALL {
        handles.push(mgr.subscribe(topic, SubscriptionFilter::default()));
    }
    assert_eq!(mgr.active_count(), Topic::ALL.len());

    // Broadcast to each topic reaches exactly one subscriber
    for &topic in Topic::ALL {
        let msg = BroadcastMessage {
            topic,
            payload: "test".to_string(),
            device_id: None,
            axis_id: None,
        };
        let ids = mgr.broadcast(&msg);
        assert_eq!(ids.len(), 1, "topic {topic} should have exactly 1 subscriber");
    }
}

#[test]
fn subscription_unsubscribe_stops_delivery() {
    let mut mgr = SubscriptionManager::new();
    let h = mgr.subscribe(Topic::DeviceEvents, SubscriptionFilter::default());

    // Before unsubscribe: delivered
    let msg = BroadcastMessage {
        topic: Topic::DeviceEvents,
        payload: "event".to_string(),
        device_id: None,
        axis_id: None,
    };
    assert_eq!(mgr.broadcast(&msg).len(), 1);

    // After unsubscribe: not delivered
    mgr.unsubscribe(&h);
    assert!(mgr.broadcast(&msg).is_empty());
}

#[test]
fn subscription_filter_device_and_axis_combined() {
    let mut mgr = SubscriptionManager::new();
    let filter = SubscriptionFilter {
        device_id: Some("js-1".to_string()),
        axis_id: Some("pitch".to_string()),
        ..Default::default()
    };
    let h = mgr.subscribe(Topic::AxisData, filter);

    // Match both
    let msg = BroadcastMessage {
        topic: Topic::AxisData,
        payload: "val".to_string(),
        device_id: Some("js-1".to_string()),
        axis_id: Some("pitch".to_string()),
    };
    assert_eq!(mgr.broadcast(&msg), vec![h.id]);

    // Wrong device
    let msg2 = BroadcastMessage {
        topic: Topic::AxisData,
        payload: "val".to_string(),
        device_id: Some("js-2".to_string()),
        axis_id: Some("pitch".to_string()),
    };
    assert!(mgr.broadcast(&msg2).is_empty());

    // Wrong axis
    let msg3 = BroadcastMessage {
        topic: Topic::AxisData,
        payload: "val".to_string(),
        device_id: Some("js-1".to_string()),
        axis_id: Some("roll".to_string()),
    };
    assert!(mgr.broadcast(&msg3).is_empty());
}

#[test]
fn subscription_resubscribe_after_cancel() {
    let mut mgr = SubscriptionManager::new();
    let h1 = mgr.subscribe(Topic::HealthStatus, SubscriptionFilter::default());
    h1.cancel();
    assert_eq!(mgr.active_count(), 0);

    // Resubscribe after cancel
    let h2 = mgr.subscribe(Topic::HealthStatus, SubscriptionFilter::default());
    assert!(h2.is_active());
    assert_eq!(mgr.active_count(), 1);
    assert_ne!(h1.id, h2.id);
}

#[tokio::test]
async fn subscription_health_stream_establishes() {
    let (handle, url) = start_mock_server().await;
    let mut client = connect_client(&url).await;

    let rx = client
        .subscribe_health(HealthSubscribeRequest::default())
        .await;
    assert!(rx.is_ok(), "health subscription should be established");

    // Drop stream and client before shutdown to avoid hanging on graceful drain.
    drop(rx);
    drop(client);
    // Graceful shutdown may block while tonic drains the health stream;
    // a timeout prevents the test from hanging indefinitely.
    if let Ok(result) = tokio::time::timeout(Duration::from_secs(2), handle.shutdown()).await {
        result.unwrap();
    }
}

// ===========================================================================
// 4. Reconnection behavior
// ===========================================================================

#[tokio::test]
async fn reconnection_after_disconnect() {
    let (handle, url) = start_mock_server().await;
    let mut client = connect_client(&url).await;

    // Verify connection
    assert!(client.is_connected().await);

    // Disconnect
    client.disconnect().await;
    assert!(!client.is_connected().await);

    // Reconnect to same server
    assert!(client.reconnect().await.is_ok());
    assert!(client.is_connected().await);

    // RPCs still work
    let info = client.get_service_info().await.unwrap();
    assert_eq!(info.version, PROTOCOL_VERSION);

    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn reconnection_fails_when_server_down() {
    let (handle, url) = start_mock_server().await;
    let mut client = connect_client(&url).await;
    assert!(client.is_connected().await);

    // Shut down server
    handle.shutdown().await.unwrap();

    // Give time for the connection to notice
    tokio::time::sleep(Duration::from_millis(100)).await;

    // RPCs should fail
    assert!(client.get_service_info().await.is_err());
}

#[tokio::test]
async fn client_can_connect_to_new_server_instance() {
    // Start server 1
    let (handle1, url1) = start_mock_server().await;
    let mut client = connect_client(&url1).await;
    assert!(client.get_service_info().await.is_ok());

    // Kill server 1
    handle1.shutdown().await.unwrap();
    assert!(client.get_service_info().await.is_err());

    // Start server 2 on new port
    let (handle2, url2) = start_mock_server().await;
    let mut client2 = connect_client(&url2).await;
    assert!(client2.get_service_info().await.is_ok());

    handle2.shutdown().await.unwrap();
}

// ===========================================================================
// 5. Backpressure handling
// ===========================================================================

#[test]
fn backpressure_subscription_throttle_drops_excess() {
    let mut mgr = SubscriptionManager::new();
    let filter = SubscriptionFilter {
        min_interval_ms: Some(1_000), // 1s throttle
        ..Default::default()
    };
    let h = mgr.subscribe(Topic::AxisData, filter);

    // First message delivered
    let msg = BroadcastMessage {
        topic: Topic::AxisData,
        payload: "v1".to_string(),
        device_id: None,
        axis_id: None,
    };
    assert_eq!(mgr.broadcast(&msg).len(), 1);

    // Rapid subsequent messages dropped
    for i in 2..=100 {
        let msg = BroadcastMessage {
            topic: Topic::AxisData,
            payload: format!("v{i}"),
            device_id: None,
            axis_id: None,
        };
        let ids = mgr.broadcast(&msg);
        assert!(ids.is_empty(), "message {i} should be throttled");
    }

    // Handle still active (not broken, just throttled)
    assert!(h.is_active());
}

#[test]
fn backpressure_changed_only_deduplicates() {
    let mut mgr = SubscriptionManager::new();
    let filter = SubscriptionFilter {
        changed_only: true,
        ..Default::default()
    };
    let h = mgr.subscribe(Topic::HealthStatus, filter);

    let msg = BroadcastMessage {
        topic: Topic::HealthStatus,
        payload: "state-A".to_string(),
        device_id: None,
        axis_id: None,
    };

    // First: delivered
    assert_eq!(mgr.broadcast(&msg), vec![h.id]);
    // Same: suppressed
    assert!(mgr.broadcast(&msg).is_empty());
    // Different: delivered
    let msg2 = BroadcastMessage {
        topic: Topic::HealthStatus,
        payload: "state-B".to_string(),
        device_id: None,
        axis_id: None,
    };
    assert_eq!(mgr.broadcast(&msg2), vec![h.id]);
}

#[test]
fn backpressure_producer_not_blocked_by_cancelled_subscriber() {
    let mut mgr = SubscriptionManager::new();
    let h1 = mgr.subscribe(Topic::AxisData, SubscriptionFilter::default());
    let h2 = mgr.subscribe(Topic::AxisData, SubscriptionFilter::default());

    // Cancel one subscriber
    h1.cancel();

    // Producer still delivers to remaining subscriber
    let msg = BroadcastMessage {
        topic: Topic::AxisData,
        payload: "data".to_string(),
        device_id: None,
        axis_id: None,
    };
    let ids = mgr.broadcast(&msg);
    assert_eq!(ids.len(), 1);
    assert_eq!(ids[0], h2.id);
}

// ===========================================================================
// 6. Concurrent client tests
// ===========================================================================

#[tokio::test]
async fn concurrent_clients_each_get_correct_responses() {
    let (handle, url) = start_mock_server().await;

    let mut tasks = Vec::new();
    for i in 0..10 {
        let url = url.clone();
        tasks.push(tokio::spawn(async move {
            let mut client = connect_client(&url).await;
            let info = client.get_service_info().await.unwrap();
            assert_eq!(info.version, PROTOCOL_VERSION, "client {i} version mismatch");
            let devices = client.list_devices().await.unwrap();
            assert_eq!(devices.total_count, 0);
            i
        }));
    }

    for task in tasks {
        let client_id = task.await.unwrap();
        assert!(client_id < 10);
    }

    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn concurrent_clients_different_rpcs() {
    let (handle, url) = start_mock_server().await;

    let u1 = url.clone();
    let t1 = tokio::spawn(async move {
        let mut c = connect_client(&u1).await;
        c.get_service_info().await.unwrap()
    });

    let u2 = url.clone();
    let t2 = tokio::spawn(async move {
        let mut c = connect_client(&u2).await;
        c.list_devices().await.unwrap()
    });

    let u3 = url.clone();
    let t3 = tokio::spawn(async move {
        let mut c = connect_client(&u3).await;
        c.negotiate_features().await.unwrap()
    });

    let u4 = url.clone();
    let t4 = tokio::spawn(async move {
        let mut c = connect_client(&u4).await;
        c.get_security_status().await.unwrap()
    });

    let (r1, r2, r3, r4) = tokio::join!(t1, t2, t3, t4);
    assert_eq!(r1.unwrap().version, PROTOCOL_VERSION);
    assert!(r2.unwrap().devices.is_empty());
    assert!(r3.unwrap().success);
    assert!(r4.unwrap().success);

    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn concurrent_subscription_isolation() {
    let mut mgr = SubscriptionManager::new();

    // Simulate two "clients" subscribing to different topics
    let h_axis = mgr.subscribe(
        Topic::AxisData,
        SubscriptionFilter {
            device_id: Some("client-a-device".to_string()),
            ..Default::default()
        },
    );
    let h_health = mgr.subscribe(Topic::HealthStatus, SubscriptionFilter::default());

    // Axis event only goes to axis subscriber
    let axis_msg = BroadcastMessage {
        topic: Topic::AxisData,
        payload: "axis-val".to_string(),
        device_id: Some("client-a-device".to_string()),
        axis_id: None,
    };
    let ids = mgr.broadcast(&axis_msg);
    assert_eq!(ids, vec![h_axis.id]);

    // Health event only goes to health subscriber
    let health_msg = BroadcastMessage {
        topic: Topic::HealthStatus,
        payload: "ok".to_string(),
        device_id: None,
        axis_id: None,
    };
    let ids = mgr.broadcast(&health_msg);
    assert_eq!(ids, vec![h_health.id]);
}

// ===========================================================================
// 7. Health check protocol
// ===========================================================================

#[tokio::test]
async fn health_check_service_info_returns_running() {
    let (handle, url) = start_mock_server().await;
    let mut client = connect_client(&url).await;

    let info = client.get_service_info().await.unwrap();
    assert_eq!(info.status(), ServiceStatus::Running);
    assert_eq!(info.version, PROTOCOL_VERSION);

    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn health_check_via_is_connected() {
    let (handle, url) = start_mock_server().await;
    let mut client = connect_client(&url).await;
    assert!(client.is_connected().await);

    client.disconnect().await;
    assert!(!client.is_connected().await);

    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn health_subscribe_stream_can_be_established() {
    let (handle, url) = start_mock_server().await;
    let mut client = connect_client(&url).await;

    let mut rx = client
        .subscribe_health(HealthSubscribeRequest {
            filter_types: vec![HealthEventType::Warning.into()],
            device_ids: vec!["dev-1".to_string()],
            include_performance_metrics: true,
        })
        .await
        .unwrap();

    // Drop client → stream should eventually close
    drop(client);
    let result = tokio::time::timeout(Duration::from_millis(300), rx.recv()).await;
    assert!(result.is_err() || result.unwrap().is_none());

    drop(rx);
    // Graceful shutdown may block while tonic drains the health stream;
    // a timeout prevents the test from hanging indefinitely.
    if let Ok(result) = tokio::time::timeout(Duration::from_secs(2), handle.shutdown()).await {
        result.unwrap();
    }
}

// ===========================================================================
// 8. Error handling
// ===========================================================================

#[tokio::test]
async fn error_connect_refused() {
    // Port 19997 is deliberately hardcoded: we need a port that is NOT listening
    // to verify "connection refused" behaviour. Using port 0 would not work here
    // because the OS would assign a valid ephemeral port during bind, not connect.
    let result = IpcClient::connect("http://127.0.0.1:19997").await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("Failed to connect") || msg.contains("Connection"),
        "unexpected error: {msg}"
    );
}

#[tokio::test]
async fn error_invalid_address() {
    let result = IpcClient::connect("not-a-url").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn error_rpc_after_server_shutdown() {
    let (handle, url) = start_mock_server().await;
    let mut client = connect_client(&url).await;
    assert!(client.get_service_info().await.is_ok());

    handle.shutdown().await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    assert!(client.get_service_info().await.is_err());
}

#[tokio::test]
async fn error_rpc_after_disconnect() {
    let (handle, url) = start_mock_server().await;
    let mut client = connect_client(&url).await;
    client.disconnect().await;

    // get_service_info has no auto-reconnect so it should fail after disconnect.
    // (list_devices has built-in reconnect logic and would succeed.)
    assert!(client.get_service_info().await.is_err());

    handle.shutdown().await.unwrap();
}

#[test]
fn error_ipc_error_variants_display() {
    let err = IpcError::VersionMismatch {
        client: "2.0.0".to_string(),
        server: "1.0.0".to_string(),
    };
    assert!(err.to_string().contains("2.0.0"));
    assert!(err.to_string().contains("1.0.0"));

    let err = IpcError::UnsupportedFeature {
        feature: "magic".to_string(),
    };
    assert!(err.to_string().contains("magic"));

    let err = IpcError::ConnectionFailed {
        reason: "pipe broken".to_string(),
    };
    assert!(err.to_string().contains("pipe broken"));
}

#[test]
fn error_ipc_error_from_grpc_status() {
    let status = tonic::Status::permission_denied("not authorized");
    let err: IpcError = status.into();
    assert!(err.to_string().contains("not authorized") || err.to_string().contains("gRPC"));
}

#[test]
fn error_ipc_error_from_serde_json() {
    let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
    let err: IpcError = json_err.into();
    assert!(err.to_string().contains("Serialization"));
}

#[test]
fn error_negotiate_version_mismatch() {
    let request = NegotiateFeaturesRequest {
        client_version: "999.0.0".to_string(),
        supported_features: vec![],
        preferred_transport: TransportType::Unspecified.into(),
    };
    let server_features: Vec<String> = SUPPORTED_FEATURES.iter().map(|s| s.to_string()).collect();
    let response = negotiation::negotiate_features(&request, &server_features).unwrap();
    assert!(!response.success);
    assert!(!response.error_message.is_empty());
}

#[test]
fn error_invalid_version_parse() {
    assert!(Version::parse("").is_err());
    assert!(Version::parse("1").is_err());
    assert!(Version::parse("1.2").is_err());
    assert!(Version::parse("1.2.3.4").is_err());
    assert!(Version::parse("a.b.c").is_err());
}

// ===========================================================================
// 9. Property tests — message validation
// ===========================================================================

mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_health_event_timestamp_preserved(ts in any::<i64>()) {
            let event = HealthEvent {
                timestamp: ts,
                ..Default::default()
            };
            let mut buf = Vec::new();
            event.encode(&mut buf).unwrap();
            let decoded = HealthEvent::decode(buf.as_slice()).unwrap();
            prop_assert_eq!(decoded.timestamp, ts);
        }

        #[test]
        fn prop_device_name_preserved(name in "[a-zA-Z0-9 _-]{0,256}") {
            let device = Device {
                name: name.clone(),
                ..Default::default()
            };
            let mut buf = Vec::new();
            device.encode(&mut buf).unwrap();
            let decoded = Device::decode(buf.as_slice()).unwrap();
            prop_assert_eq!(decoded.name, name);
        }

        #[test]
        fn prop_performance_metrics_fields_preserved(
            jitter in 0.0f32..100.0,
            latency in 0.0f32..10000.0,
            ticks in any::<u32>(),
            frames in any::<u32>(),
            cpu in 0.0f32..100.0,
            mem in any::<u64>(),
        ) {
            let metrics = PerformanceMetrics {
                jitter_p99_ms: jitter,
                hid_latency_p99_us: latency,
                missed_ticks: ticks,
                dropped_frames: frames,
                cpu_usage_percent: cpu,
                memory_usage_bytes: mem,
            };
            let mut buf = Vec::new();
            metrics.encode(&mut buf).unwrap();
            let decoded = PerformanceMetrics::decode(buf.as_slice()).unwrap();
            prop_assert_eq!(decoded.missed_ticks, ticks);
            prop_assert_eq!(decoded.dropped_frames, frames);
            prop_assert_eq!(decoded.memory_usage_bytes, mem);
        }

        #[test]
        fn prop_ipc_message_json_round_trip_device_connected(
            vid in any::<u16>(),
            pid in any::<u16>(),
        ) {
            let msg = IpcMessage::DeviceConnected {
                device_id: format!("dev-{vid}"),
                name: "Test Device".to_string(),
                vid,
                pid,
            };
            let json = msg.to_json();
            let restored = IpcMessage::from_json(&json).unwrap();
            prop_assert_eq!(msg, restored);
        }

        #[test]
        fn prop_ipc_message_json_round_trip_telemetry(
            alt in -1000i32..60000,
            speed in 0i32..600,
            hdg in 0i32..360,
        ) {
            // Use integer-sourced f64 to avoid JSON float precision drift.
            let msg = IpcMessage::TelemetryUpdate {
                altitude: f64::from(alt),
                airspeed: f64::from(speed),
                heading: f64::from(hdg),
            };
            let json = msg.to_json();
            let restored = IpcMessage::from_json(&json).unwrap();
            prop_assert_eq!(msg, restored);
        }

        #[test]
        fn prop_arbitrary_bytes_decode_no_panic(
            bytes in proptest::collection::vec(any::<u8>(), 0..1024)
        ) {
            // Must not panic — Ok or Err both acceptable
            let _ = HealthEvent::decode(bytes.as_slice());
            let _ = Device::decode(bytes.as_slice());
            let _ = ListDevicesResponse::decode(bytes.as_slice());
        }

        #[test]
        fn prop_version_display_round_trips(
            major in 0u32..1000,
            minor in 0u32..1000,
            patch in 0u32..1000,
        ) {
            let v = Version { major, minor, patch };
            let s = v.to_string();
            let v2 = Version::parse(&s).unwrap();
            prop_assert_eq!(v, v2);
        }
    }
}

// ===========================================================================
// 10. IpcMessage (JSON) round-trips for all variants
// ===========================================================================

#[test]
fn ipc_message_all_variants_round_trip() {
    let messages = vec![
        IpcMessage::DeviceConnected {
            device_id: "d1".into(),
            name: "Joystick".into(),
            vid: 0x1234,
            pid: 0x5678,
        },
        IpcMessage::DeviceDisconnected {
            device_id: "d1".into(),
            reason: "unplugged".into(),
        },
        IpcMessage::DeviceInput {
            device_id: "d1".into(),
            axes: vec![0.0, 0.5, -1.0, 1.0],
            buttons: vec![true, false, true],
        },
        IpcMessage::ProfileActivated {
            name: "combat".into(),
            aircraft: Some("F-16C".into()),
        },
        IpcMessage::ProfileDeactivated {
            name: "old".into(),
        },
        IpcMessage::ProfileError {
            name: "bad".into(),
            error: "parse failed".into(),
        },
        IpcMessage::ServiceStatus {
            status: ServiceState::Running,
            uptime_secs: 86400,
        },
        IpcMessage::HealthReport {
            components: vec![
                ComponentStatus {
                    name: "axis-engine".into(),
                    healthy: true,
                    detail: None,
                },
                ComponentStatus {
                    name: "ffb-engine".into(),
                    healthy: false,
                    detail: Some("fault".into()),
                },
            ],
        },
        IpcMessage::SimConnected {
            sim_type: "msfs".into(),
            version: "2024".into(),
        },
        IpcMessage::SimDisconnected {
            sim_type: "xplane".into(),
        },
        IpcMessage::TelemetryUpdate {
            altitude: 35000.0,
            airspeed: 250.0,
            heading: 90.0,
        },
    ];

    for msg in &messages {
        let json = msg.to_json();
        let restored = IpcMessage::from_json(&json).unwrap();
        assert_eq!(msg, &restored, "round-trip failed for {}", msg.message_type());
    }
}

#[test]
fn ipc_message_type_tag_present_in_json() {
    let msg = IpcMessage::SimConnected {
        sim_type: "dcs".into(),
        version: "2.9".into(),
    };
    let json = msg.to_json();
    assert!(json.contains(r#""type":"SimConnected""#));
}

#[test]
fn ipc_message_invalid_json_errors() {
    assert!(IpcMessage::from_json("").is_err());
    assert!(IpcMessage::from_json("null").is_err());
    assert!(IpcMessage::from_json("42").is_err());
    assert!(IpcMessage::from_json(r#"{"type":"Bogus"}"#).is_err());
}

// ===========================================================================
// Connection pool depth tests
// ===========================================================================

#[test]
fn pool_connect_reuse_after_disconnect() {
    let mut pool = ConnectionPool::new(2, 300);
    pool.connect("c1", "client-a", 100).unwrap();
    pool.connect("c2", "client-b", 100).unwrap();
    assert!(pool.is_full());

    // Disconnect frees a slot
    pool.disconnect("c1");
    assert!(!pool.is_full());

    // Re-use the slot
    pool.connect("c3", "client-c", 200).unwrap();
    assert!(pool.is_full());
    assert_eq!(pool.active_count(), 2);
}

#[test]
fn pool_prune_frees_slots_for_new_connections() {
    let mut pool = ConnectionPool::new(2, 60);
    pool.connect("old1", "a", 0).unwrap();
    pool.connect("old2", "b", 0).unwrap();
    assert!(pool.is_full());

    // Prune at t=120 (idle_timeout=60 → threshold=60, both connections at t=0)
    let pruned = pool.prune_idle(120);
    assert_eq!(pruned.len(), 2);
    assert_eq!(pool.active_count(), 0);

    // Can now connect new clients
    pool.connect("new1", "c", 120).unwrap();
    assert_eq!(pool.active_count(), 1);
}

#[test]
fn pool_activity_keeps_connection_alive_during_prune() {
    let mut pool = ConnectionPool::new(10, 60);
    pool.connect("active", "a", 0).unwrap();
    pool.connect("idle", "b", 0).unwrap();

    // Keep "active" alive
    pool.activity("active", 100).unwrap();

    // Prune at t=100: "idle" last_activity=0 < 100-60=40 → pruned
    let pruned = pool.prune_idle(100);
    assert_eq!(pruned, vec!["idle"]);
    assert!(pool.get("active").is_some());
    assert_eq!(pool.get("active").unwrap().message_count, 1);
}

#[test]
fn pool_all_connections_returns_correct_set() {
    let mut pool = ConnectionPool::new(10, 300);
    pool.connect("a", "client-a", 0).unwrap();
    pool.connect("b", "client-b", 0).unwrap();
    pool.connect("c", "client-c", 0).unwrap();

    let all = pool.all_connections();
    assert_eq!(all.len(), 3);
    let ids: Vec<&str> = all.iter().map(|c| c.id.as_str()).collect();
    assert!(ids.contains(&"a"));
    assert!(ids.contains(&"b"));
    assert!(ids.contains(&"c"));
}

#[test]
fn pool_error_display() {
    let err = PoolError::PoolFull { max: 5 };
    assert!(err.to_string().contains("5"));

    let err = PoolError::DuplicateId {
        id: "dup".to_string(),
    };
    assert!(err.to_string().contains("dup"));

    let err = PoolError::NotFound {
        id: "missing".to_string(),
    };
    assert!(err.to_string().contains("missing"));
}

// ===========================================================================
// Rate limiter depth tests
// ===========================================================================

#[test]
fn rate_limiter_independent_buckets_per_client() {
    let config = RateLimitConfig {
        max_tokens: 3,
        refill_rate: 0.0, // no refill
        refill_interval: Duration::from_secs(1),
    };
    let mut rl = RateLimiter::new(config);

    // Client A uses all tokens
    for _ in 0..3 {
        assert_eq!(rl.check("client-a"), RateLimitResult::Allowed);
    }
    assert!(matches!(
        rl.check("client-a"),
        RateLimitResult::Limited { .. }
    ));

    // Client B still has full bucket
    for _ in 0..3 {
        assert_eq!(rl.check("client-b"), RateLimitResult::Allowed);
    }
}

#[test]
fn rate_limiter_custom_client_config() {
    let default_config = RateLimitConfig {
        max_tokens: 10,
        refill_rate: 10.0,
        refill_interval: Duration::from_millis(100),
    };
    let mut rl = RateLimiter::new(default_config);

    // Give VIP a smaller bucket
    rl.configure_client(
        "vip",
        RateLimitConfig {
            max_tokens: 2,
            refill_rate: 1.0,
            refill_interval: Duration::from_secs(1),
        },
    );

    assert_eq!(rl.check("vip"), RateLimitResult::Allowed);
    assert_eq!(rl.check("vip"), RateLimitResult::Allowed);
    assert!(matches!(rl.check("vip"), RateLimitResult::Limited { .. }));

    // Regular client unaffected
    for _ in 0..10 {
        assert_eq!(rl.check("regular"), RateLimitResult::Allowed);
    }
}

#[test]
fn rate_limiter_reset_clears_all_state() {
    let config = RateLimitConfig {
        max_tokens: 5,
        refill_rate: 0.0,
        refill_interval: Duration::from_secs(1),
    };
    let mut rl = RateLimiter::new(config);

    rl.check("a");
    rl.check("b");
    assert_eq!(rl.client_count(), 2);

    rl.reset();
    assert_eq!(rl.client_count(), 0);
    assert_eq!(rl.remaining_tokens("a"), None);
}

#[test]
fn rate_limiter_cost_based_consumption() {
    let config = RateLimitConfig {
        max_tokens: 10,
        refill_rate: 0.0,
        refill_interval: Duration::from_secs(1),
    };
    let mut rl = RateLimiter::new(config);

    // Cost 7 → 3 remaining
    assert_eq!(rl.check_with_cost("c", 7), RateLimitResult::Allowed);
    assert_eq!(rl.remaining_tokens("c"), Some(3));

    // Cost 4 > 3 remaining → limited
    assert!(matches!(
        rl.check_with_cost("c", 4),
        RateLimitResult::Limited { .. }
    ));

    // Cost 3 = exactly remaining → allowed
    assert_eq!(rl.check_with_cost("c", 3), RateLimitResult::Allowed);
    assert_eq!(rl.remaining_tokens("c"), Some(0));
}

// ===========================================================================
// Negotiation depth tests
// ===========================================================================

#[test]
fn negotiation_feature_intersection() {
    let request = NegotiateFeaturesRequest {
        client_version: "1.0.0".to_string(),
        supported_features: vec![
            "device-management".to_string(),
            "unknown-feature".to_string(),
        ],
        preferred_transport: TransportType::Unspecified.into(),
    };
    let server_features = vec![
        "device-management".to_string(),
        "health-monitoring".to_string(),
    ];
    let resp = negotiation::negotiate_features(&request, &server_features).unwrap();
    assert!(resp.success);
    assert_eq!(resp.enabled_features, vec!["device-management"]);
}

#[test]
fn negotiation_empty_intersection() {
    let request = NegotiateFeaturesRequest {
        client_version: "1.0.0".to_string(),
        supported_features: vec!["feature-x".to_string()],
        preferred_transport: TransportType::Unspecified.into(),
    };
    let server_features = vec!["feature-y".to_string()];
    let resp = negotiation::negotiate_features(&request, &server_features).unwrap();
    assert!(resp.success); // Success but no features
    assert!(resp.enabled_features.is_empty());
}

#[test]
fn negotiation_validate_required_features() {
    let enabled = vec!["device-management".to_string(), "health-monitoring".to_string()];
    let required = vec!["device-management".to_string()];
    assert!(negotiation::validate_required_features(&enabled, &required).is_ok());

    let missing_required = vec!["force-feedback".to_string()];
    let err = negotiation::validate_required_features(&enabled, &missing_required).unwrap_err();
    assert!(err.to_string().contains("force-feedback"));
}

#[test]
fn negotiation_breaking_change_detection() {
    let old = "service Svc {\n  rpc Old(Req) returns (Resp);\n  rpc Keep(Req) returns (Resp);\n}";
    let new = "service Svc {\n  rpc Keep(Req) returns (Resp);\n  rpc Added(Req) returns (Resp);\n}";
    let changes = negotiation::detect_breaking_changes(old, new).unwrap();
    assert!(!changes.is_empty());
    assert!(changes.iter().any(|c| c.contains("Old")));
}

// ===========================================================================
// Transport layer depth tests
// ===========================================================================

#[test]
fn transport_config_defaults_are_sane() {
    let cfg = TransportConfig::default();
    assert!(cfg.connect_timeout.as_secs() > 0);
    assert!(cfg.request_timeout.as_secs() > 0);
    assert!(cfg.keepalive_interval.as_secs() > 0);
    assert!(cfg.health_check_interval.as_secs() > 0);
}

#[test]
fn retry_policy_exponential_backoff() {
    let rp = RetryPolicy {
        base_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(10),
        max_retries: 10,
    };
    let d0 = rp.delay_for(0);
    let d1 = rp.delay_for(1);
    let d2 = rp.delay_for(2);
    assert_eq!(d0, Duration::from_millis(100));
    assert_eq!(d1, Duration::from_millis(200));
    assert_eq!(d2, Duration::from_millis(400));
    // Check capping
    assert!(rp.delay_for(20) <= Duration::from_secs(10));
}

#[tokio::test]
async fn retry_policy_succeeds_after_failures() {
    let rp = RetryPolicy {
        max_retries: 3,
        base_delay: Duration::from_millis(1),
        max_delay: Duration::from_millis(10),
    };
    let counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let c = counter.clone();
    let result: Result<&str, &str> = rp
        .retry(|| {
            let c = c.clone();
            async move {
                let n = c.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if n < 2 { Err("transient") } else { Ok("ok") }
            }
        })
        .await;
    assert_eq!(result.unwrap(), "ok");
    assert_eq!(counter.load(std::sync::atomic::Ordering::Relaxed), 3);
}

// ===========================================================================
// Handler-level depth tests (direct GrpcFlightService trait calls)
// ===========================================================================

#[tokio::test]
async fn handler_all_curve_conflict_rpcs() {
    let ctx = Arc::new(MockServiceContext::new());
    let handler = FlightServiceHandler::new(ctx, ServerConfig::default());

    let resp = handler
        .detect_curve_conflicts(Request::new(DetectCurveConflictsRequest {
            axis_names: vec!["pitch".to_string()],
            sim_id: "msfs".to_string(),
            aircraft_id: "c172".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert!(resp.success);

    let resp = handler
        .resolve_curve_conflict(Request::new(ResolveCurveConflictRequest {
            axis_name: "pitch".to_string(),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();
    assert!(resp.success);

    let resp = handler
        .one_click_resolve(Request::new(OneClickResolveRequest {
            axis_name: "pitch".to_string(),
            create_backup: true,
            verify_resolution: true,
        }))
        .await
        .unwrap()
        .into_inner();
    assert!(resp.success);
}

#[tokio::test]
async fn handler_capability_mode_rpcs() {
    let ctx = Arc::new(MockServiceContext::new());
    let handler = FlightServiceHandler::new(ctx, ServerConfig::default());

    let resp = handler
        .set_capability_mode(Request::new(SetCapabilityModeRequest {
            mode: CapabilityMode::Demo.into(),
            axis_names: vec!["pitch".to_string(), "roll".to_string()],
            audit_enabled: true,
        }))
        .await
        .unwrap()
        .into_inner();
    assert!(resp.success);

    let resp = handler
        .get_capability_mode(Request::new(GetCapabilityModeRequest {
            axis_names: vec![],
        }))
        .await
        .unwrap()
        .into_inner();
    assert!(resp.success);
}

#[tokio::test]
async fn handler_security_and_telemetry_rpcs() {
    let ctx = Arc::new(MockServiceContext::new());
    let handler = FlightServiceHandler::new(ctx, ServerConfig::default());

    let resp = handler
        .get_security_status(Request::new(GetSecurityStatusRequest {}))
        .await
        .unwrap()
        .into_inner();
    assert!(resp.success);

    let resp = handler
        .configure_telemetry(Request::new(ConfigureTelemetryRequest {
            enabled: true,
            data_types: vec!["Performance".to_string(), "Errors".to_string()],
        }))
        .await
        .unwrap()
        .into_inner();
    assert!(resp.success);

    let resp = handler
        .get_support_bundle(Request::new(GetSupportBundleRequest {}))
        .await
        .unwrap()
        .into_inner();
    assert!(resp.success);
    assert!(!resp.redacted_data.is_empty());
}

#[tokio::test]
async fn handler_health_subscribe_stream() {
    let ctx = Arc::new(MockServiceContext::new());
    let handler = FlightServiceHandler::new(ctx, ServerConfig::default());

    // Get the broadcast sender to publish events
    let tx = handler.health_sender();

    let resp = handler
        .health_subscribe(Request::new(HealthSubscribeRequest::default()))
        .await
        .unwrap();
    let mut stream = resp.into_inner();

    // Publish an event
    let event = HealthEvent {
        timestamp: 1234,
        r#type: HealthEventType::Info.into(),
        message: "test event".to_string(),
        ..Default::default()
    };
    tx.send(event.clone()).unwrap();

    // Read from stream
    use tokio_stream::StreamExt;
    let received = tokio::time::timeout(Duration::from_millis(500), stream.next())
        .await
        .expect("should receive within timeout")
        .expect("stream should have item")
        .expect("item should be Ok");

    assert_eq!(received.timestamp, 1234);
    assert_eq!(received.message, "test event");
}

// ===========================================================================
// MockServiceContext builder tests
// ===========================================================================

#[test]
fn mock_context_builders() {
    use flight_ipc::handlers::{DeviceInfo, HealthStatus, MetricsSnapshot, ProfileInfo};

    let ctx = MockServiceContext::new()
        .with_device_info(vec![DeviceInfo {
            id: "d1".to_string(),
            name: "Stick".to_string(),
            device_type: "joystick".to_string(),
            connected: true,
        }])
        .with_profiles(vec![ProfileInfo {
            name: "default".to_string(),
            active: true,
            aircraft: None,
        }])
        .with_active_profile("default")
        .with_health(HealthStatus {
            healthy: true,
            uptime_secs: 100,
            components: vec![],
        })
        .with_metrics(MetricsSnapshot {
            jitter_p99_ms: 0.3,
            hid_latency_p99_us: 200.0,
            missed_ticks: 0,
            cpu_usage_percent: 5.0,
            memory_usage_bytes: 1024,
        });

    assert_eq!(ctx.list_device_info().len(), 1);
    assert!(ctx.get_device_info("d1").is_some());
    assert!(ctx.get_device_info("nonexistent").is_none());
    assert_eq!(ctx.list_profiles().len(), 1);
    assert_eq!(ctx.get_active_profile(), Some("default".to_string()));
    assert!(ctx.system_health().healthy);
    assert!((ctx.get_metrics().jitter_p99_ms - 0.3).abs() < 1e-6);
}

// ===========================================================================
// Server lifecycle depth tests
// ===========================================================================

#[tokio::test]
async fn server_start_binds_ephemeral_port() {
    let server = IpcServer::new_mock(ServerConfig::default());
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let handle = server.start(addr).await.unwrap();
    assert_ne!(handle.addr().port(), 0, "should bind to a real port");
    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn server_graceful_shutdown() {
    let (handle, url) = start_mock_server().await;
    let mut client = connect_client(&url).await;
    assert!(client.get_service_info().await.is_ok());

    // Shutdown is graceful
    handle.shutdown().await.unwrap();

    // Client calls now fail
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(client.get_service_info().await.is_err());
}

#[tokio::test]
async fn server_multiple_sequential_start_stop() {
    for _ in 0..3 {
        let (handle, url) = start_mock_server().await;
        let mut client = connect_client(&url).await;
        assert!(client.get_service_info().await.is_ok());
        handle.shutdown().await.unwrap();
    }
}
