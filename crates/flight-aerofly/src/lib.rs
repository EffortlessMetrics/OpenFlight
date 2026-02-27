// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Aerofly FS 2 / 4 simulator adapter for OpenFlight.
//!
//! Aerofly FS reads flight controls via DirectInput.  OpenFlight maps physical
//! HOTAS inputs through its axis pipeline and presents them as a virtual
//! controller via ViGEm or vJoy (see the `aerofly-fs` game manifest and the
//! `flight-virtual` crate).
//!
//! This crate provides two parsing paths for future telemetry work:
//!
//! 1. **Binary UDP** — a compact little-endian struct broadcast on UDP port
//!    [`AEROFLY_DEFAULT_PORT`] (stub format; not yet standardised by IPACS).
//! 2. **JSON** — a line-delimited JSON object matching the draft IPACS SDK
//!    telemetry schema.
//!
//! ## Binary frame layout
//!
//! | Offset | Size | Field          | Unit       |
//! |--------|------|----------------|------------|
//! | 0      | 4    | `magic`        | `"AFFS"`   |
//! | 4      | 4    | `pitch`        | degrees    |
//! | 8      | 4    | `roll`         | degrees    |
//! | 12     | 4    | `heading`      | degrees    |
//! | 16     | 4    | `airspeed`     | knots      |
//! | 20     | 4    | `altitude`     | feet       |
//! | 24     | 4    | `throttle_pos` | 0.0 – 1.0  |
//! | 28     | 1    | `gear_down`    | 0 / 1      |
//! | 29     | 4    | `flaps_ratio`  | 0.0 – 1.0  |
//!
//! Minimum valid frame size: [`MIN_FRAME_SIZE`] bytes.

use serde::{Deserialize, Serialize};
use thiserror::Error;

// Reserved for future real implementation; silences the unused-extern-crate lint.
#[allow(unused_extern_crates)]
extern crate flight_core;

/// Magic bytes at the start of every Aerofly binary UDP frame (`"AFFS"`).
pub const AEROFLY_MAGIC: u32 = 0x4146_4653;

/// Default UDP port for Aerofly FS telemetry (stub; not yet standardised).
pub const AEROFLY_DEFAULT_PORT: u16 = 49002;

/// Minimum valid binary frame size in bytes.
pub const MIN_FRAME_SIZE: usize = 33;

// ── Error type ────────────────────────────────────────────────────────────────

/// Errors produced by the Aerofly FS adapter.
#[derive(Debug, Error, PartialEq)]
pub enum AeroflyAdapterError {
    /// Frame is shorter than [`MIN_FRAME_SIZE`].
    #[error("frame too short: expected at least {MIN_FRAME_SIZE} bytes, got {found}")]
    FrameTooShort { found: usize },

    /// The magic number did not match [`AEROFLY_MAGIC`].
    #[error("bad magic: expected {AEROFLY_MAGIC:#010x}, got {found:#010x}")]
    BadMagic { found: u32 },

    /// A field could not be read at the given byte offset.
    #[error("failed to read field at offset {offset}")]
    ReadError { offset: usize },

    /// The JSON payload could not be parsed.
    #[error("JSON parse error: {0}")]
    JsonError(String),
}

// ── Domain types ──────────────────────────────────────────────────────────────

/// Snapshot of Aerofly FS flight state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AeroflyTelemetry {
    /// Pitch angle in degrees (positive = nose-up).
    pub pitch: f32,
    /// Roll / bank angle in degrees (positive = right-wing-down).
    pub roll: f32,
    /// Magnetic heading in degrees (0 – 360).
    pub heading: f32,
    /// Indicated airspeed in knots.
    pub airspeed: f32,
    /// Altitude in feet MSL.
    pub altitude: f32,
    /// Throttle lever position normalised to `0.0` (idle) – `1.0` (full).
    pub throttle_pos: f32,
    /// `true` when the landing gear is fully down and locked.
    pub gear_down: bool,
    /// Flap deployment ratio normalised to `0.0` (up) – `1.0` (full).
    pub flaps_ratio: f32,
}

