// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for UI view models: axis visualization, device status,
//! profile data, and event log data representations.

use chrono::Utc;
use flight_ui::dashboard::{
    AdapterStatus, DashboardState, DeviceEventKind, DeviceStatus, HealthStatus, ProfileEntry,
    WsMessage,
};

// ---------------------------------------------------------------------------
// Axis visualization data
// ---------------------------------------------------------------------------

#[test]
fn axis_values_preserve_full_range() {
    let mut state = DashboardState::new();
    state.axis_values.insert("roll".into(), -1.0);
    state.axis_values.insert("pitch".into(), 1.0);
    state.axis_values.insert("yaw".into(), 0.0);
    state.axis_values.insert("throttle".into(), 0.5);

    let json = serde_json::to_string(&state).unwrap();
    let restored: DashboardState = serde_json::from_str(&json).unwrap();

    assert!((restored.axis_values["roll"] - (-1.0)).abs() < f64::EPSILON);
    assert!((restored.axis_values["pitch"] - 1.0).abs() < f64::EPSILON);
    assert!((restored.axis_values["yaw"]).abs() < f64::EPSILON);
    assert!((restored.axis_values["throttle"] - 0.5).abs() < f64::EPSILON);
}

#[test]
fn axis_values_many_axes_round_trip() {
    let mut state = DashboardState::new();
    for i in 0..64 {
        let name = format!("axis_{i}");
        let value = (i as f64) / 64.0;
        state.axis_values.insert(name, value);
    }

    let json = serde_json::to_string(&state).unwrap();
    let restored: DashboardState = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.axis_values.len(), 64);
    for i in 0..64 {
        let expected = (i as f64) / 64.0;
        let actual = restored.axis_values[&format!("axis_{i}")];
        assert!(
            (actual - expected).abs() < f64::EPSILON,
            "axis_{i} mismatch: {actual} != {expected}"
        );
    }
}

#[test]
fn axis_update_ws_message_negative_value() {
    let msg = WsMessage::AxisUpdate {
        axis: "rudder".into(),
        value: -0.999,
    };
    let json = serde_json::to_string(&msg).unwrap();
    let restored: WsMessage = serde_json::from_str(&json).unwrap();
    match restored {
        WsMessage::AxisUpdate { axis, value } => {
            assert_eq!(axis, "rudder");
            assert!((value - (-0.999)).abs() < f64::EPSILON);
        }
        _ => panic!("wrong variant after round-trip"),
    }
}

#[test]
fn axis_update_ws_message_zero_value() {
    let msg = WsMessage::AxisUpdate {
        axis: "brake_left".into(),
        value: 0.0,
    };
    let json = serde_json::to_string(&msg).unwrap();
    let restored: WsMessage = serde_json::from_str(&json).unwrap();
    match restored {
        WsMessage::AxisUpdate { axis, value } => {
            assert_eq!(axis, "brake_left");
            assert!((value - 0.0).abs() < f64::EPSILON);
        }
        _ => panic!("wrong variant after round-trip"),
    }
}

// ---------------------------------------------------------------------------
// Device status data
// ---------------------------------------------------------------------------

#[test]
fn device_status_multiple_devices_distinct_ids() {
    let devices: Vec<DeviceStatus> = (0..5)
        .map(|i| DeviceStatus {
            id: format!("dev-{i}"),
            name: format!("Device {i}"),
            connected: i % 2 == 0,
            axis_count: i * 2,
            button_count: i * 4,
            last_seen: Utc::now(),
        })
        .collect();

    let json = serde_json::to_string(&devices).unwrap();
    let restored: Vec<DeviceStatus> = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.len(), 5);

    for i in 0..5 {
        let expected_id = format!("dev-{i}");
        assert!(restored.iter().any(|d| d.id == expected_id));
    }
}

#[test]
fn device_status_disconnected_preserves_counts() {
    let dev = DeviceStatus {
        id: "hid-0".into(),
        name: "Offline Throttle".into(),
        connected: false,
        axis_count: 6,
        button_count: 24,
        last_seen: Utc::now(),
    };
    let json = serde_json::to_string(&dev).unwrap();
    let restored: DeviceStatus = serde_json::from_str(&json).unwrap();
    assert!(!restored.connected);
    assert_eq!(restored.axis_count, 6);
    assert_eq!(restored.button_count, 24);
}

// ---------------------------------------------------------------------------
// Profile data
// ---------------------------------------------------------------------------

#[test]
fn profile_entry_inactive_serializes() {
    let entry = ProfileEntry {
        name: "cruise".into(),
        active: false,
    };
    let json = serde_json::to_string(&entry).unwrap();
    assert!(json.contains("\"active\":false"));
    let restored: ProfileEntry = serde_json::from_str(&json).unwrap();
    assert!(!restored.active);
}

#[test]
fn profile_entry_empty_name_allowed() {
    let entry = ProfileEntry {
        name: String::new(),
        active: true,
    };
    let json = serde_json::to_string(&entry).unwrap();
    let restored: ProfileEntry = serde_json::from_str(&json).unwrap();
    assert!(restored.name.is_empty());
}

