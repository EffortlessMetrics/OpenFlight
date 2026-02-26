// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Newline-delimited JSON protocol shared between Flight Hub and the X-Plane plugin.
//!
//! These types mirror `flight-xplane::plugin::{PluginMessage, PluginResponse}`.
//! They are intentionally re-defined here so the plugin crate can be built
//! independently without pulling in the full flight-xplane dependency tree.

use serde::{Deserialize, Serialize};

/// Messages sent from Flight Hub → plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PluginMessage {
    Handshake {
        version: String,
        capabilities: Vec<String>,
    },
    GetDataRef {
        id: u32,
        name: String,
    },
    SetDataRef {
        id: u32,
        name: String,
        value: serde_json::Value,
    },
    Subscribe {
        id: u32,
        name: String,
        frequency: f32,
    },
    Unsubscribe {
        id: u32,
        name: String,
    },
    Command {
        id: u32,
        name: String,
    },
    GetAircraftInfo {
        id: u32,
    },
    Ping {
        id: u32,
        timestamp: u64,
    },
}

/// Messages sent from plugin → Flight Hub.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PluginResponse {
    HandshakeAck {
        version: String,
        capabilities: Vec<String>,
        status: String,
    },
    DataRefValue {
        id: u32,
        name: String,
        value: serde_json::Value,
        timestamp: u64,
    },
    DataRefUpdate {
        name: String,
        value: serde_json::Value,
        timestamp: u64,
    },
    CommandResult {
        id: u32,
        success: bool,
        message: Option<String>,
    },
    AircraftInfo {
        id: u32,
        icao: String,
        title: String,
        author: String,
        file_path: String,
    },
    Error {
        id: Option<u32>,
        error: String,
        details: Option<String>,
    },
    Pong {
        id: u32,
        timestamp: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_message_handshake_round_trips() {
        let msg = PluginMessage::Handshake {
            version: "1.0".to_string(),
            capabilities: vec!["subscribe".to_string()],
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("Handshake"));
        // Deserialise back
        let decoded: PluginMessage = serde_json::from_str(&json).unwrap();
        if let PluginMessage::Handshake { version, .. } = decoded {
            assert_eq!(version, "1.0");
        } else {
            panic!("Wrong variant after round-trip");
        }
    }

    #[test]
    fn plugin_response_pong_round_trips() {
        let resp = PluginResponse::Pong {
            id: 42,
            timestamp: 999,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let decoded: PluginResponse = serde_json::from_str(&json).unwrap();
        if let PluginResponse::Pong { id, timestamp } = decoded {
            assert_eq!(id, 42);
            assert_eq!(timestamp, 999);
        } else {
            panic!("Wrong variant after round-trip");
        }
    }

    #[test]
    fn plugin_response_error_optional_fields() {
        let resp = PluginResponse::Error {
            id: None,
            error: "not found".to_string(),
            details: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let decoded: PluginResponse = serde_json::from_str(&json).unwrap();
        if let PluginResponse::Error { id, error, details } = decoded {
            assert!(id.is_none());
            assert_eq!(error, "not found");
            assert!(details.is_none());
        } else {
            panic!("Wrong variant after round-trip");
        }
    }

    #[test]
    fn plugin_message_get_dataref_serialises_id_and_name() {
        let msg = PluginMessage::GetDataRef {
            id: 7,
            name: "sim/cockpit/autopilot/altitude".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("GetDataRef"));
        assert!(json.contains("sim/cockpit/autopilot/altitude"));
    }

    #[test]
    fn malformed_json_returns_error() {
        let result = serde_json::from_str::<PluginMessage>("{not valid json}");
        assert!(
            result.is_err(),
            "malformed JSON must return an error, not panic"
        );

        let result2 = serde_json::from_str::<PluginMessage>(r#"{"type":"UnknownVariant"}"#);
        assert!(result2.is_err(), "unknown type tag must return an error");
    }

    #[test]
    fn handshake_json_contains_version_and_capabilities_fields() {
        let msg = PluginMessage::Handshake {
            version: "2.0".to_string(),
            capabilities: vec!["subscribe".to_string(), "commands".to_string()],
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(
            json.contains("\"version\""),
            "JSON must contain version field"
        );
        assert!(
            json.contains("\"capabilities\""),
            "JSON must contain capabilities field"
        );
    }

    #[test]
    fn get_dataref_and_dataref_value_ids_match() {
        let request = PluginMessage::GetDataRef {
            id: 99,
            name: "sim/test/dataref".to_string(),
        };
        let request_json = serde_json::to_string(&request).unwrap();
        assert!(request_json.contains("99"));

        let response = PluginResponse::DataRefValue {
            id: 99,
            name: "sim/test/dataref".to_string(),
            value: serde_json::json!(1.5),
            timestamp: 1000,
        };
        let response_json = serde_json::to_string(&response).unwrap();
        let decoded: PluginResponse = serde_json::from_str(&response_json).unwrap();
        if let PluginResponse::DataRefValue { id, name, .. } = decoded {
            assert_eq!(id, 99, "DataRefValue id must match GetDataRef id");
            assert_eq!(name, "sim/test/dataref");
        } else {
            panic!("Wrong variant after round-trip");
        }
    }
}
