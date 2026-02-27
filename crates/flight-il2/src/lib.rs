// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! IL-2 Great Battles UDP telemetry adapter for OpenFlight.
//!
//! IL-2 Great Battles can stream real-time telemetry over a local UDP socket.
//! By default frames are sent to `127.0.0.1:34385`. The frame layout
//! (community-documented) is a packed, little-endian binary struct:
//!
//! | Offset | Size | Field      | Unit       |
//! |--------|------|------------|------------|
//! | 0      | 4    | `magic`    | `u32`      |
//! | 4      | 4    | `version`  | `u32`      |
//! | 8      | 4    | `pitch`    | degrees    |
//! | 12     | 4    | `roll`     | degrees    |
//! | 16     | 4    | `yaw`      | degrees    |
//! | 20     | 4    | `speed`    | m/s        |
//! | 24     | 4    | `altitude` | metres     |
//! | 28     | 4    | `throttle` | 0.0 – 1.0  |
//! | 32     | 1    | `gear`     | [`GearState`] |
//!
//! **Magic**: `0x494C_3200` (`"IL2\0"` in ASCII, little-endian).  
//! **UDP port**: [`IL2_DEFAULT_PORT`] (configurable in `startup.cfg`).
//!
//! ## Enabling telemetry in IL-2
//!
//! Add the following section to `<game>\data\startup.cfg`:
//!
//! ```text
//! [KEY = telemetry]
//!   addr = "127.0.0.1"
//!   port = 34385
//!   freq = 50
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;

// Suppress the lint for the dep that is reserved for future real implementation.
#[allow(unused_extern_crates)]
extern crate flight_core;

/// Expected magic number at the start of every IL-2 telemetry frame (`"IL2\0"`).
pub const IL2_MAGIC: u32 = 0x494C_3200;

/// Default UDP port IL-2 sends telemetry to.
pub const IL2_DEFAULT_PORT: u16 = 34385;

/// Minimum valid frame size in bytes.
pub const MIN_FRAME_SIZE: usize = 33;

/// Protocol version supported by this adapter.
pub const SUPPORTED_VERSION: u32 = 1;

// ── Error type ────────────────────────────────────────────────────────────────

/// Errors produced by the IL-2 adapter.
#[derive(Debug, Error, PartialEq)]
pub enum Il2AdapterError {
    /// The frame is shorter than [`MIN_FRAME_SIZE`].
    #[error("frame too short: expected at least {MIN_FRAME_SIZE} bytes, got {found}")]
    FrameTooShort { found: usize },

    /// The magic number did not match [`IL2_MAGIC`].
    #[error("bad magic: expected {IL2_MAGIC:#010x}, got {found:#010x}")]
    BadMagic { found: u32 },

    /// Protocol version is not [`SUPPORTED_VERSION`].
    #[error("unsupported protocol version {found} (expected {SUPPORTED_VERSION})")]
    UnsupportedVersion { found: u32 },

    /// A field could not be read at the given byte offset.
    #[error("failed to read field at offset {offset}")]
    ReadError { offset: usize },
}

// ── Domain types ──────────────────────────────────────────────────────────────

/// Landing gear state reported by the IL-2 telemetry protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum GearState {
    /// Gear fully retracted.
    Up = 0,
    /// Gear in transit (extending or retracting).
    Transitioning = 1,
    /// Gear fully extended / locked down.
    Down = 2,
}

impl TryFrom<u8> for GearState {
    type Error = u8;

    fn try_from(v: u8) -> Result<Self, u8> {
        match v {
            0 => Ok(GearState::Up),
            1 => Ok(GearState::Transitioning),
            2 => Ok(GearState::Down),
            other => Err(other),
        }
    }
}

/// A single decoded IL-2 telemetry frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Il2TelemetryFrame {
    /// Pitch angle in degrees (positive = nose-up).
    pub pitch: f32,
    /// Roll / bank angle in degrees (positive = right-wing-down).
    pub roll: f32,
    /// Yaw / heading in degrees (0 – 360).
    pub yaw: f32,
    /// True airspeed in m/s.
    pub speed: f32,
    /// Altitude above sea level in metres.
    pub altitude: f32,
    /// Throttle position normalised to `0.0` (idle) – `1.0` (full).
    pub throttle: f32,
    /// Landing gear state.
    pub gear: GearState,
}

impl Default for Il2TelemetryFrame {
    fn default() -> Self {
        Self {
            pitch: 0.0,
            roll: 0.0,
            yaw: 0.0,
            speed: 0.0,
            altitude: 0.0,
            throttle: 0.0,
            gear: GearState::Up,
        }
    }
}

