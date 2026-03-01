// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! IPC message catalog for Flight Hub inter-process communication.
//!
//! Defines the complete set of structured messages exchanged between the
//! daemon (`flightd`) and its clients over gRPC/IPC.  Every variant carries
//! its own payload and can be round-tripped through JSON via [`serde`].

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors that can occur during message parsing.
#[derive(Debug, Error)]
pub enum ParseError {
    /// JSON deserialization failed.
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
}

// ---------------------------------------------------------------------------
// Service state
// ---------------------------------------------------------------------------

/// High-level state of the Flight Hub service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceState {
    /// Service is initializing subsystems.
    Starting,
    /// Service is fully operational.
    Running,
    /// One or more subsystems are unhealthy.
    Degraded,
    /// Service is shutting down gracefully.
    Stopping,
    /// Service has stopped.
    Stopped,
}

// ---------------------------------------------------------------------------
// Component status
// ---------------------------------------------------------------------------

/// Health status of a single service component.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentStatus {
    /// Component name (e.g. `"axis-engine"`, `"ffb-engine"`).
    pub name: String,
    /// `true` when the component is operating normally.
    pub healthy: bool,
    /// Optional human-readable detail when unhealthy.
    pub detail: Option<String>,
}

// ---------------------------------------------------------------------------
// IPC message enum
// ---------------------------------------------------------------------------

/// Complete catalog of messages exchanged over the IPC channel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IpcMessage {
    // -- Device messages ---------------------------------------------------
    /// A new HID device was detected and connected.
    DeviceConnected {
        /// Unique device identifier.
        device_id: String,
        /// Human-readable device name.
        name: String,
        /// USB Vendor ID.
        vid: u16,
        /// USB Product ID.
        pid: u16,
    },

    /// A previously connected device was removed.
    DeviceDisconnected {
        /// Unique device identifier.
        device_id: String,
        /// Reason for disconnection.
        reason: String,
    },

    /// Raw input frame from a device.
    DeviceInput {
        /// Unique device identifier.
        device_id: String,
        /// Current axis values.
        axes: Vec<f64>,
        /// Current button states.
        buttons: Vec<bool>,
    },

    // -- Profile messages --------------------------------------------------
    /// A profile was activated.
    ProfileActivated {
        /// Profile name.
        name: String,
        /// Aircraft the profile is bound to, if any.
        aircraft: Option<String>,
    },

    /// A profile was deactivated.
    ProfileDeactivated {
        /// Profile name.
        name: String,
    },

    /// A profile failed to load or apply.
    ProfileError {
        /// Profile name.
        name: String,
        /// Error description.
        error: String,
    },

    // -- Service messages --------------------------------------------------
    /// Current service status snapshot.
    ServiceStatus {
        /// Overall service state.
        status: ServiceState,
        /// Seconds since the service started.
        uptime_secs: u64,
    },

    /// Aggregated health report for all components.
    HealthReport {
        /// Per-component status entries.
        components: Vec<ComponentStatus>,
    },

    // -- Sim messages ------------------------------------------------------
    /// A simulator was connected.
    SimConnected {
        /// Simulator type identifier (e.g. `"msfs"`, `"xplane"`).
        sim_type: String,
        /// Simulator version string.
        version: String,
    },

    /// A simulator was disconnected.
    SimDisconnected {
        /// Simulator type identifier.
        sim_type: String,
    },

    /// Telemetry snapshot from the active simulator.
    TelemetryUpdate {
        /// Altitude in feet MSL.
        altitude: f64,
        /// Indicated airspeed in knots.
        airspeed: f64,
        /// Magnetic heading in degrees.
        heading: f64,
    },

    // -- Adapter messages --------------------------------------------------
    /// A simulator adapter connected.
    AdapterConnected {
        /// Simulator identifier (e.g. `"msfs"`, `"xplane"`, `"dcs"`).
        sim_id: String,
        /// Human-readable display name for the adapter.
        display_name: String,
    },

    /// A simulator adapter disconnected.
    AdapterDisconnected {
        /// Simulator identifier.
        sim_id: String,
        /// Reason the adapter disconnected.
        reason: String,
    },

    /// A simulator adapter encountered an error.
    AdapterError {
        /// Simulator identifier.
        sim_id: String,
        /// Error description.
        error: String,
    },
}

// ---------------------------------------------------------------------------
// Methods
// ---------------------------------------------------------------------------

