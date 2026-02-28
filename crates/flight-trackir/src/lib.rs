// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! TrackIR / head tracking adapter for OpenFlight via OpenTrack bridge protocol.
//!
//! NaturalPoint TrackIR and compatible head-trackers (e.g. OpenTrack, FreePIE)
//! can send 6DOF pose data over UDP using the OpenTrack UDP bridge protocol:
//! a **48-byte** datagram containing **6 × f64** little-endian values.
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
//! ## Normalisation ranges
//!
//! | Axis        | Raw range     | Normalised |
//! |-------------|---------------|------------|
//! | yaw         | ±180°         | ±1.0       |
//! | pitch       | ±90°          | ±1.0       |
//! | roll        | ±180°         | ±1.0       |
//! | x, y, z     | ±100 mm       | ±1.0 (clamped) |
//!
//! ## Quick start
//!
//! ```no_run
//! use flight_trackir::{TrackIrAdapter};
//!
//! let mut adapter = TrackIrAdapter::new();
//! let raw_udp_bytes = [0u8; 48];
//! let pose = adapter.process_packet(&raw_udp_bytes).unwrap();
//! assert!(!adapter.is_stale(500));
//! ```

// flight-core is pulled in for workspace dependency alignment; the dep is
// reserved for future integration with the profile/bus system.
#[allow(unused_extern_crates)]
extern crate flight_core;

use std::time::{Duration, Instant};
use thiserror::Error;

/// Default UDP port the OpenTrack bridge sends data on.
pub const TRACKIR_PORT: u16 = 4242;

/// Exact byte length of an OpenTrack UDP packet (6 × 8 bytes).
pub const PACKET_SIZE: usize = 48;

// ── Error ─────────────────────────────────────────────────────────────────────

/// Errors produced by the TrackIR adapter.
#[derive(Debug, Error, PartialEq)]
pub enum TrackIrError {
    /// Packet shorter than [`PACKET_SIZE`].
    #[error("packet too short: expected {PACKET_SIZE} bytes, got {actual}")]
    PacketTooShort { actual: usize },

    /// At least one field is NaN or infinite.
    #[error("non-finite value in TrackIR packet")]
    NonFiniteValue,
}

// ── Raw packet ────────────────────────────────────────────────────────────────

/// Raw decoded OpenTrack UDP packet (physical units).
#[derive(Debug, Clone, PartialEq)]
pub struct TrackIrPacket {
    /// Left/right translation (mm).
    pub x: f64,
    /// Up/down translation (mm).
    pub y: f64,
    /// Forward/back translation (mm).
    pub z: f64,
    /// Yaw angle (degrees, −180 … +180).
    pub yaw: f64,
    /// Pitch angle (degrees, −90 … +90).
    pub pitch: f64,
    /// Roll angle (degrees, −180 … +180).
    pub roll: f64,
}

// ── Normalised pose ───────────────────────────────────────────────────────────

/// Normalised 6DOF head pose with all values in **[−1.0, 1.0]**.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeadPose {
    /// Left/right translation, ±100 mm → ±1.0.
    pub x: f32,
    /// Up/down translation, ±100 mm → ±1.0.
    pub y: f32,
    /// Forward/back translation, ±100 mm → ±1.0.
    pub z: f32,
    /// Yaw, ±180° → ±1.0.
    pub yaw: f32,
    /// Pitch, ±90° → ±1.0.
    pub pitch: f32,
    /// Roll, ±180° → ±1.0.
    pub roll: f32,
}

impl Default for HeadPose {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
        }
    }
}

// ── Parsing ───────────────────────────────────────────────────────────────────

/// Decode a raw OpenTrack UDP datagram into a [`TrackIrPacket`].
///
/// # Errors
///
/// - [`TrackIrError::PacketTooShort`] — fewer than [`PACKET_SIZE`] bytes.
/// - [`TrackIrError::NonFiniteValue`] — any field is NaN or ±∞.
pub fn parse_packet(bytes: &[u8]) -> Result<TrackIrPacket, TrackIrError> {
    if bytes.len() < PACKET_SIZE {
        return Err(TrackIrError::PacketTooShort {
            actual: bytes.len(),
        });
    }

    let x = read_f64_le(bytes, 0);
    let y = read_f64_le(bytes, 8);
    let z = read_f64_le(bytes, 16);
    let yaw = read_f64_le(bytes, 24);
    let pitch = read_f64_le(bytes, 32);
    let roll = read_f64_le(bytes, 40);

    if [x, y, z, yaw, pitch, roll].iter().any(|v| !v.is_finite()) {
        return Err(TrackIrError::NonFiniteValue);
    }

    tracing::trace!(x, y, z, yaw, pitch, roll, "parsed TrackIR packet");

    Ok(TrackIrPacket {
        x,
        y,
        z,
        yaw,
        pitch,
        roll,
    })
}

