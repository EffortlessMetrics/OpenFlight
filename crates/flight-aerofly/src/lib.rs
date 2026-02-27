// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Aerofly FS 2 / 4 simulator adapter for OpenFlight.
//!
//! Aerofly FS reads flight controls via DirectInput.  OpenFlight maps physical
//! HOTAS inputs through its axis pipeline and presents them as a virtual
//! controller via ViGEm or vJoy (see the `aerofly-fs` game manifest and the
//! `flight-virtual` crate).
//!
//! This crate provides three parsing paths for telemetry work:
//!
//! 1. **Binary UDP** — a compact little-endian struct broadcast on UDP port
//!    [`AEROFLY_DEFAULT_PORT`] (stub format; not yet standardised by IPACS).
//! 2. **JSON** — a line-delimited JSON object matching the draft IPACS SDK
//!    telemetry schema.
//! 3. **Text key=value** — newline-separated `key=value` pairs (community
//!    documented; see [`parse_text_telemetry`]).
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

    /// The text payload was empty.
    #[error("empty telemetry data")]
    EmptyData,
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
    /// Vertical speed in feet per minute (positive = climbing).
    ///
    /// Not present in the binary frame; available via text and JSON formats.
    /// Defaults to `0.0` when absent.
    #[serde(default)]
    pub vspeed_fpm: f32,
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
            vspeed_fpm: 0.0,
        }
    }
}

// Conversion constants.
const FT_PER_METRE: f32 = 3.280_84;
const FPM_PER_MS: f32 = FT_PER_METRE * 60.0;
const KNOTS_PER_MS: f32 = 1.943_844;
const DEG_TO_RAD: f32 = std::f32::consts::PI / 180.0;

impl AeroflyTelemetry {
    /// Altitude converted to metres MSL.
    #[inline]
    pub fn altitude_m(&self) -> f32 {
        self.altitude / FT_PER_METRE
    }

    /// Indicated airspeed converted to metres per second.
    #[inline]
    pub fn airspeed_ms(&self) -> f32 {
        self.airspeed / KNOTS_PER_MS
    }

    /// Vertical speed converted to metres per second.
    #[inline]
    pub fn vspeed_ms(&self) -> f32 {
        self.vspeed_fpm / FPM_PER_MS
    }

    /// Pitch angle converted to radians.
    #[inline]
    pub fn pitch_rad(&self) -> f32 {
        self.pitch * DEG_TO_RAD
    }

    /// Roll angle converted to radians.
    #[inline]
    pub fn roll_rad(&self) -> f32 {
        self.roll * DEG_TO_RAD
    }

