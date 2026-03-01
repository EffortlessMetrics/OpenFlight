// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! DCS connection state machine, reconnect policy, and session health monitoring.
//!
//! Provides a lower-level connection state machine (`DcsConnectionState`) that
//! models the UDP/TCP socket lifecycle, a configurable reconnect policy
//! (`DcsConnectionPolicy`) with exponential backoff, and packet-level session
//! health tracking (`DcsSessionHealth`) with circular-buffer rate calculation
//! and sequence gap detection.

use std::time::{Duration, Instant};
use thiserror::Error;

// ---------------------------------------------------------------------------
// DcsConnectionState
// ---------------------------------------------------------------------------

/// Low-level connection states for the DCS export socket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DcsConnectionState {
    /// Socket not yet created or previously torn down.
    Disconnected,
    /// Socket bound and waiting for incoming data.
    Listening,
    /// Actively receiving packets from DCS.
    Receiving,
    /// An error occurred; carries a description.
    Error(String),
}

impl std::fmt::Display for DcsConnectionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disconnected => write!(f, "Disconnected"),
            Self::Listening => write!(f, "Listening"),
            Self::Receiving => write!(f, "Receiving"),
            Self::Error(msg) => write!(f, "Error({msg})"),
        }
    }
}

/// Events that drive [`DcsConnectionState`] transitions.
#[derive(Debug, Clone)]
pub enum DcsConnectionEvent {
    /// Socket successfully bound.
    SocketBound,
    /// First valid packet arrived.
    PacketReceived,
    /// No packets within the timeout window.
    Timeout,
    /// A transport-level error occurred.
    TransportError(String),
    /// Explicit close / shutdown.
    Close,
}

/// Error when a state transition is not allowed.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
#[error("invalid connection transition from {from} on {event}")]
pub struct ConnectionTransitionError {
    pub from: String,
    pub event: String,
}

impl DcsConnectionState {
    /// Attempt a transition, returning the new state or an error.
    pub fn transition(
        &self,
        event: &DcsConnectionEvent,
    ) -> Result<DcsConnectionState, ConnectionTransitionError> {
        use DcsConnectionEvent::*;
        use DcsConnectionState::*;

        let next = match (self, event) {
            // Close from any state → Disconnected
            (_, Close) => Disconnected,

            // TransportError from any state → Error
            (_, TransportError(msg)) => Error(msg.clone()),

            // Disconnected → Listening
            (Disconnected, SocketBound) => Listening,

            // Listening → Receiving (first packet)
            (Listening, PacketReceived) => Receiving,

            // Receiving → Receiving (continued packets)
            (Receiving, PacketReceived) => Receiving,

            // Listening timeout → Disconnected (give up listening)
            (Listening, Timeout) => Disconnected,

            // Receiving timeout → Listening (lost sender, keep socket)
            (Receiving, Timeout) => Listening,

            // Error → Listening (retry bind)
            (Error(_), SocketBound) => Listening,

            // Everything else is invalid
            (from, evt) => {
                return Err(ConnectionTransitionError {
                    from: from.to_string(),
                    event: format!("{evt:?}"),
                });
            }
        };
        Ok(next)
    }

    /// Whether data is actively flowing.
    pub fn is_receiving(&self) -> bool {
        matches!(self, Self::Receiving)
    }

    /// Whether the connection is in an error state.
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }
}

// ---------------------------------------------------------------------------
// DcsConnectionPolicy
// ---------------------------------------------------------------------------

/// Configurable reconnection policy with exponential backoff.
#[derive(Debug, Clone)]
pub struct DcsConnectionPolicy {
    /// Maximum number of consecutive retries before giving up.
    pub max_retries: u32,
    /// Base delay between retry attempts.
    pub base_delay: Duration,
    /// Maximum delay (backoff cap).
    pub max_delay: Duration,
    /// Timeout for receiving the first packet after entering `Listening`.
    pub listen_timeout: Duration,
    /// Timeout for gaps between packets while in `Receiving`.
    pub receive_timeout: Duration,
    /// Current consecutive failure count.
    attempt: u32,
}

impl DcsConnectionPolicy {
    /// Create a new policy with the given limits.
    pub fn new(max_retries: u32, base_delay: Duration, max_delay: Duration) -> Self {
        Self {
            max_retries,
            base_delay,
            max_delay,
            listen_timeout: Duration::from_secs(10),
            receive_timeout: Duration::from_secs(5),
            attempt: 0,
        }
    }

