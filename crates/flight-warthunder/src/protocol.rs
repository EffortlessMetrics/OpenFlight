// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! War Thunder `/indicators` JSON protocol types.

use serde::{Deserialize, Serialize};

/// Parsed response from `GET /state`.
///
/// The `/state` endpoint uses comma-unit key names (e.g. `"AoA, deg"`) and
/// dot-notation for engine channels (e.g. `"engine0.rpm"`).  All fields are
/// optional because the endpoint may omit values outside an active flight.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WtState {
    /// Whether telemetry is valid (player is in-flight).
    pub valid: Option<bool>,

    /// Aircraft display name.
    pub airframe: Option<String>,

    /// Angle of attack in degrees.
    #[serde(rename = "AoA, deg")]
    pub aoa_deg: Option<f32>,

    /// Angle of sideslip in degrees.
    #[serde(rename = "AoS, deg")]
    pub aos_deg: Option<f32>,

    /// True airspeed in m/s.
    #[serde(rename = "speed, m/s")]
    pub speed_mps: Option<f32>,

    /// Mach number.
    #[serde(rename = "Mach")]
    pub mach: Option<f32>,

    /// Normal (vertical) load factor in g.
    #[serde(rename = "Ny")]
    pub ny: Option<f32>,

    /// Longitudinal load factor in g.
    #[serde(rename = "Nx")]
    pub nx: Option<f32>,

    /// Lateral load factor in g.
    #[serde(rename = "Nz")]
    pub nz: Option<f32>,

    /// Engine 0 raw RPM.
    #[serde(rename = "engine0.rpm")]
    pub engine0_rpm: Option<f32>,

    /// Engine 1 raw RPM.
    #[serde(rename = "engine1.rpm")]
    pub engine1_rpm: Option<f32>,

    /// Engine 2 raw RPM.
    #[serde(rename = "engine2.rpm")]
    pub engine2_rpm: Option<f32>,

    /// Engine 3 raw RPM.
    #[serde(rename = "engine3.rpm")]
    pub engine3_rpm: Option<f32>,
}

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
mod state_tests {
    use super::*;

