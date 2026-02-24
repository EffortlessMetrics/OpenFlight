// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! War Thunder `/indicators` JSON protocol types.

use serde::{Deserialize, Serialize};

/// Parsed response from `GET /indicators`.
///
/// All fields are optional because the endpoint may omit values
/// when the game is loading or the player is in a menu.
///
/// Field names match the War Thunder HTTP API (community-documented).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WtIndicators {
    /// Whether telemetry is valid (player is in-flight).
    pub valid: Option<bool>,

    /// Aircraft display name.
    pub airframe: Option<String>,

    /// Indicated airspeed in km/h.
    #[serde(rename = "IAS km/h")]
    pub ias_kmh: Option<f32>,

    /// True airspeed in km/h.
    #[serde(rename = "TAS km/h")]
    pub tas_kmh: Option<f32>,

    /// Geometric altitude in metres.
    pub altitude: Option<f32>,

    /// Magnetic heading in degrees (0–360).
    pub heading: Option<f32>,

    /// Pitch angle in degrees (positive = nose-up).
    pub pitch: Option<f32>,

    /// Bank/roll angle in degrees (positive = right wing down).
    pub roll: Option<f32>,

    /// Normal (vertical) G-force.
    #[serde(rename = "gLoad")]
    pub g_load: Option<f32>,

    /// Vertical speed in m/s (positive = climbing).
    #[serde(rename = "vertSpeed")]
    pub vert_speed: Option<f32>,

    /// Landing gear deployment ratio (0.0 = retracted, 1.0 = fully deployed).
    pub gear: Option<f32>,

    /// Flap deployment ratio (0.0 = retracted, 1.0 = fully deployed).
    pub flaps: Option<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialises_from_json() {
        let raw = r#"{
            "valid": true,
            "airframe": "Spitfire Mk.Vc",
            "IAS km/h": 360.5,
            "TAS km/h": 380.0,
            "altitude": 1500.0,
            "heading": 270.0,
            "pitch": 3.5,
            "roll": -5.0,
            "gLoad": 1.1,
            "vertSpeed": 1.5,
            "gear": 0.0,
            "flaps": 0.5
        }"#;

        let ind: WtIndicators = serde_json::from_str(raw).expect("should deserialise");
        assert_eq!(ind.valid, Some(true));
        assert_eq!(ind.airframe.as_deref(), Some("Spitfire Mk.Vc"));
        assert!((ind.ias_kmh.unwrap() - 360.5).abs() < 0.01);
        assert!((ind.flaps.unwrap() - 0.5).abs() < 0.01);
    }

    #[test]
    fn handles_missing_optional_fields() {
        let raw = r#"{"valid": false}"#;
        let ind: WtIndicators = serde_json::from_str(raw).expect("should deserialise");
        assert_eq!(ind.valid, Some(false));
        assert!(ind.ias_kmh.is_none());
    }
}