impl Default for AeroflyTelemetry {
    fn default() -> Self {
        Self {
            pitch: 0.0,
            roll: 0.0,
            heading: 0.0,
            airspeed: 0.0,
            altitude: 0.0,
            throttle_pos: 0.0,
            gear_down: false,
            flaps_ratio: 0.0,
        }
    }
}

/// Aircraft types available in Aerofly FS 2 / 4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AeroflyAircraftType {
    /// Cessna 172 Skyhawk.
    Cessna172,
    /// Airbus A320.
    AirbusA320,
    /// Boeing 737.
    BoeingB737,
    /// Piper PA-28 Cherokee.
    PiperCherokee,
    /// Extra 330.
    Extra330,
    /// Aircraft name not recognised by this adapter.
    Unknown,
}

impl AeroflyAircraftType {
    /// Identify the aircraft type from a display-name substring.
    ///
    /// Matching is case-insensitive.
    pub fn from_name(name: &str) -> Self {
        let lower = name.to_lowercase();
        if lower.contains("cessna") || lower.contains("c172") {
            AeroflyAircraftType::Cessna172
        } else if lower.contains("a320") || lower.contains("airbus") {
            AeroflyAircraftType::AirbusA320
        } else if lower.contains("737") || lower.contains("boeing") {
            AeroflyAircraftType::BoeingB737
        } else if lower.contains("cherokee") || lower.contains("pa-28") || lower.contains("pa28") {
            AeroflyAircraftType::PiperCherokee
        } else if lower.contains("extra") || lower.contains("extra330") {
            AeroflyAircraftType::Extra330
        } else {
            AeroflyAircraftType::Unknown
        }
    }
}

// ── Adapter ───────────────────────────────────────────────────────────────────

/// Aerofly FS adapter.
///
/// In a real deployment, bind a UDP socket to [`AEROFLY_DEFAULT_PORT`] and
/// pass each received datagram to [`process_datagram`](Self::process_datagram).
pub struct AeroflyAdapter {
    /// UDP port the adapter listens on.
    pub port: u16,
    last_telemetry: Option<AeroflyTelemetry>,
}

impl AeroflyAdapter {
    /// Create a new adapter on the default Aerofly telemetry port
    /// ([`AEROFLY_DEFAULT_PORT`]).
    pub fn new() -> Self {
        tracing::info!(port = AEROFLY_DEFAULT_PORT, "Aerofly adapter created");
        Self {
            port: AEROFLY_DEFAULT_PORT,
            last_telemetry: None,
        }
    }

    /// Create a new adapter on a custom UDP `port`.
    pub fn with_port(port: u16) -> Self {
        tracing::info!(port, "Aerofly adapter created with custom port");
        Self {
            port,
            last_telemetry: None,
        }
    }

    /// Decode a raw binary UDP datagram and cache the result.
    pub fn process_datagram(
        &mut self,
        data: &[u8],
    ) -> Result<AeroflyTelemetry, AeroflyAdapterError> {
        tracing::debug!(len = data.len(), "processing Aerofly UDP datagram");
        let telemetry = parse_telemetry(data)?;
        self.last_telemetry = Some(telemetry.clone());
        Ok(telemetry)
    }

    /// Decode a JSON telemetry string and cache the result.
    pub fn process_json(&mut self, json: &str) -> Result<AeroflyTelemetry, AeroflyAdapterError> {
        tracing::debug!("processing Aerofly JSON telemetry");
        let telemetry = parse_json_telemetry(json)?;
        self.last_telemetry = Some(telemetry.clone());
        Ok(telemetry)
    }

    /// Return the most recently decoded telemetry snapshot, if any.
    pub fn last_telemetry(&self) -> Option<&AeroflyTelemetry> {
        self.last_telemetry.as_ref()
    }
}

impl Default for AeroflyAdapter {
    fn default() -> Self {
        Self::new()
    }
}

// ── Parsing ───────────────────────────────────────────────────────────────────

