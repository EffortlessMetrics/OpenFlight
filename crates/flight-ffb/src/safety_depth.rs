// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Deepened FFB safety interlocks: violation tracking, rate limiting, and watchdog
//!
//! All structures in this module are **zero-allocation on hot paths** —
//! fixed-size arrays, no `Vec`, no `String`, no heap allocations after
//! construction. Suitable for the 250 Hz RT spine.
//!
//! ## Components
//!
//! - [`EnvelopeViolation`] — records when forces exceed envelope limits
//! - [`ViolationTracker`] — circular buffer of recent violations with severity
//! - [`ForceRateLimiter`] — limits rate of change of force output (N/s)
//! - [`WatchdogTimer`] — ramps forces to zero if no valid command arrives in time
//! - [`SafetyReport`] — aggregated snapshot of all safety subsystem states
//!
//! **Validates: ADR-009 Safety Interlock Design, QG-FFB-SAFETY**

use std::time::Instant;

// ─── EnvelopeViolation ───────────────────────────────────────────────────────

/// Classification of how the envelope was violated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViolationType {
    /// Requested force magnitude exceeded the envelope limit.
    Magnitude,
    /// Rate of change of force exceeded the allowed slew rate.
    RateOfChange,
    /// Force was held at the limit for longer than the allowed duration.
    Duration,
}

/// A single recorded envelope violation.
///
/// Fixed-size, `Copy`, no heap — safe for RT circular buffers.
#[derive(Debug, Clone, Copy)]
pub struct EnvelopeViolation {
    /// Which axis triggered the violation (0-based index).
    pub axis_id: u8,
    /// The force that was requested (N or normalised, depending on context).
    pub requested_force: f32,
    /// The envelope limit that was active at the time.
    pub limit: f32,
    /// When the violation occurred.
    pub timestamp: Instant,
    /// What kind of violation this is.
    pub violation_type: ViolationType,
}

// ─── ViolationTracker ────────────────────────────────────────────────────────

/// Maximum number of violations kept in the circular buffer.
const MAX_VIOLATIONS: usize = 64;

/// Maximum number of independently tracked axes.
const MAX_AXES: usize = 8;

/// Circular buffer of recent [`EnvelopeViolation`]s with per-axis counting.
///
/// All state is stack-resident — no heap allocations.
pub struct ViolationTracker {
    /// Ring buffer storage.
    buf: [Option<EnvelopeViolation>; MAX_VIOLATIONS],
    /// Write cursor (wraps around).
    head: usize,
    /// Total violations recorded (saturates at `u64::MAX`).
    total_count: u64,
    /// Per-axis violation count (indexed by `axis_id`).
    per_axis_count: [u64; MAX_AXES],
    /// Configurable window: violations older than this are ignored in severity.
    window: std::time::Duration,
}

impl ViolationTracker {
    /// Create a new tracker with the given severity window.
    pub fn new(window: std::time::Duration) -> Self {
        Self {
            buf: [None; MAX_VIOLATIONS],
            head: 0,
            total_count: 0,
            per_axis_count: [0; MAX_AXES],
            window,
        }
    }

    /// Record a violation. Overwrites the oldest entry when full.
    pub fn record(&mut self, violation: EnvelopeViolation) {
        self.buf[self.head] = Some(violation);
        self.head = (self.head + 1) % MAX_VIOLATIONS;
        self.total_count = self.total_count.saturating_add(1);
        if (violation.axis_id as usize) < MAX_AXES {
            self.per_axis_count[violation.axis_id as usize] =
                self.per_axis_count[violation.axis_id as usize].saturating_add(1);
        }
    }

    /// Number of violations inside the current window.
    pub fn recent_count(&self, now: Instant) -> usize {
        let cutoff = now.checked_sub(self.window).unwrap_or(now);
        self.buf
            .iter()
            .filter(|slot| slot.as_ref().is_some_and(|v| v.timestamp >= cutoff))
            .count()
    }

