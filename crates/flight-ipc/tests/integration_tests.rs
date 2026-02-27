//! Integration tests for Flight Hub IPC
//!
//! These tests verify the core functionality of the IPC layer components,
//! including feature negotiation, device management, and protocol validation.

use flight_ipc::{
    PROTOCOL_VERSION,
    negotiation::negotiate_features,
    proto::{
        Device, DeviceCapabilities, DeviceHealth, DeviceStatus, DeviceType,
        NegotiateFeaturesRequest, TransportType,
    },
    server::{DeviceManager, MockDeviceManager, MockProfileManager, ProfileManager},
};
use prost::Message;
use std::collections::HashMap;
use std::time::SystemTime;

fn create_test_device() -> Device {
    Device {
        id: "test-joystick-1".to_string(),
        name: "Test Joystick".to_string(),
        r#type: DeviceType::Joystick.into(),
        status: DeviceStatus::Connected.into(),
        capabilities: Some(DeviceCapabilities {
            supports_force_feedback: false,
            supports_raw_torque: false,
            max_torque_nm: 0,
            min_period_us: 1000,
            has_health_stream: true,
            supported_protocols: vec!["hid".to_string()],
        }),
        health: Some(DeviceHealth {
            temperature_celsius: 25.5,
            current_amperes: 0.1,
            packet_loss_count: 0,
            last_seen_timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            active_faults: vec![],
        }),
        metadata: [("vendor".to_string(), "Test Corp".to_string())]
            .iter()
            .cloned()
            .collect(),
    }
}

#[test]
fn test_feature_negotiation_unit() {
    let request = NegotiateFeaturesRequest {
        client_version: "1.0.0".to_string(),
        supported_features: vec![
            "device-management".to_string(),
            "health-monitoring".to_string(),
        ],
        preferred_transport: TransportType::NamedPipes.into(),
    };

    let server_features = vec![
        "device-management".to_string(),
        "health-monitoring".to_string(),
        "profile-management".to_string(),
    ];

    let response = negotiate_features(&request, &server_features).unwrap();

    assert!(response.success);
    assert_eq!(response.server_version, PROTOCOL_VERSION);
    assert!(
        response
            .enabled_features
            .contains(&"device-management".to_string())
    );
    assert!(
        response
            .enabled_features
            .contains(&"health-monitoring".to_string())
    );
    assert!(
        !response
            .enabled_features
            .contains(&"profile-management".to_string())
    );
}

#[test]
fn test_device_manager_unit() {
    let device_manager = MockDeviceManager;

    let request = flight_ipc::proto::ListDevicesRequest {
        include_disconnected: false,
        filter_types: vec![],
    };

    let response = device_manager.list_devices(&request).unwrap();

    // Mock device manager returns empty list
    assert_eq!(response.devices.len(), 0);
    assert_eq!(response.total_count, 0);
}

#[test]
fn test_profile_manager_unit() {
    let profile_manager = MockProfileManager;

    // Test successful profile
    let request = flight_ipc::proto::ApplyProfileRequest {
        profile_json: r#"{"test": "profile"}"#.to_string(),
        validate_only: false,
        force_apply: false,
    };

    let response = profile_manager.apply_profile(&request).unwrap();

    assert!(response.success);
    assert_eq!(response.effective_profile_hash, "mock-hash");
    assert_eq!(response.compile_time_ms, 10);

    // Test empty profile
    let request = flight_ipc::proto::ApplyProfileRequest {
        profile_json: String::new(),
        validate_only: false,
        force_apply: false,
    };

    let response = profile_manager.apply_profile(&request).unwrap();

    assert!(response.success); // Mock always succeeds
}