impl IpcMessage {
    /// Returns a static string identifying the message variant.
    pub fn message_type(&self) -> &str {
        match self {
            Self::DeviceConnected { .. } => "DeviceConnected",
            Self::DeviceDisconnected { .. } => "DeviceDisconnected",
            Self::DeviceInput { .. } => "DeviceInput",
            Self::ProfileActivated { .. } => "ProfileActivated",
            Self::ProfileDeactivated { .. } => "ProfileDeactivated",
            Self::ProfileError { .. } => "ProfileError",
            Self::ServiceStatus { .. } => "ServiceStatus",
            Self::HealthReport { .. } => "HealthReport",
            Self::SimConnected { .. } => "SimConnected",
            Self::SimDisconnected { .. } => "SimDisconnected",
            Self::TelemetryUpdate { .. } => "TelemetryUpdate",
            Self::AdapterConnected { .. } => "AdapterConnected",
            Self::AdapterDisconnected { .. } => "AdapterDisconnected",
            Self::AdapterError { .. } => "AdapterError",
        }
    }

    /// Serialize the message to a JSON string.
    pub fn to_json(&self) -> String {
        // serde_json::to_string on an enum with known types cannot fail
        // in practice, but we handle it defensively.
        serde_json::to_string(self).expect("IpcMessage serialization should not fail")
    }