    /// Severity score: recent violations weighted by how far they exceeded the limit.
    ///
    /// Returns a value ≥ 0.0 where higher means more severe.
    pub fn severity(&self, now: Instant) -> f32 {
        let cutoff = now.checked_sub(self.window).unwrap_or(now);
        self.buf
            .iter()
            .filter_map(|slot| slot.as_ref())
            .filter(|v| v.timestamp >= cutoff)
            .map(|v| {
                let overshoot = (v.requested_force.abs() - v.limit.abs()).max(0.0);
                overshoot / v.limit.abs().max(f32::EPSILON)
            })
            .sum()
    }

    /// Total violations ever recorded.
    pub fn total_count(&self) -> u64 {
        self.total_count
    }

    /// Per-axis violation count (returns 0 for out-of-range axis).
    pub fn axis_count(&self, axis_id: u8) -> u64 {
        if (axis_id as usize) < MAX_AXES {
            self.per_axis_count[axis_id as usize]
        } else {
            0
        }
    }

    /// Most recent violation, if any.
    pub fn last_violation(&self) -> Option<&EnvelopeViolation> {
        // head points to the *next* write slot, so the most recent is head-1
        let idx = if self.head == 0 {
            MAX_VIOLATIONS - 1
        } else {
            self.head - 1
        };
        self.buf[idx].as_ref()
    }
}

// ─── ForceRateLimiter ────────────────────────────────────────────────────────

/// Per-axis state for the rate limiter.
#[derive(Debug, Clone, Copy)]
struct RateLimiterAxisState {
    last_force: f32,
    initialised: bool,
}

/// Limits the rate of change of force output (N/s) to prevent sudden jolts.
///
/// Pre-allocated fixed-size state for up to [`MAX_AXES`] axes.
/// Hot-path method ([`limit`]) performs no allocations.
pub struct ForceRateLimiter {
    /// Maximum allowed rate of change in force-units per second.
    max_rate: f32,
    /// Per-axis last-force state.
    axes: [RateLimiterAxisState; MAX_AXES],
}

impl ForceRateLimiter {
    /// Create a rate limiter that allows at most `max_rate` force-units/s.
    ///
    /// `max_rate` must be positive and finite.
    pub fn new(max_rate: f32) -> Self {
        assert!(
            max_rate > 0.0 && max_rate.is_finite(),
            "max_rate must be positive and finite"
        );
        Self {
            max_rate,
            axes: [RateLimiterAxisState {
                last_force: 0.0,
                initialised: false,
            }; MAX_AXES],
        }
    }

    /// Apply rate limiting to `desired` force for `axis_id` with timestep `dt_s`.
    ///
    /// Returns the (potentially clamped) output force. Zero-allocation.
    pub fn limit(&mut self, axis_id: u8, desired: f32, dt_s: f32) -> f32 {
        let idx = axis_id as usize;
        if idx >= MAX_AXES {
            return desired;
        }

        // Defensive: reject bad timing inputs — hold last known good value
        if !dt_s.is_finite() || dt_s <= 0.0 {
            return self.axes[idx].last_force;
        }
        if !desired.is_finite() {
            return self.axes[idx].last_force;
        }

        let state = &mut self.axes[idx];
        if !state.initialised {
            state.last_force = desired;
            state.initialised = true;
            return desired;
        }

        let max_delta = self.max_rate * dt_s;
        let delta = desired - state.last_force;
        let clamped_delta = delta.clamp(-max_delta, max_delta);
        let output = state.last_force + clamped_delta;
        state.last_force = output;
        output
    }

    /// Read last output for a given axis (useful for diagnostics).
    pub fn last_force(&self, axis_id: u8) -> f32 {
        let idx = axis_id as usize;
        if idx >= MAX_AXES {
            return 0.0;
        }
        self.axes[idx].last_force
    }

    /// Reset all axis state (e.g. after device reconnect).
    pub fn reset(&mut self) {
        for s in &mut self.axes {
            s.last_force = 0.0;
            s.initialised = false;
        }
    }
}

