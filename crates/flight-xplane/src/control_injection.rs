// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! X-Plane control injection via UDP
//!
//! Writes flight control inputs to X-Plane using the native UDP protocol.
//! Constructs DREF packets (set dataref to float) and CMND packets (trigger
//! a named command), with configurable rate limiting to prevent UDP flooding.
//!
//! ## Packet formats
//!
//! | Packet | Layout |
//! |--------|--------|
//! | DREF | `"DREF\0"` + f32 (LE) + 500-byte NUL-padded path |
//! | CMND | `"CMND\0"` + NUL-terminated command path |

use crate::udp_protocol::{build_cmnd_command, build_dref_command};
use std::{
    collections::HashMap,
    io,
    net::SocketAddr,
    time::{Duration, Instant},
};
use thiserror::Error;
use tokio::net::UdpSocket;
use tracing::{debug, trace};

// ── Constants ────────────────────────────────────────────────────────

/// Default maximum number of UDP packets per second.
const DEFAULT_MAX_PACKETS_PER_SECOND: u32 = 250;

/// Minimum interval between packets to the same dataref.
const DEFAULT_MIN_DATAREF_INTERVAL: Duration = Duration::from_millis(4);

/// Well-known axis datarefs used by [`XPlaneControlInjector::set_axis`].
const AXIS_DATAREFS: &[&str] = &[
    "sim/joystick/yoke_pitch_ratio",   // 0 — pitch
    "sim/joystick/yoke_roll_ratio",    // 1 — roll
    "sim/joystick/yoke_heading_ratio", // 2 — yaw/rudder
];

// ── Errors ───────────────────────────────────────────────────────────

/// Errors returned by [`XPlaneControlInjector`].
#[derive(Error, Debug)]
pub enum ControlInjectionError {
    /// I/O or network error while sending a UDP packet.
    #[error("UDP send error: {0}")]
    Io(#[from] io::Error),
    /// The dataref path exceeds the 500-byte field in the DREF packet.
    #[error("dataref path too long ({len} bytes, max 500): {path}")]
    PathTooLong { path: String, len: usize },
    /// Value is NaN or infinite.
    #[error("invalid value: {value} (must be finite)")]
    InvalidValue { value: f32 },
    /// Packet was dropped by the rate limiter.
    #[error("rate limited: global {global_pps} pkt/s or per-ref interval for `{dataref}`")]
    RateLimited { global_pps: u32, dataref: String },
    /// Unknown axis id.
    #[error("unknown axis id {id} (valid: 0=pitch, 1=roll, 2=yaw)")]
    UnknownAxis { id: u8 },
    /// Socket not bound.
    #[error("socket not bound — call `bind` first")]
    NotBound,
}

// ── Rate limiter ─────────────────────────────────────────────────────

/// Per-dataref + global rate-limiting state.
#[derive(Debug)]
struct RateLimiter {
    max_packets_per_second: u32,
    min_dataref_interval: Duration,
    /// Timestamps of the last `window_size` global sends.
    window: Vec<Instant>,
    /// Per-dataref last-send timestamp.
    per_ref: HashMap<String, Instant>,
}

impl RateLimiter {
    fn new(max_pps: u32, min_interval: Duration) -> Self {
        Self {
            max_packets_per_second: max_pps,
            min_dataref_interval: min_interval,
            window: Vec::with_capacity(max_pps as usize),
            per_ref: HashMap::new(),
        }
    }

    /// Returns `true` if the packet should be sent, `false` if rate-limited.
    fn check(&mut self, dataref: &str, now: Instant) -> bool {
        // Per-dataref interval check
        if let Some(&last) = self.per_ref.get(dataref) {
            if now.duration_since(last) < self.min_dataref_interval {
                return false;
            }
        }

        // Global rate check — sliding window
        let one_second_ago = now - Duration::from_secs(1);
        self.window.retain(|&t| t > one_second_ago);
        if self.window.len() >= self.max_packets_per_second as usize {
            return false;
        }

        // Admit
        self.window.push(now);
        self.per_ref.insert(dataref.to_owned(), now);
        true
    }

