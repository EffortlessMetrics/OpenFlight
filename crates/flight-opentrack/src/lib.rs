// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! OpenTrack head tracking adapter for OpenFlight.
//!
//! OpenTrack sends head orientation over UDP (default port 4242) as 6 × f64
//! little-endian values in a 48-byte packet:
//!
//! | Offset | Size | Field   | Unit |
//! |--------|------|---------|------|
//! | 0      | 8    | `x`     | mm   |
//! | 8      | 8    | `y`     | mm   |
//! | 16     | 8    | `z`     | mm   |
//! | 24     | 8    | `yaw`   | deg  |
//! | 32     | 8    | `pitch` | deg  |
//! | 40     | 8    | `roll`  | deg  |
//!
//! ## Enabling the UDP output in OpenTrack
//!
//! In OpenTrack: *Output* → select **UDP over network** → configure host
//! `127.0.0.1` and port `4242`.

use serde::{Deserialize, Serialize};
use thiserror::Error;

// Suppress the lint for the dep that is reserved for future real implementation.
#[allow(unused_extern_crates)]
extern crate flight_core;

/// Default UDP port OpenTrack sends head-tracking data to.
pub const OPENTRACK_PORT: u16 = 4242;

/// Exact size of an OpenTrack UDP packet in bytes (6 × 8).
pub const OPENTRACK_PACKET_SIZE: usize = 48;

// ── Error type ────────────────────────────────────────────────────────────────

/// Errors produced by the OpenTrack adapter.
#[derive(Debug, Error, PartialEq)]
pub enum OpenTrackError {
    /// The received packet is shorter than [`OPENTRACK_PACKET_SIZE`].
    #[error("packet too short: expected {OPENTRACK_PACKET_SIZE} bytes, got {actual}")]
    PacketTooShort { actual: usize },

    /// At least one field in the packet is NaN or infinite.
    #[error("non-finite value in packet")]
    NonFiniteValue,
}

// ── Domain types ──────────────────────────────────────────────────────────────

/// A decoded OpenTrack head-position / orientation sample.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeadPosition {
    /// Left/right translation in millimetres (positive = right).
    pub x_mm: f64,
    /// Up/down translation in millimetres (positive = up).
    pub y_mm: f64,
    /// Forward/backward translation in millimetres (positive = forward).
    pub z_mm: f64,
    /// Yaw angle in degrees, range approximately −180 … +180.
    pub yaw_deg: f64,
    /// Pitch angle in degrees, range approximately −90 … +90.
    pub pitch_deg: f64,
    /// Roll angle in degrees, range approximately −180 … +180.
    pub roll_deg: f64,
}

impl Default for HeadPosition {
    fn default() -> Self {
        Self {
            x_mm: 0.0,
            y_mm: 0.0,
            z_mm: 0.0,
            yaw_deg: 0.0,
            pitch_deg: 0.0,
            roll_deg: 0.0,
        }
    }
}

// ── Adapter ───────────────────────────────────────────────────────────────────

/// OpenTrack UDP head-tracking adapter.
///
/// In a real deployment, bind a UDP socket to [`OPENTRACK_PORT`] and pass
/// each received datagram to [`process_datagram`](Self::process_datagram).
pub struct OpenTrackAdapter {
    /// UDP port the adapter listens on.
    pub port: u16,
    last_position: Option<HeadPosition>,
}

impl OpenTrackAdapter {
    /// Create a new adapter bound to the default OpenTrack port
    /// ([`OPENTRACK_PORT`]).
    pub fn new() -> Self {
        tracing::info!(port = OPENTRACK_PORT, "OpenTrack adapter created");
        Self {
            port: OPENTRACK_PORT,
            last_position: None,
        }
    }

    /// Create a new adapter bound to a custom UDP `port`.
    pub fn with_port(port: u16) -> Self {
        tracing::info!(port, "OpenTrack adapter created with custom port");
        Self {
            port,
            last_position: None,
        }
    }

    /// Decode a raw UDP datagram and cache the result.
    ///
    /// Returns the parsed [`HeadPosition`] on success.
    pub fn process_datagram(&mut self, data: &[u8]) -> Result<HeadPosition, OpenTrackError> {
        tracing::debug!(len = data.len(), "processing OpenTrack UDP datagram");
        let pos = parse_packet(data)?;
        self.last_position = Some(pos.clone());
        Ok(pos)
    }

    /// Return the most recently decoded head position, if any.
    pub fn last_position(&self) -> Option<&HeadPosition> {
        self.last_position.as_ref()
    }
}

impl Default for OpenTrackAdapter {
    fn default() -> Self {
        Self::new()
    }
}

// ── Parsing ───────────────────────────────────────────────────────────────────

