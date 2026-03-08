// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Safety envelope and watchdog for Brunner CLS-E / CLS-P force feedback.
//!
//! Implements hardware-safe force limiting:
//!
//! - **Magnitude limiting** — clamp output to the device's safe envelope
//! - **Rate-of-change limiting** — prevent sudden force spikes
//! - **Watchdog** — immediately sets output to zero if no updates within the timeout
//! - **Emergency stop** — immediate force zero on critical faults
//!
//! All forces pass through the safety envelope before reaching the device.
//! This is the last line of defence before hardware output.

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// Default maximum force magnitude (normalised).
const DEFAULT_MAX_MAGNITUDE: f32 = 1.0;

/// Default maximum force rate-of-change per tick.
///
/// At 250 Hz, a rate limit of 0.1 per tick allows a full 0→1 ramp in ~40ms.
const DEFAULT_MAX_RATE: f32 = 0.1;

/// Default watchdog timeout.
const DEFAULT_WATCHDOG_TIMEOUT: Duration = Duration::from_millis(100);

/// Reason for an emergency stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EmergencyStopReason {
    /// Explicit user/software request.
    UserRequest,
    /// Watchdog timeout — no force update received.
    WatchdogTimeout,
    /// Over-temperature fault from device.
    OverTemperature,
    /// Over-current fault from device.
    OverCurrent,
    /// Communication loss with CLS2Sim.
    CommunicationLoss,
    /// NaN or Inf detected in force pipeline.
    InvalidValue,
}

impl std::fmt::Display for EmergencyStopReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UserRequest => f.write_str("user request"),
            Self::WatchdogTimeout => f.write_str("watchdog timeout"),
            Self::OverTemperature => f.write_str("over-temperature"),
            Self::OverCurrent => f.write_str("over-current"),
            Self::CommunicationLoss => f.write_str("communication loss"),
            Self::InvalidValue => f.write_str("invalid value (NaN/Inf)"),
        }
    }
}

/// Events emitted by the safety system for monitoring/logging.
#[derive(Debug, Clone, PartialEq)]
pub enum SafetyEvent {
    /// Force was clamped by the magnitude limiter.
    MagnitudeClamped { requested: f32, clamped: f32 },
    /// Force rate-of-change was limited.
    RateLimited { requested: f32, limited: f32 },
    /// Watchdog timed out — forces ramped to zero.
    WatchdogTriggered,
    /// Emergency stop activated.
    EmergencyStop(EmergencyStopReason),
    /// Safety system reset / re-armed.
    Reset,
}

/// Watchdog state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchdogState {
    /// Normal operation, receiving updates.
    Active,
    /// Watchdog has triggered — forces zeroed.
    Triggered,
    /// Emergency stop latched — requires explicit reset.
    EmergencyStopped,
}

/// Maximum number of safety events retained between drains.
const MAX_SAFETY_EVENTS: usize = 64;

/// Safety envelope for Brunner FFB output.
///
/// Enforces hardware-safe force limits, rate limiting, and watchdog timeouts.
/// Every force command must pass through [`SafetyEnvelope::apply`] before
/// being sent to the device.
#[derive(Debug)]
pub struct SafetyEnvelope {
    /// Maximum allowed force magnitude.
    max_magnitude: f32,
    /// Maximum force change per tick.
    max_rate: f32,
    /// Watchdog timeout duration.
    watchdog_timeout: Duration,
    /// Last applied force value.
    last_force: f32,
    /// Timestamp of the last force update.
    last_update: Option<Instant>,
    /// Current watchdog state.
    state: WatchdogState,
    /// Emergency stop reason (if latched).
    estop_reason: Option<EmergencyStopReason>,
    /// Collected safety events since last drain (bounded).
    events: Vec<SafetyEvent>,
}

impl SafetyEnvelope {
    /// Create a new safety envelope with default parameters.
    pub fn new() -> Self {
        Self {
            max_magnitude: DEFAULT_MAX_MAGNITUDE,
            max_rate: DEFAULT_MAX_RATE,
            watchdog_timeout: DEFAULT_WATCHDOG_TIMEOUT,
            last_force: 0.0,
            last_update: None,
            state: WatchdogState::Active,
            estop_reason: None,
            events: Vec::with_capacity(MAX_SAFETY_EVENTS),
        }
    }

    /// Set the maximum force magnitude (clamped to 0.0..=1.0).
    pub fn with_max_magnitude(mut self, max: f32) -> Self {
        self.max_magnitude = max.clamp(0.0, 1.0);
        self
    }

    /// Set the maximum force rate-of-change per tick (clamped to 0.001..=1.0).
    pub fn with_max_rate(mut self, rate: f32) -> Self {
        self.max_rate = rate.clamp(0.001, 1.0);
        self
    }