    /// Record a failed attempt and return the backoff delay to wait.
    pub fn record_failure(&mut self) -> Duration {
        self.attempt = self.attempt.saturating_add(1);
        self.current_delay()
    }

    /// Record a successful connection (resets attempt counter).
    pub fn record_success(&mut self) {
        self.attempt = 0;
    }

    /// Current backoff delay based on attempt count.
    ///
    /// Uses `Duration` arithmetic to preserve sub-millisecond precision.
    /// The result is always clamped to `max_delay`.
    pub fn current_delay(&self) -> Duration {
        if self.attempt == 0 {
            return self.base_delay.min(self.max_delay);
        }
        // Use checked_mul to prevent overflow, operating on Duration directly
        // to avoid truncating sub-millisecond precision.
        let multiplier = 2u32.checked_pow(self.attempt.min(30)).unwrap_or(u32::MAX);
        let delay = self
            .base_delay
            .checked_mul(multiplier)
            .unwrap_or(self.max_delay);
        delay.min(self.max_delay)
    }

    /// Whether more retries are allowed.
    pub fn retries_remaining(&self) -> bool {
        self.attempt < self.max_retries
    }

    /// Current attempt number (0 = no failures yet).
    pub fn attempt(&self) -> u32 {
        self.attempt
    }

    /// Reset the attempt counter.
    pub fn reset(&mut self) {
        self.attempt = 0;
    }
}

impl Default for DcsConnectionPolicy {
    fn default() -> Self {
        Self::new(5, Duration::from_secs(1), Duration::from_secs(30))
    }
}

// ---------------------------------------------------------------------------
// DcsSessionHealth
// ---------------------------------------------------------------------------

/// Packet-rate tracking window size.
const RATE_WINDOW: usize = 64;

/// Tracks session health: packet rate, sequence gaps, and error counts.
#[derive(Debug)]
pub struct DcsSessionHealth {
    /// Circular buffer of packet arrival timestamps.
    timestamps: [Option<Instant>; RATE_WINDOW],
    /// Write cursor into `timestamps`.
    cursor: usize,
    /// Total packets recorded (may exceed `RATE_WINDOW`).
    total_packets: u64,
    /// Last received timestamp.
    last_received: Option<Instant>,
    /// Last sequence number seen (for gap detection).
    last_seq: Option<u64>,
    /// Cumulative detected sequence gaps.
    gap_count: u64,
    /// Cumulative error count.
    error_count: u64,
}

impl DcsSessionHealth {
    /// Create a new, empty health tracker.
    pub fn new() -> Self {
        Self {
            timestamps: [None; RATE_WINDOW],
            cursor: 0,
            total_packets: 0,
            last_received: None,
            last_seq: None,
            gap_count: 0,
            error_count: 0,
        }
    }

    /// Record a received packet with an optional sequence number.
    pub fn record_packet(&mut self, seq: Option<u64>) {
        let now = Instant::now();
        self.timestamps[self.cursor] = Some(now);
        self.cursor = (self.cursor + 1) % RATE_WINDOW;
        self.total_packets += 1;
        self.last_received = Some(now);

        if let Some(s) = seq {
            if let Some(prev) = self.last_seq
                && prev.checked_add(1).is_some_and(|next| s > next)
            {
                self.gap_count += s.saturating_sub(prev).saturating_sub(1);
            }
            self.last_seq = Some(s);
        }
    }

    /// Record an error.
    pub fn record_error(&mut self) {
        self.error_count += 1;
    }

    /// Compute the current packet rate (packets/sec) over the window.
    ///
    /// Returns `None` if fewer than 2 samples exist. Returns
    /// `Some(f64::INFINITY)` when all samples share the same timestamp
    /// (zero-span window).
    pub fn packet_rate(&self) -> Option<f64> {
        // Collect all present timestamps
        let mut times: Vec<Instant> = self.timestamps.iter().filter_map(|t| *t).collect();
        if times.len() < 2 {
            return None;
        }
        times.sort();
        let oldest = times[0];
        let newest = *times.last().unwrap();
        let span = newest.duration_since(oldest);
        if span.is_zero() {
            // All packets arrived faster than clock resolution — the honest
            // answer is an infinite rate.
            return Some(f64::INFINITY);
        }
        Some((times.len() - 1) as f64 / span.as_secs_f64())
    }

    /// Duration since the last received packet, or `None` if no packets yet.
    pub fn since_last_packet(&self) -> Option<Duration> {
        self.last_received.map(|t| t.elapsed())
    }