    /// Deserialize an `IpcMessage` from a JSON string.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError::Json`] if the input is not valid JSON or does
    /// not match any known variant.
    pub fn from_json(json: &str) -> Result<Self, ParseError> {
        Ok(serde_json::from_str(json)?)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Helpers ------------------------------------------------------------

    fn sample_device_connected() -> IpcMessage {
        IpcMessage::DeviceConnected {
            device_id: "dev-001".into(),
            name: "Saitek X52 Pro".into(),
            vid: 0x06A3,
            pid: 0x0762,
        }
    }

    fn sample_service_status() -> IpcMessage {
        IpcMessage::ServiceStatus {
            status: ServiceState::Running,
            uptime_secs: 3600,
        }
    }

    fn sample_telemetry() -> IpcMessage {
        IpcMessage::TelemetryUpdate {
            altitude: 35_000.0,
            airspeed: 250.0,
            heading: 090.0,
        }
    }

    // -- 1. message_type returns correct variant name ----------------------
    #[test]
    fn message_type_device_connected() {
        assert_eq!(sample_device_connected().message_type(), "DeviceConnected");
    }

    #[test]
    fn message_type_device_disconnected() {
        let msg = IpcMessage::DeviceDisconnected {
            device_id: "d".into(),
            reason: "unplugged".into(),
        };
        assert_eq!(msg.message_type(), "DeviceDisconnected");
    }

    #[test]
    fn message_type_device_input() {
        let msg = IpcMessage::DeviceInput {
            device_id: "d".into(),
            axes: vec![0.5],
            buttons: vec![true],
        };
        assert_eq!(msg.message_type(), "DeviceInput");
    }

    #[test]
    fn message_type_profile_activated() {
        let msg = IpcMessage::ProfileActivated {
            name: "default".into(),
            aircraft: None,
        };
        assert_eq!(msg.message_type(), "ProfileActivated");
    }

    #[test]
    fn message_type_profile_deactivated() {
        let msg = IpcMessage::ProfileDeactivated { name: "old".into() };
        assert_eq!(msg.message_type(), "ProfileDeactivated");
    }

    #[test]
    fn message_type_profile_error() {
        let msg = IpcMessage::ProfileError {
            name: "bad".into(),
            error: "parse failed".into(),
        };
        assert_eq!(msg.message_type(), "ProfileError");
    }

    #[test]
    fn message_type_service_status() {
        assert_eq!(sample_service_status().message_type(), "ServiceStatus");
    }

    #[test]
    fn message_type_health_report() {
        let msg = IpcMessage::HealthReport { components: vec![] };
        assert_eq!(msg.message_type(), "HealthReport");
    }

    #[test]
    fn message_type_sim_connected() {
        let msg = IpcMessage::SimConnected {
            sim_type: "msfs".into(),
            version: "2024".into(),
        };
        assert_eq!(msg.message_type(), "SimConnected");
    }

    #[test]
    fn message_type_sim_disconnected() {
        let msg = IpcMessage::SimDisconnected {
            sim_type: "xplane".into(),
        };
        assert_eq!(msg.message_type(), "SimDisconnected");
    }

    #[test]
    fn message_type_telemetry_update() {
        assert_eq!(sample_telemetry().message_type(), "TelemetryUpdate");
    }

    // -- 2. JSON round-trip ------------------------------------------------
    #[test]
    fn json_roundtrip_device_connected() {
        let msg = sample_device_connected();
        let json = msg.to_json();
        let restored = IpcMessage::from_json(&json).unwrap();
        assert_eq!(msg, restored);
    }

    #[test]
    fn json_roundtrip_service_status() {
        let msg = sample_service_status();
        let json = msg.to_json();
        let restored = IpcMessage::from_json(&json).unwrap();
        assert_eq!(msg, restored);
    }

    #[test]
    fn json_roundtrip_telemetry() {
        let msg = sample_telemetry();
        let json = msg.to_json();
        let restored = IpcMessage::from_json(&json).unwrap();
        assert_eq!(msg, restored);
    }

    #[test]
    fn json_roundtrip_health_report() {
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
                    detail: Some("envelope exceeded".into()),
                },
            ],
        };
        let json = msg.to_json();
        let restored = IpcMessage::from_json(&json).unwrap();
        assert_eq!(msg, restored);
    }

    #[test]
    fn json_roundtrip_device_input() {
        let msg = IpcMessage::DeviceInput {
            device_id: "stick-1".into(),
            axes: vec![0.0, 0.5, -1.0],
            buttons: vec![false, true, false],
        };
        let json = msg.to_json();
        let restored = IpcMessage::from_json(&json).unwrap();
        assert_eq!(msg, restored);
    }

    // -- 3. from_json error handling ---------------------------------------
    #[test]
    fn from_json_invalid_json_returns_error() {
        let result = IpcMessage::from_json("not json");
        assert!(result.is_err());
    }

    #[test]
    fn from_json_unknown_type_returns_error() {
        let result = IpcMessage::from_json(r#"{"type":"UnknownVariant"}"#);
        assert!(result.is_err());
    }

    #[test]
    fn from_json_empty_string_returns_error() {
        let result = IpcMessage::from_json("");
        assert!(result.is_err());
    }

    // -- 4. to_json produces valid JSON ------------------------------------
    #[test]
    fn to_json_contains_type_tag() {
        let json = sample_device_connected().to_json();
        assert!(
            json.contains(r#""type":"DeviceConnected""#),
            "JSON should contain type tag: {json}"
        );
    }

    // -- 5. ServiceState serialization -------------------------------------
    #[test]
    fn service_state_all_variants_roundtrip() {
        for state in [
            ServiceState::Starting,
            ServiceState::Running,
            ServiceState::Degraded,
            ServiceState::Stopping,
            ServiceState::Stopped,
        ] {
            let msg = IpcMessage::ServiceStatus {
                status: state,
                uptime_secs: 0,
            };
            let json = msg.to_json();
            let restored = IpcMessage::from_json(&json).unwrap();
            assert_eq!(msg, restored);
        }
    }

    // -- 6. Profile with aircraft field ------------------------------------
    #[test]
    fn profile_activated_with_aircraft() {
        let msg = IpcMessage::ProfileActivated {
            name: "combat".into(),
            aircraft: Some("F-16C".into()),
        };
        let json = msg.to_json();
        assert!(json.contains("F-16C"));
        let restored = IpcMessage::from_json(&json).unwrap();
        assert_eq!(msg, restored);
    }

    #[test]
    fn profile_activated_without_aircraft() {
        let msg = IpcMessage::ProfileActivated {
            name: "generic".into(),
            aircraft: None,
        };
        let json = msg.to_json();
        let restored = IpcMessage::from_json(&json).unwrap();
        assert_eq!(msg, restored);
    }

    // -- 7. Adapter message variants --------------------------------------
    #[test]
    fn message_type_adapter_connected() {
        let msg = IpcMessage::AdapterConnected {
            sim_id: "msfs".into(),
            display_name: "MSFS 2024".into(),
        };
        assert_eq!(msg.message_type(), "AdapterConnected");
    }

    #[test]
    fn message_type_adapter_disconnected() {
        let msg = IpcMessage::AdapterDisconnected {
            sim_id: "xplane".into(),
            reason: "sim exited".into(),
        };
        assert_eq!(msg.message_type(), "AdapterDisconnected");
    }

    #[test]
    fn message_type_adapter_error() {
        let msg = IpcMessage::AdapterError {
            sim_id: "dcs".into(),
            error: "export.lua not found".into(),
        };
        assert_eq!(msg.message_type(), "AdapterError");
    }

    #[test]
    fn json_roundtrip_adapter_connected() {
        let msg = IpcMessage::AdapterConnected {
            sim_id: "msfs".into(),
            display_name: "MSFS 2024".into(),
        };
        let json = msg.to_json();
        let restored = IpcMessage::from_json(&json).unwrap();
        assert_eq!(msg, restored);
    }

    #[test]
    fn json_roundtrip_adapter_disconnected() {
        let msg = IpcMessage::AdapterDisconnected {
            sim_id: "xplane".into(),
            reason: "sim crashed".into(),
        };
        let json = msg.to_json();
        let restored = IpcMessage::from_json(&json).unwrap();
        assert_eq!(msg, restored);
    }

    #[test]
    fn json_roundtrip_adapter_error() {
        let msg = IpcMessage::AdapterError {
            sim_id: "dcs".into(),
            error: "connection reset".into(),
        };
        let json = msg.to_json();
        let restored = IpcMessage::from_json(&json).unwrap();
        assert_eq!(msg, restored);
    }
}
