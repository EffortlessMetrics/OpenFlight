// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Typed IPC message definitions with versioned binary encoding.
//!
//! Each message type carries a version byte prefix for forward compatibility.
//! Use [`encode`](AxisUpdate::encode) to serialize and
//! [`decode`](AxisUpdate::decode) to deserialize.
//!
//! # Wire format
//!
//! ```text
//! [version: u8][json payload ...]
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Current encoding version.
const CURRENT_VERSION: u8 = 1;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors from message encoding / decoding.
#[derive(Debug, Error)]
pub enum MessageError {
    /// The payload is empty (no version byte).
    #[error("empty payload")]
    EmptyPayload,

    /// The version byte is not supported by this build.
    #[error("unsupported message version: {version}")]
    UnsupportedVersion {
        /// The version byte that was encountered.
        version: u8,
    },

    /// JSON deserialization failed.
    #[error("decode error: {0}")]
    Decode(#[from] serde_json::Error),
}

// ---------------------------------------------------------------------------
// Message types
// ---------------------------------------------------------------------------

/// Real-time axis value update from input processing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AxisUpdate {
    /// Device that produced this update.
    pub device_id: String,
    /// Axis index on the device.
    pub axis_index: u8,
    /// Processed axis value in \[-1.0, 1.0\].
    pub value: f64,
    /// Timestamp in microseconds since epoch.
    pub timestamp_us: u64,
}

/// A hardware device event (connect, disconnect, error).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceEvent {
    /// Unique device identifier.
    pub device_id: String,
    /// Event kind: `"connected"`, `"disconnected"`, or `"error"`.
    pub kind: String,
    /// Human-readable detail.
    pub detail: String,
}

/// Notification that the active profile has changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileChange {
    /// Name of the newly active profile.
    pub profile_name: String,
    /// Aircraft binding, if any.
    pub aircraft: Option<String>,
    /// `true` if this was an automatic switch (e.g. aircraft detection).
    pub auto_switch: bool,
}

/// System health snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthStatus {
    /// Overall healthy flag.
    pub healthy: bool,
    /// Per-component status.
    pub components: Vec<ComponentHealth>,
    /// Uptime in seconds.
    pub uptime_secs: u64,
}

/// Health of a single component.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentHealth {
    /// Component name.
    pub name: String,
    /// Whether this component is healthy.
    pub healthy: bool,
    /// Optional detail message.
    pub detail: Option<String>,
}

/// Performance metrics snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    /// Tick rate in Hz.
    pub tick_rate_hz: f64,
    /// p99 jitter in milliseconds.
    pub jitter_p99_ms: f64,
    /// Number of connected devices.
    pub device_count: u32,
    /// Number of active subscriptions.
    pub subscription_count: u32,
    /// Timestamp in microseconds since epoch.
    pub timestamp_us: u64,
}

// ---------------------------------------------------------------------------
// Versioned encode / decode
// ---------------------------------------------------------------------------

macro_rules! impl_versioned_codec {
    ($ty:ty) => {
        impl $ty {
            /// Encode this message into a versioned byte buffer.
            ///
            /// Layout: `[version_byte][json_payload...]`
            pub fn encode(&self) -> Vec<u8> {
                let json = serde_json::to_vec(self).expect("serialization should not fail");
                let mut buf = Vec::with_capacity(1 + json.len());
                buf.push(CURRENT_VERSION);
                buf.extend_from_slice(&json);
                buf
            }

            /// Decode a message from a versioned byte buffer.
            ///
            /// # Errors
            ///
            /// Returns [`MessageError`] if the buffer is empty, the version
            /// is unsupported, or the JSON payload is invalid.
            pub fn decode(bytes: &[u8]) -> Result<Self, MessageError> {
                if bytes.is_empty() {
                    return Err(MessageError::EmptyPayload);
                }
                let version = bytes[0];
                if version != CURRENT_VERSION {
                    return Err(MessageError::UnsupportedVersion { version });
                }
                Ok(serde_json::from_slice(&bytes[1..])?)
            }
        }
    };
}

