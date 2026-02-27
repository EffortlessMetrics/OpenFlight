// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Circuit breaker for preventing cascading failures.
//!
//! Implements a three-state circuit breaker (Closed → Open → HalfOpen) that
//! stops calling a failing downstream service until it has had time to recover.

use std::time::{Duration, Instant};

/// Circuit breaker states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation — calls pass through.
    Closed,
    /// Failing — calls are rejected.
    Open,
    /// Testing recovery — a limited number of calls pass through.
    HalfOpen,
}

/// Configuration for a circuit breaker.
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures before opening the circuit.
    pub failure_threshold: u32,
    /// Number of consecutive successes in half-open before closing.
    pub success_threshold: u32,
    /// Time to wait in open state before transitioning to half-open.
    pub timeout: Duration,
}

/// Circuit breaker for preventing cascading failures.
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: CircuitState,
    failure_count: u32,
    success_count: u32,
    last_failure_time: Option<Instant>,
    total_calls: u64,
    total_rejections: u64,
}

/// Result of checking whether a call is allowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallResult {
    /// The call may proceed.
    Allowed,
    /// The call was rejected by the circuit breaker.
    Rejected,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with the given configuration.
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: CircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            last_failure_time: None,
            total_calls: 0,
            total_rejections: 0,
        }
    }

    /// Check whether a call should proceed.
    ///
    /// In the `Open` state the breaker automatically transitions to `HalfOpen`
    /// once the configured timeout has elapsed.
    pub fn call_allowed(&mut self) -> CallResult {
        self.total_calls += 1;

        match self.state {
            CircuitState::Closed | CircuitState::HalfOpen => CallResult::Allowed,
            CircuitState::Open => {
                if self
                    .last_failure_time
                    .is_some_and(|last| last.elapsed() >= self.config.timeout)
                {
                    self.state = CircuitState::HalfOpen;
                    self.success_count = 0;
                    return CallResult::Allowed;
                }
                self.total_rejections += 1;
                CallResult::Rejected
            }
        }
    }

    /// Record a successful call.
    pub fn record_success(&mut self) {
        match self.state {
            CircuitState::HalfOpen => {
                self.success_count += 1;
                if self.success_count >= self.config.success_threshold {
                    self.state = CircuitState::Closed;
                    self.failure_count = 0;
                    self.success_count = 0;
                }
            }
            CircuitState::Closed => {
                self.failure_count = 0;
            }
            CircuitState::Open => {}
        }
    }

    /// Record a failed call.
    pub fn record_failure(&mut self) {
        self.last_failure_time = Some(Instant::now());

        match self.state {
            CircuitState::Closed => {
                self.failure_count += 1;
                self.success_count = 0;
                if self.failure_count >= self.config.failure_threshold {
                    self.state = CircuitState::Open;
                }
            }
            CircuitState::HalfOpen => {
                self.state = CircuitState::Open;
                self.success_count = 0;
            }
            CircuitState::Open => {}
        }
    }

    /// Return the current state.
    pub fn state(&self) -> CircuitState {
        self.state
    }

    /// Return the current failure count.
    pub fn failure_count(&self) -> u32 {
        self.failure_count
    }

    /// Force-close the circuit breaker, resetting all counters.
    pub fn reset(&mut self) {
        self.state = CircuitState::Closed;
        self.failure_count = 0;
        self.success_count = 0;
        self.last_failure_time = None;
    }

    /// Total number of calls (allowed + rejected) made through this breaker.
    pub fn total_calls(&self) -> u64 {
        self.total_calls
    }

    /// Total number of calls that were rejected.
    pub fn total_rejections(&self) -> u64 {
        self.total_rejections
    }

    /// Fraction of calls that were rejected (0.0–1.0).
    ///
    /// Returns 0.0 when no calls have been made.
    pub fn rejection_rate(&self) -> f64 {
        if self.total_calls == 0 {
            return 0.0;
        }
        self.total_rejections as f64 / self.total_calls as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> CircuitBreakerConfig {
        CircuitBreakerConfig {
            failure_threshold: 3,
            success_threshold: 2,
            timeout: Duration::from_millis(100),
        }
    }

    #[test]
    fn initial_state_is_closed() {
        let cb = CircuitBreaker::new(default_config());
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn calls_allowed_in_closed_state() {
        let mut cb = CircuitBreaker::new(default_config());
        assert_eq!(cb.call_allowed(), CallResult::Allowed);
        assert_eq!(cb.call_allowed(), CallResult::Allowed);
    }

    #[test]
    fn failures_increment_count() {
        let mut cb = CircuitBreaker::new(default_config());
        cb.record_failure();
        assert_eq!(cb.failure_count(), 1);
        cb.record_failure();
        assert_eq!(cb.failure_count(), 2);
    }

    #[test]
    fn threshold_reached_opens_circuit() {
        let mut cb = CircuitBreaker::new(default_config());
        for _ in 0..3 {
            cb.record_failure();
        }
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn open_circuit_rejects_calls() {
        let mut cb = CircuitBreaker::new(default_config());
        for _ in 0..3 {
            cb.record_failure();
        }
        assert_eq!(cb.call_allowed(), CallResult::Rejected);
    }

    #[test]
    fn timeout_transitions_to_half_open() {
        let cfg = CircuitBreakerConfig {
            failure_threshold: 1,
            success_threshold: 1,
            timeout: Duration::from_millis(10),
        };
        let mut cb = CircuitBreaker::new(cfg);
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(cb.call_allowed(), CallResult::Allowed);
        assert_eq!(cb.state(), CircuitState::HalfOpen);
    }

    #[test]
    fn success_in_half_open_closes_circuit() {
        let cfg = CircuitBreakerConfig {
            failure_threshold: 1,
            success_threshold: 1,
            timeout: Duration::from_millis(10),
        };
        let mut cb = CircuitBreaker::new(cfg);
        cb.record_failure();
        std::thread::sleep(Duration::from_millis(20));
        let _ = cb.call_allowed(); // transitions to HalfOpen
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn failure_in_half_open_reopens_circuit() {
        let cfg = CircuitBreakerConfig {
            failure_threshold: 1,
            success_threshold: 2,
            timeout: Duration::from_millis(10),
        };
        let mut cb = CircuitBreaker::new(cfg);
        cb.record_failure();
        std::thread::sleep(Duration::from_millis(20));
        let _ = cb.call_allowed(); // transitions to HalfOpen
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn reset_returns_to_closed() {
        let mut cb = CircuitBreaker::new(default_config());
        for _ in 0..3 {
            cb.record_failure();
        }
        assert_eq!(cb.state(), CircuitState::Open);
        cb.reset();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.failure_count(), 0);
    }

    #[test]
    fn success_count_resets_on_failure() {
        let cfg = CircuitBreakerConfig {
            failure_threshold: 1,
            success_threshold: 3,
            timeout: Duration::from_millis(10),
        };
        let mut cb = CircuitBreaker::new(cfg);
        cb.record_failure(); // Open
        std::thread::sleep(Duration::from_millis(20));
        let _ = cb.call_allowed(); // HalfOpen
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::HalfOpen);
        cb.record_failure(); // back to Open
        assert_eq!(cb.state(), CircuitState::Open);

        // After re-entering HalfOpen, successes start from 0 again
        std::thread::sleep(Duration::from_millis(20));
        let _ = cb.call_allowed(); // HalfOpen again
        // Need all 3 successes to close — prior success doesn't carry over
        cb.record_success();
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::HalfOpen);
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn total_calls_tracked() {
        let mut cb = CircuitBreaker::new(default_config());
        assert_eq!(cb.total_calls(), 0);
        cb.call_allowed();
        cb.call_allowed();
        cb.call_allowed();
        assert_eq!(cb.total_calls(), 3);
    }

    #[test]
    fn rejection_rate_calculated() {
        let cfg = CircuitBreakerConfig {
            failure_threshold: 1,
            success_threshold: 1,
            timeout: Duration::from_secs(60),
        };
        let mut cb = CircuitBreaker::new(cfg);
        // 1 allowed call
        assert_eq!(cb.call_allowed(), CallResult::Allowed);
        cb.record_failure(); // opens the circuit
        // 1 rejected call
        assert_eq!(cb.call_allowed(), CallResult::Rejected);

        assert_eq!(cb.total_calls(), 2);
        assert_eq!(cb.total_rejections(), 1);
        let rate = cb.rejection_rate();
        assert!((rate - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn rejection_rate_zero_when_no_calls() {
        let cb = CircuitBreaker::new(default_config());
        assert!((cb.rejection_rate() - 0.0).abs() < f64::EPSILON);
    }
}
