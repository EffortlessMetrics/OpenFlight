// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the IPC layer — proto messages, connection management,
//! subscriptions, service methods, streaming, and property-based checks.

use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use flight_ipc::client::IpcClient;
use flight_ipc::handlers::{FlightServiceHandler, MockServiceContext};
use flight_ipc::proto::flight_service_client::FlightServiceClient as GrpcClient;
use flight_ipc::proto::flight_service_server::FlightServiceServer as GrpcFlightServiceServer;
use flight_ipc::proto::{self, *};
use flight_ipc::server::IpcServer;
use flight_ipc::subscriptions::{
    BroadcastMessage, SubscriptionFilter, SubscriptionManager, Topic,
};
use flight_ipc::transport::{RetryPolicy, TransportConfig};
use flight_ipc::{ClientConfig, ServerConfig, PROTOCOL_VERSION};
use prost::Message;
use proptest::prelude::*;

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
    tokio::time::sleep(Duration::from_millis(50)).await;
    (handle, url)
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
// 1. Proto message types
// ===========================================================================

mod proto_messages {
    use super::*;

    // --- All message types constructable ---

    #[test]
    fn all_request_types_constructable() {
        let _ = NegotiateFeaturesRequest::default();
        let _ = ListDevicesRequest::default();
        let _ = HealthSubscribeRequest::default();
        let _ = ApplyProfileRequest::default();
        let _ = DetectCurveConflictsRequest::default();
        let _ = ResolveCurveConflictRequest::default();
        let _ = OneClickResolveRequest::default();
        let _ = SetCapabilityModeRequest::default();
        let _ = GetCapabilityModeRequest::default();
        let _ = GetServiceInfoRequest::default();
        let _ = GetSecurityStatusRequest::default();
        let _ = ConfigureTelemetryRequest::default();
        let _ = GetSupportBundleRequest::default();
    }

    #[test]
    fn all_response_types_constructable() {
        let _ = NegotiateFeaturesResponse::default();
        let _ = ListDevicesResponse::default();
        let _ = ApplyProfileResponse::default();
        let _ = DetectCurveConflictsResponse::default();
        let _ = ResolveCurveConflictResponse::default();
        let _ = OneClickResolveResponse::default();
        let _ = SetCapabilityModeResponse::default();
        let _ = GetCapabilityModeResponse::default();
        let _ = GetServiceInfoResponse::default();
        let _ = GetSecurityStatusResponse::default();
        let _ = ConfigureTelemetryResponse::default();
        let _ = GetSupportBundleResponse::default();
    }

    #[test]
    fn nested_types_constructable() {
        let _ = Device::default();
        let _ = DeviceCapabilities::default();
        let _ = DeviceHealth::default();
        let _ = HealthEvent::default();
        let _ = PerformanceMetrics::default();
        let _ = ValidationError::default();
        let _ = CurveConflict::default();
        let _ = ConflictMetadata::default();
        let _ = ConflictResolution::default();
        let _ = ResolutionAction::default();
        let _ = ResolutionResult::default();
        let _ = OneClickResult::default();
        let _ = BackupInfo::default();
        let _ = VerificationOutcome::default();
        let _ = ResolutionMetrics::default();
        let _ = ConflictMetrics::default();
        let _ = ResolutionStep::default();
        let _ = AxisCapabilityStatus::default();
        let _ = CapabilityLimits::default();
        let _ = PluginInfo::default();
    }

    // --- Serialization round-trip ---

    #[test]
    fn negotiate_features_request_round_trip() {
        let msg = NegotiateFeaturesRequest {
            client_version: "1.0.0".into(),
            supported_features: vec!["device-management".into(), "health-monitoring".into()],
            preferred_transport: TransportType::NamedPipes.into(),
        };
        assert_eq!(round_trip(&msg), msg);
    }

    #[test]
    fn negotiate_features_response_round_trip() {
        let msg = NegotiateFeaturesResponse {
            success: true,
            server_version: "1.0.0".into(),
            enabled_features: vec!["device-management".into()],
            negotiated_transport: TransportType::NamedPipes.into(),
            error_message: String::new(),
        };
        assert_eq!(round_trip(&msg), msg);
    }

    #[test]
    fn list_devices_request_round_trip() {
        let msg = ListDevicesRequest {
            include_disconnected: true,
            filter_types: vec![DeviceType::Joystick.into(), DeviceType::Throttle.into()],
        };
        assert_eq!(round_trip(&msg), msg);
    }

    #[test]
    fn list_devices_response_with_devices_round_trip() {
        let msg = ListDevicesResponse {
            devices: vec![
                Device {
                    id: "dev-1".into(),
                    name: "Joystick".into(),
                    r#type: DeviceType::Joystick.into(),
                    status: DeviceStatus::Connected.into(),
                    capabilities: Some(DeviceCapabilities {
                        supports_force_feedback: true,
                        supports_raw_torque: false,
                        max_torque_nm: 20,
                        min_period_us: 1000,
                        has_health_stream: true,
                        supported_protocols: vec!["hid".into()],
                    }),
                    health: Some(DeviceHealth {
                        temperature_celsius: 42.5,
                        current_amperes: 1.2,
                        packet_loss_count: 0,
                        last_seen_timestamp: 1234567890,
                        active_faults: vec![],
                    }),
                    metadata: [("vendor_id".into(), "06A3".into())].into(),
                },
                Device {
                    id: "dev-2".into(),
                    name: "Throttle".into(),
                    r#type: DeviceType::Throttle.into(),
                    status: DeviceStatus::Disconnected.into(),
                    capabilities: None,
                    health: None,
                    metadata: Default::default(),
                },
            ],
            total_count: 2,
        };
        assert_eq!(round_trip(&msg), msg);
    }

