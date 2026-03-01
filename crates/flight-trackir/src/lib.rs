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

// ── Pose filter (EMA) ─────────────────────────────────────────────────────────

/// Exponential-moving-average (EMA) low-pass filter for a [`HeadPose`].
///
/// Each axis is smoothed independently:
///   `output = α × input + (1 − α) × previous_output`
///
/// * **`alpha = 1.0`** → no smoothing (pass-through)
/// * **`alpha → 0.0`** → very heavy smoothing (slow response)
#[derive(Debug, Clone)]
pub struct PoseFilter {
    alpha: f32,
    state: Option<HeadPose>,
}

impl PoseFilter {
    /// Create a new EMA filter with the given smoothing factor.
    ///
    /// `alpha` is clamped to `[0.0, 1.0]`. Non-finite values (NaN or
    /// infinities) are treated as `1.0` (no smoothing).
    pub fn new(alpha: f32) -> Self {
        let alpha = if alpha.is_finite() {
            alpha.clamp(0.0, 1.0)
        } else {
            1.0 // Non-finite defaults to pass-through (no smoothing)
        };
        Self {
            alpha,
            state: None,
        }
    }

    /// Return the current smoothing factor.
    pub fn alpha(&self) -> f32 {
        self.alpha
    }

    /// Feed a new sample and return the smoothed result.
    pub fn apply(&mut self, input: HeadPose) -> HeadPose {
        let smoothed = match self.state {
            None => input,
            Some(prev) => HeadPose {
                x: self.alpha * input.x + (1.0 - self.alpha) * prev.x,
                y: self.alpha * input.y + (1.0 - self.alpha) * prev.y,
                z: self.alpha * input.z + (1.0 - self.alpha) * prev.z,
                yaw: self.alpha * input.yaw + (1.0 - self.alpha) * prev.yaw,
                pitch: self.alpha * input.pitch + (1.0 - self.alpha) * prev.pitch,
                roll: self.alpha * input.roll + (1.0 - self.alpha) * prev.roll,
            },
        };
        self.state = Some(smoothed);
        smoothed
    }

    /// Reset the filter to its initial state (no stored sample).
    pub fn reset(&mut self) {
        self.state = None;
    }
}

// ── Adapter ───────────────────────────────────────────────────────────────────

/// Stateful adapter that decodes UDP packets and tracks staleness.
///
/// Optionally applies an EMA low-pass filter when constructed via
/// [`TrackIrAdapter::with_smoothing`].
pub struct TrackIrAdapter {
    last_pose: Option<HeadPose>,
    last_update: Option<Instant>,
    filter: Option<PoseFilter>,
}

impl TrackIrAdapter {
    /// Create a new adapter with no stored pose and no smoothing.
    pub fn new() -> Self {
        tracing::info!("TrackIR adapter created");
        Self {
            last_pose: None,
            last_update: None,
            filter: None,
        }
    }

    /// Create a new adapter with EMA smoothing applied to every incoming pose.
    ///
    /// `alpha` is clamped to `[0.0, 1.0]`; `1.0` = pass-through.
    pub fn with_smoothing(alpha: f32) -> Self {
        tracing::info!(alpha, "TrackIR adapter created with smoothing");
        Self {
            last_pose: None,
            last_update: None,
            filter: Some(PoseFilter::new(alpha)),
        }
    }