    /// Whether we have exceeded the given timeout since the last packet.
    pub fn is_timed_out(&self, timeout: Duration) -> bool {
        match self.last_received {
            Some(t) => t.elapsed() > timeout,
            None => false, // no packets yet — not timed out, just waiting
        }
    }

    /// Total packets recorded.
    pub fn total_packets(&self) -> u64 {
        self.total_packets
    }

    /// Cumulative sequence gaps detected.
    pub fn gap_count(&self) -> u64 {
        self.gap_count
    }

    /// Cumulative error count.
    pub fn error_count(&self) -> u64 {
        self.error_count
    }

    /// Reset all counters and history.
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for DcsSessionHealth {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    // ── DcsConnectionState transitions ────────────────────────────────

    #[test]
    fn disconnected_to_listening_on_socket_bound() {
        let state = DcsConnectionState::Disconnected;
        let next = state.transition(&DcsConnectionEvent::SocketBound).unwrap();
        assert_eq!(next, DcsConnectionState::Listening);
    }

    #[test]
    fn listening_to_receiving_on_packet() {
        let state = DcsConnectionState::Listening;
        let next = state
            .transition(&DcsConnectionEvent::PacketReceived)
            .unwrap();
        assert_eq!(next, DcsConnectionState::Receiving);
    }

    #[test]
    fn receiving_stays_receiving_on_packet() {
        let state = DcsConnectionState::Receiving;
        let next = state
            .transition(&DcsConnectionEvent::PacketReceived)
            .unwrap();
        assert_eq!(next, DcsConnectionState::Receiving);
    }

    #[test]
    fn receiving_to_listening_on_timeout() {
        let state = DcsConnectionState::Receiving;
        let next = state.transition(&DcsConnectionEvent::Timeout).unwrap();
        assert_eq!(next, DcsConnectionState::Listening);
    }

    #[test]
    fn listening_to_disconnected_on_timeout() {
        let state = DcsConnectionState::Listening;
        let next = state.transition(&DcsConnectionEvent::Timeout).unwrap();
        assert_eq!(next, DcsConnectionState::Disconnected);
    }

    #[test]
    fn any_state_to_error_on_transport_error() {
        for state in [
            DcsConnectionState::Disconnected,
            DcsConnectionState::Listening,
            DcsConnectionState::Receiving,
        ] {
            let next = state
                .transition(&DcsConnectionEvent::TransportError("oops".into()))
                .unwrap();
            assert!(next.is_error(), "expected Error from {state}");
        }
    }

    #[test]
    fn any_state_to_disconnected_on_close() {
        for state in [
            DcsConnectionState::Listening,
            DcsConnectionState::Receiving,
            DcsConnectionState::Error("x".into()),
        ] {
            let next = state.transition(&DcsConnectionEvent::Close).unwrap();
            assert_eq!(next, DcsConnectionState::Disconnected);
        }
    }

    #[test]
    fn error_to_listening_on_socket_bound() {
        let state = DcsConnectionState::Error("prev".into());
        let next = state.transition(&DcsConnectionEvent::SocketBound).unwrap();
        assert_eq!(next, DcsConnectionState::Listening);
    }

    #[test]
    fn invalid_disconnected_packet_received() {
        let state = DcsConnectionState::Disconnected;
        let res = state.transition(&DcsConnectionEvent::PacketReceived);
        assert!(res.is_err());
    }

    #[test]
    fn invalid_disconnected_timeout() {
        let state = DcsConnectionState::Disconnected;
        let res = state.transition(&DcsConnectionEvent::Timeout);
        assert!(res.is_err());
    }

    #[test]
    fn invalid_listening_socket_bound() {
        let state = DcsConnectionState::Listening;
        let res = state.transition(&DcsConnectionEvent::SocketBound);
        assert!(res.is_err());
    }

    #[test]
    fn display_formats() {
        assert_eq!(DcsConnectionState::Disconnected.to_string(), "Disconnected");
        assert_eq!(DcsConnectionState::Listening.to_string(), "Listening");
        assert_eq!(DcsConnectionState::Receiving.to_string(), "Receiving");
        assert_eq!(
            DcsConnectionState::Error("boom".into()).to_string(),
            "Error(boom)"
        );
    }

    // ── DcsConnectionPolicy ──────────────────────────────────────────

    #[test]
    fn policy_defaults() {
        let p = DcsConnectionPolicy::default();
        assert_eq!(p.max_retries, 5);
        assert_eq!(p.base_delay, Duration::from_secs(1));
        assert_eq!(p.max_delay, Duration::from_secs(30));
        assert_eq!(p.attempt(), 0);
        assert!(p.retries_remaining());
    }

    #[test]
    fn policy_backoff_increases() {
        let mut p =
            DcsConnectionPolicy::new(10, Duration::from_millis(100), Duration::from_secs(60));
        let d0 = p.current_delay();
        let d1 = p.record_failure();
        let d2 = p.record_failure();
        assert!(d1 >= d0, "first failure delay should be >= base");
        assert!(d2 > d1, "second failure delay should exceed first");
    }

    #[test]
    fn policy_backoff_capped() {
        let mut p = DcsConnectionPolicy::new(100, Duration::from_secs(1), Duration::from_secs(10));
        for _ in 0..50 {
            p.record_failure();
        }
        assert!(p.current_delay() <= Duration::from_secs(10));
    }

    #[test]
    fn policy_success_resets_attempts() {
        let mut p = DcsConnectionPolicy::default();
        p.record_failure();
        p.record_failure();
        assert_eq!(p.attempt(), 2);
        p.record_success();
        assert_eq!(p.attempt(), 0);
    }

    #[test]
    fn policy_retries_exhausted() {
        let mut p = DcsConnectionPolicy::new(2, Duration::from_millis(10), Duration::from_secs(1));
        p.record_failure();
        assert!(p.retries_remaining());
        p.record_failure();
        assert!(!p.retries_remaining());
    }

    #[test]
    fn policy_reset() {
        let mut p = DcsConnectionPolicy::default();
        p.record_failure();
        p.record_failure();
        p.reset();
        assert_eq!(p.attempt(), 0);
        assert!(p.retries_remaining());
    }

    // ── DcsSessionHealth ─────────────────────────────────────────────

    #[test]
    fn health_initial_state() {
        let h = DcsSessionHealth::new();
        assert_eq!(h.total_packets(), 0);
        assert_eq!(h.gap_count(), 0);
        assert_eq!(h.error_count(), 0);
        assert!(h.since_last_packet().is_none());
        assert!(h.packet_rate().is_none());
    }

    #[test]
    fn health_packet_count() {
        let mut h = DcsSessionHealth::new();
        h.record_packet(Some(1));
        h.record_packet(Some(2));
        h.record_packet(Some(3));
        assert_eq!(h.total_packets(), 3);
    }

    #[test]
    fn health_packet_rate_calculation() {
        let mut h = DcsSessionHealth::new();
        // Record packets with small gaps so rate is measurable
        h.record_packet(None);
        thread::sleep(Duration::from_millis(50));
        h.record_packet(None);
        thread::sleep(Duration::from_millis(50));
        h.record_packet(None);

        let rate = h.packet_rate().expect("should have rate with 3 samples");
        // ~20 packets/sec (50ms gap) — allow wide tolerance
        assert!(rate > 5.0, "rate {rate} too low");
        assert!(rate < 100.0, "rate {rate} too high");
    }

    #[test]
    fn health_packet_rate_none_with_single_sample() {
        let mut h = DcsSessionHealth::new();
        h.record_packet(None);
        assert!(h.packet_rate().is_none());
    }

    #[test]
    fn health_sequence_gap_detection() {
        let mut h = DcsSessionHealth::new();
        h.record_packet(Some(1));
        h.record_packet(Some(2));
        h.record_packet(Some(5)); // gap of 2 (missing 3, 4)
        assert_eq!(h.gap_count(), 2);
    }

    #[test]
    fn health_no_gap_on_consecutive_sequences() {
        let mut h = DcsSessionHealth::new();
        for seq in 1..=10 {
            h.record_packet(Some(seq));
        }
        assert_eq!(h.gap_count(), 0);
    }

    #[test]
    fn health_multiple_gaps_accumulate() {
        let mut h = DcsSessionHealth::new();
        h.record_packet(Some(1));
        h.record_packet(Some(4)); // gap 2
        h.record_packet(Some(10)); // gap 5
        assert_eq!(h.gap_count(), 7);
    }

    #[test]
    fn health_timeout_detection() {
        let mut h = DcsSessionHealth::new();
        h.record_packet(None);
        // Immediately after recording, should not be timed out with a large window
        assert!(!h.is_timed_out(Duration::from_secs(10)));
    }

    #[test]
    fn health_timeout_no_packets() {
        let h = DcsSessionHealth::new();
        // No packets yet → not timed out (haven't started)
        assert!(!h.is_timed_out(Duration::from_secs(1)));
    }

    #[test]
    fn health_error_counting() {
        let mut h = DcsSessionHealth::new();
        h.record_error();
        h.record_error();
        h.record_error();
        assert_eq!(h.error_count(), 3);
    }

    #[test]
    fn health_reset_clears_all() {
        let mut h = DcsSessionHealth::new();
        h.record_packet(Some(1));
        h.record_packet(Some(5));
        h.record_error();
        h.reset();
        assert_eq!(h.total_packets(), 0);
        assert_eq!(h.gap_count(), 0);
        assert_eq!(h.error_count(), 0);
        assert!(h.since_last_packet().is_none());
    }

    #[test]
    fn health_since_last_packet_some_after_record() {
        let mut h = DcsSessionHealth::new();
        h.record_packet(None);
        let elapsed = h.since_last_packet().expect("should be Some");
        assert!(elapsed < Duration::from_secs(1));
    }

    #[test]
    fn health_circular_buffer_wraps() {
        let mut h = DcsSessionHealth::new();
        // Fill beyond RATE_WINDOW to ensure wrapping works
        for seq in 1..=(RATE_WINDOW as u64 * 2) {
            h.record_packet(Some(seq));
        }
        assert_eq!(h.total_packets(), RATE_WINDOW as u64 * 2);
        assert_eq!(h.gap_count(), 0);
        // Rate should still be calculable
        // (all recorded near-instantly so rate is very high — just check Some)
        assert!(h.packet_rate().is_some());
    }

    #[test]
    fn health_gap_ignored_for_none_seq() {
        let mut h = DcsSessionHealth::new();
        h.record_packet(None);
        h.record_packet(None);
        h.record_packet(None);
        assert_eq!(h.gap_count(), 0);
    }

    // ── Regression tests for review fixes ────────────────────────────

    #[test]
    fn test_backoff_sub_ms_no_tight_loop() {
        let p = DcsConnectionPolicy::new(
            5,
            Duration::from_micros(500), // 0.5ms — truncates to 0ms via as_millis
            Duration::from_secs(10),
        );
        // After one failure, delay must be > 0 to avoid tight-loop retries
        let mut p = p;
        let delay = p.record_failure();
        assert!(
            !delay.is_zero(),
            "sub-ms base_delay must not produce zero delay: {delay:?}"
        );
    }

    #[test]
    fn test_backoff_clamps_attempt_zero() {
        let p = DcsConnectionPolicy::new(
            5,
            Duration::from_secs(60), // base > max
            Duration::from_secs(10),
        );
        let delay = p.current_delay();
        assert!(
            delay <= Duration::from_secs(10),
            "attempt 0 must respect max_delay cap: {delay:?}"
        );
    }

    #[test]
    fn test_packet_rate_zero_span() {
        // Directly construct health with identical timestamps to guarantee
        // a zero span, since Instant::now() may have sub-µs resolution.
        let now = Instant::now();
        let mut timestamps = [None; RATE_WINDOW];
        for slot in timestamps.iter_mut().take(10) {
            *slot = Some(now);
        }
        let h = DcsSessionHealth {
            timestamps,
            cursor: 10,
            total_packets: 10,
            last_received: Some(now),
            last_seq: None,
            gap_count: 0,
            error_count: 0,
        };
        let rate = h.packet_rate();
        assert!(
            rate.is_some(),
            "zero-span packet_rate should return Some, not None"
        );
        assert!(
            rate.unwrap().is_infinite(),
            "zero-span rate should be INFINITY, got {:?}",
            rate
        );
    }

    #[test]
    fn test_sequence_overflow_at_u64_max() {
        let mut h = DcsSessionHealth::new();
        h.record_packet(Some(u64::MAX));
        // Next packet with a higher-than-MAX seq is impossible, but ensure
        // no panic from overflow in the gap check.
        h.record_packet(Some(u64::MAX));
        assert_eq!(h.gap_count(), 0);
    }

    #[test]
    fn error_state_overwritten_on_transport_error() {
        let state = DcsConnectionState::Error("first".into());
        let next = state
            .transition(&DcsConnectionEvent::TransportError("second".into()))
            .unwrap();
        assert_eq!(next, DcsConnectionState::Error("second".into()));
    }
}