    #[test]
    fn apply_profile_request_round_trip() {
        let msg = ApplyProfileRequest {
            profile_json: r#"{"version":"1","axes":{}}"#.into(),
            validate_only: true,
            force_apply: false,
        };
        assert_eq!(round_trip(&msg), msg);
    }

    #[test]
    fn apply_profile_response_with_validation_errors_round_trip() {
        let msg = ApplyProfileResponse {
            success: false,
            error_message: "validation failed".into(),
            validation_errors: vec![ValidationError {
                field_path: "axes.pitch".into(),
                line_number: 5,
                column_number: 10,
                error_message: "out of range".into(),
                error_type: ValidationErrorType::Range.into(),
            }],
            effective_profile_hash: String::new(),
            compile_time_ms: 0,
        };
        assert_eq!(round_trip(&msg), msg);
    }

    #[test]
    fn health_event_round_trip() {
        let msg = HealthEvent {
            timestamp: 1700000000,
            r#type: HealthEventType::Warning.into(),
            message: "jitter spike".into(),
            device_id: "dev-1".into(),
            error_code: "JITTER_HIGH".into(),
            metadata: [("p99".into(), "0.8".into())].into(),
            performance: Some(PerformanceMetrics {
                jitter_p99_ms: 0.8,
                hid_latency_p99_us: 250.0,
                missed_ticks: 2,
                dropped_frames: 0,
                cpu_usage_percent: 15.3,
                memory_usage_bytes: 1024 * 1024,
            }),
        };
        assert_eq!(round_trip(&msg), msg);
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
    fn one_click_result_round_trip() {
        let msg = OneClickResult {
            resolution_type: ResolutionType::ApplyGainCompensation.into(),
            modified_files: vec!["profile.json".into()],
            backup_info: Some(BackupInfo {
                timestamp: 1700000000,
                description: "auto backup".into(),
                affected_files: vec!["profile.json".into()],
                backup_dir: "/tmp/backup".into(),
                writer_config: "{}".into(),
            }),
            verification: Some(VerificationOutcome {
                passed: true,
                details: "conflict resolved".into(),
                duration_ms: 50,
                conflict_resolved: true,
            }),
            metrics: Some(ResolutionMetrics {
                before: Some(ConflictMetrics {
                    nonlinearity: 0.9,
                    sim_curve_strength: 0.7,
                    profile_curve_strength: 0.5,
                    timestamp: 1700000000,
                }),
                after: Some(ConflictMetrics {
                    nonlinearity: 0.1,
                    sim_curve_strength: 0.0,
                    profile_curve_strength: 0.5,
                    timestamp: 1700000001,
                }),
                improvement: 0.89,
            }),
            steps_performed: vec![ResolutionStep {
                name: "disable_sim_curve".into(),
                description: "Disabled simulator curve".into(),
                success: true,
                duration_ms: 20,
                error: String::new(),
            }],
        };
        assert_eq!(round_trip(&msg), msg);
    }

    #[test]
    fn capability_limits_round_trip() {
        let msg = CapabilityLimits {
            max_axis_output: 0.5,
            max_ffb_torque: 10.0,
            max_slew_rate: 100.0,
            max_curve_expo: 0.8,
            allow_high_torque: false,
            allow_custom_curves: true,
        };
        assert_eq!(round_trip(&msg), msg);
    }

    #[test]
    fn get_service_info_response_with_capabilities_round_trip() {
        let msg = GetServiceInfoResponse {
            version: "1.0.0".into(),
            uptime_seconds: 3600,
            status: ServiceStatus::Running.into(),
            capabilities: [
                ("device-management".into(), "enabled".into()),
                ("health-monitoring".into(), "enabled".into()),
            ]
            .into(),
        };
        assert_eq!(round_trip(&msg), msg);
    }

    #[test]
    fn security_status_response_with_plugins_round_trip() {
        let msg = GetSecurityStatusResponse {
            success: true,
            error_message: String::new(),
            plugins: vec![
                PluginInfo {
                    name: "wasm-plugin".into(),
                    version: "1.0.0".into(),
                    plugin_type: PluginType::Wasm.into(),
                    signature_status: "verified".into(),
                    capabilities: vec!["read-axes".into()],
                },
                PluginInfo {
                    name: "native-plugin".into(),
                    version: "2.0.0".into(),
                    plugin_type: PluginType::Native.into(),
                    signature_status: "unsigned".into(),
                    capabilities: vec!["write-axes".into(), "read-health".into()],
                },
            ],
            telemetry_enabled: true,
            telemetry_data_types: vec!["Performance".into(), "Errors".into()],
        };
        assert_eq!(round_trip(&msg), msg);
    }

    // --- Optional fields present/absent ---

    #[test]
    fn device_with_no_capabilities_or_health() {
        let msg = Device {
            id: "dev-1".into(),
            name: "Joystick".into(),
            r#type: DeviceType::Joystick.into(),
            status: DeviceStatus::Connected.into(),
            capabilities: None,
            health: None,
            metadata: Default::default(),
        };
        let decoded = round_trip(&msg);
        assert!(decoded.capabilities.is_none());
        assert!(decoded.health.is_none());
    }

    #[test]
    fn device_with_capabilities_present() {
        let msg = Device {
            capabilities: Some(DeviceCapabilities {
                supports_force_feedback: true,
                ..Default::default()
            }),
            ..Default::default()
        };
        let decoded = round_trip(&msg);
        assert!(decoded.capabilities.is_some());
        assert!(decoded.capabilities.unwrap().supports_force_feedback);
    }

    #[test]
    fn health_event_with_no_performance() {
        let msg = HealthEvent {
            timestamp: 1,
            r#type: HealthEventType::Info.into(),
            message: "ok".into(),
            performance: None,
            ..Default::default()
        };
        let decoded = round_trip(&msg);
        assert!(decoded.performance.is_none());
    }

    #[test]
    fn resolution_result_with_no_metrics() {
        let msg = ResolutionResult {
            applied_resolution: ResolutionType::DisableProfileCurve.into(),
            before_metrics: None,
            after_metrics: None,
            ..Default::default()
        };
        let decoded = round_trip(&msg);
        assert!(decoded.before_metrics.is_none());
        assert!(decoded.after_metrics.is_none());
    }

    // --- Enum variants ---

    #[test]
    fn transport_type_all_variants_valid() {
        for val in [
            TransportType::Unspecified,
            TransportType::NamedPipes,
            TransportType::UnixSockets,
        ] {
            let i = val as i32;
            assert!(TransportType::try_from(i).is_ok());
        }
    }

    #[test]
    fn service_status_all_variants_valid() {
        for val in [
            ServiceStatus::Unspecified,
            ServiceStatus::Starting,
            ServiceStatus::Running,
            ServiceStatus::Degraded,
            ServiceStatus::Stopping,
        ] {
            let i = val as i32;
            assert!(ServiceStatus::try_from(i).is_ok());
        }
    }

    #[test]
    fn device_type_all_variants_valid() {
        for val in [
            DeviceType::Unspecified,
            DeviceType::Joystick,
            DeviceType::Throttle,
            DeviceType::Rudder,
            DeviceType::Panel,
            DeviceType::ForceFeedback,
            DeviceType::Streamdeck,
        ] {
            let i = val as i32;
            assert!(DeviceType::try_from(i).is_ok());
        }
    }

    #[test]
    fn device_status_all_variants_valid() {
        for val in [
            DeviceStatus::Unspecified,
            DeviceStatus::Connected,
            DeviceStatus::Disconnected,
            DeviceStatus::Error,
            DeviceStatus::Faulted,
        ] {
            let i = val as i32;
            assert!(DeviceStatus::try_from(i).is_ok());
        }
    }

    #[test]
    fn health_event_type_all_variants_valid() {
        for val in [
            HealthEventType::Unspecified,
            HealthEventType::Info,
            HealthEventType::Warning,
            HealthEventType::Error,
            HealthEventType::Fault,
            HealthEventType::Performance,
            HealthEventType::CurveConflict,
            HealthEventType::CurveResolution,
        ] {
            let i = val as i32;
            assert!(HealthEventType::try_from(i).is_ok());
        }
    }

    #[test]
    fn validation_error_type_all_variants_valid() {
        for val in [
            ValidationErrorType::Unspecified,
            ValidationErrorType::Schema,
            ValidationErrorType::Monotonic,
            ValidationErrorType::Range,
            ValidationErrorType::Conflict,
        ] {
            let i = val as i32;
            assert!(ValidationErrorType::try_from(i).is_ok());
        }
    }

    #[test]
    fn conflict_type_all_variants_valid() {
        for val in [
            ConflictType::Unspecified,
            ConflictType::DoubleCurve,
            ConflictType::ExcessiveNonlinearity,
            ConflictType::OpposingCurves,
        ] {
            let i = val as i32;
            assert!(ConflictType::try_from(i).is_ok());
        }
    }

    #[test]
    fn conflict_severity_all_variants_valid() {
        for val in [
            ConflictSeverity::Unspecified,
            ConflictSeverity::Low,
            ConflictSeverity::Medium,
            ConflictSeverity::High,
            ConflictSeverity::Critical,
        ] {
            let i = val as i32;
            assert!(ConflictSeverity::try_from(i).is_ok());
        }
    }

    #[test]
    fn resolution_type_all_variants_valid() {
        for val in [
            ResolutionType::Unspecified,
            ResolutionType::DisableSimCurve,
            ResolutionType::DisableProfileCurve,
            ResolutionType::ApplyGainCompensation,
            ResolutionType::ReduceCurveStrength,
        ] {
            let i = val as i32;
            assert!(ResolutionType::try_from(i).is_ok());
        }
    }

    #[test]
    fn capability_mode_all_variants_valid() {
        for val in [
            CapabilityMode::Unspecified,
            CapabilityMode::Full,
            CapabilityMode::Demo,
            CapabilityMode::Kid,
        ] {
            let i = val as i32;
            assert!(CapabilityMode::try_from(i).is_ok());
        }
    }

    #[test]
    fn plugin_type_all_variants_valid() {
        for val in [
            PluginType::Unspecified,
            PluginType::Wasm,
            PluginType::Native,
        ] {
            let i = val as i32;
            assert!(PluginType::try_from(i).is_ok());
        }
    }

    // --- Default values are sensible ---

    #[test]
    fn default_list_devices_request_includes_only_connected() {
        let msg = ListDevicesRequest::default();
        assert!(!msg.include_disconnected);
        assert!(msg.filter_types.is_empty());
    }

    #[test]
    fn default_apply_profile_request_has_safe_defaults() {
        let msg = ApplyProfileRequest::default();
        assert!(msg.profile_json.is_empty());
        assert!(!msg.validate_only);
        assert!(!msg.force_apply);
    }

    #[test]
    fn default_service_info_response_has_zero_uptime() {
        let msg = GetServiceInfoResponse::default();
        assert_eq!(msg.uptime_seconds, 0);
        assert!(msg.version.is_empty());
        assert_eq!(msg.status(), ServiceStatus::Unspecified);
    }

    #[test]
    fn empty_message_encodes_to_zero_bytes() {
        let msg = GetServiceInfoRequest {};
        let mut buf = Vec::new();
        msg.encode(&mut buf).unwrap();
        assert!(buf.is_empty());
    }

    #[test]
    fn default_health_subscribe_request_subscribes_to_all() {
        let msg = HealthSubscribeRequest::default();
        assert!(msg.filter_types.is_empty());
        assert!(msg.device_ids.is_empty());
        assert!(!msg.include_performance_metrics);
    }
}

// ===========================================================================
// 2. Connection management
// ===========================================================================

mod connection_management {
    use super::*;

