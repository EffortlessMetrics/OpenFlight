// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Snapshot tests for `flight-ipc` message formats and configuration defaults.
//!
//! Guards against accidental changes to the wire format of IPC messages and
//! the default values of client/server configurations.
//! Run `cargo insta review` to accept new or changed snapshots.

use flight_ipc::messages::{ComponentStatus, IpcMessage, ServiceState};
use flight_ipc::{ClientConfig, ServerConfig, PROTOCOL_VERSION, SUPPORTED_FEATURES};

// ── IpcMessage JSON format snapshots (every variant) ─────────────────────────

#[test]
fn snapshot_ipc_message_device_connected() {
    let msg = IpcMessage::DeviceConnected {
        device_id: "usb-044f-b10a-0".into(),
        name: "Thrustmaster T.16000M".into(),
        vid: 0x044F,
        pid: 0xB10A,
    };
    insta::assert_snapshot!("ipc_msg_device_connected", msg.to_json());
}

#[test]
fn snapshot_ipc_message_device_disconnected() {
    let msg = IpcMessage::DeviceDisconnected {
        device_id: "usb-044f-b10a-0".into(),
        reason: "USB cable unplugged".into(),
    };
    insta::assert_snapshot!("ipc_msg_device_disconnected", msg.to_json());
}

#[test]
fn snapshot_ipc_message_device_input() {
    let msg = IpcMessage::DeviceInput {
        device_id: "usb-044f-b10a-0".into(),
        axes: vec![0.0, 0.5, -1.0, 0.25],
        buttons: vec![false, true, false, false, true],
    };
    insta::assert_snapshot!("ipc_msg_device_input", msg.to_json());
}

#[test]
fn snapshot_ipc_message_profile_activated() {
    let msg = IpcMessage::ProfileActivated {
        name: "cessna-172-ifr".into(),
        aircraft: Some("C172".into()),
    };
    insta::assert_snapshot!("ipc_msg_profile_activated", msg.to_json());
}

#[test]
fn snapshot_ipc_message_profile_deactivated() {
    let msg = IpcMessage::ProfileDeactivated {
        name: "cessna-172-ifr".into(),
    };
    insta::assert_snapshot!("ipc_msg_profile_deactivated", msg.to_json());
}

#[test]
fn snapshot_ipc_message_profile_error() {
    let msg = IpcMessage::ProfileError {
        name: "broken-profile".into(),
        error: "axis 'pitch' deadzone 0.9 exceeds maximum 0.5".into(),
    };
    insta::assert_snapshot!("ipc_msg_profile_error", msg.to_json());
}

#[test]
fn snapshot_ipc_message_service_status() {
    let msg = IpcMessage::ServiceStatus {
        status: ServiceState::Running,
        uptime_secs: 86400,
    };
    insta::assert_snapshot!("ipc_msg_service_status", msg.to_json());
}

#[test]
fn snapshot_ipc_message_health_report() {
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
                detail: Some("envelope exceeded on device usb-044f".into()),
            },
            ComponentStatus {
                name: "hid-manager".into(),
                healthy: true,
                detail: None,
            },
        ],
    };
    insta::assert_snapshot!("ipc_msg_health_report", msg.to_json());
}

#[test]
fn snapshot_ipc_message_sim_connected() {
    let msg = IpcMessage::SimConnected {
        sim_type: "msfs".into(),
        version: "2024.1".into(),
    };
    insta::assert_snapshot!("ipc_msg_sim_connected", msg.to_json());
}

#[test]
fn snapshot_ipc_message_sim_disconnected() {
    let msg = IpcMessage::SimDisconnected {
        sim_type: "xplane".into(),
    };
    insta::assert_snapshot!("ipc_msg_sim_disconnected", msg.to_json());
}

#[test]
fn snapshot_ipc_message_telemetry_update() {
    let msg = IpcMessage::TelemetryUpdate {
        altitude: 35_000.0,
        airspeed: 250.0,
        heading: 090.0,
    };
    insta::assert_snapshot!("ipc_msg_telemetry_update", msg.to_json());
}

// ── Service state enum completeness ──────────────────────────────────────────

#[test]
fn snapshot_service_state_all_variants_json() {
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
            uptime_secs: 0,
        };
        output.push_str(&msg.to_json());
        output.push('\n');
    }
    insta::assert_snapshot!("service_state_all_variants", output);
}

// ── Configuration defaults ───────────────────────────────────────────────────

#[test]
fn snapshot_client_config_default() {
    let cfg = ClientConfig::default();
    insta::assert_debug_snapshot!("client_config_default", cfg);
}

#[test]
fn snapshot_server_config_default() {
    let cfg = ServerConfig::default();
    insta::assert_debug_snapshot!("server_config_default", cfg);
}

// ── Protocol constants ───────────────────────────────────────────────────────

#[test]
fn snapshot_protocol_version() {
    insta::assert_snapshot!("protocol_version", PROTOCOL_VERSION);
}

#[test]
fn snapshot_supported_features() {
    let features: Vec<&str> = SUPPORTED_FEATURES.to_vec();
    insta::assert_debug_snapshot!("supported_features", features);
}

// ── IpcError display strings ─────────────────────────────────────────────────

#[test]
fn snapshot_ipc_error_display_catalog() {
    use flight_ipc::IpcError;

    let errors: Vec<String> = vec![
        IpcError::VersionMismatch {
            client: "2.0.0".into(),
            server: "1.0.0".into(),
        }
        .to_string(),
        IpcError::UnsupportedFeature {
            feature: "force-feedback-v2".into(),
        }
        .to_string(),
        IpcError::ConnectionFailed {
            reason: "pipe not found".into(),
        }
        .to_string(),
    ];
    insta::assert_debug_snapshot!("ipc_error_display_catalog", errors);
}
