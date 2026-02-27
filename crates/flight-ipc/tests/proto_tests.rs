// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Protocol buffer round-trip tests and IPC type tests for flight-ipc.
//!
//! These tests run without any feature flags and focus on:
//!
//! 1.  `ListDevicesRequest` serializes and deserializes correctly
//! 2.  `ApplyProfileRequest` round-trip preserves every field
//! 3.  `NegotiateFeaturesRequest` round-trip
//! 4.  `GetSecurityStatusResponse` round-trip with plugin metadata
//! 5.  `ClientConfig` / `ServerConfig` defaults match `PROTOCOL_VERSION` and
//!     `SUPPORTED_FEATURES`
//! 6.  `PROTOCOL_VERSION` is a valid semver `MAJOR.MINOR.PATCH` string
//! 7.  Error responses carry status code and message faithfully
//! 8.  A large (>50 KB) payload does not crash serialization
//! 9.  `proptest`: round-tripping `ListDevicesRequest` with any bool flag works
//! 10. Empty `ListDevicesResponse` is valid proto3

use flight_ipc::{
    ClientConfig, IpcError, PROTOCOL_VERSION, SUPPORTED_FEATURES, ServerConfig,
    proto::{
        ApplyProfileRequest, ApplyProfileResponse, GetSecurityStatusRequest,
        GetSecurityStatusResponse, ListDevicesRequest, ListDevicesResponse,
        NegotiateFeaturesRequest, PluginInfo, PluginType, TransportType, ValidationError,
        ValidationErrorType,
    },
};
use proptest::prelude::*;
use prost::Message;

// ── 1. ListDevicesRequest round-trip ──────────────────────────────────────

#[test]
fn list_devices_request_with_include_disconnected_true_round_trips() {
    let req = ListDevicesRequest {
        include_disconnected: true,
        filter_types: vec![],
    };
    let mut buf = Vec::new();
    req.encode(&mut buf).unwrap();
    let decoded = ListDevicesRequest::decode(buf.as_slice()).unwrap();
    assert!(decoded.include_disconnected);
    assert!(decoded.filter_types.is_empty());
}

#[test]
fn list_devices_request_with_include_disconnected_false_round_trips() {
    let req = ListDevicesRequest {
        include_disconnected: false,
        filter_types: vec![],
    };
    let mut buf = Vec::new();
    req.encode(&mut buf).unwrap();
    let decoded = ListDevicesRequest::decode(buf.as_slice()).unwrap();
    assert!(!decoded.include_disconnected);
}

// ── 2. ApplyProfileRequest round-trip ─────────────────────────────────────

#[test]
fn apply_profile_request_preserves_all_fields() {
    let req = ApplyProfileRequest {
        profile_json: r#"{"version":"1.0","axes":{"pitch":{"expo":0.3}}}"#.to_string(),
        validate_only: true,
        force_apply: false,
    };
    let mut buf = Vec::new();
    req.encode(&mut buf).unwrap();
    let decoded = ApplyProfileRequest::decode(buf.as_slice()).unwrap();
    assert_eq!(decoded.profile_json, req.profile_json);
    assert!(decoded.validate_only);
    assert!(!decoded.force_apply);
}

#[test]
fn apply_profile_request_force_apply_flag_round_trips() {
    let req = ApplyProfileRequest {
        profile_json: "{}".to_string(),
        validate_only: false,
        force_apply: true,
    };
    let mut buf = Vec::new();
    req.encode(&mut buf).unwrap();
    let decoded = ApplyProfileRequest::decode(buf.as_slice()).unwrap();
    assert!(!decoded.validate_only);
    assert!(decoded.force_apply);
}

// ── 3. NegotiateFeaturesRequest round-trip ────────────────────────────────

#[test]
fn negotiate_features_request_round_trips_with_all_supported_features() {
    let req = NegotiateFeaturesRequest {
        client_version: PROTOCOL_VERSION.to_string(),
        supported_features: SUPPORTED_FEATURES.iter().map(|s| s.to_string()).collect(),
        preferred_transport: TransportType::NamedPipes.into(),
    };
    let mut buf = Vec::new();
    req.encode(&mut buf).unwrap();
    let decoded = NegotiateFeaturesRequest::decode(buf.as_slice()).unwrap();
    assert_eq!(decoded.client_version, PROTOCOL_VERSION);
    assert_eq!(decoded.supported_features.len(), SUPPORTED_FEATURES.len());
    for feat in SUPPORTED_FEATURES {
        assert!(
            decoded.supported_features.contains(&feat.to_string()),
            "missing feature after round-trip: {feat}"
        );
    }
}

