// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the IPC crate covering connection lifecycle, subscription
//! management, serialization round-trips, backpressure, concurrency, rate
//! limiting, connection pool behaviour, and property-based validation.

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use flight_ipc::client::IpcClient;
use flight_ipc::connection_pool::{ConnectionPool, PoolError};
use flight_ipc::handlers::{FlightServiceHandler, MockServiceContext, ServiceContext};
use flight_ipc::messages::{ComponentStatus, IpcMessage, ServiceState};
use flight_ipc::negotiation::{self, Version};
use flight_ipc::proto::flight_service_client::FlightServiceClient as GrpcClient;
use flight_ipc::proto::flight_service_server::{FlightService as GrpcFlightService, FlightServiceServer as GrpcFlightServiceServer};
use flight_ipc::proto::{self, *};
use flight_ipc::rate_limiter::{RateLimitConfig, RateLimitResult, RateLimiter};
use flight_ipc::server::IpcServer;
use flight_ipc::subscriptions::{
    BroadcastMessage, SubscriptionFilter, SubscriptionManager, Topic,
};
use flight_ipc::transport::{RetryPolicy, TransportConfig};
use flight_ipc::{ClientConfig, IpcError, ServerConfig, PROTOCOL_VERSION, SUPPORTED_FEATURES};

use prost::Message;
use proptest::prelude::*;
use tonic::Request;

// ===========================================================================
// Test helpers
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

async fn raw_client(url: &str) -> GrpcClient<tonic::transport::Channel> {
    GrpcClient::connect(url.to_string())
        .await
        .expect("client should connect")
}

/// Prost encode → decode round-trip helper.
fn round_trip<M: Message + Default + PartialEq + std::fmt::Debug>(msg: &M) -> M {
    let mut buf = Vec::new();
    msg.encode(&mut buf).expect("encode must not fail");
    M::decode(buf.as_slice()).expect("decode must not fail")
}

// ===========================================================================
// Module 1: Proto message types and serialization
// ===========================================================================

mod proto_messages {
    use super::*;

    #[test]
    fn all_types_constructable() {
        let _ = NegotiateFeaturesRequest::default();
        let _ = ListDevicesRequest::default();
        let _ = Device::default();
        let _ = HealthEvent::default();
        let _ = CurveConflict::default();
        let _ = OneClickResult::default();
    }

    #[test]
    fn health_event_round_trip() {
        let msg = HealthEvent {
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
        assert_eq!(round_trip(&msg), msg);
    }

    #[test]
    fn device_round_trip() {
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
        assert_eq!(round_trip(&device), device);
    }

    #[test]
    fn curve_conflict_round_trip() {
        let msg = CurveConflict {
            axis_name: "pitch".into(),
            conflict_type: ConflictType::DoubleCurve.into(),
            severity: ConflictSeverity::High.into(),
            description: "double curve on pitch".into(),
            suggested_resolutions: vec![ConflictResolution {
                resolution_type: ResolutionType::DisableSimCurve.into(),
                description: "disable sim curve".into(),
                action: Some(ResolutionAction {
                    r#type: ResolutionType::DisableSimCurve.into(),
                    parameters: [("axis".into(), "pitch".into())].into(),
                    affected_files: vec!["config.json".into()],
                    backup_info: "backup-001".into(),
                }),
                estimated_improvement: 0.85,
                requires_sim_restart: false,
            }],
            metadata: Some(ConflictMetadata {
                sim_curve_strength: 0.7,
                profile_curve_strength: 0.5,
                combined_nonlinearity: 0.9,
                test_inputs: vec![0.0, 0.25, 0.5, 0.75, 1.0],
                expected_outputs: vec![0.0, 0.25, 0.5, 0.75, 1.0],
                actual_outputs: vec![0.0, 0.1, 0.3, 0.6, 1.0],
                detection_timestamp: 1700000000,
            }),
        };
        assert_eq!(round_trip(&msg), msg);
    }

    #[test]
    fn enum_variants_valid() {
        assert!(TransportType::try_from(TransportType::NamedPipes as i32).is_ok());
        assert!(ServiceStatus::try_from(ServiceStatus::Running as i32).is_ok());
    }
}

// ===========================================================================
// Module 2: Connection management
// ===========================================================================

mod connection_management {
    use super::*;

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