    #[test]
    fn deserialises_full_state() {
        let raw = r#"{
            "valid": true,
            "airframe": "P-51D-20NA",
            "AoA, deg": 3.5,
            "AoS, deg": 0.5,
            "speed, m/s": 95.0,
            "Mach": 0.28,
            "Ny": 1.1,
            "Nx": 0.05,
            "Nz": 0.0,
            "engine0.rpm": 2400.0
        }"#;
        let state: WtState = serde_json::from_str(raw).expect("should deserialise");
        assert_eq!(state.valid, Some(true));
        assert_eq!(state.airframe.as_deref(), Some("P-51D-20NA"));
        assert!((state.aoa_deg.unwrap() - 3.5).abs() < 0.01);
        assert!((state.aos_deg.unwrap() - 0.5).abs() < 0.01);
        assert!((state.speed_mps.unwrap() - 95.0).abs() < 0.01);
        assert!((state.mach.unwrap() - 0.28).abs() < 0.01);
        assert!((state.engine0_rpm.unwrap() - 2400.0).abs() < 0.01);
    }

    #[test]
    fn state_empty_object_all_none() {
        let state: WtState = serde_json::from_str("{}").expect("should deserialise");
        assert!(state.aoa_deg.is_none());
        assert!(state.mach.is_none());
        assert!(state.engine0_rpm.is_none());
        assert!(state.ny.is_none());
    }

    #[test]
    fn state_missing_aoa_is_none() {
        let raw = r#"{"valid": true, "Mach": 0.5}"#;
        let state: WtState = serde_json::from_str(raw).expect("should deserialise");
        assert!(state.aoa_deg.is_none());
        assert!((state.mach.unwrap() - 0.5).abs() < 0.01);
    }

    #[test]
    fn state_multi_engine_rpms() {
        let raw = r#"{"engine0.rpm": 1800.0, "engine1.rpm": 1750.0, "engine2.rpm": 0.0, "engine3.rpm": 1900.0}"#;
        let state: WtState = serde_json::from_str(raw).expect("should deserialise");
        assert!((state.engine0_rpm.unwrap() - 1800.0).abs() < 0.01);
        assert!((state.engine1_rpm.unwrap() - 1750.0).abs() < 0.01);
        assert_eq!(state.engine2_rpm, Some(0.0));
        assert!((state.engine3_rpm.unwrap() - 1900.0).abs() < 0.01);
    }

    #[test]
    fn state_g_load_components_deserialise() {
        let raw = r#"{"Ny": 3.5, "Nx": 0.2, "Nz": -0.3}"#;
        let state: WtState = serde_json::from_str(raw).expect("should deserialise");
        assert!((state.ny.unwrap() - 3.5).abs() < 0.01);
        assert!((state.nx.unwrap() - 0.2).abs() < 0.01);
        assert!((state.nz.unwrap() - (-0.3)).abs() < 0.01);
    }

    #[test]
    fn state_malformed_json_returns_error() {
        let result: Result<WtState, _> = serde_json::from_str("not valid json");
        assert!(result.is_err());
    }

    #[test]
    fn state_round_trip_serialise_deserialise() {
        let original = WtState {
            valid: Some(true),
            airframe: Some("Bf 109 G-6".to_string()),
            aoa_deg: Some(4.0),
            mach: Some(0.6),
            ny: Some(2.5),
            ..Default::default()
        };
        let json = serde_json::to_string(&original).expect("should serialise");
        let back: WtState = serde_json::from_str(&json).expect("should deserialise");
        assert_eq!(back.valid, original.valid);
        assert_eq!(back.airframe, original.airframe);
        assert!((back.aoa_deg.unwrap() - 4.0).abs() < 0.01);
        assert!((back.mach.unwrap() - 0.6).abs() < 0.01);
    }
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

    #[test]
    fn empty_json_object_deserialises_to_all_none() {
        let ind: WtIndicators = serde_json::from_str("{}").expect("should deserialise");
        assert!(ind.valid.is_none());
        assert!(ind.airframe.is_none());
        assert!(ind.ias_kmh.is_none());
        assert!(ind.g_load.is_none());
    }

    #[test]
    fn renamed_fields_g_load_and_ias() {
        let raw = r#"{"gLoad": 2.5, "IAS km/h": 300.0}"#;
        let ind: WtIndicators = serde_json::from_str(raw).expect("should deserialise");
        assert!((ind.g_load.unwrap() - 2.5).abs() < 0.01);
        assert!((ind.ias_kmh.unwrap() - 300.0).abs() < 0.01);
    }

    #[test]
    fn round_trip_serialise_deserialise() {
        let original = WtIndicators {
            valid: Some(true),
            airframe: Some("FW-190".to_string()),
            ias_kmh: Some(500.0),
            heading: Some(90.0),
            ..Default::default()
        };
        let json = serde_json::to_string(&original).expect("should serialise");
        let back: WtIndicators = serde_json::from_str(&json).expect("should deserialise");
        assert_eq!(back.valid, original.valid);
        assert_eq!(back.airframe, original.airframe);
        assert!((back.ias_kmh.unwrap() - 500.0).abs() < 0.01);
    }

    #[test]
    fn malformed_json_returns_serde_error() {
        let result: Result<WtIndicators, _> = serde_json::from_str("not valid json at all!!");
        assert!(result.is_err(), "malformed JSON should not parse");
    }

    #[test]
    fn wrong_field_type_returns_serde_error() {
        // "IAS km/h" expects f32 but receives a string
        let result: Result<WtIndicators, _> = serde_json::from_str(r#"{"IAS km/h": "fast_speed"}"#);
        assert!(result.is_err(), "wrong-typed field should not parse");
    }

    #[test]
    fn array_instead_of_object_returns_serde_error() {
        let result: Result<WtIndicators, _> = serde_json::from_str(r#"[1, 2, 3]"#);
        assert!(
            result.is_err(),
            "JSON array should not parse as WtIndicators"
        );
    }
}