/// Aircraft types available in IL-2 Great Battles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Il2AircraftType {
    /// Supermarine Spitfire (various marks).
    Spitfire,
    /// Messerschmitt Bf 109 (various marks).
    Bf109,
    /// North American P-51 Mustang.
    P51,
    /// Focke-Wulf Fw 190 (various marks).
    Fw190,
    /// Ilyushin Il-2 Shturmovik (series).
    Il2Shturmovik,
    /// Aircraft name not recognised by this adapter.
    Unknown,
}

impl Il2AircraftType {
    /// Attempt to identify the aircraft type from a display-name substring.
    ///
    /// Matching is case-insensitive.
    pub fn from_name(name: &str) -> Self {
        let lower = name.to_lowercase();
        if lower.contains("spitfire") {
            Il2AircraftType::Spitfire
        } else if lower.contains("bf 109") || lower.contains("bf109") {
            Il2AircraftType::Bf109
        } else if lower.contains("p-51") || lower.contains("p51") {
            Il2AircraftType::P51
        } else if lower.contains("fw 190") || lower.contains("fw190") {
            Il2AircraftType::Fw190
        } else if lower.contains("il-2") || lower.contains("il2") {
            Il2AircraftType::Il2Shturmovik
        } else {
            Il2AircraftType::Unknown
        }
    }
}

// ── Adapter ───────────────────────────────────────────────────────────────────

/// IL-2 Great Battles UDP telemetry adapter.
///
/// In a real deployment, bind a UDP socket to [`IL2_DEFAULT_PORT`] and pass
/// each received datagram to [`process_datagram`](Self::process_datagram).
pub struct Il2Adapter {
    /// UDP port the adapter listens on.
    pub port: u16,
    last_frame: Option<Il2TelemetryFrame>,
}

impl Il2Adapter {
    /// Create a new adapter bound to the default IL-2 telemetry port
    /// ([`IL2_DEFAULT_PORT`]).
    pub fn new() -> Self {
        tracing::info!(port = IL2_DEFAULT_PORT, "IL-2 adapter created");
        Self {
            port: IL2_DEFAULT_PORT,
            last_frame: None,
        }
    }

    /// Create a new adapter bound to a custom UDP `port`.
    pub fn with_port(port: u16) -> Self {
        tracing::info!(port, "IL-2 adapter created with custom port");
        Self {
            port,
            last_frame: None,
        }
    }

    /// Decode a raw UDP datagram and cache the result.
    ///
    /// Returns the parsed frame on success.
    pub fn process_datagram(&mut self, data: &[u8]) -> Result<Il2TelemetryFrame, Il2AdapterError> {
        tracing::debug!(len = data.len(), "processing IL-2 UDP datagram");
        let frame = parse_telemetry_frame(data)?;
        self.last_frame = Some(frame.clone());
        Ok(frame)
    }

    /// Return the most recently decoded telemetry frame, if any.
    pub fn last_frame(&self) -> Option<&Il2TelemetryFrame> {
        self.last_frame.as_ref()
    }
}

impl Default for Il2Adapter {
    fn default() -> Self {
        Self::new()
    }
}

// ── Parsing ───────────────────────────────────────────────────────────────────

/// Decode a raw IL-2 UDP datagram into an [`Il2TelemetryFrame`].
///
/// # Errors
///
/// - [`Il2AdapterError::FrameTooShort`] — fewer than [`MIN_FRAME_SIZE`] bytes.
/// - [`Il2AdapterError::BadMagic`] — bytes 0–3 ≠ [`IL2_MAGIC`].
/// - [`Il2AdapterError::UnsupportedVersion`] — bytes 4–7 ≠ [`SUPPORTED_VERSION`].
pub fn parse_telemetry_frame(data: &[u8]) -> Result<Il2TelemetryFrame, Il2AdapterError> {
    if data.len() < MIN_FRAME_SIZE {
        return Err(Il2AdapterError::FrameTooShort { found: data.len() });
    }

    let magic = read_u32_le(data, 0)?;
    if magic != IL2_MAGIC {
        return Err(Il2AdapterError::BadMagic { found: magic });
    }

    let version = read_u32_le(data, 4)?;
    if version != SUPPORTED_VERSION {
        return Err(Il2AdapterError::UnsupportedVersion { found: version });
    }

    let pitch = read_f32_le(data, 8)?;
    let roll = read_f32_le(data, 12)?;
    let yaw = read_f32_le(data, 16)?;
    let speed = read_f32_le(data, 20)?;
    let altitude = read_f32_le(data, 24)?;
    let throttle = read_f32_le(data, 28)?.clamp(0.0, 1.0);
    let gear = GearState::try_from(data[32]).unwrap_or(GearState::Up);

    tracing::trace!(
        pitch,
        roll,
        yaw,
        speed,
        altitude,
        "parsed IL-2 telemetry frame"
    );

    Ok(Il2TelemetryFrame {
        pitch,
        roll,
        yaw,
        speed,
        altitude,
        throttle,
        gear,
    })
}