    #[tokio::test]
    async fn connect_healthy_disconnect_lifecycle() {
        let (handle, url) = start_mock_server().await;
        let mut client = IpcClient::connect(&url).await.unwrap();

        // Healthy
        assert!(client.is_connected().await);
        let info = client.get_service_info().await.unwrap();
        assert_eq!(info.version, PROTOCOL_VERSION);

        // Disconnect
        client.disconnect().await;
        assert!(!client.is_connected().await);

        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn reconnection_after_disconnect() {
        let (handle, url) = start_mock_server().await;
        let mut client = IpcClient::connect(&url).await.unwrap();

        client.disconnect().await;
        assert!(!client.is_connected().await);

        client.reconnect().await.unwrap();
        assert!(client.is_connected().await);
        assert!(client.get_service_info().await.is_ok());

        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn connection_timeout_on_unreachable() {
        let tc = TransportConfig {
            connect_timeout: Duration::from_millis(100),
            retry_policy: RetryPolicy {
                max_retries: 0,
                base_delay: Duration::from_millis(1),
                max_delay: Duration::from_millis(5),
            },
            health_check_interval: Duration::ZERO,
            ..TransportConfig::default()
        };
        let result = IpcClient::connect_with_transport(
            "http://127.0.0.1:19997",
            ClientConfig::default(),
            tc,
        )
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn multiple_simultaneous_clients() {
        let (handle, url) = start_mock_server().await;

        let mut tasks = Vec::new();
        for i in 0..10 {
            let u = url.clone();
            tasks.push(tokio::spawn(async move {
                let mut client = IpcClient::connect(&u).await.unwrap();
                let info = client.get_service_info().await.unwrap();
                assert_eq!(info.version, PROTOCOL_VERSION);
                i // return to confirm task ran
            }));
        }

        let mut results = Vec::new();
        for task in tasks {
            results.push(task.await.unwrap());
        }
        assert_eq!(results.len(), 10);

        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn server_shutdown_clients_notified() {
        let (handle, url) = start_mock_server().await;
        let mut client = IpcClient::connect(&url).await.unwrap();

        // Works before shutdown
        assert!(client.get_service_info().await.is_ok());

        handle.shutdown().await.unwrap();

        // After server shutdown, client calls should fail
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(client.get_service_info().await.is_err());
    }

    #[tokio::test]
    async fn connect_with_custom_client_config() {
        let (handle, url) = start_mock_server().await;
        let config = ClientConfig {
            connection_timeout_ms: 3000,
            supported_features: vec!["device-management".into()],
            ..ClientConfig::default()
        };
        let mut client = IpcClient::connect_with_config(&url, config).await.unwrap();
        assert!(client.is_connected().await);
        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn connect_invalid_address() {
        let result = IpcClient::connect("not-a-valid-url").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn connect_refused_no_server() {
        let result = IpcClient::connect("http://127.0.0.1:19996").await;
        assert!(result.is_err());
    }
}

// ===========================================================================
// 3. Subscription system
// ===========================================================================

mod subscription_system {
    use super::*;

    fn no_filter() -> SubscriptionFilter {
        SubscriptionFilter::default()
    }

    fn msg(topic: Topic, payload: &str) -> BroadcastMessage {
        BroadcastMessage {
            topic,
            payload: payload.to_owned(),
            device_id: None,
            axis_id: None,
        }
    }

    fn msg_device(topic: Topic, payload: &str, device: &str) -> BroadcastMessage {
        BroadcastMessage {
            topic,
            payload: payload.to_owned(),
            device_id: Some(device.to_owned()),
            axis_id: None,
        }
    }

    #[test]
    fn subscribe_to_topic_receive_updates() {
        let mut mgr = SubscriptionManager::new();
        let h = mgr.subscribe(Topic::AxisData, no_filter());
        let ids = mgr.broadcast(&msg(Topic::AxisData, "value=0.5"));
        assert_eq!(ids, vec![h.id]);
    }

    #[test]
    fn unsubscribe_stops_receiving() {
        let mut mgr = SubscriptionManager::new();
        let h = mgr.subscribe(Topic::DeviceEvents, no_filter());
        mgr.unsubscribe(&h);
        let ids = mgr.broadcast(&msg(Topic::DeviceEvents, "disconnect"));
        assert!(ids.is_empty());
    }

    #[test]
    fn multiple_subscribers_same_topic() {
        let mut mgr = SubscriptionManager::new();
        let _h1 = mgr.subscribe(Topic::HealthStatus, no_filter());
        let _h2 = mgr.subscribe(Topic::HealthStatus, no_filter());
        let _h3 = mgr.subscribe(Topic::HealthStatus, no_filter());

        let ids = mgr.broadcast(&msg(Topic::HealthStatus, "ok"));
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn subscriber_backpressure_rate_throttle() {
        let mut mgr = SubscriptionManager::new();
        let filter = SubscriptionFilter {
            min_interval_ms: Some(5000),
            ..Default::default()
        };
        let h = mgr.subscribe(Topic::AxisData, filter);

        // First message goes through
        let ids = mgr.broadcast(&msg(Topic::AxisData, "v1"));
        assert_eq!(ids, vec![h.id]);

        // Immediate second message is throttled
        let ids = mgr.broadcast(&msg(Topic::AxisData, "v2"));
        assert!(ids.is_empty());
    }

    #[test]
    fn topic_filtering_only_matching_topic() {
        let mut mgr = SubscriptionManager::new();
        let h_axis = mgr.subscribe(Topic::AxisData, no_filter());
        let _h_dev = mgr.subscribe(Topic::DeviceEvents, no_filter());

        let ids = mgr.broadcast(&msg(Topic::AxisData, "data"));
        assert_eq!(ids, vec![h_axis.id]);
    }

    #[test]
    fn device_filter_matches_correct_device() {
        let mut mgr = SubscriptionManager::new();
        let filter = SubscriptionFilter {
            device_id: Some("stick-1".into()),
            ..Default::default()
        };
        let h = mgr.subscribe(Topic::DeviceEvents, filter);

        let ids = mgr.broadcast(&msg_device(Topic::DeviceEvents, "evt", "stick-1"));
        assert_eq!(ids, vec![h.id]);

        let ids = mgr.broadcast(&msg_device(Topic::DeviceEvents, "evt", "stick-2"));
        assert!(ids.is_empty());
    }

    #[test]
    fn changed_only_suppresses_duplicates() {
        let mut mgr = SubscriptionManager::new();
        let filter = SubscriptionFilter {
            changed_only: true,
            ..Default::default()
        };
        let h = mgr.subscribe(Topic::HealthStatus, filter);

        let ids = mgr.broadcast(&msg(Topic::HealthStatus, "state-A"));
        assert_eq!(ids, vec![h.id]);

        // Same payload — suppressed
        let ids = mgr.broadcast(&msg(Topic::HealthStatus, "state-A"));
        assert!(ids.is_empty());

        // Different — delivered
        let ids = mgr.broadcast(&msg(Topic::HealthStatus, "state-B"));
        assert_eq!(ids, vec![h.id]);
    }

    #[test]
    fn subscription_handle_cancel_marks_inactive() {
        let mut mgr = SubscriptionManager::new();
        let h = mgr.subscribe(Topic::FfbStatus, no_filter());
        assert!(h.is_active());

        h.cancel();
        assert!(!h.is_active());
        assert_eq!(mgr.active_count(), 0);
    }

    #[test]
    fn subscription_ids_monotonically_increase() {
        let mut mgr = SubscriptionManager::new();
        let h1 = mgr.subscribe(Topic::AxisData, no_filter());
        let h2 = mgr.subscribe(Topic::AxisData, no_filter());
        let h3 = mgr.subscribe(Topic::AxisData, no_filter());

        assert!(h2.id > h1.id);
        assert!(h3.id > h2.id);
    }

    #[test]
    fn handle_drop_deactivates_subscription() {
        let mut mgr = SubscriptionManager::new();
        let h = mgr.subscribe(Topic::SimTelemetry, no_filter());
        drop(h);
        assert_eq!(mgr.active_count(), 0);
    }

    #[test]
    fn combined_device_and_axis_filter() {
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
    fn all_topics_subscribable() {
        let mut mgr = SubscriptionManager::new();
        let mut handles = Vec::new();
        for &topic in Topic::ALL {
            handles.push(mgr.subscribe(topic, no_filter()));
        }
        assert_eq!(mgr.active_count(), Topic::ALL.len());

        // Broadcast to each topic should reach exactly one subscriber
        for &topic in Topic::ALL {
            let ids = mgr.broadcast(&msg(topic, "test"));
            assert_eq!(ids.len(), 1);
        }
    }
}

// ===========================================================================
// 4. Service methods
// ===========================================================================

mod service_methods {
    use super::*;

    #[tokio::test]
    async fn get_service_info_returns_version_and_status() {
        let (handle, url) = start_mock_server().await;
        let mut client = raw_client(&url).await;

        let resp = client
            .get_service_info(GetServiceInfoRequest {})
            .await
            .unwrap()
            .into_inner();

        assert_eq!(resp.version, PROTOCOL_VERSION);
        assert_eq!(resp.status(), ServiceStatus::Running);

        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn apply_profile_succeeds_on_mock() {
        let (handle, url) = start_mock_server().await;
        let mut client = raw_client(&url).await;

        let resp = client
            .apply_profile(ApplyProfileRequest {
                profile_json: r#"{"version":"1"}"#.into(),
                validate_only: false,
                force_apply: false,
            })
            .await
            .unwrap()
            .into_inner();

        assert!(resp.success);
        assert!(!resp.effective_profile_hash.is_empty());

        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn list_devices_returns_empty_on_mock() {
        let (handle, url) = start_mock_server().await;
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
            id: "test-dev".into(),
            name: "Test Device".into(),
            r#type: DeviceType::Joystick.into(),
            status: DeviceStatus::Connected.into(),
            capabilities: None,
            health: None,
            metadata: Default::default(),
        }];

        let config = ServerConfig::default();
        let ctx = Arc::new(mock);
        let handler = FlightServiceHandler::new(ctx, config);
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
        assert_eq!(resp.devices[0].id, "test-dev");

        let _ = shutdown_tx.send(true);
        let _ = server_task.await;
    }

    #[tokio::test]
    async fn negotiate_features_success() {
        let (handle, url) = start_mock_server().await;
        let mut client = raw_client(&url).await;

        let resp = client
            .negotiate_features(NegotiateFeaturesRequest {
                client_version: "1.0.0".into(),
                supported_features: vec!["device-management".into()],
                preferred_transport: TransportType::NamedPipes.into(),
            })
            .await
            .unwrap()
            .into_inner();

        assert!(resp.success);
        assert!(resp.enabled_features.contains(&"device-management".into()));

        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn negotiate_features_version_mismatch() {
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

        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn detect_curve_conflicts_empty() {
        let (handle, url) = start_mock_server().await;
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

    #[tokio::test]
    async fn set_and_get_capability_mode() {
        let (handle, url) = start_mock_server().await;
        let mut client = raw_client(&url).await;

        let set_resp = client
            .set_capability_mode(SetCapabilityModeRequest {
                mode: CapabilityMode::Demo.into(),
                axis_names: vec!["pitch".into()],
                audit_enabled: true,
            })
            .await
            .unwrap()
            .into_inner();
        assert!(set_resp.success);

        let get_resp = client
            .get_capability_mode(GetCapabilityModeRequest {
                axis_names: vec![],
            })
            .await
            .unwrap()
            .into_inner();
        assert!(get_resp.success);

        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn get_security_status() {
        let (handle, url) = start_mock_server().await;
        let mut client = raw_client(&url).await;

        let resp = client
            .get_security_status(GetSecurityStatusRequest {})
            .await
            .unwrap()
            .into_inner();
        assert!(resp.success);

        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn configure_telemetry() {
        let (handle, url) = start_mock_server().await;
        let mut client = raw_client(&url).await;

        let resp = client
            .configure_telemetry(ConfigureTelemetryRequest {
                enabled: true,
                data_types: vec!["Performance".into(), "Errors".into()],
            })
            .await
            .unwrap()
            .into_inner();
        assert!(resp.success);

        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn get_support_bundle() {
        let (handle, url) = start_mock_server().await;
        let mut client = raw_client(&url).await;

        let resp = client
            .get_support_bundle(GetSupportBundleRequest {})
            .await
            .unwrap()
            .into_inner();
        assert!(resp.success);
        assert!(!resp.redacted_data.is_empty());

        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn resolve_curve_conflict() {
        let (handle, url) = start_mock_server().await;
        let mut client = raw_client(&url).await;

        let resp = client
            .resolve_curve_conflict(ResolveCurveConflictRequest {
                axis_name: "pitch".into(),
                resolution: Some(ResolutionAction {
                    r#type: ResolutionType::DisableSimCurve.into(),
                    ..Default::default()
                }),
                apply_immediately: true,
                create_backup: true,
            })
            .await
            .unwrap()
            .into_inner();
        assert!(resp.success);

        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn one_click_resolve() {
        let (handle, url) = start_mock_server().await;
        let mut client = raw_client(&url).await;

        let resp = client
            .one_click_resolve(OneClickResolveRequest {
                axis_name: "roll".into(),
                create_backup: true,
                verify_resolution: true,
            })
            .await
            .unwrap()
            .into_inner();
        assert!(resp.success);

        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn multiple_sequential_requests_on_same_connection() {
        let (handle, url) = start_mock_server().await;
        let mut client = raw_client(&url).await;

        for _ in 0..20 {
            let resp = client
                .get_service_info(GetServiceInfoRequest {})
                .await
                .unwrap()
                .into_inner();
            assert_eq!(resp.status(), ServiceStatus::Running);
        }

        handle.shutdown().await.unwrap();
    }
}

// ===========================================================================
// 5. Streaming
// ===========================================================================

mod streaming {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn health_subscribe_stream_established() {
        let (handle, url) = start_mock_server().await;
        let mut client = IpcClient::connect(&url).await.unwrap();

        let rx = client
            .subscribe_health(HealthSubscribeRequest::default())
            .await;
        assert!(rx.is_ok(), "subscribe_health should succeed");

        // Clean up — drop the subscription receiver and client first, then
        // shut the server down.  The server-side stream may keep the
        // connection open, so cap the shutdown wait.
        let mut rx = rx.unwrap();
        drop(client);

        // The receiver should eventually close (or time out).
        let _ = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await;

        let _ = tokio::time::timeout(Duration::from_secs(2), handle.shutdown()).await;
    }

    #[tokio::test]
    async fn health_subscribe_receives_published_events() {
        // Set up server with direct handler access for broadcasting
        let config = ServerConfig::default();
        let ctx = Arc::new(MockServiceContext::new());
        let handler = FlightServiceHandler::new(ctx, config.clone());
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

        // Publish an event via the broadcast channel
        let event = HealthEvent {
            timestamp: 1234567890,
            r#type: HealthEventType::Info.into(),
            message: "test event".into(),
            ..Default::default()
        };
        health_tx.send(event.clone()).unwrap();

        // Receive the event
        let received = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("should receive within timeout")
            .expect("channel should not be closed");

        assert_eq!(received.timestamp, 1234567890);
        assert_eq!(received.message, "test event");

        let _ = shutdown_tx.send(true);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn stream_cancellation_by_client_drop() {
        let (handle, url) = start_mock_server().await;
        let mut client = IpcClient::connect(&url).await.unwrap();

        let rx = client
            .subscribe_health(HealthSubscribeRequest::default())
            .await
            .unwrap();

        // Drop the receiver — should not crash the server
        drop(rx);
        drop(client);

        // Server should still be functional
        tokio::time::sleep(Duration::from_millis(100)).await;
        let mut probe = raw_client(&url).await;
        let resp = probe
            .get_service_info(GetServiceInfoRequest {})
            .await
            .unwrap()
            .into_inner();
        assert_eq!(resp.version, PROTOCOL_VERSION);

        // Server-side streaming connection may linger; cap shutdown wait.
        let _ = tokio::time::timeout(Duration::from_secs(2), handle.shutdown()).await;
    }

    #[tokio::test]
    async fn multiple_health_subscribers() {
        let config = ServerConfig::default();
        let ctx = Arc::new(MockServiceContext::new());
        let handler = FlightServiceHandler::new(ctx, config.clone());
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

        // Two subscribers
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

        // Publish
        let event = HealthEvent {
            timestamp: 42,
            r#type: HealthEventType::Warning.into(),
            message: "shared event".into(),
            ..Default::default()
        };
        health_tx.send(event).unwrap();

        // Both should receive
        let e1 = tokio::time::timeout(Duration::from_secs(2), rx1.recv())
            .await
            .unwrap()
            .unwrap();
        let e2 = tokio::time::timeout(Duration::from_secs(2), rx2.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(e1.message, "shared event");
        assert_eq!(e2.message, "shared event");

        let _ = shutdown_tx.send(true);
    }

    #[tokio::test]
    async fn stream_survives_server_busy_with_requests() {
        let config = ServerConfig::default();
        let ctx = Arc::new(MockServiceContext::new());
        let handler = FlightServiceHandler::new(ctx, config.clone());
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

        let mut stream_client = IpcClient::connect(&url).await.unwrap();
        let mut rx = stream_client
            .subscribe_health(HealthSubscribeRequest::default())
            .await
            .unwrap();

        // Make some unary requests while stream is active
        let mut rpc_client = raw_client(&url).await;
        for _ in 0..5 {
            rpc_client
                .get_service_info(GetServiceInfoRequest {})
                .await
                .unwrap();
        }

        // Stream should still work
        let event = HealthEvent {
            timestamp: 100,
            message: "after requests".into(),
            ..Default::default()
        };
        health_tx.send(event).unwrap();

        let received = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(received.message, "after requests");

        let _ = shutdown_tx.send(true);
    }
}

// ===========================================================================
// 6. Property tests
// ===========================================================================

mod property_tests {
    use super::*;

    proptest! {
        #[test]
        fn negotiate_features_request_round_trip_any_version(
            major in 0u32..100,
            minor in 0u32..100,
            patch in 0u32..100,
        ) {
            let version = format!("{major}.{minor}.{patch}");
            let msg = NegotiateFeaturesRequest {
                client_version: version.clone(),
                supported_features: vec![],
                preferred_transport: 0,
            };
            let decoded = round_trip(&msg);
            prop_assert_eq!(decoded.client_version, version);
        }

        #[test]
        fn list_devices_request_round_trip_any_flags(
            include_disconnected in any::<bool>(),
        ) {
            let msg = ListDevicesRequest {
                include_disconnected,
                filter_types: vec![],
            };
            let decoded = round_trip(&msg);
            prop_assert_eq!(decoded.include_disconnected, include_disconnected);
        }

        #[test]
        fn apply_profile_request_round_trip_any_flags(
            validate_only in any::<bool>(),
            force_apply in any::<bool>(),
            json in "\\PC{0,128}",
        ) {
            let msg = ApplyProfileRequest {
                profile_json: json.clone(),
                validate_only,
                force_apply,
            };
            let decoded = round_trip(&msg);
            prop_assert_eq!(decoded.profile_json, json);
            prop_assert_eq!(decoded.validate_only, validate_only);
            prop_assert_eq!(decoded.force_apply, force_apply);
        }

        #[test]
        fn health_event_round_trip_any_type(
            event_type in 0i32..8,
            timestamp in any::<i64>(),
            message in "\\PC{0,64}",
        ) {
            let msg = HealthEvent {
                timestamp,
                r#type: event_type,
                message: message.clone(),
                ..Default::default()
            };
            let decoded = round_trip(&msg);
            prop_assert_eq!(decoded.timestamp, timestamp);
            prop_assert_eq!(decoded.r#type, event_type);
            prop_assert_eq!(decoded.message, message);
        }

        #[test]
        fn device_round_trip_any_type_status(
            device_type in 0i32..7,
            device_status in 0i32..5,
            id in "[a-z0-9]{1,16}",
        ) {
            let msg = Device {
                id: id.clone(),
                r#type: device_type,
                status: device_status,
                ..Default::default()
            };
            let decoded = round_trip(&msg);
            prop_assert_eq!(decoded.id, id);
            prop_assert_eq!(decoded.r#type, device_type);
            prop_assert_eq!(decoded.status, device_status);
        }

        #[test]
        fn capability_limits_round_trip_any_values(
            max_output in 0.0f32..1.0,
            max_torque in 0.0f32..100.0,
            allow_high in any::<bool>(),
        ) {
            let msg = CapabilityLimits {
                max_axis_output: max_output,
                max_ffb_torque: max_torque,
                allow_high_torque: allow_high,
                ..Default::default()
            };
            let decoded = round_trip(&msg);
            prop_assert_eq!(decoded.max_axis_output, max_output);
            prop_assert_eq!(decoded.max_ffb_torque, max_torque);
            prop_assert_eq!(decoded.allow_high_torque, allow_high);
        }

        #[test]
        fn arbitrary_bytes_never_panic_on_decode(
            bytes in proptest::collection::vec(any::<u8>(), 0..512),
        ) {
            // Must not panic
            let _ = ListDevicesRequest::decode(bytes.as_slice());
            let _ = ApplyProfileRequest::decode(bytes.as_slice());
            let _ = HealthEvent::decode(bytes.as_slice());
            let _ = NegotiateFeaturesRequest::decode(bytes.as_slice());
        }

        #[test]
        fn subscription_ids_are_unique(n in 2usize..50) {
            let mut mgr = SubscriptionManager::new();
            let mut ids = HashSet::new();
            for _ in 0..n {
                let h = mgr.subscribe(Topic::AxisData, SubscriptionFilter::default());
                prop_assert!(ids.insert(h.id), "duplicate subscription ID: {}", h.id);
            }
        }

        #[test]
        fn performance_metrics_round_trip_any_values(
            jitter in any::<f32>(),
            latency in any::<f32>(),
            ticks in any::<u32>(),
            frames in any::<u32>(),
            cpu in any::<f32>(),
            mem in any::<u64>(),
        ) {
            let msg = PerformanceMetrics {
                jitter_p99_ms: jitter,
                hid_latency_p99_us: latency,
                missed_ticks: ticks,
                dropped_frames: frames,
                cpu_usage_percent: cpu,
                memory_usage_bytes: mem,
            };
            let decoded = round_trip(&msg);
            // Float NaN != NaN, but prost preserves the bit pattern
            if !jitter.is_nan() {
                prop_assert_eq!(decoded.jitter_p99_ms, jitter);
            }
            if !latency.is_nan() {
                prop_assert_eq!(decoded.hid_latency_p99_us, latency);
            }
            if !cpu.is_nan() {
                prop_assert_eq!(decoded.cpu_usage_percent, cpu);
            }
            prop_assert_eq!(decoded.missed_ticks, ticks);
            prop_assert_eq!(decoded.dropped_frames, frames);
            prop_assert_eq!(decoded.memory_usage_bytes, mem);
        }

        #[test]
        fn conflict_metadata_float_arrays_round_trip(
            n in 1usize..20,
        ) {
            let inputs: Vec<f32> = (0..n).map(|i| i as f32 / n as f32).collect();
            let msg = ConflictMetadata {
                test_inputs: inputs.clone(),
                expected_outputs: inputs.clone(),
                actual_outputs: inputs.clone(),
                ..Default::default()
            };
            let decoded = round_trip(&msg);
            prop_assert_eq!(decoded.test_inputs, inputs);
        }
    }
}