#[test]
fn negotiate_features_request_with_empty_features_round_trips() {
    let req = NegotiateFeaturesRequest {
        client_version: "1.0.0".to_string(),
        supported_features: vec![],
        preferred_transport: TransportType::Unspecified.into(),
    };
    let mut buf = Vec::new();
    req.encode(&mut buf).unwrap();
    let decoded = NegotiateFeaturesRequest::decode(buf.as_slice()).unwrap();
    assert_eq!(decoded.client_version, "1.0.0");
    assert!(decoded.supported_features.is_empty());
}

// ── 4. GetSecurityStatusResponse round-trip ───────────────────────────────

#[test]
fn security_status_response_with_wasm_plugin_round_trips() {
    let resp = GetSecurityStatusResponse {
        success: true,
        error_message: String::new(),
        plugins: vec![PluginInfo {
            name: "test-plugin".to_string(),
            version: "1.0.0".to_string(),
            plugin_type: PluginType::Wasm.into(),
            signature_status: "verified".to_string(),
            capabilities: vec!["read-axes".to_string(), "read-health".to_string()],
        }],
        telemetry_enabled: false,
        telemetry_data_types: vec![],
    };
    let mut buf = Vec::new();
    resp.encode(&mut buf).unwrap();
    let decoded = GetSecurityStatusResponse::decode(buf.as_slice()).unwrap();

    assert!(decoded.success);
    assert_eq!(decoded.plugins.len(), 1);
    assert_eq!(decoded.plugins[0].name, "test-plugin");
    assert_eq!(decoded.plugins[0].plugin_type, PluginType::Wasm as i32);
    assert_eq!(decoded.plugins[0].capabilities.len(), 2);
    assert!(!decoded.telemetry_enabled);
}

#[test]
fn security_status_response_with_native_plugin_and_telemetry_round_trips() {
    let resp = GetSecurityStatusResponse {
        success: true,
        error_message: String::new(),
        plugins: vec![PluginInfo {
            name: "native-plugin".to_string(),
            version: "2.1.0".to_string(),
            plugin_type: PluginType::Native.into(),
            signature_status: "pending".to_string(),
            capabilities: vec!["write-axes".to_string()],
        }],
        telemetry_enabled: true,
        telemetry_data_types: vec!["Performance".to_string(), "Errors".to_string()],
    };
    let mut buf = Vec::new();
    resp.encode(&mut buf).unwrap();
    let decoded = GetSecurityStatusResponse::decode(buf.as_slice()).unwrap();

    assert_eq!(decoded.plugins[0].plugin_type, PluginType::Native as i32);
    assert!(decoded.telemetry_enabled);
    assert_eq!(decoded.telemetry_data_types, vec!["Performance", "Errors"]);
}

#[test]
fn security_status_request_empty_message_encodes_to_zero_bytes() {
    let req = GetSecurityStatusRequest {};
    let mut buf = Vec::new();
    req.encode(&mut buf).unwrap();
    // Proto3: messages with all fields at default values encode to empty bytes.
    assert!(
        buf.is_empty(),
        "empty proto3 message should encode to 0 bytes"
    );
    assert!(GetSecurityStatusRequest::decode(buf.as_slice()).is_ok());
}

// ── 5. ClientConfig and ServerConfig defaults ─────────────────────────────

#[test]
fn client_config_default_version_matches_protocol_version() {
    let config = ClientConfig::default();
    assert_eq!(
        config.client_version, PROTOCOL_VERSION,
        "client_version must match the crate constant"
    );
    assert!(
        !config.supported_features.is_empty(),
        "default client config must list at least one feature"
    );
    assert!(config.connection_timeout_ms > 0, "timeout must be positive");
}

#[test]
fn server_config_default_includes_all_supported_features() {
    let config = ServerConfig::default();
    assert_eq!(config.server_version, PROTOCOL_VERSION);
    for feature in SUPPORTED_FEATURES {
        assert!(
            config.enabled_features.iter().any(|f| f == feature),
            "server config missing feature: {feature}"
        );
    }
    assert!(
        config.max_connections > 0,
        "max_connections must be positive"
    );
}