// ── Private helpers ───────────────────────────────────────────────────────────

fn read_f32_le(data: &[u8], offset: usize) -> Result<f32, Il2AdapterError> {
    let bytes: [u8; 4] = data
        .get(offset..offset + 4)
        .ok_or(Il2AdapterError::ReadError { offset })?
        .try_into()
        .map_err(|_| Il2AdapterError::ReadError { offset })?;
    Ok(f32::from_le_bytes(bytes))
}

fn read_u32_le(data: &[u8], offset: usize) -> Result<u32, Il2AdapterError> {
    let bytes: [u8; 4] = data
        .get(offset..offset + 4)
        .ok_or(Il2AdapterError::ReadError { offset })?
        .try_into()
        .map_err(|_| Il2AdapterError::ReadError { offset })?;
    Ok(u32::from_le_bytes(bytes))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal valid IL-2 frame buffer.
    fn build_frame(
        pitch: f32,
        roll: f32,
        yaw: f32,
        speed: f32,
        altitude: f32,
        throttle: f32,
        gear: u8,
    ) -> Vec<u8> {
        let mut buf = vec![0u8; MIN_FRAME_SIZE];
        buf[0..4].copy_from_slice(&IL2_MAGIC.to_le_bytes());
        buf[4..8].copy_from_slice(&SUPPORTED_VERSION.to_le_bytes());
        buf[8..12].copy_from_slice(&pitch.to_le_bytes());
        buf[12..16].copy_from_slice(&roll.to_le_bytes());
        buf[16..20].copy_from_slice(&yaw.to_le_bytes());
        buf[20..24].copy_from_slice(&speed.to_le_bytes());
        buf[24..28].copy_from_slice(&altitude.to_le_bytes());
        buf[28..32].copy_from_slice(&throttle.to_le_bytes());
        buf[32] = gear;
        buf
    }

    // ── parse_telemetry_frame ────────────────────────────────────────────────

    #[test]
    fn parse_valid_frame() {
        let data = build_frame(5.0, -3.0, 180.0, 120.0, 3_000.0, 0.8, 2);
        let frame = parse_telemetry_frame(&data).unwrap();
        assert!((frame.pitch - 5.0).abs() < 0.01);
        assert!((frame.roll - (-3.0)).abs() < 0.01);
        assert!((frame.yaw - 180.0).abs() < 0.01);
        assert!((frame.speed - 120.0).abs() < 0.01);
        assert!((frame.altitude - 3_000.0).abs() < 0.01);
        assert!((frame.throttle - 0.8).abs() < 0.01);
        assert_eq!(frame.gear, GearState::Down);
    }

    #[test]
    fn frame_too_short_returns_error() {
        let short = vec![0u8; MIN_FRAME_SIZE - 1];
        let err = parse_telemetry_frame(&short).unwrap_err();
        assert_eq!(
            err,
            Il2AdapterError::FrameTooShort {
                found: MIN_FRAME_SIZE - 1
            }
        );
    }

    #[test]
    fn empty_frame_returns_error() {
        let err = parse_telemetry_frame(&[]).unwrap_err();
        assert!(matches!(err, Il2AdapterError::FrameTooShort { found: 0 }));
    }

    #[test]
    fn bad_magic_returns_error() {
        let mut data = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0);
        data[0..4].copy_from_slice(&0xDEAD_BEEFu32.to_le_bytes());
        let err = parse_telemetry_frame(&data).unwrap_err();
        assert!(matches!(
            err,
            Il2AdapterError::BadMagic { found: 0xDEAD_BEEF }
        ));
    }

    #[test]
    fn unsupported_version_returns_error() {
        let mut data = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0);
        data[4..8].copy_from_slice(&99u32.to_le_bytes());
        let err = parse_telemetry_frame(&data).unwrap_err();
        assert!(matches!(
            err,
            Il2AdapterError::UnsupportedVersion { found: 99 }
        ));
    }

    #[test]
    fn gear_state_up() {
        let data = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0);
        let frame = parse_telemetry_frame(&data).unwrap();
        assert_eq!(frame.gear, GearState::Up);
    }

    #[test]
    fn gear_state_transitioning() {
        let data = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1);
        let frame = parse_telemetry_frame(&data).unwrap();
        assert_eq!(frame.gear, GearState::Transitioning);
    }

    #[test]
    fn gear_unknown_byte_defaults_to_up() {
        // Byte 0xFF is not a valid GearState; should default to Up.
        let data = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0xFF);
        let frame = parse_telemetry_frame(&data).unwrap();
        assert_eq!(frame.gear, GearState::Up);
    }

    #[test]
    fn throttle_clamped_above_one() {
        let data = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, 1.5, 0);
        let frame = parse_telemetry_frame(&data).unwrap();
        assert!(
            (frame.throttle - 1.0).abs() < 0.01,
            "throttle={}",
            frame.throttle
        );
    }

    #[test]
    fn throttle_clamped_below_zero() {
        let data = build_frame(0.0, 0.0, 0.0, 0.0, 0.0, -0.5, 0);
        let frame = parse_telemetry_frame(&data).unwrap();
        assert!(frame.throttle >= 0.0, "throttle={}", frame.throttle);
    }

    // ── Il2AircraftType ──────────────────────────────────────────────────────

    #[test]
    fn aircraft_type_from_name() {
        assert_eq!(
            Il2AircraftType::from_name("Spitfire Mk.Vb"),
            Il2AircraftType::Spitfire
        );
        assert_eq!(
            Il2AircraftType::from_name("Bf 109 G-14"),
            Il2AircraftType::Bf109
        );
        assert_eq!(
            Il2AircraftType::from_name("P-51D Mustang"),
            Il2AircraftType::P51
        );
        assert_eq!(
            Il2AircraftType::from_name("Fw 190 A-8"),
            Il2AircraftType::Fw190
        );
        assert_eq!(
            Il2AircraftType::from_name("IL-2 mod.1943"),
            Il2AircraftType::Il2Shturmovik
        );
        assert_eq!(
            Il2AircraftType::from_name("Unknown Aircraft"),
            Il2AircraftType::Unknown
        );
    }

    #[test]
    fn aircraft_type_case_insensitive() {
        assert_eq!(
            Il2AircraftType::from_name("SPITFIRE MK IX"),
            Il2AircraftType::Spitfire
        );
        assert_eq!(
            Il2AircraftType::from_name("BF109 E-4"),
            Il2AircraftType::Bf109
        );
    }

    // ── Il2TelemetryFrame ────────────────────────────────────────────────────

    #[test]
    fn telemetry_frame_default_values() {
        let frame = Il2TelemetryFrame::default();
        assert_eq!(frame.pitch, 0.0);
        assert_eq!(frame.roll, 0.0);
        assert_eq!(frame.yaw, 0.0);
        assert_eq!(frame.speed, 0.0);
        assert_eq!(frame.altitude, 0.0);
        assert_eq!(frame.throttle, 0.0);
        assert_eq!(frame.gear, GearState::Up);
    }

    #[test]
    fn telemetry_frame_serde_round_trip() {
        let frame = Il2TelemetryFrame {
            pitch: 10.0,
            roll: -5.0,
            yaw: 270.0,
            speed: 180.0,
            altitude: 4_500.0,
            throttle: 0.9,
            gear: GearState::Down,
        };
        let json = serde_json::to_string(&frame).expect("serialize");
        let back: Il2TelemetryFrame = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, frame);
    }

    // ── Il2Adapter ───────────────────────────────────────────────────────────

    #[test]
    fn adapter_last_frame_none_initially() {
        let adapter = Il2Adapter::new();
        assert!(adapter.last_frame().is_none());
    }

    #[test]
    fn adapter_default_port() {
        let adapter = Il2Adapter::default();
        assert_eq!(adapter.port, IL2_DEFAULT_PORT);
    }

    #[test]
    fn adapter_custom_port() {
        let adapter = Il2Adapter::with_port(9999);
        assert_eq!(adapter.port, 9999);
    }

    #[test]
    fn adapter_process_datagram_updates_last_frame() {
        let mut adapter = Il2Adapter::new();
        let data = build_frame(10.0, 5.0, 90.0, 200.0, 5_000.0, 0.5, 2);
        let frame = adapter.process_datagram(&data).unwrap();
        assert!((frame.pitch - 10.0).abs() < 0.01);
        assert!(adapter.last_frame().is_some());
    }

    #[test]
    fn adapter_process_invalid_datagram_returns_error() {
        let mut adapter = Il2Adapter::new();
        let result = adapter.process_datagram(&[0u8; 4]);
        assert!(result.is_err());
        assert!(adapter.last_frame().is_none());
    }
}