    /// Decode `bytes`, update the cached pose, and return the normalised (and
    /// optionally smoothed) pose.
    pub fn process_packet(&mut self, bytes: &[u8]) -> Result<HeadPose, TrackIrError> {
        tracing::debug!(len = bytes.len(), "processing TrackIR UDP packet");
        let raw = parse_packet(bytes)?;
        let mut pose = normalize_pose(raw);

        if let Some(filter) = &mut self.filter {
            pose = filter.apply(pose);
        }

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

    // ── Test helpers ──────────────────────────────────────────────────────────

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

    fn assert_f32_eq(a: f32, b: f32, eps: f32) {
        assert!(
            (a - b).abs() < eps,
            "expected {a} ≈ {b} (eps = {eps})"
        );
    }

    // ── Packet parsing ────────────────────────────────────────────────────────

    #[test]
    fn parse_valid_packet_exact_values() {
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
    fn parse_all_zeros() {
        let data = build_packet(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let pkt = parse_packet(&data).unwrap();
        assert_eq!(pkt.x, 0.0);
        assert_eq!(pkt.y, 0.0);
        assert_eq!(pkt.z, 0.0);
        assert_eq!(pkt.yaw, 0.0);
        assert_eq!(pkt.pitch, 0.0);
        assert_eq!(pkt.roll, 0.0);
    }

    #[test]
    fn parse_negative_zero() {
        let data = build_packet(-0.0, -0.0, -0.0, -0.0, -0.0, -0.0);
        let pkt = parse_packet(&data).unwrap();
        // -0.0 == 0.0 per IEEE 754
        assert_eq!(pkt.x, 0.0);
        assert_eq!(pkt.yaw, 0.0);
    }

    #[test]
    fn parse_subnormal_values() {
        let tiny = f64::MIN_POSITIVE / 2.0; // subnormal
        let data = build_packet(tiny, -tiny, tiny, -tiny, tiny, -tiny);
        let pkt = parse_packet(&data).unwrap();
        assert!((pkt.x - tiny).abs() < 1e-320);
    }

    #[test]
    fn parse_large_finite_values() {
        let big = f64::MAX / 2.0;
        let data = build_packet(big, -big, big, -big, big, -big);
        let pkt = parse_packet(&data).unwrap();
        assert!((pkt.x - big).abs() < 1e300);
        assert!((pkt.y - (-big)).abs() < 1e300);
    }

    #[test]
    fn parse_extra_bytes_ignored() {
        let mut data = build_packet(1.0, 2.0, 3.0, 45.0, 10.0, -5.0);
        data.extend_from_slice(&[0xFF; 32]);
        let pkt = parse_packet(&data).unwrap();
        assert!((pkt.x - 1.0).abs() < 1e-10);
        assert!((pkt.roll - (-5.0)).abs() < 1e-10);
    }

    #[test]
    fn parse_exactly_48_bytes() {
        let data = build_packet(42.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        assert_eq!(data.len(), PACKET_SIZE);
        assert!(parse_packet(&data).is_ok());
    }

    #[test]
    fn parse_47_bytes_fails() {
        let data = vec![0u8; PACKET_SIZE - 1];
        assert_eq!(
            parse_packet(&data),
            Err(TrackIrError::PacketTooShort { actual: 47 })
        );
    }

    // ── Packet parsing — error boundary sizes ─────────────────────────────────

    #[test]
    fn parse_empty_packet() {
        assert_eq!(
            parse_packet(&[]),
            Err(TrackIrError::PacketTooShort { actual: 0 })
        );
    }

    #[test]
    fn parse_single_byte_packet() {
        assert_eq!(
            parse_packet(&[0xAB]),
            Err(TrackIrError::PacketTooShort { actual: 1 })
        );
    }

    #[test]
    fn parse_one_field_only() {
        let data = vec![0u8; 8]; // only x, missing 5 other fields
        assert_eq!(
            parse_packet(&data),
            Err(TrackIrError::PacketTooShort { actual: 8 })
        );
    }

    #[test]
    fn parse_five_fields_only() {
        let data = vec![0u8; 40]; // missing roll field
        assert_eq!(
            parse_packet(&data),
            Err(TrackIrError::PacketTooShort { actual: 40 })
        );
    }

    // ── Non-finite value detection per field ──────────────────────────────────

    #[test]
    fn parse_nan_in_x() {
        let data = build_packet(f64::NAN, 0.0, 0.0, 0.0, 0.0, 0.0);
        assert_eq!(parse_packet(&data), Err(TrackIrError::NonFiniteValue));
    }

    #[test]
    fn parse_nan_in_y() {
        let data = build_packet(0.0, f64::NAN, 0.0, 0.0, 0.0, 0.0);
        assert_eq!(parse_packet(&data), Err(TrackIrError::NonFiniteValue));
    }

    #[test]
    fn parse_nan_in_z() {
        let data = build_packet(0.0, 0.0, f64::NAN, 0.0, 0.0, 0.0);
        assert_eq!(parse_packet(&data), Err(TrackIrError::NonFiniteValue));
    }

    #[test]
    fn parse_nan_in_yaw() {
        let data = build_packet(0.0, 0.0, 0.0, f64::NAN, 0.0, 0.0);
        assert_eq!(parse_packet(&data), Err(TrackIrError::NonFiniteValue));
    }

    #[test]
    fn parse_nan_in_pitch() {
        let data = build_packet(0.0, 0.0, 0.0, 0.0, f64::NAN, 0.0);
        assert_eq!(parse_packet(&data), Err(TrackIrError::NonFiniteValue));
    }

    #[test]
    fn parse_nan_in_roll() {
        let data = build_packet(0.0, 0.0, 0.0, 0.0, 0.0, f64::NAN);
        assert_eq!(parse_packet(&data), Err(TrackIrError::NonFiniteValue));
    }

    #[test]
    fn parse_positive_infinity() {
        let data = build_packet(f64::INFINITY, 0.0, 0.0, 0.0, 0.0, 0.0);
        assert_eq!(parse_packet(&data), Err(TrackIrError::NonFiniteValue));
    }

    #[test]
    fn parse_negative_infinity() {
        let data = build_packet(0.0, 0.0, 0.0, f64::NEG_INFINITY, 0.0, 0.0);
        assert_eq!(parse_packet(&data), Err(TrackIrError::NonFiniteValue));
    }

    #[test]
    fn parse_all_nan() {
        let data = build_packet(
            f64::NAN,
            f64::NAN,
            f64::NAN,
            f64::NAN,
            f64::NAN,
            f64::NAN,
        );
        assert_eq!(parse_packet(&data), Err(TrackIrError::NonFiniteValue));
    }

    // ── Error display messages ────────────────────────────────────────────────

    #[test]
    fn error_display_packet_too_short() {
        let err = TrackIrError::PacketTooShort { actual: 12 };
        assert_eq!(
            err.to_string(),
            "packet too short: expected 48 bytes, got 12"
        );
    }

    #[test]
    fn error_display_non_finite() {
        let err = TrackIrError::NonFiniteValue;
        assert_eq!(err.to_string(), "non-finite value in TrackIR packet");
    }

    // ── 6DOF normalisation ────────────────────────────────────────────────────

    #[test]
    fn normalize_yaw_full_range() {
        let pos = normalize_pose(TrackIrPacket {
            x: 0.0, y: 0.0, z: 0.0, yaw: 180.0, pitch: 0.0, roll: 0.0,
        });
        let neg = normalize_pose(TrackIrPacket {
            x: 0.0, y: 0.0, z: 0.0, yaw: -180.0, pitch: 0.0, roll: 0.0,
        });
        assert_f32_eq(pos.yaw, 1.0, 1e-6);
        assert_f32_eq(neg.yaw, -1.0, 1e-6);
    }

    #[test]
    fn normalize_pitch_full_range() {
        let pos = normalize_pose(TrackIrPacket {
            x: 0.0, y: 0.0, z: 0.0, yaw: 0.0, pitch: 90.0, roll: 0.0,
        });
        let neg = normalize_pose(TrackIrPacket {
            x: 0.0, y: 0.0, z: 0.0, yaw: 0.0, pitch: -90.0, roll: 0.0,
        });
        assert_f32_eq(pos.pitch, 1.0, 1e-6);
        assert_f32_eq(neg.pitch, -1.0, 1e-6);
    }

    #[test]
    fn normalize_roll_full_range() {
        let pos = normalize_pose(TrackIrPacket {
            x: 0.0, y: 0.0, z: 0.0, yaw: 0.0, pitch: 0.0, roll: 180.0,
        });
        let neg = normalize_pose(TrackIrPacket {
            x: 0.0, y: 0.0, z: 0.0, yaw: 0.0, pitch: 0.0, roll: -180.0,
        });
        assert_f32_eq(pos.roll, 1.0, 1e-6);
        assert_f32_eq(neg.roll, -1.0, 1e-6);
    }

    #[test]
    fn normalize_translation_full_range() {
        let pose = normalize_pose(TrackIrPacket {
            x: 100.0, y: -100.0, z: 50.0, yaw: 0.0, pitch: 0.0, roll: 0.0,
        });
        assert_f32_eq(pose.x, 1.0, 1e-6);
        assert_f32_eq(pose.y, -1.0, 1e-6);
        assert_f32_eq(pose.z, 0.5, 1e-6);
    }

    #[test]
    fn normalize_mid_range_values() {
        let pose = normalize_pose(TrackIrPacket {
            x: 25.0, y: -50.0, z: 75.0, yaw: 45.0, pitch: -22.5, roll: 90.0,
        });
        assert_f32_eq(pose.x, 0.25, 1e-6);
        assert_f32_eq(pose.y, -0.5, 1e-6);
        assert_f32_eq(pose.z, 0.75, 1e-6);
        assert_f32_eq(pose.yaw, 0.25, 1e-6);
        assert_f32_eq(pose.pitch, -0.25, 1e-6);
        assert_f32_eq(pose.roll, 0.5, 1e-6);
    }

    #[test]
    fn normalize_symmetry() {
        let pos = normalize_pose(TrackIrPacket {
            x: 50.0, y: 50.0, z: 50.0, yaw: 90.0, pitch: 45.0, roll: 90.0,
        });
        let neg = normalize_pose(TrackIrPacket {
            x: -50.0, y: -50.0, z: -50.0, yaw: -90.0, pitch: -45.0, roll: -90.0,
        });
        assert_f32_eq(pos.x, -neg.x, 1e-6);
        assert_f32_eq(pos.y, -neg.y, 1e-6);
        assert_f32_eq(pos.z, -neg.z, 1e-6);
        assert_f32_eq(pos.yaw, -neg.yaw, 1e-6);
        assert_f32_eq(pos.pitch, -neg.pitch, 1e-6);
        assert_f32_eq(pos.roll, -neg.roll, 1e-6);
    }

    #[test]
    fn normalize_zero_packet() {
        let pose = normalize_pose(TrackIrPacket {
            x: 0.0, y: 0.0, z: 0.0, yaw: 0.0, pitch: 0.0, roll: 0.0,
        });
        assert_eq!(pose.x, 0.0);
        assert_eq!(pose.y, 0.0);
        assert_eq!(pose.z, 0.0);
        assert_eq!(pose.yaw, 0.0);
        assert_eq!(pose.pitch, 0.0);
        assert_eq!(pose.roll, 0.0);
    }

    #[test]
    fn normalize_each_axis_isolated() {
        // Only x non-zero
        let p = normalize_pose(TrackIrPacket {
            x: 100.0, y: 0.0, z: 0.0, yaw: 0.0, pitch: 0.0, roll: 0.0,
        });
        assert_f32_eq(p.x, 1.0, 1e-6);
        assert_eq!(p.y, 0.0);
        assert_eq!(p.z, 0.0);
        assert_eq!(p.yaw, 0.0);
        assert_eq!(p.pitch, 0.0);
        assert_eq!(p.roll, 0.0);

        // Only pitch non-zero
        let p2 = normalize_pose(TrackIrPacket {
            x: 0.0, y: 0.0, z: 0.0, yaw: 0.0, pitch: 45.0, roll: 0.0,
        });
        assert_eq!(p2.x, 0.0);
        assert_f32_eq(p2.pitch, 0.5, 1e-6);
    }

    #[test]
    fn normalize_clamping_extreme_values() {
        let pose = normalize_pose(TrackIrPacket {
            x: 9999.0, y: -9999.0, z: 9999.0,
            yaw: 720.0, pitch: 720.0, roll: -720.0,
        });
        assert_eq!(pose.x, 1.0);
        assert_eq!(pose.y, -1.0);
        assert_eq!(pose.z, 1.0);
        assert_eq!(pose.yaw, 1.0);
        assert_eq!(pose.pitch, 1.0);
        assert_eq!(pose.roll, -1.0);
    }

    #[test]
    fn normalize_just_outside_boundary() {
        let pose = normalize_pose(TrackIrPacket {
            x: 100.001, y: -100.001, z: 0.0,
            yaw: 180.001, pitch: 90.001, roll: 0.0,
        });
        assert_eq!(pose.x, 1.0);
        assert_eq!(pose.y, -1.0);
        assert_eq!(pose.yaw, 1.0);
        assert_eq!(pose.pitch, 1.0);
    }

    // ── HeadPose default ──────────────────────────────────────────────────────

    #[test]
    fn head_pose_default_is_zero() {
        let pose = HeadPose::default();
        assert_eq!(pose.x, 0.0);
        assert_eq!(pose.y, 0.0);
        assert_eq!(pose.z, 0.0);
        assert_eq!(pose.yaw, 0.0);
        assert_eq!(pose.pitch, 0.0);
        assert_eq!(pose.roll, 0.0);
    }

    #[test]
    fn head_pose_clone_eq() {
        let pose = HeadPose {
            x: 0.1, y: 0.2, z: 0.3, yaw: 0.4, pitch: 0.5, roll: 0.6,
        };
        let cloned = pose;
        assert_eq!(pose, cloned);
    }

    // ── Round-trip serialisation ──────────────────────────────────────────────

    #[test]
    fn round_trip_build_parse() {
        let (x, y, z, yaw, pitch, roll) = (12.5, -33.3, 77.7, 120.0, -45.0, 60.0);
        let data = build_packet(x, y, z, yaw, pitch, roll);
        let pkt = parse_packet(&data).unwrap();
        assert!((pkt.x - x).abs() < 1e-10);
        assert!((pkt.y - y).abs() < 1e-10);
        assert!((pkt.z - z).abs() < 1e-10);
        assert!((pkt.yaw - yaw).abs() < 1e-10);
        assert!((pkt.pitch - pitch).abs() < 1e-10);
        assert!((pkt.roll - roll).abs() < 1e-10);
    }

    #[test]
    fn round_trip_preserves_bit_pattern() {
        let val: f64 = 1.0 / 3.0; // repeating binary fraction
        let data = build_packet(val, val, val, val, val, val);
        let pkt = parse_packet(&data).unwrap();
        // Bit-exact: no rounding through serialisation.
        assert_eq!(pkt.x.to_bits(), val.to_bits());
        assert_eq!(pkt.yaw.to_bits(), val.to_bits());
    }

    #[test]
    fn round_trip_many_values() {
        let test_cases: &[(f64, f64, f64, f64, f64, f64)] = &[
            (0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            (100.0, 100.0, 100.0, 180.0, 90.0, 180.0),
            (-100.0, -100.0, -100.0, -180.0, -90.0, -180.0),
            (0.001, -0.001, 0.0, 0.5, -0.5, 0.0),
            (99.999, -99.999, 50.0, 179.999, 89.999, -179.999),
        ];
        for &(x, y, z, yaw, pitch, roll) in test_cases {
            let data = build_packet(x, y, z, yaw, pitch, roll);
            let pkt = parse_packet(&data).unwrap();
            assert!((pkt.x - x).abs() < 1e-10, "x mismatch for {x}");
            assert!((pkt.y - y).abs() < 1e-10, "y mismatch for {y}");
            assert!((pkt.z - z).abs() < 1e-10, "z mismatch for {z}");
            assert!((pkt.yaw - yaw).abs() < 1e-10, "yaw mismatch for {yaw}");
            assert!((pkt.pitch - pitch).abs() < 1e-10, "pitch mismatch");
            assert!((pkt.roll - roll).abs() < 1e-10, "roll mismatch");
        }
    }

    #[test]
    fn round_trip_through_adapter() {
        let mut adapter = TrackIrAdapter::new();
        let data = build_packet(50.0, -25.0, 75.0, 90.0, -45.0, 135.0);
        let pose = adapter.process_packet(&data).unwrap();

        assert_f32_eq(pose.x, 0.5, 1e-6);
        assert_f32_eq(pose.y, -0.25, 1e-6);
        assert_f32_eq(pose.z, 0.75, 1e-6);
        assert_f32_eq(pose.yaw, 0.5, 1e-6);
        assert_f32_eq(pose.pitch, -0.5, 1e-6);
        assert_f32_eq(pose.roll, 0.75, 1e-6);
    }

    // ── Adapter state machine / connection lifecycle ──────────────────────────

    #[test]
    fn adapter_initial_state() {
        let adapter = TrackIrAdapter::new();
        assert!(adapter.last_pose().is_none());
        assert!(adapter.is_stale(0));
        assert!(adapter.is_stale(u64::MAX));
    }

    #[test]
    fn adapter_default_matches_new() {
        let a = TrackIrAdapter::new();
        let b = TrackIrAdapter::default();
        assert!(a.last_pose().is_none());
        assert!(b.last_pose().is_none());
        assert!(a.is_stale(1000));
        assert!(b.is_stale(1000));
    }

    #[test]
    fn adapter_process_updates_pose() {
        let mut adapter = TrackIrAdapter::new();
        let data = build_packet(0.0, 0.0, 0.0, 90.0, 0.0, 0.0);
        let pose = adapter.process_packet(&data).unwrap();

        assert_f32_eq(pose.yaw, 0.5, 1e-6);
        assert_eq!(adapter.last_pose(), Some(pose));
    }

    #[test]
    fn adapter_sequential_packets_update() {
        let mut adapter = TrackIrAdapter::new();

        let d1 = build_packet(0.0, 0.0, 0.0, 90.0, 0.0, 0.0);
        let p1 = adapter.process_packet(&d1).unwrap();

        let d2 = build_packet(0.0, 0.0, 0.0, 180.0, 0.0, 0.0);
        let p2 = adapter.process_packet(&d2).unwrap();

        assert_ne!(p1, p2);
        assert_eq!(adapter.last_pose(), Some(p2));
    }

    #[test]
    fn adapter_error_preserves_last_pose() {
        let mut adapter = TrackIrAdapter::new();

        // First: valid packet
        let valid = build_packet(0.0, 0.0, 0.0, 90.0, 0.0, 0.0);
        let pose = adapter.process_packet(&valid).unwrap();

        // Second: invalid packet (too short)
        let result = adapter.process_packet(&[0u8; 10]);
        assert!(result.is_err());

        // Last pose should still be from the first valid packet.
        assert_eq!(adapter.last_pose(), Some(pose));
    }

    #[test]
    fn adapter_error_preserves_freshness() {
        let mut adapter = TrackIrAdapter::new();

        let valid = build_packet(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        adapter.process_packet(&valid).unwrap();
        assert!(!adapter.is_stale(5_000));

        // Failed parse should NOT update the timestamp.
        let _ = adapter.process_packet(&[0u8; 5]);
        assert!(!adapter.is_stale(5_000));
    }

    #[test]
    fn adapter_nan_packet_preserves_state() {
        let mut adapter = TrackIrAdapter::new();

        let valid = build_packet(50.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let good_pose = adapter.process_packet(&valid).unwrap();

        let bad = build_packet(f64::NAN, 0.0, 0.0, 0.0, 0.0, 0.0);
        assert!(adapter.process_packet(&bad).is_err());

        assert_eq!(adapter.last_pose(), Some(good_pose));
    }

    // ── Stale detection ───────────────────────────────────────────────────────

    #[test]
    fn stale_before_any_packet() {
        let adapter = TrackIrAdapter::new();
        assert!(adapter.is_stale(100));
        assert!(adapter.is_stale(0));
    }

    #[test]
    fn stale_after_timeout() {
        let mut adapter = TrackIrAdapter::new();
        let data = build_packet(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        adapter.process_packet(&data).unwrap();

        assert!(!adapter.is_stale(5_000));

        thread::sleep(Duration::from_millis(30));
        assert!(adapter.is_stale(10));
    }

    #[test]
    fn stale_refreshed_by_new_packet() {
        let mut adapter = TrackIrAdapter::new();
        let data = build_packet(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);

        adapter.process_packet(&data).unwrap();
        thread::sleep(Duration::from_millis(30));
        assert!(adapter.is_stale(10));

        // New packet refreshes the timestamp.
        adapter.process_packet(&data).unwrap();
        assert!(!adapter.is_stale(5_000));
    }

    #[test]
    fn stale_zero_timeout_always_stale_after_packet() {
        let mut adapter = TrackIrAdapter::new();
        let data = build_packet(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        adapter.process_packet(&data).unwrap();

        // With zero timeout, even an immediate check may be stale (elapsed > 0).
        // At least verify it doesn't panic.
        let _ = adapter.is_stale(0);
    }

    // ── Constants ─────────────────────────────────────────────────────────────

    #[test]
    fn constants_are_correct() {
        assert_eq!(TRACKIR_PORT, 4242);
        assert_eq!(PACKET_SIZE, 48);
        assert_eq!(PACKET_SIZE, 6 * std::mem::size_of::<f64>());
    }

    // ── PoseFilter (EMA smoothing) ────────────────────────────────────────────

    #[test]
    fn filter_passthrough_alpha_one() {
        let mut filter = PoseFilter::new(1.0);
        let p = HeadPose { x: 0.5, y: -0.3, z: 0.1, yaw: 0.8, pitch: -0.2, roll: 0.6 };
        let out = filter.apply(p);
        assert_eq!(out, p);
    }

    #[test]
    fn filter_first_sample_always_passthrough() {
        let mut filter = PoseFilter::new(0.1);
        let p = HeadPose { x: 0.5, y: -0.3, z: 0.1, yaw: 0.8, pitch: -0.2, roll: 0.6 };
        let out = filter.apply(p);
        // First sample passes through regardless of alpha.
        assert_eq!(out, p);
    }

    #[test]
    fn filter_zero_alpha_holds_first_value() {
        let mut filter = PoseFilter::new(0.0);
        let p1 = HeadPose { x: 0.5, y: 0.0, z: 0.0, yaw: 0.0, pitch: 0.0, roll: 0.0 };
        let p2 = HeadPose { x: 1.0, y: 0.0, z: 0.0, yaw: 0.0, pitch: 0.0, roll: 0.0 };

        let _ = filter.apply(p1);
        let out = filter.apply(p2);
        // alpha=0 means output = 0*new + 1*old = old
        assert_f32_eq(out.x, 0.5, 1e-6);
    }

    #[test]
    fn filter_half_alpha_blends() {
        let mut filter = PoseFilter::new(0.5);
        let p1 = HeadPose { x: 0.0, y: 0.0, z: 0.0, yaw: 0.0, pitch: 0.0, roll: 0.0 };
        let p2 = HeadPose { x: 1.0, y: 0.0, z: 0.0, yaw: 0.0, pitch: 0.0, roll: 0.0 };

        let _ = filter.apply(p1);
        let out = filter.apply(p2);
        // 0.5 * 1.0 + 0.5 * 0.0 = 0.5
        assert_f32_eq(out.x, 0.5, 1e-6);
    }

    #[test]
    fn filter_convergence_over_many_samples() {
        let mut filter = PoseFilter::new(0.3);
        let target = HeadPose { x: 1.0, y: 0.0, z: 0.0, yaw: 0.0, pitch: 0.0, roll: 0.0 };

        // Start with zero
        let zero = HeadPose::default();
        let _ = filter.apply(zero);

        // Feed the target repeatedly; output should converge.
        let mut last = zero;
        for _ in 0..50 {
            last = filter.apply(target);
        }
        assert_f32_eq(last.x, 1.0, 1e-3);
    }

    #[test]
    fn filter_step_response_monotonic() {
        let mut filter = PoseFilter::new(0.2);
        let zero = HeadPose::default();
        let step = HeadPose { x: 1.0, y: 0.0, z: 0.0, yaw: 0.0, pitch: 0.0, roll: 0.0 };

        let _ = filter.apply(zero);

        let mut prev_x = 0.0_f32;
        for _ in 0..20 {
            let out = filter.apply(step);
            assert!(out.x >= prev_x, "EMA step response must be monotonic");
            prev_x = out.x;
        }
    }

    #[test]
    fn filter_all_axes_smoothed_independently() {
        let mut filter = PoseFilter::new(0.5);
        let p1 = HeadPose { x: 0.0, y: 1.0, z: 0.0, yaw: 0.0, pitch: 0.0, roll: 1.0 };
        let p2 = HeadPose { x: 1.0, y: 0.0, z: 1.0, yaw: 1.0, pitch: 1.0, roll: 0.0 };

        let _ = filter.apply(p1);
        let out = filter.apply(p2);

        assert_f32_eq(out.x, 0.5, 1e-6);
        assert_f32_eq(out.y, 0.5, 1e-6);
        assert_f32_eq(out.z, 0.5, 1e-6);
        assert_f32_eq(out.yaw, 0.5, 1e-6);
        assert_f32_eq(out.pitch, 0.5, 1e-6);
        assert_f32_eq(out.roll, 0.5, 1e-6);
    }

    #[test]
    fn filter_reset_clears_state() {
        let mut filter = PoseFilter::new(0.5);
        let p1 = HeadPose { x: 1.0, y: 0.0, z: 0.0, yaw: 0.0, pitch: 0.0, roll: 0.0 };
        let p2 = HeadPose { x: 0.0, y: 0.0, z: 0.0, yaw: 0.0, pitch: 0.0, roll: 0.0 };

        let _ = filter.apply(p1);
        filter.reset();
        // After reset, next sample is treated as first (passthrough).
        let out = filter.apply(p2);
        assert_f32_eq(out.x, 0.0, 1e-6);
    }

    #[test]
    fn filter_alpha_getter() {
        let f = PoseFilter::new(0.42);
        assert_f32_eq(f.alpha(), 0.42, 1e-6);
    }

    #[test]
    fn filter_alpha_clamped_above() {
        let f = PoseFilter::new(1.5);
        assert_f32_eq(f.alpha(), 1.0, 1e-6);
    }

    #[test]
    fn filter_alpha_clamped_below() {
        let f = PoseFilter::new(-0.5);
        assert_f32_eq(f.alpha(), 0.0, 1e-6);
    }

    // ── Adapter with smoothing ────────────────────────────────────────────────

    #[test]
    fn adapter_with_smoothing_passthrough() {
        let mut adapter = TrackIrAdapter::with_smoothing(1.0);
        let data = build_packet(50.0, 0.0, 0.0, 90.0, 0.0, 0.0);
        let pose = adapter.process_packet(&data).unwrap();
        assert_f32_eq(pose.x, 0.5, 1e-6);
        assert_f32_eq(pose.yaw, 0.5, 1e-6);
    }

    #[test]
    fn adapter_with_smoothing_blends() {
        let mut adapter = TrackIrAdapter::with_smoothing(0.5);

        let d1 = build_packet(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let _ = adapter.process_packet(&d1).unwrap();

        let d2 = build_packet(100.0, 0.0, 0.0, 180.0, 0.0, 0.0);
        let pose = adapter.process_packet(&d2).unwrap();

        // EMA: 0.5 * 1.0 + 0.5 * 0.0 = 0.5
        assert_f32_eq(pose.x, 0.5, 1e-6);
        assert_f32_eq(pose.yaw, 0.5, 1e-6);
    }

    #[test]
    fn adapter_without_smoothing_no_blending() {
        let mut adapter = TrackIrAdapter::new();

        let d1 = build_packet(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let _ = adapter.process_packet(&d1).unwrap();

        let d2 = build_packet(100.0, 0.0, 0.0, 180.0, 0.0, 0.0);
        let pose = adapter.process_packet(&d2).unwrap();

        // No filter → raw normalised values.
        assert_f32_eq(pose.x, 1.0, 1e-6);
        assert_f32_eq(pose.yaw, 1.0, 1e-6);
    }

    // ── Byte-level packet encoding verification ───────────────────────────────

    #[test]
    fn packet_byte_layout() {
        let data = build_packet(1.0, 2.0, 3.0, 4.0, 5.0, 6.0);
        assert_eq!(&data[0..8], &1.0_f64.to_le_bytes());
        assert_eq!(&data[8..16], &2.0_f64.to_le_bytes());
        assert_eq!(&data[16..24], &3.0_f64.to_le_bytes());
        assert_eq!(&data[24..32], &4.0_f64.to_le_bytes());
        assert_eq!(&data[32..40], &5.0_f64.to_le_bytes());
        assert_eq!(&data[40..48], &6.0_f64.to_le_bytes());
    }

    // ── TrackIrPacket clone / debug ───────────────────────────────────────────

    #[test]
    fn trackir_packet_clone_eq() {
        let pkt = TrackIrPacket {
            x: 1.0, y: 2.0, z: 3.0, yaw: 4.0, pitch: 5.0, roll: 6.0,
        };
        let cloned = pkt.clone();
        assert_eq!(pkt, cloned);
    }

    #[test]
    fn trackir_packet_debug_output() {
        let pkt = TrackIrPacket {
            x: 1.0, y: 2.0, z: 3.0, yaw: 4.0, pitch: 5.0, roll: 6.0,
        };
        let debug = format!("{pkt:?}");
        assert!(debug.contains("TrackIrPacket"));
        assert!(debug.contains("yaw"));
    }

    // ── Proptest ──────────────────────────────────────────────────────────────

    proptest! {
        #[test]
        fn proptest_normalize_always_bounded(
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

        #[test]
        fn proptest_round_trip_packet(
            x in -1e6_f64..1e6,
            y in -1e6_f64..1e6,
            z in -1e6_f64..1e6,
            yaw in -1e6_f64..1e6,
            pitch in -1e6_f64..1e6,
            roll in -1e6_f64..1e6,
        ) {
            let data = build_packet(x, y, z, yaw, pitch, roll);
            let pkt = parse_packet(&data).unwrap();
            prop_assert_eq!(pkt.x.to_bits(), x.to_bits());
            prop_assert_eq!(pkt.y.to_bits(), y.to_bits());
            prop_assert_eq!(pkt.z.to_bits(), z.to_bits());
            prop_assert_eq!(pkt.yaw.to_bits(), yaw.to_bits());
            prop_assert_eq!(pkt.pitch.to_bits(), pitch.to_bits());
            prop_assert_eq!(pkt.roll.to_bits(), roll.to_bits());
        }

        #[test]
        fn proptest_filter_bounded(
            alpha in 0.0_f32..=1.0,
            x1 in -1.0_f32..=1.0,
            x2 in -1.0_f32..=1.0,
        ) {
            let mut filter = PoseFilter::new(alpha);
            let p1 = HeadPose { x: x1, ..HeadPose::default() };
            let p2 = HeadPose { x: x2, ..HeadPose::default() };
            let _ = filter.apply(p1);
            let out = filter.apply(p2);
            // Output should be between min and max of the two inputs.
            let lo = x1.min(x2);
            let hi = x1.max(x2);
            prop_assert!(out.x >= lo - 1e-6, "output {:.6} < lo {:.6}", out.x, lo);
            prop_assert!(out.x <= hi + 1e-6, "output {:.6} > hi {:.6}", out.x, hi);
        }
    }
}