        // Disconnect
        client.disconnect().await;
        assert!(!client.is_connected().await);

        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn reconnection_after_disconnect() {
        let (handle, url) = start_mock_server().await;
        let mut client = connect_client(&url).await;
        client.disconnect().await;
        assert!(!client.is_connected().await);

        assert!(client.reconnect().await.is_ok());
        assert!(client.is_connected().await);
        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn server_shutdown_clients_notified() {
        let (handle, url) = start_mock_server().await;
        let mut client = connect_client(&url).await;
        handle.shutdown().await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(client.get_service_info().await.is_err());
    }

    #[tokio::test]
    async fn multiple_simultaneous_clients() {
        let (handle, url) = start_mock_server().await;
        let mut tasks = Vec::new();
        for i in 0..10 {
            let u = url.clone();
            tasks.push(tokio::spawn(async move {
                let mut client = connect_client(&u).await;
                assert!(client.get_service_info().await.is_ok());
                i
            }));
        }
        for task in tasks {
            task.await.unwrap();
        }
        handle.shutdown().await.unwrap();
    }
}

// ===========================================================================
// Module 3: Subscription system
// ===========================================================================

mod subscription_system {
    use super::*;

    #[test]
    fn subscribe_receive_broadcast() {
        let mut mgr = SubscriptionManager::new();
        let h = mgr.subscribe(Topic::AxisData, SubscriptionFilter::default());
        let msg = BroadcastMessage {
            topic: Topic::AxisData,
            payload: "v1".to_string(),
            device_id: None,
            axis_id: None,
        };
        assert_eq!(mgr.broadcast(&msg), vec![h.id]);
    }

    #[test]
    fn unsubscribe_stops_delivery() {
        let mut mgr = SubscriptionManager::new();
        let h = mgr.subscribe(Topic::DeviceEvents, SubscriptionFilter::default());
        mgr.unsubscribe(&h);
        let msg = BroadcastMessage {
            topic: Topic::DeviceEvents,
            payload: "evt".to_string(),
            device_id: None,
            axis_id: None,
        };
        assert!(mgr.broadcast(&msg).is_empty());
    }

    #[test]
    fn filter_device_and_axis() {
        let mut mgr = SubscriptionManager::new();
        let filter = SubscriptionFilter {
            device_id: Some("js-1".into()),
            axis_id: Some("pitch".into()),
            ..Default::default()
        };
        let h = mgr.subscribe(Topic::AxisData, filter);

        let both_match = BroadcastMessage {
            topic: Topic::AxisData,
            payload: "v".into(),
            device_id: Some("js-1".into()),
            axis_id: Some("pitch".into()),
        };
        assert_eq!(mgr.broadcast(&both_match), vec![h.id]);

        let device_only = BroadcastMessage {
            topic: Topic::AxisData,
            payload: "v".into(),
            device_id: Some("js-1".into()),
            axis_id: Some("roll".into()),
        };
        assert!(mgr.broadcast(&device_only).is_empty());
    }

    #[test]
    fn changed_only_suppresses_duplicates() {
        let mut mgr = SubscriptionManager::new();
        let filter = SubscriptionFilter {
            changed_only: true,
            ..Default::default()
        };
        let h = mgr.subscribe(Topic::HealthStatus, filter);
        let msg = BroadcastMessage {
            topic: Topic::HealthStatus,
            payload: "ok".into(),
            device_id: None,
            axis_id: None,
        };
        assert_eq!(mgr.broadcast(&msg), vec![h.id]);
        assert!(mgr.broadcast(&msg).is_empty());
    }

    #[test]
    fn throttle_drops_excess() {
        let mut mgr = SubscriptionManager::new();
        let filter = SubscriptionFilter {
            min_interval_ms: Some(1000),
            ..Default::default()
        };
        let h = mgr.subscribe(Topic::AxisData, filter);
        let msg = BroadcastMessage {
            topic: Topic::AxisData,
            payload: "v1".into(),
            device_id: None,
            axis_id: None,
        };
        assert_eq!(mgr.broadcast(&msg), vec![h.id]);
        assert!(mgr.broadcast(&msg).is_empty());
    }
}

// ===========================================================================
// Module 4: Service methods and Streaming
// ===========================================================================

mod service_methods {
    use super::*;

    #[tokio::test]
    async fn all_rpc_endpoints_callable() {
        let (handle, url) = start_mock_server().await;
        let mut client = connect_client(&url).await;

        assert!(client.get_service_info().await.is_ok());
        assert!(client.list_devices().await.is_ok());
        assert!(client.negotiate_features().await.is_ok());
        assert!(client.get_security_status().await.is_ok());
        assert!(client.get_support_bundle().await.is_ok());

        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn health_subscribe_stream_receives_events() {
        let config = ServerConfig::default();
        let ctx = Arc::new(MockServiceContext::new());
        let handler = FlightServiceHandler::new(ctx, config);
        let health_tx = handler.health_sender();
        let svc = GrpcFlightServiceServer::new(handler);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);

        tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(svc)
                .serve_with_incoming_shutdown(incoming, async move {
                    let _ = shutdown_rx.changed().await;
                })
                .await
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        let url = format!("http://127.0.0.1:{}", addr.port());
        let mut client = IpcClient::connect(&url).await.unwrap();

        let mut rx = client
            .subscribe_health(HealthSubscribeRequest::default())
            .await
            .unwrap();

        let event = HealthEvent {
            timestamp: 1234,
            message: "test".into(),
            ..Default::default()
        };
        health_tx.send(event).unwrap();

        let received = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await.unwrap().unwrap();
        assert_eq!(received.message, "test");

        let _ = shutdown_tx.send(true);
    }
}

// ===========================================================================
// Module 5: Connection pool and Rate limiting
// ===========================================================================

mod resources {
    use super::*;

