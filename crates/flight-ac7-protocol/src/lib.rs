// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! AC7 telemetry wire format definitions.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable schema identifier for AC7 telemetry packets.
pub const AC7_TELEMETRY_SCHEMA_V1: &str = "flight.ac7.telemetry/1";

/// Wire packet emitted by an AC7 bridge plugin (for example UE4SS/Lua).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Ac7TelemetryPacket {
    /// Schema/version discriminator.
    #[serde(default = "default_schema")]
    pub schema: String,
    /// Source timestamp from plugin (milliseconds).
    #[serde(default)]
    pub timestamp_ms: u64,
    /// Aircraft label from source bridge.
    #[serde(default)]
    pub aircraft: String,
    /// Optional mission identifier.
    #[serde(default)]
    pub mission: Option<String>,
    /// Aircraft state values.
    #[serde(default)]
    pub state: Ac7State,
    /// Player control surface and throttle inputs.
    #[serde(default)]
    pub controls: Ac7Controls,
}

impl Default for Ac7TelemetryPacket {
    fn default() -> Self {
        Self {
            schema: default_schema(),
            timestamp_ms: 0,
            aircraft: String::new(),
            mission: None,
            state: Ac7State::default(),
            controls: Ac7Controls::default(),
        }
    }
}

/// State fields extracted from AC7.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Ac7State {
    /// Altitude above sea level (meters).
    #[serde(default)]
    pub altitude_m: Option<f32>,
    /// Airspeed (meters per second).
    #[serde(default)]
    pub speed_mps: Option<f32>,
    /// Ground speed (meters per second).
    #[serde(default)]
    pub ground_speed_mps: Option<f32>,
    /// Vertical speed (meters per second).
    #[serde(default)]
    pub vertical_speed_mps: Option<f32>,
    /// Heading (degrees).
    #[serde(default)]
    pub heading_deg: Option<f32>,
    /// Pitch (degrees).
    #[serde(default)]
    pub pitch_deg: Option<f32>,
    /// Roll (degrees).
    #[serde(default)]
    pub roll_deg: Option<f32>,
    /// Instantaneous load factor in g.
    #[serde(default)]
    pub g_force: Option<f32>,
    /// Player health normalized to 0.0..=1.0.
    #[serde(default)]
    pub health_norm: Option<f32>,
}

/// Input state fields extracted from AC7.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Ac7Controls {
    /// Pitch input normalized to -1.0..=1.0.
    #[serde(default)]
    pub pitch: Option<f32>,
    /// Roll input normalized to -1.0..=1.0.
    #[serde(default)]
    pub roll: Option<f32>,
    /// Yaw input normalized to -1.0..=1.0.
    #[serde(default)]
    pub yaw: Option<f32>,
    /// Throttle input normalized to 0.0..=1.0.
    #[serde(default)]
    pub throttle: Option<f32>,
    /// Brake input normalized to 0.0..=1.0.
    #[serde(default)]
    pub brake: Option<f32>,
}

/// AC7 protocol parse/validation errors.
#[derive(Debug, Error)]
pub enum Ac7ProtocolError {
    #[error("invalid telemetry JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("unsupported schema: {schema}")]
    UnsupportedSchema { schema: String },
    #[error("field out of range: {field}={value} expected [{min}, {max}]")]
    OutOfRange {
        field: &'static str,
        value: f32,
        min: f32,
        max: f32,
    },
}

impl Ac7TelemetryPacket {
    /// Parse and validate a telemetry packet from JSON bytes.
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self, Ac7ProtocolError> {
        let packet = serde_json::from_slice::<Self>(bytes)?;
        packet.validate()?;
        Ok(packet)
    }

    /// Parse and validate a telemetry packet from JSON text.
    pub fn from_json_str(payload: &str) -> Result<Self, Ac7ProtocolError> {
        Self::from_json_slice(payload.as_bytes())
    }

    /// Serialize packet as JSON bytes.
    pub fn to_json_vec(&self) -> Result<Vec<u8>, Ac7ProtocolError> {
        Ok(serde_json::to_vec(self)?)
    }