#[test]
fn test_device_serialization() {
    let device = create_test_device();

    // Generated protobuf types are validated via protobuf round-trip.
    let mut bytes = Vec::new();
    device.encode(&mut bytes).unwrap();
    let deserialized = Device::decode(bytes.as_slice()).unwrap();

    assert_eq!(device.id, deserialized.id);
    assert_eq!(device.name, deserialized.name);
    assert_eq!(device.r#type, deserialized.r#type);
    assert_eq!(device.status, deserialized.status);
}

#[tokio::test]
async fn test_version_compatibility() {
    use flight_ipc::negotiation::Version;

    // Test version parsing
    let v1_0_0 = Version::parse("1.0.0").unwrap();
    let v1_1_0 = Version::parse("1.1.0").unwrap();
    let v2_0_0 = Version::parse("2.0.0").unwrap();

    // Test compatibility rules
    assert!(v1_1_0.is_compatible_with(&v1_0_0)); // Newer minor version is compatible
    assert!(!v1_0_0.is_compatible_with(&v1_1_0)); // Older version is not compatible
    assert!(!v2_0_0.is_compatible_with(&v1_0_0)); // Different major version is not compatible
    assert!(!v1_0_0.is_compatible_with(&v2_0_0)); // Different major version is not compatible
}

#[tokio::test]
async fn test_breaking_change_detection() {
    use flight_ipc::negotiation::detect_breaking_changes;

    let old_schema = r#"
    service FlightService {
        rpc ListDevices(ListDevicesRequest) returns (ListDevicesResponse);
        rpc HealthSubscribe(HealthSubscribeRequest) returns (stream HealthEvent);
    }
    
    message Device {
        string id = 1;
        string name = 2;
    }
    "#;

    let new_schema = r#"
    service FlightService {
        rpc ListDevices(ListDevicesRequest) returns (ListDevicesResponse);
        // HealthSubscribe removed - this is a breaking change
    }
    
    message Device {
        string id = 1;
        string name = 2;
        // Added new field - this is not breaking
        string type = 3;
    }
    "#;

    let breaking_changes = detect_breaking_changes(old_schema, new_schema).unwrap();

    // Should detect the removed RPC
    assert!(!breaking_changes.is_empty());
    assert!(
        breaking_changes
            .iter()
            .any(|change| change.contains("HealthSubscribe"))
    );
}

// ── Proto round-trip tests ─────────────────────────────────────────────────

#[test]
fn test_negotiate_features_response_round_trip() {
    use flight_ipc::proto::NegotiateFeaturesResponse;

    let response = NegotiateFeaturesResponse {
        success: true,
        server_version: "1.0.0".to_string(),
        enabled_features: vec![
            "device-management".to_string(),
            "health-monitoring".to_string(),
        ],
        negotiated_transport: TransportType::NamedPipes.into(),
        error_message: String::new(),
    };

    let mut bytes = Vec::new();
    response.encode(&mut bytes).unwrap();
    let decoded = NegotiateFeaturesResponse::decode(bytes.as_slice()).unwrap();

    assert_eq!(decoded.success, response.success);
    assert_eq!(decoded.server_version, response.server_version);
    assert_eq!(decoded.enabled_features, response.enabled_features);
    assert_eq!(decoded.negotiated_transport, response.negotiated_transport);
}

#[test]
fn test_list_devices_response_round_trip() {
    use flight_ipc::proto::ListDevicesResponse;

    let response = ListDevicesResponse {
        devices: vec![create_test_device()],
        total_count: 1,
    };

    let mut bytes = Vec::new();
    response.encode(&mut bytes).unwrap();
    let decoded = ListDevicesResponse::decode(bytes.as_slice()).unwrap();

    assert_eq!(decoded.total_count, 1);
    assert_eq!(decoded.devices.len(), 1);
    assert_eq!(decoded.devices[0].id, "test-joystick-1");
    assert_eq!(decoded.devices[0].name, "Test Joystick");
}

#[test]
fn test_apply_profile_request_round_trip() {
    use flight_ipc::proto::ApplyProfileRequest;

    let request = ApplyProfileRequest {
        profile_json: r#"{"version": "1.0", "axes": {}}"#.to_string(),
        validate_only: true,
        force_apply: false,
    };

    let mut bytes = Vec::new();
    request.encode(&mut bytes).unwrap();
    let decoded = ApplyProfileRequest::decode(bytes.as_slice()).unwrap();

    assert_eq!(decoded.profile_json, request.profile_json);
    assert_eq!(decoded.validate_only, request.validate_only);
    assert_eq!(decoded.force_apply, request.force_apply);
}

#[test]
fn test_apply_profile_response_with_errors_round_trip() {
    use flight_ipc::proto::{ApplyProfileResponse, ValidationError, ValidationErrorType};

    let response = ApplyProfileResponse {
        success: false,
        error_message: "Validation failed".to_string(),
        validation_errors: vec![ValidationError {
            field_path: "axes.pitch.curve".to_string(),
            line_number: 42,
            column_number: 5,
            error_message: "Curve must be monotonically increasing".to_string(),
            error_type: ValidationErrorType::Monotonic.into(),
        }],
        effective_profile_hash: String::new(),
        compile_time_ms: 0,
    };

    let mut bytes = Vec::new();
    response.encode(&mut bytes).unwrap();
    let decoded = ApplyProfileResponse::decode(bytes.as_slice()).unwrap();

    assert!(!decoded.success);
    assert_eq!(decoded.validation_errors.len(), 1);
    assert_eq!(decoded.validation_errors[0].field_path, "axes.pitch.curve");
    assert_eq!(decoded.validation_errors[0].line_number, 42);
    assert_eq!(decoded.validation_errors[0].column_number, 5);
}

#[test]
fn test_get_service_info_response_round_trip() {
    use flight_ipc::proto::{GetServiceInfoResponse, ServiceStatus};

    let mut capabilities = HashMap::new();
    capabilities.insert("force-feedback".to_string(), "true".to_string());
    capabilities.insert("max-devices".to_string(), "8".to_string());

    let response = GetServiceInfoResponse {
        version: "1.2.3".to_string(),
        uptime_seconds: 86400,
        status: ServiceStatus::Running.into(),
        capabilities,
    };

    let mut bytes = Vec::new();
    response.encode(&mut bytes).unwrap();
    let decoded = GetServiceInfoResponse::decode(bytes.as_slice()).unwrap();

    assert_eq!(decoded.version, "1.2.3");
    assert_eq!(decoded.uptime_seconds, 86400);
    assert_eq!(decoded.status, ServiceStatus::Running as i32);
    assert_eq!(
        decoded.capabilities.get("force-feedback"),
        Some(&"true".to_string())
    );
    assert_eq!(decoded.capabilities.len(), 2);
}

#[test]
fn test_health_event_with_performance_metrics_round_trip() {
    use flight_ipc::proto::{HealthEvent, HealthEventType, PerformanceMetrics};

    let event = HealthEvent {
        timestamp: 1_700_000_000,
        r#type: HealthEventType::Performance.into(),
        message: "RT jitter spike detected".to_string(),
        device_id: "joystick-1".to_string(),
        error_code: "RT_JITTER_001".to_string(),
        metadata: [("tick".to_string(), "12345".to_string())]
            .iter()
            .cloned()
            .collect(),
        performance: Some(PerformanceMetrics {
            jitter_p99_ms: 0.42,
            hid_latency_p99_us: 285.0,
            missed_ticks: 0,
            dropped_frames: 2,
            cpu_usage_percent: 12.5,
            memory_usage_bytes: 1024 * 1024,
        }),
    };

    let mut bytes = Vec::new();
    event.encode(&mut bytes).unwrap();
    let decoded = HealthEvent::decode(bytes.as_slice()).unwrap();

    assert_eq!(decoded.timestamp, 1_700_000_000);
    assert_eq!(decoded.device_id, "joystick-1");
    assert_eq!(decoded.error_code, "RT_JITTER_001");
    let perf = decoded.performance.unwrap();
    assert!((perf.jitter_p99_ms - 0.42).abs() < 1e-5);
    assert_eq!(perf.missed_ticks, 0);
    assert_eq!(perf.memory_usage_bytes, 1024 * 1024);
}

// ── Curve conflict message round-trips ─────────────────────────────────────

#[test]
fn test_detect_curve_conflicts_request_round_trip() {
    use flight_ipc::proto::DetectCurveConflictsRequest;

    let request = DetectCurveConflictsRequest {
        axis_names: vec!["pitch".to_string(), "roll".to_string()],
        sim_id: "msfs".to_string(),
        aircraft_id: "cessna-172".to_string(),
    };

    let mut bytes = Vec::new();
    request.encode(&mut bytes).unwrap();
    let decoded = DetectCurveConflictsRequest::decode(bytes.as_slice()).unwrap();

    assert_eq!(decoded.axis_names, vec!["pitch", "roll"]);
    assert_eq!(decoded.sim_id, "msfs");
    assert_eq!(decoded.aircraft_id, "cessna-172");
}

#[test]
fn test_detect_curve_conflicts_response_round_trip() {
    use flight_ipc::proto::{
        ConflictMetadata, ConflictSeverity, ConflictType, CurveConflict,
        DetectCurveConflictsResponse,
    };

    let response = DetectCurveConflictsResponse {
        success: true,
        conflicts: vec![CurveConflict {
            axis_name: "pitch".to_string(),
            conflict_type: ConflictType::DoubleCurve.into(),
            severity: ConflictSeverity::Medium.into(),
            description: "Both MSFS and profile have pitch curves".to_string(),
            suggested_resolutions: vec![],
            metadata: Some(ConflictMetadata {
                sim_curve_strength: 0.8,
                profile_curve_strength: 0.6,
                combined_nonlinearity: 0.9,
                test_inputs: vec![0.0, 0.5, 1.0],
                expected_outputs: vec![0.0, 0.5, 1.0],
                actual_outputs: vec![0.0, 0.35, 1.0],
                detection_timestamp: 1_700_000_000,
            }),
        }],
        error_message: String::new(),
    };

    let mut bytes = Vec::new();
    response.encode(&mut bytes).unwrap();
    let decoded = DetectCurveConflictsResponse::decode(bytes.as_slice()).unwrap();

    assert!(decoded.success);
    assert_eq!(decoded.conflicts.len(), 1);
    assert_eq!(decoded.conflicts[0].axis_name, "pitch");
    assert_eq!(
        decoded.conflicts[0].conflict_type,
        ConflictType::DoubleCurve as i32
    );
    assert_eq!(
        decoded.conflicts[0].severity,
        ConflictSeverity::Medium as i32
    );
    let meta = decoded.conflicts[0].metadata.as_ref().unwrap();
    assert!((meta.sim_curve_strength - 0.8).abs() < 1e-5);
    assert_eq!(meta.test_inputs.len(), 3);
}

#[test]
fn test_resolve_curve_conflict_response_round_trip() {
    use flight_ipc::proto::{
        ConflictMetadata, ResolutionResult, ResolutionType, ResolveCurveConflictResponse,
    };

    let response = ResolveCurveConflictResponse {
        success: true,
        error_message: String::new(),
        result: Some(ResolutionResult {
            applied_resolution: ResolutionType::DisableProfileCurve.into(),
            modified_files: vec!["curves.json".to_string()],
            backup_path: "/tmp/backup/curves.json.bak".to_string(),
            before_metrics: Some(ConflictMetadata {
                sim_curve_strength: 0.8,
                profile_curve_strength: 0.7,
                combined_nonlinearity: 0.95,
                test_inputs: vec![0.0, 1.0],
                expected_outputs: vec![0.0, 1.0],
                actual_outputs: vec![0.0, 0.82],
                detection_timestamp: 1_700_000_000,
            }),
            after_metrics: None,
            verification_passed: true,
            verification_details: "No conflict detected post-resolution".to_string(),
        }),
    };

    let mut bytes = Vec::new();
    response.encode(&mut bytes).unwrap();
    let decoded = ResolveCurveConflictResponse::decode(bytes.as_slice()).unwrap();

    assert!(decoded.success);
    let result = decoded.result.unwrap();
    assert_eq!(result.backup_path, "/tmp/backup/curves.json.bak");
    assert!(result.verification_passed);
    assert!(result.before_metrics.is_some());
    assert!(result.after_metrics.is_none());
}

#[test]
fn test_one_click_resolve_round_trip() {
    use flight_ipc::proto::{
        BackupInfo, ConflictMetrics, OneClickResolveRequest, OneClickResolveResponse,
        OneClickResult, ResolutionMetrics, ResolutionStep, ResolutionType, VerificationOutcome,
    };

    let request = OneClickResolveRequest {
        axis_name: "pitch".to_string(),
        create_backup: true,
        verify_resolution: true,
    };

    let mut req_bytes = Vec::new();
    request.encode(&mut req_bytes).unwrap();
    let decoded_req = OneClickResolveRequest::decode(req_bytes.as_slice()).unwrap();
    assert_eq!(decoded_req.axis_name, "pitch");
    assert!(decoded_req.create_backup);

    let response = OneClickResolveResponse {
        success: true,
        error_message: String::new(),
        result: Some(OneClickResult {
            resolution_type: ResolutionType::DisableSimCurve.into(),
            modified_files: vec!["profile.json".to_string()],
            backup_info: Some(BackupInfo {
                timestamp: 1_700_000_000,
                description: "Pre-resolution backup".to_string(),
                affected_files: vec!["profile.json".to_string()],
                backup_dir: "/tmp/backups".to_string(),
                writer_config: "{}".to_string(),
            }),
            verification: Some(VerificationOutcome {
                passed: true,
                details: "Conflict resolved successfully".to_string(),
                duration_ms: 150,
                conflict_resolved: true,
            }),
            metrics: Some(ResolutionMetrics {
                before: Some(ConflictMetrics {
                    nonlinearity: 0.9,
                    sim_curve_strength: 0.8,
                    profile_curve_strength: 0.6,
                    timestamp: 1_700_000_000,
                }),
                after: Some(ConflictMetrics {
                    nonlinearity: 0.2,
                    sim_curve_strength: 0.0,
                    profile_curve_strength: 0.6,
                    timestamp: 1_700_000_001,
                }),
                improvement: 0.78,
            }),
            steps_performed: vec![ResolutionStep {
                name: "disable_sim_curve".to_string(),
                description: "Disabled simulator pitch curve".to_string(),
                success: true,
                duration_ms: 100,
                error: String::new(),
            }],
        }),
    };

    let mut resp_bytes = Vec::new();
    response.encode(&mut resp_bytes).unwrap();
    let decoded = OneClickResolveResponse::decode(resp_bytes.as_slice()).unwrap();

    assert!(decoded.success);
    let result = decoded.result.unwrap();
    assert_eq!(result.modified_files, vec!["profile.json"]);
    let verification = result.verification.unwrap();
    assert!(verification.passed);
    assert!(verification.conflict_resolved);
    assert_eq!(verification.duration_ms, 150);
    let metrics = result.metrics.unwrap();
    assert!((metrics.improvement - 0.78).abs() < 1e-5);
    assert_eq!(result.steps_performed.len(), 1);
    assert!(result.steps_performed[0].success);
}

// ── Capability message round-trips ─────────────────────────────────────────

#[test]
fn test_set_capability_mode_request_round_trip() {
    use flight_ipc::proto::{CapabilityMode, SetCapabilityModeRequest};

    let request = SetCapabilityModeRequest {
        mode: CapabilityMode::Kid.into(),
        axis_names: vec!["pitch".to_string(), "roll".to_string()],
        audit_enabled: true,
    };

    let mut bytes = Vec::new();
    request.encode(&mut bytes).unwrap();
    let decoded = SetCapabilityModeRequest::decode(bytes.as_slice()).unwrap();

    assert_eq!(decoded.mode, CapabilityMode::Kid as i32);
    assert_eq!(decoded.axis_names, vec!["pitch", "roll"]);
    assert!(decoded.audit_enabled);
}

#[test]
fn test_set_capability_mode_response_round_trip() {
    use flight_ipc::proto::{CapabilityLimits, SetCapabilityModeResponse};

    let response = SetCapabilityModeResponse {
        success: true,
        error_message: String::new(),
        affected_axes: vec!["pitch".to_string()],
        applied_limits: Some(CapabilityLimits {
            max_axis_output: 0.5,
            max_ffb_torque: 1.0,
            max_slew_rate: 2.0,
            max_curve_expo: 0.3,
            allow_high_torque: false,
            allow_custom_curves: false,
        }),
    };

    let mut bytes = Vec::new();
    response.encode(&mut bytes).unwrap();
    let decoded = SetCapabilityModeResponse::decode(bytes.as_slice()).unwrap();

    assert!(decoded.success);
    let limits = decoded.applied_limits.unwrap();
    assert!((limits.max_axis_output - 0.5).abs() < 1e-5);
    assert!((limits.max_ffb_torque - 1.0).abs() < 1e-5);
    assert!(!limits.allow_high_torque);
    assert!(!limits.allow_custom_curves);
}

#[test]
fn test_get_capability_mode_response_round_trip() {
    use flight_ipc::proto::{
        AxisCapabilityStatus, CapabilityLimits, CapabilityMode, GetCapabilityModeResponse,
    };

    let response = GetCapabilityModeResponse {
        success: true,
        error_message: String::new(),
        axis_status: vec![AxisCapabilityStatus {
            axis_name: "pitch".to_string(),
            mode: CapabilityMode::Demo.into(),
            limits: Some(CapabilityLimits {
                max_axis_output: 0.75,
                max_ffb_torque: 2.0,
                max_slew_rate: 5.0,
                max_curve_expo: 0.5,
                allow_high_torque: false,
                allow_custom_curves: true,
            }),
            audit_enabled: false,
            clamp_events_count: 3,
            last_clamp_timestamp: 1_700_000_500,
        }],
    };

    let mut bytes = Vec::new();
    response.encode(&mut bytes).unwrap();
    let decoded = GetCapabilityModeResponse::decode(bytes.as_slice()).unwrap();

    assert!(decoded.success);
    assert_eq!(decoded.axis_status.len(), 1);
    let status = &decoded.axis_status[0];
    assert_eq!(status.axis_name, "pitch");
    assert_eq!(status.mode, CapabilityMode::Demo as i32);
    assert_eq!(status.clamp_events_count, 3);
    assert_eq!(status.last_clamp_timestamp, 1_700_000_500);
    let limits = status.limits.as_ref().unwrap();
    assert!(limits.allow_custom_curves);
}

// ── Security / telemetry message round-trips ───────────────────────────────

#[test]
fn test_get_security_status_response_round_trip() {
    use flight_ipc::proto::{GetSecurityStatusResponse, PluginInfo, PluginType};

    let response = GetSecurityStatusResponse {
        success: true,
        error_message: String::new(),
        plugins: vec![PluginInfo {
            name: "my-wasm-plugin".to_string(),
            version: "0.1.0".to_string(),
            plugin_type: PluginType::Wasm.into(),
            signature_status: "verified".to_string(),
            capabilities: vec!["read-axes".to_string()],
        }],
        telemetry_enabled: true,
        telemetry_data_types: vec!["Performance".to_string(), "Errors".to_string()],
    };

    let mut bytes = Vec::new();
    response.encode(&mut bytes).unwrap();
    let decoded = GetSecurityStatusResponse::decode(bytes.as_slice()).unwrap();

    assert!(decoded.success);
    assert_eq!(decoded.plugins.len(), 1);
    assert_eq!(decoded.plugins[0].name, "my-wasm-plugin");
    assert_eq!(decoded.plugins[0].plugin_type, PluginType::Wasm as i32);
    assert!(decoded.telemetry_enabled);
    assert_eq!(decoded.telemetry_data_types, vec!["Performance", "Errors"]);
}

#[test]
fn test_configure_telemetry_round_trip() {
    use flight_ipc::proto::{ConfigureTelemetryRequest, ConfigureTelemetryResponse};

    let request = ConfigureTelemetryRequest {
        enabled: true,
        data_types: vec!["Performance".to_string(), "DeviceEvents".to_string()],
    };

    let mut req_bytes = Vec::new();
    request.encode(&mut req_bytes).unwrap();
    let decoded_req = ConfigureTelemetryRequest::decode(req_bytes.as_slice()).unwrap();

    assert!(decoded_req.enabled);
    assert_eq!(decoded_req.data_types.len(), 2);
    assert_eq!(decoded_req.data_types[0], "Performance");

    let response = ConfigureTelemetryResponse {
        success: true,
        error_message: String::new(),
    };

    let mut resp_bytes = Vec::new();
    response.encode(&mut resp_bytes).unwrap();
    let decoded_resp = ConfigureTelemetryResponse::decode(resp_bytes.as_slice()).unwrap();

    assert!(decoded_resp.success);
    assert!(decoded_resp.error_message.is_empty());
}

#[test]
fn test_get_support_bundle_response_round_trip() {
    use flight_ipc::proto::GetSupportBundleResponse;

    let response = GetSupportBundleResponse {
        success: true,
        error_message: String::new(),
        redacted_data: r#"{"log_lines": ["[INFO] Service started"]}"#.to_string(),
        bundle_size_bytes: 4096,
    };

    let mut bytes = Vec::new();
    response.encode(&mut bytes).unwrap();
    let decoded = GetSupportBundleResponse::decode(bytes.as_slice()).unwrap();

    assert!(decoded.success);
    assert_eq!(decoded.bundle_size_bytes, 4096);
    assert!(decoded.redacted_data.contains("log_lines"));
}

// ── Edge cases ─────────────────────────────────────────────────────────────

#[test]
fn test_empty_messages_are_valid() {
    use flight_ipc::proto::{
        GetCapabilityModeRequest, GetSecurityStatusRequest, GetServiceInfoRequest,
        GetSupportBundleRequest,
    };

    // Proto3 messages with no fields / all-default values encode to zero bytes.
    let mut buf = Vec::new();
    GetServiceInfoRequest {}.encode(&mut buf).unwrap();
    assert!(buf.is_empty());
    assert!(GetServiceInfoRequest::decode(buf.as_slice()).is_ok());

    let mut buf = Vec::new();
    GetSecurityStatusRequest {}.encode(&mut buf).unwrap();
    assert!(buf.is_empty());
    assert!(GetSecurityStatusRequest::decode(buf.as_slice()).is_ok());

    let mut buf = Vec::new();
    GetSupportBundleRequest {}.encode(&mut buf).unwrap();
    assert!(buf.is_empty());
    assert!(GetSupportBundleRequest::decode(buf.as_slice()).is_ok());

    // GetCapabilityModeRequest with empty axis list
    let req = GetCapabilityModeRequest { axis_names: vec![] };
    let mut buf = Vec::new();
    req.encode(&mut buf).unwrap();
    let decoded = GetCapabilityModeRequest::decode(buf.as_slice()).unwrap();
    assert!(decoded.axis_names.is_empty());
}

#[test]
fn test_messages_with_all_optional_fields_omitted_are_valid() {
    use flight_ipc::proto::{
        ApplyProfileResponse, CurveConflict, OneClickResolveResponse, SetCapabilityModeResponse,
    };

    // ApplyProfileResponse with no validation errors and no profile hash
    let response = ApplyProfileResponse {
        success: true,
        error_message: String::new(),
        validation_errors: vec![],
        effective_profile_hash: String::new(),
        compile_time_ms: 0,
    };
    let mut bytes = Vec::new();
    response.encode(&mut bytes).unwrap();
    let decoded = ApplyProfileResponse::decode(bytes.as_slice()).unwrap();
    assert!(decoded.success);
    assert!(decoded.validation_errors.is_empty());

    // CurveConflict with no metadata and no resolutions
    let conflict = CurveConflict {
        axis_name: "pitch".to_string(),
        conflict_type: 0, // Unspecified
        severity: 0,      // Unspecified
        description: String::new(),
        suggested_resolutions: vec![],
        metadata: None,
    };
    let mut bytes = Vec::new();
    conflict.encode(&mut bytes).unwrap();
    let decoded = CurveConflict::decode(bytes.as_slice()).unwrap();
    assert_eq!(decoded.axis_name, "pitch");
    assert!(decoded.metadata.is_none());

    // OneClickResolveResponse with no result (failure case)
    let resp = OneClickResolveResponse {
        success: false,
        error_message: "Nothing to resolve".to_string(),
        result: None,
    };
    let mut bytes = Vec::new();
    resp.encode(&mut bytes).unwrap();
    let decoded = OneClickResolveResponse::decode(bytes.as_slice()).unwrap();
    assert!(!decoded.success);
    assert!(decoded.result.is_none());

    // SetCapabilityModeResponse with no applied_limits (failure case)
    let resp = SetCapabilityModeResponse {
        success: false,
        error_message: "Mode not supported".to_string(),
        affected_axes: vec![],
        applied_limits: None,
    };
    let mut bytes = Vec::new();
    resp.encode(&mut bytes).unwrap();
    let decoded = SetCapabilityModeResponse::decode(bytes.as_slice()).unwrap();
    assert!(!decoded.success);
    assert!(decoded.applied_limits.is_none());
}

#[test]
fn test_maximal_device_round_trip() {
    let device = Device {
        id: "joystick-ffb-001".to_string(),
        name: "VKB Gladiator NXT EVO R".to_string(),
        r#type: DeviceType::ForceFeedback.into(),
        status: DeviceStatus::Connected.into(),
        capabilities: Some(DeviceCapabilities {
            supports_force_feedback: true,
            supports_raw_torque: true,
            max_torque_nm: 5,
            min_period_us: 500,
            has_health_stream: true,
            supported_protocols: vec![
                "hid".to_string(),
                "ffb-legacy".to_string(),
                "ffb-extended".to_string(),
            ],
        }),
        health: Some(DeviceHealth {
            temperature_celsius: 38.7,
            current_amperes: 0.85,
            packet_loss_count: 12,
            last_seen_timestamp: 1_700_000_000,
            active_faults: vec!["TEMP_HIGH".to_string()],
        }),
        metadata: [
            ("vendor".to_string(), "VKB".to_string()),
            ("firmware".to_string(), "4.2.1".to_string()),
            ("serial".to_string(), "VKB-12345".to_string()),
            (
                "hid_path".to_string(),
                r"\\?\HID#VID_231D&PID_0127".to_string(),
            ),
        ]
        .iter()
        .cloned()
        .collect(),
    };

    let mut bytes = Vec::new();
    device.encode(&mut bytes).unwrap();
    let decoded = Device::decode(bytes.as_slice()).unwrap();

    assert_eq!(decoded.id, "joystick-ffb-001");
    assert_eq!(decoded.r#type, DeviceType::ForceFeedback as i32);
    let caps = decoded.capabilities.unwrap();
    assert!(caps.supports_force_feedback);
    assert!(caps.supports_raw_torque);
    assert_eq!(caps.max_torque_nm, 5);
    assert_eq!(caps.supported_protocols.len(), 3);
    let health = decoded.health.unwrap();
    assert!((health.temperature_celsius - 38.7).abs() < 1e-4);
    assert_eq!(health.active_faults, vec!["TEMP_HIGH"]);
    assert_eq!(decoded.metadata.len(), 4);
}

// ── Error propagation ──────────────────────────────────────────────────────

#[test]
fn test_ipc_error_version_mismatch_display() {
    use flight_ipc::IpcError;

    let err = IpcError::VersionMismatch {
        client: "2.0.0".to_string(),
        server: "1.0.0".to_string(),
    };

    let msg = err.to_string();
    assert!(
        msg.contains("2.0.0"),
        "error message should contain client version: {msg}"
    );
    assert!(
        msg.contains("1.0.0"),
        "error message should contain server version: {msg}"
    );
}

#[test]
fn test_ipc_error_unsupported_feature_display() {
    use flight_ipc::IpcError;

    let err = IpcError::UnsupportedFeature {
        feature: "real-time-telemetry".to_string(),
    };

    let msg = err.to_string();
    assert!(
        msg.contains("real-time-telemetry"),
        "error message should contain feature name: {msg}"
    );
}

#[test]
fn test_ipc_error_connection_failed_display() {
    use flight_ipc::IpcError;

    let err = IpcError::ConnectionFailed {
        reason: "pipe not found".to_string(),
    };

    let msg = err.to_string();
    assert!(
        msg.contains("pipe not found"),
        "error message should contain reason: {msg}"
    );
}

#[test]
fn test_ipc_error_from_grpc_status() {
    use flight_ipc::IpcError;

    let status = tonic::Status::not_found("device not found");
    let err: IpcError = status.into();
    let msg = err.to_string();
    // Verify the IpcError wraps the gRPC status and produces a non-empty message.
    assert!(
        !msg.is_empty(),
        "IpcError from gRPC status should have display text"
    );
    assert!(
        msg.to_lowercase().contains("not found")
            || msg.to_lowercase().contains("notfound")
            || msg.to_lowercase().contains("grpc"),
        "error should describe the gRPC failure: {msg}"
    );
}

#[test]
fn test_grpc_status_codes_propagate_correctly() {
    use flight_ipc::IpcError;

    let statuses = [
        tonic::Status::not_found("device not found"),
        tonic::Status::invalid_argument("invalid profile JSON"),
        tonic::Status::internal("internal RT error"),
        tonic::Status::unavailable("daemon not running"),
        tonic::Status::unauthenticated("missing credentials"),
        tonic::Status::permission_denied("access denied"),
        tonic::Status::resource_exhausted("too many connections"),
    ];

    for status in &statuses {
        let err: IpcError = status.clone().into();
        let err_str = err.to_string();
        assert!(
            !err_str.is_empty(),
            "IpcError display should be non-empty for status code {:?}",
            status.code()
        );
        // The original status message should be preserved somewhere in the error
        assert!(
            err_str.contains(status.message()),
            "IpcError should contain original gRPC message. Got: {err_str}"
        );
    }
}