// ─── WatchdogTimer ───────────────────────────────────────────────────────────

/// Watchdog state machine phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchdogState {
    /// Normal operation — commands are arriving on time.
    Active,
    /// Deadline missed — force is being ramped to zero.
    RampingDown,
    /// Ramp complete — force is held at zero until reset.
    Stopped,
}

/// Dead-command watchdog timer.
///
/// If no valid FFB command arrives within `deadline`, the watchdog
/// automatically ramps forces to zero over `ramp_duration`.
///
/// **Zero heap allocations.** All state is inline.
pub struct WatchdogTimer {
    /// Maximum time between valid commands.
    deadline: std::time::Duration,
    /// Duration over which force ramps to zero after deadline miss.
    ramp_duration: std::time::Duration,
    /// Timestamp of the last valid command.
    last_command: Instant,
    /// Current state.
    state: WatchdogState,
    /// Timestamp when ramp-down started (only valid in `RampingDown`).
    ramp_start: Instant,
}

impl WatchdogTimer {
    /// Create a watchdog with the given `deadline` and `ramp_duration`.
    pub fn new(deadline: std::time::Duration, ramp_duration: std::time::Duration) -> Self {
        assert!(
            !ramp_duration.is_zero(),
            "ramp_duration must be positive"
        );
        let now = Instant::now();
        Self {
            deadline,
            ramp_duration,
            last_command: now,
            state: WatchdogState::Active,
            ramp_start: now,
        }
    }

    /// Notify the watchdog that a valid command has been received at `now`.
    pub fn feed_at(&mut self, now: Instant) {
        self.last_command = now;
        self.state = WatchdogState::Active;
    }

    /// Convenience wrapper — calls [`feed_at`](Self::feed_at) with `Instant::now()`.
    pub fn feed(&mut self) {
        self.feed_at(Instant::now());
    }

    /// Evaluate the watchdog at `now` and return a force multiplier in `[0.0, 1.0]`.
    ///
    /// Call this every tick. When the watchdog is active the multiplier is 1.0.
    /// During ramp-down it decreases linearly to 0.0. After the ramp it stays
    /// at 0.0 until [`feed`](Self::feed) is called.
    pub fn evaluate_at(&mut self, now: Instant) -> f32 {
        match self.state {
            WatchdogState::Active => {
                if now.duration_since(self.last_command) >= self.deadline {
                    // Deadline missed — begin ramp-down.
                    self.state = WatchdogState::RampingDown;
                    self.ramp_start = now;
                    // First tick of ramp — still at 1.0 (ramp has just started).
                    1.0
                } else {
                    1.0
                }
            }
            WatchdogState::RampingDown => {
                let elapsed = now.duration_since(self.ramp_start);
                if elapsed >= self.ramp_duration {
                    self.state = WatchdogState::Stopped;
                    0.0
                } else {
                    let ramp_secs = self.ramp_duration.as_secs_f32();
                    // Defensive: zero ramp duration → instant cutoff
                    if ramp_secs <= 0.0 {
                        self.state = WatchdogState::Stopped;
                        return 0.0;
                    }
                    let progress = elapsed.as_secs_f32() / ramp_secs;
                    1.0 - progress
                }
            }
            WatchdogState::Stopped => 0.0,
        }
    }

    /// Convenience wrapper — calls [`evaluate_at`](Self::evaluate_at) with `Instant::now()`.
    pub fn evaluate(&mut self) -> f32 {
        self.evaluate_at(Instant::now())
    }

    /// Current watchdog state.
    pub fn state(&self) -> WatchdogState {
        self.state
    }

    /// Whether the watchdog has tripped (is ramping or stopped).
    pub fn is_tripped(&self) -> bool {
        self.state != WatchdogState::Active
    }
}

// ─── SafetyReport ────────────────────────────────────────────────────────────