    /// Reset all rate-limit state.
    fn reset(&mut self) {
        self.window.clear();
        self.per_ref.clear();
    }
}

// ── Configuration ────────────────────────────────────────────────────

/// Configuration for [`XPlaneControlInjector`].
#[derive(Debug, Clone)]
pub struct ControlInjectorConfig {
    /// X-Plane remote address (default `127.0.0.1:49000`).
    pub remote_addr: SocketAddr,
    /// Maximum global packet rate in packets/second.
    pub max_packets_per_second: u32,
    /// Minimum interval between writes to the same dataref.
    pub min_dataref_interval: Duration,
}

impl Default for ControlInjectorConfig {
    fn default() -> Self {
        Self {
            remote_addr: SocketAddr::from(([127, 0, 0, 1], 49000)),
            max_packets_per_second: DEFAULT_MAX_PACKETS_PER_SECOND,
            min_dataref_interval: DEFAULT_MIN_DATAREF_INTERVAL,
        }
    }
}

// ── XPlaneControlInjector ────────────────────────────────────────────

/// Writes flight-control inputs to X-Plane over UDP.
///
/// Uses the standard X-Plane DREF and CMND packet formats with rate limiting.
pub struct XPlaneControlInjector {
    socket: Option<UdpSocket>,
    config: ControlInjectorConfig,
    rate_limiter: RateLimiter,
    /// Cumulative counters for diagnostics.
    packets_sent: u64,
    packets_dropped: u64,
}

impl XPlaneControlInjector {
    /// Create a new injector with the given configuration.
    ///
    /// The socket is **not** bound yet — call [`Self::bind`] before sending.
    pub fn new(config: ControlInjectorConfig) -> Self {
        let rate_limiter = RateLimiter::new(
            config.max_packets_per_second,
            config.min_dataref_interval,
        );
        Self {
            socket: None,
            config,
            rate_limiter,
            packets_sent: 0,
            packets_dropped: 0,
        }
    }

    /// Bind a local UDP socket (port 0 for OS-assigned).
    pub async fn bind(&mut self) -> Result<(), ControlInjectionError> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect(self.config.remote_addr).await?;
        debug!(remote = %self.config.remote_addr, "control injector bound");
        self.socket = Some(socket);
        Ok(())
    }

    /// Create an injector that is already connected to the given socket.
    ///
    /// Useful for testing or when the caller manages the socket lifetime.
    pub fn with_socket(config: ControlInjectorConfig, socket: UdpSocket) -> Self {
        let rate_limiter = RateLimiter::new(
            config.max_packets_per_second,
            config.min_dataref_interval,
        );
        Self {
            socket: Some(socket),
            config,
            rate_limiter,
            packets_sent: 0,
            packets_dropped: 0,
        }
    }

    // ── public API ───────────────────────────────────────────────────

    /// Write a float value to an X-Plane dataref via a DREF packet.
    ///
    /// The value must be finite; the path must be ≤ 500 bytes.
    pub async fn set_dataref(
        &mut self,
        path: &str,
        value: f32,
    ) -> Result<(), ControlInjectionError> {
        Self::validate_value(value)?;
        Self::validate_path(path)?;

        let now = Instant::now();
        if !self.rate_limiter.check(path, now) {
            self.packets_dropped += 1;
            trace!(path, "DREF rate-limited");
            return Err(ControlInjectionError::RateLimited {
                global_pps: self.config.max_packets_per_second,
                dataref: path.to_owned(),
            });
        }

        let packet = build_dref_command(path, value);
        self.send_raw(&packet).await?;
        debug!(path, value, "DREF sent");
        Ok(())
    }