/// Decode a raw OpenTrack UDP datagram into a [`HeadPosition`].
///
/// # Errors
///
/// - [`OpenTrackError::PacketTooShort`] — fewer than [`OPENTRACK_PACKET_SIZE`]
///   bytes.
/// - [`OpenTrackError::NonFiniteValue`] — any field is NaN or infinite.
pub fn parse_packet(data: &[u8]) -> Result<HeadPosition, OpenTrackError> {
    if data.len() < OPENTRACK_PACKET_SIZE {
        return Err(OpenTrackError::PacketTooShort { actual: data.len() });
    }

    let x = read_f64_le(data, 0);
    let y = read_f64_le(data, 8);
    let z = read_f64_le(data, 16);
    let yaw = read_f64_le(data, 24);
    let pitch = read_f64_le(data, 32);
    let roll = read_f64_le(data, 40);

    if [x, y, z, yaw, pitch, roll].iter().any(|v| !v.is_finite()) {
        return Err(OpenTrackError::NonFiniteValue);
    }

    tracing::trace!(x, y, z, yaw, pitch, roll, "parsed OpenTrack packet");

    Ok(HeadPosition {
        x_mm: x,
        y_mm: y,
        z_mm: z,
        yaw_deg: yaw,
        pitch_deg: pitch,
        roll_deg: roll,
    })
}

// ── Normalization helpers ─────────────────────────────────────────────────────

/// Normalize yaw from [−180, +180] degrees to [0, 1].
///
/// −180° maps to 0.0, 0° maps to 0.5, +180° maps to 1.0.
pub fn yaw_to_normalized(yaw_deg: f64) -> f64 {
    (yaw_deg + 180.0) / 360.0
}

/// Normalize pitch from [−90, +90] degrees to [0, 1].
///
/// −90° maps to 0.0, 0° maps to 0.5, +90° maps to 1.0.
pub fn pitch_to_normalized(pitch_deg: f64) -> f64 {
    (pitch_deg + 90.0) / 180.0
}

// ── Private helpers ───────────────────────────────────────────────────────────