#[test]
fn dashboard_profile_name_with_special_chars() {
    let mut state = DashboardState::new();
    state.profile = "combat/night — v2.1".into();
    let json = serde_json::to_string(&state).unwrap();
    let restored: DashboardState = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.profile, "combat/night — v2.1");
}

// ---------------------------------------------------------------------------
// Event log data (WsMessage variants as event log entries)
// ---------------------------------------------------------------------------

#[test]
fn device_event_connected_round_trip() {
    let msg = WsMessage::DeviceEvent {
        device_id: "stick-usb-3".into(),
        event: DeviceEventKind::Connected,
    };
    let json = serde_json::to_string(&msg).unwrap();
    let restored: WsMessage = serde_json::from_str(&json).unwrap();
    match restored {
        WsMessage::DeviceEvent { device_id, event } => {
            assert_eq!(device_id, "stick-usb-3");
            assert_eq!(event, DeviceEventKind::Connected);
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn device_event_disconnected_round_trip() {
    let msg = WsMessage::DeviceEvent {
        device_id: "throttle-7".into(),
        event: DeviceEventKind::Disconnected,
    };
    let json = serde_json::to_string(&msg).unwrap();
    let restored: WsMessage = serde_json::from_str(&json).unwrap();
    match restored {
        WsMessage::DeviceEvent { device_id, event } => {
            assert_eq!(device_id, "throttle-7");
            assert_eq!(event, DeviceEventKind::Disconnected);
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn adapter_event_disconnected_round_trip() {
    let msg = WsMessage::AdapterEvent {
        adapter: "dcs-export".into(),
        connected: false,
    };
    let json = serde_json::to_string(&msg).unwrap();
    let restored: WsMessage = serde_json::from_str(&json).unwrap();
    match restored {
        WsMessage::AdapterEvent {
            adapter,
            connected,
        } => {
            assert_eq!(adapter, "dcs-export");
            assert!(!connected);
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn ws_message_all_variants_deserialize_by_tag() {
    let axis_json = r#"{"type":"axis_update","axis":"roll","value":0.5}"#;
    let device_json = r#"{"type":"device_event","device_id":"d1","event":"connected"}"#;
    let adapter_json = r#"{"type":"adapter_event","adapter":"msfs","connected":true}"#;

    let _: WsMessage = serde_json::from_str(axis_json).unwrap();
    let _: WsMessage = serde_json::from_str(device_json).unwrap();
    let _: WsMessage = serde_json::from_str(adapter_json).unwrap();
}

#[test]
fn ws_message_unknown_type_tag_is_rejected() {
    let bad_json = r#"{"type":"unknown_event","data":"foo"}"#;
    let result = serde_json::from_str::<WsMessage>(bad_json);
    assert!(result.is_err(), "unknown type tag must be rejected");
}

#[test]
fn dashboard_state_complex_snapshot() {
    let mut state = DashboardState::new();
    state.profile = "night-ops".into();
    state.health = HealthStatus::Degraded;
    state.uptime_secs = 3600;

    state.devices.push(DeviceStatus {
        id: "stick-1".into(),
        name: "Warthog Stick".into(),
        connected: true,
        axis_count: 3,
        button_count: 19,
        last_seen: Utc::now(),
    });
    state.devices.push(DeviceStatus {
        id: "throttle-1".into(),
        name: "Warthog Throttle".into(),
        connected: true,
        axis_count: 5,
        button_count: 32,
        last_seen: Utc::now(),
    });
    state.devices.push(DeviceStatus {
        id: "rudder-1".into(),
        name: "MFG Crosswind".into(),
        connected: false,
        axis_count: 3,
        button_count: 0,
        last_seen: Utc::now(),
    });

    state.adapters.push(AdapterStatus {
        name: "simconnect".into(),
        connected: true,
        sim_name: "MSFS 2024".into(),
        aircraft: Some("F-16C Viper".into()),
        fps: Some(45.0),
    });
    state.adapters.push(AdapterStatus {
        name: "dcs-export".into(),
        connected: false,
        sim_name: "DCS World".into(),
        aircraft: None,
        fps: None,
    });

    state.axis_values.insert("roll".into(), 0.15);
    state.axis_values.insert("pitch".into(), -0.32);
    state.axis_values.insert("yaw".into(), 0.0);
    state.axis_values.insert("throttle_left".into(), 0.8);
    state.axis_values.insert("throttle_right".into(), 0.82);

    let json = serde_json::to_string(&state).unwrap();
    let restored: DashboardState = serde_json::from_str(&json).unwrap();

    assert_eq!(restored.profile, "night-ops");
    assert_eq!(restored.health, HealthStatus::Degraded);
    assert_eq!(restored.uptime_secs, 3600);
    assert_eq!(restored.devices.len(), 3);
    assert_eq!(restored.adapters.len(), 2);
    assert_eq!(restored.axis_values.len(), 5);

    let disconnected: Vec<_> = restored.devices.iter().filter(|d| !d.connected).collect();
    assert_eq!(disconnected.len(), 1);
    assert_eq!(disconnected[0].id, "rudder-1");
}