    /// Send a named X-Plane command via a CMND packet.
    pub async fn send_command(
        &mut self,
        command_path: &str,
    ) -> Result<(), ControlInjectionError> {
        Self::validate_path(command_path)?;

        let now = Instant::now();
        if !self.rate_limiter.check(command_path, now) {
            self.packets_dropped += 1;
            trace!(command_path, "CMND rate-limited");
            return Err(ControlInjectionError::RateLimited {
                global_pps: self.config.max_packets_per_second,
                dataref: command_path.to_owned(),
            });
        }

        let packet = build_cmnd_command(command_path);
        self.send_raw(&packet).await?;
        debug!(command_path, "CMND sent");
        Ok(())
    }

    /// Set a primary flight axis value in `[-1.0, +1.0]`.
    ///
    /// `axis_id`: 0 = pitch, 1 = roll, 2 = yaw.
    pub async fn set_axis(
        &mut self,
        axis_id: u8,
        value: f32,
    ) -> Result<(), ControlInjectionError> {
        let path = AXIS_DATAREFS
            .get(axis_id as usize)
            .ok_or(ControlInjectionError::UnknownAxis { id: axis_id })?;
        let clamped = Self::validate_value(value)?.clamp(-1.0, 1.0);
        self.set_dataref(path, clamped).await
    }

    // ── diagnostics ──────────────────────────────────────────────────

    /// Total packets successfully sent since creation.
    pub fn packets_sent(&self) -> u64 {
        self.packets_sent
    }

    /// Total packets dropped by the rate limiter since creation.
    pub fn packets_dropped(&self) -> u64 {
        self.packets_dropped
    }

    /// Reset rate-limiter state (e.g. after a reconnect).
    pub fn reset_rate_limiter(&mut self) {
        self.rate_limiter.reset();
    }

    /// Whether the injector has a bound socket.
    pub fn is_bound(&self) -> bool {
        self.socket.is_some()
    }

    // ── helpers ──────────────────────────────────────────────────────

    fn validate_value(value: f32) -> Result<f32, ControlInjectionError> {
        if value.is_finite() {
            Ok(value)
        } else {
            Err(ControlInjectionError::InvalidValue { value })
        }
    }

    fn validate_path(path: &str) -> Result<(), ControlInjectionError> {
        if path.len() > 500 {
            return Err(ControlInjectionError::PathTooLong {
                path: path.to_owned(),
                len: path.len(),
            });
        }
        Ok(())
    }