    /// Validate wire schema and field ranges.
    pub fn validate(&self) -> Result<(), Ac7ProtocolError> {
        if self.schema != AC7_TELEMETRY_SCHEMA_V1 {
            return Err(Ac7ProtocolError::UnsupportedSchema {
                schema: self.schema.clone(),
            });
        }

        validate_optional_range(
            self.state.altitude_m,
            -2_000.0,
            100_000.0,
            "state.altitude_m",
        )?;
        validate_optional_range(self.state.speed_mps, 0.0, 2_500.0, "state.speed_mps")?;
        validate_optional_range(
            self.state.ground_speed_mps,
            0.0,
            2_500.0,
            "state.ground_speed_mps",
        )?;
        validate_optional_range(
            self.state.vertical_speed_mps,
            -500.0,
            500.0,
            "state.vertical_speed_mps",
        )?;
        validate_optional_range(self.state.heading_deg, -360.0, 360.0, "state.heading_deg")?;
        validate_optional_range(self.state.pitch_deg, -180.0, 180.0, "state.pitch_deg")?;
        validate_optional_range(self.state.roll_deg, -180.0, 180.0, "state.roll_deg")?;
        validate_optional_range(self.state.g_force, -20.0, 20.0, "state.g_force")?;
        validate_optional_range(self.state.health_norm, 0.0, 1.0, "state.health_norm")?;

        validate_optional_range(self.controls.pitch, -1.0, 1.0, "controls.pitch")?;
        validate_optional_range(self.controls.roll, -1.0, 1.0, "controls.roll")?;
        validate_optional_range(self.controls.yaw, -1.0, 1.0, "controls.yaw")?;
        validate_optional_range(self.controls.throttle, 0.0, 1.0, "controls.throttle")?;
        validate_optional_range(self.controls.brake, 0.0, 1.0, "controls.brake")?;

        Ok(())
    }

    /// Returns a non-empty aircraft label for downstream consumers.
    pub fn aircraft_label(&self) -> &str {
        let trimmed = self.aircraft.trim();
        if trimmed.is_empty() { "AC7" } else { trimmed }
    }
}

fn default_schema() -> String {
    AC7_TELEMETRY_SCHEMA_V1.to_string()
}

