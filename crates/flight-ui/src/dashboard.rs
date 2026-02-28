// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Dashboard state management for the Flight Hub web UI.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Overall dashboard state returned by the status endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardState {
    pub devices: Vec<DeviceStatus>,
    pub adapters: Vec<AdapterStatus>,
    pub axis_values: HashMap<String, f64>,
    pub profile: String,
    pub health: HealthStatus,
    pub uptime_secs: u64,
}

impl DashboardState {
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
            adapters: Vec::new(),
            axis_values: HashMap::new(),
            profile: String::from("default"),
            health: HealthStatus::Ok,
            uptime_secs: 0,
        }
    }
}

impl Default for DashboardState {
    fn default() -> Self {
        Self::new()
    }
}

/// Status of a connected input device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceStatus {
    pub id: String,
    pub name: String,
    pub connected: bool,
    pub axis_count: u32,
    pub button_count: u32,
    pub last_seen: DateTime<Utc>,
}

/// Status of a simulator adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterStatus {
    pub name: String,
    pub connected: bool,
    pub sim_name: String,
    pub aircraft: Option<String>,
    pub fps: Option<f64>,
}

/// Overall system health.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Ok,
    Degraded,
    Unavailable,
}

/// A named profile entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileEntry {
    pub name: String,
    pub active: bool,
}

/// WebSocket message sent to connected clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsMessage {
    AxisUpdate {
        axis: String,
        value: f64,
    },
    DeviceEvent {
        device_id: String,
        event: DeviceEventKind,
    },
    AdapterEvent {
        adapter: String,
        connected: bool,
    },
}

/// Kind of device event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceEventKind {
    Connected,
    Disconnected,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_state_default() {
        let state = DashboardState::default();
        assert!(state.devices.is_empty());
        assert!(state.adapters.is_empty());
        assert!(state.axis_values.is_empty());
        assert_eq!(state.profile, "default");
        assert_eq!(state.health, HealthStatus::Ok);
        assert_eq!(state.uptime_secs, 0);
    }

    #[test]
    fn dashboard_state_serializes_to_json() {
        let state = DashboardState::new();
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("\"profile\":\"default\""));
        assert!(json.contains("\"health\":\"ok\""));
    }

    #[test]
    fn dashboard_state_round_trip() {
        let mut state = DashboardState::new();
        state.uptime_secs = 42;
        state.profile = "combat".to_string();
        state.health = HealthStatus::Degraded;
        state.axis_values.insert("roll".into(), 0.5);

        let json = serde_json::to_string(&state).unwrap();
        let restored: DashboardState = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.uptime_secs, 42);
        assert_eq!(restored.profile, "combat");
        assert_eq!(restored.health, HealthStatus::Degraded);
        assert!((restored.axis_values["roll"] - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn device_status_serializes() {
        let dev = DeviceStatus {
            id: "dev-001".into(),
            name: "Warthog Throttle".into(),
            connected: true,
            axis_count: 5,
            button_count: 32,
            last_seen: Utc::now(),
        };
        let json = serde_json::to_string(&dev).unwrap();
        assert!(json.contains("Warthog Throttle"));
        assert!(json.contains("\"connected\":true"));
    }

    #[test]
    fn device_status_round_trip() {
        let dev = DeviceStatus {
            id: "dev-002".into(),
            name: "Cougar MFD".into(),
            connected: false,
            axis_count: 0,
            button_count: 20,
            last_seen: Utc::now(),
        };
        let json = serde_json::to_string(&dev).unwrap();
        let restored: DeviceStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, "dev-002");
        assert!(!restored.connected);
    }

    #[test]
    fn adapter_status_serializes() {
        let adapter = AdapterStatus {
            name: "simconnect".into(),
            connected: true,
            sim_name: "MSFS 2024".into(),
            aircraft: Some("F/A-18C".into()),
            fps: Some(60.0),
        };
        let json = serde_json::to_string(&adapter).unwrap();
        assert!(json.contains("MSFS 2024"));
        assert!(json.contains("F/A-18C"));
    }

    #[test]
    fn adapter_status_optional_fields_null() {
        let adapter = AdapterStatus {
            name: "xplane".into(),
            connected: false,
            sim_name: "X-Plane 12".into(),
            aircraft: None,
            fps: None,
        };
        let json = serde_json::to_string(&adapter).unwrap();
        assert!(json.contains("\"aircraft\":null"));
        assert!(json.contains("\"fps\":null"));
    }

    #[test]
    fn health_status_variants_serialize() {
        assert_eq!(serde_json::to_string(&HealthStatus::Ok).unwrap(), "\"ok\"");
        assert_eq!(
            serde_json::to_string(&HealthStatus::Degraded).unwrap(),
            "\"degraded\""
        );
        assert_eq!(
            serde_json::to_string(&HealthStatus::Unavailable).unwrap(),
            "\"unavailable\""
        );
    }

    #[test]
    fn health_status_round_trip() {
        for variant in [
            HealthStatus::Ok,
            HealthStatus::Degraded,
            HealthStatus::Unavailable,
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            let restored: HealthStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(restored, variant);
        }
    }

    #[test]
    fn ws_message_axis_update_serializes() {
        let msg = WsMessage::AxisUpdate {
            axis: "pitch".into(),
            value: -0.75,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"axis_update\""));
        assert!(json.contains("\"axis\":\"pitch\""));
    }

    #[test]
    fn ws_message_device_event_serializes() {
        let msg = WsMessage::DeviceEvent {
            device_id: "hid-1".into(),
            event: DeviceEventKind::Connected,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"device_event\""));
        assert!(json.contains("\"event\":\"connected\""));
    }

    #[test]
    fn ws_message_adapter_event_serializes() {
        let msg = WsMessage::AdapterEvent {
            adapter: "dcs".into(),
            connected: false,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"adapter_event\""));
        assert!(json.contains("\"connected\":false"));
    }

    #[test]
    fn ws_message_round_trip() {
        let msg = WsMessage::AxisUpdate {
            axis: "yaw".into(),
            value: 0.33,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let restored: WsMessage = serde_json::from_str(&json).unwrap();
        match restored {
            WsMessage::AxisUpdate { axis, value } => {
                assert_eq!(axis, "yaw");
                assert!((value - 0.33).abs() < f64::EPSILON);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn device_event_kind_serializes() {
        assert_eq!(
            serde_json::to_string(&DeviceEventKind::Connected).unwrap(),
            "\"connected\""
        );
        assert_eq!(
            serde_json::to_string(&DeviceEventKind::Disconnected).unwrap(),
            "\"disconnected\""
        );
    }

    #[test]
    fn profile_entry_serializes() {
        let entry = ProfileEntry {
            name: "combat".into(),
            active: true,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"name\":\"combat\""));
        assert!(json.contains("\"active\":true"));
    }

    #[test]
    fn dashboard_state_with_devices_and_adapters() {
        let mut state = DashboardState::new();
        state.devices.push(DeviceStatus {
            id: "d1".into(),
            name: "Stick".into(),
            connected: true,
            axis_count: 3,
            button_count: 12,
            last_seen: Utc::now(),
        });
        state.adapters.push(AdapterStatus {
            name: "msfs".into(),
            connected: true,
            sim_name: "MSFS 2024".into(),
            aircraft: Some("A320".into()),
            fps: Some(30.0),
        });
        let json = serde_json::to_string(&state).unwrap();
        let restored: DashboardState = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.devices.len(), 1);
        assert_eq!(restored.adapters.len(), 1);
        assert_eq!(restored.devices[0].name, "Stick");
        assert_eq!(restored.adapters[0].aircraft.as_deref(), Some("A320"));
    }
}