// ── 6. Protocol version format ────────────────────────────────────────────

#[test]
fn protocol_version_is_semver_major_minor_patch() {
    let parts: Vec<&str> = PROTOCOL_VERSION.split('.').collect();
    assert_eq!(
        parts.len(),
        3,
        "PROTOCOL_VERSION must be MAJOR.MINOR.PATCH, got '{PROTOCOL_VERSION}'"
    );
    for part in &parts {
        part.parse::<u32>().unwrap_or_else(|_| {
            panic!("version component '{part}' must be a non-negative integer")
        });
    }
}

#[test]
fn supported_features_is_nonempty_and_includes_device_management() {
    assert!(
        !SUPPORTED_FEATURES.is_empty(),
        "SUPPORTED_FEATURES must not be empty"
    );
    assert!(
        SUPPORTED_FEATURES.contains(&"device-management"),
        "device-management must be a supported feature"
    );
}

// ── 7. Error responses carry status code and message faithfully ───────────

#[test]
fn apply_profile_response_error_preserves_message_and_validation_errors() {
    let resp = ApplyProfileResponse {
        success: false,
        error_message: "Profile validation failed: monotonicity constraint violated".to_string(),
        validation_errors: vec![ValidationError {
            field_path: "axes.pitch.curve".to_string(),
            line_number: 12,
            column_number: 3,
            error_message: "Curve must be monotonically increasing".to_string(),
            error_type: ValidationErrorType::Monotonic.into(),
        }],
        effective_profile_hash: String::new(),
        compile_time_ms: 0,
    };
    let mut buf = Vec::new();
    resp.encode(&mut buf).unwrap();
    let decoded = ApplyProfileResponse::decode(buf.as_slice()).unwrap();

    assert!(!decoded.success);
    assert!(
        decoded.error_message.contains("monotonicity"),
        "error message should be preserved: {}",
        decoded.error_message
    );
    assert_eq!(decoded.validation_errors.len(), 1);
    assert_eq!(decoded.validation_errors[0].line_number, 12);
    assert_eq!(decoded.validation_errors[0].column_number, 3);
    assert_eq!(
        decoded.validation_errors[0].error_type,
        ValidationErrorType::Monotonic as i32
    );
}

#[test]
fn ipc_error_version_mismatch_display_includes_both_versions() {
    let err = IpcError::VersionMismatch {
        client: "2.0.0".to_string(),
        server: "1.0.0".to_string(),
    };
    let msg = err.to_string();
    assert!(
        msg.contains("2.0.0"),
        "should contain client version: {msg}"
    );
    assert!(
        msg.contains("1.0.0"),
        "should contain server version: {msg}"
    );
}

#[test]
fn ipc_error_unsupported_feature_display_includes_feature_name() {
    let err = IpcError::UnsupportedFeature {
        feature: "real-time-telemetry".to_string(),
    };
    assert!(err.to_string().contains("real-time-telemetry"));
}

#[test]
fn ipc_error_connection_failed_display_includes_reason() {
    let err = IpcError::ConnectionFailed {
        reason: "Named pipe not found".to_string(),
    };
    assert!(err.to_string().contains("Named pipe not found"));
}

#[test]
fn ipc_error_from_grpc_status_wraps_message() {
    let status = tonic::Status::not_found("device not found");
    let err: IpcError = status.into();
    let msg = err.to_string();
    assert!(!msg.is_empty(), "wrapped gRPC error must have display text");
    // The original gRPC message must survive the conversion.
    assert!(
        msg.contains("not found") || msg.contains("NotFound") || msg.contains("grpc"),
        "display text should describe the gRPC failure: {msg}"
    );
}

// ── 8. Large payloads don't crash serialization ───────────────────────────

#[test]
fn apply_profile_request_with_large_json_payload_round_trips() {
    // Build a ~100 KB JSON object.
    let big_object: serde_json::Value = {
        let mut m = serde_json::Map::new();
        for i in 0..5_000 {
            m.insert(format!("key_{i}"), serde_json::json!(i));
        }
        serde_json::Value::Object(m)
    };
    let big_json = serde_json::to_string(&big_object).unwrap();
    let payload_len = big_json.len();
    assert!(
        payload_len > 50_000,
        "test payload must be >50 KB, got {payload_len} bytes"
    );

    let req = ApplyProfileRequest {
        profile_json: big_json.clone(),
        validate_only: true,
        force_apply: false,
    };
    let mut buf = Vec::new();
    req.encode(&mut buf).unwrap();
    let decoded = ApplyProfileRequest::decode(buf.as_slice()).unwrap();
    assert_eq!(
        decoded.profile_json.len(),
        big_json.len(),
        "large payload must be preserved exactly after encode/decode"
    );
}