    /// Set the watchdog timeout.
    pub fn with_watchdog_timeout(mut self, timeout: Duration) -> Self {
        self.watchdog_timeout = timeout;
        self
    }

    /// Current watchdog state.
    pub fn state(&self) -> WatchdogState {
        self.state
    }

    /// Last applied force value.
    pub fn last_force(&self) -> f32 {
        self.last_force
    }

    /// Emergency stop reason, if latched.
    pub fn estop_reason(&self) -> Option<EmergencyStopReason> {
        self.estop_reason
    }

    /// Push a safety event, dropping oldest events when the buffer is full.
    fn push_event(&mut self, event: SafetyEvent) {
        if self.events.len() >= MAX_SAFETY_EVENTS {
            self.events.remove(0);
        }
        self.events.push(event);
    }

    /// Apply the safety envelope to a raw force command.
    ///
    /// Returns the safe force value. Use [`SafetyEnvelope::drain_events`] to
    /// retrieve any safety events that occurred.
    pub fn apply(&mut self, raw_force: f32) -> f32 {
        // Emergency stop latched — always return zero
        if self.state == WatchdogState::EmergencyStopped {
            self.last_force = 0.0;
            return 0.0;
        }

        // Check for NaN/Inf
        if !raw_force.is_finite() {
            self.emergency_stop(EmergencyStopReason::InvalidValue);
            self.last_force = 0.0;
            return 0.0;
        }

        let now = Instant::now();

        // Watchdog check
        if let Some(last) = self.last_update
            && now.duration_since(last) > self.watchdog_timeout
            && self.state == WatchdogState::Active
        {
            self.state = WatchdogState::Triggered;
            self.push_event(SafetyEvent::WatchdogTriggered);
            self.last_force = 0.0;
            self.last_update = Some(now);
            return 0.0;
        }

        // If watchdog triggered, only allow reset via `reset()`
        if self.state == WatchdogState::Triggered {
            self.last_force = 0.0;
            self.last_update = Some(now);
            return 0.0;
        }

        self.last_update = Some(now);

        // 1. Magnitude clamp
        let clamped = raw_force.clamp(-self.max_magnitude, self.max_magnitude);
        if (clamped - raw_force).abs() > 1e-6 {
            self.push_event(SafetyEvent::MagnitudeClamped {
                requested: raw_force,
                clamped,
            });
        }

        // 2. Rate-of-change limit
        let delta = clamped - self.last_force;
        let limited = if delta.abs() > self.max_rate {
            let result = self.last_force + delta.signum() * self.max_rate;
            self.push_event(SafetyEvent::RateLimited {
                requested: clamped,
                limited: result,
            });
            result
        } else {
            clamped
        };

        self.last_force = limited;
        limited
    }

    /// Trigger an emergency stop. Forces are immediately zeroed and the
    /// state is latched until [`SafetyEnvelope::reset`] is called.
    pub fn emergency_stop(&mut self, reason: EmergencyStopReason) {
        self.state = WatchdogState::EmergencyStopped;
        self.estop_reason = Some(reason);
        self.last_force = 0.0;
        self.push_event(SafetyEvent::EmergencyStop(reason));
    }

    /// Reset the safety system after an emergency stop or watchdog trigger.
    ///
    /// Forces start at zero; the watchdog timer restarts.
    pub fn reset(&mut self) {
        self.state = WatchdogState::Active;
        self.estop_reason = None;
        self.last_force = 0.0;
        self.last_update = None;
        self.push_event(SafetyEvent::Reset);
    }

    /// Drain collected safety events.
    pub fn drain_events(&mut self) -> Vec<SafetyEvent> {
        std::mem::take(&mut self.events)
    }

    /// Check if the safety system is in a healthy operational state.
    pub fn is_healthy(&self) -> bool {
        self.state == WatchdogState::Active
    }
}

impl Default for SafetyEnvelope {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_envelope() -> SafetyEnvelope {
        SafetyEnvelope::new()
            .with_max_magnitude(0.8)
            .with_max_rate(0.5)
            .with_watchdog_timeout(Duration::from_millis(200))
    }

    // ── Magnitude limiting ────────────────────────────────────────────────────

    #[test]
    fn within_envelope_passes_through() {
        let mut env = make_envelope();
        let force = env.apply(0.3);
        assert!((force - 0.3).abs() < 1e-6);
    }

    #[test]
    fn positive_over_magnitude_clamped() {
        let mut env = make_envelope();
        let force = env.apply(1.0);
        // Rate limited: from 0.0, max step 0.5 → clamped to 0.5
        // But magnitude clamp is 0.8, so requested 1.0 → clamped to 0.8 → rate: 0.5
        assert!(force <= 0.8 + 1e-6);
    }