/// Decode a raw Aerofly binary UDP datagram into [`AeroflyTelemetry`].
///
/// # Errors
///
/// - [`AeroflyAdapterError::FrameTooShort`] — fewer than [`MIN_FRAME_SIZE`] bytes.
/// - [`AeroflyAdapterError::BadMagic`] — bytes 0–3 ≠ [`AEROFLY_MAGIC`].
pub fn parse_telemetry(data: &[u8]) -> Result<AeroflyTelemetry, AeroflyAdapterError> {
    if data.len() < MIN_FRAME_SIZE {
        return Err(AeroflyAdapterError::FrameTooShort { found: data.len() });
    }

    let magic = read_u32_le(data, 0)?;
    if magic != AEROFLY_MAGIC {
        return Err(AeroflyAdapterError::BadMagic { found: magic });
    }

    let pitch = read_f32_le(data, 4)?;
    let roll = read_f32_le(data, 8)?;
    let heading = read_f32_le(data, 12)?;
    let airspeed = read_f32_le(data, 16)?;
    let altitude = read_f32_le(data, 20)?;
    let throttle_pos = read_f32_le(data, 24)?.clamp(0.0, 1.0);
    let gear_down = data[28] != 0;
    let flaps_ratio = read_f32_le(data, 29)?.clamp(0.0, 1.0);

    tracing::trace!(
        pitch,
        roll,
        heading,
        airspeed,
        altitude,
        "parsed Aerofly telemetry"
    );

    Ok(AeroflyTelemetry {
        pitch,
        roll,
        heading,
        airspeed,
        altitude,
        throttle_pos,
        gear_down,
        flaps_ratio,
    })
}

/// Decode a JSON string into [`AeroflyTelemetry`].
///
/// Expects a JSON object with camelCase or snake_case field names matching
/// the [`AeroflyTelemetry`] struct.
///
/// # Errors
///
/// Returns [`AeroflyAdapterError::JsonError`] when the string is not valid JSON
/// or does not match the expected schema.
pub fn parse_json_telemetry(json: &str) -> Result<AeroflyTelemetry, AeroflyAdapterError> {
    serde_json::from_str(json).map_err(|e| AeroflyAdapterError::JsonError(e.to_string()))
}

// ── Private helpers ───────────────────────────────────────────────────────────

fn read_f32_le(data: &[u8], offset: usize) -> Result<f32, AeroflyAdapterError> {
    let bytes: [u8; 4] = data
        .get(offset..offset + 4)
        .ok_or(AeroflyAdapterError::ReadError { offset })?
        .try_into()
        .map_err(|_| AeroflyAdapterError::ReadError { offset })?;
    Ok(f32::from_le_bytes(bytes))
}