fn validate_optional_range(
    value: Option<f32>,
    min: f32,
    max: f32,
    field: &'static str,
) -> Result<(), Ac7ProtocolError> {
    if let Some(v) = value
        && !(min..=max).contains(&v)
    {
        return Err(Ac7ProtocolError::OutOfRange {
            field,
            value: v,
            min,
            max,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn parses_valid_packet() {
        let payload = r#"{
            "schema":"flight.ac7.telemetry/1",
            "timestamp_ms":1234,
            "aircraft":"F-15C",
            "state":{"altitude_m":2500.0,"speed_mps":220.0,"heading_deg":90.0},
            "controls":{"pitch":0.2,"roll":-0.1,"yaw":0.0,"throttle":0.8}
        }"#;

        let packet = Ac7TelemetryPacket::from_json_str(payload).unwrap();
        assert_eq!(packet.aircraft, "F-15C");
        assert_eq!(packet.state.altitude_m, Some(2500.0));
        assert_eq!(packet.controls.throttle, Some(0.8));
    }

    #[test]
    fn rejects_out_of_range_control_value() {
        let payload = r#"{
            "schema":"flight.ac7.telemetry/1",
            "controls":{"throttle":1.5}
        }"#;

        let err = Ac7TelemetryPacket::from_json_str(payload).unwrap_err();
        assert!(matches!(
            err,
            Ac7ProtocolError::OutOfRange {
                field: "controls.throttle",
                ..
            }
        ));
    }

    #[test]
    fn defaults_schema_and_label() {
        let payload = r#"{"state":{"speed_mps":100.0}}"#;
        let packet = Ac7TelemetryPacket::from_json_str(payload).unwrap();
        assert_eq!(packet.schema, AC7_TELEMETRY_SCHEMA_V1);
        assert_eq!(packet.aircraft_label(), "AC7");
    }

    proptest! {
        #[test]
        fn property_pitch_control_is_bounded(v in -1.0f32..1.0f32) {
            let packet = Ac7TelemetryPacket {
                controls: Ac7Controls {
                    pitch: Some(v),
                    ..Default::default()
                },
                ..Default::default()
            };
            prop_assert!(packet.validate().is_ok());
        }
    }

    #[test]
    fn rejects_wrong_schema() {
        let payload = r#"{"schema":"flight.ac7.telemetry/0","aircraft":"F-15C"}"#;
        let err = Ac7TelemetryPacket::from_json_str(payload).unwrap_err();
        assert!(matches!(err, Ac7ProtocolError::UnsupportedSchema { .. }));
    }

    #[test]
    fn rejects_invalid_json() {
        let err = Ac7TelemetryPacket::from_json_str("{not json}").unwrap_err();
        assert!(matches!(err, Ac7ProtocolError::InvalidJson(_)));
    }

    #[test]
    fn rejects_out_of_range_altitude() {
        let payload = r#"{"schema":"flight.ac7.telemetry/1","state":{"altitude_m":999999.0}}"#;
        let err = Ac7TelemetryPacket::from_json_str(payload).unwrap_err();
        assert!(matches!(err, Ac7ProtocolError::OutOfRange { field: "state.altitude_m", .. }));
    }

    #[test]
    fn rejects_negative_speed() {
        let payload = r#"{"schema":"flight.ac7.telemetry/1","state":{"speed_mps":-1.0}}"#;
        let err = Ac7TelemetryPacket::from_json_str(payload).unwrap_err();
        assert!(matches!(err, Ac7ProtocolError::OutOfRange { field: "state.speed_mps", .. }));
    }

    #[test]
    fn aircraft_label_trims_whitespace() {
        let packet = Ac7TelemetryPacket {
            aircraft: "  Su-33  ".to_string(),
            ..Default::default()
        };
        assert_eq!(packet.aircraft_label(), "Su-33");
    }

    #[test]
    fn json_round_trip() {
        let original = Ac7TelemetryPacket {
            schema: AC7_TELEMETRY_SCHEMA_V1.to_string(),
            timestamp_ms: 9876,
            aircraft: "XFA-27".to_string(),
            mission: Some("Mission01".to_string()),
            state: Ac7State {
                altitude_m: Some(5000.0),
                speed_mps: Some(300.0),
                heading_deg: Some(180.0),
                pitch_deg: Some(10.0),
                roll_deg: Some(-5.0),
                g_force: Some(2.5),
                health_norm: Some(0.9),
                ..Default::default()
            },
            controls: Ac7Controls {
                pitch: Some(-0.3),
                roll: Some(0.5),
                throttle: Some(0.75),
                ..Default::default()
            },
        };
        let bytes = original.to_json_vec().unwrap();
        let restored = Ac7TelemetryPacket::from_json_slice(&bytes).unwrap();
        assert_eq!(restored, original);
    }

    proptest! {
        #[test]
        fn property_all_bounded_controls_valid(
            pitch in -1.0f32..=1.0f32,
            roll in -1.0f32..=1.0f32,
            yaw in -1.0f32..=1.0f32,
            throttle in 0.0f32..=1.0f32,
            brake in 0.0f32..=1.0f32,
        ) {
            let packet = Ac7TelemetryPacket {
                controls: Ac7Controls {
                    pitch: Some(pitch),
                    roll: Some(roll),
                    yaw: Some(yaw),
                    throttle: Some(throttle),
                    brake: Some(brake),
                },
                ..Default::default()
            };
            prop_assert!(packet.validate().is_ok());
        }

        #[test]
        fn property_bounded_altitude_valid(alt_m in -2000.0f32..=100_000.0f32) {
            let packet = Ac7TelemetryPacket {
                state: Ac7State { altitude_m: Some(alt_m), ..Default::default() },
                ..Default::default()
            };
            prop_assert!(packet.validate().is_ok());
        }

        #[test]
        fn property_out_of_bounds_throttle_rejected(v in proptest::num::f32::NORMAL) {
            // Only test values strictly outside [0.0, 1.0]
            let outside = if v > 1.0 || v < 0.0 { v } else { v * 10.0 + 2.0 };
            if !(0.0..=1.0).contains(&outside) {
                let packet = Ac7TelemetryPacket {
                    controls: Ac7Controls {
                        throttle: Some(outside),
                        ..Default::default()
                    },
                    ..Default::default()
                };
                prop_assert!(packet.validate().is_err());
            }
        }
    }
}