#[test]
fn list_devices_response_with_many_devices_round_trips() {
    use flight_ipc::proto::{Device, DeviceStatus, DeviceType};

    let devices: Vec<Device> = (0..100)
        .map(|i| Device {
            id: format!("device-{i}"),
            name: format!("Device {i}"),
            r#type: DeviceType::Joystick.into(),
            status: DeviceStatus::Connected.into(),
            capabilities: None,
            health: None,
            metadata: Default::default(),
        })
        .collect();
    let total_count = devices.len() as i32;
    let resp = ListDevicesResponse {
        devices,
        total_count,
    };

    let mut buf = Vec::new();
    resp.encode(&mut buf).unwrap();
    let decoded = ListDevicesResponse::decode(buf.as_slice()).unwrap();

    assert_eq!(decoded.devices.len(), 100);
    assert_eq!(decoded.total_count, 100);
    assert_eq!(decoded.devices[0].id, "device-0");
    assert_eq!(decoded.devices[99].id, "device-99");
}

// ── 9 & 10. proptest / empty edge-cases ──────────────────────────────────

#[test]
fn empty_list_devices_response_is_valid_proto3() {
    let resp = ListDevicesResponse {
        devices: vec![],
        total_count: 0,
    };
    let mut buf = Vec::new();
    resp.encode(&mut buf).unwrap();
    let decoded = ListDevicesResponse::decode(buf.as_slice()).unwrap();
    assert!(decoded.devices.is_empty());
    assert_eq!(decoded.total_count, 0);
}

proptest! {
    /// Round-tripping `ListDevicesRequest` works for any combination of flags.
    #[test]
    fn list_devices_request_round_trips_with_any_include_flag(
        include_disconnected in any::<bool>()
    ) {
        let req = ListDevicesRequest {
            include_disconnected,
            filter_types: vec![],
        };
        let mut buf = Vec::new();
        req.encode(&mut buf).unwrap();
        let decoded = ListDevicesRequest::decode(buf.as_slice()).unwrap();
        prop_assert_eq!(decoded.include_disconnected, include_disconnected);
    }

    /// Round-tripping `ApplyProfileRequest` preserves both boolean flags and the JSON string.
    #[test]
    fn apply_profile_request_round_trips_with_any_flags(
        validate_only in any::<bool>(),
        force_apply in any::<bool>(),
        profile_json in "\\PC{0,256}",
    ) {
        let req = ApplyProfileRequest {
            profile_json: profile_json.clone(),
            validate_only,
            force_apply,
        };
        let mut buf = Vec::new();
        req.encode(&mut buf).unwrap();
        let decoded = ApplyProfileRequest::decode(buf.as_slice()).unwrap();
        prop_assert_eq!(decoded.profile_json, profile_json);
        prop_assert_eq!(decoded.validate_only, validate_only);
        prop_assert_eq!(decoded.force_apply, force_apply);
    }

    /// Round-tripping `NegotiateFeaturesRequest` preserves the version string.
    #[test]
    fn negotiate_features_request_round_trips_with_generated_version(
        major in 0u32..100,
        minor in 0u32..100,
        patch in 0u32..100,
    ) {
        let version = format!("{major}.{minor}.{patch}");
        let req = NegotiateFeaturesRequest {
            client_version: version.clone(),
            supported_features: vec![],
            preferred_transport: 0, // Unspecified
        };
        let mut buf = Vec::new();
        req.encode(&mut buf).unwrap();
        let decoded = NegotiateFeaturesRequest::decode(buf.as_slice()).unwrap();
        prop_assert_eq!(decoded.client_version, version);
    }

    /// Decoding random bytes never panics — only produces `Ok` or `Err`.
    #[test]
    fn list_devices_request_tolerates_arbitrary_bytes(
        bytes in proptest::collection::vec(any::<u8>(), 0..512)
    ) {
        // Must not panic; result can be Ok or Err.
        let _ = ListDevicesRequest::decode(bytes.as_slice());
    }
}