// ── Normalisation ─────────────────────────────────────────────────────────────

/// Map a raw [`TrackIrPacket`] to a normalised [`HeadPose`] with all values in
/// **[−1.0, 1.0]**.
///
/// Mapping:
/// - yaw   : ÷ 180.0 (clamped)
/// - pitch : ÷ 90.0  (clamped)
/// - roll  : ÷ 180.0 (clamped)
/// - x/y/z : ÷ 100.0 (clamped, ±100 mm = full range)
pub fn normalize_pose(raw: TrackIrPacket) -> HeadPose {
    HeadPose {
        x: ((raw.x / 100.0) as f32).clamp(-1.0, 1.0),
        y: ((raw.y / 100.0) as f32).clamp(-1.0, 1.0),
        z: ((raw.z / 100.0) as f32).clamp(-1.0, 1.0),
        yaw: ((raw.yaw / 180.0) as f32).clamp(-1.0, 1.0),
        pitch: ((raw.pitch / 90.0) as f32).clamp(-1.0, 1.0),
        roll: ((raw.roll / 180.0) as f32).clamp(-1.0, 1.0),
    }
}

// ── Adapter ───────────────────────────────────────────────────────────────────

/// Stateful adapter that decodes UDP packets and tracks staleness.
pub struct TrackIrAdapter {
    last_pose: Option<HeadPose>,
    last_update: Option<Instant>,
}

impl TrackIrAdapter {
    /// Create a new adapter with no stored pose.
    pub fn new() -> Self {
        tracing::info!("TrackIR adapter created");
        Self {
            last_pose: None,
            last_update: None,
        }
    }

    /// Decode `bytes`, update the cached pose, and return the normalised pose.
    pub fn process_packet(&mut self, bytes: &[u8]) -> Result<HeadPose, TrackIrError> {
        tracing::debug!(len = bytes.len(), "processing TrackIR UDP packet");
        let raw = parse_packet(bytes)?;
        let pose = normalize_pose(raw);
        self.last_pose = Some(pose);
        self.last_update = Some(Instant::now());
        Ok(pose)
    }

    /// Return `true` if no packet has been received within `max_age_ms`
    /// milliseconds (or if no packet has ever been received).
    pub fn is_stale(&self, max_age_ms: u64) -> bool {
        match self.last_update {
            None => true,
            Some(t) => t.elapsed() > Duration::from_millis(max_age_ms),
        }
    }

    /// Return the most recently decoded pose, if any.
    pub fn last_pose(&self) -> Option<HeadPose> {
        self.last_pose
    }
}

impl Default for TrackIrAdapter {
    fn default() -> Self {
        Self::new()
    }
}

// ── Private helpers ───────────────────────────────────────────────────────────