    async fn send_raw(&mut self, data: &[u8]) -> Result<(), ControlInjectionError> {
        let socket = self
            .socket
            .as_ref()
            .ok_or(ControlInjectionError::NotBound)?;
        socket.send(data).await?;
        self.packets_sent += 1;
        Ok(())
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::udp_protocol;

    // ── Unit tests (no network) ──────────────────────────────────────

    #[test]
    fn validate_value_rejects_nan_and_inf() {
        assert!(XPlaneControlInjector::validate_value(f32::NAN).is_err());
        assert!(XPlaneControlInjector::validate_value(f32::INFINITY).is_err());
        assert!(XPlaneControlInjector::validate_value(f32::NEG_INFINITY).is_err());
    }

    #[test]
    fn validate_value_accepts_finite() {
        assert_eq!(XPlaneControlInjector::validate_value(0.0).unwrap(), 0.0);
        assert_eq!(XPlaneControlInjector::validate_value(-1.0).unwrap(), -1.0);
        assert_eq!(XPlaneControlInjector::validate_value(42.5).unwrap(), 42.5);
    }

    #[test]
    fn validate_path_rejects_too_long() {
        let long_path = "a".repeat(501);
        assert!(XPlaneControlInjector::validate_path(&long_path).is_err());
    }

    #[test]
    fn validate_path_accepts_max_length() {
        let path = "b".repeat(500);
        assert!(XPlaneControlInjector::validate_path(&path).is_ok());
    }

    // ── DREF packet format ───────────────────────────────────────────

    #[test]
    fn dref_packet_has_correct_total_length() {
        let pkt = udp_protocol::build_dref_command("sim/test", 1.0);
        // 5 (header) + 4 (f32) + 500 (path) = 509
        assert_eq!(pkt.len(), 509);
    }

    #[test]
    fn dref_packet_starts_with_header() {
        let pkt = udp_protocol::build_dref_command("sim/test", 0.0);
        assert_eq!(&pkt[..5], b"DREF\0");
    }

    #[test]
    fn dref_packet_encodes_value_le() {
        let val = 3.14f32;
        let pkt = udp_protocol::build_dref_command("sim/test", val);
        let decoded = f32::from_le_bytes([pkt[5], pkt[6], pkt[7], pkt[8]]);
        assert!((decoded - val).abs() < f32::EPSILON);
    }

    #[test]
    fn dref_packet_encodes_path_and_pads() {
        let path = "sim/joystick/yoke_pitch_ratio";
        let pkt = udp_protocol::build_dref_command(path, 0.0);
        let path_start = 9;
        let path_region = &pkt[path_start..path_start + path.len()];
        assert_eq!(path_region, path.as_bytes());
        // Remaining bytes must be NUL padding
        assert!(pkt[path_start + path.len()..].iter().all(|&b| b == 0));
    }

    #[test]
    fn dref_packet_negative_value() {
        let pkt = udp_protocol::build_dref_command("sim/test", -0.75);
        let decoded = f32::from_le_bytes([pkt[5], pkt[6], pkt[7], pkt[8]]);
        assert!((decoded - (-0.75)).abs() < f32::EPSILON);
    }

    #[test]
    fn dref_packet_zero_value() {
        let pkt = udp_protocol::build_dref_command("sim/test", 0.0);
        let decoded = f32::from_le_bytes([pkt[5], pkt[6], pkt[7], pkt[8]]);
        assert_eq!(decoded, 0.0);
    }

    // ── CMND packet format ───────────────────────────────────────────

    #[test]
    fn cmnd_packet_starts_with_header() {
        let pkt = udp_protocol::build_cmnd_command("sim/flight_controls/flaps_down");
        assert_eq!(&pkt[..5], b"CMND\0");
    }

    #[test]
    fn cmnd_packet_contains_command_path() {
        let cmd = "sim/flight_controls/landing_gear_down";
        let pkt = udp_protocol::build_cmnd_command(cmd);
        let end = pkt[5..].iter().position(|&b| b == 0).unwrap();
        let extracted = std::str::from_utf8(&pkt[5..5 + end]).unwrap();
        assert_eq!(extracted, cmd);
    }

    #[test]
    fn cmnd_packet_is_nul_terminated() {
        let pkt = udp_protocol::build_cmnd_command("sim/test");
        assert_eq!(*pkt.last().unwrap(), 0u8);
    }

    // ── Rate limiter ─────────────────────────────────────────────────

    #[test]
    fn rate_limiter_allows_first_packet() {
        let mut rl = RateLimiter::new(100, Duration::from_millis(4));
        assert!(rl.check("sim/test", Instant::now()));
    }

    #[test]
    fn rate_limiter_blocks_rapid_same_dataref() {
        let mut rl = RateLimiter::new(1000, Duration::from_millis(50));
        let now = Instant::now();
        assert!(rl.check("sim/test", now));
        // Immediately after — should be blocked by per-ref interval
        assert!(!rl.check("sim/test", now));
    }

    #[test]
    fn rate_limiter_allows_different_datarefs() {
        let mut rl = RateLimiter::new(1000, Duration::from_millis(50));
        let now = Instant::now();
        assert!(rl.check("sim/a", now));
        assert!(rl.check("sim/b", now));
        assert!(rl.check("sim/c", now));
    }

    #[test]
    fn rate_limiter_blocks_at_global_limit() {
        let mut rl = RateLimiter::new(2, Duration::from_millis(0));
        let now = Instant::now();
        assert!(rl.check("sim/a", now));
        assert!(rl.check("sim/b", now));
        // Third packet within the same second — blocked
        assert!(!rl.check("sim/c", now));
    }

    #[test]
    fn rate_limiter_allows_after_per_ref_interval() {
        let interval = Duration::from_millis(10);
        let mut rl = RateLimiter::new(1000, interval);
        let t0 = Instant::now();
        assert!(rl.check("sim/x", t0));
        // After interval elapses
        assert!(rl.check("sim/x", t0 + interval));
    }

    #[test]
    fn rate_limiter_reset_clears_state() {
        let mut rl = RateLimiter::new(1, Duration::from_millis(0));
        let now = Instant::now();
        assert!(rl.check("sim/a", now));
        assert!(!rl.check("sim/b", now)); // global limit hit
        rl.reset();
        assert!(rl.check("sim/b", now)); // allowed after reset
    }

    // ── Config defaults ──────────────────────────────────────────────

    #[test]
    fn default_config_targets_localhost_49000() {
        let cfg = ControlInjectorConfig::default();
        assert_eq!(cfg.remote_addr, SocketAddr::from(([127, 0, 0, 1], 49000)));
    }

    #[test]
    fn default_config_rate_limits() {
        let cfg = ControlInjectorConfig::default();
        assert_eq!(cfg.max_packets_per_second, 250);
        assert_eq!(cfg.min_dataref_interval, Duration::from_millis(4));
    }

    // ── Axis mapping ─────────────────────────────────────────────────

    #[test]
    fn axis_datarefs_pitch_roll_yaw() {
        assert_eq!(AXIS_DATAREFS[0], "sim/joystick/yoke_pitch_ratio");
        assert_eq!(AXIS_DATAREFS[1], "sim/joystick/yoke_roll_ratio");
        assert_eq!(AXIS_DATAREFS[2], "sim/joystick/yoke_heading_ratio");
    }

    // ── Async integration (loopback) ─────────────────────────────────

    #[tokio::test]
    async fn set_dataref_sends_well_formed_dref_packet() {
        let receiver = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let recv_addr = receiver.local_addr().unwrap();

        let cfg = ControlInjectorConfig {
            remote_addr: recv_addr,
            ..Default::default()
        };
        let sender = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        sender.connect(recv_addr).await.unwrap();

        let mut injector = XPlaneControlInjector::with_socket(cfg, sender);

        injector
            .set_dataref("sim/joystick/yoke_pitch_ratio", 0.75)
            .await
            .unwrap();

        let mut buf = [0u8; 1024];
        let n = receiver.recv(&mut buf).await.unwrap();
        assert_eq!(n, 509); // DREF packet size
        assert_eq!(&buf[..5], b"DREF\0");

        let val = f32::from_le_bytes([buf[5], buf[6], buf[7], buf[8]]);
        assert!((val - 0.75).abs() < f32::EPSILON);
    }

    #[tokio::test]
    async fn send_command_sends_well_formed_cmnd_packet() {
        let receiver = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let recv_addr = receiver.local_addr().unwrap();

        let cfg = ControlInjectorConfig {
            remote_addr: recv_addr,
            ..Default::default()
        };
        let sender = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        sender.connect(recv_addr).await.unwrap();

        let mut injector = XPlaneControlInjector::with_socket(cfg, sender);

        let cmd = "sim/flight_controls/landing_gear_down";
        injector.send_command(cmd).await.unwrap();

        let mut buf = [0u8; 1024];
        let n = receiver.recv(&mut buf).await.unwrap();
        assert_eq!(&buf[..5], b"CMND\0");

        let end = buf[5..n].iter().position(|&b| b == 0).unwrap();
        let received_cmd = std::str::from_utf8(&buf[5..5 + end]).unwrap();
        assert_eq!(received_cmd, cmd);
    }

    #[tokio::test]
    async fn set_axis_clamps_and_sends() {
        let receiver = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let recv_addr = receiver.local_addr().unwrap();

        let cfg = ControlInjectorConfig {
            remote_addr: recv_addr,
            ..Default::default()
        };
        let sender = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        sender.connect(recv_addr).await.unwrap();

        let mut injector = XPlaneControlInjector::with_socket(cfg, sender);

        // Value > 1.0 should be clamped to 1.0
        injector.set_axis(0, 1.5).await.unwrap();

        let mut buf = [0u8; 1024];
        receiver.recv(&mut buf).await.unwrap();
        let val = f32::from_le_bytes([buf[5], buf[6], buf[7], buf[8]]);
        assert!((val - 1.0).abs() < f32::EPSILON);
    }

    #[tokio::test]
    async fn set_axis_rejects_unknown_id() {
        let receiver = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let recv_addr = receiver.local_addr().unwrap();

        let cfg = ControlInjectorConfig {
            remote_addr: recv_addr,
            ..Default::default()
        };
        let sender = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        sender.connect(recv_addr).await.unwrap();

        let mut injector = XPlaneControlInjector::with_socket(cfg, sender);
        let err = injector.set_axis(99, 0.0).await.unwrap_err();
        assert!(matches!(err, ControlInjectionError::UnknownAxis { id: 99 }));
    }

    #[tokio::test]
    async fn set_dataref_rejects_nan() {
        let receiver = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let recv_addr = receiver.local_addr().unwrap();

        let cfg = ControlInjectorConfig {
            remote_addr: recv_addr,
            ..Default::default()
        };
        let sender = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        sender.connect(recv_addr).await.unwrap();

        let mut injector = XPlaneControlInjector::with_socket(cfg, sender);
        let err = injector.set_dataref("sim/test", f32::NAN).await.unwrap_err();
        assert!(matches!(err, ControlInjectionError::InvalidValue { .. }));
    }

    #[tokio::test]
    async fn not_bound_returns_error() {
        let cfg = ControlInjectorConfig::default();
        let mut injector = XPlaneControlInjector::new(cfg);
        let err = injector.set_dataref("sim/test", 0.0).await.unwrap_err();
        assert!(matches!(err, ControlInjectionError::NotBound));
    }

    #[tokio::test]
    async fn packets_sent_counter_increments() {
        let receiver = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let recv_addr = receiver.local_addr().unwrap();

        let cfg = ControlInjectorConfig {
            remote_addr: recv_addr,
            ..Default::default()
        };
        let sender = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        sender.connect(recv_addr).await.unwrap();

        let mut injector = XPlaneControlInjector::with_socket(cfg, sender);
        assert_eq!(injector.packets_sent(), 0);

        injector.set_dataref("sim/test", 1.0).await.unwrap();
        assert_eq!(injector.packets_sent(), 1);

        injector
            .send_command("sim/flight_controls/flaps_down")
            .await
            .unwrap();
        assert_eq!(injector.packets_sent(), 2);
    }

    #[tokio::test]
    async fn rate_limiter_increments_dropped_counter() {
        let receiver = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let recv_addr = receiver.local_addr().unwrap();

        let cfg = ControlInjectorConfig {
            remote_addr: recv_addr,
            max_packets_per_second: 1,
            min_dataref_interval: Duration::from_secs(10),
        };
        let sender = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        sender.connect(recv_addr).await.unwrap();

        let mut injector = XPlaneControlInjector::with_socket(cfg, sender);

        // First succeeds
        injector.set_dataref("sim/test", 0.0).await.unwrap();
        // Second should be rate-limited
        let result = injector.set_dataref("sim/test", 1.0).await;
        assert!(result.is_err());
        assert_eq!(injector.packets_dropped(), 1);
    }
}