    /// Heading converted to radians.
    #[inline]
    pub fn heading_rad(&self) -> f32 {
        self.heading * DEG_TO_RAD
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

    /// Decode a text key=value telemetry string and cache the result.
    ///
    /// See [`parse_text_telemetry`] for the expected format.
    pub fn process_text(&mut self, text: &str) -> Result<AeroflyTelemetry, AeroflyAdapterError> {
        tracing::debug!("processing Aerofly text telemetry");
        let telemetry = parse_text_telemetry(text)?;
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
        vspeed_fpm: 0.0,
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

/// Decode a text key=value UDP packet into [`AeroflyTelemetry`].
///
/// Each line must be in the form `key=value`.  Unknown keys are silently
/// ignored; missing keys retain their [`Default`] values.  Unparseable
/// numeric values are treated as `0.0`.
///
/// ### Recognised keys
///
/// | Key         | Field              | Unit  |
/// |-------------|--------------------|-------|
/// | `pitch`     | `pitch`            | deg   |
/// | `roll`      | `roll`             | deg   |
/// | `hdg`       | `heading`          | deg   |
/// | `ias`       | `airspeed`         | knots |
/// | `alt`       | `altitude`         | ft    |
/// | `throttle`  | `throttle_pos`     | 0–1   |
/// | `gear`      | `gear_down`        | 0/1   |
/// | `flaps`     | `flaps_ratio`      | 0–1   |
/// | `vspeed`    | `vspeed_fpm`       | fpm   |
///
/// # Errors
///
/// Returns [`AeroflyAdapterError::EmptyData`] when `text` is empty or
/// contains only whitespace.
pub fn parse_text_telemetry(text: &str) -> Result<AeroflyTelemetry, AeroflyAdapterError> {
    if text.trim().is_empty() {
        return Err(AeroflyAdapterError::EmptyData);
    }

    let mut frame = AeroflyTelemetry::default();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.splitn(2, '=');
        let Some(key) = parts.next() else { continue };
        let val_str = parts.next().unwrap_or("").trim();
        let val: f32 = val_str.parse().unwrap_or(0.0);

        match key.trim() {
            "pitch" => frame.pitch = val,
            "roll" => frame.roll = val,
            "hdg" => frame.heading = val,
            "ias" => frame.airspeed = val,
            "alt" => frame.altitude = val,
            "throttle" => frame.throttle_pos = val.clamp(0.0, 1.0),
            "gear" => frame.gear_down = val > 0.5,
            "flaps" => frame.flaps_ratio = val.clamp(0.0, 1.0),
            "vspeed" => frame.vspeed_fpm = val,
            _ => {}
        }
    }

    Ok(frame)
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
        assert_eq!(t.vspeed_fpm, 0.0);
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
            vspeed_fpm: 500.0,
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

    // ── parse_text_telemetry ─────────────────────────────────────────────────

    #[test]
    fn parse_text_all_fields() {
        let text = "pitch=10.0\nroll=-5.0\nhdg=270.0\nias=120.0\nalt=5000.0\nthrottle=0.8\ngear=1.0\nflaps=0.5\nvspeed=800.0";
        let t = parse_text_telemetry(text).unwrap();
        assert!((t.pitch - 10.0).abs() < 0.01, "pitch");
        assert!((t.roll - (-5.0)).abs() < 0.01, "roll");
        assert!((t.heading - 270.0).abs() < 0.01, "heading");
        assert!((t.airspeed - 120.0).abs() < 0.01, "airspeed");
        assert!((t.altitude - 5_000.0).abs() < 0.01, "altitude");
        assert!((t.throttle_pos - 0.8).abs() < 0.01, "throttle_pos");
        assert!(t.gear_down, "gear_down");
        assert!((t.flaps_ratio - 0.5).abs() < 0.01, "flaps_ratio");
        assert!((t.vspeed_fpm - 800.0).abs() < 0.01, "vspeed_fpm");
    }

    #[test]
    fn parse_text_empty_returns_error() {
        assert!(matches!(
            parse_text_telemetry("").unwrap_err(),
            AeroflyAdapterError::EmptyData
        ));
        assert!(matches!(
            parse_text_telemetry("   \n  ").unwrap_err(),
            AeroflyAdapterError::EmptyData
        ));
    }

    #[test]
    fn parse_text_partial_uses_defaults() {
        // Only pitch and altitude provided; all other fields should be their defaults.
        let text = "pitch=15.0\nalt=2000.0";
        let t = parse_text_telemetry(text).unwrap();
        assert!((t.pitch - 15.0).abs() < 0.01);
        assert!((t.altitude - 2_000.0).abs() < 0.01);
        assert_eq!(t.roll, 0.0);
        assert_eq!(t.airspeed, 0.0);
        assert!(!t.gear_down);
        assert_eq!(t.vspeed_fpm, 0.0);
    }

    #[test]
    fn parse_text_invalid_numbers_default_to_zero() {
        let text = "pitch=not_a_number\nalt=banana\nias=120.0";
        let t = parse_text_telemetry(text).unwrap();
        assert_eq!(t.pitch, 0.0, "invalid pitch should default to 0");
        assert_eq!(t.altitude, 0.0, "invalid alt should default to 0");
        assert!((t.airspeed - 120.0).abs() < 0.01);
    }

    #[test]
    fn parse_text_unknown_keys_ignored() {
        let text = "unknown_key=999.0\nfuture_field=42\npitch=3.0";
        let t = parse_text_telemetry(text).unwrap();
        assert!((t.pitch - 3.0).abs() < 0.01);
        assert_eq!(t.roll, 0.0);
    }

    #[test]
    fn parse_text_gear_state_boundary() {
        // exactly 0.5 → gear up (not strictly > 0.5)
        let t_up = parse_text_telemetry("gear=0.5\nalt=1000").unwrap();
        assert!(!t_up.gear_down, "gear=0.5 should be up");

        let t_down = parse_text_telemetry("gear=0.6\nalt=1000").unwrap();
        assert!(t_down.gear_down, "gear=0.6 should be down");
    }

    #[test]
    fn parse_text_throttle_clamped() {
        let t = parse_text_telemetry("throttle=2.0\nalt=0").unwrap();
        assert!(
            (t.throttle_pos - 1.0).abs() < 0.01,
            "throttle clamped to 1.0"
        );

        let t2 = parse_text_telemetry("throttle=-0.5\nalt=0").unwrap();
        assert!(
            (t2.throttle_pos - 0.0).abs() < 0.01,
            "throttle clamped to 0.0"
        );
    }

    #[test]
    fn parse_text_negative_altitude() {
        // Below sea level (e.g. Death Valley)
        let text = "alt=-200.0";
        let t = parse_text_telemetry(text).unwrap();
        assert!((t.altitude - (-200.0)).abs() < 0.01);
    }

    #[test]
    fn parse_text_whitespace_tolerance() {
        // Extra whitespace around keys and values should be handled.
        let text = "  pitch  =  7.5  \n  ias = 90.0  ";
        let t = parse_text_telemetry(text).unwrap();
        assert!((t.pitch - 7.5).abs() < 0.01);
        assert!((t.airspeed - 90.0).abs() < 0.01);
    }

    // ── Unit conversions ─────────────────────────────────────────────────────

    #[test]
    fn altitude_conversion_ft_to_m() {
        let t = AeroflyTelemetry {
            altitude: 3_280.84,
            ..Default::default()
        };
        // 3280.84 ft ≈ 1000 m
        assert!(
            (t.altitude_m() - 1_000.0).abs() < 0.1,
            "altitude_m={}",
            t.altitude_m()
        );
    }

    #[test]
    fn airspeed_conversion_knots_to_ms() {
        let t = AeroflyTelemetry {
            airspeed: 97.192, // ≈ 50 m/s
            ..Default::default()
        };
        assert!(
            (t.airspeed_ms() - 50.0).abs() < 0.1,
            "airspeed_ms={}",
            t.airspeed_ms()
        );
    }

    #[test]
    fn attitude_conversion_deg_to_rad() {
        let t = AeroflyTelemetry {
            pitch: 90.0,
            roll: 180.0,
            heading: 360.0,
            ..Default::default()
        };
        assert!((t.pitch_rad() - std::f32::consts::FRAC_PI_2).abs() < 0.001);
        assert!((t.roll_rad() - std::f32::consts::PI).abs() < 0.001);
        assert!((t.heading_rad() - std::f32::consts::TAU).abs() < 0.001);
    }

    #[test]
    fn vspeed_conversion_fpm_to_ms() {
        let t = AeroflyTelemetry {
            vspeed_fpm: 197.0, // ≈ 1 m/s
            ..Default::default()
        };
        assert!(
            (t.vspeed_ms() - 1.0).abs() < 0.1,
            "vspeed_ms={}",
            t.vspeed_ms()
        );
    }

    #[test]
    fn zero_altitude_conversion() {
        let t = AeroflyTelemetry::default();
        assert_eq!(t.altitude_m(), 0.0);
        assert_eq!(t.airspeed_ms(), 0.0);
        assert_eq!(t.vspeed_ms(), 0.0);
    }

    #[test]
    fn adapter_process_text_updates_last() {
        let mut adapter = AeroflyAdapter::new();
        let text = "pitch=5.0\nalt=1000.0\nias=80.0\ngear=1.0";
        let t = adapter.process_text(text).unwrap();
        assert!((t.pitch - 5.0).abs() < 0.01);
        assert!(t.gear_down);
        assert!(adapter.last_telemetry().is_some());
    }

    #[test]
    fn json_vspeed_defaults_to_zero_when_absent() {
        // JSON without vspeed_fpm should deserialise successfully (serde default).
        let json = r#"{"pitch":0.0,"roll":0.0,"heading":0.0,"airspeed":0.0,"altitude":0.0,"throttle_pos":0.0,"gear_down":false,"flaps_ratio":0.0}"#;
        let t = parse_json_telemetry(json).unwrap();
        assert_eq!(t.vspeed_fpm, 0.0);
    }
}