    #[test]
    fn negative_over_magnitude_clamped() {
        let mut env = make_envelope();
        let force = env.apply(-1.0);
        assert!(force >= -0.8 - 1e-6);
    }

    #[test]
    fn magnitude_clamp_event_emitted() {
        let mut env = make_envelope();
        env.apply(1.0);
        let events = env.drain_events();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, SafetyEvent::MagnitudeClamped { .. }))
        );
    }

    // ── Rate limiting ─────────────────────────────────────────────────────────

    #[test]
    fn rate_limiting_from_zero() {
        let mut env = SafetyEnvelope::new()
            .with_max_magnitude(1.0)
            .with_max_rate(0.2);
        // Request 1.0 from 0.0 — should be rate-limited to 0.2
        let force = env.apply(1.0);
        assert!((force - 0.2).abs() < 1e-6, "expected 0.2, got {force}");
    }

    #[test]
    fn rate_limiting_incremental() {
        let mut env = SafetyEnvelope::new()
            .with_max_magnitude(1.0)
            .with_max_rate(0.3);
        let f1 = env.apply(1.0);
        assert!((f1 - 0.3).abs() < 1e-6);
        let f2 = env.apply(1.0);
        assert!((f2 - 0.6).abs() < 1e-6);
        let f3 = env.apply(1.0);
        assert!((f3 - 0.9).abs() < 1e-6);
        let f4 = env.apply(1.0);
        assert!((f4 - 1.0).abs() < 1e-6);
    }

    #[test]
    fn rate_limiting_negative_direction() {
        let mut env = SafetyEnvelope::new()
            .with_max_magnitude(1.0)
            .with_max_rate(0.2);
        let force = env.apply(-1.0);
        assert!((force - (-0.2)).abs() < 1e-6);
    }

    #[test]
    fn rate_limit_event_emitted() {
        let mut env = SafetyEnvelope::new()
            .with_max_magnitude(1.0)
            .with_max_rate(0.1);
        env.apply(1.0);
        let events = env.drain_events();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, SafetyEvent::RateLimited { .. }))
        );
    }

    // ── Watchdog ──────────────────────────────────────────────────────────────

    #[test]
    fn watchdog_triggers_after_timeout() {
        let mut env = SafetyEnvelope::new()
            .with_max_magnitude(1.0)
            .with_max_rate(1.0)
            .with_watchdog_timeout(Duration::from_millis(10));

        // First update
        env.apply(0.5);
        assert_eq!(env.state(), WatchdogState::Active);

        // Wait for timeout
        std::thread::sleep(Duration::from_millis(20));

        // Next update should trigger watchdog
        let force = env.apply(0.5);
        assert_eq!(force, 0.0);
        assert_eq!(env.state(), WatchdogState::Triggered);
    }

    #[test]
    fn watchdog_triggered_returns_zero() {
        let mut env = SafetyEnvelope::new()
            .with_max_magnitude(1.0)
            .with_max_rate(1.0)
            .with_watchdog_timeout(Duration::from_millis(10));

        env.apply(0.5);
        std::thread::sleep(Duration::from_millis(20));
        env.apply(0.5); // triggers watchdog

        // Subsequent calls also return zero
        let force = env.apply(0.8);
        assert_eq!(force, 0.0);
    }

    #[test]
    fn watchdog_reset_re_enables() {
        let mut env = SafetyEnvelope::new()
            .with_max_magnitude(1.0)
            .with_max_rate(1.0)
            .with_watchdog_timeout(Duration::from_millis(10));

        env.apply(0.5);
        std::thread::sleep(Duration::from_millis(20));
        env.apply(0.5); // triggers
        assert_eq!(env.state(), WatchdogState::Triggered);

        env.reset();
        assert_eq!(env.state(), WatchdogState::Active);
        let force = env.apply(0.3);
        assert!((force - 0.3).abs() < 1e-6);
    }

    // ── Emergency stop ────────────────────────────────────────────────────────

    #[test]
    fn emergency_stop_zeros_force() {
        let mut env = make_envelope();
        env.apply(0.5);
        env.emergency_stop(EmergencyStopReason::UserRequest);
        assert_eq!(env.last_force(), 0.0);
        assert_eq!(env.state(), WatchdogState::EmergencyStopped);
    }

    #[test]
    fn emergency_stop_latches() {
        let mut env = make_envelope();
        env.emergency_stop(EmergencyStopReason::OverTemperature);
        // Apply should still return 0
        let force = env.apply(0.5);
        assert_eq!(force, 0.0);
    }

    #[test]
    fn emergency_stop_reason_stored() {
        let mut env = make_envelope();
        env.emergency_stop(EmergencyStopReason::OverCurrent);
        assert_eq!(env.estop_reason(), Some(EmergencyStopReason::OverCurrent));
    }

    #[test]
    fn emergency_stop_reset_clears() {
        let mut env = make_envelope();
        env.emergency_stop(EmergencyStopReason::UserRequest);
        env.reset();
        assert_eq!(env.state(), WatchdogState::Active);
        assert_eq!(env.estop_reason(), None);
    }

    #[test]
    fn emergency_stop_event_emitted() {
        let mut env = make_envelope();
        env.emergency_stop(EmergencyStopReason::CommunicationLoss);
        let events = env.drain_events();
        assert!(events.iter().any(|e| matches!(
            e,
            SafetyEvent::EmergencyStop(EmergencyStopReason::CommunicationLoss)
        )));
    }

    // ── NaN / Inf handling ────────────────────────────────────────────────────

    #[test]
    fn nan_triggers_emergency_stop() {
        let mut env = make_envelope();
        let force = env.apply(f32::NAN);
        assert_eq!(force, 0.0);
        assert_eq!(env.state(), WatchdogState::EmergencyStopped);
        assert_eq!(env.estop_reason(), Some(EmergencyStopReason::InvalidValue));
    }

    #[test]
    fn infinity_triggers_emergency_stop() {
        let mut env = make_envelope();
        let force = env.apply(f32::INFINITY);
        assert_eq!(force, 0.0);
        assert_eq!(env.state(), WatchdogState::EmergencyStopped);
    }

    #[test]
    fn neg_infinity_triggers_emergency_stop() {
        let mut env = make_envelope();
        let force = env.apply(f32::NEG_INFINITY);
        assert_eq!(force, 0.0);
        assert_eq!(env.state(), WatchdogState::EmergencyStopped);
    }

    // ── Misc ──────────────────────────────────────────────────────────────────

    #[test]
    fn is_healthy_when_active() {
        let env = make_envelope();
        assert!(env.is_healthy());
    }

    #[test]
    fn not_healthy_when_estopped() {
        let mut env = make_envelope();
        env.emergency_stop(EmergencyStopReason::UserRequest);
        assert!(!env.is_healthy());
    }

    #[test]
    fn default_envelope_is_healthy() {
        let env = SafetyEnvelope::default();
        assert!(env.is_healthy());
        assert_eq!(env.last_force(), 0.0);
    }

    #[test]
    fn drain_events_clears() {
        let mut env = make_envelope();
        env.apply(1.0); // should generate events
        let events = env.drain_events();
        assert!(!events.is_empty());
        let events2 = env.drain_events();
        assert!(events2.is_empty());
    }

    #[test]
    fn estop_reason_display() {
        assert_eq!(EmergencyStopReason::UserRequest.to_string(), "user request");
        assert_eq!(
            EmergencyStopReason::WatchdogTimeout.to_string(),
            "watchdog timeout"
        );
        assert_eq!(
            EmergencyStopReason::OverTemperature.to_string(),
            "over-temperature"
        );
        assert_eq!(
            EmergencyStopReason::InvalidValue.to_string(),
            "invalid value (NaN/Inf)"
        );
    }

    // ── Property tests ────────────────────────────────────────────────────────

    mod prop {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn force_always_within_envelope(
                raw in -5.0f32..=5.0,
                max_mag in 0.1f32..=1.0,
            ) {
                let mut env = SafetyEnvelope::new()
                    .with_max_magnitude(max_mag)
                    .with_max_rate(1.0); // no rate limit
                let force = env.apply(raw);
                prop_assert!(force.abs() <= max_mag + 1e-6,
                    "force {} exceeds max_magnitude {}", force, max_mag);
            }

            #[test]
            fn rate_limited_force_always_within_step(
                raw in -2.0f32..=2.0,
                rate in 0.01f32..=1.0,
            ) {
                let mut env = SafetyEnvelope::new()
                    .with_max_magnitude(2.0)
                    .with_max_rate(rate);
                let force = env.apply(raw);
                // From 0, max step is rate
                prop_assert!(force.abs() <= rate + 1e-6,
                    "force {} exceeds rate limit {} from zero", force, rate);
            }

            #[test]
            fn nan_inf_never_passes_through(
                raw in proptest::bool::ANY,
            ) {
                let mut env = SafetyEnvelope::new();
                let input = if raw { f32::NAN } else { f32::INFINITY };
                let force = env.apply(input);
                prop_assert!(force.is_finite(), "non-finite force leaked through");
                prop_assert_eq!(force, 0.0);
            }
        }
    }
}
