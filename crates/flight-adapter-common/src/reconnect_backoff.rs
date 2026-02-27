// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Exponential reconnection backoff with jitter.
//!
//! Provides a stateful backoff calculator suitable for adapter reconnection
//! loops.  Supports configurable initial delay, maximum delay, multiplier,
//! and optional jitter.

use std::time::Duration;

/// Exponential backoff calculator with optional jitter.
#[derive(Debug, Clone)]
pub struct ExponentialBackoff {
    /// Delay returned on the first attempt.
    initial_delay: Duration,
    /// Upper bound for any computed delay.
    max_delay: Duration,
    /// Multiplier applied on each successive attempt (typically 2.0).
    multiplier: f64,
    /// Jitter factor in `[0.0, 1.0]`. A value of 0.25 adds up to ±25%.
    jitter: f64,
    /// Current attempt count (0-based internally).
    attempt: u32,
}

impl ExponentialBackoff {
    /// Create a new backoff calculator.
    ///
    /// # Panics
    /// Panics if `multiplier` is less than 1.0 or `jitter` is outside `[0.0, 1.0]`.
    pub fn new(initial_delay: Duration, max_delay: Duration, multiplier: f64, jitter: f64) -> Self {
        assert!(multiplier >= 1.0, "multiplier must be >= 1.0");
        assert!(
            (0.0..=1.0).contains(&jitter),
            "jitter must be in [0.0, 1.0]"
        );
        Self {
            initial_delay,
            max_delay,
            multiplier,
            jitter,
            attempt: 0,
        }
    }

    /// Compute the next backoff delay, advancing the internal attempt counter.
    pub fn next_delay(&mut self) -> Duration {
        let base_ms =
            self.initial_delay.as_millis() as f64 * self.multiplier.powi(self.attempt as i32);
        self.attempt = self.attempt.saturating_add(1);

        let jittered = self.apply_jitter(base_ms);
        let delay = Duration::from_millis(jittered as u64);
        delay.min(self.max_delay)
    }

    /// Reset the backoff counter (e.g. after a successful connection).
    pub fn reset(&mut self) {
        self.attempt = 0;
    }

    /// Current attempt number (0-based, incremented after each `next_delay`).
    pub fn attempt(&self) -> u32 {
        self.attempt
    }

    /// Apply jitter to a base delay in milliseconds.
    fn apply_jitter(&self, base_ms: f64) -> f64 {
        if self.jitter == 0.0 {
            return base_ms;
        }
        // Deterministic jitter based on attempt count for reproducibility in
        // tests.  A real deployment may replace this with an RNG.
        let hash = simple_hash(self.attempt);
        // Map hash to [-1.0, 1.0]
        let factor = (hash as f64 / u32::MAX as f64) * 2.0 - 1.0;
        let jitter_amount = base_ms * self.jitter * factor;
        (base_ms + jitter_amount).max(0.0)
    }
}

/// Simple deterministic hash for jitter seeding (not cryptographic).
fn simple_hash(n: u32) -> u32 {
    let mut x = n.wrapping_mul(2654435761);
    x ^= x >> 16;
    x = x.wrapping_mul(2246822519);
    x ^= x >> 13;
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_first_delay_equals_initial() {
        let mut b = ExponentialBackoff::new(
            Duration::from_millis(100),
            Duration::from_secs(10),
            2.0,
            0.0,
        );
        assert_eq!(b.next_delay(), Duration::from_millis(100));
    }

    #[test]
    fn test_exponential_growth() {
        let mut b = ExponentialBackoff::new(
            Duration::from_millis(100),
            Duration::from_secs(60),
            2.0,
            0.0,
        );
        assert_eq!(b.next_delay(), Duration::from_millis(100));
        assert_eq!(b.next_delay(), Duration::from_millis(200));
        assert_eq!(b.next_delay(), Duration::from_millis(400));
        assert_eq!(b.next_delay(), Duration::from_millis(800));
    }

    #[test]
    fn test_max_delay_cap() {
        let mut b = ExponentialBackoff::new(
            Duration::from_millis(100),
            Duration::from_millis(500),
            2.0,
            0.0,
        );
        // 100, 200, 400, 800 → capped to 500
        b.next_delay();
        b.next_delay();
        b.next_delay();
        let d = b.next_delay();
        assert_eq!(d, Duration::from_millis(500));
    }

    #[test]
    fn test_reset_restarts_sequence() {
        let mut b = ExponentialBackoff::new(
            Duration::from_millis(100),
            Duration::from_secs(10),
            2.0,
            0.0,
        );
        b.next_delay();
        b.next_delay();
        assert_eq!(b.attempt(), 2);
        b.reset();
        assert_eq!(b.attempt(), 0);
        assert_eq!(b.next_delay(), Duration::from_millis(100));
    }

    #[test]
    fn test_jitter_stays_within_bounds() {
        let mut b = ExponentialBackoff::new(
            Duration::from_millis(1000),
            Duration::from_secs(60),
            2.0,
            0.25,
        );
        for _ in 0..20 {
            let d = b.next_delay();
            // With 25% jitter the delay must not exceed max_delay
            assert!(d <= Duration::from_secs(60));
        }
    }

    #[test]
    fn test_jitter_zero_means_no_jitter() {
        let mut b1 = ExponentialBackoff::new(
            Duration::from_millis(200),
            Duration::from_secs(10),
            2.0,
            0.0,
        );
        let mut b2 = ExponentialBackoff::new(
            Duration::from_millis(200),
            Duration::from_secs(10),
            2.0,
            0.0,
        );
        for _ in 0..5 {
            assert_eq!(b1.next_delay(), b2.next_delay());
        }
    }

    #[test]
    #[should_panic(expected = "multiplier must be >= 1.0")]
    fn test_invalid_multiplier_panics() {
        ExponentialBackoff::new(Duration::from_millis(100), Duration::from_secs(1), 0.5, 0.0);
    }

    #[test]
    #[should_panic(expected = "jitter must be in [0.0, 1.0]")]
    fn test_invalid_jitter_panics() {
        ExponentialBackoff::new(Duration::from_millis(100), Duration::from_secs(1), 2.0, 1.5);
    }
}