#[inline]
fn read_f64_le(data: &[u8], offset: usize) -> f64 {
    let bytes: [u8; 8] = data[offset..offset + 8].try_into().unwrap();
    f64::from_le_bytes(bytes)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::thread;
    use std::time::Duration;

    fn build_packet(x: f64, y: f64, z: f64, yaw: f64, pitch: f64, roll: f64) -> Vec<u8> {
        let mut buf = vec![0u8; PACKET_SIZE];
        buf[0..8].copy_from_slice(&x.to_le_bytes());
        buf[8..16].copy_from_slice(&y.to_le_bytes());
        buf[16..24].copy_from_slice(&z.to_le_bytes());
        buf[24..32].copy_from_slice(&yaw.to_le_bytes());
        buf[32..40].copy_from_slice(&pitch.to_le_bytes());
        buf[40..48].copy_from_slice(&roll.to_le_bytes());
        buf
    }

    #[test]
    fn test_parse_valid_packet() {
        let data = build_packet(10.0, -5.0, 3.5, 45.0, -15.0, 90.0);
        let pkt = parse_packet(&data).unwrap();
        assert!((pkt.x - 10.0).abs() < 1e-10);
        assert!((pkt.y - (-5.0)).abs() < 1e-10);
        assert!((pkt.z - 3.5).abs() < 1e-10);
        assert!((pkt.yaw - 45.0).abs() < 1e-10);
        assert!((pkt.pitch - (-15.0)).abs() < 1e-10);
        assert!((pkt.roll - 90.0).abs() < 1e-10);
    }

    #[test]
    fn test_parse_too_short_packet() {
        assert_eq!(
            parse_packet(&[0u8; 10]),
            Err(TrackIrError::PacketTooShort { actual: 10 })
        );
        assert_eq!(
            parse_packet(&[]),
            Err(TrackIrError::PacketTooShort { actual: 0 })
        );
    }

    #[test]
    fn test_normalize_yaw_full_rotation() {
        let pkt_pos = TrackIrPacket {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            yaw: 180.0,
            pitch: 0.0,
            roll: 0.0,
        };
        let pkt_neg = TrackIrPacket {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            yaw: -180.0,
            pitch: 0.0,
            roll: 0.0,
        };
        let pose_pos = normalize_pose(pkt_pos);
        let pose_neg = normalize_pose(pkt_neg);
        assert!((pose_pos.yaw - 1.0).abs() < 1e-6);
        assert!((pose_neg.yaw - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn test_normalize_pitch_limits() {
        let pkt_pos = TrackIrPacket {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 90.0,
            roll: 0.0,
        };
        let pkt_neg = TrackIrPacket {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            yaw: 0.0,
            pitch: -90.0,
            roll: 0.0,
        };
        assert!((normalize_pose(pkt_pos).pitch - 1.0).abs() < 1e-6);
        assert!((normalize_pose(pkt_neg).pitch - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn test_normalize_clamping() {
        let pkt = TrackIrPacket {
            x: 9999.0,
            y: -9999.0,
            z: 9999.0,
            yaw: 720.0,
            pitch: 720.0,
            roll: -720.0,
        };
        let pose = normalize_pose(pkt);
        assert_eq!(pose.x, 1.0);
        assert_eq!(pose.y, -1.0);
        assert_eq!(pose.z, 1.0);
        assert_eq!(pose.yaw, 1.0);
        assert_eq!(pose.pitch, 1.0);
        assert_eq!(pose.roll, -1.0);
    }

    #[test]
    fn test_adapter_last_pose() {
        let mut adapter = TrackIrAdapter::new();
        assert!(adapter.last_pose().is_none());

        let data = build_packet(0.0, 0.0, 0.0, 90.0, 0.0, 0.0);
        let pose = adapter.process_packet(&data).unwrap();
        assert!((pose.yaw - 0.5).abs() < 1e-6);
        assert!(adapter.last_pose().is_some());
    }

    #[test]
    fn test_stale_detection() {
        let mut adapter = TrackIrAdapter::new();
        // Before any packet: always stale.
        assert!(adapter.is_stale(100));

        let data = build_packet(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        adapter.process_packet(&data).unwrap();
        // Immediately after packet: not stale with a generous timeout.
        assert!(!adapter.is_stale(5_000));

        // After sleeping longer than the timeout: should be stale.
        thread::sleep(Duration::from_millis(30));
        assert!(adapter.is_stale(10));
    }

    #[test]
    fn test_parse_extra_bytes_ignored() {
        let mut data = build_packet(1.0, 2.0, 3.0, 45.0, 10.0, -5.0);
        data.extend_from_slice(&[0xFF; 32]);
        let pkt = parse_packet(&data).unwrap();
        assert!((pkt.x - 1.0).abs() < 1e-10);
        assert!((pkt.roll - (-5.0)).abs() < 1e-10);
    }

    #[test]
    fn test_parse_nan_returns_error() {
        let data = build_packet(f64::NAN, 0.0, 0.0, 0.0, 0.0, 0.0);
        assert_eq!(parse_packet(&data), Err(TrackIrError::NonFiniteValue));
    }

    proptest! {
        #[test]
        fn test_pose_proptest(
            x in -1000.0_f64..1000.0,
            y in -1000.0_f64..1000.0,
            z in -1000.0_f64..1000.0,
            yaw in -360.0_f64..360.0,
            pitch in -180.0_f64..180.0,
            roll in -360.0_f64..360.0,
        ) {
            let pkt = TrackIrPacket { x, y, z, yaw, pitch, roll };
            let pose = normalize_pose(pkt);
            prop_assert!((-1.0..=1.0).contains(&pose.x));
            prop_assert!((-1.0..=1.0).contains(&pose.y));
            prop_assert!((-1.0..=1.0).contains(&pose.z));
            prop_assert!((-1.0..=1.0).contains(&pose.yaw));
            prop_assert!((-1.0..=1.0).contains(&pose.pitch));
            prop_assert!((-1.0..=1.0).contains(&pose.roll));
        }
    }
}
