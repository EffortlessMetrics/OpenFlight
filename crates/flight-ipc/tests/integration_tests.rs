//! Integration tests for Flight Hub IPC
//!
//! These tests verify the core functionality of the IPC layer components,
//! including feature negotiation, device management, and protocol validation.

use flight_ipc::{
    PROTOCOL_VERSION,
    negotiation::{Version, detect_breaking_changes, negotiate_features},
    proto::{
        Device, DeviceCapabilities, DeviceHealth, DeviceStatus, DeviceType,
        NegotiateFeaturesRequest, TransportType,
    },
    server::{DeviceManager, MockDeviceManager, MockProfileManager, ProfileManager},
};
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

    // Test that device can be serialized and deserialized
    let json = serde_json::to_string(&device).unwrap();
    let deserialized: Device = serde_json::from_str(&json).unwrap();

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