fn read_u32_le(data: &[u8], offset: usize) -> Result<u32, AeroflyAdapterError> {
    let bytes: [u8; 4] = data
        .get(offset..offset + 4)
        .ok_or(AeroflyAdapterError::ReadError { offset })?
        .try_into()
        .map_err(|_| AeroflyAdapterError::ReadError { offset })?;
    Ok(u32::from_le_bytes(bytes))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal valid Aerofly binary frame.
    fn build_frame(
        pitch: f32,
        roll: f32,
        heading: f32,
        airspeed: f32,
        altitude: f32,
        throttle_pos: f32,
        gear_down: u8,
        flaps_ratio: f32,
    ) -> Vec<u8> {
        let mut buf = vec![0u8; MIN_FRAME_SIZE];
        buf[0..4].copy_from_slice(&AEROFLY_MAGIC.to_le_bytes());
        buf[4..8].copy_from_slice(&pitch.to_le_bytes());
        buf[8..12].copy_from_slice(&roll.to_le_bytes());
        buf[12..16].copy_from_slice(&heading.to_le_bytes());
        buf[16..20].copy_from_slice(&airspeed.to_le_bytes());
        buf[20..24].copy_from_slice(&altitude.to_le_bytes());
        buf[24..28].copy_from_slice(&throttle_pos.to_le_bytes());
        buf[28] = gear_down;
        buf[29..33].copy_from_slice(&flaps_ratio.to_le_bytes());
        buf
    }

    // ── parse_telemetry (binary) ─────────────────────────────────────────────

    #[test]
    fn parse_valid_binary_frame() {
        let data = build_frame(5.0, -3.0, 270.0, 120.0, 3_000.0, 0.8, 1, 0.3);
        let t = parse_telemetry(&data).unwrap();
        assert!((t.pitch - 5.0).abs() < 0.01);
        assert!((t.roll - (-3.0)).abs() < 0.01);
        assert!((t.heading - 270.0).abs() < 0.01);
        assert!((t.airspeed - 120.0).abs() < 0.01);
        assert!((t.altitude - 3_000.0).abs() < 0.01);
        assert!((t.throttle_pos - 0.8).abs() < 0.01);
        assert!(t.gear_down);
        assert!((t.flaps_ratio - 0.3).abs() < 0.01);
    }

    #[test]
    fn frame_too_short_returns_error() {
        let short = vec![0u8; MIN_FRAME_SIZE - 1];
        let err = parse_telemetry(&short).unwrap_err();
        assert_eq!(
            err,
            AeroflyAdapterError::FrameTooShort {
                found: MIN_FRAME_SIZE - 1
            }
        );
    }

    #[test]
    fn empty_frame_returns_error() {
        let err = parse_telemetry(&[]).unwrap_err();
        assert!(matches!(
            err,
            AeroflyAdapterError::FrameTooShort { found: 0 }
        ));
    }

    #[test]
    fn bad_magic_returns_error() {
        let mut data = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0, 0.0);
        data[0..4].copy_from_slice(&0xDEAD_BEEFu32.to_le_bytes());
        let err = parse_telemetry(&data).unwrap_err();
        assert!(matches!(
            err,
            AeroflyAdapterError::BadMagic { found: 0xDEAD_BEEF }
        ));
    }

    #[test]
    fn gear_down_byte_nonzero() {
        let data = build_frame(0.0, 0.0, 0.0, 100.0, 1_000.0, 0.5, 1, 0.0);
        let t = parse_telemetry(&data).unwrap();
        assert!(t.gear_down);
    }

    #[test]
    fn gear_up_byte_zero() {
        let data = build_frame(0.0, 0.0, 0.0, 200.0, 10_000.0, 0.9, 0, 0.0);
        let t = parse_telemetry(&data).unwrap();
        assert!(!t.gear_down);
    }

    #[test]
    fn throttle_clamped_above_one() {
        let data = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, 1.5, 0, 0.0);
        let t = parse_telemetry(&data).unwrap();
        assert!(
            (t.throttle_pos - 1.0).abs() < 0.01,
            "throttle_pos={}",
            t.throttle_pos
        );
    }

    #[test]
    fn flaps_ratio_clamped_below_zero() {
        let data = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0, -0.5);
        let t = parse_telemetry(&data).unwrap();
        assert!(t.flaps_ratio >= 0.0, "flaps_ratio={}", t.flaps_ratio);
    }

    // ── parse_json_telemetry ─────────────────────────────────────────────────

    #[test]
    fn parse_valid_json() {
        let json = r#"{
            "pitch": 3.5,
            "roll": -1.2,
            "heading": 180.0,
            "airspeed": 95.0,
            "altitude": 2500.0,
            "throttle_pos": 0.7,
            "gear_down": true,
            "flaps_ratio": 0.5
        }"#;
        let t = parse_json_telemetry(json).unwrap();
        assert!((t.pitch - 3.5).abs() < 0.01);
        assert!((t.heading - 180.0).abs() < 0.01);
        assert!(t.gear_down);
        assert!((t.flaps_ratio - 0.5).abs() < 0.01);
    }

    #[test]
    fn invalid_json_returns_error() {
        let err = parse_json_telemetry("not json at all").unwrap_err();
        assert!(matches!(err, AeroflyAdapterError::JsonError(_)));
    }

    // ── AeroflyAircraftType ──────────────────────────────────────────────────

    #[test]
    fn aircraft_type_from_name() {
        assert_eq!(
            AeroflyAircraftType::from_name("Cessna 172 Skyhawk"),
            AeroflyAircraftType::Cessna172
        );
        assert_eq!(
            AeroflyAircraftType::from_name("Airbus A320"),
            AeroflyAircraftType::AirbusA320
        );
        assert_eq!(
            AeroflyAircraftType::from_name("Boeing 737"),
            AeroflyAircraftType::BoeingB737
        );
        assert_eq!(
            AeroflyAircraftType::from_name("Piper PA-28 Cherokee"),
            AeroflyAircraftType::PiperCherokee
        );
        assert_eq!(
            AeroflyAircraftType::from_name("Extra 330SC"),
            AeroflyAircraftType::Extra330
        );
        assert_eq!(
            AeroflyAircraftType::from_name("Unknown Plane"),
            AeroflyAircraftType::Unknown
        );
    }

    #[test]
    fn aircraft_type_case_insensitive() {
        assert_eq!(
            AeroflyAircraftType::from_name("CESSNA C172"),
            AeroflyAircraftType::Cessna172
        );
        assert_eq!(
            AeroflyAircraftType::from_name("a320neo"),
            AeroflyAircraftType::AirbusA320
        );
    }

    // ── AeroflyTelemetry ─────────────────────────────────────────────────────

    #[test]
    fn telemetry_default_values() {
        let t = AeroflyTelemetry::default();
        assert_eq!(t.pitch, 0.0);
        assert_eq!(t.roll, 0.0);
        assert_eq!(t.heading, 0.0);
        assert_eq!(t.airspeed, 0.0);
        assert_eq!(t.altitude, 0.0);
        assert_eq!(t.throttle_pos, 0.0);
        assert!(!t.gear_down);
        assert_eq!(t.flaps_ratio, 0.0);
    }

    #[test]
    fn telemetry_serde_round_trip() {
        let t = AeroflyTelemetry {
            pitch: 5.0,
            roll: -2.0,
            heading: 90.0,
            airspeed: 110.0,
            altitude: 4_000.0,
            throttle_pos: 0.75,
            gear_down: true,
            flaps_ratio: 0.25,
        };
        let json = serde_json::to_string(&t).expect("serialize");
        let back: AeroflyTelemetry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, t);
    }

    // ── AeroflyAdapter ───────────────────────────────────────────────────────

    #[test]
    fn adapter_no_telemetry_initially() {
        let adapter = AeroflyAdapter::new();
        assert!(adapter.last_telemetry().is_none());
    }

    #[test]
    fn adapter_default_port() {
        let adapter = AeroflyAdapter::default();
        assert_eq!(adapter.port, AEROFLY_DEFAULT_PORT);
    }

    #[test]
    fn adapter_custom_port() {
        let adapter = AeroflyAdapter::with_port(12345);
        assert_eq!(adapter.port, 12345);
    }

    #[test]
    fn adapter_process_datagram_updates_last() {
        let mut adapter = AeroflyAdapter::new();
        let data = build_frame(2.0, 1.0, 45.0, 80.0, 1_500.0, 0.6, 0, 0.0);
        let t = adapter.process_datagram(&data).unwrap();
        assert!((t.pitch - 2.0).abs() < 0.01);
        assert!(adapter.last_telemetry().is_some());
    }

    #[test]
    fn adapter_process_invalid_datagram_returns_error() {
        let mut adapter = AeroflyAdapter::new();
        let result = adapter.process_datagram(&[0u8; 4]);
        assert!(result.is_err());
        assert!(adapter.last_telemetry().is_none());
    }

    #[test]
    fn adapter_process_json_updates_last() {
        let mut adapter = AeroflyAdapter::new();
        let json = r#"{"pitch":1.0,"roll":0.0,"heading":0.0,"airspeed":60.0,"altitude":500.0,"throttle_pos":0.4,"gear_down":false,"flaps_ratio":0.0}"#;
        let t = adapter.process_json(json).unwrap();
        assert!((t.airspeed - 60.0).abs() < 0.01);
        assert!(adapter.last_telemetry().is_some());
    }
}