/// Aggregated snapshot of the safety subsystem state.
///
/// Intended for diagnostics, logging, and UI display. Copy-able, no heap.
#[derive(Debug, Clone, Copy)]
pub struct SafetyReport {
    /// Total envelope violations ever recorded.
    pub violation_count: u64,
    /// Number of recent violations (inside the window).
    pub recent_violation_count: usize,
    /// Severity score of recent violations.
    pub severity: f32,
    /// Timestamp of the most recent violation, if any.
    pub last_violation_time: Option<Instant>,
    /// Whether the rate limiter is currently active (clamping deltas).
    pub rate_limiting_active: bool,
    /// Current watchdog state.
    pub watchdog_state: WatchdogState,
}

impl SafetyReport {
    /// Build a report from the current state of each subsystem.
    pub fn from_state(
        tracker: &ViolationTracker,
        rate_limiter: &ForceRateLimiter,
        watchdog: &WatchdogTimer,
        now: Instant,
    ) -> Self {
        // Rate limiter is considered "active" if any axis has been initialised
        // (meaning it could be clamping). For a more precise check the caller
        // should compare desired vs actual output, but this gives a quick flag.
        let rate_limiting_active = rate_limiter.axes.iter().any(|s| s.initialised);

        Self {
            violation_count: tracker.total_count(),
            recent_violation_count: tracker.recent_count(now),
            severity: tracker.severity(now),
            last_violation_time: tracker.last_violation().map(|v| v.timestamp),
            rate_limiting_active,
            watchdog_state: watchdog.state(),
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    // ── helpers ──────────────────────────────────────────────────────────

    fn make_violation(
        axis_id: u8,
        requested: f32,
        limit: f32,
        vtype: ViolationType,
    ) -> EnvelopeViolation {
        EnvelopeViolation {
            axis_id,
            requested_force: requested,
            limit,
            timestamp: Instant::now(),
            violation_type: vtype,
        }
    }

    fn make_violation_at(
        axis_id: u8,
        requested: f32,
        limit: f32,
        vtype: ViolationType,
        ts: Instant,
    ) -> EnvelopeViolation {
        EnvelopeViolation {
            axis_id,
            requested_force: requested,
            limit,
            timestamp: ts,
            violation_type: vtype,
        }
    }

    // =====================================================================
    //  EnvelopeViolation detection
    // =====================================================================

    #[test]
    fn violation_magnitude_detected() {
        let v = make_violation(0, 12.0, 10.0, ViolationType::Magnitude);
        assert_eq!(v.violation_type, ViolationType::Magnitude);
        assert!(v.requested_force > v.limit);
    }

    #[test]
    fn violation_rate_of_change_detected() {
        let v = make_violation(1, 8.0, 5.0, ViolationType::RateOfChange);
        assert_eq!(v.violation_type, ViolationType::RateOfChange);
    }

    #[test]
    fn violation_duration_detected() {
        let v = make_violation(2, 10.0, 10.0, ViolationType::Duration);
        assert_eq!(v.violation_type, ViolationType::Duration);
    }

    // =====================================================================
    //  ViolationTracker
    // =====================================================================

    #[test]
    fn tracker_records_and_counts() {
        let mut tracker = ViolationTracker::new(Duration::from_secs(10));
        assert_eq!(tracker.total_count(), 0);

        tracker.record(make_violation(0, 12.0, 10.0, ViolationType::Magnitude));
        assert_eq!(tracker.total_count(), 1);
        assert_eq!(tracker.axis_count(0), 1);

        tracker.record(make_violation(0, 15.0, 10.0, ViolationType::Magnitude));
        assert_eq!(tracker.total_count(), 2);
        assert_eq!(tracker.axis_count(0), 2);
    }

    #[test]
    fn tracker_recent_count_inside_window() {
        let mut tracker = ViolationTracker::new(Duration::from_secs(10));
        tracker.record(make_violation(0, 12.0, 10.0, ViolationType::Magnitude));
        tracker.record(make_violation(0, 11.0, 10.0, ViolationType::Magnitude));

        let now = Instant::now();
        assert_eq!(tracker.recent_count(now), 2);
    }

    #[test]
    fn tracker_window_eviction() {
        let mut tracker = ViolationTracker::new(Duration::from_millis(50));

        // Record a violation in the past
        let old_ts = Instant::now() - Duration::from_millis(100);
        tracker.record(make_violation_at(
            0,
            12.0,
            10.0,
            ViolationType::Magnitude,
            old_ts,
        ));

        // Record a fresh one
        tracker.record(make_violation(0, 11.0, 10.0, ViolationType::Magnitude));

        let now = Instant::now();
        // Only the fresh one should be inside the window
        assert_eq!(tracker.recent_count(now), 1);
        // But total count is still 2
        assert_eq!(tracker.total_count(), 2);
    }

    #[test]
    fn tracker_circular_buffer_wraps() {
        let mut tracker = ViolationTracker::new(Duration::from_secs(60));
        // Fill more than MAX_VIOLATIONS
        for i in 0..(MAX_VIOLATIONS + 10) {
            tracker.record(make_violation(
                0,
                12.0 + i as f32,
                10.0,
                ViolationType::Magnitude,
            ));
        }
        assert_eq!(tracker.total_count(), (MAX_VIOLATIONS + 10) as u64);
        // Buffer should still function; recent_count is capped at MAX_VIOLATIONS
        let now = Instant::now();
        assert!(tracker.recent_count(now) <= MAX_VIOLATIONS);
    }

    #[test]
    fn tracker_severity_calculation() {
        let mut tracker = ViolationTracker::new(Duration::from_secs(10));
        // 12.0 requested vs 10.0 limit → overshoot 2.0, relative = 2.0/10.0 = 0.2
        tracker.record(make_violation(0, 12.0, 10.0, ViolationType::Magnitude));
        let sev = tracker.severity(Instant::now());
        assert!((sev - 0.2).abs() < 0.01, "expected ~0.2, got {sev}");
    }

    #[test]
    fn tracker_multiple_axes_independent() {
        let mut tracker = ViolationTracker::new(Duration::from_secs(10));
        tracker.record(make_violation(0, 12.0, 10.0, ViolationType::Magnitude));
        tracker.record(make_violation(1, 15.0, 10.0, ViolationType::Magnitude));
        tracker.record(make_violation(1, 16.0, 10.0, ViolationType::Magnitude));

        assert_eq!(tracker.axis_count(0), 1);
        assert_eq!(tracker.axis_count(1), 2);
        assert_eq!(tracker.axis_count(2), 0);
        assert_eq!(tracker.total_count(), 3);
    }

    #[test]
    fn tracker_last_violation_returns_most_recent() {
        let mut tracker = ViolationTracker::new(Duration::from_secs(10));
        tracker.record(make_violation(0, 12.0, 10.0, ViolationType::Magnitude));
        tracker.record(make_violation(1, 20.0, 10.0, ViolationType::RateOfChange));

        let last = tracker.last_violation().expect("should have a violation");
        assert_eq!(last.axis_id, 1);
        assert_eq!(last.violation_type, ViolationType::RateOfChange);
    }

    // =====================================================================
    //  ForceRateLimiter
    // =====================================================================

    #[test]
    fn rate_limiter_smooths_sudden_change() {
        let mut limiter = ForceRateLimiter::new(50.0); // 50 N/s max
        let dt = 0.004; // 250 Hz

        // First call initialises; second call should be rate-limited
        let _ = limiter.limit(0, 0.0, dt);
        let output = limiter.limit(0, 10.0, dt);

        // max_delta = 50 * 0.004 = 0.2
        assert!((output - 0.2).abs() < 0.001, "expected 0.2, got {output}");
    }

    #[test]
    fn rate_limiter_allows_gradual_change() {
        let mut limiter = ForceRateLimiter::new(1000.0); // generous rate
        let dt = 0.004;

        let _ = limiter.limit(0, 0.0, dt);
        let output = limiter.limit(0, 0.5, dt);

        // max_delta = 1000 * 0.004 = 4.0 — so 0.5 delta is fine
        assert!((output - 0.5).abs() < 0.001, "expected 0.5, got {output}");
    }

    #[test]
    fn rate_limiter_multiple_axes_independent() {
        let mut limiter = ForceRateLimiter::new(50.0);
        let dt = 0.004;

        let _ = limiter.limit(0, 0.0, dt);
        let _ = limiter.limit(1, 5.0, dt); // axis 1 starts at 5.0

        let out0 = limiter.limit(0, 10.0, dt); // limited from 0
        let out1 = limiter.limit(1, 10.0, dt); // limited from 5

        // Axis 0: max delta = 0.2, output = 0.2
        assert!(
            (out0 - 0.2).abs() < 0.001,
            "axis 0: expected 0.2, got {out0}"
        );
        // Axis 1: max delta = 0.2, output = 5.2
        assert!(
            (out1 - 5.2).abs() < 0.001,
            "axis 1: expected 5.2, got {out1}"
        );
    }

    #[test]
    fn rate_limiter_reset_clears_state() {
        let mut limiter = ForceRateLimiter::new(50.0);
        let dt = 0.004;

        let _ = limiter.limit(0, 5.0, dt);
        assert!((limiter.last_force(0) - 5.0).abs() < 0.001);

        limiter.reset();
        assert!((limiter.last_force(0)).abs() < 0.001);
    }

    // =====================================================================
    //  WatchdogTimer
    // =====================================================================

    #[test]
    fn watchdog_active_when_fed() {
        let mut wd = WatchdogTimer::new(Duration::from_millis(100), Duration::from_millis(50));
        wd.feed();
        let mult = wd.evaluate();
        assert!((mult - 1.0).abs() < 0.001);
        assert_eq!(wd.state(), WatchdogState::Active);
    }

    #[test]
    fn watchdog_triggers_on_timeout() {
        let mut wd = WatchdogTimer::new(Duration::from_millis(10), Duration::from_millis(50));
        // Let the deadline pass
        std::thread::sleep(Duration::from_millis(20));
        let _mult = wd.evaluate();
        // Should have entered ramp-down (first tick returns 1.0 then ramps)
        assert!(wd.is_tripped());
    }

    #[test]
    fn watchdog_resets_on_feed() {
        let mut wd = WatchdogTimer::new(Duration::from_millis(10), Duration::from_millis(50));
        std::thread::sleep(Duration::from_millis(20));
        let _ = wd.evaluate();
        assert!(wd.is_tripped());

        wd.feed();
        assert!(!wd.is_tripped());
        assert_eq!(wd.state(), WatchdogState::Active);
    }

    #[test]
    fn watchdog_ramps_to_zero() {
        let mut wd = WatchdogTimer::new(
            Duration::from_millis(1),  // very short deadline
            Duration::from_millis(20), // short ramp
        );
        std::thread::sleep(Duration::from_millis(5));

        // Trigger ramp-down
        let _ = wd.evaluate();
        assert!(wd.is_tripped());

        // Wait for ramp to finish
        std::thread::sleep(Duration::from_millis(30));
        let mult = wd.evaluate();
        assert!(
            (mult - 0.0).abs() < 0.001,
            "expected 0.0 after ramp, got {mult}"
        );
        assert_eq!(wd.state(), WatchdogState::Stopped);
    }

    #[test]
    fn watchdog_stays_stopped_until_feed() {
        let mut wd = WatchdogTimer::new(Duration::from_millis(1), Duration::from_millis(1));
        std::thread::sleep(Duration::from_millis(10));
        let _ = wd.evaluate(); // trigger
        std::thread::sleep(Duration::from_millis(10));
        let _ = wd.evaluate(); // should be stopped

        assert_eq!(wd.state(), WatchdogState::Stopped);
        let mult = wd.evaluate();
        assert!((mult - 0.0).abs() < 0.001);

        // Now feed
        wd.feed();
        assert_eq!(wd.state(), WatchdogState::Active);
        let mult = wd.evaluate();
        assert!((mult - 1.0).abs() < 0.001);
    }

    // =====================================================================
    //  SafetyReport
    // =====================================================================

    #[test]
    fn safety_report_aggregation() {
        let mut tracker = ViolationTracker::new(Duration::from_secs(10));
        tracker.record(make_violation(0, 12.0, 10.0, ViolationType::Magnitude));
        tracker.record(make_violation(1, 15.0, 10.0, ViolationType::RateOfChange));

        let mut limiter = ForceRateLimiter::new(50.0);
        let _ = limiter.limit(0, 1.0, 0.004);

        let wd = WatchdogTimer::new(Duration::from_secs(1), Duration::from_millis(50));

        let now = Instant::now();
        let report = SafetyReport::from_state(&tracker, &limiter, &wd, now);

        assert_eq!(report.violation_count, 2);
        assert_eq!(report.recent_violation_count, 2);
        assert!(report.severity > 0.0);
        assert!(report.last_violation_time.is_some());
        assert!(report.rate_limiting_active);
        assert_eq!(report.watchdog_state, WatchdogState::Active);
    }

    #[test]
    fn safety_report_with_tripped_watchdog() {
        let tracker = ViolationTracker::new(Duration::from_secs(10));
        let limiter = ForceRateLimiter::new(50.0);
        let mut wd = WatchdogTimer::new(Duration::from_millis(1), Duration::from_millis(1));

        std::thread::sleep(Duration::from_millis(10));
        let _ = wd.evaluate();
        std::thread::sleep(Duration::from_millis(10));
        let _ = wd.evaluate();

        let now = Instant::now();
        let report = SafetyReport::from_state(&tracker, &limiter, &wd, now);

        assert_eq!(report.violation_count, 0);
        assert_eq!(report.watchdog_state, WatchdogState::Stopped);
    }

    // =====================================================================
    //  Edge-case safety tests (NaN / Inf / bad inputs)
    // =====================================================================

    #[test]
    fn test_rate_limiter_nan_dt() {
        let mut limiter = ForceRateLimiter::new(50.0);
        let _ = limiter.limit(0, 5.0, 0.004); // initialise
        let output = limiter.limit(0, 10.0, f32::NAN);
        assert!(
            (output - 5.0).abs() < 0.001,
            "NaN dt_s should hold last output (5.0), got {output}"
        );
    }

    #[test]
    fn test_rate_limiter_negative_dt() {
        let mut limiter = ForceRateLimiter::new(50.0);
        let _ = limiter.limit(0, 5.0, 0.004);
        let output = limiter.limit(0, 10.0, -0.004);
        assert!(
            (output - 5.0).abs() < 0.001,
            "negative dt_s should hold last output (5.0), got {output}"
        );
    }

    #[test]
    fn test_rate_limiter_infinite_dt() {
        let mut limiter = ForceRateLimiter::new(50.0);
        let _ = limiter.limit(0, 5.0, 0.004);
        let output = limiter.limit(0, 10.0, f32::INFINITY);
        assert!(
            (output - 5.0).abs() < 0.001,
            "infinite dt_s should hold last output (5.0), got {output}"
        );
    }

    #[test]
    fn test_rate_limiter_nan_desired() {
        let mut limiter = ForceRateLimiter::new(50.0);
        let _ = limiter.limit(0, 5.0, 0.004);
        let output = limiter.limit(0, f32::NAN, 0.004);
        assert!(
            (output - 5.0).abs() < 0.001,
            "NaN desired should hold last output (5.0), got {output}"
        );
    }

    #[test]
    #[should_panic(expected = "ramp_duration must be positive")]
    fn test_watchdog_zero_ramp_panics() {
        let _wd = WatchdogTimer::new(Duration::from_millis(1), Duration::ZERO);
    }
}