impl_versioned_codec!(AxisUpdate);
impl_versioned_codec!(DeviceEvent);
impl_versioned_codec!(ProfileChange);
impl_versioned_codec!(HealthStatus);
impl_versioned_codec!(MetricsSnapshot);

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- helpers --

    fn sample_axis_update() -> AxisUpdate {
        AxisUpdate {
            device_id: "stick-1".into(),
            axis_index: 2,
            value: 0.75,
            timestamp_us: 1_000_000,
        }
    }

    fn sample_device_event() -> DeviceEvent {
        DeviceEvent {
            device_id: "throttle-1".into(),
            kind: "connected".into(),
            detail: "Saitek X52 Pro detected".into(),
        }
    }

    fn sample_profile_change() -> ProfileChange {
        ProfileChange {
            profile_name: "combat".into(),
            aircraft: Some("F-16C".into()),
            auto_switch: true,
        }
    }

    fn sample_health_status() -> HealthStatus {
        HealthStatus {
            healthy: true,
            components: vec![
                ComponentHealth {
                    name: "axis-engine".into(),
                    healthy: true,
                    detail: None,
                },
                ComponentHealth {
                    name: "ffb-engine".into(),
                    healthy: false,
                    detail: Some("envelope exceeded".into()),
                },
            ],
            uptime_secs: 3600,
        }
    }

    fn sample_metrics_snapshot() -> MetricsSnapshot {
        MetricsSnapshot {
            tick_rate_hz: 250.0,
            jitter_p99_ms: 0.3,
            device_count: 4,
            subscription_count: 12,
            timestamp_us: 2_000_000,
        }
    }

    // 1. AxisUpdate round-trip
    #[test]
    fn axis_update_roundtrip() {
        let msg = sample_axis_update();
        let bytes = msg.encode();
        let decoded = AxisUpdate::decode(&bytes).unwrap();
        assert_eq!(msg, decoded);
    }

    // 2. DeviceEvent round-trip
    #[test]
    fn device_event_roundtrip() {
        let msg = sample_device_event();
        let bytes = msg.encode();
        let decoded = DeviceEvent::decode(&bytes).unwrap();
        assert_eq!(msg, decoded);
    }

    // 3. ProfileChange round-trip
    #[test]
    fn profile_change_roundtrip() {
        let msg = sample_profile_change();
        let bytes = msg.encode();
        let decoded = ProfileChange::decode(&bytes).unwrap();
        assert_eq!(msg, decoded);
    }

    // 4. HealthStatus round-trip
    #[test]
    fn health_status_roundtrip() {
        let msg = sample_health_status();
        let bytes = msg.encode();
        let decoded = HealthStatus::decode(&bytes).unwrap();
        assert_eq!(msg, decoded);
    }

    // 5. MetricsSnapshot round-trip
    #[test]
    fn metrics_snapshot_roundtrip() {
        let msg = sample_metrics_snapshot();
        let bytes = msg.encode();
        let decoded = MetricsSnapshot::decode(&bytes).unwrap();
        assert_eq!(msg, decoded);
    }

    // 6. Version byte is present
    #[test]
    fn encode_starts_with_version() {
        let bytes = sample_axis_update().encode();
        assert_eq!(bytes[0], CURRENT_VERSION);
    }

    // 7. Empty payload error
    #[test]
    fn decode_empty_payload() {
        let err = AxisUpdate::decode(&[]).unwrap_err();
        assert!(matches!(err, MessageError::EmptyPayload));
    }

    // 8. Unsupported version error
    #[test]
    fn decode_unsupported_version() {
        let mut bytes = sample_device_event().encode();
        bytes[0] = 99;
        let err = DeviceEvent::decode(&bytes).unwrap_err();
        assert!(matches!(
            err,
            MessageError::UnsupportedVersion { version: 99 }
        ));
    }

    // 9. Invalid JSON after version byte
    #[test]
    fn decode_invalid_json() {
        let bytes = vec![CURRENT_VERSION, b'{', b'x'];
        let err = ProfileChange::decode(&bytes).unwrap_err();
        assert!(matches!(err, MessageError::Decode(_)));
    }

    // 10. ProfileChange without aircraft
    #[test]
    fn profile_change_no_aircraft() {
        let msg = ProfileChange {
            profile_name: "default".into(),
            aircraft: None,
            auto_switch: false,
        };
        let bytes = msg.encode();
        let decoded = ProfileChange::decode(&bytes).unwrap();
        assert_eq!(msg, decoded);
    }

    // 11. HealthStatus with no components
    #[test]
    fn health_status_empty_components() {
        let msg = HealthStatus {
            healthy: true,
            components: vec![],
            uptime_secs: 0,
        };
        let bytes = msg.encode();
        let decoded = HealthStatus::decode(&bytes).unwrap();
        assert_eq!(msg, decoded);
    }

    // 12. AxisUpdate boundary values
    #[test]
    fn axis_update_boundary_values() {
        let msg = AxisUpdate {
            device_id: String::new(),
            axis_index: 255,
            value: -1.0,
            timestamp_us: u64::MAX,
        };
        let bytes = msg.encode();
        let decoded = AxisUpdate::decode(&bytes).unwrap();
        assert_eq!(msg, decoded);
    }

    // 13. Send + Sync assertions
    #[test]
    fn types_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<AxisUpdate>();
        assert_send_sync::<DeviceEvent>();
        assert_send_sync::<ProfileChange>();
        assert_send_sync::<HealthStatus>();
        assert_send_sync::<MetricsSnapshot>();
    }
}