fn read_f64_le(data: &[u8], offset: usize) -> f64 {
    let bytes: [u8; 8] = data[offset..offset + 8].try_into().unwrap();
    f64::from_le_bytes(bytes)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Build a valid 48-byte OpenTrack packet from six f64 values.
    fn build_packet(x: f64, y: f64, z: f64, yaw: f64, pitch: f64, roll: f64) -> Vec<u8> {
        let mut buf = vec![0u8; OPENTRACK_PACKET_SIZE];
        buf[0..8].copy_from_slice(&x.to_le_bytes());
        buf[8..16].copy_from_slice(&y.to_le_bytes());
        buf[16..24].copy_from_slice(&z.to_le_bytes());
        buf[24..32].copy_from_slice(&yaw.to_le_bytes());
        buf[32..40].copy_from_slice(&pitch.to_le_bytes());
        buf[40..48].copy_from_slice(&roll.to_le_bytes());
        buf
    }

    // ── parse_packet ─────────────────────────────────────────────────────────

    #[test]
    fn parse_all_zeros_produces_zero_fields() {
        let data = [0u8; OPENTRACK_PACKET_SIZE];
        let pos = parse_packet(&data).unwrap();
        assert_eq!(pos.x_mm, 0.0);
        assert_eq!(pos.y_mm, 0.0);
        assert_eq!(pos.z_mm, 0.0);
        assert_eq!(pos.yaw_deg, 0.0);
        assert_eq!(pos.pitch_deg, 0.0);
        assert_eq!(pos.roll_deg, 0.0);
    }

    #[test]
    fn parse_known_byte_sequence() {
        let data = build_packet(10.0, -5.0, 3.5, 45.0, -15.0, 2.0);
        let pos = parse_packet(&data).unwrap();
        assert!((pos.x_mm - 10.0).abs() < 1e-10);
        assert!((pos.y_mm - (-5.0)).abs() < 1e-10);
        assert!((pos.z_mm - 3.5).abs() < 1e-10);
        assert!((pos.yaw_deg - 45.0).abs() < 1e-10);
        assert!((pos.pitch_deg - (-15.0)).abs() < 1e-10);
        assert!((pos.roll_deg - 2.0).abs() < 1e-10);
    }

    #[test]
    fn parse_nan_in_first_field_returns_non_finite_error() {
        let data = build_packet(f64::NAN, 0.0, 0.0, 0.0, 0.0, 0.0);
        assert_eq!(parse_packet(&data), Err(OpenTrackError::NonFiniteValue));
    }

    #[test]
    fn parse_nan_in_yaw_returns_non_finite_error() {
        let data = build_packet(0.0, 0.0, 0.0, f64::NAN, 0.0, 0.0);
        assert_eq!(parse_packet(&data), Err(OpenTrackError::NonFiniteValue));
    }

    #[test]
    fn parse_positive_infinity_returns_non_finite_error() {
        let data = build_packet(0.0, 0.0, 0.0, 0.0, f64::INFINITY, 0.0);
        assert_eq!(parse_packet(&data), Err(OpenTrackError::NonFiniteValue));
    }

    #[test]
    fn parse_negative_infinity_returns_non_finite_error() {
        let data = build_packet(0.0, 0.0, f64::NEG_INFINITY, 0.0, 0.0, 0.0);
        assert_eq!(parse_packet(&data), Err(OpenTrackError::NonFiniteValue));
    }

    #[test]
    fn parse_packet_47_bytes_returns_too_short() {
        let data = vec![0u8; 47];
        assert_eq!(
            parse_packet(&data),
            Err(OpenTrackError::PacketTooShort { actual: 47 })
        );
    }

    #[test]
    fn parse_empty_packet_returns_too_short() {
        assert_eq!(
            parse_packet(&[]),
            Err(OpenTrackError::PacketTooShort { actual: 0 })
        );
    }

    #[test]
    fn parse_extra_bytes_are_ignored() {
        let mut data = build_packet(1.0, 2.0, 3.0, 4.0, 5.0, 6.0);
        data.extend_from_slice(&[0xFF; 64]);
        let pos = parse_packet(&data).unwrap();
        assert!((pos.x_mm - 1.0).abs() < 1e-10);
        assert!((pos.roll_deg - 6.0).abs() < 1e-10);
    }

    // ── yaw_to_normalized ────────────────────────────────────────────────────

    #[test]
    fn yaw_zero_degrees_normalizes_to_half() {
        assert!((yaw_to_normalized(0.0) - 0.5).abs() < 1e-10);
    }

    #[test]
    fn yaw_positive_180_normalizes_to_one() {
        assert!((yaw_to_normalized(180.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn yaw_negative_180_normalizes_to_zero() {
        assert!((yaw_to_normalized(-180.0) - 0.0).abs() < 1e-10);
    }

    // ── pitch_to_normalized ──────────────────────────────────────────────────

    #[test]
    fn pitch_zero_degrees_normalizes_to_half() {
        assert!((pitch_to_normalized(0.0) - 0.5).abs() < 1e-10);
    }

    #[test]
    fn pitch_positive_90_normalizes_to_one() {
        assert!((pitch_to_normalized(90.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn pitch_negative_90_normalizes_to_zero() {
        assert!((pitch_to_normalized(-90.0) - 0.0).abs() < 1e-10);
    }

    // ── HeadPosition ─────────────────────────────────────────────────────────

    #[test]
    fn head_position_default_is_all_zeros() {
        let pos = HeadPosition::default();
        assert_eq!(pos.x_mm, 0.0);
        assert_eq!(pos.yaw_deg, 0.0);
    }

    #[test]
    fn head_position_serde_round_trip() {
        let pos = HeadPosition {
            x_mm: 12.5,
            y_mm: -3.0,
            z_mm: 0.5,
            yaw_deg: 30.0,
            pitch_deg: -10.0,
            roll_deg: 5.0,
        };
        let json = serde_json::to_string(&pos).expect("serialize");
        let back: HeadPosition = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, pos);
    }

    // ── OpenTrackAdapter ─────────────────────────────────────────────────────

    #[test]
    fn adapter_last_position_none_initially() {
        let adapter = OpenTrackAdapter::new();
        assert!(adapter.last_position().is_none());
    }

    #[test]
    fn adapter_default_port() {
        let adapter = OpenTrackAdapter::default();
        assert_eq!(adapter.port, OPENTRACK_PORT);
    }

    #[test]
    fn adapter_custom_port() {
        let adapter = OpenTrackAdapter::with_port(9999);
        assert_eq!(adapter.port, 9999);
    }

    #[test]
    fn adapter_process_datagram_updates_last_position() {
        let mut adapter = OpenTrackAdapter::new();
        let data = build_packet(1.0, 2.0, 3.0, 10.0, -5.0, 0.0);
        let pos = adapter.process_datagram(&data).unwrap();
        assert!((pos.yaw_deg - 10.0).abs() < 1e-10);
        assert!(adapter.last_position().is_some());
    }

    #[test]
    fn adapter_process_invalid_datagram_returns_error() {
        let mut adapter = OpenTrackAdapter::new();
        let result = adapter.process_datagram(&[0u8; 10]);
        assert!(result.is_err());
        assert!(adapter.last_position().is_none());
    }

    // ── proptest ─────────────────────────────────────────────────────────────

    proptest! {
        #[test]
        fn arbitrary_48_bytes_never_panics(data: [u8; 48]) {
            // parse_packet must never panic on any 48-byte input; it may return an error.
            let _ = parse_packet(&data);
        }

        #[test]
        fn finite_values_round_trip(
            x in -1000.0_f64..1000.0,
            y in -1000.0_f64..1000.0,
            z in -1000.0_f64..1000.0,
            yaw in -180.0_f64..180.0,
            pitch in -90.0_f64..90.0,
            roll in -180.0_f64..180.0,
        ) {
            let data = build_packet(x, y, z, yaw, pitch, roll);
            let pos = parse_packet(&data).expect("finite values must parse");
            prop_assert!((pos.x_mm - x).abs() < 1e-10);
            prop_assert!((pos.yaw_deg - yaw).abs() < 1e-10);
            prop_assert!((pos.roll_deg - roll).abs() < 1e-10);
        }
    }
}