    #[test]
    fn pool_reuse_after_disconnect() {
        let mut pool = ConnectionPool::new(2, 300);
        pool.connect("c1", "client-a", 100).unwrap();
        pool.connect("c2", "client-b", 100).unwrap();
        assert!(pool.is_full());
        pool.disconnect("c1");
        assert!(!pool.is_full());
        pool.connect("c3", "client-c", 200).unwrap();
        assert_eq!(pool.active_count(), 2);
    }

    #[test]
    fn rate_limiter_buckets_per_client() {
        let config = RateLimitConfig {
            max_tokens: 3,
            refill_rate: 0.0,
            refill_interval: Duration::from_secs(1),
        };
        let mut rl = RateLimiter::new(config);
        for _ in 0..3 {
            assert_eq!(rl.check("client-a"), RateLimitResult::Allowed);
        }
        assert!(matches!(rl.check("client-a"), RateLimitResult::Limited { .. }));
        assert_eq!(rl.check("client-b"), RateLimitResult::Allowed);
    }
}

// ===========================================================================
// Module 6: Error handling
// ===========================================================================

mod error_handling {
    use super::*;

    #[tokio::test]
    async fn connect_refused() {
        let result = IpcClient::connect("http://127.0.0.1:19997").await;
        assert!(result.is_err());
    }

    #[test]
    fn ipc_error_variants() {
        let err = IpcError::VersionMismatch { client: "2".into(), server: "1".into() };
        assert!(err.to_string().contains("2"));
    }

    #[test]
    fn negotiation_version_mismatch() {
        let request = NegotiateFeaturesRequest {
            client_version: "999.0.0".to_string(),
            ..Default::default()
        };
        let server_features = vec![];
        let response = negotiation::negotiate_features(&request, &server_features).unwrap();
        assert!(!response.success);
    }
}

// ===========================================================================
// Module 7: Property tests
// ===========================================================================

mod property_tests {
    use super::*;

    proptest! {
        #[test]
        fn prop_health_event_round_trip(
            ts in any::<i64>(),
            msg in "\\PC{0,64}",
        ) {
            let event = HealthEvent { timestamp: ts, message: msg.clone(), ..Default::default() };
            let decoded = round_trip(&event);
            prop_assert_eq!(decoded.timestamp, ts);
            prop_assert_eq!(decoded.message, msg);
        }

        #[test]
        fn prop_device_round_trip(
            id in "[a-z0-9]{1,16}",
            device_type in 0i32..7,
        ) {
            let msg = Device { id: id.clone(), r#type: device_type, ..Default::default() };
            let decoded = round_trip(&msg);
            prop_assert_eq!(decoded.id, id);
            prop_assert_eq!(decoded.r#type, device_type);
        }

        #[test]
        fn prop_performance_metrics_round_trip(
            jitter in any::<f32>(),
            mem in any::<u64>(),
        ) {
            let msg = PerformanceMetrics { jitter_p99_ms: jitter, memory_usage_bytes: mem, ..Default::default() };
            let decoded = round_trip(&msg);
            if !jitter.is_nan() {
                prop_assert_eq!(decoded.jitter_p99_ms, jitter);
            }
            prop_assert_eq!(decoded.memory_usage_bytes, mem);
        }

        #[test]
        fn prop_ipc_message_json_round_trip_telemetry(
            alt in -1000i32..60000,
            speed in 0i32..600,
        ) {
            let msg = IpcMessage::TelemetryUpdate {
                altitude: f64::from(alt),
                airspeed: f64::from(speed),
                heading: 0.0,
            };
            let json = msg.to_json();
            let restored = IpcMessage::from_json(&json).unwrap();
            prop_assert_eq!(msg, restored);
        }
    }
}

// ===========================================================================
// Module 8: Additional IpcMessage (JSON) tests
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
        IpcMessage::ServiceStatus {
            status: ServiceState::Running,
            uptime_secs: 86400,
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
        assert_eq!(msg, &restored);
    }
}
