// SPDX-License-Identifier: MIT OR Apache-2.0

//! Snapshot tests for IPC message serialization, subscription topics,
//! and connection state display.

use flight_ipc::messages::{ComponentStatus, IpcMessage, ServiceState};
use flight_ipc::subscriptions::{SubscriptionFilter, Topic};

// ── IPC message JSON serialization ──────────────────────────────────────────

#[test]
fn snapshot_device_connected_json() {
    let msg = IpcMessage::DeviceConnected {
        device_id: "dev-001".into(),
        name: "Thrustmaster T.Flight HOTAS 4".into(),
        vid: 0x044F,
        pid: 0xB10A,
    };
    insta::assert_snapshot!("device_connected_json", msg.to_json());
}

#[test]
fn snapshot_device_disconnected_json() {
    let msg = IpcMessage::DeviceDisconnected {
        device_id: "dev-001".into(),
        reason: "USB cable unplugged".into(),
    };
    insta::assert_snapshot!("device_disconnected_json", msg.to_json());
}

#[test]
fn snapshot_device_input_json() {
    let msg = IpcMessage::DeviceInput {
        device_id: "stick-1".into(),
        axes: vec![0.0, 0.5, -0.75, 1.0],
        buttons: vec![false, true, false, true, false],
    };
    insta::assert_snapshot!("device_input_json", msg.to_json());
}

#[test]
fn snapshot_profile_activated_with_aircraft_json() {
    let msg = IpcMessage::ProfileActivated {
        name: "combat-f16".into(),
        aircraft: Some("F-16C".into()),
    };
    insta::assert_snapshot!("profile_activated_with_aircraft_json", msg.to_json());
}

#[test]
fn snapshot_profile_activated_without_aircraft_json() {
    let msg = IpcMessage::ProfileActivated {
        name: "default-ga".into(),
        aircraft: None,
    };
    insta::assert_snapshot!("profile_activated_without_aircraft_json", msg.to_json());
}

#[test]
fn snapshot_profile_error_json() {
    let msg = IpcMessage::ProfileError {
        name: "corrupted-profile".into(),
        error: "JSON parse error at line 42: unexpected token".into(),
    };
    insta::assert_snapshot!("profile_error_json", msg.to_json());
}

#[test]
fn snapshot_service_status_all_states() {
    let states = [
        ServiceState::Starting,
        ServiceState::Running,
        ServiceState::Degraded,
        ServiceState::Stopping,
        ServiceState::Stopped,
    ];
    let mut output = String::new();
    for state in &states {
        let msg = IpcMessage::ServiceStatus {
            status: *state,
            uptime_secs: 3600,
        };
        output.push_str(&msg.to_json());
        output.push('\n');
    }
    insta::assert_snapshot!("service_status_all_states", output);
}

#[test]
fn snapshot_health_report_json() {
    let msg = IpcMessage::HealthReport {
        components: vec![
            ComponentStatus {
                name: "axis-engine".into(),
                healthy: true,
                detail: None,
            },
            ComponentStatus {
                name: "ffb-engine".into(),
                healthy: false,
                detail: Some("envelope limit exceeded on channel 0".into()),
            },
            ComponentStatus {
                name: "hid-manager".into(),
                healthy: true,
                detail: None,
            },
        ],
    };
    insta::assert_snapshot!("health_report_json", msg.to_json());
}

#[test]
fn snapshot_sim_connected_json() {
    let msg = IpcMessage::SimConnected {
        sim_type: "msfs".into(),
        version: "2024.1.28".into(),
    };
    insta::assert_snapshot!("sim_connected_json", msg.to_json());
}

#[test]
fn snapshot_telemetry_update_json() {
    let msg = IpcMessage::TelemetryUpdate {
        altitude: 35000.0,
        airspeed: 250.0,
        heading: 90.0,
    };
    insta::assert_snapshot!("telemetry_update_json", msg.to_json());
}

// ── Subscription topic display ──────────────────────────────────────────────

#[test]
fn snapshot_all_topic_display_names() {
    let mut output = String::new();
    for topic in Topic::ALL {
        output.push_str(&format!("{}\n", topic));
    }
    insta::assert_snapshot!("all_topic_display_names", output);
}

#[test]
fn snapshot_subscription_filter_yaml() {
    let filter = SubscriptionFilter {
        device_id: Some("dev-001".into()),
        axis_id: Some("pitch".into()),
        min_interval_ms: Some(100),
        changed_only: true,
    };
    insta::assert_yaml_snapshot!("subscription_filter", filter);
}

#[test]
fn snapshot_subscription_filter_default_yaml() {
    let filter = SubscriptionFilter::default();
    insta::assert_yaml_snapshot!("subscription_filter_default", filter);
}

// ── IPC error display ───────────────────────────────────────────────────────

#[test]
fn snapshot_ipc_error_display() {
    let errors = vec![
        flight_ipc::IpcError::VersionMismatch {
            client: "1.0.0".into(),
            server: "2.0.0".into(),
        },
        flight_ipc::IpcError::UnsupportedFeature {
            feature: "force-feedback-v2".into(),
        },
        flight_ipc::IpcError::ConnectionFailed {
            reason: "named pipe not found".into(),
        },
    ];
    let mut output = String::new();
    for err in &errors {
        output.push_str(&format!("{}\n", err));
    }
    insta::assert_snapshot!("ipc_error_display", output);
}
